// Copyright (C) 2022 Mathieu Duponchelle <mathieu@centricular.com>
// Copyright (C) 2023 Seungha Yang <seungha@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

// This creates a master playlist for live HLS CMAF stream with one video playlist and two audio playlists.

use gst::glib;
use gst::prelude::*;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::Error;

use m3u8_rs::{AlternativeMedia, AlternativeMediaType, MasterPlaylist, VariantStream};

const VIDEO_WIDTH: u32 = 640;
const VIDEO_HEIGHT: u32 = 480;
const VIDEO_BITRATE: u32 = 2_048_000;

fn create_sink(path: &Path, name: &str) -> gst::Element {
    let mut path: PathBuf = path.into();
    path.push(name);
    std::fs::create_dir_all(&path).expect("failed to create directory");

    let mut playlist_location: PathBuf = path.clone();
    playlist_location.push("manifest.m3u8");

    let mut init_location: PathBuf = path.clone();
    init_location.push("init_%03d.mp4");

    let mut location: PathBuf = path.clone();
    location.push("segment_%05d.m4s");

    let sink = gst::ElementFactory::make("hlscmafsink")
        .name(name)
        .property("target-duration", 5u32)
        .property("playlist-location", playlist_location.to_str().unwrap())
        .property("init-location", init_location.to_str().unwrap())
        .property("location", location.to_str().unwrap())
        .property("enable-program-date-time", true)
        // Need sync=true for cmafmux to timeout properly in case of live pipeline
        .property("sync", true)
        .build()
        .expect("failed to create hlscmafsink");

    // The same as default implementation of hlscmafsink.
    // Connecting signals here to print debug log to stdout
    sink.connect_closure(
        "get-init-stream",
        false,
        glib::closure!(move |sink: &gst::Element, location: &str| {
            println!("{}, writing init segment to {location}", sink.name());
            let file = std::fs::File::create(location).unwrap();
            gio::WriteOutputStream::new(file).upcast::<gio::OutputStream>()
        }),
    );

    sink.connect_closure(
        "get-fragment-stream",
        false,
        glib::closure!(move |sink: &gst::Element, location: &str| {
            println!("{}, writing segment to {location}", sink.name());
            let file = std::fs::File::create(location).unwrap();
            gio::WriteOutputStream::new(file).upcast::<gio::OutputStream>()
        }),
    );

    sink.connect_closure(
        "delete-fragment",
        false,
        glib::closure!(move |sink: &gst::Element, location: &str| {
            println!("{}, removing segment {location}", sink.name());
            std::fs::remove_file(location).unwrap();

            true
        }),
    );

    sink
}

fn setup_video_sink(pipeline: &gst::Pipeline, path: &Path, name: &str) -> Result<(), Error> {
    let src = gst::ElementFactory::make("videotestsrc")
        .property("is-live", true)
        .build()?;

    let raw_capsfilter = gst::ElementFactory::make("capsfilter")
        .property(
            "caps",
            gst_video::VideoCapsBuilder::new()
                .format(gst_video::VideoFormat::I420)
                .width(VIDEO_WIDTH as i32)
                .height(VIDEO_HEIGHT as i32)
                .framerate(30.into())
                .build(),
        )
        .build()?;

    let timeoverlay = gst::ElementFactory::make("timeoverlay").build()?;
    let queue = gst::ElementFactory::make("queue").build()?;
    let enc = gst::ElementFactory::make("x264enc")
        .property("bframes", 0u32)
        .property("bitrate", VIDEO_BITRATE / 1000u32)
        .property("key-int-max", i32::MAX as u32)
        .property_from_str("tune", "zerolatency")
        .build()?;
    let h264_capsfilter = gst::ElementFactory::make("capsfilter")
        .property(
            "caps",
            gst::Caps::builder("video/x-h264")
                .field("profile", "main")
                .build(),
        )
        .build()?;
    let sink = create_sink(path, name);

    pipeline.add_many([
        &src,
        &raw_capsfilter,
        &timeoverlay,
        &queue,
        &enc,
        &h264_capsfilter,
        &sink,
    ])?;

    gst::Element::link_many([
        &src,
        &raw_capsfilter,
        &timeoverlay,
        &queue,
        &enc,
        &h264_capsfilter,
        &sink,
    ])?;

    Ok(())
}

fn setup_audio_sink(
    pipeline: &gst::Pipeline,
    path: &Path,
    name: &str,
    wave: &str,
) -> Result<(), Error> {
    let src = gst::ElementFactory::make("audiotestsrc")
        .property("is-live", true)
        .property_from_str("wave", wave)
        .build()?;
    let capsfilter = gst::ElementFactory::make("capsfilter")
        .property(
            "caps",
            gst::Caps::builder("audio/x-raw")
                .field("channels", 2)
                .build(),
        )
        .build()?;
    let queue = gst::ElementFactory::make("queue").build()?;
    let enc = gst::ElementFactory::make("avenc_aac").build()?;
    let parse = gst::ElementFactory::make("aacparse").build()?;
    let sink = create_sink(path, name);

    pipeline.add_many([&src, &capsfilter, &queue, &enc, &parse, &sink])?;
    gst::Element::link_many([&src, &capsfilter, &queue, &enc, &parse, &sink])?;

    Ok(())
}

fn get_codec_string(pipeline: &gst::Pipeline, name: &str) -> String {
    let sink = pipeline.by_name(name).unwrap();
    let pad = sink.static_pad("sink").unwrap();
    let caps = pad.sticky_event::<gst::event::Caps>(0).unwrap();
    gst_pbutils::codec_utils_caps_get_mime_codec(caps.caps())
        .unwrap()
        .to_string()
}

fn write_master_playlist(pipeline: &gst::Pipeline, path: &PathBuf) {
    // Gets configured caps and constructs CODEC string
    let video_codec = get_codec_string(pipeline, "video_0");

    // Both audios should have the same caps in this example
    let audio_codec = get_codec_string(pipeline, "audio_0");

    let codecs = format!("{video_codec},{audio_codec}");

    let variants = vec![VariantStream {
        uri: "video_0/manifest.m3u8".to_string(),
        codecs: Some(codecs),
        bandwidth: VIDEO_BITRATE as u64,
        resolution: Some(m3u8_rs::Resolution {
            width: VIDEO_WIDTH as u64,
            height: VIDEO_HEIGHT as u64,
        }),
        audio: Some("audio".to_string()),
        ..Default::default()
    }];

    let mut alternatives = Vec::new();
    for i in 0..2 {
        let name = format!("audio_{i}");
        let language = if i == 0 {
            Some("enc".to_string())
        } else {
            Some("fre".to_string())
        };
        alternatives.push(AlternativeMedia {
            media_type: AlternativeMediaType::Audio,
            uri: Some(format!("{name}/manifest.m3u8")),
            group_id: "audio".to_string(),
            language,
            name,
            default: i == 0,
            autoselect: i == 0,
            channels: Some("2".to_string()),
            ..Default::default()
        })
    }

    let playlist = MasterPlaylist {
        version: Some(6),
        variants,
        alternatives,
        independent_segments: true,
        ..Default::default()
    };

    println!("Writing master manifest to {}", path.display());

    let mut file = std::fs::File::create(path).unwrap();
    playlist
        .write_to(&mut file)
        .expect("Failed to write master playlist");
}

fn main() -> Result<(), Error> {
    gst::init()?;

    gsthlssink3::plugin_register_static()?;

    let path = PathBuf::from("hls_live_stream");

    std::fs::create_dir_all(&path).expect("failed to create directory");

    let mut manifest_path = path.clone();
    manifest_path.push("manifest.m3u8");

    let pipeline = gst::Pipeline::default();
    setup_video_sink(&pipeline, &path, "video_0")?;
    setup_audio_sink(&pipeline, &path, "audio_0", "sine")?;
    setup_audio_sink(&pipeline, &path, "audio_1", "white-noise")?;

    pipeline.set_state(gst::State::Playing)?;

    let bus = pipeline
        .bus()
        .expect("Pipeline without bus. Shouldn't happen!");

    let pipeline_weak = pipeline.downgrade();
    let write_playlist = Arc::new(Mutex::new(true));
    for msg in bus.iter_timed(gst::ClockTime::NONE) {
        use gst::MessageView;

        match msg.view() {
            MessageView::StateChanged(state_changed) => {
                let Some(pipeline) = pipeline_weak.upgrade() else {
                    break;
                };

                let mut need_write = write_playlist.lock().unwrap();
                if *need_write
                    && state_changed.src() == Some(pipeline.upcast_ref())
                    && state_changed.old() == gst::State::Paused
                    && state_changed.current() == gst::State::Playing
                {
                    *need_write = false;
                    write_master_playlist(&pipeline, &manifest_path);
                }
            }
            MessageView::Eos(..) => {
                println!("EOS");
                break;
            }
            MessageView::Error(err) => {
                pipeline.set_state(gst::State::Null)?;
                eprintln!(
                    "Got error from {}: {} ({})",
                    msg.src()
                        .map(|s| String::from(s.path_string()))
                        .unwrap_or_else(|| "None".into()),
                    err.error(),
                    err.debug().unwrap_or_else(|| "".into()),
                );
                break;
            }
            _ => (),
        }
    }

    pipeline.set_state(gst::State::Null)?;

    Ok(())
}
