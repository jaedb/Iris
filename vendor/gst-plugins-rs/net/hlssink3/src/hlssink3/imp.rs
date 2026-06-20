// Copyright (C) 2021 Rafael Caricio <rafael@caricio.com>
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
use chrono::{DateTime, Utc};
use gio::prelude::*;
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use m3u8_rs::{MediaPlaylist, MediaPlaylistType, MediaSegment};
use std::sync::LazyLock;
use std::sync::Mutex;

const DEFAULT_TS_LOCATION: &str = "segment%05d.ts";
const DEFAULT_TARGET_DURATION: u32 = 15;
const DEFAULT_PLAYLIST_TYPE: HlsSink3PlaylistType = HlsSink3PlaylistType::Unspecified;
const DEFAULT_I_FRAMES_ONLY_PLAYLIST: bool = false;
const DEFAULT_SEND_KEYFRAME_REQUESTS: bool = true;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new("hlssink3", gst::DebugColorFlags::empty(), Some("HLS sink"))
});

macro_rules! base_imp {
    ($i:expr) => {
        $i.obj().upcast_ref::<HlsBaseSink>().imp()
    };
}

/// Offset between NTP and UNIX epoch in seconds.
/// NTP = UNIX + NTP_UNIX_OFFSET.
const NTP_UNIX_OFFSET: u64 = 2_208_988_800;

/// Reference timestamp meta caps for NTP timestamps.
static NTP_CAPS: LazyLock<gst::Caps> =
    LazyLock::new(|| gst::Caps::builder("timestamp/x-ntp").build());

/// Reference timestamp meta caps for UNIX timestamps.
static UNIX_CAPS: LazyLock<gst::Caps> =
    LazyLock::new(|| gst::Caps::builder("timestamp/x-unix").build());

/// Returns the UTC time of the buffer in the UNIX epoch.
fn get_utc_time_from_buffer(buffer: &gst::BufferRef) -> Option<DateTime<Utc>> {
    buffer
        .iter_meta::<gst::ReferenceTimestampMeta>()
        .find_map(|meta| {
            if meta.reference().can_intersect(&UNIX_CAPS) {
                Some(meta.timestamp())
            } else if meta.reference().can_intersect(&NTP_CAPS) {
                meta.timestamp().checked_sub(NTP_UNIX_OFFSET.seconds())
            } else {
                None
            }
        })
        .and_then(|clock_time| {
            let time_nsec = clock_time.nseconds();
            let one_sec = gst::ClockTime::SECOND.nseconds();
            let timestamp_secs = (time_nsec / one_sec).try_into().ok()?;
            let timestamp_nanos = (time_nsec % one_sec).try_into().ok()?;
            DateTime::<Utc>::from_timestamp(timestamp_secs, timestamp_nanos)
        })
}

impl From<HlsSink3PlaylistType> for Option<MediaPlaylistType> {
    fn from(pl_type: HlsSink3PlaylistType) -> Self {
        use HlsSink3PlaylistType::*;
        match pl_type {
            Unspecified => None,
            Event => Some(MediaPlaylistType::Event),
            Vod => Some(MediaPlaylistType::Vod),
        }
    }
}

impl From<Option<&MediaPlaylistType>> for HlsSink3PlaylistType {
    fn from(inner_pl_type: Option<&MediaPlaylistType>) -> Self {
        use HlsSink3PlaylistType::*;
        match inner_pl_type {
            None | Some(MediaPlaylistType::Other(_)) => Unspecified,
            Some(MediaPlaylistType::Event) => Event,
            Some(MediaPlaylistType::Vod) => Vod,
        }
    }
}

struct HlsSink3Settings {
    location: String,
    target_duration: u32,
    playlist_type: Option<MediaPlaylistType>,
    i_frames_only: bool,
    send_keyframe_requests: bool,

    splitmuxsink: gst::Element,
    giostreamsink: gst::Element,
    video_sink: bool,
    audio_sink: bool,
}

impl Default for HlsSink3Settings {
    fn default() -> Self {
        let muxer = gst::ElementFactory::make("mpegtsmux")
            .name("mpeg-ts_mux")
            .build()
            .expect("Could not make element mpegtsmux");
        let giostreamsink = gst::ElementFactory::make("giostreamsink")
            .name("giostream_sink")
            .build()
            .expect("Could not make element giostreamsink");
        let splitmuxsink = gst::ElementFactory::make("splitmuxsink")
            .name("split_mux_sink")
            .property("muxer", &muxer)
            .property("reset-muxer", false)
            .property("send-keyframe-requests", DEFAULT_SEND_KEYFRAME_REQUESTS)
            .property(
                "max-size-time",
                gst::ClockTime::from_seconds(DEFAULT_TARGET_DURATION as u64),
            )
            .property("sink", &giostreamsink)
            .build()
            .expect("Could not make element splitmuxsink");

        // giostreamsink doesn't let go of its stream until the element is finalized, which might
        // be too late for the calling application. Let's try to force it to close while tearing
        // down the pipeline.
        if giostreamsink.has_property_with_type("close-on-stop", bool::static_type()) {
            giostreamsink.set_property("close-on-stop", true);
        } else {
            gst::warning!(
                CAT,
                "hlssink3 may sometimes fail to write out the final playlist update. This can be fixed by using giostreamsink from GStreamer 1.24 or later."
            )
        }

        Self {
            location: String::from(DEFAULT_TS_LOCATION),
            target_duration: DEFAULT_TARGET_DURATION,
            playlist_type: None,
            send_keyframe_requests: DEFAULT_SEND_KEYFRAME_REQUESTS,
            i_frames_only: DEFAULT_I_FRAMES_ONLY_PLAYLIST,

            splitmuxsink,
            giostreamsink,
            video_sink: false,
            audio_sink: false,
        }
    }
}

#[derive(Default)]
struct HlsSink3State {
    fragment_opened_at: Option<gst::ClockTime>,
    fragment_running_time: Option<gst::ClockTime>,
    current_segment_location: Option<String>,
    fragment_start_timestamp: Option<DateTime<Utc>>,
    offset: u64,
}

#[derive(Default)]
pub struct HlsSink3 {
    settings: Mutex<HlsSink3Settings>,
    state: Mutex<HlsSink3State>,
}

#[glib::object_subclass]
impl ObjectSubclass for HlsSink3 {
    const NAME: &'static str = "GstHlsSink3";
    type Type = super::HlsSink3;
    type ParentType = HlsBaseSink;
}

impl ObjectImpl for HlsSink3 {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecString::builder("location")
                    .nick("File Location")
                    .blurb("Location of the file to write")
                    .default_value(Some(DEFAULT_TS_LOCATION))
                    .build(),
                glib::ParamSpecUInt::builder("target-duration")
                    .nick("Target duration")
                    .blurb("The target duration in seconds of a segment/file. (0 - disabled, useful for management of segment duration by the streaming server)")
                    .default_value(DEFAULT_TARGET_DURATION)
                    .build(),
                glib::ParamSpecEnum::builder_with_default("playlist-type", DEFAULT_PLAYLIST_TYPE)
                    .nick("Playlist Type")
                    .blurb("The type of the playlist to use. When VOD type is set, the playlist will be live until the pipeline ends execution.")
                    .build(),
                glib::ParamSpecBoolean::builder("i-frames-only")
                    .nick("I-Frames only playlist")
                    .blurb("Each video segments is single iframe, So put EXT-X-I-FRAMES-ONLY tag in the playlist")
                    .default_value(DEFAULT_I_FRAMES_ONLY_PLAYLIST)
                    .build(),
                glib::ParamSpecBoolean::builder("send-keyframe-requests")
                    .nick("Send Keyframe Requests")
                    .blurb("Send keyframe requests to ensure correct fragmentation. If this is disabled then the input must have keyframes in regular intervals.")
                    .default_value(DEFAULT_SEND_KEYFRAME_REQUESTS)
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().unwrap();
        match pspec.name() {
            "location" => {
                settings.location = value
                    .get::<Option<String>>()
                    .expect("type checked upstream")
                    .unwrap_or_else(|| DEFAULT_TS_LOCATION.into());
                settings
                    .splitmuxsink
                    .set_property("location", &settings.location);
            }
            "target-duration" => {
                settings.target_duration = value.get().expect("type checked upstream");
                settings.splitmuxsink.set_property(
                    "max-size-time",
                    gst::ClockTime::from_seconds(settings.target_duration as u64),
                );
            }
            "playlist-type" => {
                settings.playlist_type = value
                    .get::<HlsSink3PlaylistType>()
                    .expect("type checked upstream")
                    .into();
            }
            "i-frames-only" => {
                settings.i_frames_only = value.get().expect("type checked upstream");
                if settings.i_frames_only && settings.audio_sink {
                    gst::element_error!(
                        self.obj(),
                        gst::StreamError::WrongType,
                        ("Invalid configuration"),
                        ["Audio not allowed for i-frames-only-stream"]
                    );
                }
            }
            "send-keyframe-requests" => {
                settings.send_keyframe_requests = value.get().expect("type checked upstream");
                settings
                    .splitmuxsink
                    .set_property("send-keyframe-requests", settings.send_keyframe_requests);
            }
            _ => unimplemented!(),
        };
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().unwrap();
        match pspec.name() {
            "location" => settings.location.to_value(),
            "target-duration" => settings.target_duration.to_value(),
            "playlist-type" => {
                let playlist_type: HlsSink3PlaylistType = settings.playlist_type.as_ref().into();
                playlist_type.to_value()
            }
            "i-frames-only" => settings.i_frames_only.to_value(),
            "send-keyframe-requests" => settings.send_keyframe_requests.to_value(),
            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        let settings = self.settings.lock().unwrap();

        obj.add(&settings.splitmuxsink).unwrap();
        settings
            .splitmuxsink
            .connect("format-location-full", false, {
                let imp_weak = self.downgrade();
                move |args| {
                    let Some(imp) = imp_weak.upgrade() else {
                        return Some(None::<String>.to_value());
                    };
                    let fragment_id = args[1].get::<u32>().unwrap();
                    gst::info!(CAT, imp = imp, "Got fragment-id: {}", fragment_id);

                    let sample = args[2].get::<gst::Sample>().unwrap();
                    let buffer = sample.buffer();

                    let buffer_timestamp = buffer.and_then(get_utc_time_from_buffer);

                    let running_time = if let Some(buffer) = buffer {
                        let segment = sample
                            .segment()
                            .expect("segment not available")
                            .downcast_ref::<gst::ClockTime>()
                            .expect("no time segment");
                        segment.to_running_time(buffer.pts().unwrap())
                    } else {
                        gst::warning!(
                            CAT,
                            imp = imp,
                            "buffer null for fragment-id: {}",
                            fragment_id
                        );
                        None
                    };

                    {
                        let mut state = imp.state.lock().unwrap();
                        if state.fragment_start_timestamp.is_none() {
                            state.fragment_start_timestamp = buffer_timestamp;
                        }
                    }

                    match imp.on_format_location(fragment_id, running_time) {
                        Ok(segment_location) => Some(segment_location.to_value()),
                        Err(err) => {
                            gst::error!(CAT, imp = imp, "on format-location handler: {}", err);
                            Some("unknown_segment".to_value())
                        }
                    }
                }
            });
    }
}

impl GstObjectImpl for HlsSink3 {}

impl ElementImpl for HlsSink3 {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "HTTP Live Streaming sink",
                "Sink/Muxer",
                "HTTP Live Streaming sink",
                "Alessandro Decina <alessandro.d@gmail.com>, \
                Sebastian Dröge <sebastian@centricular.com>, \
                Rafael Caricio <rafael@caricio.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::new_any();
            let video_pad_template = gst::PadTemplate::new(
                "video",
                gst::PadDirection::Sink,
                gst::PadPresence::Request,
                &caps,
            )
            .unwrap();

            let caps = gst::Caps::new_any();
            let audio_pad_template = gst::PadTemplate::new(
                "audio",
                gst::PadDirection::Sink,
                gst::PadPresence::Request,
                &caps,
            )
            .unwrap();

            vec![video_pad_template, audio_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        if transition == gst::StateChange::ReadyToPaused {
            let (target_duration, playlist_type, i_frames_only, segment_template) = {
                let settings = self.settings.lock().unwrap();
                (
                    settings.target_duration,
                    settings.playlist_type.clone(),
                    settings.i_frames_only,
                    settings.location.clone(),
                )
            };

            let playlist = self.start(target_duration, playlist_type, i_frames_only);
            base_imp!(self).open_playlist(playlist, segment_template);
        }

        self.parent_change_state(transition)
    }

    fn request_new_pad(
        &self,
        templ: &gst::PadTemplate,
        _name: Option<&str>,
        _caps: Option<&gst::Caps>,
    ) -> Option<gst::Pad> {
        let mut settings = self.settings.lock().unwrap();
        match templ.name_template() {
            "audio" => {
                if settings.audio_sink {
                    gst::debug!(
                        CAT,
                        imp = self,
                        "requested_new_pad: audio pad is already set"
                    );
                    return None;
                }
                if settings.i_frames_only {
                    gst::element_error!(
                        self.obj(),
                        gst::StreamError::WrongType,
                        ("Invalid configuration"),
                        ["Audio not allowed for i-frames-only-stream"]
                    );
                    return None;
                }

                let peer_pad = settings.splitmuxsink.request_pad_simple("audio_0").unwrap();
                let sink_pad = gst::GhostPad::from_template_with_target(templ, &peer_pad).unwrap();
                self.obj().add_pad(&sink_pad).unwrap();
                sink_pad.set_active(true).unwrap();
                settings.audio_sink = true;

                Some(sink_pad.upcast())
            }
            "video" => {
                if settings.video_sink {
                    gst::debug!(
                        CAT,
                        imp = self,
                        "requested_new_pad: video pad is already set"
                    );
                    return None;
                }
                let peer_pad = settings.splitmuxsink.request_pad_simple("video").unwrap();

                let sink_pad = gst::GhostPad::from_template_with_target(templ, &peer_pad).unwrap();
                self.obj().add_pad(&sink_pad).unwrap();
                sink_pad.set_active(true).unwrap();
                settings.video_sink = true;

                Some(sink_pad.upcast())
            }
            other_name => {
                gst::debug!(
                    CAT,
                    imp = self,
                    "requested_new_pad: name \"{}\" is not audio or video",
                    other_name
                );
                None
            }
        }
    }

    fn release_pad(&self, pad: &gst::Pad) {
        let mut settings = self.settings.lock().unwrap();

        if !settings.audio_sink && !settings.video_sink {
            return;
        }

        let ghost_pad = pad.downcast_ref::<gst::GhostPad>().unwrap();
        if let Some(peer) = ghost_pad.target() {
            settings.splitmuxsink.release_request_pad(&peer);
        }

        pad.set_active(false).unwrap();
        self.obj().remove_pad(pad).unwrap();

        if "audio" == ghost_pad.name() {
            settings.audio_sink = false;
        } else {
            settings.video_sink = false;
        }
    }
}

impl BinImpl for HlsSink3 {
    #[allow(clippy::single_match)]
    fn handle_message(&self, msg: gst::Message) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Element(msg) => {
                let event_is_from_splitmuxsink = {
                    let settings = self.settings.lock().unwrap();

                    msg.src() == Some(settings.splitmuxsink.upcast_ref())
                };
                if !event_is_from_splitmuxsink {
                    return;
                }

                let s = msg.structure().unwrap();
                match s.name().as_str() {
                    "splitmuxsink-fragment-opened" => {
                        if let Ok(new_fragment_opened_at) = s.get::<gst::ClockTime>("running-time")
                        {
                            let mut state = self.state.lock().unwrap();
                            state.fragment_opened_at = Some(new_fragment_opened_at);
                        }
                    }
                    "splitmuxsink-fragment-closed" => {
                        let s = msg.structure().unwrap();
                        if let Ok(fragment_closed_at) = s.get::<gst::ClockTime>("running-time") {
                            self.on_fragment_closed(s, fragment_closed_at);
                        }
                    }
                    _ => {}
                }
            }
            _ => self.parent_handle_message(msg),
        }
    }
}

impl HlsBaseSinkImpl for HlsSink3 {}

impl HlsSink3 {
    fn start(
        &self,
        target_duration: u32,
        playlist_type: Option<MediaPlaylistType>,
        i_frames_only: bool,
    ) -> Playlist {
        gst::info!(CAT, imp = self, "Starting");

        let mut state = self.state.lock().unwrap();
        *state = HlsSink3State::default();

        let (turn_vod, playlist_type) = if playlist_type == Some(MediaPlaylistType::Vod) {
            (true, Some(MediaPlaylistType::Event))
        } else {
            (false, playlist_type)
        };

        let is_single_media_file = base_imp!(self).is_single_media_file();
        if is_single_media_file {
            // `splitmuxsink` will stop the sink on every fragment,
            // don't do that for single media file case as we need
            // to keep the stream around for writing.
            let giostreamsink = self.settings.lock().unwrap().giostreamsink.clone();
            if giostreamsink.has_property_with_type("close-on-stop", bool::static_type()) {
                giostreamsink.set_property("close-on-stop", false);
            } else {
                gst::element_imp_error!(
                    self,
                    gst::ResourceError::Settings,
                    ("Invalid configuration"),
                    ["Single media file support with hlssink3 needs GStreamer 1.24 or later"]
                );
            }
        }

        let version = if i_frames_only || is_single_media_file {
            Some(4)
        } else {
            Some(3)
        };

        let playlist = MediaPlaylist {
            version,
            target_duration: target_duration as u64,
            playlist_type,
            i_frames_only,
            ..Default::default()
        };

        Playlist::new(playlist, turn_vod, false)
    }

    fn on_format_location(
        &self,
        fragment_id: u32,
        running_time: Option<gst::ClockTime>,
    ) -> Result<String, String> {
        gst::info!(
            CAT,
            imp = self,
            "Starting the formatting of the fragment-id: {}",
            fragment_id
        );

        let (fragment_stream, segment_file_location) = base_imp!(self)
            .get_fragment_stream(fragment_id)
            .ok_or_else(|| String::from("Error while getting fragment stream"))?;

        let mut state = self.state.lock().unwrap();
        state.current_segment_location = Some(segment_file_location.clone());
        state.fragment_running_time = running_time;

        let settings = self.settings.lock().unwrap();

        gst::info!(
            CAT,
            imp = self,
            "New segment location: {:?}",
            segment_file_location,
        );

        settings
            .giostreamsink
            .set_property("stream", &fragment_stream);

        Ok(segment_file_location)
    }

    fn on_fragment_closed(&self, s: &gst::StructureRef, closed_at: gst::ClockTime) {
        let obj = self.obj();
        let base_imp = obj.upcast_ref::<HlsBaseSink>().imp();

        let mut state = self.state.lock().unwrap();

        let location = match s
            .get::<u32>("fragment-id")
            .ok()
            // Try to get the location of the fragment we just closed, if splitmuxsink gave us the
            // fragment id (1.26+)
            .and_then(|fragment_id| base_imp.get_location(fragment_id))
            // Else fallback to the last fragment location we generated, which is technically wrong
            // (because this is the current fragment in progress, not the last fragment closed)
            .or_else(|| state.current_segment_location.take())
        {
            Some(location) => location,
            None => {
                gst::error!(CAT, imp = self, "Unknown segment location");
                return;
            }
        };

        let (duration, duration_msec) = {
            if let Ok(fragment_duration) = s.get::<gst::ClockTime>("fragment-duration") {
                (
                    fragment_duration,
                    fragment_duration.mseconds() as f32 / 1_000f32,
                )
            } else {
                let opened_at = match state.fragment_opened_at.take() {
                    Some(opened_at) => opened_at,
                    None => {
                        gst::error!(CAT, imp = self, "Unknown segment duration");
                        return;
                    }
                };

                let fragment_duration = closed_at - opened_at;

                (
                    fragment_duration,
                    fragment_duration.mseconds() as f32 / 1_000f32,
                )
            }
        };

        let running_time = state.fragment_running_time;
        let fragment_start_timestamp = state.fragment_start_timestamp.take();
        let byte_range = if base_imp!(self).is_single_media_file() {
            let offset = state.offset;
            let length = base_imp!(self).out_bytes() as u64 - offset;
            state.offset += length;
            Some(m3u8_rs::ByteRange {
                length,
                offset: Some(offset),
            })
        } else {
            None
        };

        drop(state);

        let uri = base_imp.get_segment_uri(&location, None);
        let _ = base_imp.add_segment(
            &location,
            running_time,
            duration,
            fragment_start_timestamp,
            MediaSegment {
                uri,
                duration: duration_msec,
                byte_range,
                ..Default::default()
            },
        );
    }
}
