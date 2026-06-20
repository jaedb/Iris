// GStreamer RTSP Source 2
//
// Copyright (C) 2023 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::fmt;
use std::marker::Unpin;

use futures::{Sink, Stream};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::body::Body;
use rtsp_types::Message;

#[derive(Debug)]
pub enum ReadError {
    Io(std::io::Error),
    TooBig,
    ParseError,
}

impl std::error::Error for ReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ReadError::Io(ref io) => Some(io),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ReadError {
    fn from(err: std::io::Error) -> Self {
        ReadError::Io(err)
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadError::Io(ref io) => fmt::Display::fmt(io, fmt),
            ReadError::TooBig => write!(fmt, "Too big message"),
            ReadError::ParseError => write!(fmt, "Parse error"),
        }
    }
}

pub(crate) fn async_read<R: AsyncRead + Unpin + Send>(
    read: R,
    max_size: usize,
) -> impl Stream<Item = Result<Message<Body>, ReadError>> + Send {
    const INITIAL_BUF_SIZE: usize = 8192;
    const MAX_EMPTY_BUF_SIZE: usize = 8 * INITIAL_BUF_SIZE;

    struct State<R> {
        read: R,
        buf: Vec<u8>,
        write_pos: usize,
        // If > 0 then we first need to try parsing as there might be more messages
        read_pos: usize,
    }

    let state = State {
        read,
        buf: vec![0; INITIAL_BUF_SIZE],
        write_pos: 0,
        read_pos: 0,
    };

    futures::stream::unfold(Some(state), move |mut state| async move {
        let State {
            mut read,
            mut buf,
            mut write_pos,
            mut read_pos,
        } = state.take()?;

        let read_one = async {
            loop {
                assert!(read_pos <= write_pos);

                // First check if there are more messages left in the buffer
                if read_pos != write_pos {
                    assert_ne!(read_pos, write_pos);
                    match Message::<Body>::parse(&buf[read_pos..write_pos]) {
                        Ok((msg, consumed)) => {
                            read_pos += consumed;

                            // Need to first read more data on the next call
                            if read_pos == write_pos {
                                read_pos = 0;
                                write_pos = 0;
                            }

                            gst::trace!(super::imp::CAT, "Read message {:?}", msg);

                            return Ok((Some(msg), write_pos, read_pos));
                        }
                        Err(rtsp_types::ParseError::Error) => return Err(ReadError::ParseError),
                        Err(rtsp_types::ParseError::Incomplete(_)) => {
                            if read_pos > 0 {
                                // Not a complete message left, copy to the beginning and read more
                                // data
                                buf.copy_within(read_pos..write_pos, 0);
                                write_pos -= read_pos;
                                read_pos = 0;

                                // Shrink the buffer again if possible and needed
                                if buf.len() > MAX_EMPTY_BUF_SIZE && write_pos < MAX_EMPTY_BUF_SIZE
                                {
                                    buf.resize(MAX_EMPTY_BUF_SIZE, 0);
                                }
                            }
                        }
                    }
                }

                assert_eq!(read_pos, 0);

                if write_pos == max_size {
                    gst::error!(super::imp::CAT, "Message bigger than maximum {}", max_size);
                    return Err(ReadError::TooBig);
                }

                // Grow the buffer if needed up to the maximum
                let new_size = std::cmp::min(
                    max_size,
                    buf.len().checked_next_power_of_two().unwrap_or(usize::MAX),
                );
                if buf.len() < new_size {
                    buf.resize(new_size, 0);
                }

                let b = read.read(&mut buf[write_pos..]).await?;

                if b == 0 {
                    gst::debug!(super::imp::CAT, "Connection closed");
                    return Ok((None, write_pos, read_pos));
                }
                write_pos += b;

                // Try parsing on the next iteration
            }
        };

        match read_one.await {
            Ok((Some(msg), write_pos, read_pos)) => Some((
                Ok(msg),
                Some(State {
                    read,
                    buf,
                    write_pos,
                    read_pos,
                }),
            )),
            Ok((None, _, _)) => None,
            Err(err) => {
                gst::error!(super::imp::CAT, "Read error {}", err);
                Some((Err(err), None))
            }
        }
    })
}

pub(crate) fn async_write<W: AsyncWrite + Unpin + Send>(
    write: W,
) -> impl Sink<Message<Body>, Error = std::io::Error> + Send {
    struct State<W> {
        write: W,
        buffer: Vec<u8>,
    }

    let state = State {
        write,
        buffer: Vec::with_capacity(8192),
    };

    futures::sink::unfold(state, |mut state, item: Message<Body>| {
        async move {
            gst::trace!(super::imp::CAT, "Writing message {:?}", item);

            // TODO: Write data messages more efficiently by writing header / body separately
            state.buffer.clear();
            item.write(&mut state.buffer).expect("can't fail");

            match state.write.write_all(&state.buffer).await {
                Ok(_) => {
                    gst::trace!(super::imp::CAT, "Finished writing queued message");
                    Ok(state)
                }
                Err(err) => {
                    gst::error!(super::imp::CAT, "Write error {}", err);
                    Err(err)
                }
            }
        }
    })
}
