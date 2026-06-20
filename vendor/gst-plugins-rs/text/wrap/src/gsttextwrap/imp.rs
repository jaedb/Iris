// Copyright (C) 2020 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use std::default::Default;
use std::fs::File;
use std::io;
use std::mem;
use std::sync::Mutex;

use std::sync::LazyLock;

use hyphenation::{Load, Standard};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "textwrap",
        gst::DebugColorFlags::empty(),
        Some("Text wrapper element"),
    )
});

const DEFAULT_DICTIONARY: Option<String> = None;
const DEFAULT_COLUMNS: u32 = 32; /* CEA 608 max columns */
const DEFAULT_LINES: u32 = 0;
const DEFAULT_ACCUMULATE: gst::ClockTime = gst::ClockTime::ZERO;

#[derive(Debug, Clone)]
struct Settings {
    dictionary: Option<String>,
    columns: u32,
    lines: u32,
    accumulate_time: gst::ClockTime,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            dictionary: DEFAULT_DICTIONARY,
            columns: DEFAULT_COLUMNS, /* CEA 608 max columns */
            lines: DEFAULT_LINES,
            accumulate_time: DEFAULT_ACCUMULATE,
        }
    }
}

struct State {
    options: Option<textwrap::Options<'static>>,

    current_text: String,
    start_ts: Option<gst::ClockTime>,
    end_ts: Option<gst::ClockTime>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            options: None,

            current_text: "".to_string(),
            start_ts: None,
            end_ts: None,
        }
    }
}

pub struct TextWrap {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
    settings: Mutex<Settings>,
    state: Mutex<State>,
}

fn is_punctuation(word: &str) -> bool {
    word == "." || word == "," || word == "?" || word == "!" || word == ";" || word == ":"
}

impl TextWrap {
    fn update_wrapper(&self) {
        let settings = self.settings.lock().unwrap();
        let mut state = self.state.lock().unwrap();

        if state.options.is_some() {
            return;
        }

        let mut options = textwrap::Options::new(settings.columns as usize);

        if let Some(dictionary) = &settings.dictionary {
            let dict_file = match File::open(dictionary) {
                Err(err) => {
                    gst::error!(CAT, imp = self, "Failed to open dictionary file: {}", err);
                    return;
                }
                Ok(dict_file) => dict_file,
            };

            let mut reader = io::BufReader::new(dict_file);
            let standard = match Standard::any_from_reader(&mut reader) {
                Err(err) => {
                    gst::error!(
                        CAT,
                        imp = self,
                        "Failed to load standard from file: {}",
                        err
                    );
                    return;
                }
                Ok(standard) => standard,
            };

            options.word_splitter = textwrap::WordSplitter::Hyphenation(standard);
        } else {
            options.word_splitter = textwrap::WordSplitter::NoHyphenation;
        }

        state.options = Some(options);
    }

    fn try_drain(
        &self,
        state: &mut State,
        pts: gst::ClockTime,
        accumulate_time: gst::ClockTime,
        bufferlist: &mut gst::BufferList,
    ) {
        let add_buffer = state
            .start_ts
            .opt_add(accumulate_time)
            .opt_le(pts)
            .unwrap_or(false);

        if add_buffer {
            let drained = mem::take(&mut state.current_text);
            let duration = state.end_ts.opt_checked_sub(state.start_ts).ok().flatten();
            gst::debug!(
                CAT,
                imp = self,
                "Outputting contents {}, ts: {}, duration: {}",
                drained,
                state.start_ts.display(),
                duration.display(),
            );
            let mut buf = gst::Buffer::from_mut_slice(drained.into_bytes());
            {
                let buf_mut = buf.get_mut().unwrap();
                buf_mut.set_pts(state.start_ts);
                buf_mut.set_duration(duration);
            }
            bufferlist.get_mut().unwrap().add(buf);

            state.start_ts = None;
            state.end_ts = None;
        }
    }

    fn sink_chain(
        &self,
        _pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        self.update_wrapper();

        let mut pts = buffer.pts().ok_or_else(|| {
            gst::error!(CAT, imp = self, "Need timestamped buffers");
            gst::FlowError::Error
        })?;

        let duration = buffer.duration().ok_or_else(|| {
            gst::error!(CAT, imp = self, "Need buffers with duration");
            gst::FlowError::Error
        })?;

        let data = buffer.map_readable().map_err(|_| {
            gst::error!(CAT, imp = self, "Can't map buffer readable");
            gst::FlowError::Error
        })?;

        let data = std::str::from_utf8(&data).map_err(|err| {
            gst::error!(CAT, imp = self, "Can't decode utf8: {}", err);

            gst::FlowError::Error
        })?;

        if data.is_empty() {
            gst::trace!(CAT, imp = self, "processing gap {buffer:?}");
        } else {
            gst::debug!(CAT, imp = self, "processing {data} in {buffer:?}");
        }

        let accumulate_time = self.settings.lock().unwrap().accumulate_time;
        let mut state = self.state.lock().unwrap();

        if !accumulate_time.is_zero() {
            let mut bufferlist = gst::BufferList::new();
            let n_lines = std::cmp::max(self.settings.lock().unwrap().lines, 1);

            self.try_drain(&mut state, pts, accumulate_time, &mut bufferlist);

            let add_buffer = state
                .start_ts
                .opt_add(accumulate_time)
                .opt_lt(pts)
                .unwrap_or(false);

            if add_buffer {
                let drained = mem::take(&mut state.current_text);
                let duration = state.end_ts.opt_checked_sub(state.start_ts).ok().flatten();
                gst::info!(
                    CAT,
                    imp = self,
                    "Outputting contents {}, ts: {}, duration: {}",
                    drained,
                    state.start_ts.display(),
                    duration.display(),
                );
                let mut buf = gst::Buffer::from_mut_slice(drained.into_bytes());
                {
                    let buf_mut = buf.get_mut().unwrap();
                    buf_mut.set_pts(state.start_ts);
                    buf_mut.set_duration(duration);
                }
                bufferlist.get_mut().unwrap().add(buf);

                state.start_ts = None;
                state.end_ts = None;
            }

            let num_words = data.split_whitespace().count() as u64;
            let duration_per_word = (num_words != 0).then(|| duration / num_words);

            if state.start_ts.is_none() {
                state.start_ts = Some(pts);
            }

            state.end_ts = Some(pts);

            let words = data.split_ascii_whitespace();
            let mut current_text = state.current_text.to_string();

            for word in words {
                if !current_text.is_empty() && !is_punctuation(word) {
                    current_text.push(' ');
                }
                current_text.push_str(word);

                let options = state
                    .options
                    .as_ref()
                    .expect("We should have a wrapper by now");

                let lines = textwrap::wrap(&current_text, options);
                let mut chunks = lines.chunks(n_lines as usize).peekable();
                let mut trailing = "".to_string();

                while let Some(chunk) = chunks.next() {
                    if chunks.peek().is_none() {
                        trailing = chunk
                            .iter()
                            .map(|l| l.to_string())
                            .collect::<Vec<String>>()
                            .join("\n");
                    } else {
                        let duration = state.end_ts.opt_checked_sub(state.start_ts).ok().flatten();
                        let contents = chunk
                            .iter()
                            .map(|l| l.to_string())
                            .collect::<Vec<String>>()
                            .join("\n");
                        gst::info!(
                            CAT,
                            imp = self,
                            "Outputting contents {}, ts: {}, duration: {}",
                            contents.to_string(),
                            state.start_ts.display(),
                            duration.display(),
                        );
                        let mut buf = gst::Buffer::from_mut_slice(contents.into_bytes());
                        {
                            let buf_mut = buf.get_mut().unwrap();
                            buf_mut.set_pts(state.start_ts);
                            buf_mut.set_duration(duration);
                        }
                        bufferlist.get_mut().unwrap().add(buf);
                        state.start_ts = state.end_ts;
                    }
                }

                current_text = trailing;
                state.end_ts = state.end_ts.opt_add(duration_per_word);
            }

            state.current_text = current_text;
            state.end_ts = Some(pts + duration);

            if let Some(pts) = state.end_ts {
                self.try_drain(&mut state, pts, accumulate_time, &mut bufferlist);
            }

            if state.current_text.is_empty() {
                state.start_ts = None;
                state.end_ts = None;
            }

            drop(state);

            if bufferlist.is_empty() {
                Ok(gst::FlowSuccess::Ok)
            } else {
                self.srcpad.push_list(bufferlist)
            }
        } else {
            let lines = self.settings.lock().unwrap().lines;

            let data = {
                let options = state
                    .options
                    .as_ref()
                    .expect("We should have a wrapper by now");
                textwrap::fill(data, options)
            };

            gst::log!(CAT, imp = self, "fill result: {data}");

            // If the lines property was set, we want to split the result into buffers
            // of at most N lines. We compute the duration for each of those based on
            // the total number of words, and the number of words in each of the split-up
            // buffers.
            if lines > 0 {
                let mut bufferlist = gst::BufferList::new();
                let duration_per_word: gst::ClockTime =
                    duration / data.split_whitespace().count() as u64;

                for chunk in data.lines().collect::<Vec<&str>>().chunks(lines as usize) {
                    let data = chunk.join("\n");
                    let duration: gst::ClockTime =
                        duration_per_word * data.split_whitespace().count() as u64;
                    gst::info!(CAT, "Pushing lines {}", data);
                    let mut buf = gst::Buffer::from_mut_slice(data.into_bytes());

                    {
                        let buf = buf.get_mut().unwrap();

                        buf.set_pts(pts);
                        buf.set_duration(duration);
                        pts += duration;
                    }

                    bufferlist.get_mut().unwrap().add(buf);
                }

                drop(state);

                self.srcpad.push_list(bufferlist)
            } else {
                let mut buf = gst::Buffer::from_mut_slice(data.into_bytes());

                {
                    let buf = buf.get_mut().unwrap();

                    buf.set_pts(pts);
                    buf.set_duration(duration);
                }

                drop(state);

                self.srcpad.push(buf)
            }
        }
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        gst::log!(CAT, obj = pad, "Handling event {:?}", event);

        use gst::EventView;

        match event.view() {
            EventView::Gap(gap) => {
                let state = self.state.lock().unwrap();
                /* We are currently accumulating text, no need to forward the gap */
                if state.start_ts.is_some() {
                    let (pts, duration) = gap.get();

                    let mut gap_buffer = gst::Buffer::new();
                    {
                        let buf_mut = gap_buffer.get_mut().expect("reference should be exclusive");
                        buf_mut.set_pts(pts);
                        buf_mut.set_duration(duration);
                    }

                    drop(state);

                    let res = self.sink_chain(pad, gap_buffer);

                    if res != Ok(gst::FlowSuccess::Ok) {
                        gst::warning!(CAT, "Failed to process gap: {:?}", res);
                    }

                    true
                } else {
                    gst::Pad::event_default(pad, Some(&*self.obj()), event)
                }
            }
            EventView::FlushStart(_) => {
                let mut state = self.state.lock().unwrap();
                let options = state.options.take();
                *state = State::default();
                state.options = options;
                drop(state);
                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            EventView::Eos(_) => {
                let mut state = self.state.lock().unwrap();
                if !state.current_text.is_empty() {
                    let mut buf = gst::Buffer::from_mut_slice(
                        mem::take(&mut state.current_text).into_bytes(),
                    );
                    {
                        let buf_mut = buf.get_mut().unwrap();
                        buf_mut.set_pts(state.start_ts);
                        buf_mut.set_duration(
                            state.end_ts.opt_checked_sub(state.start_ts).ok().flatten(),
                        );
                    }

                    state.start_ts = None;
                    state.end_ts = None;

                    drop(state);
                    let _ = self.srcpad.push(buf);
                } else {
                    drop(state);
                }
                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }

    fn src_query(&self, pad: &gst::Pad, query: &mut gst::QueryRef) -> bool {
        use gst::QueryViewMut;

        gst::log!(CAT, obj = pad, "Handling query {:?}", query);

        match query.view_mut() {
            QueryViewMut::Latency(q) => {
                let mut peer_query = gst::query::Latency::new();

                let ret = self.sinkpad.peer_query(&mut peer_query);

                if ret {
                    let (live, min, _) = peer_query.result();
                    let our_latency: gst::ClockTime = self.settings.lock().unwrap().accumulate_time;
                    gst::info!(
                        CAT,
                        imp = self,
                        "Reporting our latency {} + {}",
                        our_latency,
                        min
                    );
                    q.set(live, our_latency + min, gst::ClockTime::NONE);
                }
                ret
            }
            _ => gst::Pad::query_default(pad, Some(&*self.obj()), query),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for TextWrap {
    const NAME: &'static str = "GstTextWrap";
    type Type = super::TextWrap;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                TextWrap::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |textwrap| textwrap.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                TextWrap::catch_panic_pad_function(
                    parent,
                    || false,
                    |textwrap| textwrap.sink_event(pad, event),
                )
            })
            .flags(gst::PadFlags::PROXY_CAPS | gst::PadFlags::FIXED_CAPS)
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::Pad::builder_from_template(&templ)
            .query_function(|pad, parent, query| {
                TextWrap::catch_panic_pad_function(
                    parent,
                    || false,
                    |textwrap| textwrap.src_query(pad, query),
                )
            })
            .flags(gst::PadFlags::PROXY_CAPS | gst::PadFlags::FIXED_CAPS)
            .build();

        let settings = Mutex::new(Settings::default());
        let state = Mutex::new(State::default());

        Self {
            srcpad,
            sinkpad,
            settings,
            state,
        }
    }
}

impl ObjectImpl for TextWrap {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> =
            LazyLock::new(|| {
                vec![
                glib::ParamSpecString::builder("dictionary")
                    .nick("Dictionary")
                    .blurb("Path to a dictionary to load at runtime to perform hyphenation, see \
                        <https://docs.rs/crate/hyphenation/0.7.1> for more information")
                    .mutable_playing()
                    .build(),
                glib::ParamSpecUInt::builder("columns")
                    .nick("Columns")
                    .blurb("Maximum number of columns for any given line")
                    .minimum(1)
                    .default_value(DEFAULT_COLUMNS)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecUInt::builder("lines")
                    .nick("Lines")
                    .blurb("Split input buffer into output buffers with max lines (0=do not split)")
                    .default_value(DEFAULT_LINES)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecUInt64::builder("accumulate-time")
                    .nick("accumulate-time")
                    .blurb("Cut-off time for input text accumulation (0=do not accumulate)")
                    .maximum(u64::MAX - 1)
                    .default_value(DEFAULT_ACCUMULATE.nseconds())
                    .mutable_playing()
                    .build(),
            ]
            });

        PROPERTIES.as_ref()
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "dictionary" => {
                let mut settings = self.settings.lock().unwrap();
                let mut state = self.state.lock().unwrap();
                settings.dictionary = value.get().expect("type checked upstream");
                state.options = None;
            }
            "columns" => {
                let mut settings = self.settings.lock().unwrap();
                let mut state = self.state.lock().unwrap();
                settings.columns = value.get().expect("type checked upstream");
                state.options = None;
            }
            "lines" => {
                let mut settings = self.settings.lock().unwrap();
                settings.lines = value.get().expect("type checked upstream");
            }
            "accumulate-time" => {
                let mut settings = self.settings.lock().unwrap();
                let old_accumulate_time = settings.accumulate_time;
                settings.accumulate_time = value.get::<u64>().unwrap().nseconds();
                if settings.accumulate_time != old_accumulate_time {
                    gst::debug!(
                        CAT,
                        imp = self,
                        "Accumulate time changed: {}",
                        settings.accumulate_time.display(),
                    );
                    drop(settings);
                    let _ = self
                        .obj()
                        .post_message(gst::message::Latency::builder().src(&*self.obj()).build());
                }
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "dictionary" => {
                let settings = self.settings.lock().unwrap();
                settings.dictionary.to_value()
            }
            "columns" => {
                let settings = self.settings.lock().unwrap();
                settings.columns.to_value()
            }
            "lines" => {
                let settings = self.settings.lock().unwrap();
                settings.lines.to_value()
            }
            "accumulate-time" => {
                let settings = self.settings.lock().unwrap();
                settings.accumulate_time.nseconds().to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for TextWrap {}

impl ElementImpl for TextWrap {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Text Wrapper",
                "Text/Filter",
                "Breaks text into fixed-size lines, with optional hyphenation",
                "Mathieu Duponchelle <mathieu@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::builder("text/x-raw")
                .field("format", "utf8")
                .build();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst::info!(CAT, imp = self, "Changing state {:?}", transition);

        if let gst::StateChange::PausedToReady = transition {
            let mut state = self.state.lock().unwrap();
            *state = State::default();
        }

        let success = self.parent_change_state(transition)?;

        Ok(success)
    }
}
