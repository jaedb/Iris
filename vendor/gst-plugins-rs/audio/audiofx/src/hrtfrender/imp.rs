// Copyright (C) 2021 Tomasz Andrzejak <andreiltd@gmail.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use gst_base::prelude::*;
use gst_base::subclass::prelude::*;

use hrtf::{HrirSphere, HrtfContext, HrtfProcessor, Vec3};

use std::io::Error;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Weak};

use byte_slice_cast::*;
use rayon::prelude::*;
use rayon::{ThreadPool, ThreadPoolBuilder};

use std::sync::LazyLock;
static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "hrtfrender",
        gst::DebugColorFlags::empty(),
        Some("Head-Related Transfer Function Renderer"),
    )
});

static THREAD_POOL: LazyLock<Mutex<Weak<ThreadPool>>> = LazyLock::new(|| Mutex::new(Weak::new()));

const DEFAULT_INTERPOLATION_STEPS: u64 = 8;
const DEFAULT_BLOCK_LENGTH: u64 = 512;
const DEFAULT_DISTANCE_GAIN: f32 = 1.0;

#[derive(Clone, Copy)]
struct SpatialObject {
    /// Position values use a left-handed Cartesian coordinate system
    position: Vec3,
    /// Object attenuation by distance
    distance_gain: f32,
}

impl From<gst::Structure> for SpatialObject {
    fn from(s: gst::Structure) -> Self {
        SpatialObject {
            position: Vec3 {
                x: s.get("x").expect("type checked upstream"),
                y: s.get("y").expect("type checked upstream"),
                z: s.get("z").expect("type checked upstream"),
            },
            distance_gain: s.get("distance-gain").expect("type checked upstream"),
        }
    }
}

impl From<SpatialObject> for gst::Structure {
    fn from(obj: SpatialObject) -> Self {
        gst::Structure::builder("application/spatial-object")
            .field("x", obj.position.x)
            .field("y", obj.position.y)
            .field("z", obj.position.z)
            .field("distance-gain", obj.distance_gain)
            .build()
    }
}

impl TryFrom<gst_audio::AudioChannelPosition> for SpatialObject {
    type Error = gst::FlowError;

    fn try_from(pos: gst_audio::AudioChannelPosition) -> Result<Self, gst::FlowError> {
        use gst_audio::AudioChannelPosition::*;

        let position = match pos {
            FrontLeft => Vec3::new(-1.45, 0.0, 2.5),
            FrontRight => Vec3::new(1.45, 0.0, 2.5),
            FrontCenter | Mono => Vec3::new(0.0, 0.0, 2.5),
            Lfe1 | Lfe2 => Vec3::new(0.0, 0.0, 0.0),
            RearLeft => Vec3::new(-1.45, 0.0, -2.5),
            RearRight => Vec3::new(1.45, 0.0, -2.5),
            FrontLeftOfCenter => Vec3::new(-0.72, 0.0, 2.5),
            FrontRightOfCenter => Vec3::new(0.72, 0.0, 2.5),
            RearCenter => Vec3::new(0.0, 0.0, -2.5),
            SideLeft => Vec3::new(-2.5, 0.0, -0.44),
            SideRight => Vec3::new(2.5, 0.0, -0.44),
            TopFrontLeft => Vec3::new(-0.72, 2.5, 1.25),
            TopFrontRight => Vec3::new(0.72, 2.5, 1.25),
            TopFrontCenter => Vec3::new(0.0, 2.5, 1.25),
            TopCenter => Vec3::new(0.0, 2.5, 0.0),
            TopRearLeft => Vec3::new(-0.72, 2.5, -1.25),
            TopRearRight => Vec3::new(0.72, 2.5, -1.25),
            TopSideLeft => Vec3::new(-1.25, 2.5, -0.22),
            TopSideRight => Vec3::new(1.25, 2.5, -0.22),
            TopRearCenter => Vec3::new(0.0, 2.5, -1.25),
            BottomFrontCenter => Vec3::new(0.0, -2.5, 1.25),
            BottomFrontLeft => Vec3::new(-0.72, -2.5, 1.25),
            BottomFrontRight => Vec3::new(0.72, -2.5, 1.25),
            WideLeft => Vec3::new(-2.5, 0.0, 1.45),
            WideRight => Vec3::new(2.5, 0.0, 1.45),
            SurroundLeft => Vec3::new(-2.5, 0.0, -1.45),
            SurroundRight => Vec3::new(2.5, 0.0, -1.45),
            _ => return Err(gst::FlowError::NotSupported),
        };

        Ok(SpatialObject {
            position,
            distance_gain: DEFAULT_DISTANCE_GAIN,
        })
    }
}

#[derive(Clone)]
struct Settings {
    interpolation_steps: u64,
    block_length: u64,
    spatial_objects: Option<Vec<SpatialObject>>,
    hrir_raw_bytes: Option<glib::Bytes>,
    hrir_file_location: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            interpolation_steps: DEFAULT_INTERPOLATION_STEPS,
            block_length: DEFAULT_BLOCK_LENGTH,
            spatial_objects: None,
            hrir_raw_bytes: None,
            hrir_file_location: None,
        }
    }
}

impl Settings {
    fn position(&self, channel: usize) -> Result<Vec3, gst::FlowError> {
        Ok(self
            .spatial_objects
            .as_ref()
            .ok_or(gst::FlowError::NotNegotiated)?[channel]
            .position)
    }

    fn distance_gain(&self, channel: usize) -> Result<f32, gst::FlowError> {
        Ok(self
            .spatial_objects
            .as_ref()
            .ok_or(gst::FlowError::NotNegotiated)?[channel]
            .distance_gain)
    }

    fn sphere(&self, rate: u32) -> Result<hrtf::HrirSphere, hrtf::HrtfError> {
        if let Some(bytes) = &self.hrir_raw_bytes {
            return HrirSphere::new(bytes.as_byte_slice(), rate);
        }

        if let Some(path) = &self.hrir_file_location {
            return HrirSphere::from_file(PathBuf::from(path), rate);
        }

        Err(Error::other("Impulse response not set").into())
    }
}

struct ChannelProcessor {
    prev_left_samples: Vec<f32>,
    prev_right_samples: Vec<f32>,
    prev_sample_vector: Option<Vec3>,
    prev_distance_gain: Option<f32>,
    indata_scratch: Box<[f32]>,
    outdata_scratch: Box<[(f32, f32)]>,
    processor: HrtfProcessor,
}

struct State {
    ininfo: gst_audio::AudioInfo,
    outinfo: gst_audio::AudioInfo,
    adapter: gst_base::UniqueAdapter,
    block_samples: usize,
    channel_processors: Vec<ChannelProcessor>,
}

impl State {
    fn input_block_size(&self) -> usize {
        self.block_samples * self.ininfo.bpf() as usize
    }

    fn output_block_size(&self) -> usize {
        self.block_samples * self.outinfo.bpf() as usize
    }

    fn reset_processors(&mut self) {
        for cp in self.channel_processors.iter_mut() {
            cp.prev_left_samples.fill(0.0);
            cp.prev_right_samples.fill(0.0);
        }
    }
}

#[derive(Default)]
pub struct HrtfRender {
    settings: Mutex<Settings>,
    state: Mutex<Option<State>>,
    thread_pool: Mutex<Option<Arc<ThreadPool>>>,
}

#[glib::object_subclass]
impl ObjectSubclass for HrtfRender {
    const NAME: &'static str = "GstHrtfRender";
    type Type = super::HrtfRender;
    type ParentType = gst_base::BaseTransform;
}

impl HrtfRender {
    fn process(
        &self,
        outbuf: &mut gst::BufferRef,
        state: &mut State,
        settings: &Settings,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut outbuf =
            gst_audio::AudioBufferRef::from_buffer_ref_writable(outbuf, &state.outinfo).map_err(
                |err| {
                    gst::error!(CAT, imp = self, "Failed to map buffer : {}", err);
                    gst::FlowError::Error
                },
            )?;

        let outdata = outbuf
            .plane_data_mut(0)
            .unwrap()
            .as_mut_slice_of::<f32>()
            .unwrap();

        // prefill output with zeros so we can mix processed samples into it
        outdata.fill(0.0);

        let inblksz = state.input_block_size();
        let channels = state.ininfo.channels();

        let mut written_samples = 0;

        let thread_pool_guard = self.thread_pool.lock().unwrap();
        let thread_pool = thread_pool_guard.as_ref().ok_or(gst::FlowError::Error)?;

        while state.adapter.available() >= inblksz {
            let inbuf = state.adapter.take_buffer(inblksz).map_err(|_| {
                gst::error!(CAT, imp = self, "Failed to map buffer");
                gst::FlowError::Error
            })?;

            let inbuf = gst_audio::AudioBuffer::from_buffer_readable(inbuf, &state.ininfo)
                .map_err(|_| {
                    gst::error!(CAT, imp = self, "Failed to map buffer");
                    gst::FlowError::Error
                })?;

            let indata = inbuf.plane_data(0).unwrap().as_slice_of::<f32>().unwrap();

            thread_pool.install(|| -> Result<(), gst::FlowError> {
                state
                    .channel_processors
                    .par_iter_mut()
                    .enumerate()
                    .try_for_each(|(i, cp)| -> Result<(), gst::FlowError> {
                        let new_distance_gain = settings.distance_gain(i)?;
                        let new_sample_vector = settings.position(i)?;

                        // Convert to Right Handed, this is what HRTF crate expects
                        let new_sample_vector = Vec3 {
                            z: -new_sample_vector.z,
                            ..new_sample_vector
                        };

                        // Deinterleave single channel to scratch buffer
                        for (x, y) in Iterator::zip(
                            indata.iter().skip(i).step_by(channels as usize),
                            cp.indata_scratch.iter_mut(),
                        ) {
                            *y = *x;
                        }

                        cp.processor.process_samples(HrtfContext {
                            source: &cp.indata_scratch,
                            output: &mut cp.outdata_scratch,
                            new_sample_vector,
                            new_distance_gain,
                            prev_sample_vector: cp.prev_sample_vector.unwrap_or(new_sample_vector),
                            prev_distance_gain: cp.prev_distance_gain.unwrap_or(new_distance_gain),
                            prev_left_samples: &mut cp.prev_left_samples,
                            prev_right_samples: &mut cp.prev_right_samples,
                        });

                        cp.prev_sample_vector = Some(new_sample_vector);
                        cp.prev_distance_gain = Some(new_distance_gain);

                        Ok(())
                    })
            })?;

            // unpack output scratch to output buffer
            state.channel_processors.iter_mut().for_each(|cp| {
                for (x, y) in Iterator::zip(
                    cp.outdata_scratch.iter(),
                    outdata[2 * written_samples..].chunks_exact_mut(2),
                ) {
                    y[0] += x.0;
                    y[1] += x.1;
                }

                // HRTF is mixing processed samples with samples in output buffer, we need to
                // reset scratch so it is not mixed with the next frame
                cp.outdata_scratch.fill((0.0, 0.0));
            });

            written_samples += state.block_samples;
        }

        // we only support stereo output, we can assert that we filled the whole
        // output buffer with stereo frames
        assert_eq!(outdata.len(), written_samples * 2);

        Ok(gst::FlowSuccess::Ok)
    }

    fn drain(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        let settings = &self.settings.lock().unwrap();

        let mut state_guard = self.state.lock().unwrap();
        let state = state_guard.as_mut().ok_or(gst::FlowError::NotNegotiated)?;

        let avail = state.adapter.available();

        if avail == 0 {
            return Ok(gst::FlowSuccess::Ok);
        }

        let inblksz = state.input_block_size();
        let outblksz = state.output_block_size();
        let inbpf = state.ininfo.bpf() as usize;
        let outbpf = state.outinfo.bpf() as usize;

        let inputsz = inblksz - avail;
        let outputsz = avail / inbpf * outbpf;

        let mut inbuf = gst::Buffer::with_size(inputsz).map_err(|_| gst::FlowError::Error)?;
        let inbuf_mut = inbuf.get_mut().ok_or(gst::FlowError::Error)?;

        let mut map = inbuf_mut
            .map_writable()
            .map_err(|_| gst::FlowError::Error)?;
        let data = map
            .as_mut_slice_of::<f32>()
            .map_err(|_| gst::FlowError::Error)?;

        data.fill(0.0);
        drop(map);

        let (pts, offset, duration) = {
            let samples_to_time = |samples: u64| {
                samples
                    .mul_div_round(*gst::ClockTime::SECOND, state.ininfo.rate() as u64)
                    .map(gst::ClockTime::from_nseconds)
            };

            let (prev_pts, distance) = state.adapter.prev_pts();
            let distance_samples = distance / inbpf as u64;
            let pts = prev_pts.opt_add(samples_to_time(distance_samples));

            let (prev_offset, _) = state.adapter.prev_offset();
            let offset = prev_offset.checked_add(distance_samples).unwrap_or(0);

            let duration_samples = outputsz / outbpf;
            let duration = samples_to_time(duration_samples as u64);

            (pts, offset, duration)
        };

        state.adapter.push(inbuf);

        let mut outbuf = gst::Buffer::with_size(outblksz).map_err(|_| gst::FlowError::Error)?;
        let outbuf_mut = outbuf.get_mut().unwrap();

        self.process(outbuf_mut, state, settings)?;

        outbuf_mut.set_size(outputsz);
        outbuf_mut.set_pts(pts);
        outbuf_mut.set_offset(offset);
        outbuf_mut.set_duration(duration);

        state.reset_processors();

        drop(state_guard);
        self.obj().src_pad().push(outbuf)
    }
}

impl ObjectImpl for HrtfRender {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecBoxed::builder::<glib::Bytes>("hrir-raw")
                    .nick("Head Transform Impulse Response")
                    .blurb("Head Transform Impulse Response raw bytes")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("hrir-file")
                    .nick("Head Transform Impulse Response")
                    .blurb("Head Transform Impulse Response file location to read from")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt64::builder("interpolation-steps")
                    .nick("Interpolation Steps")
                    .blurb("Interpolation Steps is the amount of slices to cut source to")
                    .maximum(u64::MAX - 1)
                    .default_value(DEFAULT_INTERPOLATION_STEPS)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt64::builder("block-length")
                    .nick("Block Length")
                    .blurb("Block Length is the length of each slice")
                    .maximum(u64::MAX - 1)
                    .default_value(DEFAULT_BLOCK_LENGTH)
                    .mutable_ready()
                    .build(),
                gst::ParamSpecArray::builder("spatial-objects")
                    .element_spec(
                        &glib::ParamSpecBoxed::builder::<gst::Structure>("spatial-object")
                            .nick("Spatial Object")
                            .blurb("Spatial Object Metadata")
                            .build(),
                    )
                    .nick("Spatial Objects")
                    .blurb("Spatial object Metadata to apply on input channels")
                    .mutable_playing()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "hrir-raw" => {
                let mut settings = self.settings.lock().unwrap();
                settings.hrir_raw_bytes = value.get().expect("type checked upstream");
            }
            "hrir-file" => {
                let mut settings = self.settings.lock().unwrap();
                settings.hrir_file_location = value.get().expect("type checked upstream");
            }
            "interpolation-steps" => {
                let mut settings = self.settings.lock().unwrap();
                settings.interpolation_steps = value.get().expect("type checked upstream");
            }
            "block-length" => {
                let mut settings = self.settings.lock().unwrap();
                settings.block_length = value.get().expect("type checked upstream");
            }
            "spatial-objects" => {
                let mut settings = self.settings.lock().unwrap();

                let objs = value
                    .get::<gst::Array>()
                    .expect("type checked upstream")
                    .iter()
                    .map(|v| {
                        let s = v.get::<gst::Structure>().expect("type checked upstream");
                        SpatialObject::from(s)
                    })
                    .collect::<Vec<_>>();

                let mut state_guard = self.state.lock().unwrap();

                if let Some(state) = state_guard.as_mut() {
                    if objs.len() != state.ininfo.channels() as usize {
                        gst::warning!(
                            CAT,
                            "Could not update spatial objects, expected {} channels, got {}",
                            state.ininfo.channels(),
                            objs.len()
                        );
                        return;
                    }
                }

                settings.spatial_objects = if objs.is_empty() { None } else { Some(objs) };
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "hrir-raw" => {
                let settings = self.settings.lock().unwrap();
                settings.hrir_raw_bytes.to_value()
            }
            "hrir-file" => {
                let settings = self.settings.lock().unwrap();
                settings.hrir_file_location.to_value()
            }
            "interpolation-steps" => {
                let settings = self.settings.lock().unwrap();
                settings.interpolation_steps.to_value()
            }
            "block-length" => {
                let settings = self.settings.lock().unwrap();
                settings.block_length.to_value()
            }
            "spatial-objects" => {
                let settings = self.settings.lock().unwrap();

                settings
                    .spatial_objects
                    .as_ref()
                    .unwrap_or(&Vec::new())
                    .iter()
                    .map(|x| gst::Structure::from(*x).to_send_value())
                    .collect::<gst::Array>()
                    .to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for HrtfRender {}

impl ElementImpl for HrtfRender {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Head-Related Transfer Function (HRTF) renderer",
                "Filter/Effect/Audio",
                "Renders spatial sounds to a given position",
                "Tomasz Andrzejak <andreiltd@gmail.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let src_caps = gst_audio::AudioCapsBuilder::new_interleaved()
                .channels(2)
                .format(gst_audio::AUDIO_FORMAT_F32)
                .build();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &src_caps,
            )
            .unwrap();

            let sink_caps = gst_audio::AudioCapsBuilder::new_interleaved()
                .channels_range(1..=64)
                .format(gst_audio::AUDIO_FORMAT_F32)
                .build();

            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &sink_caps,
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl BaseTransformImpl for HrtfRender {
    const MODE: gst_base::subclass::BaseTransformMode =
        gst_base::subclass::BaseTransformMode::NeverInPlace;
    const PASSTHROUGH_ON_SAME_CAPS: bool = false;
    const TRANSFORM_IP_ON_PASSTHROUGH: bool = false;

    fn transform(
        &self,
        inbuf: &gst::Buffer,
        outbuf: &mut gst::BufferRef,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let settings = &self.settings.lock().unwrap();

        let mut state_guard = self.state.lock().unwrap();
        let state = state_guard.as_mut().ok_or(gst::FlowError::NotNegotiated)?;

        state.adapter.push(inbuf.clone());

        if state.adapter.available() >= state.input_block_size() {
            return self.process(outbuf, state, settings);
        }

        Ok(gst::FlowSuccess::Ok)
    }

    fn transform_size(
        &self,
        _direction: gst::PadDirection,
        _caps: &gst::Caps,
        size: usize,
        _othercaps: &gst::Caps,
    ) -> Option<usize> {
        assert_ne!(_direction, gst::PadDirection::Src);

        let mut state_guard = self.state.lock().unwrap();
        let state = state_guard.as_mut()?;

        let othersize = {
            let full_blocks = (size + state.adapter.available()) / (state.input_block_size());
            full_blocks * state.output_block_size()
        };

        gst::log!(
            CAT,
            imp = self,
            "Adapter size: {}, input size {}, transformed size {}",
            state.adapter.available(),
            size,
            othersize,
        );

        Some(othersize)
    }

    fn transform_caps(
        &self,
        direction: gst::PadDirection,
        caps: &gst::Caps,
        filter: Option<&gst::Caps>,
    ) -> Option<gst::Caps> {
        let mut other_caps = {
            let mut new_caps = caps.clone();

            for s in new_caps.make_mut().iter_mut() {
                s.set("format", gst_audio::AUDIO_FORMAT_F32.to_str());
                s.set("layout", "interleaved");

                if direction == gst::PadDirection::Sink {
                    s.set("channels", 2);
                    s.set("channel-mask", gst::Bitmask(0x3));
                } else {
                    let settings = self.settings.lock().unwrap();
                    if let Some(objs) = &settings.spatial_objects {
                        s.set("channels", objs.len() as i32);
                    } else {
                        s.set("channels", gst::IntRange::new(1, i32::MAX));
                    }

                    s.remove_field("channel-mask");
                }
            }
            new_caps
        };

        if let Some(filter) = filter {
            other_caps = filter.intersect_with_mode(&other_caps, gst::CapsIntersectMode::First);
        }

        gst::debug!(
            CAT,
            imp = self,
            "Transformed caps from {} to {} in direction {:?}",
            caps,
            other_caps,
            direction
        );

        Some(other_caps)
    }

    fn set_caps(&self, incaps: &gst::Caps, outcaps: &gst::Caps) -> Result<(), gst::LoggableError> {
        let ininfo = gst_audio::AudioInfo::from_caps(incaps)
            .map_err(|_| gst::loggable_error!(CAT, "Failed to parse input caps"))?;

        let outinfo = gst_audio::AudioInfo::from_caps(outcaps)
            .map_err(|_| gst::loggable_error!(CAT, "Failed to parse output caps"))?;

        let settings = &mut self.settings.lock().unwrap();

        if settings.spatial_objects.is_none() {
            if let Some(positions) = ininfo.positions() {
                let objs: Result<Vec<_>, _> = positions
                    .iter()
                    .map(|p| SpatialObject::try_from(*p))
                    .collect();

                if objs.is_err() {
                    return Err(gst::loggable_error!(CAT, "Unsupported channel position"));
                }

                settings.spatial_objects = objs.ok();
            } else {
                return Err(gst::loggable_error!(CAT, "Cannot infer object positions"));
            }
        }

        if settings.spatial_objects.as_ref().unwrap().len() != ininfo.channels() as usize {
            return Err(gst::loggable_error!(CAT, "Wrong number of spatial objects"));
        }

        let sphere = settings
            .sphere(ininfo.rate())
            .map_err(|e| gst::loggable_error!(CAT, "Failed to load sphere {:?}", e))?;

        let steps = settings.interpolation_steps as usize;
        let blklen = settings.block_length as usize;

        let block_samples = blklen
            .checked_mul(steps)
            .ok_or_else(|| gst::loggable_error!(CAT, "Not enough memory for frame allocation"))?;

        let channel_processors = (0..ininfo.channels())
            .map(|_| ChannelProcessor {
                prev_left_samples: vec![0.0; block_samples],
                prev_right_samples: vec![0.0; block_samples],
                prev_sample_vector: None,
                prev_distance_gain: None,
                indata_scratch: vec![0.0; block_samples].into_boxed_slice(),
                outdata_scratch: vec![(0.0, 0.0); block_samples].into_boxed_slice(),
                processor: HrtfProcessor::new(sphere.to_owned(), steps, blklen),
            })
            .collect();

        *self.state.lock().unwrap() = Some(State {
            ininfo,
            outinfo,
            block_samples,
            channel_processors,
            adapter: gst_base::UniqueAdapter::new(),
        });

        gst::debug!(CAT, imp = self, "Configured for caps {}", incaps);

        Ok(())
    }

    fn sink_event(&self, event: gst::Event) -> bool {
        use gst::EventView;

        gst::debug!(CAT, imp = self, "Handling event {:?}", event);

        match event.view() {
            EventView::FlushStop(_) => {
                let mut state_guard = self.state.lock().unwrap();

                if let Some(state) = state_guard.as_mut() {
                    let avail = state.adapter.available();
                    state.adapter.flush(avail);
                    state.reset_processors();
                }
            }
            EventView::Eos(_) => {
                if self.drain().is_err() {
                    gst::warning!(CAT, "Failed to drain internal buffer");
                    gst::element_imp_warning!(
                        self,
                        gst::CoreError::Event,
                        ["Failed to drain internal buffer"]
                    );
                }
            }
            _ => {}
        }

        self.parent_sink_event(event)
    }

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        // get global thread pool
        let mut thread_pool_g = THREAD_POOL.lock().unwrap();
        let mut thread_pool = self.thread_pool.lock().unwrap();

        if let Some(tp) = thread_pool_g.upgrade() {
            *thread_pool = Some(tp);
        } else {
            let tp = ThreadPoolBuilder::new().build().map_err(|_| {
                gst::error_msg!(
                    gst::CoreError::Failed,
                    ["Could not create rayon thread pool"]
                )
            })?;

            let tp = Arc::new(tp);

            *thread_pool = Some(tp);
            *thread_pool_g = Arc::downgrade(thread_pool.as_ref().unwrap());
        }

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        // Drop state
        let _ = self.state.lock().unwrap().take();
        // Drop thread pool
        let _ = self.thread_pool.lock().unwrap().take();

        Ok(())
    }
}
