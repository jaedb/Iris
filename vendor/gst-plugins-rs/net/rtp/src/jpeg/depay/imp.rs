//
// Copyright (C) 2023 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::{collections::BTreeMap, io, mem};

use atomic_refcell::AtomicRefCell;
use bitstream_io::{BigEndian, ByteRead, ByteReader, ByteWrite as _, ByteWriter};
/**
 * SECTION:element-rtpjpegdepay2
 * @see_also: rtpjpegpay2, jpegdec, jpegenc
 *
 * Extracts a JPEG video stream from RTP packets as per [RFC 2435][rfc-2435].
 *
 * [rfc-2435]: https://www.rfc-editor.org/rfc/rfc2435.html
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 udpsrc caps='application/x-rtp, media=video, clock-rate=90000' ! rtpjitterbuffer latency=50 ! rtpjpegdepay2 ! jpegdec ! videoconvert ! autovideosink
 * ]| This will depayload an incoming RTP JPEG video stream. You can use the #jpegenc and
 * #rtpjpegpay2 elements to create such an RTP stream.
 *
 * Since: plugins-rs-0.13.0
 */
use gst::{glib, prelude::*, subclass::prelude::*};

use std::sync::LazyLock;

use crate::{
    basedepay::{RtpBaseDepay2Ext, RtpBaseDepay2Impl},
    jpeg::header::{
        make_quant_tables, JpegHeader, MainHeader, QuantizationTableHeader, RestartHeader,
    },
};

struct PendingFrame {
    /// RTP main header from the first fragment.
    main_header: MainHeader,
    /// Pending JPEG data.
    ///
    /// Already contains the JPEG headers.
    data: Vec<u8>,
    /// Expected next fragment offset.
    expected_fragment_offset: u32,
    /// Start extended seqnum.
    start_ext_seqnum: u64,
}

#[derive(Default)]
struct State {
    /// Resolution from `x-dimensions` attribute
    sdp_dimensions: Option<(u16, u16)>,
    /// Framerate from `a-framerate` / `x-framerate`
    sdp_framerate: Option<gst::Fraction>,
    /// Last configured width/height
    dimensions: Option<(i32, i32)>,
    /// Last configured framerate
    framerate: Option<gst::Fraction>,

    /// Currently pending frame, if any.
    pending_frame: Option<PendingFrame>,

    /// Cache quantization tables.
    quant_tables: BTreeMap<u8, QuantizationTableHeader>,
}

#[derive(Default)]
pub struct RtpJpegDepay {
    state: AtomicRefCell<State>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpjpegdepay2",
        gst::DebugColorFlags::empty(),
        Some("RTP JPEG Depayloader"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for RtpJpegDepay {
    const NAME: &'static str = "GstRtpJpegDepay2";
    type Type = super::RtpJpegDepay;
    type ParentType = crate::basedepay::RtpBaseDepay2;
}

impl ObjectImpl for RtpJpegDepay {}

impl GstObjectImpl for RtpJpegDepay {}

impl ElementImpl for RtpJpegDepay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP JPEG Depayloader",
                "Codec/Depayloader/Network/RTP",
                "Depayload a JPEG Video stream from RTP packets (RFC 2435)",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &gst::Caps::builder_full()
                    .structure(
                        gst::Structure::builder("application/x-rtp")
                            .field("media", "video")
                            .field("payload", 26i32)
                            .field("clock-rate", 90_000i32)
                            .build(),
                    )
                    .structure(
                        gst::Structure::builder("application/x-rtp")
                            .field("media", "video")
                            .field("encoding-name", "JPEG")
                            .field("clock-rate", 90_000i32)
                            .build(),
                    )
                    .build(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("image/jpeg").build(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}
impl RtpBaseDepay2Impl for RtpJpegDepay {
    const ALLOWED_META_TAGS: &'static [&'static str] = &["video"];

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.borrow_mut();
        *state = State::default();

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.borrow_mut();
        *state = State::default();

        Ok(())
    }

    fn set_sink_caps(&self, caps: &gst::Caps) -> bool {
        let s = caps.structure(0).unwrap();

        let mut state = self.state.borrow_mut();
        state.sdp_framerate = None;
        state.sdp_dimensions = None;

        if let Ok(dimensions_str) = s.get::<&str>("x-dimensions") {
            let dimensions = dimensions_str.split_once(',').and_then(|(width, height)| {
                Some((
                    width.trim().parse::<u16>().ok()?,
                    height.trim().parse::<u16>().ok()?,
                ))
            });

            if let Some((width, height)) = dimensions {
                gst::debug!(CAT, imp = self, "Parsed SDP dimensions {width}x{height}");
                state.sdp_dimensions = dimensions;
            } else {
                gst::warning!(
                    CAT,
                    imp = self,
                    "Failed to parse 'x-dimensions' attribute: {dimensions_str}"
                );
            }
        }

        if let Some(framerate_str) = s
            .get::<&str>("x-framerate")
            .ok()
            .or_else(|| s.get::<&str>("a-framerate").ok())
        {
            // Theoretically only `.` is allowed as decimal point but thanks to C formatting
            // functions being locale dependent, a lot of code out there puts a comma.
            let framerate_str = framerate_str.replace(',', ".");
            if let Some(framerate) = framerate_str
                .parse::<f64>()
                .ok()
                .and_then(gst::Fraction::approximate_f64)
            {
                gst::debug!(CAT, imp = self, "Parsed SDP framerate {framerate}");
                state.sdp_framerate = Some(framerate);
            } else {
                gst::warning!(
                    CAT,
                    imp = self,
                    "Failed to parse 'a-framerate' attribute: {framerate_str}"
                );
            }
        }

        true
    }

    fn drain(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.borrow_mut();
        // FIXME: Could theoretically forward / handle incomplete frames
        // with complete restart intervals
        state.pending_frame = None;

        Ok(gst::FlowSuccess::Ok)
    }

    fn handle_packet(
        &self,
        packet: &crate::basedepay::Packet,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.borrow_mut();

        let payload = packet.payload();
        let mut cursor = io::Cursor::new(payload);
        let mut r = ByteReader::endian(&mut cursor, BigEndian);

        // TODO: Currently only types 0, 1, 64, 65 (4:2:0 / 4:2:2 YUV) and progressive frames
        // (subtype 0) are supported.
        let main_header = match r.parse::<MainHeader>() {
            Ok(main_header) => main_header,
            Err(err) => {
                gst::warning!(CAT, imp = self, "Failed to parse main header: {err}");
                state.pending_frame = None;
                self.obj().drop_packet(packet);
                return Ok(gst::FlowSuccess::Ok);
            }
        };

        gst::trace!(CAT, imp = self, "Parsed main header {main_header:?}");

        if state.pending_frame.is_none() && main_header.fragment_offset > 0 {
            gst::trace!(CAT, imp = self, "Waiting for start of frame");
            self.obj().drop_packet(packet);
            return Ok(gst::FlowSuccess::Ok);
        }

        let restart_header = if main_header.type_ >= 64 {
            match r.parse::<RestartHeader>() {
                Ok(restart_header) => Some(restart_header),
                Err(err) => {
                    gst::warning!(CAT, imp = self, "Failed to parse restart header: {err}");
                    self.obj().drop_packet(packet);
                    return Ok(gst::FlowSuccess::Ok);
                }
            }
        } else {
            None
        };

        // Handle initial setup for a frame
        if main_header.fragment_offset == 0 {
            if state.pending_frame.is_some() {
                gst::warning!(CAT, imp = self, "Dropping incomplete pending frame");
                state.pending_frame = None;
            }

            // Retrieve quantization tables, either from the packet itself or frame cached/static
            // quantization tables depending on the Q value.
            let quant = if main_header.q >= 128 {
                match r.parse_with::<QuantizationTableHeader>(&main_header) {
                    Ok(quant_table_header)
                        if quant_table_header.luma_len != 0
                            && quant_table_header.chroma_len != 0 =>
                    {
                        // Dynamic quantization tables are not cached
                        if main_header.q != 255 {
                            state
                                .quant_tables
                                .insert(main_header.q, quant_table_header.clone());
                        }

                        quant_table_header
                    }
                    Ok(_) => match state.quant_tables.get(&main_header.q) {
                        Some(quant) => quant.clone(),
                        None => {
                            gst::warning!(
                                CAT,
                                imp = self,
                                "Have no quantization table for Q {} yet",
                                main_header.q
                            );
                            self.obj().drop_packet(packet);
                            return Ok(gst::FlowSuccess::Ok);
                        }
                    },
                    Err(err) => {
                        gst::warning!(
                            CAT,
                            imp = self,
                            "Failed to parse quantization table header: {err}"
                        );
                        self.obj().drop_packet(packet);
                        return Ok(gst::FlowSuccess::Ok);
                    }
                }
            } else {
                let quant = state.quant_tables.entry(main_header.q).or_insert_with(|| {
                    let (luma_quant, chroma_quant) = make_quant_tables(main_header.q);
                    QuantizationTableHeader {
                        luma_quant,
                        luma_len: 64,
                        chroma_quant,
                        chroma_len: 64,
                    }
                });
                quant.clone()
            };

            // Negotiate with downstream
            let width = if main_header.width != 0 {
                main_header.width as i32
            } else if let Some((width, _)) = state.sdp_dimensions {
                width as i32
            } else {
                gst::warning!(CAT, imp = self, "Can't determine valid width for frame");
                self.obj().drop_packet(packet);
                return Ok(gst::FlowSuccess::Ok);
            };

            let height = if main_header.height != 0 {
                main_header.height as i32
            } else if let Some((height, _)) = state.sdp_dimensions {
                height as i32
            } else {
                gst::warning!(CAT, imp = self, "Can't determine valid height for frame");
                self.obj().drop_packet(packet);
                return Ok(gst::FlowSuccess::Ok);
            };

            if !self.obj().src_pad().has_current_caps()
                || state.dimensions != Some((width, height))
                || state.framerate != state.sdp_framerate
            {
                let caps = gst::Caps::builder("image/jpeg")
                    .field("parsed", true)
                    .field("width", width)
                    .field("height", height)
                    .field("sof-marker", 0i32)
                    .field("colorspace", "sYUV")
                    .field(
                        "sampling",
                        if main_header.type_ & 0x3f == 0 {
                            "YCbCr-4:2:2"
                        } else {
                            "YCbCr-4:2:0"
                        },
                    )
                    .field_if_some("framerate", state.sdp_framerate)
                    .build();
                gst::debug!(CAT, imp = self, "Setting caps {caps:?}");
                self.obj().set_src_caps(&caps);
                state.dimensions = Some((width, height));
                state.framerate = state.sdp_framerate;
            }

            let mut data = Vec::new();

            // Prepend the JPEG headers before the actual JPEG data that comes from the packet
            // payload.
            let jpeg_header = match JpegHeader::new(
                &main_header,
                restart_header.as_ref(),
                quant,
                width as u16,
                height as u16,
            ) {
                Ok(jpeg_header) => jpeg_header,
                Err(err) => {
                    gst::warning!(CAT, imp = self, "Can't create JPEG header for frame: {err}");
                    self.obj().drop_packet(packet);
                    return Ok(gst::FlowSuccess::Ok);
                }
            };
            let mut w = ByteWriter::endian(&mut data, BigEndian);

            if let Err(err) = w.build::<JpegHeader>(&jpeg_header) {
                gst::warning!(
                    CAT,
                    imp = self,
                    "Failed to write JPEG header for frame: {err}"
                );
                self.obj().drop_packet(packet);
                return Ok(gst::FlowSuccess::Ok);
            }

            state.pending_frame = Some(PendingFrame {
                main_header: main_header.clone(),
                data,
                expected_fragment_offset: 0,
                start_ext_seqnum: packet.ext_seqnum(),
            });
        }

        let pending_frame = state.pending_frame.as_mut().expect("no pending frame");
        if pending_frame.expected_fragment_offset != main_header.fragment_offset {
            gst::warning!(
                CAT,
                imp = self,
                "Expected fragment offset {} but got {}",
                pending_frame.expected_fragment_offset,
                main_header.fragment_offset,
            );
            state.pending_frame = None;
            self.obj().drop_packet(packet);
            return Ok(gst::FlowSuccess::Ok);
        }

        if pending_frame.main_header.type_specific != main_header.type_specific
            || pending_frame.main_header.type_ != main_header.type_
            || pending_frame.main_header.q != main_header.q
            || pending_frame.main_header.width != main_header.width
            || pending_frame.main_header.height != main_header.height
        {
            gst::warning!(
                CAT,
                imp = self,
                "Main header changed in incompatible ways from {:?} to {:?} during a frame",
                pending_frame.main_header,
                main_header,
            );

            state.pending_frame = None;
            self.obj().drop_packet(packet);
            return Ok(gst::FlowSuccess::Ok);
        }

        let jpeg_payload_offset = cursor.position() as usize;
        let jpeg_payload = &payload[jpeg_payload_offset..];
        pending_frame.data.extend_from_slice(jpeg_payload);
        pending_frame.expected_fragment_offset += jpeg_payload.len() as u32;

        // Wait for marker before outputting anything
        if !packet.marker_bit() {
            return Ok(gst::FlowSuccess::Ok);
        }

        let mut pending_frame = state.pending_frame.take().expect("no pending frame");

        // Add EOI marker if there is none
        if !pending_frame.data.ends_with(&[0xff, 0xd9]) {
            pending_frame.data.extend_from_slice(&[0xff, 0xd9]);
        }

        let buffer = gst::Buffer::from_mut_slice(mem::take(&mut pending_frame.data));
        self.obj().queue_buffer(
            crate::basedepay::PacketToBufferRelation::Seqnums(
                pending_frame.start_ext_seqnum..=packet.ext_seqnum(),
            ),
            buffer,
        )
    }
}
