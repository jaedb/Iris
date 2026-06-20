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
use bitstream_io::{
    BigEndian, ByteWrite as _, ByteWriter, FromByteStream, FromByteStreamWith, ToByteStream,
    ToByteStreamWith,
};
use smallvec::SmallVec;

const I_BIT: u8 = 0b1000_0000;
const P_BIT: u8 = 0b0100_0000;
const L_BIT: u8 = 0b0010_0000;
const F_BIT: u8 = 0b0001_0000;
const B_BIT: u8 = 0b0000_1000;
const E_BIT: u8 = 0b0000_0100;
const V_BIT: u8 = 0b0000_0010;
const Z_BIT: u8 = 0b0000_0001;
const N_BIT: u8 = 0b0000_0001;
const M_BIT: u8 = 0b1000_0000;

#[derive(Debug, Clone)]
pub struct PayloadDescriptor {
    pub picture_id: Option<PictureId>,
    pub layer_index: Option<LayerIndex>,
    pub inter_picture_predicted_frame: bool,
    pub flexible_mode: bool,
    pub reference_indices: SmallVec<[u8; 3]>,
    pub start_of_frame: bool,
    pub end_of_frame: bool,
    pub scalability_structure: Option<ScalabilityStructure>,
    pub not_reference_frame_for_upper_layers: bool,
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

        let picture_id = if (flags & I_BIT) != 0 {
            Some(r.parse::<PictureId>().context("picture_id")?)
        } else {
            None
        };

        let inter_picture_predicted_frame = (flags & P_BIT) != 0;

        let flexible_mode = (flags & F_BIT) != 0;

        let layer_index = if (flags & L_BIT) != 0 {
            Some(
                r.parse_with::<LayerIndex>(&flexible_mode)
                    .context("layer_index")?,
            )
        } else {
            None
        };

        let mut reference_indices = SmallVec::default();
        if inter_picture_predicted_frame && flexible_mode {
            for n in 0..3 {
                let p_diff = r.read::<u8>().context("p_diff")?;
                if n == 3 && (p_diff & N_BIT) != 0 {
                    bail!("More than 3 reference indices");
                }
                reference_indices.push(p_diff >> 1);
                if (p_diff & N_BIT) == 0 {
                    break;
                }
            }
        }

        let start_of_frame = (flags & B_BIT) != 0;
        let end_of_frame = (flags & E_BIT) != 0;

        let not_reference_frame_for_upper_layers = (flags & Z_BIT) != 0;

        let scalability_structure = if (flags & V_BIT) != 0 {
            Some(
                r.parse::<ScalabilityStructure>()
                    .context("scalability_structure")?,
            )
        } else {
            None
        };

        Ok(PayloadDescriptor {
            picture_id,
            layer_index,
            inter_picture_predicted_frame,
            flexible_mode,
            reference_indices,
            start_of_frame,
            end_of_frame,
            scalability_structure,
            not_reference_frame_for_upper_layers,
        })
    }
}

impl ToByteStream for PayloadDescriptor {
    type Error = anyhow::Error;

    fn to_writer<W: bitstream_io::ByteWrite + ?Sized>(&self, w: &mut W) -> Result<(), Self::Error>
    where
        Self: Sized,
    {
        if self.reference_indices.len() > 3 {
            bail!("Too many reference indices");
        }
        if self.inter_picture_predicted_frame
            && self.flexible_mode
            && self.reference_indices.is_empty()
        {
            bail!("Reference indices required");
        }

        let flags = if self.picture_id.is_some() { I_BIT } else { 0 }
            | if self.inter_picture_predicted_frame {
                P_BIT
            } else {
                0
            }
            | if self.flexible_mode { F_BIT } else { 0 }
            | if self.layer_index.is_some() { L_BIT } else { 0 }
            | if self.start_of_frame { B_BIT } else { 0 }
            | if self.end_of_frame { E_BIT } else { 0 }
            | if self.not_reference_frame_for_upper_layers {
                Z_BIT
            } else {
                0
            }
            | if self.scalability_structure.is_some() {
                V_BIT
            } else {
                0
            };

        w.write::<u8>(flags).context("flags")?;

        if let Some(picture_id) = self.picture_id {
            w.build::<PictureId>(&picture_id).context("picture_id")?;
        }

        if let Some(ref layer_index) = self.layer_index {
            w.build_with::<LayerIndex>(layer_index, &self.flexible_mode)
                .context("layer_index")?;
        }

        for (i, reference_index) in self.reference_indices.iter().enumerate() {
            if *reference_index > 0b0111_1111 {
                bail!("Too high reference index");
            }

            let b = if i == self.reference_indices.len() - 1 {
                N_BIT
            } else {
                0
            } | *reference_index;

            w.write::<u8>(b).context("reference_index")?;
        }

        if let Some(ref scalability_structure) = self.scalability_structure {
            w.build::<ScalabilityStructure>(scalability_structure)
                .context("scalability_structure")?;
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

#[derive(Debug, Clone)]
pub struct LayerIndex {
    pub temporal_layer_id: u8,
    pub switching_point: bool,
    pub spatial_layer_id: u8,
    pub inter_layer_dependency_used: bool,
    pub temporal_layer_zero_index: Option<u8>,
}

impl FromByteStreamWith<'_> for LayerIndex {
    type Error = anyhow::Error;
    /// Flexible mode?
    type Context = bool;

    fn from_reader<R: bitstream_io::ByteRead + ?Sized>(
        r: &mut R,
        flexible_mode: &Self::Context,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let layer_index = r.read::<u8>().context("layer_index")?;

        let temporal_layer_id = layer_index >> 5;
        let switching_point = (layer_index >> 4) & 0b0001 != 0;
        let spatial_layer_id = (layer_index >> 1) & 0b0111;
        let inter_layer_dependency_used = layer_index & 0b0001 != 0;

        let temporal_layer_zero_index = if !flexible_mode {
            Some(r.read::<u8>().context("temporal_layer_zero_index")?)
        } else {
            None
        };

        Ok(LayerIndex {
            temporal_layer_id,
            switching_point,
            spatial_layer_id,
            inter_layer_dependency_used,
            temporal_layer_zero_index,
        })
    }
}

impl ToByteStreamWith<'_> for LayerIndex {
    type Error = anyhow::Error;

    /// Flexible mode?
    type Context = bool;
    fn to_writer<W: bitstream_io::ByteWrite + ?Sized>(
        &self,
        w: &mut W,
        flexible_mode: &Self::Context,
    ) -> Result<(), Self::Error>
    where
        Self: Sized,
    {
        {
            if self.temporal_layer_id > 0b111 {
                bail!("Too high temporal layer id");
            }
            if self.spatial_layer_id > 0b111 {
                bail!("Too high spatial layer id");
            }

            if *flexible_mode && self.temporal_layer_zero_index.is_some() {
                bail!("temporal_layer_zero_index can't be provided in flexible mode");
            } else if !*flexible_mode && self.temporal_layer_zero_index.is_none() {
                bail!("temporal_layer_zero_index must be provided in non-flexible mode");
            }

            let b = (self.temporal_layer_id << 5)
                | (u8::from(self.switching_point) << 4)
                | (self.spatial_layer_id << 1)
                | u8::from(self.inter_layer_dependency_used);
            w.write::<u8>(b).context("layer_index")?;

            if let Some(temporal_layer_zero_index) = self.temporal_layer_zero_index {
                w.write::<u8>(temporal_layer_zero_index)
                    .context("temporal_layer_zero_index")?;
            }

            Ok(())
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScalabilityStructure {
    pub num_spatial_layers: u8,
    pub spatial_layer_frame_resolutions: SmallVec<[(u16, u16); 8]>,
    pub picture_description: SmallVec<[PictureDescription; 16]>,
}

impl FromByteStream for ScalabilityStructure {
    type Error = anyhow::Error;

    fn from_reader<R: bitstream_io::ByteRead + ?Sized>(r: &mut R) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        const Y_FLAG: u8 = 0b0001_0000;
        const G_FLAG: u8 = 0b0000_1000;

        let b = r.read::<u8>().context("scalability_structure")?;

        let num_spatial_layers = (b >> 5) + 1;

        let mut spatial_layer_frame_resolutions = SmallVec::default();
        if (b & Y_FLAG) != 0 {
            for _ in 0..num_spatial_layers {
                let width = r.read_as::<BigEndian, u16>().context("width")?;
                let height = r.read_as::<BigEndian, u16>().context("height")?;

                spatial_layer_frame_resolutions.push((width, height));
            }
        }

        let num_pictures_in_group = if (b & G_FLAG) != 0 {
            Some(r.read::<u8>().context("num_pictures_in_group")?)
        } else {
            None
        };

        let mut picture_description = SmallVec::new();
        if let Some(num_pictures_in_group) = num_pictures_in_group {
            picture_description.reserve(num_pictures_in_group as usize);
            for _ in 0..num_pictures_in_group {
                picture_description.push(
                    r.parse::<PictureDescription>()
                        .context("picture_description")?,
                );
            }
        }

        Ok(ScalabilityStructure {
            num_spatial_layers,
            spatial_layer_frame_resolutions,
            picture_description,
        })
    }
}

impl ToByteStream for ScalabilityStructure {
    type Error = anyhow::Error;

    fn to_writer<W: bitstream_io::ByteWrite + ?Sized>(&self, w: &mut W) -> Result<(), Self::Error>
    where
        Self: Sized,
    {
        const Y_FLAG: u8 = 0b0001_0000;
        const G_FLAG: u8 = 0b0000_1000;

        if self.num_spatial_layers == 0 {
            bail!("Zero spatial layers not allowed");
        }
        if self.num_spatial_layers - 1 > 0b111 {
            bail!("Too many spatial layers");
        }
        if self.picture_description.len() > 255 {
            bail!("Too many picture descriptions");
        }

        let b = ((self.num_spatial_layers - 1) << 5)
            | if !self.spatial_layer_frame_resolutions.is_empty() {
                Y_FLAG
            } else {
                0
            }
            | if !self.picture_description.is_empty() {
                G_FLAG
            } else {
                0
            };
        w.write::<u8>(b).context("scalability_structure")?;

        for (width, height) in &self.spatial_layer_frame_resolutions {
            w.write::<u16>(*width).context("width")?;
            w.write::<u16>(*height).context("height")?;
        }

        if !self.picture_description.is_empty() {
            w.write::<u8>(self.picture_description.len() as u8)
                .context("num_pictures_in_group")?;
        }

        for picture_description in &self.picture_description {
            w.build::<PictureDescription>(picture_description)
                .context("picture_description")?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PictureDescription {
    pub temporal_layer_id: u8,
    pub switching_point: bool,
    pub reference_indices: SmallVec<[u8; 3]>,
}

impl FromByteStream for PictureDescription {
    type Error = anyhow::Error;

    fn from_reader<R: bitstream_io::ByteRead + ?Sized>(r: &mut R) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let b = r.read::<u8>().context("picture_description")?;

        let temporal_layer_id = b >> 5;
        let switching_point = ((b >> 4) & 0b0001) != 0;

        let num_reference_indices = (b >> 2) & 0b0011;
        let mut reference_indices = SmallVec::default();
        for _ in 0..num_reference_indices {
            reference_indices.push(r.read::<u8>().context("reference_indices")?);
        }

        Ok(PictureDescription {
            temporal_layer_id,
            switching_point,
            reference_indices,
        })
    }
}

impl ToByteStream for PictureDescription {
    type Error = anyhow::Error;

    fn to_writer<W: bitstream_io::ByteWrite + ?Sized>(&self, w: &mut W) -> Result<(), Self::Error>
    where
        Self: Sized,
    {
        if self.temporal_layer_id > 0b111 {
            bail!("Too high temporal layer id");
        }

        if self.reference_indices.len() > 3 {
            bail!("Too many reference indices");
        }

        let b = (self.temporal_layer_id << 5)
            | (u8::from(self.switching_point) << 4)
            | ((self.reference_indices.len() as u8) << 2);

        w.write::<u8>(b).context("picture_description")?;

        for reference_index in &self.reference_indices {
            w.write::<u8>(*reference_index)
                .context("reference_indices")?;
        }

        Ok(())
    }
}
