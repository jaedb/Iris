// Copyright (C) 2019-2020 Sebastian Dröge <sebastian@centricular.com>
//
// Audio processing part of this file ported from ffmpeg/libavfilter/af_loudnorm.c
//
// Original C code
//   Copyright (c) 2016 Kyle Swanson <k@ylo.ph>
// licensed under the LGPL-2.1+ and generously relicensed to MPL-2.0.
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use std::mem;
use std::sync::Mutex;

use byte_slice_cast::*;

use std::sync::LazyLock;

use atomic_refcell::AtomicRefCell;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "audioloudnorm",
        gst::DebugColorFlags::empty(),
        Some("Audio Loudless Normalization Filter"),
    )
});

const DEFAULT_LOUDNESS_TARGET: f64 = -24.0;
const DEFAULT_LOUDNESS_RANGE_TARGET: f64 = 7.0;
const DEFAULT_MAX_TRUE_PEAK: f64 = -2.0;
const DEFAULT_OFFSET: f64 = 0.0;

#[derive(Debug, Clone, Copy)]
struct Settings {
    pub loudness_target: f64,
    pub loudness_range_target: f64,
    pub max_true_peak: f64,
    pub offset: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            loudness_target: DEFAULT_LOUDNESS_TARGET,
            loudness_range_target: DEFAULT_LOUDNESS_RANGE_TARGET,
            max_true_peak: DEFAULT_MAX_TRUE_PEAK,
            offset: DEFAULT_OFFSET,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameType {
    First,
    Inner,
    Final,
    Linear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LimiterState {
    Out,
    Attack,
    Sustain,
    Release,
}

struct State {
    info: gst_audio::AudioInfo,
    adapter: gst_base::UniqueAdapter,

    // Current amount of sample we consume per iteration: for the first frame 3s, afterwards 100ms
    current_samples_per_frame: usize,

    // Settings during setup
    offset: f64,
    target_i: f64,
    target_lra: f64,
    target_tp: f64,

    // Input ringbuffer for loudness analysis
    // TODO: Convert to a proper ringbuffer
    buf: Box<[f64]>,
    // read index
    buf_index: usize,
    // write index (always 210ms from buf_index)
    prev_buf_index: usize,

    // Gaussian filter for gains
    // TODO: These are actually constant. Once `for` is allowed
    // in `const fn` we can make them proper constants.
    weights: [f64; 21],
    // TODO: Convert to a proper ringbuffer
    delta: [f64; 30],
    index: usize,
    prev_delta: f64,

    // Limiter
    gain_reduction: [f64; 2],
    // TODO: Convert to a proper ringbuffer
    limiter_buf: Box<[f64]>,
    // Read/write index, depending on context
    limiter_buf_index: usize,
    // Previous sample (potentially of the previous frame) used for detecting peaks in the limiter
    prev_smp: Box<[f64]>,
    limiter_state: LimiterState,
    // During attack/release state, the position in the corresponding window
    env_cnt: usize,
    // Number of samples to sustain at the beginning of the sustain state, if any
    sustain_cnt: Option<usize>,

    frame_type: FrameType,
    above_threshold: bool,

    // Input loudness calculation
    r128_in: ebur128::EbuR128,
    // Actual output loudness calculation
    r128_out: ebur128::EbuR128,
}

impl State {
    fn new(settings: &Settings, info: gst_audio::AudioInfo) -> Self {
        let r128_in = ebur128::EbuR128::new(
            info.channels(),
            info.rate(),
            ebur128::Mode::HISTOGRAM
                | ebur128::Mode::I
                | ebur128::Mode::S
                | ebur128::Mode::LRA
                | ebur128::Mode::SAMPLE_PEAK,
        )
        .unwrap();
        let r128_out = ebur128::EbuR128::new(
            info.channels(),
            info.rate(),
            ebur128::Mode::HISTOGRAM
                | ebur128::Mode::I
                | ebur128::Mode::S
                | ebur128::Mode::LRA
                | ebur128::Mode::SAMPLE_PEAK,
        )
        .unwrap();

        let buf_size = GAIN_LOOKAHEAD * info.channels() as usize;
        let buf = vec![0.0; buf_size].into_boxed_slice();

        let limiter_buf_size = (2 * FRAME_SIZE + LIMITER_LOOKAHEAD) * info.channels() as usize;
        let limiter_buf = vec![0.0; limiter_buf_size].into_boxed_slice();

        let prev_smp = vec![0.0; info.channels() as usize].into_boxed_slice();

        let current_samples_per_frame = GAIN_LOOKAHEAD;

        let buf_index = 0;
        let prev_buf_index = 0;
        let limiter_buf_index = 0;
        let index = 1;
        let limiter_state = LimiterState::Out;
        let offset = f64::powf(10., settings.offset / 20.);
        let target_tp = f64::powf(10., settings.max_true_peak / 20.);

        State {
            info,
            adapter: gst_base::UniqueAdapter::new(),
            current_samples_per_frame,
            offset,
            target_i: settings.loudness_target,
            target_lra: settings.loudness_range_target,
            target_tp,
            buf,
            buf_index,
            prev_buf_index,
            delta: [0.0; 30],
            weights: init_gaussian_filter(),
            prev_delta: 0.0,
            index,
            gain_reduction: [0.0; 2],
            limiter_buf,
            prev_smp,
            limiter_buf_index,
            limiter_state,
            env_cnt: 0,
            sustain_cnt: None,
            frame_type: FrameType::First,
            above_threshold: false,
            r128_in,
            r128_out,
        }
    }
}

pub struct AudioLoudNorm {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
    settings: Mutex<Settings>,
    state: AtomicRefCell<Option<State>>,
}

// Gain analysis parameters
const GAIN_LOOKAHEAD: usize = 3 * 192_000; // 3s
const FRAME_SIZE: usize = 19_200; // 100ms

// Limiter parameters
const LIMITER_ATTACK_WINDOW: usize = 1920; // 10ms
const LIMITER_RELEASE_WINDOW: usize = 19_200; // 100ms
const LIMITER_LOOKAHEAD: usize = 1920; // 10ms

impl State {
    // Drains all full frames that are currently in the adapter
    fn drain_full_frames(
        &mut self,
        imp: &AudioLoudNorm,
    ) -> Result<Vec<gst::Buffer>, gst::FlowError> {
        let mut outbufs = vec![];
        while self.adapter.available() >= self.info.bpf() as usize * self.current_samples_per_frame
        {
            let (pts, distance) = self.adapter.prev_pts();
            let distance_samples = distance / self.info.bpf() as u64;
            let distance_ts = distance_samples
                .mul_div_floor(*gst::ClockTime::SECOND, self.info.rate() as u64)
                .map(gst::ClockTime::from_nseconds);
            let pts = pts
                .opt_checked_add(distance_ts)
                .map_err(|_| gst::FlowError::Error)?;

            let inbuf = self
                .adapter
                .take_buffer(self.info.bpf() as usize * self.current_samples_per_frame)
                .unwrap();
            let src = inbuf.map_readable().map_err(|_| gst::FlowError::Error)?;
            let src = src
                .as_slice_of::<f64>()
                .map_err(|_| gst::FlowError::Error)?;

            let (mut outbuf, pts) = self.process(imp, src, pts)?;

            {
                let outbuf = outbuf.get_mut().unwrap();
                outbuf.set_pts(pts);
                outbuf.set_duration(
                    (outbuf.size() as u64)
                        .mul_div_floor(
                            *gst::ClockTime::SECOND,
                            (self.info.bpf() * self.info.rate()) as u64,
                        )
                        .map(gst::ClockTime::from_nseconds),
                );
            }

            outbufs.push(outbuf);
        }

        Ok(outbufs)
    }

    // Drains everything
    fn drain(&mut self, imp: &AudioLoudNorm) -> Result<gst::Buffer, gst::FlowError> {
        gst::debug!(CAT, imp = imp, "Draining");

        let (pts, distance) = self.adapter.prev_pts();
        let distance_samples = distance / self.info.bpf() as u64;
        let distance_ts = distance_samples
            .mul_div_floor(*gst::ClockTime::SECOND, self.info.rate() as u64)
            .map(gst::ClockTime::from_nseconds);
        let pts = pts
            .opt_checked_add(distance_ts)
            .map_err(|_| gst::FlowError::Error)?;

        let mut _mapped_inbuf = None;
        let src = if self.adapter.available() > 0 {
            let inbuf = self.adapter.take_buffer(self.adapter.available()).unwrap();

            let inbuf = inbuf
                .into_mapped_buffer_readable()
                .map_err(|_| gst::FlowError::Error)?;

            _mapped_inbuf = Some(inbuf);
            _mapped_inbuf
                .as_ref()
                .unwrap()
                .as_slice_of::<f64>()
                .map_err(|_| gst::FlowError::Error)?
        } else {
            &[]
        };

        // If we already output something before then we go into final frame processing, otherwise
        // we drain any data we still have by doing linear processing.
        if self.current_samples_per_frame == FRAME_SIZE {
            self.frame_type = FrameType::Final;
        } else if src.is_empty() {
            // Nothing to drain at all
            gst::debug!(CAT, imp = imp, "No data to drain");
            return Err(gst::FlowError::Eos);
        }

        let (mut outbuf, pts) = self.process(imp, src, pts)?;

        {
            let outbuf = outbuf.get_mut().unwrap();
            outbuf.set_pts(pts);
            outbuf.set_duration(
                (outbuf.size() as u64)
                    .mul_div_floor(
                        *gst::ClockTime::SECOND,
                        (self.info.bpf() * self.info.rate()) as u64,
                    )
                    .map(gst::ClockTime::from_nseconds),
            );
        }

        Ok(outbuf)
    }

    fn process_first_frame_is_last(&mut self, imp: &AudioLoudNorm) -> Result<(), gst::FlowError> {
        // Calculated loudness in LUFS
        let global = self
            .r128_in
            .loudness_global()
            .map_err(|_| gst::FlowError::Error)?;

        // Peak sample value for all changes
        let mut true_peak = 0.0;
        for c in 0..(self.info.channels()) {
            let peak = self
                .r128_in
                .sample_peak(c)
                .map_err(|_| gst::FlowError::Error)?;
            if c == 0 || peak > true_peak {
                true_peak = peak;
            }
        }

        gst::debug!(
            CAT,
            imp = imp,
            "Calculated global loudness for first frame {} with peak {}",
            global,
            true_peak
        );

        // Difference between targeted and calculated LUFS loudness as a linear scalefactor.
        let offset = f64::powf(10., (self.target_i - global) / 20.);
        // What the new peak would be after adjusting for the targeted loudness.
        let offset_tp = true_peak * offset;

        // If the new peak would be more quiet than targeted one, take it. Otherwise only go as
        // high as the true peak allows.
        self.offset = if offset_tp < self.target_tp {
            offset
        } else {
            self.target_tp / true_peak
        };

        self.frame_type = FrameType::Linear;

        Ok(())
    }

    fn process_first_frame(
        &mut self,
        imp: &AudioLoudNorm,
        src: &[f64],
        pts: impl Into<Option<gst::ClockTime>>,
    ) -> Result<(gst::Buffer, Option<gst::ClockTime>), gst::FlowError> {
        // Fill our whole buffer here with the initial input, i.e. 3000ms of samples.
        self.buf.copy_from_slice(src);

        // Calculate the shortterm loudness in LUFS.
        let shortterm = self
            .r128_in
            .loudness_shortterm()
            .map_err(|_| gst::FlowError::Error)?;

        let env_shortterm = if shortterm < -70.0 {
            self.above_threshold = false;
            0.
        } else {
            self.above_threshold = true;
            self.target_i - shortterm
        };

        // Initialize with linear scale factor for reaching the target loudness.
        for delta in self.delta.iter_mut() {
            *delta = f64::powf(10.0, env_shortterm / 20.);
        }
        self.prev_delta = self.delta[self.index];
        gst::debug!(
            CAT,
            imp = imp,
            "Initializing for first frame with gain adjustment of {}",
            self.prev_delta
        );

        // Fill the whole limiter_buf with the gain corrected first part of the buffered
        // input, i.e. 210ms. 100ms for the current frame plus 100ms lookahead for the
        // limiter with the next frame plus 10ms in addition because the limiter would
        // look a few samples further when detecting a peak to make sure no higher values
        // are following.
        for (limiter_buf, sample) in self.limiter_buf.iter_mut().zip(self.buf.iter()) {
            *limiter_buf = sample * self.prev_delta * self.offset;
        }

        // Read position of the buffer is now advanced.
        self.buf_index = self.limiter_buf.len();

        // Write position of the limiter_buf is at the beginning still. We consume
        // the first 100ms of it below directly so that the next iteration will
        // overwrite these 100ms directly.
        self.limiter_buf_index = 0;

        let mut outbuf = gst::Buffer::with_size(FRAME_SIZE * self.info.bpf() as usize)
            .map_err(|_| gst::FlowError::Error)?;
        {
            let outbuf = outbuf.get_mut().unwrap();
            let mut dst = outbuf.map_writable().map_err(|_| gst::FlowError::Error)?;
            let dst = dst
                .as_mut_slice_of::<f64>()
                .map_err(|_| gst::FlowError::Error)?;
            // This now consumes the first 100ms of limiter_buf for the output.
            self.true_peak_limiter(imp, dst);
            self.r128_out
                .add_frames_f64(dst)
                .map_err(|_| gst::FlowError::Error)?;
        }

        // From now on we consume 100ms input frames and output 100ms.
        self.current_samples_per_frame = FRAME_SIZE;
        self.frame_type = FrameType::Inner;

        // PTS is the input PTS for the first frame, we output the first 100ms of the input
        // buffer here
        Ok((outbuf, pts.into()))
    }

    fn process_fill_inner_frame(&mut self, imp: &AudioLoudNorm, src: &[f64]) {
        // Get gain for this and the next 100ms frame based the delta array
        // and smoothened with a gaussian filter.
        let gain = self.gaussian_filter(if self.index + 10 < 30 {
            self.index + 10
        } else {
            self.index + 10 - 30
        });
        let gain_next = self.gaussian_filter(if self.index + 11 < 30 {
            self.index + 11
        } else {
            self.index + 11 - 30
        });

        gst::debug!(
            CAT,
            imp = imp,
            "Applying gain adjustment {}-{}",
            gain,
            gain_next
        );

        // Overwrite the first 100ms of the limiter_buf with the gain corrected 100ms of
        // buf. This is correct because either above (for the first frame) or in the
        // previous iteration here we already have output these 100ms.
        //
        // Also fill 100ms of buf with the 100ms of new input at the same time.
        let channels = self.info.channels() as usize;
        assert!(src.len() / channels <= FRAME_SIZE);
        for (n, samples) in src.chunks_exact(channels).enumerate() {
            // Safety: Index ranges are checked below and both slices from buf are
            // guaranteed to be non-overlapping (210ms limiter_buf difference).
            let (buf_read, buf_write, limiter_buf) = unsafe {
                let (buf_read, buf_write) = {
                    let (a1, a2) = self.buf.split_at_mut_unchecked(self.buf_index);
                    let (buf_read, after_buf_read) = a2.split_at_mut_unchecked(channels);

                    let buf_write = if self.prev_buf_index < self.buf_index {
                        // Non-overlapping guarantee
                        &mut a1[self.prev_buf_index..self.prev_buf_index + channels]
                    } else {
                        // Split happens at buf_index + channels
                        let idx = self.prev_buf_index - (self.buf_index + channels);
                        &mut after_buf_read[idx..idx + channels]
                    };

                    // Seal the read buffer
                    (&*buf_read, buf_write)
                };
                let limiter_buf = self
                    .limiter_buf
                    .get_unchecked_mut(self.limiter_buf_index..(self.limiter_buf_index + channels));

                (buf_read, buf_write, limiter_buf)
            };

            buf_write.copy_from_slice(samples);

            // Linearly interpolate between the current and next gain for each sample.
            let current_gain =
                (gain + ((n as f64 / FRAME_SIZE as f64) * (gain_next - gain))) * self.offset;
            for (o, i) in limiter_buf.iter_mut().zip(buf_read.iter()) {
                *o = *i * current_gain;
            }

            self.limiter_buf_index += channels;
            if self.limiter_buf_index >= self.limiter_buf.len() {
                self.limiter_buf_index -= self.limiter_buf.len();
            }

            self.prev_buf_index += channels;
            if self.prev_buf_index >= self.buf.len() {
                self.prev_buf_index -= self.buf.len();
            }

            self.buf_index += channels;
            if self.buf_index >= self.buf.len() {
                self.buf_index -= self.buf.len();
            }
        }
    }

    fn process_update_gain_inner_frame(
        &mut self,
        imp: &AudioLoudNorm,
    ) -> Result<(), gst::FlowError> {
        // Calculate global, shortterm loudness and relative threshold in LUFS.
        let global = self
            .r128_in
            .loudness_global()
            .map_err(|_| gst::FlowError::Error)?;
        let shortterm = self
            .r128_in
            .loudness_shortterm()
            .map_err(|_| gst::FlowError::Error)?;
        let relative_threshold = self
            .r128_in
            .relative_threshold()
            .map_err(|_| gst::FlowError::Error)?;

        gst::debug!(
            CAT,
            imp = imp,
            "Calculated global loudness {}, short term loudness {} and relative threshold {}",
            global,
            shortterm,
            relative_threshold
        );

        // If we were previously not above the threshold but are now above in the
        // shortterm, slightly increase the scale factor. If the shortterm output was above
        // the target then also consider this frame above threshold.
        if !self.above_threshold {
            if shortterm > -70.0 {
                self.prev_delta *= 1.0058;
            }

            let shortterm_out = self
                .r128_out
                .loudness_shortterm()
                .map_err(|_| gst::FlowError::Error)?;
            if shortterm_out >= self.target_i {
                self.above_threshold = true;
                gst::debug!(
                    CAT,
                    imp = imp,
                    "Above threshold now ({} >= {}, {} > -70)",
                    shortterm_out,
                    self.target_i,
                    shortterm
                );
            }
        }

        // If we're still below the threshold, continue using the previous delta. Otherwise
        // calculate a new one.
        if shortterm < relative_threshold || shortterm <= -70. || !self.above_threshold {
            self.delta[self.index] = self.prev_delta;
        } else {
            let env_global = if (shortterm - global).abs() < (self.target_lra / 2.) {
                shortterm - global
            } else if (self.target_lra / 2.) * (shortterm - global) < 0.0 {
                -1.
            } else {
                1.
            };
            let env_shortterm = self.target_i - shortterm;
            self.delta[self.index] = f64::powf(10., (env_global + env_shortterm) / 20.);
        }

        self.prev_delta = self.delta[self.index];
        gst::debug!(
            CAT,
            imp = imp,
            "Calculated new gain adjustment {}",
            self.prev_delta
        );

        self.index += 1;
        if self.index >= 30 {
            self.index -= 30;
        }

        Ok(())
    }

    fn process_inner_frame(
        &mut self,
        imp: &AudioLoudNorm,
        src: &[f64],
        pts: impl Into<Option<gst::ClockTime>>,
    ) -> Result<(gst::Buffer, Option<gst::ClockTime>), gst::FlowError> {
        // Fill in these 100ms and adjust its gain according to previous measurements, and
        // at the same time copy 100ms over to the limiter_buf.
        self.process_fill_inner_frame(imp, src);

        // limiter_buf_index was 100ms advanced above, which brings us to exactly the
        // position where we have to start consuming 100ms for the output now, and exactly
        // the position where we have to start writing the next 100ms in the next
        // iteration.

        let mut outbuf =
            gst::Buffer::with_size(self.current_samples_per_frame * self.info.bpf() as usize)
                .map_err(|_| gst::FlowError::Error)?;
        {
            let outbuf = outbuf.get_mut().unwrap();
            let mut dst = outbuf.map_writable().map_err(|_| gst::FlowError::Error)?;
            let dst = dst
                .as_mut_slice_of::<f64>()
                .map_err(|_| gst::FlowError::Error)?;
            // This now consumes the next 100ms of limiter_buf for the output.
            self.true_peak_limiter(imp, dst);
            self.r128_out
                .add_frames_f64(dst)
                .map_err(|_| gst::FlowError::Error)?;
        }

        self.process_update_gain_inner_frame(imp)?;

        // PTS is 2.9s seconds before the input PTS as we buffer 3s of samples and just
        // outputted here the first 100ms of that.
        let pts = pts.into().map(|pts| pts + 100.mseconds() - 3.seconds());
        Ok((outbuf, pts))
    }

    fn process_fill_final_frame(&mut self, _imp: &AudioLoudNorm, idx: usize, num_samples: usize) {
        let channels = self.info.channels() as usize;

        // Get gain for this and the next 100ms frame based the delta array
        // and smoothened with a gaussian filter.
        let gain = self.gaussian_filter(if self.index + 10 < 30 {
            self.index + 10
        } else {
            self.index + 10 - 30
        });
        let gain_next = self.gaussian_filter(if self.index + 11 < 30 {
            self.index + 11
        } else {
            self.index + 11 - 30
        });

        for n in idx..num_samples {
            // Safety: Index ranges are checked below.
            let (buf_read, limiter_buf) = unsafe {
                let buf_read = self
                    .buf
                    .get_unchecked(self.buf_index..(self.buf_index + channels));
                let limiter_buf = self
                    .limiter_buf
                    .get_unchecked_mut(self.limiter_buf_index..(self.limiter_buf_index + channels));

                (buf_read, limiter_buf)
            };

            // Linearly interpolate between the current and next gain for each sample.
            let current_gain =
                (gain + ((n as f64 / num_samples as f64) * (gain_next - gain))) * self.offset;
            for (o, i) in limiter_buf.iter_mut().zip(buf_read.iter()) {
                *o = *i * current_gain;
            }

            self.limiter_buf_index += channels;
            if self.limiter_buf_index >= self.limiter_buf.len() {
                self.limiter_buf_index -= self.limiter_buf.len();
            }

            self.buf_index += channels;
            if self.buf_index >= self.buf.len() {
                self.buf_index -= self.buf.len();
            }
        }
    }

    fn process_final_frame(
        &mut self,
        imp: &AudioLoudNorm,
        src: &[f64],
        pts: impl Into<Option<gst::ClockTime>>,
    ) -> Result<(gst::Buffer, Option<gst::ClockTime>), gst::FlowError> {
        let channels = self.info.channels() as usize;
        let num_samples = src.len() / channels;

        // First process any new/leftover data we get passed. This is the same
        // as for inner frames. After this we will have done all gain adjustments
        // and all samples we ever output are in buf or limiter_buf.
        self.process_fill_inner_frame(imp, src);

        // If we got passed less than 100ms in src then limiter_buf_index is now
        // not yet at the correct read position! Adjust accordingly here so that all
        // further reads come from the right position by copying over the next samples
        // from buf.
        if num_samples != FRAME_SIZE {
            self.process_fill_final_frame(imp, num_samples, FRAME_SIZE);
        }

        // Now repeatedly run the limiter, output the output gain, update the gains, copy further
        // data from the buf to limiter_buf until we have output everything.
        //
        // At this point we have to output 3s - (FRAME_SIZE - num_samples)
        // buf.
        let out_num_samples = 30 * FRAME_SIZE - (FRAME_SIZE - num_samples);

        let mut outbuf = gst::Buffer::with_size(out_num_samples * self.info.bpf() as usize)
            .map_err(|_| gst::FlowError::Error)?;
        {
            let outbuf = outbuf.get_mut().unwrap();
            let mut dst = outbuf.map_writable().map_err(|_| gst::FlowError::Error)?;
            let dst = dst
                .as_mut_slice_of::<f64>()
                .map_err(|_| gst::FlowError::Error)?;
            let mut smp_cnt = 0;
            while smp_cnt < out_num_samples {
                let frame_size = std::cmp::min(out_num_samples - smp_cnt, FRAME_SIZE);
                let dst = &mut dst[(smp_cnt * channels)..((smp_cnt + frame_size) * channels)];

                // This now consumes the next frame_size samples of limiter_buf for the output.
                // Note that on the very last call this will read up to 10ms of old limiter_buf
                // data but as this was already processed it will not find any peak in there and
                // just pass through.
                //if frame_size < FRAME_SIZE {
                //    self.limiter_buf_index += FRAME_SIZE - num_samples;
                //}

                self.true_peak_limiter(imp, dst);

                smp_cnt += frame_size;
                if smp_cnt == out_num_samples {
                    break;
                }

                // Update the gain for the next iteration
                self.r128_out
                    .add_frames_f64(dst)
                    .map_err(|_| gst::FlowError::Error)?;
                self.process_update_gain_inner_frame(imp)?;

                // And now copy over the next block of samples from buf to limiter_buf
                let next_frame_size = std::cmp::min(out_num_samples - smp_cnt, FRAME_SIZE);
                self.process_fill_final_frame(imp, 0, next_frame_size);

                // Now for the very last frame we need to update the limiter buffer index by the
                // amount of samples the last frame is short to reach the correct read position.
                if next_frame_size < FRAME_SIZE {
                    self.limiter_buf_index += FRAME_SIZE - next_frame_size;
                    if self.limiter_buf_index >= self.limiter_buf.len() {
                        self.limiter_buf_index -= self.limiter_buf.len();
                    }
                }
            }
        }

        // PTS is 2.9s seconds before the input PTS as we buffer 3s of samples and just
        // outputted here the first 100ms of that.
        let pts = pts.into().map(|pts| pts + 100.mseconds() - 3.seconds());
        Ok((outbuf, pts))
    }

    fn process_linear_frame(
        &mut self,
        imp: &AudioLoudNorm,
        src: &[f64],
        pts: impl Into<Option<gst::ClockTime>>,
    ) -> Result<(gst::Buffer, Option<gst::ClockTime>), gst::FlowError> {
        // Apply a linear scale factor to the whole buffer

        gst::debug!(
            CAT,
            imp = imp,
            "Applying linear gain adjustment of {}",
            self.offset
        );

        let mut outbuf =
            gst::Buffer::with_size(mem::size_of_val(src)).map_err(|_| gst::FlowError::Error)?;
        {
            let outbuf = outbuf.get_mut().unwrap();
            let mut dst = outbuf.map_writable().map_err(|_| gst::FlowError::Error)?;
            let dst = dst
                .as_mut_slice_of::<f64>()
                .map_err(|_| gst::FlowError::Error)?;

            for (o, i) in dst.iter_mut().zip(src.iter()) {
                *o = *i * self.offset;
            }

            self.r128_out
                .add_frames_f64(dst)
                .map_err(|_| gst::FlowError::Error)?;
        }

        // PTS is input PTS as we just pass through the data without latency.
        Ok((outbuf, pts.into()))
    }

    fn process(
        &mut self,
        imp: &AudioLoudNorm,
        src: &[f64],
        pts: impl Into<Option<gst::ClockTime>>,
    ) -> Result<(gst::Buffer, Option<gst::ClockTime>), gst::FlowError> {
        self.r128_in
            .add_frames_f64(src)
            .map_err(|_| gst::FlowError::Error)?;

        // If we are at the end and had less than 3s of samples overall, do simple linear volume
        // adjustment. frame_type should only ever be set to Final at the end if we ended up in
        // Inner state before.
        if self.frame_type == FrameType::First
            && (src.len() / self.info.channels() as usize) < self.current_samples_per_frame
        {
            self.process_first_frame_is_last(imp)?;
        }

        match self.frame_type {
            FrameType::First => self.process_first_frame(imp, src, pts),
            FrameType::Inner => self.process_inner_frame(imp, src, pts),
            FrameType::Final => self.process_final_frame(imp, src, pts),
            FrameType::Linear => self.process_linear_frame(imp, src, pts),
        }
    }

    fn true_peak_limiter_out(
        &mut self,
        imp: &AudioLoudNorm,
        mut smp_cnt: usize,
        nb_samples: usize,
    ) -> usize {
        // Default out state, check if we have a new peak to act on in the next frame
        // and otherwise simply output all samples with the current gain adjustment.
        let peak = self.detect_peak(smp_cnt, nb_samples - smp_cnt);

        if let Some((peak_delta, peak_value)) = peak {
            self.limiter_state = LimiterState::Attack;
            self.env_cnt = 0;
            self.sustain_cnt = None;
            self.gain_reduction[0] = 1.;
            self.gain_reduction[1] = self.target_tp / peak_value;

            // Skip all samples that don't have to be adjusted because the peak is far
            // enough in the future.
            // Note: peak_delta=0 is LIMITER_LOOKAHEAD in the future and we have to start
            // LIMITER_ATTACK_WINDOW before the peak position.
            smp_cnt += LIMITER_LOOKAHEAD + peak_delta - LIMITER_ATTACK_WINDOW;

            gst::debug!(
               CAT,
               imp = imp,
               "Found peak {} at sample {}, going to attack state at sample {} (gain reduction {}-{})",
               peak_value,
               smp_cnt + LIMITER_ATTACK_WINDOW,
               smp_cnt,
               self.gain_reduction[0],
               self.gain_reduction[1]
            );
        } else {
            // Process all samples, no peak found
            smp_cnt = nb_samples;
        }

        smp_cnt
    }

    fn true_peak_limiter_attack(
        &mut self,
        imp: &AudioLoudNorm,
        mut smp_cnt: usize,
        nb_samples: usize,
    ) -> usize {
        let channels = self.info.channels() as usize;

        // Attack state, we have a peak in the near future and need to apply gain
        // reduction smoothly over the next milliseconds to not go over the threshold.
        // Once env_cnt reaches attack window we're at the peak sample.
        //
        // As there might be another, higher peak right afterwards we still need to
        // check for this and potentially update the gain reduction accordingly.

        let peak = self.detect_peak(smp_cnt, nb_samples - smp_cnt);
        let mut new_peak_smp_cnt = None;
        if let Some((peak_delta, _)) = peak {
            // If smp_cnt == new_peak_smp we're exactly 10ms before the new, higher
            // peak and need to increase the slope.
            new_peak_smp_cnt = Some(smp_cnt + peak_delta);
        }

        let mut index = self.limiter_buf_index + smp_cnt * channels;
        if index >= self.limiter_buf.len() {
            index -= self.limiter_buf.len();
        }

        while self.env_cnt < LIMITER_ATTACK_WINDOW && smp_cnt < nb_samples {
            // Stop once we're exactly 10ms before the new higher peak so we can
            // restart the attack state.
            if let Some(new_peak_smp_cnt) = new_peak_smp_cnt {
                if smp_cnt == new_peak_smp_cnt {
                    break;
                }
            }

            // Linear interpolation between the start and target gain reduction
            let env = self.gain_reduction[0]
                - (self.env_cnt as f64 / (LIMITER_ATTACK_WINDOW as f64 - 1.0)
                    * (self.gain_reduction[0] - self.gain_reduction[1]));

            // Safety: Index checked below
            let samples = unsafe {
                self.limiter_buf
                    .get_unchecked_mut(index..(index + channels))
            };
            for sample in samples {
                *sample *= env;
            }

            index += channels;
            if index >= self.limiter_buf.len() {
                index -= self.limiter_buf.len();
            }

            smp_cnt += 1;
            self.env_cnt += 1;
        }

        if let Some(new_peak_smp) = new_peak_smp_cnt {
            assert!(smp_cnt < nb_samples);

            // Sustain until we are exactly 10ms before the new peak in case
            // we finished the attack window above already.
            if smp_cnt < new_peak_smp {
                for _ in smp_cnt..new_peak_smp {
                    // Safety: Index checked below
                    let samples = unsafe {
                        self.limiter_buf
                            .get_unchecked_mut(index..(index + channels))
                    };
                    for sample in samples {
                        *sample *= self.gain_reduction[1];
                    }

                    index += channels;
                    if index >= self.limiter_buf.len() {
                        index -= self.limiter_buf.len();
                    }
                }

                smp_cnt = new_peak_smp;
            }

            assert!(smp_cnt < nb_samples);

            let (_, peak_value) = peak.unwrap();
            let gain_reduction = self.target_tp / peak_value;

            // If the gain reduction is more than our current target gain reduction we
            // need to change the attack state. If it less or the same we can simply
            // contain the current attack state as we will end up at a low enough again
            // before the new peak. We however have to remember to sustain at least
            // that long.
            if gain_reduction < self.gain_reduction[1] {
                // If we need to change something we need to consider two different
                // cases based on the slope of the gain reduction.

                let current_gain_reduction = self.gain_reduction[0]
                    - (self.env_cnt as f64 / (LIMITER_ATTACK_WINDOW as f64 - 1.0)
                        * (self.gain_reduction[0] - self.gain_reduction[1]));

                // Calculate the slopes. Note the minus!
                let old_slope = -(self.gain_reduction[0] - self.gain_reduction[1]);
                let new_slope = -(current_gain_reduction - gain_reduction);

                if new_slope <= old_slope {
                    // If the slope from our current position to the new gain reduction at
                    // the new peak is higher (we need to reduce gain faster) then we
                    // restart the attack state at this point with the higher slope. We
                    // will then reach the new peak at the end of the attack window.

                    self.limiter_state = LimiterState::Attack;
                    self.gain_reduction[0] = current_gain_reduction;
                    self.gain_reduction[1] = gain_reduction;
                    self.env_cnt = 0;
                    self.sustain_cnt = None;

                    gst::debug!(
                        CAT,
                        imp = imp,
                        "Found new peak {} at sample {}, restarting attack state at sample {} (gain reduction {}-{})",
                        peak_value,
                        smp_cnt + LIMITER_ATTACK_WINDOW,
                        smp_cnt,
                        self.gain_reduction[0],
                        self.gain_reduction[1],
                    );
                } else {
                    // If the slope is lower we can't simply reduce the slope as we would
                    // then have a lower gain reduction than needed at the previous peak.
                    // Instead of continue with the same slope but continue further than
                    // the old peak until we reach the required gain reduction for the new
                    // peak. Just like above we need to remember to sustain from the end of
                    // the attack window until the new peak.

                    // Calculate at which point we would reach the new gain reduction
                    // relative to 0.0 == attack window start, 1.0 attack window end.
                    let new_end = (gain_reduction - self.gain_reduction[0]) / old_slope;
                    let new_end = f64::max(new_end, 1.0);

                    // New start of the window, this will be in the past
                    let new_start = new_end - 1.0;

                    // Gain reduction at the new start. Note the plus as the slope is
                    // negative already here.
                    //
                    // Clippy warning ignored here because this is just incidentally the same as
                    // AssignAdd: we calculate a new adjusted gain reduction here, and override the
                    // previous one.
                    #[allow(clippy::assign_op_pattern)]
                    {
                        self.gain_reduction[0] = self.gain_reduction[0] + new_start * old_slope;
                    }

                    // At env_cnt == ATTACK_WINDOW we need the new gain reduction
                    self.gain_reduction[1] = gain_reduction;

                    // Calculate the current position in the attack window
                    let cur_pos = (current_gain_reduction - self.gain_reduction[0]) / old_slope;
                    let cur_pos = f64::clamp(cur_pos, 0.0, 1.0);
                    self.env_cnt = ((LIMITER_ATTACK_WINDOW as f64 - 1.0) * cur_pos) as usize;

                    // Need to sustain in any case for this many samples to actually
                    // reach the new peak
                    self.sustain_cnt = Some(self.env_cnt);

                    gst::debug!(
                        CAT,
                        imp = imp,
                        "Found new peak {} at sample {}, adjusting attack state at sample {} (gain reduction {}-{})",
                        peak_value,
                        smp_cnt + LIMITER_ATTACK_WINDOW,
                        smp_cnt,
                        self.gain_reduction[0],
                        self.gain_reduction[1],
                    );
                }
                return smp_cnt;
            } else {
                // We're ending the attack state this much before the new peak so need
                // to ensure that we at least sustain it for that long afterwards.
                gst::debug!(
                    CAT,
                    imp = imp,
                    "Found new low peak {} at sample {} in attack state at sample {}",
                    peak_value,
                    smp_cnt + LIMITER_ATTACK_WINDOW,
                    smp_cnt,
                );
                if self.env_cnt < LIMITER_ATTACK_WINDOW {
                    self.sustain_cnt = Some(self.env_cnt);
                }
            }
        }

        if self.env_cnt == LIMITER_ATTACK_WINDOW && smp_cnt < nb_samples {
            // If we reached the target gain reduction, go into sustain state.
            gst::debug!(
                CAT,
                imp = imp,
                "Going to sustain state at sample {} (gain reduction {})",
                smp_cnt,
                self.gain_reduction[1]
            );
            self.limiter_state = LimiterState::Sustain;
            // Keep sustain_cnt as is from above
        }

        smp_cnt
    }

    fn true_peak_limiter_sustain(
        &mut self,
        imp: &AudioLoudNorm,
        mut smp_cnt: usize,
        nb_samples: usize,
    ) -> usize {
        let channels = self.info.channels() as usize;

        // Sustain the previous gain reduction as long as a peak is found in the
        // next frame, otherwise go over to smoothly release.
        let peak = self.detect_peak(smp_cnt, nb_samples - smp_cnt);

        // We might have to sustain for a few more samples regardless of any new peak
        // we find in 10ms because of code above (first frame or ending the attack
        // state).
        // If another peak was found afterwards we can start working with that one: if
        // it's higher than we go into attack state, if it's lower we sustain for now.
        if let Some(sustain_cnt) = peak.map(|(d, _v)| d).or(self.sustain_cnt) {
            // Apply the final gain reduction from the previous attack for the next
            // samples until we're 1920 samples / 10ms before the peak and then either
            // need to go into attack state if the peak was higher, or stay in sustain
            // state and check for the next peak.

            let mut index = self.limiter_buf_index + smp_cnt * channels;
            if index >= self.limiter_buf.len() {
                index -= self.limiter_buf.len();
            }

            // Sustain the current gain reduction until we're exactly 10ms before
            // the new peak
            let mut s = 0;
            while s < sustain_cnt && smp_cnt < nb_samples {
                // Safety: Index checked below
                let samples = unsafe {
                    self.limiter_buf
                        .get_unchecked_mut(index..(index + channels))
                };
                for sample in samples {
                    *sample *= self.gain_reduction[1];
                }

                index += channels;
                if index >= self.limiter_buf.len() {
                    index -= self.limiter_buf.len();
                }

                smp_cnt += 1;
                s += 1;
            }

            if let Some((_, peak_value)) = peak {
                // If a higher peak than before is found in the next frame need to move
                // into attack state again to reduce the gain smoothly further.
                //
                // Otherwise we stay in sustain mode and smp_cnt is now exactly 10ms before
                // the new peak, i.e. the next call to detect_peak() would find the *next*
                // peak.
                let gain_reduction = self.target_tp / peak_value;
                if gain_reduction < self.gain_reduction[1] {
                    self.limiter_state = LimiterState::Attack;
                    self.env_cnt = 0;
                    self.sustain_cnt = None;
                    self.gain_reduction[0] = self.gain_reduction[1];
                    self.gain_reduction[1] = gain_reduction;

                    gst::debug!(
                        CAT,
                        imp = imp,
                        "Found new peak {} at sample {}, going back to attack state at sample {} (gain reduction {}-{})",
                        peak_value,
                        smp_cnt + LIMITER_ATTACK_WINDOW,
                        smp_cnt,
                        self.gain_reduction[0],
                        self.gain_reduction[1],
                    );
                } else {
                    gst::debug!(
                        CAT,
                        imp = imp,
                        "Found new peak {} at sample {}, going sustain further at sample {} (gain reduction {})",
                        peak_value,
                        smp_cnt + LIMITER_ATTACK_WINDOW,
                        smp_cnt,
                        self.gain_reduction[1],
                    );
                    // We need to sustain until the peak at least
                    self.sustain_cnt = Some(LIMITER_LOOKAHEAD);
                }
            } else if let Some(ref mut sustain_cnt) = self.sustain_cnt {
                *sustain_cnt -= s;
                if *sustain_cnt == 0 {
                    self.sustain_cnt = None;
                }
            } else {
                unreachable!();
            }
        } else {
            // If no new peak is found, release smoothly over the next 100ms.
            self.limiter_state = LimiterState::Release;
            self.gain_reduction[0] = self.gain_reduction[1];
            self.gain_reduction[1] = 1.;
            self.env_cnt = 0;

            gst::debug!(
                CAT,
                imp = imp,
                "Going to release state for sample {} at sample {} (gain reduction {}-1.0)",
                smp_cnt + LIMITER_RELEASE_WINDOW,
                smp_cnt,
                self.gain_reduction[0]
            );
        }

        smp_cnt
    }

    fn true_peak_limiter_release(
        &mut self,
        imp: &AudioLoudNorm,
        mut smp_cnt: usize,
        nb_samples: usize,
    ) -> usize {
        let channels = self.info.channels() as usize;

        // Smoothly release over the duration of 1 frame (100ms, 19200 samples).

        let mut index = self.limiter_buf_index + smp_cnt * channels;
        if index >= self.limiter_buf.len() {
            index -= self.limiter_buf.len();
        }

        // There might be a new peak during these 100ms, which we will have to detect
        // and in that case go into attack state again if the gain reduction is higher
        // than the current gain reduction we have, or go into sustain mode if it is
        // equal or lower. We don't stay in release mode if a peak is found.
        let peak = self.detect_peak(smp_cnt, nb_samples - smp_cnt);

        if let Some((peak_delta, peak_value)) = peak {
            let gain_reduction = self.target_tp / peak_value;
            let current_gain_reduction = self.gain_reduction[0]
                - (self.env_cnt as f64 / (LIMITER_RELEASE_WINDOW as f64 - 1.0)
                    * (self.gain_reduction[1] - self.gain_reduction[0]));

            if gain_reduction < current_gain_reduction {
                assert!(smp_cnt + peak_delta < nb_samples);

                // Sustain the current gain reduction until we're exactly 10ms before
                // the new peak
                for _ in 0..peak_delta {
                    // Safety: Index checked below
                    let samples = unsafe {
                        self.limiter_buf
                            .get_unchecked_mut(index..(index + channels))
                    };
                    for sample in samples {
                        *sample *= self.gain_reduction[1];
                    }

                    index += channels;
                    if index >= self.limiter_buf.len() {
                        index -= self.limiter_buf.len();
                    }

                    smp_cnt += 1;
                    assert!(smp_cnt < nb_samples);
                }

                self.limiter_state = LimiterState::Attack;
                self.env_cnt = 0;
                self.sustain_cnt = None;
                self.gain_reduction[0] = current_gain_reduction;
                self.gain_reduction[1] = gain_reduction;

                gst::debug!(
                   CAT,
                   imp = imp,
                   "Found new peak {} at sample {}, going back to attack state at sample {} (gain reduction {}-{})",
                   peak_value,
                   smp_cnt + LIMITER_ATTACK_WINDOW,
                   smp_cnt,
                   self.gain_reduction[0],
                   self.gain_reduction[1],
                );
            } else {
                self.gain_reduction[1] = current_gain_reduction;
                gst::debug!(
                    CAT,
                    imp = imp,
                    "Going from release to sustain state at sample {} because of low peak {} at sample {} (gain reduction {})",
                    smp_cnt,
                    peak_value,
                    smp_cnt + LIMITER_ATTACK_WINDOW,
                    self.gain_reduction[1]
                );
                self.limiter_state = LimiterState::Sustain;
            }

            return smp_cnt;
        }

        while self.env_cnt < LIMITER_RELEASE_WINDOW && smp_cnt < nb_samples {
            let env = self.gain_reduction[0]
                - (self.env_cnt as f64 / (LIMITER_RELEASE_WINDOW as f64 - 1.0)
                    * (self.gain_reduction[1] - self.gain_reduction[0]));

            // Safety: Index checked below
            let samples = unsafe {
                self.limiter_buf
                    .get_unchecked_mut(index..(index + channels))
            };
            for sample in samples {
                *sample *= env;
            }

            index += channels;
            if index >= self.limiter_buf.len() {
                index -= self.limiter_buf.len();
            }

            smp_cnt += 1;
            self.env_cnt += 1;
        }

        // If we're done with the release, go to out state
        if smp_cnt < nb_samples {
            self.limiter_state = LimiterState::Out;
            gst::debug!(
                CAT,
                imp = imp,
                "Leaving release state and going to out state at sample {}",
                smp_cnt,
            );
        }

        smp_cnt
    }

    fn true_peak_limiter_first_frame(&mut self, imp: &AudioLoudNorm) {
        let channels = self.info.channels() as usize;

        assert_eq!(self.limiter_buf_index, 0);
        let mut max = 0.;
        for sample in &self.limiter_buf[0..((LIMITER_LOOKAHEAD + 1) * channels)] {
            if sample.abs() > max {
                max = *sample;
            }
        }

        // Initialize the previous sample for peak detection with the last sample we looked at
        // above
        for (o, i) in self
            .prev_smp
            .iter_mut()
            .zip(self.limiter_buf[(LIMITER_LOOKAHEAD * channels)..].iter())
        {
            *o = i.abs();
        }

        if max > self.target_tp {
            // Pretend the first peak was at the last sample so that the sustain code can work
            // as with normal peaks
            self.limiter_state = LimiterState::Sustain;
            self.sustain_cnt = Some(LIMITER_LOOKAHEAD);
            self.gain_reduction[1] = self.target_tp / max;
            gst::debug!(
                CAT,
                imp = imp,
                "Reducing gain for start of first frame by {} ({} > {}) and going to sustain state",
                self.gain_reduction[1],
                max,
                self.target_tp
            );

            // The sustain code below will already handle the gain reduction and checking for
            // further peaks.
        }
    }

    fn true_peak_limiter(&mut self, imp: &AudioLoudNorm, dst: &mut [f64]) {
        let channels = self.info.channels() as usize;
        let nb_samples = dst.len() / channels;

        gst::debug!(CAT, imp = imp, "Running limiter for {} samples", nb_samples);

        // For the first frame we can't adjust the gain before it smoothly anymore so instead
        // apply the gain reduction immediately if we get above the threshold and move to sustain
        // state directly.
        if self.frame_type == FrameType::First {
            self.true_peak_limiter_first_frame(imp);
        }

        let mut smp_cnt = 0;
        while smp_cnt < nb_samples {
            match self.limiter_state {
                LimiterState::Out => {
                    smp_cnt = self.true_peak_limiter_out(imp, smp_cnt, nb_samples);
                }
                LimiterState::Attack => {
                    smp_cnt = self.true_peak_limiter_attack(imp, smp_cnt, nb_samples);
                }
                LimiterState::Sustain => {
                    smp_cnt = self.true_peak_limiter_sustain(imp, smp_cnt, nb_samples);
                }
                LimiterState::Release => {
                    smp_cnt = self.true_peak_limiter_release(imp, smp_cnt, nb_samples);
                }
            }
        }

        // Copy over the samples into the output buffer, after going through the limiter above.
        let mut index = self.limiter_buf_index;
        for dest_samples in dst.chunks_exact_mut(channels) {
            // Safety: Index checked below
            let in_samples = unsafe {
                self.limiter_buf
                    .get_unchecked_mut(index..(index + channels))
            };

            for (o, i) in dest_samples.iter_mut().zip(in_samples.iter()) {
                *o = *i;
                // Clamp to the maximum for rounding errors above
                if o.abs() > self.target_tp {
                    *o = self.target_tp * o.signum();
                }
            }

            index += channels;
            if index >= self.limiter_buf.len() {
                index -= self.limiter_buf.len();
            }
        }
    }

    // Checks if there is a peak above the threshold 10ms or 1920 samples after the current
    // sample. Returns the peak delta and its value. The peak delta is relative to
    // offset + LIMITER_LOOKAHEAD (10ms), i.e. a peak delta of 0 would be 10ms after the offset.
    //
    // peak delta 0 is never returned, i.e. it is safe to call this 10ms before a peak and it would
    // then return the next peak.
    fn detect_peak(&mut self, offset: usize, samples: usize) -> Option<(usize, f64)> {
        let channels = self.info.channels() as usize;

        // Check for a peak 1920 samples / 10ms in the future
        let mut index = self.limiter_buf_index + (offset + LIMITER_LOOKAHEAD) * channels;
        if index >= self.limiter_buf.len() {
            index -= self.limiter_buf.len();
        }

        for n in 0..samples {
            let mut next_index = index + channels;
            if next_index >= self.limiter_buf.len() {
                next_index -= self.limiter_buf.len();
            }

            // Get the current sample for each channel and the next here
            // Safety: Index checked above
            let (this, next) = unsafe {
                (
                    self.limiter_buf.get_unchecked(index..(index + channels)),
                    self.limiter_buf
                        .get_unchecked(next_index..(next_index + channels)),
                )
            };

            let mut detected = false;
            // Iterate over the previous sample for each channel, the current and the next, i.e.
            // in each iteration we're looking at channel c for those 3 samples.
            for (c, (prev_smp, (this, next))) in self
                .prev_smp
                .iter_mut()
                .zip(this.iter().zip(next.iter()))
                .enumerate()
            {
                let this = this.abs();
                let next = next.abs();

                detected = false;
                // Check if the current sample is the highest peak
                if (*prev_smp <= this) && (this >= next) && (this > self.target_tp) && (n > 0) {
                    detected = true;

                    // Check the 12 following samples, if one of them is higher then that would be
                    // the peak.
                    for i in 2..12 {
                        // Safety: Index checked right here
                        let next = unsafe {
                            let mut next_index = index + c + i * channels;
                            if next_index >= self.limiter_buf.len() {
                                next_index -= self.limiter_buf.len();
                            }

                            self.limiter_buf.get_unchecked(next_index).abs()
                        };

                        if next > this {
                            detected = false;
                            break;
                        }
                    }

                    if detected {
                        break;
                    }
                }

                // Remember as previous sample.
                *prev_smp = this;
            }

            // If this was the highest peak then remember it as the previous sample (as we didn't
            // just above here because of the break!) and return the peak index and value.
            if detected {
                let mut max_peak = 0.0;
                for (c, (prev_smp, this)) in (self.prev_smp.iter_mut().zip(this.iter())).enumerate()
                {
                    if c == 0 || this.abs() > max_peak {
                        max_peak = this.abs();
                    }
                    *prev_smp = this.abs();
                }

                return Some((n, max_peak));
            }

            index = next_index;
        }

        None
    }

    fn gaussian_filter(&self, index: usize) -> f64 {
        let mut result = 0.;

        let index = if index > 10 { index - 10 } else { index + 20 };

        // Apply gaussian filter to the gain adjustments for smoothening them.
        let delta = self.delta[index..].iter().chain(self.delta.iter());
        for (weight, delta) in self.weights.iter().zip(delta) {
            result += delta * weight;
        }

        result
    }
}

impl AudioLoudNorm {
    fn sink_chain(
        &self,
        _pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(CAT, imp = self, "Handling buffer {:?}", buffer);

        let mut state_guard = self.state.borrow_mut();
        let state = match *state_guard {
            None => {
                gst::error!(CAT, imp = self, "Not negotiated yet");
                return Err(gst::FlowError::NotNegotiated);
            }
            Some(ref mut state) => state,
        };

        let mut outbufs = vec![];
        if buffer.flags().contains(gst::BufferFlags::DISCONT) {
            gst::debug!(CAT, imp = self, "Draining on discontinuity");
            match state.drain(self) {
                Ok(outbuf) => {
                    outbufs.push(outbuf);
                }
                Err(gst::FlowError::Eos) => (),
                Err(err) => return Err(err),
            }

            // Need to reset the state now
            *state = State::new(&self.settings.lock().unwrap(), state.info.clone());
        }

        state.adapter.push(buffer);
        outbufs.append(&mut state.drain_full_frames(self)?);
        drop(state_guard);

        for buffer in outbufs {
            gst::log!(CAT, imp = self, "Outputting buffer {:?}", buffer);
            self.srcpad.push(buffer)?;
        }

        Ok(gst::FlowSuccess::Ok)
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::log!(CAT, obj = pad, "Handling event {:?}", event);

        match event.view() {
            EventView::Caps(c) => {
                let caps = c.caps();
                gst::info!(CAT, obj = pad, "Got caps {:?}", caps);

                let info = match gst_audio::AudioInfo::from_caps(caps) {
                    Ok(info) => info,
                    Err(_) => {
                        gst::error!(CAT, obj = pad, "Failed to parse caps");
                        return false;
                    }
                };

                let mut state = self.state.borrow_mut();
                let mut outbuf = None;
                if let Some(ref mut state) = &mut *state {
                    outbuf = match state.drain(self) {
                        Ok(outbuf) => Some(outbuf),
                        Err(gst::FlowError::Eos) => None,
                        Err(_) => return false,
                    };
                }
                *state = Some(State::new(&self.settings.lock().unwrap(), info));
                drop(state);

                if let Some(outbuf) = outbuf {
                    gst::log!(CAT, imp = self, "Outputting buffer {:?}", outbuf);
                    if let Err(err) = self.srcpad.push(outbuf) {
                        gst::error!(CAT, imp = self, "Failed to push drained data: {}", err);

                        return false;
                    }
                }
            }
            EventView::Eos(_) | EventView::Segment(_) => {
                let mut state = self.state.borrow_mut();
                let mut outbuf = None;
                if let Some(ref mut state) = &mut *state {
                    outbuf = match state.drain(self) {
                        Ok(outbuf) => Some(outbuf),
                        Err(gst::FlowError::Eos) => None,
                        Err(_) => return false,
                    };
                    *state = State::new(&self.settings.lock().unwrap(), state.info.clone());
                }
                drop(state);

                if let Some(outbuf) = outbuf {
                    gst::log!(CAT, imp = self, "Outputting buffer {:?}", outbuf);
                    if let Err(err) = self.srcpad.push(outbuf) {
                        gst::error!(
                            CAT,
                            imp = self,
                            "Failed to push drained data on EOS: {}",
                            err
                        );

                        return false;
                    }
                }
            }
            EventView::FlushStop(_) => {
                // Resetting our whole state
                let mut state = self.state.borrow_mut();

                if let Some(info) = state.as_ref().map(|s| s.info.clone()) {
                    let settings = *self.settings.lock().unwrap();
                    *state = Some(State::new(&settings, info));
                } else {
                    *state = None;
                }
            }
            _ => (),
        }

        gst::Pad::event_default(pad, Some(&*self.obj()), event)
    }

    #[allow(clippy::single_match)]
    fn src_query(&self, pad: &gst::Pad, query: &mut gst::QueryRef) -> bool {
        use gst::QueryViewMut;

        gst::log!(CAT, obj = pad, "Handling query {:?}", query);
        match query.view_mut() {
            QueryViewMut::Latency(q) => {
                let mut peer_query = gst::query::Latency::new();
                if self.sinkpad.peer_query(&mut peer_query) {
                    let (live, min_latency, max_latency) = peer_query.result();
                    q.set(
                        live,
                        min_latency + 3.seconds(),
                        max_latency.opt_add(3.seconds()),
                    );
                    true
                } else {
                    false
                }
            }
            _ => gst::Pad::query_default(pad, Some(&*self.obj()), query),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for AudioLoudNorm {
    const NAME: &'static str = "GstAudioLoudNorm";
    type Type = super::AudioLoudNorm;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                Self::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |this| this.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                Self::catch_panic_pad_function(parent, || false, |this| this.sink_event(pad, event))
            })
            .flags(gst::PadFlags::PROXY_CAPS)
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::Pad::builder_from_template(&templ)
            .query_function(|pad, parent, query| {
                Self::catch_panic_pad_function(parent, || false, |this| this.src_query(pad, query))
            })
            .flags(gst::PadFlags::PROXY_CAPS)
            .build();

        Self {
            sinkpad,
            srcpad,
            settings: Mutex::new(Default::default()),
            state: AtomicRefCell::new(None),
        }
    }
}

impl ObjectImpl for AudioLoudNorm {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecDouble::builder("loudness-target")
                    .nick("Loudness Target")
                    .blurb("Loudness target in LUFS")
                    .minimum(-70.0)
                    .maximum(-5.0)
                    .default_value(DEFAULT_LOUDNESS_TARGET)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecDouble::builder("loudness-range-target")
                    .nick("Loudness Range Target")
                    .blurb("Loudness range target in LU")
                    .minimum(1.0)
                    .maximum(20.0)
                    .default_value(DEFAULT_LOUDNESS_RANGE_TARGET)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecDouble::builder("max-true-peak")
                    .nick("Maximum True Peak")
                    .blurb("Maximum True Peak in dbTP")
                    .minimum(-9.0)
                    .maximum(0.0)
                    .default_value(DEFAULT_MAX_TRUE_PEAK)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecDouble::builder("offset")
                    .nick("Offset Gain")
                    .blurb("Offset Gain in LU")
                    .minimum(-99.0)
                    .maximum(99.0)
                    .default_value(DEFAULT_OFFSET)
                    .mutable_ready()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "loudness-target" => {
                let mut settings = self.settings.lock().unwrap();
                settings.loudness_target = value.get().expect("type checked upstream");
            }
            "loudness-range-target" => {
                let mut settings = self.settings.lock().unwrap();
                settings.loudness_range_target = value.get().expect("type checked upstream");
            }
            "max-true-peak" => {
                let mut settings = self.settings.lock().unwrap();
                settings.max_true_peak = value.get().expect("type checked upstream");
            }
            "offset" => {
                let mut settings = self.settings.lock().unwrap();
                settings.offset = value.get().expect("type checked upstream");
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "loudness-target" => {
                let settings = self.settings.lock().unwrap();
                settings.loudness_target.to_value()
            }
            "loudness-range-target" => {
                let settings = self.settings.lock().unwrap();
                settings.loudness_range_target.to_value()
            }
            "max-true-peak" => {
                let settings = self.settings.lock().unwrap();
                settings.max_true_peak.to_value()
            }
            "offset" => {
                let settings = self.settings.lock().unwrap();
                settings.offset.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for AudioLoudNorm {}

impl ElementImpl for AudioLoudNorm {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Audio loudness normalizer",
                "Filter/Effect/Audio",
                "Normalizes perceived loudness of an audio stream",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst_audio::AudioCapsBuilder::new_interleaved()
                .format(gst_audio::AUDIO_FORMAT_F64)
                .rate(192_000)
                .build();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    #[allow(clippy::single_match)]
    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        let res = self.parent_change_state(transition);

        match transition {
            gst::StateChange::PausedToReady => {
                // Drop state
                *self.state.borrow_mut() = None;
            }
            _ => (),
        }

        res
    }
}

fn init_gaussian_filter() -> [f64; 21] {
    let mut weights = [0.0f64; 21];
    let mut total_weight = 0.0f64;
    let sigma = 3.5f64;

    let offset = 21 / 2;
    let c1 = 1.0 / (sigma * f64::sqrt(2.0 * std::f64::consts::PI));
    let c2 = 2.0 * f64::powf(sigma, 2.0);

    for (i, weight) in weights.iter_mut().enumerate() {
        let x = i as f64 - offset as f64;
        *weight = c1 * f64::exp(-(f64::powf(x, 2.0) / c2));
        total_weight += *weight;
    }

    let adjust = 1.0 / total_weight;
    for weight in weights.iter_mut() {
        *weight *= adjust;
    }

    weights
}
