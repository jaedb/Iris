// Copyright (C) 2017,2018 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_audio::subclass::prelude::*;

use std::cmp;
use std::sync::Mutex;

use byte_slice_cast::*;

use num_traits::cast::{FromPrimitive, ToPrimitive};
use num_traits::float::Float;

use std::sync::LazyLock;
static _CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rsaudioecho",
        gst::DebugColorFlags::empty(),
        Some("Rust Audio Echo Filter"),
    )
});

use super::ring_buffer::RingBuffer;

const DEFAULT_MAX_DELAY: gst::ClockTime = gst::ClockTime::SECOND;
const DEFAULT_DELAY: gst::ClockTime = gst::ClockTime::from_seconds(500);
const DEFAULT_INTENSITY: f64 = 0.5;
const DEFAULT_FEEDBACK: f64 = 0.0;

#[derive(Debug, Clone, Copy)]
struct Settings {
    pub max_delay: gst::ClockTime,
    pub delay: gst::ClockTime,
    pub intensity: f64,
    pub feedback: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            max_delay: DEFAULT_MAX_DELAY,
            delay: DEFAULT_DELAY,
            intensity: DEFAULT_INTENSITY,
            feedback: DEFAULT_FEEDBACK,
        }
    }
}

struct State {
    info: gst_audio::AudioInfo,
    buffer: RingBuffer,
}

#[derive(Default)]
pub struct AudioEcho {
    settings: Mutex<Settings>,
    state: Mutex<Option<State>>,
}

impl AudioEcho {
    fn process<F: Float + ToPrimitive + FromPrimitive>(
        data: &mut [F],
        state: &mut State,
        settings: &Settings,
    ) {
        let delay_frames = (settings.delay
            * (state.info.channels() as u64)
            * (state.info.rate() as u64))
            .seconds() as usize;

        for (i, (o, e)) in data.iter_mut().zip(state.buffer.iter(delay_frames)) {
            let inp = (*i).to_f64().unwrap();
            let out = inp + settings.intensity * e;
            *o = inp + settings.feedback * e;
            *i = FromPrimitive::from_f64(out).unwrap();
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for AudioEcho {
    const NAME: &'static str = "GstRsAudioEcho";
    type Type = super::AudioEcho;
    type ParentType = gst_audio::AudioFilter;
}

impl ObjectImpl for AudioEcho {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecUInt64::builder("max-delay")
                    .nick("Maximum Delay")
                    .blurb("Maximum delay of the echo in nanoseconds (can't be changed in PLAYING or PAUSED state)")
                    .maximum(u64::MAX - 1)
                    .default_value(DEFAULT_MAX_DELAY.nseconds())
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt64::builder("delay")
                    .nick("Delay")
                    .blurb("Delay of the echo in nanoseconds")
                    .maximum(u64::MAX - 1)
                    .default_value(DEFAULT_DELAY.nseconds())
                    .mutable_ready()
                    .build(),
                glib::ParamSpecDouble::builder("intensity")
                    .nick("Intensity")
                    .blurb("Intensity of the echo")
                    .minimum(0.0)
                    .maximum(1.0)
                    .default_value(DEFAULT_INTENSITY)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecDouble::builder("feedback")
                    .nick("Feedback")
                    .blurb("Amount of feedback")
                    .minimum(0.0)
                    .maximum(1.0)
                    .default_value(DEFAULT_FEEDBACK)
                    .mutable_ready()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "max-delay" => {
                let mut settings = self.settings.lock().unwrap();
                if self.state.lock().unwrap().is_none() {
                    settings.max_delay = value.get::<u64>().unwrap().nseconds();
                }
            }
            "delay" => {
                let mut settings = self.settings.lock().unwrap();
                settings.delay = value.get::<u64>().unwrap().nseconds();
            }
            "intensity" => {
                let mut settings = self.settings.lock().unwrap();
                settings.intensity = value.get().expect("type checked upstream");
            }
            "feedback" => {
                let mut settings = self.settings.lock().unwrap();
                settings.feedback = value.get().expect("type checked upstream");
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "max-delay" => {
                let settings = self.settings.lock().unwrap();
                settings.max_delay.to_value()
            }
            "delay" => {
                let settings = self.settings.lock().unwrap();
                settings.delay.to_value()
            }
            "intensity" => {
                let settings = self.settings.lock().unwrap();
                settings.intensity.to_value()
            }
            "feedback" => {
                let settings = self.settings.lock().unwrap();
                settings.feedback.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for AudioEcho {}

impl ElementImpl for AudioEcho {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Audio echo",
                "Filter/Effect/Audio",
                "Adds an echo or reverb effect to an audio stream",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }
}

impl BaseTransformImpl for AudioEcho {
    const MODE: gst_base::subclass::BaseTransformMode =
        gst_base::subclass::BaseTransformMode::AlwaysInPlace;
    const PASSTHROUGH_ON_SAME_CAPS: bool = false;
    const TRANSFORM_IP_ON_PASSTHROUGH: bool = false;

    fn transform_ip(&self, buf: &mut gst::BufferRef) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut settings = *self.settings.lock().unwrap();
        settings.delay = cmp::min(settings.max_delay, settings.delay);

        let mut state_guard = self.state.lock().unwrap();
        let state = state_guard.as_mut().ok_or(gst::FlowError::NotNegotiated)?;

        let mut map = buf.map_writable().map_err(|_| gst::FlowError::Error)?;

        match state.info.format() {
            gst_audio::AUDIO_FORMAT_F64 => {
                let data = map.as_mut_slice_of::<f64>().unwrap();
                Self::process(data, state, &settings);
            }
            gst_audio::AUDIO_FORMAT_F32 => {
                let data = map.as_mut_slice_of::<f32>().unwrap();
                Self::process(data, state, &settings);
            }
            _ => return Err(gst::FlowError::NotNegotiated),
        }

        Ok(gst::FlowSuccess::Ok)
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        // Drop state
        let _ = self.state.lock().unwrap().take();

        Ok(())
    }
}

impl AudioFilterImpl for AudioEcho {
    fn allowed_caps() -> &'static gst::Caps {
        static CAPS: LazyLock<gst::Caps> = LazyLock::new(|| {
            gst_audio::AudioCapsBuilder::new_interleaved()
                .format_list([gst_audio::AUDIO_FORMAT_F32, gst_audio::AUDIO_FORMAT_F64])
                .build()
        });

        &CAPS
    }

    fn setup(&self, info: &gst_audio::AudioInfo) -> Result<(), gst::LoggableError> {
        let max_delay = self.settings.lock().unwrap().max_delay;
        let size = (max_delay * (info.rate() as u64)).seconds() as usize;
        let buffer_size = size * (info.channels() as usize);

        *self.state.lock().unwrap() = Some(State {
            info: info.clone(),
            buffer: RingBuffer::new(buffer_size),
        });

        Ok(())
    }
}
