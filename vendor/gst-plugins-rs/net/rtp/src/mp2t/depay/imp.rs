// GStreamer RTP MPEG-TS Depayloader
//
// Copyright (C) 2023-2024 Tim-Philipp Müller <tim centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-rtpmp2tdepay2
 * @see_also: rtpmp2tpay2, rtpmp2tdepay, rtpmp2tpay, tsdemux, mpegtsmux
 *
 * Depayload an MPEG Transport Stream from RTP packets as per [RFC 2250][rfc-2250].
 *
 * [rfc-2250]: https://www.rfc-editor.org/rfc/rfc2250.html
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 udpsrc address=127.0.0.1 port=5555 caps='application/x-rtp,media=video,clock-rate=90000,encoding-name=MP2T' ! rtpjitterbuffer latency=100 ! rtpmp2tdepay2 ! decodebin3 ! videoconvertscale ! autovideosink
 * ]| This will depayload an incoming RTP MPEG-TS stream. You can use the #rtpmp2tpay2 or #rtpmp2tpay
 * element to create such an RTP stream.
 *
 * Since: plugins-rs-0.13.0
 */
use atomic_refcell::AtomicRefCell;

use gst::{glib, prelude::*, subclass::prelude::*};

use std::sync::LazyLock;

use std::num::NonZeroUsize;
use std::sync::Mutex;

use crate::basedepay::RtpBaseDepay2Ext;

const TS_PACKET_SYNC: u8 = 0x47;

#[derive(Default)]
pub struct RtpMP2TDepay {
    state: AtomicRefCell<State>,
    settings: Mutex<Settings>,
}

#[derive(Default)]
struct State {
    packet_size: Option<NonZeroUsize>,
    bytes_to_skip: usize,
}

#[derive(Debug, Clone)]
struct Settings {
    skip_first_bytes: u32,
}

const DEFAULT_SKIP_FIRST_BYTES: u32 = 0;

impl Default for Settings {
    fn default() -> Self {
        Settings {
            skip_first_bytes: DEFAULT_SKIP_FIRST_BYTES,
        }
    }
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpmp2tdepay2",
        gst::DebugColorFlags::empty(),
        Some("RTP MPEG-TS Depayloader"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for RtpMP2TDepay {
    const NAME: &'static str = "GstRtpMP2TDepay2";
    type Type = super::RtpMP2TDepay;
    type ParentType = crate::basedepay::RtpBaseDepay2;
}

impl ObjectImpl for RtpMP2TDepay {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![glib::ParamSpecUInt::builder("skip-first-bytes")
                .nick("Skip first bytes")
                .blurb("Number of bytes to skip at the beginning of the payload")
                .default_value(DEFAULT_SKIP_FIRST_BYTES)
                .mutable_ready()
                .build()]
        });

        PROPERTIES.as_ref()
    }
    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "skip-first-bytes" => {
                let mut settings = self.settings.lock().unwrap();
                settings.skip_first_bytes = value.get().expect("type checked upstream");
            }
            name => unimplemented!("Property '{name}'"),
        };
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "skip-first-bytes" => {
                let settings = self.settings.lock().unwrap();
                settings.skip_first_bytes.to_value()
            }
            name => unimplemented!("Property '{name}'"),
        }
    }
}

impl GstObjectImpl for RtpMP2TDepay {}

impl ElementImpl for RtpMP2TDepay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP MPEG-TS Depayloader",
                "Codec/Depayloader/Network/RTP",
                "Depayload an MPEG Transport Stream from RTP packets (RFC 2250)",
                "Tim-Philipp Müller <tim centricular com>",
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
                        // Note: C depayloader accepts MP2T-ES as well but that was just for
                        // backward compatibility because the GStreamer 0.10 payloader used
                        // to (wrongly) produce that at some point a long time ago.
                        // Also spec (and common sense) say clock-rate should always be 90000
                        // (C depayloader accepts any clock rate in caps), see
                        // https://gitlab.freedesktop.org/gstreamer/gst-plugins-good/-/issues/691
                        gst::Structure::builder("application/x-rtp")
                            .field("media", "video")
                            .field("clock-rate", 90000i32)
                            .field("encoding-name", "MP2T")
                            .build(),
                    )
                    .structure(
                        gst::Structure::builder("application/x-rtp")
                            .field("media", "video")
                            .field("payload", 33i32)
                            .field("clock-rate", 90000i32)
                            .build(),
                    )
                    .build(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("video/mpegts")
                    .field("packetsize", gst::List::new([188i32, 192, 204, 208]))
                    .field("systemstream", true)
                    .build(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl crate::basedepay::RtpBaseDepay2Impl for RtpMP2TDepay {
    fn start(&self) -> Result<(), gst::ErrorMessage> {
        let settings = self.settings.lock().unwrap();
        // Copy skip bytes into state so we don't have to take the settings lock all the time
        *self.state.borrow_mut() = State {
            packet_size: None,
            bytes_to_skip: settings.skip_first_bytes as usize,
        };

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        *self.state.borrow_mut() = State::default();

        Ok(())
    }

    // Encapsulation of MPEG System and Transport Streams:
    // https://www.rfc-editor.org/rfc/rfc2250.html#section-2
    //
    fn handle_packet(
        &self,
        packet: &crate::basedepay::Packet,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.borrow_mut();
        let bytes_to_skip = state.bytes_to_skip;

        let payload = packet.payload();

        if payload.len() < 188 + bytes_to_skip {
            gst::warning!(
                CAT,
                imp = self,
                "Payload too small: {} bytes, but need at least {} bytes",
                payload.len(),
                188 + bytes_to_skip
            );

            self.obj().drop_packet(packet);

            return Ok(gst::FlowSuccess::Ok);
        }

        let (_, payload) = payload.split_at(bytes_to_skip);

        if state.packet_size.is_none() {
            state.packet_size = self.detect_packet_size(payload);

            if let Some(packet_size) = state.packet_size {
                let src_caps = gst::Caps::builder("video/mpegts")
                    .field("packetsize", packet_size.get() as i32)
                    .field("systemstream", true)
                    .build();

                self.obj().set_src_caps(&src_caps);
            }
        }

        let Some(packet_size) = state.packet_size else {
            gst::debug!(
                CAT,
                imp = self,
                "Could not determine packet size, dropping packet {packet:?}"
            );
            self.obj().drop_packet(packet);
            return Ok(gst::FlowSuccess::Ok);
        };

        let packet_size = packet_size.get();

        // For MPEG2 Transport Streams the RTP payload will contain an integral
        // number of MPEG transport packets.
        let n_packets = payload.len() / packet_size;

        if payload.len() % packet_size != 0 {
            gst::warning!(
                CAT,
                imp = self,
                "Payload does not contain an integral number of MPEG-TS packets! ({} left over)",
                payload.len() % packet_size
            );
        }

        let output_size = n_packets * packet_size;

        gst::trace!(
            CAT,
            imp = self,
            "Packet with {n_packets} MPEG-TS packets of size {packet_size}"
        );

        let mut buffer =
            packet.payload_subbuffer_from_offset_with_length(bytes_to_skip, output_size);

        // Marker flag indicates MPEG-TS timestamping discontinuity
        if packet.marker_bit() {
            let buffer_ref = buffer.get_mut().unwrap();
            buffer_ref.set_flags(gst::BufferFlags::RESYNC);
        }

        gst::trace!(CAT, imp = self, "Finishing buffer {buffer:?}");

        self.obj().queue_buffer(packet.into(), buffer)
    }
}

impl RtpMP2TDepay {
    fn detect_packet_size(&self, payload: &[u8]) -> Option<NonZeroUsize> {
        const PACKET_SIZES: [(usize, usize); 4] = [(188, 0), (192, 4), (204, 0), (208, 0)];

        for (size, offset) in PACKET_SIZES {
            gst::debug!(
                CAT,
                imp = self,
                "Trying MPEG-TS packet size of {size} bytes.."
            );

            // Try exact size match for the payload first
            if payload.len() >= size
                && payload.len() % size == 0
                && payload
                    .chunks_exact(size)
                    .all(|packet| packet[offset] == TS_PACKET_SYNC)
            {
                gst::info!(
                    CAT,
                    imp = self,
                    "Detected MPEG-TS packet size of {size} bytes, {} packets",
                    payload.len() / size
                );
                return NonZeroUsize::new(size);
            }
        }

        gst::warning!(
            CAT,
            imp = self,
            "Could not detect MPEG-TS packet size using full payload"
        );

        // No match? Try if we find a size if we ignore any leftover bytes
        for (size, offset) in PACKET_SIZES {
            gst::debug!(
                CAT,
                imp = self,
                "Trying MPEG-TS packet size of {size} bytes with remainder.."
            );

            if payload.len() >= size
                && payload.len() % size != 0
                && payload
                    .chunks_exact(size)
                    .all(|packet| packet[offset] == TS_PACKET_SYNC)
            {
                gst::info!(
                    CAT,
                    imp = self,
                    "Detected MPEG-TS packet size of {size} bytes, {} packets, {} bytes leftover",
                    payload.len() / size,
                    payload.len() % size
                );
                return NonZeroUsize::new(size);
            }
        }

        gst::warning!(CAT, imp = self, "Could not detect MPEG-TS packet size");

        None
    }
}
