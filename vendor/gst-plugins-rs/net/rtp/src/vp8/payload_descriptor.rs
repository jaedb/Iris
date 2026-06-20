//
// Copyright (C) 2023 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::io;

use anyhow::{bail, Context as _};
use bitstream_io::{BigEndian, ByteWrite as _, ByteWriter, FromByteStream, ToByteStream};

const X_BIT: u8 = 0b1000_0000;
const N_BIT: u8 = 0b0010_0000;
const S_BIT: u8 = 0b0001_0000;
const I_BIT: u8 = 0b1000_0000;
const L_BIT: u8 = 0b0100_0000;
const T_BIT: u8 = 0b0010_0000;
const K_BIT: u8 = 0b0001_0000;
const M_BIT: u8 = 0b1000_0000;

#[derive(Debug, Clone)]
pub struct PayloadDescriptor {
    pub non_reference_frame: bool,
    pub start_of_partition: bool,
    pub partition_index: u8,

    pub picture_id: Option<PictureId>,
    pub temporal_layer_zero_index: Option<u8>,
    pub temporal_layer_id: Option<LayerId>,
    pub key_index: Option<u8>,
}

#[derive(Debug, Clone)]
pub struct LayerId {
    pub id: u8,
    pub sync: bool,
}

impl PayloadDescriptor {
    pub fn size(&self) -> Result<usize, anyhow::Error> {
        #[derive(Default)]
        struct Counter(usize);

        impl io::Write for Counter {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.0 += buf.len();
                Ok(buf.len())
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let mut counter = Counter::default();
        let mut w = ByteWriter::endian(&mut counter, BigEndian);
        w.build::<PayloadDescriptor>(self)?;

        Ok(counter.0)
    }
}

impl FromByteStream for PayloadDescriptor {
    type Error = anyhow::Error;

    fn from_reader<R: bitstream_io::ByteRead + ?Sized>(r: &mut R) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let flags = r.read::<u8>().context("flags")?;

        let non_reference_frame = (flags & N_BIT) != 0;
        let start_of_partition = (flags & S_BIT) != 0;
        let partition_index = flags & 0b0000_0111;

        let ext_flags = if (flags & X_BIT) != 0 {
            r.read::<u8>().context("ext_flags")?
        } else {
            0
        };

        let picture_id = if (ext_flags & I_BIT) != 0 {
            Some(r.parse::<PictureId>().context("picture_id")?)
        } else {
            None
        };

        let temporal_layer_zero_index = if (ext_flags & L_BIT) != 0 {
            Some(r.read::<u8>().context("temporal_layer_zero_index")?)
        } else {
            None
        };

        let (temporal_layer_id, key_index) = if (ext_flags & T_BIT) != 0 || (ext_flags & K_BIT) != 0
        {
            let b = r.read::<u8>().context("tid_y_keyidx")?;

            let temporal_layer_id = if (ext_flags & T_BIT) != 0 {
                Some(LayerId {
                    id: b >> 6,
                    sync: (b & 0b0010_0000) != 0,
                })
            } else {
                None
            };

            let key_index = if (ext_flags & K_BIT) != 0 {
                Some(b & 0b0001_1111)
            } else {
                None
            };

            (temporal_layer_id, key_index)
        } else {
            (None, None)
        };

        Ok(PayloadDescriptor {
            non_reference_frame,
            start_of_partition,
            partition_index,
            picture_id,
            temporal_layer_zero_index,
            temporal_layer_id,
            key_index,
        })
    }
}

impl ToByteStream for PayloadDescriptor {
    type Error = anyhow::Error;

    fn to_writer<W: bitstream_io::ByteWrite + ?Sized>(&self, w: &mut W) -> Result<(), Self::Error>
    where
        Self: Sized,
    {
        if self.partition_index > 0b111 {
            bail!("Too large partition index");
        }

        let flags = if self.non_reference_frame { N_BIT } else { 0 }
            | if self.start_of_partition { S_BIT } else { 0 }
            | if self.picture_id.is_some()
                || self.temporal_layer_id.is_some()
                || self.temporal_layer_zero_index.is_some()
                || self.key_index.is_some()
            {
                X_BIT
            } else {
                0
            }
            | self.partition_index;

        w.write::<u8>(flags).context("flags")?;

        if (flags & X_BIT) != 0 {
            let ext_flags = if self.picture_id.is_some() { I_BIT } else { 0 }
                | if self.temporal_layer_zero_index.is_some() {
                    L_BIT
                } else {
                    0
                }
                | if self.temporal_layer_id.is_some() {
                    T_BIT
                } else {
                    0
                }
                | if self.key_index.is_some() { K_BIT } else { 0 };
            w.write::<u8>(ext_flags).context("ext_flags")?;
        }

        if let Some(picture_id) = self.picture_id {
            w.build::<PictureId>(&picture_id).context("picture_id")?;
        }

        if let Some(temporal_layer_zero_index) = self.temporal_layer_zero_index {
            w.write::<u8>(temporal_layer_zero_index)
                .context("temporal_layer_zero_index")?;
        }

        if self.temporal_layer_id.is_some() || self.key_index.is_some() {
            let mut b = 0;

            if let Some(LayerId { id, sync }) = self.temporal_layer_id {
                if id > 0b11 {
                    bail!("Too high temporal layer id");
                }

                b |= id << 6;
                b |= if sync { 0b0010_0000 } else { 0 };
            }

            if let Some(key_index) = self.key_index {
                if key_index > 0b0001_1111 {
                    bail!("Too high key index");
                }
                b |= key_index;
            }

            w.write::<u8>(b).context("tid_y_keyidx")?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PictureId {
    SevenBit(u8),
    FifteenBit(u16),
}

impl PictureId {
    pub fn new(mode: super::pay::PictureIdMode, v: u16) -> Option<Self> {
        match mode {
            super::pay::PictureIdMode::None => None,
            super::pay::PictureIdMode::SevenBit => Some(PictureId::SevenBit((v & 0x7f) as u8)),
            super::pay::PictureIdMode::FifteenBit => Some(PictureId::FifteenBit(v & 0x7fff)),
        }
    }

    pub fn increment(self) -> Self {
        match self {
            PictureId::SevenBit(v) => PictureId::SevenBit((v + 1) & 0x7f),
            PictureId::FifteenBit(v) => PictureId::FifteenBit((v + 1) & 0x7fff),
        }
    }

    pub fn update_mode(self, mode: super::pay::PictureIdMode) -> Self {
        match (self, mode) {
            (_, super::pay::PictureIdMode::None) => self,
            (PictureId::SevenBit(_), super::pay::PictureIdMode::SevenBit) => self,
            (PictureId::FifteenBit(_), super::pay::PictureIdMode::FifteenBit) => self,
            (PictureId::SevenBit(v), super::pay::PictureIdMode::FifteenBit) => {
                PictureId::FifteenBit(v as u16)
            }
            (PictureId::FifteenBit(v), super::pay::PictureIdMode::SevenBit) => {
                PictureId::SevenBit((v & 0x7f) as u8)
            }
        }
    }
}

impl From<PictureId> for u16 {
    fn from(value: PictureId) -> Self {
        match value {
            PictureId::SevenBit(v) => v as u16,
            PictureId::FifteenBit(v) => v,
        }
    }
}

impl PartialEq for PictureId {
    fn eq(&self, other: &Self) -> bool {
        match (*self, *other) {
            (PictureId::SevenBit(s), PictureId::SevenBit(o)) => s == o,
            (PictureId::SevenBit(s), PictureId::FifteenBit(o)) => s == (o & 0x7f) as u8,
            (PictureId::FifteenBit(s), PictureId::SevenBit(o)) => (s & 0x7f) as u8 == o,
            (PictureId::FifteenBit(s), PictureId::FifteenBit(o)) => s == o,
        }
    }
}

impl FromByteStream for PictureId {
    type Error = anyhow::Error;

    fn from_reader<R: bitstream_io::ByteRead + ?Sized>(r: &mut R) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let pid = r.read::<u8>().context("picture_id")?;
        if (pid & M_BIT) != 0 {
            let ext_pid = r.read::<u8>().context("extended_pid")?;
            Ok(PictureId::FifteenBit(
                (((pid & !M_BIT) as u16) << 8) | ext_pid as u16,
            ))
        } else {
            Ok(PictureId::SevenBit(pid))
        }
    }
}

impl ToByteStream for PictureId {
    type Error = anyhow::Error;

    fn to_writer<W: bitstream_io::ByteWrite + ?Sized>(&self, w: &mut W) -> Result<(), Self::Error>
    where
        Self: Sized,
    {
        match self {
            PictureId::SevenBit(v) => w.write::<u8>(*v).context("picture_id"),
            PictureId::FifteenBit(v) => {
                w.write::<u8>(M_BIT | (v >> 8) as u8)
                    .context("picture_id")?;
                w.write::<u8>((v & 0b1111_1111) as u8)
                    .context("extended_pid")
            }
        }
    }
}
