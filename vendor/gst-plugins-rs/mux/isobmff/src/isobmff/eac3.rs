// Copyright (C) 2021 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use anyhow::{bail, Context, Error};
use bitstream_io::{FromBitStream, ToBitStream};

const NUM_BLOCKS: [u8; 4] = [1, 2, 3, 6];
const SAMPLE_RATES: [u16; 4] = [48000, 44100, 32000, 0];

#[derive(Debug, Clone, Copy)]
pub struct Header {
    #[expect(unused)]
    pub syncinfo: SyncInfo,
    pub bsi: Bsi,
}

impl FromBitStream for Header {
    type Error = Error;

    fn from_reader<R: bitstream_io::BitRead + ?Sized>(r: &mut R) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let syncinfo = r.parse::<SyncInfo>().context("syncinfo")?;
        let bsi = r.parse::<Bsi>().context("bsi")?;

        Ok(Header { syncinfo, bsi })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SyncInfo {
    // No fields for EAC-3
}

impl FromBitStream for SyncInfo {
    type Error = Error;

    fn from_reader<R: bitstream_io::BitRead + ?Sized>(r: &mut R) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let _syncword = r.read_to::<u16>().context("syncword")?;
        if _syncword != 0x0b77 {
            bail!("Invalid syncword");
        }

        Ok(SyncInfo {})
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Bsi {
    #[expect(unused)]
    pub strmtyp: u8,
    pub substreamid: u8,
    pub frmsiz: u16,
    pub fscod: u8,
    pub fscod2: Option<u8>,
    pub numblkscod: u8,
    pub acmod: u8,
    pub lfeon: bool,
    pub bsid: u8,
    // skipping dialnorm, compre, compr, dialnorm2, compr2e
    pub chanmap: Option<u16>,
    // skipping ...
    pub bsmod: u8,
}

impl FromBitStream for Bsi {
    type Error = Error;

    fn from_reader<R: bitstream_io::BitRead + ?Sized>(r: &mut R) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let strmtyp = r.read::<2, u8>().context("strmtyp")?;
        let substreamid = r.read::<3, u8>().context("substreamid")?;
        let frmsiz = r.read::<11, u16>().context("frmsiz")?;
        let fscod = r.read::<2, u8>().context("fscod")?;

        let fscod2;
        let numblkscod;
        if fscod == 0x3 {
            fscod2 = Some(r.read::<2, u8>().context("fscod2")?);

            numblkscod = 6;
        } else {
            fscod2 = None;
            numblkscod = r.read::<2, u8>().context("numblkscod")?;
        }
        let number_of_blocks_per_sync_frame = NUM_BLOCKS[numblkscod as usize];

        let acmod = r.read::<3, u8>().context("acmod")?;
        let lfeon = r.read_bit().context("lfeon")?;
        let bsid = r.read::<5, u8>().context("bsid")?;

        r.skip(5).context("dialnorm")?;
        let compre = r.read_bit().context("compre")?;
        if compre {
            r.skip(8).context("compr")?;
        }

        if acmod == 0x00 {
            r.skip(5).context("dialnorm2")?;
            let compr2e = r.read_bit().context("compr2e")?;
            if compr2e {
                r.skip(8).context("compr2")?;
            }
        }

        let mut chanmap = None;
        if strmtyp == 0x1 {
            let chanmape = r.read_bit().context("chanmap2")?;
            if chanmape {
                chanmap = Some(r.read::<16, u16>().context("chanmap")?);
            }
        }

        let mixmdate = r.read_bit().context("mixmdate")?;
        if mixmdate {
            if acmod > 0x2 {
                r.skip(2).context("dmixmod")?;
            }
            if acmod & 0x1 != 0 && acmod > 0x2 {
                r.skip(3).context("ltrtcmixlev")?;
                r.skip(3).context("lorocmixlev")?;
            }
            if acmod & 0x4 != 0 {
                r.skip(3).context("ltrtsurmixlev")?;
                r.skip(3).context("lorosurmixlev")?;
            }
            if lfeon {
                let lfemixlevcode = r.read_bit().context("lfemixlevcode")?;
                if lfemixlevcode {
                    r.skip(5).context("lfemixlevcod")?;
                }
            }

            if strmtyp == 0x0 {
                let pgmscle = r.read_bit().context("pgmscle")?;
                if pgmscle {
                    r.skip(6).context("pgmscl")?;
                }
            }

            if acmod == 0x0 {
                let pgmscl2e = r.read_bit().context("pgmscl2e")?;
                if pgmscl2e {
                    r.skip(6).context("pgmscl2")?;
                }
            }

            let extpgmscle = r.read_bit().context("extpgmscle")?;
            if extpgmscle {
                r.skip(6).context("extpgmscl")?;
            }

            let mixdef = r.read::<2, u8>().context("mixdef")?;
            match mixdef {
                0x0 => {}
                0x1 => {
                    r.skip(1).context("premixcmpsel")?;
                    r.skip(1).context("drcsrc")?;
                    r.skip(3).context("premixcmpscl")?;
                }
                0x2 => {
                    r.skip(12).context("mixdata")?;
                }
                0x3 => {
                    let mixdeflen = r.read::<5, u8>().context("mixdeflen")?;
                    r.skip((mixdeflen as u32 + 2) * 8).context("mixdata")?;
                }
                _ => unreachable!(),
            }

            if acmod < 0x2 {
                let paninfoe = r.read_bit().context("paninfoe")?;
                if paninfoe {
                    r.skip(8).context("panmean")?;
                    r.skip(6).context("paninfo")?;
                }

                if acmod == 0x00 {
                    let paninfo2e = r.read_bit().context("paninfo2e")?;
                    if paninfo2e {
                        r.skip(8).context("panmean2")?;
                        r.skip(6).context("paninfo2")?;
                    }
                }
            }

            let frmmixcfginfoe = r.read_bit().context("frmmixcfginfoe")?;
            if frmmixcfginfoe {
                if numblkscod == 0 {
                    r.skip(5).context("blkmixcfginfo")?;
                } else {
                    for _ in 0..number_of_blocks_per_sync_frame {
                        let blkmixcfginfoe = r.read_bit().context("blkmixcfginfoe")?;
                        if blkmixcfginfoe {
                            r.skip(5).context("blkmixcfginfo")?;
                        }
                    }
                }
            }
        }

        let infomdate = r.read_bit().context("infomdate")?;
        let mut bsmod = 0;
        if infomdate {
            bsmod = r.read::<3, u8>().context("bsmod")?;
        }

        Ok(Bsi {
            strmtyp,
            substreamid,
            frmsiz,
            fscod,
            fscod2,
            numblkscod,
            acmod,
            lfeon,
            bsid,
            chanmap,
            bsmod,
        })
    }
}

#[derive(Debug)]
pub(crate) struct Dec3 {
    pub headers: Vec<Header>,
}

impl ToBitStream for Dec3 {
    type Error = Error;

    fn to_writer<W: bitstream_io::BitWrite + ?Sized>(&self, w: &mut W) -> Result<(), Self::Error>
    where
        Self: Sized,
    {
        struct IndSub {
            header: Header,
            num_dep_sub: u8,
            chan_loc: u16,
        }

        let mut num_ind_sub = 0u8;
        let mut ind_subs = Vec::new();

        // We assume the stream is well-formed and don't validate increasing
        // substream ids and that each first substream of an id is an independent
        // stream.
        for substream in self
            .headers
            .chunk_by(|h1, h2| h1.bsi.substreamid == h2.bsi.substreamid)
        {
            num_ind_sub += 1;

            let mut num_dep_sub = 0u8;

            let independent_stream = substream[0];

            let mut chan_loc = 0u16;
            for dependent_stream in substream.iter().skip(1) {
                num_dep_sub += 1;
                chan_loc |= dependent_stream
                    .bsi
                    .chanmap
                    .map(|chanmap| {
                        let mut chan_loc = 0u16;
                        for bit in 5..=14 {
                            if (chanmap & (1 << bit)) != 0 {
                                chan_loc |= match bit {
                                    5 => 1 << 0,
                                    6 => 1 << 1,
                                    7 => 1 << 2,
                                    8 => 1 << 3,
                                    9 => 1 << 4,
                                    10 => 1 << 5,
                                    11 => 1 << 6,
                                    12 => 1 << 7,
                                    13 => 0, // not mapped anywhere
                                    14 => 1 << 8,
                                    _ => unreachable!(),
                                }
                            }
                        }
                        chan_loc
                    })
                    .unwrap_or(0);
            }

            ind_subs.push(IndSub {
                header: independent_stream,
                // One less than the number of independent substreams, according to the spec
                num_dep_sub: num_dep_sub.saturating_sub(1),
                chan_loc,
            });
        }

        // One less than the number of independent substreams, according to the spec
        num_ind_sub = num_ind_sub.saturating_sub(1);

        let len = 4
            + 4
            + 2
            + ind_subs
                .iter()
                .map(|s| 3 + if s.num_dep_sub > 0 { 1 } else { 0 })
                .sum::<u32>();

        w.write_from::<u32>(len).context("size")?;
        w.write_bytes(b"dec3").context("type")?;

        let data_rate = self
            .headers
            .iter()
            .map(|header| {
                ((header.bsi.frmsiz as u32 + 1)
                    * if let Some(fscod2) = header.bsi.fscod2 {
                        SAMPLE_RATES[fscod2 as usize] as u32 / 2
                    } else {
                        SAMPLE_RATES[header.bsi.fscod as usize] as u32
                    })
                    / (NUM_BLOCKS[header.bsi.numblkscod as usize] as u32 * 16)
            })
            .sum::<u32>();
        w.write::<13, u16>((data_rate / 1000) as u16)
            .context("data_rate")?;

        w.write::<3, u8>(num_ind_sub).context("num_ind_sub")?;

        for ind_sub in ind_subs {
            w.write::<2, u8>(ind_sub.header.bsi.fscod)
                .context("fscod")?;
            w.write::<5, u8>(ind_sub.header.bsi.bsid).context("bsid")?;
            w.write::<1, u8>(0).context("reserved")?;

            // We take asvc from bsmod (Section 4.4.2.2). Not sure if this is correct, as it is
            // redundant to store info twice. My guess is that asvc is a quick way of telling the
            // decoder device to ignore the audio track, whereas bsmod provides a more detailed
            // explanation of the service type.
            w.write::<1, u8>(u8::from(
                (2..=6).contains(&ind_sub.header.bsi.bsmod)
                    || (ind_sub.header.bsi.bsmod == 7 && ind_sub.header.bsi.acmod < 2),
            ))
            .context("asvc")?;
            w.write::<3, u8>(ind_sub.header.bsi.bsmod)
                .context("bsmod")?;
            w.write::<3, u8>(ind_sub.header.bsi.acmod)
                .context("acmod")?;
            w.write_bit(ind_sub.header.bsi.lfeon).context("lfeon")?;
            w.write::<3, u8>(0).context("reserved")?;

            w.write::<4, u8>(ind_sub.num_dep_sub)
                .context("num_dep_sub")?;
            if ind_sub.num_dep_sub > 0 {
                w.write::<9, u16>(ind_sub.chan_loc).context("chan_loc")?;
            } else {
                w.write::<1, u8>(0).context("reserved")?;
            }
        }

        w.byte_align().context("reserved")?;

        Ok(())
    }
}
