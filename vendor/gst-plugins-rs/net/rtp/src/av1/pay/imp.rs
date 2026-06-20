//
// Copyright (C) 2022 Vivienne Watermeier <vwatermeier@igalia.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use atomic_refcell::AtomicRefCell;
use gst::{glib, subclass::prelude::*};
use std::{
    collections::VecDeque,
    io::{Cursor, Read, Seek, SeekFrom, Write},
};

use bitstream_io::{BitReader, BitWriter};
use std::sync::LazyLock;

use crate::{
    av1::common::{err_flow, leb128_size, write_leb128, ObuType, SizedObu, CLOCK_RATE, ENDIANNESS},
    basepay::{PacketToBufferRelation, RtpBasePay2Ext},
};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpav1pay",
        gst::DebugColorFlags::empty(),
        Some("RTP AV1 Payloader"),
    )
});

/// Information about the OBUs intended to be grouped into one packet
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct PacketOBUData {
    obu_count: usize,
    payload_size: u32,
    start_of_coded_video_sequence: bool,
    last_obu_fragment_size: Option<u32>,
    omit_last_size_field: bool,
    ends_temporal_unit: bool,
}

impl Default for PacketOBUData {
    fn default() -> Self {
        PacketOBUData {
            payload_size: 1, // 1 byte is used for the aggregation header
            omit_last_size_field: true,
            start_of_coded_video_sequence: false,
            obu_count: 0,
            last_obu_fragment_size: None,
            ends_temporal_unit: false,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ObuData {
    info: SizedObu,
    keyframe: bool,
    bytes: Vec<u8>,
    offset: usize,
    id: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct State {
    /// Holds header information and raw bytes for all received OBUs,
    /// as well as DTS and PTS
    obus: VecDeque<ObuData>,

    /// Indicates that the first element in the Buffer is an OBU fragment,
    /// left over from the previous RTP packet
    open_obu_fragment: bool,

    /// If the input is TU or frame aligned.
    framed: bool,
}

#[derive(Debug, Default)]
pub struct RTPAv1Pay {
    state: AtomicRefCell<State>,
}

impl RTPAv1Pay {
    fn reset(&self, state: &mut State, full: bool) {
        gst::debug!(CAT, imp = self, "resetting state");

        if full {
            *state = State::default();
        } else {
            *state = State {
                framed: state.framed,
                ..State::default()
            };
        }
    }

    /// Parses new OBUs, stores them in the state,
    /// and constructs and sends new RTP packets when appropriate.
    fn handle_new_obus(
        &self,
        state: &mut State,
        id: u64,
        data: &[u8],
        keyframe: bool,
        marker: bool,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut reader = Cursor::new(data);

        while reader.position() < data.len() as u64 {
            let obu_start = reader.position();
            let obu = SizedObu::parse(&mut BitReader::endian(&mut reader, ENDIANNESS))
                .map_err(err_flow!(self, buf_read))?;

            // tile lists and temporal delimiters should not be transmitted,
            // see section 5 of the RTP AV1 spec
            match obu.obu_type {
                // completely ignore tile lists and padding
                ObuType::TileList | ObuType::Padding => {
                    gst::log!(CAT, imp = self, "ignoring {:?} OBU", obu.obu_type);
                    reader
                        .seek(SeekFrom::Current(obu.size as i64))
                        .map_err(err_flow!(self, buf_read))?;
                }

                // keep these OBUs around for now so we know where temporal units end
                ObuType::TemporalDelimiter => {
                    if obu.size != 0 {
                        gst::element_imp_error!(
                            self,
                            gst::ResourceError::Read,
                            ["temporal delimiter OBUs should have empty payload"]
                        );
                        return Err(gst::FlowError::Error);
                    }
                    state.obus.push_back(ObuData {
                        info: obu,
                        keyframe,
                        bytes: Vec::new(),
                        offset: 0,
                        id,
                    });
                }

                _ => {
                    let bytes_total = (obu.header_len + obu.size) as usize;
                    let mut bytes = vec![0; bytes_total];

                    // read header
                    reader
                        .seek(SeekFrom::Start(obu_start))
                        .map_err(err_flow!(self, buf_read))?;
                    reader
                        .read_exact(&mut bytes[0..(obu.header_len as usize)])
                        .map_err(err_flow!(self, buf_read))?;

                    // skip size field
                    bytes[0] &= !2_u8; // set `has_size_field` to 0
                    reader
                        .seek(SeekFrom::Current(obu.leb_size as i64))
                        .map_err(err_flow!(self, buf_read))?;

                    // read OBU bytes
                    reader
                        .read_exact(&mut bytes[(obu.header_len as usize)..bytes_total])
                        .map_err(err_flow!(self, buf_read))?;

                    state.obus.push_back(ObuData {
                        info: obu,
                        keyframe,
                        bytes,
                        offset: 0,
                        id,
                    });
                }
            }
        }

        while let Some(packet_data) = self.consider_new_packet(state, false, marker) {
            self.generate_new_packet(state, packet_data)?;
        }

        Ok(gst::FlowSuccess::Ok)
    }

    /// Look at the size the currently stored OBUs would require,
    /// as well as their temporal IDs to decide if it is time to construct a
    /// new packet, and what OBUs to include in it.
    ///
    /// If `true` is passed for `force`, packets of any size will be accepted,
    /// which is used in flushing the last OBUs after receiving an EOS for example.
    ///
    /// If `true` is passed for `marker` then all queued OBUs are considered to finish this TU.
    fn consider_new_packet(
        &self,
        state: &mut State,
        force: bool,
        marker: bool,
    ) -> Option<PacketOBUData> {
        gst::trace!(
            CAT,
            imp = self,
            "{} new packet, currently storing {} OBUs (marker {})",
            if force { "forcing" } else { "considering" },
            state.obus.len(),
            marker,
        );

        let payload_limit = self.obj().max_payload_size();

        // Create information about the packet that can be created now while iterating over the
        // OBUs and return this if a full packet can indeed be created now.
        let mut packet = PacketOBUData::default();
        let mut pending_bytes = 0;
        let mut required_ids = None::<(u8, u8)>;

        // Detect if this packet starts a keyframe and contains a sequence header, and if so
        // set the N flag to indicate that this is the start of a new codec video sequence.
        let mut contains_keyframe = false;
        let mut contains_sequence_header = false;

        // figure out how many OBUs we can fit into this packet
        for (idx, obu) in state.obus.iter().enumerate() {
            // for OBUs with extension headers, spatial and temporal IDs must be equal
            // to all other such OBUs in the packet
            let matching_obu_ids = |obu: &SizedObu, required_ids: &mut Option<(u8, u8)>| -> bool {
                if let Some((sid, tid)) = *required_ids {
                    sid == obu.spatial_id && tid == obu.temporal_id
                } else {
                    *required_ids = Some((obu.spatial_id, obu.temporal_id));
                    true
                }
            };

            let current = &obu.info;

            // should this packet be finished here?
            if current.obu_type == ObuType::TemporalDelimiter {
                // ignore the temporal delimiter, it is not supposed to be transmitted,
                // it will be skipped later when building the packet
                gst::log!(CAT, imp = self, "ignoring temporal delimiter OBU");

                if packet.obu_count > 0 {
                    if marker {
                        gst::warning!(
                            CAT,
                            imp = self,
                            "Temporal delimited in the middle of a frame"
                        );
                    }

                    packet.start_of_coded_video_sequence =
                        contains_keyframe && contains_sequence_header;
                    packet.ends_temporal_unit = true;
                    if packet.obu_count > 3 {
                        packet.payload_size += pending_bytes;
                        packet.omit_last_size_field = false;
                    }

                    return Some(packet);
                }

                contains_keyframe |= obu.keyframe;
                continue;
            } else if packet.payload_size >= payload_limit
                || (packet.obu_count > 0 && current.obu_type == ObuType::SequenceHeader)
                || !matching_obu_ids(current, &mut required_ids)
            {
                if packet.obu_count > 3 {
                    packet.payload_size += pending_bytes;
                    packet.omit_last_size_field = false;
                }
                packet.start_of_coded_video_sequence =
                    contains_keyframe && contains_sequence_header;
                packet.ends_temporal_unit = marker && idx == state.obus.len() - 1;
                return Some(packet);
            }

            // would the full OBU fit?
            if packet.payload_size + pending_bytes + current.full_size() <= payload_limit {
                packet.obu_count += 1;
                packet.payload_size += current.partial_size() + pending_bytes;
                contains_keyframe |= obu.keyframe;
                contains_sequence_header |= obu.info.obu_type == ObuType::SequenceHeader;
                pending_bytes = current.leb_size;
            }
            // would it fit without the size field?
            else if packet.obu_count < 3
                && packet.payload_size + pending_bytes + current.partial_size() <= payload_limit
            {
                packet.obu_count += 1;
                packet.payload_size += current.partial_size() + pending_bytes;
                contains_keyframe |= obu.keyframe;
                contains_sequence_header |= obu.info.obu_type == ObuType::SequenceHeader;
                packet.start_of_coded_video_sequence =
                    contains_keyframe && contains_sequence_header;
                packet.ends_temporal_unit = marker && idx == state.obus.len() - 1;

                return Some(packet);
            }
            // otherwise consider putting an OBU fragment
            else {
                let leb_size = if packet.obu_count < 3 {
                    0
                } else {
                    // assume the biggest possible OBU fragment,
                    // so if anything the size field will be smaller than expected
                    leb128_size(payload_limit - packet.payload_size) as u32
                };

                // is there even enough space to bother?
                if packet.payload_size + pending_bytes + leb_size + current.header_len
                    < payload_limit
                {
                    packet.obu_count += 1;
                    packet.last_obu_fragment_size =
                        Some(payload_limit - packet.payload_size - pending_bytes - leb_size);
                    packet.payload_size = payload_limit;
                    packet.omit_last_size_field = leb_size == 0;
                    contains_keyframe |= obu.keyframe;
                    contains_sequence_header |= obu.info.obu_type == ObuType::SequenceHeader;
                } else if packet.obu_count > 3 {
                    packet.ends_temporal_unit = marker && idx == state.obus.len() - 1;
                    packet.payload_size += pending_bytes;
                }

                packet.start_of_coded_video_sequence =
                    contains_keyframe && contains_sequence_header;

                return Some(packet);
            }
        }

        if (force || marker) && packet.obu_count > 0 {
            if packet.obu_count > 3 {
                packet.payload_size += pending_bytes;
                packet.omit_last_size_field = false;
            }
            packet.start_of_coded_video_sequence = contains_keyframe && contains_sequence_header;
            packet.ends_temporal_unit = true;

            Some(packet)
        } else {
            // if we ran out of OBUs with space in the packet to spare, wait a bit longer
            None
        }
    }

    /// Given the information returned by consider_new_packet(), construct and return
    /// new RTP packet, filled with those OBUs.
    fn generate_new_packet(
        &self,
        state: &mut State,
        packet: PacketOBUData,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(
            CAT,
            imp = self,
            "constructing new RTP packet with {} OBUs",
            packet.obu_count
        );

        // prepare the outgoing buffer
        let mut payload = Vec::with_capacity(packet.payload_size as usize);
        let mut writer = Cursor::new(&mut payload);

        {
            // construct aggregation header
            let w = if packet.omit_last_size_field && packet.obu_count < 4 {
                packet.obu_count
            } else {
                0
            };

            let aggr_header: [u8; 1] = [
                    ((state.open_obu_fragment as u8) << 7) |                    // Z
                     (((packet.last_obu_fragment_size.is_some()) as u8) << 6) | // Y
                     ((w as u8) << 4) |                                         // W
                     ((packet.start_of_coded_video_sequence as u8) << 3)        // N
                ; 1];

            writer
                .write(&aggr_header)
                .map_err(err_flow!(self, aggr_header_write))?;
        }

        let mut start_id = None;
        let end_id;

        // append OBUs to the buffer
        for _ in 1..packet.obu_count {
            let obu = loop {
                let obu = state.obus.pop_front().unwrap();

                // Drop temporal delimiter from here
                if obu.info.obu_type != ObuType::TemporalDelimiter {
                    break obu;
                }
            };

            if start_id.is_none() {
                start_id = Some(obu.id);
            }

            write_leb128(
                &mut BitWriter::endian(&mut writer, ENDIANNESS),
                obu.info.size + obu.info.header_len,
            )
            .map_err(err_flow!(self, leb_write))?;
            writer
                .write(&obu.bytes[obu.offset..])
                .map_err(err_flow!(self, obu_write))?;
        }
        state.open_obu_fragment = false;

        {
            let last_obu = loop {
                let obu = state.obus.front_mut().unwrap();

                // Drop temporal delimiter from here
                if obu.info.obu_type != ObuType::TemporalDelimiter {
                    break obu;
                }
                let _ = state.obus.pop_front().unwrap();
            };

            if start_id.is_none() {
                start_id = Some(last_obu.id);
            }
            end_id = last_obu.id;

            // do the last OBU separately
            // in this instance `obu_size` includes the header length
            let obu_size = if let Some(size) = packet.last_obu_fragment_size {
                state.open_obu_fragment = true;
                size
            } else {
                last_obu.bytes.len() as u32 - last_obu.offset as u32
            };

            if !packet.omit_last_size_field {
                write_leb128(&mut BitWriter::endian(&mut writer, ENDIANNESS), obu_size)
                    .map_err(err_flow!(self, leb_write))?;
            }

            // if this OBU is not a fragment, handle it as usual
            if packet.last_obu_fragment_size.is_none() {
                writer
                    .write(&last_obu.bytes[last_obu.offset..])
                    .map_err(err_flow!(self, obu_write))?;
                let _ = state.obus.pop_front().unwrap();
            }
            // otherwise write only a slice, and update the element
            // to only contain the unwritten bytes
            else {
                writer
                    .write(&last_obu.bytes[last_obu.offset..last_obu.offset + obu_size as usize])
                    .map_err(err_flow!(self, obu_write))?;

                let new_size = last_obu.bytes.len() as u32 - last_obu.offset as u32 - obu_size;
                last_obu.info = SizedObu {
                    size: new_size,
                    header_len: 0,
                    leb_size: leb128_size(new_size) as u32,
                    is_fragment: true,
                    ..last_obu.info
                };
                last_obu.offset += obu_size as usize;
            }
        }

        // OBUs were consumed above so start_id will be set now
        let start_id = start_id.unwrap();

        gst::log!(
            CAT,
            imp = self,
            "generated RTP packet of size {}",
            payload.len()
        );

        self.obj().queue_packet(
            PacketToBufferRelation::Ids(start_id..=end_id),
            rtp_types::RtpPacketBuilder::new()
                .marker_bit(packet.ends_temporal_unit)
                .payload(&payload),
        )?;

        Ok(gst::FlowSuccess::Ok)
    }
}

#[glib::object_subclass]
impl ObjectSubclass for RTPAv1Pay {
    const NAME: &'static str = "GstRtpAv1Pay";
    type Type = super::RTPAv1Pay;
    type ParentType = crate::basepay::RtpBasePay2;
}

impl ObjectImpl for RTPAv1Pay {}

impl GstObjectImpl for RTPAv1Pay {}

impl ElementImpl for RTPAv1Pay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP AV1 payloader",
                "Codec/Payloader/Network/RTP",
                "Payload AV1 as RTP packets",
                "Vivienne Watermeier <vwatermeier@igalia.com>",
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
                &gst::Caps::builder("video/x-av1")
                    .field("parsed", true)
                    .field("stream-format", "obu-stream")
                    .field("alignment", gst::List::new(["tu", "frame", "obu"]))
                    .build(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("application/x-rtp")
                    .field("media", "video")
                    .field("clock-rate", CLOCK_RATE as i32)
                    .field("encoding-name", "AV1")
                    .build(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl crate::basepay::RtpBasePay2Impl for RTPAv1Pay {
    const ALLOWED_META_TAGS: &'static [&'static str] = &["video"];

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.borrow_mut();
        self.reset(&mut state, true);

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.borrow_mut();
        self.reset(&mut state, true);

        Ok(())
    }

    fn set_sink_caps(&self, caps: &gst::Caps) -> bool {
        gst::debug!(CAT, imp = self, "received caps {caps:?}");

        self.obj().set_src_caps(
            &gst::Caps::builder("application/x-rtp")
                .field("media", "video")
                .field("clock-rate", CLOCK_RATE as i32)
                .field("encoding-name", "AV1")
                .build(),
        );

        let mut state = self.state.borrow_mut();
        let s = caps.structure(0).unwrap();
        match s.get::<&str>("alignment").unwrap() {
            "tu" | "frame" => {
                state.framed = true;
            }
            _ => {
                state.framed = false;
            }
        }

        true
    }

    fn handle_buffer(
        &self,
        buffer: &gst::Buffer,
        id: u64,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::trace!(CAT, imp = self, "received buffer of size {}", buffer.size());

        let mut state = self.state.borrow_mut();
        let map = buffer.map_readable().map_err(|_| {
            gst::element_imp_error!(
                self,
                gst::ResourceError::Read,
                ["Failed to map buffer readable"]
            );

            gst::FlowError::Error
        })?;

        let keyframe = !buffer.flags().contains(gst::BufferFlags::DELTA_UNIT);
        // Does the buffer finished a full TU?
        let marker = buffer.flags().contains(gst::BufferFlags::MARKER) || state.framed;
        let res = self.handle_new_obus(&mut state, id, map.as_slice(), keyframe, marker)?;
        drop(map);
        drop(state);

        Ok(res)
    }

    fn drain(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        // flush all remaining OBUs
        let mut res = Ok(gst::FlowSuccess::Ok);

        let mut state = self.state.borrow_mut();
        while let Some(packet_data) = self.consider_new_packet(&mut state, true, true) {
            res = self.generate_new_packet(&mut state, packet_data);
            if res.is_err() {
                break;
            }
        }

        res
    }

    fn flush(&self) {
        let mut state = self.state.borrow_mut();
        self.reset(&mut state, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::av1::common::*;

    #[test]
    fn test_consider_new_packet() {
        gst::init().unwrap();

        let base_obu = SizedObu {
            has_extension: false,
            has_size_field: true,
            leb_size: 1,
            header_len: 1,
            is_fragment: false,
            ..SizedObu::default()
        };

        let input_data = [
            (
                false, // force argument
                State {
                    // payloader state
                    obus: VecDeque::from(vec![
                        ObuData {
                            info: SizedObu {
                                obu_type: ObuType::Frame,
                                size: 3,
                                ..base_obu
                            },
                            bytes: vec![1, 2, 3],
                            ..ObuData::default()
                        },
                        ObuData {
                            info: SizedObu {
                                obu_type: ObuType::Frame,
                                size: 4,
                                ..base_obu
                            },
                            bytes: vec![1, 2, 3, 4],
                            ..ObuData::default()
                        },
                        ObuData {
                            info: SizedObu {
                                obu_type: ObuType::Frame,
                                size: 5,
                                ..base_obu
                            },
                            bytes: vec![1, 2, 3, 4, 5],
                            ..ObuData::default()
                        },
                        ObuData {
                            // last two OBUs should not be counted
                            info: SizedObu {
                                obu_type: ObuType::TemporalDelimiter,
                                size: 0,
                                ..base_obu
                            },
                            ..ObuData::default()
                        },
                        ObuData {
                            info: SizedObu {
                                obu_type: ObuType::Frame,
                                size: 10,
                                ..base_obu
                            },
                            bytes: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
                            ..ObuData::default()
                        },
                    ]),
                    ..State::default()
                },
            ),
            (
                true,
                State {
                    obus: VecDeque::from(vec![
                        ObuData {
                            info: SizedObu {
                                obu_type: ObuType::TemporalDelimiter,
                                size: 0,
                                ..base_obu
                            },
                            keyframe: true,
                            ..ObuData::default()
                        },
                        ObuData {
                            info: SizedObu {
                                obu_type: ObuType::SequenceHeader,
                                size: 0,
                                ..base_obu
                            },
                            ..ObuData::default()
                        },
                        ObuData {
                            info: SizedObu {
                                obu_type: ObuType::Frame,
                                size: 7,
                                ..base_obu
                            },
                            bytes: vec![1, 2, 3, 4, 5, 6, 7],
                            ..ObuData::default()
                        },
                        ObuData {
                            info: SizedObu {
                                obu_type: ObuType::Frame,
                                size: 6,
                                ..base_obu
                            },
                            bytes: vec![1, 2, 3, 4, 5, 6],
                            ..ObuData::default()
                        },
                        ObuData {
                            info: SizedObu {
                                obu_type: ObuType::Frame,
                                size: 9,
                                ..base_obu
                            },
                            bytes: vec![1, 2, 3, 4, 5, 6, 7, 8, 9],
                            ..ObuData::default()
                        },
                        ObuData {
                            info: SizedObu {
                                obu_type: ObuType::Frame,
                                size: 3,
                                ..base_obu
                            },
                            bytes: vec![1, 2, 3],
                            ..ObuData::default()
                        },
                    ]),
                    ..State::default()
                },
            ),
            (
                false,
                State {
                    obus: VecDeque::from(vec![
                        ObuData {
                            info: SizedObu {
                                obu_type: ObuType::TemporalDelimiter,
                                size: 0,
                                ..base_obu
                            },
                            ..ObuData::default()
                        },
                        ObuData {
                            info: SizedObu {
                                obu_type: ObuType::Frame,
                                size: 4,
                                ..base_obu
                            },
                            bytes: vec![1, 2, 3, 4],
                            ..ObuData::default()
                        },
                    ]),
                    ..State::default()
                },
            ),
        ];

        let results = [
            (
                Some(PacketOBUData {
                    obu_count: 3,
                    payload_size: 18,
                    start_of_coded_video_sequence: false,
                    last_obu_fragment_size: None,
                    omit_last_size_field: true,
                    ends_temporal_unit: true,
                }),
                State {
                    obus: VecDeque::from(vec![
                        input_data[0].1.obus[0].clone(),
                        input_data[0].1.obus[1].clone(),
                        input_data[0].1.obus[2].clone(),
                        input_data[0].1.obus[4].clone(),
                    ]),
                    ..input_data[0].1
                },
            ),
            (
                Some(PacketOBUData {
                    obu_count: 5,
                    payload_size: 36,
                    start_of_coded_video_sequence: true,
                    last_obu_fragment_size: None,
                    omit_last_size_field: false,
                    ends_temporal_unit: true,
                }),
                State {
                    obus: {
                        let mut copy = input_data[1].1.obus.clone();
                        copy.pop_front().unwrap();
                        copy
                    },
                    ..input_data[1].1
                },
            ),
            (
                None,
                State {
                    obus: {
                        let mut copy = input_data[2].1.obus.clone();
                        copy.pop_front().unwrap();
                        copy
                    },
                    ..input_data[2].1
                },
            ),
        ];

        // Element exists just for logging purposes
        let element = glib::Object::new::<crate::av1::pay::RTPAv1Pay>();

        let pay = element.imp();
        for idx in 0..input_data.len() {
            println!("running test {idx}...");

            let mut state = pay.state.borrow_mut();
            *state = input_data[idx].1.clone();

            assert_eq!(
                pay.consider_new_packet(&mut state, input_data[idx].0, false),
                results[idx].0,
            );
            assert_eq!(
                state
                    .obus
                    .iter()
                    .filter(|o| o.info.obu_type != ObuType::TemporalDelimiter
                        && o.info.obu_type != ObuType::Padding)
                    .cloned()
                    .collect::<Vec<_>>(),
                results[idx].1.obus.iter().cloned().collect::<Vec<_>>()
            );
            assert_eq!(state.open_obu_fragment, results[idx].1.open_obu_fragment);
        }
    }
}
