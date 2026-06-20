// Copyright (C) 2018 Sebastian Dröge <sebastian@centricular.com>
// Copyright (C) 2018 LEE Dongjun <redongjun@gmail.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;

use std::sync::LazyLock;

use std::error;
use std::fmt;
use std::future::Future;
use std::io;
use std::net::UdpSocket;

use crate::runtime::Async;

#[cfg(unix)]
use std::os::fd::OwnedFd;

#[cfg(windows)]
use std::os::windows::io::OwnedSocket;

static SOCKET_CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "ts-socket",
        gst::DebugColorFlags::empty(),
        Some("Thread-sharing Socket"),
    )
});

pub trait SocketRead: Send + Unpin {
    const DO_TIMESTAMP: bool;

    fn read<'buf>(
        &'buf mut self,
        buffer: &'buf mut [u8],
    ) -> impl Future<Output = io::Result<(usize, Option<std::net::SocketAddr>)>> + Send;
}

pub struct Socket<T: SocketRead> {
    element: gst::Element,
    buffer_pool: gst::BufferPool,
    reader: T,
    mapped_buffer: Option<gst::MappedBuffer<gst::buffer::Writable>>,
    clock: Option<gst::Clock>,
    base_time: Option<gst::ClockTime>,
}

impl<T: SocketRead> Socket<T> {
    pub fn try_new(
        element: gst::Element,
        buffer_pool: gst::BufferPool,
        reader: T,
    ) -> Result<Self, glib::BoolError> {
        // FIXME couldn't we just delegate this to caller?
        buffer_pool.set_active(true).map_err(|err| {
            gst::error!(
                SOCKET_CAT,
                obj = element,
                "Failed to prepare socket: {}",
                err
            );

            err
        })?;

        Ok(Socket::<T> {
            buffer_pool,
            element,
            reader,
            mapped_buffer: None,
            clock: None,
            base_time: None,
        })
    }

    pub fn set_clock(&mut self, clock: Option<gst::Clock>, base_time: Option<gst::ClockTime>) {
        self.clock = clock;
        self.base_time = base_time;
    }

    pub fn get(&self) -> &T {
        &self.reader
    }
}

#[derive(Debug)]
pub enum SocketError {
    Gst(gst::FlowError),
    Io(io::Error),
}

impl error::Error for SocketError {}

impl fmt::Display for SocketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SocketError::Gst(err) => write!(f, "flow error: {err}"),
            SocketError::Io(err) => write!(f, "IO error: {err}"),
        }
    }
}

impl<T: SocketRead> Socket<T> {
    // Can't implement this as a Stream trait because we end up using things like
    // tokio::net::UdpSocket which don't implement pollable functions.
    #[allow(clippy::should_implement_trait)]
    pub async fn try_next(
        &mut self,
    ) -> Result<(gst::Buffer, Option<std::net::SocketAddr>), SocketError> {
        gst::log!(SOCKET_CAT, obj = self.element, "Trying to read data");

        if self.mapped_buffer.is_none() {
            match self.buffer_pool.acquire_buffer(None) {
                Ok(buffer) => {
                    self.mapped_buffer = Some(buffer.into_mapped_buffer_writable().unwrap());
                }
                Err(err) => {
                    gst::debug!(
                        SOCKET_CAT,
                        obj = self.element,
                        "Failed to acquire buffer {:?}",
                        err
                    );
                    return Err(SocketError::Gst(err));
                }
            }
        }

        match self
            .reader
            .read(self.mapped_buffer.as_mut().unwrap().as_mut_slice())
            .await
        {
            Ok((len, saddr)) => {
                let dts = if T::DO_TIMESTAMP {
                    let time = self.clock.as_ref().unwrap().time();
                    let running_time = time.opt_checked_sub(self.base_time).ok().flatten();
                    // FIXME maybe we should check if running_time.is_none
                    // so as to display another message
                    gst::debug!(
                        SOCKET_CAT,
                        obj = self.element,
                        "Read {} bytes at {} (clock {})",
                        len,
                        running_time.display(),
                        time.display(),
                    );
                    running_time
                } else {
                    gst::debug!(SOCKET_CAT, obj = self.element, "Read {} bytes", len);
                    gst::ClockTime::NONE
                };

                let mut buffer = self.mapped_buffer.take().unwrap().into_buffer();
                {
                    let buffer = buffer.get_mut().unwrap();
                    if len < buffer.size() {
                        buffer.set_size(len);
                    }
                    buffer.set_dts(dts);
                }

                Ok((buffer, saddr))
            }
            Err(err) => {
                gst::debug!(SOCKET_CAT, obj = self.element, "Read error {:?}", err);

                Err(SocketError::Io(err))
            }
        }
    }
}

impl<T: SocketRead> Drop for Socket<T> {
    fn drop(&mut self) {
        if let Err(err) = self.buffer_pool.set_active(false) {
            gst::error!(
                SOCKET_CAT,
                obj = self.element,
                "Failed to unprepare socket: {}",
                err
            );
        }
    }
}

// Send/Sync struct for passing around a gio::Socket
// and getting the fd from it
//
// gio::Socket is not Send/Sync as it's generally unsafe
// to access it from multiple threads. Getting the underlying raw
// fd is safe though, as is receiving/sending from two different threads
#[derive(Debug, Clone)]
pub struct GioSocketWrapper {
    socket: gio::Socket,
}

unsafe impl Send for GioSocketWrapper {}
unsafe impl Sync for GioSocketWrapper {}

impl GioSocketWrapper {
    pub fn new(socket: &gio::Socket) -> Self {
        Self {
            socket: socket.clone(),
        }
    }

    pub fn as_socket(&self) -> &gio::Socket {
        &self.socket
    }

    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "linux",
        target_os = "android",
        target_os = "aix",
        target_os = "fuchsia",
        target_os = "haiku",
        target_env = "newlib"
    ))]
    pub fn set_tos(&self, qos_dscp: i32) -> rustix::io::Result<()> {
        use std::os::fd::AsFd;

        use gio::prelude::*;
        use rustix::net::sockopt;

        let tos = (qos_dscp & 0x3f) << 2;

        let socket = self.as_socket();

        sockopt::set_ip_tos(socket.as_fd(), tos as u8)?;

        if socket.family() == gio::SocketFamily::Ipv6 {
            sockopt::set_ipv6_tclass(socket.as_fd(), tos as u32)?;
        }

        Ok(())
    }

    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "linux",
        target_os = "android",
        target_os = "aix",
        target_os = "fuchsia",
        target_os = "haiku",
        target_env = "newlib"
    )))]
    pub fn set_tos(&self, _qos_dscp: i32) -> rustix::io::Result<()> {
        Ok(())
    }

    #[cfg(not(windows))]
    pub fn get<T: From<OwnedFd>>(&self) -> T {
        use std::os::fd::AsFd;

        let fd = self.socket.as_fd();
        let fd = fd.try_clone_to_owned().unwrap();
        T::from(fd)
    }

    #[cfg(windows)]
    pub fn get<T: From<OwnedSocket>>(&self) -> T {
        unsafe {
            use std::os::windows::io::{AsRawSocket, BorrowedSocket};

            let socket = self.socket.as_raw_socket();
            let socket = BorrowedSocket::borrow_raw(socket);
            let socket = socket.try_clone_to_owned().unwrap();
            T::from(socket)
        }
    }
}

pub fn wrap_socket(socket: &Async<UdpSocket>) -> Result<GioSocketWrapper, gst::ErrorMessage> {
    #[cfg(unix)]
    {
        use std::os::fd::AsFd;

        let fd = socket.as_fd();
        let fd = fd.try_clone_to_owned().unwrap();

        let gio_socket = gio::Socket::from_fd(fd);
        let gio_socket = gio_socket.map_err(|err| {
            gst::error_msg!(
                gst::ResourceError::OpenWrite,
                ["Failed to create wrapped GIO socket: {}", err]
            )
        })?;

        Ok(GioSocketWrapper::new(&gio_socket))
    }
    #[cfg(windows)]
    unsafe {
        use std::os::windows::io::{AsRawSocket, BorrowedSocket};

        let socket = socket.as_raw_socket();
        let socket = BorrowedSocket::borrow_raw(socket);
        let socket = socket.try_clone_to_owned().unwrap();

        let gio_socket = gio::Socket::from_socket(socket).map_err(|err| {
            gst::error_msg!(
                gst::ResourceError::OpenWrite,
                ["Failed to create wrapped GIO socket: {}", err]
            )
        })?;

        Ok(GioSocketWrapper::new(&gio_socket))
    }
}
