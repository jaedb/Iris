// Rust RTSP Server
//
// Copyright (C) 2020-2021 Sebastian Dröge <sebastian@centricular.com>
// Copyright (C) 2024 Nirbheek Chauhan <nirbheek@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::cmp;
use std::fmt;
use std::ops;

use gst::buffer::{MappedBuffer, Readable};

/// Body used for RTSP messages in the server.
#[derive(Debug)]
pub struct Body(Inner);

enum Inner {
    Vec(Vec<u8>),
    Custom(Box<dyn Custom>),
    Buffer(MappedBuffer<Readable>),
}

trait Custom: AsRef<[u8]> + Send + Sync + 'static {}
impl<T: AsRef<[u8]> + Send + Sync + 'static> Custom for T {}

impl Default for Body {
    fn default() -> Self {
        Body(Inner::Vec(Vec::new()))
    }
}

impl Body {
    /// Create a body from custom memory without copying.
    #[allow(dead_code)]
    pub fn custom<T: AsRef<[u8]> + Send + Sync + 'static>(custom: T) -> Self {
        Body(Inner::Custom(Box::new(custom)))
    }

    pub fn mapped(mapped: MappedBuffer<Readable>) -> Self {
        Body(Inner::Buffer(mapped))
    }
}

impl From<Vec<u8>> for Body {
    fn from(v: Vec<u8>) -> Self {
        Body(Inner::Vec(v))
    }
}

impl<'a> From<&'a [u8]> for Body {
    fn from(s: &'a [u8]) -> Self {
        Body::from(Vec::from(s))
    }
}

impl ops::Deref for Body {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl AsRef<[u8]> for Body {
    fn as_ref(&self) -> &[u8] {
        match self.0 {
            Inner::Vec(ref vec) => vec.as_slice(),
            Inner::Custom(ref custom) => (**custom).as_ref(),
            Inner::Buffer(ref mapped) => mapped.as_ref(),
        }
    }
}

impl cmp::PartialEq for Body {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref().eq(other.as_ref())
    }
}

impl cmp::Eq for Body {}

impl Clone for Body {
    fn clone(&self) -> Self {
        Body::from(Vec::from(self.as_ref()))
    }
}

impl fmt::Debug for Inner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Inner::Vec(ref vec) => f.debug_tuple("Vec").field(&vec).finish(),
            Inner::Custom(ref custom) => {
                f.debug_tuple("Custom").field(&(**custom).as_ref()).finish()
            }
            Inner::Buffer(ref mapped) => mapped.fmt(f),
        }
    }
}
