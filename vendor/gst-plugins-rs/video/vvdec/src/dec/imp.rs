// Copyright (C) 2025 Carlos Bentzen <cadubentzen@igalia.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-vvdec
 *
 * #vvdec is a decoder element for VVC/H.266 video streams using the VVdeC decoder.
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch filesrc location=vvc.mp4 ! qtdemux ! h266parse ! vvdec ! videoconvert ! autovideosink
 * ]|
 *
 * Since: plugins-rs-0.14.0
 */
use std::sync::{Mutex, MutexGuard};

use gst::glib::{self, Properties};
use gst::subclass::prelude::*;
use gst_video::prelude::*;
use gst_video::subclass::prelude::*;
use std::sync::LazyLock;
use vvdec::AccessUnit;

const DEFAULT_N_THREADS: i32 = -1;
const DEFAULT_N_PARSER_THREADS: i32 = -1;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "vvdec",
        gst::DebugColorFlags::empty(),
        Some("VVdeC VVC/H.266 decoder"),
    )
});

struct State {
    decoder: vvdec::Decoder,
    video_meta_supported: bool,
    output_info: Option<gst_video::VideoInfo>,
    input_state: gst_video::VideoCodecState<'static, gst_video::video_codec_state::Readable>,
}

#[derive(Debug)]
struct Settings {
    n_threads: i32,
    parse_delay: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            n_threads: DEFAULT_N_THREADS,
            parse_delay: DEFAULT_N_PARSER_THREADS,
        }
    }
}

impl Settings {
    fn create_decoder(&self) -> Result<vvdec::Decoder, vvdec::Error> {
        vvdec::Decoder::builder()
            .num_threads(self.n_threads)
            .parse_delay(self.parse_delay)
            .build()
    }
}

#[derive(Default, Properties)]
#[properties(wrapper_type = super::VVdeC)]
pub struct VVdeC {
    state: Mutex<Option<State>>,
    #[property(name = "n-threads", get, set, type = i32, member = n_threads, minimum = -1, default = DEFAULT_N_THREADS, blurb = "Number of threads to use while decoding (set to -1 to use the number of logical cores, set to 0 to decode in a single thread)")]
    #[property(name = "n-parser-threads", get, set, type = i32, member = parse_delay, minimum = -1, default = DEFAULT_N_PARSER_THREADS, blurb = "Number of parser threads to use while decoding (set to -1 to let libvvdec choose the number of parser threads, set to 0 to parse on the element streaming thread)")]
    settings: Mutex<Settings>,
}

type StateGuard<'s> = MutexGuard<'s, Option<State>>;

impl VVdeC {
    fn create_decoder(&self) -> Result<vvdec::Decoder, vvdec::Error> {
        self.settings.lock().unwrap().create_decoder()
    }

    fn decode<'s>(
        &'s self,
        mut state_guard: StateGuard<'s>,
        input_buffer: gst::Buffer,
        frame: gst_video::VideoCodecFrame,
    ) -> Result<(), gst::FlowError> {
        let state = state_guard.as_mut().ok_or(gst::FlowError::Flushing)?;

        // VVdeC doesn't have a mechanism for passing opaque data per-frame.
        // But we can store system_frame_number via the cts field, since it's not
        // used in the decoder but the value is returned in the matching decoded frames.
        let cts = frame.system_frame_number() as u64;
        let dts = input_buffer.dts().map(|ts| *ts);
        let is_random_access_point = frame
            .flags()
            .contains(gst_video::VideoCodecFrameFlags::SYNC_POINT);
        let payload = input_buffer
            .into_mapped_buffer_readable()
            .map_err(|_| gst::FlowError::Error)?;

        let access_unit = AccessUnit {
            payload,
            cts: Some(cts),
            dts,
            is_random_access_point,
        };

        match state.decoder.decode(access_unit) {
            Ok(Some(frame)) => {
                drop(self.handle_decoded_frame(state_guard, &frame)?);
            }
            Ok(None) => (),
            Err(vvdec::Error::TryAgain) => {
                gst::trace!(CAT, imp = self, "Decoder returned TryAgain");
            }
            Err(vvdec::Error::DecInput) => {
                gst_video::video_decoder_error!(
                    &*self.obj(),
                    1,
                    gst::StreamError::Decode,
                    ["Discarding frame {} because it's undecodable", cts]
                )?;
                drop(state_guard);
                self.obj().release_frame(frame);
            }
            Err(vvdec::Error::RestartRequired) => {
                gst::warning!(CAT, imp = self, "decoder requires restart");
                state_guard = self.forward_pending_frames(state_guard)?;
                let state = state_guard.as_mut().ok_or(gst::FlowError::Flushing)?;
                state.decoder = self.create_decoder().map_err(|_| {
                    gst::error!(CAT, imp = self, "Failed to create new decoder instance");
                    gst::FlowError::Error
                })?;
            }
            Err(err) => {
                gst::error!(CAT, imp = self, "decoder returned error: {err}");
                return Err(gst::FlowError::Error);
            }
        }

        Ok(())
    }

    fn handle_decoded_frame<'s>(
        &'s self,
        state_guard: StateGuard<'s>,
        decoded_frame: &vvdec::Frame,
    ) -> Result<StateGuard<'s>, gst::FlowError> {
        let system_frame_number = decoded_frame.cts().expect("frames were pushed with cts");
        gst::trace!(
            CAT,
            imp = self,
            "Handling decoded frame {system_frame_number}"
        );

        let state_guard = self.handle_resolution_changes(state_guard, decoded_frame)?;
        let instance = self.obj();

        let frame = instance.frame(system_frame_number.try_into().unwrap());
        if let Some(mut frame) = frame {
            self.set_decoded_frame_as_output_buffer(state_guard, decoded_frame, &mut frame)?;
            instance.finish_frame(frame)?;
            gst::trace!(CAT, imp = self, "Finished frame {system_frame_number}");
            Ok(self.state.lock().unwrap())
        } else {
            gst::warning!(
                CAT,
                imp = self,
                "No frame found for system frame number {system_frame_number}"
            );
            Ok(state_guard)
        }
    }

    fn handle_resolution_changes<'s>(
        &'s self,
        mut state_guard: StateGuard<'s>,
        frame: &vvdec::Frame,
    ) -> Result<StateGuard<'s>, gst::FlowError> {
        let format = self.gst_video_format_from_vvdec_frame(frame);
        if format == gst_video::VideoFormat::Unknown {
            return Err(gst::FlowError::NotNegotiated);
        }

        let state = state_guard.as_mut().ok_or(gst::FlowError::Flushing)?;
        let need_negotiate = {
            match state.output_info {
                Some(ref i) => {
                    (i.width() != frame.width())
                        || (i.height() != frame.height() || (i.format() != format))
                }
                None => true,
            }
        };
        if !need_negotiate {
            return Ok(state_guard);
        }

        gst::info!(
            CAT,
            imp = self,
            "Negotiating format {} frame dimensions {}x{}",
            format,
            frame.width(),
            frame.height()
        );

        let input_state = state.input_state.clone();
        drop(state_guard);

        let instance = self.obj();

        let interlace_mode = match frame.frame_format() {
            vvdec::FrameFormat::TopBottom | vvdec::FrameFormat::BottomTop => {
                gst_video::VideoInterlaceMode::Interleaved
            }
            vvdec::FrameFormat::TopField | vvdec::FrameFormat::BottomField => {
                gst_video::VideoInterlaceMode::Alternate
            }
            vvdec::FrameFormat::Progressive => gst_video::VideoInterlaceMode::Progressive,
            _ => {
                gst::error!(
                    CAT,
                    imp = self,
                    "Unsupported VVdeC frame format {:?}",
                    frame.frame_format()
                );
                return Err(gst::FlowError::NotNegotiated);
            }
        };

        let mut output_state_height = frame.height();
        if interlace_mode == gst_video::VideoInterlaceMode::Alternate {
            output_state_height *= 2;
        }

        let output_state = instance.set_interlaced_output_state(
            format,
            interlace_mode,
            frame.width(),
            output_state_height,
            Some(&input_state),
        )?;
        instance.negotiate(output_state)?;
        let out_state = instance.output_state().unwrap();

        let mut state_guard = self.state.lock().unwrap();
        let state = state_guard.as_mut().ok_or(gst::FlowError::Flushing)?;
        state.output_info = Some(out_state.info().clone());

        gst::trace!(CAT, imp = self, "Negotiated format");

        Ok(state_guard)
    }

    fn gst_video_format_from_vvdec_frame(&self, frame: &vvdec::Frame) -> gst_video::VideoFormat {
        let color_format = frame.color_format();
        let bit_depth = frame.bit_depth();

        match (&color_format, bit_depth) {
            (vvdec::ColorFormat::Yuv400Planar, 8) => gst_video::VideoFormat::Gray8,
            (vvdec::ColorFormat::Yuv420Planar, 8) => gst_video::VideoFormat::I420,
            (vvdec::ColorFormat::Yuv422Planar, 8) => gst_video::VideoFormat::Y42b,
            (vvdec::ColorFormat::Yuv444Planar, 8) => gst_video::VideoFormat::Y444,
            #[cfg(target_endian = "little")]
            (vvdec::ColorFormat::Yuv400Planar, 10) => gst_video::VideoFormat::Gray10Le16,
            #[cfg(target_endian = "little")]
            (vvdec::ColorFormat::Yuv420Planar, 10) => gst_video::VideoFormat::I42010le,
            #[cfg(target_endian = "little")]
            (vvdec::ColorFormat::Yuv422Planar, 10) => gst_video::VideoFormat::I42210le,
            #[cfg(target_endian = "little")]
            (vvdec::ColorFormat::Yuv444Planar, 10) => gst_video::VideoFormat::Y44410le,
            #[cfg(target_endian = "big")]
            (vvdec::ColorFormat::Yuv420Planar, 10) => gst_video::VideoFormat::I42010be,
            #[cfg(target_endian = "big")]
            (vvdec::ColorFormat::Yuv422Planar, 10) => gst_video::VideoFormat::I42210be,
            #[cfg(target_endian = "big")]
            (vvdec::ColorFormat::Yuv444Planar, 10) => gst_video::VideoFormat::Y44410be,
            _ => {
                gst::warning!(
                    CAT,
                    imp = self,
                    "Unsupported VVdeC format {:?}/{:?}",
                    color_format,
                    bit_depth
                );
                gst_video::VideoFormat::Unknown
            }
        }
    }

    fn forward_pending_frames<'s>(
        &'s self,
        mut state_guard: StateGuard<'s>,
    ) -> Result<StateGuard<'s>, gst::FlowError> {
        loop {
            let state = state_guard.as_mut().ok_or(gst::FlowError::Flushing)?;
            match state.decoder.flush() {
                Ok(Some(frame)) => {
                    gst::trace!(CAT, imp = self, "Forwarding pending frame.");
                    state_guard = self.handle_decoded_frame(state_guard, &frame)?;
                }
                Ok(None) | Err(vvdec::Error::RestartRequired) => break,
                Err(err) => {
                    gst::error!(
                        CAT,
                        imp = self,
                        "Decoder returned error while flushing: {err}"
                    );
                    return Err(gst::FlowError::Error);
                }
            };
        }
        Ok(state_guard)
    }

    fn flush_decoder(&self, state: &mut State) -> bool {
        loop {
            match state.decoder.flush() {
                Ok(Some(_)) => continue,
                Ok(None) | Err(vvdec::Error::RestartRequired) | Err(vvdec::Error::Eof) => break,
                Err(err) => {
                    gst::error!(
                        CAT,
                        imp = self,
                        "Decoder returned error while flushing: {err}"
                    );
                    return false;
                }
            }
        }
        true
    }

    fn set_decoded_frame_as_output_buffer<'s>(
        &'s self,
        mut state_guard: StateGuard<'s>,
        frame: &vvdec::Frame,
        codec_frame: &mut gst_video::VideoCodecFrame,
    ) -> Result<(), gst::FlowError> {
        let state = state_guard.as_mut().expect("state is set");
        let video_meta_supported = state.video_meta_supported;

        let info = state
            .output_info
            .as_ref()
            .expect("output_info is set")
            .clone();
        let color_format = frame.color_format();
        if color_format == vvdec::ColorFormat::Invalid {
            gst::error!(CAT, imp = self, "Invalid color format");
            return Err(gst::FlowError::Error);
        }

        let components = if color_format == vvdec::ColorFormat::Yuv400Planar {
            const GRAY_COMPONENTS: [vvdec::PlaneComponent; 1] = [vvdec::PlaneComponent::Y];
            &GRAY_COMPONENTS[..]
        } else {
            const YUV_COMPONENTS: [vvdec::PlaneComponent; 3] = [
                vvdec::PlaneComponent::Y,
                vvdec::PlaneComponent::U,
                vvdec::PlaneComponent::V,
            ];
            &YUV_COMPONENTS[..]
        };

        let mut offsets = [0; 4];
        let mut strides = [0; 4];
        let mut acc_offset = 0usize;

        for (idx, component) in components.iter().enumerate() {
            let plane = frame
                .plane(*component)
                .expect("frame contains the requested plane");
            let src_stride = plane.stride();
            let mem_size = plane.len();

            strides[idx] = src_stride as i32;
            offsets[idx] = acc_offset;
            acc_offset += mem_size;
        }

        let strides = &strides[..components.len()];
        let offsets = &offsets[..components.len()];

        let strides_matching = info.offset() == offsets && info.stride() == strides;

        // We can forward decoded memory as-is if downstream supports the video meta
        // or the strides/offsets are matching with the default ones.
        let forward_memory = video_meta_supported || strides_matching;

        drop(state_guard);

        let mut out_buffer = forward_memory.then(gst::Buffer::new);
        let mut_buffer = if forward_memory {
            out_buffer.as_mut().unwrap().get_mut().unwrap()
        } else {
            self.obj().allocate_output_frame(codec_frame, None)?;
            codec_frame
                .output_buffer_mut()
                .expect("output_buffer is set")
        };

        let frame_format = frame.frame_format();
        let video_flags = match frame_format {
            vvdec::FrameFormat::Progressive => None,
            vvdec::FrameFormat::TopField => Some(
                gst_video::VideoBufferFlags::INTERLACED | gst_video::VideoBufferFlags::TOP_FIELD,
            ),
            vvdec::FrameFormat::BottomField => Some(
                gst_video::VideoBufferFlags::INTERLACED | gst_video::VideoBufferFlags::BOTTOM_FIELD,
            ),
            vvdec::FrameFormat::TopBottom => {
                Some(gst_video::VideoBufferFlags::INTERLACED | gst_video::VideoBufferFlags::TFF)
            }
            vvdec::FrameFormat::BottomTop => Some(gst_video::VideoBufferFlags::INTERLACED),
            _ => {
                gst::error!(
                    CAT,
                    imp = self,
                    "Unsupported VVdeC frame format {:?}",
                    frame_format
                );
                return Err(gst::FlowError::Error);
            }
        };
        if let Some(video_flags) = video_flags {
            mut_buffer.set_video_flags(video_flags);
        }

        if forward_memory {
            assert!(mut_buffer.size() == 0);

            for component in components {
                let plane = frame
                    .plane(*component)
                    .expect("frame contains the requested plane");
                let mem = gst::Memory::from_slice(plane);
                mut_buffer.append_memory(mem);
            }

            let frame_flags = match frame_format {
                vvdec::FrameFormat::Progressive => gst_video::VideoFrameFlags::empty(),
                vvdec::FrameFormat::TopField => {
                    gst_video::VideoFrameFlags::INTERLACED | gst_video::VideoFrameFlags::TOP_FIELD
                }
                vvdec::FrameFormat::BottomField => {
                    gst_video::VideoFrameFlags::INTERLACED
                        | gst_video::VideoFrameFlags::BOTTOM_FIELD
                }
                vvdec::FrameFormat::TopBottom => {
                    gst_video::VideoFrameFlags::INTERLACED | gst_video::VideoFrameFlags::TFF
                }
                vvdec::FrameFormat::BottomTop => gst_video::VideoFrameFlags::INTERLACED,
                _ => unreachable!("frame_format is checked above"),
            };

            if video_meta_supported {
                gst_video::VideoMeta::add_full(
                    mut_buffer,
                    frame_flags,
                    info.format(),
                    info.width(),
                    info.height(),
                    offsets,
                    strides,
                )
                .unwrap();
            }

            assert!(codec_frame.output_buffer().is_none());
            codec_frame.set_output_buffer(out_buffer.expect("out_buffer is set"));
        } else {
            gst::trace!(
                gst::CAT_PERFORMANCE,
                imp = self,
                "Copying decoded video frame to output",
            );

            assert!(mut_buffer.size() > 0);
            let mut vframe = gst_video::VideoFrameRef::from_buffer_ref_writable(mut_buffer, &info)
                .expect("can map writable frame");
            for component in components {
                let dest_stride = vframe.plane_stride()[*component as usize] as u32;
                let dest_height = vframe.plane_height(*component as u32);
                let dest_plane_data = vframe
                    .plane_data_mut(*component as u32)
                    .expect("can get plane data");
                let plane = frame
                    .plane(*component)
                    .expect("frame contains the requested plane");
                let src_stride = plane.stride();
                let src_slice = plane.as_ref();
                let src_height = plane.height();
                let chunk_len = std::cmp::min(src_stride, dest_stride) as usize;

                if src_stride == dest_stride && src_height == dest_height {
                    dest_plane_data.copy_from_slice(src_slice);
                } else {
                    for (out_line, in_line) in dest_plane_data
                        .chunks_exact_mut(dest_stride as usize)
                        .zip(src_slice.chunks_exact(src_stride as usize))
                    {
                        out_line.copy_from_slice(&in_line[..chunk_len]);
                    }
                }
            }
        }

        Ok(())
    }
}

fn video_output_formats() -> impl IntoIterator<Item = gst_video::VideoFormat> {
    [
        gst_video::VideoFormat::Gray8,
        gst_video::VideoFormat::I420,
        gst_video::VideoFormat::Y42b,
        gst_video::VideoFormat::Y444,
        #[cfg(target_endian = "little")]
        gst_video::VideoFormat::Gray10Le16,
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
    ]
}

#[glib::object_subclass]
impl ObjectSubclass for VVdeC {
    const NAME: &'static str = "GstVVdeC";
    type Type = super::VVdeC;
    type ParentType = gst_video::VideoDecoder;
}

#[glib::derived_properties]
impl ObjectImpl for VVdeC {}

impl GstObjectImpl for VVdeC {}

impl ElementImpl for VVdeC {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "VVdeC VVC/H.266 Decoder",
                "Codec/Decoder/Video",
                "Decode VVC/H.266 video streams with VVdeC",
                "Carlos Bentzen <cadubentzen@igalia.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let sink_caps = gst::Caps::builder("video/x-h266")
                .field("stream-format", "byte-stream")
                .field("alignment", "au")
                .build();
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

impl VideoDecoderImpl for VVdeC {
    fn set_format(
        &self,
        input_state: &gst_video::VideoCodecState<'static, gst_video::video_codec_state::Readable>,
    ) -> Result<(), gst::LoggableError> {
        let mut state_guard = self.state.lock().unwrap();
        let settings = self.settings.lock().unwrap();

        if state_guard.is_some() {
            state_guard = self
                .forward_pending_frames(state_guard)
                .map_err(|_| gst::loggable_error!(CAT, "Failed to forward pending frames"))?;
        }

        gst::trace!(CAT, imp = self, "Creating decoder with {settings:?}");
        let decoder = settings
            .create_decoder()
            .map_err(|_| gst::loggable_error!(CAT, "Failed to create decoder instance"))?;

        *state_guard = Some(State {
            decoder,
            video_meta_supported: false,
            output_info: None,
            input_state: input_state.clone(),
        });

        self.parent_set_format(input_state)
    }

    fn handle_frame(
        &self,
        frame: gst_video::VideoCodecFrame,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let system_frame_number = frame.system_frame_number();
        gst::trace!(CAT, imp = self, "Decoding frame {}", system_frame_number);

        let input_buffer = frame.input_buffer_owned().expect("frame has input buffer");
        {
            let state_guard = self.state.lock().unwrap();
            self.decode(state_guard, input_buffer, frame)?;
        }

        Ok(gst::FlowSuccess::Ok)
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        gst::info!(CAT, imp = self, "Stopping");

        {
            let mut state_guard = self.state.lock().unwrap();
            *state_guard = None;
        }

        self.parent_stop()
    }

    fn drain(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::info!(CAT, imp = self, "Draining");

        {
            let mut state_guard = self.state.lock().unwrap();
            if state_guard.as_mut().is_some() {
                drop(self.forward_pending_frames(state_guard)?);
            }
        }

        self.parent_drain()
    }

    fn finish(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::info!(CAT, imp = self, "Finishing");

        {
            let mut state_guard = self.state.lock().unwrap();
            if state_guard.as_mut().is_some() {
                drop(self.forward_pending_frames(state_guard)?);
            }
        }

        self.parent_finish()
    }

    fn flush(&self) -> bool {
        gst::info!(CAT, imp = self, "Flushing");

        {
            let mut state_guard = self.state.lock().unwrap();
            if let Some(state) = state_guard.as_mut() {
                return self.flush_decoder(state);
            }
        }

        true
    }

    fn decide_allocation(
        &self,
        query: &mut gst::query::Allocation,
    ) -> Result<(), gst::LoggableError> {
        gst::trace!(CAT, imp = self, "Deciding allocation");
        self.parent_decide_allocation(query)?;

        let mut state_guard = self.state.lock().unwrap();
        if let Some(state) = state_guard.as_mut() {
            state.video_meta_supported = query
                .find_allocation_meta::<gst_video::VideoMeta>()
                .is_some();
            gst::info!(
                CAT,
                imp = self,
                "Video meta support: {}",
                state.video_meta_supported
            );
        };

        Ok(())
    }
}
