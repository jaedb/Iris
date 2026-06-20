// GStreamer RTSP Source 2
//
// Copyright (C) 2023-2024 Nirbheek Chauhan <nirbheek centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0
//
// https://www.rfc-editor.org/rfc/rfc2326.html

use super::imp::{RtspError, RtspProtocol};
use rtsp_types::headers::{transport::RtpTransport, RtpLowerTransport};
use std::{convert::TryFrom, net::IpAddr};
use tokio::net::UdpSocket;

#[derive(Debug)]
pub enum RtspTransportInfo {
    Tcp {
        channels: (u8, Option<u8>),
    },
    Udp {
        source: Option<String>,
        server_port: Option<(u16, Option<u16>)>,
        client_port: Option<(u16, Option<u16>)>,
        sockets: Option<(UdpSocket, Option<UdpSocket>)>,
    },
    UdpMulticast {
        dest: IpAddr,
        port: (u16, Option<u16>),
        ttl: Option<u8>,
    },
}

impl TryFrom<&RtpTransport> for RtspTransportInfo {
    type Error = RtspError;

    fn try_from(t: &RtpTransport) -> Result<Self, Self::Error> {
        match &t.lower_transport {
            Some(RtpLowerTransport::Tcp) => match t.params.interleaved {
                Some(v) => Ok(RtspTransportInfo::Tcp { channels: v }),
                None => Err(RtspError::Fatal(format!(
                    "Expected interleaved channels: {t:#?}",
                ))),
            },
            Some(RtpLowerTransport::Udp) | None => {
                if t.params.multicast {
                    let dest = if let Some(d) = t.params.destination.as_ref() {
                        match d.parse::<IpAddr>() {
                            Ok(d) => d,
                            Err(err) => {
                                return Err(RtspError::Fatal(format!(
                                    "Failed to parse multicast dest addr: {err:?}"
                                )));
                            }
                        }
                    } else {
                        return Err(RtspError::Fatal(format!(
                            "Need multicast dest addr: {:#?}",
                            t.params,
                        )));
                    };
                    let Some(port) = t.params.port else {
                        return Err(RtspError::Fatal(format!(
                            "Need multicast UDP port(s): {:#?}",
                            t.params,
                        )));
                    };
                    Ok(RtspTransportInfo::UdpMulticast {
                        dest,
                        port,
                        ttl: t.params.ttl,
                    })
                } else {
                    Ok(RtspTransportInfo::Udp {
                        source: t.params.source.clone(),
                        server_port: t.params.server_port,
                        client_port: t.params.client_port,
                        sockets: None,
                    })
                }
            }
            Some(RtpLowerTransport::Other(token)) => Err(RtspError::Fatal(format!(
                "Unsupported RTP lower transport {token:?}"
            ))),
        }
    }
}

impl RtspTransportInfo {
    pub fn to_protocol(&self) -> RtspProtocol {
        match &self {
            Self::Tcp { .. } => RtspProtocol::Tcp,
            Self::Udp { .. } => RtspProtocol::Udp,
            Self::UdpMulticast { .. } => RtspProtocol::UdpMulticast,
        }
    }
}
