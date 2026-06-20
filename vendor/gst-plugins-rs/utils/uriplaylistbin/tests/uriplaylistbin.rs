// Copyright (C) 2021 OneStream Live <guillaume.desmottes@onestream.live>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    path::PathBuf,
    sync::{atomic::AtomicI32, Arc},
};

use gst::prelude::*;
use gst::MessageView;
use more_asserts::assert_ge;

struct TestMedia {
    uri: String,
    len: gst::ClockTime,
}

impl PartialEq for TestMedia {
    fn eq(&self, other: &Self) -> bool {
        self.uri == other.uri
    }
}

fn file_name_to_uri(name: &str) -> String {
    let input_path = {
        let mut r = PathBuf::new();
        r.push(env!("CARGO_MANIFEST_DIR"));
        r.push("tests");
        r.push(name);
        r
    };

    let url = url::Url::from_file_path(input_path).unwrap();
    url.to_string()
}

impl TestMedia {
    fn ogg() -> Self {
        Self {
            uri: file_name_to_uri("sample.ogg"),
            len: 510.mseconds(),
        }
    }

    fn mkv_http() -> Self {
        Self {
            uri: "https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/raw/main/utils/uriplaylistbin/tests/sample.mkv?ref_type=heads&inline=false".to_string(),
            len: 510.mseconds(),
        }
    }

    fn mkv() -> Self {
        Self {
            uri: file_name_to_uri("sample.mkv"),
            len: 510.mseconds(),
        }
    }

    fn missing_file() -> Self {
        Self {
            uri: "file://not-there.ogg".to_string(),
            len: 10.mseconds(),
        }
    }

    fn missing_http() -> Self {
        Self {
            uri: "http://not-there.ogg".to_string(),
            len: 10.mseconds(),
        }
    }
}

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gsturiplaylistbin::plugin_register_static()
            .expect("Failed to register uriplaylistbin plugin");
    });
}

struct IterationsChange {
    /// change the uriplaylistbin iterations property when receiving the nth stream-start event
    when_ss: u32,
    /// new 'iterations' value
    iterations: u32,
}

struct Pipeline(gst::Pipeline);

impl std::ops::Deref for Pipeline {
    type Target = gst::Pipeline;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        let _ = self.0.set_state(gst::State::Null);
    }
}

fn test(
    medias: Vec<TestMedia>,
    n_streams: u32,
    iterations: u32,
    check_streams: bool,
    iterations_change: Option<IterationsChange>,
    cache: bool,
) -> (Vec<gst::Message>, u32, u64, bool) {
    init();

    let total_len: gst::ClockTime = medias.iter().map(|t| t.len * (iterations as u64)).sum();

    let uris: Vec<String> = medias.iter().map(|t| t.uri.clone()).collect();

    // create a temp directory to store the cache
    let cache_dir =
        cache.then(|| tempfile::tempdir().expect("failed to create temp cache directory"));

    let pipeline = Pipeline(gst::Pipeline::default());
    let playlist = gst::ElementFactory::make("uriplaylistbin")
        .property("uris", &uris)
        .property("iterations", iterations)
        .property("cache", cache)
        .property(
            "cache-dir",
            cache_dir.as_ref().map(|dir| dir.path().to_str().unwrap()),
        )
        .build()
        .unwrap();
    let mq = gst::ElementFactory::make("multiqueue").build().unwrap();

    pipeline.add_many([&playlist, &mq]).unwrap();

    assert_eq!(playlist.property::<u32>("current-iteration"), 0);
    assert_eq!(playlist.property::<u64>("current-uri-index"), 0);

    let mq_clone = mq.clone();
    playlist.connect_pad_added(move |_playlist, src_pad| {
        let mq_sink = mq_clone.request_pad_simple("sink_%u").unwrap();
        src_pad.link(&mq_sink).unwrap();
    });

    let eos_count = Arc::new(AtomicI32::new(0));

    let pipeline_weak = pipeline.downgrade();
    let eos_count_clone = eos_count.clone();
    mq.connect_pad_added(move |_mq, pad| {
        if pad.direction() != gst::PadDirection::Src {
            return;
        }

        let Some(pipeline) = pipeline_weak.upgrade() else {
            return;
        };

        let sink = gst::ElementFactory::make("fakesink")
            .property("sync", true)
            .build()
            .unwrap();
        pipeline.add(&sink).unwrap();
        sink.sync_state_with_parent().unwrap();

        pad.link(&sink.static_pad("sink").unwrap()).unwrap();

        let eos_count_clone = eos_count_clone.clone();
        pad.add_probe(
            gst::PadProbeType::EVENT_DOWNSTREAM,
            move |_pad, info| match info.data {
                Some(gst::PadProbeData::Event(ref ev)) if ev.type_() == gst::EventType::Eos => {
                    eos_count_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    gst::PadProbeReturn::Remove
                }
                _ => gst::PadProbeReturn::Ok,
            },
        );
    });

    pipeline.set_state(gst::State::Playing).unwrap();

    let bus = pipeline.bus().unwrap();
    let mut events = vec![];
    let mut n_stream_start = 0;

    loop {
        let msg = bus.iter_timed(gst::ClockTime::from_mseconds(100)).next();

        // Wait for each stream to EOS rather than rely on the EOS message.
        // Prevent races as our test files are really short, see https://gitlab.freedesktop.org/gstreamer/gstreamer/-/issues/4204
        if eos_count.load(std::sync::atomic::Ordering::Relaxed) == n_streams as i32 {
            // all streams EOS
            break;
        }

        let Some(msg) = msg else {
            continue;
        };

        match msg.view() {
            MessageView::Error(_) => {
                events.push(msg.clone());
                break;
            }
            // check stream related messages
            MessageView::StreamCollection(sc) => {
                if let Some(prev) = events.last() {
                    if let MessageView::StreamCollection(prev_sc) = prev.view() {
                        if prev_sc.src() == sc.src()
                            && prev_sc.stream_collection() == sc.stream_collection()
                        {
                            // decodebin3 may send twice the same collection
                            continue;
                        }
                    }
                }

                events.push(msg.clone())
            }
            MessageView::StreamsSelected(_) => events.push(msg.clone()),
            MessageView::StreamStart(_) => {
                n_stream_start += 1;
                if let Some(change) = &iterations_change {
                    if change.when_ss == n_stream_start {
                        playlist.set_property("iterations", change.iterations);
                    }
                }
            }
            _ => {}
        }
    }

    // check we actually played all files and all streams
    fn stream_end_ts(sink: &gst::Element) -> gst::ClockTime {
        let sample: gst::Sample = sink.property("last-sample");
        let buffer = sample.buffer().unwrap();
        let pts = buffer.pts().unwrap();
        let segment = sample.segment().unwrap();
        let segment = segment.downcast_ref::<gst::ClockTime>().unwrap();
        let rt = segment.to_running_time(pts).unwrap();

        rt + buffer.duration().unwrap()
    }

    if check_streams {
        // check all streams have been fully played
        let mut n = 0;
        for sink in pipeline.iterate_sinks() {
            let sink = sink.unwrap();
            assert_ge!(
                stream_end_ts(&sink),
                total_len,
                "{}: {} < {}",
                sink.name(),
                stream_end_ts(&sink),
                total_len
            );
            n += 1;
        }
        assert_eq!(n, n_streams);

        // check stream-collection and streams-selected message ordering
        let mut events = events.clone().into_iter();
        let playlist = std::iter::repeat_n(medias.iter(), iterations as usize).flatten();
        let mut last_media = None;

        for media in playlist {
            let mut media_changed = false;

            if last_media
                .as_ref()
                .is_none_or(|last_media| *last_media != media)
            {
                last_media = Some(media);
                media_changed = true;
            }

            // decodebin3 only sends a new stream-collection and streams-selected if it actually
            // changes, which only happens here if the actual underlying media is changing.
            if media_changed {
                let decodebin =
                    assert_stream_collection(events.next().unwrap(), n_streams as usize);
                assert_eq!(
                    assert_stream_selected(events.next().unwrap(), n_streams as usize),
                    decodebin
                );
            }
        }
    }

    if let Some(cache_dir) = cache_dir {
        let dir = std::fs::read_dir(cache_dir.path()).expect("failed to read cache dir");
        // all items should have been cached if we looped the playlist
        let n_cached_files = if iterations > 1 { uris.len() } else { 0 };
        assert_eq!(dir.count(), n_cached_files);
    }

    let current_iteration = playlist.property::<u32>("current-iteration");
    let current_uri_index = playlist.property::<u64>("current-uri-index");
    let eos = eos_count.load(std::sync::atomic::Ordering::Relaxed) == n_streams as i32;

    pipeline.set_state(gst::State::Null).unwrap();

    (events, current_iteration, current_uri_index, eos)
}

#[track_caller]
fn assert_error(msg: gst::Message, failing: TestMedia) {
    match msg.view() {
        MessageView::Error(err) => {
            let details = err.details().unwrap();
            assert_eq!(details.get::<&str>("uri").unwrap(), failing.uri);
        }
        _ => {
            panic!("last message is not an error");
        }
    }
}

#[track_caller]
fn assert_stream_collection(msg: gst::Message, n_streams: usize) -> gst::Object {
    match msg.view() {
        MessageView::StreamCollection(sc) => {
            let collection = sc.stream_collection();
            assert_eq!(collection.len(), n_streams);
            sc.src().unwrap().clone()
        }
        _ => {
            panic!("message is not a stream collection");
        }
    }
}

#[track_caller]
fn assert_stream_selected(msg: gst::Message, n_streams: usize) -> gst::Object {
    match msg.view() {
        MessageView::StreamsSelected(ss) => {
            let collection = ss.stream_collection();
            assert_eq!(collection.len(), n_streams);
            ss.src().unwrap().clone()
        }
        _ => {
            panic!("message is not stream selected");
        }
    }
}

#[test]
// FIXME: racy: https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/issues/514
#[ignore]
fn single_audio() {
    let (_events, current_iteration, current_uri_index, eos) =
        test(vec![TestMedia::ogg()], 1, 1, true, None, false);
    assert!(eos);
    assert_eq!(current_iteration, 0);
    assert_eq!(current_uri_index, 0);
}

#[test]
// FIXME: racy: https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/issues/514
#[ignore]
fn single_video() {
    let (_events, current_iteration, current_uri_index, eos) =
        test(vec![TestMedia::mkv()], 2, 1, true, None, false);
    assert!(eos);
    assert_eq!(current_iteration, 0);
    assert_eq!(current_uri_index, 0);
}

#[test]
// FIXME: racy: https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/issues/514
#[ignore]
fn multi_audio() {
    let (_events, current_iteration, current_uri_index, eos) = test(
        vec![TestMedia::ogg(), TestMedia::ogg(), TestMedia::ogg()],
        1,
        1,
        true,
        None,
        false,
    );
    assert!(eos);
    assert_eq!(current_iteration, 0);
    assert_eq!(current_uri_index, 2);
}

#[test]
// FIXME: racy: https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/issues/514
#[ignore]
fn multi_audio_video() {
    let (_events, current_iteration, current_uri_index, eos) = test(
        vec![TestMedia::mkv(), TestMedia::mkv()],
        2,
        1,
        true,
        None,
        false,
    );
    assert!(eos);
    assert_eq!(current_iteration, 0);
    assert_eq!(current_uri_index, 1);
}

#[test]
// FIXME: racy: https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/issues/514
#[ignore]
fn iterations() {
    let (_events, current_iteration, current_uri_index, eos) = test(
        vec![TestMedia::mkv(), TestMedia::mkv()],
        2,
        2,
        true,
        None,
        false,
    );
    assert!(eos);
    assert_eq!(current_iteration, 1);
    assert_eq!(current_uri_index, 1);
}

#[test]
// FIXME: racy: https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/issues/514
#[ignore]
fn nb_streams_increasing() {
    let (_events, current_iteration, current_uri_index, eos) = test(
        vec![TestMedia::ogg(), TestMedia::mkv()],
        2,
        1,
        false,
        None,
        false,
    );
    assert!(eos);
    assert_eq!(current_iteration, 0);
    assert_eq!(current_uri_index, 1);
}

#[test]
// FIXME: racy: https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/issues/514
#[ignore]
fn missing_file() {
    let (events, current_iteration, current_uri_index, eos) = test(
        vec![TestMedia::ogg(), TestMedia::missing_file()],
        1,
        1,
        false,
        None,
        false,
    );
    assert_error(
        events.into_iter().next_back().unwrap(),
        TestMedia::missing_file(),
    );
    assert!(!eos);
    assert_eq!(current_iteration, 0);
    assert_eq!(current_uri_index, 0);
}

#[test]
// FIXME: racy: https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/issues/514
#[ignore]
fn missing_http() {
    let (events, current_iteration, current_uri_index, eos) = test(
        vec![TestMedia::ogg(), TestMedia::missing_http()],
        1,
        1,
        false,
        None,
        false,
    );
    assert_error(
        events.into_iter().next_back().unwrap(),
        TestMedia::missing_http(),
    );
    assert!(!eos);
    assert_eq!(current_iteration, 0);
    assert_eq!(current_uri_index, 0);
}

#[test]
// FIXME: racy: https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/issues/514
#[ignore]
/// increase playlist iterations while it's playing
fn increase_iterations() {
    let (_events, current_iteration, current_uri_index, eos) = test(
        vec![TestMedia::mkv()],
        2,
        4,
        false,
        Some(IterationsChange {
            when_ss: 2,
            iterations: 8,
        }),
        false,
    );
    assert!(eos);
    assert_eq!(current_iteration, 7);
    assert_eq!(current_uri_index, 0);
}

#[test]
/// decrease playlist iterations while it's playing
// FIXME: racy: https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/issues/514
#[ignore]
fn decrease_iterations() {
    let (_events, current_iteration, current_uri_index, eos) = test(
        vec![TestMedia::mkv()],
        2,
        4,
        false,
        Some(IterationsChange {
            when_ss: 2,
            iterations: 1,
        }),
        false,
    );
    assert!(eos);
    assert_eq!(current_iteration, 2);
    assert_eq!(current_uri_index, 0);
}

#[test]
// FIXME: racy: https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/issues/514
#[ignore]
/// change an infinite playlist to a finite one
fn infinite_to_finite() {
    let (_events, current_iteration, current_uri_index, eos) = test(
        vec![TestMedia::mkv()],
        2,
        0,
        false,
        Some(IterationsChange {
            when_ss: 2,
            iterations: 4,
        }),
        false,
    );
    assert!(eos);
    assert_eq!(current_iteration, 3);
    assert_eq!(current_uri_index, 0);
}

#[test]
// FIXME: racy: https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/issues/514
#[ignore]
/// cache HTTP playlist items
fn cache() {
    let media = TestMedia::mkv_http();

    if let Err(err) = reqwest::blocking::get(&media.uri) {
        println!("skipping test as {} is not available: {}", media.uri, err);
        return;
    }

    let (_events, current_iteration, current_uri_index, eos) =
        test(vec![media], 2, 3, true, None, true);
    assert!(eos);
    assert_eq!(current_iteration, 2);
    assert_eq!(current_uri_index, 0);
}
