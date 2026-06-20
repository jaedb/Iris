// Copyright (C) 2025 Sanchayan Maity <sanchayan@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

#![allow(dead_code)]

use mp4_atom::{Atom, FourCC, Ftyp, Header, Mdat, Moof, Moov, ReadAtom, ReadFrom, Styp};
use std::io::Cursor;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ByteRange {
    length: u64,
    offset: u64,
}

impl ByteRange {
    fn end(&self) -> u64 {
        self.offset + self.length
    }
}

pub fn get_byte_ranges(s: &gst::StructureRef) -> Vec<ByteRange> {
    let mut ranges = Vec::new();

    if let Ok(br) = s.get::<gst::Structure>("initialization-segment-byte-range") {
        let length = br.get::<u64>("length").unwrap();
        let offset = br.get::<u64>("offset").unwrap();
        assert!(length != 0);
        ranges.push(ByteRange { length, offset })
    }

    if let Ok(br) = s.get::<gst::Structure>("segment-byte-range") {
        let length = br.get::<u64>("length").unwrap();
        let offset = br.get::<u64>("offset").unwrap();
        assert!(length != 0);
        ranges.push(ByteRange { length, offset })
    }

    ranges
}

pub fn validate_byterange_sequence(ranges: &[ByteRange]) -> bool {
    if ranges.is_empty() {
        return false; // Empty sequence is invalid
    }

    ranges.windows(2).all(|pair| {
        let current = &pair[0];
        let next = &pair[1];
        next.offset == current.offset + current.length
    })
}

fn ranges_overlap(a: &ByteRange, b: &ByteRange) -> bool {
    a.offset < b.end() && b.offset < a.end()
}

pub fn all_ranges_non_overlapping(ranges: &[ByteRange]) -> bool {
    if ranges.len() <= 1 {
        return true; // 0 or 1 range cannot overlap
    }

    for i in 0..ranges.len() {
        for j in (i + 1)..ranges.len() {
            if ranges_overlap(&ranges[i], &ranges[j]) {
                return false;
            }
        }
    }

    true
}

fn extract_range(source: &[u8], range: &ByteRange) -> Option<Vec<u8>> {
    let start = range.offset as usize;
    let length = range.length as usize;
    let end = start + length;

    if start > source.len() {
        return None;
    }

    if end > source.len() {
        return None;
    }

    Some(source[start..end].to_vec())
}

pub fn extract_ranges(source: &[u8], ranges: &[ByteRange]) -> Option<Vec<u8>> {
    let total_length = ranges.iter().map(|r| r.length as usize).sum::<usize>();

    let mut result = Vec::with_capacity(total_length);

    for range in ranges {
        let extracted = extract_range(source, range).unwrap();
        result.extend_from_slice(&extracted);
    }

    Some(result)
}

pub fn validate_iframe_segments(
    source: &[u8],
    ranges: &[ByteRange],
) -> Result<(), ValidationError> {
    // Skip the first ByteRange which will be the init segment.
    for range in ranges.iter().skip(1) {
        let extracted = extract_range(source, range).unwrap();
        let moof_count = validate_iframe_segment(&extracted)?;
        assert_eq!(moof_count, 1);
    }

    Ok(())
}

fn validate_ftyp_for_fragmented_mp4(ftyp: &Ftyp) -> Result<(), ValidationError> {
    let major_brand = &ftyp.major_brand;

    let fragmented_brands: Vec<FourCC> = [
        *b"isom", *b"iso2", *b"iso3", *b"iso4", *b"iso5", *b"iso6", *b"avc1", *b"mp41", *b"mp42",
        *b"dash", *b"cmfc", *b"cfsd",
    ]
    .iter()
    .map(FourCC::from)
    .collect::<Vec<_>>();

    let is_fragmented_brand = fragmented_brands.contains(major_brand)
        || ftyp
            .compatible_brands
            .iter()
            .any(|brand| fragmented_brands.contains(brand));

    if !is_fragmented_brand {
        return Err(ValidationError::UnsupportedFormat(format!(
            "Major brand {:?} may not support fragmentation",
            major_brand.to_string()
        )));
    }

    Ok(())
}

fn validate_moov_for_fragmented_mp4(moov: &Moov) -> Result<bool, ValidationError> {
    let has_mvex = moov.mvex.is_some();

    if !has_mvex {
        return Err(ValidationError::MissingRequiredAtom(
            "mvex (movie extends) atom required for fragmented MP4".to_string(),
        ));
    }

    Ok(has_mvex)
}

fn validate_moof(moof: &Moof) -> Result<(i32, u32), ValidationError> {
    // 0 is valid but the spec says that it "usually starts at 1"
    if moof.mfhd.sequence_number == 0 {
        return Err(ValidationError::InvalidAtomData(
            "Movie fragment header appears invalid".to_string(),
        ));
    }

    if moof.traf.is_empty() {
        return Err(ValidationError::MissingRequiredAtom(
            "traf (track fragment) required in moof".to_string(),
        ));
    }

    // For HLS/CMAF, only one track/stream should be present.
    if moof.traf.len() != 1 {
        return Err(ValidationError::StructuralError(
            "Only one traf (track fragment) required in moof for CMAF".to_string(),
        ));
    }

    let traf = moof.traf.first().unwrap();
    assert!(!traf.trun.is_empty());

    let data_offset = traf.trun.first().unwrap().data_offset;
    assert!(data_offset.is_some());

    // First trun entry will be key frame when chunking on key frames.
    let trun_entries: Vec<mp4_atom::TrunEntry> = traf.trun.first().unwrap().entries.clone();
    let trun_entry = trun_entries.first().unwrap();
    assert!(trun_entry.size.is_some());

    Ok((data_offset.unwrap(), trun_entry.size.unwrap()))
}

#[derive(Debug)]
pub enum ValidationError {
    ParseError(String),
    MissingRequiredAtom(String),
    InvalidAtomData(String),
    StructuralError(String),
    UnsupportedFormat(String),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            ValidationError::MissingRequiredAtom(atom) => {
                write!(f, "Missing required atom: {}", atom)
            }
            ValidationError::InvalidAtomData(msg) => write!(f, "Invalid atom data: {}", msg),
            ValidationError::StructuralError(msg) => write!(f, "Structural error: {}", msg),
            ValidationError::UnsupportedFormat(msg) => write!(f, "Unsupported format: {}", msg),
        }
    }
}

impl std::error::Error for ValidationError {}

pub fn validate_fragmented_mp4(data: &Vec<u8>) -> Result<(), ValidationError> {
    let mut has_ftyp = false;
    let mut has_moov = false;
    let mut has_moof = false;
    let mut fragment_count = 0;
    let mut found_mvex = false;

    let mut input = Cursor::new(data);

    while let Ok(header) = Header::read_from(&mut input) {
        match header.kind {
            Ftyp::KIND => {
                has_ftyp = true;
                let ftyp = Ftyp::read_atom(&header, &mut input).map_err(|e| {
                    ValidationError::ParseError(format!("Failed to parse ftyp: {}", e))
                })?;
                validate_ftyp_for_fragmented_mp4(&ftyp)?;
            }
            Moov::KIND => {
                has_moov = true;
                let moov = Moov::read_atom(&header, &mut input).map_err(|e| {
                    ValidationError::ParseError(format!("Failed to parse moov: {}", e))
                })?;
                found_mvex = validate_moov_for_fragmented_mp4(&moov)?;
            }
            Styp::KIND => {
                let _ = Styp::read_atom(&header, &mut input).map_err(|e| {
                    ValidationError::ParseError(format!("Failed to parse styp: {}", e))
                })?;
            }
            Moof::KIND => {
                has_moof = true;
                fragment_count += 1;
                let moof = Moof::read_atom(&header, &mut input).map_err(|e| {
                    ValidationError::ParseError(format!("Failed to parse moof: {}", e))
                })?;
                validate_moof(&moof)?;
            }
            Mdat::KIND => {
                let mdat = mp4_atom::Mdat::read_atom(&header, &mut input).unwrap();
                assert!(!mdat.data.is_empty());
            }
            _ => {
                panic!("Unexpected top level box: {:?}", header.kind);
            }
        }
    }

    if !has_ftyp {
        return Err(ValidationError::MissingRequiredAtom("ftyp".to_string()));
    }

    if !has_moov {
        return Err(ValidationError::MissingRequiredAtom("moov".to_string()));
    }

    if !has_moof {
        return Err(ValidationError::MissingRequiredAtom("moof".to_string()));
    }

    if !found_mvex {
        return Err(ValidationError::MissingRequiredAtom(
            "mvex in moov".to_string(),
        ));
    }

    if fragment_count == 0 {
        return Err(ValidationError::StructuralError(
            "No movie fragments found - not a fragmented MP4".to_string(),
        ));
    }

    Ok(())
}

fn validate_iframe_segment(data: &Vec<u8>) -> Result<u32, ValidationError> {
    let mut has_moof = false;
    let mut has_mdat = false;
    let mut fragment_count = 0;
    let mut moof_size = 0;
    let mut keyframe_size = 0;
    let mut trun_data_offset = 0;

    let input_size = data.len() as u64;
    let mut input = Cursor::new(data);

    while let Ok(header) = Header::read_from(&mut input) {
        match header.kind {
            Moof::KIND => {
                has_moof = true;
                fragment_count += 1;
                let moof = Moof::read_atom(&header, &mut input).map_err(|e| {
                    ValidationError::ParseError(format!("Failed to parse moof: {}", e))
                })?;
                moof_size = input.position();
                (trun_data_offset, keyframe_size) = validate_moof(&moof)?;
            }
            Mdat::KIND => {
                has_mdat = true;
                // We don't try to read `mdat` here as the I-frame
                // segment won't reference the complete `mdat` as
                // per the `mdat` size, but, only include the I-frame.
                assert!(8 /* moof header */ + moof_size + keyframe_size as u64 == input_size);
                assert!(8 /* moof header */ + moof_size == trun_data_offset as u64);
            }
            _ => {
                panic!("Unexpected top level box: {:?}", header.kind);
            }
        }
    }

    if !has_moof {
        return Err(ValidationError::MissingRequiredAtom("moof".to_string()));
    }

    if !has_mdat {
        return Err(ValidationError::MissingRequiredAtom("mdat".to_string()));
    }

    if fragment_count == 0 {
        return Err(ValidationError::StructuralError(
            "No movie fragments found - not a fragmented MP4".to_string(),
        ));
    }

    Ok(fragment_count)
}
