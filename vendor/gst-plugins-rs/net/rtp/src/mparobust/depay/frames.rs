//! ADU & MP3 frames.

use bitstream_io::{BigEndian, BitRead, BitReader, BitWrite, BitWriter};
use gst::prelude::*;

use std::{collections::VecDeque, fmt};

use super::{
    super::{peek_frame_header, FrameHeader},
    MpegAudioRobustDepayError, CAT,
};

/// An identifier to track a chunk of data to the RTP packet it was extracted from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DataProvenance {
    /// The RTP packet extended sequence number.
    pub ext_seqnum: u64,
    /// The index of the data chunk in the RTP packet.
    pub idx: usize,
}

impl fmt::Display for DataProvenance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "seqnum {}~{}", self.ext_seqnum, self.idx)
    }
}

/// A partial Application Data Unit.
#[derive(Debug)]
struct AduFragment {
    provenance: DataProvenance,
    // Make sure it's not changed once initialised
    total_len: usize,
    buf: Vec<u8>,
}

impl AduFragment {
    fn for_size(provenance: DataProvenance, total_len: usize) -> Self {
        AduFragment {
            provenance,
            total_len,
            buf: Vec::with_capacity(total_len),
        }
    }

    fn total_len(&self) -> usize {
        self.total_len
    }
}

/// A complete unparsed Application Data Unit ready for deinterleaving.
#[derive(Debug)]
pub struct AduCompleteUnparsed {
    provenance: DataProvenance,
    is_discont: bool,
    buf: Vec<u8>,
}

impl AduCompleteUnparsed {
    pub fn try_new(
        provenance: DataProvenance,
        buf: impl Into<Vec<u8>>,
    ) -> Result<Self, MpegAudioRobustDepayError> {
        let buf = buf.into();

        if buf.len() <= 4 {
            return Err(MpegAudioRobustDepayError::InsufficientUnparsedAduSize {
                provenance,
                len: buf.len(),
                expected: 4,
            });
        }

        Ok(AduCompleteUnparsed {
            provenance,
            is_discont: false,
            buf,
        })
    }

    pub fn interleaving_seqnum(&self) -> InterleavingSeqnum {
        InterleavingSeqnum {
            index: self.buf[0],
            cycle: (self.buf[1] & 0xe0) >> 5,
        }
    }

    pub fn set_discont(&mut self) {
        self.is_discont = true;
    }
}

impl TryFrom<AduFragment> for AduCompleteUnparsed {
    type Error = MpegAudioRobustDepayError;

    fn try_from(frag: AduFragment) -> Result<Self, MpegAudioRobustDepayError> {
        AduCompleteUnparsed::try_new(frag.provenance, frag.buf)
    }
}

impl fmt::Display for AduCompleteUnparsed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ADU {} total len {}", self.provenance, self.buf.len())
    }
}

/// Accumulates packets for a fragmented ADU frame.
///
/// Used for packets containing fragments for a single ADU frame.
///
/// See https://www.rfc-editor.org/rfc/rfc5219.html#section-4.3.
#[derive(Debug, Default)]
pub struct AduAccumulator(Option<AduFragment>);

impl AduAccumulator {
    #[inline]
    #[track_caller]
    pub fn start(
        &mut self,
        provenance: DataProvenance,
        total_size: usize,
        buf: &[u8],
    ) -> Result<(), MpegAudioRobustDepayError> {
        use MpegAudioRobustDepayError::*;

        if self.0.is_some() {
            // Should be empty when starting from a new ADU.
            // The incoming fragment is unexpected => invalidate the accumulator.
            self.0 = None;

            return Err(NonEmptyAduAccumulator { provenance });
        }

        assert!(buf.len() < total_size, "ADU is not fragmented");

        if buf.len() > total_size {
            return Err(ExcessiveAduFragmentLen {
                provenance,
                len: buf.len(),
                remaining: total_size,
            });
        }

        let mut adu_frag = AduFragment::for_size(provenance, total_size);
        adu_frag.buf.extend(buf);

        self.0 = Some(adu_frag);

        Ok(())
    }

    #[inline]
    pub fn try_accumulate(
        &mut self,
        provenance: DataProvenance,
        total_size: usize,
        buf: &[u8],
    ) -> Result<Option<AduCompleteUnparsed>, MpegAudioRobustDepayError> {
        use MpegAudioRobustDepayError::*;

        let Some(mut adu_frag) = self.0.take() else {
            return Err(MpegAudioRobustDepayError::NoInitialAduFragment { provenance });
        };

        if adu_frag.total_len() != total_size {
            return Err(MpegAudioRobustDepayError::AduFragmentSizeMismatch {
                provenance,
                len: total_size,
                expected: adu_frag.total_len(),
            });
        }

        if adu_frag.buf.len() + buf.len() > adu_frag.total_len() {
            return Err(ExcessiveAduFragmentLen {
                provenance,
                len: buf.len(),
                remaining: adu_frag.total_len() - adu_frag.buf.len(),
            });
        }

        adu_frag.buf.extend(buf);
        if adu_frag.buf.len() < adu_frag.total_len() {
            self.0 = Some(adu_frag);
            return Ok(None);
        }

        // ADU is complete and ready for deinterleaving
        Ok(Some(adu_frag.try_into()?))
    }

    pub fn flush(&mut self) {
        self.0 = None;
    }
}

/// The interleaving sequence number (ISN) of an Application Data Unit.
///
/// See https://www.rfc-editor.org/rfc/rfc5219.html#section-7.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterleavingSeqnum {
    pub index: u8,
    pub cycle: u8,
}

impl Default for InterleavingSeqnum {
    fn default() -> Self {
        InterleavingSeqnum {
            index: 0xff,
            cycle: 7,
        }
    }
}

impl fmt::Display for InterleavingSeqnum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if *self == InterleavingSeqnum::default() {
            return f.write_str("isn N/A");
        }

        write!(f, "isn {}~{}", self.cycle, self.index)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AduIdentifier {
    pub provenance: DataProvenance,
    pub interleaving_seqnum: InterleavingSeqnum,
}

impl fmt::Display for AduIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.interleaving_seqnum, self.provenance)
    }
}

/// A deinterleaved & parsed Application Data Unit.
#[derive(Debug, PartialEq)]
pub struct Adu {
    id: AduIdentifier,
    is_dummy: bool,
    is_discont: bool,
    header: FrameHeader,
    //// The data capacity of the MP3 frame.
    mp3_frame_data_capacity: usize,
    /// The actual data from this ADU in the MP3 frame.
    /// This excludes data from this ADU stored in a previous MP3 frame,
    /// which len is deduced from `backpointer`.
    mp3_frame_data_len: usize,
    backpointer: usize,
    header_side_info_len: usize,
    data_len: usize,
    buf: Vec<u8>,
}

impl Adu {
    pub fn try_from(
        adu_unparsed: AduCompleteUnparsed,
        interleaving_seqnum: InterleavingSeqnum,
    ) -> Result<Self, MpegAudioRobustDepayError> {
        use MpegAudioRobustDepayError::*;

        let AduCompleteUnparsed {
            provenance,
            is_discont,
            mut buf,
        } = adu_unparsed;

        assert!(buf.len() >= 4);

        let id = AduIdentifier {
            interleaving_seqnum,
            provenance,
        };

        // Restore the MPEG syncword in place of the interleaving sequence number
        buf[0] = 0xff;
        buf[1] |= 0xe0;

        let header = peek_frame_header(&buf).or(Err(MP3HeaderParsingError { id }))?;

        let mut side_info = match (header.version, header.channels) {
            (1, 2) => 32,
            (1, 1) | (_, 2) => 17,
            (_, 1) => 9,
            (version, channels) => {
                return Err(UnsupportedMp3Frame {
                    id,
                    version,
                    channels,
                })
            }
        };

        let crc = buf[1] & 0x01;
        if crc == 0 {
            side_info += 2;
        }

        let header_side_info_len = 4 + side_info;
        if header_side_info_len > buf.len() {
            return Err(InsufficientAduSize {
                id,
                len: buf.len(),
                expected: header_side_info_len,
            });
        }

        let data_len = buf.len() - header_side_info_len;

        let (backpointer, mp3_frame_data_len) = if header.layer == 3 {
            if buf.len() <= 6 {
                return Err(MpegAudioRobustDepayError::InsufficientAduSize {
                    id,
                    len: buf.len(),
                    expected: 6,
                });
            }

            let backpointer = if header.version > 1 {
                // lsf
                buf[4] as u16
            } else {
                u16::from_be_bytes([buf[4], buf[5]]) >> 7
            };

            (backpointer, data_len.saturating_sub(backpointer as usize))
        } else {
            (0, data_len)
        };

        Ok(Adu {
            id,
            is_dummy: false,
            is_discont,
            mp3_frame_data_capacity: header.frame_len - header_side_info_len,
            header,
            mp3_frame_data_len,
            backpointer: backpointer as usize,
            header_side_info_len,
            data_len,
            buf,
        })
    }

    pub fn interleaving_seqnum(&self) -> InterleavingSeqnum {
        self.id.interleaving_seqnum
    }

    /// Sets the discont flag on current ADU.
    pub fn set_discont(&mut self) {
        self.is_discont = true;
    }

    /// Returns the Header & Side Info part of this ADU.
    pub fn header_side_info(&self) -> &[u8] {
        &self.buf[..self.header_side_info_len]
    }

    /// Returns the data part of this ADU.
    pub fn data(&self) -> &[u8] {
        &self.buf[self.header_side_info_len..]
    }

    /// Creates a dummy ADU based on `self`.
    ///
    /// See https://www.rfc-editor.org/rfc/rfc5219.html#appendix-A.2
    /// in `insertDummyADUsIfNecessary()`.
    ///
    /// > This ADU can have the same "header" (and thus, "mp3FrameSize") as the tail ADU,
    /// > but should have a "backpointer" of "prevADUend", and an "aduDataSize" of zero.
    /// > The simplest way to do this is to copy the "sideInfo" from the tail ADU,
    /// > replace the value of "main_data_begin" with "prevADUend",
    /// > and set all of the "part2_3_length" fields to zero.
    ///
    /// # Panic
    ///
    /// Panics if layer is not 3.
    pub fn to_dummy(&self, is_first: bool, prev_adu_end: usize) -> anyhow::Result<Adu> {
        use anyhow::Context;

        assert_eq!(self.header.layer, 3, "Only makes sense for a layer 3 ADU");

        let header_side_info = self.header_side_info();
        let mut r = BitReader::endian(header_side_info, BigEndian);

        let mut buf = Vec::with_capacity(self.header_side_info_len);
        let mut w = BitWriter::endian(&mut buf, BigEndian);

        // Copy the header and unset the CRC
        w.write_from(r.read_to::<u16>().context("header")? | 0x01)
            .unwrap();
        w.write_from(r.read_to::<u16>().context("header")?).unwrap();

        // Skip CRC from the original if any
        let crc = header_side_info[1] & 0x01;
        if crc == 0 {
            r.skip(16).context("CRC")?;
        }

        let lsf = self.header.version > 1;

        // Use prev_adu_avail as backpointer
        let nb_granules = if lsf {
            w.write::<8, u32>(prev_adu_end as u32).unwrap();
            r.skip(8).context("main_data_end")?;

            1
        } else {
            w.write::<9, u32>(prev_adu_end as u32).unwrap();
            r.skip(9).context("main_data_end")?;

            if self.header.channels == 2 {
                w.write::<3, u8>(r.read::<3, u8>().context("private_bits")?)
                    .unwrap();
            } else {
                w.write::<5, u8>(r.read::<5, u8>().context("private_bits")?)
                    .unwrap();
            }

            for _ in 0..self.header.channels {
                w.write::<4, u8>(r.read::<4, u8>().context("scfsi")?)
                    .unwrap();
            }

            2
        };

        // Set the rest of side info with all of the "part2_3_length" fields to zero
        for _ in 0..nb_granules {
            for _ in 0..self.header.channels {
                w.write_const::<12, 0>().unwrap();
                r.skip(12).context("part2_3_length")?;

                const BIG_VALUES_PLUS_GLOBAL_GAIN_SIZE: u32 = 9 + 8;
                w.write::<BIG_VALUES_PLUS_GLOBAL_GAIN_SIZE, u32>(
                    r.read::<BIG_VALUES_PLUS_GLOBAL_GAIN_SIZE, u32>()
                        .context("big values + global gain")?,
                )
                .unwrap();
                if lsf {
                    w.write::<9, u16>(r.read::<9, u16>().context("scalefac_compress")?)
                        .unwrap();
                } else {
                    w.write::<4, u8>(r.read::<4, u8>().context("scalefac_compress")?)
                        .unwrap();
                }

                let blocksplit_flag = r.read_bit().context("blocksplit_flag")?;
                w.write_bit(blocksplit_flag).unwrap();
                if blocksplit_flag {
                    const BLCK_SWITCH_TBLE_SUBBLOC_GAIN: u32 = 2 + 1 + 2 * 5 + 3 * 3;
                    w.write::<BLCK_SWITCH_TBLE_SUBBLOC_GAIN, u32>(
                        r.read::<BLCK_SWITCH_TBLE_SUBBLOC_GAIN, u32>()
                            .context("block_type + switch_point + table_select + subblock_gain")?,
                    )
                    .unwrap();
                } else {
                    const TBLE_REGION_ADDR_1_2: u32 = 3 * 5 + 4 + 3;
                    w.write::<TBLE_REGION_ADDR_1_2, u32>(
                        r.read::<TBLE_REGION_ADDR_1_2, u32>()
                            .context("table_select + region_address 1 & 2")?,
                    )
                    .unwrap();
                }

                if !lsf {
                    w.write_bit(r.read_bit().context("preflag")?).unwrap();
                }

                const SCALEFAC_COUNT1TBL: u32 = 1 + 1;
                w.write::<SCALEFAC_COUNT1TBL, u8>(
                    r.read::<SCALEFAC_COUNT1TBL, u8>()
                        .context("scalefac_scale + count1table_select")?,
                )
                .unwrap();
            }
        }

        // copy the rest of the side info as is
        while let Ok(b) = r.read_bit() {
            w.write_bit(b).unwrap();
        }

        Ok(Adu {
            id: self.id,
            is_dummy: true,
            is_discont: is_first,
            header: self.header.clone(),
            mp3_frame_data_capacity: self.mp3_frame_data_capacity,
            mp3_frame_data_len: 0,
            backpointer: prev_adu_end,
            header_side_info_len: self.header_side_info_len,
            data_len: 0,
            buf,
        })
    }
}

impl fmt::Display for Adu {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_dummy {
            return write!(f, "dummy ADU before {}", self.id);
        }

        write!(f, "ADU {} data len {}", self.id, self.data_len)
    }
}

pub struct PartialMp3Frame {
    adu_id: AduIdentifier,
    is_filler: bool,
    is_discont: bool,
    header: FrameHeader,
    data_capacity: usize,
    data_offset: usize,
    buf: Vec<u8>,
}

impl From<Adu> for PartialMp3Frame {
    fn from(adu: Adu) -> Self {
        let (buf, end_of_data) = if adu.backpointer == 0 || adu.data_len == 0 {
            (adu.buf, adu.data_len)
        } else {
            let mut buf = adu.buf;
            let mut data = buf.split_off(adu.header_side_info_len);

            debug_assert_eq!(data.len(), adu.data_len);
            if adu.mp3_frame_data_len > 0 {
                debug_assert_eq!(adu.data_len, adu.backpointer + adu.mp3_frame_data_len);
                buf.extend(data.drain(adu.backpointer..));
            }

            (buf, adu.mp3_frame_data_len)
        };

        PartialMp3Frame {
            adu_id: adu.id,
            is_filler: adu.is_dummy,
            is_discont: adu.is_discont,
            header: adu.header,
            data_capacity: adu.mp3_frame_data_capacity,
            data_offset: end_of_data,
            buf,
        }
    }
}

impl PartialMp3Frame {
    pub fn append_data(&mut self, data: &[u8], to_offset: usize) {
        debug_assert!(to_offset + data.len() <= self.data_capacity);

        self.buf.resize(
            self.buf.len() + to_offset.checked_sub(self.data_offset).unwrap(),
            0,
        );
        self.buf.extend(data);

        self.data_offset = to_offset + data.len();
    }

    /// Terminates this MP3 frame, setting any unfilled data to 0.
    pub fn terminate(mut self) -> Mp3Frame {
        self.buf.resize(self.header.frame_len, 0);

        let mut buf = gst::Buffer::from_mut_slice(self.buf);

        let buf_ref = buf.get_mut().unwrap();
        buf_ref.set_duration(
            (self.header.samples_per_frame as u64)
                .mul_div_floor(*gst::ClockTime::SECOND, self.header.sample_rate as u64)
                .map(gst::ClockTime::from_nseconds),
        );
        if self.is_discont {
            buf_ref.set_flags(gst::BufferFlags::DISCONT);
        }

        Mp3Frame {
            adu_id: self.adu_id,
            is_filler: self.is_filler,
            header: self.header,
            buf,
        }
    }
}

impl fmt::Display for PartialMp3Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_filler {
            return write!(f, "filler Partial MP3 frame before ADU {}", self.adu_id);
        }

        write!(f, "Partial MP3 frame from ADU {}", self.adu_id)
    }
}

#[derive(Debug, Default)]
pub struct AduQueue {
    pending_adus: VecDeque<Adu>,
}

impl AduQueue {
    pub fn drain(&mut self, elem: &super::RtpMpegAudioRobustDepay) -> MaybeSingleMp3FrameOrList {
        let mut mp3frames = MaybeSingleMp3FrameOrList::default();
        self.drain_priv(elem, &mut mp3frames);

        mp3frames
    }

    #[inline]
    fn drain_priv(
        &mut self,
        elem: &super::RtpMpegAudioRobustDepay,
        mp3frames: &mut MaybeSingleMp3FrameOrList,
    ) {
        while let Some(mp3frame) = self.generate_mp3frame(elem) {
            mp3frames.push(mp3frame);
        }
    }

    pub fn flush(&mut self) {
        self.pending_adus.clear();
    }

    pub fn push_adus_pop_mp3_frames(
        &mut self,
        elem: &super::RtpMpegAudioRobustDepay,
        adus: SingleAduOrList,
    ) -> MaybeSingleMp3FrameOrList {
        // See https://www.rfc-editor.org/rfc/rfc5219.html#appendix-A.2
        // The implementation here is as close as possible to the pseudo-code from the RFC.
        // The pseudo-code handles all cases with the end_of_data offset shifting as we
        // iterate through ADUs. This allows checking whether the backpointer from current ADU
        // would point back to the MP3 frame that will be generated from the front ADU.
        // For this reason, some of the variables below have to be signed integers.

        let mut mp3frames = MaybeSingleMp3FrameOrList::default();

        for adu in adus {
            if adu.header.layer != 3 {
                // non-layer III have no bitreservoir and must be sent as-is
                mp3frames.push(PartialMp3Frame::from(adu).terminate());
                continue;
            }

            if adu.is_discont {
                gst::trace!(
                    CAT,
                    obj = elem,
                    "Found discont for {adu} => draining ADU queue"
                );
                self.drain_priv(elem, &mut mp3frames);
            }

            self.enqueue_adu(elem, adu);

            while !self.pending_adus.is_empty() {
                let front_adu = self.pending_adus.front().unwrap();
                let front_data_capacity = front_adu.mp3_frame_data_capacity as isize;

                let mut can_generate = false;
                let mut frame_data_offset = 0isize;
                for cur_adu in &self.pending_adus {
                    let end_of_data = frame_data_offset - cur_adu.backpointer as isize
                        + cur_adu.mp3_frame_data_len as isize;
                    if end_of_data >= front_data_capacity {
                        // Enough data to generate a frame
                        can_generate = true;
                        break;
                    }

                    frame_data_offset += cur_adu.mp3_frame_data_capacity as isize;
                }

                if can_generate {
                    if let Some(mp3frame) = self.generate_mp3frame(elem) {
                        mp3frames.push(mp3frame);
                    }
                } else {
                    break;
                }
            }
        }

        mp3frames
    }

    fn enqueue_adu(&mut self, elem: &super::RtpMpegAudioRobustDepay, mut adu: Adu) {
        // See https://www.rfc-editor.org/rfc/rfc5219.html#appendix-A.2
        // Insert a dummy ADU if the backpointer of the ADU from the back of
        // the queue overlaps the data of the previous ADU. This should occur
        // only if an intermediate ADU was missing, e.g. due to packet loss.

        let (mut prev_adu_end, prev_adu_id) = if let Some(prev_adu) = self.pending_adus.back() {
            (
                (prev_adu.mp3_frame_data_capacity + prev_adu.backpointer)
                    .saturating_sub(prev_adu.data_len),
                Some(prev_adu.id),
            )
        } else {
            (0, None)
        };

        let mut is_first = true;
        while adu.backpointer > prev_adu_end {
            gst::trace!(CAT, obj = elem, "Inserting dummy ADU with data capacity {} before incoming {adu}{} backpointer {} prev_adu_end {}",
                adu.mp3_frame_data_capacity,
                prev_adu_id.as_ref().map_or_else(String::new, |prev_adu_id| format!(" and after ADU {prev_adu_id:?}")),
                adu.backpointer, prev_adu_end,
            );

            let dummy_adu = match adu.to_dummy(is_first, prev_adu_end) {
                Ok(dummy_adu) => dummy_adu,
                Err(err) => {
                    gst::element_warning!(elem, gst::ResourceError::Read, ["{err}"]);

                    return;
                }
            };

            self.pending_adus.push_back(dummy_adu);
            prev_adu_end += adu.mp3_frame_data_capacity;

            if is_first {
                // Unset the discount flag on current ADU if any
                // as it will be handled by the Dummy ADU
                adu.is_discont = false;
                is_first = false;
            }
        }

        gst::trace!(
            CAT,
            obj = elem,
            "Inserting {adu} frame data capacity {} frame data len {} backpointer {}",
            adu.mp3_frame_data_capacity,
            adu.mp3_frame_data_len,
            adu.backpointer,
        );

        self.pending_adus.push_back(adu);
    }

    fn generate_mp3frame(&mut self, elem: &super::RtpMpegAudioRobustDepay) -> Option<Mp3Frame> {
        // See https://www.rfc-editor.org/rfc/rfc5219.html#appendix-A.2
        // in `generateFrameFromHeadADU()`.
        // See comment in `push_adus_pop_mp3_frames()` about using signed integers.

        let front_adu = self.pending_adus.pop_front()?;
        gst::trace!(
            CAT,
            obj = elem,
            "Generating MP3 frame from {}",
            if front_adu.is_dummy {
                "dummy ADU".to_string()
            } else {
                front_adu.to_string()
            },
        );

        let mut mp3frame = PartialMp3Frame::from(front_adu);
        let mut to_offset = mp3frame.data_offset as isize;
        let mut frame_offset = mp3frame.data_capacity as isize;

        let mut adu_iter = self.pending_adus.iter();
        while mp3frame.data_offset < mp3frame.data_capacity {
            let Some(cur_adu) = adu_iter.next() else {
                break;
            };

            let mut start_of_data = frame_offset - cur_adu.backpointer as isize;
            if start_of_data > mp3frame.data_capacity as isize {
                // Current ADU is too far away to append its backpointed data to the MP3 frame
                break;
            }

            if cur_adu.data_len > 0 {
                let mut end_of_data = (start_of_data + cur_adu.data_len as isize) as usize;
                if end_of_data > mp3frame.data_capacity {
                    end_of_data = mp3frame.data_capacity;
                }

                let from_offset = if start_of_data <= to_offset {
                    let from_offset = (to_offset - start_of_data) as usize;

                    if from_offset >= std::cmp::min(cur_adu.backpointer, cur_adu.data_len) {
                        gst::trace!(
                            CAT,
                            obj = elem,
                            "not using {cur_adu}: from offset {from_offset}, backpointer {}",
                            cur_adu.backpointer,
                        );

                        frame_offset += cur_adu.mp3_frame_data_capacity as isize;

                        continue;
                    }

                    start_of_data = to_offset;
                    if (end_of_data as isize) < start_of_data {
                        end_of_data = start_of_data as usize;
                    }

                    from_offset
                } else {
                    to_offset = start_of_data;

                    0
                };

                let bytes_used_here = end_of_data - start_of_data as usize;
                gst::trace!(
                    CAT,
                    obj = elem,
                    "Appending {bytes_used_here} bytes from {cur_adu} starting @ {from_offset}"
                );
                mp3frame.append_data(
                    &cur_adu.data()[from_offset..][..bytes_used_here],
                    to_offset as usize,
                );
            }

            frame_offset += cur_adu.mp3_frame_data_capacity as isize;
        }

        Some(mp3frame.terminate())
    }
}

#[derive(Debug)]
pub struct Mp3Frame {
    pub adu_id: AduIdentifier,
    is_filler: bool,
    pub(crate) header: FrameHeader,
    // Final state of the buffer, so not using a `MappedBuffer`.
    pub(crate) buf: gst::Buffer,
}

impl fmt::Display for Mp3Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_filler {
            return write!(f, "filler MP3 frame before ADU {}", self.adu_id);
        }

        write!(f, "MP3 frame from ADU {}", self.adu_id)
    }
}

#[derive(Debug)]
pub enum SingleOrList<T> {
    Single(T),
    // Up to 255 for ADU
    List(Vec<T>),
}

impl<T> SingleOrList<T> {
    pub fn new_list(capacity: usize) -> Self {
        Self::List(Vec::with_capacity(capacity))
    }

    /// Reserves capacity for at least `additional` more elements.
    pub fn reserve(&mut self, additional: usize) {
        if additional == 0 {
            return;
        }

        match self {
            SingleOrList::<T>::Single(_) => {
                let list = SingleOrList::<T>::List(Vec::with_capacity(1 + additional));
                let prev = std::mem::replace(self, list);
                let SingleOrList::<T>::Single(prev) = prev else {
                    unreachable!()
                };
                let SingleOrList::<T>::List(list) = self else {
                    unreachable!()
                };
                list.push(prev);
            }
            SingleOrList::<T>::List(list) => list.reserve(additional),
        }
    }

    #[inline]
    pub fn push(&mut self, item: T) {
        match self {
            SingleOrList::<T>::Single(_) => {
                let list = SingleOrList::<T>::List(Vec::with_capacity(255));
                let prev = std::mem::replace(self, list);
                let SingleOrList::<T>::Single(prev) = prev else {
                    unreachable!()
                };
                let SingleOrList::<T>::List(list) = self else {
                    unreachable!()
                };
                list.push(prev);
                list.push(item);
            }
            SingleOrList::<T>::List(list) => list.push(item),
        }
    }
}

impl<T> IntoIterator for SingleOrList<T> {
    type Item = T;
    type IntoIter = SingleOrListIter<T>;

    fn into_iter(self) -> SingleOrListIter<T> {
        match self {
            SingleOrList::<T>::Single(item) => SingleOrListIter::<T>::Single(Some(item)),
            SingleOrList::<T>::List(items) => SingleOrListIter::<T>::List(items.into_iter()),
        }
    }
}

impl<T> From<T> for SingleOrList<T> {
    fn from(item: T) -> Self {
        SingleOrList::<T>::Single(item)
    }
}

pub type SingleAduOrList = SingleOrList<Adu>;
pub type SingleMp3FrameOrList = SingleOrList<Mp3Frame>;

#[derive(Debug)]
pub enum SingleOrListIter<T> {
    Single(Option<T>),
    List(std::vec::IntoIter<T>),
}

impl<T> Iterator for SingleOrListIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        match self {
            SingleOrListIter::<T>::Single(item) => item.take(),
            SingleOrListIter::<T>::List(iter) => iter.next(),
        }
    }
}

#[derive(Debug)]
pub struct MaybeSingleOrList<T>(Option<SingleOrList<T>>);

impl<T> Default for MaybeSingleOrList<T> {
    fn default() -> Self {
        MaybeSingleOrList::<T>(None)
    }
}

impl<T> MaybeSingleOrList<T> {
    pub fn new_list(capacity: usize) -> Self {
        MaybeSingleOrList::<T>(Some(SingleOrList::<T>::new_list(capacity)))
    }

    /// Reserves capacity for at least `additional` more elements.
    pub fn reserve(&mut self, additional: usize) {
        if additional == 0 {
            return;
        }

        match &mut self.0 {
            Some(inner) => inner.reserve(additional),
            None if additional > 1 => self.0 = Some(SingleOrList::<T>::new_list(additional)),
            _ => (),
        }
    }

    pub fn push(&mut self, item: T) {
        match &mut self.0 {
            Some(inner) => inner.push(item),
            None => self.0 = Some(SingleOrList::<T>::Single(item)),
        }
    }
}

impl<T> From<T> for MaybeSingleOrList<T> {
    fn from(item: T) -> Self {
        MaybeSingleOrList::<T>(Some(SingleOrList::<T>::Single(item)))
    }
}

impl<T> From<MaybeSingleOrList<T>> for Option<SingleOrList<T>> {
    fn from(mut maybe: MaybeSingleOrList<T>) -> Self {
        maybe.0.take()
    }
}

pub type MaybeSingleAduOrList = MaybeSingleOrList<Adu>;
pub type MaybeSingleMp3FrameOrList = MaybeSingleOrList<Mp3Frame>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mparobust::depay::RtpMpegAudioRobustDepay;
    use gst::glib;
    use std::{iter, sync::LazyLock};

    const HEADER_LEN: usize = 4;
    /// Side info: backpointer: 0 (9 bits), remaining 243 set to 0 => 32bits
    const SIDE_INFO_LEN: usize = 32;
    const MP3FRAME_LEN: usize = 384;
    const DATA_CAPACITY: usize = MP3FRAME_LEN - HEADER_LEN - SIDE_INFO_LEN;

    static ELEM: LazyLock<RtpMpegAudioRobustDepay> =
        LazyLock::new(glib::Object::new::<RtpMpegAudioRobustDepay>);

    fn new_adu_queue() -> AduQueue {
        gst::init().unwrap();

        AduQueue::default()
    }

    fn new_adu_layer3(idx: u8, backpointer: usize, data_len: usize) -> Adu {
        // Header:
        // * Version 1 layer 3 no CRC => 0xff 0xfb
        // * bitrate: 128 (idx 9), sample rate 48000 (idx 1) => 0x94
        // * chans 2, padding: 0 => 0x00
        let mut buf = vec![0xff, 0xfb, 0x94, 0x00];

        buf.extend(&((backpointer as u16) << 7).to_be_bytes());
        buf.resize(buf.len() + SIDE_INFO_LEN - 2, 0);

        let backpointed = std::cmp::min(backpointer, data_len);
        let mp3_frame_data_len = data_len.saturating_sub(backpointer);

        // Data
        buf.extend(std::iter::repeat_n(0xb0 | idx, backpointed));
        buf.extend(std::iter::repeat_n(0xf0 | idx, mp3_frame_data_len));
        assert_eq!(buf.len(), HEADER_LEN + SIDE_INFO_LEN + data_len);

        let adu = AduCompleteUnparsed::try_new(
            DataProvenance {
                ext_seqnum: 0,
                idx: idx as usize,
            },
            buf,
        )
        .unwrap();
        let adu = Adu::try_from(adu, InterleavingSeqnum::default()).unwrap();

        assert_eq!(adu.header.version, 1);
        assert_eq!(adu.header.layer, 3);
        assert_eq!(adu.header.channels, 2);
        assert_eq!(adu.header.frame_len, MP3FRAME_LEN);
        assert_eq!(adu.mp3_frame_data_capacity, DATA_CAPACITY);
        assert_eq!(adu.mp3_frame_data_len, mp3_frame_data_len);
        assert_eq!(adu.backpointer, backpointer);
        assert_eq!(adu.header_side_info_len, HEADER_LEN + SIDE_INFO_LEN);
        assert_eq!(adu.data_len, data_len);

        adu
    }

    #[test]
    fn no_backpointers() {
        let mut pending_adus = new_adu_queue();

        let adu0 = new_adu_layer3(0, 0, DATA_CAPACITY);
        let adu0_buf = adu0.buf.clone();
        let mp3frames: Option<SingleMp3FrameOrList> = pending_adus
            .push_adus_pop_mp3_frames(&ELEM, adu0.into())
            .into();
        let mp3frame = mp3frames.unwrap().into_iter().next().unwrap();
        {
            let mp3buf = mp3frame.buf.map_readable().unwrap();
            assert_eq!(mp3buf.as_slice(), adu0_buf);
        }

        let adu1 = new_adu_layer3(1, 0, DATA_CAPACITY);
        let adu1_buf = adu1.buf.clone();
        let mp3frames: Option<SingleMp3FrameOrList> = pending_adus
            .push_adus_pop_mp3_frames(&ELEM, adu1.into())
            .into();
        let mp3frame = mp3frames.unwrap().into_iter().next().unwrap();
        {
            let mp3buf = mp3frame.buf.map_readable().unwrap();
            assert_eq!(mp3buf.as_slice(), adu1_buf);
        }
    }

    #[test]
    fn backpointer() {
        let mut pending_adus = new_adu_queue();

        const BACKPOINTER1: usize = 48;
        const HEADROOM1: usize = 32;

        const ADU0_LEN: usize = MP3FRAME_LEN - BACKPOINTER1;
        const ADU0_DATA_LEN: usize = DATA_CAPACITY - BACKPOINTER1;
        let adu0 = new_adu_layer3(0, 0, ADU0_DATA_LEN);
        // MP3 frame can use more data
        assert!(Option::<SingleMp3FrameOrList>::from(
            pending_adus.push_adus_pop_mp3_frames(&ELEM, adu0.into())
        )
        .is_none());

        const ADU1_FRAMED_DATA_LEN: usize = DATA_CAPACITY - HEADROOM1;
        const ADU1_DATA_LEN: usize = ADU1_FRAMED_DATA_LEN + BACKPOINTER1;
        let adu1 = new_adu_layer3(1, BACKPOINTER1, ADU1_DATA_LEN);

        // Popping first MP3 frame with ADU0 & backpointed data from ADU1
        let mp3frames: Option<SingleMp3FrameOrList> = pending_adus
            .push_adus_pop_mp3_frames(&ELEM, adu1.into())
            .into();

        let mut mp3frame_iter = mp3frames.unwrap().into_iter();
        let mp3frame = mp3frame_iter.next().unwrap();
        {
            let mp3buf = mp3frame.buf.map_readable().unwrap();
            assert_eq!(mp3buf.len(), MP3FRAME_LEN);

            // Check ADU0 data
            assert!(mp3buf[HEADER_LEN + SIDE_INFO_LEN..][..ADU0_DATA_LEN]
                .iter()
                .eq(iter::repeat_n(&0xf0u8, ADU0_DATA_LEN)));

            // Check ADU1 backpointed data
            assert!(mp3buf[ADU0_LEN..][..BACKPOINTER1]
                .iter()
                .eq(iter::repeat_n(&0xb1u8, BACKPOINTER1)));
        }

        assert!(mp3frame_iter.next().is_none());

        // Draining second MP3 frame with framed data from ADU1
        let mp3frames: Option<SingleMp3FrameOrList> = pending_adus.drain(&ELEM).into();
        let mut mp3frame_iter = mp3frames.unwrap().into_iter();
        let mp3frame = mp3frame_iter.next().unwrap();
        {
            let mp3buf = mp3frame.buf.map_readable().unwrap();
            assert_eq!(mp3buf.len(), MP3FRAME_LEN);

            // Check ADU1 framed data
            assert!(mp3buf[HEADER_LEN + SIDE_INFO_LEN..][..ADU1_FRAMED_DATA_LEN]
                .iter()
                .eq(iter::repeat_n(&0xf1u8, ADU1_FRAMED_DATA_LEN)));

            // Check 0 filler
            assert!(mp3buf[HEADER_LEN + SIDE_INFO_LEN + ADU1_FRAMED_DATA_LEN..]
                .iter()
                .eq(iter::repeat_n(&0u8, HEADROOM1)));
        }

        assert!(mp3frame_iter.next().is_none());
    }

    #[test]
    fn two_frames_in_reservoir() {
        let mut pending_adus = new_adu_queue();

        const ADU1_DATA_LEN: usize = 48;
        const ADU1_IN_FRAME_0: usize = ADU1_DATA_LEN; // All in frame 0
        const ADU2_IN_FRAME_0: usize = 64;
        const ADU1_BACKPOINTER: usize = ADU1_IN_FRAME_0 + ADU2_IN_FRAME_0; // All in frame 0 and before ADU2_IN_FRAME_0

        const DATA_GAP0: usize = 2; // Gap between framed 0 data and the backpointed data from frame 1 & 2
        const HEADROOM0: usize = DATA_GAP0 + ADU1_DATA_LEN + ADU2_IN_FRAME_0;

        const ADU0_LEN: usize = MP3FRAME_LEN - HEADROOM0;
        const ADU0_DATA_LEN: usize = DATA_CAPACITY - HEADROOM0;

        const ADU2_IN_FRAME_1: usize = DATA_CAPACITY;
        const ADU2_FRAMED_DATA_LEN: usize = DATA_CAPACITY;
        const ADU2_DATA_LEN: usize = ADU2_IN_FRAME_0 + ADU2_IN_FRAME_1 + ADU2_FRAMED_DATA_LEN;
        const ADU2_BACKPOINTER: usize = ADU2_IN_FRAME_0 + ADU2_IN_FRAME_1;

        let adu0 = new_adu_layer3(0, 0, ADU0_DATA_LEN);
        // MP3 frame can use more data
        assert!(Option::<SingleMp3FrameOrList>::from(
            pending_adus.push_adus_pop_mp3_frames(&ELEM, adu0.into())
        )
        .is_none());

        let adu1 = new_adu_layer3(1, ADU1_BACKPOINTER, ADU1_DATA_LEN);
        // MP3 frame can use more data
        assert!(Option::<SingleMp3FrameOrList>::from(
            pending_adus.push_adus_pop_mp3_frames(&ELEM, adu1.into())
        )
        .is_none());

        let adu2 = new_adu_layer3(2, ADU2_BACKPOINTER, ADU2_DATA_LEN);

        let mp3frames: Option<SingleMp3FrameOrList> = pending_adus
            .push_adus_pop_mp3_frames(&ELEM, adu2.into())
            .into();

        let mut mp3frame_iter = mp3frames.unwrap().into_iter();
        let mp3frame = mp3frame_iter.next().unwrap();
        {
            let mp3buf = mp3frame.buf.map_readable().unwrap();
            assert_eq!(mp3buf.len(), MP3FRAME_LEN);

            // Check ADU0 data
            assert!(mp3buf[HEADER_LEN + SIDE_INFO_LEN..][..ADU0_DATA_LEN]
                .iter()
                .eq(iter::repeat_n(&0xf0u8, ADU0_DATA_LEN)));

            // Check the gap before backpointed data
            assert!(mp3buf[ADU0_LEN..][..DATA_GAP0]
                .iter()
                .eq(iter::repeat_n(&0u8, DATA_GAP0)));

            // Check ADU1 backpointed data (all in frame 0)
            assert!(mp3buf[ADU0_LEN + DATA_GAP0..][..ADU1_IN_FRAME_0]
                .iter()
                .eq(iter::repeat_n(&0xb1u8, ADU1_IN_FRAME_0)));

            // Check ADU2 backpointed data in frame 0
            assert!(mp3buf[ADU0_LEN + DATA_GAP0 + ADU1_IN_FRAME_0..]
                .iter()
                .eq(iter::repeat_n(&0xb2u8, ADU2_IN_FRAME_0)));
        }

        assert!(mp3frame_iter.next().is_none());

        let mp3frames: Option<SingleMp3FrameOrList> = pending_adus.drain(&ELEM).into();
        let mut mp3frame_iter = mp3frames.unwrap().into_iter();
        let mp3frame = mp3frame_iter.next().unwrap();
        {
            let mp3buf = mp3frame.buf.map_readable().unwrap();
            assert_eq!(mp3buf.len(), MP3FRAME_LEN);

            // Check ADU2 backpointed data in frame 1
            assert!(mp3buf[HEADER_LEN + SIDE_INFO_LEN..]
                .iter()
                .eq(iter::repeat_n(&0xb2u8, ADU2_IN_FRAME_1)));
        }

        // Pop third MP3 frame with framed data from ADU2
        let mp3frame = mp3frame_iter.next().unwrap();
        {
            let mp3buf = mp3frame.buf.map_readable().unwrap();
            assert_eq!(mp3buf.len(), MP3FRAME_LEN);

            // Check ADU2 framed data
            assert!(mp3buf[HEADER_LEN + SIDE_INFO_LEN..]
                .iter()
                .eq(iter::repeat_n(&0xf2u8, ADU2_FRAMED_DATA_LEN)));
        }

        assert!(mp3frame_iter.next().is_none());
    }

    #[test]
    fn overlap() {
        let mut pending_adus = new_adu_queue();

        const BACKPOINTER1: usize = 48;
        const HEADROOM0: usize = BACKPOINTER1 - 8; // less than BACKPOINTER1 => overlap

        const ADU0_DATA_LEN: usize = DATA_CAPACITY - HEADROOM0;
        let adu0 = new_adu_layer3(0, 0, ADU0_DATA_LEN);
        // MP3 frame can use more data
        assert!(Option::<SingleMp3FrameOrList>::from(
            pending_adus.push_adus_pop_mp3_frames(&ELEM, adu0.into())
        )
        .is_none());

        const ADU1_FRAMED_DATA_LEN: usize = DATA_CAPACITY;
        const ADU1_DATA_LEN: usize = ADU1_FRAMED_DATA_LEN + BACKPOINTER1;
        let adu1 = new_adu_layer3(1, BACKPOINTER1, ADU1_DATA_LEN);

        // Popping first MP3 frame with ADU0 which was pushed without ADU1
        // because its backpointed data would have overlapped ADU0's data
        let mp3frames: Option<SingleMp3FrameOrList> = pending_adus
            .push_adus_pop_mp3_frames(&ELEM, adu1.into())
            .into();

        let mut mp3frame_iter = mp3frames.unwrap().into_iter();
        let mp3frame = mp3frame_iter.next().unwrap();
        {
            let mp3buf = mp3frame.buf.map_readable().unwrap();
            assert_eq!(mp3buf.len(), MP3FRAME_LEN);

            // Check ADU0 data
            assert!(mp3buf[HEADER_LEN + SIDE_INFO_LEN..][..ADU0_DATA_LEN]
                .iter()
                .eq(iter::repeat_n(&0xf0u8, ADU0_DATA_LEN)));

            // Check 0 filler
            assert!(mp3buf[HEADER_LEN + SIDE_INFO_LEN + ADU0_DATA_LEN..]
                .iter()
                .eq(iter::repeat_n(&0u8, HEADROOM0)));
        }

        // Second MP3 frame contains backpointed data from ADU1
        // in the dummy frame inserted to avoid overlap
        let mp3frame = mp3frame_iter.next().unwrap();
        {
            let mp3buf = mp3frame.buf.map_readable().unwrap();
            assert_eq!(mp3buf.len(), MP3FRAME_LEN);

            // Check 0 gap before ADU1 backpointed data
            assert!(
                mp3buf[HEADER_LEN + SIDE_INFO_LEN..][..DATA_CAPACITY - BACKPOINTER1]
                    .iter()
                    .eq(iter::repeat_n(&0u8, DATA_CAPACITY - BACKPOINTER1))
            );

            // Check ADU1 backpointed data
            assert!(
                mp3buf[HEADER_LEN + SIDE_INFO_LEN + DATA_CAPACITY - BACKPOINTER1..]
                    .iter()
                    .eq(iter::repeat_n(&0xb1u8, BACKPOINTER1))
            );
        }

        // Draining third MP3 frame with framed data from ADU1
        let mp3frames: Option<SingleMp3FrameOrList> = pending_adus.drain(&ELEM).into();
        let mut mp3frame_iter = mp3frames.unwrap().into_iter();
        let mp3frame = mp3frame_iter.next().unwrap();
        {
            let mp3buf = mp3frame.buf.map_readable().unwrap();
            assert_eq!(mp3buf.len(), MP3FRAME_LEN);

            // Check ADU1 framed data
            assert!(mp3buf[HEADER_LEN + SIDE_INFO_LEN..]
                .iter()
                .eq(iter::repeat_n(&0xf1u8, ADU1_FRAMED_DATA_LEN)));
        }

        assert!(mp3frame_iter.next().is_none());
    }

    #[test]
    fn first_frame_discontinuity() {
        let mut pending_adus = new_adu_queue();

        const BACKPOINTER0: usize = 48;
        const HEADROOM0: usize = 32;

        const ADU0_FRAMED_DATA_LEN: usize = DATA_CAPACITY - HEADROOM0;
        const ADU0_DATA_LEN: usize = ADU0_FRAMED_DATA_LEN + BACKPOINTER0;
        let adu0 = new_adu_layer3(0, BACKPOINTER0, ADU0_DATA_LEN);

        const ADU1_BACKPOINTER: usize = HEADROOM0;
        const ADU1_FRAMED_DATA_LEN: usize = DATA_CAPACITY;
        const ADU1_DATA_LEN: usize = ADU1_FRAMED_DATA_LEN + ADU1_BACKPOINTER;
        let adu1 = new_adu_layer3(1, ADU1_BACKPOINTER, ADU1_DATA_LEN);

        // Popping first MP3 frame with backpointed data from ADU0
        let mp3frames: Option<SingleMp3FrameOrList> = pending_adus
            .push_adus_pop_mp3_frames(&ELEM, adu0.into())
            .into();

        let mut mp3frame_iter = mp3frames.unwrap().into_iter();
        let mp3frame = mp3frame_iter.next().unwrap();
        {
            let mp3buf = mp3frame.buf.map_readable().unwrap();
            assert_eq!(mp3buf.len(), MP3FRAME_LEN);

            // Check 0 gap before ADU0 backpointed data
            assert!(
                mp3buf[HEADER_LEN + SIDE_INFO_LEN..][..DATA_CAPACITY - BACKPOINTER0]
                    .iter()
                    .eq(iter::repeat_n(&0u8, DATA_CAPACITY - BACKPOINTER0))
            );

            // Check ADU0 backpointed data after inserted dummy frame header
            assert!(
                mp3buf[HEADER_LEN + SIDE_INFO_LEN + DATA_CAPACITY - BACKPOINTER0..]
                    .iter()
                    .eq(iter::repeat_n(&0xb0u8, BACKPOINTER0))
            );
        }

        assert!(mp3frame_iter.next().is_none());

        // Popping second MP3 frame with framed data from ADU0
        // & backpointed data from ADU1
        let mp3frames: Option<SingleMp3FrameOrList> = pending_adus
            .push_adus_pop_mp3_frames(&ELEM, adu1.into())
            .into();
        let mut mp3frame_iter = mp3frames.unwrap().into_iter();
        let mp3frame = mp3frame_iter.next().unwrap();
        {
            let mp3buf = mp3frame.buf.map_readable().unwrap();
            assert_eq!(mp3buf.len(), MP3FRAME_LEN);

            // Check ADU0 framed data
            assert!(mp3buf[HEADER_LEN + SIDE_INFO_LEN..][..ADU0_FRAMED_DATA_LEN]
                .iter()
                .eq(iter::repeat_n(&0xf0u8, ADU0_FRAMED_DATA_LEN)));

            // Check ADU1 backpointed data
            assert!(mp3buf[HEADER_LEN + SIDE_INFO_LEN + ADU0_FRAMED_DATA_LEN..]
                .iter()
                .eq(iter::repeat_n(&0xb1u8, ADU1_BACKPOINTER)));
        }

        assert!(mp3frame_iter.next().is_none());

        // Draining third MP3 frame with framed data from ADU1
        let mp3frames: Option<SingleMp3FrameOrList> = pending_adus.drain(&ELEM).into();
        let mut mp3frame_iter = mp3frames.unwrap().into_iter();
        let mp3frame = mp3frame_iter.next().unwrap();
        {
            let mp3buf = mp3frame.buf.map_readable().unwrap();
            assert_eq!(mp3buf.len(), MP3FRAME_LEN);

            // Check ADU1 framed data
            assert!(mp3buf[HEADER_LEN + SIDE_INFO_LEN..]
                .iter()
                .eq(iter::repeat_n(&0xf1u8, ADU1_FRAMED_DATA_LEN)));
        }

        assert!(mp3frame_iter.next().is_none());
    }
}
