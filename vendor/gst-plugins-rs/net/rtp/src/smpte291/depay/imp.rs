// GStreamer RTP SMPTE291 ANC Depayloader
//
// Copyright (C) 2025 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-rtpsmpte291depay
 * @see_also: rtpsmpte291pay, st2038demux, st2038mux, cctost2038anc, st2038anctocc
 *
 * Depayload an SMPTE ST291-1 ANC stream as ST2038 from RTP packets as per [RFC 8331][rfc-8331].
 *
 * [rfc-8331]: https://www.rfc-editor.org/rfc/rfc8331.html
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 udpsrc caps='application/x-rtp, media=(string)application, clock-rate=(int)90000, encoding-name=(string)smpte291' ! rtpsmpte291depay ! fakesink dump=true
 * ]| This will depayload an RTP ST291-1 stream and display a hexdump of the ST2038 data on stdout.
 * You can use the #rtpsmpte291pay element to create such an RTP stream.
 *
 * Since: plugins-rs-0.15.0
 */
use gst::{glib, subclass::prelude::*};

use std::sync::LazyLock;

use crate::basedepay::{Packet, RtpBaseDepay2Ext, RtpBaseDepay2Impl};

#[derive(Default)]
pub struct RtpSmpte291Depay;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpsmpte291depay",
        gst::DebugColorFlags::empty(),
        Some("RTP ST291-1 ANC Depayloader"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for RtpSmpte291Depay {
    const NAME: &'static str = "GstRtpSmpte291Depay";
    type Type = super::RtpSmpte291Depay;
    type ParentType = crate::basedepay::RtpBaseDepay2;
}

impl ObjectImpl for RtpSmpte291Depay {}

impl GstObjectImpl for RtpSmpte291Depay {}

impl ElementImpl for RtpSmpte291Depay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP ST291-1 ANC Depayloader",
                "Codec/Depayloader/Network/RTP",
                "Depayload an SMPTE ST291-1 ANC stream from RTP packets (RFC 8331)",
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
                            .field("media", "application")
                            .field("clock-rate", 90_000i32)
                            .field("encoding-name", "smpte291")
                            .build(),
                    )
                    .build(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("meta/x-st-2038")
                    .field("alignment", "packet")
                    .build(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl RtpBaseDepay2Impl for RtpSmpte291Depay {
    const ALLOWED_META_TAGS: &'static [&'static str] = &[];

    fn set_sink_caps(&self, _caps: &gst::Caps) -> bool {
        let src_caps = gst::Caps::builder("meta/x-st-2038")
            .field("alignment", "packet")
            .build();

        self.obj().set_src_caps(&src_caps);

        true
    }

    // https://www.rfc-editor.org/rfc/rfc8331.html#section-2.1
    fn handle_packet(&self, packet: &Packet) -> Result<gst::FlowSuccess, gst::FlowError> {
        let ext_seqnum = packet.ext_seqnum();
        let marker_bit = packet.marker_bit();
        let payload = packet.payload();

        let inner = || -> Result<(), anyhow::Error> {
            use anyhow::Context;
            use bitstream_io::{BigEndian, BitRead, BitReader, BitWrite, BitWriter};
            use std::io::Cursor;

            let rcursor = Cursor::new(payload);
            let mut r = BitReader::endian(rcursor, BigEndian);

            let extended_seqnum = r.read_to::<u16>().context("extended_seqnum")?;
            let length = r.read_to::<u16>().context("length")?;
            let anc_count = r.read_to::<u8>().context("ANC_count")?;
            let field = r.read::<2, u8>().context("F")?;
            r.skip(22).context("reserved")?;
            assert!(r.byte_aligned());

            gst::trace!(
                CAT,
                imp = self,
                "Handling ST291-1 packet with ext seqnum {extended_seqnum}, \
                 length {length}, ANC count {anc_count} and field {field}",
            );

            for _ in 0..anc_count {
                // Maximum size of one ST2038 packet
                let mut packet = Vec::with_capacity(328);

                let c_not_y_channel_flag = r.read_bit().context("C")?;
                let line_number = r.read::<11, u16>().context("line_number")?;
                let horizontal_offset = r.read::<12, u16>().context("horizontal_offset")?;
                let s = r.read::<1, u8>().context("S")?;
                let stream_num = r.read::<7, u8>().context("StreamNum")?;

                let did = r.read::<10, u16>().context("did")?;
                let sdid = r.read::<10, u16>().context("sdid")?;
                let data_count = r.read::<10, u16>().context("data_count")?;
                let data_count8 = (data_count & 0xff) as u8;

                gst::trace!(
                    CAT,
                    imp = self,
                    "Handling ANC packet with C {c_not_y_channel_flag}, line number {line_number}, \
                     horizontal offset {horizontal_offset}, S {s}, stream number {stream_num}, \
                     DID {did:03x}, SDID {sdid:03x}, data count {data_count8}",
                );

                let wcursor: Cursor<&mut Vec<u8>> = Cursor::new(&mut packet);
                let mut w = BitWriter::endian(wcursor, BigEndian);

                w.write::<6, u8>(0).context("zeroes")?;
                w.write_bit(c_not_y_channel_flag)
                    .context("c_not_y_channel_flag")?;
                w.write::<11, u16>(line_number).context("line_number")?;
                w.write::<12, u16>(horizontal_offset)
                    .context("horizontal_offset")?;

                w.write::<10, u16>(did).context("did")?;
                w.write::<10, u16>(sdid).context("sdid")?;
                w.write::<10, u16>(data_count).context("data_count")?;

                for _ in 0..data_count8 {
                    w.write::<10, u16>(r.read::<10, u16>()?)
                        .context("user_data")?;
                }
                w.write::<10, u16>(r.read::<10, u16>()?)
                    .context("checksum")?;
                while !w.byte_aligned() {
                    w.write_bit(true).context("padding")?;
                }
                w.flush().context("flush")?;

                let mut buffer = gst::Buffer::from_mut_slice(packet);
                if marker_bit {
                    let buffer = buffer.get_mut().unwrap();
                    buffer.set_flags(gst::BufferFlags::MARKER);
                }

                self.obj().queue_buffer(ext_seqnum.into(), buffer)?;
            }

            Ok(())
        };

        match inner() {
            Ok(_) => Ok(gst::FlowSuccess::Ok),
            Err(err) => {
                if let Some(res) = err.downcast_ref::<gst::FlowError>() {
                    return Err(*res);
                }

                gst::warning!(CAT, imp = self, "Failed to process packet: {err}");

                Ok(gst::FlowSuccess::Ok)
            }
        }
    }
}
