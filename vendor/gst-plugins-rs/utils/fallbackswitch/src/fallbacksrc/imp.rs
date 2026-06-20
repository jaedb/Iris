// Copyright (C) 2020 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use parking_lot::Condvar;
use parking_lot::Mutex;

use std::sync::Arc;
use std::time::Instant;
use std::{cmp, mem};

use std::sync::LazyLock;

use super::custom_source::CustomSource;
use super::{RetryReason, Status};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "fallbacksrc",
        gst::DebugColorFlags::empty(),
        Some("Fallback Source Bin"),
    )
});

static DEFAULT_RAW_CAPS: LazyLock<gst::Caps> = LazyLock::new(|| {
    let mut raw_caps = gst::Caps::new_empty();
    raw_caps.merge(gst::Caps::builder("audio/x-raw").any_features().build());
    raw_caps.merge(gst::Caps::builder("video/x-raw").any_features().build());
    raw_caps
});

#[derive(Debug, Clone)]
struct Stats {
    num_retry: u64,
    num_fallback_retry: u64,
    last_retry_reason: RetryReason,
    last_fallback_retry_reason: RetryReason,
    buffering_percent: i32,
    fallback_buffering_percent: i32,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            num_retry: 0,
            num_fallback_retry: 0,
            last_retry_reason: RetryReason::None,
            last_fallback_retry_reason: RetryReason::None,
            buffering_percent: 100,
            fallback_buffering_percent: 100,
        }
    }
}

impl Stats {
    fn to_structure(&self) -> gst::Structure {
        gst::Structure::builder("application/x-fallbacksrc-stats")
            .field("num-retry", self.num_retry)
            .field("num-fallback-retry", self.num_fallback_retry)
            .field("last-retry-reason", self.last_retry_reason)
            .field(
                "last-fallback-retry-reason",
                self.last_fallback_retry_reason,
            )
            .field("buffering-percent", self.buffering_percent)
            .field(
                "fallback-buffering-percent",
                self.fallback_buffering_percent,
            )
            .build()
    }
}

#[derive(Debug, Clone)]
struct Settings {
    uri: Option<String>,
    source: Option<gst::Element>,
    fallback_uri: Option<String>,
    timeout: gst::ClockTime,
    restart_timeout: gst::ClockTime,
    retry_timeout: gst::ClockTime,
    restart_on_eos: bool,
    min_latency: gst::ClockTime,
    buffer_duration: i64,
    immediate_fallback: bool,
    manual_unblock: bool,
    video_caps: gst::Caps,
    audio_caps: gst::Caps,
    dummy_video_caps: gst::Caps,
    dummy_audio_caps: gst::Caps,

    enable_audio: bool,
    enable_video: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let audio_caps = gst::Caps::builder("audio/x-raw").any_features().build();
        let video_caps = gst::Caps::builder("video/x-raw").any_features().build();

        Settings {
            uri: None,
            source: None,
            fallback_uri: None,
            timeout: 5.seconds(),
            restart_timeout: 5.seconds(),
            retry_timeout: 60.seconds(),
            restart_on_eos: false,
            min_latency: gst::ClockTime::ZERO,
            buffer_duration: -1,
            immediate_fallback: false,
            manual_unblock: false,
            video_caps,
            audio_caps,
            dummy_video_caps: gst::Caps::new_any(),
            dummy_audio_caps: gst::Caps::new_any(),

            enable_audio: true,
            enable_video: true,
        }
    }
}

#[derive(Debug)]
enum Source {
    Uri(String),
    Element(gst::Element),
}

// Blocking buffer pad probe on the source pads. Once blocked we have a running time for the
// current buffer that can later be used for offsetting
//
// This is used for the initial offsetting after starting of the stream and for "pausing" when
// buffering.
struct Block {
    pad: gst::Pad,
    probe_id: gst::PadProbeId,
    qos_probe_id: gst::PadProbeId,
    running_time: Option<gst::ClockTime>,
}

struct OutputBranch {
    // source pad from actual source inside the source bin
    source_srcpad: gst::Pad,
    // blocking pad probe on the source pad of the source queue
    source_srcpad_block: Option<Block>,
    // event pad probe on the source pad of the source bin, listening for EOS
    eos_probe: Option<gst::PadProbeId>,

    // other elements in the source bin before the ghostpad
    clocksync: gst::Element,
    converters: gst::Element,
    queue: gst::Element,
    // queue source pad, target pad of the source ghost pad
    queue_srcpad: gst::Pad,

    // Request pad on the fallbackswitch
    switch_pad: gst::Pad,

    // Ghost pad on SourceBin.bin
    ghostpad: gst::GhostPad,
}

// Connects one source pad with fallbackswitch and the corresponding fallback input
struct Output {
    // Main stream and fallback stream branches to the fallback switch
    main_branch: Option<OutputBranch>,
    // If this does not exist then the fallbackswitch is connected directly to the dummy
    // audio/video sources
    fallback_branch: Option<OutputBranch>,

    // Dummy source bin if fallback stream fails or is not present at all
    dummy_source: gst::Bin,

    // fallbackswitch
    // fallbackswitch in the main bin, linked to the ghostpads above
    switch: gst::Element,

    // output source pad on the main bin, switch source pad is ghostpad target
    srcpad: gst::GhostPad,

    // filter caps for the fallback/dummy streams
    filter_caps: gst::Caps,
}

struct Stream {
    // Our internal stream which will be exposed to the outside world
    gst_stream: gst::Stream,

    main_id: Option<glib::GString>,
    fallback_id: Option<glib::GString>,

    // Output of this stream, if selected
    output: Option<Output>,

    /// Whether it's one of the initial two streams which we never remove the output for
    persistent: bool,
}

struct SourceBin {
    // uridecodebin3 or custom source element inside a bin.
    //
    // This bin would also contain imagefreeze, clocksync and queue elements as needed for the
    // outputs and would be connected via ghost pads to the fallbackswitch elements.
    bin: gst::Bin,
    // Actual source element, e.g. uridecodebin3
    source: gst::Element,
    pending_restart: bool,
    running: bool,
    is_live: bool,
    is_image: bool,

    // For timing out the source and shutting it down to restart it
    restart_timeout: Option<gst::SingleShotClockId>,
    // For restarting the source after shutting it down
    pending_restart_timeout: Option<gst::SingleShotClockId>,
    // For failing completely if we didn't recover after the retry timeout
    retry_timeout: Option<gst::SingleShotClockId>,

    // Whole stream collection posted by source
    posted_streams: Option<gst::StreamCollection>,
}

struct State {
    source: SourceBin,
    fallback_source: Option<SourceBin>,
    flow_combiner: gst_base::UniqueFlowCombiner,

    // Our stream collection
    streams: Vec<Stream>,
    // Stream ID prefix
    stream_id_prefix: String,
    // Counters for the stream ids
    num_audio: usize,
    num_video: usize,

    // Counters for the output pads
    num_audio_pads: usize,
    num_video_pads: usize,

    last_buffering_update: Option<Instant>,
    fallback_last_buffering_update: Option<Instant>,

    // Configure settings
    settings: Settings,
    configured_source: Source,

    // Statistics
    stats: Stats,

    // When application is using the manual-unblock property
    manually_blocked: bool,
    // So that we don't schedule a restart when manually unblocking
    // and our source hasn't reached the required state
    schedule_restart_on_unblock: bool,

    // Group ID for all our output streams
    group_id: gst::GroupId,

    // Sequence number for all our srcpad events
    // Chosen randomly on start() and then changed on seek
    seqnum: gst::Seqnum,

    // Used to check if we handled any stream select events after posting our collection
    selection_seqnum: gst::Seqnum,

    // Caps on which to stop decoding on `uridecodebin3`
    source_caps: gst::Caps,
}

impl State {
    fn selected_streams(&self) -> impl Iterator<Item = &Stream> {
        self.streams.iter().filter(|s| s.output.is_some())
    }

    fn stream_collection(&self) -> gst::StreamCollection {
        let streams = self.streams.iter().map(|s| s.gst_stream.clone());
        gst::StreamCollection::builder(None)
            .streams(streams)
            .build()
    }

    // Structure: { "our_stream_id": { "main": "main_stream_id", "fallback": "fallback_stream_id" } }
    fn stream_map(&self) -> gst::Structure {
        let mut map = gst::Structure::builder("fallbacksrc-stream-map");

        for stream in &self.streams {
            let stream_map = gst::Structure::builder("bound-streams")
                .field("main", &stream.main_id)
                .field("fallback", &stream.fallback_id)
                .build();

            map = map.field(stream.gst_stream.stream_id().unwrap(), stream_map);
        }

        map.build()
    }
}

#[derive(Default)]
pub struct FallbackSrc {
    settings: Mutex<Settings>,
    state: Mutex<Option<State>>,
}

#[glib::object_subclass]
impl ObjectSubclass for FallbackSrc {
    const NAME: &'static str = "GstFallbackSrc";
    type Type = super::FallbackSrc;
    type ParentType = gst::Bin;
}

impl ObjectImpl for FallbackSrc {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecBoolean::builder("enable-audio")
                    .nick("Enable Audio (DEPRECATED)")
                    .blurb("Enable the audio stream, this will output silence if there's no audio in the configured URI")
                    .default_value(true)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("enable-video")
                    .nick("Enable Video (DEPRECATED)")
                    .blurb("Enable the video stream, this will output black or the fallback video if there's no video in the configured URI")
                    .default_value(true)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("uri")
                    .nick("URI")
                    .blurb("URI to use")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecObject::builder::<gst::Element>("source")
                    .nick("Source")
                    .blurb("Source to use instead of the URI")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("fallback-uri")
                    .nick("Fallback URI")
                    .blurb("Fallback URI to use for video in case the main stream doesn't work")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt64::builder("timeout")
                    .nick("Timeout")
                    .blurb("Timeout for switching to the fallback URI")
                    .maximum(u64::MAX - 1)
                    .default_value(5 * *gst::ClockTime::SECOND)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt64::builder("restart-timeout")
                    .nick("Timeout")
                    .blurb("Timeout for restarting an active source")
                    .maximum(u64::MAX - 1)
                    .default_value(5 * *gst::ClockTime::SECOND)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt64::builder("retry-timeout")
                    .nick("Retry Timeout")
                    .blurb("Timeout for stopping after repeated failure")
                    .maximum(u64::MAX - 1)
                    .default_value(60 * *gst::ClockTime::SECOND)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("restart-on-eos")
                    .nick("Restart on EOS")
                    .blurb("Restart source on EOS")
                    .default_value(false)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecEnum::builder_with_default("status", Status::Stopped)
                    .nick("Status")
                    .blurb("Current source status")
                    .read_only()
                    .build(),
                glib::ParamSpecUInt64::builder("min-latency")
                    .nick("Minimum Latency")
                    .blurb("When the main source has a higher latency than the fallback source \
                     this allows to configure a minimum latency that would be configured \
                     if initially the fallback is enabled")
                    .maximum(u64::MAX - 1)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecInt64::builder("buffer-duration")
                    .nick("Buffer Duration")
                    .blurb("Buffer duration when buffering streams (-1 default value)")
                    .minimum(-1)
                    .maximum(i64::MAX - 1)
                    .default_value(-1)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Structure>("statistics")
                    .nick("Statistics")
                    .blurb("Various statistics")
                    .read_only()
                    .build(),
                glib::ParamSpecBoolean::builder("manual-unblock")
                    .nick("Manual unblock")
                    .blurb("When enabled, the application must call the unblock signal, except for live streams")
                    .default_value(false)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("immediate-fallback")
                    .nick("Immediate fallback")
                    .blurb("Forward the fallback streams immediately at startup, when the primary streams are slow to start up and immediate output is required")
                    .default_value(false)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Caps>("fallback-video-caps")
                    .nick("Fallback Video Caps")
                    .blurb("Raw video caps for fallback stream (DEPRECATED)")
                    .deprecated()
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Caps>("fallback-audio-caps")
                    .nick("Fallback Audio Caps")
                    .blurb("Raw audio caps for fallback stream (DEPRECATED)")
                    .deprecated()
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Caps>("video-caps")
                    .nick("Video Caps")
                    .blurb("Video caps on which to stop decoding")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Caps>("audio-caps")
                    .nick("Audio Caps")
                    .blurb("Audio caps on which to stop decoding")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Caps>("dummy-video-caps")
                    .nick("Dummy Video Caps")
                    .blurb("Raw video caps for dummy video stream")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Caps>("dummy-audio-caps")
                    .nick("Dummy Audio Caps")
                    .blurb("Raw audio caps for dummy audio stream")
                    .mutable_ready()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "enable-audio" => {
                let mut settings = self.settings.lock();
                let new_value = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing enable-audio from {:?} to {:?}",
                    settings.enable_audio,
                    new_value,
                );
                settings.enable_audio = new_value;
            }
            "enable-video" => {
                let mut settings = self.settings.lock();
                let new_value = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing enable-video from {:?} to {:?}",
                    settings.enable_video,
                    new_value,
                );
                settings.enable_video = new_value;
            }
            "uri" => {
                let mut settings = self.settings.lock();
                let new_value = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing URI from {:?} to {:?}",
                    settings.uri,
                    new_value,
                );
                settings.uri = new_value;
            }
            "source" => {
                let mut settings = self.settings.lock();
                let new_value = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing source from {:?} to {:?}",
                    settings.source,
                    new_value,
                );
                settings.source = new_value;
            }
            "fallback-uri" => {
                let mut settings = self.settings.lock();
                let new_value = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing Fallback URI from {:?} to {:?}",
                    settings.fallback_uri,
                    new_value,
                );
                settings.fallback_uri = new_value;
            }
            "timeout" => {
                let mut settings = self.settings.lock();
                let new_value = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing timeout from {:?} to {:?}",
                    settings.timeout,
                    new_value,
                );
                settings.timeout = new_value;
            }
            "restart-timeout" => {
                let mut settings = self.settings.lock();
                let new_value = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing Restart Timeout from {:?} to {:?}",
                    settings.restart_timeout,
                    new_value,
                );
                settings.restart_timeout = new_value;
            }
            "retry-timeout" => {
                let mut settings = self.settings.lock();
                let new_value = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing Retry Timeout from {:?} to {:?}",
                    settings.retry_timeout,
                    new_value,
                );
                settings.retry_timeout = new_value;
            }
            "restart-on-eos" => {
                let mut settings = self.settings.lock();
                let new_value = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing restart-on-eos from {:?} to {:?}",
                    settings.restart_on_eos,
                    new_value,
                );
                settings.restart_on_eos = new_value;
            }
            "min-latency" => {
                let mut settings = self.settings.lock();
                let new_value = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing Minimum Latency from {:?} to {:?}",
                    settings.min_latency,
                    new_value,
                );
                settings.min_latency = new_value;
            }
            "buffer-duration" => {
                let mut settings = self.settings.lock();
                let new_value = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing Buffer Duration from {:?} to {:?}",
                    settings.buffer_duration,
                    new_value,
                );
                settings.buffer_duration = new_value;
            }
            "immediate-fallback" => {
                let mut settings = self.settings.lock();
                let new_value = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing immediate-fallback from {:?} to {:?}",
                    settings.immediate_fallback,
                    new_value,
                );
                settings.immediate_fallback = new_value;
            }
            "manual-unblock" => {
                let mut settings = self.settings.lock();
                let new_value = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing manual-unblock from {:?} to {:?}",
                    settings.manual_unblock,
                    new_value,
                );
                settings.manual_unblock = new_value;
            }
            "dummy-video-caps" | "fallback-video-caps" => {
                let mut settings = self.settings.lock();
                let new_value = value
                    .get::<Option<gst::Caps>>()
                    .expect("type checked upstream")
                    .unwrap_or_else(gst::Caps::new_any);
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing fallback video caps from {} to {}",
                    settings.dummy_video_caps,
                    new_value,
                );
                settings.dummy_video_caps = new_value;
            }
            "dummy-audio-caps" | "fallback-audio-caps" => {
                let mut settings = self.settings.lock();
                let new_value = value
                    .get::<Option<gst::Caps>>()
                    .expect("type checked upstream")
                    .unwrap_or_else(gst::Caps::new_any);
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing fallback audio caps from {} to {}",
                    settings.dummy_audio_caps,
                    new_value,
                );
                settings.dummy_audio_caps = new_value;
            }
            "video-caps" => {
                let mut settings = self.settings.lock();
                let new_value = value.get::<gst::Caps>().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing video caps from {:?} to {:?}",
                    settings.video_caps,
                    new_value,
                );
                settings.video_caps = new_value;
            }
            "audio-caps" => {
                let mut settings = self.settings.lock();
                let new_value = value.get::<gst::Caps>().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing audio caps from {:?} to {:?}",
                    settings.audio_caps,
                    new_value,
                );
                settings.audio_caps = new_value;
            }
            _ => unimplemented!(),
        }
    }

    // Called whenever a value of a property is read. It can be called
    // at any time from any thread.
    #[allow(clippy::blocks_in_conditions)]
    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "enable-audio" => {
                let settings = self.settings.lock();
                settings.enable_audio.to_value()
            }
            "enable-video" => {
                let settings = self.settings.lock();
                settings.enable_video.to_value()
            }
            "uri" => {
                let settings = self.settings.lock();
                settings.uri.to_value()
            }
            "source" => {
                let settings = self.settings.lock();
                settings.source.to_value()
            }
            "fallback-uri" => {
                let settings = self.settings.lock();
                settings.fallback_uri.to_value()
            }
            "timeout" => {
                let settings = self.settings.lock();
                settings.timeout.to_value()
            }
            "restart-timeout" => {
                let settings = self.settings.lock();
                settings.restart_timeout.to_value()
            }
            "retry-timeout" => {
                let settings = self.settings.lock();
                settings.retry_timeout.to_value()
            }
            "restart-on-eos" => {
                let settings = self.settings.lock();
                settings.restart_on_eos.to_value()
            }
            "status" => {
                let state_guard = self.state.lock();

                // If we have no state then we're stopped
                let state = match &*state_guard {
                    None => return Status::Stopped.to_value(),
                    Some(ref state) => state,
                };

                if !state.source.running {
                    return Status::Stopped.to_value();
                }

                // If any restarts/retries are pending, we're retrying
                if state.source.pending_restart
                    || state.source.pending_restart_timeout.is_some()
                    || state.source.retry_timeout.is_some()
                {
                    return Status::Retrying.to_value();
                }

                // Otherwise if buffering < 100, we have no streams yet or of the expected
                // streams not all have their source pad yet, we're buffering
                if state.stats.buffering_percent < 100
                    || state.source.restart_timeout.is_some()
                    || state.source.posted_streams.is_none()
                {
                    return Status::Buffering.to_value();
                }

                // Output present means a stream was chosen:
                // If its main branch is missing, we didn't get a srcpad for it yet.
                // If we did but it's blocked, at least one other stream is still missing a srcpad.
                // In both cases we're still buffering.
                if state
                    .streams
                    .iter()
                    .filter_map(|s| s.output.as_ref())
                    .map(|o| o.main_branch.as_ref())
                    .any(|b| b.map(|b| b.source_srcpad_block.is_some()).unwrap_or(true))
                {
                    return Status::Buffering.to_value();
                }

                // Otherwise we're running now
                Status::Running.to_value()
            }
            "min-latency" => {
                let settings = self.settings.lock();
                settings.min_latency.to_value()
            }
            "buffer-duration" => {
                let settings = self.settings.lock();
                settings.buffer_duration.to_value()
            }
            "statistics" => self.stats().to_value(),
            "immediate-fallback" => {
                let settings = self.settings.lock();
                settings.immediate_fallback.to_value()
            }
            "manual-unblock" => {
                let settings = self.settings.lock();
                settings.manual_unblock.to_value()
            }
            "dummy-video-caps" | "fallback-video-caps" => {
                let settings = self.settings.lock();
                settings.dummy_video_caps.to_value()
            }
            "dummy-audio-caps" | "fallback-audio-caps" => {
                let settings = self.settings.lock();
                settings.dummy_audio_caps.to_value()
            }
            "video-caps" => {
                let settings = self.settings.lock();
                settings.video_caps.to_value()
            }
            "audio-caps" => {
                let settings = self.settings.lock();
                settings.audio_caps.to_value()
            }
            _ => unimplemented!(),
        }
    }

    fn signals() -> &'static [glib::subclass::Signal] {
        static SIGNALS: LazyLock<Vec<glib::subclass::Signal>> = LazyLock::new(|| {
            vec![
                glib::subclass::Signal::builder("update-uri")
                    .param_types([String::static_type()])
                    .return_type::<String>()
                    .class_handler(|args| {
                        // Simply return the input by default
                        Some(args[1].clone())
                    })
                    .accumulator(|_hint, _acc, value| {
                        // First signal handler wins
                        std::ops::ControlFlow::Break(value.clone())
                    })
                    .build(),
                glib::subclass::Signal::builder("unblock")
                    .action()
                    .class_handler(|args| {
                        let element = args[0].get::<super::FallbackSrc>().expect("signal arg");
                        let imp = element.imp();
                        let mut state_guard = imp.state.lock();
                        let state = match &mut *state_guard {
                            None => {
                                return None;
                            }
                            Some(state) => state,
                        };

                        state.manually_blocked = false;

                        if state.schedule_restart_on_unblock
                            && imp.all_pads_fallback_activated(state)
                        {
                            imp.schedule_source_restart_timeout(state, gst::ClockTime::ZERO, false);
                        }

                        imp.unblock_pads(state, false);

                        None
                    })
                    .build(),
                /**
                 * GstFallbackSrc::map-streams:
                 * @map: A #GstStructure containing the map of our streams to main and fallback streams
                 * @main_collection: (nullable): A #GstStreamCollection posted by the main source
                 * @fallback_collection: (nullable): A #GstStreamCollection posted by the fallback source
                 *
                 * The @map contains the following structure:
                 * { "our_stream_id": { "main": "main_stream_id", "fallback": "fallback_stream_id" } }
                 *
                 * Consumers should only change the fallback IDs and leave other fields untouched.
                 * If any other changes are detected, the new map will be ignored.
                 * If the current mapping is fine, the map should be returned as is.
                 *
                 * This signal is emitted when either the fallback source posts a new stream collection,
                 * or the main source posts a new stream collection after the fallback source has posted one.
                 * Internally, streams will be relinked immediately after this signal is handled.
                 *
                 * Returns: An updated #GstStructure containing the mapping of our stream IDs to main and fallback stream IDs.
                 */
                glib::subclass::Signal::builder("map-streams")
                    .param_types([
                        gst::Structure::static_type(),
                        gst::StreamCollection::static_type(),
                        gst::StreamCollection::static_type(),
                    ])
                    .return_type::<gst::Structure>()
                    .build(),
            ]
        });

        SIGNALS.as_ref()
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.set_suppressed_flags(gst::ElementFlags::SOURCE | gst::ElementFlags::SINK);
        obj.set_element_flags(gst::ElementFlags::SOURCE);
        obj.set_bin_flags(gst::BinFlags::STREAMS_AWARE);
    }
}

impl GstObjectImpl for FallbackSrc {}

impl ElementImpl for FallbackSrc {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            #[cfg(feature = "doc")]
            Status::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
            gst::subclass::ElementMetadata::new(
                "Fallback Source",
                "Generic/Source",
                "Live source with uridecodebin3 or custom source, and fallback stream",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let audio_src_pad_template = gst::PadTemplate::new(
                "audio_%u",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &gst::Caps::new_any(),
            )
            .unwrap();

            let video_src_pad_template = gst::PadTemplate::new(
                "video_%u",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &gst::Caps::new_any(),
            )
            .unwrap();

            vec![audio_src_pad_template, video_src_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    #[allow(clippy::single_match)]
    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst::debug!(CAT, imp = self, "Changing state {:?}", transition);

        match transition {
            gst::StateChange::NullToReady => {
                self.start()?;
            }
            gst::StateChange::ReadyToPaused => {
                self.post_initial_collection()?;

                if let Some(parent) = self.obj().parent() {
                    if let Some(bin) = parent.downcast_ref::<gst::Bin>() {
                        if !bin.bin_flags().contains(gst::BinFlags::STREAMS_AWARE) {
                            self.obj().no_more_pads();
                        }
                    }
                }
            }
            _ => (),
        }

        self.parent_change_state(transition).inspect_err(|_err| {
            gst::error!(
                CAT,
                imp = self,
                "Parent state change transition {:?} failed",
                transition
            );
        })?;

        // Change the source state manually here to be able to catch errors. State changes always
        // happen from sink to source, so we do this after chaining up.
        self.change_source_state(transition, false);

        // Change the fallback source state manually here to be able to catch errors. State changes always
        // happen from sink to source, so we do this after chaining up.
        self.change_source_state(transition, true);

        // Ignore parent state change return to prevent spurious async/no-preroll return values
        // due to core state change bugs
        match transition {
            gst::StateChange::ReadyToPaused
            | gst::StateChange::PausedToPaused
            | gst::StateChange::PlayingToPaused => Ok(gst::StateChangeSuccess::NoPreroll),
            gst::StateChange::ReadyToNull => {
                self.stop();
                Ok(gst::StateChangeSuccess::Success)
            }
            _ => Ok(gst::StateChangeSuccess::Success),
        }
    }

    fn send_event(&self, event: gst::Event) -> bool {
        match event.view() {
            gst::EventView::SelectStreams(e) => {
                gst::debug!(CAT, imp = self, "Handling stream selection event");

                self.handle_select_stream_event(e)
            }
            gst::EventView::Eos(..) => {
                gst::debug!(
                    CAT,
                    imp = self,
                    "Handling element-level EOS, forwarding to all streams"
                );

                let mut state_guard = self.state.lock();
                let state = match &mut *state_guard {
                    None => {
                        return true;
                    }
                    Some(state) => state,
                };

                // We don't want to hold the state lock while pushing out EOS
                let mut send_eos_elements = vec![];
                let mut send_eos_pads = vec![];

                send_eos_elements.push(state.source.bin.clone());

                // Not strictly necessary as the switch will EOS when receiving
                // EOS on its primary pad, just good form.
                if let Some(ref source) = state.fallback_source {
                    send_eos_elements.push(source.bin.clone());
                }

                for output in state.streams.iter().filter_map(|s| s.output.as_ref()) {
                    send_eos_elements.push(output.dummy_source.clone());

                    for branch in [output.main_branch.as_ref(), output.fallback_branch.as_ref()]
                        .iter()
                        .flatten()
                    {
                        send_eos_pads.push(branch.queue.static_pad("sink").unwrap());
                    }
                }

                drop(state_guard);

                for elem in send_eos_elements {
                    elem.send_event(event.clone());
                }

                for pad in send_eos_pads {
                    pad.send_event(event.clone());
                }

                true
            }
            _ => true,
        }
    }
}

impl BinImpl for FallbackSrc {
    fn handle_message(&self, msg: gst::Message) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Buffering(m) => {
                // Don't forward upwards, we handle this internally
                self.handle_buffering(m);
            }
            MessageView::StreamsSelected(m) => {
                // Don't forward upwards, we have internal mapping between our and source streams
                self.handle_streams_selected(m);
            }
            MessageView::StreamCollection(m) => {
                self.handle_stream_collection(m);
            }
            MessageView::Error(m) => {
                if !self.handle_error(m) {
                    self.parent_handle_message(msg);
                }
            }
            _ => self.parent_handle_message(msg),
        }
    }
}

impl FallbackSrc {
    fn create_dummy_audio_source(filter_caps: &gst::Caps, min_latency: gst::ClockTime) -> gst::Bin {
        let bin = gst::Bin::default();

        let audiotestsrc = gst::ElementFactory::make("audiotestsrc")
            .name("audiosrc")
            .property_from_str("wave", "silence")
            .property("is-live", true)
            .build()
            .expect("No audiotestsrc found");

        let audioconvert = gst::ElementFactory::make("audioconvert")
            .name("audio_audioconvert")
            .build()
            .expect("No audioconvert found");

        let audioresample = gst::ElementFactory::make("audioresample")
            .name("audio_audioresample")
            .build()
            .expect("No audioresample found");

        let capsfilter = gst::ElementFactory::make("capsfilter")
            .name("audio_capsfilter")
            .property("caps", filter_caps)
            .build()
            .expect("No capsfilter found");

        let queue = gst::ElementFactory::make("queue")
            .property("max-size-bytes", 0u32)
            .property("max-size-buffers", 0u32)
            .property("max-size-time", cmp::max(min_latency, 1.seconds()))
            .build()
            .expect("No queue found");

        bin.add_many([
            &audiotestsrc,
            &audioconvert,
            &audioresample,
            &capsfilter,
            &queue,
        ])
        .unwrap();

        gst::Element::link_many([
            &audiotestsrc,
            &audioconvert,
            &audioresample,
            &capsfilter,
            &queue,
        ])
        .unwrap();

        let ghostpad = gst::GhostPad::with_target(&queue.static_pad("src").unwrap()).unwrap();
        ghostpad.set_active(true).unwrap();
        bin.add_pad(&ghostpad).unwrap();

        bin
    }

    fn create_dummy_video_source(filter_caps: &gst::Caps, min_latency: gst::ClockTime) -> gst::Bin {
        let bin = gst::Bin::default();

        let videotestsrc = gst::ElementFactory::make("videotestsrc")
            .name("videosrc")
            .property_from_str("pattern", "black")
            .property("is-live", true)
            .build()
            .expect("No videotestsrc found");

        let videoconvert = gst::ElementFactory::make("videoconvert")
            .name("video_videoconvert")
            .build()
            .expect("No videoconvert found");

        let videoscale = gst::ElementFactory::make("videoscale")
            .name("video_videoscale")
            .build()
            .expect("No videoscale found");

        let capsfilter = gst::ElementFactory::make("capsfilter")
            .name("video_capsfilter")
            .property("caps", filter_caps)
            .build()
            .expect("No capsfilter found");

        let queue = gst::ElementFactory::make("queue")
            .property("max-size-bytes", 0u32)
            .property("max-size-buffers", 0u32)
            .property("max-size-time", cmp::max(min_latency, 1.seconds()))
            .build()
            .expect("No queue found");

        bin.add_many([
            &videotestsrc,
            &videoconvert,
            &videoscale,
            &capsfilter,
            &queue,
        ])
        .unwrap();

        gst::Element::link_many([
            &videotestsrc,
            &videoconvert,
            &videoscale,
            &capsfilter,
            &queue,
        ])
        .unwrap();

        let ghostpad = gst::GhostPad::with_target(&queue.static_pad("src").unwrap()).unwrap();
        ghostpad.set_active(true).unwrap();
        bin.add_pad(&ghostpad).unwrap();

        bin
    }

    fn create_main_input(
        &self,
        source: &Source,
        buffer_duration: i64,
        source_caps: gst::Caps,
    ) -> SourceBin {
        let bin = gst::Bin::default();

        let source = match source {
            Source::Uri(ref uri) => {
                let uri = self
                    .obj()
                    .emit_by_name::<glib::GString>("update-uri", &[uri]);

                let source = gst::ElementFactory::make("uridecodebin3")
                    .name("dbin-main")
                    .property("uri", uri)
                    .property("use-buffering", true)
                    .property("buffer-duration", buffer_duration)
                    .property("caps", source_caps)
                    .build()
                    .expect("No uridecodebin3 found");

                source
            }
            Source::Element(ref source) => CustomSource::new(source).upcast(),
        };

        bin.add(&source).unwrap();

        // Handle any async state changes internally, they don't affect the pipeline because we
        // convert everything to a live stream
        bin.set_property("async-handling", true);
        // Don't let the bin handle state changes of the source. We want to do it manually to catch
        // possible errors and retry, without causing the whole bin state change to fail
        bin.set_locked_state(true);

        source.connect_pad_added(move |source, pad| {
            let element = match source
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.downcast::<super::FallbackSrc>().ok())
            {
                None => return,
                Some(element) => element,
            };
            let imp = element.imp();

            if let Err(msg) = imp.handle_source_pad_added(pad, false) {
                element.post_error_message(msg);
            }
        });
        source.connect_pad_removed(move |source, pad| {
            let element = match source
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.downcast::<super::FallbackSrc>().ok())
            {
                None => return,
                Some(element) => element,
            };
            let imp = element.imp();

            imp.handle_source_pad_removed(pad, false);
        });

        self.obj().add(&bin).unwrap();

        SourceBin {
            bin,
            source,
            pending_restart: false,
            running: false,
            is_live: false,
            is_image: false,
            restart_timeout: None,
            pending_restart_timeout: None,
            retry_timeout: None,
            posted_streams: None,
        }
    }

    fn create_fallback_input(
        &self,
        fallback_uri: Option<&str>,
        buffer_duration: i64,
        source_caps: gst::Caps,
    ) -> Option<SourceBin> {
        let source: gst::Element = match fallback_uri {
            Some(uri) => {
                let dbin = gst::ElementFactory::make("uridecodebin3")
                    .name("dbin-fallback")
                    .property("uri", uri)
                    .property("use-buffering", true)
                    .property("buffer-duration", buffer_duration)
                    .property("caps", source_caps)
                    .build()
                    .expect("No uridecodebin3 found");

                dbin
            }
            None => return None,
        };

        let bin = gst::Bin::default();

        bin.add(&source).unwrap();

        source.connect_pad_added(move |source, pad| {
            let element = match source
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.downcast::<super::FallbackSrc>().ok())
            {
                None => return,
                Some(element) => element,
            };
            let imp = element.imp();

            if let Err(msg) = imp.handle_source_pad_added(pad, true) {
                element.post_error_message(msg);
            }
        });
        source.connect_pad_removed(move |source, pad| {
            let element = match source
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.downcast::<super::FallbackSrc>().ok())
            {
                None => return,
                Some(element) => element,
            };
            let src = element.imp();
            src.handle_source_pad_removed(pad, true);
        });

        // Handle any async state changes internally, they don't affect the pipeline because we
        // convert everything to a live stream
        bin.set_property("async-handling", true);
        // Don't let the bin handle state changes of the dbin. We want to do it manually to catch
        // possible errors and retry, without causing the whole bin state change to fail
        bin.set_locked_state(true);

        self.obj().add(&bin).unwrap();

        Some(SourceBin {
            bin,
            source,
            pending_restart: false,
            running: false,
            is_live: false,
            is_image: false,
            restart_timeout: None,
            pending_restart_timeout: None,
            retry_timeout: None,
            posted_streams: None,
        })
    }

    /// Creates a new stream together with its gst::Stream.
    ///
    /// Streams will be later mapped to individual streams of the main/fallback sources, and will
    /// get Outputs assigned if they were selected by the application.
    ///
    /// Initially one audio and one video stream are created, which are persistent. If the main
    /// source adds more streams of a kind then more streams are created here to map to them.
    fn create_stream(
        &self,
        state: &mut State,
        stream_type: gst::StreamType,
        stream_caps: gst::Caps,
        is_persistent: bool,
    ) -> Stream {
        let (stream_id, caps) = match stream_type {
            gst::StreamType::AUDIO => {
                state.num_audio += 1;
                (
                    format!("{}/audio/{}", state.stream_id_prefix, state.num_audio - 1),
                    stream_caps,
                )
            }
            gst::StreamType::VIDEO => {
                state.num_video += 1;
                (
                    format!("{}/video/{}", state.stream_id_prefix, state.num_video - 1),
                    stream_caps,
                )
            }
            _ => unreachable!(),
        };

        let gst_stream = gst::Stream::new(
            Some(&stream_id),
            Some(&caps),
            stream_type,
            gst::StreamFlags::empty(),
        );

        Stream {
            gst_stream: gst_stream.clone(),
            main_id: None,
            fallback_id: None,
            output: None,
            persistent: is_persistent,
        }
    }

    /// Creates a new output for the given stream.
    ///
    /// An output corresponds to a selected stream, and has an external source pad.
    ///
    /// Each output has its own dummy source for simplicity, which is linked to its
    /// fallbackswitch.
    ///
    /// Streams of the main/fallback sources which are mapped to this stream are connected to this
    /// output, and also linked to the fallbackswitch.
    #[allow(clippy::too_many_arguments)]
    fn create_output(
        &self,
        stream: &gst::Stream,
        timeout: gst::ClockTime,
        min_latency: gst::ClockTime,
        is_audio: bool,
        number: usize,
        immediate_fallback: bool,
        filter_caps: &gst::Caps,
        seqnum: gst::Seqnum,
        group_id: gst::GroupId,
    ) -> Output {
        let switch = gst::ElementFactory::make("fallbackswitch")
            .property("timeout", timeout.nseconds())
            .property("min-upstream-latency", min_latency.nseconds())
            .property("immediate-fallback", immediate_fallback)
            .build()
            .expect("No fallbackswitch found");

        self.obj().add(&switch).unwrap();
        switch.sync_state_with_parent().unwrap();

        let dummy_source = if is_audio {
            Self::create_dummy_audio_source(filter_caps, min_latency)
        } else {
            Self::create_dummy_video_source(filter_caps, min_latency)
        };

        self.obj().add(&dummy_source).unwrap();
        dummy_source.sync_state_with_parent().unwrap();

        let dummy_srcpad = dummy_source.static_pad("src").unwrap();
        let dummy_sinkpad = switch.request_pad_simple("sink_%u").unwrap();
        dummy_sinkpad.set_property("priority", 2u32);
        dummy_srcpad
            .link_full(
                &dummy_sinkpad,
                gst::PadLinkCheck::all() & !gst::PadLinkCheck::CAPS,
            )
            .unwrap();

        let stream_clone = stream.clone();
        switch.connect_notify(Some("active-pad"), move |switch, _pspec| {
            let element = match switch
                .parent()
                .and_then(|p| p.downcast::<super::FallbackSrc>().ok())
            {
                None => return,
                Some(element) => element,
            };

            let imp = element.imp();
            imp.handle_switch_active_pad_change(&stream_clone);
        });

        let srcpad = switch.static_pad("src").unwrap();
        let templ = self
            .obj()
            .pad_template(if is_audio { "audio_%u" } else { "video_%u" })
            .unwrap();
        let name = format!("{}_{}", if is_audio { "audio" } else { "video" }, number);

        let ghostpad = gst::GhostPad::builder_from_template_with_target(&templ, &srcpad)
            .unwrap()
            .name(name.clone())
            .proxy_pad_chain_function({
                move |pad, parent, buffer| {
                    let parent = parent.and_then(|p| p.parent());
                    FallbackSrc::catch_panic_pad_function(
                        parent.as_ref(),
                        || Err(gst::FlowError::Error),
                        |imp| imp.proxy_pad_chain(pad, buffer),
                    )
                }
            })
            .proxy_pad_event_function({
                let stream = stream.clone();
                move |pad, parent, event| {
                    let parent = parent.and_then(|p| p.parent());
                    if parent.is_none() {
                        return gst::Pad::event_default(pad, parent.as_ref(), event);
                    }

                    FallbackSrc::catch_panic_pad_function(
                        parent.as_ref(),
                        || false,
                        |imp| imp.proxy_pad_event(&stream, pad, event),
                    )
                }
            })
            .event_function({
                move |pad, parent, event| {
                    if parent.is_none() {
                        return gst::Pad::event_default(pad, parent, event);
                    }

                    FallbackSrc::catch_panic_pad_function(
                        parent,
                        || false,
                        |imp| imp.ghost_pad_event(pad, event),
                    )
                }
            })
            .build();

        // Directly store the stream-start event on the pad before adding so the application can
        // map it to the stream.
        let stream_id = stream.stream_id().unwrap();
        let stream_start_event = gst::event::StreamStart::builder(&stream_id)
            .seqnum(seqnum)
            .group_id(group_id)
            .stream(stream.clone())
            .build();
        ghostpad.set_active(true).unwrap();
        ghostpad.store_sticky_event(&stream_start_event).unwrap();
        self.obj().add_pad(&ghostpad).unwrap();

        Output {
            main_branch: None,
            fallback_branch: None,
            dummy_source,
            switch,
            srcpad: ghostpad.upcast(),
            filter_caps: filter_caps.clone(),
        }
    }

    /// Removes an output and all its elements.
    fn remove_output(&self, mut output: Output) {
        if let Some(ref mut branch) = output.main_branch {
            let source = branch
                .clocksync
                .parent()
                .and_downcast::<gst::Bin>()
                .unwrap();
            self.handle_branch_teardown(&output.switch, &source, branch, false);
        }

        if let Some(ref mut branch) = output.fallback_branch {
            let source = branch
                .clocksync
                .parent()
                .and_downcast::<gst::Bin>()
                .unwrap();
            self.handle_branch_teardown(&output.switch, &source, branch, true);
        }

        let element = self.obj();

        output.switch.set_state(gst::State::Null).unwrap();
        element.remove(&output.switch).unwrap();

        output.dummy_source.set_state(gst::State::Null).unwrap();
        element.remove(&output.dummy_source).unwrap();

        let _ = output.srcpad.set_target(None::<&gst::Pad>);
        element.remove_pad(&output.srcpad).unwrap();
    }

    /// Sets everything up, but doesn't create any outputs.
    /// That happens in ReadyToPaused, when post_initial_collection() is called.
    fn start(&self) -> Result<(), gst::StateChangeError> {
        gst::debug!(CAT, imp = self, "Starting");
        let mut state_guard = self.state.lock();
        if state_guard.is_some() {
            return Err(gst::StateChangeError);
        }

        let settings = self.settings.lock().clone();
        let configured_source = match settings
            .uri
            .as_ref()
            .cloned()
            .map(Source::Uri)
            .or_else(|| settings.source.as_ref().cloned().map(Source::Element))
        {
            Some(source) => source,
            None => {
                gst::error!(CAT, imp = self, "No URI or source element configured");
                gst::element_imp_error!(
                    self,
                    gst::LibraryError::Settings,
                    ["No URI or source element configured"]
                );
                return Err(gst::StateChangeError);
            }
        };

        let fallback_uri = &settings.fallback_uri;

        let mut source_caps = gst::Caps::new_empty();
        source_caps.merge(settings.audio_caps.clone());
        source_caps.merge(settings.video_caps.clone());

        // Create main input
        let source = self.create_main_input(
            &configured_source,
            settings.buffer_duration,
            source_caps.clone(),
        );

        // Create fallback input
        let fallback_source = self.create_fallback_input(
            fallback_uri.as_deref(),
            settings.buffer_duration,
            source_caps.clone(),
        );

        let flow_combiner = gst_base::UniqueFlowCombiner::new();
        let manually_blocked = settings.manual_unblock;

        let audio_caps = settings.audio_caps.clone();
        let video_caps = settings.video_caps.clone();

        let mut state = State {
            source,
            fallback_source,
            streams: vec![],
            stream_id_prefix: format!("{:016x}", rand::random::<u64>()),
            num_audio: 0,
            num_video: 0,
            num_audio_pads: 0,
            num_video_pads: 0,
            flow_combiner,
            last_buffering_update: None,
            fallback_last_buffering_update: None,
            settings,
            configured_source,
            stats: Stats::default(),
            manually_blocked,
            schedule_restart_on_unblock: false,
            group_id: gst::GroupId::next(),
            seqnum: gst::Seqnum::next(),
            selection_seqnum: gst::Seqnum::next(),
            source_caps,
        };

        // Always propose at least 1 video and 1 audio stream by default
        // (will just do fallback/dummy if main source doesn't provide either of those)
        // See post_initial_collection() which is called in ReadyToPaused
        let stream = self.create_stream(&mut state, gst::StreamType::AUDIO, audio_caps, true);
        state.streams.push(stream);
        let stream = self.create_stream(&mut state, gst::StreamType::VIDEO, video_caps, true);
        state.streams.push(stream);

        *state_guard = Some(state);

        drop(state_guard);
        self.obj().notify("status");

        gst::debug!(CAT, imp = self, "Started");
        Ok(())
    }

    /// Posts the initial StreamCollection with our two default streams.
    /// Either user selects no streams, so we go with the defaults and call perform_selection()
    /// ourselves here, or select-streams arrives and it's called there.
    /// Either way, StreamCollection and StreamsSelected messages will be posted as a result.
    fn post_initial_collection(&self) -> Result<(), gst::StateChangeError> {
        let mut state_guard = self.state.lock();
        let state = match &mut *state_guard {
            None => {
                return Ok(());
            }
            Some(state) => state,
        };

        let collection = state.stream_collection();
        assert_eq!(collection.size(), 2);
        let our_seqnum = state.selection_seqnum;

        drop(state_guard);
        gst::debug!(CAT, imp = self, "Posting initial StreamCollection");
        let _ = self
            .obj()
            .post_message(gst::message::StreamCollection::builder(&collection).build());

        state_guard = self.state.lock();
        let state = match &mut *state_guard {
            None => {
                return Ok(());
            }
            Some(state) => state,
        };
        if state.selection_seqnum == our_seqnum {
            gst::debug!(CAT, imp = self, "Using default selection, creating outputs");

            let settings = self.settings.lock();
            let selected_ids = state
                .streams
                .iter()
                .filter(|s| {
                    s.gst_stream.stream_type() == gst::StreamType::AUDIO && settings.enable_audio
                        || s.gst_stream.stream_type() == gst::StreamType::VIDEO
                            && settings.enable_video
                })
                .map(|s| s.gst_stream.stream_id().unwrap())
                .collect::<Vec<_>>();
            drop(settings);
            drop(state_guard);

            if let Some((msg, events)) = self.perform_selection(&selected_ids) {
                for (element, event) in events {
                    if !element.send_event(event) {
                        gst::error!(
                            CAT,
                            imp = self,
                            "Sending select-streams to {} failed",
                            element.name()
                        );

                        return Err(gst::StateChangeError);
                    }
                }

                gst::debug!(
                    CAT,
                    imp = self,
                    "Posting streams-selected for default collection"
                );
                let _ = self.obj().post_message(msg);
            }
        } else {
            gst::debug!(
                CAT,
                imp = self,
                "Stream selection handled while posting default StreamCollection"
            );
        }

        Ok(())
    }

    fn stop(&self) {
        gst::debug!(CAT, imp = self, "Stopping");
        let mut state_guard = self.state.lock();
        let mut state = match state_guard.take() {
            Some(state) => state,
            None => return,
        };
        drop(state_guard);

        self.obj().notify("status");

        // In theory all streams should've been removed from the source's pad-removed signal
        // handler when going from Paused to Ready but better safe than sorry here
        for output in state.streams.drain(..).filter_map(|s| s.output) {
            self.remove_output(output);
        }

        if let Source::Element(ref source) = state.configured_source {
            // Explicitly remove the source element from the CustomSource so that we can
            // later create a new CustomSource and add it again there.
            let parent = if source.has_as_parent(&state.source.bin) {
                Some(&state.source.bin)
            } else if source.has_as_parent(&state.source.source) {
                Some(state.source.source.downcast_ref::<gst::Bin>().unwrap())
            } else {
                None
            };

            if let Some(parent) = parent {
                let _ = source.set_state(gst::State::Null);
                let _ = parent.remove(source);
            }
        }

        for source in [Some(&mut state.source), state.fallback_source.as_mut()]
            .iter_mut()
            .flatten()
        {
            self.obj().remove(&source.bin).unwrap();

            if let Some(timeout) = source.pending_restart_timeout.take() {
                timeout.unschedule();
            }

            if let Some(timeout) = source.retry_timeout.take() {
                timeout.unschedule();
            }

            if let Some(timeout) = source.restart_timeout.take() {
                timeout.unschedule();
            }
        }

        gst::debug!(CAT, imp = self, "Stopped");
    }

    fn change_source_state(&self, transition: gst::StateChange, fallback_source: bool) {
        gst::debug!(
            CAT,
            imp = self,
            "Changing {}source state: {:?}",
            if fallback_source { "fallback " } else { "" },
            transition
        );
        let mut state_guard = self.state.lock();
        let state = match &mut *state_guard {
            Some(state) => state,
            None => return,
        };

        let source = if fallback_source {
            if let Some(ref mut source) = state.fallback_source {
                source
            } else {
                return;
            }
        } else {
            &mut state.source
        };

        source.running = transition.next() > gst::State::Ready;
        if transition.current() <= transition.next() && source.pending_restart {
            gst::debug!(
                CAT,
                imp = self,
                "Not starting {}source because pending restart",
                if fallback_source { "fallback " } else { "" }
            );
            return;
        } else if transition.next() <= gst::State::Ready && source.pending_restart {
            gst::debug!(
                CAT,
                imp = self,
                "Unsetting pending {}restart because shutting down",
                if fallback_source { "fallback " } else { "" }
            );
            source.pending_restart = false;
            if let Some(timeout) = source.pending_restart_timeout.take() {
                timeout.unschedule();
            }
        }
        let source = source.bin.clone();
        drop(state_guard);

        self.obj().notify("status");

        let res = source.set_state(transition.next());
        gst::debug!(
            CAT,
            imp = self,
            "{}source changing state: {:?}",
            if fallback_source { "fallback " } else { "" },
            res
        );
        match res {
            Err(_) => {
                gst::error!(
                    CAT,
                    imp = self,
                    "{}source failed to change state",
                    if fallback_source { "fallback " } else { "" }
                );
                // Try again later if we're not shutting down
                if transition != gst::StateChange::ReadyToNull {
                    let _ = source.set_state(gst::State::Null);
                    let mut state_guard = self.state.lock();
                    let Some(ref mut state) = &mut *state_guard else {
                        return;
                    };
                    self.handle_source_error(
                        state,
                        RetryReason::StateChangeFailure,
                        fallback_source,
                    );
                    drop(state_guard);
                    self.obj().notify("statistics");
                }
            }
            Ok(res) => {
                gst::debug!(
                    CAT,
                    imp = self,
                    "{}source changed state successfully: {:?}",
                    if fallback_source { "fallback " } else { "" },
                    res
                );

                let mut state_guard = self.state.lock();
                let Some(ref mut state) = &mut *state_guard else {
                    return;
                };

                let source = if fallback_source {
                    if let Some(ref mut source) = state.fallback_source {
                        source
                    } else {
                        return;
                    }
                } else {
                    &mut state.source
                };

                // Remember if the source is live
                if transition == gst::StateChange::ReadyToPaused {
                    source.is_live = res == gst::StateChangeSuccess::NoPreroll;
                }

                if (!source.is_live && transition == gst::StateChange::ReadyToPaused)
                    || (source.is_live && transition == gst::StateChange::PausedToPlaying)
                {
                    if !fallback_source {
                        state.schedule_restart_on_unblock = true;
                    }
                    if source.restart_timeout.is_none() {
                        self.schedule_source_restart_timeout(
                            state,
                            gst::ClockTime::ZERO,
                            fallback_source,
                        );
                    }
                } else if (!source.is_live && transition == gst::StateChange::PausedToReady)
                    || (source.is_live && transition == gst::StateChange::PlayingToPaused)
                {
                    if let Some(timeout) = source.pending_restart_timeout.take() {
                        timeout.unschedule();
                    }

                    if let Some(timeout) = source.retry_timeout.take() {
                        timeout.unschedule();
                    }

                    if let Some(timeout) = source.restart_timeout.take() {
                        timeout.unschedule();
                    }
                }
            }
        }
    }

    #[allow(clippy::single_match)]
    fn proxy_pad_event(
        &self,
        stream: &gst::Stream,
        pad: &gst::ProxyPad,
        mut event: gst::Event,
    ) -> bool {
        match event.view() {
            gst::EventView::StreamStart(_) => {
                let stream_id = stream.stream_id().unwrap();

                let mut state_guard = self.state.lock();
                let state = match &mut *state_guard {
                    None => return false,
                    Some(state) => state,
                };

                event = gst::event::StreamStart::builder(&stream_id)
                    .seqnum(state.seqnum)
                    .group_id(state.group_id)
                    .stream(stream.clone())
                    .build();

                drop(state_guard);
            }
            _ => (),
        }

        gst::Pad::event_default(pad, Some(&*self.obj()), event)
    }

    fn proxy_pad_chain(
        &self,
        pad: &gst::ProxyPad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let res = gst::ProxyPad::chain_default(pad, Some(&*self.obj()), buffer);

        let mut state_guard = self.state.lock();
        let state = match &mut *state_guard {
            None => return res,
            Some(state) => state,
        };

        state.flow_combiner.update_pad_flow(pad, res)
    }

    fn ghost_pad_event(&self, pad: &gst::GhostPad, event: gst::Event) -> bool {
        match event.view() {
            gst::EventView::Seek(_s_ev) => {
                let mut state_guard = self.state.lock();
                let state = match &mut *state_guard {
                    None => return false,
                    Some(state) => state,
                };
                state.seqnum = event.seqnum();
                drop(state_guard);

                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            gst::EventView::SelectStreams(ss_ev) => {
                gst::debug!(CAT, imp = self, "Handling stream selection event");
                self.handle_select_stream_event(ss_ev);
                true
            }
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }

    fn create_image_converts(
        &self,
        filter_caps: &gst::Caps,
        fallback_source: bool,
    ) -> gst::Element {
        let imagefreeze = gst::ElementFactory::make("imagefreeze")
            .property("is-live", true)
            .build()
            .expect("No imagefreeze found");

        if !fallback_source || filter_caps.is_any() {
            return imagefreeze;
        }

        let bin = gst::Bin::default();
        let videoconvert = gst::ElementFactory::make("videoconvert")
            .name("video_videoconvert")
            .build()
            .expect("No videoconvert found");

        let videoscale = gst::ElementFactory::make("videoscale")
            .name("video_videoscale")
            .build()
            .expect("No videoscale found");

        let capsfilter = gst::ElementFactory::make("capsfilter")
            .name("video_capsfilter")
            .property("caps", filter_caps)
            .build()
            .expect("No capsfilter found");

        bin.add_many([&videoconvert, &videoscale, &imagefreeze, &capsfilter])
            .unwrap();

        gst::Element::link_many([&videoconvert, &videoscale, &imagefreeze, &capsfilter]).unwrap();

        let ghostpad =
            gst::GhostPad::with_target(&videoconvert.static_pad("sink").unwrap()).unwrap();
        ghostpad.set_active(true).unwrap();
        bin.add_pad(&ghostpad).unwrap();

        let ghostpad = gst::GhostPad::with_target(&capsfilter.static_pad("src").unwrap()).unwrap();
        ghostpad.set_active(true).unwrap();
        bin.add_pad(&ghostpad).unwrap();

        bin.upcast()
    }

    fn create_video_converts(
        &self,
        filter_caps: &gst::Caps,
        fallback_source: bool,
        is_passthrough: bool,
    ) -> gst::Element {
        if !fallback_source || filter_caps.is_any() || is_passthrough {
            return gst::ElementFactory::make("identity")
                .build()
                .expect("No identity found");
        }

        let bin = gst::Bin::default();
        let videoconvert = gst::ElementFactory::make("videoconvert")
            .name("video_videoconvert")
            .build()
            .expect("No videoconvert found");

        let videoscale = gst::ElementFactory::make("videoscale")
            .name("video_videoscale")
            .build()
            .expect("No videoscale found");

        let capsfilter = gst::ElementFactory::make("capsfilter")
            .name("video_capsfilter")
            .property("caps", filter_caps)
            .build()
            .expect("No capsfilter found");

        bin.add_many([&videoconvert, &videoscale, &capsfilter])
            .unwrap();

        gst::Element::link_many([&videoconvert, &videoscale, &capsfilter]).unwrap();

        let ghostpad =
            gst::GhostPad::with_target(&videoconvert.static_pad("sink").unwrap()).unwrap();
        ghostpad.set_active(true).unwrap();
        bin.add_pad(&ghostpad).unwrap();

        let ghostpad = gst::GhostPad::with_target(&capsfilter.static_pad("src").unwrap()).unwrap();
        ghostpad.set_active(true).unwrap();
        bin.add_pad(&ghostpad).unwrap();

        bin.upcast()
    }

    fn create_audio_converts(
        &self,
        filter_caps: &gst::Caps,
        fallback_source: bool,
        is_passthrough: bool,
    ) -> gst::Element {
        if !fallback_source || filter_caps.is_any() || is_passthrough {
            return gst::ElementFactory::make("identity")
                .build()
                .expect("No identity found");
        }

        let bin = gst::Bin::default();
        let audioconvert = gst::ElementFactory::make("audioconvert")
            .name("audio_audioconvert")
            .build()
            .expect("No audioconvert found");

        let audioresample = gst::ElementFactory::make("audioresample")
            .name("audio_audioresample")
            .build()
            .expect("No audioresample found");

        let capsfilter = gst::ElementFactory::make("capsfilter")
            .name("audio_capsfilter")
            .property("caps", filter_caps)
            .build()
            .expect("No capsfilter found");

        bin.add_many([&audioconvert, &audioresample, &capsfilter])
            .unwrap();

        gst::Element::link_many([&audioconvert, &audioresample, &capsfilter]).unwrap();

        let ghostpad =
            gst::GhostPad::with_target(&audioconvert.static_pad("sink").unwrap()).unwrap();
        ghostpad.set_active(true).unwrap();
        bin.add_pad(&ghostpad).unwrap();

        let ghostpad = gst::GhostPad::with_target(&capsfilter.static_pad("src").unwrap()).unwrap();
        ghostpad.set_active(true).unwrap();
        bin.add_pad(&ghostpad).unwrap();

        bin.upcast()
    }

    fn handle_source_pad_added(
        &self,
        pad: &gst::Pad,
        fallback_source: bool,
    ) -> Result<(), gst::ErrorMessage> {
        gst::debug!(
            CAT,
            imp = self,
            "Pad {} added to {}source",
            pad.name(),
            if fallback_source { "fallback " } else { "" }
        );

        let mut state_guard = self.state.lock();
        let state = match &mut *state_guard {
            None => {
                return Ok(());
            }
            Some(state) => state,
        };

        self.setup_output_branch(state, pad, fallback_source)?;

        drop(state_guard);
        self.obj().notify("status");

        Ok(())
    }

    /// Sets up an OutputBranch for a matching Output, which it finds based on the stream ID.
    /// Used by pad-added callback, as well as during relinking if main<->fallback stream mapping changes.
    fn setup_output_branch(
        &self,
        state: &mut State,
        pad: &gst::Pad,
        is_fallback: bool,
    ) -> Result<(), gst::ErrorMessage> {
        let mut is_image = false;
        let is_passthrough = !state.source_caps.can_intersect(&DEFAULT_RAW_CAPS);

        if let Some(ev) = pad.sticky_event::<gst::event::StreamStart>(0) {
            let stream = ev.stream();

            if let Some(caps) = stream.and_then(|s| s.caps()) {
                if let Some(s) = caps.structure(0) {
                    is_image = s.name().starts_with("image/");
                }
            }
        }

        let source = if is_fallback {
            if let Some(ref mut source) = state.fallback_source {
                if source.posted_streams.is_none() {
                    gst::error!(
                        CAT,
                        imp = self,
                        "Got fallback source pads before stream collection"
                    );
                    return Err(gst::error_msg!(
                        gst::CoreError::StateChange,
                        ["Got fallback source pads before stream collection"]
                    ));
                }

                source
            } else {
                return Ok(());
            }
        } else {
            if state.source.posted_streams.is_none() {
                gst::error!(
                    CAT,
                    imp = self,
                    "Got main source pads before stream collection"
                );
                return Err(gst::error_msg!(
                    gst::CoreError::StateChange,
                    ["Got main source pads before stream collection"]
                ));
            }

            &mut state.source
        };

        if is_image {
            if let Some(timeout) = source.pending_restart_timeout.take() {
                timeout.unschedule();
            }

            if let Some(timeout) = source.retry_timeout.take() {
                timeout.unschedule();
            }

            if let Some(timeout) = source.restart_timeout.take() {
                timeout.unschedule();
            }
        }

        source.is_image |= is_image;

        let stream_id = pad.stream_id().unwrap();
        let (is_video, stream) = {
            let Some(stream) = state.streams.iter_mut().find(|s| {
                if is_fallback {
                    s.fallback_id.as_ref() == Some(&stream_id)
                } else {
                    s.main_id.as_ref() == Some(&stream_id)
                }
            }) else {
                gst::warning!(
                    CAT,
                    imp = self,
                    "Source added an unwanted pad with stream id {stream_id}!"
                );
                return Ok(());
            };

            let is_video = stream.gst_stream.stream_type() == gst::StreamType::VIDEO;

            (is_video, stream)
        };

        let (branch_storage, filter_caps, switch) = match stream.output {
            None => {
                gst::warning!(
                    CAT,
                    imp = self,
                    "Source added an unwanted pad with stream id {stream_id}!"
                );
                return Ok(());
            }
            Some(Output {
                ref mut main_branch,
                ref switch,
                ref filter_caps,
                ..
            }) if !is_fallback => {
                if main_branch.is_some() {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Already configured main stream for {}",
                        stream_id
                    );
                    return Ok(());
                }

                (main_branch, filter_caps, switch)
            }
            Some(Output {
                ref mut fallback_branch,
                ref switch,
                ref filter_caps,
                ..
            }) => {
                if fallback_branch.is_some() {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Already configured a fallback stream for {}",
                        stream_id
                    );
                    return Ok(());
                }

                (fallback_branch, filter_caps, switch)
            }
        };

        // FIXME: This adds/removes pads while having the state locked

        // Configure conversion elements only for fallback stream
        // (if fallback caps is not ANY) or image source.
        let converters = if is_image {
            self.create_image_converts(filter_caps, is_fallback)
        } else if is_video {
            self.create_video_converts(filter_caps, is_fallback, is_passthrough)
        } else {
            self.create_audio_converts(filter_caps, is_fallback, is_passthrough)
        };

        let queue = gst::ElementFactory::make("queue")
            .property("max-size-bytes", 0u32)
            .property("max-size-buffers", 0u32)
            .property("max-size-time", 1.seconds())
            .build()
            .unwrap();
        let clocksync = gst::ElementFactory::make("clocksync")
            .build()
            .unwrap_or_else(|_| {
                let identity = gst::ElementFactory::make("identity")
                    .property("sync", true)
                    .build()
                    .unwrap();
                identity
            });

        source
            .bin
            .add_many([&converters, &queue, &clocksync])
            .unwrap();
        converters.sync_state_with_parent().unwrap();
        queue.sync_state_with_parent().unwrap();
        clocksync.sync_state_with_parent().unwrap();

        let sinkpad = converters.static_pad("sink").unwrap();

        pad.link(&sinkpad).map_err(|err| {
            gst::error!(CAT, imp = self, "Failed to link new source pad: {}", err);
            gst::error_msg!(
                gst::CoreError::Negotiation,
                ["Failed to link new source pad: {}", err]
            )
        })?;

        gst::Element::link_many([&converters, &queue, &clocksync]).unwrap();

        let ghostpad = gst::GhostPad::builder_with_target(&clocksync.static_pad("src").unwrap())
            .unwrap()
            .name(pad.name())
            .build();
        let _ = ghostpad.set_active(true);
        source.bin.add_pad(&ghostpad).unwrap();

        // Link the new source pad in
        let switch_pad = switch.request_pad_simple("sink_%u").unwrap();
        switch_pad.set_property("priority", u32::from(is_fallback));
        ghostpad
            .link_full(
                &switch_pad,
                gst::PadLinkCheck::all() & !gst::PadLinkCheck::CAPS,
            )
            .unwrap();

        let stream_id = stream.gst_stream.stream_id().unwrap();
        let eos_probe = pad
            .add_probe(gst::PadProbeType::EVENT_DOWNSTREAM, move |pad, info| {
                let element = match pad
                    .parent()
                    .and_then(|p| p.parent())
                    .and_then(|p| p.parent())
                    .and_then(|p| p.downcast::<super::FallbackSrc>().ok())
                {
                    None => return gst::PadProbeReturn::Ok,
                    Some(element) => element,
                };

                let imp = element.imp();

                let Some(ev) = info.event() else {
                    return gst::PadProbeReturn::Ok;
                };

                if ev.type_() != gst::EventType::Eos {
                    return gst::PadProbeReturn::Ok;
                }

                gst::debug!(
                    CAT,
                    obj = element,
                    "Received EOS from {}source on pad {}",
                    if is_fallback { "fallback " } else { "" },
                    pad.name()
                );

                let mut state_guard = imp.state.lock();
                let state = match &mut *state_guard {
                    None => {
                        return gst::PadProbeReturn::Ok;
                    }
                    Some(state) => state,
                };

                if is_image {
                    gst::PadProbeReturn::Ok
                } else if state.settings.restart_on_eos || is_fallback {
                    imp.handle_source_error(state, RetryReason::Eos, is_fallback);
                    drop(state_guard);
                    element.notify("statistics");

                    gst::PadProbeReturn::Drop
                } else {
                    // EOS all streams if all main branches are EOS
                    let mut sinkpads = vec![];

                    if let Some(output) = {
                        state
                            .streams
                            .iter()
                            .find(|s| s.gst_stream.stream_id().unwrap() == stream_id)
                            .and_then(|s| s.output.as_ref())
                    } {
                        sinkpads.extend(output.switch.sink_pads().into_iter().filter(|p| p != pad));
                    };

                    let all_main_branches_eos = state
                        .streams
                        .iter()
                        .filter_map(|s| s.output.as_ref())
                        .all(|o| {
                            o.main_branch
                                .as_ref()
                                // no main branch = ignore
                                .is_none_or(|b| {
                                    &b.queue_srcpad == pad
                            // now check if the pad is EOS
                            && b.queue_srcpad.pad_flags().contains(gst::PadFlags::EOS)
                                })
                        });

                    if all_main_branches_eos {
                        // EOS all fallbackswitches of outputs without main branch
                        for output in state
                            .streams
                            .iter()
                            .filter_map(|s| s.output.as_ref())
                            .filter(|o| o.main_branch.is_none())
                        {
                            sinkpads
                                .extend(output.switch.sink_pads().into_iter().filter(|p| p != pad));
                        }
                    }

                    let event = ev.clone();
                    element.call_async(move |_| {
                        for sinkpad in sinkpads {
                            sinkpad.send_event(event.clone());
                        }
                    });

                    gst::PadProbeReturn::Ok
                }
            })
            .unwrap();

        let queue_srcpad = queue.static_pad("src").unwrap();
        let source_srcpad_block = Some(self.add_pad_probe(pad, &queue_srcpad, is_fallback));

        *branch_storage = Some(OutputBranch {
            source_srcpad: pad.clone(),
            source_srcpad_block,
            eos_probe: Some(eos_probe),
            clocksync,
            converters,
            queue,
            queue_srcpad,
            switch_pad,
            ghostpad,
        });

        Ok(())
    }

    fn add_pad_probe(&self, pad: &gst::Pad, block_pad: &gst::Pad, fallback_source: bool) -> Block {
        // FIXME: Not literally correct as we add the probe to the queue source pad but that's only
        // a workaround until
        //     https://gitlab.freedesktop.org/gstreamer/gst-plugins-base/-/issues/800
        // is fixed.
        gst::debug!(
            CAT,
            imp = self,
            "Adding blocking probe to pad {} for pad {} (fallback: {})",
            block_pad.name(),
            pad.name(),
            fallback_source,
        );

        let probe_id = block_pad
            .add_probe(
                gst::PadProbeType::BLOCK
                    | gst::PadProbeType::BUFFER
                    | gst::PadProbeType::EVENT_DOWNSTREAM,
                move |inner_pad, info| {
                    let element = match inner_pad
                        .parent()
                        .and_then(|p| p.parent())
                        .and_then(|p| p.parent())
                        .and_then(|p| p.downcast::<super::FallbackSrc>().ok())
                    {
                        None => return gst::PadProbeReturn::Ok,
                        Some(element) => element,
                    };

                    let pts = match info.data {
                        Some(gst::PadProbeData::Buffer(ref buffer)) => buffer.pts(),
                        Some(gst::PadProbeData::Event(ref ev)) => match ev.view() {
                            gst::EventView::Gap(ev) => Some(ev.get().0),
                            _ => return gst::PadProbeReturn::Pass,
                        },
                        _ => unreachable!(),
                    };

                    let imp = element.imp();

                    if let Err(msg) = imp.handle_pad_blocked(inner_pad, pts, fallback_source) {
                        imp.post_error_message(msg);
                    }

                    gst::PadProbeReturn::Ok
                },
            )
            .unwrap();

        let qos_probe_id = block_pad
            .add_probe(gst::PadProbeType::EVENT_UPSTREAM, |_pad, info| {
                let Some(ev) = info.event() else {
                    return gst::PadProbeReturn::Ok;
                };

                if ev.type_() != gst::EventType::Qos {
                    return gst::PadProbeReturn::Ok;
                }

                gst::PadProbeReturn::Drop
            })
            .unwrap();

        Block {
            pad: block_pad.clone(),
            probe_id,
            qos_probe_id,
            running_time: gst::ClockTime::NONE,
        }
    }

    fn handle_pad_blocked(
        &self,
        pad: &gst::Pad,
        pts: impl Into<Option<gst::ClockTime>>,
        fallback_source: bool,
    ) -> Result<(), gst::ErrorMessage> {
        let mut state_guard = self.state.lock();
        let state = match &mut *state_guard {
            None => {
                return Ok(());
            }
            Some(state) => state,
        };

        let (branch, source) = {
            let output = state
                .streams
                .iter_mut()
                .filter_map(|s| s.output.as_mut())
                .find(|o| {
                    if fallback_source {
                        o.fallback_branch
                            .as_ref()
                            .is_some_and(|b| &b.queue_srcpad == pad)
                    } else {
                        o.main_branch
                            .as_ref()
                            .is_some_and(|b| &b.queue_srcpad == pad)
                    }
                })
                .unwrap();

            let branch = if fallback_source {
                output.fallback_branch.as_mut().unwrap()
            } else {
                output.main_branch.as_mut().unwrap()
            };

            let source = if fallback_source {
                if let Some(ref source) = state.fallback_source {
                    source
                } else {
                    return Ok(());
                }
            } else {
                &state.source
            };

            (branch, source)
        };

        gst::debug!(
            CAT,
            imp = self,
            "Called probe on pad {} for pad {} (fallback: {})",
            pad.name(),
            branch.source_srcpad.name(),
            fallback_source
        );

        // Directly unblock for live streams
        if source.is_live {
            if let Some(block) = branch.source_srcpad_block.take() {
                gst::debug!(
                    CAT,
                    imp = self,
                    "Removing pad probe on pad {} for pad {} (fallback: {})",
                    pad.name(),
                    branch.source_srcpad.name(),
                    fallback_source,
                );
                block.pad.remove_probe(block.probe_id);
                block.pad.remove_probe(block.qos_probe_id);
            }

            gst::debug!(CAT, imp = self, "Live source, unblocking directly");

            drop(state_guard);
            self.obj().notify("status");

            return Ok(());
        }

        // Update running time for this block
        let block = match branch.source_srcpad_block {
            Some(ref mut block) => block,
            None => return Ok(()),
        };

        let segment = match pad.sticky_event::<gst::event::Segment>(0) {
            Some(ev) => ev.segment().clone(),
            None => {
                gst::warning!(CAT, imp = self, "Have no segment event yet");
                return Ok(());
            }
        };

        let segment = segment.downcast::<gst::ClockTime>().map_err(|_| {
            gst::error!(CAT, imp = self, "Have no time segment");
            gst::error_msg!(gst::CoreError::Clock, ["Have no time segment"])
        })?;

        let pts = pts.into();
        let running_time = if let Some((_, start)) =
            pts.zip(segment.start()).filter(|(pts, start)| pts < start)
        {
            segment.to_running_time(start)
        } else if let Some((_, stop)) = pts.zip(segment.stop()).filter(|(pts, stop)| pts >= stop) {
            segment.to_running_time(stop)
        } else {
            segment.to_running_time(pts)
        };

        gst::debug!(
            CAT,
            imp = self,
            "Have block running time {}",
            running_time.display(),
        );

        block.running_time = running_time;

        self.unblock_pads(state, fallback_source);

        drop(state_guard);
        self.obj().notify("status");

        Ok(())
    }

    fn unblock_pads(&self, state: &mut State, fallback_source: bool) {
        let current_running_time = match self.obj().current_running_time() {
            Some(current_running_time) => current_running_time,
            None => {
                gst::debug!(CAT, imp = self, "Waiting for current_running_time");
                return;
            }
        };

        if !fallback_source && state.manually_blocked {
            gst::debug!(CAT, imp = self, "Not unblocking yet: manual unblock");
            return;
        }

        // Check if all streams are blocked and have a running time and we have
        // 100% buffering
        if (fallback_source && state.stats.fallback_buffering_percent < 100)
            || (!fallback_source && state.stats.buffering_percent < 100)
        {
            gst::debug!(
                CAT,
                imp = self,
                "Not unblocking yet: buffering {}%",
                state.stats.buffering_percent
            );
            return;
        }

        let source = if fallback_source {
            if let Some(ref source) = state.fallback_source {
                source
            } else {
                // There are no blocked pads if there is no fallback source
                return;
            }
        } else {
            &state.source
        };

        if source.posted_streams.is_none() {
            gst::debug!(CAT, imp = self, "Have no stream collection yet");
            return;
        };

        let mut branches = state
            .streams
            .iter_mut()
            .filter_map(|s| s.output.as_mut())
            .filter_map(|o| {
                if fallback_source {
                    o.fallback_branch.as_mut()
                } else {
                    o.main_branch.as_mut()
                }
            })
            .collect::<Vec<_>>();

        let mut min_running_time = gst::ClockTime::NONE;
        for branch in branches.iter() {
            let running_time = branch
                .source_srcpad_block
                .as_ref()
                .and_then(|b| b.running_time);
            let srcpad = branch.source_srcpad.clone();
            let is_eos = srcpad.pad_flags().contains(gst::PadFlags::EOS);

            if running_time.is_none() && !is_eos {
                gst::debug!(
                    CAT,
                    imp = self,
                    "Waiting for all pads to block (fallback: {})",
                    fallback_source
                );
                return;
            }

            if is_eos {
                gst::debug!(
                    CAT,
                    imp = self,
                    "Ignoring EOS pad for running time (fallback: {})",
                    fallback_source
                );
                // not checking running time for EOS pads
                continue;
            }

            let running_time = running_time.unwrap();
            if min_running_time.is_none_or(|min_running_time| running_time < min_running_time) {
                min_running_time = Some(running_time);
            }
        }

        let offset = min_running_time.map(|min_running_time| {
            if current_running_time > min_running_time {
                (current_running_time - min_running_time).nseconds() as i64
            } else {
                -((min_running_time - current_running_time).nseconds() as i64)
            }
        });

        gst::debug!(
            CAT,
            imp = self,
            "Unblocking at {} with pad offset {:?}, is_fallback: {}",
            current_running_time,
            offset,
            fallback_source
        );

        for branch in branches.iter_mut() {
            if let Some(block) = branch.source_srcpad_block.take() {
                let srcpad = branch.source_srcpad.clone();
                let is_eos = srcpad.pad_flags().contains(gst::PadFlags::EOS);

                if !is_eos {
                    if let Some(offset) = offset {
                        block.pad.set_offset(offset);
                    }
                }
                block.pad.remove_probe(block.probe_id);
                block.pad.remove_probe(block.qos_probe_id);
            }
        }
    }

    /// Remove OutputBranch from the Output if there is still one.
    fn handle_source_pad_removed(&self, pad: &gst::Pad, fallback_source: bool) {
        gst::debug!(
            CAT,
            imp = self,
            "Pad {} removed from {}source",
            pad.name(),
            if fallback_source { "fallback " } else { "" }
        );

        let mut state_guard = self.state.lock();
        let state = match &mut *state_guard {
            None => {
                return;
            }
            Some(state) => state,
        };

        let (mut branch, source, switch) = {
            let Some(stream) = state.streams.iter_mut().find(|s| {
                let Some(ref o) = s.output else { return false };
                if fallback_source {
                    o.fallback_branch
                        .as_ref()
                        .is_some_and(|b| &b.source_srcpad == pad)
                } else {
                    o.main_branch
                        .as_ref()
                        .is_some_and(|b| &b.source_srcpad == pad)
                }
            }) else {
                return;
            };

            let output = stream.output.as_mut().unwrap();

            let branch: OutputBranch = if fallback_source {
                output.fallback_branch.take().unwrap()
            } else {
                output.main_branch.take().unwrap()
            };

            let source = if fallback_source {
                if let Some(ref source) = state.fallback_source {
                    source
                } else {
                    return;
                }
            } else {
                &state.source
            };

            (branch, source.bin.clone(), output.switch.clone())
        };
        drop(state_guard);

        self.handle_branch_teardown(&switch, &source, &mut branch, fallback_source);

        state_guard = self.state.lock();
        let state = match &mut *state_guard {
            None => {
                return;
            }
            Some(state) => state,
        };
        self.unblock_pads(state, fallback_source);

        drop(state_guard);
        self.obj().notify("status");
    }

    fn handle_branch_teardown(
        &self,
        switch: &gst::Element,
        source: &gst::Bin,
        branch: &mut OutputBranch,
        is_fallback: bool,
    ) {
        gst::debug!(
            CAT,
            imp = self,
            "Tearing down branch for pad {}, fallback: {}",
            branch.source_srcpad.name(),
            is_fallback
        );

        branch.converters.set_locked_state(true);
        let _ = branch.converters.set_state(gst::State::Null);
        source.remove(&branch.converters).unwrap();

        branch.queue.set_locked_state(true);
        let _ = branch.queue.set_state(gst::State::Null);
        source.remove(&branch.queue).unwrap();

        branch.clocksync.set_locked_state(true);
        let _ = branch.clocksync.set_state(gst::State::Null);
        source.remove(&branch.clocksync).unwrap();

        if branch.switch_pad.parent().as_ref() == Some(switch.upcast_ref()) {
            switch.release_request_pad(&branch.switch_pad);
        }

        let _ = branch.ghostpad.set_active(false);
        source.remove_pad(&branch.ghostpad).unwrap();

        if let Some(block) = branch.source_srcpad_block.take() {
            block.pad.remove_probe(block.probe_id);
            block.pad.remove_probe(block.qos_probe_id);
        }
    }

    fn handle_buffering(&self, m: &gst::message::Buffering) {
        let mut state_guard = self.state.lock();
        let state = match &mut *state_guard {
            None => {
                return;
            }
            Some(state) => state,
        };

        let src = match m.src() {
            Some(src) => src,
            None => return,
        };

        let fallback_source = if let Some(ref source) = state.fallback_source {
            src.has_as_ancestor(&source.bin)
        } else if src.has_as_ancestor(&state.source.bin) {
            false
        } else {
            return;
        };

        let source = if fallback_source {
            if let Some(ref mut source) = state.fallback_source {
                source
            } else {
                return;
            }
        } else {
            &mut state.source
        };

        if source.pending_restart {
            gst::debug!(CAT, imp = self, "Has pending restart");
            return;
        } else if !source.running {
            gst::debug!(CAT, imp = self, "Was shut down");
            return;
        }

        gst::trace!(
            CAT,
            imp = self,
            "Got buffering {}% (fallback: {})",
            m.percent(),
            fallback_source
        );

        let buffering_percent = if fallback_source {
            &mut state.stats.fallback_buffering_percent
        } else {
            &mut state.stats.buffering_percent
        };
        let last_buffering_update = if fallback_source {
            &mut state.fallback_last_buffering_update
        } else {
            &mut state.last_buffering_update
        };

        *buffering_percent = m.percent();
        if *buffering_percent < 100 {
            *last_buffering_update = Some(Instant::now());
            // Block source pads if needed to pause
            for output in state.streams.iter_mut().filter_map(|s| s.output.as_mut()) {
                let branch = match output {
                    Output {
                        main_branch: Some(ref mut branch),
                        ..
                    } if !fallback_source => branch,
                    Output {
                        fallback_branch: Some(ref mut branch),
                        ..
                    } if fallback_source => branch,
                    _ => continue,
                };

                if branch.source_srcpad_block.is_none() {
                    branch.source_srcpad_block = Some(self.add_pad_probe(
                        &branch.source_srcpad,
                        &branch.queue_srcpad,
                        fallback_source,
                    ));
                }
            }
        } else {
            // Check if we can unblock now
            self.unblock_pads(state, fallback_source);
        }

        drop(state_guard);
        self.obj().notify("status");
        self.obj().notify("statistics");
    }

    /// Handles a stream collection from main/fallback sources.
    /// Calls try_bind_streams() which does the magic to bind stream IDs from the received collection
    /// with our actual output Streams.
    /// If that returns that it has changed the collection (= added or removed a stream on our side),
    /// we'll post a new stream collection.
    ///
    /// Also, even if we don't change the collection, try_bind_streams() might've bound
    /// one of the new source streams to one of our selected ones, so we call perform_selection()
    /// to send select-streams to the relevant source to give us that stream, BUT ignore the message
    /// it creates, as we're not changing our outputs in that case, just input :))
    fn handle_stream_collection(&self, m: &gst::message::StreamCollection) {
        let mut state_guard = self.state.lock();
        let mut state = match &mut *state_guard {
            None => {
                return;
            }
            Some(state) => state,
        };

        let src = match m.src() {
            Some(src) => src,
            None => return,
        };

        let is_fallback = if let Some(ref source) = state.fallback_source {
            src.has_as_ancestor(&source.bin)
        } else if src.has_as_ancestor(&state.source.bin) {
            false
        } else {
            return;
        };

        let collection = m.stream_collection();

        gst::debug!(
            CAT,
            imp = self,
            "Got stream collection {:?} (fallback: {})",
            collection.debug(),
            is_fallback,
        );

        if is_fallback {
            if let Some(ref mut source) = state.fallback_source {
                source.posted_streams = Some(collection.clone());
            }
        } else {
            state.source.posted_streams = Some(collection.clone());
        }

        // We're guaranteed to not remove any streams that are selected or persistent
        // So we don't need to touch outputs here, only in stream-select!
        let should_post = self.try_bind_streams(state, &collection, is_fallback);

        // Need to check if any already present fallback streams have been remapped and relink them accordingly.
        // We can skip relinking if we're sure nothing was linked before, which is only if this is the first
        // fallback stream collection we got since startup.
        let (mut branches_relink, branches_correct) = {
            gst::debug!(
                CAT,
                imp = self,
                "Relinking fallback streams according to mapping"
            );

            // If stream has no fallback bound (e.g. it disappeared) we do nothing, its branch will be removed in pad-removed.
            // If output has no fallback branch (e.g. main source had more streams than fallback), same case, handled in pad-added.
            // In all other cases we need to check if any fallback streams need to be rewired.
            let (mut relink, mut correct) = (vec![], vec![]);
            for stream in state.streams.iter_mut().filter(|s| {
                s.fallback_id.is_some()
                    && s.output
                        .as_ref()
                        .is_some_and(|o| o.fallback_branch.is_some())
            }) {
                let output = stream.output.as_mut().unwrap();
                let branch = output.fallback_branch.as_ref().unwrap();
                let gst_stream = branch.source_srcpad.stream().unwrap();

                if gst_stream.stream_id() == stream.fallback_id {
                    correct.push(output.fallback_branch.as_mut().unwrap());
                } else {
                    let switch = output.switch.clone();
                    let source = state.fallback_source.as_ref().unwrap().bin.clone();
                    let branch = output.fallback_branch.take().unwrap();
                    relink.push((switch, source, branch, None::<gst::PadProbeId>, gst_stream));
                }
            }

            (relink, correct)
        };

        if !branches_relink.is_empty() {
            for branch in branches_correct {
                if branch.source_srcpad_block.is_none() {
                    branch.source_srcpad_block =
                        Some(self.add_pad_probe(&branch.source_srcpad, &branch.queue_srcpad, true));
                }
            }

            drop(state_guard);

            for (switch, source, branch, probe, _) in branches_relink.iter_mut() {
                let srcpad = branch.source_srcpad.clone();
                let stream_id = srcpad.stream_id().unwrap();
                let cvar_pair = Arc::new((Mutex::new(false), Condvar::new()));
                let cvar_pair_clone = cvar_pair.clone();

                gst::debug!(
                    CAT,
                    imp = self,
                    "Removing fallback branch for pad {}, stream {}",
                    srcpad.name(),
                    stream_id
                );

                *probe = Some(
                    srcpad
                        // Waiting for IDLE means there's no stream-id here afterwards for some reason
                        // Workaround is below (manually restoring the stream-start event)
                        .add_probe(gst::PadProbeType::IDLE, move |_, _| {
                            let (lock, cvar) = &*cvar_pair_clone;
                            *lock.lock() = true;
                            cvar.notify_one();
                            gst::PadProbeReturn::Ok
                        })
                        .unwrap(),
                );

                let (lock, cvar) = &*cvar_pair;
                let mut blocked = lock.lock();
                if !*blocked {
                    cvar.wait(&mut blocked);
                }

                if let Some(eos_probe) = branch.eos_probe.take() {
                    srcpad.remove_probe(eos_probe);
                }

                let peer_pad = srcpad.peer().unwrap();
                srcpad.unlink(&peer_pad).unwrap();
                self.handle_branch_teardown(switch, source, branch, true);
            }

            state_guard = self.state.lock();
            state = match &mut *state_guard {
                None => {
                    return;
                }
                Some(state) => state,
            };

            for (_, _, branch, probe_id, stream) in branches_relink {
                let srcpad = branch.source_srcpad.clone();
                gst::debug!(
                    CAT,
                    imp = self,
                    "Setting up new fallback branch for pad {}, stream {}",
                    srcpad.name(),
                    srcpad.stream_id().unwrap()
                );
                // Workaround for stream-id disappearing from the srcpad if we wait for IDLE in the probe
                let stream_start_event =
                    gst::event::StreamStart::builder(&stream.stream_id().unwrap())
                        .seqnum(state.seqnum)
                        .group_id(state.group_id)
                        .stream(stream.clone())
                        .build();
                let _ = srcpad.store_sticky_event(&stream_start_event);
                self.setup_output_branch(state, &srcpad, true).unwrap();
                srcpad.remove_probe(probe_id.unwrap());
            }

            self.unblock_pads(state, true);
            gst::debug!(CAT, imp = self, "Relinking done");
        }

        if should_post {
            let collection = state.stream_collection();
            let our_seqnum = gst::Seqnum::next();
            state.selection_seqnum = our_seqnum;

            drop(state_guard);

            gst::debug!(
                CAT,
                imp = self,
                "Posting new stream collection {:?}",
                collection.debug(),
            );
            let _ = self
                .obj()
                .post_message(gst::message::StreamCollection::builder(&collection).build());

            state_guard = self.state.lock();
            state = match &mut *state_guard {
                None => {
                    return;
                }
                Some(state) => state,
            };

            if state.selection_seqnum != our_seqnum {
                // application selected streams so perform_selection() was already called
                // no action needed, else see below
                return;
            }
        }

        // We didn't post a new collection, OR we did and application didn't select anything,
        // but we need to re-try sending select-streams because we could have bound new streams
        let selected_ids = state
            .selected_streams()
            .map(|s| s.gst_stream.stream_id().unwrap())
            .collect::<Vec<_>>();

        // This call sends select-streams to the sources as necessary.
        // Since try_bind_streams() guarantees that any currently selected streams
        // will stay selected and won't be removed, we don't need the streams-selected message here.
        drop(state_guard);
        if let Some((_msg, events)) = self.perform_selection(&selected_ids) {
            for (element, event) in events {
                if !element.send_event(event) {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Sending select-streams to {} failed, streams might be missing",
                        element.name()
                    );
                }
            }
        }
    }

    /// Attempts to match streams from the incoming collection
    /// with the streams (streams as in streamcollection, not outputs!) we have internally.
    /// Will also create new / remove old streams as needed.
    /// Guarantees that any currently selected streams will not be removed, just unbound and switched to fallback if needed.
    fn try_bind_streams(
        &self,
        state: &mut State,
        collection: &gst::StreamCollection,
        is_fallback: bool,
    ) -> bool {
        let mut collection_changed = false;

        // If a fallback stream disappears, just unbind it.
        // If a main stream disappears, remove the matching stream on our side,
        // unless it's marked as persistent or selected, in which case also unbind it.
        state.streams.retain_mut(|stream| {
            let id = match if is_fallback {
                stream.fallback_id.as_ref()
            } else {
                stream.main_id.as_ref()
            } {
                Some(id) => id,
                None => return true,
            };

            if !collection
                .iter()
                .any(|s| s.stream_id().as_ref() == Some(id))
            {
                if is_fallback {
                    gst::debug!(
                        CAT,
                        imp = self,
                        "Unbinding fallback stream {:?}",
                        stream.gst_stream
                    );
                    stream.fallback_id = None;
                } else if stream.output.is_some() || stream.persistent {
                    // Persistent == one of the 'base' two streams
                    // If persistent or selected, the output is guaranteed to stay (and fallback to dummy if needed),
                    // so we don't have to ever send streams-selected if a source stream disappears.
                    gst::debug!(
                        CAT,
                        imp = self,
                        "Unbinding main stream from selected or persistent output {:?}",
                        stream.gst_stream
                    );
                    stream.main_id = None;
                } else {
                    gst::debug!(
                        CAT,
                        imp = self,
                        "Removing unused stream {:?}",
                        stream.gst_stream
                    );
                    collection_changed = true;
                    return false;
                }
            }

            true
        });

        if !is_fallback {
            // If it's from the main source, try to bind to existing streams or create new ones
            collection_changed |= self.try_bind_main_streams(state, collection);

            // Then ask for mapping and bind fallback streams to the new ones
            if let Some(fallback_collection) = state
                .fallback_source
                .as_ref()
                .and_then(|s| s.posted_streams.clone())
            {
                self.try_bind_fallback_streams(state, &fallback_collection);
            }
        } else {
            // Otherwise just ask which fallback should go where, even if we don't have anything from the main source yet
            self.try_bind_fallback_streams(state, collection);
        }

        collection_changed
    }

    fn try_bind_main_streams(&self, state: &mut State, collection: &gst::StreamCollection) -> bool {
        let mut collection_changed = false;

        for stream in collection {
            // TODO: Handle subtitles and other stream types
            if stream.stream_type() != gst::StreamType::AUDIO
                && stream.stream_type() != gst::StreamType::VIDEO
            {
                continue;
            }

            if state
                .streams
                .iter()
                .any(|s| s.main_id.as_ref() == Some(&stream.stream_id().unwrap()))
            {
                continue;
            }

            // Main streams are always automatically matched to the first available slot we have.
            // If there are no streams with free slots, we create a new one.
            // try_bind_streams() will then ask for mapping to bind fallback streams to those.
            if let Some(suitable_stream) = state
                .streams
                .iter_mut()
                .find(|s| s.main_id.is_none() && s.gst_stream.stream_type() == stream.stream_type())
            {
                suitable_stream.main_id = Some(stream.stream_id().unwrap());

                gst::debug!(
                    CAT,
                    imp = self,
                    "Bound main stream {:?} to our stream {:?}",
                    stream.stream_id().unwrap(),
                    suitable_stream.gst_stream.stream_id().unwrap()
                );
            } else {
                let stream_caps = match stream.stream_type() {
                    gst::StreamType::AUDIO => state.settings.audio_caps.clone(),
                    gst::StreamType::VIDEO => state.settings.video_caps.clone(),
                    _ => unreachable!(),
                };
                let mut new_stream =
                    self.create_stream(state, stream.stream_type(), stream_caps, false);
                new_stream.main_id = Some(stream.stream_id().unwrap());

                gst::debug!(
                    CAT,
                    imp = self,
                    "Adding stream for main {:?} mapped to our stream {:?}",
                    stream.stream_id().unwrap(),
                    new_stream.gst_stream.stream_id().unwrap()
                );

                collection_changed = true;
                state.streams.push(new_stream);
            }
        }

        collection_changed
    }

    fn try_bind_fallback_streams(&self, state: &mut State, collection: &gst::StreamCollection) {
        let mut map = state.stream_map();
        let user_map = self.obj().emit_by_name::<Option<gst::Structure>>(
            "map-streams",
            &[
                &map.clone(),
                &state.source.posted_streams.as_ref(),
                &collection,
            ],
        );

        if let Some(new_map) = user_map {
            if new_map == map {
                gst::debug!(CAT, imp = self, "Stream map unchanged");
            } else if self.verify_map_correct(&map, &new_map, collection) {
                gst::debug!(
                    CAT,
                    imp = self,
                    "Stream map changed, binding fallback streams"
                );
                map = new_map;
            }
        }

        let mut used_ids = vec![];
        for our_stream in state.streams.iter_mut() {
            let id = our_stream.gst_stream.stream_id().unwrap();
            let map_entry = map.get::<&gst::StructureRef>(&id).unwrap();

            // Let's first remove all bindings on our side.
            // We'll bind according to the map first and then auto-match the rest.
            our_stream.fallback_id = None;

            if let Ok(fallback_id) = map_entry.get::<glib::GString>("fallback") {
                let fallback_stream = collection
                    .iter()
                    .find(|s| s.stream_id().as_ref() == Some(&fallback_id))
                    .unwrap();

                if fallback_stream.stream_type() != our_stream.gst_stream.stream_type() {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Fallback stream {:?} type mismatch: expected {:?}, got {:?}, ignoring mapping",
                        fallback_stream.stream_id().unwrap(),
                        our_stream.gst_stream.stream_type(),
                        fallback_stream.stream_type()
                    );

                    continue;
                }

                gst::debug!(
                    CAT,
                    imp = self,
                    "Bound fallback stream {:?} to our stream {:?}",
                    fallback_stream.stream_id().unwrap(),
                    our_stream.gst_stream.stream_id().unwrap()
                );
                our_stream.fallback_id = Some(fallback_id.clone());
                used_ids.push(fallback_id);
            }
        }

        // At this point explicitly mapped streams have been bound,
        // and the rest can just be matched to first available slots.
        for fallback_stream in collection
            .iter()
            .filter(|s| !used_ids.contains(&s.stream_id().unwrap()))
        {
            // TODO: Handle subtitles and other stream types
            if fallback_stream.stream_type() != gst::StreamType::AUDIO
                && fallback_stream.stream_type() != gst::StreamType::VIDEO
            {
                continue;
            }

            if let Some(suitable_stream) = state.streams.iter_mut().find(|s| {
                s.fallback_id.is_none()
                    && s.gst_stream.stream_type() == fallback_stream.stream_type()
            }) {
                suitable_stream.fallback_id = Some(fallback_stream.stream_id().unwrap());

                gst::debug!(
                    CAT,
                    imp = self,
                    "Bound fallback stream {:?} to our stream {:?}",
                    fallback_stream.stream_id().unwrap(),
                    suitable_stream.gst_stream.stream_id().unwrap()
                );
            }
        }
    }

    /// Checks whether, in the provided map:
    /// - All entries for 'our' streams are present
    /// - None of the main stream mappings have changed
    /// - Chosen fallback IDs actually exist in the provided collection
    fn verify_map_correct(
        &self,
        our_map: &gst::Structure,
        user_map: &gst::Structure,
        fallback_collection: &gst::StreamCollection,
    ) -> bool {
        if our_map.len() != user_map.len() {
            gst::warning!(
                CAT,
                imp = self,
                "Amount of entries in map changed: {} -> {}, ignoring map",
                our_map.len(),
                user_map.len()
            );
            return false;
        }

        for (id, entry) in our_map
            .iter()
            .map(|(id, entry)| (id, entry.get::<&gst::StructureRef>().unwrap()))
        {
            let user_entry = match user_map.get::<&gst::StructureRef>(id) {
                Ok(user_entry) => user_entry,
                Err(_) => {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Entry for stream ID {:?} missing, ignoring map",
                        id
                    );
                    return false;
                }
            };

            match (entry.get::<&str>("main"), user_entry.get::<&str>("main")) {
                (Ok(main_id), Ok(user_main_id)) if main_id != user_main_id => {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Main stream mapping for stream {:?} changed: {:?} -> {:?}, ignoring map",
                        id,
                        main_id,
                        user_main_id
                    );
                    return false;
                }
                (Ok(_), Err(_)) => {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Main stream mapping for stream {:?} missing, ignoring map",
                        id
                    );
                    return false;
                }
                _ => {}
            }

            if let Ok(fallback_id) = entry.get::<&str>("fallback") {
                if !fallback_collection
                    .iter()
                    .any(|s| s.stream_id().as_ref().map(|id| id.as_str()) == Some(fallback_id))
                {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Fallback stream ID {:?} not from fallback collection, ignoring map",
                        fallback_id
                    );
                    return false;
                }
            }
        }

        true
    }

    /// Simply calls perform_selection() to give the user outputs for the streams they've selected
    fn handle_select_stream_event(&self, e: &gst::event::SelectStreams) -> bool {
        let mut state_guard = self.state.lock();
        let state = match &mut *state_guard {
            None => {
                return false;
            }
            Some(state) => state,
        };

        let seqnum = e.seqnum();

        if state.selection_seqnum == seqnum {
            gst::debug!(
                CAT,
                imp = self,
                "select-streams with seqnum {:?} already handled",
                seqnum
            );
            return true;
        }

        let selected_streams = e
            .streams()
            .into_iter()
            .map(glib::GString::from)
            .collect::<Vec<_>>();
        gst::debug!(
            CAT,
            imp = self,
            "Got select-streams event with streams {:?}",
            selected_streams
        );

        drop(state_guard);
        if let Some((msg, events)) = self.perform_selection(&selected_streams) {
            self.state.lock().as_mut().unwrap().selection_seqnum = seqnum;
            for (element, event) in events {
                if !element.send_event(event) {
                    gst::error!(
                        CAT,
                        imp = self,
                        "Sending select-streams to {} failed",
                        element.name()
                    );

                    return false;
                }
            }
            gst::debug!(CAT, imp = self, "Posting streams-selected message");
            let _ = self.obj().post_message(msg);
            return true;
        }

        // perform_selection fail indicates an incorrect stream being specified
        false
    }

    /// Goes through all streams we have, checks if they're on the selected list.
    ///
    /// Checks which of them don't have an output yet and creates one if needed.
    /// If any streams have outputs but are not selected anymore, they're removed.
    ///
    /// Afterwards, we send select-streams to main/fallback sources to give us the matching streams.
    /// At the end, this returns a StreamsSelected message that can be posted to the pipeline by
    /// the caller if needed, and the SelectStreams events that are to be sent to the source(s).
    ///
    /// Needs state to be unlocked before calling.
    fn perform_selection(
        &self,
        selected_stream_ids: &[glib::GString],
    ) -> Option<(gst::Message, Vec<(gst::Element, gst::Event)>)> {
        let mut state_guard = self.state.lock();
        let state = match &mut *state_guard {
            None => {
                return None;
            }
            Some(state) => state,
        };

        let stream_ids = state
            .streams
            .iter()
            .map(|s| s.gst_stream.stream_id().unwrap())
            .collect::<Vec<_>>();
        gst::debug!(
            CAT,
            imp = self,
            "Stream IDs: {:?}, select stream IDs: {:?}",
            stream_ids,
            selected_stream_ids
        );

        if selected_stream_ids
            .iter()
            .any(|id| !stream_ids.iter().any(|other_id| id == other_id))
        {
            gst::warning!(
                CAT,
                imp = self,
                "Got unknown stream in select-streams event"
            );
            return None;
        }

        // Removing outputs is susceptible to deadlocking with state being locked,
        // so we do it separately after we've checked all streams.
        // Adding should ideally also be done separately, but so far hasn't caused issues.
        let mut outputs_to_remove = vec![];
        for stream in &mut state.streams {
            let is_selected = selected_stream_ids
                .iter()
                .any(|id| id == &stream.gst_stream.stream_id().unwrap());

            if stream.output.is_some() && !is_selected {
                gst::debug!(
                    CAT,
                    imp = self,
                    "Scheduling output removal for stream {:?}",
                    stream.gst_stream
                );

                let output = stream.output.take().unwrap();
                state.flow_combiner.remove_pad(&output.srcpad);
                outputs_to_remove.push(output);
            } else if stream.output.is_none() && is_selected {
                let is_audio = stream
                    .gst_stream
                    .stream_type()
                    .contains(gst::StreamType::AUDIO);
                let dummy_caps = if is_audio {
                    state.settings.dummy_audio_caps.clone()
                } else {
                    state.settings.dummy_video_caps.clone()
                };

                let number = if is_audio {
                    state.num_audio_pads += 1;
                    state.num_audio_pads - 1
                } else {
                    state.num_video_pads += 1;
                    state.num_video_pads - 1
                };

                let output = self.create_output(
                    &stream.gst_stream,
                    state.settings.timeout,
                    state.settings.min_latency,
                    is_audio,
                    number,
                    state.settings.immediate_fallback,
                    &dummy_caps,
                    state.seqnum,
                    state.group_id,
                );
                state.flow_combiner.add_pad(&output.srcpad);
                stream.output = Some(output);

                gst::debug!(
                    CAT,
                    imp = self,
                    "Added output for stream {:?}",
                    stream.gst_stream.debug()
                );
            }
        }

        drop(state_guard);
        for output in outputs_to_remove {
            gst::debug!(
                CAT,
                imp = self,
                "Removing output for stream {:?}",
                output.srcpad
            );
            self.remove_output(output);
        }
        let mut state_guard = self.state.lock();
        let state = match &mut *state_guard {
            None => {
                return None;
            }
            Some(state) => state,
        };

        let mut events = Vec::new();

        // send selected streams to main and fallback sources
        let main_ids = state
            .selected_streams()
            .filter_map(|s| s.main_id.as_ref())
            .map(|s| s.as_str())
            .collect::<Vec<_>>();
        if !main_ids.is_empty() {
            gst::debug!(
                CAT,
                imp = self,
                "Sending select-streams event to main source with streams {:?}",
                main_ids
            );
            let main_event = gst::event::SelectStreams::builder(main_ids).build();
            events.push((state.source.source.clone(), main_event));
        }

        if let Some(ref source) = state.fallback_source {
            let fallback_ids = state
                .selected_streams()
                .filter_map(|s| s.fallback_id.as_ref())
                .map(|s| s.as_str())
                .collect::<Vec<_>>();
            if !fallback_ids.is_empty() {
                gst::debug!(
                    CAT,
                    imp = self,
                    "Sending select-streams event to fallback source with streams {:?}",
                    fallback_ids
                );
                let fallback_event = gst::event::SelectStreams::builder(fallback_ids).build();
                events.push((source.source.clone(), fallback_event));
            }
        };

        let selected_streams = state
            .streams
            .iter()
            .filter_map(|s| {
                if s.output.is_some() {
                    Some(s.gst_stream.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let collection = state.stream_collection();

        // The collection passed here is 'all streams we have'
        // and then the streams are the selected ones - can be a bit confusing ;)
        Some((
            gst::message::StreamsSelected::builder(&collection)
                .streams(&selected_streams)
                .build(),
            events,
        ))
    }

    fn handle_streams_selected(&self, m: &gst::message::StreamsSelected) {
        let mut state_guard = self.state.lock();
        let state = match &mut *state_guard {
            None => {
                return;
            }
            Some(state) => state,
        };

        let src = match m.src() {
            Some(src) => src,
            None => return,
        };

        let fallback_source = if let Some(ref source) = state.fallback_source {
            src.has_as_ancestor(&source.bin)
        } else if src.has_as_ancestor(&state.source.bin) {
            false
        } else {
            return;
        };

        let streams = m.streams();

        gst::debug!(
            CAT,
            imp = self,
            "Got streams selected {:?} (fallback: {})",
            streams.map(|s| s.stream_id().unwrap()).collect::<Vec<_>>(),
            fallback_source,
        );

        // This might not be the first stream collection and we might have some unblocked pads from
        // before already, which would need to be blocked again now for keeping things in sync
        for branch in state
            .streams
            .iter_mut()
            .filter_map(|s| s.output.as_mut())
            .filter_map(|o| {
                if fallback_source {
                    o.fallback_branch.as_mut()
                } else {
                    o.main_branch.as_mut()
                }
            })
        {
            if branch.source_srcpad_block.is_none() {
                branch.source_srcpad_block = Some(self.add_pad_probe(
                    &branch.source_srcpad,
                    &branch.queue_srcpad,
                    fallback_source,
                ));
            }
        }

        self.unblock_pads(state, fallback_source);

        drop(state_guard);
        self.obj().notify("status");
    }

    fn handle_error(&self, m: &gst::message::Error) -> bool {
        let mut state_guard = self.state.lock();
        let state = match &mut *state_guard {
            None => {
                return false;
            }
            Some(state) => state,
        };

        let src = match m.src().and_then(|s| s.downcast_ref::<gst::Element>()) {
            None => return false,
            Some(src) => src,
        };

        gst::debug!(
            CAT,
            imp = self,
            "Got error message from {}: {}",
            src.path_string(),
            m.error()
        );

        if src == &state.source.bin || src.has_as_ancestor(&state.source.bin) {
            self.handle_source_error(state, RetryReason::Error, false);
            drop(state_guard);
            self.obj().notify("status");
            self.obj().notify("statistics");
            return true;
        }

        // Check if error is from fallback input and if so, use a dummy fallback
        if let Some(ref source) = state.fallback_source {
            if src == &source.bin || src.has_as_ancestor(&source.bin) {
                self.handle_source_error(state, RetryReason::Error, true);
                drop(state_guard);
                self.obj().notify("status");
                self.obj().notify("statistics");
                return true;
            }
        }

        gst::error!(
            CAT,
            imp = self,
            "Give up for error message from {}",
            src.path_string()
        );

        false
    }

    fn handle_source_error(&self, state: &mut State, reason: RetryReason, fallback_source: bool) {
        gst::debug!(
            CAT,
            imp = self,
            "Handling source error (fallback: {}): {:?}",
            fallback_source,
            reason
        );

        if fallback_source {
            state.stats.last_fallback_retry_reason = reason;
        } else {
            state.stats.last_retry_reason = reason;
        }

        let source = if fallback_source {
            state.fallback_source.as_mut().unwrap()
        } else {
            &mut state.source
        };

        if source.pending_restart {
            gst::debug!(
                CAT,
                imp = self,
                "{}source is already pending restart",
                if fallback_source { "fallback " } else { "" }
            );
            return;
        } else if !source.running {
            gst::debug!(
                CAT,
                imp = self,
                "{}source was shut down",
                if fallback_source { "fallback " } else { "" }
            );
            return;
        }

        // Increase retry count only if there was no pending restart
        if fallback_source {
            state.stats.num_fallback_retry += 1;
        } else {
            state.stats.num_retry += 1;
        }

        // Unschedule pending timeout, we're restarting now
        if let Some(timeout) = source.restart_timeout.take() {
            timeout.unschedule();
        }

        // Prevent state changes from changing the state in an uncoordinated way
        source.pending_restart = true;

        // Drop any EOS events from any source pads of the source that might happen because of the
        // error. We don't need to remove these pad probes because restarting the source will also
        // remove/add the pads again.
        for pad in source.bin.src_pads() {
            pad.add_probe(gst::PadProbeType::EVENT_DOWNSTREAM, |_pad, info| {
                let Some(ev) = info.event() else {
                    return gst::PadProbeReturn::Pass;
                };

                if ev.type_() != gst::EventType::Eos {
                    return gst::PadProbeReturn::Pass;
                }

                gst::PadProbeReturn::Drop
            })
            .unwrap();
        }

        let source_weak = source.bin.downgrade();
        self.obj().call_async(move |element| {
            let imp = element.imp();

            let Some(source) = source_weak.upgrade() else {
                return;
            };

            // Remove blocking pad probes if they are still there as otherwise shutting down the
            // source will deadlock on the probes.
            let mut state_guard = imp.state.lock();
            let state = match &mut *state_guard {
                None => {
                    gst::debug!(
                        CAT,
                        imp = imp,
                        "Restarting {}source not needed anymore",
                        if fallback_source { "fallback " } else { "" }
                    );
                    return;
                }
                Some(State { source, .. })
                    if !fallback_source && (!source.pending_restart || !source.running) =>
                {
                    gst::debug!(CAT, imp = imp, "Restarting source not needed anymore");
                    return;
                }
                Some(State {
                    fallback_source: Some(ref source),
                    ..
                }) if fallback_source && (!source.pending_restart || !source.running) => {
                    gst::debug!(
                        CAT,
                        imp = imp,
                        "Restarting fallback source not needed anymore",
                    );
                    return;
                }
                Some(state) => state,
            };
            for (source_srcpad, block) in state
                .streams
                .iter_mut()
                .filter_map(|s| s.output.as_mut())
                .filter_map(|o| {
                    if fallback_source {
                        o.fallback_branch.as_mut()
                    } else {
                        o.main_branch.as_mut()
                    }
                })
                .filter_map(|branch| {
                    if let Some(block) = branch.source_srcpad_block.take() {
                        Some((&branch.source_srcpad, block))
                    } else {
                        None
                    }
                })
            {
                gst::debug!(
                    CAT,
                    imp = imp,
                    "Removing pad probe for pad {}",
                    source_srcpad.name()
                );
                block.pad.remove_probe(block.probe_id);
                block.pad.remove_probe(block.qos_probe_id);
            }
            let switch_sinkpads = state
                .streams
                .iter()
                .filter_map(|s| s.output.as_ref())
                .filter_map(|o| {
                    if fallback_source {
                        o.fallback_branch.as_ref()
                    } else {
                        o.main_branch.as_ref()
                    }
                })
                .map(|branch| branch.switch_pad.clone())
                .collect::<Vec<_>>();
            drop(state_guard);

            gst::debug!(CAT, imp = imp, "Flushing source");
            for pad in switch_sinkpads {
                let _ = pad.push_event(gst::event::FlushStart::builder().build());
                if let Some(switch) = pad.parent().map(|p| p.downcast::<gst::Element>().unwrap()) {
                    switch.release_request_pad(&pad);
                }
            }

            gst::debug!(
                CAT,
                imp = imp,
                "Shutting down {}source",
                if fallback_source { "fallback " } else { "" }
            );
            let _ = source.set_state(gst::State::Null);

            // Sleep for 1s before retrying
            let mut state_guard = imp.state.lock();
            let state = match &mut *state_guard {
                None => {
                    gst::debug!(
                        CAT,
                        imp = imp,
                        "Restarting {}source not needed anymore",
                        if fallback_source { "fallback " } else { "" }
                    );
                    return;
                }
                Some(State { source, .. })
                    if !fallback_source && (!source.pending_restart || !source.running) =>
                {
                    gst::debug!(CAT, imp = imp, "Restarting source not needed anymore");
                    return;
                }
                Some(State {
                    fallback_source: Some(ref source),
                    ..
                }) if fallback_source && (!source.pending_restart || !source.running) => {
                    gst::debug!(
                        CAT,
                        imp = imp,
                        "Restarting fallback source not needed anymore",
                    );
                    return;
                }
                Some(state) => state,
            };

            for branch in state
                .streams
                .iter_mut()
                .filter_map(|s| s.output.as_mut())
                .filter_map(|o| {
                    if fallback_source {
                        o.fallback_branch.as_mut()
                    } else {
                        o.main_branch.as_mut()
                    }
                })
            {
                branch.source_srcpad_block = None;
            }

            gst::debug!(CAT, imp = imp, "Waiting for 1s before retrying");
            let clock = gst::SystemClock::obtain();
            let wait_time = clock.time() + gst::ClockTime::SECOND;
            if fallback_source {
                assert!(state
                    .fallback_source
                    .as_ref()
                    .map(|s| s.pending_restart_timeout.is_none())
                    .unwrap_or(true));
            } else {
                assert!(state.source.pending_restart_timeout.is_none());
            }

            let timeout = clock.new_single_shot_id(wait_time);
            let element_weak = element.downgrade();
            timeout
                .wait_async(move |_clock, _time, _id| {
                    let Some(element) = element_weak.upgrade() else {
                        return;
                    };

                    gst::debug!(CAT, obj = element, "Woke up, retrying");
                    element.call_async(move |element| {
                        let imp = element.imp();

                        let mut state_guard = imp.state.lock();
                        let state = match &mut *state_guard {
                            None => {
                                gst::debug!(
                                    CAT,
                                    imp = imp,
                                    "Restarting {}source not needed anymore",
                                    if fallback_source { "fallback " } else { "" }
                                );
                                return;
                            }
                            Some(State { source, .. })
                                if !fallback_source
                                    && (!source.pending_restart || !source.running) =>
                            {
                                gst::debug!(CAT, imp = imp, "Restarting source not needed anymore");
                                return;
                            }
                            Some(State {
                                fallback_source: Some(ref source),
                                ..
                            }) if fallback_source
                                && (!source.pending_restart || !source.running) =>
                            {
                                gst::debug!(
                                    CAT,
                                    imp = imp,
                                    "Restarting fallback source not needed anymore",
                                );
                                return;
                            }
                            Some(state) => state,
                        };

                        let (source, old_source) = if !fallback_source {
                            if let Source::Uri(..) = state.configured_source {
                                // FIXME: Create a new uridecodebin3 because it currently is not reusable
                                // See https://gitlab.freedesktop.org/gstreamer/gst-plugins-base/-/issues/746
                                element.remove(&state.source.bin).unwrap();

                                let mut source = imp.create_main_input(
                                    &state.configured_source,
                                    state.settings.buffer_duration,
                                    state.source_caps.clone(),
                                );

                                source.running = state.source.running;

                                (
                                    source.bin.clone(),
                                    Some(mem::replace(&mut state.source, source)),
                                )
                            } else {
                                state.source.pending_restart = false;
                                state.source.pending_restart_timeout = None;
                                state.stats.buffering_percent = 100;
                                state.last_buffering_update = None;

                                if let Some(timeout) = state.source.restart_timeout.take() {
                                    gst::debug!(CAT, imp = imp, "Unscheduling restart timeout");
                                    timeout.unschedule();
                                }

                                (state.source.bin.clone(), None)
                            }
                        } else if let Some(ref mut source) = state.fallback_source {
                            source.pending_restart = false;
                            source.pending_restart_timeout = None;
                            state.stats.fallback_buffering_percent = 100;
                            state.fallback_last_buffering_update = None;

                            if let Some(timeout) = source.restart_timeout.take() {
                                gst::debug!(CAT, imp = imp, "Unscheduling restart timeout");
                                timeout.unschedule();
                            }

                            (source.bin.clone(), None)
                        } else {
                            return;
                        };

                        drop(state_guard);

                        if let Some(old_source) = old_source {
                            // Drop old source after releasing the lock, it might call the pad-removed callback
                            // still
                            drop(old_source);
                        }

                        if source.sync_state_with_parent().is_err() {
                            gst::error!(
                                CAT,
                                imp = imp,
                                "{}source failed to change state",
                                if fallback_source { "fallback " } else { "" }
                            );
                            let _ = source.set_state(gst::State::Null);
                            let mut state_guard = imp.state.lock();
                            let Some(ref mut state) = &mut *state_guard else {
                                return;
                            };
                            imp.handle_source_error(
                                state,
                                RetryReason::StateChangeFailure,
                                fallback_source,
                            );
                            drop(state_guard);
                            element.notify("statistics");
                        } else {
                            let mut state_guard = imp.state.lock();
                            let Some(ref mut state) = &mut *state_guard else {
                                return;
                            };
                            let source = if fallback_source {
                                if let Some(source) = &state.fallback_source {
                                    source
                                } else {
                                    return;
                                }
                            } else {
                                &state.source
                            };

                            if source.restart_timeout.is_none() {
                                imp.schedule_source_restart_timeout(
                                    state,
                                    gst::ClockTime::ZERO,
                                    fallback_source,
                                );
                            }
                        }
                    });
                })
                .expect("Failed to wait async");
            if fallback_source {
                if let Some(ref mut source) = state.fallback_source {
                    source.pending_restart_timeout = Some(timeout);
                }
            } else {
                state.source.pending_restart_timeout = Some(timeout);
            }
        });
    }

    #[allow(clippy::blocks_in_conditions)]
    fn schedule_source_restart_timeout(
        &self,
        state: &mut State,
        elapsed: gst::ClockTime,
        fallback_source: bool,
    ) {
        if fallback_source {
            gst::fixme!(
                CAT,
                imp = self,
                "Restart timeout not implemented for fallback source"
            );
            return;
        }

        let source = if fallback_source {
            if let Some(ref mut source) = state.fallback_source {
                source
            } else {
                return;
            }
        } else {
            &mut state.source
        };

        if source.pending_restart {
            gst::debug!(
                CAT,
                imp = self,
                "Not scheduling {}source restart timeout because source is pending restart already",
                if fallback_source { "fallback " } else { "" },
            );
            return;
        } else if !source.running {
            gst::debug!(
                CAT,
                imp = self,
                "Not scheduling {}source restart timeout because source was shut down",
                if fallback_source { "fallback " } else { "" },
            );
            return;
        }

        if source.is_image {
            gst::debug!(
                CAT,
                imp = self,
                "Not scheduling {}source restart timeout because we are playing back an image",
                if fallback_source { "fallback " } else { "" },
            );
            return;
        }

        if !fallback_source && state.manually_blocked {
            gst::debug!(
                CAT,
                imp = self,
                "Not scheduling source restart timeout because we are manually blocked",
            );
            return;
        }

        let clock = gst::SystemClock::obtain();
        let wait_time = clock.time() + state.settings.restart_timeout - elapsed;
        gst::debug!(
            CAT,
            imp = self,
            "Scheduling {}source restart timeout for {}",
            if fallback_source { "fallback " } else { "" },
            wait_time,
        );

        let timeout = clock.new_single_shot_id(wait_time);
        let element_weak = self.obj().downgrade();
        timeout
            .wait_async(move |_clock, _time, _id| {
                let Some(element) = element_weak.upgrade() else {
                    return;
                };

                element.call_async(move |element| {
                    let imp = element.imp();

                    gst::debug!(
                        CAT,
                        imp = imp,
                        "{}source restart timeout triggered",
                        if fallback_source { "fallback " } else { "" }
                    );
                    let mut state_guard = imp.state.lock();
                    let state = match &mut *state_guard {
                        None => {
                            gst::debug!(
                                CAT,
                                imp = imp,
                                "Restarting {}source not needed anymore",
                                if fallback_source { "fallback " } else { "" }
                            );
                            return;
                        }
                        Some(state) => state,
                    };

                    let source = if fallback_source {
                        if let Some(ref mut source) = state.fallback_source {
                            source
                        } else {
                            return;
                        }
                    } else {
                        &mut state.source
                    };

                    source.restart_timeout = None;

                    // If we have the fallback activated then restart the source now.
                    if fallback_source || imp.all_pads_fallback_activated(state) {
                        let (last_buffering_update, buffering_percent) = if fallback_source {
                            (
                                state.fallback_last_buffering_update,
                                state.stats.fallback_buffering_percent,
                            )
                        } else {
                            (state.last_buffering_update, state.stats.buffering_percent)
                        };
                        // If we're not actively buffering right now let's restart the source
                        if last_buffering_update
                            .map(|i| i.elapsed() >= state.settings.restart_timeout.into())
                            .unwrap_or(buffering_percent == 100)
                        {
                            gst::debug!(
                                CAT,
                                imp = imp,
                                "Not buffering, restarting {}source",
                                if fallback_source { "fallback " } else { "" }
                            );

                            imp.handle_source_error(state, RetryReason::Timeout, fallback_source);
                            drop(state_guard);
                            element.notify("statistics");
                        } else {
                            gst::debug!(
                                CAT,
                                imp = imp,
                                "Buffering, restarting {}source later",
                                if fallback_source { "fallback " } else { "" }
                            );
                            let elapsed = last_buffering_update
                                .and_then(|last_buffering_update| {
                                    gst::ClockTime::try_from(last_buffering_update.elapsed()).ok()
                                })
                                .unwrap_or(gst::ClockTime::ZERO);

                            imp.schedule_source_restart_timeout(state, elapsed, fallback_source);
                        }
                    } else {
                        gst::debug!(
                            CAT,
                            imp = imp,
                            "Restarting {}source not needed anymore",
                            if fallback_source { "fallback " } else { "" }
                        );
                    }
                });
            })
            .expect("Failed to wait async");

        source.restart_timeout = Some(timeout);
    }

    #[allow(clippy::blocks_in_conditions)]
    fn all_pads_fallback_activated(&self, state: &State) -> bool {
        // If we have no streams yet, or all active pads for the ones we have
        // are the fallback pads.

        if state.source.posted_streams.is_some() {
            state
                .streams
                .iter()
                .filter_map(|s| s.output.as_ref())
                .all(|o| {
                    let pad = o.switch.property::<Option<gst::Pad>>("active-pad").unwrap();

                    pad.property::<u32>("priority") != 0
                })
        } else {
            true
        }
    }

    #[allow(clippy::blocks_in_conditions)]
    fn has_fallback_activated(&self, state: &State, gst_stream: &gst::Stream) -> bool {
        // This is only called from handle_switch_active_pad_change() below,
        // so we can assume that the stream mentioned does have an output

        let stream = state
            .streams
            .iter()
            .find(|s| s.gst_stream.stream_id() == gst_stream.stream_id())
            .unwrap();

        let output = stream.output.as_ref().unwrap();
        output
            .switch
            .property::<Option<gst::Pad>>("active-pad")
            .unwrap()
            .property::<u32>("priority")
            != 0
    }

    fn handle_switch_active_pad_change(&self, stream: &gst::Stream) {
        let mut state_guard = self.state.lock();
        let state = match &mut *state_guard {
            None => {
                return;
            }
            Some(state) => state,
        };

        // If we have the fallback activated then start the retry timeout once
        // unless it was started already. Otherwise cancel the retry timeout.
        // If not all pads have fallback activated by the time this timeout triggers,
        // (e.g. we have a main source, but with less streams than the user selected)
        // the source will not be restarted.
        if self.has_fallback_activated(state, stream) {
            gst::warning!(
                CAT,
                imp = self,
                "Switched to {} fallback stream",
                stream.stream_id().unwrap()
            );
            if state.source.restart_timeout.is_none() {
                self.schedule_source_restart_timeout(state, gst::ClockTime::ZERO, false);
            }
        } else {
            gst::debug!(
                CAT,
                imp = self,
                "Switched to {} main stream",
                stream.stream_id().unwrap()
            );
            if let Some(timeout) = state.source.retry_timeout.take() {
                gst::debug!(CAT, imp = self, "Unscheduling retry timeout");
                timeout.unschedule();
            }

            if let Some(timeout) = state.source.restart_timeout.take() {
                gst::debug!(CAT, imp = self, "Unscheduling restart timeout");
                timeout.unschedule();
            }
        }

        drop(state_guard);
        self.obj().notify("status");
    }

    fn stats(&self) -> gst::Structure {
        let state_guard = self.state.lock();

        let state = match &*state_guard {
            None => return Stats::default().to_structure(),
            Some(ref state) => state,
        };

        state.stats.to_structure()
    }
}
