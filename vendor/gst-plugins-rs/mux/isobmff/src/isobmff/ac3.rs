// Copyright (C) 2021 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use anyhow::{bail, Context, Error};
use bitstream_io::{FromBitStream, ToBitStream};

#[derive(Debug, Clone, Copy)]
pub struct Header {
    syncinfo: SyncInfo,
    bsi: Bsi,
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
struct SyncInfo {
    // skipping crc1
    fscod: u8,
    frmsizecod: u8,
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

        r.skip(16).context("crc1")?;

        let fscod = r.read::<2, u8>().context("fscod")?;
        let frmsizecod = r.read::<6, u8>().context("frmsizecod")?;

        Ok(SyncInfo { fscod, frmsizecod })
    }
}

#[derive(Debug, Clone, Copy)]
struct Bsi {
    bsid: u8,
    bsmod: u8,
    acmod: u8,
    // skipping cmixlev, surmixlev, dsurmod
    lfeon: bool,
}

impl FromBitStream for Bsi {
    type Error = Error;

    fn from_reader<R: bitstream_io::BitRead + ?Sized>(r: &mut R) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let bsid = r.read::<5, u8>().context("bsid")?;
        let bsmod = r.read::<3, u8>().context("bsmod")?;
        let acmod = r.read::<3, u8>().context("acmod")?;

        if acmod & 0x01 != 0 && acmod != 0x01 {
            r.skip(2).context("cmixlev")?;
        }
        if acmod & 0x04 != 0 {
            r.skip(2).context("surmixlev")?;
        }
        if acmod == 0x02 {
            r.skip(2).context("dsurmod")?;
        }

        let lfeon = r.read_bit().context("lfeon")?;

        Ok(Bsi {
            bsid,
            bsmod,
            acmod,
            lfeon,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Dac3 {
    pub header: Header,
}

impl ToBitStream for Dac3 {
    type Error = Error;

    fn to_writer<W: bitstream_io::BitWrite + ?Sized>(&self, w: &mut W) -> Result<(), Self::Error>
    where
        Self: Sized,
    {
        w.write_from::<u32>(11).context("size")?;
        w.write_bytes(b"dac3").context("type")?;

        w.write::<2, u8>(self.header.syncinfo.fscod)
            .context("fscod")?;
        w.write::<5, u8>(self.header.bsi.bsid).context("bsid")?;
        w.write::<3, u8>(self.header.bsi.bsmod).context("bsmod")?;
        w.write::<3, u8>(self.header.bsi.acmod).context("acmod")?;
        w.write_bit(self.header.bsi.lfeon).context("lfeon")?;
        w.write::<5, u8>(self.header.syncinfo.frmsizecod >> 1)
            .context("bit_rate_code")?;
        w.write::<5, u8>(0).context("reserved")?;

        assert!(w.byte_aligned());

        Ok(())
    }
}
