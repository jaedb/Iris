// Copyright (C) 2017,2018 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

pub struct RingBuffer {
    buffer: Box<[f64]>,
    pos: usize,
}

impl RingBuffer {
    pub fn new(size: usize) -> Self {
        let mut buffer = Vec::with_capacity(size);
        buffer.extend(std::iter::repeat_n(0.0, size));

        Self {
            buffer: buffer.into_boxed_slice(),
            pos: 0,
        }
    }

    pub fn iter(&mut self, delay: usize) -> RingBufferIter<'_> {
        RingBufferIter::new(self, delay)
    }
}

pub struct RingBufferIter<'a> {
    buffer: &'a mut [f64],
    buffer_pos: &'a mut usize,
    read_pos: usize,
    write_pos: usize,
}

impl<'a> RingBufferIter<'a> {
    fn new(buffer: &'a mut RingBuffer, delay: usize) -> RingBufferIter<'a> {
        let size = buffer.buffer.len();

        assert!(size >= delay);
        assert_ne!(size, 0);

        let read_pos = (size - delay + buffer.pos) % size;
        let write_pos = buffer.pos % size;

        let buffer_pos = &mut buffer.pos;
        let buffer = &mut buffer.buffer;

        RingBufferIter {
            buffer,
            buffer_pos,
            read_pos,
            write_pos,
        }
    }
}

impl<'a> Iterator for RingBufferIter<'a> {
    type Item = (&'a mut f64, f64);

    fn next(&mut self) -> Option<Self::Item> {
        let res = unsafe {
            let r = *self.buffer.get_unchecked(self.read_pos);
            let w = self.buffer.get_unchecked_mut(self.write_pos);
            // Cast needed to get from &mut f64 to &'a mut f64
            (&mut *(w as *mut f64), r)
        };

        let size = self.buffer.len();
        self.write_pos = (self.write_pos + 1) % size;
        self.read_pos = (self.read_pos + 1) % size;

        Some(res)
    }
}

impl Drop for RingBufferIter<'_> {
    fn drop(&mut self) {
        *self.buffer_pos = self.write_pos;
    }
}
