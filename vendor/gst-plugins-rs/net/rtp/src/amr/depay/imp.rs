// Copyright (C) 2023 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::io;

use atomic_refcell::AtomicRefCell;
use bitstream_io::{BigEndian, BitRead as _, BitReader, ByteRead as _, ByteReader};
/**
 * SECTION:element-rtpamrdepay2
 * @see_also: rtpamrpay2, rtpamrpay, rtpamrdepay, amrnbdec, amrnbenc, amrwbdec, voamrwbenc
 *
 * Extracts an AMR audio stream from RTP packets as per [RFC 3267][rfc-3267].
 *
 * [rfc-3267]: https://datatracker.ietf.org/doc/html/rfc3267
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 udpsrc caps='application/x-rtp, media=audio, clock-rate=8000, encoding-name=AMR, octet-align=(string)1' ! rtpjitterbuffer latency=50 ! rtpamrdepay2 ! amrnbdec ! audioconvert ! audioresample ! autoaudiosink
 * ]| This will depayload an incoming RTP AMR NB audio stream. You can use the #amrnbenc and
 * #rtpamrpay2 elements to create such an RTP stream.
 *
 * Since: 0.14
 */
use gst::{glib, subclass::prelude::*};

use std::sync::LazyLock;

use crate::{
    amr::payload_header::{
        PayloadConfiguration, PayloadHeader, NB_FRAME_SIZES, NB_FRAME_SIZES_BYTES, WB_FRAME_SIZES,
        WB_FRAME_SIZES_BYTES,
    },
    basedepay::{RtpBaseDepay2Ext, RtpBaseDepay2Impl},
};

#[derive(Default)]
struct State {
    wide_band: bool,
    has_crc: bool,
    bandwidth_efficient: bool,
}

#[derive(Default)]
pub struct RtpAmrDepay {
    state: AtomicRefCell<State>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpamrdepay2",
        gst::DebugColorFlags::empty(),
        Some("RTP AMR Depayloader"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for RtpAmrDepay {
    const NAME: &'static str = "GstRtpAmrDepay2";
    type Type = super::RtpAmrDepay;
    type ParentType = crate::basedepay::RtpBaseDepay2;
}

impl ObjectImpl for RtpAmrDepay {}

impl GstObjectImpl for RtpAmrDepay {}

impl ElementImpl for RtpAmrDepay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP AMR Depayloader",
                "Codec/Depayloader/Network/RTP",
                "Depayload an AMR audio stream from RTP packets (RFC 3267)",
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
                            .field("media", "audio")
                            .field("encoding-name", "AMR")
                            .field("clock-rate", 8_000i32)
                            .build(),
                    )
                    .structure(
                        gst::Structure::builder("application/x-rtp")
                            .field("media", "audio")
                            .field("encoding-name", "AMR-WB")
                            .field("clock-rate", 16_000i32)
                            .build(),
                    )
                    .build(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder_full()
                    .structure(
                        gst::Structure::builder("audio/AMR")
                            .field("channels", 1i32)
                            .field("rate", 8_000i32)
                            .build(),
                    )
                    .structure(
                        gst::Structure::builder("audio/AMR-WB")
                            .field("channels", 1i32)
                            .field("rate", 16_000i32)
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
impl RtpBaseDepay2Impl for RtpAmrDepay {
    const ALLOWED_META_TAGS: &'static [&'static str] = &["audio"];

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

        let encoding_name = s.get::<&str>("encoding-name").unwrap();

        // We currently only support
        //
        // octet-align={"0", "1" }, default is "0"
        // robust-sorting="0", default
        // interleaving="0", default
        // encoding-params="1", (channels), default
        // crc={"0", "1"}, default "0"

        if s.get::<&str>("robust-sorting").is_ok_and(|s| s != "0") {
            gst::error!(CAT, imp = self, "Only robust-sorting=0 supported");
            return false;
        }

        if s.get::<&str>("interleaving").is_ok_and(|s| s != "0") {
            gst::error!(CAT, imp = self, "Only interleaving=0 supported");
            return false;
        }

        if s.get::<&str>("encoding-params").is_ok_and(|s| s != "1") {
            gst::error!(CAT, imp = self, "Only encoding-params=1 supported");
            return false;
        }

        let mut state = self.state.borrow_mut();

        let has_crc = s.get::<&str>("crc").is_ok_and(|s| s != "0");
        let bandwidth_efficient = s.get::<&str>("octet-align") != Ok("1");

        if bandwidth_efficient && has_crc {
            gst::error!(
                CAT,
                imp = self,
                "CRC not supported in bandwidth-efficient mode"
            );
            return false;
        }

        let wide_band = match encoding_name {
            "AMR" => false,
            "AMR-WB" => true,
            _ => unreachable!(),
        };

        state.has_crc = has_crc;
        state.wide_band = wide_band;
        state.bandwidth_efficient = bandwidth_efficient;

        let src_caps = gst::Caps::builder(if wide_band {
            "audio/AMR-WB"
        } else {
            "audio/AMR"
        })
        .field("channels", 1i32)
        .field("rate", if wide_band { 16_000i32 } else { 8_000i32 })
        .build();

        self.obj().set_src_caps(&src_caps);

        true
    }

    fn handle_packet(
        &self,
        packet: &crate::basedepay::Packet,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let payload = packet.payload();

        let state = self.state.borrow();

        let payload_configuration = PayloadConfiguration {
            has_crc: state.has_crc,
            wide_band: state.wide_band,
        };

        let mut out_data;
        let mut num_packets = 0;
        let mut cursor = io::Cursor::new(payload);
        if state.bandwidth_efficient {
            let frame_sizes = if state.wide_band {
                WB_FRAME_SIZES.as_slice()
            } else {
                NB_FRAME_SIZES.as_slice()
            };

            let mut r = BitReader::endian(&mut cursor, BigEndian);
            let payload_header = match r.parse_with::<PayloadHeader>(&payload_configuration) {
                Ok(payload_header) => payload_header,
                Err(err) => {
                    gst::error!(CAT, imp = self, "Failed parsing payload header: {err}");
                    self.obj().drop_packet(packet);
                    return Ok(gst::FlowSuccess::Ok);
                }
            };

            gst::trace!(CAT, imp = self, "Parsed payload header {payload_header:?}");

            out_data = Vec::with_capacity(payload_header.buffer_size(state.wide_band));

            'entries: for toc_entry in &payload_header.toc_entries {
                let prev_len = out_data.len();

                out_data.push(toc_entry.frame_header());

                if let Some(&frame_size) = frame_sizes.get(toc_entry.frame_type as usize) {
                    let mut frame_size = frame_size as u32;

                    while frame_size > 8 {
                        match r.read_to::<u8>() {
                            Ok(b) => {
                                out_data.push(b);
                            }
                            Err(_) => {
                                gst::warning!(CAT, imp = self, "Short packet");
                                out_data.truncate(prev_len);
                                break 'entries;
                            }
                        }
                        frame_size -= 8;
                    }

                    if frame_size > 0 {
                        match r.read_var::<u8>(frame_size) {
                            Ok(b) => {
                                out_data.push(b << (8 - frame_size));
                            }
                            Err(_) => {
                                gst::warning!(CAT, imp = self, "Short packet");
                                out_data.truncate(prev_len);
                                break 'entries;
                            }
                        }
                    }
                }

                num_packets += 1;
            }
        } else {
            let frame_sizes = if state.wide_band {
                WB_FRAME_SIZES_BYTES.as_slice()
            } else {
                NB_FRAME_SIZES_BYTES.as_slice()
            };

            let mut r = ByteReader::endian(&mut cursor, BigEndian);
            let payload_header = match r.parse_with::<PayloadHeader>(&payload_configuration) {
                Ok(payload_header) => payload_header,
                Err(err) => {
                    gst::error!(CAT, imp = self, "Failed parsing payload header: {err}");
                    self.obj().drop_packet(packet);
                    return Ok(gst::FlowSuccess::Ok);
                }
            };

            gst::trace!(CAT, imp = self, "Parsed payload header {payload_header:?}");

            out_data = Vec::with_capacity(payload_header.buffer_size(state.wide_band));

            let payload_start = cursor.position() as usize;
            let mut data = &payload[payload_start..];

            for toc_entry in &payload_header.toc_entries {
                if let Some(&frame_size) = frame_sizes.get(toc_entry.frame_type as usize) {
                    let frame_size = frame_size as usize;

                    if data.len() < frame_size {
                        gst::warning!(CAT, imp = self, "Short packet");
                        break;
                    }
                    out_data.push(toc_entry.frame_header());
                    out_data.extend_from_slice(&data[..frame_size]);
                    data = &data[frame_size..];
                }

                num_packets += 1;
            }
        }

        gst::trace!(
            CAT,
            imp = self,
            "Finishing buffer of {} bytes with {num_packets} packets",
            out_data.len()
        );

        if !out_data.is_empty() {
            let mut outbuf = gst::Buffer::from_mut_slice(out_data);
            {
                let outbuf = outbuf.get_mut().unwrap();

                outbuf.set_duration(gst::ClockTime::from_mseconds(20) * num_packets);

                if packet.marker_bit() {
                    outbuf.set_flags(gst::BufferFlags::RESYNC);
                }
            }

            self.obj().queue_buffer(packet.into(), outbuf)?;
        }

        Ok(gst::FlowSuccess::Ok)
    }
}
