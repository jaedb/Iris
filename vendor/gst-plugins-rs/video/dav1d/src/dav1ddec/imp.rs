// Copyright (C) 2019 Philippe Normand <philn@igalia.com>
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
use gst_video::prelude::*;
use gst_video::subclass::prelude::*;

use std::cell::UnsafeCell;
use std::cmp;
use std::mem;
use std::ptr;
use std::sync::LazyLock;

use std::sync::{Mutex, MutexGuard};

#[glib::flags(name = "GstDav1dInloopFilterType")]
pub(crate) enum InloopFilterType {
    #[flags_value(name = "Enable deblocking filter", nick = "deblock")]
    DEBLOCK = dav1d::InloopFilterType::DEBLOCK.bits(),
    #[flags_value(
        name = "Enable Constrained Directional Enhancement Filter",
        nick = "cdef"
    )]
    CDEF = dav1d::InloopFilterType::CDEF.bits(),
    #[flags_value(name = "Enable loop restoration filter", nick = "restoration")]
    RESTORATION = dav1d::InloopFilterType::RESTORATION.bits(),
}

impl From<InloopFilterType> for dav1d::InloopFilterType {
    fn from(inloop_filter_type: InloopFilterType) -> Self {
        let mut dav1d_inloop_filter_type = dav1d::InloopFilterType::empty();
        if inloop_filter_type.contains(InloopFilterType::DEBLOCK) {
            dav1d_inloop_filter_type.set(dav1d::InloopFilterType::DEBLOCK, true);
        }
        if inloop_filter_type.contains(InloopFilterType::CDEF) {
            dav1d_inloop_filter_type.set(dav1d::InloopFilterType::CDEF, true);
        }
        if inloop_filter_type.contains(InloopFilterType::RESTORATION) {
            dav1d_inloop_filter_type.set(dav1d::InloopFilterType::RESTORATION, true);
        }

        dav1d_inloop_filter_type
    }
}

const DEFAULT_N_THREADS: u32 = 0;
const DEFAULT_MAX_FRAME_DELAY: i64 = -1;
const DEFAULT_APPLY_GRAIN: bool = false;
const DEFAULT_INLOOP_FILTERS: InloopFilterType = InloopFilterType::empty();

struct State {
    // Decoder always exists when the state does but is temporarily removed
    // while passing frames to it to allow unlocking the state as the decoder
    // will call back into the element for allocations.
    decoder: Option<dav1d::Decoder<super::Dav1dDec>>,
    configured_max_frame_delay: u32,
    input_state: gst_video::VideoCodecState<'static, gst_video::video_codec_state::Readable>,
    output_state:
        Option<gst_video::VideoCodecState<'static, gst_video::video_codec_state::Readable>>,
    output_pool: Option<gst::BufferPool>,
    video_meta_supported: bool,
    n_cpus: usize,
}

// We make our own settings object so we don't have to deal with a Sync impl for dav1d::Settings
struct Settings {
    n_threads: u32,
    max_frame_delay: i64,
    apply_grain: bool,
    inloop_filters: InloopFilterType,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            n_threads: DEFAULT_N_THREADS,
            max_frame_delay: DEFAULT_MAX_FRAME_DELAY,
            apply_grain: DEFAULT_APPLY_GRAIN,
            inloop_filters: DEFAULT_INLOOP_FILTERS,
        }
    }
}

#[derive(Default)]
pub struct Dav1dDec {
    state: Mutex<Option<State>>,
    settings: Mutex<Settings>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "dav1ddec",
        gst::DebugColorFlags::empty(),
        Some("Dav1d AV1 decoder"),
    )
});

impl Dav1dDec {
    fn with_decoder_unlocked<'a, T, F: FnOnce(&mut dav1d::Decoder<super::Dav1dDec>) -> T>(
        &'a self,
        mut state_guard: MutexGuard<'a, Option<State>>,
        func: F,
    ) -> (MutexGuard<'a, Option<State>>, Option<T>) {
        let Some(state) = &mut *state_guard else {
            return (state_guard, None);
        };

        let Some(mut decoder) = state.decoder.take() else {
            return (state_guard, None);
        };

        drop(state_guard);

        let res = func(&mut decoder);

        state_guard = self.state.lock().unwrap();
        if let Some(state) = &mut *state_guard {
            state.decoder = Some(decoder);
        }

        (state_guard, Some(res))
    }

    fn with_decoder_mut<T, F: FnOnce(&mut dav1d::Decoder<super::Dav1dDec>) -> T>(
        &self,
        state: &mut State,
        func: F,
    ) -> Option<T> {
        let decoder = state.decoder.as_mut()?;

        let res = func(decoder);

        Some(res)
    }

    // FIXME: drop this once we have API from dav1d to query this value
    // https://code.videolan.org/videolan/dav1d/-/merge_requests/1407
    fn estimate_frame_delay(&self, max_frame_delay: u32, n_threads: u32) -> u32 {
        if max_frame_delay > 0 {
            std::cmp::min(max_frame_delay, n_threads)
        } else {
            let n_tc = n_threads as f64;
            std::cmp::min(8, n_tc.sqrt().ceil() as u32)
        }
    }

    fn video_format_from_picture_parameters(
        &self,
        params: &dav1d::PictureParameters,
        input_state: &gst_video::VideoCodecState<'static, gst_video::video_codec_state::Readable>,
    ) -> gst_video::VideoFormat {
        let bpc = params.bits_per_component();
        let format_desc = match (params.pixel_layout(), bpc) {
            (dav1d::PixelLayout::I400, Some(dav1d::BitsPerComponent(8))) => "GRAY8",
            #[cfg(target_endian = "little")]
            (dav1d::PixelLayout::I400, Some(dav1d::BitsPerComponent(16))) => "GRAY16_LE",
            #[cfg(target_endian = "big")]
            (dav1d::PixelLayout::I400, Some(dav1d::BitsPerComponent(16))) => "GRAY16_BE",
            // (dav1d::PixelLayout::I400, Some(dav1d::BitsPerComponent(10))) => "GRAY10_LE32",
            (dav1d::PixelLayout::I420, _) => "I420",
            (dav1d::PixelLayout::I422, Some(dav1d::BitsPerComponent(8))) => "Y42B",
            (dav1d::PixelLayout::I422, _) => "I422",
            (dav1d::PixelLayout::I444, _) => "Y444",
            (layout, bpc) => {
                gst::warning!(
                    CAT,
                    imp = self,
                    "Unsupported dav1d format {:?}/{:?}",
                    layout,
                    bpc
                );
                return gst_video::VideoFormat::Unknown;
            }
        };

        let f = if format_desc.starts_with("GRAY") {
            format_desc.into()
        } else {
            match bpc {
                Some(b) => match b.0 {
                    8 => format_desc.into(),
                    _ => {
                        let endianness = if cfg!(target_endian = "little") {
                            "LE"
                        } else {
                            "BE"
                        };
                        format!("{f}_{b}{e}", f = format_desc, b = b.0, e = endianness)
                    }
                },
                None => format_desc.into(),
            }
        };

        let mut format = f.parse::<gst_video::VideoFormat>().unwrap_or_else(|_| {
            gst::warning!(CAT, imp = self, "Unsupported dav1d format: {}", f);
            gst_video::VideoFormat::Unknown
        });

        if format == gst_video::VideoFormat::Unknown {
            return format;
        }

        // Special handling of RGB
        let input_info = input_state.info();
        if input_info.colorimetry().matrix() == gst_video::VideoColorMatrix::Rgb
            || (input_info.colorimetry().matrix() == gst_video::VideoColorMatrix::Unknown
                && params.matrix_coefficients() == dav1d::pixel::MatrixCoefficients::Identity)
        {
            gst::debug!(
                CAT,
                imp = self,
                "Input is actually RGB, switching format {format:?} to RGB"
            );

            if params.pixel_layout() != dav1d::PixelLayout::I444 {
                gst::error!(
                    CAT,
                    imp = self,
                    "Unsupported non-4:4:4 YUV format {format:?} for RGB"
                );
                return gst_video::VideoFormat::Unknown;
            }

            let rgb_format = format.to_str().replace("Y444", "GBR");
            format = match rgb_format.parse::<gst_video::VideoFormat>() {
                Ok(format) => format,
                Err(_) => {
                    gst::error!(CAT, imp = self, "Unsupported YUV format {format:?} for RGB");
                    return gst_video::VideoFormat::Unknown;
                }
            };

            if input_info.colorimetry().transfer() != gst_video::VideoTransferFunction::Unknown
                && input_info.colorimetry().transfer() != gst_video::VideoTransferFunction::Srgb
                || (input_info.colorimetry().transfer()
                    == gst_video::VideoTransferFunction::Unknown
                    && params.transfer_characteristic()
                        != dav1d::pixel::TransferCharacteristic::SRGB)
            {
                gst::warning!(CAT, imp = self, "Unexpected non-sRGB transfer function");
            }

            if input_info.colorimetry().range() != gst_video::VideoColorRange::Unknown
                && input_info.colorimetry().range() != gst_video::VideoColorRange::Range0_255
                || (input_info.colorimetry().range() == gst_video::VideoColorRange::Unknown
                    && params.color_range() != dav1d::pixel::YUVRange::Full)
            {
                gst::warning!(CAT, imp = self, "Unexpected non-full-range RGB");
            }
        }

        format
    }

    fn colorimetry_from_picture_parameters(
        &self,
        params: &dav1d::PictureParameters,
        input_state: &gst_video::VideoCodecState<'static, gst_video::video_codec_state::Readable>,
    ) -> gst_video::VideoColorimetry {
        use dav1d::pixel;

        let input_caps = input_state.caps().unwrap();
        let input_structure = input_caps.structure(0).unwrap();
        let input_info = input_state.info();
        let input_colorimetry = input_info.colorimetry();

        let range = if !input_structure.has_field("colorimetry")
            || input_colorimetry.range() == gst_video::VideoColorRange::Unknown
        {
            match params.color_range() {
                pixel::YUVRange::Limited => gst_video::VideoColorRange::Range16_235,
                pixel::YUVRange::Full => gst_video::VideoColorRange::Range0_255,
            }
        } else {
            input_colorimetry.range()
        };

        let matrix = if !input_structure.has_field("colorimetry")
            || input_colorimetry.matrix() == gst_video::VideoColorMatrix::Unknown
        {
            match params.matrix_coefficients() {
                pixel::MatrixCoefficients::Identity => gst_video::VideoColorMatrix::Rgb,
                pixel::MatrixCoefficients::BT709 => gst_video::VideoColorMatrix::Bt709,
                pixel::MatrixCoefficients::Unspecified => gst_video::VideoColorMatrix::Bt709,
                pixel::MatrixCoefficients::BT470M => gst_video::VideoColorMatrix::Fcc,
                pixel::MatrixCoefficients::BT470BG => gst_video::VideoColorMatrix::Bt601,
                pixel::MatrixCoefficients::ST240M => gst_video::VideoColorMatrix::Smpte240m,
                pixel::MatrixCoefficients::BT2020NonConstantLuminance => {
                    gst_video::VideoColorMatrix::Bt2020
                }
                _ => {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Unsupported dav1d colorimetry matrix coefficients"
                    );
                    gst_video::VideoColorMatrix::Unknown
                }
            }
        } else {
            input_colorimetry.matrix()
        };

        let transfer = if !input_structure.has_field("colorimetry")
            || input_colorimetry.transfer() == gst_video::VideoTransferFunction::Unknown
        {
            match params.transfer_characteristic() {
                pixel::TransferCharacteristic::BT1886 => gst_video::VideoTransferFunction::Bt709,
                pixel::TransferCharacteristic::Unspecified => {
                    gst_video::VideoTransferFunction::Bt709
                }
                pixel::TransferCharacteristic::BT470M => gst_video::VideoTransferFunction::Bt709,
                pixel::TransferCharacteristic::BT470BG => gst_video::VideoTransferFunction::Gamma28,
                pixel::TransferCharacteristic::ST170M => gst_video::VideoTransferFunction::Bt601,
                pixel::TransferCharacteristic::ST240M => {
                    gst_video::VideoTransferFunction::Smpte240m
                }
                pixel::TransferCharacteristic::Linear => gst_video::VideoTransferFunction::Gamma10,
                pixel::TransferCharacteristic::Logarithmic100 => {
                    gst_video::VideoTransferFunction::Log100
                }
                pixel::TransferCharacteristic::Logarithmic316 => {
                    gst_video::VideoTransferFunction::Log316
                }
                pixel::TransferCharacteristic::SRGB => gst_video::VideoTransferFunction::Srgb,
                pixel::TransferCharacteristic::BT2020Ten => {
                    gst_video::VideoTransferFunction::Bt202010
                }
                pixel::TransferCharacteristic::BT2020Twelve => {
                    gst_video::VideoTransferFunction::Bt202012
                }
                pixel::TransferCharacteristic::PerceptualQuantizer => {
                    gst_video::VideoTransferFunction::Smpte2084
                }
                pixel::TransferCharacteristic::HybridLogGamma => {
                    gst_video::VideoTransferFunction::AribStdB67
                }
                _ => {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Unsupported dav1d colorimetry transfer function"
                    );
                    gst_video::VideoTransferFunction::Unknown
                }
            }
        } else {
            input_colorimetry.transfer()
        };

        let primaries = if !input_structure.has_field("colorimetry")
            || input_colorimetry.primaries() == gst_video::VideoColorPrimaries::Unknown
        {
            match params.color_primaries() {
                pixel::ColorPrimaries::BT709 => gst_video::VideoColorPrimaries::Bt709,
                pixel::ColorPrimaries::Unspecified => gst_video::VideoColorPrimaries::Bt709,
                pixel::ColorPrimaries::BT470M => gst_video::VideoColorPrimaries::Bt470m,
                pixel::ColorPrimaries::BT470BG => gst_video::VideoColorPrimaries::Bt470bg,
                pixel::ColorPrimaries::ST240M => gst_video::VideoColorPrimaries::Smpte240m,
                pixel::ColorPrimaries::Film => gst_video::VideoColorPrimaries::Film,
                pixel::ColorPrimaries::BT2020 => gst_video::VideoColorPrimaries::Bt2020,
                pixel::ColorPrimaries::ST428 => gst_video::VideoColorPrimaries::Smptest428,
                pixel::ColorPrimaries::P3DCI => gst_video::VideoColorPrimaries::Smpterp431,
                pixel::ColorPrimaries::P3Display => gst_video::VideoColorPrimaries::Smpteeg432,
                pixel::ColorPrimaries::Tech3213 => gst_video::VideoColorPrimaries::Ebu3213,
                _ => {
                    gst::warning!(CAT, imp = self, "Unsupported dav1d color primaries");
                    gst_video::VideoColorPrimaries::Unknown
                }
            }
        } else {
            input_colorimetry.primaries()
        };

        gst_video::VideoColorimetry::new(range, matrix, transfer, primaries)
    }

    fn chroma_site_from_picture_parameters(
        &self,
        params: &dav1d::PictureParameters,
        input_state: &gst_video::VideoCodecState<'static, gst_video::video_codec_state::Readable>,
    ) -> gst_video::VideoChromaSite {
        use dav1d::pixel;

        let input_caps = input_state.caps().unwrap();
        let input_structure = input_caps.structure(0).unwrap();

        let chroma_site = if !input_structure.has_field("chroma-site") {
            match params.pixel_layout() {
                dav1d::PixelLayout::I420 | dav1d::PixelLayout::I422 => {
                    match params.chroma_location() {
                        pixel::ChromaLocation::Center => Some(gst_video::VideoChromaSite::JPEG),
                        pixel::ChromaLocation::Left => Some(gst_video::VideoChromaSite::MPEG2),
                        _ => None,
                    }
                }
                _ => None,
            }
        } else {
            Some(input_state.info().chroma_site())
        };

        chroma_site.unwrap_or(input_state.info().chroma_site())
    }

    fn flush_decoder(&self, state: &mut State) {
        gst::info!(CAT, imp = self, "Flushing decoder");

        self.with_decoder_mut(state, |decoder| decoder.flush());
    }

    #[allow(clippy::type_complexity)]
    fn send_data<'a>(
        &'a self,
        state_guard: MutexGuard<'a, Option<State>>,
        input_buffer: gst::Buffer,
        frame: gst_video::VideoCodecFrame,
    ) -> Result<(MutexGuard<'a, Option<State>>, std::ops::ControlFlow<(), ()>), gst::FlowError>
    {
        gst::trace!(
            CAT,
            imp = self,
            "Sending data to decoder for frame {}",
            frame.system_frame_number()
        );

        let timestamp = frame.dts().map(|ts| *ts as i64);
        let duration = frame.duration().map(|d| *d as i64);

        let frame_number = Some(frame.system_frame_number() as i64);

        let input_data = input_buffer
            .into_mapped_buffer_readable()
            .map_err(|_| gst::FlowError::Error)?;

        let (state_guard, res) = self.with_decoder_unlocked(state_guard, |decoder| {
            decoder.send_data(input_data, frame_number, timestamp, duration)
        });

        let res = res.ok_or(gst::FlowError::Flushing)?;

        let res = match res {
            Ok(()) => {
                gst::trace!(CAT, imp = self, "Decoder returned OK");
                Ok(std::ops::ControlFlow::Break(()))
            }
            Err(dav1d::Error::Again) => {
                gst::trace!(CAT, imp = self, "Decoder returned EAGAIN");
                Ok(std::ops::ControlFlow::Continue(()))
            }
            Err(dav1d::Error::InvalidArgument) => {
                gst::trace!(CAT, imp = self, "Decoder returned EINVAL");
                gst_video::video_decoder_error!(
                    &*self.obj(),
                    1,
                    gst::LibraryError::Encode,
                    ["Bitstream error"]
                )?;
                Ok(std::ops::ControlFlow::Continue(()))
            }
            Err(err) => {
                gst::error!(CAT, "Sending data failed (error code: {})", err);
                self.obj().release_frame(frame);
                gst_video::video_decoder_error!(
                    &*self.obj(),
                    1,
                    gst::StreamError::Decode,
                    ["Sending data failed (error code {})", err]
                )
                .map(|_| std::ops::ControlFlow::Break(()))
            }
        };

        let res = res?;

        Ok((state_guard, res))
    }

    #[allow(clippy::type_complexity)]
    fn send_pending_data<'a>(
        &'a self,
        state_guard: MutexGuard<'a, Option<State>>,
    ) -> Result<(MutexGuard<'a, Option<State>>, std::ops::ControlFlow<(), ()>), gst::FlowError>
    {
        gst::trace!(CAT, imp = self, "Sending pending data to decoder");

        let (state_guard, res) =
            self.with_decoder_unlocked(state_guard, |decoder| decoder.send_pending_data());

        let res = res.ok_or(gst::FlowError::Flushing)?;

        let res = match res {
            Ok(()) => {
                gst::trace!(CAT, imp = self, "Decoder returned OK");
                Ok(std::ops::ControlFlow::Break(()))
            }
            Err(err) if err.is_again() => {
                gst::trace!(CAT, imp = self, "Decoder returned EAGAIN");
                Ok(std::ops::ControlFlow::Continue(()))
            }
            Err(err) => {
                gst::error!(CAT, "Sending data failed (error code: {})", err);
                gst_video::video_decoder_error!(
                    &*self.obj(),
                    1,
                    gst::StreamError::Decode,
                    ["Sending data failed (error code {})", err]
                )
                .map(|_| std::ops::ControlFlow::Break(()))
            }
        };

        let res = res?;

        Ok((state_guard, res))
    }

    fn decoded_picture_as_buffer(
        &self,
        mut state_guard: MutexGuard<Option<State>>,
        pic: &dav1d::Picture<super::Dav1dDec>,
        codec_frame: &mut gst_video::VideoCodecFrame,
    ) -> Result<(), gst::FlowError> {
        let state = state_guard.as_mut().ok_or(gst::FlowError::Flushing)?;
        let video_meta_supported = state.video_meta_supported;

        let output_state = state.output_state.as_ref().unwrap().clone();
        let info = output_state.info();
        let allocator_data = pic.allocator_data().unwrap();

        let strides_matching = info.offset() == allocator_data.frame().plane_offset()
            && info.stride() == allocator_data.frame().plane_stride();

        // We can forward the decoded buffer as-is if downstream supports the video meta or the
        // strides/offsets are matching with the default ones.
        let forward_buffer = video_meta_supported || strides_matching;

        drop(state_guard);

        let duration = pic.duration() as u64;
        if duration > 0 {
            codec_frame.set_duration(gst::ClockTime::from_nseconds(duration));
        }

        if forward_buffer {
            // SAFETY: Only this thread has access to this frame's allocator data so it
            // can be safely frozen at this point.
            unsafe {
                allocator_data.freeze(self, codec_frame);
            };
        } else {
            self.obj().allocate_output_frame(codec_frame, None)?;

            let mut_buffer = codec_frame
                .output_buffer_mut()
                .expect("output_buffer is set");

            gst::trace!(
                gst::CAT_PERFORMANCE,
                imp = self,
                "Copying decoded video frame to output",
            );

            assert!(mut_buffer.size() > 0);

            let mut out_vframe =
                gst_video::VideoFrameRef::from_buffer_ref_writable(mut_buffer, output_state.info())
                    .expect("can map writable frame");

            if allocator_data.frame().copy(&mut out_vframe).is_err() {
                gst::error!(CAT, imp = self, "Failed to copy video frame to output");
                return Err(gst::FlowError::Error);
            }
        }

        Ok(())
    }

    fn handle_picture<'s>(
        &'s self,
        state_guard: MutexGuard<'s, Option<State>>,
        pic: &dav1d::Picture<super::Dav1dDec>,
    ) -> Result<MutexGuard<'s, Option<State>>, gst::FlowError> {
        gst::trace!(CAT, imp = self, "Handling picture {}", pic.offset());

        let instance = self.obj();

        let output_state = instance
            .output_state()
            .expect("Output state not set. Shouldn't happen!");

        {
            let allocator_data = pic.allocator_data().unwrap();

            let info = output_state.info();
            if info.format() != allocator_data.frame().format()
                || info.width() != allocator_data.frame().width()
                || info.height() != allocator_data.frame().height()
            {
                gst::error!(
                    CAT,
                    imp = self,
                    "Received picture format does not match output state",
                );
                return Err(gst::FlowError::NotNegotiated);
            }
        }

        let offset = pic.offset() as i32;

        let frame = instance.frame(offset);
        if let Some(mut frame) = frame {
            self.decoded_picture_as_buffer(state_guard, pic, &mut frame)?;
            instance.finish_frame(frame)?;
            Ok(self.state.lock().unwrap())
        } else {
            gst::warning!(CAT, imp = self, "No frame found for offset {}", offset);
            Ok(state_guard)
        }
    }

    #[allow(clippy::type_complexity)]
    fn pending_picture<'a>(
        &'a self,
        mut state_guard: MutexGuard<'a, Option<State>>,
    ) -> Result<
        (
            MutexGuard<'a, Option<State>>,
            Option<dav1d::Picture<super::Dav1dDec>>,
        ),
        gst::FlowError,
    > {
        gst::trace!(CAT, imp = self, "Retrieving pending picture");

        let _state = state_guard.as_mut().ok_or(gst::FlowError::Flushing)?;

        let res;
        (state_guard, res) =
            self.with_decoder_unlocked(state_guard, |decoder| decoder.get_picture());
        let res = res.ok_or(gst::FlowError::Flushing)?;

        let res = match res {
            Ok(pic) => {
                gst::trace!(CAT, imp = self, "Retrieved picture {}", pic.offset());
                Ok(Some(pic))
            }
            Err(err) if err.is_again() => {
                gst::trace!(CAT, imp = self, "Decoder needs more data");
                Ok(None)
            }
            Err(err) => {
                gst::error!(
                    CAT,
                    imp = self,
                    "Retrieving decoded picture failed (error code {})",
                    err
                );

                gst_video::video_decoder_error!(
                    &*self.obj(),
                    1,
                    gst::StreamError::Decode,
                    ["Retrieving decoded picture failed (error code {})", err]
                )
                .map(|_| None)
            }
        };

        let res = res?;

        Ok((state_guard, res))
    }

    fn forward_pending_pictures<'s>(
        &'s self,
        mut state_guard: MutexGuard<'s, Option<State>>,
        drain: bool,
    ) -> Result<MutexGuard<'s, Option<State>>, gst::FlowError> {
        // dav1d wants to have get_picture() called a second time after it return EAGAIN to
        // actually drain all pending pictures.
        let mut call_twice = drain;

        loop {
            loop {
                let pic;
                (state_guard, pic) = self.pending_picture(state_guard)?;
                let Some(pic) = pic else {
                    break;
                };

                state_guard = self.handle_picture(state_guard, &pic)?;
                call_twice = false;
                if !drain {
                    break;
                }
            }

            if !call_twice {
                break;
            }
            call_twice = false;
        }

        Ok(state_guard)
    }
}

pub struct AllocatorData(UnsafeCell<AllocatorDataInner>);
struct AllocatorDataInner {
    vframe: mem::ManuallyDrop<gst_video::VideoFrame<gst_video::video_frame::Writable>>,
    cframe: Option<ptr::NonNull<gst_video::ffi::GstVideoCodecFrame>>,
}

impl AllocatorData {
    /// SAFETY: There must be no other thread freezing the same allocator data and codec frame
    /// at the same time.
    unsafe fn freeze(&self, imp: &Dav1dDec, codec_frame: &mut gst_video::VideoCodecFrame) {
        use glib::translate::*;

        let inner = self.0.get();

        if let Some(cframe) = (*inner).cframe {
            let buffer_ptr = (*(*inner).vframe.as_ptr()).buffer;
            gst::debug!(
                CAT,
                imp = imp,
                "Allocated frame ({:?}) is used for multiple output frames ({} and {})",
                (*inner).vframe.buffer(),
                codec_frame.system_frame_number(),
                cframe.as_ref().system_frame_number,
            );
            // Take an additional reference for storing in the second video codec frame
            gst::ffi::gst_mini_object_ref(buffer_ptr as *mut _);
        } else {
            // Steal the reference from the video frame and make sure to keep the video
            // codec frame alive as long as the video frame is mapped.
            (*(*inner).vframe.as_mut_ptr()).map[0].flags |=
                gst_video::ffi::GST_VIDEO_FRAME_MAP_FLAG_NO_REF;
            (*inner).cframe = Some(ptr::NonNull::new_unchecked(
                gst_video::ffi::gst_video_codec_frame_ref(codec_frame.to_glib_none().0),
            ));
        }

        codec_frame.set_output_buffer(from_glib_full((*(*inner).vframe.as_ptr()).buffer));
    }

    fn frame(&self) -> gst_video::VideoFrameRef<&gst::BufferRef> {
        unsafe {
            let inner = self.0.get();

            (*inner).vframe.as_video_frame_ref()
        }
    }
}

unsafe impl Send for AllocatorData {}
unsafe impl Sync for AllocatorData {}

impl Drop for AllocatorDataInner {
    fn drop(&mut self) {
        // The underlying buffer is either kept alive by the video frame
        // if no video codec frame was assigned yet, or otherwise by
        // the video codec frame.
        let no_ref_flag = unsafe {
            let vframe_ptr = self.vframe.as_ptr();
            ((*vframe_ptr).map[0].flags & gst_video::ffi::GST_VIDEO_FRAME_MAP_FLAG_NO_REF) != 0
        };

        assert!(self.cframe.is_none() ^ no_ref_flag);

        // Drop the video frame before unreffing the codec frame to ensure that
        // the buffer is still valid
        unsafe {
            mem::ManuallyDrop::drop(&mut self.vframe);
        }

        if let Some(cframe) = self.cframe {
            unsafe {
                gst_video::ffi::gst_video_codec_frame_unref(cframe.as_ptr());
            }
        }
    }
}

unsafe impl dav1d::PictureAllocator for super::Dav1dDec {
    type AllocatorData = AllocatorData;

    unsafe fn alloc_picture(
        &self,
        params: &dav1d::PictureParameters,
    ) -> Result<dav1d::PictureAllocation<Self::AllocatorData>, dav1d::Error> {
        let imp = self.imp();

        let mut state_guard = imp.state.lock().unwrap();

        let Some(mut state) = state_guard.as_mut() else {
            gst::trace!(CAT, obj = self, "Flushing");
            return Err(dav1d::Error::InvalidArgument);
        };

        let format = imp.video_format_from_picture_parameters(params, &state.input_state);
        if format == gst_video::VideoFormat::Unknown {
            state.output_state = None;
            return Err(dav1d::Error::InvalidArgument);
        }

        gst::trace!(
            CAT,
            obj = self,
            "Allocating for format {:?} with picture dimensions {}x{}",
            format,
            params.width(),
            params.height()
        );

        let colorimetry = imp.colorimetry_from_picture_parameters(params, &state.input_state);
        let chroma_site = imp.chroma_site_from_picture_parameters(params, &state.input_state);

        let mut new_output_state = false;
        if state.output_state.as_ref().is_none_or(|state| {
            let info = state.info();
            info.format() != format
                || info.width() != params.width()
                || info.height() != params.height()
                || info.chroma_site() != chroma_site
                || info.colorimetry() != colorimetry
        }) {
            state.output_state = None;
            state.output_pool = None;

            gst::debug!(CAT, obj = self, "Need to renegotiate");

            let Ok(mut output_state) = self.set_output_state(
                format,
                params.width(),
                params.height(),
                Some(&state.input_state),
            ) else {
                gst::error!(CAT, obj = self, "Failed to set output state");
                return Err(dav1d::Error::InvalidArgument);
            };

            let Ok(info) = gst_video::VideoInfo::builder_from_info(output_state.info())
                .colorimetry(&colorimetry)
                .chroma_site(chroma_site)
                .build()
            else {
                gst::error!(CAT, obj = self, "Failed to build output video info");
                return Err(dav1d::Error::InvalidArgument);
            };
            output_state.set_info(info);
            drop(state_guard);

            if self.negotiate(output_state).is_err() {
                gst::error!(CAT, obj = self, "Failed to negotiate");
                return Err(dav1d::Error::InvalidArgument);
            }

            state_guard = imp.state.lock().unwrap();
            let Some(s) = state_guard.as_mut() else {
                gst::trace!(CAT, obj = self, "Flushing");
                return Err(dav1d::Error::InvalidArgument);
            };
            state = s;

            let Some(output_state) = self.output_state() else {
                gst::error!(CAT, obj = self, "Have no output state");
                return Err(dav1d::Error::InvalidArgument);
            };
            state.output_state = Some(output_state.clone());
            new_output_state = true;

            // At this point either a pool is available and we can directly pass pool buffers to
            // downstream, or we create our own internal pool now from which we allocate and then
            // copy over to a downstream allocation.
            if state.output_pool.is_some() {
                gst::debug!(CAT, obj = self, "Using negotiated buffer pool");
            }
        }

        // Must be set now
        let output_state = state.output_state.as_ref().unwrap();

        if state.output_pool.is_none() {
            gst::debug!(CAT, obj = self, "Creating fallback buffer pool");

            let mut info = output_state.info().clone();

            let pool = gst_video::VideoBufferPool::new();

            let mut config = pool.config();

            let mut params = gst::AllocationParams::default();
            params.set_align(cmp::max(params.align(), dav1d::PICTURE_ALIGNMENT - 1));
            params.set_padding(cmp::max(params.padding(), dav1d::PICTURE_ALIGNMENT));

            config.set_allocator(None::<&gst::Allocator>, Some(&params));
            config.add_option(gst_video::BUFFER_POOL_OPTION_VIDEO_META);

            let aligned_width = info.width().next_multiple_of(128);
            let aligned_height = info.height().next_multiple_of(128);

            let stride_align = [params.align() as u32; gst_video::VIDEO_MAX_PLANES];

            let mut align = gst_video::VideoAlignment::new(
                0,
                aligned_height - info.height(),
                aligned_width - info.width(),
                0,
                &stride_align,
            );

            config.add_option(gst_video::BUFFER_POOL_OPTION_VIDEO_ALIGNMENT);

            if info.align(&mut align).is_err() {
                gst::error!(CAT, obj = self, "Failed to align video info");
                state.output_state = None;
                return Err(dav1d::Error::InvalidArgument);
            }
            config.set_video_alignment(&align);
            config.set_params(output_state.caps_owned().as_ref(), info.size() as u32, 0, 0);

            pool.set_config(config).unwrap();
            pool.set_active(true).unwrap();

            state.output_pool = Some(pool.upcast());
        }

        // Must be set now
        let pool = state.output_pool.as_ref().unwrap().clone();
        let output_state = output_state.clone();

        // Report the new latency
        if new_output_state {
            let frame_latency = self
                .imp()
                .estimate_frame_delay(state.configured_max_frame_delay, state.n_cpus as u32)
                as u64;

            let (fps_n, fps_d) = match (
                output_state.info().fps().numer(),
                output_state.info().fps().denom(),
            ) {
                (0, _) => (30, 1), // Pretend we're at 30fps if we don't know latency
                n => n,
            };

            let latency = frame_latency * (fps_d as u64).seconds() / (fps_n as u64);

            gst::debug!(CAT, obj = self, "Reporting latency of {}", latency);

            drop(state_guard);

            self.set_latency(latency, None);
        } else {
            drop(state_guard);
        }

        let Ok(buffer) = pool.acquire_buffer(None) else {
            gst::error!(CAT, obj = self, "Failed to acquire buffer");
            return Err(dav1d::Error::NotEnoughMemory);
        };

        let Ok(mut frame) =
            gst_video::VideoFrame::from_buffer_writable(buffer, output_state.info())
        else {
            gst::error!(CAT, obj = self, "Failed to map buffer");
            return Err(dav1d::Error::InvalidArgument);
        };

        let is_gray = frame.info().is_gray();

        let planes = frame.planes_data_mut();
        let data = [
            planes[0].as_mut_ptr(),
            if !is_gray {
                planes[1].as_mut_ptr()
            } else {
                ptr::null_mut()
            },
            if !is_gray {
                planes[2].as_mut_ptr()
            } else {
                ptr::null_mut()
            },
        ];
        let plane_strides = frame.plane_stride();
        let stride = [
            plane_strides[0] as isize,
            if !is_gray {
                assert_eq!(plane_strides[1], plane_strides[2]);
                plane_strides[1] as isize
            } else {
                0
            },
        ];

        Ok(dav1d::PictureAllocation {
            data,
            stride,
            allocator_data: AllocatorData(UnsafeCell::new(AllocatorDataInner {
                vframe: mem::ManuallyDrop::new(frame),
                cframe: None,
            })),
        })
    }

    unsafe fn release_picture(&self, allocation: dav1d::PictureAllocation<Self::AllocatorData>) {
        gst::trace!(
            CAT,
            obj = self,
            "Releasing video frame with buffer {:?}",
            allocation.allocator_data.frame().buffer()
        );
    }
}

fn video_output_formats() -> impl IntoIterator<Item = gst_video::VideoFormat> {
    [
        gst_video::VideoFormat::Gray8,
        #[cfg(target_endian = "little")]
        gst_video::VideoFormat::Gray16Le,
        #[cfg(target_endian = "big")]
        gst_video::VideoFormat::Gray16Be,
        // #[cfg(target_endian = "little")]
        // gst_video::VideoFormat::Gray10Le32,
        gst_video::VideoFormat::I420,
        gst_video::VideoFormat::Y42b,
        gst_video::VideoFormat::Y444,
        #[cfg(target_endian = "little")]
        gst_video::VideoFormat::I42010le,
        #[cfg(target_endian = "little")]
        gst_video::VideoFormat::I42210le,
        #[cfg(target_endian = "little")]
        gst_video::VideoFormat::Y44410le,
        #[cfg(target_endian = "big")]
        gst_video::VideoFormat::I42010be,
        #[cfg(target_endian = "big")]
        gst_video::VideoFormat::I42210be,
        #[cfg(target_endian = "big")]
        gst_video::VideoFormat::Y44410be,
        #[cfg(target_endian = "little")]
        gst_video::VideoFormat::I42012le,
        #[cfg(target_endian = "little")]
        gst_video::VideoFormat::I42212le,
        #[cfg(target_endian = "little")]
        gst_video::VideoFormat::Y44412le,
        #[cfg(target_endian = "big")]
        gst_video::VideoFormat::I42012be,
        #[cfg(target_endian = "big")]
        gst_video::VideoFormat::I42212be,
        #[cfg(target_endian = "big")]
        gst_video::VideoFormat::Y44412be,
    ]
}

#[glib::object_subclass]
impl ObjectSubclass for Dav1dDec {
    const NAME: &'static str = "GstDav1dDec";
    type Type = super::Dav1dDec;
    type ParentType = gst_video::VideoDecoder;
}

impl ObjectImpl for Dav1dDec {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecUInt::builder("n-threads")
                    .nick("Number of threads")
                    .blurb("Number of threads to use while decoding (set to 0 to use number of logical cores)")
                    .default_value(DEFAULT_N_THREADS)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecInt64::builder("max-frame-delay")
                    .nick("Maximum frame delay")
                    .blurb("Maximum delay in frames for the decoder (set to 1 for low latency, 0 to be equal to the number of logical cores. -1 to choose between these two based on pipeline liveness)")
                    .minimum(-1)
                    .maximum(u32::MAX.into())
                    .default_value(DEFAULT_MAX_FRAME_DELAY)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("apply-grain")
                    .nick("Enable film grain synthesis")
                    .blurb("Enable out-of-loop normative film grain filter")
                    .default_value(DEFAULT_APPLY_GRAIN)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecFlags::builder("inloop-filters")
                    .nick("Inloop filters")
                    .blurb("Flags to enable in-loop post processing filters")
                    .default_value(DEFAULT_INLOOP_FILTERS)
                    .mutable_ready()
                    .build()

            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().unwrap();

        match pspec.name() {
            "n-threads" => {
                settings.n_threads = value.get().expect("type checked upstream");
            }
            "max-frame-delay" => {
                settings.max_frame_delay = value.get().expect("type checked upstream");
            }
            "apply-grain" => {
                settings.apply_grain = value.get().expect("type checked upstream");
            }
            "inloop-filters" => {
                settings.inloop_filters = value.get().expect("type checked upstream");
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().unwrap();

        match pspec.name() {
            "n-threads" => settings.n_threads.to_value(),
            "max-frame-delay" => settings.max_frame_delay.to_value(),
            "apply-grain" => settings.apply_grain.to_value(),
            "inloop-filters" => settings.inloop_filters.to_value(),
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for Dav1dDec {}

impl ElementImpl for Dav1dDec {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Dav1d AV1 Decoder",
                "Codec/Decoder/Video",
                "Decode AV1 video streams with dav1d",
                "Philippe Normand <philn@igalia.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let sink_caps = if gst::version() >= (1, 19, 0, 0) {
                gst::Caps::builder("video/x-av1")
                    .field("stream-format", "obu-stream")
                    .field("alignment", gst::List::new(["frame", "tu"]))
                    .build()
            } else {
                gst::Caps::builder("video/x-av1").build()
            };
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &sink_caps,
            )
            .unwrap();

            let src_caps = gst_video::VideoCapsBuilder::new()
                .format_list(video_output_formats())
                .build();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &src_caps,
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl VideoDecoderImpl for Dav1dDec {
    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        {
            let mut state_guard = self.state.lock().unwrap();
            *state_guard = None;
        }

        self.parent_stop()
    }

    fn set_format(
        &self,
        input_state: &gst_video::VideoCodecState<'static, gst_video::video_codec_state::Readable>,
    ) -> Result<(), gst::LoggableError> {
        let mut state_guard = self.state.lock().unwrap();
        // Drain decoder on caps changes if necessary
        if state_guard.is_some() {
            state_guard = self
                .forward_pending_pictures(state_guard, true)
                .map_err(|err| {
                    gst::loggable_error!(CAT, "Failed to forward pending pictures: {}", err)
                })?;
        }
        *state_guard = None;
        drop(state_guard);

        self.parent_set_format(input_state)?;

        state_guard = self.state.lock().unwrap();

        let settings = self.settings.lock().unwrap();
        let mut decoder_settings = dav1d::Settings::new();
        let max_frame_delay: u32;
        let n_cpus = num_cpus::get();

        gst::info!(CAT, imp = self, "Detected {} logical CPUs", n_cpus);

        // For autodetection: 1 if live, else whatever dav1d gives us
        if settings.max_frame_delay == -1 {
            let mut latency_query = gst::query::Latency::new();
            let mut is_live = false;

            if self.obj().sink_pad().peer_query(&mut latency_query) {
                is_live = latency_query.result().0;
            }

            max_frame_delay = u32::from(is_live);
        } else {
            max_frame_delay = settings.max_frame_delay.try_into().unwrap();
        }

        gst::info!(
            CAT,
            imp = self,
            "Creating decoder with n-threads={} and max-frame-delay={}",
            settings.n_threads,
            max_frame_delay
        );
        decoder_settings.set_n_threads(settings.n_threads);
        decoder_settings.set_max_frame_delay(max_frame_delay);
        decoder_settings.set_apply_grain(settings.apply_grain);
        decoder_settings.set_inloop_filters(dav1d::InloopFilterType::from(settings.inloop_filters));

        let decoder =
            dav1d::Decoder::with_settings_and_allocator(&decoder_settings, self.obj().clone())
                .map_err(|err| {
                    gst::loggable_error!(CAT, "Failed to create decoder instance: {}", err)
                })?;

        *state_guard = Some(State {
            decoder: Some(decoder),
            configured_max_frame_delay: max_frame_delay,
            input_state: input_state.clone(),
            output_state: None,
            output_pool: None,
            video_meta_supported: false,
            n_cpus,
        });

        self.parent_set_format(input_state)
    }

    fn handle_frame(
        &self,
        frame: gst_video::VideoCodecFrame,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let input_buffer = frame
            .input_buffer_owned()
            .expect("frame without input buffer");

        let mut state_guard = self.state.lock().unwrap();
        let mut res;
        (state_guard, res) = self.send_data(state_guard, input_buffer, frame)?;

        if res == std::ops::ControlFlow::Continue(()) {
            loop {
                state_guard = self.forward_pending_pictures(state_guard, false)?;
                (state_guard, res) = self.send_pending_data(state_guard)?;
                if res == std::ops::ControlFlow::Break(()) {
                    break;
                }
            }
        }
        let _state_guard = self.forward_pending_pictures(state_guard, false)?;

        Ok(gst::FlowSuccess::Ok)
    }

    fn flush(&self) -> bool {
        gst::info!(CAT, imp = self, "Flushing");

        {
            let mut state_guard = self.state.lock().unwrap();
            if let Some(state) = &mut *state_guard {
                self.flush_decoder(state);
            }
        }

        true
    }

    fn drain(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::info!(CAT, imp = self, "Draining");

        {
            let state_guard = self.state.lock().unwrap();
            if state_guard.is_some() {
                let _state_guard = self.forward_pending_pictures(state_guard, true)?;
            }
        }

        self.parent_drain()
    }

    fn finish(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::info!(CAT, imp = self, "Finishing");

        {
            let state_guard = self.state.lock().unwrap();
            if state_guard.is_some() {
                let _state_guard = self.forward_pending_pictures(state_guard, true)?;
            }
        }

        self.parent_finish()
    }

    fn decide_allocation(
        &self,
        query: &mut gst::query::Allocation,
    ) -> Result<(), gst::LoggableError> {
        gst::debug!(CAT, imp = self, "Renegotiating allocation");

        {
            let mut state_guard = self.state.lock().unwrap();
            if let Some(state) = &mut *state_guard {
                state.video_meta_supported = false;
                state.output_pool = None;
            }
        }

        self.parent_decide_allocation(query)?;

        let video_meta_supported = query
            .find_allocation_meta::<gst_video::VideoMeta>()
            .is_some();

        let mut state_guard = self.state.lock().unwrap();
        let Some(state) = &mut *state_guard else {
            gst::trace!(CAT, imp = self, "Flushing");
            return Ok(());
        };

        state.video_meta_supported = video_meta_supported;

        let Some(output_state) = self.obj().output_state() else {
            gst::warning!(CAT, imp = self, "No output state set");
            return Ok(());
        };

        // Base class is adding one allocation param and pool at least
        let (allocator, mut params) = query.allocation_params().next().unwrap();
        params.set_align(cmp::max(params.align(), dav1d::PICTURE_ALIGNMENT - 1));
        params.set_padding(cmp::max(params.padding(), dav1d::PICTURE_ALIGNMENT));

        let (pool, size, min, max) = query.allocation_pools().next().unwrap();
        // Base class always sets one
        let pool = pool.unwrap();

        // FIXME: What's the minimum dav1d needs?
        if max != 0 && max < 32 {
            // Need to use a default pool
            gst::debug!(
                CAT,
                imp = self,
                "Negotiated pool limit too low ({max} < 32)"
            );
            return Ok(());
        }

        let mut config = pool.config();
        config.set_allocator(allocator.as_ref(), Some(&params));
        if video_meta_supported {
            config.add_option(gst_video::BUFFER_POOL_OPTION_VIDEO_META);
        }

        let video_alignment_supported =
            pool.has_option(gst_video::BUFFER_POOL_OPTION_VIDEO_ALIGNMENT);

        if !video_meta_supported || !video_alignment_supported {
            // Need to use a default pool
            gst::debug!(
                CAT,
                imp = self,
                "Video meta or alignment not supported by negotiated pool"
            );
            return Ok(());
        }

        let mut info = output_state.info().clone();
        let aligned_width = info.width().next_multiple_of(128);
        let aligned_height = info.height().next_multiple_of(128);

        let stride_align = [params.align() as u32; gst_video::VIDEO_MAX_PLANES];

        let mut align = gst_video::VideoAlignment::new(
            0,
            aligned_height - info.height(),
            aligned_width - info.width(),
            0,
            &stride_align,
        );

        config.add_option(gst_video::BUFFER_POOL_OPTION_VIDEO_ALIGNMENT);

        if info.align(&mut align).is_err() {
            // Need to use a default pool
            gst::debug!(CAT, imp = self, "Failed to align video info");
            return Ok(());
        }

        config.set_video_alignment(&align);

        config.set_params(
            output_state.caps_owned().as_ref(),
            cmp::max(size, info.size() as u32),
            min,
            max,
        );

        if pool.set_config(config.clone()).is_err() {
            let updated_config = pool.config();
            if updated_config
                .validate_params(output_state.caps_owned().as_ref(), size, min, max)
                .is_ok()
                && pool.set_config(updated_config).is_err()
            {
                // Need to use a default pool
                gst::debug!(
                    CAT,
                    imp = self,
                    "Configuration not accepted by negotiated pool"
                );
                return Ok(());
            }
        }

        state.output_pool = Some(pool);

        Ok(())
    }
}
