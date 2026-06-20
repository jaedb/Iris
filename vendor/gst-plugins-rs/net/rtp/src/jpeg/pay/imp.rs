//
// Copyright (C) 2023 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use atomic_refcell::AtomicRefCell;
/**
 * SECTION:element-rtpjpegpay2
 * @see_also: rtpjpegdepay2, jpegdec, jpegenc
 *
 * Payload a JPEG video stream into RTP packets as per [RFC 2435][rfc-2435].
 *
 * [rfc-2435]: https://www.rfc-editor.org/rfc/rfc2435.html
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 videotestsrc ! video/x-raw,width=1280,height=720,format=I420 ! timeoverlay font-desc=Sans,22 ! jpegenc ! jpegparse ! rtpjpegpay2 ! udpsink host=127.0.0.1 port=5004
 * ]| This will create and payload a JPEG video stream with a test pattern and
 * send it out via UDP to localhost port 5004.
 *
 * Since: plugins-rs-0.13.0
 */
use gst::{glib, subclass::prelude::*};
use smallvec::SmallVec;
use std::{cmp, io};

use bitstream_io::{BigEndian, ByteRead as _, ByteReader, ByteWrite as _, ByteWriter};
use std::sync::LazyLock;

use crate::{
    basepay::RtpBasePay2Ext,
    jpeg::header::{detect_static_quant_table, JpegHeader, MainHeader, QuantizationTableHeader},
};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpjpegpay2",
        gst::DebugColorFlags::empty(),
        Some("RTP JPEG Payloader"),
    )
});

#[derive(Default)]
struct State {
    width: Option<u16>,
    height: Option<u16>,
    previous_q: Option<u8>,
}

#[derive(Default)]
pub struct RtpJpegPay {
    state: AtomicRefCell<State>,
}

#[glib::object_subclass]
impl ObjectSubclass for RtpJpegPay {
    const NAME: &'static str = "GstRtpJpegPay2";
    type Type = super::RtpJpegPay;
    type ParentType = crate::basepay::RtpBasePay2;
}

impl ObjectImpl for RtpJpegPay {}

impl GstObjectImpl for RtpJpegPay {}

impl ElementImpl for RtpJpegPay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP JPEG payloader",
                "Codec/Payloader/Network/RTP",
                "Payload a JPEG Video stream to RTP packets (RFC 2435)",
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
                &gst::Caps::builder("image/jpeg")
                    .field("parsed", true)
                    .field("width", gst::IntRange::new(1i32, u16::MAX as i32))
                    .field("height", gst::IntRange::new(1i32, u16::MAX as i32))
                    .field("sof-marker", 0i32)
                    .field("colorspace", "sYUV")
                    .field("sampling", gst::List::new(["YCbCr-4:2:0", "YCbCr-4:2:2"]))
                    .build(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
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

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl crate::basepay::RtpBasePay2Impl for RtpJpegPay {
    const ALLOWED_META_TAGS: &'static [&'static str] = &["video"];
    const DEFAULT_PT: u8 = 26;

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
        gst::debug!(CAT, imp = self, "received caps {caps:?}");

        let s = caps.structure(0).unwrap();

        let mut caps_builder = gst::Caps::builder("application/x-rtp")
            .field("media", "video")
            .field("clock-rate", 90_000i32);

        if let Some(framerate) = s
            .get::<gst::Fraction>("framerate")
            .ok()
            .filter(|fps| *fps > gst::Fraction::new(0, 1))
        {
            caps_builder = caps_builder.field(
                "a-framerate",
                format!(
                    "{}",
                    (framerate.numer() as f64 / (framerate.denom() as f64))
                ),
            );
        }

        let width = s.get::<i32>("width").unwrap() as u16;
        let height = s.get::<i32>("height").unwrap() as u16;

        // If the resolution doesn't fit into the RTP payload header then pass it via the SDP and
        // set it to 0 inside the RTP payload header
        if width > 2040 || height > 2040 {
            caps_builder = caps_builder.field("x-dimensions", format!("{width},{height}"));
        }

        self.obj().set_src_caps(&caps_builder.build());

        let mut state = self.state.borrow_mut();
        // If the resolution doesn't fit into the RTP payload header then pass it via the SDP and
        // set it to 0 inside the RTP payload header
        if width > 2040 || height > 2040 {
            state.width = Some(0);
            state.height = Some(0);
        } else {
            state.width = Some(width);
            state.height = Some(height);
        }

        true
    }

    fn handle_buffer(
        &self,
        buffer: &gst::Buffer,
        id: u64,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.borrow_mut();

        let max_payload_size = self.obj().max_payload_size();

        gst::trace!(CAT, imp = self, "received buffer of size {}", buffer.size());

        let map = buffer.map_readable().map_err(|_| {
            gst::element_imp_error!(
                self,
                gst::ResourceError::Read,
                ["Failed to map buffer readable"]
            );

            gst::FlowError::Error
        })?;

        // Set together with the caps
        let width = state.width.unwrap();
        let height = state.height.unwrap();

        let mut cursor = io::Cursor::new(&map);
        let mut r = ByteReader::endian(&mut cursor, BigEndian);
        let jpeg_header = match r.parse::<JpegHeader>() {
            Ok(header) => header,
            Err(err) => {
                gst::error!(CAT, imp = self, "Failed parsing JPEG header: {err}");
                return Err(gst::FlowError::Error);
            }
        };
        let data_offset = cursor.position() as usize;
        gst::trace!(
            CAT,
            imp = self,
            "Parsed JPEG header {jpeg_header:?}, data starts at offset {data_offset}"
        );

        // Try detecting static quantization headers
        let luma_quant = &jpeg_header.quant.luma_quant[..jpeg_header.quant.luma_len as usize];
        let chroma_quant = &jpeg_header.quant.chroma_quant[..jpeg_header.quant.chroma_len as usize];
        let q = if let Some(q) =
            detect_static_quant_table(luma_quant, chroma_quant, state.previous_q)
        {
            state.previous_q = Some(q);
            q
        } else {
            state.previous_q = None;
            255
        };

        gst::trace!(CAT, imp = self, "Using Q {q}");

        let mut data = &map[data_offset..];
        let mut fragment_offset = 0;
        while !data.is_empty() {
            let main_header = MainHeader {
                type_specific: 0,
                fragment_offset,
                type_: jpeg_header.type_,
                q,
                width,
                height,
            };
            let main_header_size = main_header.size().map_err(|err| {
                gst::error!(CAT, imp = self, "Failed to write main header: {err:?}");
                gst::FlowError::Error
            })?;

            // TODO: can handle restart headers better, for now we just don't bother

            let quant_table_header = if fragment_offset == 0 && q >= 128 {
                Some(jpeg_header.quant.clone())
            } else {
                None
            };
            let quant_table_header_size = quant_table_header
                .as_ref()
                .map(|q| q.size(&main_header))
                .unwrap_or(Ok(0))
                .map_err(|err| {
                    gst::error!(
                        CAT,
                        imp = self,
                        "Failed to write quantization table header: {err:?}"
                    );
                    gst::FlowError::Error
                })?;

            let overhead = main_header_size + quant_table_header_size;
            let payload_size = (max_payload_size as usize)
                .checked_sub(overhead + 1)
                .ok_or_else(|| {
                    gst::error!(CAT, imp = self, "Too small MTU configured for stream");
                    gst::element_imp_error!(
                        self,
                        gst::LibraryError::Settings,
                        ["Too small MTU configured for stream"]
                    );
                    gst::FlowError::Error
                })?
                + 1;
            let payload_size = cmp::min(payload_size, data.len());

            gst::trace!(
                CAT,
                imp = self,
                "Writing packet with main header {main_header:?}, quantization table header {quant_table_header:?} and payload size {payload_size}",
            );

            // 8 bytes main header, 4 bytes quantization table header and up to 2x 128 bytes
            // quantization table.
            let mut headers_buffer = SmallVec::<[u8; 8 + 4 + 256]>::with_capacity(
                main_header_size + quant_table_header_size,
            );

            let mut w = ByteWriter::endian(&mut headers_buffer, BigEndian);
            w.build::<MainHeader>(&main_header).map_err(|err| {
                gst::error!(CAT, imp = self, "Failed to write main header: {err:?}");
                gst::FlowError::Error
            })?;
            if let Some(quant_table_header) = quant_table_header {
                w.build_with::<QuantizationTableHeader>(&quant_table_header, &main_header)
                    .map_err(|err| {
                        gst::error!(
                            CAT,
                            imp = self,
                            "Failed to write quantization table header: {err:?}"
                        );
                        gst::FlowError::Error
                    })?;
            }
            assert_eq!(
                headers_buffer.len(),
                main_header_size + quant_table_header_size,
            );

            self.obj().queue_packet(
                id.into(),
                rtp_types::RtpPacketBuilder::new()
                    .marker_bit(data.len() == payload_size)
                    .payload(headers_buffer.as_slice())
                    .payload(&data[..payload_size]),
            )?;

            fragment_offset += payload_size as u32;
            data = &data[payload_size..];
        }

        Ok(gst::FlowSuccess::Ok)
    }
}
