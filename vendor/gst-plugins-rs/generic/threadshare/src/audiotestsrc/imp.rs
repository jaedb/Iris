// Copyright (C) 2022 François Laignel <fengalin@free.fr>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use byte_slice_cast::*;
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use num_traits::{cast::FromPrimitive, identities::Zero, ops::bytes::ToBytes};

use std::mem;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;
#[cfg(feature = "tuning")]
use std::time::Instant;

use crate::runtime::prelude::*;
use crate::runtime::{self, task, timer, PadSrc, Task};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "ts-audiotestsrc",
        gst::DebugColorFlags::empty(),
        Some("Thread-sharing audio test src"),
    )
});

static CAPS_FILTER: LazyLock<gst::Caps> = LazyLock::new(|| {
    gst_audio::AudioCapsBuilder::new_interleaved()
        .format_list([
            gst_audio::AudioFormat::S16le,
            gst_audio::AudioFormat::S16be,
            gst_audio::AudioFormat::U16le,
            gst_audio::AudioFormat::U16be,
            gst_audio::AudioFormat::S32le,
            gst_audio::AudioFormat::S32be,
            gst_audio::AudioFormat::U32le,
            gst_audio::AudioFormat::U32be,
            gst_audio::AudioFormat::F32le,
            gst_audio::AudioFormat::F32be,
            gst_audio::AudioFormat::F64le,
            gst_audio::AudioFormat::F64be,
            gst_audio::AudioFormat::S8,
            gst_audio::AudioFormat::U8,
        ])
        .build()
});

static DEFAULT_CAPS: LazyLock<gst::Caps> = LazyLock::new(|| {
    let caps = gst_audio::AudioInfo::builder(DEFAULT_FORMAT, DEFAULT_RATE, DEFAULT_CHANNELS as u32)
        .build()
        .unwrap()
        .to_caps()
        .unwrap();

    assert!(CAPS_FILTER.can_intersect(&caps));

    caps
});

const DEFAULT_CONTEXT: &str = "";
const DEFAULT_CONTEXT_WAIT: Duration = Duration::ZERO;
const DEFAULT_SAMPLES_PER_BUFFER: u32 = 1024;
const DEFAULT_FORMAT: gst_audio::AudioFormat = gst_audio::AUDIO_FORMAT_S16;
const DEFAULT_RATE: u32 = 44_100;
const DEFAULT_CHANNELS: usize = 1;
const DEFAULT_FREQ: u32 = 440;
const DEFAULT_VOLUME: f64 = 0.8;
const DEFAULT_MUTE: bool = false;
const DEFAULT_IS_LIVE: bool = true;
const DEFAULT_NUM_BUFFERS: i32 = -1;

// Audio format definitions
const S_OFFSET: u64 = 0;
const F_MAX: u64 = 1;
const F_OFFSET: u64 = 0;
const B_ENDIAN: bool = false;
const L_ENDIAN: bool = true;

const S8_MAX: u64 = i8::MAX as u64;
const U8_MAX: u64 = i8::MAX as u64;
const U8_OFFSET: u64 = i8::MAX as u64 + 1;
const S16_MAX: u64 = i16::MAX as u64;
const U16_MAX: u64 = i16::MAX as u64;
const U16_OFFSET: u64 = i16::MAX as u64 + 1;
const S32_MAX: u64 = i32::MAX as u64;
const U32_MAX: u64 = i32::MAX as u64;
const U32_OFFSET: u64 = i32::MAX as u64 + 1;

#[cfg(feature = "tuning")]
const RAMPUP_BUFFER_COUNT: u32 = 500;
#[cfg(feature = "tuning")]
const LOG_BUFFER_INTERVAL: u32 = 2000;

#[derive(Debug, Clone)]
struct Settings {
    context: String,
    context_wait: Duration,
    samples_per_buffer: u32,
    freq: u32,
    volume: f64,
    mute: bool,
    is_live: bool,
    num_buffers: Option<u32>,
    #[cfg(feature = "tuning")]
    is_main_elem: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            context: DEFAULT_CONTEXT.into(),
            context_wait: DEFAULT_CONTEXT_WAIT,
            samples_per_buffer: DEFAULT_SAMPLES_PER_BUFFER,
            freq: DEFAULT_FREQ,
            volume: DEFAULT_VOLUME,
            mute: DEFAULT_MUTE,
            is_live: DEFAULT_IS_LIVE,
            num_buffers: None,
            #[cfg(feature = "tuning")]
            is_main_elem: false,
        }
    }
}

#[derive(Clone, Debug)]
struct AudioTestSrcPadHandler;
impl PadSrcHandler for AudioTestSrcPadHandler {
    type ElementImpl = AudioTestSrc;

    fn src_query(
        self,
        pad: &gst::Pad,
        elem: &Self::ElementImpl,
        query: &mut gst::QueryRef,
    ) -> bool {
        gst::debug!(CAT, obj = pad, "Received {query:?}");

        if query.is_serialized() {
            // See comment in runtime::pad::PadSrcHandler
            return false;
        }

        if let gst::QueryViewMut::Latency(q) = query.view_mut() {
            let rate = elem
                .caps
                .lock()
                .unwrap()
                .structure(0)
                .unwrap()
                .get::<i32>("rate")
                .expect("negotiated");

            let settings = elem.settings.lock().unwrap();
            // `delay_for_at_least` timers can be up to 'context-wait' late
            // See comment in `AudioTestSrcTask::try_next`
            let context_wait =
                gst::ClockTime::from_nseconds(settings.context_wait.as_nanos() as u64);
            let min = gst::ClockTime::SECOND
                .mul_div_floor(settings.samples_per_buffer as u64, rate as u64)
                .unwrap()
                + context_wait;

            gst::debug!(
                CAT,
                imp = elem,
                "Returning latency: live {}, min {min}, max None",
                settings.is_live
            );
            q.set(settings.is_live, min, gst::ClockTime::NONE);

            return true;
        }

        gst::Pad::query_default(pad, Some(&*elem.obj()), query)
    }
}

#[derive(Debug)]
struct AudioTestSrcTask {
    elem: super::AudioTestSrc,
    segment: gst::FormattedSegment<gst::format::Time>,
    need_initial_events: bool,

    info: gst_audio::AudioInfo,
    volume: f64,
    mute: bool,
    freq: f64,
    is_live: bool,
    samples_per_buffer: u32,
    buffer_duration: gst::ClockTime,
    sample_offset: u64,
    sample_stop: Option<u64>,
    step: f64,
    accumulator: f64,

    buffer_count: u32,
    num_buffers: Option<u32>,
    #[cfg(feature = "tuning")]
    is_main_elem: bool,
    #[cfg(feature = "tuning")]
    parked_duration_init: Option<Duration>,
    #[cfg(feature = "tuning")]
    log_start: Instant,
}

impl AudioTestSrcTask {
    fn new(elem: super::AudioTestSrc) -> Self {
        let imp = elem.imp();
        let info = gst_audio::AudioInfo::from_caps(&imp.caps.lock().unwrap()).unwrap();
        let settings = imp.settings.lock().unwrap();

        AudioTestSrcTask {
            elem: elem.clone(),
            segment: gst::FormattedSegment::<gst::format::Time>::new(),
            need_initial_events: true,

            // TODO handle live changes for these settings
            volume: settings.volume,
            mute: settings.mute,
            freq: settings.freq as f64,
            is_live: settings.is_live,
            samples_per_buffer: settings.samples_per_buffer,
            buffer_duration: gst::ClockTime::SECOND
                .mul_div_floor(settings.samples_per_buffer as u64, info.rate() as u64)
                .unwrap(),

            sample_offset: 0,
            sample_stop: None,
            step: 0.0,
            accumulator: 0.0,

            info,

            buffer_count: 0,
            num_buffers: settings.num_buffers,
            #[cfg(feature = "tuning")]
            is_main_elem: settings.is_main_elem,
            #[cfg(feature = "tuning")]
            parked_duration_init: None,
            #[cfg(feature = "tuning")]
            log_start: Instant::now(),
        }
    }

    async fn negotiate(&mut self) -> Result<(), gst::ErrorMessage> {
        let imp = self.elem.imp();
        let pad = imp.src_pad.gst_pad();
        gst::info!(CAT, imp = imp, "Negotiating");

        let mut caps = pad.peer_query_caps(Some(&CAPS_FILTER));
        gst::debug!(CAT, obj = pad, "Peer returned {caps}");

        if caps.is_empty() {
            pad.mark_reconfigure();
            let err = gst::error_msg!(gst::CoreError::Pad, ["No common Caps"]);
            gst::error!(CAT, imp = imp, "{err}");
            return Err(err);
        }

        if caps.is_any() {
            gst::debug!(CAT, imp = imp, "Using our own Caps");
            caps = gst_audio::AudioCapsBuilder::new_interleaved()
                .format(gst_audio::AUDIO_FORMAT_S16)
                .channels(DEFAULT_CHANNELS as i32)
                .rate(DEFAULT_RATE as i32)
                .build();
        }

        self.set_caps(caps).await
    }

    async fn set_caps(&mut self, mut caps: gst::Caps) -> Result<(), gst::ErrorMessage> {
        use std::ops::Rem;

        let imp = self.elem.imp();

        caps.truncate();
        caps.intersect_with_mode(&CAPS_FILTER, gst::CapsIntersectMode::First);

        {
            let caps_mut = caps.make_mut();
            let s = caps_mut.structure_mut(0).ok_or_else(|| {
                gst::error_msg!(gst::CoreError::Negotiation, ["Invalid peer Caps structure"])
            })?;

            if let Ok(formats) = s.get::<gst::List>("format") {
                let Some(first) = formats.first() else {
                    return Err(gst::error_msg!(
                        gst::CoreError::Negotiation,
                        ["Empty format list in {caps}"]
                    ));
                };
                match first.get::<&glib::GStr>() {
                    Ok(first) => s.set("format", first),
                    Err(err) => {
                        return Err(gst::error_msg!(
                            gst::CoreError::Negotiation,
                            ["Unexpected format type in {caps}: {err}"]
                        ));
                    }
                }
            }

            s.fixate_field_nearest_int("rate", DEFAULT_RATE as i32);
            s.fixate_field_nearest_int("channels", DEFAULT_CHANNELS as i32);
        }

        let info = gst_audio::AudioInfo::from_caps(&caps).map_err(|_| {
            gst::error_msg!(
                gst::CoreError::Negotiation,
                ["Failed to build `AudioInfo` from {caps}"]
            )
        })?;

        gst::info!(CAT, imp = imp, "Configuring for {caps}");

        let old_rate = self.info.rate() as u64;
        self.info = info;

        {
            self.step = 2.0 * std::f64::consts::PI * self.freq / (self.info.rate() as f64);

            // Update sample offset and accumulator based on the previous values and the
            // sample rate change, if any
            let old_sample_offset = self.sample_offset;
            let sample_offset = old_sample_offset
                .mul_div_floor(self.info.rate() as u64, old_rate)
                .unwrap();

            let old_sample_stop = self.sample_stop;
            self.sample_stop = old_sample_stop
                .map(|v| v.mul_div_floor(self.info.rate() as u64, old_rate).unwrap());

            self.accumulator = (sample_offset as f64).rem(self.step);

            self.buffer_duration = gst::ClockTime::SECOND
                .mul_div_floor(self.samples_per_buffer as u64, self.info.rate() as u64)
                .unwrap();
        }

        imp.src_pad.push_event(gst::event::Caps::new(&caps)).await;
        *imp.caps.lock().unwrap() = caps;

        let _ = self
            .elem
            .post_message(gst::message::Latency::builder().src(&self.elem).build());

        Ok(())
    }
}

impl TaskImpl for AudioTestSrcTask {
    type Item = gst::Buffer;

    fn obj(&self) -> &impl IsA<glib::Object> {
        &self.elem
    }

    async fn start(&mut self) -> Result<(), gst::ErrorMessage> {
        gst::log!(CAT, obj = self.elem, "Starting Task");

        self.buffer_count = 0;

        #[cfg(feature = "tuning")]
        if self.is_main_elem {
            self.parked_duration_init = None;
        }

        Ok(())
    }

    async fn stop(&mut self) -> Result<(), gst::ErrorMessage> {
        gst::log!(CAT, obj = self.elem, "Stopping Task");

        self.need_initial_events = true;
        self.sample_offset = 0;
        self.sample_stop = None;
        self.accumulator = 0.0;

        Ok(())
    }

    async fn try_next(&mut self) -> Result<gst::Buffer, gst::FlowError> {
        if self.need_initial_events {
            gst::debug!(CAT, obj = self.elem, "Pushing initial events");

            let stream_id = format!("{:08x}{:08x}", rand::random::<u32>(), rand::random::<u32>());
            let stream_start_evt = gst::event::StreamStart::builder(&stream_id)
                .group_id(gst::GroupId::next())
                .build();
            self.elem.imp().src_pad.push_event(stream_start_evt).await;
        }

        if self.elem.imp().src_pad.gst_pad().check_reconfigure() {
            self.negotiate()
                .await
                .map_err(|_| gst::FlowError::NotNegotiated)?;
        }

        if self.need_initial_events {
            let segment_evt = gst::event::Segment::new(&self.segment);
            self.elem.imp().src_pad.push_event(segment_evt).await;

            self.need_initial_events = false;
        }

        let n_samples = if let Some(sample_stop) = self.sample_stop {
            if sample_stop <= self.sample_offset {
                gst::log!(CAT, obj = self.elem, "At EOS");
                return Err(gst::FlowError::Eos);
            }

            sample_stop - self.sample_offset
        } else {
            self.samples_per_buffer as u64
        };

        let Ok(mut buffer) = gst::Buffer::with_size(n_samples as usize * self.info.bpf() as usize)
        else {
            gst::error!(CAT, obj = self.elem, "Failed to create buffer");
            return Err(gst::FlowError::Flushing);
        };
        let buffer_mut = buffer.get_mut().unwrap();

        use gst_audio::AudioFormat::*;
        match self.info.format() {
            S16le => self.fill_buffer::<i16, L_ENDIAN, S16_MAX, S_OFFSET>(buffer_mut),
            S16be => self.fill_buffer::<i16, B_ENDIAN, S16_MAX, S_OFFSET>(buffer_mut),
            U16le => self.fill_buffer::<u16, L_ENDIAN, U16_MAX, U16_OFFSET>(buffer_mut),
            U16be => self.fill_buffer::<u16, B_ENDIAN, U16_MAX, U16_OFFSET>(buffer_mut),
            S32le => self.fill_buffer::<i32, L_ENDIAN, S32_MAX, S_OFFSET>(buffer_mut),
            S32be => self.fill_buffer::<i32, B_ENDIAN, S32_MAX, S_OFFSET>(buffer_mut),
            U32le => self.fill_buffer::<u32, L_ENDIAN, U32_MAX, U32_OFFSET>(buffer_mut),
            U32be => self.fill_buffer::<u32, B_ENDIAN, U32_MAX, U32_OFFSET>(buffer_mut),
            F32le => self.fill_buffer::<f32, L_ENDIAN, F_MAX, F_OFFSET>(buffer_mut),
            F32be => self.fill_buffer::<f32, B_ENDIAN, F_MAX, F_OFFSET>(buffer_mut),
            F64le => self.fill_buffer::<f64, L_ENDIAN, F_MAX, F_OFFSET>(buffer_mut),
            F64be => self.fill_buffer::<f64, B_ENDIAN, F_MAX, F_OFFSET>(buffer_mut),
            S8 => self.fill_buffer::<i8, L_ENDIAN, S8_MAX, S_OFFSET>(buffer_mut),
            U8 => self.fill_buffer::<u8, L_ENDIAN, U8_MAX, U8_OFFSET>(buffer_mut),
            _ => unreachable!("Not in caps"),
        }

        let pts = self
            .sample_offset
            .mul_div_floor(*gst::ClockTime::SECOND, self.info.rate() as u64)
            .map(gst::ClockTime::from_nseconds)
            .unwrap();
        let next_pts = (self.sample_offset + n_samples)
            .mul_div_floor(*gst::ClockTime::SECOND, self.info.rate() as u64)
            .map(gst::ClockTime::from_nseconds)
            .unwrap();
        buffer_mut.set_pts(pts);
        buffer_mut.set_duration(next_pts - pts);

        self.sample_offset += n_samples;

        if self.is_live {
            let running_time = self
                .segment
                .to_running_time(buffer.pts().opt_add(buffer.duration()));

            let Some(cur_rt) = self.elem.current_running_time() else {
                // Let the scheduler share time with other tasks
                runtime::executor::yield_now().await;
                return Ok(buffer);
            };

            let Ok(Some(delay)) = running_time.opt_checked_sub(cur_rt) else {
                // Let the scheduler share time with other tasks
                runtime::executor::yield_now().await;
                return Ok(buffer);
            };

            // Wait for all samples to fit in last time slice
            // We don't use `delay_for` here because buffers could be pushed
            // 1/2 'context-wait' earlier than the deadline, which would not
            // reflect the behaviour of actual source elements: once the buffer
            // is captured, there can be a `context-wait` delay before the
            // source element gets scheduled.
            timer::delay_for_at_least(delay.into()).await;
        } else {
            // Let the scheduler share time with other tasks
            runtime::executor::yield_now().await;
        }

        Ok(buffer)
    }

    async fn handle_item(&mut self, buffer: gst::Buffer) -> Result<(), gst::FlowError> {
        let imp = self.elem.imp();

        gst::debug!(CAT, imp = imp, "Pushing {buffer:?}");
        imp.src_pad.push(buffer).await?;
        gst::log!(CAT, imp = imp, "Successfully pushed buffer");

        self.buffer_count += 1;

        #[cfg(feature = "tuning")]
        if self.is_main_elem {
            if let Some(parked_duration_init) = self.parked_duration_init {
                if self.buffer_count % LOG_BUFFER_INTERVAL == 0 {
                    let parked_duration = runtime::Context::current().unwrap().parked_duration()
                        - parked_duration_init;

                    gst::info!(
                        CAT,
                        "Parked: {:5.2?}%",
                        parked_duration.as_nanos() as f32 * 100.0
                            / self.log_start.elapsed().as_nanos() as f32,
                    );
                }
            } else if self.buffer_count == RAMPUP_BUFFER_COUNT {
                self.parked_duration_init =
                    Some(runtime::Context::current().unwrap().parked_duration());
                self.log_start = Instant::now();

                gst::info!(CAT, "Ramp up complete");
            }
        }

        if self.num_buffers.opt_eq(self.buffer_count) == Some(true) {
            return Err(gst::FlowError::Eos);
        }

        Ok(())
    }

    async fn handle_loop_error(&mut self, err: gst::FlowError) -> task::Trigger {
        match err {
            gst::FlowError::Flushing => {
                gst::debug!(CAT, obj = self.elem, "Flushing");

                task::Trigger::FlushStart
            }
            gst::FlowError::Eos => {
                gst::debug!(CAT, obj = self.elem, "EOS");
                self.elem
                    .imp()
                    .src_pad
                    .push_event(gst::event::Eos::new())
                    .await;

                task::Trigger::Stop
            }
            err => {
                gst::error!(CAT, obj = self.elem, "Got error {err}");
                gst::element_error!(
                    &self.elem,
                    gst::StreamError::Failed,
                    ("Internal data stream error"),
                    ["streaming stopped, reason {}", err]
                );

                task::Trigger::Error
            }
        }
    }
}

impl AudioTestSrcTask {
    // Can't use f64 as a const parameter as of rustc 1.90.0
    fn fill_buffer<T, const IS_LE: bool, const MAX: u64, const OFFSET: u64>(
        &mut self,
        buffer_mut: &mut gst::BufferRef,
    ) where
        T: FromByteSlice + FromPrimitive + Zero + ToBytes + Copy,
    {
        let mut mapped = buffer_mut.map_writable().unwrap();
        let data = mapped.as_mut_slice();

        if self.mute {
            let value = T::from_u64(OFFSET).unwrap();
            let value = if IS_LE {
                value.to_le_bytes()
            } else {
                value.to_be_bytes()
            };

            for sample in data.chunks_exact_mut(mem::size_of::<T>()) {
                sample.copy_from_slice(value.as_ref());
            }
        } else {
            for chunk in data.chunks_exact_mut(self.info.channels() as usize * mem::size_of::<T>())
            {
                let value = T::from_f64(
                    MAX as f64 * self.volume * f64::sin(self.accumulator) + OFFSET as f64,
                )
                .unwrap();
                let value = if IS_LE {
                    value.to_le_bytes()
                } else {
                    value.to_be_bytes()
                };

                for sample in chunk.chunks_exact_mut(mem::size_of::<T>()) {
                    sample.copy_from_slice(value.as_ref());
                }

                self.accumulator += self.step;
                if self.accumulator >= 2.0 * std::f64::consts::PI {
                    self.accumulator -= 2.0 * std::f64::consts::PI;
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct AudioTestSrc {
    src_pad: PadSrc,
    task: Task,
    caps: Mutex<gst::Caps>,
    settings: Mutex<Settings>,
}

impl AudioTestSrc {
    fn prepare(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp = self, "Preparing");

        let settings = self.settings.lock().unwrap();
        let context =
            runtime::Context::acquire(&settings.context, settings.context_wait).map_err(|err| {
                gst::error_msg!(
                    gst::ResourceError::OpenRead,
                    ["Failed to acquire Context: {}", err]
                )
            })?;
        drop(settings);

        self.task
            .prepare(AudioTestSrcTask::new(self.obj().clone()), context)
            .block_on_or_add_subtask_then(self.obj(), |elem, res| {
                if res.is_ok() {
                    gst::debug!(CAT, obj = elem, "Prepared");
                }
            })
    }

    fn unprepare(&self) {
        gst::debug!(CAT, imp = self, "Unpreparing");
        let _ = self
            .task
            .unprepare()
            .block_on_or_add_subtask_then(self.obj(), |elem, _| {
                gst::debug!(CAT, obj = elem, "Unprepared");
            });
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp = self, "Stopping");
        self.task
            .stop()
            .block_on_or_add_subtask_then(self.obj(), |elem, res| {
                if res.is_ok() {
                    gst::debug!(CAT, obj = elem, "Stopped");
                }
            })
    }

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp = self, "Starting");
        self.task
            .start()
            .block_on_or_add_subtask_then(self.obj(), |elem, res| {
                if res.is_ok() {
                    gst::debug!(CAT, obj = elem, "Started");
                }
            })
    }
}

#[glib::object_subclass]
impl ObjectSubclass for AudioTestSrc {
    const NAME: &'static str = "TsAudioTestSrc";
    type Type = super::AudioTestSrc;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        Self {
            src_pad: PadSrc::new(
                gst::Pad::from_template(&klass.pad_template("src").unwrap()),
                AudioTestSrcPadHandler,
            ),
            task: Task::default(),
            caps: Mutex::new(DEFAULT_CAPS.clone()),
            settings: Default::default(),
        }
    }
}

impl ObjectImpl for AudioTestSrc {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecString::builder("context")
                    .nick("Context")
                    .blurb("Context name to share threads with")
                    .default_value(Some(DEFAULT_CONTEXT))
                    .build(),
                glib::ParamSpecUInt::builder("context-wait")
                    .nick("Context Wait")
                    .blurb("Throttle poll loop to run at most once every this many ms")
                    .maximum(1000)
                    .default_value(DEFAULT_CONTEXT_WAIT.as_millis() as u32)
                    .build(),
                glib::ParamSpecUInt::builder("samples-per-buffer")
                    .nick("Samples Per Buffer")
                    .blurb("Number of samples per output buffer")
                    .minimum(1)
                    .default_value(DEFAULT_SAMPLES_PER_BUFFER)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder("freq")
                    .nick("Frequency")
                    .blurb("Frequency")
                    .minimum(1)
                    .default_value(DEFAULT_FREQ)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecDouble::builder("volume")
                    .nick("Volume")
                    .blurb("Output volume")
                    .maximum(10.0)
                    .default_value(DEFAULT_VOLUME)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecBoolean::builder("mute")
                    .nick("Mute")
                    .blurb("Mute")
                    .default_value(DEFAULT_MUTE)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecBoolean::builder("is-live")
                    .nick("Is Live")
                    .blurb("(Pseudo) live output")
                    .default_value(DEFAULT_IS_LIVE)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecInt::builder("num-buffers")
                    .nick("Num Buffers")
                    .blurb("Number of buffers to output before sending EOS (-1 = unlimited)")
                    .minimum(-1i32)
                    .default_value(DEFAULT_NUM_BUFFERS)
                    .build(),
                #[cfg(feature = "tuning")]
                glib::ParamSpecBoolean::builder("main-elem")
                    .nick("Main Element")
                    .blurb("Declare this element as the main one")
                    .write_only()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().unwrap();
        match pspec.name() {
            "context" => {
                settings.context = value
                    .get::<Option<String>>()
                    .unwrap()
                    .unwrap_or_else(|| DEFAULT_CONTEXT.into());
            }
            "context-wait" => {
                settings.context_wait = Duration::from_millis(value.get::<u32>().unwrap().into());
            }
            "samples-per-buffer" => {
                settings.samples_per_buffer = value.get().expect("type checked upstream");
                drop(settings);

                let _ = self
                    .obj()
                    .post_message(gst::message::Latency::builder().src(&*self.obj()).build());
            }
            "freq" => {
                settings.freq = value.get().expect("type checked upstream");
            }
            "volume" => {
                settings.volume = value.get().expect("type checked upstream");
            }
            "mute" => {
                settings.mute = value.get().expect("type checked upstream");
            }
            "is-live" => {
                settings.is_live = value.get().expect("type checked upstream");
            }
            "num-buffers" => {
                let value = value.get::<i32>().unwrap();
                settings.num_buffers = if value > 0 { Some(value as u32) } else { None };
            }
            #[cfg(feature = "tuning")]
            "main-elem" => {
                settings.is_main_elem = value.get::<bool>().unwrap();
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().unwrap();
        match pspec.name() {
            "context" => settings.context.to_value(),
            "context-wait" => (settings.context_wait.as_millis() as u32).to_value(),
            "samples-per-buffer" => settings.samples_per_buffer.to_value(),
            "freq" => settings.freq.to_value(),
            "volume" => settings.volume.to_value(),
            "mute" => settings.mute.to_value(),
            "is-live" => settings.is_live.to_value(),
            "num-buffers" => settings
                .num_buffers
                .and_then(|val| val.try_into().ok())
                .unwrap_or(-1i32)
                .to_value(),
            #[cfg(feature = "tuning")]
            "main-elem" => settings.is_main_elem.to_value(),
            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(self.src_pad.gst_pad()).unwrap();
        obj.set_element_flags(gst::ElementFlags::SOURCE);
    }
}

impl GstObjectImpl for AudioTestSrc {}

impl ElementImpl for AudioTestSrc {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Thread-sharing audio test source",
                "Source/Test",
                "Thread-sharing audio test source",
                "François Laignel <fengalin@free.fr>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &CAPS_FILTER,
            )
            .unwrap();

            vec![src_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst::trace!(CAT, imp = self, "Changing state {transition:?}");

        match transition {
            gst::StateChange::NullToReady => {
                self.prepare().map_err(|err| {
                    self.post_error_message(err);
                    gst::StateChangeError
                })?;
            }
            gst::StateChange::PausedToReady => {
                self.stop().map_err(|_| gst::StateChangeError)?;
            }
            gst::StateChange::ReadyToNull => {
                self.unprepare();
            }
            _ => (),
        }

        let mut success = self.parent_change_state(transition)?;

        match transition {
            gst::StateChange::ReadyToPaused => {
                success = gst::StateChangeSuccess::NoPreroll;
            }
            gst::StateChange::PausedToPlaying => {
                self.start().map_err(|_| gst::StateChangeError)?;
            }
            gst::StateChange::PlayingToPaused => {
                success = gst::StateChangeSuccess::NoPreroll;
            }
            _ => (),
        }

        Ok(success)
    }
}
