// GStreamer RTP MPEG-4 Audio Payloader
//
// Copyright (C) 2023 François Laignel <francois centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-rtpmp4apay2
 * @see_also: rtpmp4apay2, rtpmp4apay, fdkaacenc
 *
 * Payload an MPEG-4 Audio bitstream into RTP packets as per [RFC 3016][rfc-3016].
 * Also see the [IANA media-type page for MPEG-4 Advanced Audio Coding][iana-aac].
 *
 * [rfc-3016]: https://www.rfc-editor.org/rfc/rfc3016.html#section-4
 * [iana-aac]: https://www.iana.org/assignments/media-types/audio/aac
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 audiotestsrc ! fdkaacenc ! rtpmp4apay2 ! udpsink host=127.0.0.1 port=5004
 * ]| This will encode an audio test signal to AAC and then payload the encoded audio
 * into RTP packets and send them out via UDP to localhost (IPv4) port 5004.
 * You can use the #rtpmp4adepay2 or #rtpmp4adepay elements to depayload such a stream, and
 * the #fdkaacdec element to decode the depayloaded stream.
 *
 * Since: plugins-rs-0.13.0
 */
use bitstream_io::{BigEndian, BitRead, BitReader, BitWrite, BitWriter};
use smallvec::SmallVec;
use std::sync::LazyLock;

use gst::{glib, subclass::prelude::*};

use crate::basepay::{RtpBasePay2Ext, RtpBasePay2Impl};

use crate::mp4a::parsers::AudioSpecificConfig;
use crate::mp4a::ENCODING_NAME;

#[derive(Default)]
pub struct RtpMpeg4AudioPay;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpmp4apay2",
        gst::DebugColorFlags::empty(),
        Some("RTP MPEG-4 Audio Payloader"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for RtpMpeg4AudioPay {
    const NAME: &'static str = "GstRtpMpeg4AudioPay";
    type Type = super::RtpMpeg4AudioPay;
    type ParentType = crate::basepay::RtpBasePay2;
}

impl ObjectImpl for RtpMpeg4AudioPay {}
impl GstObjectImpl for RtpMpeg4AudioPay {}

impl ElementImpl for RtpMpeg4AudioPay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP MPEG-4 Audio Payloader",
                "Codec/Payloader/Network/RTP",
                "Payload an MPEG-4 Audio bitstream (e.g. AAC) into RTP packets (RFC 3016)",
                "François Laignel <francois centricular com>",
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
                &gst::Caps::builder("audio/mpeg")
                    .field("mpegversion", 4i32)
                    .field("framed", true)
                    .field("stream-format", "raw")
                    .build(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("application/x-rtp")
                    .field("media", "audio")
                    .field("clock-rate", gst::IntRange::new(1i32, i32::MAX))
                    .field("encoding-name", ENCODING_NAME)
                    /* All optional parameters
                     *
                     * "profile-level-id=[1,MAX]"
                     * "cpresent="
                     * "config="
                     */
                    .build(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

#[derive(Debug)]
struct ConfigWithCodecData {
    audio_config: AudioSpecificConfig,
    config_data: SmallVec<[u8; 4]>,
}

impl ConfigWithCodecData {
    fn from_codec_data(s: &gst::StructureRef) -> anyhow::Result<ConfigWithCodecData> {
        use anyhow::Context;

        let codec_data = s
            .get::<gst::Buffer>("codec_data")
            .context("codec_data field")?;
        let codec_data_ref = codec_data.map_readable().context("mapping codec_data")?;

        if codec_data_ref.size() != 2 {
            anyhow::bail!("Unsupported size {} for codec_data", codec_data_ref.size());
        }

        let mut r = BitReader::endian(codec_data_ref.as_slice(), BigEndian);
        let audio_config = r.parse::<AudioSpecificConfig>()?;

        let mut config_data = SmallVec::new();
        let mut w = BitWriter::endian(&mut config_data, BigEndian);

        // StreamMuxConfig - ISO/IEC 14496-3 sub 1 table 1.21
        // audioMuxVersion           == 0 (1 bit)
        // allStreamsSameTimeFraming == 1 (1 bit)
        // numSubFrames              == 0 means 1 subframe (6 bits)
        // numProgram                == 0 means 1 program (4 bits)
        // numLayer                  == 0 means 1 layer (3 bits)

        w.write::<1, _>(0).unwrap();
        w.write_bit(true).unwrap();
        w.write::<13, _>(0).unwrap();
        // 1 bit missing for byte alignment

        // Append AudioSpecificConfig for prog 1 layer 1 (from codec_data)
        for byte in codec_data_ref.as_slice() {
            w.write::<8, _>(*byte).context("appending codec_data")?
        }

        // Padding
        w.write::<7, _>(0).unwrap();

        Ok(ConfigWithCodecData {
            audio_config,
            config_data,
        })
    }
}

impl RtpBasePay2Impl for RtpMpeg4AudioPay {
    const ALLOWED_META_TAGS: &'static [&'static str] = &["audio"];

    fn set_sink_caps(&self, caps: &gst::Caps) -> bool {
        let s = caps.structure(0).unwrap();

        let (config, config_data) = match ConfigWithCodecData::from_codec_data(s) {
            Ok(c) => (c.audio_config, c.config_data),
            Err(err) => {
                gst::error!(CAT, imp = self, "Unusable codec_data: {err:#}");
                return false;
            }
        };

        let rate = if let Ok(rate) = s.get::<i32>("rate") {
            rate
        } else {
            config.sampling_freq as i32
        };

        self.obj().set_src_caps(
            &gst::Caps::builder("application/x-rtp")
                .field("media", "audio")
                .field("encoding-name", ENCODING_NAME)
                .field("clock-rate", rate)
                .field("profile-level-id", config.audio_object_type)
                .field("cpresent", 0)
                .field("config", hex::encode(config_data))
                .build(),
        );

        true
    }

    // Encapsulation of MPEG-4 Audio bitstream:
    // https://www.rfc-editor.org/rfc/rfc3016.html#section-4
    //
    // We either put 1 whole AAC frame into a single RTP packet,
    // or fragment a single AAC frame over multiple RTP packets.
    //
    fn handle_buffer(
        &self,
        buffer: &gst::Buffer,
        id: u64,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        if buffer.size() == 0 {
            gst::info!(CAT, imp = self, "Dropping empty buffer {id}");
            self.obj().drop_buffers(..=id);

            return Ok(gst::FlowSuccess::Ok);
        }

        let Ok(buffer_ref) = buffer.map_readable() else {
            gst::error!(CAT, imp = self, "Failed to map buffer {id} readable");

            return Err(gst::FlowError::Error);
        };

        let max_payload_size = self.obj().max_payload_size() as usize;

        let mut size_prefix = SmallVec::<[u8; 3]>::new();
        let mut rem_size = buffer_ref.size();
        while rem_size > 0xff {
            size_prefix.push(0xff);
            rem_size -= 0xff;
        }
        size_prefix.push(rem_size as u8);

        if max_payload_size < size_prefix.len() {
            gst::error!(
                CAT,
                imp = self,
                "Insufficient max-payload-size {} for buffer {id} at least {} bytes needed",
                self.obj().max_payload_size(),
                size_prefix.len() + 1,
            );
            self.obj().drop_buffers(..=id);

            return Err(gst::FlowError::Error);
        }

        let mut rem_data = buffer_ref.as_slice();
        let mut is_first = true;
        while !rem_data.is_empty() {
            let mut packet = rtp_types::RtpPacketBuilder::new();

            let chunk_size = if is_first {
                packet = packet.payload(size_prefix.as_slice());

                std::cmp::min(rem_data.len(), max_payload_size - size_prefix.len())
            } else {
                std::cmp::min(rem_data.len(), max_payload_size)
            };

            let payload = &rem_data[..chunk_size];
            rem_data = &rem_data[chunk_size..];

            // The marker bit indicates audioMuxElement boundaries.
            // It is set to one to indicate that the RTP packet contains a complete
            // audioMuxElement or the last fragment of an audioMuxElement.
            let marker = rem_data.is_empty();

            gst::log!(
                CAT,
                imp = self,
                "Queuing {}packet with size {} for {}buffer {id}",
                if marker { "marked " } else { "" },
                payload.len(),
                if !marker || !is_first {
                    "fragmented "
                } else {
                    ""
                },
            );

            self.obj()
                .queue_packet(id.into(), packet.payload(payload).marker_bit(marker))?;

            is_first = false;
        }

        self.obj().finish_pending_packets()?;

        Ok(gst::FlowSuccess::Ok)
    }
}
