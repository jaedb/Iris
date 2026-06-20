// Copyright (C) 2023 François Laignel <francois@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::subclass::prelude::*;

use aws_sdk_translate as aws_translate;
use aws_sdk_translate::error::ProvideErrorMetadata;

use futures::channel::mpsc;
use futures::prelude::*;

use std::collections::VecDeque;
use std::sync::Arc;

use super::imp::TranslateSrcPad;
use super::transcribe::TranscriptItem;
use super::{TranslationTokenizationMethod, CAT};

const SPAN_START: &str = "<span>";
const SPAN_END: &str = "</span>";

#[derive(Debug)]
pub struct TranslatedItem {
    pub pts: gst::ClockTime,
    pub duration: gst::ClockTime,
    pub content: String,
}

impl From<&TranscriptItem> for TranslatedItem {
    fn from(transcript_item: &TranscriptItem) -> Self {
        TranslatedItem {
            pts: transcript_item.pts,
            duration: transcript_item.duration,
            content: transcript_item.content.clone(),
        }
    }
}

#[derive(Debug)]
pub struct Translation {
    pub items: Vec<TranslatedItem>,
    pub translation: String,
}

pub struct TranslateLoop {
    pad: glib::subclass::ObjectImplRef<TranslateSrcPad>,
    client: aws_translate::Client,
    input_lang: String,
    output_lang: String,
    tokenization_method: TranslationTokenizationMethod,
    transcript_rx: mpsc::Receiver<Arc<Vec<TranscriptItem>>>,
    translate_tx: mpsc::Sender<Translation>,
}

impl TranslateLoop {
    pub fn new(
        imp: &super::imp::Transcriber,
        pad: &TranslateSrcPad,
        input_lang: &str,
        output_lang: &str,
        tokenization_method: TranslationTokenizationMethod,
        transcript_rx: mpsc::Receiver<Arc<Vec<TranscriptItem>>>,
        translate_tx: mpsc::Sender<Translation>,
    ) -> Self {
        let aws_config = imp.aws_config.lock().unwrap();
        let aws_config = aws_config
            .as_ref()
            .expect("aws_config must be initialized at this stage");

        TranslateLoop {
            pad: pad.ref_counted(),
            client: aws_sdk_translate::Client::new(aws_config),
            input_lang: input_lang.to_string(),
            output_lang: output_lang.to_string(),
            tokenization_method,
            transcript_rx,
            translate_tx,
        }
    }

    pub async fn check_language(&self) -> Result<(), gst::ErrorMessage> {
        let language_list = self.client.list_languages().send().await.map_err(|err| {
            let err = format!(
                "Failed to call list_languages service: {err}: {}",
                err.meta()
            );
            gst::info!(CAT, imp = self.pad, "{err}");
            gst::error_msg!(gst::LibraryError::Failed, ["{err}"])
        })?;

        let found_output_lang = language_list
            .languages()
            .iter()
            .any(|lang| lang.language_code() == self.output_lang);

        if !found_output_lang {
            let err = format!("Unknown output languages: {}", self.output_lang);
            gst::info!(CAT, imp = self.pad, "{err}");
            return Err(gst::error_msg!(gst::LibraryError::Failed, ["{err}"]));
        }

        Ok(())
    }

    pub async fn run(mut self) -> Result<(), gst::ErrorMessage> {
        use TranslationTokenizationMethod as Tokenization;

        while let Some(transcript_items) = self.transcript_rx.next().await {
            if transcript_items.is_empty() {
                continue;
            }

            let mut ts_duration_list: Vec<(gst::ClockTime, gst::ClockTime)> = vec![];
            let mut content: Vec<String> = vec![];

            let mut it = transcript_items.iter().peekable();

            while let Some(item) = it.next() {
                let suffix = match it.peek() {
                    Some(next_item) => {
                        if next_item.is_punctuation {
                            ""
                        } else {
                            " "
                        }
                    }
                    None => "",
                };
                ts_duration_list.push((item.pts, item.duration));
                content.push(match self.tokenization_method {
                    Tokenization::None => format!("{}{}", item.content, suffix),
                    Tokenization::SpanBased => {
                        format!("{SPAN_START}{}{SPAN_END}{}", item.content, suffix)
                    }
                });
            }

            let content: String = content.join("");

            gst::debug!(
                CAT,
                imp = self.pad,
                "Translating {content} with {ts_duration_list:?}"
            );

            let translated_text = self
                .client
                .translate_text()
                .set_source_language_code(Some(self.input_lang.clone()))
                .set_target_language_code(Some(self.output_lang.clone()))
                .set_text(Some(content))
                .send()
                .await
                .map_err(|err| {
                    let err = format!("Failed to call translation service: {err}: {}", err.meta());
                    gst::info!(CAT, imp = self.pad, "{err}");
                    gst::error_msg!(gst::LibraryError::Failed, ["{err}"])
                })?
                .translated_text;

            gst::debug!(CAT, imp = self.pad, "Got translation {translated_text}");

            let translated_items = match self.tokenization_method {
                Tokenization::None => {
                    // Push translation as a single item
                    let mut ts_duration_iter = ts_duration_list.into_iter().peekable();

                    let &(first_pts, _) = ts_duration_iter.peek().expect("at least one item");
                    let (last_pts, last_duration) =
                        ts_duration_iter.last().expect("at least one item");

                    vec![TranslatedItem {
                        pts: first_pts,
                        duration: last_pts.saturating_sub(first_pts) + last_duration,
                        content: translated_text.clone(),
                    }]
                }
                Tokenization::SpanBased => span_tokenize_items(&translated_text, ts_duration_list),
            };

            gst::trace!(CAT, imp = self.pad, "Sending {translated_items:?}");

            if self
                .translate_tx
                .send(Translation {
                    items: translated_items,
                    translation: translated_text,
                })
                .await
                .is_err()
            {
                gst::info!(
                    CAT,
                    imp = self.pad,
                    "translation chan terminated, exiting translation loop"
                );
                break;
            }
        }

        Ok(())
    }
}

/// Parses translated items from the `translation` `String` using `span` tags.
///
/// The `translation` is expected to have been returned by the `Translate` ws.
/// It can contain id-less `<span>` and `</span>` tags, matching similar
/// id-less tags from the content submitted to the `Translate` ws.
///
/// This parser accepts both serial `<span></span>` as well as nested
/// `<span><span></span></span>`.
///
/// The parsed items are assigned the ts and duration from `ts_duration_list`
/// in their order of appearance.
///
/// If more parsed items are found, the last item will concatenate the remaining items.
///
/// If less parsed items are found, the last item will be assign the remaining
/// duration from the `ts_duration_list`.
pub fn span_tokenize_items(
    translation: &str,
    ts_duration_list: impl IntoIterator<Item = (gst::ClockTime, gst::ClockTime)>,
) -> Vec<TranslatedItem> {
    const SPAN_START_LEN: usize = SPAN_START.len();
    const SPAN_END_LEN: usize = SPAN_END.len();

    let mut translated_items = vec![];

    let mut ts_duration_iter = ts_duration_list.into_iter();

    // Content for a translated item
    let mut content = String::new();

    // Alleged span chunk
    let mut chunk = String::new();

    for c in translation.chars() {
        if content.is_empty() && c.is_whitespace() {
            // ignore leading whitespaces
            continue;
        }

        if chunk.is_empty() {
            if c == '<' {
                // Start an alleged span chunk
                chunk.push(c);
            } else {
                content.push(c);
            }

            continue;
        }

        chunk.push(c);

        match chunk.len() {
            len if len < SPAN_START_LEN => continue,
            SPAN_START_LEN => {
                if chunk != SPAN_START {
                    continue;
                }
                // Got a <span>
            }
            SPAN_END_LEN => {
                if chunk != SPAN_END {
                    continue;
                }
                // Got a </span>
            }
            _ => {
                // Can no longer be a span
                content.extend(chunk.drain(..));
                continue;
            }
        }

        // got a span
        chunk.clear();

        if content.is_empty() {
            continue;
        }

        // Add pending content
        // assign it the next pts and duration from the input list
        if let Some((pts, duration)) = ts_duration_iter.next() {
            translated_items.push(TranslatedItem {
                pts,
                duration,
                content: content.trim().to_string(),
            });

            content = String::new();
        } else if let Some(last_item) = translated_items.last_mut() {
            // exhausted available pts and duration
            // add content to last item
            let starts_with_punctuation = content.starts_with(|c: char| c.is_ascii_punctuation());

            if !starts_with_punctuation {
                last_item.content.push(' ');
            }

            last_item.content.push_str(content.trim());

            content = String::new();
        }
    }

    content.extend(chunk.drain(..));

    if !content.is_empty() {
        // Add last content
        if let Some((pts, mut duration)) = ts_duration_iter.next() {
            if let Some((last_pts, last_duration)) = ts_duration_iter.last() {
                // Fix remaining duration
                duration = last_pts.saturating_sub(pts) + last_duration;
            }

            translated_items.push(TranslatedItem {
                pts,
                duration,
                content: content.trim().to_string(),
            });
        } else if let Some(last_item) = translated_items.last_mut() {
            // No more pts and duration in the index
            // Add remaining content to the last item pushed
            let starts_with_punctuation = content.starts_with(|c: char| c.is_ascii_punctuation());
            if !starts_with_punctuation {
                last_item.content.push(' ');
            }
            last_item.content.push_str(content.trim());
        }
    } else if let Some((last_pts, last_duration)) = ts_duration_iter.last() {
        if let Some(last_item) = translated_items.last_mut() {
            // No more content, but need to fix last item's duration
            last_item.duration = last_pts.saturating_sub(last_item.pts) + last_duration;
        }
    }

    let mut consolidated_items: VecDeque<TranslatedItem> = VecDeque::new();
    let mut consolidate = false;

    for item in translated_items.drain(..) {
        if consolidate {
            let last_item = consolidated_items.back_mut().unwrap();
            last_item.duration = item.pts + item.duration - last_item.pts;
            last_item.content += &item.content;
            consolidate = false;
            continue;
        }
        if item.content.ends_with("'") || item.content.ends_with("’") {
            consolidate = true;
        }
        consolidated_items.push_back(item);
    }

    consolidated_items.into()
}

#[cfg(test)]
mod tests {
    use super::span_tokenize_items;
    use gst::prelude::*;

    #[test]
    fn serial_spans() {
        let input = "<span>first</span> <span>second</span> <span>third</span>";
        let ts_duration_list = vec![
            (0.seconds(), 1.seconds()),
            (1.seconds(), 2.seconds()),
            (4.seconds(), 3.seconds()),
        ];

        let mut items = span_tokenize_items(input, ts_duration_list).into_iter();

        let first = items.next().unwrap();
        assert_eq!(first.pts, 0.seconds());
        assert_eq!(first.duration, 1.seconds());
        assert_eq!(first.content, "first");

        let second = items.next().unwrap();
        assert_eq!(second.pts, 1.seconds());
        assert_eq!(second.duration, 2.seconds());
        assert_eq!(second.content, "second");

        let third = items.next().unwrap();
        assert_eq!(third.pts, 4.seconds());
        assert_eq!(third.duration, 3.seconds());
        assert_eq!(third.content, "third");

        assert!(items.next().is_none());
    }

    #[test]
    fn serial_and_nested_spans() {
        let input = "<span>first</span> <span>second <span>third</span></span> <span>fourth</span>";
        let ts_duration_list = vec![
            (0.seconds(), 1.seconds()),
            (1.seconds(), 2.seconds()),
            (3.seconds(), 1.seconds()),
            (4.seconds(), 2.seconds()),
        ];

        let mut items = span_tokenize_items(input, ts_duration_list).into_iter();

        let first = items.next().unwrap();
        assert_eq!(first.pts, 0.seconds());
        assert_eq!(first.duration, 1.seconds());
        assert_eq!(first.content, "first");

        let second = items.next().unwrap();
        assert_eq!(second.pts, 1.seconds());
        assert_eq!(second.duration, 2.seconds());
        assert_eq!(second.content, "second");

        let third = items.next().unwrap();
        assert_eq!(third.pts, 3.seconds());
        assert_eq!(third.duration, 1.seconds());
        assert_eq!(third.content, "third");

        let fourth = items.next().unwrap();
        assert_eq!(fourth.pts, 4.seconds());
        assert_eq!(fourth.duration, 2.seconds());
        assert_eq!(fourth.content, "fourth");

        assert!(items.next().is_none());
    }

    #[test]
    fn nonspaned_serial_and_nested_spans() {
        let input = "Initial <span>first</span> <span>second <span>third</span></span> <span>fourth</span> final";
        let ts_duration_list = vec![
            (0.seconds(), 1.seconds()),
            (1.seconds(), 1.seconds()),
            (2.seconds(), 1.seconds()),
            (3.seconds(), 1.seconds()),
            (4.seconds(), 1.seconds()),
            (5.seconds(), 1.seconds()),
        ];

        let mut items = span_tokenize_items(input, ts_duration_list).into_iter();

        let init = items.next().unwrap();
        assert_eq!(init.pts, 0.seconds());
        assert_eq!(init.duration, 1.seconds());
        assert_eq!(init.content, "Initial");

        let first = items.next().unwrap();
        assert_eq!(first.pts, 1.seconds());
        assert_eq!(first.duration, 1.seconds());
        assert_eq!(first.content, "first");

        let second = items.next().unwrap();
        assert_eq!(second.pts, 2.seconds());
        assert_eq!(second.duration, 1.seconds());
        assert_eq!(second.content, "second");

        let third = items.next().unwrap();
        assert_eq!(third.pts, 3.seconds());
        assert_eq!(third.duration, 1.seconds());
        assert_eq!(third.content, "third");

        let fourth = items.next().unwrap();
        assert_eq!(fourth.pts, 4.seconds());
        assert_eq!(fourth.duration, 1.seconds());
        assert_eq!(fourth.content, "fourth");

        let final_ = items.next().unwrap();
        assert_eq!(final_.pts, 5.seconds());
        assert_eq!(final_.duration, 1.seconds());
        assert_eq!(final_.content, "final");

        assert!(items.next().is_none());
    }

    #[test]
    fn more_parsed_items() {
        let input = "<span>first</span> <span>second</span> <span>third</span> <span>fourth</span>";
        let ts_duration_list = vec![
            (0.seconds(), 1.seconds()),
            (1.seconds(), 2.seconds()),
            (4.seconds(), 3.seconds()),
        ];

        let mut items = span_tokenize_items(input, ts_duration_list).into_iter();

        let first = items.next().unwrap();
        assert_eq!(first.pts, 0.seconds());
        assert_eq!(first.duration, 1.seconds());
        assert_eq!(first.content, "first");

        let second = items.next().unwrap();
        assert_eq!(second.pts, 1.seconds());
        assert_eq!(second.duration, 2.seconds());
        assert_eq!(second.content, "second");

        let third = items.next().unwrap();
        assert_eq!(third.pts, 4.seconds());
        assert_eq!(third.duration, 3.seconds());
        assert_eq!(third.content, "third fourth");

        assert!(items.next().is_none());
    }

    #[test]
    fn more_parsed_items_nonspan_final() {
        let input = "<span>first</span> <span>second</span> <span>third</span> final";
        let ts_duration_list = vec![
            (0.seconds(), 1.seconds()),
            (1.seconds(), 2.seconds()),
            (4.seconds(), 3.seconds()),
        ];

        let mut items = span_tokenize_items(input, ts_duration_list).into_iter();

        let first = items.next().unwrap();
        assert_eq!(first.pts, 0.seconds());
        assert_eq!(first.duration, 1.seconds());
        assert_eq!(first.content, "first");

        let second = items.next().unwrap();
        assert_eq!(second.pts, 1.seconds());
        assert_eq!(second.duration, 2.seconds());
        assert_eq!(second.content, "second");

        let third = items.next().unwrap();
        assert_eq!(third.pts, 4.seconds());
        assert_eq!(third.duration, 3.seconds());
        assert_eq!(third.content, "third final");

        assert!(items.next().is_none());
    }

    #[test]
    fn less_parsed_items() {
        let input = "<span>first</span> <span>second</span>";
        let ts_duration_list = vec![
            (0.seconds(), 1.seconds()),
            (1.seconds(), 2.seconds()),
            (4.seconds(), 3.seconds()),
        ];

        let mut items = span_tokenize_items(input, ts_duration_list).into_iter();

        let first = items.next().unwrap();
        assert_eq!(first.pts, 0.seconds());
        assert_eq!(first.duration, 1.seconds());
        assert_eq!(first.content, "first");

        let second = items.next().unwrap();
        assert_eq!(second.pts, 1.seconds());
        assert_eq!(second.duration, 6.seconds());
        assert_eq!(second.content, "second");

        assert!(items.next().is_none());
    }

    #[test]
    fn less_parsed_items_nonspan_final() {
        let input = "<span>first</span> final";
        let ts_duration_list = vec![
            (0.seconds(), 1.seconds()),
            (1.seconds(), 2.seconds()),
            (4.seconds(), 3.seconds()),
        ];

        let mut items = span_tokenize_items(input, ts_duration_list).into_iter();

        let first = items.next().unwrap();
        assert_eq!(first.pts, 0.seconds());
        assert_eq!(first.duration, 1.seconds());
        assert_eq!(first.content, "first");

        let final_ = items.next().unwrap();
        assert_eq!(final_.pts, 1.seconds());
        assert_eq!(final_.duration, 6.seconds());
        assert_eq!(final_.content, "final");

        assert!(items.next().is_none());
    }

    #[test]
    fn utf8_input() {
        let input = "caractères accentués";
        let ts_duration_list = vec![(0.seconds(), 1.seconds())];

        let mut items = span_tokenize_items(input, ts_duration_list).into_iter();

        let first = items.next().unwrap();
        assert_eq!(first.pts, 0.seconds());
        assert_eq!(first.duration, 1.seconds());
        assert_eq!(first.content, "caractères accentués");

        assert!(items.next().is_none());
    }

    #[test]
    fn exhausted_spans_join_punctuation() {
        let input = "<span>et</span> <span><span>les</span> <span>Clippers</span> <span>sont</span> <span><span>au</span></span> <span>tableau</span><span>,</span> <span>et</span> <span>c'est <span>Norman</span> qui</span> <span>attaque</span> en <span>lisant</span> <span>Max <span>Christie</span>.</span></span>";

        let ts_duration_list = vec![
            (0.seconds(), 1.seconds()),
            (1.seconds(), 1.seconds()),
            (2.seconds(), 1.seconds()),
            (3.seconds(), 1.seconds()),
            (4.seconds(), 1.seconds()),
            (5.seconds(), 1.seconds()),
            (6.seconds(), 1.seconds()),
            (7.seconds(), 1.seconds()),
            (8.seconds(), 1.seconds()),
            (9.seconds(), 1.seconds()),
            (10.seconds(), 1.seconds()),
            (11.seconds(), 1.seconds()),
            (12.seconds(), 1.seconds()),
            (13.seconds(), 1.seconds()),
            (14.seconds(), 1.seconds()),
            (15.seconds(), 1.seconds()),
        ];

        let mut items = span_tokenize_items(input, ts_duration_list).into_iter();

        let final_ = items.next_back().unwrap();

        // when all spans are consumed and punctuation remains as the content,
        // don't join it with a space with the last item content (Christie .)
        assert!(final_.content == "Christie.");
    }
}
