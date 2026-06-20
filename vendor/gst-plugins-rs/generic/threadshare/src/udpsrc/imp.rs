// Copyright (C) 2018 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use futures::prelude::*;

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_net::*;

use std::sync::LazyLock;

use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::sync::Mutex;
use std::time::Duration;

use crate::runtime::prelude::*;
use crate::runtime::{task, Async, Context, PadSrc, Task, TaskState};

use crate::net;
use crate::socket::{wrap_socket, GioSocketWrapper, Socket, SocketError, SocketRead};
use futures::channel::mpsc::{channel, Receiver, Sender};
use futures::pin_mut;

const DEFAULT_ADDRESS: Option<&str> = Some("0.0.0.0");
const DEFAULT_PORT: i32 = 5004;
const DEFAULT_REUSE: bool = true;
const DEFAULT_CAPS: Option<gst::Caps> = None;
const DEFAULT_MTU: u32 = 1492;
const DEFAULT_SOCKET: Option<GioSocketWrapper> = None;
const DEFAULT_USED_SOCKET: Option<GioSocketWrapper> = None;
const DEFAULT_CONTEXT: &str = "";
const DEFAULT_CONTEXT_WAIT: Duration = Duration::ZERO;
const DEFAULT_RETRIEVE_SENDER_ADDRESS: bool = true;
const DEFAULT_MULTICAST_LOOP: bool = true;
const DEFAULT_BUFFER_SIZE: u32 = 0;
const DEFAULT_MULTICAST_IFACE: Option<&str> = None;

#[derive(Debug, Default)]
struct State {
    event_sender: Option<Sender<gst::Event>>,
}

#[derive(Debug, Clone)]
struct Settings {
    address: Option<String>,
    port: i32, // for conformity with C based udpsrc
    reuse: bool,
    caps: Option<gst::Caps>,
    mtu: u32,
    socket: Option<GioSocketWrapper>,
    used_socket: Option<GioSocketWrapper>,
    context: String,
    context_wait: Duration,
    retrieve_sender_address: bool,
    multicast_loop: bool,
    buffer_size: u32,
    multicast_iface: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            address: DEFAULT_ADDRESS.map(Into::into),
            port: DEFAULT_PORT,
            reuse: DEFAULT_REUSE,
            caps: DEFAULT_CAPS,
            mtu: DEFAULT_MTU,
            socket: DEFAULT_SOCKET,
            used_socket: DEFAULT_USED_SOCKET,
            context: DEFAULT_CONTEXT.into(),
            context_wait: DEFAULT_CONTEXT_WAIT,
            retrieve_sender_address: DEFAULT_RETRIEVE_SENDER_ADDRESS,
            multicast_loop: DEFAULT_MULTICAST_LOOP,
            buffer_size: DEFAULT_BUFFER_SIZE,
            multicast_iface: DEFAULT_MULTICAST_IFACE.map(Into::into),
        }
    }
}

#[derive(Debug)]
struct UdpReader(Async<UdpSocket>);

impl UdpReader {
    fn new(socket: Async<UdpSocket>) -> Self {
        UdpReader(socket)
    }
}

impl SocketRead for UdpReader {
    const DO_TIMESTAMP: bool = true;

    async fn read<'buf>(
        &'buf mut self,
        buffer: &'buf mut [u8],
    ) -> io::Result<(usize, Option<std::net::SocketAddr>)> {
        let (read_size, saddr) = self.0.recv_from(buffer).await?;
        Ok((read_size, Some(saddr)))
    }
}

#[derive(Clone, Debug)]
struct UdpSrcPadHandler;

impl PadSrcHandler for UdpSrcPadHandler {
    type ElementImpl = UdpSrc;

    fn src_event(self, pad: &gst::Pad, imp: &UdpSrc, event: gst::Event) -> bool {
        gst::log!(CAT, obj = pad, "Handling {:?}", event);

        use gst::EventView;
        let ret = match event.view() {
            EventView::FlushStart(..) => {
                imp.task.flush_start().block_on_or_add_subtask(pad).is_ok()
            }
            EventView::FlushStop(..) => imp.task.flush_stop().block_on_or_add_subtask(pad).is_ok(),
            EventView::Reconfigure(..) => true,
            EventView::Latency(..) => true,
            _ => false,
        };

        if ret {
            gst::log!(CAT, obj = pad, "Handled {:?}", event);
        } else {
            gst::log!(CAT, obj = pad, "Didn't handle {:?}", event);
        }

        ret
    }

    fn src_query(self, pad: &gst::Pad, imp: &UdpSrc, query: &mut gst::QueryRef) -> bool {
        gst::log!(CAT, obj = pad, "Handling {:?}", query);

        use gst::QueryViewMut;
        let ret = match query.view_mut() {
            QueryViewMut::Latency(q) => {
                let latency =
                    gst::ClockTime::try_from(imp.settings.lock().unwrap().context_wait).unwrap();
                gst::debug!(CAT, obj = pad, "Reporting latency {latency}");
                q.set(true, latency, gst::ClockTime::NONE);
                true
            }
            QueryViewMut::Scheduling(q) => {
                q.set(gst::SchedulingFlags::SEQUENTIAL, 1, -1, 0);
                q.add_scheduling_modes([gst::PadMode::Push]);
                true
            }
            QueryViewMut::Caps(q) => {
                let caps = if let Some(caps) = imp.configured_caps.lock().unwrap().as_ref() {
                    q.filter()
                        .map(|f| f.intersect_with_mode(caps, gst::CapsIntersectMode::First))
                        .unwrap_or_else(|| caps.clone())
                } else {
                    q.filter()
                        .map(|f| f.to_owned())
                        .unwrap_or_else(gst::Caps::new_any)
                };

                q.set_result(&caps);

                true
            }
            _ => false,
        };

        if ret {
            gst::log!(CAT, obj = pad, "Handled {:?}", query);
        } else {
            gst::log!(CAT, obj = pad, "Didn't handle {:?}", query);
        }

        ret
    }
}

struct UdpSrcTask {
    element: super::UdpSrc,
    socket: Option<Socket<UdpReader>>,
    retrieve_sender_address: bool,
    need_initial_events: bool,
    need_segment: bool,
    event_receiver: Receiver<gst::Event>,
    multicast_ifaces: Vec<getifaddrs::Interface>,
    multicast_addr: Option<IpAddr>,
}

impl UdpSrcTask {
    fn new(element: super::UdpSrc, event_receiver: Receiver<gst::Event>) -> Self {
        UdpSrcTask {
            element,
            socket: None,
            retrieve_sender_address: DEFAULT_RETRIEVE_SENDER_ADDRESS,
            need_initial_events: true,
            need_segment: true,
            event_receiver,
            multicast_ifaces: Vec::<getifaddrs::Interface>::new(),
            multicast_addr: None,
        }
    }
}

impl TaskImpl for UdpSrcTask {
    type Item = gst::Buffer;

    fn obj(&self) -> &impl IsA<glib::Object> {
        &self.element
    }

    async fn prepare(&mut self) -> Result<(), gst::ErrorMessage> {
        let udpsrc = self.element.imp();
        let mut settings = udpsrc.settings.lock().unwrap();

        gst::debug!(CAT, obj = self.element, "Preparing Task");

        self.retrieve_sender_address = settings.retrieve_sender_address;

        let socket = if let Some(ref wrapped_socket) = settings.socket {
            let socket: UdpSocket;

            #[cfg(unix)]
            {
                socket = wrapped_socket.get()
            }
            #[cfg(windows)]
            {
                socket = wrapped_socket.get()
            }

            let socket = Async::<UdpSocket>::try_from(socket).map_err(|err| {
                gst::error_msg!(
                    gst::ResourceError::OpenRead,
                    ["Failed to setup Async socket: {}", err]
                )
            })?;

            settings.used_socket = Some(wrapped_socket.clone());

            socket
        } else {
            let addr: IpAddr = match settings.address {
                None => {
                    return Err(gst::error_msg!(
                        gst::ResourceError::Settings,
                        ["No address set"]
                    ));
                }
                Some(ref addr) => match addr.parse() {
                    Err(err) => {
                        return Err(gst::error_msg!(
                            gst::ResourceError::Settings,
                            ["Invalid address '{}' set: {}", addr, err]
                        ));
                    }
                    Ok(addr) => {
                        self.multicast_addr = Some(addr);
                        addr
                    }
                },
            };
            let port = settings.port;

            // TODO: TTL etc
            let saddr = if addr.is_multicast() {
                let bind_addr = if addr.is_ipv4() {
                    IpAddr::V4(Ipv4Addr::UNSPECIFIED)
                } else {
                    IpAddr::V6(Ipv6Addr::UNSPECIFIED)
                };

                let saddr = SocketAddr::new(bind_addr, port as u16);
                gst::debug!(
                    CAT,
                    obj = self.element,
                    "Binding to {:?} for multicast group {:?}",
                    saddr,
                    addr
                );

                saddr
            } else {
                let saddr = SocketAddr::new(addr, port as u16);
                gst::debug!(CAT, obj = self.element, "Binding to {:?}", saddr);

                saddr
            };

            let socket = if addr.is_ipv4() {
                socket2::Socket::new(
                    socket2::Domain::IPV4,
                    socket2::Type::DGRAM,
                    Some(socket2::Protocol::UDP),
                )
            } else {
                socket2::Socket::new(
                    socket2::Domain::IPV6,
                    socket2::Type::DGRAM,
                    Some(socket2::Protocol::UDP),
                )
            }
            .map_err(|err| {
                gst::error_msg!(
                    gst::ResourceError::OpenRead,
                    ["Failed to create socket: {}", err]
                )
            })?;

            socket.set_reuse_address(settings.reuse).map_err(|err| {
                gst::error_msg!(
                    gst::ResourceError::OpenRead,
                    ["Failed to set reuse_address: {}", err]
                )
            })?;

            gst::debug!(
                CAT,
                obj = self.element,
                "socket recv buffer size is {:?}",
                socket.recv_buffer_size()
            );
            if settings.buffer_size != 0 {
                gst::debug!(
                    CAT,
                    obj = self.element,
                    "changing the socket recv buffer size to {}",
                    settings.buffer_size
                );
                socket
                    .set_recv_buffer_size(settings.buffer_size as usize)
                    .map_err(|err| {
                        gst::error_msg!(
                            gst::ResourceError::OpenRead,
                            ["Failed to set buffer_size: {}", err]
                        )
                    })?;
            }

            #[cfg(unix)]
            {
                socket.set_reuse_port(settings.reuse).map_err(|err| {
                    gst::error_msg!(
                        gst::ResourceError::OpenRead,
                        ["Failed to set reuse_port: {}", err]
                    )
                })?;
            }

            socket.bind(&saddr.into()).map_err(|err| {
                gst::error_msg!(
                    gst::ResourceError::OpenRead,
                    ["Failed to bind socket: {}", err]
                )
            })?;

            let socket = Async::<UdpSocket>::try_from(socket).map_err(|err| {
                gst::error_msg!(
                    gst::ResourceError::OpenRead,
                    ["Failed to setup Async socket: {}", err]
                )
            })?;

            if addr.is_multicast() {
                if let Some(multicast_iface) = &settings.multicast_iface {
                    let multi_ifaces: Vec<String> =
                        multicast_iface.split(',').map(|s| s.to_string()).collect();

                    let iter = getifaddrs::getifaddrs().map_err(|err| {
                        gst::error_msg!(
                            gst::ResourceError::OpenRead,
                            ["Failed to get interfaces: {}", err]
                        )
                    })?;

                    iter.for_each(|iface| {
                        let ip_ver = if iface.address.is_ipv4() {
                            "IPv4"
                        } else {
                            "IPv6"
                        };

                        for m in &multi_ifaces {
                            if &iface.name == m {
                                self.multicast_ifaces.push(iface.clone());
                                gst::debug!(
                                    CAT,
                                    obj = self.element,
                                    "Interface {m} available, version: {ip_ver}"
                                );
                            } else {
                                // check if name matches the interface description (Friendly name) on Windows
                                #[cfg(windows)]
                                if &iface.description == m {
                                    self.multicast_ifaces.push(iface.clone());
                                    gst::debug!(
                                        CAT,
                                        obj = self.element,
                                        "Interface {m} available, version: {ip_ver}"
                                    );
                                }
                            }
                        }
                    });
                }

                if self.multicast_ifaces.is_empty() {
                    gst::warning!(
                        CAT,
                        obj = self.element,
                        "No suitable network interfaces found, adding default iface"
                    );

                    self.multicast_ifaces.push(getifaddrs::Interface {
                        name: "default".to_owned(),
                        #[cfg(windows)]
                        description: "default".to_owned(),
                        address: getifaddrs::Address::V4(getifaddrs::NetworkAddress {
                            address: Ipv4Addr::UNSPECIFIED,
                            netmask: None,
                            associated_address: None,
                        }),
                        flags: getifaddrs::InterfaceFlags::UP,
                        index: Some(0),
                    });
                }

                match addr {
                    IpAddr::V4(addr) => {
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
                            // use the custom written API to be able to pass the interface index
                            // for all types of target OS
                            net::imp::join_multicast_v4(socket.as_ref(), &addr, iface).map_err(
                                |err| {
                                    gst::error_msg!(
                                        gst::ResourceError::OpenRead,
                                        ["Failed to join multicast group: {}", err]
                                    )
                                },
                            )?;
                        }

                        socket
                            .as_ref()
                            .set_multicast_loop_v4(settings.multicast_loop)
                            .map_err(|err| {
                                gst::error_msg!(
                                    gst::ResourceError::OpenWrite,
                                    [
                                        "Failed to set multicast loop to {}: {}",
                                        settings.multicast_loop,
                                        err
                                    ]
                                )
                            })?;
                    }
                    IpAddr::V6(addr) => {
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
                                    gst::error_msg!(
                                        gst::ResourceError::OpenRead,
                                        ["Failed to join multicast group: {}", err]
                                    )
                                })?;
                        }

                        socket
                            .as_ref()
                            .set_multicast_loop_v6(settings.multicast_loop)
                            .map_err(|err| {
                                gst::error_msg!(
                                    gst::ResourceError::OpenWrite,
                                    [
                                        "Failed to set multicast loop to {}: {}",
                                        settings.multicast_loop,
                                        err
                                    ]
                                )
                            })?;
                    }
                }
            }

            settings.used_socket = Some(wrap_socket(&socket)?);

            socket
        };

        let port: i32 = socket.as_ref().local_addr().unwrap().port().into();
        if settings.port != port {
            settings.port = port;
            drop(settings);
            self.element.notify("port");

            settings = udpsrc.settings.lock().unwrap();
        };

        let buffer_pool = gst::BufferPool::new();
        let mut config = buffer_pool.config();
        config.set_params(None, settings.mtu, 0, 0);
        buffer_pool.set_config(config).map_err(|err| {
            gst::error_msg!(
                gst::ResourceError::Settings,
                ["Failed to configure buffer pool {:?}", err]
            )
        })?;

        drop(settings);

        self.socket = Some(
            Socket::try_new(
                self.element.clone().upcast(),
                buffer_pool,
                UdpReader::new(socket),
            )
            .map_err(|err| {
                gst::error_msg!(
                    gst::ResourceError::OpenRead,
                    ["Failed to prepare socket {:?}", err]
                )
            })?,
        );

        self.element.notify("used-socket");

        Ok(())
    }

    async fn unprepare(&mut self) {
        gst::debug!(CAT, obj = self.element, "Unpreparing Task");
        let udpsrc = self.element.imp();
        if let Some(reader) = &self.socket {
            let socket = &reader.get().0;
            if let Some(addr) = self.multicast_addr {
                match addr {
                    IpAddr::V4(addr) => {
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
                            net::imp::leave_multicast_v4(socket.as_ref(), &addr, iface).unwrap();
                        }
                    }
                    IpAddr::V6(addr) => {
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
                                .unwrap();
                        }
                    }
                }
            }
        }
        udpsrc.settings.lock().unwrap().used_socket = None;
        self.element.notify("used-socket");
    }

    async fn start(&mut self) -> Result<(), gst::ErrorMessage> {
        gst::log!(CAT, obj = self.element, "Starting task");
        self.socket
            .as_mut()
            .unwrap()
            .set_clock(self.element.clock(), self.element.base_time());
        gst::log!(CAT, obj = self.element, "Task started");
        Ok(())
    }

    async fn try_next(&mut self) -> Result<gst::Buffer, gst::FlowError> {
        let event_fut = self.event_receiver.next().fuse();
        let socket_fut = self.socket.as_mut().unwrap().try_next().fuse();

        pin_mut!(event_fut);
        pin_mut!(socket_fut);

        futures::select! {
            event_res = event_fut => match event_res {
                Some(event) => {
                    gst::debug!(CAT, obj = self.element, "Handling element level event {event:?}");

                    match event.view() {
                        gst::EventView::Eos(_) => Err(gst::FlowError::Eos),
                        ev => {
                            gst::error!(CAT, obj = self.element, "Unexpected event {ev:?} on channel");
                            Err(gst::FlowError::Error)
                        }
                    }
                }
                None => {
                    gst::error!(CAT, obj = self.element, "Unexpected return on event channel");
                    Err(gst::FlowError::Error)
                }
            },
            socket_res = socket_fut => match socket_res {
                Ok((mut buffer, saddr)) => {
                    if let Some(saddr) = saddr {
                        if self.retrieve_sender_address {
                            NetAddressMeta::add(
                                buffer.get_mut().unwrap(),
                                &gio::InetSocketAddress::from(saddr),
                            );
                        }
                    }

                    Ok(buffer)
                },
                Err(err) => {
                    gst::error!(CAT, obj = self.element, "Got error {err:#}");

                    match err {
                        SocketError::Gst(err) => {
                            gst::element_error!(
                                self.element,
                                gst::StreamError::Failed,
                                ("Internal data stream error"),
                                ["streaming stopped, reason {err}"]
                            );
                        }
                        SocketError::Io(err) => {
                            gst::element_error!(
                                self.element,
                                gst::StreamError::Failed,
                                ("I/O error"),
                                ["streaming stopped, I/O error {err}"]
                            );
                        }
                    }

                    Err(gst::FlowError::Error)
                }
            },
        }
    }

    async fn handle_item(&mut self, buffer: gst::Buffer) -> Result<(), gst::FlowError> {
        gst::log!(CAT, obj = self.element, "Handling {:?}", buffer);
        let udpsrc = self.element.imp();

        if self.need_initial_events {
            gst::debug!(CAT, obj = self.element, "Pushing initial events");

            let stream_id = format!("{:08x}{:08x}", rand::random::<u32>(), rand::random::<u32>());
            let stream_start_evt = gst::event::StreamStart::builder(&stream_id)
                .group_id(gst::GroupId::next())
                .build();
            udpsrc.src_pad.push_event(stream_start_evt).await;

            let caps = udpsrc.settings.lock().unwrap().caps.clone();
            if let Some(caps) = caps {
                udpsrc
                    .src_pad
                    .push_event(gst::event::Caps::new(&caps))
                    .await;
                *udpsrc.configured_caps.lock().unwrap() = Some(caps);
            }

            self.need_initial_events = false;
        }

        if self.need_segment {
            let segment_evt =
                gst::event::Segment::new(&gst::FormattedSegment::<gst::format::Time>::new());
            udpsrc.src_pad.push_event(segment_evt).await;

            self.need_segment = false;
        }

        let res = udpsrc.src_pad.push(buffer).await.map(drop);
        match res {
            Ok(_) => gst::log!(CAT, obj = self.element, "Successfully pushed buffer"),
            Err(gst::FlowError::Flushing) => gst::debug!(CAT, obj = self.element, "Flushing"),
            Err(gst::FlowError::Eos) => {
                gst::debug!(CAT, obj = self.element, "EOS");
                udpsrc.src_pad.push_event(gst::event::Eos::new()).await;
            }
            Err(err) => {
                gst::error!(CAT, obj = self.element, "Got error {}", err);
                gst::element_error!(
                    self.element,
                    gst::StreamError::Failed,
                    ("Internal data stream error"),
                    ["streaming stopped, reason {}", err]
                );
            }
        }

        res
    }

    async fn stop(&mut self) -> Result<(), gst::ErrorMessage> {
        gst::log!(CAT, obj = self.element, "Stopping task");
        self.need_initial_events = true;
        self.need_segment = true;
        gst::log!(CAT, obj = self.element, "Task stopped");
        Ok(())
    }

    async fn flush_stop(&mut self) -> Result<(), gst::ErrorMessage> {
        gst::log!(CAT, obj = self.element, "Stopping task flush");
        self.need_segment = true;
        gst::log!(CAT, obj = self.element, "Stopped task flush");
        Ok(())
    }

    async fn handle_loop_error(&mut self, err: gst::FlowError) -> task::Trigger {
        match err {
            gst::FlowError::Flushing => {
                gst::debug!(CAT, obj = self.element, "Flushing");

                task::Trigger::FlushStart
            }
            gst::FlowError::Eos => {
                gst::debug!(CAT, obj = self.element, "EOS");
                self.element
                    .imp()
                    .src_pad
                    .push_event(gst::event::Eos::new())
                    .await;

                task::Trigger::Stop
            }
            err => {
                gst::error!(CAT, obj = self.element, "Got error {err}");
                gst::element_error!(
                    &self.element,
                    gst::StreamError::Failed,
                    ("Internal data stream error"),
                    ["streaming stopped, reason {}", err]
                );

                task::Trigger::Error
            }
        }
    }
}

pub struct UdpSrc {
    src_pad: PadSrc,
    task: Task,
    configured_caps: Mutex<Option<gst::Caps>>,
    settings: Mutex<Settings>,
    state: Mutex<State>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "ts-udpsrc",
        gst::DebugColorFlags::empty(),
        Some("Thread-sharing UDP source"),
    )
});

impl UdpSrc {
    fn prepare(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp = self, "Preparing");

        let settings = self.settings.lock().unwrap();
        let context =
            Context::acquire(&settings.context, settings.context_wait).map_err(|err| {
                gst::error_msg!(
                    gst::ResourceError::OpenRead,
                    ["Failed to acquire Context: {}", err]
                )
            })?;
        drop(settings);

        let (sender, receiver) = channel(1);
        self.state.lock().unwrap().event_sender = Some(sender);

        *self.configured_caps.lock().unwrap() = None;
        self.task
            .prepare(UdpSrcTask::new(self.obj().clone(), receiver), context)
            .block_on_or_add_subtask_then(self.obj(), |elem, res| {
                if res.is_ok() {
                    gst::debug!(CAT, obj = elem, "Prepared");
                }
            })
    }

    fn unprepare(&self) {
        gst::debug!(CAT, imp = self, "Unpreparing");
        let _ = self
            .task
            .unprepare()
            .block_on_or_add_subtask_then(self.obj(), |elem, _| {
                gst::debug!(CAT, obj = elem, "Unprepared");
            });
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp = self, "Stopping");
        self.task
            .stop()
            .block_on_or_add_subtask_then(self.obj(), |elem, res| {
                if res.is_ok() {
                    gst::debug!(CAT, obj = elem, "Stopped");
                }
            })
    }

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp = self, "Starting");
        self.task
            .start()
            .block_on_or_add_subtask_then(self.obj(), |elem, res| {
                if res.is_ok() {
                    gst::debug!(CAT, obj = elem, "Started");
                }
            })
    }

    fn state(&self) -> TaskState {
        self.task.state()
    }
}

#[glib::object_subclass]
impl ObjectSubclass for UdpSrc {
    const NAME: &'static str = "GstTsUdpSrc";
    type Type = super::UdpSrc;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        Self {
            src_pad: PadSrc::new(
                gst::Pad::from_template(&klass.pad_template("src").unwrap()),
                UdpSrcPadHandler,
            ),
            task: Task::default(),
            configured_caps: Default::default(),
            settings: Default::default(),
            state: Default::default(),
        }
    }
}

impl ObjectImpl for UdpSrc {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            let mut properties = vec![
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
                glib::ParamSpecString::builder("address")
                    .nick("Address")
                    .blurb("Address/multicast group to listen on")
                    .default_value(DEFAULT_ADDRESS)
                    .build(),
                glib::ParamSpecInt::builder("port")
                    .nick("Port")
                    .blurb("Port to listen on")
                    .minimum(0)
                    .maximum(u16::MAX as i32)
                    .default_value(DEFAULT_PORT)
                    .build(),
                glib::ParamSpecBoolean::builder("reuse")
                    .nick("Reuse")
                    .blurb("Allow reuse of the port")
                    .default_value(DEFAULT_REUSE)
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Caps>("caps")
                    .nick("Caps")
                    .blurb("Caps to use")
                    .build(),
                glib::ParamSpecUInt::builder("mtu")
                    .nick("MTU")
                    .blurb("Maximum expected packet size. This directly defines the allocation size of the receive buffer pool")
                    .maximum(i32::MAX as u32)
                    .default_value(DEFAULT_MTU)
                    .build(),
                glib::ParamSpecBoolean::builder("retrieve-sender-address")
                    .nick("Retrieve sender address")
                    .blurb("Whether to retrieve the sender address and add it to buffers as meta. Disabling this might result in minor performance improvements in certain scenarios")
                    .default_value(DEFAULT_RETRIEVE_SENDER_ADDRESS)
                    .build(),
                glib::ParamSpecBoolean::builder("loop")
                    .nick("Loop")
                    .blurb("Set the multicast loop parameter")
                    .default_value(DEFAULT_MULTICAST_LOOP)
                    .build(),
                glib::ParamSpecUInt::builder("buffer-size")
                    .nick("Buffer Size")
                    .blurb("Size of the kernel receive buffer in bytes, 0=default")
                    .maximum(u32::MAX)
                    .default_value(DEFAULT_BUFFER_SIZE)
                    .build(),
                glib::ParamSpecString::builder("multicast-iface")
                    .nick("Multicast Interface")
                    .blurb("The network interface on which to join the multicast group. This allows multiple interfaces
                        separated by comma. (\"eth0,eth1\")")
                    .default_value(DEFAULT_MULTICAST_IFACE)
                    .build(),

            ];

            #[cfg(not(windows))]
            {
                properties.push(
                    glib::ParamSpecObject::builder::<gio::Socket>("socket")
                        .nick("Socket")
                        .blurb("Socket to use for UDP reception. (None == allocate)")
                        .build(),
                );
                properties.push(
                    glib::ParamSpecObject::builder::<gio::Socket>("used-socket")
                        .nick("Used Socket")
                        .blurb("Socket currently in use for UDP reception. (None = no socket)")
                        .read_only()
                        .build(),
                );
            }

            properties
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().unwrap();
        match pspec.name() {
            "address" => {
                settings.address = value.get().expect("type checked upstream");
            }
            "port" => {
                settings.port = value.get().expect("type checked upstream");
            }
            "reuse" => {
                settings.reuse = value.get().expect("type checked upstream");
            }
            "caps" => {
                settings.caps = value.get().expect("type checked upstream");
            }
            "mtu" => {
                settings.mtu = value.get().expect("type checked upstream");
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
            "retrieve-sender-address" => {
                settings.retrieve_sender_address = value.get().expect("type checked upstream");
            }
            "loop" => {
                settings.multicast_loop = value.get().expect("type checked upstream");
            }
            "buffer-size" => {
                settings.buffer_size = value.get().expect("type checked upstream");
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
            "address" => settings.address.to_value(),
            "port" => settings.port.to_value(),
            "reuse" => settings.reuse.to_value(),
            "caps" => settings.caps.to_value(),
            "mtu" => settings.mtu.to_value(),
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
            "context" => settings.context.to_value(),
            "context-wait" => (settings.context_wait.as_millis() as u32).to_value(),
            "retrieve-sender-address" => settings.retrieve_sender_address.to_value(),
            "loop" => settings.multicast_loop.to_value(),
            "buffer-size" => settings.buffer_size.to_value(),
            "multicast-iface" => settings.multicast_iface.to_value(),
            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(self.src_pad.gst_pad()).unwrap();
        obj.set_element_flags(gst::ElementFlags::SOURCE);
    }
}

impl GstObjectImpl for UdpSrc {}

impl ElementImpl for UdpSrc {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Thread-sharing UDP source",
                "Source/Network",
                "Receives data over the network via UDP",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::new_any();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            vec![src_pad_template]
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
            gst::StateChange::PausedToReady => {
                self.stop().map_err(|_| gst::StateChangeError)?;
            }
            gst::StateChange::ReadyToNull => {
                self.unprepare();
            }
            _ => (),
        }

        let mut success = self.parent_change_state(transition)?;

        match transition {
            gst::StateChange::ReadyToPaused => {
                success = gst::StateChangeSuccess::NoPreroll;
            }
            gst::StateChange::PausedToPlaying => {
                self.start().map_err(|_| gst::StateChangeError)?;
            }
            gst::StateChange::PlayingToPaused => {
                success = gst::StateChangeSuccess::NoPreroll;
            }
            _ => (),
        }

        Ok(success)
    }

    fn send_event(&self, event: gst::Event) -> bool {
        use gst::EventView;

        gst::debug!(CAT, imp = self, "Handling element level event {event:?}");

        match event.view() {
            EventView::Eos(_) => {
                if self.state() != TaskState::Started {
                    if let Err(err) = self.start() {
                        gst::error!(CAT, imp = self, "Failed to start task thread {err:?}");
                    }
                }

                if self.state() == TaskState::Started {
                    let mut state = self.state.lock().unwrap();

                    if let Some(event_tx) = state.event_sender.as_mut() {
                        return event_tx.try_send(event.clone()).is_ok();
                    }
                }

                false
            }
            _ => self.parent_send_event(event),
        }
    }
}
