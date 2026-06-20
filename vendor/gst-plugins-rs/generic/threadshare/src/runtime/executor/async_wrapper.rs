// SPDX-License-Identifier: MIT OR Apache-2.0
// This is based on https://github.com/smol-rs/async-io
// with adaptations by:
//
// Copyright (C) 2021 François Laignel <fengalin@free.fr>

use futures::io::{AsyncRead, AsyncWrite};
use futures::stream::{self, Stream};
use futures::{future, pin_mut, ready};

use std::future::Future;
use std::io::{self, IoSlice, IoSliceMut, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

#[cfg(unix)]
use std::{
    os::unix::io::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd},
    os::unix::net::{SocketAddr as UnixSocketAddr, UnixDatagram, UnixListener, UnixStream},
    path::Path,
};

#[cfg(windows)]
use std::os::windows::io::{AsRawSocket, AsSocket, BorrowedSocket, OwnedSocket, RawSocket};

use rustix::io as rio;
use rustix::net as rn;
use rustix::net::addr::SocketAddrArg;

use crate::runtime::RUNTIME_CAT;

use super::scheduler;
use super::{Reactor, Readable, ReadableOwned, Registration, Source, Writable, WritableOwned};

/// Async adapter for I/O types.
///
/// This type puts an I/O handle into non-blocking mode, registers it in
/// [epoll]/[kqueue]/[event ports]/[wepoll], and then provides an async interface for it.
///
/// [epoll]: https://en.wikipedia.org/wiki/Epoll
/// [kqueue]: https://en.wikipedia.org/wiki/Kqueue
/// [event ports]: https://illumos.org/man/port_create
/// [wepoll]: https://github.com/piscisaureus/wepoll
///
/// # Caveats
///
/// The [`Async`] implementation is specific to the threadshare implementation.
/// Neither [`async-net`] nor [`async-process`] (on Unix) can be used.
///
/// [`async-net`]: https://github.com/smol-rs/async-net
/// [`async-process`]: https://github.com/smol-rs/async-process
///
/// ### Supported types
///
/// [`Async`] supports all networking types, as well as some OS-specific file descriptors like
/// [timerfd] and [inotify].
///
/// However, do not use [`Async`] with types like [`File`][`std::fs::File`],
/// [`Stdin`][`std::io::Stdin`], [`Stdout`][`std::io::Stdout`], or [`Stderr`][`std::io::Stderr`]
/// because all operating systems have issues with them when put in non-blocking mode.
///
/// [timerfd]: https://github.com/smol-rs/async-io/blob/master/examples/linux-timerfd.rs
/// [inotify]: https://github.com/smol-rs/async-io/blob/master/examples/linux-inotify.rs
///
/// ### Concurrent I/O
///
/// Note that [`&Async<T>`][`Async`] implements [`AsyncRead`] and [`AsyncWrite`] if `&T`
/// implements those traits, which means tasks can concurrently read and write using shared
/// references.
///
/// But there is a catch: only one task can read a time, and only one task can write at a time. It
/// is okay to have two tasks where one is reading and the other is writing at the same time, but
/// it is not okay to have two tasks reading at the same time or writing at the same time. If you
/// try to do that, conflicting tasks will just keep waking each other in turn, thus wasting CPU
/// time.
///
/// Besides [`AsyncRead`] and [`AsyncWrite`], this caveat also applies to
/// [`poll_readable()`][`Async::poll_readable()`] and
/// [`poll_writable()`][`Async::poll_writable()`].
///
/// However, any number of tasks can be concurrently calling other methods like
/// [`readable()`][`Async::readable()`] or [`read_with()`][`Async::read_with()`].
///
/// ### Closing
///
/// Closing the write side of [`Async`] with [`close()`][`futures::AsyncWriteExt::close()`]
/// simply flushes. If you want to shutdown a TCP or Unix socket, use
/// [`Shutdown`][`std::net::Shutdown`].
///
#[derive(Debug)]
pub struct Async<T: Send + 'static> {
    /// A source registered in the reactor.
    pub(super) source: Arc<Source>,

    /// The inner I/O handle.
    pub(super) io: Option<T>,

    // The [`ThrottlingHandle`] on the [`scheduler::Throttling`] on which this Async wrapper is registered.
    pub(super) throttling_sched_hdl: Option<scheduler::ThrottlingHandleWeak>,
}

impl<T: Send + 'static> Unpin for Async<T> {}

#[cfg(unix)]
impl<T: AsFd + Send + 'static> Async<T> {
    /// Creates an async I/O handle.
    ///
    /// This method will put the handle in non-blocking mode and register it in
    /// [epoll]/[kqueue]/[event ports]/[IOCP].
    ///
    /// On Unix systems, the handle must implement `AsFd`, while on Windows it must implement
    /// `AsSocket`.
    ///
    /// [epoll]: https://en.wikipedia.org/wiki/Epoll
    /// [kqueue]: https://en.wikipedia.org/wiki/Kqueue
    /// [event ports]: https://illumos.org/man/port_create
    /// [IOCP]: https://learn.microsoft.com/en-us/windows/win32/fileio/i-o-completion-ports
    pub fn new(io: T) -> io::Result<Async<T>> {
        // Put the file descriptor in non-blocking mode.
        set_nonblocking(io.as_fd())?;

        Self::new_nonblocking(io)
    }

    /// Creates an async I/O handle without setting it to non-blocking mode.
    ///
    /// This method will register the handle in [epoll]/[kqueue]/[event ports]/[IOCP].
    ///
    /// On Unix systems, the handle must implement `AsFd`, while on Windows it must implement
    /// `AsSocket`.
    ///
    /// [epoll]: https://en.wikipedia.org/wiki/Epoll
    /// [kqueue]: https://en.wikipedia.org/wiki/Kqueue
    /// [event ports]: https://illumos.org/man/port_create
    /// [IOCP]: https://learn.microsoft.com/en-us/windows/win32/fileio/i-o-completion-ports
    ///
    /// # Caveats
    ///
    /// The caller should ensure that the handle is set to non-blocking mode or that it is okay if
    /// it is not set. If not set to non-blocking mode, I/O operations may block the current thread
    /// and cause a deadlock in an asynchronous context.
    pub fn new_nonblocking(io: T) -> io::Result<Async<T>> {
        // SAFETY: It is impossible to drop the I/O source while it is registered through
        // this type.
        let registration = unsafe { Registration::new(io.as_fd()) };

        let source = Reactor::with_mut(|reactor| reactor.insert_io(registration))?;
        Ok(Async {
            source,
            io: Some(io),
            throttling_sched_hdl: scheduler::Throttling::current()
                .as_ref()
                .map(scheduler::ThrottlingHandle::downgrade),
        })
    }
}

#[cfg(unix)]
impl<T: AsRawFd + Send + 'static> AsRawFd for Async<T> {
    fn as_raw_fd(&self) -> RawFd {
        self.get_ref().as_raw_fd()
    }
}

#[cfg(unix)]
impl<T: AsFd + Send + 'static> AsFd for Async<T> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.get_ref().as_fd()
    }
}

#[cfg(unix)]
impl<T: AsFd + From<OwnedFd> + Send + 'static> TryFrom<OwnedFd> for Async<T> {
    type Error = io::Error;

    fn try_from(value: OwnedFd) -> Result<Self, Self::Error> {
        Async::new(value.into())
    }
}

#[cfg(unix)]
impl<T: Into<OwnedFd> + Send + 'static> TryFrom<Async<T>> for OwnedFd {
    type Error = io::Error;

    fn try_from(value: Async<T>) -> Result<Self, Self::Error> {
        value.into_inner().map(Into::into)
    }
}

#[cfg(windows)]
impl<T: AsSocket + Send + 'static> Async<T> {
    /// Creates an async I/O handle.
    ///
    /// This method will put the handle in non-blocking mode and register it in
    /// [epoll]/[kqueue]/[event ports]/[IOCP].
    ///
    /// On Unix systems, the handle must implement `AsFd`, while on Windows it must implement
    /// `AsSocket`.
    ///
    /// [epoll]: https://en.wikipedia.org/wiki/Epoll
    /// [kqueue]: https://en.wikipedia.org/wiki/Kqueue
    /// [event ports]: https://illumos.org/man/port_create
    /// [IOCP]: https://learn.microsoft.com/en-us/windows/win32/fileio/i-o-completion-ports
    pub fn new(io: T) -> io::Result<Async<T>> {
        // Put the socket in non-blocking mode.
        set_nonblocking(io.as_socket())?;

        Self::new_nonblocking(io)
    }

    /// Creates an async I/O handle without setting it to non-blocking mode.
    ///
    /// This method will register the handle in [epoll]/[kqueue]/[event ports]/[IOCP].
    ///
    /// On Unix systems, the handle must implement `AsFd`, while on Windows it must implement
    /// `AsSocket`.
    ///
    /// [epoll]: https://en.wikipedia.org/wiki/Epoll
    /// [kqueue]: https://en.wikipedia.org/wiki/Kqueue
    /// [event ports]: https://illumos.org/man/port_create
    /// [IOCP]: https://learn.microsoft.com/en-us/windows/win32/fileio/i-o-completion-ports
    ///
    /// # Caveats
    ///
    /// The caller should ensure that the handle is set to non-blocking mode or that it is okay if
    /// it is not set. If not set to non-blocking mode, I/O operations may block the current thread
    /// and cause a deadlock in an asynchronous context.
    pub fn new_nonblocking(io: T) -> io::Result<Async<T>> {
        // Create the registration.
        //
        // SAFETY: It is impossible to drop the I/O source while it is registered through
        // this type.
        let registration = unsafe { Registration::new(io.as_socket()) };

        let source = Reactor::with_mut(|reactor| reactor.insert_io(registration))?;
        Ok(Async {
            source,
            io: Some(io),
            throttling_sched_hdl: scheduler::Throttling::current()
                .as_ref()
                .map(scheduler::ThrottlingHandle::downgrade),
        })
    }
}

#[cfg(windows)]
impl<T: AsRawSocket + Send + 'static> AsRawSocket for Async<T> {
    fn as_raw_socket(&self) -> RawSocket {
        self.get_ref().as_raw_socket()
    }
}

#[cfg(windows)]
impl<T: AsSocket + Send + 'static> AsSocket for Async<T> {
    fn as_socket(&self) -> BorrowedSocket<'_> {
        self.get_ref().as_socket()
    }
}

#[cfg(windows)]
impl<T: AsSocket + From<OwnedSocket> + Send + 'static> TryFrom<OwnedSocket> for Async<T> {
    type Error = io::Error;

    fn try_from(value: OwnedSocket) -> Result<Self, Self::Error> {
        Async::new(value.into())
    }
}

#[cfg(windows)]
impl<T: Into<OwnedSocket> + Send + 'static> TryFrom<Async<T>> for OwnedSocket {
    type Error = io::Error;

    fn try_from(value: Async<T>) -> Result<Self, Self::Error> {
        value.into_inner().map(Into::into)
    }
}

impl<T: Send + 'static> Async<T> {
    /// Gets a reference to the inner I/O handle.
    pub fn get_ref(&self) -> &T {
        self.io.as_ref().unwrap()
    }

    /// Gets a mutable reference to the inner I/O handle.
    ///
    /// # Safety
    ///
    /// The underlying I/O source must not be dropped using this function.
    pub unsafe fn get_mut(&mut self) -> &mut T {
        self.io.as_mut().unwrap()
    }

    /// Unwraps the inner I/O handle.
    pub fn into_inner(mut self) -> io::Result<T> {
        let io = self.io.take().unwrap();
        Reactor::with_mut(|reactor| reactor.remove_io(&self.source))?;
        Ok(io)
    }

    /// Waits until the I/O handle is readable.
    ///
    /// This method completes when a read operation on this I/O handle wouldn't block.
    pub fn readable(&self) -> Readable<'_, T> {
        Source::readable(self)
    }

    /// Waits until the I/O handle is readable.
    ///
    /// This method completes when a read operation on this I/O handle wouldn't block.
    pub fn readable_owned(self: Arc<Self>) -> ReadableOwned<T> {
        Source::readable_owned(self)
    }

    /// Waits until the I/O handle is writable.
    ///
    /// This method completes when a write operation on this I/O handle wouldn't block.
    pub fn writable(&self) -> Writable<'_, T> {
        Source::writable(self)
    }

    /// Waits until the I/O handle is writable.
    ///
    /// This method completes when a write operation on this I/O handle wouldn't block.
    pub fn writable_owned(self: Arc<Self>) -> WritableOwned<T> {
        Source::writable_owned(self)
    }

    /// Polls the I/O handle for readability.
    ///
    /// When this method returns [`Poll::Ready`], that means the OS has delivered an event
    /// indicating readability since the last time this task has called the method and received
    /// [`Poll::Pending`].
    ///
    /// # Caveats
    ///
    /// Two different tasks should not call this method concurrently. Otherwise, conflicting tasks
    /// will just keep waking each other in turn, thus wasting CPU time.
    ///
    /// Note that the [`AsyncRead`] implementation for [`Async`] also uses this method.
    pub fn poll_readable(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.source.poll_readable(cx)
    }

    /// Polls the I/O handle for writability.
    ///
    /// When this method returns [`Poll::Ready`], that means the OS has delivered an event
    /// indicating writability since the last time this task has called the method and received
    /// [`Poll::Pending`].
    ///
    /// # Caveats
    ///
    /// Two different tasks should not call this method concurrently. Otherwise, conflicting tasks
    /// will just keep waking each other in turn, thus wasting CPU time.
    ///
    /// Note that the [`AsyncWrite`] implementation for [`Async`] also uses this method.
    pub fn poll_writable(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.source.poll_writable(cx)
    }

    /// Performs a read operation asynchronously.
    ///
    /// The I/O handle is registered in the reactor and put in non-blocking mode. This method
    /// invokes the `op` closure in a loop until it succeeds or returns an error other than
    /// [`io::ErrorKind::WouldBlock`]. In between iterations of the loop, it waits until the OS
    /// sends a notification that the I/O handle is readable.
    ///
    /// The closure receives a shared reference to the I/O handle.
    pub async fn read_with<R>(&self, op: impl FnMut(&T) -> io::Result<R>) -> io::Result<R> {
        let mut op = op;
        loop {
            match op(self.get_ref()) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return res,
            }
            optimistic(self.readable()).await?;
        }
    }

    /// Performs a read operation asynchronously.
    ///
    /// The I/O handle is registered in the reactor and put in non-blocking mode. This method
    /// invokes the `op` closure in a loop until it succeeds or returns an error other than
    /// [`io::ErrorKind::WouldBlock`]. In between iterations of the loop, it waits until the OS
    /// sends a notification that the I/O handle is readable.
    ///
    /// The closure receives a mutable reference to the I/O handle.
    ///
    /// # Safety
    ///
    /// In the closure, the underlying I/O source must not be dropped.
    pub async unsafe fn read_with_mut<R>(
        &mut self,
        op: impl FnMut(&mut T) -> io::Result<R>,
    ) -> io::Result<R> {
        let mut op = op;
        loop {
            match op(self.get_mut()) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return res,
            }
            optimistic(self.readable()).await?;
        }
    }

    /// Performs a write operation asynchronously.
    ///
    /// The I/O handle is registered in the reactor and put in non-blocking mode. This method
    /// invokes the `op` closure in a loop until it succeeds or returns an error other than
    /// [`io::ErrorKind::WouldBlock`]. In between iterations of the loop, it waits until the OS
    /// sends a notification that the I/O handle is writable.
    ///
    /// The closure receives a shared reference to the I/O handle.
    pub async fn write_with<R>(&self, op: impl FnMut(&T) -> io::Result<R>) -> io::Result<R> {
        let mut op = op;
        loop {
            match op(self.get_ref()) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return res,
            }
            optimistic(self.writable()).await?;
        }
    }

    /// Performs a write operation asynchronously.
    ///
    /// The I/O handle is registered in the reactor and put in non-blocking mode. This method
    /// invokes the `op` closure in a loop until it succeeds or returns an error other than
    /// [`io::ErrorKind::WouldBlock`]. In between iterations of the loop, it waits until the OS
    /// sends a notification that the I/O handle is writable.
    ///
    /// The closure receives a mutable reference to the I/O handle.
    ///
    /// # Safety
    ///
    /// The closure receives a mutable reference to the I/O handle. In the closure, the underlying
    /// I/O source must not be dropped.
    pub async unsafe fn write_with_mut<R>(
        &mut self,
        op: impl FnMut(&mut T) -> io::Result<R>,
    ) -> io::Result<R> {
        let mut op = op;
        loop {
            match op(self.get_mut()) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return res,
            }
            optimistic(self.writable()).await?;
        }
    }
}

impl<T: Send + 'static> AsRef<T> for Async<T> {
    fn as_ref(&self) -> &T {
        self.get_ref()
    }
}

impl<T: Send + 'static> Drop for Async<T> {
    fn drop(&mut self) {
        if let Some(io) = self.io.take() {
            if let Some(throttling_sched_hdl) = self.throttling_sched_hdl.take() {
                if let Some(sched) = throttling_sched_hdl.upgrade() {
                    let source = Arc::clone(&self.source);
                    sched.spawn_and_unpark(async move {
                        Reactor::with_mut(|reactor| {
                            if let Err(err) = reactor.remove_io(&source) {
                                gst::error!(
                                    RUNTIME_CAT,
                                    "Failed to remove fd {:?}: {err}",
                                    source.registration,
                                );
                            }
                        });
                        drop(io);
                    });
                }
            } else {
                Reactor::with_mut(|reactor| {
                    if let Err(err) = reactor.remove_io(&self.source) {
                        gst::error!(
                            RUNTIME_CAT,
                            "Failed to remove fd {:?}: {err}",
                            self.source.registration,
                        );
                    }
                });
            }
        }
    }
}

/// Types whose I/O trait implementations do not drop the underlying I/O source.
///
/// The resource contained inside of the [`Async`] cannot be invalidated. This invalidation can
/// happen if the inner resource (the [`TcpStream`], [`UnixListener`] or other `T`) is moved out
/// and dropped before the [`Async`]. Because of this, functions that grant mutable access to
/// the inner type are unsafe, as there is no way to guarantee that the source won't be dropped
/// and a dangling handle won't be left behind.
///
/// Unfortunately this extends to implementations of [`Read`] and [`Write`]. Since methods on those
/// traits take `&mut`, there is no guarantee that the implementor of those traits won't move the
/// source out while the method is being run.
///
/// This trait is an antidote to this predicament. By implementing this trait, the user pledges
/// that using any I/O traits won't destroy the source. This way, [`Async`] can implement the
/// `async` version of these I/O traits, like [`AsyncRead`] and [`AsyncWrite`].
///
/// # Safety
///
/// Any I/O trait implementations for this type must not drop the underlying I/O source. Traits
/// affected by this trait include [`Read`], [`Write`], [`Seek`] and [`BufRead`].
///
/// This trait is implemented by default on top of `libstd` types. In addition, it is implemented
/// for immutable reference types, as it is impossible to invalidate any outstanding references
/// while holding an immutable reference, even with interior mutability. As Rust's current pinning
/// system relies on similar guarantees, I believe that this approach is robust.
///
/// [`BufRead`]: https://doc.rust-lang.org/std/io/trait.BufRead.html
/// [`Read`]: https://doc.rust-lang.org/std/io/trait.Read.html
/// [`Seek`]: https://doc.rust-lang.org/std/io/trait.Seek.html
/// [`Write`]: https://doc.rust-lang.org/std/io/trait.Write.html
///
/// [`AsyncRead`]: https://docs.rs/futures-io/latest/futures_io/trait.AsyncRead.html
/// [`AsyncWrite`]: https://docs.rs/futures-io/latest/futures_io/trait.AsyncWrite.html
pub unsafe trait IoSafe {}

/// Reference types can't be mutated.
///
/// The worst thing that can happen is that external state is used to change what kind of pointer
/// `as_fd()` returns. For instance:
///
/// ```
/// # #[cfg(unix)] {
/// use std::cell::Cell;
/// use std::net::TcpStream;
/// use std::os::unix::io::{AsFd, BorrowedFd};
///
/// struct Bar {
///     flag: Cell<bool>,
///     a: TcpStream,
///     b: TcpStream
/// }
///
/// impl AsFd for Bar {
///     fn as_fd(&self) -> BorrowedFd<'_> {
///         if self.flag.replace(!self.flag.get()) {
///             self.a.as_fd()
///         } else {
///             self.b.as_fd()
///         }
///     }
/// }
/// # }
/// ```
///
/// We solve this problem by only calling `as_fd()` once to get the original source. Implementations
/// like this are considered buggy (but not unsound) and are thus not really supported by `async-io`.
unsafe impl<T: ?Sized> IoSafe for &T {}

// Can be implemented on top of libstd types.
unsafe impl IoSafe for std::fs::File {}
unsafe impl IoSafe for std::io::Stderr {}
unsafe impl IoSafe for std::io::Stdin {}
unsafe impl IoSafe for std::io::Stdout {}
unsafe impl IoSafe for std::io::StderrLock<'_> {}
unsafe impl IoSafe for std::io::StdinLock<'_> {}
unsafe impl IoSafe for std::io::StdoutLock<'_> {}
unsafe impl IoSafe for std::net::TcpStream {}

#[cfg(unix)]
unsafe impl IoSafe for std::os::unix::net::UnixStream {}

unsafe impl<T: IoSafe + Read> IoSafe for std::io::BufReader<T> {}
unsafe impl<T: IoSafe + Write> IoSafe for std::io::BufWriter<T> {}
unsafe impl<T: IoSafe + Write> IoSafe for std::io::LineWriter<T> {}
unsafe impl<T: IoSafe + ?Sized> IoSafe for &mut T {}
unsafe impl<T: IoSafe + ?Sized> IoSafe for Box<T> {}
unsafe impl<T: Clone + IoSafe> IoSafe for std::borrow::Cow<'_, T> {}

impl<T: IoSafe + Read + Send + 'static> AsyncRead for Async<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        loop {
            match unsafe { (*self).get_mut() }.read(buf) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }
            ready!(self.poll_readable(cx))?;
        }
    }

    fn poll_read_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        loop {
            match unsafe { (*self).get_mut() }.read_vectored(bufs) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }
            ready!(self.poll_readable(cx))?;
        }
    }
}

// Since this is through a reference, we can't mutate the inner I/O source.
// Therefore this is safe!
impl<T: Send + 'static> AsyncRead for &Async<T>
where
    for<'a> &'a T: Read,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        loop {
            match (*self).get_ref().read(buf) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }
            ready!(self.poll_readable(cx))?;
        }
    }

    fn poll_read_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        loop {
            match (*self).get_ref().read_vectored(bufs) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }
            ready!(self.poll_readable(cx))?;
        }
    }
}

impl<T: IoSafe + Write + Send + 'static> AsyncWrite for Async<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        loop {
            match unsafe { (*self).get_mut() }.write(buf) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }
            ready!(self.poll_writable(cx))?;
        }
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        loop {
            match unsafe { (*self).get_mut() }.write_vectored(bufs) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }
            ready!(self.poll_writable(cx))?;
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        loop {
            match unsafe { (*self).get_mut() }.flush() {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }
            ready!(self.poll_writable(cx))?;
        }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_flush(cx)
    }
}

impl<T: Send + 'static> AsyncWrite for &Async<T>
where
    for<'a> &'a T: Write,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        loop {
            match (*self).get_ref().write(buf) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }
            ready!(self.poll_writable(cx))?;
        }
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        loop {
            match (*self).get_ref().write_vectored(bufs) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }
            ready!(self.poll_writable(cx))?;
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        loop {
            match (*self).get_ref().flush() {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }
            ready!(self.poll_writable(cx))?;
        }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_flush(cx)
    }
}

impl Async<TcpListener> {
    /// Creates a TCP listener bound to the specified address.
    ///
    /// Binding with port number 0 will request an available port from the OS.
    pub fn bind<A: Into<SocketAddr>>(addr: A) -> io::Result<Async<TcpListener>> {
        let addr = addr.into();
        Async::new(TcpListener::bind(addr)?)
    }

    /// Accepts a new incoming TCP connection.
    ///
    /// When a connection is established, it will be returned as a TCP stream together with its
    /// remote address.
    pub async fn accept(&self) -> io::Result<(Async<TcpStream>, SocketAddr)> {
        let (stream, addr) = self.read_with(|io| io.accept()).await?;
        Ok((Async::new(stream)?, addr))
    }

    /// Returns a stream of incoming TCP connections.
    ///
    /// The stream is infinite, i.e. it never stops with a [`None`].
    pub fn incoming(&self) -> impl Stream<Item = io::Result<Async<TcpStream>>> + Send + '_ {
        stream::unfold(self, |listener| async move {
            let res = listener.accept().await.map(|(stream, _)| stream);
            Some((res, listener))
        })
    }
}

impl TryFrom<std::net::TcpListener> for Async<std::net::TcpListener> {
    type Error = io::Error;

    fn try_from(listener: std::net::TcpListener) -> io::Result<Self> {
        Async::new(listener)
    }
}

impl Async<TcpStream> {
    /// Creates a TCP connection to the specified address.
    pub async fn connect<A: Into<SocketAddr>>(addr: A) -> io::Result<Async<TcpStream>> {
        // Figure out how to handle this address.
        let addr = addr.into();
        let (domain, sock_addr) = match addr {
            SocketAddr::V4(v4) => (rn::AddressFamily::INET, v4.as_any()),
            SocketAddr::V6(v6) => (rn::AddressFamily::INET6, v6.as_any()),
        };

        // Begin async connect.
        let socket = connect(sock_addr, domain, Some(rn::ipproto::TCP))?;
        // Use new_nonblocking because connect already sets socket to non-blocking mode.
        let stream = Async::new_nonblocking(TcpStream::from(socket))?;

        // The stream becomes writable when connected.
        stream.writable().await?;

        // Check if there was an error while connecting.
        match stream.get_ref().take_error()? {
            None => Ok(stream),
            Some(err) => Err(err),
        }
    }

    /// Reads data from the stream without removing it from the buffer.
    ///
    /// Returns the number of bytes read. Successive calls of this method read the same data.
    pub async fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_with(|io| io.peek(buf)).await
    }
}

impl TryFrom<std::net::TcpStream> for Async<std::net::TcpStream> {
    type Error = io::Error;

    fn try_from(stream: std::net::TcpStream) -> io::Result<Self> {
        Async::new(stream)
    }
}

impl Async<UdpSocket> {
    /// Creates a UDP socket bound to the specified address.
    ///
    /// Binding with port number 0 will request an available port from the OS.
    pub fn bind<A: Into<SocketAddr>>(addr: A) -> io::Result<Async<UdpSocket>> {
        let addr = addr.into();
        Async::new(UdpSocket::bind(addr)?)
    }

    /// Receives a single datagram message.
    ///
    /// Returns the number of bytes read and the address the message came from.
    ///
    /// This method must be called with a valid byte slice of sufficient size to hold the message.
    /// If the message is too long to fit, excess bytes may get discarded.
    pub async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.read_with(|io| io.recv_from(buf)).await
    }

    /// Receives a single datagram message without removing it from the queue.
    ///
    /// Returns the number of bytes read and the address the message came from.
    ///
    /// This method must be called with a valid byte slice of sufficient size to hold the message.
    /// If the message is too long to fit, excess bytes may get discarded.
    pub async fn peek_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.read_with(|io| io.peek_from(buf)).await
    }

    /// Sends data to the specified address.
    ///
    /// Returns the number of bytes written.
    pub async fn send_to<A: Into<SocketAddr>>(&self, buf: &[u8], addr: A) -> io::Result<usize> {
        let addr = addr.into();
        self.write_with(|io| io.send_to(buf, addr)).await
    }

    /// Receives a single datagram message from the connected peer.
    ///
    /// Returns the number of bytes read.
    ///
    /// This method must be called with a valid byte slice of sufficient size to hold the message.
    /// If the message is too long to fit, excess bytes may get discarded.
    ///
    /// The [`connect`][`UdpSocket::connect()`] method connects this socket to a remote address.
    /// This method will fail if the socket is not connected.
    pub async fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_with(|io| io.recv(buf)).await
    }

    /// Receives a single datagram message from the connected peer without removing it from the
    /// queue.
    ///
    /// Returns the number of bytes read and the address the message came from.
    ///
    /// This method must be called with a valid byte slice of sufficient size to hold the message.
    /// If the message is too long to fit, excess bytes may get discarded.
    ///
    /// The [`connect`][`UdpSocket::connect()`] method connects this socket to a remote address.
    /// This method will fail if the socket is not connected.
    pub async fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_with(|io| io.peek(buf)).await
    }

    /// Sends data to the connected peer.
    ///
    /// Returns the number of bytes written.
    ///
    /// The [`connect`][`UdpSocket::connect()`] method connects this socket to a remote address.
    /// This method will fail if the socket is not connected.
    pub async fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.write_with(|io| io.send(buf)).await
    }
}

impl TryFrom<std::net::UdpSocket> for Async<std::net::UdpSocket> {
    type Error = io::Error;

    fn try_from(socket: std::net::UdpSocket) -> io::Result<Self> {
        Async::new(socket)
    }
}

impl TryFrom<socket2::Socket> for Async<std::net::UdpSocket> {
    type Error = io::Error;

    fn try_from(socket: socket2::Socket) -> io::Result<Self> {
        Async::new(std::net::UdpSocket::from(socket))
    }
}

#[cfg(unix)]
impl Async<UnixListener> {
    /// Creates a UDS listener bound to the specified path.
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<Async<UnixListener>> {
        let path = path.as_ref().to_owned();
        Async::new(UnixListener::bind(path)?)
    }

    /// Accepts a new incoming UDS stream connection.
    pub async fn accept(&self) -> io::Result<(Async<UnixStream>, UnixSocketAddr)> {
        let (stream, addr) = self.read_with(|io| io.accept()).await?;
        Ok((Async::new(stream)?, addr))
    }

    /// Returns a stream of incoming UDS connections.
    ///
    /// The stream is infinite, i.e. it never stops with a [`None`] item.
    pub fn incoming(&self) -> impl Stream<Item = io::Result<Async<UnixStream>>> + Send + '_ {
        stream::unfold(self, |listener| async move {
            let res = listener.accept().await.map(|(stream, _)| stream);
            Some((res, listener))
        })
    }
}

#[cfg(unix)]
impl TryFrom<std::os::unix::net::UnixListener> for Async<std::os::unix::net::UnixListener> {
    type Error = io::Error;

    fn try_from(listener: std::os::unix::net::UnixListener) -> io::Result<Self> {
        Async::new(listener)
    }
}

#[cfg(unix)]
impl Async<UnixStream> {
    /// Creates a UDS stream connected to the specified path.
    pub async fn connect<P: AsRef<Path>>(path: P) -> io::Result<Async<UnixStream>> {
        let address = convert_path_to_socket_address(path.as_ref())?;

        // Begin async connect.
        let socket = connect(address.into(), rn::AddressFamily::UNIX, None)?;
        // Use new_nonblocking because connect already sets socket to non-blocking mode.
        let stream = Async::new_nonblocking(UnixStream::from(socket))?;

        // The stream becomes writable when connected.
        stream.writable().await?;

        // On Linux, it appears the socket may become writable even when connecting fails, so we
        // must do an extra check here and see if the peer address is retrievable.
        stream.get_ref().peer_addr()?;
        Ok(stream)
    }

    /// Creates an unnamed pair of connected UDS stream sockets.
    pub fn pair() -> io::Result<(Async<UnixStream>, Async<UnixStream>)> {
        let (stream1, stream2) = UnixStream::pair()?;
        Ok((Async::new(stream1)?, Async::new(stream2)?))
    }
}

#[cfg(unix)]
impl TryFrom<std::os::unix::net::UnixStream> for Async<std::os::unix::net::UnixStream> {
    type Error = io::Error;

    fn try_from(stream: std::os::unix::net::UnixStream) -> io::Result<Self> {
        Async::new(stream)
    }
}

#[cfg(unix)]
impl Async<UnixDatagram> {
    /// Creates a UDS datagram socket bound to the specified path.
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<Async<UnixDatagram>> {
        let path = path.as_ref().to_owned();
        Async::new(UnixDatagram::bind(path)?)
    }

    /// Creates a UDS datagram socket not bound to any address.
    pub fn unbound() -> io::Result<Async<UnixDatagram>> {
        Async::new(UnixDatagram::unbound()?)
    }

    /// Creates an unnamed pair of connected Unix datagram sockets.
    pub fn pair() -> io::Result<(Async<UnixDatagram>, Async<UnixDatagram>)> {
        let (socket1, socket2) = UnixDatagram::pair()?;
        Ok((Async::new(socket1)?, Async::new(socket2)?))
    }

    /// Receives data from the socket.
    ///
    /// Returns the number of bytes read and the address the message came from.
    pub async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, UnixSocketAddr)> {
        self.read_with(|io| io.recv_from(buf)).await
    }

    /// Sends data to the specified address.
    ///
    /// Returns the number of bytes written.
    pub async fn send_to<P: AsRef<Path>>(&self, buf: &[u8], path: P) -> io::Result<usize> {
        self.write_with(|io| io.send_to(buf, &path)).await
    }

    /// Receives data from the connected peer.
    ///
    /// Returns the number of bytes read and the address the message came from.
    ///
    /// The [`connect`][`UnixDatagram::connect()`] method connects this socket to a remote address.
    /// This method will fail if the socket is not connected.
    pub async fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_with(|io| io.recv(buf)).await
    }

    /// Sends data to the connected peer.
    ///
    /// Returns the number of bytes written.
    ///
    /// The [`connect`][`UnixDatagram::connect()`] method connects this socket to a remote address.
    /// This method will fail if the socket is not connected.
    pub async fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.write_with(|io| io.send(buf)).await
    }
}

#[cfg(unix)]
impl TryFrom<std::os::unix::net::UnixDatagram> for Async<std::os::unix::net::UnixDatagram> {
    type Error = io::Error;

    fn try_from(socket: std::os::unix::net::UnixDatagram) -> io::Result<Self> {
        Async::new(socket)
    }
}

/// Polls a future once, waits for a wakeup, and then optimistically assumes the future is ready.
async fn optimistic(fut: impl Future<Output = io::Result<()>>) -> io::Result<()> {
    let mut polled = false;
    pin_mut!(fut);

    future::poll_fn(|cx| {
        if !polled {
            polled = true;
            fut.as_mut().poll(cx)
        } else {
            Poll::Ready(Ok(()))
        }
    })
    .await
}

fn connect(
    addr: rn::SocketAddrAny,
    domain: rn::AddressFamily,
    protocol: Option<rn::Protocol>,
) -> io::Result<rustix::fd::OwnedFd> {
    #[cfg(windows)]
    use rustix::fd::AsFd;

    setup_networking();

    #[cfg(any(
        target_os = "android",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "illumos",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    let socket = rn::socket_with(
        domain,
        rn::SocketType::STREAM,
        rn::SocketFlags::CLOEXEC | rn::SocketFlags::NONBLOCK,
        protocol,
    )?;

    #[cfg(not(any(
        target_os = "android",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "illumos",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd"
    )))]
    let socket = {
        #[cfg(not(any(
            target_os = "aix",
            target_vendor = "apple",
            target_os = "espidf",
            windows,
        )))]
        let flags = rn::SocketFlags::CLOEXEC;
        #[cfg(any(
            target_os = "aix",
            target_vendor = "apple",
            target_os = "espidf",
            windows,
        ))]
        let flags = rn::SocketFlags::empty();

        // Create the socket.
        let socket = rn::socket_with(domain, rn::SocketType::STREAM, flags, protocol)?;

        // Set cloexec if necessary.
        #[cfg(any(target_os = "aix", target_vendor = "apple"))]
        rio::fcntl_setfd(&socket, rio::fcntl_getfd(&socket)? | rio::FdFlags::CLOEXEC)?;

        // Set non-blocking mode.
        set_nonblocking(socket.as_fd())?;

        socket
    };

    // Set nosigpipe if necessary.
    #[cfg(any(
        target_vendor = "apple",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "dragonfly",
    ))]
    rn::sockopt::set_socket_nosigpipe(&socket, true)?;

    // Set the handle information to HANDLE_FLAG_INHERIT.
    #[cfg(windows)]
    unsafe {
        if windows_sys::Win32::Foundation::SetHandleInformation(
            socket.as_raw_socket() as _,
            windows_sys::Win32::Foundation::HANDLE_FLAG_INHERIT,
            windows_sys::Win32::Foundation::HANDLE_FLAG_INHERIT,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
    }

    #[allow(unreachable_patterns)]
    match rn::connect(&socket, &addr) {
        Ok(_) => {}
        #[cfg(unix)]
        Err(rio::Errno::INPROGRESS) => {}
        Err(rio::Errno::AGAIN) | Err(rio::Errno::WOULDBLOCK) => {}
        Err(err) => return Err(err.into()),
    }
    Ok(socket)
}

#[inline]
fn setup_networking() {
    #[cfg(windows)]
    {
        // On Windows, we need to call WSAStartup before calling any networking code.
        // Make sure to call it at least once.
        static INIT: std::sync::Once = std::sync::Once::new();

        INIT.call_once(|| {
            let _ = rustix::net::wsa_startup();
        });
    }
}

#[inline]
fn set_nonblocking(
    #[cfg(unix)] fd: BorrowedFd<'_>,
    #[cfg(windows)] fd: BorrowedSocket<'_>,
) -> io::Result<()> {
    cfg_if::cfg_if! {
        // ioctl(FIONBIO) sets the flag atomically, but we use this only on Linux
        // for now, as with the standard library, because it seems to behave
        // differently depending on the platform.
        // https://github.com/rust-lang/rust/commit/efeb42be2837842d1beb47b51bb693c7474aba3d
        // https://github.com/libuv/libuv/blob/e9d91fccfc3e5ff772d5da90e1c4a24061198ca0/src/unix/poll.c#L78-L80
        // https://github.com/tokio-rs/mio/commit/0db49f6d5caf54b12176821363d154384357e70a
        if #[cfg(any(windows, target_os = "linux"))] {
            rustix::io::ioctl_fionbio(fd, true)?;
        } else {
            let previous = rustix::fs::fcntl_getfl(fd)?;
            let new = previous | rustix::fs::OFlags::NONBLOCK;
            if new != previous {
                rustix::fs::fcntl_setfl(fd, new)?;
            }
        }
    }

    Ok(())
}

/// Converts a `Path` to its socket address representation.
///
/// This function is abstract socket-aware.
#[cfg(unix)]
#[inline]
fn convert_path_to_socket_address(path: &Path) -> io::Result<rn::SocketAddrUnix> {
    // SocketAddrUnix::new() will throw EINVAL when a path with a zero in it is passed in.
    // However, some users expect to be able to pass in paths to abstract sockets, which
    // triggers this error as it has a zero in it. Therefore, if a path starts with a zero,
    // make it an abstract socket.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let address = {
        use std::os::unix::ffi::OsStrExt;

        let path = path.as_os_str();
        match path.as_bytes().first() {
            Some(0) => rn::SocketAddrUnix::new_abstract_name(path.as_bytes().get(1..).unwrap())?,
            _ => rn::SocketAddrUnix::new(path)?,
        }
    };

    // Only Linux and Android support abstract sockets.
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    let address = rn::SocketAddrUnix::new(path)?;

    Ok(address)
}
