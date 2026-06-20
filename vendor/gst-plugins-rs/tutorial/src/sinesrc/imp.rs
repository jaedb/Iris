// Copyright (C) 2018 Sebastian Dröge <sebastian@centricular.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_base::prelude::*;
use gst_base::subclass::base_src::CreateSuccess;
use gst_base::subclass::prelude::*;

use byte_slice_cast::*;

use std::ops::Rem;
use std::sync::Mutex;

use num_traits::cast::NumCast;
use num_traits::float::Float;

use std::sync::LazyLock;

// This module contains the private implementation details of our element

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rssinesrc",
        gst::DebugColorFlags::empty(),
        Some("Rust Sine Wave Source"),
    )
});

// Default values of properties
const DEFAULT_SAMPLES_PER_BUFFER: u32 = 1024;
const DEFAULT_FREQ: u32 = 440;
const DEFAULT_VOLUME: f64 = 0.8;
const DEFAULT_MUTE: bool = false;
const DEFAULT_IS_LIVE: bool = false;

// Property value storage
#[derive(Debug, Clone, Copy)]
struct Settings {
    samples_per_buffer: u32,
    freq: u32,
    volume: f64,
    mute: bool,
    is_live: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            samples_per_buffer: DEFAULT_SAMPLES_PER_BUFFER,
            freq: DEFAULT_FREQ,
            volume: DEFAULT_VOLUME,
            mute: DEFAULT_MUTE,
            is_live: DEFAULT_IS_LIVE,
        }
    }
}

// Stream-specific state, i.e. audio format configuration
// and sample offset
struct State {
    info: Option<gst_audio::AudioInfo>,
    sample_offset: u64,
    sample_stop: Option<u64>,
    accumulator: f64,
}

impl Default for State {
    fn default() -> State {
        State {
            info: None,
            sample_offset: 0,
            sample_stop: None,
            accumulator: 0.0,
        }
    }
}

struct ClockWait {
    clock_id: Option<gst::SingleShotClockId>,
    flushing: bool,
}

impl Default for ClockWait {
    fn default() -> ClockWait {
        ClockWait {
            clock_id: None,
            flushing: true,
        }
    }
}

// Struct containing all the element data
#[derive(Default)]
pub struct SineSrc {
    settings: Mutex<Settings>,
    state: Mutex<State>,
    clock_wait: Mutex<ClockWait>,
}

impl SineSrc {
    fn process<F: Float + FromByteSlice>(
        data: &mut [u8],
        accumulator_ref: &mut f64,
        freq: u32,
        rate: u32,
        channels: u32,
        vol: f64,
    ) {
        use std::f64::consts::PI;

        // Reinterpret our byte-slice as a slice containing elements of the type
        // we're interested in. GStreamer requires for raw audio that the alignment
        // of memory is correct, so this will never ever fail unless there is an
        // actual bug elsewhere.
        let data = data.as_mut_slice_of::<F>().unwrap();

        // Convert all our parameters to the target type for calculations
        let vol: F = NumCast::from(vol).unwrap();
        let freq = freq as f64;
        let rate = rate as f64;
        let two_pi = 2.0 * PI;

        // We're carrying a accumulator with up to 2pi around instead of working
        // on the sample offset. High sample offsets cause too much inaccuracy when
        // converted to floating point numbers and then iterated over in 1-steps
        let mut accumulator = *accumulator_ref;
        let step = two_pi * freq / rate;

        for chunk in data.chunks_exact_mut(channels as usize) {
            let value = vol * F::sin(NumCast::from(accumulator).unwrap());
            for sample in chunk {
                *sample = value;
            }

            accumulator += step;
            if accumulator >= two_pi {
                accumulator -= two_pi;
            }
        }

        *accumulator_ref = accumulator;
    }
}

// This trait registers our type with the GObject object system and
// provides the entry points for creating a new instance and setting
// up the class data
#[glib::object_subclass]
impl ObjectSubclass for SineSrc {
    const NAME: &'static str = "GstRsSineSrc";
    type Type = super::SineSrc;
    type ParentType = gst_base::PushSrc;
}

// Implementation of glib::Object virtual methods
impl ObjectImpl for SineSrc {
    // Metadata for the properties
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
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
            ]
        });

        PROPERTIES.as_ref()
    }

    // Called right after construction of a new instance
    fn constructed(&self) {
        // Call the parent class' ::constructed() implementation first
        self.parent_constructed();

        let obj = self.obj();
        // Initialize live-ness and notify the base class that
        // we'd like to operate in Time format
        obj.set_live(DEFAULT_IS_LIVE);
        obj.set_format(gst::Format::Time);
    }

    // Called whenever a value of a property is changed. It can be called
    // at any time from any thread.
    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "samples-per-buffer" => {
                let mut settings = self.settings.lock().unwrap();
                let samples_per_buffer = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing samples-per-buffer from {} to {}",
                    settings.samples_per_buffer,
                    samples_per_buffer
                );
                settings.samples_per_buffer = samples_per_buffer;
                drop(settings);

                let _ = self
                    .obj()
                    .post_message(gst::message::Latency::builder().src(&*self.obj()).build());
            }
            "freq" => {
                let mut settings = self.settings.lock().unwrap();
                let freq = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing freq from {} to {}",
                    settings.freq,
                    freq
                );
                settings.freq = freq;
            }
            "volume" => {
                let mut settings = self.settings.lock().unwrap();
                let volume = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing volume from {} to {}",
                    settings.volume,
                    volume
                );
                settings.volume = volume;
            }
            "mute" => {
                let mut settings = self.settings.lock().unwrap();
                let mute = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing mute from {} to {}",
                    settings.mute,
                    mute
                );
                settings.mute = mute;
            }
            "is-live" => {
                let mut settings = self.settings.lock().unwrap();
                let is_live = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing is-live from {} to {}",
                    settings.is_live,
                    is_live
                );
                settings.is_live = is_live;
            }
            _ => unimplemented!(),
        }
    }

    // Called whenever a value of a property is read. It can be called
    // at any time from any thread.
    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "samples-per-buffer" => {
                let settings = self.settings.lock().unwrap();
                settings.samples_per_buffer.to_value()
            }
            "freq" => {
                let settings = self.settings.lock().unwrap();
                settings.freq.to_value()
            }
            "volume" => {
                let settings = self.settings.lock().unwrap();
                settings.volume.to_value()
            }
            "mute" => {
                let settings = self.settings.lock().unwrap();
                settings.mute.to_value()
            }
            "is-live" => {
                let settings = self.settings.lock().unwrap();
                settings.is_live.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for SineSrc {}

// Implementation of gst::Element virtual methods
impl ElementImpl for SineSrc {
    // Set the element specific metadata. This information is what
    // is visible from gst-inspect-1.0 and can also be programmatically
    // retrieved from the gst::Registry after initial registration
    // without having to load the plugin in memory.
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Sine Wave Source",
                "Source/Audio",
                "Creates a sine wave",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    // Create and add pad templates for our sink and source pad. These
    // are later used for actually creating the pads and beforehand
    // already provide information to GStreamer about all possible
    // pads that could exist for this type.
    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            // On the src pad, we can produce F32/F64 with any sample rate
            // and any number of channels
            let caps = gst_audio::AudioCapsBuilder::new_interleaved()
                .format_list([gst_audio::AUDIO_FORMAT_F32, gst_audio::AUDIO_FORMAT_F64])
                .build();
            // The src pad template must be named "src" for basesrc
            // and specific a pad that is always there
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            vec![src_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    // Called whenever the state of the element should be changed. This allows for
    // starting up the element, allocating/deallocating resources or shutting down
    // the element again.
    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        // Configure live'ness once here just before starting the source
        if let gst::StateChange::ReadyToPaused = transition {
            self.obj().set_live(self.settings.lock().unwrap().is_live);
        }

        // Call the parent class' implementation of ::change_state()
        self.parent_change_state(transition)
    }
}

// Implementation of gst_base::BaseSrc virtual methods
impl BaseSrcImpl for SineSrc {
    // Called whenever the input/output caps are changing, i.e. in the very beginning before data
    // flow happens and whenever the situation in the pipeline is changing. All buffers after this
    // call have the caps given here.
    //
    // We simply remember the resulting AudioInfo from the caps to be able to use this for knowing
    // the sample rate, etc. when creating buffers
    fn set_caps(&self, caps: &gst::Caps) -> Result<(), gst::LoggableError> {
        use std::f64::consts::PI;

        let info = gst_audio::AudioInfo::from_caps(caps).map_err(|_| {
            gst::loggable_error!(CAT, "Failed to build `AudioInfo` from caps {}", caps)
        })?;

        gst::debug!(CAT, imp = self, "Configuring for caps {}", caps);

        self.obj()
            .set_blocksize(info.bpf() * (self.settings.lock().unwrap()).samples_per_buffer);

        let settings = *self.settings.lock().unwrap();
        let mut state = self.state.lock().unwrap();

        // If we have no caps yet, any old sample_offset and sample_stop will be
        // in nanoseconds
        let old_rate = match state.info {
            Some(ref info) => info.rate() as u64,
            None => *gst::ClockTime::SECOND,
        };

        // Update sample offset and accumulator based on the previous values and the
        // sample rate change, if any
        let old_sample_offset = state.sample_offset;
        let sample_offset = old_sample_offset
            .mul_div_floor(info.rate() as u64, old_rate)
            .unwrap();

        let old_sample_stop = state.sample_stop;
        let sample_stop =
            old_sample_stop.map(|v| v.mul_div_floor(info.rate() as u64, old_rate).unwrap());

        let accumulator =
            (sample_offset as f64).rem(2.0 * PI * (settings.freq as f64) / (info.rate() as f64));

        *state = State {
            info: Some(info),
            sample_offset,
            sample_stop,
            accumulator,
        };

        drop(state);

        let _ = self
            .obj()
            .post_message(gst::message::Latency::builder().src(&*self.obj()).build());

        Ok(())
    }

    // Called when starting, so we can initialize all stream-related state to its defaults
    fn start(&self) -> Result<(), gst::ErrorMessage> {
        // Reset state
        *self.state.lock().unwrap() = Default::default();
        self.unlock_stop()?;

        gst::info!(CAT, imp = self, "Started");

        Ok(())
    }

    // Called when shutting down the element so we can release all stream-related state
    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        // Reset state
        *self.state.lock().unwrap() = Default::default();
        self.unlock()?;

        gst::info!(CAT, imp = self, "Stopped");

        Ok(())
    }

    fn query(&self, query: &mut gst::QueryRef) -> bool {
        use gst::QueryViewMut;

        match query.view_mut() {
            // In Live mode we will have a latency equal to the number of samples in each buffer.
            // We can't output samples before they were produced, and the last sample of a buffer
            // is produced that much after the beginning, leading to this latency calculation
            QueryViewMut::Latency(q) => {
                let settings = *self.settings.lock().unwrap();
                let state = self.state.lock().unwrap();

                if let Some(ref info) = state.info {
                    let latency = gst::ClockTime::SECOND
                        .mul_div_floor(settings.samples_per_buffer as u64, info.rate() as u64)
                        .unwrap();
                    gst::debug!(CAT, imp = self, "Returning latency {}", latency);
                    q.set(settings.is_live, latency, gst::ClockTime::NONE);
                    true
                } else {
                    false
                }
            }
            _ => BaseSrcImplExt::parent_query(self, query),
        }
    }

    fn fixate(&self, mut caps: gst::Caps) -> gst::Caps {
        // Fixate the caps. BaseSrc will do some fixation for us, but
        // as we allow any rate between 1 and MAX it would fixate to 1. 1Hz
        // is generally not a useful sample rate.
        //
        // We fixate to the closest integer value to 48kHz that is possible
        // here, and for good measure also decide that the closest value to 1
        // channel is good.
        caps.truncate();
        {
            let caps = caps.make_mut();
            let s = caps.structure_mut(0).unwrap();
            s.fixate_field_nearest_int("rate", 48_000);
            s.fixate_field_nearest_int("channels", 1);
        }

        // Let BaseSrc fixate anything else for us. We could've alternatively have
        // called caps.fixate() here
        self.parent_fixate(caps)
    }

    fn is_seekable(&self) -> bool {
        true
    }

    fn do_seek(&self, segment: &mut gst::Segment) -> bool {
        // Handle seeking here. For Time and Default (sample offset) seeks we can
        // do something and have to update our sample offset and accumulator accordingly.
        //
        // Also we should remember the stop time (so we can stop at that point), and if
        // reverse playback is requested. These values will all be used during buffer creation
        // and for calculating the timestamps, etc.

        if segment.rate() < 0.0 {
            gst::error!(CAT, imp = self, "Reverse playback not supported");
            return false;
        }

        let settings = *self.settings.lock().unwrap();
        let mut state = self.state.lock().unwrap();

        // We store sample_offset and sample_stop in nanoseconds if we
        // don't know any sample rate yet. It will be converted correctly
        // once a sample rate is known.
        let rate = match state.info {
            None => *gst::ClockTime::SECOND,
            Some(ref info) => info.rate() as u64,
        };

        if let Some(segment) = segment.downcast_ref::<gst::format::Time>() {
            use std::f64::consts::PI;

            let sample_offset = segment
                .start()
                .unwrap()
                .nseconds()
                .mul_div_floor(rate, *gst::ClockTime::SECOND)
                .unwrap();

            let sample_stop = segment
                .stop()
                .and_then(|v| v.nseconds().mul_div_floor(rate, *gst::ClockTime::SECOND));

            let accumulator =
                (sample_offset as f64).rem(2.0 * PI * (settings.freq as f64) / (rate as f64));

            gst::debug!(
                CAT,
                imp = self,
                "Seeked to {}-{:?} (accum: {}) for segment {:?}",
                sample_offset,
                sample_stop,
                accumulator,
                segment
            );

            *state = State {
                info: state.info.clone(),
                sample_offset,
                sample_stop,
                accumulator,
            };

            true
        } else if let Some(segment) = segment.downcast_ref::<gst::format::Default>() {
            use std::f64::consts::PI;

            if state.info.is_none() {
                gst::error!(
                    CAT,
                    imp = self,
                    "Can only seek in Default format if sample rate is known"
                );
                return false;
            }

            let sample_offset = *segment.start().unwrap();
            let sample_stop = segment.stop().map(|stop| *stop);

            let accumulator =
                (sample_offset as f64).rem(2.0 * PI * (settings.freq as f64) / (rate as f64));

            gst::debug!(
                CAT,
                imp = self,
                "Seeked to {}-{:?} (accum: {}) for segment {:?}",
                sample_offset,
                sample_stop,
                accumulator,
                segment
            );

            *state = State {
                info: state.info.clone(),
                sample_offset,
                sample_stop,
                accumulator,
            };

            true
        } else {
            gst::error!(
                CAT,
                imp = self,
                "Can't seek in format {:?}",
                segment.format()
            );

            false
        }
    }

    fn unlock(&self) -> Result<(), gst::ErrorMessage> {
        // This should unblock the create() function ASAP, so we
        // just unschedule the clock it here, if any.
        gst::debug!(CAT, imp = self, "Unlocking");
        let mut clock_wait = self.clock_wait.lock().unwrap();
        if let Some(clock_id) = clock_wait.clock_id.take() {
            clock_id.unschedule();
        }
        clock_wait.flushing = true;

        Ok(())
    }

    fn unlock_stop(&self) -> Result<(), gst::ErrorMessage> {
        // This signals that unlocking is done, so we can reset
        // all values again.
        gst::debug!(CAT, imp = self, "Unlock stop");
        let mut clock_wait = self.clock_wait.lock().unwrap();
        clock_wait.flushing = false;

        Ok(())
    }
}

impl PushSrcImpl for SineSrc {
    // Creates the audio buffers
    fn create(
        &self,
        _buffer: Option<&mut gst::BufferRef>,
    ) -> Result<CreateSuccess, gst::FlowError> {
        // Keep a local copy of the values of all our properties at this very moment. This
        // ensures that the mutex is never locked for long and the application wouldn't
        // have to block until this function returns when getting/setting property values
        let settings = *self.settings.lock().unwrap();

        // Get a locked reference to our state, i.e. the input and output AudioInfo
        let mut state = self.state.lock().unwrap();
        let info = match state.info {
            None => {
                gst::element_imp_error!(self, gst::CoreError::Negotiation, ["Have no caps yet"]);
                return Err(gst::FlowError::NotNegotiated);
            }
            Some(ref info) => info.clone(),
        };

        // If a stop position is set (from a seek), only produce samples up to that
        // point but at most samples_per_buffer samples per buffer
        let n_samples = if let Some(sample_stop) = state.sample_stop {
            if sample_stop <= state.sample_offset {
                gst::log!(CAT, imp = self, "At EOS");
                return Err(gst::FlowError::Eos);
            }

            sample_stop - state.sample_offset
        } else {
            settings.samples_per_buffer as u64
        };

        // Allocate a new buffer of the required size, update the metadata with the
        // current timestamp and duration and then fill it according to the current
        // caps
        let mut buffer =
            gst::Buffer::with_size((n_samples as usize) * (info.bpf() as usize)).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();

            // Calculate the current timestamp (PTS) and the next one,
            // and calculate the duration from the difference instead of
            // simply the number of samples to prevent rounding errors
            let pts = state
                .sample_offset
                .mul_div_floor(*gst::ClockTime::SECOND, info.rate() as u64)
                .map(gst::ClockTime::from_nseconds)
                .unwrap();
            let next_pts = (state.sample_offset + n_samples)
                .mul_div_floor(*gst::ClockTime::SECOND, info.rate() as u64)
                .map(gst::ClockTime::from_nseconds)
                .unwrap();
            buffer.set_pts(pts);
            buffer.set_duration(next_pts - pts);

            // Map the buffer writable and create the actual samples
            let mut map = buffer.map_writable().unwrap();
            let data = map.as_mut_slice();

            if info.format() == gst_audio::AUDIO_FORMAT_F32 {
                Self::process::<f32>(
                    data,
                    &mut state.accumulator,
                    settings.freq,
                    info.rate(),
                    info.channels(),
                    settings.volume,
                );
            } else {
                Self::process::<f64>(
                    data,
                    &mut state.accumulator,
                    settings.freq,
                    info.rate(),
                    info.channels(),
                    settings.volume,
                );
            }
        }
        state.sample_offset += n_samples;
        drop(state);

        // If we're live, we are waiting until the time of the last sample in our buffer has
        // arrived. This is the very reason why we have to report that much latency.
        // A real live-source would of course only allow us to have the data available after
        // that latency, e.g. when capturing from a microphone, and no waiting from our side
        // would be necessary..
        //
        // Waiting happens based on the pipeline clock, which means that a real live source
        // with its own clock would require various translations between the two clocks.
        // This is out of scope for the tutorial though.
        if self.obj().is_live() {
            let (clock, base_time) = match Option::zip(self.obj().clock(), self.obj().base_time()) {
                None => return Ok(CreateSuccess::NewBuffer(buffer)),
                Some(res) => res,
            };

            let segment = self
                .obj()
                .segment()
                .downcast::<gst::format::Time>()
                .unwrap();
            let running_time = segment.to_running_time(buffer.pts().opt_add(buffer.duration()));

            // The last sample's clock time is the base time of the element plus the
            // running time of the last sample
            let wait_until = match running_time.opt_add(base_time) {
                Some(wait_until) => wait_until,
                None => return Ok(CreateSuccess::NewBuffer(buffer)),
            };

            // Store the clock ID in our struct unless we're flushing anyway.
            // This allows to asynchronously cancel the waiting from unlock()
            // so that we immediately stop waiting on e.g. shutdown.
            let mut clock_wait = self.clock_wait.lock().unwrap();
            if clock_wait.flushing {
                gst::debug!(CAT, imp = self, "Flushing");
                return Err(gst::FlowError::Flushing);
            }

            let id = clock.new_single_shot_id(wait_until);
            clock_wait.clock_id = Some(id.clone());
            drop(clock_wait);

            gst::log!(
                CAT,
                imp = self,
                "Waiting until {}, now {}",
                wait_until,
                clock.time(),
            );
            let (res, jitter) = id.wait();
            gst::log!(CAT, imp = self, "Waited res {:?} jitter {}", res, jitter);
            self.clock_wait.lock().unwrap().clock_id.take();

            // If the clock ID was unscheduled, unlock() was called
            // and we should return Flushing immediately.
            if res == Err(gst::ClockError::Unscheduled) {
                gst::debug!(CAT, imp = self, "Flushing");
                return Err(gst::FlowError::Flushing);
            }
        }

        gst::debug!(CAT, imp = self, "Produced buffer {:?}", buffer);

        Ok(CreateSuccess::NewBuffer(buffer))
    }
}
