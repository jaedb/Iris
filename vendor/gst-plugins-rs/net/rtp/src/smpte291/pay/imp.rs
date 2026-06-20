// GStreamer RTP SMPTE291 ANC Payloader
//
// Copyright (C) 2025 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-rtpsmpte291pay
 * @see_also: rtpsmpte291depay, st2038demux, st2038mux, cctost2038anc, st2038anctocc
 *
 * Payload an SMPTE ST291-1 ANC stream as ST2038 into RTP packets as per [RFC 8331][rfc-8331].
 *
 * [rfc-8331]: https://www.rfc-editor.org/rfc/rfc8331.html
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 filesrc location=video-with-st2038.ts ! tsdemux ! rtpsmpte291pay ! udpsink
 * ]| This example pipeline will payload an RTP ANC stream extracted from an
 * MPEG-TS stream and send it via UDP to an RTP receiver. Note that `rtpsmpte291pay` expects the
 * incoming ST2038 packets to be timestamped, which may not always be the case when they come from
 * an MPEG-TS file.
 *
 * Since: plugins-rs-0.15.0
 */
use gst::{glib, subclass::prelude::*};

use std::{num::Wrapping, sync::LazyLock};

use atomic_refcell::AtomicRefCell;

use crate::basepay::{RtpBasePay2Ext, RtpBasePay2Impl};

#[derive(Default)]
pub struct RtpSmpte291Pay {
    state: AtomicRefCell<State>,
}

#[derive(Default)]
struct State {
    extended_seqnum: Wrapping<u32>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpsmpte291pay",
        gst::DebugColorFlags::empty(),
        Some("RTP ST291-1 ANC Payloader"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for RtpSmpte291Pay {
    const NAME: &'static str = "GstRtpSmpte291Pay";
    type Type = super::RtpSmpte291Pay;
    type ParentType = crate::basepay::RtpBasePay2;
}

impl ObjectImpl for RtpSmpte291Pay {}

impl GstObjectImpl for RtpSmpte291Pay {}

impl ElementImpl for RtpSmpte291Pay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP ST291-1 ANC Payloader",
                "Codec/Payloader/Network/RTP",
                "Payload an SMPTE ST291-1 ANC stream into RTP packets (RFC 8331)",
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
                &gst::Caps::builder("meta/x-st-2038")
                    .field("alignment", "frame")
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
                            .field("media", "application")
                            .field("clock-rate", 90000i32)
                            .field("encoding-name", "smpte291")
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

impl RtpBasePay2Impl for RtpSmpte291Pay {
    const ALLOWED_META_TAGS: &'static [&'static str] = &[];

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.borrow_mut();
        state.extended_seqnum = Wrapping(0);
        drop(state);

        Ok(())
    }

    fn set_sink_caps(&self, _caps: &gst::Caps) -> bool {
        let src_caps = gst::Caps::builder("application/x-rtp")
            .field("media", "application")
            .field("clock-rate", 90000i32)
            .field("encoding-name", "smpte291")
            .build();

        self.obj().set_src_caps(&src_caps);

        true
    }

    // https://www.rfc-editor.org/rfc/rfc8331.html#section-2.1
    fn handle_buffer(
        &self,
        buffer: &gst::Buffer,
        id: u64,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let map = buffer.map_readable().map_err(|_| {
            gst::error!(CAT, imp = self, "Can't map buffer readable");
            gst::FlowError::Error
        })?;

        if map.size() == 0 {
            gst::log!(CAT, imp = self, "Empty buffer, skipping");
            self.obj().drop_buffers(id..=id);
            return Ok(gst::FlowSuccess::Ok);
        }

        let max_payload_size = self.obj().max_payload_size() as usize;
        // Minimum size of at least one maximum sized RTP ST291-1 ANC packet
        // FIXME: Smaller MTU could be handled theoretically as it's
        // unlikely to get ANC packets that are this big.
        if max_payload_size < 328 {
            gst::element_imp_error!(
                self,
                gst::ResourceError::Settings,
                ["Too small MTU configured"]
            );
            return Err(gst::FlowError::Error);
        }

        let mut data = map.as_slice();
        let mut payload = Vec::with_capacity(max_payload_size);
        let mut state = self.state.borrow_mut();

        // Reset extended seqnum counter if the seqnum was reset
        if state.extended_seqnum.0 & 0x0000_ffff != self.obj().next_seqnum() as u32 {
            state.extended_seqnum.0 = self.obj().next_seqnum() as u32;
        }

        while !data.is_empty() {
            #[allow(clippy::collapsible_if)]
            if self.convert_next_packet(
                state.extended_seqnum.0,
                max_payload_size,
                &mut data,
                &mut payload,
            ) {
                if !payload.is_empty() {
                    self.obj().queue_packet(
                        id.into(),
                        rtp_types::RtpPacketBuilder::new()
                            .marker_bit(data.is_empty())
                            .payload(&payload),
                    )?;
                    payload.clear();
                    state.extended_seqnum += 1;
                }
            }
        }
        assert!(data.is_empty());

        Ok(gst::FlowSuccess::Ok)
    }
}

impl RtpSmpte291Pay {
    fn convert_next_packet(
        &self,
        extended_seqnum: u32,
        max_payload_size: usize,
        data: &mut &[u8],
        payload: &mut Vec<u8>,
    ) -> bool {
        let len = payload.len();

        let mut inner = || -> Result<bool, anyhow::Error> {
            use anyhow::Context;
            use bitstream_io::{BigEndian, BitRead, BitReader, BitWrite, BitWriter};
            use std::io::Cursor;

            let rcursor = Cursor::new(*data);
            let mut r = BitReader::endian(rcursor, BigEndian);

            let zeroes = r.read::<6, u8>().context("zero bits")?;
            if zeroes != 0 {
                anyhow::bail!("Zero bits not zero!");
            }
            let c_not_y_channel_flag = r.read_bit().context("c_not_y_channel_flag")?;
            let line_number = r.read::<11, u16>().context("line number")?;
            let horizontal_offset = r.read::<12, u16>().context("horizontal offset")?;
            // Top two bits are parity bits and can be stripped off
            let did = r.read::<10, u16>().context("DID")?;
            let sdid = r.read::<10, u16>().context("SDID")?;
            let data_count = r.read::<10, u16>().context("data count")?;
            let data_count8 = (data_count & 0xff) as u8;

            gst::trace!(
                CAT,
                imp = self,
                "Handling ST2038 packet with c_not_y_channel_flag {c_not_y_channel_flag}, \
                 line number {line_number}, horizontal offset {horizontal_offset}, \
                 DID {did:03x}, SDID {sdid:03x}, data count {data_count8}",
            );

            // 32 bits "header", DID/SDID/DC, user data words, checksum and then aligned.
            let required_size =
                (32 + 3 * 10 + data_count8 as usize * 10 + 10).next_multiple_of(32) / 8;

            if payload.is_empty() {
                // One packet always fits, include it here.
                // See above for why.
                let payload: &mut Vec<u8> = &mut *payload;
                let mut w = BitWriter::endian(payload, BigEndian);
                w.write_from::<u16>(((extended_seqnum) >> 16) as u16)
                    .context("extended_seqnum")?;
                w.write_from::<u16>(0u16).context("length")?;
                w.write_from::<u8>(0).context("ANC_count")?;
                // TODO: Need to get field information here
                w.write::<2, u8>(0u8).context("F")?;
                w.write::<22, u32>(0u32).context("reserved")?;
            } else if payload.len() + required_size > max_payload_size || payload[4] == 255 {
                // Output the existing payload first.
                return Ok(true);
            }

            let start_pos = payload.len() as u64;
            let mut wcursor: Cursor<&mut Vec<u8>> = Cursor::new(&mut *payload);
            wcursor.set_position(start_pos);
            let mut w = BitWriter::endian(wcursor, BigEndian);

            w.write_bit(c_not_y_channel_flag).context("C")?;
            w.write::<11, u16>(line_number).context("line_number")?;
            w.write::<12, u16>(horizontal_offset)
                .context("horizontal_offset")?;
            // TODO: Need to get stream information here
            w.write_bit(false).context("S")?;
            w.write::<7, u8>(0u8).context("StreamNum")?;

            w.write::<10, u16>(did).context("did")?;
            w.write::<10, u16>(sdid).context("sdid")?;
            w.write::<10, u16>(data_count).context("data_count")?;

            for _ in 0..data_count8 {
                w.write::<10, u16>(r.read::<10, u16>()?)
                    .context("user_data")?;
            }
            w.write::<10, u16>(r.read::<10, u16>()?)
                .context("checksum")?;

            let position = w.aligned_writer().context("align")?.position();
            if position % 4 != 0 {
                w.write_var(32 - (position % 4) as u32 * 8, 0)
                    .context("align")?;
            }
            assert!(w.byte_aligned());
            w.flush().context("flush")?;
            let position = w.into_writer().position() as usize;

            while !r.byte_aligned() {
                let one = r.read::<1, u8>().context("alignment")?;
                if one != 1 {
                    anyhow::bail!("Alignment bits are not ones!");
                }
            }

            // Update ANC_count and length
            payload[4] += 1;
            payload[2..4].copy_from_slice(&(position as u16 - 8).to_be_bytes());

            // Advance to the next packet
            assert!(r.byte_aligned());
            let position = r.into_reader().position() as usize;
            *data = &data[position..];

            // If empty, output what we have
            Ok(data.is_empty())
        };

        match inner() {
            Ok(res) => res,
            Err(err) => {
                gst::warning!(CAT, imp = self, "Failed to process packet: {err}");

                // Stop parsing, reset payload to the previous data and output it
                *data = &[];
                payload.resize(len, 0);

                true
            }
        }
    }
}
