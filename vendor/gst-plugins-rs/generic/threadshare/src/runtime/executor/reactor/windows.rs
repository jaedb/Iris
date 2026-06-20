// SPDX-License-Identifier: MIT OR Apache-2.0
// This is based on https://github.com/smol-rs/async-io

use polling::{Event, Poller};
use std::fmt;
use std::io::Result;
use std::os::windows::io::{AsRawSocket, BorrowedSocket, RawSocket};

/// The raw registration into the reactor.
#[doc(hidden)]
pub struct Registration {
    /// Raw socket handle on Windows.
    ///
    /// # Invariant
    ///
    /// This describes a valid socket that has not been `close`d. It will not be
    /// closed while this object is alive.
    raw: RawSocket,
}

unsafe impl Send for Registration {}
unsafe impl Sync for Registration {}

impl fmt::Debug for Registration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.raw, f)
    }
}

impl Registration {
    /// Add this file descriptor into the reactor.
    ///
    /// # Safety
    ///
    /// The provided file descriptor must be valid and not be closed while this object is alive.
    pub(crate) unsafe fn new(f: BorrowedSocket<'_>) -> Self {
        Self {
            raw: f.as_raw_socket(),
        }
    }

    /// Registers the object into the reactor.
    #[inline]
    pub(crate) fn add(&self, poller: &Poller, token: usize) -> Result<()> {
        // SAFETY: This object's existence validates the invariants of Poller::add
        unsafe { poller.add(self.raw, Event::none(token)) }
    }

    /// Re-registers the object into the reactor.
    #[inline]
    pub(crate) fn modify(&self, poller: &Poller, interest: Event) -> Result<()> {
        // SAFETY: self.raw is a valid file descriptor
        let fd = unsafe { BorrowedSocket::borrow_raw(self.raw) };
        poller.modify(fd, interest)
    }

    /// Deregisters the object from the reactor.
    #[inline]
    pub(crate) fn delete(&self, poller: &Poller) -> Result<()> {
        // SAFETY: self.raw is a valid file descriptor
        let fd = unsafe { BorrowedSocket::borrow_raw(self.raw) };
        poller.delete(fd)
    }
}
