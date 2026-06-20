// Copyright (C) 2020 Arun Raghavan <arun@asymptotic.io>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use ffv1::constants::{RGB, YCBCR};
use ffv1::decoder::{Decoder, Frame};
use ffv1::record::ConfigRecord;

use gst::glib;
use gst::subclass::prelude::*;
use gst_video::prelude::*;
use gst_video::subclass::prelude::*;
use gst_video::VideoFormat;

use std::sync::LazyLock;
use std::sync::Mutex;

#[derive(Default)]
enum DecoderState {
    #[default]
    Stopped,
    Started {
        output_info: Option<gst_video::VideoInfo>,
        decoder: Box<Decoder>,
        video_meta_supported: bool,
    },
}

#[derive(Default)]
pub struct Ffv1Dec {
    state: Mutex<DecoderState>,
}

fn get_all_video_formats() -> impl IntoIterator<Item = gst_video::VideoFormat> {
    [
        VideoFormat::Gray8,
        VideoFormat::Gray16Le,
        VideoFormat::Gray16Be,
        VideoFormat::Y444,
        VideoFormat::Y44410le,
        VideoFormat::Y44410be,
        VideoFormat::A44410le,
        VideoFormat::A44410be,
        VideoFormat::Y44412le,
        VideoFormat::Y44412be,
        VideoFormat::Y44416le,
        VideoFormat::Y44416be,
        VideoFormat::A420,
        VideoFormat::Y42b,
        VideoFormat::I42210le,
        VideoFormat::I42210be,
        VideoFormat::A42210le,
        VideoFormat::A42210be,
        VideoFormat::I42212le,
        VideoFormat::I42212be,
        VideoFormat::I420,
        VideoFormat::I42010le,
        VideoFormat::I42010be,
        VideoFormat::I42012le,
        VideoFormat::I42012be,
        VideoFormat::Gbra,
        VideoFormat::Gbr,
        VideoFormat::Gbr10le,
        VideoFormat::Gbr10be,
        VideoFormat::Gbra10le,
        VideoFormat::Gbra10be,
        VideoFormat::Gbr12le,
        VideoFormat::Gbr12be,
        VideoFormat::Gbra12le,
        VideoFormat::Gbra12be,
        VideoFormat::Y41b,
        VideoFormat::Yuv9,
    ]
}

fn get_output_format(record: &ConfigRecord) -> Option<VideoFormat> {
    const IS_LITTLE_ENDIAN: bool = cfg!(target_endian = "little");

    match record.colorspace_type as usize {
        YCBCR => match (
            record.chroma_planes,
            record.log2_v_chroma_subsample,
            record.log2_h_chroma_subsample,
            record.bits_per_raw_sample,
            record.extra_plane,
            IS_LITTLE_ENDIAN,
        ) {
            // Interpret luma-only as grayscale
            (false, _, _, 8, false, _) => Some(VideoFormat::Gray8),
            (false, _, _, 16, false, true) => Some(VideoFormat::Gray16Le),
            (false, _, _, 16, false, false) => Some(VideoFormat::Gray16Be),
            // 4:4:4
            (true, 0, 0, 8, false, _) => Some(VideoFormat::Y444),
            (true, 0, 0, 10, false, true) => Some(VideoFormat::Y44410le),
            (true, 0, 0, 10, false, false) => Some(VideoFormat::Y44410be),
            (true, 0, 0, 10, true, true) => Some(VideoFormat::A44410le),
            (true, 0, 0, 10, true, false) => Some(VideoFormat::A44410be),
            (true, 0, 0, 12, false, true) => Some(VideoFormat::Y44412le),
            (true, 0, 0, 12, false, false) => Some(VideoFormat::Y44412be),
            (true, 0, 0, 16, false, true) => Some(VideoFormat::Y44416le),
            (true, 0, 0, 16, false, false) => Some(VideoFormat::Y44416be),
            // 4:2:2
            (true, 0, 1, 8, false, _) => Some(VideoFormat::Y42b),
            (true, 0, 1, 10, false, true) => Some(VideoFormat::I42210le),
            (true, 0, 1, 10, false, false) => Some(VideoFormat::I42210be),
            (true, 0, 1, 10, true, true) => Some(VideoFormat::A42210le),
            (true, 0, 1, 10, true, false) => Some(VideoFormat::A42210be),
            (true, 0, 1, 12, false, true) => Some(VideoFormat::I42212le),
            (true, 0, 1, 12, false, false) => Some(VideoFormat::I42212be),
            // 4:1:1
            (true, 0, 2, 8, false, _) => Some(VideoFormat::Y41b),
            // 4:2:0
            (true, 1, 1, 8, false, _) => Some(VideoFormat::I420),
            (true, 1, 1, 8, true, _) => Some(VideoFormat::A420),
            (true, 1, 1, 10, false, true) => Some(VideoFormat::I42010le),
            (true, 1, 1, 10, false, false) => Some(VideoFormat::I42010be),
            (true, 1, 1, 12, false, true) => Some(VideoFormat::I42012le),
            (true, 1, 1, 12, false, false) => Some(VideoFormat::I42012be),
            // 4:1:0
            (true, 2, 2, 8, false, _) => Some(VideoFormat::Yuv9),
            // Nothing matched
            (_, _, _, _, _, _) => None,
        },
        RGB => match (
            record.bits_per_raw_sample,
            record.extra_plane,
            IS_LITTLE_ENDIAN,
        ) {
            (8, true, _) => Some(VideoFormat::Gbra),
            (8, false, _) => Some(VideoFormat::Gbr),
            (10, false, true) => Some(VideoFormat::Gbr10le),
            (10, false, false) => Some(VideoFormat::Gbr10be),
            (10, true, true) => Some(VideoFormat::Gbra10le),
            (10, true, false) => Some(VideoFormat::Gbra10be),
            (12, false, true) => Some(VideoFormat::Gbr12le),
            (12, false, false) => Some(VideoFormat::Gbr12be),
            (12, true, true) => Some(VideoFormat::Gbra12le),
            (12, true, false) => Some(VideoFormat::Gbra12be),
            // Nothing matched
            (_, _, _) => None,
        },
        _ => panic!("Unknown color_space type"),
    }
}

impl Ffv1Dec {
    pub fn get_decoded_frame(
        &self,
        mut decoded_frame: Frame,
        output_info: &gst_video::VideoInfo,
        video_meta_supported: bool,
    ) -> gst::Buffer {
        let mut buf = gst::Buffer::new();
        let mut_buf = buf.make_mut();
        let format_info = output_info.format_info();

        let mut offsets = vec![];
        let mut strides = vec![];
        let mut acc_offset = 0;

        if decoded_frame.bit_depth == 8 {
            for (plane, decoded_plane) in decoded_frame.buf.drain(..).enumerate() {
                let component = format_info
                    .plane()
                    .iter()
                    .position(|&p| p == plane as u32)
                    .unwrap() as u8;

                let comp_height =
                    format_info.scale_height(component, output_info.height()) as usize;
                let src_stride = decoded_plane.len() / comp_height;
                let dest_stride = output_info.stride()[plane] as usize;

                let mem = if video_meta_supported || src_stride == dest_stride {
                    // Just wrap the decoded frame vecs and push them out
                    gst::Memory::from_mut_slice(decoded_plane)
                } else {
                    // Mismatched stride, let's copy
                    let out_plane = gst::Memory::with_size(dest_stride * comp_height);
                    let mut out_plane_mut = out_plane.into_mapped_memory_writable().unwrap();

                    for (in_line, out_line) in decoded_plane
                        .as_slice()
                        .chunks_exact(src_stride)
                        .zip(out_plane_mut.as_mut_slice().chunks_exact_mut(dest_stride))
                    {
                        out_line[..src_stride].copy_from_slice(in_line);
                    }

                    out_plane_mut.into_memory()
                };

                let mem_size = mem.size();
                mut_buf.append_memory(mem);

                strides.push(src_stride as i32);
                offsets.push(acc_offset);
                acc_offset += mem_size;
            }
        } else if decoded_frame.bit_depth <= 16 {
            use byte_slice_cast::{AsByteSlice, AsMutByteSlice};

            for (plane, decoded_plane) in decoded_frame.buf16.drain(..).enumerate() {
                let component = format_info
                    .plane()
                    .iter()
                    .position(|&p| p == plane as u32)
                    .unwrap() as u8;

                let comp_height =
                    format_info.scale_height(component, output_info.height()) as usize;
                let src_stride = (decoded_plane.len() * 2) / comp_height;
                let dest_stride = output_info.stride()[plane] as usize;

                let mem = if video_meta_supported || src_stride == dest_stride {
                    struct WrappedVec16(Vec<u16>);
                    impl AsRef<[u8]> for WrappedVec16 {
                        fn as_ref(&self) -> &[u8] {
                            self.0.as_byte_slice()
                        }
                    }
                    impl AsMut<[u8]> for WrappedVec16 {
                        fn as_mut(&mut self) -> &mut [u8] {
                            self.0.as_mut_byte_slice()
                        }
                    }
                    // Just wrap the decoded frame vecs and push them out
                    gst::Memory::from_mut_slice(WrappedVec16(decoded_plane))
                } else {
                    // Mismatched stride, let's copy
                    let out_plane = gst::Memory::with_size(dest_stride * comp_height);
                    let mut out_plane_mut = out_plane.into_mapped_memory_writable().unwrap();

                    for (in_line, out_line) in decoded_plane
                        .as_slice()
                        .as_byte_slice()
                        .chunks_exact(src_stride)
                        .zip(out_plane_mut.as_mut_slice().chunks_exact_mut(dest_stride))
                    {
                        out_line[..src_stride].copy_from_slice(in_line);
                    }

                    out_plane_mut.into_memory()
                };

                let mem_size = mem.size();
                mut_buf.append_memory(mem);

                strides.push(src_stride as i32);
                offsets.push(acc_offset);
                acc_offset += mem_size;
            }
        } else {
            unimplemented!("Bit depth {} not supported yet", decoded_frame.bit_depth);
        }

        if video_meta_supported {
            gst_video::VideoMeta::add_full(
                buf.get_mut().unwrap(),
                gst_video::VideoFrameFlags::empty(),
                output_info.format(),
                output_info.width(),
                output_info.height(),
                &offsets,
                &strides[..],
            )
            .unwrap();
        }

        buf
    }
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "ffv1dec",
        gst::DebugColorFlags::empty(),
        Some("FFV1 decoder"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for Ffv1Dec {
    const NAME: &'static str = "GstFfv1Dec";
    type Type = super::Ffv1Dec;
    type ParentType = gst_video::VideoDecoder;
}

impl ObjectImpl for Ffv1Dec {}

impl GstObjectImpl for Ffv1Dec {}

impl ElementImpl for Ffv1Dec {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "FFV1 Decoder",
                "Codec/Decoder/Video",
                "Decode FFV1 video streams",
                "Arun Raghavan <arun@asymptotic.io>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let sink_caps = gst::Caps::builder("video/x-ffv")
                .field("ffvversion", 1)
                .field("width", gst::IntRange::new(1, i32::MAX))
                .field("height", gst::IntRange::new(1, i32::MAX))
                .field(
                    "framerate",
                    gst::FractionRange::new(
                        gst::Fraction::new(0, 1),
                        gst::Fraction::new(i32::MAX, 1),
                    ),
                )
                .build();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &sink_caps,
            )
            .unwrap();

            let src_caps = gst_video::VideoCapsBuilder::new()
                .format_list(get_all_video_formats())
                .build();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &src_caps,
            )
            .unwrap();

            vec![sink_pad_template, src_pad_template]
        });

        &PAD_TEMPLATES
    }
}

impl VideoDecoderImpl for Ffv1Dec {
    /* We allocate the decoder here rather than start() because we need the sink caps */
    fn set_format(
        &self,
        state: &gst_video::VideoCodecState<'static, gst_video::video_codec_state::Readable>,
    ) -> Result<(), gst::LoggableError> {
        let info = state.info();
        let codec_data = state
            .codec_data()
            .ok_or_else(|| gst::loggable_error!(CAT, "Missing codec_data"))?
            .map_readable()?;
        let decoder = Decoder::new(codec_data.as_slice(), info.width(), info.height())
            .map_err(|err| gst::loggable_error!(CAT, "Could not instantiate decoder: {}", err))?;

        let format = get_output_format(decoder.config_record())
            .ok_or_else(|| gst::loggable_error!(CAT, "Unsupported format"))?;

        let instance = self.obj();
        let output_state = instance
            .set_output_state(format, info.width(), info.height(), Some(state))
            .map_err(|err| gst::loggable_error!(CAT, "Failed to set output params: {}", err))?;

        let output_info = Some(output_state.info().clone());

        let mut decoder_state = self.state.lock().unwrap();
        *decoder_state = DecoderState::Started {
            output_info,
            decoder: Box::new(decoder),
            video_meta_supported: false,
        };
        drop(decoder_state);

        instance
            .negotiate(output_state)
            .map_err(|err| gst::loggable_error!(CAT, "Negotiation failed: {}", err))?;

        self.parent_set_format(state)
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        let mut decoder_state = self.state.lock().unwrap();
        *decoder_state = DecoderState::Stopped;

        self.parent_stop()
    }

    fn handle_frame(
        &self,
        mut frame: gst_video::VideoCodecFrame,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.lock().unwrap();

        let (output_info, decoder, video_meta_supported) = match *state {
            DecoderState::Started {
                output_info: Some(ref output_info),
                ref mut decoder,
                video_meta_supported,
                ..
            } => Ok((output_info, decoder, video_meta_supported)),
            _ => Err(gst::FlowError::Error),
        }?;

        let input_buffer = frame
            .input_buffer()
            .expect("Frame must have input buffer")
            .map_readable()
            .expect("Could not map input buffer for read");

        let decoded_frame = decoder.decode_frame(input_buffer.as_slice()).map_err(|e| {
            gst::error!(CAT, "Decoding failed: {}", e);
            gst::FlowError::Error
        })?;

        // Drop so we can mutably borrow frame later
        drop(input_buffer);

        //  * Make sure the decoder and output plane orders match for all cases
        let buf = self.get_decoded_frame(decoded_frame, output_info, video_meta_supported);

        // We no longer need the state lock
        drop(state);

        frame.set_output_buffer(buf);
        self.obj().finish_frame(frame)?;

        Ok(gst::FlowSuccess::Ok)
    }

    fn decide_allocation(
        &self,
        query: &mut gst::query::Allocation,
    ) -> Result<(), gst::LoggableError> {
        let supported = query
            .find_allocation_meta::<gst_video::VideoMeta>()
            .is_some();

        let mut state = self.state.lock().unwrap();
        if let DecoderState::Started {
            ref mut video_meta_supported,
            ..
        } = *state
        {
            *video_meta_supported = supported;
        }

        self.parent_decide_allocation(query)
    }
}
