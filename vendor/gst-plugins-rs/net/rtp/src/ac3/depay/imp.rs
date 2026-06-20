// GStreamer RTP AC-3 Audio Depayloader
//
// Copyright (C) 2023 Tim-Philipp Müller <tim centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-rtpac3depay2
 * @see_also: rtpac3pay2, rtpac3depay, rtpac3pay, avdec_ac3, avenc_ac3
 *
 * Depayload an AC-3 Audio Stream from RTP packets as per [RFC 4184][rfc-4184].
 *
 * [rfc-4184]: https://www.rfc-editor.org/rfc/rfc4184.html
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 udpsrc caps='application/x-rtp,media=audio,clock-rate=48000,encoding-name=AC3,payload=96' ! rtpjitterbuffer latency=250 ! rtpac3depay2 ! decodebin3 ! audioconvert ! audioresample ! autoaudiosink
 * ]| This will depayload an incoming RTP AC-3 audio stream and decode it and play it.
 * You can use the `rtpac3pay2` or `rtpac3pay` elements with `avenc_ac3` to create such an RTP stream.
 *
 * Since: plugins-rs-0.13.0
 */
use atomic_refcell::AtomicRefCell;

use gst::{glib, subclass::prelude::*};

use std::sync::LazyLock;

use crate::basedepay::{
    Packet, PacketToBufferRelation, RtpBaseDepay2Ext, RtpBaseDepay2Impl, TimestampOffset,
};

use crate::ac3::ac3_audio_utils;

#[derive(Default)]
pub struct RtpAc3Depay {
    state: AtomicRefCell<State>,
}

#[derive(Debug, PartialEq)]
enum FragType {
    NotFragmented,
    Start,
    Continuation,
    End,
}

#[derive(Debug)]
struct FragmentedFrame {
    data: Vec<u8>,
    ext_seqnum: u64,
    ext_timestamp: u64,
}

#[derive(Default)]
struct State {
    last_frame_header: Option<ac3_audio_utils::FrameHeader>,
    partial_frame: Option<FragmentedFrame>,
    clock_rate: Option<i32>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpac3depay2",
        gst::DebugColorFlags::empty(),
        Some("RTP AC-3 Audio Depayloader"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for RtpAc3Depay {
    const NAME: &'static str = "GstRtpAc3Depay";
    type Type = super::RtpAc3Depay;
    type ParentType = crate::basedepay::RtpBaseDepay2;
}

impl ObjectImpl for RtpAc3Depay {}

impl GstObjectImpl for RtpAc3Depay {}

impl ElementImpl for RtpAc3Depay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP AC-3 Audio Depayloader",
                "Codec/Depayloader/Network/RTP",
                "Depayload an AC-3 Audio Stream from RTP packets (RFC 4184)",
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
                &gst::Caps::builder("application/x-rtp")
                    .field("media", "audio")
                    .field("encoding-name", "AC3")
                    .field("clock-rate", gst::List::new([48000i32, 44100, 32000]))
                    .build(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("audio/x-ac3")
                    .field("channels", gst::IntRange::new(1, 6))
                    .field("rate", gst::List::new([48000i32, 44100, 32000]))
                    .field("framed", true)
                    .field("alignment", "frame")
                    .build(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl RtpBaseDepay2Impl for RtpAc3Depay {
    const ALLOWED_META_TAGS: &'static [&'static str] = &["audio"];

    fn set_sink_caps(&self, caps: &gst::Caps) -> bool {
        let s = caps.structure(0).unwrap();

        let mut state = self.state.borrow_mut();

        state.clock_rate = s.get::<i32>("clock-rate").ok();

        // We'll set output caps later based on the frame header
        true
    }

    // Encapsulation of AC-3 Audio Streams:
    // https://www.rfc-editor.org/rfc/rfc4184.html#section-4
    //
    // We either get 1-N whole AC-3 audio frames in an RTP packet,
    // or a single AC-3 audio frame split over multiple RTP packets.
    fn handle_packet(&self, packet: &Packet) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.borrow_mut();

        if packet.discont() {
            state.partial_frame = None;
        }

        let payload = packet.payload();

        if payload.len() < 2 + 6 {
            gst::warning!(
                CAT,
                imp = self,
                "Payload too small: {} bytes, but need at least 8 bytes",
                payload.len(),
            );
            state.partial_frame = None;
            self.obj().drop_packet(packet);
            return Ok(gst::FlowSuccess::Ok);
        }

        gst::log!(
            CAT,
            imp = self,
            "Have payload of {} bytes, header {:02x?} {:02x?}",
            payload.len(),
            payload[0],
            payload[1],
        );

        // AC-3 specific header

        let frag_type = match payload[0] & 0x03 {
            0 => FragType::NotFragmented,
            1 | 2 => FragType::Start,
            3 => {
                if packet.marker_bit() {
                    FragType::End
                } else {
                    FragType::Continuation
                }
            }
            _ => unreachable!(),
        };

        let num_frames_or_frags = payload[1] as usize;

        // Clear out unfinished pending partial frame if needed

        if frag_type == FragType::Start || frag_type == FragType::NotFragmented {
            if let Some(partial_frame) = state.partial_frame.as_ref() {
                gst::warning!(CAT, imp = self, "Dropping unfinished partial frame");

                self.obj()
                    .drop_packets(partial_frame.ext_seqnum..=packet.ext_seqnum() - 1);

                state.partial_frame = None;
            }
        }

        // Skip to AC-3 payload data

        let payload = &payload[2..];

        match frag_type {
            FragType::Start => {
                let mut data = Vec::with_capacity(num_frames_or_frags * payload.len());
                data.extend_from_slice(payload);

                state.partial_frame = Some(FragmentedFrame {
                    data,
                    ext_seqnum: packet.ext_seqnum(),
                    ext_timestamp: packet.ext_timestamp(),
                });

                gst::trace!(CAT, imp = self, "Partial frame {:?}", state.partial_frame);

                return Ok(gst::FlowSuccess::Ok);
            }

            FragType::Continuation | FragType::End => {
                let Some(partial_frame) = state.partial_frame.as_mut() else {
                    gst::debug!(
                        CAT,
                        imp = self,
                        "{frag_type:?} packet but no partial frame (most likely indicates packet loss)",
                    );
                    self.obj().drop_packet(packet);
                    state.partial_frame = None;
                    return Ok(gst::FlowSuccess::Ok);
                };

                if partial_frame.ext_timestamp != packet.ext_timestamp() {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "{frag_type:?} packet timestamp {} doesn't match existing partial fragment timestamp {}",
                        packet.ext_timestamp(),
                        partial_frame.ext_timestamp,
                    );
                    state.partial_frame = None;
                    self.obj().drop_packet(packet);
                    return Ok(gst::FlowSuccess::Ok);
                }

                partial_frame.data.extend_from_slice(payload);

                gst::log!(
                    CAT,
                    imp = self,
                    "Added {frag_type:?} packet payload, assembled {} bytes now",
                    partial_frame.data.len()
                );

                if frag_type == FragType::End {
                    let partial_frame = state.partial_frame.take().unwrap();

                    let Ok(hdr) = ac3_audio_utils::peek_frame_header(&partial_frame.data) else {
                        gst::warning!(
                            CAT,
                            imp = self,
                            "Could not parse frame header, dropping frame"
                        );
                        self.obj()
                            .drop_packets(partial_frame.ext_seqnum..=packet.ext_seqnum());
                        return Ok(gst::FlowSuccess::Ok);
                    };

                    gst::trace!(CAT, imp = self, "{hdr:?}");

                    if partial_frame.data.len() != hdr.frame_len {
                        gst::warning!(
                            CAT,
                            imp = self,
                            "Partial frame finished, but have {} bytes, and expected {} bytes!",
                            partial_frame.data.len(),
                            hdr.frame_len,
                        );
                    }

                    self.ensure_output_caps(&mut state, &hdr);

                    let mut outbuf = gst::Buffer::from_mut_slice(partial_frame.data);

                    let outbuf_ref = outbuf.get_mut().unwrap();

                    outbuf_ref.set_duration(gst::ClockTime::from_nseconds(hdr.duration()));

                    gst::trace!(CAT, imp = self, "Finishing buffer {outbuf:?}");

                    return self.obj().queue_buffer(
                        PacketToBufferRelation::Seqnums(
                            partial_frame.ext_seqnum..=packet.ext_seqnum(),
                        ),
                        outbuf,
                    );
                }

                // Wait for more frame fragments
                return Ok(gst::FlowSuccess::Ok);
            }

            FragType::NotFragmented => {
                let mut offset = 0;
                let mut ts_offset = 0;

                while offset < payload.len() {
                    let Ok(hdr) = ac3_audio_utils::peek_frame_header(&payload[offset..]) else {
                        gst::warning!(
                            CAT,
                            imp = self,
                            "Could not parse frame header at offset {offset}"
                        );
                        break;
                    };

                    gst::trace!(CAT, imp = self, "{hdr:?} at offset {offset}");

                    let frame_len = if offset + hdr.frame_len <= payload.len() {
                        hdr.frame_len
                    } else {
                        gst::warning!(
                            CAT,
                            imp = self,
                            "Frame at offset {offset} is {} bytes, but we have only {} bytes left!",
                            hdr.frame_len,
                            payload.len() - offset,
                        );
                        // We'll still push out what we have, there might be decodable blocks
                        payload.len() - offset
                    };

                    self.ensure_output_caps(&mut state, &hdr);

                    gst::trace!(CAT, imp = self, "Getting frame @ {offset}+{frame_len}");

                    // The packet has a 2-byte payload header that we need to skip here too
                    let mut outbuf =
                        packet.payload_subbuffer_from_offset_with_length(2 + offset, frame_len);

                    let outbuf_ref = outbuf.get_mut().unwrap();

                    outbuf_ref.set_duration(gst::ClockTime::from_nseconds(hdr.duration()));

                    gst::trace!(
                        CAT,
                        imp = self,
                        "Finishing frame @ {offset}, buffer {outbuf:?}"
                    );

                    self.obj().queue_buffer(
                        PacketToBufferRelation::SeqnumsWithOffset {
                            seqnums: packet.ext_seqnum()..=packet.ext_seqnum(),
                            timestamp_offset: TimestampOffset::Pts(
                                gst::Signed::<gst::ClockTime>::from(ts_offset as i64),
                            ),
                        },
                        outbuf,
                    )?;

                    offset += frame_len;
                    ts_offset += hdr.duration();
                }
            }
        }

        Ok(gst::FlowSuccess::Ok)
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        *self.state.borrow_mut() = State::default();

        Ok(())
    }
}

impl RtpAc3Depay {
    fn ensure_output_caps(&self, state: &mut State, frame_header: &ac3_audio_utils::FrameHeader) {
        let update_caps = state.last_frame_header.as_ref() != Some(frame_header);

        if update_caps {
            if state.clock_rate != Some(frame_header.sample_rate as i32) {
                gst::warning!(
                    CAT,
                    imp = self,
                    "clock-rate {} does not match sample rate {}!",
                    state.clock_rate.unwrap(),
                    frame_header.sample_rate,
                );
            }

            let src_caps = gst::Caps::builder("audio/x-ac3")
                .field("rate", frame_header.sample_rate as i32)
                .field("channels", frame_header.channels as i32)
                .field("framed", true)
                .field("alignment", "frame")
                .build();

            gst::info!(CAT, imp = self, "Setting output caps {src_caps}..");

            // Ignore failure here and let the next buffer push yield an appropriate flow return
            self.obj().set_src_caps(&src_caps);

            state.last_frame_header = Some(frame_header.clone());
        }
    }
}
