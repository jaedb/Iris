// Copyright (C) 2024, Asymptotic Inc.
//      Author: Sanchayan Maity <sanchayan@asymptotic.io>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use crate::quinnconnection::*;
use crate::quinnquicmeta::QuinnQuicMeta;
use crate::quinnquicquery::*;
use crate::utils::{
    client_endpoint, get_stats, make_socket_addr, server_endpoint, wait, Canceller, WaitError,
    CONNECTION_CLOSE_CODE, CONNECTION_CLOSE_MSG, RUNTIME,
};
use crate::{common::*, utils};
use async_channel::{unbounded, Receiver, Sender};
use bytes::Bytes;
use futures::{future::BoxFuture, stream::FuturesUnordered, StreamExt};
use gst::{glib, prelude::*, subclass::prelude::*};
use gst_base::prelude::*;
use gst_base::subclass::base_src::CreateSuccess;
use gst_base::subclass::prelude::*;
use quinn::{Chunk, Connection, ConnectionError, ReadError, RecvStream, TransportConfig, VarInt};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};
use std::thread::{spawn, Builder, JoinHandle};
use tokio::sync::oneshot;

const DEFAULT_ROLE: QuinnQuicRole = QuinnQuicRole::Server;
const DEFAULT_USE_DATAGRAM: bool = false;
const DATA_HANDLER_THREAD: &str = "data-handler";

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "quinnquicsrc",
        gst::DebugColorFlags::empty(),
        Some("Quinn QUIC Source"),
    )
});

enum QuinnData {
    Datagram(Bytes),
    Stream(u64, Bytes),
    Closed(u64),
    Eos,
}

struct Started {
    connection: Connection,
    data_handler: Option<JoinHandle<()>>,
    // TODO: Use tokio channel
    //
    // We use async-channel to keep a clone of the receive channel around
    // for use in every `create` call. tokio's UnboundedReceiver does not
    // implement clone.
    data_rx: Option<Receiver<QuinnData>>,
    thread_quit: Option<oneshot::Sender<()>>,
}

#[derive(Default)]
enum State {
    #[default]
    Stopped,
    Started(Started),
}

#[derive(Debug)]
struct Settings {
    address: String,
    port: u16,
    server_name: String,
    bind_address: String,
    bind_port: u16,
    alpns: Vec<String>,
    role: QuinnQuicRole,
    timeout: u32,
    keep_alive_interval: u64,
    secure_conn: bool,
    caps: gst::Caps,
    use_datagram: bool,
    certificate_file: Option<PathBuf>,
    private_key_file: Option<PathBuf>,
    transport_config: QuinnQuicTransportConfig,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            address: DEFAULT_ADDR.to_string(),
            port: DEFAULT_PORT,
            server_name: DEFAULT_SERVER_NAME.to_string(),
            bind_address: DEFAULT_BIND_ADDR.to_string(),
            bind_port: DEFAULT_BIND_PORT,
            alpns: vec![DEFAULT_ALPN.to_string()],
            role: DEFAULT_ROLE,
            timeout: DEFAULT_TIMEOUT,
            keep_alive_interval: 0,
            secure_conn: DEFAULT_SECURE_CONNECTION,
            caps: gst::Caps::new_any(),
            use_datagram: DEFAULT_USE_DATAGRAM,
            certificate_file: None,
            private_key_file: None,
            transport_config: QuinnQuicTransportConfig::default(),
        }
    }
}

pub struct QuinnQuicSrc {
    settings: Mutex<Settings>,
    state: Mutex<State>,
    canceller: Mutex<utils::Canceller>,
    connection: Mutex<Option<Connection>>,
}

impl Default for QuinnQuicSrc {
    fn default() -> Self {
        Self {
            settings: Mutex::new(Settings::default()),
            state: Mutex::new(State::default()),
            canceller: Mutex::new(utils::Canceller::default()),
            connection: Mutex::new(None),
        }
    }
}

impl GstObjectImpl for QuinnQuicSrc {}

impl ElementImpl for QuinnQuicSrc {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Quinn QUIC Source",
                "Source/Network/QUIC",
                "Receive data over the network via QUIC",
                "Sanchayan Maity <sanchayan@asymptotic.io>",
            )
        });
        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::new_any(),
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
        if transition == gst::StateChange::NullToReady {
            let settings = self.settings.lock().unwrap();

            /*
             * Fail the state change if a secure connection was requested but
             * no certificate path was provided.
             */
            if settings.secure_conn
                && (settings.certificate_file.is_none() || settings.private_key_file.is_none())
            {
                gst::error!(
                    CAT,
                    imp = self,
                    "Certificate or private key file not provided for secure connection"
                );
                return Err(gst::StateChangeError);
            }
        }

        self.parent_change_state(transition)
    }
}

impl ObjectImpl for QuinnQuicSrc {
    fn constructed(&self) {
        self.parent_constructed();
        self.obj().set_format(gst::Format::Time);
        self.obj().set_do_timestamp(true);
        self.obj().set_live(true);
    }

    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecString::builder("server-name")
                    .nick("QUIC server name")
                    .blurb("Name of the QUIC server which is in server certificate in case of server role")
                    .build(),
                glib::ParamSpecString::builder("address")
                    .nick("QUIC server address")
                    .blurb("Address of the QUIC server e.g. 127.0.0.1")
                    .build(),
                glib::ParamSpecUInt::builder("port")
                    .nick("QUIC server port")
                    .blurb("Port of the QUIC server e.g. 5000")
                    .maximum(65535)
                    .default_value(DEFAULT_PORT as u32)
                    .readwrite()
                    .build(),
                glib::ParamSpecString::builder("bind-address")
                    .nick("QUIC client bind address")
                    .blurb("Address to bind QUIC client e.g. 0.0.0.0")
                    .build(),
                glib::ParamSpecUInt::builder("bind-port")
                    .nick("QUIC client port")
                    .blurb("Port to bind QUIC client e.g. 5001")
                    .maximum(65535)
                    .default_value(DEFAULT_BIND_PORT as u32)
                    .readwrite()
                    .build(),
		gst::ParamSpecArray::builder("alpn-protocols")
                    .nick("QUIC ALPN values")
                    .blurb("QUIC connection Application-Layer Protocol Negotiation (ALPN) values")
                    .element_spec(&glib::ParamSpecString::builder("alpn-protocol").build())
                    .build(),
		glib::ParamSpecEnum::builder_with_default("role", DEFAULT_ROLE)
                    .nick("QUIC role")
                    .blurb("QUIC connection role to use.")
		    .build(),
                glib::ParamSpecUInt::builder("timeout")
                    .nick("Timeout")
                    .blurb("Value in seconds to timeout QUIC endpoint requests (0 = No timeout).")
                    .maximum(3600)
                    .default_value(DEFAULT_TIMEOUT)
                    .readwrite()
                    .build(),
		glib::ParamSpecUInt64::builder("keep-alive-interval")
                    .nick("QUIC connection keep alive interval in ms")
                    .blurb("Keeps QUIC connection alive by periodically pinging the server. Value set in ms, 0 disables this feature")
		    .default_value(0)
                    .readwrite()
                    .build(),
                glib::ParamSpecBoolean::builder("secure-connection")
                    .nick("Use secure connection")
                    .blurb("Use certificates for QUIC connection. False: Insecure connection, True: Secure connection.")
                    .default_value(DEFAULT_SECURE_CONNECTION)
                    .build(),
                glib::ParamSpecString::builder("certificate-file")
                    .nick("Certificate file")
                    .blurb("Path to certificate chain in single file")
                    .build(),
                glib::ParamSpecString::builder("private-key-file")
                    .nick("Private key file")
                    .blurb("Path to a PKCS8 or RSA private key file")
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Caps>("caps")
                    .nick("caps")
                    .blurb("The caps of the source pad")
                    .build(),
                glib::ParamSpecBoolean::builder("use-datagram")
                    .nick("Use datagram")
                    .blurb("Use datagram for lower latency, unreliable messaging")
                    .default_value(DEFAULT_USE_DATAGRAM)
                    .build(),
                glib::ParamSpecUInt::builder("initial-mtu")
                    .nick("Initial MTU")
                    .blurb("Initial value to be used as the maximum UDP payload size")
                    .minimum(DEFAULT_INITIAL_MTU.into())
                    .default_value(DEFAULT_INITIAL_MTU.into())
                    .build(),
                glib::ParamSpecUInt::builder("min-mtu")
                    .nick("Minimum MTU")
                    .blurb("Maximum UDP payload size guaranteed to be supported by the network, must be <= initial-mtu")
                    .minimum(DEFAULT_MINIMUM_MTU.into())
                    .default_value(DEFAULT_MINIMUM_MTU.into())
                    .build(),
                glib::ParamSpecUInt::builder("upper-bound-mtu")
                    .nick("Upper bound MTU")
                    .blurb("Upper bound to the max UDP payload size that MTU discovery will search for")
                    .minimum(DEFAULT_UPPER_BOUND_MTU.into())
                    .maximum(DEFAULT_MAX_UPPER_BOUND_MTU.into())
                    .default_value(DEFAULT_UPPER_BOUND_MTU.into())
                    .build(),
                glib::ParamSpecUInt::builder("max-udp-payload-size")
                    .nick("Maximum UDP payload size")
                    .blurb("Maximum UDP payload size accepted from peers (excluding UDP and IP overhead)")
                    .minimum(DEFAULT_MIN_UDP_PAYLOAD_SIZE.into())
                    .maximum(DEFAULT_MAX_UDP_PAYLOAD_SIZE.into())
                    .default_value(DEFAULT_UDP_PAYLOAD_SIZE.into())
                    .build(),
                glib::ParamSpecUInt64::builder("datagram-receive-buffer-size")
                    .nick("Datagram Receiver Buffer Size")
                    .blurb("Maximum number of incoming application datagram bytes to buffer")
                    .build(),
                glib::ParamSpecUInt64::builder("datagram-send-buffer-size")
                    .nick("Datagram Send Buffer Size")
                    .blurb("Maximum number of outgoing application datagram bytes to buffer")
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Structure>("stats")
                    .nick("Connection statistics")
                    .blurb("Connection statistics")
                    .read_only()
                    .build(),
		glib::ParamSpecUInt64::builder("max-concurrent-uni-streams")
                    .nick("Maximum concurrent uni-directional streams")
                    .blurb("Maximum number of incoming unidirectional streams that may be open concurrently")
		    .default_value(DEFAULT_MAX_CONCURRENT_UNI_STREAMS.into())
                    .readwrite()
                    .build(),
		glib::ParamSpecUInt64::builder("receive-window")
                    .nick("Receive Window")
                    .blurb("Maximum number of bytes the peer may transmit across all streams of a connection before becoming blocked")
                    .maximum(VarInt::MAX.into())
                    .readwrite()
                    .build(),
		glib::ParamSpecUInt64::builder("stream-receive-window")
                    .nick("Stream Receive Window")
                    .blurb("Maximum number of bytes the peer may transmit without ACK on any one stream before becoming blocked")
                    .maximum(VarInt::MAX.into())
                    .readwrite()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().unwrap();

        match pspec.name() {
            "server-name" => {
                settings.server_name = value.get::<String>().expect("type checked upstream");
            }
            "address" => {
                settings.address = value.get::<String>().expect("type checked upstream");
            }
            "port" => {
                settings.port = value.get::<u32>().expect("type checked upstream") as u16;
            }
            "bind-address" => {
                settings.bind_address = value.get::<String>().expect("type checked upstream");
            }
            "bind-port" => {
                settings.bind_port = value.get::<u32>().expect("type checked upstream") as u16;
            }
            "alpn-protocols" => {
                settings.alpns = value
                    .get::<gst::ArrayRef>()
                    .expect("type checked upstream")
                    .as_slice()
                    .iter()
                    .map(|alpn| {
                        alpn.get::<&str>()
                            .expect("type checked upstream")
                            .to_string()
                    })
                    .collect::<Vec<String>>()
            }
            "role" => {
                settings.role = value.get::<QuinnQuicRole>().expect("type checked upstream");
            }
            "caps" => {
                settings.caps = value
                    .get::<Option<gst::Caps>>()
                    .expect("type checked upstream")
                    .unwrap_or_else(gst::Caps::new_any);

                let srcpad = self.obj().static_pad("src").expect("source pad expected");
                srcpad.mark_reconfigure();
            }
            "timeout" => {
                settings.timeout = value.get().expect("type checked upstream");
            }
            "keep-alive-interval" => {
                settings.keep_alive_interval = value.get().expect("type checked upstream");
            }
            "secure-connection" => {
                settings.secure_conn = value.get().expect("type checked upstream");
            }
            "certificate-file" => {
                let value: String = value.get().unwrap();
                settings.certificate_file = Some(value.into());
            }
            "private-key-file" => {
                let value: String = value.get().unwrap();
                settings.private_key_file = Some(value.into());
            }
            "use-datagram" => {
                settings.use_datagram = value.get().expect("type checked upstream");
            }
            "initial-mtu" => {
                let value = value.get::<u32>().expect("type checked upstream");
                settings.transport_config.initial_mtu =
                    value.max(DEFAULT_INITIAL_MTU.into()) as u16;
            }
            "min-mtu" => {
                let value = value.get::<u32>().expect("type checked upstream");
                let initial_mtu = settings.transport_config.initial_mtu;
                settings.transport_config.min_mtu = value.min(initial_mtu.into()) as u16;
            }
            "upper-bound-mtu" => {
                let value = value.get::<u32>().expect("type checked upstream");
                settings.transport_config.upper_bound_mtu = value as u16;
            }
            "max-udp-payload-size" => {
                let value = value.get::<u32>().expect("type checked upstream");
                settings.transport_config.max_udp_payload_size = value as u16;
            }
            "datagram-receive-buffer-size" => {
                let value = value.get::<u64>().expect("type checked upstream");
                settings.transport_config.datagram_receive_buffer_size = value as usize;
            }
            "datagram-send-buffer-size" => {
                let value = value.get::<u64>().expect("type checked upstream");
                settings.transport_config.datagram_send_buffer_size = value as usize;
            }
            "max-concurrent-uni-streams" => {
                let value = value.get::<u64>().expect("type checked upstream");
                settings.transport_config.max_concurrent_uni_streams =
                    VarInt::from_u64(value.max(VarInt::MAX.into())).unwrap();
            }
            "receive-window" => {
                let value = value.get::<u64>().expect("type checked upstream");
                settings.transport_config.receive_window =
                    VarInt::from_u64(value.max(VarInt::MAX.into())).unwrap();
            }
            "stream-receive-window" => {
                let value = value.get::<u64>().expect("type checked upstream");
                settings.transport_config.stream_receive_window =
                    VarInt::from_u64(value.max(VarInt::MAX.into())).unwrap();
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().unwrap();

        match pspec.name() {
            "server-name" => settings.server_name.to_value(),
            "address" => settings.address.to_string().to_value(),
            "port" => {
                let port = settings.port as u32;
                port.to_value()
            }
            "bind-address" => settings.bind_address.to_string().to_value(),
            "bind-port" => {
                let port = settings.bind_port as u32;
                port.to_value()
            }
            "alpn-protocols" => {
                let alpns = settings.alpns.iter().map(|v| v.as_str());
                gst::Array::new(alpns).to_value()
            }
            "role" => settings.role.to_value(),
            "caps" => settings.caps.to_value(),
            "timeout" => settings.timeout.to_value(),
            "keep-alive-interval" => settings.keep_alive_interval.to_value(),
            "secure-connection" => settings.secure_conn.to_value(),
            "certificate-file" => {
                let certfile = settings.certificate_file.as_ref();
                certfile.and_then(|file| file.to_str()).to_value()
            }
            "private-key-file" => {
                let privkey = settings.private_key_file.as_ref();
                privkey.and_then(|file| file.to_str()).to_value()
            }
            "use-datagram" => settings.use_datagram.to_value(),
            "initial-mtu" => (settings.transport_config.initial_mtu as u32).to_value(),
            "min-mtu" => (settings.transport_config.min_mtu as u32).to_value(),
            "upper-bound-mtu" => (settings.transport_config.upper_bound_mtu as u32).to_value(),
            "max-udp-payload-size" => {
                (settings.transport_config.max_udp_payload_size as u32).to_value()
            }
            "datagram-receive-buffer-size" => {
                (settings.transport_config.datagram_receive_buffer_size as u64).to_value()
            }
            "datagram-send-buffer-size" => {
                (settings.transport_config.datagram_send_buffer_size as u64).to_value()
            }
            "stats" => {
                let state = self.state.lock().unwrap();
                match *state {
                    State::Started(ref state) => {
                        let connection = state.connection.clone();
                        get_stats(Some(connection.stats())).to_value()
                    }
                    State::Stopped => get_stats(None).to_value(),
                }
            }
            "max-concurrent-uni-streams" => {
                u64::from(settings.transport_config.max_concurrent_uni_streams).to_value()
            }
            "receive-window" => u64::from(settings.transport_config.receive_window).to_value(),
            "stream-receive-window" => {
                u64::from(settings.transport_config.stream_receive_window).to_value()
            }
            _ => unimplemented!(),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for QuinnQuicSrc {
    const NAME: &'static str = "GstQuinnQuicSrc";
    type Type = super::QuinnQuicSrc;
    type ParentType = gst_base::PushSrc;
}

impl BaseSrcImpl for QuinnQuicSrc {
    fn start(&self) -> Result<(), gst::ErrorMessage> {
        let state = self.state.lock().unwrap();
        if let State::Started { .. } = *state {
            unreachable!("QuicSrc already started");
        }
        drop(state);

        let conn_guard = self.connection.lock().unwrap();
        let connection = match *conn_guard {
            Some(ref c) => {
                // We will end up here if downstream MoQ demuxer
                // requested for a Connection before we could
                // set up.
                gst::info!(
                    CAT,
                    imp = self,
                    "Using existing connection with ID: {}",
                    c.stable_id()
                );
                c.clone()
            }
            None => self.setup_connection()?,
        };
        drop(conn_guard);

        let (tx_quit, rx_quit): (oneshot::Sender<()>, oneshot::Receiver<()>) = oneshot::channel();
        let (data_tx, data_rx): (Sender<QuinnData>, Receiver<QuinnData>) = unbounded();

        let conn_clone = connection.clone();
        let self_ = self.ref_counted();
        let data_handler = Builder::new()
            .name(DATA_HANDLER_THREAD.to_string())
            .spawn(move || {
                self_.handle_data(conn_clone, data_tx, rx_quit);
                gst::debug!(CAT, imp = self_, "Data handler thread exit");
            })
            .unwrap();

        let mut state = self.state.lock().unwrap();
        *state = State::Started(Started {
            connection,
            data_handler: Some(data_handler),
            data_rx: Some(data_rx),
            thread_quit: Some(tx_quit),
        });
        drop(state);

        gst::info!(CAT, imp = self, "Started");

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        gst::info!(CAT, imp = self, "Stopping");

        let mut state = self.state.lock().unwrap();

        if let State::Started(ref mut state) = *state {
            if let Some(channel) = state.thread_quit.take() {
                gst::debug!(CAT, imp = self, "Signalling threads to exit");
                let _ = channel.send(());
            }

            gst::debug!(CAT, imp = self, "Joining data handler thread");
            if let Some(handle) = state.data_handler.take() {
                match handle.join() {
                    Ok(_) => gst::debug!(CAT, imp = self, "Joined data handler thread"),
                    Err(e) => {
                        gst::error!(CAT, imp = self, "Failed to join data handler thread: {e:?}")
                    }
                }
            }

            state.connection.close(
                CONNECTION_CLOSE_CODE.into(),
                CONNECTION_CLOSE_MSG.as_bytes(),
            );
        }

        *state = State::Stopped;

        *self.connection.lock().unwrap() = None;

        gst::info!(CAT, imp = self, "Stopped");

        Ok(())
    }

    fn unlock(&self) -> Result<(), gst::ErrorMessage> {
        let mut canceller = self.canceller.lock().unwrap();
        canceller.abort();
        Ok(())
    }

    fn unlock_stop(&self) -> Result<(), gst::ErrorMessage> {
        let mut canceller = self.canceller.lock().unwrap();
        if matches!(&*canceller, Canceller::Cancelled) {
            *canceller = Canceller::None;
        }
        Ok(())
    }

    fn caps(&self, filter: Option<&gst::Caps>) -> Option<gst::Caps> {
        let settings = self.settings.lock().unwrap();

        let mut tmp_caps = settings.caps.clone();

        gst::debug!(CAT, imp = self, "Advertising our own caps: {:?}", &tmp_caps);

        if let Some(filter_caps) = filter {
            gst::debug!(
                CAT,
                imp = self,
                "Intersecting with filter caps: {:?}",
                &filter_caps
            );

            tmp_caps = filter_caps.intersect_with_mode(&tmp_caps, gst::CapsIntersectMode::First);
        };

        gst::debug!(CAT, imp = self, "Returning caps: {:?}", &tmp_caps);

        Some(tmp_caps)
    }

    fn is_seekable(&self) -> bool {
        false
    }

    fn query(&self, query: &mut gst::QueryRef) -> bool {
        use gst::QueryViewMut;

        match query.view_mut() {
            QueryViewMut::Context(c) => {
                gst::debug!(CAT, imp = self, "Handling Context query");

                if let Some(context) = self.set_connection_context() {
                    gst::info!(CAT, imp = self, "Setting Quinn Connection Context");
                    c.set_context(&context);
                    return true;
                }

                match self.setup_connection() {
                    Ok(connection) => {
                        let mut conn_guard = self.connection.lock().unwrap();
                        *conn_guard = Some(connection);
                        drop(conn_guard);

                        match self.set_connection_context() {
                            Some(context) => {
                                gst::info!(CAT, imp = self, "Setting Quinn Connection Context");
                                c.set_context(&context);
                                true
                            }
                            None => {
                                gst::error!(CAT, imp = self, "Failed to set context");
                                false
                            }
                        }
                    }
                    Err(e) => {
                        gst::error!(CAT, imp = self, "Failed to setup connection, {e:?}");
                        false
                    }
                }
            }
            _ => BaseSrcImplExt::parent_query(self, query),
        }
    }
}

impl PushSrcImpl for QuinnQuicSrc {
    fn create(
        &self,
        _buffer: Option<&mut gst::BufferRef>,
    ) -> Result<CreateSuccess, gst::FlowError> {
        loop {
            // We do not want `create` to return when a stream is closed,
            // but, wait for one of the other streams to receive data.
            match self.get() {
                Ok(Some(QuinnData::Stream(stream_id, bytes))) => {
                    break Ok(self.create_buffer(bytes, Some(stream_id)));
                }
                Ok(Some(QuinnData::Datagram(bytes))) => {
                    break Ok(self.create_buffer(bytes, None));
                }
                Ok(Some(QuinnData::Eos)) => {
                    gst::debug!(CAT, imp = self, "End of stream");
                    break Err(gst::FlowError::Eos);
                }
                Ok(None) => {
                    gst::debug!(CAT, imp = self, "End of stream");
                    break Err(gst::FlowError::Eos);
                }
                Err(None) => {
                    gst::debug!(CAT, imp = self, "Flushing");
                    break Err(gst::FlowError::Flushing);
                }
                Err(Some(err)) => {
                    gst::error!(CAT, imp = self, "Could not GET: {}", err);
                    break Err(gst::FlowError::Error);
                }
                Ok(Some(QuinnData::Closed(stream_id))) => {
                    // Send custom downstream event for demuxer to close
                    // and remove the stream.
                    let srcpad = self.obj().static_pad("src").expect("source pad expected");
                    close_stream(&srcpad, stream_id);
                }
            }
        }
    }
}

impl QuinnQuicSrc {
    fn create_buffer(&self, bytes: Bytes, stream_id: Option<u64>) -> CreateSuccess {
        gst::trace!(
            CAT,
            imp = self,
            "Pushing buffer of {} bytes for stream: {stream_id:?}",
            bytes.len()
        );

        let mut buffer = gst::Buffer::from_slice(bytes);
        {
            let buffer = buffer.get_mut().unwrap();
            match stream_id {
                Some(id) => QuinnQuicMeta::add(buffer, id, false),
                None => QuinnQuicMeta::add(buffer, 0, true),
            };
        }

        CreateSuccess::NewBuffer(buffer.to_owned())
    }

    fn get(&self) -> Result<Option<QuinnData>, Option<gst::ErrorMessage>> {
        let settings = self.settings.lock().unwrap();
        let timeout = settings.timeout;
        drop(settings);

        let state = self.state.lock().unwrap();
        let rx_chan = match *state {
            State::Started(ref started) => started.data_rx.clone(),
            State::Stopped => {
                return Err(Some(gst::error_msg!(
                    gst::LibraryError::Failed,
                    ["Cannot get data before start"]
                )));
            }
        };
        drop(state);

        let rx_chan = rx_chan.expect("Channel must be valid here");

        match wait(&self.canceller, rx_chan.recv(), timeout) {
            Ok(Ok(bytes)) => Ok(Some(bytes)),
            Ok(Err(_)) => Ok(None),
            Err(e) => match e {
                WaitError::FutureAborted => {
                    gst::warning!(CAT, imp = self, "Read from stream request aborted");
                    Ok(None)
                }
                WaitError::FutureError(e) => {
                    gst::error!(CAT, imp = self, "Failed to read from stream: {}", e);
                    Err(Some(e))
                }
            },
        }
    }

    fn get_role_and_epconfig(
        &self,
    ) -> Result<(QuinnQuicRole, QuinnQuicEndpointConfig), gst::ErrorMessage> {
        let settings = self.settings.lock().unwrap();

        let client_addr =
            make_socket_addr(format!("{}:{}", settings.bind_address, settings.bind_port).as_str())?;

        let server_addr =
            make_socket_addr(format!("{}:{}", settings.address, settings.port).as_str())?;

        let server_name = settings.server_name.clone();
        let alpns = settings.alpns.clone();
        let role = settings.role;
        let keep_alive_interval = settings.keep_alive_interval;
        let secure_conn = settings.secure_conn;
        let certificate_file = settings.certificate_file.clone();
        let private_key_file = settings.private_key_file.clone();
        let transport_config = settings.transport_config;

        Ok((
            role,
            QuinnQuicEndpointConfig {
                server_addr,
                server_name,
                client_addr: Some(client_addr),
                secure_conn,
                alpns,
                certificate_file,
                private_key_file,
                keep_alive_interval,
                transport_config,
                with_client_auth: false,
                webtransport: false,
            },
        ))
    }

    fn handle_connection_error(&self, connection_err: ConnectionError) {
        match connection_err {
            ConnectionError::ConnectionClosed(cc) => {
                gst::info!(CAT, imp = self, "Closed connection, {cc}");
            }
            ConnectionError::ApplicationClosed(ac) => {
                gst::info!(CAT, imp = self, "Application closed connection, {ac}");
            }
            ConnectionError::LocallyClosed => {
                gst::info!(CAT, imp = self, "Connection locally closed");
            }
            ConnectionError::VersionMismatch => {
                gst::error!(CAT, imp = self, "Version Mismatch");
            }
            ConnectionError::TransportError(terr) => {
                gst::error!(CAT, imp = self, "Transport error {terr:?}");
            }
            ConnectionError::Reset => {
                gst::error!(CAT, imp = self, "Connection Reset");
            }
            ConnectionError::TimedOut => {
                gst::error!(CAT, imp = self, "Connection Timedout");
            }
            ConnectionError::CidsExhausted => {
                gst::error!(CAT, imp = self, "Cids Exhausted");
            }
        }
    }

    fn handle_data(
        &self,
        connection: Connection,
        sender: Sender<QuinnData>,
        receiver: oneshot::Receiver<()>,
    ) {
        // Unifies the Future return types
        enum QuinnFuture {
            Datagram(Bytes),
            StreamData(RecvStream, QuinnData),
            Stream(RecvStream),
            Stop,
        }

        let blocksize = self.obj().blocksize() as usize;
        gst::info!(CAT, imp = self, "Using a blocksize of {blocksize} for read",);

        let incoming_stream = |conn: Connection| async move {
            match conn.accept_uni().await {
                Ok(recv_stream) => QuinnFuture::Stream(recv_stream),
                Err(err) => {
                    self.handle_connection_error(err);
                    QuinnFuture::Stop
                }
            }
        };

        let datagram = |conn: Connection| async move {
            match conn.read_datagram().await {
                Ok(bytes) => QuinnFuture::Datagram(bytes),
                Err(err) => {
                    self.handle_connection_error(err);
                    QuinnFuture::Stop
                }
            }
        };

        let recv_stream = |mut s: RecvStream| async move {
            let stream_id = s.id().index();
            match s.read_chunk(blocksize, true).await {
                Ok(Some(chunk)) => {
                    QuinnFuture::StreamData(s, QuinnData::Stream(stream_id, chunk.bytes))
                }
                Ok(None) => QuinnFuture::StreamData(s, QuinnData::Closed(stream_id)),
                Err(err) => match err {
                    ReadError::ClosedStream => {
                        gst::debug!(CAT, "Stream closed: {stream_id}");
                        QuinnFuture::StreamData(s, QuinnData::Closed(stream_id))
                    }
                    ReadError::ConnectionLost(err) => {
                        gst::error!(CAT, "Connection lost: {err:?}");
                        QuinnFuture::StreamData(s, QuinnData::Eos)
                    }
                    rerr => {
                        gst::error!(CAT, "Read error on stream {stream_id}: {rerr:?}");
                        QuinnFuture::StreamData(s, QuinnData::Eos)
                    }
                },
            }
        };

        let tx_send = |sender: Sender<QuinnData>, data: QuinnData| async move {
            if let Err(err) = sender.send(data).await {
                gst::error!(CAT, imp = self, "Error sending data: {err:?}");
            }
        };

        // TODO:
        // Decide if the ordering matters when we might have a STREAM
        // Close followed by a Connection Close almost immediately.
        let mut tasks: FuturesUnordered<BoxFuture<QuinnFuture>> = FuturesUnordered::new();

        tasks.push(Box::pin(datagram(connection.clone())));
        tasks.push(Box::pin(incoming_stream(connection.clone())));
        // We only ever expect to receive on this channel once, so we
        // need not push this in the loop below.
        tasks.push(Box::pin(async {
            let _ = receiver.await;
            gst::debug!(CAT, imp = self, "Quitting");
            QuinnFuture::Stop
        }));

        RUNTIME.block_on(async {
            while let Some(stream) = tasks.next().await {
                match stream {
                    QuinnFuture::Stop => {
                        tx_send(sender.clone(), QuinnData::Eos).await;
                        break;
                    }
                    QuinnFuture::StreamData(s, data) => match data {
                        d @ QuinnData::Stream(stream_id, _) => {
                            gst::trace!(CAT, imp = self, "Sending data for stream: {stream_id}");
                            tx_send(sender.clone(), d).await;
                            tasks.push(Box::pin(recv_stream(s)));
                        }
                        eos @ QuinnData::Eos => {
                            tx_send(sender.clone(), eos).await;
                            drop(s);
                            break;
                        }
                        c @ QuinnData::Closed(stream_id) => {
                            gst::trace!(CAT, imp = self, "Stream closed: {stream_id}");
                            tx_send(sender.clone(), c).await;
                            drop(s);
                        }
                        QuinnData::Datagram(_) => unreachable!(),
                    },
                    QuinnFuture::Stream(s) => {
                        gst::trace!(CAT, imp = self, "Incoming stream connection {:?}", s.id());
                        tasks.push(Box::pin(recv_stream(s)));
                        tasks.push(Box::pin(incoming_stream(connection.clone())));
                    }
                    QuinnFuture::Datagram(b) => {
                        gst::trace!(CAT, imp = self, "Received {} bytes on datagram", b.len());
                        tx_send(sender.clone(), QuinnData::Datagram(b)).await;
                        tasks.push(Box::pin(datagram(connection.clone())));
                    }
                }
            }
        });

        gst::info!(CAT, imp = self, "Quit data handler thread");
    }

    fn setup_connection(&self) -> Result<Connection, gst::ErrorMessage> {
        let settings = self.settings.lock().unwrap();
        let timeout = settings.timeout;
        drop(settings);

        let (role, endpoint_config) = self.get_role_and_epconfig()?;
        let addr = match role {
            QuinnQuicRole::Client => endpoint_config.server_addr,
            QuinnQuicRole::Server => endpoint_config
                .client_addr
                .expect("Client address should be valid"),
        };
        let mut shared_connection = SharedConnection::get_or_init(addr);

        shared_connection.set_endpoint_config(endpoint_config);

        let connection = match shared_connection.connection() {
            Some(c) => match c {
                QuinnConnection::Quic(conn) => {
                    gst::info!(
                        CAT,
                        imp = self,
                        "Using existing connection with ID: {}",
                        conn.stable_id()
                    );

                    conn
                }
                QuinnConnection::WebTransport(_) => unreachable!(),
            },
            None => match wait(
                &self.canceller,
                shared_connection.connect(role, None),
                timeout,
            ) {
                Ok(Ok(_)) => {
                    let c = shared_connection
                        .connection()
                        .expect("Connection should be valid here");
                    match c {
                        QuinnConnection::WebTransport(_) => unreachable!(),
                        QuinnConnection::Quic(conn) => {
                            gst::info!(
                                CAT,
                                imp = self,
                                "Using existing connection with ID: {}",
                                conn.stable_id()
                            );

                            conn
                        }
                    }
                }
                Ok(Err(e)) | Err(e) => match e {
                    WaitError::FutureAborted => {
                        gst::warning!(CAT, imp = self, "Connection aborted");
                        return Err(gst::error_msg!(
                            gst::ResourceError::Failed,
                            ["Connection request aborted"]
                        ));
                    }
                    WaitError::FutureError(err) => {
                        gst::error!(CAT, imp = self, "Connection request failed: {}", err);
                        return Err(gst::error_msg!(
                            gst::ResourceError::Failed,
                            ["Connection request failed: {err}"]
                        ));
                    }
                },
            },
        };

        gst::info!(
            CAT,
            imp = self,
            "QUIC connection established with ID: {}",
            connection.stable_id()
        );

        Ok(connection)
    }

    fn set_connection_context(&self) -> Option<gst::Context> {
        let conn_guard = self.connection.lock().unwrap();

        if let Some(ref connection) = *conn_guard {
            gst::debug!(CAT, imp = self, "Using already established Connection");

            let conn = QuinnConnectionContext(Arc::new(QuinnConnectionContextInner {
                connection: QuinnConnection::Quic(connection.clone()),
            }));

            let mut context = gst::Context::new(QUINN_CONNECTION_CONTEXT, true);
            {
                let context = context.get_mut().unwrap();
                let s = context.structure_mut();
                s.set("connection", conn);
            };

            self.obj().set_context(&context);

            let _ = self.obj().post_message(
                gst::message::HaveContext::builder(context.clone())
                    .src(&*self.obj())
                    .build(),
            );

            return Some(context.to_owned());
        }

        None
    }
}
