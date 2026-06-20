// Copyright (C) 2021 Rafael Caricio <rafael@caricio.com>
// Copyright (C) 2023 Seungha Yang <seungha@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use crate::playlist::Playlist;
use chrono::{DateTime, Duration, Utc};
use gio::prelude::*;
use gio::subclass::prelude::OutputStreamImpl;
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use m3u8_rs::MediaSegment;
use std::fs;
use std::io::Write;
use std::path;
use std::sync::{LazyLock, Mutex};

const DEFAULT_PLAYLIST_LOCATION: &str = "playlist.m3u8";
const DEFAULT_MAX_NUM_SEGMENT_FILES: u32 = 10;
const DEFAULT_PLAYLIST_LENGTH: u32 = 5;
const DEFAULT_PROGRAM_DATE_TIME_TAG: bool = false;
const DEFAULT_CLOCK_TRACKING_FOR_PDT: bool = true;
const DEFAULT_ENDLIST: bool = true;
const DEFAULT_PROGRAM_DATE_TIME_REFERENCE: HlsProgramDateTimeReference =
    HlsProgramDateTimeReference::Pipeline;

const SIGNAL_GET_PLAYLIST_STREAM: &str = "get-playlist-stream";
const SIGNAL_GET_FRAGMENT_STREAM: &str = "get-fragment-stream";
const SIGNAL_DELETE_FRAGMENT: &str = "delete-fragment";

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "hlsbasesink",
        gst::DebugColorFlags::empty(),
        Some("HLS Base sink"),
    )
});

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstHlsProgramDateTimeReference")]
#[non_exhaustive]
pub enum HlsProgramDateTimeReference {
    #[enum_value(name = "Pipeline: Use the pipeline clock", nick = "pipeline")]
    Pipeline = 0,
    #[enum_value(name = "System: Use the system clock", nick = "system")]
    System = 1,
    #[enum_value(
        name = "BufferReferenceTimestamp: Use the buffer reference timestamp",
        nick = "buffer-reference-timestamp"
    )]
    BufferReferenceTimestamp = 2,
}

// We need to keep an OutputStream around for writing to the same file
// to support the single media file use case. OutputStream not being
// thread safe, use this wrapper to keep an OutputStream around in State.
#[derive(Default)]
pub struct HlsBaseSinkGioOutputStream {
    stream: Mutex<Option<gio::OutputStream>>,
    out_bytes: Mutex<usize>,
}

unsafe impl Send for HlsBaseSinkGioOutputStream {}
unsafe impl Sync for HlsBaseSinkGioOutputStream {}

#[glib::object_subclass]
impl ObjectSubclass for HlsBaseSinkGioOutputStream {
    const NAME: &'static str = "GstHlsBaseSinkGioOutputStream";
    type Type = super::HlsBaseSinkGioOutputStream;
    type ParentType = gio::OutputStream;
}

impl ObjectImpl for HlsBaseSinkGioOutputStream {}

impl OutputStreamImpl for HlsBaseSinkGioOutputStream {
    fn write(
        &self,
        buffer: &[u8],
        cancellable: Option<&gio::Cancellable>,
    ) -> Result<usize, glib::Error> {
        let stream = self.stream.lock().unwrap();
        if let Some(ref s) = *stream {
            s.write_all(buffer, cancellable)?;
            let mut out_bytes = self.out_bytes.lock().unwrap();
            *out_bytes += buffer.len();
            return Ok(buffer.len());
        }

        Ok(0)
    }

    fn close(&self, cancellable: Option<&gio::Cancellable>) -> Result<(), glib::Error> {
        let stream = self.stream.lock().unwrap().take();
        if let Some(ref s) = stream {
            s.close(cancellable)?;
        }

        Ok(())
    }

    fn flush(&self, cancellable: Option<&gio::Cancellable>) -> Result<(), glib::Error> {
        let stream = self.stream.lock().unwrap();
        if let Some(ref s) = *stream {
            s.flush(cancellable)?;
        }

        Ok(())
    }
}

impl super::HlsBaseSinkGioOutputStream {
    pub fn new(stream: gio::OutputStream) -> Self {
        let obj = glib::Object::new::<Self>();
        *obj.imp().stream.lock().unwrap() = Some(stream);
        obj
    }

    pub fn out_bytes(&self) -> usize {
        let out_bytes = self.imp().out_bytes.lock().unwrap();
        *out_bytes
    }
}

struct Settings {
    playlist_location: String,
    playlist_root: Option<String>,
    playlist_length: u32,
    max_num_segment_files: usize,
    enable_program_date_time: bool,
    program_date_time_reference: HlsProgramDateTimeReference,
    enable_endlist: bool,
    single_media_file: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            playlist_location: String::from(DEFAULT_PLAYLIST_LOCATION),
            playlist_root: None,
            playlist_length: DEFAULT_PLAYLIST_LENGTH,
            max_num_segment_files: DEFAULT_MAX_NUM_SEGMENT_FILES as usize,
            enable_program_date_time: DEFAULT_PROGRAM_DATE_TIME_TAG,
            program_date_time_reference: DEFAULT_PROGRAM_DATE_TIME_REFERENCE,
            enable_endlist: DEFAULT_ENDLIST,
            single_media_file: None,
        }
    }
}

pub struct PlaylistContext {
    pdt_base_utc: Option<DateTime<Utc>>,
    pdt_base_running_time: Option<gst::ClockTime>,
    playlist: Playlist,
    old_segment_locations: Vec<String>,
    segment_template: String,
    playlist_location: String,
    max_num_segment_files: usize,
    playlist_length: u32,
    single_media_file: bool,
}

#[derive(Default)]
pub struct State {
    context: Option<PlaylistContext>,
    stream: Option<super::HlsBaseSinkGioOutputStream>,
    iframe_context: Option<PlaylistContext>,
}

#[derive(Default)]
pub struct HlsBaseSink {
    settings: Mutex<Settings>,
    state: Mutex<State>,
}

#[glib::object_subclass]
impl ObjectSubclass for HlsBaseSink {
    const NAME: &'static str = "GstHlsBaseSink";
    type Type = super::HlsBaseSink;
    type ParentType = gst::Bin;
}

pub trait HlsBaseSinkImpl: BinImpl + ObjectSubclass<Type: IsA<super::HlsBaseSink>> {}

unsafe impl<T: HlsBaseSinkImpl> IsSubclassable<T> for super::HlsBaseSink {}

impl ObjectImpl for HlsBaseSink {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.set_suppressed_flags(gst::ElementFlags::SINK | gst::ElementFlags::SOURCE);
        obj.set_element_flags(gst::ElementFlags::SINK);
    }

    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecString::builder("playlist-location")
                    .nick("Playlist Location")
                    .blurb("Location of the playlist to write.")
                    .default_value(Some(DEFAULT_PLAYLIST_LOCATION))
                    .build(),
                glib::ParamSpecString::builder("playlist-root")
                    .nick("Playlist Root")
                    .blurb("Base path for the segments in the playlist file.")
                    .build(),
                glib::ParamSpecUInt::builder("max-files")
                    .nick("Max files")
                    .blurb("Maximum number of files to keep on disk. Once the maximum is reached, old files start to be deleted to make room for new ones.")
                    .build(),
                glib::ParamSpecUInt::builder("playlist-length")
                    .nick("Playlist length")
                    .blurb("Length of HLS playlist. To allow players to conform to section 6.3.3 of the HLS specification, this should be at least 3. If set to 0, the playlist will be infinite.")
                    .default_value(DEFAULT_PLAYLIST_LENGTH)
                    .build(),
                glib::ParamSpecBoolean::builder("enable-program-date-time")
                    .nick("add EXT-X-PROGRAM-DATE-TIME tag")
                    .blurb("put EXT-X-PROGRAM-DATE-TIME tag in the playlist")
                    .default_value(DEFAULT_PROGRAM_DATE_TIME_TAG)
                    .build(),
                glib::ParamSpecEnum::builder_with_default("program-date-time-reference", DEFAULT_PROGRAM_DATE_TIME_REFERENCE)
                    .nick("Program Date Time Reference")
                    .blurb("Sets the reference for program date time. Pipeline: Use pipeline clock. System: Use system clock (As there might be drift between the wallclock and pipeline clock), BufferReferenceTimestamp: Use buffer time from reference timestamp meta.")
                    .build(),
                glib::ParamSpecBoolean::builder("pdt-follows-pipeline-clock")
                    .nick("Whether Program-Date-Time should follow the pipeline clock")
                    .blurb("As there might be drift between the wallclock and pipeline clock, this controls whether the Program-Date-Time markers should follow the pipeline clock rate (true), or be skewed to match the wallclock rate (false) (deprecated).")
                    .default_value(DEFAULT_CLOCK_TRACKING_FOR_PDT)
                    .build(),
                glib::ParamSpecBoolean::builder("enable-endlist")
                    .nick("Enable Endlist")
                    .blurb("Write \"EXT-X-ENDLIST\" tag to manifest at the end of stream")
                    .default_value(DEFAULT_ENDLIST)
                    .build(),
                glib::ParamSpecString::builder("single-media-file")
                    .nick("Single media file")
                    .blurb("Location of the single media file to write (media playlist will use byte-range addressing)")
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().unwrap();
        match pspec.name() {
            "playlist-location" => {
                settings.playlist_location = value
                    .get::<Option<String>>()
                    .expect("type checked upstream")
                    .unwrap_or_else(|| String::from(DEFAULT_PLAYLIST_LOCATION));
            }
            "playlist-root" => {
                settings.playlist_root = value
                    .get::<Option<String>>()
                    .expect("type checked upstream");
            }
            "max-files" => {
                let max_files: u32 = value.get().expect("type checked upstream");
                settings.max_num_segment_files = max_files as usize;
            }
            "playlist-length" => {
                settings.playlist_length = value.get().expect("type checked upstream");
            }
            "enable-program-date-time" => {
                settings.enable_program_date_time = value.get().expect("type checked upstream");
            }
            "pdt-follows-pipeline-clock" => {
                gst::warning!(CAT, "The 'pdt-follows-pipeline-clock' property is deprecated. Use 'program-date-time-reference' instead.");
                settings.program_date_time_reference =
                    if value.get().expect("type checked upstream") {
                        HlsProgramDateTimeReference::Pipeline
                    } else {
                        HlsProgramDateTimeReference::System
                    };
            }
            "program-date-time-reference" => {
                settings.program_date_time_reference = value.get().expect("type checked upstream");
            }
            "enable-endlist" => {
                settings.enable_endlist = value.get().expect("type checked upstream");
            }
            "single-media-file" => {
                settings.single_media_file = value
                    .get::<Option<String>>()
                    .expect("type checked upstream");
            }
            _ => unimplemented!(),
        };
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().unwrap();
        match pspec.name() {
            "playlist-location" => settings.playlist_location.to_value(),
            "playlist-root" => settings.playlist_root.to_value(),
            "max-files" => {
                let max_files = settings.max_num_segment_files as u32;
                max_files.to_value()
            }
            "playlist-length" => settings.playlist_length.to_value(),
            "enable-program-date-time" => settings.enable_program_date_time.to_value(),
            "pdt-follows-pipeline-clock" => (settings.program_date_time_reference
                == HlsProgramDateTimeReference::Pipeline)
                .to_value(),
            "program-date-time-reference" => settings.program_date_time_reference.to_value(),
            "enable-endlist" => settings.enable_endlist.to_value(),
            "single-media-file" => settings.single_media_file.to_value(),
            _ => unimplemented!(),
        }
    }

    fn signals() -> &'static [glib::subclass::Signal] {
        static SIGNALS: LazyLock<Vec<glib::subclass::Signal>> = LazyLock::new(|| {
            vec![
                glib::subclass::Signal::builder(SIGNAL_GET_PLAYLIST_STREAM)
                    .param_types([String::static_type()])
                    .return_type::<Option<gio::OutputStream>>()
                    .class_handler(|args| {
                        let elem = args[0].get::<super::HlsBaseSink>().expect("signal arg");
                        let playlist_location = args[1].get::<String>().expect("signal arg");
                        let imp = elem.imp();

                        Some(imp.new_file_stream(&playlist_location).ok().to_value())
                    })
                    .accumulator(|_hint, _acc, value| {
                        // First signal handler wins
                        std::ops::ControlFlow::Break(value.clone())
                    })
                    .build(),
                glib::subclass::Signal::builder(SIGNAL_GET_FRAGMENT_STREAM)
                    .param_types([String::static_type()])
                    .return_type::<Option<gio::OutputStream>>()
                    .class_handler(|args| {
                        let elem = args[0].get::<super::HlsBaseSink>().expect("signal arg");
                        let fragment_location = args[1].get::<String>().expect("signal arg");
                        let imp = elem.imp();

                        Some(imp.new_file_stream(&fragment_location).ok().to_value())
                    })
                    .accumulator(|_hint, _acc, value| {
                        // First signal handler wins
                        std::ops::ControlFlow::Break(value.clone())
                    })
                    .build(),
                glib::subclass::Signal::builder(SIGNAL_DELETE_FRAGMENT)
                    .param_types([String::static_type()])
                    .return_type::<bool>()
                    .class_handler(|args| {
                        let elem = args[0].get::<super::HlsBaseSink>().expect("signal arg");
                        let fragment_location = args[1].get::<String>().expect("signal arg");
                        let imp = elem.imp();

                        imp.delete_fragment(&fragment_location);
                        Some(true.to_value())
                    })
                    .accumulator(|_hint, _acc, value| {
                        // First signal handler wins
                        std::ops::ControlFlow::Break(value.clone())
                    })
                    .build(),
            ]
        });

        SIGNALS.as_ref()
    }
}

impl GstObjectImpl for HlsBaseSink {}

impl ElementImpl for HlsBaseSink {
    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        let ret = self.parent_change_state(transition)?;

        match transition {
            gst::StateChange::PlayingToPaused => {
                let mut state = self.state.lock().unwrap();
                if let Some(context) = state.context.as_mut() {
                    // reset mapping from rt to utc. during pause
                    // rt is stopped but utc keep moving so need to
                    // calculate the mapping again
                    context.pdt_base_running_time = None;
                    context.pdt_base_utc = None
                }
            }
            gst::StateChange::PausedToReady => {
                self.close_playlist();
            }
            _ => (),
        }

        Ok(ret)
    }
}

impl BinImpl for HlsBaseSink {}

impl HlsBaseSinkImpl for HlsBaseSink {}

impl HlsBaseSink {
    pub fn open_playlist(&self, playlist: Playlist, segment_template: String) {
        let mut state = self.state.lock().unwrap();
        let settings = self.settings.lock().unwrap();
        state.context = Some(PlaylistContext {
            pdt_base_utc: None,
            pdt_base_running_time: None,
            playlist,
            old_segment_locations: Vec::new(),
            segment_template,
            playlist_location: settings.playlist_location.clone(),
            max_num_segment_files: settings.max_num_segment_files,
            playlist_length: settings.playlist_length,
            single_media_file: settings.single_media_file.is_some(),
        });
    }

    pub fn open_iframe_playlist(
        &self,
        playlist: Playlist,
        segment_template: String,
        playlist_location: String,
    ) {
        let mut state = self.state.lock().unwrap();
        let settings = self.settings.lock().unwrap();

        state.iframe_context = Some(PlaylistContext {
            pdt_base_utc: None,
            pdt_base_running_time: None,
            playlist,
            old_segment_locations: Vec::new(),
            segment_template,
            playlist_location,
            max_num_segment_files: settings.max_num_segment_files,
            playlist_length: settings.playlist_length,
            single_media_file: false,
        });
    }

    pub fn close_playlist(&self) {
        let mut state = self.state.lock().unwrap();
        if let Some(mut context) = state.context.take() {
            if context.playlist.is_rendering() {
                context
                    .playlist
                    .stop(self.settings.lock().unwrap().enable_endlist);
                let _ = self.write_playlist(&mut context);
            }
        }

        if let Some(mut context) = state.iframe_context.take() {
            if context.playlist.is_rendering() {
                context
                    .playlist
                    .stop(self.settings.lock().unwrap().enable_endlist);
                let _ = self.write_playlist(&mut context);
            }
        }
    }

    pub fn get_location(&self, fragment_id: u32) -> Option<String> {
        let settings = self.settings.lock().unwrap();
        if settings.single_media_file.is_none() {
            let mut state = self.state.lock().unwrap();
            let context = match state.context.as_mut() {
                Some(context) => context,
                None => {
                    gst::error!(CAT, imp = self, "Playlist is not configured",);
                    return None;
                }
            };

            sprintf::sprintf!(&context.segment_template, fragment_id).ok()
        } else {
            settings.single_media_file.clone()
        }
    }

    pub fn get_stream_location(&self, fragment_id: u32) -> Option<String> {
        let mut state = self.state.lock().unwrap();
        let context = match state.context.as_mut() {
            Some(context) => context,
            None => {
                gst::error!(CAT, imp = self, "Playlist is not configured",);
                return None;
            }
        };

        let settings = self.settings.lock().unwrap();
        if settings.single_media_file.is_none() {
            let location = match sprintf::sprintf!(&context.segment_template, fragment_id) {
                Ok(file_name) => file_name,
                Err(err) => {
                    gst::error!(CAT, imp = self, "Couldn't build file name, err: {:?}", err,);
                    return None;
                }
            };

            gst::trace!(CAT, imp = self, "Segment location formatted: {}", location);

            Some(location)
        } else {
            Some(settings.single_media_file.as_ref().unwrap().clone())
        }
    }

    pub fn get_fragment_stream(&self, fragment_id: u32) -> Option<(gio::OutputStream, String)> {
        let location = self.get_stream_location(fragment_id)?;

        let settings = self.settings.lock().unwrap();
        let is_single_media_file = settings.single_media_file.is_some();
        drop(settings);

        let mut state = self.state.lock().unwrap();

        if !is_single_media_file {
            let stream = self.obj().emit_by_name::<Option<gio::OutputStream>>(
                SIGNAL_GET_FRAGMENT_STREAM,
                &[&location],
            )?;

            gst::trace!(CAT, imp = self, "Segment location formatted: {}", location);

            Some((stream, location))
        } else if state.stream.is_none() {
            let stream = self.obj().emit_by_name::<Option<gio::OutputStream>>(
                SIGNAL_GET_FRAGMENT_STREAM,
                &[&location],
            )?;

            let gios = super::HlsBaseSinkGioOutputStream::new(stream);
            let stream = gios.upcast_ref::<gio::OutputStream>().clone();

            state.stream = Some(gios);

            Some((stream, location))
        } else {
            let gios = state.stream.as_ref().unwrap();
            let stream = gios.upcast_ref::<gio::OutputStream>().clone();
            Some((stream, location))
        }
    }

    pub fn get_segment_uri(&self, location: &str, prefix: Option<&str>) -> String {
        let settings = self.settings.lock().unwrap();
        let file_name = path::Path::new(&location)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();

        if let Some(prefix) = prefix {
            format!("{prefix}/{file_name}")
        } else if let Some(playlist_root) = &settings.playlist_root {
            format!("{playlist_root}/{file_name}")
        } else {
            file_name.to_string()
        }
    }

    pub fn add_segment(
        &self,
        location: &str,
        running_time: Option<gst::ClockTime>,
        duration: gst::ClockTime,
        timestamp: Option<DateTime<Utc>>,
        mut segment: MediaSegment,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.lock().unwrap();
        let context = match state.context.as_mut() {
            Some(context) => context,
            None => {
                gst::error!(CAT, imp = self, "Playlist is not configured",);

                return Err(gst::FlowError::Error);
            }
        };

        if let Some(running_time) = running_time {
            if context.pdt_base_running_time.is_none() {
                context.pdt_base_running_time = Some(running_time);
            }

            let settings = self.settings.lock().unwrap();

            // Calculate the mapping from running time to UTC
            // calculate pdt_base_utc for each segment if program_date_time_reference == System
            // when following system clock, we calculate the base time every time
            // this avoids the drift between pdt tag and external clock (if gst clock has skew w.r.t external clock)
            if context.pdt_base_utc.is_none()
                || settings.program_date_time_reference == HlsProgramDateTimeReference::System
            {
                let obj = self.obj();
                let now_utc = Utc::now();
                let now_gst = obj.clock().unwrap().time();
                let pts_clock_time = running_time + obj.base_time().unwrap();

                let diff = now_gst.nseconds() as i64 - pts_clock_time.nseconds() as i64;
                let pts_utc = now_utc
                    .checked_sub_signed(Duration::nanoseconds(diff))
                    .expect("offsetting the utc with gstreamer clock-diff overflow");

                context.pdt_base_utc = Some(pts_utc);
            }

            if settings.enable_program_date_time {
                // if program_date_time_reference == BufferReferenceTimestamp and timestamp is provided, use the timestamp provided in the buffer
                if settings.program_date_time_reference
                    == HlsProgramDateTimeReference::BufferReferenceTimestamp
                    && timestamp.is_some()
                {
                    if let Some(timestamp) = timestamp {
                        segment.program_date_time = Some(timestamp.into());
                    }
                } else {
                    // Add the diff of running time to UTC time
                    // date_time = first_segment_utc + (current_seg_running_time - first_seg_running_time)
                    let date_time =
                        context
                            .pdt_base_utc
                            .unwrap()
                            .checked_add_signed(Duration::nanoseconds(
                                running_time
                                    .opt_checked_sub(context.pdt_base_running_time)
                                    .unwrap()
                                    .unwrap()
                                    .nseconds() as i64,
                            ));

                    if let Some(date_time) = date_time {
                        segment.program_date_time = Some(date_time.into());
                    }
                }
            }
        }

        let (init_segment_br, segment_br) = self.byte_ranges(&segment);

        context.playlist.add_segment(segment);

        if context.playlist.is_type_undefined() {
            context.old_segment_locations.push(location.to_string());
        }

        self.write_playlist(context).inspect(|_res| {
            let mut s = gst::Structure::builder("hls-segment-added")
                .field("location", location)
                .field("running-time", running_time.unwrap())
                .field("duration", duration);
            if let Some(ts) = timestamp {
                s = s.field("timestamp", ts.timestamp());
            };
            if let Some(br) = init_segment_br {
                s = s.field("initialization-segment-byte-range", br);
            }
            if let Some(br) = segment_br {
                s = s.field("segment-byte-range", br);
            }
            self.post_message(
                gst::message::Element::builder(s.build())
                    .src(&*self.obj())
                    .build(),
            );
        })
    }

    pub fn add_iframe_segment(
        &self,
        location: &str,
        segment: MediaSegment,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.lock().unwrap();
        let context = match state.iframe_context.as_mut() {
            Some(context) => context,
            None => {
                gst::error!(CAT, imp = self, "I-Frame playlist is not configured",);

                return Err(gst::FlowError::Error);
            }
        };

        let (init_segment_br, segment_br) = self.byte_ranges(&segment);

        context.playlist.add_segment(segment);

        if context.playlist.is_type_undefined() {
            context.old_segment_locations.push(location.to_string());
        }

        self.write_playlist(context)?;

        let mut s = gst::Structure::builder("hls-iframe-segment-added").field("location", location);
        if let Some(br) = init_segment_br {
            s = s.field("initialization-segment-byte-range", br);
        }
        if let Some(br) = segment_br {
            s = s.field("segment-byte-range", br);
        }

        self.post_message(
            gst::message::Element::builder(s.build())
                .src(&*self.obj())
                .build(),
        );

        Ok(gst::FlowSuccess::Ok)
    }

    fn write_playlist(
        &self,
        context: &mut PlaylistContext,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::info!(
            CAT,
            imp = self,
            "Preparing to write new playlist, COUNT {}",
            context.playlist.len()
        );

        context
            .playlist
            .update_playlist_state(context.playlist_length as usize);

        // Acquires the playlist file handle so we can update it with new content. By default, this
        // is expected to be the same file every time.
        let mut playlist_stream = self
            .obj()
            .emit_by_name::<Option<gio::OutputStream>>(
                SIGNAL_GET_PLAYLIST_STREAM,
                &[&context.playlist_location],
            )
            .ok_or_else(|| {
                gst::error!(
                    CAT,
                    imp = self,
                    "Could not get stream to write playlist content",
                );
                gst::FlowError::Error
            })?
            .into_write();

        context
            .playlist
            .write_to(&mut playlist_stream)
            .map_err(|err| {
                gst::error!(
                    CAT,
                    imp = self,
                    "Could not write new playlist: {}",
                    err.to_string()
                );
                gst::FlowError::Error
            })?;
        playlist_stream.flush().map_err(|err| {
            gst::error!(
                CAT,
                imp = self,
                "Could not flush playlist: {}",
                err.to_string()
            );
            gst::FlowError::Error
        })?;

        let delete_fragment = context.playlist.is_type_undefined()
            && context.max_num_segment_files > 0
            && !context.single_media_file;
        if delete_fragment {
            // Cleanup old segments from filesystem
            while context.old_segment_locations.len() > context.max_num_segment_files {
                let old_segment_location = context.old_segment_locations.remove(0);
                if !self
                    .obj()
                    .emit_by_name::<bool>(SIGNAL_DELETE_FRAGMENT, &[&old_segment_location])
                {
                    gst::error!(CAT, imp = self, "Could not delete fragment");
                }
            }
        }

        gst::debug!(CAT, imp = self, "Wrote new playlist file!");
        Ok(gst::FlowSuccess::Ok)
    }

    pub fn new_file_stream<P>(&self, location: &P) -> Result<gio::OutputStream, String>
    where
        P: AsRef<path::Path>,
    {
        let file = gio::File::for_path(location);
        // Open the file for writing, creating it if it doesn't exist
        // Use replace() to write the content in atomic mode
        // (writes to a temporary file and then atomically rename over the destination when the stream is closed)
        let output_stream = file
            .replace(
                None,
                false,
                gio::FileCreateFlags::empty(),
                None::<&gio::Cancellable>,
            )
            .map_err(move |err| {
                let error_msg = gst::error_msg!(
                    gst::ResourceError::OpenWrite,
                    [
                        "Could not open file {} for writing: {}",
                        location.as_ref().to_str().unwrap(),
                        err.to_string(),
                    ]
                );
                self.post_error_message(error_msg);
                err.to_string()
            })?;
        Ok(output_stream.upcast())
    }

    fn delete_fragment<P>(&self, location: &P)
    where
        P: AsRef<path::Path>,
    {
        let _ = fs::remove_file(location).map_err(|err| {
            gst::warning!(
                CAT,
                imp = self,
                "Could not delete segment file: {}",
                err.to_string()
            );
        });
    }

    pub fn is_single_media_file(&self) -> bool {
        let settings = self.settings.lock().unwrap();
        settings.single_media_file.is_some()
    }

    pub fn out_bytes(&self) -> usize {
        let state = self.state.lock().unwrap();
        state.stream.as_ref().unwrap().out_bytes()
    }

    fn byte_ranges(
        &self,
        segment: &MediaSegment,
    ) -> (Option<gst::Structure>, Option<gst::Structure>) {
        let mut init_segment_br = None;
        let mut segment_br = None;

        if let Some(ref map) = segment.map {
            if let Some(br) = &map.byte_range {
                let mut s = gst::Structure::new_empty("byte-range");
                s.set("length", br.length);
                s.set("offset", br.offset.unwrap());
                init_segment_br = Some(s);
            }
        }

        if let Some(ref br) = segment.byte_range {
            let mut s = gst::Structure::new_empty("byte-range");
            s.set("length", br.length);
            s.set("offset", br.offset.unwrap());
            segment_br = Some(s)
        }

        (init_segment_br, segment_br)
    }
}
