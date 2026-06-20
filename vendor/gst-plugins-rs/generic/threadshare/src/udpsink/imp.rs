// Copyright (C) 2019 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst::EventView;
use gst::{element_error, error_msg};

use std::sync::LazyLock;

use crate::net;
use crate::runtime::executor::block_on_or_add_subtask;
use crate::runtime::prelude::*;
use crate::runtime::{self, Async, Context, PadSink};
use crate::socket::{wrap_socket, GioSocketWrapper};

use std::collections::BTreeSet;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const DEFAULT_HOST: Option<&str> = Some("127.0.0.1");
const DEFAULT_PORT: i32 = 5004;
const DEFAULT_SYNC: bool = true;
const DEFAULT_BIND_ADDRESS: &str = "0.0.0.0";
const DEFAULT_BIND_PORT: i32 = 0;
const DEFAULT_BIND_ADDRESS_V6: &str = "::";
const DEFAULT_BIND_PORT_V6: i32 = 0;
const DEFAULT_SOCKET: Option<GioSocketWrapper> = None;
const DEFAULT_USED_SOCKET: Option<GioSocketWrapper> = None;
const DEFAULT_SOCKET_V6: Option<GioSocketWrapper> = None;
const DEFAULT_USED_SOCKET_V6: Option<GioSocketWrapper> = None;
const DEFAULT_AUTO_MULTICAST: bool = true;
const DEFAULT_LOOP: bool = true;
const DEFAULT_TTL: u32 = 64;
const DEFAULT_TTL_MC: u32 = 1;
const DEFAULT_QOS_DSCP: i32 = -1;
const DEFAULT_CLIENTS: &str = "";
const DEFAULT_CONTEXT: &str = "";
const DEFAULT_CONTEXT_WAIT: Duration = Duration::ZERO;
const DEFAULT_MULTICAST_IFACE: Option<&str> = None;

#[derive(Debug, Clone, Copy)]
struct SocketConf {
    auto_multicast: bool,
    multicast_loop: bool,
    ttl: u32,
    ttl_mc: u32,
}

impl Default for SocketConf {
    fn default() -> Self {
        SocketConf {
            auto_multicast: DEFAULT_AUTO_MULTICAST,
            multicast_loop: DEFAULT_LOOP,
            ttl: DEFAULT_TTL,
            ttl_mc: DEFAULT_TTL_MC,
        }
    }
}

#[derive(Debug, Clone)]
struct Settings {
    sync: bool,
    bind_address: String,
    bind_port: i32,
    bind_address_v6: String,
    bind_port_v6: i32,
    socket: Option<GioSocketWrapper>,
    used_socket: Option<GioSocketWrapper>,
    socket_v6: Option<GioSocketWrapper>,
    used_socket_v6: Option<GioSocketWrapper>,
    socket_conf: SocketConf,
    qos_dscp: i32,
    context: String,
    context_wait: Duration,
    multicast_iface: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            sync: DEFAULT_SYNC,
            bind_address: DEFAULT_BIND_ADDRESS.into(),
            bind_port: DEFAULT_BIND_PORT,
            bind_address_v6: DEFAULT_BIND_ADDRESS_V6.into(),
            bind_port_v6: DEFAULT_BIND_PORT_V6,
            socket: DEFAULT_SOCKET,
            used_socket: DEFAULT_USED_SOCKET,
            socket_v6: DEFAULT_SOCKET_V6,
            used_socket_v6: DEFAULT_USED_SOCKET_V6,
            socket_conf: SocketConf::default(),
            qos_dscp: DEFAULT_QOS_DSCP,
            context: DEFAULT_CONTEXT.into(),
            context_wait: DEFAULT_CONTEXT_WAIT,
            multicast_iface: DEFAULT_MULTICAST_IFACE.map(Into::into),
        }
    }
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "ts-udpsink",
        gst::DebugColorFlags::empty(),
        Some("Thread-sharing UDP sink"),
    )
});

#[derive(Clone, Debug, Default)]
struct UdpSinkPadHandler(Arc<futures::lock::Mutex<UdpSinkPadHandlerInner>>);

impl UdpSinkPadHandler {
    fn prepare(
        &self,
        imp: &UdpSink,
        socket: Option<Async<UdpSocket>>,
        socket_v6: Option<Async<UdpSocket>>,
        settings: &Settings,
    ) -> Result<(), gst::ErrorMessage> {
        let this = self.clone();
        let elem = imp.obj().clone();
        let settings = settings.clone();

        block_on_or_add_subtask(async move {
            let mut inner = this.0.lock().await;

            inner.sync = settings.sync;
            inner.socket_conf = settings.socket_conf;
            inner.socket = socket;
            inner.socket_v6 = socket_v6;

            if let Some(multicast_iface) = &settings.multicast_iface {
                gst::debug!(
                    CAT,
                    obj = elem,
                    "searching for interface: {multicast_iface}",
                );

                // The 'InterfaceFilter::name' only checks for the 'name' field , it does not check
                // whether the given name with the interface 'description' (Friendly Name) on Windows

                // So we first get all the interfaces and then apply filter
                // for name and description (Friendly Name) of each interface.

                let ifaces = getifaddrs::getifaddrs().map_err(|err| {
                    gst::error_msg!(
                        gst::ResourceError::OpenRead,
                        ["Failed to find interface {multicast_iface}: {err}"]
                    )
                })?;

                let iface_filter = ifaces.filter(|i| {
                    let ip_ver = if i.address.is_ipv4() { "IPv4" } else { "IPv6" };

                    if &i.name == multicast_iface {
                        gst::debug!(
                            CAT,
                            obj = elem,
                            "Found interface: {}, version: {ip_ver}",
                            i.name,
                        );
                        true
                    } else {
                        #[cfg(windows)]
                        if &i.description == multicast_iface {
                            gst::debug!(
                                CAT,
                                obj = elem,
                                "Found interface: {}, version: {ip_ver}",
                                i.description,
                            );
                            return true;
                        }

                        gst::trace!(CAT, obj = elem, "skipping interface {}", i.name);
                        false
                    }
                });

                inner.multicast_ifaces = iface_filter.collect();
            }

            for addr in inner.clients.iter() {
                inner.configure_client(addr)?;
            }

            Ok(())
        })
        .unwrap_or(Ok(()))
    }

    fn unprepare(&self) {
        let this = self.clone();
        block_on_or_add_subtask(async move {
            let mut inner = this.0.lock().await;

            for addr in inner.clients.iter() {
                let _ = inner.unconfigure_client(addr);
            }

            inner.socket = None;
            inner.socket_v6 = None;
        });
    }

    fn start(&self) {
        let this = self.clone();
        block_on_or_add_subtask(async move {
            this.0.lock().await.is_flushing = false;
        });
    }

    fn stop(&self) {
        let this = self.clone();
        block_on_or_add_subtask(async move {
            this.0.lock().await.is_flushing = true;
        });
    }

    fn set_latency(&self, latency: Option<gst::ClockTime>) {
        let this = self.clone();
        block_on_or_add_subtask(async move {
            this.0.lock().await.latency = latency;
        });
    }

    fn set_sync(&self, sync: bool) {
        let this = self.clone();
        block_on_or_add_subtask(async move {
            this.0.lock().await.sync = sync;
        });
    }

    fn set_socket_conf(&self, socket_conf: SocketConf) {
        let this = self.clone();
        block_on_or_add_subtask(async move {
            this.0.lock().await.socket_conf = socket_conf;
        });
    }

    fn add_client(&self, imp: &UdpSink, addr: SocketAddr) {
        let this = self.clone();
        let elem = imp.obj().clone();
        block_on_or_add_subtask(async move {
            let mut inner = this.0.lock().await;
            if inner.clients.contains(&addr) {
                gst::warning!(CAT, obj = elem, "Not adding client {addr:?} again");
                return;
            }

            match inner.configure_client(&addr) {
                Ok(()) => {
                    gst::info!(CAT, obj = elem, "Added client {addr:?}");
                    inner.clients.insert(addr);
                }
                Err(err) => {
                    gst::error!(CAT, obj = elem, "Failed to add client {addr:?}: {err}");
                    elem.post_error_message(err);
                }
            }
        });
    }

    fn remove_client(&self, imp: &UdpSink, addr: SocketAddr) {
        let this = self.clone();
        let elem = imp.obj().clone();
        block_on_or_add_subtask(async move {
            let mut inner = this.0.lock().await;
            if inner.clients.take(&addr).is_none() {
                gst::warning!(CAT, obj = elem, "Not removing unknown client {addr:?}");
                return;
            }

            match inner.unconfigure_client(&addr) {
                Ok(()) => {
                    gst::info!(CAT, obj = elem, "Removed client {addr:?}");
                }
                Err(err) => {
                    gst::error!(CAT, obj = elem, "Failed to remove client {addr:?}: {err}");
                    elem.post_error_message(err);
                }
            }
        });
    }

    fn replace_clients(&self, imp: &UdpSink, mut new_clients: BTreeSet<SocketAddr>) {
        let this = self.clone();
        let elem = imp.obj().clone();
        block_on_or_add_subtask(async move {
            let mut inner = this.0.lock().await;
            if new_clients.is_empty() {
                gst::info!(CAT, obj = elem, "Clearing clients");
            } else {
                gst::info!(CAT, obj = elem, "Replacing clients");
            }

            let old_clients = std::mem::take(&mut inner.clients);

            let mut res = Ok(());

            for addr in old_clients.iter() {
                if new_clients.take(addr).is_some() {
                    // client is already configured
                    inner.clients.insert(*addr);
                } else if let Err(err) = inner.unconfigure_client(addr) {
                    gst::error!(CAT, obj = elem, "Failed to remove client {addr:?}: {err}");
                    res = Err(err);
                } else {
                    gst::info!(CAT, obj = elem, "Removed client {addr:?}");
                }
            }

            for addr in new_clients.into_iter() {
                if let Err(err) = inner.configure_client(&addr) {
                    gst::error!(CAT, obj = elem, "Failed to add client {addr:?}: {err}");
                    res = Err(err);
                } else {
                    gst::info!(CAT, obj = elem, "Added client {addr:?}");
                    inner.clients.insert(addr);
                }
            }

            // FIXME: which error handling:
            // - If at least one client could be configured, should we keep going? (current)
            // - or, should we consider the preparation failed when the first client
            //   configuration fails? (previously)
            if let Err(err) = res {
                elem.post_error_message(err);
            }
        });
    }
}

impl PadSinkHandler for UdpSinkPadHandler {
    type ElementImpl = UdpSink;

    async fn sink_chain(
        self,
        _pad: gst::Pad,
        elem: super::UdpSink,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        self.0.lock().await.handle_buffer(&elem, buffer).await
    }

    async fn sink_chain_list(
        self,
        _pad: gst::Pad,
        elem: super::UdpSink,
        list: gst::BufferList,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut inner = self.0.lock().await;
        for buffer in list.iter_owned() {
            inner.handle_buffer(&elem, buffer).await?;
        }

        Ok(gst::FlowSuccess::Ok)
    }

    async fn sink_event_serialized(
        self,
        _pad: gst::Pad,
        elem: super::UdpSink,
        event: gst::Event,
    ) -> bool {
        gst::debug!(CAT, obj = elem, "Handling {event:?}");

        match event.view() {
            EventView::Eos(_) => {
                let _ = elem.post_message(gst::message::Eos::builder().src(&elem).build());
            }
            EventView::Segment(e) => {
                self.0.lock().await.segment = Some(e.segment().clone());
            }
            EventView::FlushStop(_) => {
                self.0.lock().await.is_flushing = false;
            }
            EventView::SinkMessage(e) => {
                let _ = elem.post_message(e.message());
            }
            _ => (),
        }

        true
    }

    fn sink_event(self, _pad: &gst::Pad, imp: &UdpSink, event: gst::Event) -> bool {
        gst::debug!(CAT, imp = imp, "Handling {event:?}");

        if let EventView::FlushStart(..) = event.view() {
            block_on_or_add_subtask(async move {
                self.0.lock().await.is_flushing = true;
            });
        }

        true
    }
}

#[derive(Debug)]
struct UdpSinkPadHandlerInner {
    is_flushing: bool,
    sync: bool,
    latency: Option<gst::ClockTime>,
    socket: Option<Async<UdpSocket>>,
    socket_v6: Option<Async<UdpSocket>>,
    clients: BTreeSet<SocketAddr>,
    socket_conf: SocketConf,
    segment: Option<gst::Segment>,
    multicast_ifaces: Vec<getifaddrs::Interface>,
}

impl Default for UdpSinkPadHandlerInner {
    fn default() -> Self {
        Self {
            is_flushing: true,
            sync: DEFAULT_SYNC,
            latency: None,
            socket: None,
            socket_v6: None,
            clients: BTreeSet::from([SocketAddr::new(
                DEFAULT_HOST.unwrap().parse().unwrap(),
                DEFAULT_PORT as u16,
            )]),
            socket_conf: Default::default(),
            segment: None,
            multicast_ifaces: Vec::<getifaddrs::Interface>::new(),
        }
    }
}

/// Socket configuration.
impl UdpSinkPadHandlerInner {
    fn configure_client(&self, client: &SocketAddr) -> Result<(), gst::ErrorMessage> {
        if client.ip().is_multicast() {
            match client.ip() {
                IpAddr::V4(addr) => {
                    let Some(socket) = self.socket.as_ref() else {
                        return Ok(());
                    };

                    if self.socket_conf.auto_multicast {
                        for iface in &self.multicast_ifaces {
                            if !iface.address.is_ipv4() {
                                gst::debug!(
                                    CAT,
                                    "Skipping the IPv6 version of the interface {}",
                                    iface.name
                                );
                                continue;
                            }

                            gst::debug!(CAT, "interface {} joining the multicast", iface.name);
                            net::imp::join_multicast_v4(socket.as_ref(), &addr, iface).map_err(
                                |err| {
                                    error_msg!(
                                        gst::ResourceError::OpenWrite,
                                        [
                                            "Failed to join multicast group on iface {} for {:?}: {}",
                                            iface.name,
                                            client,
                                            err
                                        ]
                                    )
                                },
                            )?;
                        }
                    }

                    if self.socket_conf.multicast_loop {
                        socket.as_ref().set_multicast_loop_v4(true).map_err(|err| {
                            error_msg!(
                                gst::ResourceError::OpenWrite,
                                ["Failed to set multicast loop for {:?}: {}", client, err]
                            )
                        })?;
                    }

                    socket
                        .as_ref()
                        .set_multicast_ttl_v4(self.socket_conf.ttl_mc)
                        .map_err(|err| {
                            error_msg!(
                                gst::ResourceError::OpenWrite,
                                ["Failed to set multicast ttl for {:?}: {}", client, err]
                            )
                        })?;
                }
                IpAddr::V6(addr) => {
                    let Some(socket) = self.socket.as_ref() else {
                        return Err(error_msg!(
                            gst::ResourceError::OpenWrite,
                            ["Socket not available"]
                        ));
                    };

                    if self.socket_conf.auto_multicast {
                        for iface in &self.multicast_ifaces {
                            if !iface.address.is_ipv6() {
                                gst::debug!(
                                    CAT,
                                    "Skipping the IPv4 version of the interface {}",
                                    iface.name
                                );
                                continue;
                            }

                            gst::debug!(CAT, "interface {} joining the multicast", iface.name);
                            socket
                                .as_ref()
                                .join_multicast_v6(&addr, iface.index.unwrap_or(0))
                                .map_err(|err| {
                                    error_msg!(
                                        gst::ResourceError::OpenWrite,
                                        [
                                            "Failed to join multicast group on iface {} for {:?}: {}",
                                            iface.name,
                                            client,
                                            err
                                        ]
                                    )
                                })?;
                        }
                    }

                    if self.socket_conf.multicast_loop {
                        socket.as_ref().set_multicast_loop_v6(true).map_err(|err| {
                            error_msg!(
                                gst::ResourceError::OpenWrite,
                                ["Failed to set multicast loop for {:?}: {}", client, err]
                            )
                        })?;
                    }
                    /* FIXME no API for set_multicast_ttl_v6 ? */
                }
            }
        } else {
            match client.ip() {
                IpAddr::V4(_) => {
                    if let Some(socket) = self.socket.as_ref() {
                        socket
                            .as_ref()
                            .set_ttl(self.socket_conf.ttl)
                            .map_err(|err| {
                                error_msg!(
                                    gst::ResourceError::OpenWrite,
                                    ["Failed to set unicast ttl for {:?}: {}", client, err]
                                )
                            })?;
                    }
                }
                IpAddr::V6(_) => {
                    if let Some(socket) = self.socket_v6.as_ref() {
                        socket
                            .as_ref()
                            .set_ttl(self.socket_conf.ttl)
                            .map_err(|err| {
                                error_msg!(
                                    gst::ResourceError::OpenWrite,
                                    ["Failed to set unicast ttl for {:?}: {}", client, err]
                                )
                            })?;
                    }
                }
            }
        }

        Ok(())
    }

    fn unconfigure_client(&self, client: &SocketAddr) -> Result<(), gst::ErrorMessage> {
        if client.ip().is_multicast() {
            match client.ip() {
                IpAddr::V4(addr) => {
                    let Some(socket) = self.socket.as_ref() else {
                        return Err(error_msg!(
                            gst::ResourceError::OpenWrite,
                            ["Socket not available"]
                        ));
                    };

                    if self.socket_conf.auto_multicast {
                        for iface in &self.multicast_ifaces {
                            if !iface.address.is_ipv4() {
                                gst::debug!(
                                    CAT,
                                    "Skipping the IPv6 version of the interface {}",
                                    iface.name
                                );
                                continue;
                            }

                            gst::debug!(CAT, "interface {} leaving the multicast", iface.name);
                            net::imp::leave_multicast_v4(socket.as_ref(), &addr, iface).map_err(
                                |err| {
                                    error_msg!(
                                        gst::ResourceError::OpenWrite,
                                        [
                                            "Failed to leave multicast group for {:?}: {}",
                                            client,
                                            err
                                        ]
                                    )
                                },
                            )?;
                        }
                    }
                }
                IpAddr::V6(addr) => {
                    let Some(socket) = self.socket.as_ref() else {
                        return Err(error_msg!(
                            gst::ResourceError::OpenWrite,
                            ["Socket not available"]
                        ));
                    };

                    if self.socket_conf.auto_multicast {
                        for iface in &self.multicast_ifaces {
                            if !iface.address.is_ipv6() {
                                gst::debug!(
                                    CAT,
                                    "Skipping the IPv4 version of the interface {}",
                                    iface.name
                                );
                                continue;
                            }

                            gst::debug!(CAT, "interface {} leaving the multicast", iface.name);
                            socket
                                .as_ref()
                                .leave_multicast_v6(&addr, iface.index.unwrap_or(0))
                                .map_err(|err| {
                                    error_msg!(
                                        gst::ResourceError::OpenWrite,
                                        [
                                            "Failed to leave multicast group for {:?}: {}",
                                            client,
                                            err
                                        ]
                                    )
                                })?;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

/// Buffer handling.
impl UdpSinkPadHandlerInner {
    async fn render(
        &mut self,
        elem: &super::UdpSink,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let data = buffer.map_readable().map_err(|_| {
            gst::element_error!(
                elem,
                gst::StreamError::Format,
                ["Failed to map buffer readable"]
            );
            gst::FlowError::Error
        })?;

        for client in self.clients.iter() {
            let socket = match client.ip() {
                IpAddr::V4(_) => &mut self.socket,
                IpAddr::V6(_) => &mut self.socket_v6,
            };

            if let Some(socket) = socket.as_mut() {
                gst::log!(CAT, obj = elem, "Sending to {client:?}");
                socket.send_to(&data, *client).await.map_err(|err| {
                    gst::element_error!(
                        elem,
                        gst::StreamError::Failed,
                        ("I/O error"),
                        ["streaming stopped, I/O error {}", err]
                    );
                    gst::FlowError::Error
                })?;
            } else {
                gst::element_error!(
                    elem,
                    gst::StreamError::Failed,
                    ("I/O error"),
                    ["No socket available for sending to {}", client]
                );
                return Err(gst::FlowError::Error);
            }
        }

        gst::log!(CAT, obj = elem, "Sent buffer {buffer:?} to all clients");

        Ok(gst::FlowSuccess::Ok)
    }

    /// Waits until specified time.
    async fn sync(&self, elem: &super::UdpSink, running_time: gst::ClockTime) {
        let now = elem.current_running_time();

        if let Ok(Some(delay)) = running_time.opt_checked_sub(now) {
            gst::trace!(CAT, obj = elem, "sync: waiting {delay}");
            // Up to 1/2 context-wait delay. See also `UdpSink::query()`.
            runtime::timer::delay_for(delay.into()).await;
        }
    }

    async fn handle_buffer(
        &mut self,
        elem: &super::UdpSink,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        if self.is_flushing {
            gst::info!(CAT, obj = elem, "Discarding {buffer:?} (flushing)");

            return Err(gst::FlowError::Flushing);
        }

        if self.sync {
            let rtime = self.segment.as_ref().and_then(|segment| {
                segment
                    .downcast_ref::<gst::format::Time>()
                    .and_then(|segment| segment.to_running_time(buffer.pts()).opt_add(self.latency))
            });

            if let Some(rtime) = rtime {
                self.sync(elem, rtime).await;

                if self.is_flushing {
                    gst::info!(CAT, obj = elem, "Discarding {buffer:?} (flushing)");

                    return Err(gst::FlowError::Flushing);
                }
            }
        }

        gst::debug!(CAT, obj = elem, "Handling {buffer:?}");

        self.render(elem, buffer).await.map_err(|err| {
            element_error!(
                elem,
                gst::StreamError::Failed,
                ["Failed to render item, stopping task: {}", err]
            );
            gst::FlowError::Error
        })
    }
}

#[derive(Debug)]
enum SocketFamily {
    Ipv4,
    Ipv6,
}

#[derive(Debug)]
pub struct UdpSink {
    sink_pad: PadSink,
    sink_pad_handler: UdpSinkPadHandler,
    settings: Mutex<Settings>,
    ts_ctx: Mutex<Option<Context>>,
}

impl UdpSink {
    fn prepare_socket(
        &self,
        ts_ctx: &Context,
        settings: &mut Settings,
        family: SocketFamily,
    ) -> Result<Option<Async<UdpSocket>>, gst::ErrorMessage> {
        let wrapped_socket = match family {
            SocketFamily::Ipv4 => &settings.socket,
            SocketFamily::Ipv6 => &settings.socket_v6,
        };

        if let Some(ref wrapped_socket) = wrapped_socket {
            let socket: UdpSocket = wrapped_socket.get();
            let socket = ts_ctx.enter(|| {
                Async::<UdpSocket>::try_from(socket).map_err(|err| {
                    error_msg!(
                        gst::ResourceError::OpenWrite,
                        ["Failed to setup Async socket: {}", err]
                    )
                })
            })?;

            match family {
                SocketFamily::Ipv4 => {
                    settings.used_socket = Some(wrapped_socket.clone());
                }
                SocketFamily::Ipv6 => {
                    settings.used_socket_v6 = Some(wrapped_socket.clone());
                }
            }

            Ok(Some(socket))
        } else {
            let bind_addr = match family {
                SocketFamily::Ipv4 => &settings.bind_address,
                SocketFamily::Ipv6 => &settings.bind_address_v6,
            };

            let bind_addr: IpAddr = bind_addr.parse().map_err(|err| {
                error_msg!(
                    gst::ResourceError::Settings,
                    ["Invalid address '{}' set: {}", bind_addr, err]
                )
            })?;

            let bind_port = match family {
                SocketFamily::Ipv4 => settings.bind_port,
                SocketFamily::Ipv6 => settings.bind_port_v6,
            };

            let saddr = SocketAddr::new(bind_addr, bind_port as u16);
            gst::debug!(CAT, imp = self, "Binding to {:?}", saddr);

            let socket = match family {
                SocketFamily::Ipv4 => socket2::Socket::new(
                    socket2::Domain::IPV4,
                    socket2::Type::DGRAM,
                    Some(socket2::Protocol::UDP),
                ),
                SocketFamily::Ipv6 => socket2::Socket::new(
                    socket2::Domain::IPV6,
                    socket2::Type::DGRAM,
                    Some(socket2::Protocol::UDP),
                ),
            };

            let socket = match socket {
                Ok(socket) => socket,
                Err(err) => {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Failed to create {} socket: {}",
                        match family {
                            SocketFamily::Ipv4 => "IPv4",
                            SocketFamily::Ipv6 => "IPv6",
                        },
                        err
                    );
                    return Ok(None);
                }
            };

            socket.bind(&saddr.into()).map_err(|err| {
                error_msg!(
                    gst::ResourceError::OpenWrite,
                    ["Failed to bind socket: {}", err]
                )
            })?;

            let socket = ts_ctx.enter(|| {
                Async::<UdpSocket>::try_from(socket).map_err(|err| {
                    error_msg!(
                        gst::ResourceError::OpenWrite,
                        ["Failed to setup Async socket: {}", err]
                    )
                })
            })?;

            let wrapper = wrap_socket(&socket)?;

            if settings.qos_dscp != -1 {
                wrapper.set_tos(settings.qos_dscp).map_err(|err| {
                    error_msg!(
                        gst::ResourceError::OpenWrite,
                        ["Failed to set QoS DSCP: {}", err]
                    )
                })?;
            }

            match family {
                SocketFamily::Ipv4 => {
                    settings.used_socket = Some(wrapper);
                }
                SocketFamily::Ipv6 => {
                    settings.used_socket_v6 = Some(wrapper);
                }
            }

            Ok(Some(socket))
        }
    }

    fn prepare(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp = self, "Preparing");

        let mut settings = self.settings.lock().unwrap();

        let ts_ctx = Context::acquire(&settings.context, settings.context_wait).map_err(|err| {
            error_msg!(
                gst::ResourceError::OpenWrite,
                ["Failed to acquire Context: {}", err]
            )
        })?;

        let socket = self.prepare_socket(&ts_ctx, &mut settings, SocketFamily::Ipv4)?;
        let socket_v6 = self.prepare_socket(&ts_ctx, &mut settings, SocketFamily::Ipv6)?;

        self.sink_pad_handler
            .prepare(self, socket, socket_v6, &settings)?;
        *self.ts_ctx.lock().unwrap() = Some(ts_ctx);

        gst::debug!(CAT, imp = self, "Started preparation");

        Ok(())
    }

    fn unprepare(&self) {
        gst::debug!(CAT, imp = self, "Unpreparing");
        self.sink_pad_handler.unprepare();
        *self.ts_ctx.lock().unwrap() = None;
        gst::debug!(CAT, imp = self, "Unprepared");
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp = self, "Stopping");
        self.sink_pad_handler.stop();
        gst::debug!(CAT, imp = self, "Stopped");
        Ok(())
    }

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp = self, "Starting");
        self.sink_pad_handler.start();
        gst::debug!(CAT, imp = self, "Started");
        Ok(())
    }

    fn try_into_socket_addr(&self, host: &str, port: i32) -> Result<SocketAddr, ()> {
        let addr: IpAddr = match host.parse() {
            Err(err) => {
                gst::error!(CAT, imp = self, "Failed to parse host {}: {}", host, err);
                return Err(());
            }
            Ok(addr) => addr,
        };

        let port: u16 = match port.try_into() {
            Err(err) => {
                gst::error!(CAT, imp = self, "Invalid port {}: {}", port, err);
                return Err(());
            }
            Ok(port) => port,
        };

        Ok(SocketAddr::new(addr, port))
    }
}

#[glib::object_subclass]
impl ObjectSubclass for UdpSink {
    const NAME: &'static str = "GstTsUdpSink";
    type Type = super::UdpSink;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let sink_pad_handler = UdpSinkPadHandler::default();
        Self {
            sink_pad: PadSink::new(
                gst::Pad::from_template(&klass.pad_template("sink").unwrap()),
                sink_pad_handler.clone(),
            ),
            sink_pad_handler,
            settings: Default::default(),
            ts_ctx: Default::default(),
        }
    }
}

impl ObjectImpl for UdpSink {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecString::builder("context")
                    .nick("Context")
                    .blurb("Context name to share threads with")
                    .default_value(Some(DEFAULT_CONTEXT))
                    .build(),
                glib::ParamSpecUInt::builder("context-wait")
                    .nick("Context Wait")
                    .blurb("Throttle poll loop to run at most once every this many ms")
                    .maximum(1000)
                    .default_value(DEFAULT_CONTEXT_WAIT.as_millis() as u32)
                    .build(),
                glib::ParamSpecBoolean::builder("sync")
                    .nick("Sync")
                    .blurb("Sync on the clock")
                    .default_value(DEFAULT_SYNC)
                    .build(),
                glib::ParamSpecString::builder("bind-address")
                    .nick("Bind Address")
                    .blurb("Address to bind the socket to")
                    .default_value(Some(DEFAULT_BIND_ADDRESS))
                    .build(),
                glib::ParamSpecInt::builder("bind-port")
                    .nick("Bind Port")
                    .blurb("Port to bind the socket to")
                    .minimum(0)
                    .maximum(u16::MAX as i32)
                    .default_value(DEFAULT_BIND_PORT)
                    .build(),
                glib::ParamSpecString::builder("bind-address-v6")
                    .nick("Bind Address V6")
                    .blurb("Address to bind the V6 socket to")
                    .default_value(Some(DEFAULT_BIND_ADDRESS_V6))
                    .build(),
                glib::ParamSpecInt::builder("bind-port-v6")
                    .nick("Bind Port")
                    .blurb("Port to bind the V6 socket to")
                    .minimum(0)
                    .maximum(u16::MAX as i32)
                    .default_value(DEFAULT_BIND_PORT_V6)
                    .build(),
                glib::ParamSpecObject::builder::<gio::Socket>("socket")
                    .nick("Socket")
                    .blurb("Socket to use for UDP transmission. (None == allocate)")
                    .build(),
                glib::ParamSpecObject::builder::<gio::Socket>("used-socket")
                    .nick("Used Socket")
                    .blurb("Socket currently in use for UDP transmission. (None = no socket)")
                    .read_only()
                    .build(),
                glib::ParamSpecObject::builder::<gio::Socket>("socket-v6")
                    .nick("Socket V6")
                    .blurb("IPV6 Socket to use for UDP transmission. (None == allocate)")
                    .build(),
                glib::ParamSpecObject::builder::<gio::Socket>("used-socket-v6")
                    .nick("Used Socket V6")
                    .blurb("V6 Socket currently in use for UDP transmission. (None = no socket)")
                    .read_only()
                    .build(),
                glib::ParamSpecBoolean::builder("auto-multicast")
                    .nick("Auto multicast")
                    .blurb("Automatically join/leave the multicast groups, FALSE means user has to do it himself")
                    .default_value(DEFAULT_AUTO_MULTICAST)
                    .build(),
                glib::ParamSpecBoolean::builder("loop")
                    .nick("Loop")
                    .blurb("Set the multicast loop parameter.")
                    .default_value(DEFAULT_LOOP)
                    .build(),
                glib::ParamSpecUInt::builder("ttl")
                    .nick("Time To Live")
                    .blurb("Used for setting the unicast TTL parameter")
                    .maximum(u8::MAX as u32)
                    .default_value(DEFAULT_TTL)
                    .build(),
                glib::ParamSpecUInt::builder("ttl-mc")
                    .nick("Time To Live Multicast")
                    .blurb("Used for setting the multicast TTL parameter")
                    .maximum(u8::MAX as u32)
                    .default_value(DEFAULT_TTL_MC)
                    .build(),
                glib::ParamSpecInt::builder("qos-dscp")
                    .nick("QoS DSCP")
                    .blurb("Quality of Service, differentiated services code point (-1 default)")
                    .minimum(-1)
                    .maximum(63)
                    .default_value(DEFAULT_QOS_DSCP)
                    .build(),
                glib::ParamSpecString::builder("clients")
                    .nick("Clients")
                    .blurb("A comma separated list of host:port pairs with destinations")
                    .default_value(Some(DEFAULT_CLIENTS))
                    .build(),
                glib::ParamSpecString::builder("multicast-iface")
                    .nick("Multicast Interface")
                    .blurb("The network interface on which to join the multicast group. (Supports only single interface)")
                    .default_value(DEFAULT_MULTICAST_IFACE)
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn signals() -> &'static [glib::subclass::Signal] {
        static SIGNALS: LazyLock<Vec<glib::subclass::Signal>> = LazyLock::new(|| {
            vec![
                glib::subclass::Signal::builder("add")
                    .param_types([String::static_type(), i32::static_type()])
                    .action()
                    .class_handler(|args| {
                        let elem = args[0].get::<super::UdpSink>().expect("signal arg");
                        let host = args[1].get::<String>().expect("signal arg");
                        let port = args[2].get::<i32>().expect("signal arg");
                        let imp = elem.imp();

                        if let Ok(addr) = imp.try_into_socket_addr(&host, port) {
                            imp.sink_pad_handler.add_client(imp, addr);
                        }

                        None
                    })
                    .build(),
                glib::subclass::Signal::builder("remove")
                    .param_types([String::static_type(), i32::static_type()])
                    .action()
                    .class_handler(|args| {
                        let elem = args[0].get::<super::UdpSink>().expect("signal arg");
                        let host = args[1].get::<String>().expect("signal arg");
                        let port = args[2].get::<i32>().expect("signal arg");
                        let imp = elem.imp();

                        if let Ok(addr) = imp.try_into_socket_addr(&host, port) {
                            imp.sink_pad_handler.remove_client(imp, addr);
                        }

                        None
                    })
                    .build(),
                glib::subclass::Signal::builder("clear")
                    .action()
                    .class_handler(|args| {
                        let elem = args[0].get::<super::UdpSink>().expect("signal arg");

                        let imp = elem.imp();
                        imp.sink_pad_handler.replace_clients(imp, BTreeSet::new());

                        None
                    })
                    .build(),
            ]
        });

        SIGNALS.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().unwrap();
        match pspec.name() {
            "sync" => {
                let sync = value.get().expect("type checked upstream");
                settings.sync = sync;
                self.sink_pad_handler.set_sync(sync);
            }
            "bind-address" => {
                settings.bind_address = value
                    .get::<Option<String>>()
                    .expect("type checked upstream")
                    .unwrap_or_else(|| "".into());
            }
            "bind-port" => {
                settings.bind_port = value.get().expect("type checked upstream");
            }
            "bind-address-v6" => {
                settings.bind_address_v6 = value
                    .get::<Option<String>>()
                    .expect("type checked upstream")
                    .unwrap_or_else(|| "".into());
            }
            "bind-port-v6" => {
                settings.bind_port_v6 = value.get().expect("type checked upstream");
            }
            "socket" => {
                settings.socket = value
                    .get::<Option<gio::Socket>>()
                    .expect("type checked upstream")
                    .map(|socket| GioSocketWrapper::new(&socket));
            }
            "used-socket" => {
                unreachable!();
            }
            "socket-v6" => {
                settings.socket_v6 = value
                    .get::<Option<gio::Socket>>()
                    .expect("type checked upstream")
                    .map(|socket| GioSocketWrapper::new(&socket));
            }
            "used-socket-v6" => {
                unreachable!();
            }
            "auto-multicast" => {
                settings.socket_conf.auto_multicast = value.get().expect("type checked upstream");
                self.sink_pad_handler.set_socket_conf(settings.socket_conf);
            }
            "loop" => {
                settings.socket_conf.multicast_loop = value.get().expect("type checked upstream");
                self.sink_pad_handler.set_socket_conf(settings.socket_conf);
            }
            "ttl" => {
                settings.socket_conf.ttl = value.get().expect("type checked upstream");
                self.sink_pad_handler.set_socket_conf(settings.socket_conf);
            }
            "ttl-mc" => {
                settings.socket_conf.ttl_mc = value.get().expect("type checked upstream");
                self.sink_pad_handler.set_socket_conf(settings.socket_conf);
            }
            "qos-dscp" => {
                settings.qos_dscp = value.get().expect("type checked upstream");
            }
            "clients" => {
                let clients = value
                    .get::<Option<String>>()
                    .expect("type checked upstream")
                    .unwrap_or_else(|| "".into());

                let clients = clients.split(',').filter_map(|client| {
                    let mut split = client.splitn(2, ':');
                    if let Some((addr, port)) = split.next().zip(split.next()) {
                        match port.parse::<i32>() {
                            Ok(port) => match self.try_into_socket_addr(addr, port) {
                                Ok(socket_addr) => Some(socket_addr),
                                Err(()) => {
                                    gst::error!(
                                        CAT,
                                        imp = self,
                                        "Invalid socket address {addr}:{port}"
                                    );
                                    None
                                }
                            },
                            Err(err) => {
                                gst::error!(CAT, imp = self, "Invalid port {err}");
                                None
                            }
                        }
                    } else {
                        gst::error!(CAT, imp = self, "Invalid client {client}");
                        None
                    }
                });

                let clients = BTreeSet::from_iter(clients);
                self.sink_pad_handler.replace_clients(self, clients);
            }
            "context" => {
                settings.context = value
                    .get::<Option<String>>()
                    .expect("type checked upstream")
                    .unwrap_or_else(|| DEFAULT_CONTEXT.into());
            }
            "context-wait" => {
                settings.context_wait = Duration::from_millis(
                    value.get::<u32>().expect("type checked upstream").into(),
                );
            }
            "multicast-iface" => {
                settings.multicast_iface = value.get().expect("type checked upstream");
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().unwrap();
        match pspec.name() {
            "sync" => settings.sync.to_value(),
            "bind-address" => settings.bind_address.to_value(),
            "bind-port" => settings.bind_port.to_value(),
            "bind-address-v6" => settings.bind_address_v6.to_value(),
            "bind-port-v6" => settings.bind_port_v6.to_value(),
            "socket" => settings
                .socket
                .as_ref()
                .map(GioSocketWrapper::as_socket)
                .to_value(),
            "used-socket" => settings
                .used_socket
                .as_ref()
                .map(GioSocketWrapper::as_socket)
                .to_value(),
            "socket-v6" => settings
                .socket_v6
                .as_ref()
                .map(GioSocketWrapper::as_socket)
                .to_value(),
            "used-socket-v6" => settings
                .used_socket_v6
                .as_ref()
                .map(GioSocketWrapper::as_socket)
                .to_value(),
            "auto-multicast" => settings.socket_conf.auto_multicast.to_value(),
            "loop" => settings.socket_conf.multicast_loop.to_value(),
            "ttl" => settings.socket_conf.ttl.to_value(),
            "ttl-mc" => settings.socket_conf.ttl_mc.to_value(),
            "qos-dscp" => settings.qos_dscp.to_value(),
            "clients" => {
                let clients = futures::executor::block_on(async {
                    self.sink_pad_handler.0.lock().await.clients.clone()
                });
                let clients: Vec<String> = clients.iter().map(ToString::to_string).collect();

                clients.join(",").to_value()
            }
            "context" => settings.context.to_value(),
            "context-wait" => (settings.context_wait.as_millis() as u32).to_value(),
            "multicast-iface" => settings.multicast_iface.to_value(),
            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(self.sink_pad.gst_pad()).unwrap();
        obj.set_element_flags(gst::ElementFlags::SINK);
    }
}

impl GstObjectImpl for UdpSink {}

impl ElementImpl for UdpSink {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Thread-sharing UDP sink",
                "Sink/Network",
                "Thread-sharing UDP sink",
                "Mathieu <mathieu@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::new_any();

            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            vec![sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst::trace!(CAT, imp = self, "Changing state {:?}", transition);

        match transition {
            gst::StateChange::NullToReady => {
                self.prepare().map_err(|err| {
                    self.post_error_message(err);
                    gst::StateChangeError
                })?;
            }
            gst::StateChange::ReadyToPaused => {
                self.start().map_err(|_| gst::StateChangeError)?;
            }
            gst::StateChange::PausedToReady => {
                self.stop().map_err(|_| gst::StateChangeError)?;
            }
            gst::StateChange::ReadyToNull => {
                self.unprepare();
            }
            _ => (),
        }

        self.parent_change_state(transition)
    }

    fn send_event(&self, event: gst::Event) -> bool {
        gst::log!(CAT, imp = self, "Got {event:?}");

        match event.view() {
            EventView::Latency(ev) => {
                if self.settings.lock().unwrap().sync {
                    // The element syncs on the clock so it needs the pipeline's
                    // consolidated latency to sync buffers in-sync with other branches.
                    self.sink_pad_handler.set_latency(Some(ev.latency()));
                }
                // otherwise, we need the branch latency as returned by `query()`
                self.sink_pad.gst_pad().push_event(event)
            }
            EventView::Step(..) => false,
            _ => self.sink_pad.gst_pad().push_event(event),
        }
    }

    fn query(&self, query: &mut gst::QueryRef) -> bool {
        gst::log!(CAT, imp = self, "Got {query:?}");

        if !Self::parent_query(self, query) {
            gst::debug!(CAT, imp = self, "Upstream didn't process {query:?}");
            return false;
        }

        if query.type_() == gst::QueryType::Latency {
            gst::log!(CAT, imp = self, "Upstream returned {query:?}");

            let gst::QueryViewMut::Latency(q) = query.view_mut() else {
                unreachable!();
            };
            let (sync, context_wait) = {
                let settings = self.settings.lock().unwrap();
                (settings.sync, settings.context_wait)
            };

            let (_, mut min, max) = q.result();
            if sync {
                // Element uses `delay_for` for sync (see `UdpSinkPadHandlerInner::sync`)
                // which adds up to 1/2 context-wait delay
                min += gst::ClockTime::from_nseconds(context_wait.as_nanos() as u64 / 2);

                // The element syncs on the clock so it needs the pipelines's
                // consolidated latency which is captured by `send_event()`.
            } else {
                // The element doesn't sync on the clock so the latency it sees is the branch latency.
                // (not used as of this writting but added here for consistancy)
                self.sink_pad_handler.set_latency(Some(min));
            }

            gst::log!(
                CAT,
                imp = self,
                "Returning latency: live {sync}, min {min}, max {max:?}"
            );
            q.set(sync, min, max);
        }

        true
    }
}
