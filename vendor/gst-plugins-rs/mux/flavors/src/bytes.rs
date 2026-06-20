// Copyright (C) 2016-2017 Sebastian Dröge <sebastian@centricular.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: MIT OR Apache-2.0

pub use byteorder::{BigEndian, LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io;

#[allow(unused)]
pub trait ReadBytesExtShort: io::Read {
    fn read_u16le(&mut self) -> io::Result<u16> {
        self.read_u16::<LittleEndian>()
    }
    fn read_i16le(&mut self) -> io::Result<i16> {
        self.read_i16::<LittleEndian>()
    }
    fn read_u32le(&mut self) -> io::Result<u32> {
        self.read_u32::<LittleEndian>()
    }
    fn read_i32le(&mut self) -> io::Result<i32> {
        self.read_i32::<LittleEndian>()
    }
    fn read_u64le(&mut self) -> io::Result<u64> {
        self.read_u64::<LittleEndian>()
    }
    fn read_i64le(&mut self) -> io::Result<i64> {
        self.read_i64::<LittleEndian>()
    }
    fn read_uintle(&mut self, nbytes: usize) -> io::Result<u64> {
        self.read_uint::<LittleEndian>(nbytes)
    }
    fn read_intle(&mut self, nbytes: usize) -> io::Result<i64> {
        self.read_int::<LittleEndian>(nbytes)
    }
    fn read_f32le(&mut self) -> io::Result<f32> {
        self.read_f32::<LittleEndian>()
    }
    fn read_f64le(&mut self) -> io::Result<f64> {
        self.read_f64::<LittleEndian>()
    }
    fn read_u16be(&mut self) -> io::Result<u16> {
        self.read_u16::<BigEndian>()
    }
    fn read_i16be(&mut self) -> io::Result<i16> {
        self.read_i16::<BigEndian>()
    }
    fn read_u32be(&mut self) -> io::Result<u32> {
        self.read_u32::<BigEndian>()
    }
    fn read_i32be(&mut self) -> io::Result<i32> {
        self.read_i32::<BigEndian>()
    }
    fn read_u64be(&mut self) -> io::Result<u64> {
        self.read_u64::<BigEndian>()
    }
    fn read_i64be(&mut self) -> io::Result<i64> {
        self.read_i64::<BigEndian>()
    }
    fn read_uintbe(&mut self, nbytes: usize) -> io::Result<u64> {
        self.read_uint::<BigEndian>(nbytes)
    }
    fn read_intbe(&mut self, nbytes: usize) -> io::Result<i64> {
        self.read_int::<BigEndian>(nbytes)
    }
    fn read_f32be(&mut self) -> io::Result<f32> {
        self.read_f32::<BigEndian>()
    }
    fn read_f64be(&mut self) -> io::Result<f64> {
        self.read_f64::<BigEndian>()
    }
}

impl<T> ReadBytesExtShort for T where T: ReadBytesExt {}

#[allow(unused)]
pub trait WriteBytesExtShort: WriteBytesExt {
    fn write_u16le(&mut self, n: u16) -> io::Result<()> {
        self.write_u16::<LittleEndian>(n)
    }
    fn write_i16le(&mut self, n: i16) -> io::Result<()> {
        self.write_i16::<LittleEndian>(n)
    }
    fn write_u32le(&mut self, n: u32) -> io::Result<()> {
        self.write_u32::<LittleEndian>(n)
    }
    fn write_i32le(&mut self, n: i32) -> io::Result<()> {
        self.write_i32::<LittleEndian>(n)
    }
    fn write_u64le(&mut self, n: u64) -> io::Result<()> {
        self.write_u64::<LittleEndian>(n)
    }
    fn write_i64le(&mut self, n: i64) -> io::Result<()> {
        self.write_i64::<LittleEndian>(n)
    }
    fn write_uintle(&mut self, n: u64, nbytes: usize) -> io::Result<()> {
        self.write_uint::<LittleEndian>(n, nbytes)
    }
    fn write_intle(&mut self, n: i64, nbytes: usize) -> io::Result<()> {
        self.write_int::<LittleEndian>(n, nbytes)
    }
    fn write_f32le(&mut self, n: f32) -> io::Result<()> {
        self.write_f32::<LittleEndian>(n)
    }
    fn write_f64le(&mut self, n: f64) -> io::Result<()> {
        self.write_f64::<LittleEndian>(n)
    }
    fn write_u16be(&mut self, n: u16) -> io::Result<()> {
        self.write_u16::<BigEndian>(n)
    }
    fn write_i16be(&mut self, n: i16) -> io::Result<()> {
        self.write_i16::<BigEndian>(n)
    }
    fn write_u32be(&mut self, n: u32) -> io::Result<()> {
        self.write_u32::<BigEndian>(n)
    }
    fn write_i32be(&mut self, n: i32) -> io::Result<()> {
        self.write_i32::<BigEndian>(n)
    }
    fn write_u64be(&mut self, n: u64) -> io::Result<()> {
        self.write_u64::<BigEndian>(n)
    }
    fn write_i64be(&mut self, n: i64) -> io::Result<()> {
        self.write_i64::<BigEndian>(n)
    }
    fn write_uintbe(&mut self, n: u64, nbytes: usize) -> io::Result<()> {
        self.write_uint::<BigEndian>(n, nbytes)
    }
    fn write_intbe(&mut self, n: i64, nbytes: usize) -> io::Result<()> {
        self.write_int::<BigEndian>(n, nbytes)
    }
    fn write_f32be(&mut self, n: f32) -> io::Result<()> {
        self.write_f32::<BigEndian>(n)
    }
    fn write_f64be(&mut self, n: f64) -> io::Result<()> {
        self.write_f64::<BigEndian>(n)
    }
}

impl<T> WriteBytesExtShort for T where T: WriteBytesExt {}
