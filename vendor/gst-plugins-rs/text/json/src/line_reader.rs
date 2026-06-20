// Copyright (C) 2018 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::collections::VecDeque;

#[derive(Debug)]
pub struct LineReader<T: AsRef<[u8]>> {
    queue: VecDeque<T>,
    // Read position into the queue in bytes
    read_pos: usize,
    // Offset into queue where we have to look for a newline
    // All previous items don't contain a newline
    search_pos: usize,
    buf: Vec<u8>,
}

impl<T: AsRef<[u8]>> LineReader<T> {
    pub fn new() -> LineReader<T> {
        Self {
            queue: VecDeque::new(),
            read_pos: 0,
            search_pos: 0,
            buf: Vec::new(),
        }
    }

    pub fn push(&mut self, b: T) {
        self.queue.push_back(b);
    }

    /// Drops everything from the internal queue that was previously returned, i.e.
    /// if previously a line was returned we drop this whole line so that we can
    /// proceed with the next line
    fn drop_previous_line(&mut self) {
        // Drop everything we read the last time now
        while self.read_pos > 0
            && self.read_pos >= self.queue.front().map(|f| f.as_ref().len()).unwrap_or(0)
        {
            self.read_pos -= self.queue.front().map(|f| f.as_ref().len()).unwrap_or(0);
            self.queue.pop_front();
            if self.search_pos > 0 {
                self.search_pos -= 1;
            }
        }

        self.buf.clear();
    }

    #[allow(unused)]
    pub fn line_or_drain(&mut self) -> Option<&[u8]> {
        self.line_with_drain(true)
    }

    #[allow(unused)]
    pub fn line(&mut self) -> Option<&[u8]> {
        self.line_with_drain(false)
    }

    /// Searches the first '\n' in the currently queued buffers and returns the index in buffers
    /// inside the queue and the index in bytes from the beginning of the queue, or None.
    ///
    /// Also updates the search_pos so that we don't look again in the previous buffers on the next
    /// call
    fn find_newline(&mut self) -> Option<(usize, usize)> {
        let mut offset = 0;
        for (idx, buf) in self.queue.iter().enumerate() {
            let buf = buf.as_ref();

            // Fast skip-ahead
            if idx < self.search_pos {
                offset += buf.len();
                continue;
            }

            let pos = buf
                .iter()
                .enumerate()
                .skip(if idx == 0 { self.read_pos } else { 0 })
                .find(|(_, b)| **b == b'\n')
                .map(|(idx, _)| idx);

            if let Some(pos) = pos {
                // On the next call we have to search in this buffer again
                // as it might contain a second newline
                self.search_pos = idx;
                return Some((idx, offset + pos + 1));
            }

            // This buffer did not contain a newline so we don't have to look
            // in it again next time
            self.search_pos = idx + 1;
            offset += buf.len();
        }

        None
    }

    /// Copies length bytes from all buffers from the beginning until last_idx into our internal
    /// buffer, and skips the first offset bytes from the first buffer.
    fn copy_into_internal_buffer(&mut self, last_idx: usize, offset: usize, len: usize) {
        // Reserve space for the whole line beforehand
        if self.buf.capacity() < len {
            self.buf.reserve(len - self.buf.capacity());
        }

        // Then iterate over all buffers inside the queue until the one that contains
        // the newline character
        for (idx, buf) in self.queue.iter().enumerate().take(last_idx + 1) {
            let buf = buf.as_ref();

            // Calculate how much data we still have to copy
            let rem = len - self.buf.len();
            assert!(rem > 0);

            // For the first index we need to take into account the offset. The first
            // bytes might have to be skipped and as such we have fewer bytes available
            // than the whole length of the buffer
            let buf_len = if idx == 0 {
                assert!(offset < buf.len());
                buf.len() - offset
            } else {
                buf.len()
            };

            // Calculate how much we can copy from this buffer. At most the size of the buffer
            // itself, but never more than the amount we still have to copy overall
            let copy_len = if rem > buf_len { buf_len } else { rem };
            assert!(copy_len > 0);

            if idx == 0 {
                self.buf
                    .extend_from_slice(&buf[offset..(offset + copy_len)]);
            } else {
                self.buf.extend_from_slice(&buf[..copy_len]);
            }
        }

        assert_eq!(self.buf.len(), len);
    }

    pub fn line_with_drain(&mut self, drain: bool) -> Option<&[u8]> {
        // Drop all data from the previous line
        self.drop_previous_line();

        // read_pos must always be inside the first buffer of our queue here
        // or otherwise we went into an inconsistent state: the first buffer(s)
        // would've had to be dropped above then as they are not relevant anymore
        assert!(
            self.read_pos == 0
                || self.read_pos < self.queue.front().map(|f| f.as_ref().len()).unwrap_or(0)
        );

        // Find the next newline character from our previous position
        if let Some((idx, pos)) = self.find_newline() {
            // We now need to copy everything from the old read_pos to the new
            // pos, and on the next call we have to start from the new pos
            let old_read_pos = self.read_pos;
            self.read_pos = pos;

            assert!(self.read_pos > old_read_pos);
            assert!(idx < self.queue.len());

            // If the newline is found in the first buffer in our queue, we can directly return
            // the slice from it without doing any copying.
            //
            // On average this should be the most common case.
            if idx == 0 {
                let buf = self.queue.front().unwrap().as_ref();
                return Some(&buf[old_read_pos..self.read_pos]);
            } else {
                // Copy into our internal buffer as the current line spans multiple buffers
                let len = self.read_pos - old_read_pos;
                self.copy_into_internal_buffer(idx, old_read_pos, len);

                return Some(&self.buf[0..len]);
            }
        }

        // No newline found above and we're not draining, so let's wait until
        // more data is available that might contain a newline character
        if !drain {
            return None;
        }

        if self.queue.is_empty() {
            return None;
        }

        // When draining and we only have a single buffer in the queue we can
        // directly return a slice into it
        if self.queue.len() == 1 {
            let res = &self.queue.front().unwrap().as_ref()[self.read_pos..];
            self.read_pos += res.len();
            self.search_pos = 1;
            return Some(res);
        }

        // Otherwise we have to copy everything that is remaining into our
        // internal buffer and then return a slice from that
        let len = self.queue.iter().map(|v| v.as_ref().len()).sum::<usize>();
        if self.buf.capacity() < len {
            self.buf.reserve(len - self.buf.capacity());
        }

        for (idx, ref v) in self.queue.iter().enumerate() {
            if idx == 0 {
                self.buf.extend_from_slice(&v.as_ref()[self.read_pos..]);
            } else {
                self.buf.extend_from_slice(v.as_ref());
            }
        }

        self.read_pos += self.buf.len();
        self.search_pos = self.queue.len();

        Some(self.buf.as_ref())
    }

    pub fn clear(&mut self) {
        self.queue.clear();
        self.read_pos = 0;
        self.search_pos = 0;
        self.buf.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::LineReader;

    #[test]
    fn test_single_buffer() {
        let mut r = LineReader::new();
        r.push(Vec::from(b"abcd\nefgh\nijkl\n".as_ref()));

        assert_eq!(r.line(), Some(b"abcd\n".as_ref()));
        assert_eq!(r.line(), Some(b"efgh\n".as_ref()));
        assert_eq!(r.line(), Some(b"ijkl\n".as_ref()));
        assert_eq!(r.line(), None);
    }

    #[test]
    fn test_empty_line() {
        let mut r = LineReader::new();
        r.push(Vec::from(b"abcd\nefgh\n\nijkl\n".as_ref()));

        assert_eq!(r.line(), Some(b"abcd\n".as_ref()));
        assert_eq!(r.line(), Some(b"efgh\n".as_ref()));
        assert_eq!(r.line(), Some(b"\n".as_ref()));
        assert_eq!(r.line(), Some(b"ijkl\n".as_ref()));
        assert_eq!(r.line(), None);
    }

    #[test]
    fn test_multi_buffer_split() {
        let mut r = LineReader::new();
        r.push(Vec::from(b"abcd\nef".as_ref()));
        r.push(Vec::from(b"gh\nijkl\n".as_ref()));

        assert_eq!(r.line(), Some(b"abcd\n".as_ref()));
        assert_eq!(r.line(), Some(b"efgh\n".as_ref()));
        assert_eq!(r.line(), Some(b"ijkl\n".as_ref()));
        assert_eq!(r.line(), None);
    }

    #[test]
    fn test_multi_buffer_split_2() {
        let mut r = LineReader::new();
        r.push(Vec::from(b"abcd\ne".as_ref()));
        r.push(Vec::from(b"f".as_ref()));
        r.push(Vec::from(b"g".as_ref()));
        r.push(Vec::from(b"h\nijkl\n".as_ref()));

        assert_eq!(r.line(), Some(b"abcd\n".as_ref()));
        assert_eq!(r.line(), Some(b"efgh\n".as_ref()));
        assert_eq!(r.line(), Some(b"ijkl\n".as_ref()));
        assert_eq!(r.line(), None);
    }

    #[test]
    fn test_single_buffer_drain() {
        let mut r = LineReader::new();
        r.push(Vec::from(b"abcd\nefgh\nijkl".as_ref()));

        assert_eq!(r.line(), Some(b"abcd\n".as_ref()));
        assert_eq!(r.line(), Some(b"efgh\n".as_ref()));
        assert_eq!(r.line(), None);
        assert_eq!(r.line_or_drain(), Some(b"ijkl".as_ref()));
        assert_eq!(r.line_or_drain(), None);
    }

    #[test]
    fn test_single_buffer_drain_multi_line() {
        let mut r = LineReader::new();
        r.push(Vec::from(b"abcd\nefgh\n".as_ref()));
        r.push(Vec::from(b"ijkl".as_ref()));

        assert_eq!(r.line(), Some(b"abcd\n".as_ref()));
        assert_eq!(r.line(), Some(b"efgh\n".as_ref()));
        assert_eq!(r.line(), None);
        assert_eq!(r.line_or_drain(), Some(b"ijkl".as_ref()));
        assert_eq!(r.line_or_drain(), None);
    }

    #[test]
    fn test_single_buffer_drain_multi_line_2() {
        let mut r = LineReader::new();
        r.push(Vec::from(b"abcd\nefgh\ni".as_ref()));
        r.push(Vec::from(b"j".as_ref()));
        r.push(Vec::from(b"k".as_ref()));
        r.push(Vec::from(b"l".as_ref()));

        assert_eq!(r.line(), Some(b"abcd\n".as_ref()));
        assert_eq!(r.line(), Some(b"efgh\n".as_ref()));
        assert_eq!(r.line(), None);
        assert_eq!(r.line_or_drain(), Some(b"ijkl".as_ref()));
        assert_eq!(r.line_or_drain(), None);
    }
}
