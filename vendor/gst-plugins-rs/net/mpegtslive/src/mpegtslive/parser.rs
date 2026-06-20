// Copyright (C) 2024 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

#![allow(unused)]

use anyhow::{bail, Context, Result};
use bitstream_io::{
    BigEndian, BitRead, BitReader, ByteRead, ByteReader, FromBitStream, FromByteStream,
};
use smallvec::SmallVec;

pub struct SectionParser {
    /// Current value of the continuity counter
    cc: Option<u8>,
    /// Pending PSI data
    pending: Vec<u8>,
    /// If we skip data until the next PUSI
    waiting_for_pusi: bool,
    /// If pending starts on PUSI, i.e. still contains potential padding
    pending_starts_on_pusi: bool,
}

impl Default for SectionParser {
    fn default() -> Self {
        Self {
            cc: None,
            pending: Vec::new(),
            waiting_for_pusi: true,
            pending_starts_on_pusi: false,
        }
    }
}

impl SectionParser {
    /// Push PSI `payload`.
    ///
    /// After this call `parse()` until `None` is returned.
    pub fn push(
        &mut self,
        header: &PacketHeader,
        adaptation_field: Option<&AdaptionField>,

        payload: &[u8],
    ) {
        if header.pusi {
            self.clear();
        } else if adaptation_field.is_some_and(|af| af.discontinuity_flag) {
            // discontinuity_flag only defines that there is an expected discountinuity in the
            // continuity counter but the actual data is continuous
        } else if self.cc.is_none_or(|cc| (cc + 1) & 0xf != header.cc) {
            self.clear();
            self.waiting_for_pusi = true;
            // Not start of a payload and we didn't see the start, just return
            return;
        } else if self.waiting_for_pusi {
            // Not start of a payload and we didn't see the start, just return
            return;
        }
        self.cc = Some(header.cc);

        // Store payload for parsing, in case it's split over multiple packets
        if header.pusi {
            self.waiting_for_pusi = false;
            self.pending_starts_on_pusi = true;
        }
        self.pending.extend_from_slice(payload);
    }

    /// Parse PSI payload that is currently queued up.
    ///
    /// Call until `None` is returned, which means that more data is required to continue parsing.
    ///
    /// It's safe to call this again after errors.
    pub fn parse(&mut self) -> Result<Option<Section>> {
        // No payload to handle right now
        if self.pending.is_empty() {
            return Ok(None);
        }

        let payload = self.pending.as_slice();

        // Skip padding first
        if self.pending_starts_on_pusi {
            let pointer_field = payload[0] as usize;
            // Need more data
            if payload.len() < 1 + pointer_field {
                return Ok(None);
            }

            // Skip padding
            self.pending.copy_within(1 + pointer_field.., 0);
            let new_length = self.pending.len() - 1 - pointer_field;
            self.pending.resize(new_length, 0u8);
            self.pending_starts_on_pusi = false;
        }

        let payload = self.pending.as_slice();
        if payload.len() < 3 {
            // Need more data for table header
            return Ok(None);
        }

        // Parse table header, payload_reader stays at beginning of section header
        let mut payload_reader = BitReader::endian(payload, BigEndian);
        let table_header = match payload_reader
            .parse::<TableHeader>()
            .context("table_header")
        {
            Ok(table_header) => table_header,
            Err(err) => {
                self.clear();
                return Err(err);
            }
        };

        // Need more data for this section, don't update pending
        let remaining_length = payload_reader.reader().unwrap().len();
        if remaining_length < table_header.section_length as usize {
            return Ok(None);
        }

        let section = &payload_reader.reader().unwrap()[..table_header.section_length as usize];
        // Skip whole section so the reader is at the beginning of the next section header
        payload_reader
            .skip(8 * table_header.section_length as u32)
            .unwrap();

        let section = Self::parse_section(&table_header, section);

        // Skip parsed section, even in case of parsing error
        let remaining_length = payload_reader.reader().unwrap().len();
        let new_pending_range = (self.pending.len() - remaining_length)..;
        self.pending.copy_within(new_pending_range, 0);
        self.pending.resize(remaining_length, 0u8);

        section
            .map(Some)
            .map_err(|err| err.context(format!("section with table header {table_header:?}")))
    }

    fn parse_section(table_header: &TableHeader, section: &[u8]) -> Result<Section> {
        let mut section_reader = BitReader::endian(section, BigEndian);

        // TODO: If TSS is available one could check the CRC32 at the end of the section
        let table_syntax_section = if table_header.section_syntax_indicator {
            Some(
                section_reader
                    .parse::<TableSyntaxSection>()
                    .context("section")?,
            )
        } else {
            None
        };

        let section = match table_header.table_id {
            // PAT
            0x00 => {
                let Some(table_syntax_section) = table_syntax_section else {
                    bail!("PAT without TSS");
                };

                let remaining_length = section_reader.reader().unwrap().len();
                if remaining_length < 4 {
                    bail!("too short PAT");
                }
                let n_pats = (remaining_length - 4) / 4;
                let mut pat = SmallVec::with_capacity(n_pats);
                for _ in 0..n_pats {
                    pat.push(
                        section_reader
                            .parse::<ProgramAccessTable>()
                            .context("pat_entry")?,
                    );
                }

                Section::ProgramAccessTable {
                    table_header: table_header.clone(),
                    table_syntax_section,
                    pat,
                }
            }
            // PAT
            0x02 => {
                let Some(table_syntax_section) = table_syntax_section else {
                    bail!("PMT without TSS");
                };
                let pmt = section_reader
                    .parse::<ProgramMappingTable>()
                    .context("pmt")?;

                Section::ProgramMappingTable {
                    table_header: table_header.clone(),
                    table_syntax_section,
                    pmt,
                }
            }
            // Unknown
            _ => Section::Unknown {
                table_header: table_header.clone(),
                table_syntax_section,
            },
        };

        Ok(section)
    }

    pub fn clear(&mut self) {
        self.cc = None;
        self.pending.clear();
        self.pending_starts_on_pusi = false;
        self.waiting_for_pusi = true;
    }
}

#[derive(Debug, Clone)]
pub enum Section {
    ProgramAccessTable {
        table_header: TableHeader,
        table_syntax_section: TableSyntaxSection,
        pat: SmallVec<[ProgramAccessTable; 4]>,
    },
    ProgramMappingTable {
        table_header: TableHeader,
        table_syntax_section: TableSyntaxSection,
        pmt: ProgramMappingTable,
    },
    Unknown {
        table_header: TableHeader,
        table_syntax_section: Option<TableSyntaxSection>,
    },
}

#[derive(Debug, Clone)]
pub struct TableHeader {
    pub table_id: u8,
    pub section_syntax_indicator: bool,
    pub section_length: u16,
}

impl FromBitStream for TableHeader {
    type Error = anyhow::Error;

    fn from_reader<R: BitRead + ?Sized>(r: &mut R) -> std::result::Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let table_id = r.read_to::<u8>().context("table_id")?;
        let section_syntax_indicator = r.read_bit().context("table_syntax_indicator")?;
        r.skip(5).context("reserved")?;
        let section_length = r.read::<10, u16>().context("section_length")?;

        Ok(TableHeader {
            table_id,
            section_syntax_indicator,
            section_length,
        })
    }
}

#[derive(Debug, Clone)]
pub struct TableSyntaxSection {
    pub table_id_extension: u16,
    pub version_number: u8,
    pub current_next_indicator: bool,
    pub section_number: u8,
    pub last_section_number: u8,
}

impl FromBitStream for TableSyntaxSection {
    type Error = anyhow::Error;

    fn from_reader<R: BitRead + ?Sized>(r: &mut R) -> std::result::Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let table_id_extension = r.read_to::<u16>().context("table_id_extension")?;
        r.skip(2).context("reserved")?;
        let version_number = r.read::<5, u8>().context("version_number")?;
        let current_next_indicator = r.read_bit().context("current_next_indicator")?;
        let section_number = r.read_to::<u8>().context("section_number")?;
        let last_section_number = r.read_to::<u8>().context("last_section_number")?;

        Ok(TableSyntaxSection {
            table_id_extension,
            version_number,
            current_next_indicator,
            section_number,
            last_section_number,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProgramAccessTable {
    pub program_num: u16,
    pub program_map_pid: u16,
}

impl FromBitStream for ProgramAccessTable {
    type Error = anyhow::Error;

    fn from_reader<R: BitRead + ?Sized>(r: &mut R) -> std::result::Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let program_num = r.read_to::<u16>().context("program_num")?;
        r.skip(3).context("reserved")?;
        let program_map_pid = r.read::<13, u16>().context("program_map_pid")?;

        Ok(ProgramAccessTable {
            program_num,
            program_map_pid,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProgramMappingTable {
    pub pcr_pid: u16,
    pub elementary_pids: SmallVec<[u16; 16]>,
    // Add other fields as needed
}

impl FromBitStream for ProgramMappingTable {
    type Error = anyhow::Error;

    fn from_reader<R: BitRead + ?Sized>(r: &mut R) -> std::result::Result<Self, Self::Error>
    where
        Self: Sized,
    {
        r.skip(3).context("reserved")?;
        let pcr_pid = r.read::<13, u16>().context("pcr_pid")?;
        r.skip(4).context("reserved")?;
        r.skip(2).context("program_info_length_unused")?;

        let program_info_length = r.read::<10, u16>().context("program_info_length")?;
        r.skip(8 * program_info_length as u32)
            .context("program_descriptors")?;

        fn try_read<R: BitRead + ?Sized, F: Fn(&mut R) -> Result<T, std::io::Error>, T>(
            r: &mut R,
            op: F,
        ) -> Result<Option<T>, std::io::Error> {
            match op(r) {
                Ok(v) => Ok(Some(v)),
                Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => Ok(None),
                Err(err) => Err(err),
            }
        }

        let mut elementary_pids = SmallVec::new();
        loop {
            let Some(_stream_type) = try_read(r, |r| r.read_to::<u8>()).context("stream_type")?
            else {
                break;
            };
            let Some(_) = try_read(r, |r| r.skip(3)).context("reserved_bits")? else {
                break;
            };

            let Some(elementary_pid) =
                try_read(r, |r| r.read::<13, u16>()).context("elementary_pid")?
            else {
                break;
            };

            let Some(_) = try_read(r, |r| r.skip(4)).context("reserved_bits")? else {
                break;
            };

            let Some(_) = try_read(r, |r| r.skip(2)).context("es_info_length_unused_bits")? else {
                break;
            };

            let Some(es_info_length) =
                try_read(r, |r| r.read::<10, u16>()).context("es_info_length")?
            else {
                break;
            };

            let Some(_) =
                try_read(r, |r| r.skip(8 * es_info_length as u32)).context("es_descriptors")?
            else {
                break;
            };

            elementary_pids.push(elementary_pid);
        }

        Ok(ProgramMappingTable {
            pcr_pid,
            elementary_pids,
        })
    }
}

#[derive(Debug)]
pub struct PESParser {
    /// Current value of the continuity counter
    cc: Option<u8>,
    /// Pending PES data
    pending: Vec<u8>,
    /// If we skip data until the next PUSI
    waiting_for_pusi: bool,
}

impl Default for PESParser {
    fn default() -> Self {
        Self {
            cc: None,
            pending: Vec::new(),
            waiting_for_pusi: true,
        }
    }
}

impl PESParser {
    /// Push PES `payload`.
    ///
    /// After this call `parse()` until `None` is returned.
    pub fn push(
        &mut self,
        header: &PacketHeader,
        adaptation_field: Option<&AdaptionField>,

        payload: &[u8],
    ) {
        if header.pusi {
            self.clear();
        } else if adaptation_field.is_some_and(|af| af.discontinuity_flag) {
            // discontinuity_flag only defines that there is an expected discountinuity in the
            // continuity counter but the actual data is continuous
        } else if self.cc.is_none_or(|cc| (cc + 1) & 0xf != header.cc) {
            self.clear();
            self.waiting_for_pusi = true;
            // Not start of a payload and we didn't see the start, just return
            return;
        } else if self.waiting_for_pusi {
            // Not start of a payload and we didn't see the start, just return
            return;
        }
        self.cc = Some(header.cc);

        // Store payload for parsing, in case it's split over multiple packets
        if header.pusi {
            self.waiting_for_pusi = false;
        }
        self.pending.extend_from_slice(payload);
    }

    /// Parse PES payload that is currently queued up.
    ///
    /// Call until `None` is returned, which means that more data is required to continue parsing.
    ///
    /// It is safe to call this again after an error.
    pub fn parse(&mut self) -> Result<Option<(PESHeader, Option<OptionalPESHeader>)>> {
        match self.parse_internal() {
            Ok(res) => Ok(res),
            Err(err) => {
                self.clear();
                Err(err)
            }
        }
    }

    fn parse_internal(&mut self) -> Result<Option<(PESHeader, Option<OptionalPESHeader>)>> {
        // No payload to handle right now
        if self.pending.is_empty() {
            return Ok(None);
        }

        let payload = self.pending.as_slice();

        // Size of PES header
        if payload.len() < 3 {
            return Ok(None);
        }

        let mut reader = ByteReader::endian(payload, BigEndian);
        let header = reader.parse::<PESHeader>().context("PES header")?;

        // Stream IDs without optional PES header
        if header.stream_id == 0xbc
            || header.stream_id == 0xbe
            || header.stream_id == 0xbf
            || (0xf0..=0xf2).contains(&header.stream_id)
            || header.stream_id == 0xff
            || header.stream_id == 0xf8
        {
            // We only care about the header so stop parsing now
            self.pending.clear();
            self.waiting_for_pusi = true;
            return Ok(Some((header, None)));
        }

        // Size of PES header + size of optional PES header
        if payload.len() < 6 {
            return Ok(None);
        }

        let optional_pes_header_flags =
            reader.read::<u16>().context("optional_pes_header_flags")?;
        if optional_pes_header_flags & 0b1100_0000_0000_0000 != 0b1000_0000_0000_0000 {
            bail!("Missing marker bits");
        }
        if optional_pes_header_flags & 0b0000_0000_1100_0000 == 0b0000_0000_0100_0000 {
            bail!("DTS without PTS is forbidden");
        }

        let optional_pes_header_length =
            reader.read::<u8>().context("optional_pes_header_length")?;

        let remaining_length = reader.reader().len();
        if remaining_length < optional_pes_header_length as usize {
            return Ok(None);
        }

        fn read_pes_ts<R: ByteRead + ?Sized>(r: &mut R) -> std::result::Result<u64, anyhow::Error> {
            let mut ts = [0u8; 5];
            r.read_bytes(&mut ts).context("pes_ts")?;

            if ts[0] & 0x01 != 0x01 || ts[2] & 0x01 != 0x01 || ts[4] & 0x01 != 0x01 {
                bail!("lost PTS sync");
            }

            let ts = ((ts[0] as u64 & 0x0e) << 29)
                | ((ts[1] as u64) << 22)
                | ((ts[2] as u64 & 0xfe) << 14)
                | ((ts[3] as u64) << 7)
                | ((ts[4] as u64 & 0xfe) >> 1);

            Ok(ts)
        }

        let pts = if optional_pes_header_flags & 0b0000_0000_1000_0000 != 0 {
            let pts = read_pes_ts(&mut reader).context("pts")?;
            Some(pts)
        } else {
            None
        };

        let dts = if optional_pes_header_flags & 0b0000_0000_0100_0000 != 0 {
            let dts = read_pes_ts(&mut reader).context("dts")?;
            Some(dts)
        } else {
            None
        };

        let optional_pes_header = OptionalPESHeader {
            flags: optional_pes_header_flags,
            length: optional_pes_header_length,
            pts,
            dts,
        };

        // We only care about the header so stop parsing now
        self.pending.clear();
        self.waiting_for_pusi = true;

        Ok(Some((header, Some(optional_pes_header))))
    }

    pub fn clear(&mut self) {
        self.cc = None;
        self.pending.clear();
        self.waiting_for_pusi = true;
    }
}

#[derive(Debug, Clone)]
pub struct PESHeader {
    pub stream_id: u8,
    pub length: u16,
}

impl FromByteStream for PESHeader {
    type Error = anyhow::Error;

    fn from_reader<R: ByteRead + ?Sized>(r: &mut R) -> std::result::Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let start_code = r.read::<u32>().context("packet_start_code")?;

        if start_code >> 8 != 0x00_00_01 {
            bail!("Lost sync");
        }

        let stream_id = (start_code & 0xff) as u8;
        let length = r.read::<u16>().context("packet_length")?;

        Ok(PESHeader { stream_id, length })
    }
}

#[derive(Debug, Clone)]
pub struct OptionalPESHeader {
    pub flags: u16,
    pub length: u8,
    pub pts: Option<u64>,
    pub dts: Option<u64>,
    // Add other fields as needed
}

#[derive(Debug, Clone)]
pub struct PacketHeader {
    pub tei: bool,
    pub pusi: bool,
    pub tp: bool,
    pub pid: u16,
    pub tsc: u8,
    pub afc: u8,
    pub cc: u8,
}

impl FromBitStream for PacketHeader {
    type Error = anyhow::Error;

    fn from_reader<R: BitRead + ?Sized>(r: &mut R) -> std::result::Result<Self, Self::Error>
    where
        Self: Sized,
    {
        if r.read_to::<u8>().context("sync_byte")? != 0x47 {
            bail!("Lost sync");
        }

        let tei = r.read_bit().context("tei")?;
        let pusi = r.read_bit().context("pusi")?;
        let tp = r.read_bit().context("tp")?;
        let pid = r.read::<13, u16>().context("pid")?;

        let tsc = r.read::<2, u8>().context("tsc")?;
        let afc = r.read::<2, u8>().context("afc")?;
        let cc = r.read::<4, u8>().context("cc")?;

        Ok(PacketHeader {
            tei,
            pusi,
            tp,
            pid,
            tsc,
            afc,
            cc,
        })
    }
}

#[derive(Debug, Clone)]
pub struct AdaptionField {
    pub discontinuity_flag: bool,
    pub pcr: Option<u64>,
    // Add other fields as needed
}

impl FromBitStream for AdaptionField {
    type Error = anyhow::Error;

    fn from_reader<R: BitRead + ?Sized>(r: &mut R) -> std::result::Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let discontinuity_flag = r.read_bit().context("discontinuity_flag")?;
        r.skip(2).context("flags")?;
        let pcr_present = r.read_bit().context("pcr_present")?;
        r.skip(4).context("flags")?;

        // PCR present
        let pcr = if pcr_present {
            let pcr = r.read::<33, u64>().context("pcr_base")? * 300;
            r.skip(6).context("pcr_reserved")?;
            let pcr = pcr + r.read::<9, u64>().context("pcr_extension")? % 300;
            Some(pcr)
        } else {
            None
        };

        // Skip all other parts of the adaptation field for now

        Ok(AdaptionField {
            discontinuity_flag,
            pcr,
        })
    }
}
