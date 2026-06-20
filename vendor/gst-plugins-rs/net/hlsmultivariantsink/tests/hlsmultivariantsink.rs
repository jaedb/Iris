// Copyright (C) 2024, asymptotic.io
//      Author: Sanchayan Maity <sanchayan@asymptotic.io>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/*
 * Written on the same lines as the `hlssink3` tests.
 */

use gio::prelude::*;
use gst::prelude::*;
use gsthlsmultivariantsink::{HlsMultivariantSinkMuxerType, HlsMultivariantSinkPlaylistType};
use serial_test::serial;
use std::io::Write;
use std::str::FromStr;
use std::sync::mpsc::SyncSender;
use std::sync::{mpsc, Arc, LazyLock, Mutex};
use std::time::Duration;
use std::{collections::HashSet, hash::Hash};

const DEFAULT_TS_LOCATION: &str = "segment%05d.ts";
const DEFAULT_INIT_LOCATION: &str = "init%05d.mp4";
const DEFAULT_CMAF_LOCATION: &str = "segment%05d.m4s";

fn is_playlist_events_eq<T>(a: &[T], b: &[T]) -> bool
where
    T: Eq + Hash + std::fmt::Debug,
{
    let a: HashSet<_> = a.iter().collect();
    let b: HashSet<_> = b.iter().collect();

    a == b
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "hlsmultivariantsink-test",
        gst::DebugColorFlags::empty(),
        Some("HLS sink test"),
    )
});

macro_rules! try_create_element {
    ($l:expr, $n:expr) => {
        match gst::ElementFactory::find($l) {
            Some(factory) => Ok(factory.create().name($n).build().unwrap()),
            None => {
                eprintln!("Could not find {} ({}) plugin, skipping test", $l, $n);
                return Err(());
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
        gstisobmff::plugin_register_static().expect("Need cmafmux for hlsmultivariantsink test");
        gsthlssink3::plugin_register_static().expect("Need hlssink3 for hlsmultivariantsink test");
        gsthlsmultivariantsink::plugin_register_static().expect("hlsmultivariantsink test");
    });
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum HlsSinkEvent {
    DeleteFragment(String),
    GetMultivariantPlaylistStream(String),
    GetFragmentStream(String),
    GetInitStream(String),
    GetPlaylistStream(String),
}

struct MemoryPlaylistFile {
    handler: Arc<Mutex<String>>,
}

impl MemoryPlaylistFile {
    fn clear_content(&self) {
        let mut string = self.handler.lock().unwrap();
        string.clear();
    }
}

impl Write for MemoryPlaylistFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let value = std::str::from_utf8(buf).unwrap();
        let mut string = self.handler.lock().unwrap();
        string.push_str(value);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn video_bin(
    width: u32,
    height: u32,
    fps: u32,
    bitrate: u32,
    iframe_only: bool,
    h265: bool,
) -> Result<gst::Bin, ()> {
    const BUFFER_NB: i32 = 62;

    let bin = gst::Bin::new();

    let videosrc = try_create_element!("videotestsrc")?;
    let videoconvert = try_create_element!("videoconvert")?;
    let videoscale = try_create_element!("videoscale")?;
    let videorate = try_create_element!("videorate")?;
    let capsfilter = try_create_element!("capsfilter")?;
    let x26xenc = if h265 {
        try_create_element!("x265enc")?
    } else {
        try_create_element!("x264enc")?
    };
    let h26xparse = if h265 {
        try_create_element!("h265parse")?
    } else {
        try_create_element!("h264parse")?
    };
    let queue = try_create_element!("queue")?;

    videosrc.set_property("is-live", true);
    videosrc.set_property_from_str("pattern", "ball");
    videosrc.set_property("num-buffers", BUFFER_NB);

    let caps = gst::Caps::from_str(format!("video/x-raw,width={width},height={height},framerate={fps}/1,pixel-aspect-ratio=1/1,format=I420").as_str()).unwrap();
    capsfilter.set_property("caps", caps);

    x26xenc.set_property("bitrate", bitrate);
    if iframe_only {
        x26xenc.set_property("key-int-max", 1);
    } else if h265 {
        x26xenc.set_property("key-int-max", (fps * 2) as i32);
    } else {
        x26xenc.set_property("key-int-max", fps * 2);
    }

    bin.add(&videosrc).unwrap();
    bin.add(&videoconvert).unwrap();
    bin.add(&videoscale).unwrap();
    bin.add(&videorate).unwrap();
    bin.add(&capsfilter).unwrap();
    bin.add(&x26xenc).unwrap();
    bin.add(&h26xparse).unwrap();
    bin.add(&queue).unwrap();

    videosrc.link(&videoconvert).unwrap();
    videoconvert.link(&videorate).unwrap();
    videorate.link(&videoscale).unwrap();
    videoscale.link(&capsfilter).unwrap();
    capsfilter.link(&x26xenc).unwrap();
    x26xenc.link(&h26xparse).unwrap();
    h26xparse.link(&queue).unwrap();

    let queue_srcpad = queue.static_pad("src").unwrap();

    let srcpad = gst::GhostPad::with_target(&queue_srcpad).unwrap();

    bin.add_pad(&srcpad).unwrap();

    Ok(bin)
}

fn audio_bin(bitrate: i32) -> Result<gst::Bin, ()> {
    const BUFFER_NB: i32 = 62;

    let bin = gst::Bin::new();

    let audiosrc = try_create_element!("audiotestsrc")?;
    let audioconvert = try_create_element!("audioconvert")?;
    let audiorate = try_create_element!("audiorate")?;
    let capsfilter = try_create_element!("capsfilter")?;
    let aacenc = try_create_element!("fdkaacenc")?;
    let aacparse = try_create_element!("aacparse")?;
    let queue = try_create_element!("queue")?;

    let caps = gst::Caps::from_str("audio/x-raw,channels=2,rate=48000,format=S16LE").unwrap();

    audiosrc.set_property("is-live", true);
    audiosrc.set_property("num-buffers", BUFFER_NB);
    capsfilter.set_property("caps", caps);
    aacenc.set_property("bitrate", bitrate);

    bin.add(&audiosrc).unwrap();
    bin.add(&audioconvert).unwrap();
    bin.add(&audiorate).unwrap();
    bin.add(&capsfilter).unwrap();
    bin.add(&aacenc).unwrap();
    bin.add(&aacparse).unwrap();
    bin.add(&queue).unwrap();

    audiosrc.link(&audioconvert).unwrap();
    audioconvert.link(&audiorate).unwrap();
    audiorate.link(&capsfilter).unwrap();
    capsfilter.link(&aacenc).unwrap();
    aacenc.link(&aacparse).unwrap();
    aacparse.link(&queue).unwrap();

    let queue_srcpad = queue.static_pad("src").unwrap();

    let srcpad = gst::GhostPad::with_target(&queue_srcpad).unwrap();

    bin.add_pad(&srcpad).unwrap();

    Ok(bin)
}

/*
* For the tests below the directory structure would be expected to be
* as follows.
*
* multivariant playlist will have the path /tmp/hlssink/multivariant.m3u8.
* Every other variant stream or alternate rendition will be relative to
* /tmp/hlssink.
*
   # tmp → hlssink λ:
   .
   ├── hi
   ├── hi-audio
   ├── low
   ├── low-audio
   ├── mid
   ├── mid-audio
*
*/

fn setup_signals(
    hlssink: &gst::Element,
    hls_events_sender: SyncSender<HlsSinkEvent>,
    multivariant_playlist_content: Arc<Mutex<String>>,
    playlist_content: Arc<Mutex<String>>,
    muxer_type: HlsMultivariantSinkMuxerType,
) {
    hlssink.connect("get-multivariant-playlist-stream", false, {
        let hls_events_sender = hls_events_sender.clone();
        let multivariant_playlist_content = multivariant_playlist_content.clone();
        move |args| {
            let location = args[1].get::<String>().expect("No location given");

            gst::info!(CAT, "get-multivariant-playlist-stream: {}", location);

            hls_events_sender
                .try_send(HlsSinkEvent::GetMultivariantPlaylistStream(location))
                .expect("Send multivariant playlist event");

            let playlist = MemoryPlaylistFile {
                handler: Arc::clone(&multivariant_playlist_content),
            };

            playlist.clear_content();
            let output = gio::WriteOutputStream::new(playlist);
            Some(output.to_value())
        }
    });

    if muxer_type == HlsMultivariantSinkMuxerType::Cmaf {
        hlssink.connect("get-init-stream", false, {
            let hls_events_sender = hls_events_sender.clone();
            move |args| {
                let location = args[1].get::<String>().expect("No location given");

                hls_events_sender
                    .try_send(HlsSinkEvent::GetInitStream(location))
                    .expect("Send init event");

                let stream = gio::MemoryOutputStream::new_resizable();
                Some(stream.to_value())
            }
        });
    }

    hlssink.connect("get-playlist-stream", false, {
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

    hlssink.connect("get-fragment-stream", false, {
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

    hlssink.connect("delete-fragment", false, move |args| {
        let location = args[1].get::<String>().expect("No location given");
        hls_events_sender
            .try_send(HlsSinkEvent::DeleteFragment(location))
            .expect("Send delete fragment event");
        Some(true.to_value())
    });
}

#[ignore]
#[test]
#[serial]
fn hlsmultivariantsink_multiple_audio_rendition_multiple_video_variant() -> Result<(), ()> {
    init();

    let pipeline = gst::Pipeline::with_name("hlsmultivariantsink_pipeline");

    let hlsmultivariantsink = gst::ElementFactory::make("hlsmultivariantsink")
        .name("test_hlsmultivariantsink")
        .property(
            "multivariant-playlist-location",
            "/tmp/hlssink/multivariant.m3u8",
        )
        .property("target-duration", 2u32)
        .property("playlist-length", 2u32)
        .property("max-files", 2u32)
        .build()
        .expect("Must be able to instantiate hlsmultivariantsink");

    hlsmultivariantsink.set_property("playlist-type", HlsMultivariantSinkPlaylistType::Event);
    let pl_type: HlsMultivariantSinkPlaylistType = hlsmultivariantsink.property("playlist-type");
    assert_eq!(pl_type, HlsMultivariantSinkPlaylistType::Event);

    hlsmultivariantsink.set_property_from_str("playlist-type", "unspecified");

    let muxer_type: HlsMultivariantSinkMuxerType = hlsmultivariantsink.property("muxer-type");
    assert_eq!(muxer_type, HlsMultivariantSinkMuxerType::Cmaf);

    pipeline.add(&hlsmultivariantsink).unwrap();

    let (hls_events_sender, hls_events_receiver) = mpsc::sync_channel(100);
    let multivariant_playlist_content = Arc::new(Mutex::new(String::from("")));
    let playlist_content = Arc::new(Mutex::new(String::from("")));

    setup_signals(
        &hlsmultivariantsink,
        hls_events_sender.clone(),
        multivariant_playlist_content.clone(),
        playlist_content.clone(),
        HlsMultivariantSinkMuxerType::Cmaf,
    );

    let audio_bin1 = audio_bin(256000).unwrap();
    let audio_bin1_pad = audio_bin1.static_pad("src").unwrap();
    let audio1_pad = hlsmultivariantsink.request_pad_simple("audio_%u").unwrap();
    let r = gst::Structure::builder("audio1-rendition")
        .field("media", "AUDIO")
        .field("uri", "hi-audio/audio.m3u8")
        .field("group_id", "aac")
        .field("language", "en")
        .field("name", "English")
        .field("default", true)
        .field("autoselect", false)
        .build();
    audio1_pad.set_property("alternate-rendition", r);
    audio1_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/hi-audio/audio.m3u8".to_string(),
    );
    audio1_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/hi-audio/{DEFAULT_CMAF_LOCATION}"),
    );
    audio1_pad.set_property(
        "init-segment-location",
        format!("/tmp/hlssink/hi-audio/{DEFAULT_INIT_LOCATION}"),
    );
    pipeline.add(&audio_bin1).unwrap();
    audio_bin1_pad.link(&audio1_pad).unwrap();

    let audio_bin2 = audio_bin(128000).unwrap();
    let audio_bin2_pad = audio_bin2.static_pad("src").unwrap();
    let audio2_pad = hlsmultivariantsink.request_pad_simple("audio_%u").unwrap();
    let r = gst::Structure::builder("audio2-rendition")
        .field("media", "AUDIO")
        .field("uri", "mid-audio/audio.m3u8")
        .field("group_id", "aac")
        .field("language", "fr")
        .field("name", "French")
        .field("default", false)
        .field("autoselect", false)
        .build();
    audio2_pad.set_property("alternate-rendition", r);
    audio2_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/mid-audio/audio.m3u8".to_string(),
    );
    audio2_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/mid-audio/{DEFAULT_CMAF_LOCATION}"),
    );
    audio2_pad.set_property(
        "init-segment-location",
        format!("/tmp/hlssink/mid-audio/{DEFAULT_INIT_LOCATION}"),
    );
    pipeline.add(&audio_bin2).unwrap();
    audio_bin2_pad.link(&audio2_pad).unwrap();

    let video_bin1 = video_bin(1920, 1080, 30, 2500, false, false).unwrap();
    let video_bin1_pad = video_bin1.static_pad("src").unwrap();
    let video1_pad = hlsmultivariantsink.request_pad_simple("video_%u").unwrap();
    let v = gst::Structure::builder("video1-variant")
        .field("uri", "hi/video.m3u8")
        .field("audio", "aac")
        .field("bandwidth", 2500)
        .build();
    video1_pad.set_property("variant", v);
    video1_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/hi/video.m3u8".to_string(),
    );
    video1_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/hi/{DEFAULT_CMAF_LOCATION}"),
    );
    video1_pad.set_property(
        "init-segment-location",
        format!("/tmp/hlssink/hi/{DEFAULT_INIT_LOCATION}"),
    );
    pipeline.add(&video_bin1).unwrap();
    video_bin1_pad.link(&video1_pad).unwrap();

    let video_bin2 = video_bin(1280, 720, 30, 1500, false, false).unwrap();
    let video_bin2_pad = video_bin2.static_pad("src").unwrap();
    let video2_pad = hlsmultivariantsink.request_pad_simple("video_%u").unwrap();
    let v = gst::Structure::builder("video2-variant")
        .field("uri", "mid/video.m3u8")
        .field("audio", "aac")
        .field("bandwidth", 1500)
        .build();
    video2_pad.set_property("variant", v);
    video2_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/mid/video.m3u8".to_string(),
    );
    video2_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/mid/{DEFAULT_CMAF_LOCATION}"),
    );
    video2_pad.set_property(
        "init-segment-location",
        format!("/tmp/hlssink/mid/{DEFAULT_INIT_LOCATION}"),
    );
    pipeline.add(&video_bin2).unwrap();
    video_bin2_pad.link(&video2_pad).unwrap();

    let video_bin3 = video_bin(640, 360, 24, 700, false, false).unwrap();
    let video_bin3_pad = video_bin3.static_pad("src").unwrap();
    let video3_pad = hlsmultivariantsink.request_pad_simple("video_%u").unwrap();
    let v = gst::Structure::builder("video2-variant")
        .field("uri", "low/video.m3u8")
        .field("audio", "aac")
        .field("bandwidth", 700)
        .build();
    video3_pad.set_property("variant", v);
    video3_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/low/video.m3u8".to_string(),
    );
    video3_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/low/{DEFAULT_CMAF_LOCATION}"),
    );
    video3_pad.set_property(
        "init-segment-location",
        format!("/tmp/hlssink/low/{DEFAULT_INIT_LOCATION}"),
    );
    pipeline.add(&video_bin3).unwrap();
    video_bin3_pad.link(&video3_pad).unwrap();

    pipeline.set_state(gst::State::Playing).unwrap();

    let mut eos = false;
    let bus = pipeline.bus().unwrap();
    while let Some(msg) = bus.timed_pop(gst::ClockTime::NONE) {
        use gst::MessageView;
        match msg.view() {
            MessageView::Eos(..) => {
                eos = true;
                break;
            }
            MessageView::Error(e) => gst::error!(CAT, "hlsmultivariantsink error: {}", e),
            _ => (),
        }
    }

    pipeline.debug_to_dot_file_with_ts(
        gst::DebugGraphDetails::all(),
        "multiple_audio_rendition_multiple_video_variant",
    );

    pipeline.set_state(gst::State::Null).unwrap();
    assert!(eos);

    let mut actual_events = Vec::new();
    while let Ok(event) = hls_events_receiver.recv_timeout(Duration::from_secs(30)) {
        actual_events.push(event);
    }
    let expected_events = {
        use self::HlsSinkEvent::*;
        vec![
            GetMultivariantPlaylistStream("/tmp/hlssink/multivariant.m3u8".to_string()),
            GetInitStream("/tmp/hlssink/hi/init00000.mp4".to_string()),
            GetInitStream("/tmp/hlssink/mid/init00000.mp4".to_string()),
            GetInitStream("/tmp/hlssink/low/init00000.mp4".to_string()),
            GetInitStream("/tmp/hlssink/hi-audio/init00000.mp4".to_string()),
            GetInitStream("/tmp/hlssink/mid-audio/init00000.mp4".to_string()),
            GetPlaylistStream("/tmp/hlssink/hi/video.m3u8".to_string()),
            GetPlaylistStream("/tmp/hlssink/mid/video.m3u8".to_string()),
            GetPlaylistStream("/tmp/hlssink/low/video.m3u8".to_string()),
            GetPlaylistStream("/tmp/hlssink/hi-audio/audio.m3u8".to_string()),
            GetPlaylistStream("/tmp/hlssink/mid-audio/audio.m3u8".to_string()),
            GetFragmentStream("/tmp/hlssink/hi/segment00000.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/hi/segment00001.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/mid/segment00000.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/mid/segment00001.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/low/segment00000.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/low/segment00001.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/hi-audio/segment00000.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/hi-audio/segment00001.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/mid-audio/segment00000.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/mid-audio/segment00001.m4s".to_string()),
        ]
    };
    assert!(is_playlist_events_eq(&expected_events, &actual_events));

    let contents = multivariant_playlist_content.lock().unwrap();

    #[rustfmt::skip]
        assert_eq!(
            r###"#EXTM3U
#EXT-X-VERSION:6
#EXT-X-MEDIA:TYPE=AUDIO,URI="hi-audio/audio.m3u8",GROUP-ID="aac",LANGUAGE="en",NAME="English",DEFAULT=YES
#EXT-X-MEDIA:TYPE=AUDIO,URI="mid-audio/audio.m3u8",GROUP-ID="aac",LANGUAGE="fr",NAME="French"
#EXT-X-STREAM-INF:BANDWIDTH=2500,CODECS="avc1.64001E,avc1.64001F,avc1.640028,mp4a.40.2",AUDIO="aac"
hi/video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=1500,CODECS="avc1.64001E,avc1.64001F,avc1.640028,mp4a.40.2",AUDIO="aac"
mid/video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=700,CODECS="avc1.64001E,avc1.64001F,avc1.640028,mp4a.40.2",AUDIO="aac"
low/video.m3u8
"###,
            contents.to_string()
        );

    Ok(())
}

#[ignore]
#[test]
#[serial]
fn hlsmultivariantsink_multiple_audio_rendition_multiple_video_variant_with_relative_path(
) -> Result<(), ()> {
    /*
     * For this test, we do not set the playlist and segment location on the pad. The sink
     * will figure out and use relative path to the multivariant playlist.
     *
     * In this case, a playlist path like `hi-audio/audio.m3u8` will map to
     * `/tmp/hlssink/hi-audio/audio.m3u8`.
     */
    init();

    let pipeline = gst::Pipeline::with_name("hlsmultivariantsink_pipeline");

    let hlsmultivariantsink = gst::ElementFactory::make("hlsmultivariantsink")
        .name("test_hlsmultivariantsink")
        .property(
            "multivariant-playlist-location",
            "/tmp/hlssink/multivariant.m3u8",
        )
        .property("target-duration", 2u32)
        .property("playlist-length", 2u32)
        .property("max-files", 2u32)
        .build()
        .expect("Must be able to instantiate hlsmultivariantsink");

    hlsmultivariantsink.set_property("playlist-type", HlsMultivariantSinkPlaylistType::Event);
    let pl_type: HlsMultivariantSinkPlaylistType = hlsmultivariantsink.property("playlist-type");
    assert_eq!(pl_type, HlsMultivariantSinkPlaylistType::Event);

    hlsmultivariantsink.set_property_from_str("playlist-type", "unspecified");

    let muxer_type: HlsMultivariantSinkMuxerType = hlsmultivariantsink.property("muxer-type");
    assert_eq!(muxer_type, HlsMultivariantSinkMuxerType::Cmaf);

    pipeline.add(&hlsmultivariantsink).unwrap();

    let (hls_events_sender, hls_events_receiver) = mpsc::sync_channel(100);
    let multivariant_playlist_content = Arc::new(Mutex::new(String::from("")));
    let playlist_content = Arc::new(Mutex::new(String::from("")));

    setup_signals(
        &hlsmultivariantsink,
        hls_events_sender.clone(),
        multivariant_playlist_content.clone(),
        playlist_content.clone(),
        HlsMultivariantSinkMuxerType::Cmaf,
    );

    let audio_bin1 = audio_bin(256000).unwrap();
    let audio_bin1_pad = audio_bin1.static_pad("src").unwrap();
    let audio1_pad = hlsmultivariantsink.request_pad_simple("audio_%u").unwrap();
    let r = gst::Structure::builder("audio1-rendition")
        .field("media", "AUDIO")
        .field("uri", "hi-audio/audio.m3u8")
        .field("group_id", "aac")
        .field("language", "en")
        .field("name", "English")
        .field("default", true)
        .field("autoselect", false)
        .build();
    audio1_pad.set_property("alternate-rendition", r);
    pipeline.add(&audio_bin1).unwrap();
    audio_bin1_pad.link(&audio1_pad).unwrap();

    let audio_bin2 = audio_bin(128000).unwrap();
    let audio_bin2_pad = audio_bin2.static_pad("src").unwrap();
    let audio2_pad = hlsmultivariantsink.request_pad_simple("audio_%u").unwrap();
    let r = gst::Structure::builder("audio2-rendition")
        .field("media", "AUDIO")
        .field("uri", "mid-audio/audio.m3u8")
        .field("group_id", "aac")
        .field("language", "fr")
        .field("name", "French")
        .field("default", false)
        .field("autoselect", false)
        .build();
    audio2_pad.set_property("alternate-rendition", r);
    pipeline.add(&audio_bin2).unwrap();
    audio_bin2_pad.link(&audio2_pad).unwrap();

    let video_bin1 = video_bin(1920, 1080, 30, 2500, false, false).unwrap();
    let video_bin1_pad = video_bin1.static_pad("src").unwrap();
    let video1_pad = hlsmultivariantsink.request_pad_simple("video_%u").unwrap();
    let v = gst::Structure::builder("video1-variant")
        .field("uri", "hi/video.m3u8")
        .field("audio", "aac")
        .field("bandwidth", 2500)
        .build();
    video1_pad.set_property("variant", v);
    pipeline.add(&video_bin1).unwrap();
    video_bin1_pad.link(&video1_pad).unwrap();

    let video_bin2 = video_bin(1280, 720, 30, 1500, false, false).unwrap();
    let video_bin2_pad = video_bin2.static_pad("src").unwrap();
    let video2_pad = hlsmultivariantsink.request_pad_simple("video_%u").unwrap();
    let v = gst::Structure::builder("video2-variant")
        .field("uri", "mid/video.m3u8")
        .field("audio", "aac")
        .field("bandwidth", 1500)
        .build();
    video2_pad.set_property("variant", v);
    pipeline.add(&video_bin2).unwrap();
    video_bin2_pad.link(&video2_pad).unwrap();

    let video_bin3 = video_bin(640, 360, 24, 700, false, false).unwrap();
    let video_bin3_pad = video_bin3.static_pad("src").unwrap();
    let video3_pad = hlsmultivariantsink.request_pad_simple("video_%u").unwrap();
    let v = gst::Structure::builder("video2-variant")
        .field("uri", "low/video.m3u8")
        .field("audio", "aac")
        .field("bandwidth", 700)
        .build();
    video3_pad.set_property("variant", v);
    pipeline.add(&video_bin3).unwrap();
    video_bin3_pad.link(&video3_pad).unwrap();

    pipeline.set_state(gst::State::Playing).unwrap();

    let mut eos = false;
    let bus = pipeline.bus().unwrap();
    while let Some(msg) = bus.timed_pop(gst::ClockTime::NONE) {
        use gst::MessageView;
        match msg.view() {
            MessageView::Eos(..) => {
                eos = true;
                break;
            }
            MessageView::Error(e) => gst::error!(CAT, "hlsmultivariantsink error: {}", e),
            _ => (),
        }
    }

    pipeline.debug_to_dot_file_with_ts(
        gst::DebugGraphDetails::all(),
        "multiple_audio_rendition_multiple_video_variant",
    );

    pipeline.set_state(gst::State::Null).unwrap();
    assert!(eos);

    let mut actual_events = Vec::new();
    while let Ok(event) = hls_events_receiver.recv_timeout(Duration::from_secs(30)) {
        actual_events.push(event);
    }
    let expected_events = {
        use self::HlsSinkEvent::*;
        vec![
            GetMultivariantPlaylistStream("/tmp/hlssink/multivariant.m3u8".to_string()),
            GetInitStream("/tmp/hlssink/hi/init00000.mp4".to_string()),
            GetInitStream("/tmp/hlssink/mid/init00000.mp4".to_string()),
            GetInitStream("/tmp/hlssink/low/init00000.mp4".to_string()),
            GetInitStream("/tmp/hlssink/hi-audio/init00000.mp4".to_string()),
            GetInitStream("/tmp/hlssink/mid-audio/init00000.mp4".to_string()),
            GetPlaylistStream("/tmp/hlssink/hi/video.m3u8".to_string()),
            GetPlaylistStream("/tmp/hlssink/mid/video.m3u8".to_string()),
            GetPlaylistStream("/tmp/hlssink/low/video.m3u8".to_string()),
            GetPlaylistStream("/tmp/hlssink/hi-audio/audio.m3u8".to_string()),
            GetPlaylistStream("/tmp/hlssink/mid-audio/audio.m3u8".to_string()),
            GetFragmentStream("/tmp/hlssink/hi/segment00000.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/hi/segment00001.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/mid/segment00000.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/mid/segment00001.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/low/segment00000.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/low/segment00001.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/hi-audio/segment00000.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/hi-audio/segment00001.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/mid-audio/segment00000.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/mid-audio/segment00001.m4s".to_string()),
        ]
    };
    assert!(is_playlist_events_eq(&expected_events, &actual_events));

    let contents = multivariant_playlist_content.lock().unwrap();

    #[rustfmt::skip]
        assert_eq!(
            r###"#EXTM3U
#EXT-X-VERSION:6
#EXT-X-MEDIA:TYPE=AUDIO,URI="hi-audio/audio.m3u8",GROUP-ID="aac",LANGUAGE="en",NAME="English",DEFAULT=YES
#EXT-X-MEDIA:TYPE=AUDIO,URI="mid-audio/audio.m3u8",GROUP-ID="aac",LANGUAGE="fr",NAME="French"
#EXT-X-STREAM-INF:BANDWIDTH=2500,CODECS="avc1.64001E,avc1.64001F,avc1.640028,mp4a.40.2",AUDIO="aac"
hi/video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=1500,CODECS="avc1.64001E,avc1.64001F,avc1.640028,mp4a.40.2",AUDIO="aac"
mid/video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=700,CODECS="avc1.64001E,avc1.64001F,avc1.640028,mp4a.40.2",AUDIO="aac"
low/video.m3u8
"###,
            contents.to_string()
        );

    Ok(())
}

#[ignore]
#[test]
#[serial]
fn hlsmultivariantsink_multiple_audio_rendition_single_video_variant() -> Result<(), ()> {
    init();

    let pipeline = gst::Pipeline::with_name("hlsmultivariantsink_pipeline");

    let hlsmultivariantsink = gst::ElementFactory::make("hlsmultivariantsink")
        .name("test_hlsmultivariantsink")
        .property(
            "multivariant-playlist-location",
            "/tmp/hlssink/multivariant.m3u8",
        )
        .property("target-duration", 2u32)
        .property("playlist-length", 2u32)
        .property("max-files", 2u32)
        .build()
        .expect("Must be able to instantiate hlsmultivariantsink");

    hlsmultivariantsink.set_property("playlist-type", HlsMultivariantSinkPlaylistType::Event);
    let pl_type: HlsMultivariantSinkPlaylistType = hlsmultivariantsink.property("playlist-type");
    assert_eq!(pl_type, HlsMultivariantSinkPlaylistType::Event);

    hlsmultivariantsink.set_property_from_str("playlist-type", "unspecified");

    let muxer_type: HlsMultivariantSinkMuxerType = hlsmultivariantsink.property("muxer-type");
    assert_eq!(muxer_type, HlsMultivariantSinkMuxerType::Cmaf);

    pipeline.add(&hlsmultivariantsink).unwrap();

    let (hls_events_sender, hls_events_receiver) = mpsc::sync_channel(100);
    let multivariant_playlist_content = Arc::new(Mutex::new(String::from("")));
    let playlist_content = Arc::new(Mutex::new(String::from("")));

    setup_signals(
        &hlsmultivariantsink,
        hls_events_sender.clone(),
        multivariant_playlist_content.clone(),
        playlist_content.clone(),
        HlsMultivariantSinkMuxerType::Cmaf,
    );

    let audio_bin1 = audio_bin(256000).unwrap();
    let audio_bin1_pad = audio_bin1.static_pad("src").unwrap();
    let audio1_pad = hlsmultivariantsink.request_pad_simple("audio_%u").unwrap();
    audio1_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/hi-audio/audio.m3u8".to_string(),
    );
    audio1_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/hi-audio/{DEFAULT_CMAF_LOCATION}"),
    );
    audio1_pad.set_property(
        "init-segment-location",
        format!("/tmp/hlssink/hi-audio/{DEFAULT_INIT_LOCATION}"),
    );
    let r = gst::Structure::builder("audio1-rendition")
        .field("media", "AUDIO")
        .field("uri", "hi-audio/audio.m3u8")
        .field("group_id", "aac")
        .field("language", "en")
        .field("name", "English")
        .field("default", true)
        .field("autoselect", false)
        .build();
    audio1_pad.set_property("alternate-rendition", r);
    pipeline.add(&audio_bin1).unwrap();
    audio_bin1_pad.link(&audio1_pad).unwrap();

    let audio_bin2 = audio_bin(128000).unwrap();
    let audio_bin2_pad = audio_bin2.static_pad("src").unwrap();
    let audio2_pad = hlsmultivariantsink.request_pad_simple("audio_%u").unwrap();
    audio2_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/mid-audio/audio.m3u8".to_string(),
    );
    audio2_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/mid-audio/{DEFAULT_CMAF_LOCATION}"),
    );
    audio2_pad.set_property(
        "init-segment-location",
        format!("/tmp/hlssink/mid-audio/{DEFAULT_INIT_LOCATION}"),
    );
    let r = gst::Structure::builder("audio2-rendition")
        .field("media", "AUDIO")
        .field("uri", "mid-audio/audio.m3u8")
        .field("group_id", "aac")
        .field("language", "fr")
        .field("name", "French")
        .field("default", false)
        .field("autoselect", false)
        .build();
    audio2_pad.set_property("alternate-rendition", r);
    pipeline.add(&audio_bin2).unwrap();
    audio_bin2_pad.link(&audio2_pad).unwrap();

    let video_bin1 = video_bin(1920, 1080, 30, 2500, false, false).unwrap();
    let video_bin1_pad = video_bin1.static_pad("src").unwrap();
    let video1_pad = hlsmultivariantsink.request_pad_simple("video_%u").unwrap();
    video1_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/hi/video.m3u8".to_string(),
    );
    video1_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/hi/{DEFAULT_CMAF_LOCATION}"),
    );
    video1_pad.set_property(
        "init-segment-location",
        format!("/tmp/hlssink/hi/{DEFAULT_INIT_LOCATION}"),
    );
    let v = gst::Structure::builder("video1-variant")
        .field("uri", "hi/video.m3u8")
        .field("audio", "aac")
        .field("bandwidth", 2500)
        .build();
    video1_pad.set_property("variant", v);
    pipeline.add(&video_bin1).unwrap();
    video_bin1_pad.link(&video1_pad).unwrap();

    pipeline.set_state(gst::State::Playing).unwrap();

    let mut eos = false;
    let bus = pipeline.bus().unwrap();
    while let Some(msg) = bus.timed_pop(gst::ClockTime::NONE) {
        use gst::MessageView;
        match msg.view() {
            MessageView::Eos(..) => {
                eos = true;
                break;
            }
            MessageView::Error(e) => gst::error!(CAT, "hlsmultivariantsink error: {}", e),
            _ => (),
        }
    }

    pipeline.debug_to_dot_file_with_ts(
        gst::DebugGraphDetails::all(),
        "multiple_audio_rendition_single_video_variant",
    );

    pipeline.set_state(gst::State::Null).unwrap();
    assert!(eos);

    let mut actual_events = Vec::new();
    while let Ok(event) = hls_events_receiver.recv_timeout(Duration::from_secs(30)) {
        actual_events.push(event);
    }
    let expected_events = {
        use self::HlsSinkEvent::*;
        vec![
            GetMultivariantPlaylistStream("/tmp/hlssink/multivariant.m3u8".to_string()),
            GetInitStream("/tmp/hlssink/hi/init00000.mp4".to_string()),
            GetInitStream("/tmp/hlssink/hi-audio/init00000.mp4".to_string()),
            GetInitStream("/tmp/hlssink/mid-audio/init00000.mp4".to_string()),
            GetPlaylistStream("/tmp/hlssink/hi/video.m3u8".to_string()),
            GetPlaylistStream("/tmp/hlssink/hi-audio/audio.m3u8".to_string()),
            GetPlaylistStream("/tmp/hlssink/mid-audio/audio.m3u8".to_string()),
            GetFragmentStream("/tmp/hlssink/hi/segment00000.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/hi/segment00001.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/hi-audio/segment00000.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/hi-audio/segment00001.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/mid-audio/segment00000.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/mid-audio/segment00001.m4s".to_string()),
        ]
    };
    assert!(is_playlist_events_eq(&expected_events, &actual_events));

    let contents = multivariant_playlist_content.lock().unwrap();

    #[rustfmt::skip]
    assert_eq!(
        r###"#EXTM3U
#EXT-X-VERSION:6
#EXT-X-MEDIA:TYPE=AUDIO,URI="hi-audio/audio.m3u8",GROUP-ID="aac",LANGUAGE="en",NAME="English",DEFAULT=YES
#EXT-X-MEDIA:TYPE=AUDIO,URI="mid-audio/audio.m3u8",GROUP-ID="aac",LANGUAGE="fr",NAME="French"
#EXT-X-STREAM-INF:BANDWIDTH=2500,CODECS="avc1.640028,mp4a.40.2",AUDIO="aac"
hi/video.m3u8
"###,
        contents.to_string()
    );

    Ok(())
}

#[ignore]
#[test]
#[serial]
fn hlsmultivariantsink_single_audio_rendition_multiple_video_variant() -> Result<(), ()> {
    init();

    let pipeline = gst::Pipeline::with_name("hlsmultivariantsink_pipeline");

    let hlsmultivariantsink = gst::ElementFactory::make("hlsmultivariantsink")
        .name("test_hlsmultivariantsink")
        .property(
            "multivariant-playlist-location",
            "/tmp/hlssink/multivariant.m3u8",
        )
        .property("target-duration", 2u32)
        .property("playlist-length", 2u32)
        .property("max-files", 2u32)
        .build()
        .expect("Must be able to instantiate hlsmultivariantsink");

    hlsmultivariantsink.set_property("playlist-type", HlsMultivariantSinkPlaylistType::Event);
    let pl_type: HlsMultivariantSinkPlaylistType = hlsmultivariantsink.property("playlist-type");
    assert_eq!(pl_type, HlsMultivariantSinkPlaylistType::Event);

    hlsmultivariantsink.set_property_from_str("playlist-type", "unspecified");

    let muxer_type: HlsMultivariantSinkMuxerType = hlsmultivariantsink.property("muxer-type");
    assert_eq!(muxer_type, HlsMultivariantSinkMuxerType::Cmaf);

    pipeline.add(&hlsmultivariantsink).unwrap();

    let (hls_events_sender, hls_events_receiver) = mpsc::sync_channel(100);
    let multivariant_playlist_content = Arc::new(Mutex::new(String::from("")));
    let playlist_content = Arc::new(Mutex::new(String::from("")));

    setup_signals(
        &hlsmultivariantsink,
        hls_events_sender.clone(),
        multivariant_playlist_content.clone(),
        playlist_content.clone(),
        HlsMultivariantSinkMuxerType::Cmaf,
    );

    let audio_bin1 = audio_bin(256000).unwrap();
    let audio_bin1_pad = audio_bin1.static_pad("src").unwrap();
    let audio1_pad = hlsmultivariantsink.request_pad_simple("audio_%u").unwrap();
    audio1_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/hi-audio/audio.m3u8".to_string(),
    );
    audio1_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/hi-audio/{DEFAULT_CMAF_LOCATION}"),
    );
    audio1_pad.set_property(
        "init-segment-location",
        format!("/tmp/hlssink/hi-audio/{DEFAULT_INIT_LOCATION}"),
    );
    let r = gst::Structure::builder("audio1-rendition")
        .field("media", "AUDIO")
        .field("uri", "hi-audio/audio.m3u8")
        .field("group_id", "aac")
        .field("language", "en")
        .field("name", "English")
        .field("default", true)
        .field("autoselect", false)
        .build();
    audio1_pad.set_property("alternate-rendition", r);
    pipeline.add(&audio_bin1).unwrap();
    audio_bin1_pad.link(&audio1_pad).unwrap();

    let video_bin1 = video_bin(1920, 1080, 30, 2500, false, false).unwrap();
    let video_bin1_pad = video_bin1.static_pad("src").unwrap();
    let video1_pad = hlsmultivariantsink.request_pad_simple("video_%u").unwrap();
    video1_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/hi/video.m3u8".to_string(),
    );
    video1_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/hi/{DEFAULT_CMAF_LOCATION}"),
    );
    video1_pad.set_property(
        "init-segment-location",
        format!("/tmp/hlssink/hi/{DEFAULT_INIT_LOCATION}"),
    );
    let v = gst::Structure::builder("video1-variant")
        .field("uri", "hi/video.m3u8")
        .field("audio", "aac")
        .field("bandwidth", 2500)
        .build();
    video1_pad.set_property("variant", v);
    pipeline.add(&video_bin1).unwrap();
    video_bin1_pad.link(&video1_pad).unwrap();

    let video_bin2 = video_bin(1280, 720, 30, 1500, false, false).unwrap();
    let video_bin2_pad = video_bin2.static_pad("src").unwrap();
    let video2_pad = hlsmultivariantsink.request_pad_simple("video_%u").unwrap();
    video2_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/mid/video.m3u8".to_string(),
    );
    video2_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/mid/{DEFAULT_CMAF_LOCATION}"),
    );
    video2_pad.set_property(
        "init-segment-location",
        format!("/tmp/hlssink/mid/{DEFAULT_INIT_LOCATION}"),
    );
    let v = gst::Structure::builder("video2-variant")
        .field("uri", "mid/video.m3u8")
        .field("audio", "aac")
        .field("bandwidth", 1500)
        .build();
    video2_pad.set_property("variant", v);
    pipeline.add(&video_bin2).unwrap();
    video_bin2_pad.link(&video2_pad).unwrap();

    let video_bin3 = video_bin(640, 360, 24, 700, false, false).unwrap();
    let video_bin3_pad = video_bin3.static_pad("src").unwrap();
    let video3_pad = hlsmultivariantsink.request_pad_simple("video_%u").unwrap();
    video3_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/low/video.m3u8".to_string(),
    );
    video3_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/low/{DEFAULT_CMAF_LOCATION}"),
    );
    video3_pad.set_property(
        "init-segment-location",
        format!("/tmp/hlssink/low/{DEFAULT_INIT_LOCATION}"),
    );
    let v = gst::Structure::builder("video2-variant")
        .field("uri", "low/video.m3u8")
        .field("audio", "aac")
        .field("bandwidth", 700)
        .build();
    video3_pad.set_property("variant", v);
    pipeline.add(&video_bin3).unwrap();
    video_bin3_pad.link(&video3_pad).unwrap();

    pipeline.set_state(gst::State::Playing).unwrap();

    let mut eos = false;
    let bus = pipeline.bus().unwrap();
    while let Some(msg) = bus.timed_pop(gst::ClockTime::NONE) {
        use gst::MessageView;
        match msg.view() {
            MessageView::Eos(..) => {
                eos = true;
                break;
            }
            MessageView::Error(e) => gst::error!(CAT, "hlsmultivariantsink error: {}", e),
            _ => (),
        }
    }

    pipeline.debug_to_dot_file_with_ts(
        gst::DebugGraphDetails::all(),
        "single_audio_rendition_multiple_video_variant",
    );

    pipeline.set_state(gst::State::Null).unwrap();
    assert!(eos);

    let mut actual_events = Vec::new();
    while let Ok(event) = hls_events_receiver.recv_timeout(Duration::from_secs(30)) {
        actual_events.push(event);
    }
    let expected_events = {
        use self::HlsSinkEvent::*;
        vec![
            GetMultivariantPlaylistStream("/tmp/hlssink/multivariant.m3u8".to_string()),
            GetInitStream("/tmp/hlssink/hi/init00000.mp4".to_string()),
            GetInitStream("/tmp/hlssink/mid/init00000.mp4".to_string()),
            GetInitStream("/tmp/hlssink/low/init00000.mp4".to_string()),
            GetInitStream("/tmp/hlssink/hi-audio/init00000.mp4".to_string()),
            GetPlaylistStream("/tmp/hlssink/hi/video.m3u8".to_string()),
            GetPlaylistStream("/tmp/hlssink/mid/video.m3u8".to_string()),
            GetPlaylistStream("/tmp/hlssink/low/video.m3u8".to_string()),
            GetPlaylistStream("/tmp/hlssink/hi-audio/audio.m3u8".to_string()),
            GetFragmentStream("/tmp/hlssink/hi/segment00000.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/hi/segment00001.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/mid/segment00000.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/mid/segment00001.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/low/segment00000.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/low/segment00001.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/hi-audio/segment00000.m4s".to_string()),
            GetFragmentStream("/tmp/hlssink/hi-audio/segment00001.m4s".to_string()),
        ]
    };
    assert!(is_playlist_events_eq(&expected_events, &actual_events));

    let contents = multivariant_playlist_content.lock().unwrap();

    #[rustfmt::skip]
    assert_eq!(
        r###"#EXTM3U
#EXT-X-VERSION:6
#EXT-X-MEDIA:TYPE=AUDIO,URI="hi-audio/audio.m3u8",GROUP-ID="aac",LANGUAGE="en",NAME="English",DEFAULT=YES
#EXT-X-STREAM-INF:BANDWIDTH=2500,CODECS="avc1.64001E,avc1.64001F,avc1.640028,mp4a.40.2",AUDIO="aac"
hi/video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=1500,CODECS="avc1.64001E,avc1.64001F,avc1.640028,mp4a.40.2",AUDIO="aac"
mid/video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=700,CODECS="avc1.64001E,avc1.64001F,avc1.640028,mp4a.40.2",AUDIO="aac"
low/video.m3u8
"###,
        contents.to_string()
    );

    Ok(())
}

#[ignore]
#[test]
#[serial]
fn hlsmultivariantsink_multiple_audio_rendition_multiple_video_variant_with_mpegts(
) -> Result<(), ()> {
    init();

    let pipeline = gst::Pipeline::with_name("hlsmultivariantsink_pipeline");

    let hlsmultivariantsink = gst::ElementFactory::make("hlsmultivariantsink")
        .name("test_hlsmultivariantsink")
        .property(
            "multivariant-playlist-location",
            "/tmp/hlssink/multivariant.m3u8",
        )
        .property("target-duration", 2u32)
        .property("playlist-length", 2u32)
        .property("max-files", 2u32)
        .build()
        .expect("Must be able to instantiate hlsmultivariantsink");

    hlsmultivariantsink.set_property("muxer-type", HlsMultivariantSinkMuxerType::MpegTs);
    let muxer_type: HlsMultivariantSinkMuxerType = hlsmultivariantsink.property("muxer-type");
    assert_eq!(muxer_type, HlsMultivariantSinkMuxerType::MpegTs);

    pipeline.add(&hlsmultivariantsink).unwrap();

    let (hls_events_sender, hls_events_receiver) = mpsc::sync_channel(100);
    let multivariant_playlist_content = Arc::new(Mutex::new(String::from("")));
    let playlist_content = Arc::new(Mutex::new(String::from("")));

    setup_signals(
        &hlsmultivariantsink,
        hls_events_sender.clone(),
        multivariant_playlist_content.clone(),
        playlist_content.clone(),
        HlsMultivariantSinkMuxerType::MpegTs,
    );

    let audio_bin1 = audio_bin(256000).unwrap();
    let audio_bin1_pad = audio_bin1.static_pad("src").unwrap();
    let audio1_pad = hlsmultivariantsink.request_pad_simple("audio_%u").unwrap();
    audio1_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/hi-audio/audio.m3u8".to_string(),
    );
    audio1_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/hi-audio/{DEFAULT_TS_LOCATION}"),
    );
    let r = gst::Structure::builder("audio1-rendition")
        .field("media", "AUDIO")
        .field("uri", "hi-audio/audio.m3u8")
        .field("group_id", "aac")
        .field("language", "en")
        .field("name", "English")
        .field("default", true)
        .field("autoselect", false)
        .build();
    audio1_pad.set_property("alternate-rendition", r);
    pipeline.add(&audio_bin1).unwrap();
    audio_bin1_pad.link(&audio1_pad).unwrap();

    let audio_bin2 = audio_bin(128000).unwrap();
    let audio_bin2_pad = audio_bin2.static_pad("src").unwrap();
    let audio2_pad = hlsmultivariantsink.request_pad_simple("audio_%u").unwrap();
    audio2_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/mid-audio/audio.m3u8".to_string(),
    );
    audio2_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/mid-audio/{DEFAULT_TS_LOCATION}"),
    );
    let r = gst::Structure::builder("audio2-rendition")
        .field("media", "AUDIO")
        .field("uri", "mid-audio/audio.m3u8")
        .field("group_id", "aac")
        .field("language", "fr")
        .field("name", "French")
        .field("default", false)
        .field("autoselect", false)
        .build();
    audio2_pad.set_property("alternate-rendition", r);
    pipeline.add(&audio_bin2).unwrap();
    audio_bin2_pad.link(&audio2_pad).unwrap();

    let video_bin1 = video_bin(1920, 1080, 30, 2500, false, false).unwrap();
    let video_bin1_pad = video_bin1.static_pad("src").unwrap();
    let video1_pad = hlsmultivariantsink.request_pad_simple("video_%u").unwrap();
    video1_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/hi/video.m3u8".to_string(),
    );
    video1_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/hi/{DEFAULT_TS_LOCATION}"),
    );
    let v = gst::Structure::builder("video1-variant")
        .field("uri", "hi/video.m3u8")
        .field("audio", "aac")
        .field("bandwidth", 2500)
        .build();
    video1_pad.set_property("variant", v);
    pipeline.add(&video_bin1).unwrap();
    video_bin1_pad.link(&video1_pad).unwrap();

    let video_bin2 = video_bin(1280, 720, 30, 1500, false, false).unwrap();
    let video_bin2_pad = video_bin2.static_pad("src").unwrap();
    let video2_pad = hlsmultivariantsink.request_pad_simple("video_%u").unwrap();
    video2_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/mid/video.m3u8".to_string(),
    );
    video2_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/mid/{DEFAULT_TS_LOCATION}"),
    );
    let v = gst::Structure::builder("video2-variant")
        .field("uri", "mid/video.m3u8")
        .field("audio", "aac")
        .field("bandwidth", 1500)
        .build();
    video2_pad.set_property("variant", v);
    pipeline.add(&video_bin2).unwrap();
    video_bin2_pad.link(&video2_pad).unwrap();

    let video_bin3 = video_bin(640, 360, 24, 700, false, false).unwrap();
    let video_bin3_pad = video_bin3.static_pad("src").unwrap();
    let video3_pad = hlsmultivariantsink.request_pad_simple("video_%u").unwrap();
    video3_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/low/video.m3u8".to_string(),
    );
    video3_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/low/{DEFAULT_TS_LOCATION}"),
    );
    let v = gst::Structure::builder("video3-variant")
        .field("uri", "low/video.m3u8")
        .field("audio", "aac")
        .field("bandwidth", 700)
        .build();
    video3_pad.set_property("variant", v);
    pipeline.add(&video_bin3).unwrap();
    video_bin3_pad.link(&video3_pad).unwrap();

    pipeline.set_state(gst::State::Playing).unwrap();

    let mut eos = false;
    let bus = pipeline.bus().unwrap();
    while let Some(msg) = bus.timed_pop(gst::ClockTime::NONE) {
        use gst::MessageView;
        match msg.view() {
            MessageView::Eos(..) => {
                eos = true;
                break;
            }
            MessageView::Error(e) => gst::error!(CAT, "hlsmultivariantsink error: {}", e),
            _ => (),
        }
    }

    pipeline.debug_to_dot_file_with_ts(
        gst::DebugGraphDetails::all(),
        "multiple_audio_rendition_multiple_video_variant_with_mpegts",
    );

    pipeline.set_state(gst::State::Null).unwrap();
    assert!(eos);

    let mut actual_events = Vec::new();
    while let Ok(event) = hls_events_receiver.recv_timeout(Duration::from_secs(30)) {
        actual_events.push(event);
    }
    let expected_events = {
        use self::HlsSinkEvent::*;
        vec![
            GetMultivariantPlaylistStream("/tmp/hlssink/multivariant.m3u8".to_string()),
            GetPlaylistStream("/tmp/hlssink/hi/video.m3u8".to_string()),
            GetPlaylistStream("/tmp/hlssink/mid/video.m3u8".to_string()),
            GetPlaylistStream("/tmp/hlssink/low/video.m3u8".to_string()),
            GetPlaylistStream("/tmp/hlssink/hi-audio/audio.m3u8".to_string()),
            GetPlaylistStream("/tmp/hlssink/mid-audio/audio.m3u8".to_string()),
            GetFragmentStream("/tmp/hlssink/hi-audio/segment00000.ts".to_string()),
            GetFragmentStream("/tmp/hlssink/mid-audio/segment00000.ts".to_string()),
            GetFragmentStream("/tmp/hlssink/hi/segment00000.ts".to_string()),
            GetFragmentStream("/tmp/hlssink/mid/segment00000.ts".to_string()),
            GetFragmentStream("/tmp/hlssink/low/segment00000.ts".to_string()),
            GetFragmentStream("/tmp/hlssink/hi/segment00001.ts".to_string()),
            GetFragmentStream("/tmp/hlssink/mid/segment00001.ts".to_string()),
            GetFragmentStream("/tmp/hlssink/low/segment00001.ts".to_string()),
        ]
    };
    assert!(is_playlist_events_eq(&expected_events, &actual_events));

    let contents = multivariant_playlist_content.lock().unwrap();

    #[rustfmt::skip]
    assert_eq!(
        r###"#EXTM3U
#EXT-X-VERSION:4
#EXT-X-MEDIA:TYPE=AUDIO,URI="hi-audio/audio.m3u8",GROUP-ID="aac",LANGUAGE="en",NAME="English",DEFAULT=YES
#EXT-X-MEDIA:TYPE=AUDIO,URI="mid-audio/audio.m3u8",GROUP-ID="aac",LANGUAGE="fr",NAME="French"
#EXT-X-STREAM-INF:BANDWIDTH=2500,CODECS="avc1.64001E,avc1.64001F,avc1.640028,mp4a.40.2",AUDIO="aac"
hi/video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=1500,CODECS="avc1.64001E,avc1.64001F,avc1.640028,mp4a.40.2",AUDIO="aac"
mid/video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=700,CODECS="avc1.64001E,avc1.64001F,avc1.640028,mp4a.40.2",AUDIO="aac"
low/video.m3u8
"###,
        contents.to_string()
    );

    Ok(())
}

#[ignore]
#[test]
#[serial]
fn hlsmultivariantsink_multiple_video_variant_with_mpegts_audio_video_muxed() -> Result<(), ()> {
    init();

    let pipeline = gst::Pipeline::with_name("hlsmultivariantsink_pipeline");

    let hlsmultivariantsink = gst::ElementFactory::make("hlsmultivariantsink")
        .name("test_hlsmultivariantsink")
        .property(
            "multivariant-playlist-location",
            "/tmp/hlssink/multivariant.m3u8",
        )
        .property("target-duration", 2u32)
        .property("playlist-length", 2u32)
        .property("max-files", 2u32)
        .build()
        .expect("Must be able to instantiate hlsmultivariantsink");

    hlsmultivariantsink.set_property("muxer-type", HlsMultivariantSinkMuxerType::MpegTs);
    let muxer_type: HlsMultivariantSinkMuxerType = hlsmultivariantsink.property("muxer-type");
    assert_eq!(muxer_type, HlsMultivariantSinkMuxerType::MpegTs);

    pipeline.add(&hlsmultivariantsink).unwrap();

    let (hls_events_sender, hls_events_receiver) = mpsc::sync_channel(20);
    let multivariant_playlist_content = Arc::new(Mutex::new(String::from("")));
    let playlist_content = Arc::new(Mutex::new(String::from("")));

    setup_signals(
        &hlsmultivariantsink,
        hls_events_sender.clone(),
        multivariant_playlist_content.clone(),
        playlist_content.clone(),
        HlsMultivariantSinkMuxerType::MpegTs,
    );

    let video_bin1 = video_bin(1920, 1080, 30, 2500, false, false).unwrap();
    let video_bin1_pad = video_bin1.static_pad("src").unwrap();
    let video1_pad = hlsmultivariantsink.request_pad_simple("video_%u").unwrap();
    video1_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/hi/video.m3u8".to_string(),
    );
    video1_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/hi/{DEFAULT_TS_LOCATION}"),
    );
    let v = gst::Structure::builder("video1-variant")
        .field("uri", "hi/video.m3u8")
        .field("bandwidth", 2500)
        .field("codecs", "avc1,mp4a.40.2")
        .build();
    video1_pad.set_property("variant", v);
    pipeline.add(&video_bin1).unwrap();
    video_bin1_pad.link(&video1_pad).unwrap();

    let video_bin2 = video_bin(1280, 720, 30, 1500, false, false).unwrap();
    let video_bin2_pad = video_bin2.static_pad("src").unwrap();
    let video2_pad = hlsmultivariantsink.request_pad_simple("video_%u").unwrap();
    video2_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/mid/video.m3u8".to_string(),
    );
    video2_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/mid/{DEFAULT_TS_LOCATION}"),
    );
    let v = gst::Structure::builder("video2-variant")
        .field("uri", "mid/video.m3u8")
        .field("bandwidth", 1500)
        .field("codecs", "avc1,mp4a.40.2")
        .build();
    video2_pad.set_property("variant", v);
    pipeline.add(&video_bin2).unwrap();
    video_bin2_pad.link(&video2_pad).unwrap();

    /*
     * Note that the next two audio variants have the same URI as
     * the above two video variants. In the MPEG-TS case, this will
     * result in them being muxed.
     */
    let audio_bin1 = audio_bin(256000).unwrap();
    let audio_bin1_pad = audio_bin1.static_pad("src").unwrap();
    let audio1_pad = hlsmultivariantsink.request_pad_simple("audio_%u").unwrap();
    audio1_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/hi/video.m3u8".to_string(),
    );
    audio1_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/hi/{DEFAULT_TS_LOCATION}"),
    );
    let v = gst::Structure::builder("audio1-variant")
        .field("uri", "hi/video.m3u8")
        .field("bandwidth", 256000)
        .field("codecs", "avc1,mp4a.40.2")
        .build();
    audio1_pad.set_property("variant", v);
    pipeline.add(&audio_bin1).unwrap();
    audio_bin1_pad.link(&audio1_pad).unwrap();

    let audio_bin2 = audio_bin(128000).unwrap();
    let audio_bin2_pad = audio_bin2.static_pad("src").unwrap();
    let audio2_pad = hlsmultivariantsink.request_pad_simple("audio_%u").unwrap();
    audio2_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/mid/video.m3u8".to_string(),
    );
    audio2_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/mid/{DEFAULT_TS_LOCATION}"),
    );
    let v = gst::Structure::builder("audio2-variant")
        .field("uri", "mid/video.m3u8")
        .field("bandwidth", 128000)
        .field("codecs", "avc1,mp4a.40.2")
        .build();
    audio2_pad.set_property("variant", v);
    pipeline.add(&audio_bin2).unwrap();
    audio_bin2_pad.link(&audio2_pad).unwrap();

    /* Audio only variant, not muxed with video */
    let audio_bin3 = audio_bin(64000).unwrap();
    let audio_bin3_pad = audio_bin3.static_pad("src").unwrap();
    let audio3_pad = hlsmultivariantsink.request_pad_simple("audio_%u").unwrap();
    audio3_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/low-audio/audio-only.m3u8".to_string(),
    );
    audio3_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/low-audio/{DEFAULT_TS_LOCATION}"),
    );
    let v = gst::Structure::builder("audio3-variant")
        .field("uri", "low-audio/audio-only.m3u8")
        .field("bandwidth", 64000)
        .field("codecs", "mp4a.40.2")
        .build();
    audio3_pad.set_property("variant", v);
    pipeline.add(&audio_bin3).unwrap();
    audio_bin3_pad.link(&audio3_pad).unwrap();

    pipeline.set_state(gst::State::Playing).unwrap();

    let mut eos = false;
    let bus = pipeline.bus().unwrap();
    while let Some(msg) = bus.timed_pop(gst::ClockTime::NONE) {
        use gst::MessageView;
        match msg.view() {
            MessageView::Eos(..) => {
                eos = true;
                break;
            }
            MessageView::Error(e) => gst::error!(CAT, "hlsmultivariantsink error: {}", e),
            _ => (),
        }
    }

    pipeline.debug_to_dot_file_with_ts(
        gst::DebugGraphDetails::all(),
        "multiple_video_variant_with_mpegts_audio_video_muxed",
    );

    pipeline.set_state(gst::State::Null).unwrap();
    assert!(eos);

    let mut actual_events = Vec::new();
    while let Ok(event) = hls_events_receiver.recv_timeout(Duration::from_secs(30)) {
        actual_events.push(event);
    }
    let expected_events = {
        use self::HlsSinkEvent::*;
        vec![
            GetMultivariantPlaylistStream("/tmp/hlssink/multivariant.m3u8".to_string()),
            GetFragmentStream("/tmp/hlssink/hi/segment00000.ts".to_string()),
            GetFragmentStream("/tmp/hlssink/mid/segment00000.ts".to_string()),
            GetFragmentStream("/tmp/hlssink/mid/segment00001.ts".to_string()),
            GetFragmentStream("/tmp/hlssink/hi/segment00001.ts".to_string()),
            GetFragmentStream("/tmp/hlssink/low-audio/segment00000.ts".to_string()),
            GetPlaylistStream("/tmp/hlssink/hi/video.m3u8".to_string()),
            GetPlaylistStream("/tmp/hlssink/mid/video.m3u8".to_string()),
            GetPlaylistStream("/tmp/hlssink/low-audio/audio-only.m3u8".to_string()),
        ]
    };
    assert!(is_playlist_events_eq(&expected_events, &actual_events));

    let contents = multivariant_playlist_content.lock().unwrap();

    #[rustfmt::skip]
    assert_eq!(
        r###"#EXTM3U
#EXT-X-VERSION:4
#EXT-X-STREAM-INF:BANDWIDTH=2500,CODECS="avc1.640028,mp4a.40.2"
hi/video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=1500,CODECS="avc1.64001F,mp4a.40.2"
mid/video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=64000,CODECS="mp4a.40.2"
low-audio/audio-only.m3u8
"###,
        contents.to_string()
    );

    Ok(())
}

#[ignore]
#[test]
#[serial]
fn hlsmultivariantsink_multiple_audio_rendition_multiple_video_variant_with_mpegts_h265(
) -> Result<(), ()> {
    init();

    // Skip the test if `x265enc` is not available which is the case on CI
    if gst::ElementFactory::make("x265enc").build().is_err() {
        gst::info!(CAT, "Skipping test for MPEG-TS with H265");
        return Ok(());
    } else {
        gst::info!(CAT, "Testing for MPEG-TS with H265");
    }

    let pipeline = gst::Pipeline::with_name("hlsmultivariantsink_pipeline");

    let hlsmultivariantsink = gst::ElementFactory::make("hlsmultivariantsink")
        .name("test_hlsmultivariantsink")
        .property(
            "multivariant-playlist-location",
            "/tmp/hlssink/multivariant.m3u8",
        )
        .property("target-duration", 2u32)
        .property("playlist-length", 2u32)
        .property("max-files", 2u32)
        .build()
        .expect("Must be able to instantiate hlsmultivariantsink");

    hlsmultivariantsink.set_property("muxer-type", HlsMultivariantSinkMuxerType::MpegTs);
    let muxer_type: HlsMultivariantSinkMuxerType = hlsmultivariantsink.property("muxer-type");
    assert_eq!(muxer_type, HlsMultivariantSinkMuxerType::MpegTs);

    pipeline.add(&hlsmultivariantsink).unwrap();

    let (hls_events_sender, hls_events_receiver) = mpsc::sync_channel(100);
    let multivariant_playlist_content = Arc::new(Mutex::new(String::from("")));
    let playlist_content = Arc::new(Mutex::new(String::from("")));

    setup_signals(
        &hlsmultivariantsink,
        hls_events_sender.clone(),
        multivariant_playlist_content.clone(),
        playlist_content.clone(),
        HlsMultivariantSinkMuxerType::MpegTs,
    );

    let audio_bin1 = audio_bin(256000).unwrap();
    let audio_bin1_pad = audio_bin1.static_pad("src").unwrap();
    let audio1_pad = hlsmultivariantsink.request_pad_simple("audio_%u").unwrap();
    audio1_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/hi-audio/audio.m3u8".to_string(),
    );
    audio1_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/hi-audio/{DEFAULT_TS_LOCATION}"),
    );
    let r = gst::Structure::builder("audio1-rendition")
        .field("media", "AUDIO")
        .field("uri", "hi-audio/audio.m3u8")
        .field("group_id", "aac")
        .field("language", "en")
        .field("name", "English")
        .field("default", true)
        .field("autoselect", false)
        .build();
    audio1_pad.set_property("alternate-rendition", r);
    pipeline.add(&audio_bin1).unwrap();
    audio_bin1_pad.link(&audio1_pad).unwrap();

    let audio_bin2 = audio_bin(128000).unwrap();
    let audio_bin2_pad = audio_bin2.static_pad("src").unwrap();
    let audio2_pad = hlsmultivariantsink.request_pad_simple("audio_%u").unwrap();
    audio2_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/mid-audio/audio.m3u8".to_string(),
    );
    audio2_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/mid-audio/{DEFAULT_TS_LOCATION}"),
    );
    let r = gst::Structure::builder("audio2-rendition")
        .field("media", "AUDIO")
        .field("uri", "mid-audio/audio.m3u8")
        .field("group_id", "aac")
        .field("language", "fr")
        .field("name", "French")
        .field("default", false)
        .field("autoselect", false)
        .build();
    audio2_pad.set_property("alternate-rendition", r);
    pipeline.add(&audio_bin2).unwrap();
    audio_bin2_pad.link(&audio2_pad).unwrap();

    let video_bin1 = video_bin(1920, 1080, 30, 2500, false, true).unwrap();
    let video_bin1_pad = video_bin1.static_pad("src").unwrap();
    let video1_pad = hlsmultivariantsink.request_pad_simple("video_%u").unwrap();
    video1_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/hi/video.m3u8".to_string(),
    );
    video1_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/hi/{DEFAULT_TS_LOCATION}"),
    );
    let v = gst::Structure::builder("video1-variant")
        .field("uri", "hi/video.m3u8")
        .field("audio", "aac")
        .field("bandwidth", 2500)
        .build();
    video1_pad.set_property("variant", v);
    pipeline.add(&video_bin1).unwrap();
    video_bin1_pad.link(&video1_pad).unwrap();

    let video_bin2 = video_bin(1280, 720, 30, 1500, false, true).unwrap();
    let video_bin2_pad = video_bin2.static_pad("src").unwrap();
    let video2_pad = hlsmultivariantsink.request_pad_simple("video_%u").unwrap();
    video2_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/mid/video.m3u8".to_string(),
    );
    video2_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/mid/{DEFAULT_TS_LOCATION}"),
    );
    let v = gst::Structure::builder("video2-variant")
        .field("uri", "mid/video.m3u8")
        .field("audio", "aac")
        .field("bandwidth", 1500)
        .build();
    video2_pad.set_property("variant", v);
    pipeline.add(&video_bin2).unwrap();
    video_bin2_pad.link(&video2_pad).unwrap();

    let video_bin3 = video_bin(640, 360, 24, 700, false, true).unwrap();
    let video_bin3_pad = video_bin3.static_pad("src").unwrap();
    let video3_pad = hlsmultivariantsink.request_pad_simple("video_%u").unwrap();
    video3_pad.set_property(
        "playlist-location",
        "/tmp/hlssink/low/video.m3u8".to_string(),
    );
    video3_pad.set_property(
        "segment-location",
        format!("/tmp/hlssink/low/{DEFAULT_TS_LOCATION}"),
    );
    let v = gst::Structure::builder("video3-variant")
        .field("uri", "low/video.m3u8")
        .field("audio", "aac")
        .field("bandwidth", 700)
        .build();
    video3_pad.set_property("variant", v);
    pipeline.add(&video_bin3).unwrap();
    video_bin3_pad.link(&video3_pad).unwrap();

    pipeline.set_state(gst::State::Playing).unwrap();

    let mut eos = false;
    let bus = pipeline.bus().unwrap();
    while let Some(msg) = bus.timed_pop(gst::ClockTime::NONE) {
        use gst::MessageView;
        match msg.view() {
            MessageView::Eos(..) => {
                eos = true;
                break;
            }
            MessageView::Error(e) => gst::error!(CAT, "hlsmultivariantsink error: {}", e),
            _ => (),
        }
    }

    pipeline.debug_to_dot_file_with_ts(
        gst::DebugGraphDetails::all(),
        "multiple_audio_rendition_multiple_video_variant_with_mpegts",
    );

    pipeline.set_state(gst::State::Null).unwrap();
    assert!(eos);

    let mut actual_events = Vec::new();
    while let Ok(event) = hls_events_receiver.recv_timeout(Duration::from_secs(30)) {
        actual_events.push(event);
    }
    let expected_events = {
        use self::HlsSinkEvent::*;
        vec![
            GetMultivariantPlaylistStream("/tmp/hlssink/multivariant.m3u8".to_string()),
            GetPlaylistStream("/tmp/hlssink/hi/video.m3u8".to_string()),
            GetPlaylistStream("/tmp/hlssink/mid/video.m3u8".to_string()),
            GetPlaylistStream("/tmp/hlssink/low/video.m3u8".to_string()),
            GetPlaylistStream("/tmp/hlssink/hi-audio/audio.m3u8".to_string()),
            GetPlaylistStream("/tmp/hlssink/mid-audio/audio.m3u8".to_string()),
            GetFragmentStream("/tmp/hlssink/hi-audio/segment00000.ts".to_string()),
            GetFragmentStream("/tmp/hlssink/mid-audio/segment00000.ts".to_string()),
            GetFragmentStream("/tmp/hlssink/hi/segment00000.ts".to_string()),
            GetFragmentStream("/tmp/hlssink/mid/segment00000.ts".to_string()),
            GetFragmentStream("/tmp/hlssink/low/segment00000.ts".to_string()),
            GetFragmentStream("/tmp/hlssink/hi/segment00001.ts".to_string()),
            GetFragmentStream("/tmp/hlssink/mid/segment00001.ts".to_string()),
            GetFragmentStream("/tmp/hlssink/low/segment00001.ts".to_string()),
        ]
    };
    assert!(is_playlist_events_eq(&expected_events, &actual_events));

    let contents = multivariant_playlist_content.lock().unwrap();

    #[rustfmt::skip]
    assert_eq!(
        r###"#EXTM3U
#EXT-X-VERSION:4
#EXT-X-MEDIA:TYPE=AUDIO,URI="hi-audio/audio.m3u8",GROUP-ID="aac",LANGUAGE="en",NAME="English",DEFAULT=YES
#EXT-X-MEDIA:TYPE=AUDIO,URI="mid-audio/audio.m3u8",GROUP-ID="aac",LANGUAGE="fr",NAME="French"
#EXT-X-STREAM-INF:BANDWIDTH=2500,CODECS="hvc1.1.6.L120.90,hvc1.1.6.L63.90,hvc1.1.6.L93.90,mp4a.40.2",AUDIO="aac"
hi/video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=1500,CODECS="hvc1.1.6.L120.90,hvc1.1.6.L63.90,hvc1.1.6.L93.90,mp4a.40.2",AUDIO="aac"
mid/video.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=700,CODECS="hvc1.1.6.L120.90,hvc1.1.6.L63.90,hvc1.1.6.L93.90,mp4a.40.2",AUDIO="aac"
low/video.m3u8
"###,
        contents.to_string()
    );

    Ok(())
}
