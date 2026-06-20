// Copyright (C) 2022 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

// This creates a live HLS stream with one video playlist and two video playlists.
// Basic trimming is implemented

use gst::prelude::*;

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::Error;
use chrono::{DateTime, Duration, Utc};
use m3u8_rs::{
    AlternativeMedia, AlternativeMediaType, MasterPlaylist, MediaPlaylist, MediaSegment,
    VariantStream,
};

struct State {
    video_streams: Vec<VideoStream>,
    audio_streams: Vec<AudioStream>,
    all_mimes: Vec<String>,
    path: PathBuf,
    wrote_manifest: bool,
}

impl State {
    fn maybe_write_manifest(&mut self) {
        if self.wrote_manifest {
            return;
        }

        if self.all_mimes.len() < self.video_streams.len() + self.audio_streams.len() {
            return;
        }

        let mut all_mimes = self.all_mimes.clone();
        all_mimes.sort();
        all_mimes.dedup();

        let playlist = MasterPlaylist {
            version: Some(7),
            variants: self
                .video_streams
                .iter()
                .map(|stream| {
                    let mut path = PathBuf::new();

                    path.push(&stream.name);
                    path.push("manifest.m3u8");

                    VariantStream {
                        uri: path.as_path().display().to_string(),
                        bandwidth: stream.bitrate,
                        codecs: Some(all_mimes.join(",")),
                        resolution: Some(m3u8_rs::Resolution {
                            width: stream.width,
                            height: stream.height,
                        }),
                        audio: Some("audio".to_string()),
                        ..Default::default()
                    }
                })
                .collect(),
            alternatives: self
                .audio_streams
                .iter()
                .map(|stream| {
                    let mut path = PathBuf::new();
                    path.push(&stream.name);
                    path.push("manifest.m3u8");

                    AlternativeMedia {
                        media_type: AlternativeMediaType::Audio,
                        uri: Some(path.as_path().display().to_string()),
                        group_id: "audio".to_string(),
                        language: Some(stream.lang.clone()),
                        name: stream.name.clone(),
                        default: stream.default,
                        autoselect: stream.default,
                        channels: Some("2".to_string()),
                        ..Default::default()
                    }
                })
                .collect(),
            independent_segments: true,
            ..Default::default()
        };

        println!("Writing master manifest to {}", self.path.display());

        let mut file = std::fs::File::create(&self.path).unwrap();
        playlist
            .write_to(&mut file)
            .expect("Failed to write master playlist");

        self.wrote_manifest = true;
    }
}

struct Segment {
    date_time: DateTime<Utc>,
    duration: gst::ClockTime,
    path: String,
}

struct UnreffedSegment {
    removal_time: DateTime<Utc>,
    path: String,
}

struct StreamState {
    path: PathBuf,
    segments: VecDeque<Segment>,
    trimmed_segments: VecDeque<UnreffedSegment>,
    start_date_time: Option<DateTime<Utc>>,
    start_time: Option<gst::ClockTime>,
    media_sequence: u64,
    segment_index: u32,
}

struct VideoStream {
    name: String,
    bitrate: u64,
    width: u64,
    height: u64,
}

struct AudioStream {
    name: String,
    lang: String,
    default: bool,
    wave: String,
}

fn trim_segments(state: &mut StreamState) {
    // Arbitrary 5 segments window
    while state.segments.len() > 5 {
        let segment = state.segments.pop_front().unwrap();

        state.media_sequence += 1;

        state.trimmed_segments.push_back(UnreffedSegment {
            // HLS spec mandates that segments are removed from the filesystem no sooner
            // than the duration of the longest playlist + duration of the segment.
            // This is 18 seconds (15 + 3) in our case, we use 25 seconds to be on the
            // safe side
            removal_time: segment
                .date_time
                .checked_add_signed(Duration::try_seconds(25).unwrap())
                .unwrap(),
            path: segment.path.clone(),
        });
    }

    while let Some(segment) = state.trimmed_segments.front() {
        if segment.removal_time < state.segments.front().unwrap().date_time {
            let segment = state.trimmed_segments.pop_front().unwrap();

            let mut path = state.path.clone();
            path.push(segment.path);
            println!("Removing {}", path.display());
            std::fs::remove_file(path).expect("Failed to remove old segment");
        } else {
            break;
        }
    }
}

fn update_manifest(state: &mut StreamState) {
    // Now write the manifest
    let mut path = state.path.clone();
    path.push("manifest.m3u8");

    println!("writing manifest to {}", path.display());

    trim_segments(state);

    let playlist = MediaPlaylist {
        version: Some(7),
        target_duration: 3,
        media_sequence: state.media_sequence,
        segments: state
            .segments
            .iter()
            .enumerate()
            .map(|(idx, segment)| MediaSegment {
                uri: segment.path.to_string(),
                duration: (segment.duration.nseconds() as f64
                    / gst::ClockTime::SECOND.nseconds() as f64) as f32,
                map: Some(m3u8_rs::Map {
                    uri: "init.cmfi".into(),
                    ..Default::default()
                }),
                program_date_time: if idx == 0 {
                    Some(segment.date_time.into())
                } else {
                    None
                },
                ..Default::default()
            })
            .collect(),
        end_list: false,
        playlist_type: None,
        i_frames_only: false,
        start: None,
        independent_segments: true,
        ..Default::default()
    };

    let mut file = std::fs::File::create(path).unwrap();
    playlist
        .write_to(&mut file)
        .expect("Failed to write media playlist");
}

fn setup_appsink(appsink: &gst_app::AppSink, name: &str, path: &Path, is_video: bool) {
    let mut path: PathBuf = path.into();
    path.push(name);

    let state = Arc::new(Mutex::new(StreamState {
        segments: VecDeque::new(),
        trimmed_segments: VecDeque::new(),
        path,
        start_date_time: None,
        start_time: gst::ClockTime::NONE,
        media_sequence: 0,
        segment_index: 0,
    }));

    appsink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |sink| {
                let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                let mut state = state.lock().unwrap();

                // The muxer only outputs non-empty buffer lists
                let mut buffer_list = sample.buffer_list_owned().expect("no buffer list");
                assert!(!buffer_list.is_empty());

                let mut first = buffer_list.get(0).unwrap();

                // Each list contains a full segment, i.e. does not start with a DELTA_UNIT
                assert!(!first.flags().contains(gst::BufferFlags::DELTA_UNIT));

                // If the buffer has the DISCONT and HEADER flag set then it contains the media
                // header, i.e. the `ftyp`, `moov` and other media boxes.
                //
                // This might be the initial header or the updated header at the end of the stream.
                if first
                    .flags()
                    .contains(gst::BufferFlags::DISCONT | gst::BufferFlags::HEADER)
                {
                    let mut path = state.path.clone();
                    std::fs::create_dir_all(&path).expect("failed to create directory");
                    path.push("init.cmfi");

                    println!("writing header to {}", path.display());
                    let map = first.map_readable().unwrap();
                    std::fs::write(path, &map).expect("failed to write header");
                    drop(map);

                    // Remove the header from the buffer list
                    buffer_list.make_mut().remove(0..1);

                    // If the list is now empty then it only contained the media header and nothing
                    // else.
                    if buffer_list.is_empty() {
                        return Ok(gst::FlowSuccess::Ok);
                    }

                    // Otherwise get the next buffer and continue working with that.
                    first = buffer_list.get(0).unwrap();
                }

                // If the buffer only has the HEADER flag set then this is a segment header that is
                // followed by one or more actual media buffers.
                assert!(first.flags().contains(gst::BufferFlags::HEADER));

                let mut path = state.path.clone();
                let basename = format!(
                    "segment_{}.{}",
                    state.segment_index,
                    if is_video { "cmfv" } else { "cmfa" }
                );
                state.segment_index += 1;
                path.push(&basename);

                let segment = sample
                    .segment()
                    .expect("no segment")
                    .downcast_ref::<gst::ClockTime>()
                    .expect("no time segment");
                let pts = segment
                    .to_running_time(first.pts().unwrap())
                    .expect("can't get running time");

                if state.start_time.is_none() {
                    state.start_time = Some(pts);
                }

                if state.start_date_time.is_none() {
                    let now_utc = Utc::now();
                    let now_gst = sink.clock().unwrap().time();
                    let pts_clock_time = pts + sink.base_time().unwrap();

                    let diff = now_gst.checked_sub(pts_clock_time).unwrap();
                    let pts_utc = now_utc
                        .checked_sub_signed(Duration::nanoseconds(diff.nseconds() as i64))
                        .unwrap();

                    state.start_date_time = Some(pts_utc);
                }

                let duration = first.duration().unwrap();

                let mut file = std::fs::File::create(&path).expect("failed to open fragment");
                for buffer in &*buffer_list {
                    use std::io::prelude::*;

                    let map = buffer.map_readable().unwrap();
                    file.write_all(&map).expect("failed to write fragment");
                }

                let date_time = state
                    .start_date_time
                    .unwrap()
                    .checked_add_signed(Duration::nanoseconds(
                        pts.opt_checked_sub(state.start_time)
                            .unwrap()
                            .unwrap()
                            .nseconds() as i64,
                    ))
                    .unwrap();

                println!(
                    "wrote segment with date time {} to {}",
                    date_time,
                    path.display()
                );

                state.segments.push_back(Segment {
                    duration,
                    path: basename.to_string(),
                    date_time,
                });

                update_manifest(&mut state);

                Ok(gst::FlowSuccess::Ok)
            })
            .eos(move |_sink| {
                unreachable!();
            })
            .build(),
    );
}

fn probe_encoder(state: Arc<Mutex<State>>, enc: gst::Element) {
    enc.static_pad("src").unwrap().add_probe(
        gst::PadProbeType::EVENT_DOWNSTREAM,
        move |_pad, info| {
            let Some(ev) = info.event() else {
                return gst::PadProbeReturn::Ok;
            };
            let gst::EventView::Caps(ev) = ev.view() else {
                return gst::PadProbeReturn::Ok;
            };

            let mime = gst_pbutils::codec_utils_caps_get_mime_codec(ev.caps());

            let mut state = state.lock().unwrap();
            state.all_mimes.push(mime.unwrap().into());
            state.maybe_write_manifest();

            gst::PadProbeReturn::Remove
        },
    );
}

impl VideoStream {
    fn setup(
        &self,
        state: Arc<Mutex<State>>,
        pipeline: &gst::Pipeline,
        path: &Path,
    ) -> Result<(), Error> {
        let src = gst::ElementFactory::make("videotestsrc")
            .property("is-live", true)
            .build()?;

        let raw_capsfilter = gst::ElementFactory::make("capsfilter")
            .property(
                "caps",
                gst_video::VideoCapsBuilder::new()
                    .format(gst_video::VideoFormat::I420)
                    .width(self.width as i32)
                    .height(self.height as i32)
                    .framerate(30.into())
                    .build(),
            )
            .build()?;
        let timeoverlay = gst::ElementFactory::make("timeoverlay").build()?;
        let enc = gst::ElementFactory::make("x264enc")
            .property("bframes", 0u32)
            .property("bitrate", self.bitrate as u32 / 1000u32)
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
        let mux = gst::ElementFactory::make("cmafmux")
            .property("fragment-duration", 3000.mseconds())
            .build()?;
        let appsink = gst_app::AppSink::builder().buffer_list(true).build();

        pipeline.add_many([
            &src,
            &raw_capsfilter,
            &timeoverlay,
            &enc,
            &h264_capsfilter,
            &mux,
            appsink.upcast_ref(),
        ])?;

        gst::Element::link_many([
            &src,
            &raw_capsfilter,
            &timeoverlay,
            &enc,
            &h264_capsfilter,
            &mux,
            appsink.upcast_ref(),
        ])?;

        probe_encoder(state, enc);

        setup_appsink(&appsink, &self.name, path, true);

        Ok(())
    }
}

impl AudioStream {
    fn setup(
        &self,
        state: Arc<Mutex<State>>,
        pipeline: &gst::Pipeline,
        path: &Path,
    ) -> Result<(), Error> {
        let src = gst::ElementFactory::make("audiotestsrc")
            .property("is-live", true)
            .property_from_str("wave", &self.wave)
            .build()?;
        let enc = gst::ElementFactory::make("avenc_aac").build()?;
        let mux = gst::ElementFactory::make("cmafmux")
            .property_from_str("header-update-mode", "update")
            .property("write-mehd", true)
            .property("fragment-duration", 3000.mseconds())
            .build()?;
        let appsink = gst_app::AppSink::builder().buffer_list(true).build();

        pipeline.add_many([&src, &enc, &mux, appsink.upcast_ref()])?;

        gst::Element::link_many([&src, &enc, &mux, appsink.upcast_ref()])?;

        probe_encoder(state, enc);

        setup_appsink(&appsink, &self.name, path, false);

        Ok(())
    }
}

fn main() -> Result<(), Error> {
    gst::init()?;

    gstisobmff::plugin_register_static()?;

    let path = PathBuf::from("hls_live_stream");

    let pipeline = gst::Pipeline::default();

    std::fs::create_dir_all(&path).expect("failed to create directory");

    let mut manifest_path = path.clone();
    manifest_path.push("manifest.m3u8");

    let state = Arc::new(Mutex::new(State {
        video_streams: vec![VideoStream {
            name: "video_0".to_string(),
            bitrate: 2_048_000,
            width: 1280,
            height: 720,
        }],
        audio_streams: vec![
            AudioStream {
                name: "audio_0".to_string(),
                lang: "eng".to_string(),
                default: true,
                wave: "sine".to_string(),
            },
            AudioStream {
                name: "audio_1".to_string(),
                lang: "fre".to_string(),
                default: false,
                wave: "white-noise".to_string(),
            },
        ],
        all_mimes: vec![],
        path: manifest_path.clone(),
        wrote_manifest: false,
    }));

    {
        let state_lock = state.lock().unwrap();

        for stream in &state_lock.video_streams {
            stream.setup(state.clone(), &pipeline, &path)?;
        }

        for stream in &state_lock.audio_streams {
            stream.setup(state.clone(), &pipeline, &path)?;
        }
    }

    pipeline.set_state(gst::State::Playing)?;

    let bus = pipeline
        .bus()
        .expect("Pipeline without bus. Shouldn't happen!");

    for msg in bus.iter_timed(gst::ClockTime::NONE) {
        use gst::MessageView;

        match msg.view() {
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
