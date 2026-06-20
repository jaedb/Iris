// GStreamer RTSP Source 2
//
// Copyright (C) 2023 Tim-Philipp Müller <tim centricular com>
// Copyright (C) 2023-2024 Nirbheek Chauhan <nirbheek centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0
//
// https://www.rfc-editor.org/rfc/rfc2326.html

use std::collections::{btree_set::BTreeSet, HashMap};
use std::convert::TryFrom;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Result;
use std::sync::LazyLock;

use futures::{Sink, SinkExt, Stream, StreamExt};
use socket2::Socket;
use tokio::net::{TcpStream, UdpSocket};
use tokio::runtime;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time;

use rtsp_types::headers::{
    CSeq, NptRange, NptTime, Public, Range, RtpInfos, RtpLowerTransport, RtpProfile, RtpTransport,
    RtpTransportParameters, Session, Transport, TransportMode, Transports, ACCEPT, CONTENT_BASE,
    CONTENT_LOCATION, USER_AGENT,
};
use rtsp_types::{Message, Method, Request, Response, StatusCode, Version};

use lru::LruCache;
use url::Url;

use gst::buffer::{MappedBuffer, Readable};
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_net::gio;

use super::body::Body;
use super::sdp;
use super::transport::RtspTransportInfo;

const DEFAULT_LOCATION: Option<Url> = None;
const DEFAULT_TIMEOUT: gst::ClockTime = gst::ClockTime::from_seconds(5);
const DEFAULT_PORT_START: u16 = 0;
// Priority list has multicast first, because we want to prefer multicast if it's available
const DEFAULT_PROTOCOLS: &str = "udp-mcast,udp,tcp";
// Equal to MTU + 8 by default to avoid incorrectly detecting an MTU sized buffer as having
// possibly overflown our receive buffer, and triggering a doubling of the buffer sizes.
const DEFAULT_RECEIVE_MTU: u32 = 1500 + 8;

const MAX_MESSAGE_SIZE: usize = 1024 * 1024;
const MAX_BIND_PORT_RETRY: u16 = 100;
const UDP_PACKET_MAX_SIZE: u32 = 65535 - 8;
const RTCP_ADDR_CACHE_SIZE: usize = 100;

static RTCP_CAPS: LazyLock<gst::Caps> =
    LazyLock::new(|| gst::Caps::from(gst::Structure::new_empty("application/x-rtcp")));

// Hardcoded for now
const DEFAULT_USER_AGENT: &str = concat!(
    "GStreamer rtspsrc2 ",
    env!("CARGO_PKG_VERSION"),
    "-",
    env!("COMMIT_ID")
);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RtspProtocol {
    UdpMulticast,
    Udp,
    Tcp,
}

impl fmt::Display for RtspProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            RtspProtocol::Udp => write!(f, "udp"),
            RtspProtocol::UdpMulticast => write!(f, "udp-mcast"),
            RtspProtocol::Tcp => write!(f, "tcp"),
        }
    }
}

#[derive(Debug, Clone)]
struct Settings {
    location: Option<Url>,
    port_start: u16,
    protocols: Vec<RtspProtocol>,
    timeout: gst::ClockTime,
    receive_mtu: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            location: DEFAULT_LOCATION,
            port_start: DEFAULT_PORT_START,
            timeout: DEFAULT_TIMEOUT,
            protocols: parse_protocols_str(DEFAULT_PROTOCOLS).unwrap(),
            receive_mtu: DEFAULT_RECEIVE_MTU,
        }
    }
}

#[derive(Debug)]
enum Commands {
    Play,
    //Pause,
    Teardown(Option<oneshot::Sender<()>>),
    Data(rtsp_types::Data<Body>),
}

#[derive(Debug, Default)]
pub struct RtspSrc {
    settings: Mutex<Settings>,
    task_handle: Mutex<Option<JoinHandle<()>>>,
    command_queue: Mutex<Option<mpsc::Sender<Commands>>>,
}

#[derive(thiserror::Error, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum RtspError {
    #[error("Generic I/O error")]
    IOGeneric(#[from] std::io::Error),
    #[error("Read I/O error")]
    Read(#[from] super::tcp_message::ReadError),
    #[error("RTSP header parse error")]
    HeaderParser(#[from] rtsp_types::headers::HeaderParseError),
    #[error("SDP parse error")]
    SDPParser(#[from] sdp_types::ParserError),
    #[error("Unexpected RTSP message: expected, received")]
    UnexpectedMessage(&'static str, rtsp_types::Message<Body>),
    #[error("Invalid RTSP message")]
    InvalidMessage(&'static str),
    #[error("Fatal error")]
    Fatal(String),
}

pub(crate) static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtspsrc2",
        gst::DebugColorFlags::empty(),
        Some("RTSP source"),
    )
});

static RUNTIME: LazyLock<runtime::Runtime> = LazyLock::new(|| {
    runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(1)
        .build()
        .unwrap()
});

fn parse_protocols_str(s: &str) -> Result<Vec<RtspProtocol>, glib::Error> {
    let mut acc = Vec::new();
    if s.is_empty() {
        return Err(glib::Error::new(
            gst::CoreError::Failed,
            "Protocols list is empty",
        ));
    }
    for each in s.split(',') {
        match each {
            "udp-mcast" => acc.push(RtspProtocol::UdpMulticast),
            "udp" => acc.push(RtspProtocol::Udp),
            "tcp" => acc.push(RtspProtocol::Tcp),
            _ => {
                return Err(glib::Error::new(
                    gst::CoreError::Failed,
                    &format!("Unsupported RTSP protocol: {each}"),
                ))
            }
        }
    }
    Ok(acc)
}

impl RtspSrc {
    fn set_location(&self, uri: Option<&str>) -> Result<(), glib::Error> {
        if self.obj().current_state() > gst::State::Ready {
            return Err(glib::Error::new(
                gst::URIError::BadState,
                "Changing the 'location' property on a started 'rtspsrc2' is not supported",
            ));
        }

        let mut settings = self.settings.lock().unwrap();

        let Some(uri) = uri else {
            settings.location = DEFAULT_LOCATION;
            return Ok(());
        };

        let uri = Url::parse(uri).map_err(|err| {
            glib::Error::new(
                gst::URIError::BadUri,
                &format!("Failed to parse URI '{uri}': {err:?}"),
            )
        })?;

        if uri.password().is_some() || !uri.username().is_empty() {
            // TODO
            gst::fixme!(CAT, "URI credentials are currently ignored");
        }

        match (uri.host_str(), uri.port()) {
            (Some(_), Some(_)) | (Some(_), None) => Ok(()),
            _ => Err(glib::Error::new(gst::URIError::BadUri, "Invalid host")),
        }?;

        let protocols: &[RtspProtocol] = match uri.scheme() {
            "rtspu" => &[RtspProtocol::UdpMulticast, RtspProtocol::Udp],
            "rtspt" => &[RtspProtocol::Tcp],
            "rtsp" => &settings.protocols,
            scheme => {
                return Err(glib::Error::new(
                    gst::URIError::UnsupportedProtocol,
                    &format!("Unsupported URI scheme '{scheme}'"),
                ));
            }
        };

        if !settings.protocols.iter().any(|p| protocols.contains(p)) {
            return Err(glib::Error::new(
                gst::URIError::UnsupportedProtocol,
                &format!(
                    "URI scheme '{}' does not match allowed protocols: {:?}",
                    uri.scheme(),
                    settings.protocols,
                ),
            ));
        }

        settings.protocols = protocols.to_vec();
        settings.location = Some(uri);

        Ok(())
    }

    fn set_protocols(&self, protocol_s: Option<&str>) -> Result<(), glib::Error> {
        if self.obj().current_state() > gst::State::Ready {
            return Err(glib::Error::new(
                gst::CoreError::Failed,
                "Changing the 'protocols' property on a started 'rtspsrc2' is not supported",
            ));
        }

        let mut settings = self.settings.lock().unwrap();

        settings.protocols = match protocol_s {
            Some(s) => parse_protocols_str(s)?,
            None => parse_protocols_str(DEFAULT_PROTOCOLS).unwrap(),
        };

        Ok(())
    }
}

impl ObjectImpl for RtspSrc {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecUInt::builder("receive-mtu")
                    .nick("Receive packet size")
                    .blurb("Initial size of buffers to allocate in the buffer pool, will be increased if too small")
                    .default_value(DEFAULT_RECEIVE_MTU)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("location")
                    .nick("Location")
                    .blurb("RTSP server, credentials and media path, e.g. rtsp://user:p4ssw0rd@camera-5.local:8554/h264_1080p30")
                    .mutable_ready()
                    .build(),
                // We purposely use port-start instead of port-range (like in rtspsrc), because
                // there is no way for the user to know how many ports we actually need. It depends
                // on how many streams the media contains, and whether the server wants RTCP or
                // RTCP-mux, or no RTCP. This property can be used to specify the start of the
                // valid range, and if the user wants to know how many ports were used, we can
                // add API for that later.
                glib::ParamSpecUInt::builder("port-start")
                    .nick("Port start")
                    .blurb("Port number to start allocating client ports for receiving RTP and RTCP data, eg. 3000 (0 = automatic selection)")
                    .default_value(DEFAULT_PORT_START.into())
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("protocols")
                    .nick("Protocols")
                    .blurb("Allowed lower transport protocols, in order of preference")
                    .default_value("udp-mcast,udp,tcp")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt64::builder("timeout")
                    .nick("Timeout")
                    .blurb("Timeout for network activity, in nanoseconds")
                    .maximum(gst::ClockTime::MAX.into())
                    .default_value(DEFAULT_TIMEOUT.into())
                    .mutable_ready()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }
    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let res = match pspec.name() {
            "receive-mtu" => {
                let mut settings = self.settings.lock().unwrap();
                settings.receive_mtu = value.get::<u32>().expect("type checked upstream");
                Ok(())
            }
            "location" => {
                let location = value.get::<Option<&str>>().expect("type checked upstream");
                self.set_location(location)
            }
            "port-start" => {
                let mut settings = self.settings.lock().unwrap();
                let start = value.get::<u32>().expect("type checked upstream");
                match u16::try_from(start) {
                    Ok(start) => {
                        settings.port_start = start;
                        Ok(())
                    }
                    Err(err) => Err(glib::Error::new(
                        gst::CoreError::Failed,
                        &format!("Failed to set port start: {err:?}"),
                    )),
                }
            }
            "protocols" => {
                let protocols = value.get::<Option<&str>>().expect("type checked upstream");
                self.set_protocols(protocols)
            }
            "timeout" => {
                let mut settings = self.settings.lock().unwrap();
                let timeout = value.get().expect("type checked upstream");
                settings.timeout = timeout;
                Ok(())
            }
            name => unimplemented!("Property '{name}'"),
        };

        if let Err(err) = res {
            gst::error!(
                CAT,
                imp = self,
                "Failed to set property `{}`: {:?}",
                pspec.name(),
                err
            );
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "receive-mtu" => {
                let settings = self.settings.lock().unwrap();
                settings.receive_mtu.to_value()
            }
            "location" => {
                let settings = self.settings.lock().unwrap();
                let location = settings.location.as_ref().map(Url::to_string);

                location.to_value()
            }
            "port-start" => {
                let settings = self.settings.lock().unwrap();
                (settings.port_start as u32).to_value()
            }
            "protocols" => {
                let settings = self.settings.lock().unwrap();
                (settings
                    .protocols
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","))
                .to_value()
            }
            "timeout" => {
                let settings = self.settings.lock().unwrap();
                settings.timeout.to_value()
            }
            name => unimplemented!("Property '{name}'"),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.set_suppressed_flags(gst::ElementFlags::SINK | gst::ElementFlags::SOURCE);
        obj.set_element_flags(gst::ElementFlags::SOURCE);
    }
}

impl GstObjectImpl for RtspSrc {}

impl ElementImpl for RtspSrc {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTSP Source",
                "Source/Network",
                "Receive audio or video from a network device via the Real Time Streaming Protocol (RTSP) (RFC 2326, 7826)",
                "Nirbheek Chauhan <nirbheek centricular com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let src_pad_template = gst::PadTemplate::new(
                "stream_%u",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &gst::Caps::new_empty_simple("application/x-rtp"),
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
        match transition {
            gst::StateChange::NullToReady => {
                self.start().map_err(|err_msg| {
                    self.post_error_message(err_msg);
                    gst::StateChangeError
                })?;
            }
            gst::StateChange::PausedToPlaying => {
                let cmd_queue = self.cmd_queue();
                //self.async_start().map_err(|_| gst::StateChangeError)?;
                RUNTIME.spawn(async move { cmd_queue.send(Commands::Play).await });
            }
            _ => {}
        }

        let mut ret = self.parent_change_state(transition)?;

        match transition {
            gst::StateChange::ReadyToPaused | gst::StateChange::PlayingToPaused => {
                ret = gst::StateChangeSuccess::NoPreroll;
            }
            gst::StateChange::PausedToReady => {
                match tokio::runtime::Handle::try_current() {
                    Ok(_) => {
                        // If the app does set_state(NULL) from a block_on() inside its own tokio
                        // runtime, calling block_on() on our own runtime will cause a panic
                        // because of nested blocking calls. So, shutdown the task from another
                        // thread.
                        // The app's usage is also incorrect since they are blocking the runtime
                        // on I/O, so emit a warning.
                        gst::warning!(
                            CAT,
                            "Blocking I/O: state change to NULL called from an async \
                            tokio context, redirecting to another thread to prevent \
                            the tokio panic, but you should refactor your code to \
                            make use of gst::Element::call_async and set the state \
                            to NULL from there, without blocking the runtime"
                        );
                        let (tx, rx) = std::sync::mpsc::channel();
                        self.obj().call_async(move |element| {
                            tx.send(element.imp().stop()).unwrap();
                        });
                        rx.recv().unwrap()
                    }
                    Err(_) => self.stop(),
                }
                .map_err(|err_msg| {
                    self.post_error_message(err_msg);
                    gst::StateChangeError
                })?;
            }
            _ => (),
        }

        Ok(ret)
    }
}

impl BinImpl for RtspSrc {}

impl URIHandlerImpl for RtspSrc {
    const URI_TYPE: gst::URIType = gst::URIType::Src;

    fn protocols() -> &'static [&'static str] {
        &["rtsp", "rtspu", "rtspt"]
    }

    fn uri(&self) -> Option<String> {
        let settings = self.settings.lock().unwrap();

        settings.location.as_ref().map(Url::to_string)
    }

    fn set_uri(&self, uri: &str) -> Result<(), glib::Error> {
        self.set_location(Some(uri))
    }
}

type RtspStream =
    Pin<Box<dyn Stream<Item = Result<Message<Body>, super::tcp_message::ReadError>> + Send>>;
type RtspSink = Pin<Box<dyn Sink<Message<Body>, Error = std::io::Error> + Send>>;

impl RtspSrc {
    #[track_caller]
    fn cmd_queue(&self) -> mpsc::Sender<Commands> {
        self.command_queue.lock().unwrap().as_ref().unwrap().clone()
    }

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        let Some(url) = self.settings.lock().unwrap().location.clone() else {
            return Err(gst::error_msg!(
                gst::ResourceError::Settings,
                ["No location set"]
            ));
        };

        gst::info!(CAT, imp = self, "Location: {url}",);

        gst::info!(CAT, imp = self, "Starting RTSP connection thread.. ");

        let task_src = self.ref_counted();

        let mut task_handle = self.task_handle.lock().unwrap();

        let (tx, rx) = mpsc::channel(1);
        {
            let mut cmd_queue_opt = self.command_queue.lock().unwrap();
            debug_assert!(cmd_queue_opt.is_none());
            cmd_queue_opt.replace(tx);
        }

        let join_handle = RUNTIME.spawn(async move {
            gst::info!(CAT, "Connecting to {url} ..");
            let hostname_port =
                format!("{}:{}", url.host_str().unwrap(), url.port().unwrap_or(554));

            // TODO: Add TLS support
            let s = match TcpStream::connect(hostname_port).await {
                Ok(s) => s,
                Err(err) => {
                    gst::element_imp_error!(
                        task_src,
                        gst::ResourceError::OpenRead,
                        ["Failed to connect to RTSP server: {err:#?}"]
                    );
                    return;
                }
            };
            let _ = s.set_nodelay(true);

            gst::info!(CAT, "Connected!");

            let (read, write) = s.into_split();

            let stream = Box::pin(super::tcp_message::async_read(read, MAX_MESSAGE_SIZE).fuse());
            let sink = Box::pin(super::tcp_message::async_write(write));

            let mut state = RtspTaskState::new(url, stream, sink);

            let task_ret = task_src.rtsp_task(&mut state, rx).await;
            gst::info!(CAT, "Exited rtsp_task");

            // Cleanup after stopping
            for h in &state.handles {
                h.abort();
            }
            for h in state.handles {
                let _ = h.await;
            }
            let obj = task_src.obj();
            for e in obj.iterate_sorted() {
                let Ok(e) = e else {
                    continue;
                };
                if let Err(err) = e.set_state(gst::State::Null) {
                    gst::warning!(CAT, "{} failed to go to Null state: {err:?}", e.name());
                }
            }
            for pad in obj.src_pads() {
                if let Err(err) = obj.remove_pad(&pad) {
                    gst::warning!(CAT, "Failed to remove pad {}: {err:?}", pad.name());
                }
            }
            for e in obj.iterate_sorted() {
                let Ok(e) = e else {
                    continue;
                };
                if let Err(err) = obj.remove(&e) {
                    gst::warning!(CAT, "Failed to remove element {}: {err:?}", e.name());
                }
            }

            // Post the element error after cleanup
            if let Err(err) = task_ret {
                gst::element_imp_error!(
                    task_src,
                    gst::CoreError::Failed,
                    ["RTSP task exited: {err:#?}"]
                );
            }
            gst::info!(CAT, "Cleanup complete");
        });

        debug_assert!(task_handle.is_none());
        task_handle.replace(join_handle);

        gst::info!(CAT, imp = self, "Started");

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        gst::info!(CAT, "Stopping...");
        let cmd_queue = self.cmd_queue();
        let task_handle = { self.task_handle.lock().unwrap().take() };

        RUNTIME.block_on(async {
            let (tx, rx) = oneshot::channel();
            if let Ok(()) = cmd_queue.send(Commands::Teardown(Some(tx))).await {
                if let Err(_elapsed) = time::timeout(Duration::from_millis(500), rx).await {
                    gst::warning!(
                        CAT,
                        "Timeout waiting for Teardown, going to NULL asynchronously"
                    );
                }
            }
        });

        if let Some(join_handle) = task_handle {
            gst::debug!(CAT, "Waiting for RTSP connection thread to shut down..");
            let _ = RUNTIME.block_on(join_handle);
        }

        self.command_queue.lock().unwrap().take();

        gst::info!(CAT, imp = self, "Stopped");

        Ok(())
    }

    fn make_rtp_appsrc(
        &self,
        rtpsession_n: usize,
        caps: &gst::Caps,
        manager: &RtspManager,
    ) -> Result<gst_app::AppSrc> {
        let callbacks = gst_app::AppSrcCallbacks::builder()
            .enough_data(|appsrc| {
                gst::warning!(CAT, "appsrc {} is overrunning: enough data!", appsrc.name());
            })
            .build();
        let appsrc = gst_app::AppSrc::builder()
            .name(format!("rtp_appsrc_{rtpsession_n}"))
            .format(gst::Format::Time)
            .handle_segment_change(true)
            .caps(caps)
            .stream_type(gst_app::AppStreamType::Stream)
            .max_bytes(0)
            .max_buffers(0)
            .max_time(gst::ClockTime::from_seconds(2))
            .leaky_type(gst_app::AppLeakyType::Downstream)
            .callbacks(callbacks)
            .is_live(true)
            .build();
        let obj = self.obj();
        obj.add(&appsrc)?;
        appsrc
            .static_pad("src")
            .unwrap()
            .link(&manager.rtp_recv_sinkpad(rtpsession_n).unwrap())?;
        let templ = obj.pad_template("stream_%u").unwrap();
        let ghostpad = gst::GhostPad::builder_from_template(&templ)
            .name(format!("stream_{rtpsession_n}"))
            .build();
        gst::info!(CAT, "Adding ghost srcpad {}", ghostpad.name());
        obj.add_pad(&ghostpad)
            .expect("Adding a ghostpad should never fail");
        appsrc.sync_state_with_parent()?;
        Ok(appsrc)
    }

    fn make_rtcp_appsrc(
        &self,
        rtpsession_n: usize,
        manager: &RtspManager,
    ) -> Result<gst_app::AppSrc> {
        let appsrc = gst_app::AppSrc::builder()
            .name(format!("rtcp_appsrc_{rtpsession_n}"))
            .format(gst::Format::Time)
            .handle_segment_change(true)
            .caps(&RTCP_CAPS)
            .stream_type(gst_app::AppStreamType::Stream)
            .is_live(true)
            .build();
        self.obj().add(&appsrc)?;
        appsrc
            .static_pad("src")
            .unwrap()
            .link(&manager.rtcp_recv_sinkpad(rtpsession_n).unwrap())?;
        appsrc.sync_state_with_parent()?;
        Ok(appsrc)
    }

    fn make_rtcp_appsink<
        F: FnMut(&gst_app::AppSink) -> Result<gst::FlowSuccess, gst::FlowError> + Send + 'static,
    >(
        &self,
        rtpsession_n: usize,
        manager: &RtspManager,
        on_rtcp: F,
    ) -> Result<()> {
        let cmd_tx_eos = self.cmd_queue();
        let cbs = gst_app::app_sink::AppSinkCallbacks::builder()
            .eos(move |_appsink| {
                let cmd_tx = cmd_tx_eos.clone();
                RUNTIME.spawn(async move {
                    let _ = cmd_tx.send(Commands::Teardown(None)).await;
                });
            })
            .new_sample(on_rtcp)
            .build();

        let rtcp_appsink = gst_app::AppSink::builder()
            .name(format!("rtcp_appsink_{rtpsession_n}"))
            .sync(false)
            .async_(false)
            .callbacks(cbs)
            .build();
        self.obj().add(&rtcp_appsink)?;
        manager
            .rtcp_send_srcpad(rtpsession_n)
            .unwrap()
            .link(&rtcp_appsink.static_pad("sink").unwrap())?;
        Ok(())
    }

    fn post_start(&self, code: &str, text: &str) {
        let obj = self.obj();
        let msg = gst::message::Progress::builder(gst::ProgressType::Start, code, text)
            .src(&*obj)
            .build();
        let _ = obj.post_message(msg);
    }

    fn post_complete(&self, code: &str, text: &str) {
        let obj = self.obj();
        let msg = gst::message::Progress::builder(gst::ProgressType::Complete, code, text)
            .src(&*obj)
            .build();
        let _ = obj.post_message(msg);
    }

    fn post_cancelled(&self, code: &str, text: &str) {
        let obj = self.obj();
        let msg = gst::message::Progress::builder(gst::ProgressType::Canceled, code, text)
            .src(&*obj)
            .build();
        let _ = obj.post_message(msg);
    }

    async fn rtsp_task(
        &self,
        state: &mut RtspTaskState,
        mut cmd_rx: mpsc::Receiver<Commands>,
    ) -> Result<()> {
        let cmd_tx = self.cmd_queue();

        let settings = { self.settings.lock().unwrap().clone() };

        // OPTIONS
        state.options().await?;

        // DESCRIBE
        state.describe().await?;

        let mut session: Option<Session> = None;
        // SETUP streams (TCP interleaved)
        state.setup_params = {
            state
                .setup(
                    &mut session,
                    settings.port_start,
                    &settings.protocols,
                    TransportMode::Play,
                )
                .await?
        };
        let manager = RtspManager::new(std::env::var("USE_RTP2").is_ok_and(|s| s == "1"));

        let obj = self.obj();
        manager
            .add_to(obj.upcast_ref::<gst::Bin>())
            .expect("Adding the manager cannot fail");

        let mut tcp_interleave_appsrcs = HashMap::new();
        for (rtpsession_n, p) in state.setup_params.iter_mut().enumerate() {
            let (tx, rx) = mpsc::channel(1);
            let on_rtcp = move |appsink: &_| on_rtcp_udp(appsink, tx.clone());
            match &mut p.transport {
                RtspTransportInfo::UdpMulticast {
                    dest,
                    port: (rtp_port, rtcp_port),
                    ttl,
                } => {
                    let rtp_socket = bind_port(*rtp_port, dest.is_ipv4())?;
                    let rtcp_socket = rtcp_port.and_then(|p| {
                        bind_port(p, dest.is_ipv4())
                            .map_err(|err| {
                                gst::warning!(CAT, "Could not bind to RTCP port: {err:?}");
                                err
                            })
                            .ok()
                    });

                    match &dest {
                        IpAddr::V4(addr) => {
                            rtp_socket.join_multicast_v4(*addr, Ipv4Addr::UNSPECIFIED)?;
                            if let Some(ttl) = ttl {
                                let _ = rtp_socket.set_multicast_ttl_v4(*ttl as u32);
                            }
                            let _ = rtp_socket.set_multicast_loop_v4(false);
                            if let Some(rtcp_socket) = &rtcp_socket {
                                if let Err(err) =
                                    rtcp_socket.join_multicast_v4(*addr, Ipv4Addr::UNSPECIFIED)
                                {
                                    gst::warning!(
                                        CAT,
                                        "Failed to join RTCP multicast address {addr}: {err:?}"
                                    );
                                } else {
                                    if let Some(ttl) = ttl {
                                        let _ = rtcp_socket.set_multicast_ttl_v4(*ttl as u32);
                                    }
                                    let _ = rtcp_socket.set_multicast_loop_v4(false);
                                }
                            }
                        }
                        IpAddr::V6(addr) => {
                            rtp_socket.join_multicast_v6(addr, 0)?;
                            let _ = rtp_socket.set_multicast_loop_v6(false);
                            if let Some(rtcp_socket) = &rtcp_socket {
                                if let Err(err) = rtcp_socket.join_multicast_v6(addr, 0) {
                                    gst::warning!(
                                        CAT,
                                        "Failed to join RTCP multicast address {addr}: {err:?}"
                                    );
                                } else {
                                    let _ = rtcp_socket.set_multicast_loop_v6(false);
                                }
                            }
                        }
                    };

                    let rtp_appsrc = self.make_rtp_appsrc(rtpsession_n, &p.caps, &manager)?;
                    p.rtp_appsrc = Some(rtp_appsrc.clone());
                    // Spawn RTP udp receive task
                    state.handles.push(RUNTIME.spawn(async move {
                        udp_rtp_task(
                            &rtp_socket,
                            rtp_appsrc,
                            settings.timeout,
                            settings.receive_mtu,
                            None,
                        )
                        .await
                    }));

                    // Spawn RTCP udp send/recv task
                    if let Some(rtcp_socket) = rtcp_socket {
                        let rtcp_dest = rtcp_port.and_then(|p| Some(SocketAddr::new(*dest, p)));
                        let rtcp_appsrc = self.make_rtcp_appsrc(rtpsession_n, &manager)?;
                        self.make_rtcp_appsink(rtpsession_n, &manager, on_rtcp)?;
                        state.handles.push(RUNTIME.spawn(async move {
                            udp_rtcp_task(&rtcp_socket, rtcp_appsrc, rtcp_dest, true, rx).await
                        }));
                    }
                }
                RtspTransportInfo::Udp {
                    source,
                    server_port,
                    client_port: _,
                    sockets,
                } => {
                    let Some((rtp_socket, rtcp_socket)) = sockets.take() else {
                        gst::warning!(
                            CAT,
                            "Skipping: no UDP sockets for {rtpsession_n}: {:#?}",
                            p.transport
                        );
                        continue;
                    };
                    let (rtp_sender_addr, rtcp_sender_addr) = match (source, server_port) {
                        (Some(ip), Some((rtp_port, Some(rtcp_port)))) => {
                            let ip = ip.parse().unwrap();
                            (
                                Some(SocketAddr::new(ip, *rtp_port)),
                                Some(SocketAddr::new(ip, *rtcp_port)),
                            )
                        }
                        (Some(ip), Some((rtp_port, None))) => {
                            (Some(SocketAddr::new(ip.parse().unwrap(), *rtp_port)), None)
                        }
                        _ => (None, None),
                    };

                    // Spawn RTP udp receive task
                    let rtp_appsrc = self.make_rtp_appsrc(rtpsession_n, &p.caps, &manager)?;
                    p.rtp_appsrc = Some(rtp_appsrc.clone());
                    state.handles.push(RUNTIME.spawn(async move {
                        udp_rtp_task(
                            &rtp_socket,
                            rtp_appsrc,
                            settings.timeout,
                            settings.receive_mtu,
                            rtp_sender_addr,
                        )
                        .await
                    }));

                    // Spawn RTCP udp send/recv task
                    if let Some(rtcp_socket) = rtcp_socket {
                        let rtcp_appsrc = self.make_rtcp_appsrc(rtpsession_n, &manager)?;
                        self.make_rtcp_appsink(rtpsession_n, &manager, on_rtcp)?;
                        state.handles.push(RUNTIME.spawn(async move {
                            udp_rtcp_task(&rtcp_socket, rtcp_appsrc, rtcp_sender_addr, false, rx)
                                .await
                        }));
                    }
                }
                RtspTransportInfo::Tcp {
                    channels: (rtp_channel, rtcp_channel),
                } => {
                    let rtp_appsrc = self.make_rtp_appsrc(rtpsession_n, &p.caps, &manager)?;
                    p.rtp_appsrc = Some(rtp_appsrc.clone());
                    tcp_interleave_appsrcs.insert(*rtp_channel, rtp_appsrc);

                    if let Some(rtcp_channel) = rtcp_channel {
                        // RTCP SR
                        let rtcp_appsrc = self.make_rtcp_appsrc(rtpsession_n, &manager)?;
                        tcp_interleave_appsrcs.insert(*rtcp_channel, rtcp_appsrc.clone());
                        // RTCP RR
                        let rtcp_channel = *rtcp_channel;
                        let cmd_tx = cmd_tx.clone();
                        self.make_rtcp_appsink(rtpsession_n, &manager, move |appsink| {
                            on_rtcp_tcp(appsink, cmd_tx.clone(), rtcp_channel)
                        })?;
                    }
                }
            }
        }

        obj.no_more_pads();

        // Expose RTP srcpads
        manager.recv.connect_pad_added(|manager, pad| {
            if pad.direction() != gst::PadDirection::Src {
                return;
            }
            let Some(obj) = manager
                .parent()
                .and_then(|o| o.downcast::<gst::Element>().ok())
            else {
                return;
            };
            let name = pad.name();
            match *name.split('_').collect::<Vec<_>>() {
                // rtpbin and rtp2
                ["recv", "rtp", "src", stream_id, ssrc, pt]
                | ["rtp", "src", stream_id, ssrc, pt] => {
                    if stream_id.parse::<u32>().is_err() {
                        gst::info!(CAT, "Ignoring srcpad with invalid stream id: {name}");
                        return;
                    };
                    gst::info!(CAT, "Setting rtpbin pad {} as ghostpad target", name);
                    let srcpad = obj
                        .static_pad(&format!("stream_{stream_id}"))
                        .expect("ghostpad should've been available already");
                    let ghostpad = srcpad
                        .downcast::<gst::GhostPad>()
                        .expect("rtspsrc src pads are ghost pads");
                    if let Err(err) = ghostpad.set_target(Some(pad)) {
                        gst::element_error!(
                            obj,
                            gst::ResourceError::Failed,
                            (
                                "Failed to set ghostpad {} target {}: {err:?}",
                                ghostpad.name(),
                                name
                            ),
                            ["pt: {pt}, ssrc: {ssrc}"]
                        );
                    }
                }
                _ => {
                    gst::info!(CAT, "Ignoring unknown srcpad: {name}");
                }
            }
        });

        let mut expected_response: Option<(Method, u32)> = None;
        loop {
            tokio::select! {
                msg = state.stream.next() => match msg {
                    Some(Ok(rtsp_types::Message::Data(data))) => {
                        let Some(appsrc) = tcp_interleave_appsrcs.get(&data.channel_id()) else {
                            gst::warning!(CAT,
                                "ignored data of size {}: unknown channel {}",
                                data.len(),
                                data.channel_id()
                            );
                            continue;
                        };
                        let t = appsrc.current_running_time();
                        let channel_id = data.channel_id();
                        gst::trace!(CAT, "Received data on channel {channel_id}");
                        // TODO: this should be from_mut_slice() after making the necessary
                        // modifications to Body
                        let mut buffer = gst::Buffer::from_slice(data.into_body());
                        let bufref = buffer.make_mut();
                        bufref.set_dts(t);
                        // TODO: Allow unlinked source pads
                        if let Err(err) = appsrc.push_buffer(buffer) {
                            gst::error!(CAT, "Failed to push buffer on pad {} for channel {}", appsrc.name(), channel_id);
                            return Err(err.into());
                        }
                    }
                    Some(Ok(rtsp_types::Message::Request(req))) => {
                        // TODO: implement incoming GET_PARAMETER requests
                        gst::debug!(CAT, "<-- {req:#?}");
                    }
                    Some(Ok(rtsp_types::Message::Response(rsp))) => {
                        gst::debug!(CAT, "<-- {rsp:#?}");
                        let Some((expected, cseq)) = &expected_response else {
                            continue;
                        };
                        let Some(s) = &session else {
                            return Err(RtspError::Fatal(format!("Can't handle {expected:?} response, no SETUP")).into());
                        };
                        match expected {
                            Method::Play => {
                                state.play_response(&rsp, *cseq, s).await?;
                                self.post_complete("request", "PLAY response received");
                            }
                            Method::Teardown => state.teardown_response(&rsp, *cseq, s).await?,
                            m => unreachable!("BUG: unexpected response method: {m:?}"),
                        };
                    }
                    Some(Err(e)) => {
                        // TODO: reconnect or ignore if UDP sockets are still receiving data
                        gst::error!(CAT, "I/O error: {e:?}, quitting");
                        return Err(gst::FlowError::Error.into());
                    }
                    None => {
                        // TODO: reconnect or ignore if UDP sockets are still receiving data
                        gst::error!(CAT, "TCP connection EOF, quitting");
                        return Err(gst::FlowError::Eos.into());
                    }
                },
                Some(cmd) = cmd_rx.recv() => match cmd {
                    Commands::Play => {
                        let Some(s) = &session else {
                            return Err(RtspError::InvalidMessage("Can't PLAY, no SETUP").into());
                        };
                        self.post_start("request", "PLAY request sent");
                        let cseq = state.play(s).await.inspect_err(|_err| {
                            self.post_cancelled("request", "PLAY request cancelled");
                        })?;
                        expected_response = Some((Method::Play, cseq));
                    },
                    Commands::Teardown(tx) => {
                        gst::info!(CAT, "Received Teardown command");
                        let Some(s) = &session else {
                            return Err(RtspError::InvalidMessage("Can't TEARDOWN, no SETUP").into());
                        };
                        let _ = state.teardown(s).await;
                        if let Some(tx) = tx {
                            let _ = tx.send(());
                        }
                        break;
                    }
                    Commands::Data(data) => {
                        // We currently only send RTCP RR as data messages, this will change when
                        // we support TCP ONVIF backchannels
                        state.sink.send(Message::Data(data)).await?;
                        gst::debug!(CAT, "Sent RTCP RR over TCP");
                    }
                },
                else => {
                    gst::error!(CAT, "No select statement matched, breaking loop");
                    break;
                }
            }
        }
        Ok(())
    }
}

struct RtspManager {
    recv: gst::Element,
    send: gst::Element,
    using_rtp2: bool,
}

impl RtspManager {
    fn new(rtp2: bool) -> Self {
        let (recv, send) = if rtp2 {
            let recv = gst::ElementFactory::make_with_name("rtprecv", None)
                .unwrap_or_else(|_| panic!("rtprecv not found"));
            let send = gst::ElementFactory::make("rtpsend")
                .property("rtp-id", recv.property::<String>("rtp-id"))
                .build()
                .unwrap_or_else(|_| panic!("rtpsend not found"));
            (recv, send)
        } else {
            let e = gst::ElementFactory::make_with_name("rtpbin", None)
                .unwrap_or_else(|_| panic!("rtpbin not found"));
            (e.clone(), e)
        };
        if !rtp2 {
            let on_bye = |args: &[glib::Value]| {
                let m = args[0].get::<gst::Element>().unwrap();
                let obj = m.parent()?;
                let bin = obj.downcast::<gst::Bin>().unwrap();
                bin.send_event(gst::event::Eos::new());
                None
            };
            recv.connect("on-bye-ssrc", true, move |args| {
                gst::info!(CAT, "Received BYE packet");
                on_bye(args)
            });
            recv.connect("on-bye-timeout", true, move |args| {
                gst::info!(CAT, "BYE due to timeout");
                on_bye(args)
            });
        }
        RtspManager {
            recv,
            send,
            using_rtp2: rtp2,
        }
    }

    fn rtp_recv_sinkpad(&self, rtpsession: usize) -> Option<gst::Pad> {
        let name = if self.using_rtp2 {
            format!("rtp_sink_{rtpsession}")
        } else {
            format!("recv_rtp_sink_{rtpsession}")
        };
        gst::info!(CAT, "requesting {name} for receiving RTP");
        self.recv.request_pad_simple(&name)
    }

    fn rtcp_recv_sinkpad(&self, rtpsession: usize) -> Option<gst::Pad> {
        let name = if self.using_rtp2 {
            format!("rtcp_sink_{rtpsession}")
        } else {
            format!("recv_rtcp_sink_{rtpsession}")
        };
        gst::info!(CAT, "requesting {name} for receiving RTCP");
        self.recv.request_pad_simple(&name)
    }

    fn rtcp_send_srcpad(&self, rtpsession: usize) -> Option<gst::Pad> {
        let name = if self.using_rtp2 {
            format!("rtcp_src_{rtpsession}")
        } else {
            format!("send_rtcp_src_{rtpsession}")
        };
        gst::info!(CAT, "requesting {name} for sending RTCP");
        self.send.request_pad_simple(&name)
    }

    fn add_to<T: IsA<gst::Bin>>(&self, bin: &T) -> Result<(), glib::BoolError> {
        if self.using_rtp2 {
            bin.add_many([&self.recv, &self.send])?;
            self.recv.sync_state_with_parent()?;
            self.send.sync_state_with_parent()?;
        } else {
            bin.add_many([&self.recv])?;
            self.recv.sync_state_with_parent()?;
        }
        Ok(())
    }
}

struct RtspTaskState {
    cseq: u32,
    url: Url,
    version: Version,
    content_base_or_location: Option<String>,
    aggregate_control: Option<Url>,
    sdp: Option<sdp_types::Session>,

    stream:
        Pin<Box<dyn Stream<Item = Result<Message<Body>, super::tcp_message::ReadError>> + Send>>,
    sink: Pin<Box<dyn Sink<Message<Body>, Error = std::io::Error> + Send>>,

    setup_params: Vec<RtspSetupParams>,
    handles: Vec<JoinHandle<()>>,
}

struct RtspSetupParams {
    control_url: Url,
    transport: RtspTransportInfo,
    rtp_appsrc: Option<gst_app::AppSrc>,
    caps: gst::Caps,
}

impl RtspTaskState {
    fn new(url: Url, stream: RtspStream, sink: RtspSink) -> Self {
        RtspTaskState {
            cseq: 0u32,
            url,
            version: Version::V1_0,
            content_base_or_location: None,
            aggregate_control: None,
            sdp: None,
            stream,
            sink,
            setup_params: Vec::new(),
            handles: Vec::new(),
        }
    }

    #[allow(clippy::result_large_err)]
    fn check_response(
        rsp: &Response<Body>,
        cseq: u32,
        req_name: Method,
        session: Option<&Session>,
    ) -> Result<(), RtspError> {
        if rsp.status() != StatusCode::Ok {
            return Err(RtspError::Fatal(format!(
                "{req_name:?} request failed: {}",
                rsp.reason_phrase()
            )));
        }
        match rsp.typed_header::<CSeq>() {
            Ok(Some(v)) => {
                if *v != cseq {
                    return Err(RtspError::InvalidMessage("cseq does not match"));
                }
            }
            Ok(None) => {
                gst::warning!(
                    CAT,
                    "No cseq in response, continuing... {:#?}",
                    rsp.headers().collect::<Vec<_>>()
                );
            }
            Err(_) => {
                gst::warning!(
                    CAT,
                    "Invalid cseq in response, continuing... {:#?}",
                    rsp.headers().collect::<Vec<_>>()
                );
            }
        };
        if let Some(s) = session {
            if let Some(have_s) = rsp.typed_header::<Session>()? {
                if s.0 != have_s.0 {
                    return Err(RtspError::Fatal(format!(
                        "Session in header {} does not match our session {}",
                        s.0, have_s.0
                    )));
                }
            } else {
                gst::warning!(
                    CAT,
                    "No Session header in response, continuing... {:#?}",
                    rsp.headers().collect::<Vec<_>>()
                );
            }
        }
        Ok(())
    }

    async fn options(&mut self) -> Result<(), RtspError> {
        self.cseq += 1;
        let req = Request::builder(Method::Options, self.version)
            .typed_header::<CSeq>(&self.cseq.into())
            .request_uri(self.url.clone())
            .header(USER_AGENT, DEFAULT_USER_AGENT)
            .build(Body::default());

        gst::debug!(CAT, "-->> {req:#?}");
        self.sink.send(req.into()).await?;

        let rsp = match self.stream.next().await {
            Some(Ok(rtsp_types::Message::Response(rsp))) => Ok(rsp),
            Some(Ok(m)) => Err(RtspError::UnexpectedMessage("OPTIONS response", m)),
            Some(Err(e)) => Err(e.into()),
            None => Err(
                std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "options response").into(),
            ),
        }?;
        gst::debug!(CAT, "<<-- {rsp:#?}");
        Self::check_response(&rsp, self.cseq, Method::Options, None)?;

        let Ok(Some(methods)) = rsp.typed_header::<Public>() else {
            return Err(RtspError::InvalidMessage(
                "OPTIONS response does not contain a valid Public header",
            ));
        };

        let needed = [
            Method::Describe,
            Method::Setup,
            Method::Play,
            Method::Teardown,
        ];
        let mut unsupported = Vec::new();
        for method in &needed {
            if !methods.contains(method) {
                unsupported.push(format!("{method:?}"));
            }
        }
        if !unsupported.is_empty() {
            Err(RtspError::Fatal(format!(
                "Server doesn't support the required method{} {}",
                if unsupported.len() == 1 { "" } else { "s:" },
                unsupported.join(",")
            )))
        } else {
            Ok(())
        }
    }

    async fn describe(&mut self) -> Result<(), RtspError> {
        self.cseq += 1;
        let req = Request::builder(Method::Describe, self.version)
            .typed_header::<CSeq>(&self.cseq.into())
            .header(USER_AGENT, DEFAULT_USER_AGENT)
            .header(ACCEPT, "application/sdp")
            .request_uri(self.url.clone())
            .build(Body::default());

        gst::debug!(CAT, "-->> {req:#?}");
        self.sink.send(req.into()).await?;

        let rsp = match self.stream.next().await {
            Some(Ok(rtsp_types::Message::Response(rsp))) => Ok(rsp),
            Some(Ok(m)) => Err(RtspError::UnexpectedMessage("DESCRIBE response", m)),
            Some(Err(e)) => Err(e.into()),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "describe response",
            )
            .into()),
        }?;
        gst::debug!(
            CAT,
            "<<-- Response {:#?}",
            rsp.headers().collect::<Vec<_>>()
        );
        Self::check_response(&rsp, self.cseq, Method::Describe, None)?;

        self.content_base_or_location = rsp
            .header(&CONTENT_BASE)
            .or(rsp.header(&CONTENT_LOCATION))
            .map(|v| v.to_string());

        gst::info!(CAT, "{}", std::str::from_utf8(rsp.body()).unwrap());
        // TODO: read range attribute from SDP for VOD use-cases
        let sdp = sdp_types::Session::parse(rsp.body())?;
        gst::debug!(CAT, "{sdp:#?}");

        self.sdp.replace(sdp);
        Ok(())
    }

    #[allow(clippy::result_large_err)]
    fn parse_setup_transports(
        transports: &Transports,
        s: &mut gst::Structure,
        protocols: &[RtspProtocol],
        mode: &TransportMode,
    ) -> Result<RtspTransportInfo, RtspError> {
        let mut last_error =
            RtspError::Fatal("No matching transport found matching selected protocols".to_string());
        let mut parsed_transports = Vec::new();
        for transport in transports.iter() {
            let Transport::Rtp(t) = transport else {
                last_error =
                    RtspError::Fatal(format!("Expected RTP transport, got {transports:#?}"));
                continue;
            };
            // RTSP 2 specifies that we can have multiple SSRCs in the response
            // Transport header, but it's not clear why, so we don't support it
            if let Some(ssrc) = t.params.ssrc.first() {
                s.set("ssrc", ssrc)
            }
            if !t.params.mode.is_empty() && !t.params.mode.contains(mode) {
                last_error = RtspError::Fatal(format!(
                    "Requested mode {:?} doesn't match server modes: {:?}",
                    mode, t.params.mode
                ));
                continue;
            }
            let parsed = match RtspTransportInfo::try_from(t) {
                Ok(p) => p,
                Err(err) => {
                    last_error = err;
                    continue;
                }
            };
            parsed_transports.push(parsed);
        }
        for protocol in protocols {
            for n in 0..parsed_transports.len() {
                if parsed_transports[n].to_protocol() == *protocol {
                    let t = parsed_transports.swap_remove(n);
                    return Ok(t);
                }
            }
        }
        Err(last_error)
    }

    async fn setup(
        &mut self,
        session: &mut Option<Session>,
        port_start: u16,
        protocols: &[RtspProtocol],
        mode: TransportMode,
    ) -> Result<Vec<RtspSetupParams>, RtspError> {
        let sdp = self.sdp.as_ref().expect("Must have SDP by now");
        let base = self
            .content_base_or_location
            .as_ref()
            .and_then(|s| Url::parse(s).ok())
            .unwrap_or_else(|| self.url.clone());
        self.aggregate_control = sdp
            .get_first_attribute_value("control")
            // No attribute and no value have the same meaning for us
            .ok()
            .flatten()
            .and_then(|v| sdp::parse_control_path(v, &base));
        let mut b = gst::Structure::builder("application/x-rtp");

        // TODO: parse range for VOD
        let skip_attrs = ["control", "range"];
        for sdp_types::Attribute { attribute, value } in &sdp.attributes {
            if skip_attrs.contains(&attribute.as_str()) {
                continue;
            }
            b = b.field(format!("a-{attribute}"), value);
        }
        // TODO: parse global extmap

        let message_structure = b.build();

        let conn_source = sdp
            .connection
            .as_ref()
            .map(|c| c.connection_address.as_str())
            .filter(|c| !c.is_empty())
            .unwrap_or_else(|| base.host_str().unwrap());
        let mut port_next = port_start;
        let mut stream_num = 0;
        let mut setup_params: Vec<RtspSetupParams> = Vec::new();
        for m in &sdp.medias {
            if !["audio", "video"].contains(&m.media.as_str()) {
                gst::info!(CAT, "Ignoring unsupported media {}", m.media);
                continue;
            }
            let media_control = m
                .get_first_attribute_value("control")
                // No attribute and no value have the same meaning for us
                .ok()
                .flatten()
                .and_then(|v| sdp::parse_control_path(v, &base));
            let Some(control_url) = media_control.as_ref().or(self.aggregate_control.as_ref())
            else {
                gst::warning!(
                    CAT,
                    "No session control or media control for {} fmt {}, ignoring",
                    m.media,
                    m.fmt
                );
                continue;
            };

            // RTP caps
            let Ok(pt) = m.fmt.parse::<u8>() else {
                gst::error!(CAT, "Could not parse pt: {}, ignoring media", m.fmt);
                continue;
            };

            let mut s = message_structure.clone();
            let media = m.media.to_ascii_lowercase();
            s.set("media", &media);
            s.set("payload", pt as i32);

            if let Err(err) = sdp::parse_media_attributes(&m.attributes, pt, &media, &mut s) {
                gst::warning!(
                    CAT,
                    "Skipping media {} {}, no rtpmap: {err:?}",
                    m.media,
                    m.fmt
                );
                continue;
            }

            // SETUP
            let mut rtp_socket: Option<UdpSocket> = None;
            let mut rtcp_socket: Option<UdpSocket> = None;
            let mut transports = Vec::new();
            let (conn_protocols, is_ipv4) = sdp::parse_connections(&m.connections);

            let protocols = if !conn_protocols.is_empty() {
                let p = protocols.iter().cloned().collect::<BTreeSet<_>>();
                p.intersection(&conn_protocols).cloned().collect::<Vec<_>>()
            } else {
                protocols.to_owned()
            };

            if protocols.is_empty() {
                gst::error!(CAT, "No available protocols left, skipping media");
                continue;
            }

            if protocols.contains(&RtspProtocol::UdpMulticast) {
                let params = RtpTransportParameters {
                    mode: vec![mode.clone()],
                    multicast: true,
                    ..Default::default()
                };
                transports.push(Transport::Rtp(RtpTransport {
                    profile: RtpProfile::Avp,
                    lower_transport: Some(RtpLowerTransport::Udp),
                    params,
                }));
            }
            if protocols.contains(&RtspProtocol::Udp) {
                let (sock1, rtp_port) = bind_start_port(port_next, is_ipv4).await;
                // Get the actual port that was successfully bound
                port_next = rtp_port;
                let (sock2, rtcp_port) = bind_start_port(rtp_port + 1, is_ipv4).await;
                rtp_socket = Some(sock1);
                rtcp_socket = Some(sock2);
                let params = RtpTransportParameters {
                    mode: vec![mode.clone()],
                    unicast: true,
                    client_port: Some((rtp_port, Some(rtcp_port))),
                    ..Default::default()
                };
                transports.push(Transport::Rtp(RtpTransport {
                    profile: RtpProfile::Avp,
                    lower_transport: Some(RtpLowerTransport::Udp),
                    params,
                }));
            }
            if protocols.contains(&RtspProtocol::Tcp) {
                let params = RtpTransportParameters {
                    mode: vec![mode.clone()],
                    interleaved: Some((stream_num, Some(stream_num + 1))),
                    ..Default::default()
                };
                transports.push(Transport::Rtp(RtpTransport {
                    // RTSP 2.0 adds AVPF and more
                    profile: RtpProfile::Avp,
                    lower_transport: Some(RtpLowerTransport::Tcp),
                    params,
                }));
            }

            self.cseq += 1;
            let transports: Transports = transports.as_slice().into();
            let req = Request::builder(Method::Setup, self.version)
                .typed_header::<CSeq>(&self.cseq.into())
                .header(USER_AGENT, DEFAULT_USER_AGENT)
                .typed_header::<Transports>(&transports)
                .request_uri(control_url.clone());
            let req = if let Some(s) = session {
                req.typed_header::<Session>(s)
            } else {
                req
            };
            let req = req.build(Body::default());
            let cseq = self.cseq;

            gst::debug!(CAT, "-->> {req:#?}");
            self.sink.send(req.into()).await?;

            // RTSP 2 supports pipelining of SETUP requests, so this ping-pong would have to be
            // reworked if we want to support it.
            let rsp = match self.stream.next().await {
                Some(Ok(rtsp_types::Message::Response(rsp))) => Ok(rsp),
                Some(Ok(m)) => Err(RtspError::UnexpectedMessage("SETUP response", m)),
                Some(Err(e)) => Err(e.into()),
                None => Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "setup response",
                )
                .into()),
            }?;
            gst::debug!(CAT, "<<-- {rsp:#?}");
            Self::check_response(&rsp, cseq, Method::Setup, session.as_ref())?;
            let new_session = rsp
                .typed_header::<Session>()?
                .ok_or(RtspError::InvalidMessage("No session in SETUP response"))?;
            // Manually strip timeout field: https://github.com/sdroege/rtsp-types/issues/24
            session.replace(Session(new_session.0, None));
            let mut parsed_transport = if let Some(transports) = rsp.typed_header::<Transports>()? {
                Self::parse_setup_transports(&transports, &mut s, &protocols, &mode)
            } else {
                // Transport header in response is optional if only one transport was offered
                // https://datatracker.ietf.org/doc/html/rfc2326#section-12.39
                if transports.len() == 1 {
                    Self::parse_setup_transports(&transports, &mut s, &protocols, &mode)
                } else {
                    Err(RtspError::InvalidMessage(
                        "No transport header in SETUP response",
                    ))
                }
            }?;
            match &mut parsed_transport {
                RtspTransportInfo::UdpMulticast { .. } => {}
                RtspTransportInfo::Udp {
                    source,
                    server_port: _,
                    client_port,
                    sockets,
                } => {
                    if source.is_none() {
                        *source = Some(conn_source.to_string());
                    }
                    if let Some((rtp_port, rtcp_port)) = client_port {
                        // There is no reason for the server to reject the client ports WE
                        // selected, so if it does, just ignore it.
                        if *rtp_port != port_next {
                            gst::warning!(
                                CAT,
                                "RTP port changed: {port_next} -> {rtp_port}, ignoring"
                            );
                            *rtp_port = port_next;
                        }
                        port_next += 1;
                        *sockets = if let Some(rtcp_port) = rtcp_port {
                            if *rtcp_port != port_next {
                                gst::warning!(
                                    CAT,
                                    "RTCP port changed: {port_next} -> {rtcp_port}, ignoring"
                                );
                                *rtcp_port = port_next;
                            }
                            port_next += 1;
                            Some((rtp_socket.unwrap(), rtcp_socket))
                        } else {
                            Some((rtp_socket.unwrap(), None))
                        }
                    };
                }
                RtspTransportInfo::Tcp {
                    channels: (rtp_ch, rtcp_ch),
                } => {
                    if *rtp_ch != stream_num {
                        gst::info!(CAT, "RTP channel changed: {stream_num} -> {rtp_ch}");
                    }
                    stream_num += 1;
                    if let Some(rtcp_ch) = rtcp_ch {
                        if *rtcp_ch != stream_num {
                            gst::info!(CAT, "RTCP channel changed: {stream_num} -> {rtcp_ch}");
                        }
                        stream_num += 1;
                    }
                }
            };
            let caps = gst::Caps::from(s);
            setup_params.push(RtspSetupParams {
                control_url: control_url.clone(),
                transport: parsed_transport,
                rtp_appsrc: None,
                caps,
            });
        }
        Ok(setup_params)
    }

    async fn play(&mut self, session: &Session) -> Result<u32, RtspError> {
        self.cseq += 1;
        let request_uri = self.aggregate_control.as_ref().unwrap_or(&self.url).clone();
        let req = Request::builder(Method::Play, self.version)
            .typed_header::<CSeq>(&self.cseq.into())
            .typed_header::<Range>(&Range::Npt(NptRange::From(NptTime::Now)))
            .header(USER_AGENT, DEFAULT_USER_AGENT)
            .request_uri(request_uri)
            .typed_header::<Session>(session);

        let req = req.build(Body::default());
        gst::debug!(CAT, "-->> {req:#?}");
        self.sink.send(req.into()).await?;
        Ok(self.cseq)
    }

    async fn play_response(
        &mut self,
        rsp: &Response<Body>,
        cseq: u32,
        session: &Session,
    ) -> Result<(), RtspError> {
        Self::check_response(rsp, cseq, Method::Play, Some(session))?;
        if let Some(RtpInfos::V1(rtpinfos)) = rsp.typed_header::<RtpInfos>()? {
            for rtpinfo in rtpinfos {
                for params in self.setup_params.iter_mut() {
                    if params.control_url == rtpinfo.uri {
                        let mut changed = false;
                        let mut caps = params.rtp_appsrc.as_ref().unwrap().caps().unwrap();
                        let capsref = caps.make_mut();
                        if let Some(v) = rtpinfo.seq {
                            capsref.set("seqnum-base", v as u32);
                            changed = true;
                        }
                        if let Some(v) = rtpinfo.rtptime {
                            capsref.set("clock-base", v);
                            changed = true;
                        }
                        if changed {
                            params.rtp_appsrc.as_ref().unwrap().set_caps(Some(&caps));
                        }
                    }
                }
            }
        } else {
            gst::warning!(CAT, "No RTPInfos V1 header in PLAY response");
        };
        Ok(())
    }

    async fn teardown(&mut self, session: &Session) -> Result<u32, RtspError> {
        self.cseq += 1;
        let request_uri = self.aggregate_control.as_ref().unwrap_or(&self.url).clone();
        let req = Request::builder(Method::Teardown, self.version)
            .typed_header::<CSeq>(&self.cseq.into())
            .header(USER_AGENT, DEFAULT_USER_AGENT)
            .request_uri(request_uri)
            .typed_header::<Session>(session);

        let req = req.build(Body::default());
        gst::debug!(CAT, "-->> {req:#?}");
        self.sink.send(req.into()).await?;
        Ok(self.cseq)
    }

    async fn teardown_response(
        &mut self,
        rsp: &Response<Body>,
        cseq: u32,
        session: &Session,
    ) -> Result<(), RtspError> {
        Self::check_response(rsp, cseq, Method::Teardown, Some(session))?;
        Ok(())
    }
}

fn bind_port(port: u16, is_ipv4: bool) -> Result<UdpSocket, std::io::Error> {
    let domain = if is_ipv4 {
        socket2::Domain::IPV4
    } else {
        socket2::Domain::IPV6
    };
    let sock = Socket::new(domain, socket2::Type::DGRAM, Some(socket2::Protocol::UDP))?;
    let _ = sock.set_reuse_address(true);
    #[cfg(unix)]
    let _ = sock.set_reuse_port(true);
    sock.set_nonblocking(true)?;
    let addr: SocketAddr = if is_ipv4 {
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port))
    } else {
        SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, port, 0, 0))
    };
    sock.bind(&addr.into())?;
    let bound_port = if is_ipv4 {
        sock.local_addr()?.as_socket_ipv4().unwrap().port()
    } else {
        sock.local_addr()?.as_socket_ipv6().unwrap().port()
    };
    gst::debug!(CAT, "Bound to UDP port {bound_port}");

    UdpSocket::from_std(sock.into())
}

async fn bind_start_port(port: u16, is_ipv4: bool) -> (UdpSocket, u16) {
    let mut next_port = port;
    loop {
        match bind_port(next_port, is_ipv4) {
            Ok(socket) => {
                if next_port != 0 {
                    return (socket, next_port);
                }
                let addr = socket
                    .local_addr()
                    .expect("Newly-bound port should not fail");
                return (socket, addr.port());
            }
            Err(err) => {
                gst::debug!(CAT, "Failed to bind to {next_port}: {err:?}, trying next");
                next_port += 1;
                // If we fail too much, panic instead of forever doing a hot-loop
                if (next_port - MAX_BIND_PORT_RETRY) > port {
                    panic!("Failed to allocate any ports from {port} to {next_port}");
                }
            }
        };
    }
}

fn on_rtcp_udp(
    appsink: &gst_app::AppSink,
    tx: mpsc::Sender<MappedBuffer<Readable>>,
) -> Result<gst::FlowSuccess, gst::FlowError> {
    let Ok(sample) = appsink.pull_sample() else {
        return Err(gst::FlowError::Error);
    };
    let Some(buffer) = sample.buffer_owned() else {
        return Ok(gst::FlowSuccess::Ok);
    };
    let map = buffer.into_mapped_buffer_readable();
    match map {
        Ok(map) => match tx.try_send(map) {
            Ok(_) => Ok(gst::FlowSuccess::Ok),
            Err(mpsc::error::TrySendError::Full(_)) => {
                gst::error!(CAT, "Could not send RTCP, channel is full");
                Err(gst::FlowError::Error)
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Err(gst::FlowError::Eos),
        },
        Err(err) => {
            gst::error!(CAT, "Failed to map buffer: {err:?}");
            Err(gst::FlowError::Error)
        }
    }
}

fn on_rtcp_tcp(
    appsink: &gst_app::AppSink,
    cmd_tx: mpsc::Sender<Commands>,
    rtcp_channel: u8,
) -> Result<gst::FlowSuccess, gst::FlowError> {
    let Ok(sample) = appsink.pull_sample() else {
        return Err(gst::FlowError::Error);
    };
    let Some(buffer) = sample.buffer_owned() else {
        return Ok(gst::FlowSuccess::Ok);
    };
    let map = buffer.into_mapped_buffer_readable();
    match map {
        Ok(map) => {
            let data: rtsp_types::Data<Body> =
                rtsp_types::Data::new(rtcp_channel, Body::mapped(map));
            let cmd_tx = cmd_tx.clone();
            RUNTIME.spawn(async move { cmd_tx.send(Commands::Data(data)).await });
            Ok(gst::FlowSuccess::Ok)
        }
        Err(err) => {
            gst::error!(CAT, "Failed to map buffer: {err:?}");
            Err(gst::FlowError::Error)
        }
    }
}

async fn udp_rtp_task(
    socket: &UdpSocket,
    appsrc: gst_app::AppSrc,
    timeout: gst::ClockTime,
    receive_mtu: u32,
    sender_addr: Option<SocketAddr>,
) {
    let t = Duration::from_secs(timeout.into());
    let sender_addr = match sender_addr {
        Some(addr) => addr,
        // Server didn't give us a Transport header or its Transport header didn't specify the
        // server port, so we don't know the sender port from which we will get data till we get
        // the first packet here.
        None => {
            let ret = match time::timeout(t, socket.peek_sender()).await {
                Ok(Ok(addr)) => Ok(addr),
                Ok(Err(_elapsed)) => Err(format!(
                    "No data after {} seconds, exiting",
                    timeout.seconds()
                )),
                Err(err) => Err(format!("UDP socket was closed: {err:?}")),
            };
            match ret {
                Ok(addr) => addr,
                Err(err) => {
                    gst::element_error!(
                        appsrc,
                        gst::ResourceError::Failed,
                        ("{}", err),
                        ["{:#?}", socket]
                    );
                    return;
                }
            }
        }
    };
    gst::info!(CAT, "Receiving from address {sender_addr:?}");
    let gio_addr = {
        let inet_addr: gio::InetAddress = sender_addr.ip().into();
        gio::InetSocketAddress::new(&inet_addr, sender_addr.port())
    };
    let mut size = receive_mtu;
    let caps = appsrc.caps();
    let mut pool = gst::BufferPool::new();
    let mut config = pool.config();
    config.set_params(caps.as_ref(), size, 2, 0);
    pool.set_config(config).unwrap();
    pool.set_active(true).unwrap();
    let error = loop {
        let Ok(buffer) = pool.acquire_buffer(None) else {
            break "Failed to acquire buffer".to_string();
        };
        let Ok(mut map) = buffer.into_mapped_buffer_writable() else {
            break "Failed to map buffer writable".to_string();
        };
        match time::timeout(t, socket.recv_from(map.as_mut_slice())).await {
            Ok(Ok((len, addr))) => {
                // Ignore packets from the wrong sender
                if addr != sender_addr {
                    continue;
                }
                if size < UDP_PACKET_MAX_SIZE && len == size as usize {
                    gst::warning!(
                        CAT,
                        "Data maybe lost: UDP buffer size {size} filled, doubling"
                    );
                    size = (size * 2).min(UDP_PACKET_MAX_SIZE);
                    if let Err(err) = pool.set_active(false) {
                        break format!("Failed to deactivate buffer pool: {err:?}");
                    }
                    pool = gst::BufferPool::new();
                    let mut config = pool.config();
                    config.set_params(caps.as_ref(), size, 2, 0);
                    pool.set_config(config).unwrap();
                    if let Err(err) = pool.set_active(true) {
                        break format!("Failed to reallocate buffer pool: {err:?}");
                    }
                }
                let t = appsrc.current_running_time();
                let mut buffer = map.into_buffer();
                let bufref = buffer.make_mut();
                bufref.set_size(len);
                bufref.set_dts(t);
                gst_net::NetAddressMeta::add(bufref, &gio_addr);
                gst::trace!(CAT, "received RTP packet from {addr:?}");
                if let Err(err) = appsrc.push_buffer(buffer) {
                    break format!("UDP buffer push failed: {err:?}");
                }
            }
            Ok(Err(_elapsed)) => {
                break format!("No data after {} seconds, exiting", timeout.seconds())
            }
            Err(err) => break format!("UDP socket was closed: {err:?}"),
        };
    };
    gst::element_error!(
        appsrc,
        gst::ResourceError::Failed,
        ("{}", error),
        ["{:#?}", socket]
    );
}

async fn udp_rtcp_task(
    socket: &UdpSocket,
    appsrc: gst_app::AppSrc,
    mut sender_addr: Option<SocketAddr>,
    is_multicast: bool,
    mut rx: mpsc::Receiver<MappedBuffer<Readable>>,
) {
    let mut buf = vec![0; UDP_PACKET_MAX_SIZE as usize];
    let mut cache: LruCache<_, _> = LruCache::new(NonZeroUsize::new(RTCP_ADDR_CACHE_SIZE).unwrap());
    let error = loop {
        tokio::select! {
            send_rtcp = rx.recv() => match send_rtcp {
                // The server either didn't specify a server_port for RTCP, or if the server didn't
                // send a Transport header in the SETUP response at all.
                Some(data) => if let Some(addr) = sender_addr.as_ref() {
                    match socket.send_to(data.as_ref(), addr).await {
                        Ok(_) => gst::debug!(CAT, "Sent RTCP RR packet"),
                        Err(err) => {
                            rx.close();
                            break format!("RTCP send error: {err:?}, stopping task");
                        }
                    }
                } else {
                    gst::warning!(CAT, "Can't send RTCP yet: don't have dest addr");
                },
                None => {
                    rx.close();
                    break format!("UDP socket {socket:?} closed, no more RTCP will be sent");
                }
            },
            recv_rtcp = socket.recv_from(&mut buf) => match recv_rtcp {
                Ok((len, addr)) => {
                    gst::debug!(CAT, "Received RTCP packet");
                    if let Some(sender_addr) = sender_addr {
                        // Ignore RTCP from the wrong sender
                        if !is_multicast && addr != sender_addr {
                            continue;
                        }
                    } else {
                        sender_addr.replace(addr);
                        gst::info!(CAT, "Delayed RTCP UDP send address: {addr:?}");
                    };
                    let t = appsrc.current_running_time();
                    let mut buffer = gst::Buffer::from_slice(buf[..len].to_owned());
                    let bufref = buffer.make_mut();
                    bufref.set_dts(t);
                    let gio_addr = cache.get_or_insert(addr, || {
                        let inet_addr: gio::InetAddress = addr.ip().into();
                        gio::InetSocketAddress::new(&inet_addr, addr.port())
                    });
                    gst_net::NetAddressMeta::add(bufref, gio_addr);
                    if let Err(err) = appsrc.push_buffer(buffer) {
                        break format!("UDP buffer push failed: {err:?}");
                    }
                }
                Err(err) => break format!("UDP socket was closed: {err:?}"),
            },
        }
    };
    gst::element_error!(
        appsrc,
        gst::ResourceError::Failed,
        ("{}", error),
        ["{:#?}", socket]
    );
}

#[glib::object_subclass]
impl ObjectSubclass for RtspSrc {
    const NAME: &'static str = "GstRtspSrc2";
    type Type = super::RtspSrc;
    type ParentType = gst::Bin;
    type Interfaces = (gst::URIHandler,);
}
