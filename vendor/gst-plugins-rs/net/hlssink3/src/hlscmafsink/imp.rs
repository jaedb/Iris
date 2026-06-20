// Copyright (C) 2023 Seungha Yang <seungha@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use crate::hlsbasesink::HlsBaseSinkImpl;
use crate::hlssink3::HlsSink3PlaylistType;
use crate::playlist::Playlist;
use crate::HlsBaseSink;
use gio::prelude::*;
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use m3u8_rs::{MediaPlaylist, MediaPlaylistType, MediaSegment};
use std::io::Write;
use std::sync::LazyLock;
use std::sync::Mutex;

const DEFAULT_INIT_LOCATION: &str = "init%05d.mp4";
const DEFAULT_CMAF_LOCATION: &str = "segment%05d.m4s";
const DEFAULT_TARGET_DURATION: u32 = 15;
const DEFAULT_PLAYLIST_TYPE: HlsSink3PlaylistType = HlsSink3PlaylistType::Unspecified;
const DEFAULT_SYNC: bool = true;
const DEFAULT_LATENCY: gst::ClockTime =
    gst::ClockTime::from_mseconds((DEFAULT_TARGET_DURATION * 500) as u64);
const SIGNAL_GET_INIT_STREAM: &str = "get-init-stream";
const SIGNAL_NEW_PLAYLIST: &str = "new-playlist";

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "hlscmafsink",
        gst::DebugColorFlags::empty(),
        Some("HLS CMAF sink"),
    )
});

macro_rules! base_imp {
    ($i:expr) => {
        $i.obj().upcast_ref::<HlsBaseSink>().imp()
    };
}

struct KeyframeInfo {
    duration: gst::ClockTime,
    length: u64,
    offset: u64,
}

struct HlsCmafSinkSettings {
    init_location: String,
    location: String,
    target_duration: u32,
    playlist_type: Option<MediaPlaylistType>,
    sync: bool,
    latency: gst::ClockTime,
    playlist_root_init: Option<String>,
    iframe_playlist_location: Option<String>,

    cmafmux: gst::Element,
    appsink: gst_app::AppSink,
}

impl Default for HlsCmafSinkSettings {
    fn default() -> Self {
        let cmafmux = gst::ElementFactory::make("cmafmux")
            .name("muxer")
            .property(
                "fragment-duration",
                gst::ClockTime::from_seconds(DEFAULT_TARGET_DURATION as u64),
            )
            .property("latency", DEFAULT_LATENCY)
            .build()
            .expect("Could not make element cmafmux");
        let appsink = gst_app::AppSink::builder()
            .buffer_list(true)
            .sync(DEFAULT_SYNC)
            .name("sink")
            .build();

        Self {
            init_location: String::from(DEFAULT_INIT_LOCATION),
            location: String::from(DEFAULT_CMAF_LOCATION),
            target_duration: DEFAULT_TARGET_DURATION,
            playlist_type: None,
            sync: DEFAULT_SYNC,
            latency: DEFAULT_LATENCY,
            playlist_root_init: None,
            iframe_playlist_location: None,
            cmafmux,
            appsink,
        }
    }
}

#[derive(Default)]
struct HlsCmafSinkState {
    init_idx: u32,
    segment_idx: u32,
    init_segment: Option<m3u8_rs::Map>,
    iframe_init_segment: Option<m3u8_rs::Map>,
    new_header: bool,
    offset: u64,
    keyframe_offset: u64,
    fragment_keyframes: Vec<KeyframeInfo>,
    fragment_buffers: Vec<Vec<u8>>,
    fragment_duration: Option<gst::ClockTime>,
}

#[derive(Default)]
pub struct HlsCmafSink {
    settings: Mutex<HlsCmafSinkSettings>,
    state: Mutex<HlsCmafSinkState>,
}

#[glib::object_subclass]
impl ObjectSubclass for HlsCmafSink {
    const NAME: &'static str = "GstHlsCmafSink";
    type Type = super::HlsCmafSink;
    type ParentType = HlsBaseSink;
}

impl ObjectImpl for HlsCmafSink {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecString::builder("init-location")
                    .nick("Init Location")
                    .blurb("Location of the init fragment file to write")
                    .default_value(Some(DEFAULT_INIT_LOCATION))
                    .build(),
                glib::ParamSpecString::builder("location")
                    .nick("Location")
                    .blurb("Location of the fragment file to write")
                    .default_value(Some(DEFAULT_CMAF_LOCATION))
                    .build(),
                glib::ParamSpecUInt::builder("target-duration")
                    .nick("Target duration")
                    .blurb("The target duration in seconds of a segment/file. (0 - disabled, useful for management of segment duration by the streaming server)")
                    .default_value(DEFAULT_TARGET_DURATION)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecEnum::builder_with_default("playlist-type", DEFAULT_PLAYLIST_TYPE)
                    .nick("Playlist Type")
                    .blurb("The type of the playlist to use. When VOD type is set, the playlist will be live until the pipeline ends execution.")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("sync")
                    .nick("Sync")
                    .blurb("Sync on the clock")
                    .default_value(DEFAULT_SYNC)
                    .build(),
                glib::ParamSpecUInt64::builder("latency")
                    .nick("Latency")
                    .blurb(
                        "Additional latency to allow upstream to take longer to \
                         produce buffers for the current position (in nanoseconds)",
                    )
                    .maximum(i64::MAX as u64)
                    .default_value(DEFAULT_LATENCY.nseconds())
                    .build(),
                glib::ParamSpecString::builder("playlist-root-init")
                    .nick("Playlist Root Init")
                    .blurb("Base path for the init fragment in the playlist file.")
                    .build(),
                glib::ParamSpecString::builder("iframe-playlist-location")
                    .nick("I-frame playlist Location")
                    .blurb("Location of the I-frame playlist file to write")
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().unwrap();
        match pspec.name() {
            "init-location" => {
                settings.init_location = value
                    .get::<Option<String>>()
                    .expect("type checked upstream")
                    .unwrap_or_else(|| DEFAULT_INIT_LOCATION.into());
            }
            "location" => {
                settings.location = value
                    .get::<Option<String>>()
                    .expect("type checked upstream")
                    .unwrap_or_else(|| DEFAULT_CMAF_LOCATION.into());
            }
            "target-duration" => {
                settings.target_duration = value.get().expect("type checked upstream");
                settings.cmafmux.set_property(
                    "fragment-duration",
                    gst::ClockTime::from_seconds(settings.target_duration as u64),
                );
            }
            "playlist-type" => {
                settings.playlist_type = value
                    .get::<HlsSink3PlaylistType>()
                    .expect("type checked upstream")
                    .into();
            }
            "sync" => {
                settings.sync = value.get().expect("type checked upstream");
                settings.appsink.set_property("sync", settings.sync);
            }
            "latency" => {
                settings.latency = value.get().expect("type checked upstream");
                settings.cmafmux.set_property("latency", settings.latency);
            }
            "playlist-root-init" => {
                settings.playlist_root_init = value
                    .get::<Option<String>>()
                    .expect("type checked upstream");
            }
            "iframe-playlist-location" => {
                settings.iframe_playlist_location = value
                    .get::<Option<String>>()
                    .expect("type checked upstream");
                if settings.iframe_playlist_location.is_some() {
                    settings.cmafmux.set_property("enable-keyframe-meta", true);
                    settings
                        .cmafmux
                        .set_property_from_str("chunk-mode", "keyframe");
                }
            }
            _ => unimplemented!(),
        };
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().unwrap();
        match pspec.name() {
            "init-location" => settings.init_location.to_value(),
            "location" => settings.location.to_value(),
            "target-duration" => settings.target_duration.to_value(),
            "playlist-type" => {
                let playlist_type: HlsSink3PlaylistType = settings.playlist_type.as_ref().into();
                playlist_type.to_value()
            }
            "sync" => settings.sync.to_value(),
            "latency" => settings.latency.to_value(),
            "playlist-root-init" => settings.playlist_root_init.to_value(),
            "iframe-playlist-location" => settings.iframe_playlist_location.to_value(),
            _ => unimplemented!(),
        }
    }

    fn signals() -> &'static [glib::subclass::Signal] {
        static SIGNALS: LazyLock<Vec<glib::subclass::Signal>> = LazyLock::new(|| {
            vec![
                glib::subclass::Signal::builder(SIGNAL_GET_INIT_STREAM)
                    .param_types([String::static_type()])
                    .return_type::<Option<gio::OutputStream>>()
                    .class_handler(|args| {
                        let elem = args[0].get::<HlsBaseSink>().expect("signal arg");
                        let init_location = args[1].get::<String>().expect("signal arg");
                        let imp = elem.imp();

                        Some(imp.new_file_stream(&init_location).ok().to_value())
                    })
                    .accumulator(|_hint, _acc, value| {
                        // First signal handler wins
                        std::ops::ControlFlow::Break(value.clone())
                    })
                    .build(),
                glib::subclass::Signal::builder(SIGNAL_NEW_PLAYLIST)
                    .action()
                    .class_handler(|args| {
                        // Forces hlscmafsink to finish the current playlist and start a new one.
                        // Meant to be used after changing output location at runtime, which would
                        // otherwise require changing playback state to READY to make sure that the
                        // old playlist is closed correctly and new init segment is written.
                        let elem = args[0].get::<super::HlsCmafSink>().expect("signal arg");
                        let imp = elem.imp();

                        gst::debug!(
                            CAT,
                            imp = imp,
                            "Closing current playlist and starting a new one"
                        );
                        base_imp!(imp).close_playlist();

                        let (
                            target_duration,
                            playlist_type,
                            segment_template,
                            cmafmux,
                            iframe_playlist_loc,
                        ) = {
                            let settings = imp.settings.lock().unwrap();
                            (
                                settings.target_duration,
                                settings.playlist_type.clone(),
                                settings.location.clone(),
                                settings.cmafmux.clone(),
                                settings.iframe_playlist_location.clone(),
                            )
                        };

                        let (playlist, iframe_playlist) = imp.start(
                            target_duration,
                            playlist_type,
                            iframe_playlist_loc.is_some(),
                        );
                        base_imp!(imp).open_playlist(playlist, segment_template.clone());

                        if let (Some(playlist), Some(playlist_loc)) =
                            (iframe_playlist, iframe_playlist_loc)
                        {
                            base_imp!(imp).open_iframe_playlist(
                                playlist,
                                segment_template,
                                playlist_loc,
                            );
                        }

                        // This forces cmafmux to send the init headers again.
                        cmafmux.emit_by_name::<()>("send-headers", &[]);

                        None
                    })
                    .build(),
            ]
        });

        SIGNALS.as_ref()
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        let settings = self.settings.lock().unwrap();

        obj.add_many([&settings.cmafmux, settings.appsink.upcast_ref()])
            .unwrap();
        settings.cmafmux.link(&settings.appsink).unwrap();

        let sinkpad = settings.cmafmux.static_pad("sink").unwrap();
        let gpad = gst::GhostPad::builder_with_target(&sinkpad)
            .unwrap()
            .event_function(move |pad, parent, event| {
                HlsCmafSink::catch_panic_pad_function(
                    parent,
                    || false,
                    |hls| hls.sink_event(pad, parent, event),
                )
            })
            .build();

        obj.add_pad(&gpad).unwrap();

        let self_weak = self.downgrade();
        settings.appsink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |sink| {
                    let Some(imp) = self_weak.upgrade() else {
                        return Err(gst::FlowError::Eos);
                    };

                    let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                    imp.on_new_sample(sample)
                })
                .build(),
        );
    }
}

impl GstObjectImpl for HlsCmafSink {}

impl ElementImpl for HlsCmafSink {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "HTTP Live Streaming CMAF Sink",
                "Sink/Muxer",
                "HTTP Live Streaming CMAF Sink",
                "Seungha Yang <seungha@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &[
                    gst::Structure::builder("video/x-h264")
                        .field("stream-format", gst::List::new(["avc", "avc3"]))
                        .field("alignment", "au")
                        .field("width", gst::IntRange::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("video/x-h265")
                        .field("stream-format", gst::List::new(["hvc1", "hev1"]))
                        .field("alignment", "au")
                        .field("width", gst::IntRange::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("audio/mpeg")
                        .field("mpegversion", 4i32)
                        .field("stream-format", "raw")
                        .field("channels", gst::IntRange::new(1, u16::MAX as i32))
                        .field("rate", gst::IntRange::new(1, i32::MAX))
                        .build(),
                ]
                .into_iter()
                .collect::<gst::Caps>(),
            )
            .unwrap();

            vec![pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        if transition == gst::StateChange::ReadyToPaused {
            let (target_duration, playlist_type, segment_template, iframe_playlist_loc) = {
                let settings = self.settings.lock().unwrap();
                (
                    settings.target_duration,
                    settings.playlist_type.clone(),
                    settings.location.clone(),
                    settings.iframe_playlist_location.clone(),
                )
            };

            let (playlist, iframe_playlist) = self.start(
                target_duration,
                playlist_type,
                iframe_playlist_loc.is_some(),
            );
            base_imp!(self).open_playlist(playlist, segment_template.clone());

            if let (Some(playlist), Some(playlist_loc)) = (iframe_playlist, iframe_playlist_loc) {
                base_imp!(self).open_iframe_playlist(playlist, segment_template, playlist_loc);
            }
        }

        self.parent_change_state(transition)
    }
}

impl BinImpl for HlsCmafSink {}

impl HlsBaseSinkImpl for HlsCmafSink {}

impl HlsCmafSink {
    fn start(
        &self,
        target_duration: u32,
        playlist_type: Option<MediaPlaylistType>,
        iframe_playlist: bool,
    ) -> (Playlist, Option<Playlist>) {
        gst::info!(CAT, imp = self, "Starting");

        let mut state = self.state.lock().unwrap();
        *state = HlsCmafSinkState::default();

        let (turn_vod, playlist_type) = if playlist_type == Some(MediaPlaylistType::Vod) {
            (true, Some(MediaPlaylistType::Event))
        } else {
            (false, playlist_type)
        };

        let playlist = MediaPlaylist {
            version: Some(7),
            target_duration: target_duration as u64,
            playlist_type: playlist_type.clone(),
            independent_segments: true,
            ..Default::default()
        };

        let iframe_playlist: Option<Playlist> = if iframe_playlist {
            let pl = MediaPlaylist {
                version: Some(7),
                target_duration: target_duration as u64,
                playlist_type,
                independent_segments: false,
                i_frames_only: true,
                ..Default::default()
            };
            Some(Playlist::new(pl, turn_vod, true))
        } else {
            None
        };

        (Playlist::new(playlist, turn_vod, true), iframe_playlist)
    }

    fn on_init_segment(
        &self,
        init_segment_size: u64,
    ) -> Result<gio::OutputStreamWrite<gio::OutputStream>, String> {
        let settings = self.settings.lock().unwrap();

        let (stream, location, byte_range) = if !base_imp!(self).is_single_media_file() {
            let state = self.state.lock().unwrap();

            match sprintf::sprintf!(&settings.init_location, state.init_idx) {
                Ok(location) => {
                    let stream = self
                        .obj()
                        .emit_by_name::<Option<gio::OutputStream>>(
                            SIGNAL_GET_INIT_STREAM,
                            &[&location],
                        )
                        .ok_or_else(|| String::from("Error while getting init stream"))?
                        .into_write();

                    (stream, location, None)
                }
                Err(err) => {
                    gst::error!(CAT, imp = self, "Couldn't build file name, err: {:?}", err,);
                    return Err(String::from("Invalid init segment file pattern"));
                }
            }
        } else {
            let mut state = self.state.lock().unwrap();
            let (stream, location) = self.on_new_fragment(&mut state).map_err(|err| {
                gst::error!(
                    CAT,
                    imp = self,
                    "Couldn't get fragment stream for init segment, {err}",
                );
                String::from("Couldn't get fragment stream for init segment")
            })?;

            (
                stream,
                location,
                Some(m3u8_rs::ByteRange {
                    length: init_segment_size,
                    offset: Some(0),
                }),
            )
        };

        let uri =
            base_imp!(self).get_segment_uri(&location, settings.playlist_root_init.as_deref());

        let mut state = self.state.lock().unwrap();
        state.init_segment = Some(m3u8_rs::Map {
            uri,
            byte_range,
            ..Default::default()
        });
        state.new_header = true;
        state.init_idx += 1;
        state.offset = init_segment_size;
        state.keyframe_offset = 0;
        state.iframe_init_segment = state.init_segment.clone();

        Ok(stream)
    }

    fn on_new_fragment(
        &self,
        state: &mut HlsCmafSinkState,
    ) -> Result<(gio::OutputStreamWrite<gio::OutputStream>, String), String> {
        let (stream, location) = base_imp!(self)
            .get_fragment_stream(state.segment_idx)
            .ok_or_else(|| String::from("Error while getting fragment stream"))?;

        state.segment_idx += 1;

        Ok((stream.into_write(), location))
    }

    fn add_iframe_segment(
        &self,
        state: &mut HlsCmafSinkState,
        duration: gst::ClockTime,
        location: &str,
        offset: u64,
        length: u64,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let uri = base_imp!(self).get_segment_uri(location, None);
        let map = state.iframe_init_segment.take();

        base_imp!(self).add_iframe_segment(
            location,
            MediaSegment {
                uri,
                map,
                duration: duration.mseconds() as f32 / 1_000f32,
                byte_range: Some(m3u8_rs::ByteRange {
                    length,
                    offset: Some(offset),
                }),
                ..Default::default()
            },
        )
    }

    fn add_segment(
        &self,
        state: &mut HlsCmafSinkState,
        duration: gst::ClockTime,
        running_time: Option<gst::ClockTime>,
        location: String,
        byte_range: Option<m3u8_rs::ByteRange>,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let uri = base_imp!(self).get_segment_uri(&location, None);

        let map = if state.new_header {
            state.new_header = false;
            state.init_segment.clone()
        } else {
            None
        };

        base_imp!(self).add_segment(
            &location,
            running_time,
            duration,
            None,
            MediaSegment {
                uri,
                duration: duration.mseconds() as f32 / 1_000f32,
                map,
                byte_range,
                ..Default::default()
            },
        )
    }

    fn on_new_sample(&self, sample: gst::Sample) -> Result<gst::FlowSuccess, gst::FlowError> {
        let settings = self.settings.lock().unwrap();
        let iframe_playlist = settings.iframe_playlist_location.is_some();
        drop(settings);

        let mut buffer_list = sample.buffer_list_owned().unwrap();
        let mut first = buffer_list.get(0).unwrap();

        if first
            .flags()
            .contains(gst::BufferFlags::DISCONT | gst::BufferFlags::HEADER)
        {
            let mut stream = self.on_init_segment(first.size() as u64).map_err(|err| {
                gst::error!(
                    CAT,
                    imp = self,
                    "Couldn't get output stream for init segment, {err}",
                );
                gst::FlowError::Error
            })?;

            let map = first.map_readable().unwrap();
            stream.write(&map).map_err(|_| {
                gst::error!(
                    CAT,
                    imp = self,
                    "Couldn't write init segment to output stream",
                );
                gst::FlowError::Error
            })?;

            stream.flush().map_err(|_| {
                gst::error!(CAT, imp = self, "Couldn't flush output stream",);
                gst::FlowError::Error
            })?;

            drop(map);

            buffer_list.make_mut().remove(0..1);
            if buffer_list.is_empty() {
                return Ok(gst::FlowSuccess::Ok);
            }

            first = buffer_list.get(0).unwrap();
        }

        let segment = sample
            .segment()
            .unwrap()
            .downcast_ref::<gst::ClockTime>()
            .unwrap();
        let running_time = segment.to_running_time(first.pts().unwrap());
        let duration = first.duration().unwrap();

        let mut state = self.state.lock().unwrap();

        if !iframe_playlist {
            self.write_segment(&mut state, &buffer_list, running_time, Some(duration))
        } else {
            let is_header = first.flags().contains(gst::BufferFlags::HEADER);
            let is_delta_unit = first.flags().contains(gst::BufferFlags::DELTA_UNIT);

            // Fragment header: is_header = true, is_delta_unit = false
            // Chunk header: is_header = true, is_delta_unit = true
            let segment_start = is_header && !is_delta_unit;

            let kf_meta = gst::meta::CustomMeta::from_buffer(first, "FMP4KeyframeMeta").unwrap();
            // We cannot rely on `hlscmafsink` receiving the EOS, as we will
            // get chunks from `cmafmux` upstream even after receiving EOS.
            let eos = kf_meta.structure().get::<bool>("eos").unwrap();

            if eos {
                // Write the last segment
                if !state.fragment_buffers.is_empty() {
                    self.write_segment(&mut state, &buffer_list, running_time, None)?;
                }

                // Write the current segment
                self.accumulate_fragments(&mut state, &buffer_list, &kf_meta, duration);

                self.write_segment(&mut state, &buffer_list, running_time, None)
            } else {
                // Write the last segment
                if !state.fragment_buffers.is_empty() && segment_start {
                    self.write_segment(&mut state, &buffer_list, running_time, None)?;
                }

                // Accumulate chunk/fragment
                self.accumulate_fragments(&mut state, &buffer_list, &kf_meta, duration);

                Ok(gst::FlowSuccess::Ok)
            }
        }
    }

    fn accumulate_fragments(
        &self,
        state: &mut HlsCmafSinkState,
        buffer_list: &gst::BufferList,
        keyframe_meta: &gst::MetaRef<gst::meta::CustomMeta>,
        duration: gst::ClockTime,
    ) {
        gst::trace!(
            CAT,
            imp = self,
            "Accumulating {} buffers",
            buffer_list.len()
        );

        for buffer in buffer_list.iter() {
            let map = buffer.map_readable().unwrap();
            state.fragment_buffers.push(map.as_slice().to_vec());
        }

        // When chunking, we need to accumulate the duration for each
        // chunk comprising a segment.
        state.fragment_duration = if let Some(d) = state.fragment_duration {
            Some(d + duration)
        } else {
            Some(duration)
        };

        let meta = keyframe_meta
            .structure()
            .get::<gst::Structure>("keyframe")
            .unwrap();

        let duration = meta.get::<gst::ClockTime>("keyframe-duration").unwrap();
        let length = meta.get::<u64>("keyframe-length").unwrap();
        let offset = meta.get::<u64>("keyframe-offset").unwrap() + state.keyframe_offset;

        // For multiple key frames within a segment, keyframe_offset tracks
        // the offset within a segment. Resets on segment write.
        state.keyframe_offset += buffer_list.calculate_size() as u64;

        state.fragment_keyframes.push(KeyframeInfo {
            duration,
            length,
            offset,
        })
    }

    fn write_segment(
        &self,
        state: &mut HlsCmafSinkState,
        buffer_list: &gst::BufferList,
        running_time: Option<gst::ClockTime>,
        duration: Option<gst::ClockTime>,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let is_single_media_file = base_imp!(self).is_single_media_file();
        let iframe_playlist = duration.is_none();

        let (mut stream, location) = self.on_new_fragment(state).map_err(|err| {
            gst::error!(
                CAT,
                imp = self,
                "Couldn't get output stream for segment, {err}",
            );
            gst::FlowError::Error
        })?;

        gst::trace!(
            CAT,
            imp = self,
            "Writing {} buffers for segment: {location} with running_time: {:?}",
            if iframe_playlist {
                state.fragment_buffers.len()
            } else {
                buffer_list.len()
            },
            running_time
        );

        if iframe_playlist {
            let keyframes = state.fragment_keyframes.drain(..).collect::<Vec<_>>();
            if !keyframes.is_empty() {
                for m in keyframes {
                    let offset = if is_single_media_file {
                        state.offset + m.offset
                    } else {
                        m.offset
                    };

                    self.add_iframe_segment(state, m.duration, &location, offset, m.length)?;
                }

                // Reset for the next segment
                state.keyframe_offset = 0;
            }

            for segment in &state.fragment_buffers {
                stream.write(segment).map_err(|_| {
                    gst::error!(CAT, imp = self, "Couldn't write segment to output stream",);
                    gst::FlowError::Error
                })?;
            }
        } else {
            for buffer in buffer_list.iter() {
                let map = buffer.map_readable().unwrap();

                stream.write(&map).map_err(|_| {
                    gst::error!(CAT, imp = self, "Couldn't write segment to output stream",);
                    gst::FlowError::Error
                })?;
            }
        };

        stream.flush().map_err(|_| {
            gst::error!(CAT, imp = self, "Couldn't flush output stream",);
            gst::FlowError::Error
        })?;

        let (duration, length) = if iframe_playlist {
            (
                state.fragment_duration.take().unwrap(),
                state.fragment_buffers.iter().map(|b| b.len() as u64).sum(),
            )
        } else {
            (duration.unwrap(), buffer_list.calculate_size() as u64)
        };

        state.fragment_buffers.clear();

        let byte_range = if !is_single_media_file {
            None
        } else {
            let offset = Some(state.offset);
            state.offset += length;

            Some(m3u8_rs::ByteRange { length, offset })
        };

        self.add_segment(state, duration, running_time, location, byte_range)
    }

    fn sink_event(
        &self,
        pad: &gst::GhostPad,
        parent: Option<&impl IsA<gst::Object>>,
        event: gst::Event,
    ) -> bool {
        if let gst::EventView::Caps(caps) = event.view() {
            let s = caps.caps().structure(0).unwrap();

            if s.name().starts_with("audio/") {
                let settings = self.settings.lock().unwrap();
                let iframe_playlist = settings.iframe_playlist_location.is_some();
                drop(settings);

                if iframe_playlist {
                    gst::element_error!(
                        self.obj(),
                        gst::StreamError::WrongType,
                        ("Invalid configuration"),
                        ["Audio not allowed with I-frame playlist enabled"]
                    );
                }
            }
        }

        gst::Pad::event_default(pad, parent, event)
    }
}
