//
// Copyright (C) 2022 Vivienne Watermeier <vwatermeier@igalia.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use atomic_refcell::AtomicRefCell;
use gst::{glib, prelude::*, subclass::prelude::*};
use std::{
    cmp::Ordering,
    io::{Cursor, Read, Seek, SeekFrom},
    ops::RangeInclusive,
    sync::Mutex,
};

use bitstream_io::{BitReader, BitWriter};
use std::sync::LazyLock;

use crate::{
    av1::common::{
        err_flow, leb128_size, parse_leb128, write_leb128, AggregationHeader, ObuType, SizedObu,
        UnsizedObu, CLOCK_RATE, ENDIANNESS,
    },
    basedepay::PacketToBufferRelation,
};

use crate::basedepay::RtpBaseDepay2Ext;

#[derive(Clone, Default)]
struct Settings {
    request_keyframe: bool,
    wait_for_keyframe: bool,
}

struct PendingFragment {
    ext_seqnum: u64,
    bytes: Vec<u8>,
}

struct State {
    last_timestamp: Option<u64>,
    /// if true, the last packet of a temporal unit has been received
    marked_packet: bool,
    /// if the next output buffer needs the DISCONT flag set
    needs_discont: bool,
    /// if we saw a valid OBU since the last reset
    found_valid_obu: bool,
    /// holds data for a fragment
    obu_fragment: Option<PendingFragment>,
    /// if we saw a keyframe since the last discont
    seen_keyframe: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            last_timestamp: None,
            marked_packet: false,
            needs_discont: true,
            found_valid_obu: false,
            obu_fragment: None,
            seen_keyframe: false,
        }
    }
}

#[derive(Default)]
pub struct RTPAv1Depay {
    state: AtomicRefCell<State>,
    settings: Mutex<Settings>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpav1depay",
        gst::DebugColorFlags::empty(),
        Some("RTP AV1 Depayloader"),
    )
});

static TEMPORAL_DELIMITER: [u8; 2] = [0b0001_0010, 0];

impl RTPAv1Depay {
    fn reset(&self, state: &mut State) {
        gst::debug!(CAT, imp = self, "resetting state");

        *state = State::default()
    }
}

#[glib::object_subclass]
impl ObjectSubclass for RTPAv1Depay {
    const NAME: &'static str = "GstRtpAv1Depay";
    type Type = super::RTPAv1Depay;
    type ParentType = crate::basedepay::RtpBaseDepay2;
}

impl ObjectImpl for RTPAv1Depay {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecBoolean::builder("request-keyframe")
                    .nick("Request Keyframe")
                    .blurb("Request new keyframe when packet loss is detected")
                    .default_value(Settings::default().request_keyframe)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("wait-for-keyframe")
                    .nick("Wait For Keyframe")
                    .blurb("Wait for the next keyframe after packet loss")
                    .default_value(Settings::default().wait_for_keyframe)
                    .mutable_ready()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "request-keyframe" => {
                self.settings.lock().unwrap().request_keyframe = value.get().unwrap();
            }
            "wait-for-keyframe" => {
                self.settings.lock().unwrap().wait_for_keyframe = value.get().unwrap();
            }
            _ => unimplemented!(),
        };
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "request-keyframe" => self.settings.lock().unwrap().request_keyframe.to_value(),
            "wait-for-keyframe" => self.settings.lock().unwrap().wait_for_keyframe.to_value(),
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for RTPAv1Depay {}

impl ElementImpl for RTPAv1Depay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP AV1 Depayloader",
                "Codec/Depayloader/Network/RTP",
                "Depayload AV1 from RTP packets",
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
                &gst::Caps::builder("application/x-rtp")
                    .field("media", "video")
                    .field("clock-rate", CLOCK_RATE as i32)
                    .field("encoding-name", "AV1")
                    .build(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("video/x-av1")
                    .field("parsed", true)
                    .field("stream-format", "obu-stream")
                    .field("alignment", "obu")
                    .build(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl crate::basedepay::RtpBaseDepay2Impl for RTPAv1Depay {
    const ALLOWED_META_TAGS: &'static [&'static str] = &["video"];

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.borrow_mut();
        self.reset(&mut state);

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.borrow_mut();
        self.reset(&mut state);

        Ok(())
    }

    fn set_sink_caps(&self, _caps: &gst::Caps) -> bool {
        self.obj()
            .set_src_caps(&self.obj().src_pad().pad_template_caps());

        true
    }

    fn flush(&self) {
        let mut state = self.state.borrow_mut();
        self.reset(&mut state);
    }

    fn handle_packet(
        &self,
        packet: &crate::basedepay::Packet,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        match self.handle_rtp_packet(packet) {
            Ok(Some((seqnums, buffer))) => self
                .obj()
                .queue_buffer(PacketToBufferRelation::Seqnums(seqnums), buffer),
            Ok(None) => Ok(gst::FlowSuccess::Ok),
            Err(err) => {
                gst::warning!(CAT, imp = self, "Failed to handle RTP packet: {err:?}");
                self.reset(&mut self.state.borrow_mut());
                self.obj().drop_packets(..=packet.ext_seqnum());
                Ok(gst::FlowSuccess::Ok)
            }
        }
    }
}

impl RTPAv1Depay {
    fn handle_rtp_packet(
        &self,
        packet: &crate::basedepay::Packet,
    ) -> Result<Option<(RangeInclusive<u64>, gst::Buffer)>, gst::FlowError> {
        gst::trace!(CAT, imp = self, "Processing RTP packet {packet:?}",);

        let settings = self.settings.lock().unwrap().clone();
        let mut state = self.state.borrow_mut();

        let mut reader = Cursor::new(packet.payload());
        let mut ready_obus = Vec::new();

        let aggr_header = {
            let mut byte = [0; 1];
            reader
                .read_exact(&mut byte)
                .map_err(err_flow!(self, aggr_header_read))?;
            AggregationHeader::from(&byte)
        };

        gst::trace!(CAT, imp = self, "Aggregation header {aggr_header:?}");

        // handle new temporal units
        if state.marked_packet || state.last_timestamp != Some(packet.ext_timestamp()) {
            if state.last_timestamp.is_some() && state.obu_fragment.is_some() {
                gst::error!(
                    CAT,
                    imp = self,
                    concat!(
                        "invalid packet: packet is part of a new TU but ",
                        "the previous TU still has an incomplete OBU",
                        "marked_packet: {}, last_timestamp: {:?}"
                    ),
                    state.marked_packet,
                    state.last_timestamp
                );
                self.reset(&mut state);
                self.obj().drop_packets(..packet.ext_seqnum());
            }

            if aggr_header.start_of_seq {
                state.seen_keyframe = true;
            }

            // If this is a new temporal unit and we never saw a keyframe so far,
            // handle this according to the request-keyframe / wait-for-keyframe properties.
            if !state.seen_keyframe {
                if settings.request_keyframe {
                    gst::debug!(CAT, imp = self, "Requesting keyframe from upstream");
                    let event = gst_video::UpstreamForceKeyUnitEvent::builder()
                        .all_headers(true)
                        .build();
                    let _ = self.obj().sink_pad().push_event(event);
                }

                if settings.wait_for_keyframe {
                    gst::trace!(CAT, imp = self, "Waiting for keyframe");
                    self.reset(&mut state);
                    self.obj().drop_packets(..=packet.ext_seqnum());
                    return Ok(None);
                }
            }

            // the next temporal unit starts with a temporal delimiter OBU
            ready_obus.extend_from_slice(&TEMPORAL_DELIMITER);
        }
        state.marked_packet = packet.marker_bit();
        state.last_timestamp = Some(packet.ext_timestamp());

        // parse and prepare the received OBUs
        let mut idx = 0;

        // handle leading OBU fragment
        if state.obu_fragment.is_some() && !aggr_header.leading_fragment {
            gst::error!(
                CAT,
                imp = self,
                "invalid packet: dropping unclosed OBU fragment"
            );
            self.reset(&mut state);
            self.obj().drop_packets(..packet.ext_seqnum());
        }

        // If we finish an OBU here, it will start with the ext seqnum of this packet
        // but if it also extends a fragment then the start will be set to the start
        // of the fragment instead.
        let mut start_ext_seqnum = packet.ext_seqnum();

        if let Some(PendingFragment {
            ext_seqnum,
            ref mut bytes,
        }) = state.obu_fragment
        {
            assert!(aggr_header.leading_fragment);
            let (element_size, is_last_obu) = self
                .find_element_info(&mut reader, &aggr_header, idx)
                .map_err(err_flow!(self, find_element))?;

            let bytes_end = bytes.len();
            bytes.resize(bytes_end + element_size as usize, 0);
            reader
                .read_exact(&mut bytes[bytes_end..])
                .map_err(err_flow!(self, buf_read))?;

            // if this OBU is complete, it can be output
            if !is_last_obu || !aggr_header.trailing_fragment {
                let obu_fragment = state.obu_fragment.take().unwrap();
                self.translate_obus(
                    &mut state,
                    &mut Cursor::new(&obu_fragment.bytes),
                    &mut ready_obus,
                )?;
                start_ext_seqnum = ext_seqnum;
            }

            idx += 1;
        }

        // handle other OBUs, including trailing fragments
        while (reader.position() as usize) < reader.get_ref().len() {
            let (element_size, is_last_obu) =
                self.find_element_info(&mut reader, &aggr_header, idx)?;

            if idx == 0 && aggr_header.leading_fragment {
                if state.found_valid_obu {
                    gst::error!(
                        CAT,
                        imp = self,
                        "invalid packet: unexpected leading OBU fragment"
                    );
                }
                reader
                    .seek(SeekFrom::Current(element_size as i64))
                    .map_err(err_flow!(self, buf_read))?;
                idx += 1;
                if (reader.position() as usize) == reader.get_ref().len() {
                    self.obj().drop_packets(..=packet.ext_seqnum());
                }
                continue;
            }

            // trailing OBU fragments are stored in the state
            if is_last_obu && aggr_header.trailing_fragment {
                let bytes_left = reader.get_ref().len() - (reader.position() as usize);
                let mut bytes = vec![0; bytes_left];
                reader
                    .read_exact(bytes.as_mut_slice())
                    .map_err(err_flow!(self, buf_read))?;

                state.obu_fragment = Some(PendingFragment {
                    ext_seqnum: packet.ext_seqnum(),
                    bytes,
                });
            }
            // full OBUs elements are translated and appended to the ready OBUs
            else {
                let remaining_slice = &reader.get_ref()[reader.position() as usize..];
                if remaining_slice.len() < element_size as usize {
                    gst::error!(
                        CAT,
                        imp = self,
                        "invalid packet: not enough data left for OBU {idx} (needed {element_size}, have {})",
                        remaining_slice.len(),
                    );
                    self.reset(&mut state);
                    if ready_obus.is_empty() || ready_obus == TEMPORAL_DELIMITER {
                        self.obj().drop_packets(..=packet.ext_seqnum());
                    }
                    break;
                }
                self.translate_obus(
                    &mut state,
                    &mut Cursor::new(&remaining_slice[..element_size as usize]),
                    &mut ready_obus,
                )?;

                reader
                    .seek(SeekFrom::Current(element_size as i64))
                    .map_err(err_flow!(self, buf_read))?;
            }

            idx += 1;
        }

        // now push all the complete OBUs
        let buffer = if !ready_obus.is_empty() && ready_obus != TEMPORAL_DELIMITER {
            gst::log!(
                CAT,
                imp = self,
                "Creating buffer containing {} bytes of data (marker {}, discont {})...",
                ready_obus.len(),
                state.marked_packet,
                state.needs_discont,
            );

            let mut buffer = gst::Buffer::from_mut_slice(ready_obus);
            {
                let buffer = buffer.get_mut().unwrap();
                if state.marked_packet {
                    buffer.set_flags(gst::BufferFlags::MARKER);
                }
                if state.needs_discont {
                    buffer.set_flags(gst::BufferFlags::DISCONT);
                    state.needs_discont = false;
                }
            }

            Some(buffer)
        } else {
            None
        };

        // It's important to check this after the packet was created as otherwise
        // the discont flag is already before the missing data.
        if state.marked_packet && state.obu_fragment.is_some() {
            gst::error!(
                CAT,
                imp = self,
                concat!(
                    "invalid packet: has marker bit set, but ",
                    "last OBU is not yet complete. Dropping incomplete OBU."
                )
            );
            self.reset(&mut state);
            if buffer.is_none() {
                self.obj().drop_packets(..=packet.ext_seqnum());
            }
        }

        if let Some(buffer) = buffer {
            Ok(Some((start_ext_seqnum..=packet.ext_seqnum(), buffer)))
        } else {
            Ok(None)
        }
    }

    /// Find out the next OBU element's size, and if it is the last OBU in the packet.
    /// The reader is expected to be at the first byte of the element,
    /// or its preceding size field if present,
    /// and will be at the first byte past the element's size field afterwards.
    fn find_element_info(
        &self,
        reader: &mut Cursor<&[u8]>,
        aggr_header: &AggregationHeader,
        index: u32,
    ) -> Result<(u32, bool), gst::FlowError> {
        let is_last_obu: bool;

        let element_size = if let Some(count) = aggr_header.obu_count {
            is_last_obu = index + 1 == count as u32;
            if is_last_obu {
                (reader.get_ref().len() - reader.position() as usize) as u32
            } else {
                let mut bitreader = BitReader::endian(reader, ENDIANNESS);
                let (size, _) = parse_leb128(&mut bitreader).map_err(err_flow!(self, leb_read))?;
                size
            }
        } else {
            let (size, _) = parse_leb128(&mut BitReader::endian(&mut *reader, ENDIANNESS))
                .map_err(err_flow!(self, leb_read))?;
            is_last_obu = match reader
                .get_ref()
                .len()
                .cmp(&(reader.position() as usize + size as usize))
            {
                Ordering::Greater => false,
                Ordering::Equal => true,
                Ordering::Less => {
                    gst::error!(
                        CAT,
                        imp = self,
                        "invalid packet: size field gives impossibly large OBU size"
                    );
                    return Err(gst::FlowError::Error);
                }
            };
            size
        };

        Ok((element_size, is_last_obu))
    }

    /// Using a single OBU element from one or more RTP packets, construct a buffer containing that
    /// OBU in AV1 bitstream format with size field
    fn translate_obu(
        &self,
        reader: &mut Cursor<&[u8]>,
        obu: &SizedObu,
        w: &mut Vec<u8>,
    ) -> Result<(), gst::FlowError> {
        let pos = w.len();
        w.resize(pos + obu.full_size() as usize, 0);
        let bytes = &mut w[pos..];

        // write OBU header
        reader
            .read_exact(&mut bytes[..obu.header_len as usize])
            .map_err(err_flow!(self, buf_read))?;

        // set `has_size_field`
        bytes[0] |= 1 << 1;

        // skip internal size field if present
        if obu.has_size_field {
            parse_leb128(&mut BitReader::endian(&mut *reader, ENDIANNESS))
                .map_err(err_flow!(self, leb_read))?;
        }

        // write size field
        write_leb128(
            &mut BitWriter::endian(
                Cursor::new(&mut bytes[obu.header_len as usize..]),
                ENDIANNESS,
            ),
            obu.size,
        )
        .map_err(err_flow!(self, leb_write))?;

        // write OBU payload
        reader
            .read_exact(&mut bytes[(obu.header_len + obu.leb_size) as usize..])
            .map_err(err_flow!(self, buf_read))?;

        Ok(())
    }

    /// Using a complete payload unit from one or more RTP packets, construct a buffer containing
    /// the contained OBU(s) in AV1 bitstream format with size field.
    ///
    /// Theoretically this should only contain a single OBU but Pion is sometimes putting multiple
    /// OBUs into one payload unit and we can easily support this here despite it not being allowed
    /// by the specification.
    fn translate_obus(
        &self,
        state: &mut State,
        reader: &mut Cursor<&[u8]>,
        w: &mut Vec<u8>,
    ) -> Result<(), gst::FlowError> {
        let mut first = true;

        while (reader.position() as usize) < reader.get_ref().len() {
            let header_pos = reader.position();
            let mut bitreader = BitReader::endian(&mut *reader, ENDIANNESS);
            let obu = match UnsizedObu::parse(&mut bitreader).map_err(err_flow!(self, obu_read)) {
                Ok(obu) => obu,
                Err(err) => {
                    if first {
                        return Err(err);
                    } else {
                        gst::warning!(CAT, imp = self, "Trailing payload unit is not a valid OBU");
                        return Ok(());
                    }
                }
            };

            reader
                .seek(SeekFrom::Start(header_pos))
                .map_err(err_flow!(self, buf_read))?;

            gst::trace!(CAT, imp = self, "Handling OBU {obu:?}");

            let remaining_slice = &reader.get_ref()[reader.position() as usize..];
            let element_size = if let Some((size, leb_size)) = obu.size {
                let size = (size + leb_size + obu.header_len) as usize;
                if size > remaining_slice.len() {
                    if first {
                        gst::warning!(
                            CAT,
                            imp = self,
                            "Payload unit starts with an incomplete OBU"
                        );
                        return Err(gst::FlowError::Error);
                    } else {
                        gst::warning!(
                            CAT,
                            imp = self,
                            "Trailing payload unit is an incomplete OBU"
                        );
                        return Ok(());
                    }
                }

                if !first {
                    gst::debug!(CAT, imp = self, "Multiple OBUs in a single payload unit");
                }
                size
            } else {
                remaining_slice.len()
            };

            state.found_valid_obu = true;
            first = false;

            // ignore these OBU types
            if matches!(
                obu.obu_type,
                ObuType::TemporalDelimiter | ObuType::TileList | ObuType::Padding
            ) {
                gst::trace!(
                    CAT,
                    imp = self,
                    "Dropping {:?} of size {element_size}",
                    obu.obu_type
                );
                reader
                    .seek(SeekFrom::Current(element_size as i64))
                    .map_err(err_flow!(self, buf_read))?;
                continue;
            }

            let full_obu = {
                if let Some((size, leb_size)) = obu.size {
                    obu.as_sized(size, leb_size)
                } else {
                    let size = element_size as u32 - obu.header_len;
                    let leb_size = leb128_size(size) as u32;
                    obu.as_sized(size, leb_size)
                }
            };

            self.translate_obu(
                &mut Cursor::new(&remaining_slice[..element_size]),
                &full_obu,
                w,
            )?;

            reader
                .seek(SeekFrom::Current(element_size as i64))
                .map_err(err_flow!(self, buf_read))?;
        }

        Ok(())
    }
}

#[cfg(test)]
#[rustfmt::skip]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_translate_obu() {
        gst::init().unwrap();

        let test_data = [
            (
                SizedObu {
                    obu_type: ObuType::TemporalDelimiter,
                    has_extension: false,
                    has_size_field: false,
                    temporal_id: 0,
                    spatial_id: 0,
                    size: 0,
                    leb_size: 1,
                    header_len: 1,
                    is_fragment: false,
                },
                vec![0b0001_0000],
                vec![0b0001_0010, 0],
            ), (
                SizedObu {
                    obu_type: ObuType::Frame,
                    has_extension: true,
                    has_size_field: false,
                    temporal_id: 3,
                    spatial_id: 2,
                    size: 5,
                    leb_size: 1,
                    header_len: 2,
                    is_fragment: false,
                },
                vec![0b0011_0100, 0b0111_0000, 1, 2, 3, 4, 5],
                vec![0b0011_0110, 0b0111_0000, 0b0000_0101, 1, 2, 3, 4, 5],
            ), (
                SizedObu {
                    obu_type: ObuType::Frame,
                    has_extension: true,
                    has_size_field: true,
                    temporal_id: 3,
                    spatial_id: 2,
                    size: 5,
                    leb_size: 1,
                    header_len: 2,
                    is_fragment: false,
                },
                vec![0b0011_0100, 0b0111_0000, 0b0000_0101, 1, 2, 3, 4, 5],
                vec![0b0011_0110, 0b0111_0000, 0b0000_0101, 1, 2, 3, 4, 5],
            )
        ];

        // Element exists just for logging purposes
        let element = glib::Object::new::<crate::av1::depay::RTPAv1Depay>();

        for (idx, (obu, rtp_bytes, out_bytes)) in test_data.into_iter().enumerate() {
            println!("running test {idx}...");
            let mut reader = Cursor::new(rtp_bytes.as_slice());

            let mut actual = Vec::new();
            element.imp().translate_obu(&mut reader, &obu, &mut actual).unwrap();
            assert_eq!(reader.position(), rtp_bytes.len() as u64);

            assert_eq!(actual.as_slice(), out_bytes.as_slice());
        }
    }

    #[test]
    #[allow(clippy::type_complexity)]
    fn test_find_element_info() {
        gst::init().unwrap();

        let test_data: [(Vec<(u32, bool)>, Vec<u8>, AggregationHeader); 4] = [
            (
                vec![(1, false)],   // expected results
                vec![0b0000_0001, 0b0001_0000, 0],
                AggregationHeader { obu_count: None, ..AggregationHeader::default() },
            ), (
                vec![(5, true)],
                vec![0b0111_1000, 0, 0, 0, 0],
                AggregationHeader { obu_count: Some(1), ..AggregationHeader::default() },
            ), (
                vec![(7, true)],
                vec![0b0000_0111, 0b0011_0110, 0b0010_1000, 0b0000_1010, 1, 2, 3, 4],
                AggregationHeader { obu_count: None, ..AggregationHeader::default() },
            ), (
                vec![(6, false), (4, true)],
                vec![0b0000_0110, 0b0111_1000, 1, 2, 3, 4, 5, 0b0011_0000, 1, 2, 3],
                AggregationHeader { obu_count: Some(2), ..AggregationHeader::default() },
            )
        ];

        // Element exists just for logging purposes
        let element = glib::Object::new::<crate::av1::depay::RTPAv1Depay>();

        for (idx, (
            info,
            rtp_bytes,
            aggr_header,
        )) in test_data.into_iter().enumerate() {
            println!("running test {idx}...");
            let mut reader = Cursor::new(rtp_bytes.as_slice());

            let mut element_size = 0;
            for (obu_idx, expected) in info.into_iter().enumerate() {
                if element_size != 0 {
                    reader.seek(SeekFrom::Current(element_size as i64)).unwrap();
                }

                println!("testing element {} with reader position {}...", obu_idx, reader.position());

                let actual = element.imp().find_element_info(&mut reader, &aggr_header, obu_idx as u32);
                assert_eq!(actual, Ok(expected));
                element_size = actual.unwrap().0;
            }
        }
    }
}
