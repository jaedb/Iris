// Copyright (C) 2024, Asymptotic Inc.
//      Author: Sanchayan Maity <sanchayan@asymptotic.io>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use bytes::BufMut;
use gst::glib;
use quinn::VarInt;
use std::{net::SocketAddr, path::PathBuf};

pub(crate) static DEFAULT_SERVER_NAME: &str = "localhost";
pub(crate) static DEFAULT_ADDR: &str = "127.0.0.1";
pub(crate) static DEFAULT_PORT: u16 = 5000;
pub(crate) static DEFAULT_BIND_ADDR: &str = "0.0.0.0";
pub(crate) static DEFAULT_BIND_PORT: u16 = 0;
pub(crate) static DEFAULT_INITIAL_MTU: u16 = 1200;
pub(crate) static DEFAULT_MINIMUM_MTU: u16 = 1200;
pub(crate) static DEFAULT_UPPER_BOUND_MTU: u16 = 1452;
pub(crate) static DEFAULT_MAX_UPPER_BOUND_MTU: u16 = 65527;
pub(crate) static DEFAULT_UDP_PAYLOAD_SIZE: u16 = 1452;
pub(crate) static DEFAULT_MIN_UDP_PAYLOAD_SIZE: u16 = 1200;
pub(crate) static DEFAULT_MAX_UDP_PAYLOAD_SIZE: u16 = 65527;
pub(crate) static DEFAULT_DROP_BUFFER_FOR_DATAGRAM: bool = false;
pub(crate) static DEFAULT_MAX_CONCURRENT_BI_STREAMS: VarInt = VarInt::from_u32(1);
pub(crate) static DEFAULT_MAX_CONCURRENT_UNI_STREAMS: VarInt = VarInt::from_u32(32);
pub(crate) static DEFAULT_USE_DATAGRAM: bool = false;

/*
 * For QUIC transport parameters
 * <https://datatracker.ietf.org/doc/html/rfc9000#section-7.4>
 *
 * A HTTP client might specify "http/1.1" and/or "h2" or "h3".
 * Other well-known values are listed in the at IANA registry at
 * <https://www.iana.org/assignments/tls-extensiontype-values/tls-extensiontype-values.xhtml#alpn-protocol-ids>.
 */
pub(crate) const DEFAULT_ALPN: &str = "gst-quinn";
pub(crate) const DEFAULT_TIMEOUT: u32 = 15;
pub(crate) const DEFAULT_SECURE_CONNECTION: bool = true;
pub(crate) const HTTP3_ALPN: &str = "h3";

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstQuinnQuicRole")]
pub enum QuinnQuicRole {
    #[enum_value(name = "Server: Act as QUIC server.", nick = "server")]
    Server,

    #[enum_value(name = "Client: Act as QUIC client.", nick = "client")]
    Client,
}

#[derive(Clone, Debug)]
pub struct QuinnQuicEndpointConfig {
    pub server_addr: SocketAddr,
    pub server_name: String,
    pub client_addr: Option<SocketAddr>,
    pub secure_conn: bool,
    pub alpns: Vec<String>,
    pub certificate_file: Option<PathBuf>,
    pub private_key_file: Option<PathBuf>,
    pub keep_alive_interval: u64,
    pub transport_config: QuinnQuicTransportConfig,
    pub with_client_auth: bool,
    pub webtransport: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuinnQuicTransportConfig {
    pub datagram_receive_buffer_size: usize,
    pub datagram_send_buffer_size: usize,
    pub initial_mtu: u16,
    pub max_udp_payload_size: u16,
    pub min_mtu: u16,
    pub upper_bound_mtu: u16,
    pub max_concurrent_bidi_streams: VarInt,
    pub max_concurrent_uni_streams: VarInt,
    pub send_window: u64,
    pub stream_receive_window: VarInt,
    pub receive_window: VarInt,
}

impl Default for QuinnQuicTransportConfig {
    fn default() -> Self {
        // Copied from Quinn::TransportConfig defaults
        const EXPECTED_RTT: u32 = 100; // ms
        const MAX_STREAM_BANDWIDTH: u32 = 12500 * 1000; // bytes/s
                                                        // Window size needed to avoid pipeline
                                                        // stalls
        const STREAM_RWND: u32 = MAX_STREAM_BANDWIDTH / 1000 * EXPECTED_RTT;

        Self {
            datagram_receive_buffer_size: STREAM_RWND as usize,
            datagram_send_buffer_size: 1024 * 1024,
            initial_mtu: DEFAULT_INITIAL_MTU,
            max_udp_payload_size: DEFAULT_MAX_UDP_PAYLOAD_SIZE,
            min_mtu: DEFAULT_MINIMUM_MTU,
            upper_bound_mtu: DEFAULT_UPPER_BOUND_MTU,
            max_concurrent_bidi_streams: DEFAULT_MAX_CONCURRENT_BI_STREAMS,
            max_concurrent_uni_streams: DEFAULT_MAX_CONCURRENT_UNI_STREAMS,
            send_window: (8 * STREAM_RWND).into(),
            stream_receive_window: STREAM_RWND.into(),
            receive_window: VarInt::MAX,
        }
    }
}

// Taken from quinn-rs.
pub fn get_varint_size(val: u64) -> usize {
    if val < 2u64.pow(6) {
        1
    } else if val < 2u64.pow(14) {
        2
    } else if val < 2u64.pow(30) {
        4
    } else if val < 2u64.pow(62) {
        8
    } else {
        unreachable!("malformed VarInt");
    }
}

// Adapted from quinn-rs.
pub fn get_varint(data: &[u8]) -> Option<(u64 /* VarInt value */, usize /* VarInt length */)> {
    if data.is_empty() {
        return None;
    }

    let data_length = data.len();
    let tag = data[0] >> 6;

    match tag {
        0b00 => {
            let mut slice = [0; 1];
            slice.clone_from_slice(&data[..1]);
            slice[0] &= 0b0011_1111;

            Some((u64::from(slice[0]), 1))
        }
        0b01 => {
            if data_length < 2 {
                return None;
            }

            let mut buf = [0; 2];
            buf.clone_from_slice(&data[..2]);
            buf[0] &= 0b0011_1111;

            Some((
                u64::from(u16::from_be_bytes(buf[..2].try_into().unwrap())),
                2,
            ))
        }
        0b10 => {
            if data_length < 4 {
                return None;
            }

            let mut buf = [0; 4];
            buf.clone_from_slice(&data[..4]);
            buf[0] &= 0b0011_1111;

            Some((
                u64::from(u32::from_be_bytes(buf[..4].try_into().unwrap())),
                4,
            ))
        }
        0b11 => {
            if data_length < 8 {
                return None;
            }

            let mut buf = [0; 8];
            buf.clone_from_slice(&data[..8]);
            buf[0] &= 0b0011_1111;

            Some((u64::from_be_bytes(buf[..8].try_into().unwrap()), 8))
        }
        _ => unreachable!(),
    }
}

// Taken from quinn-rs.
pub fn set_varint<B: BufMut>(data: &mut B, val: u64) {
    if val < 2u64.pow(6) {
        data.put_u8(val as u8);
    } else if val < 2u64.pow(14) {
        data.put_u16((0b01 << 14) | val as u16);
    } else if val < 2u64.pow(30) {
        data.put_u32((0b10 << 30) | val as u32);
    } else if val < 2u64.pow(62) {
        data.put_u64((0b11 << 62) | val);
    } else {
        unreachable!("malformed varint");
    }
}
