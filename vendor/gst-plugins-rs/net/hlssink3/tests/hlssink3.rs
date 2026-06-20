// Copyright (C) 2021 Rafael Caricio <rafael@caricio.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gio::prelude::*;
use gst::prelude::*;
use gsthlssink3::hlssink3::HlsSink3PlaylistType;
use std::io::Write;
use std::sync::LazyLock;
use std::sync::{mpsc, Arc, Mutex};

mod common;
use crate::common::*;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "hlssink3-test",
        gst::DebugColorFlags::empty(),
        Some("Flex HLS sink test"),
    )
});

macro_rules! try_or_pause {
    ($l:expr) => {
        match $l {
            Ok(v) => v,
            Err(err) => {
                eprintln!("Skipping Test: {:?}", err);
                return Ok(());
            }
        }
    };
}

macro_rules! try_create_element {
    ($l:expr, $n:expr) => {
        match gst::ElementFactory::find($l) {
            Some(factory) => factory.create().name($n).build().unwrap(),
            None => {
                eprintln!("Could not find {} ({}) plugin, skipping test", $l, $n);
                return Ok(());
            }
        }
    };
    ($l:expr) => {{
        let alias: String = format!("test_{}", $l);
        try_create_element!($l, <std::string::String as AsRef<str>>::as_ref(&alias))
    }};
}

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gsthlssink3::plugin_register_static().expect("hlssink3 test");
    });
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum HlsSinkEvent {
    GetPlaylistStream(String),
    GetFragmentStream(String),
    DeleteFragment(String),
    SegmentAddedMessage(String),
}

/// Represents a HLS playlist file that writes to a shared string.
///
/// Only used in tests to check the resulting playlist content without the need to write
/// to actual filesystem.
struct MemoryPlaylistFile {
    /// Reference to the content handler.
    handler: Arc<Mutex<String>>,
}

impl MemoryPlaylistFile {
    /// Whenever we know the file will be overridden we call this method to clear the contents.
    fn clear_content(&self) {
        let mut string = self.handler.lock().unwrap();
        string.clear();
    }
}

impl Write for MemoryPlaylistFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // We don't need to handle errors here as this is used only in tests
        let value = std::str::from_utf8(buf).unwrap();
        let mut string = self.handler.lock().unwrap();
        string.push_str(value);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        // We are writing to a `String` object, no need to flush content
        Ok(())
    }
}

#[test]
fn test_hlssink3_element_with_video_content() -> Result<(), ()> {
    init();

    const BUFFER_NB: i32 = 250;

    let pipeline = gst::Pipeline::with_name("video_pipeline");

    let video_src = try_create_element!("videotestsrc");
    video_src.set_property("is-live", false);
    video_src.set_property("num-buffers", BUFFER_NB);

    let x264enc = try_create_element!("x264enc");
    let h264parse = try_create_element!("h264parse");

    let hlssink3 = gst::ElementFactory::make("hlssink3")
        .name("test_hlssink3")
        .property("target-duration", 2u32)
        .property("playlist-length", 2u32)
        .property("max-files", 2u32)
        .build()
        .expect("Must be able to instantiate hlssink3");

    hlssink3.set_property("playlist-type", HlsSink3PlaylistType::Event);
    let pl_type: HlsSink3PlaylistType = hlssink3.property("playlist-type");
    assert_eq!(pl_type, HlsSink3PlaylistType::Event);

    hlssink3.set_property_from_str("playlist-type", "unspecified");

    let (hls_events_sender, hls_events_receiver) = mpsc::sync_channel(20);
    let (hls_messages_sender, hls_messages_receiver) = mpsc::sync_channel(10);
    let playlist_content = Arc::new(Mutex::new(String::from("")));

    hlssink3.connect("get-playlist-stream", false, {
        let hls_events_sender = hls_events_sender.clone();
        let playlist_content = playlist_content.clone();
        move |args| {
            let location = args[1].get::<String>().expect("No location given");

            hls_events_sender
                .try_send(HlsSinkEvent::GetPlaylistStream(location))
                .expect("Send playlist event");

            // We need an owned type to pass to `gio::WriteOutputStream`
            let playlist = MemoryPlaylistFile {
                handler: Arc::clone(&playlist_content),
            };
            // Since here a new file will be created, the content is cleared
            playlist.clear_content();
            let output = gio::WriteOutputStream::new(playlist);
            Some(output.to_value())
        }
    });

    hlssink3.connect("get-fragment-stream", false, {
        let hls_events_sender = hls_events_sender.clone();
        move |args| {
            let location = args[1].get::<String>().expect("No location given");

            hls_events_sender
                .try_send(HlsSinkEvent::GetFragmentStream(location))
                .expect("Send fragment event");

            let stream = gio::MemoryOutputStream::new_resizable();
            Some(stream.to_value())
        }
    });

    hlssink3.connect("delete-fragment", false, {
        let hls_events_sender = hls_events_sender.clone();
        move |args| {
            let location = args[1].get::<String>().expect("No location given");
            hls_events_sender
                .try_send(HlsSinkEvent::DeleteFragment(location))
                .expect("Send delete fragment event");
            Some(true.to_value())
        }
    });

    try_or_pause!(pipeline.add_many([&video_src, &x264enc, &h264parse, &hlssink3,]));
    try_or_pause!(gst::Element::link_many([
        &video_src, &x264enc, &h264parse, &hlssink3
    ]));

    pipeline.set_state(gst::State::Playing).unwrap();

    gst::info!(
        CAT,
        "hlssink3_video_pipeline: waiting for {} buffers",
        BUFFER_NB
    );

    let mut eos = false;
    let bus = pipeline.bus().unwrap();
    while let Some(msg) = bus.timed_pop(gst::ClockTime::NONE) {
        use gst::MessageView;
        match msg.view() {
            MessageView::Eos(..) => {
                eos = true;
                break;
            }
            MessageView::Element(msg) => {
                if let Some(structure) = msg.structure() {
                    if structure.has_name("hls-segment-added") {
                        let location = structure.get::<String>("location").unwrap();
                        hls_messages_sender
                            .try_send(HlsSinkEvent::SegmentAddedMessage(location))
                            .expect("Send segment added event");
                    }
                }
            }
            MessageView::Error(err) => panic!("{err}"),
            _ => (),
        }
    }

    pipeline.set_state(gst::State::Null).unwrap();
    assert!(eos);

    // Collect all events triggered during execution of the pipeline
    let mut actual_events = Vec::new();
    while let Ok(event) = hls_events_receiver.try_recv() {
        actual_events.push(event);
    }
    let expected_ordering_of_events = {
        use self::HlsSinkEvent::*;
        vec![
            GetFragmentStream("segment00000.ts".to_string()),
            GetPlaylistStream("playlist.m3u8".to_string()),
            GetFragmentStream("segment00001.ts".to_string()),
            GetPlaylistStream("playlist.m3u8".to_string()),
            GetFragmentStream("segment00002.ts".to_string()),
            GetPlaylistStream("playlist.m3u8".to_string()),
            DeleteFragment("segment00000.ts".to_string()),
            GetFragmentStream("segment00003.ts".to_string()),
            GetPlaylistStream("playlist.m3u8".to_string()),
            DeleteFragment("segment00001.ts".into()),
            GetFragmentStream("segment00004.ts".to_string()),
            GetPlaylistStream("playlist.m3u8".to_string()),
            DeleteFragment("segment00002.ts".to_string()),
            GetPlaylistStream("playlist.m3u8".to_string()),
        ]
    };
    assert_eq!(expected_ordering_of_events, actual_events);

    let mut actual_messages = Vec::new();
    while let Ok(event) = hls_messages_receiver.try_recv() {
        actual_messages.push(event);
    }
    let expected_messages = {
        use self::HlsSinkEvent::*;
        vec![
            SegmentAddedMessage("segment00000.ts".to_string()),
            SegmentAddedMessage("segment00001.ts".to_string()),
            SegmentAddedMessage("segment00002.ts".to_string()),
            SegmentAddedMessage("segment00003.ts".to_string()),
            SegmentAddedMessage("segment00004.ts".to_string()),
        ]
    };
    assert_eq!(expected_messages, actual_messages);

    let contents = playlist_content.lock().unwrap();
    assert_eq!(
        r###"#EXTM3U
#EXT-X-VERSION:3
#EXT-X-TARGETDURATION:2
#EXT-X-MEDIA-SEQUENCE:4
#EXTINF:1.999,
segment00003.ts
#EXTINF:0.333,
segment00004.ts
#EXT-X-ENDLIST
"###,
        contents.to_string()
    );

    Ok(())
}

#[test]
fn test_hlssink3_element_with_audio_content() -> Result<(), ()> {
    init();

    const BUFFER_NB: i32 = 100;

    let pipeline = gst::Pipeline::with_name("audio_pipeline");

    let audio_src = try_create_element!("audiotestsrc");
    audio_src.set_property("is-live", false);
    audio_src.set_property("num-buffers", BUFFER_NB);

    let hls_avenc_aac = try_or_pause!(gst::ElementFactory::make("avenc_aac")
        .name("hls_avenc_aac")
        .build());
    let hlssink3 = gst::ElementFactory::make("hlssink3")
        .name("hlssink3")
        .property("target-duration", 6u32)
        .build()
        .expect("Must be able to instantiate hlssink3");

    hlssink3.connect("get-playlist-stream", false, move |_args| {
        let stream = gio::MemoryOutputStream::new_resizable();
        Some(stream.to_value())
    });

    hlssink3.connect("get-fragment-stream", false, move |_args| {
        let stream = gio::MemoryOutputStream::new_resizable();
        Some(stream.to_value())
    });

    hlssink3.connect("delete-fragment", false, move |_| Some(true.to_value()));

    try_or_pause!(pipeline.add_many([&audio_src, &hls_avenc_aac, &hlssink3,]));
    try_or_pause!(gst::Element::link_many([
        &audio_src,
        &hls_avenc_aac,
        &hlssink3
    ]));

    pipeline.set_state(gst::State::Playing).unwrap();

    gst::info!(CAT, "audio_pipeline: waiting for {} buffers", BUFFER_NB);

    let mut eos = false;
    let bus = pipeline.bus().unwrap();
    while let Some(msg) = bus.timed_pop(gst::ClockTime::NONE) {
        use gst::MessageView;
        match msg.view() {
            MessageView::Eos(..) => {
                eos = true;
                break;
            }
            MessageView::Error(..) => unreachable!(),
            _ => (),
        }
    }

    pipeline.set_state(gst::State::Null).unwrap();
    assert!(eos);
    Ok(())
}

#[test]
fn test_hlssink3_write_correct_playlist_content() -> Result<(), ()> {
    init();

    const BUFFER_NB: i32 = 50;

    let pipeline = gst::Pipeline::with_name("video_pipeline");

    let video_src = try_create_element!("videotestsrc");
    video_src.set_property("is-live", false);
    video_src.set_property("num-buffers", BUFFER_NB);

    let x264enc = try_create_element!("x264enc");
    let h264parse = try_create_element!("h264parse");

    let hlssink3 = gst::ElementFactory::make("hlssink3")
        .name("test_hlssink3")
        .property("location", "/www/media/segments/my-own-filename-%03d.ts")
        .property("playlist-location", "/www/media/main.m3u8")
        .property("playlist-root", "segments")
        .build()
        .expect("Must be able to instantiate hlssink3");

    let (hls_events_sender, hls_events_receiver) = mpsc::sync_channel(20);
    let (hls_messages_sender, hls_messages_receiver) = mpsc::sync_channel(10);
    let playlist_content = Arc::new(Mutex::new(String::from("")));

    hlssink3.connect("get-playlist-stream", false, {
        let hls_events_sender = hls_events_sender.clone();
        let playlist_content = playlist_content.clone();
        move |args| {
            let location = args[1].get::<String>().expect("No location given");

            hls_events_sender
                .try_send(HlsSinkEvent::GetPlaylistStream(location))
                .expect("Send playlist event");

            // We need an owned type to pass to `gio::WriteOutputStream`
            let playlist = MemoryPlaylistFile {
                handler: Arc::clone(&playlist_content),
            };
            // Since here a new file will be created, the content is cleared
            playlist.clear_content();
            let output = gio::WriteOutputStream::new(playlist);
            Some(output.to_value())
        }
    });

    hlssink3.connect("get-fragment-stream", false, {
        let hls_events_sender = hls_events_sender.clone();
        move |args| {
            let location = args[1].get::<String>().expect("No location given");

            hls_events_sender
                .try_send(HlsSinkEvent::GetFragmentStream(location))
                .expect("Send fragment event");

            let stream = gio::MemoryOutputStream::new_resizable();
            Some(stream.to_value())
        }
    });

    hlssink3.connect("delete-fragment", false, {
        let hls_events_sender = hls_events_sender.clone();
        move |args| {
            let location = args[1].get::<String>().expect("No location given");
            hls_events_sender
                .try_send(HlsSinkEvent::DeleteFragment(location))
                .expect("Send delete fragment event");
            Some(true.to_value())
        }
    });

    try_or_pause!(pipeline.add_many([&video_src, &x264enc, &h264parse, &hlssink3,]));
    try_or_pause!(gst::Element::link_many([
        &video_src, &x264enc, &h264parse, &hlssink3
    ]));

    pipeline.set_state(gst::State::Playing).unwrap();

    gst::info!(
        CAT,
        "hlssink3_video_pipeline: waiting for {} buffers",
        BUFFER_NB
    );

    let mut eos = false;
    let bus = pipeline.bus().unwrap();
    while let Some(msg) = bus.timed_pop(gst::ClockTime::NONE) {
        use gst::MessageView;
        match msg.view() {
            MessageView::Eos(..) => {
                eos = true;
                break;
            }
            MessageView::Element(msg) => {
                if let Some(structure) = msg.structure() {
                    if structure.has_name("hls-segment-added") {
                        let location = structure.get::<String>("location").unwrap();
                        hls_messages_sender
                            .try_send(HlsSinkEvent::SegmentAddedMessage(location))
                            .expect("Send segment added event");
                    }
                }
            }
            MessageView::Error(..) => unreachable!(),
            _ => (),
        }
    }

    pipeline.set_state(gst::State::Null).unwrap();
    assert!(eos);

    // Collect all events triggered during execution of the pipeline
    let mut actual_events = Vec::new();
    while let Ok(event) = hls_events_receiver.try_recv() {
        actual_events.push(event);
    }
    let expected_ordering_of_events = {
        use self::HlsSinkEvent::*;
        vec![
            GetFragmentStream("/www/media/segments/my-own-filename-000.ts".to_string()),
            GetPlaylistStream("/www/media/main.m3u8".to_string()),
            GetPlaylistStream("/www/media/main.m3u8".to_string()),
        ]
    };
    assert_eq!(expected_ordering_of_events, actual_events);

    let mut actual_messages = Vec::new();
    while let Ok(event) = hls_messages_receiver.try_recv() {
        actual_messages.push(event);
    }
    let expected_messages = {
        use self::HlsSinkEvent::*;
        vec![SegmentAddedMessage(
            "/www/media/segments/my-own-filename-000.ts".to_string(),
        )]
    };
    assert_eq!(expected_messages, actual_messages);

    let contents = playlist_content.lock().unwrap();
    assert_eq!(
        r###"#EXTM3U
#EXT-X-VERSION:3
#EXT-X-TARGETDURATION:15
#EXT-X-MEDIA-SEQUENCE:1
#EXTINF:1.666,
segments/my-own-filename-000.ts
#EXT-X-ENDLIST
"###,
        contents.to_string()
    );

    Ok(())
}

#[test]
fn test_hlssink3_video_with_single_media_file() -> Result<(), ()> {
    init();

    const BUFFER_NB: i32 = 512;

    let pipeline = gst::Pipeline::with_name("video_pipeline");

    let video_src = gst::ElementFactory::make("videotestsrc")
        .property("is-live", true)
        .property("num-buffers", 512i32)
        .build()
        .unwrap();

    let caps = gst::Caps::builder("video/x-raw")
        .field("wifth", 320)
        .field("height", 240)
        .field("format", "I420")
        .field("framerate", gst::Fraction::new(30, 1))
        .build();
    let capsfilter = gst::ElementFactory::make("capsfilter")
        .property("caps", caps)
        .build()
        .expect("Must be able to instantiate capsfilter");
    let videorate = gst::ElementFactory::make("videorate").build().unwrap();
    let x264enc = gst::ElementFactory::make("x264enc")
        .property("key-int-max", 60u32)
        .property("bitrate", 2000u32)
        .property_from_str("speed-preset", "ultrafast")
        .property_from_str("tune", "zerolatency")
        .build()
        .expect("Must be able to instantiate x264enc");
    let h264parse = gst::ElementFactory::make("h264parse").build().unwrap();

    let hlssink3 = gst::ElementFactory::make("hlssink3")
        .name("test_hlssink3")
        .property("target-duration", 6u32)
        .property("single-media-file", "main.ts")
        .build()
        .expect("Must be able to instantiate hlssink3");

    hlssink3.set_property("playlist-type", HlsSink3PlaylistType::Vod);
    let pl_type: HlsSink3PlaylistType = hlssink3.property("playlist-type");
    assert_eq!(pl_type, HlsSink3PlaylistType::Vod);

    let (hls_events_sender, hls_events_receiver) = mpsc::sync_channel(5);
    let (hls_messages_sender, hls_messages_receiver) = mpsc::sync_channel(5);
    let playlist_content = Arc::new(Mutex::new(String::from("")));

    hlssink3.connect("get-playlist-stream", false, {
        let hls_events_sender = hls_events_sender.clone();
        let playlist_content = playlist_content.clone();
        move |args| {
            let location = args[1].get::<String>().expect("No location given");

            hls_events_sender
                .try_send(HlsSinkEvent::GetPlaylistStream(location))
                .expect("Send playlist event");

            let playlist = MemoryPlaylistFile {
                handler: Arc::clone(&playlist_content),
            };

            playlist.clear_content();
            let output = gio::WriteOutputStream::new(playlist);
            Some(output.to_value())
        }
    });

    hlssink3.connect("get-fragment-stream", false, {
        let hls_events_sender = hls_events_sender.clone();
        move |args| {
            let location = args[1].get::<String>().expect("No location given");

            hls_events_sender
                .try_send(HlsSinkEvent::GetFragmentStream(location))
                .expect("Send fragment event");

            let stream = gio::MemoryOutputStream::new_resizable();
            Some(stream.to_value())
        }
    });

    pipeline
        .add_many([
            &video_src,
            &capsfilter,
            &videorate,
            &x264enc,
            &h264parse,
            &hlssink3,
        ])
        .unwrap();
    gst::Element::link_many([
        &video_src,
        &capsfilter,
        &videorate,
        &x264enc,
        &h264parse,
        &hlssink3,
    ])
    .unwrap();

    pipeline.set_state(gst::State::Playing).unwrap();

    gst::info!(
        CAT,
        "hlssink3_video_pipeline: waiting for {} buffers",
        BUFFER_NB
    );

    let mut byte_ranges: Vec<ByteRange> = Vec::new();
    let mut eos = false;
    let bus = pipeline.bus().unwrap();
    while let Some(msg) = bus.timed_pop(gst::ClockTime::NONE) {
        use gst::MessageView;
        match msg.view() {
            MessageView::Eos(..) => {
                eos = true;
                break;
            }
            MessageView::Element(msg) => {
                if let Some(structure) = msg.structure() {
                    if structure.has_name("hls-segment-added") {
                        let location = structure.get::<String>("location").unwrap();
                        let byte_range = get_byte_ranges(structure);
                        byte_ranges.extend(byte_range);

                        hls_messages_sender
                            .try_send(HlsSinkEvent::SegmentAddedMessage(location))
                            .expect("Send segment added event");
                    }
                }
            }
            MessageView::Error(err) => panic!("{err}"),
            _ => (),
        }
    }

    pipeline.set_state(gst::State::Null).unwrap();
    assert!(eos);

    let mut actual_events = Vec::new();
    while let Ok(event) = hls_events_receiver.try_recv() {
        actual_events.push(event);
    }

    let mut actual_messages = Vec::new();
    while let Ok(event) = hls_messages_receiver.try_recv() {
        actual_messages.push(event);
    }

    eprintln!("Playlist byte ranges: {byte_ranges:?}");
    // We only validate the byte range tag. The actual value of byte
    // range can differ from each run and hence we do not validate
    // the entire playlist.
    assert!(validate_byterange_sequence(&byte_ranges));
    assert!(all_ranges_non_overlapping(&byte_ranges));

    let expected_messages = {
        use self::HlsSinkEvent::*;
        vec![
            // For single media file, location of added segment will
            // always be same.
            SegmentAddedMessage("main.ts".to_string()),
            SegmentAddedMessage("main.ts".to_string()),
            SegmentAddedMessage("main.ts".to_string()),
        ]
    };
    assert_eq!(expected_messages, actual_messages);

    let expected_ordering_of_events = {
        use self::HlsSinkEvent::*;
        vec![
            // For single media file, GET_FRAGMENT_STREAM will only be
            // emitted once.
            GetFragmentStream("main.ts".to_string()),
            GetPlaylistStream("playlist.m3u8".to_string()),
            GetPlaylistStream("playlist.m3u8".to_string()),
            GetPlaylistStream("playlist.m3u8".to_string()),
            GetPlaylistStream("playlist.m3u8".to_string()),
        ]
    };
    assert_eq!(expected_ordering_of_events, actual_events);

    Ok(())
}
