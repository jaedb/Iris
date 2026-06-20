// Copyright (C) 2022 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use crate::isobmff::boxes::{write_full_box, FULL_BOX_VERSION_0, FULL_BOX_VERSION_1};
use crate::isobmff::AuxiliaryInformation;
use anyhow::Error;

impl AuxiliaryInformation {
    pub(crate) fn write_full_saiz(&self, v: &mut Vec<u8>) -> Result<(), Error> {
        if let Some(aux_info_type) = self.aux_info_type {
            write_full_box(v, b"saiz", FULL_BOX_VERSION_0, 1, |v| {
                v.extend(aux_info_type);
                v.extend(self.aux_info_type_parameter.to_be_bytes());
                self.write_saiz_entries(v)
            })?;
        } else {
            write_full_box(v, b"saiz", FULL_BOX_VERSION_0, 0, |v| {
                self.write_saiz_entries(v)
            })?;
        }
        Ok(())
    }

    fn write_saiz_entries(&self, v: &mut Vec<u8>) -> Result<(), Error> {
        assert!(!self.entries.is_empty());
        let first_entry_length = self.entries[0].entry_len;
        if self.entries[1..]
            .iter()
            .all(|entry| entry.entry_len == first_entry_length)
        {
            v.extend(first_entry_length.to_be_bytes());
            v.extend((self.entries.len() as u32).to_be_bytes());
        } else {
            v.extend(0u8.to_be_bytes());
            v.extend((self.entries.len() as u32).to_be_bytes());
            for entry in &self.entries {
                v.extend(entry.entry_len.to_be_bytes());
            }
        }
        Ok(())
    }

    pub(crate) fn write_full_saio(&self, v: &mut Vec<u8>) -> Result<(), Error> {
        let version = if self
            .entries
            .iter()
            .any(|entry| entry.entry_offset > (u32::MAX as u64))
        {
            FULL_BOX_VERSION_1
        } else {
            FULL_BOX_VERSION_0
        };
        if let Some(aux_info_type) = self.aux_info_type {
            write_full_box(v, b"saio", version, 1, |v| {
                v.extend(aux_info_type);
                v.extend(self.aux_info_type_parameter.to_be_bytes());
                if version == FULL_BOX_VERSION_0 {
                    self.write_saio_entries_v0(v)
                } else {
                    self.write_saio_entries_v1(v)
                }
            })?;
        } else {
            write_full_box(v, b"saio", version, 0, |v| {
                if version == FULL_BOX_VERSION_0 {
                    self.write_saio_entries_v0(v)
                } else {
                    self.write_saio_entries_v1(v)
                }
            })?;
        }
        Ok(())
    }

    fn write_saio_entries_v0(&self, v: &mut Vec<u8>) -> Result<(), Error> {
        v.extend((self.entries.len() as u32).to_be_bytes());
        for entry in &self.entries {
            v.extend((entry.entry_offset as u32).to_be_bytes());
        }
        Ok(())
    }

    fn write_saio_entries_v1(&self, v: &mut Vec<u8>) -> Result<(), Error> {
        v.extend((self.entries.len() as u32).to_be_bytes());
        for entry in &self.entries {
            v.extend(entry.entry_offset.to_be_bytes());
        }
        Ok(())
    }
}
