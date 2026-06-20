// Copyright (C) 2024, Fluendo, SA
//      Author: Ruben González <rgonzalez@fluendo.com>
//      Author: Andoni Morales <amorales@fluendo.com>
//
// Copyright (C) 2024, Asymptotic Inc.
//      Author: Sanchayan Maity <sanchayan@asymptotic.io>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use crate::quinnconnection::*;
use crate::quinnquicmeta::*;
use crate::quinnquicquery::*;
use crate::utils::{
    client_endpoint, get_stats, make_socket_addr, server_endpoint, wait, WaitError,
    CONNECTION_CLOSE_CODE, CONNECTION_CLOSE_MSG,
};
use crate::{common::*, utils};
use bytes::Bytes;
use futures::future;
use gst::{glib, prelude::*, subclass::prelude::*};
use gst_base::subclass::prelude::*;
use quinn::{Connection, TransportConfig};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::{LazyLock, Mutex};
use tokio::net::lookup_host;
use web_transport_quinn::{Request, SendStream, Session};

const DEFAULT_ROLE: QuinnQuicRole = QuinnQuicRole::Server;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "quinnwtserversink",
        gst::DebugColorFlags::empty(),
        Some("Quinn WebTransport Server Sink"),
    )
});

struct Started {
    session: Session,
    stream: Option<SendStream>,
    stream_map: HashMap<u64, SendStream>,
    stream_idx: u64,
}

#[allow(clippy::large_enum_variant)]
#[derive(Default)]
enum State {
    #[default]
    Stopped,
    Started(Started),
}

#[derive(Debug)]
struct Settings {
    bind_address: String,
    bind_port: u16,
    address: String,
    port: u16,
    server_name: String,
    timeout: u32,
    use_datagram: bool,
    certificate_file: Option<PathBuf>,
    private_key_file: Option<PathBuf>,
    secure_conn: bool,
    transport_config: QuinnQuicTransportConfig,
    drop_buffer_for_datagram: bool,
    role: QuinnQuicRole,
    url: String,
}

impl Default for Settings {
    fn default() -> Self {
        let transport_config = QuinnQuicTransportConfig {
            // Required for the WebTransport handshake
            max_concurrent_bidi_streams: 2u32.into(),
            max_concurrent_uni_streams: 1u32.into(),
            ..Default::default()
        };

        Settings {
            bind_address: DEFAULT_BIND_ADDR.to_string(),
            bind_port: DEFAULT_BIND_PORT,
            address: DEFAULT_ADDR.to_string(),
            port: DEFAULT_PORT,
            server_name: DEFAULT_SERVER_NAME.to_string(),
            timeout: 0,
            use_datagram: DEFAULT_USE_DATAGRAM,
            certificate_file: None,
            private_key_file: None,
            secure_conn: DEFAULT_SECURE_CONNECTION,
            transport_config,
            drop_buffer_for_datagram: DEFAULT_DROP_BUFFER_FOR_DATAGRAM,
            role: DEFAULT_ROLE,
            url: DEFAULT_ADDR.to_string(),
        }
    }
}

pub struct QuinnWebTransportSink {
    settings: Mutex<Settings>,
    state: Mutex<State>,
    canceller: Mutex<utils::Canceller>,
    session: Mutex<Option<Session>>,
}

impl Default for QuinnWebTransportSink {
    fn default() -> Self {
        Self {
            settings: Mutex::new(Settings::default()),
            state: Mutex::new(State::default()),
            canceller: Mutex::new(utils::Canceller::default()),
            session: Mutex::new(None),
        }
    }
}

impl GstObjectImpl for QuinnWebTransportSink {}

impl ElementImpl for QuinnWebTransportSink {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Quinn WebTransport Server Sink",
                "Source/Network/WebTransport",
                "Send data over the network via WebTransport",
                "Ruben Gonzalez <rgonzalez@fluendo.com>",
            )
        });
        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &gst::Caps::new_any(),
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

impl ObjectImpl for QuinnWebTransportSink {
    fn constructed(&self) {
        self.parent_constructed();
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
                glib::ParamSpecUInt::builder("timeout")
                    .nick("Timeout")
                    .blurb("Value in seconds to timeout QUIC endpoint requests (0 = No timeout).")
                    .maximum(3600)
                    .default_value(DEFAULT_TIMEOUT)
                    .readwrite()
                    .build(),
                glib::ParamSpecString::builder("certificate-file")
                    .nick("Certificate file")
                    .blurb("Path to certificate chain in single file")
                    .build(),
                glib::ParamSpecString::builder("private-key-file")
                    .nick("Private key file")
                    .blurb("Path to a PKCS8 or RSA private key file")
                    .build(),
                glib::ParamSpecBoolean::builder("use-datagram")
                    .nick("Use datagram")
                    .blurb("Use datagram for lower latency, unreliable messaging")
                    .default_value(false)
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
                glib::ParamSpecBoolean::builder("drop-buffer-for-datagram")
                    .nick("Drop buffer for datagram")
                    .blurb("Drop buffers when using datagram if buffer size > max datagram size")
                    .default_value(DEFAULT_DROP_BUFFER_FOR_DATAGRAM)
                    .build(),
                glib::ParamSpecBoolean::builder("secure-connection")
                    .nick("Use secure connection.")
                    .blurb("Use certificates for QUIC connection. False: Insecure connection, True: Secure connection.")
                    .default_value(DEFAULT_SECURE_CONNECTION)
                    .build(),
                glib::ParamSpecEnum::builder_with_default("role", DEFAULT_ROLE)
                    .nick("WebTransport role")
                    .blurb("WebTransport session role to use.")
                    .build(),
                glib::ParamSpecString::builder("url")
                    .nick("Server URL")
                    .blurb("URL of the HTTP/3 server to connect to.")
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
            "timeout" => {
                settings.timeout = value.get().expect("type checked upstream");
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
            "drop-buffer-for-datagram" => {
                settings.drop_buffer_for_datagram = value.get().expect("type checked upstream");
            }
            "secure-connection" => {
                settings.secure_conn = value.get().expect("type checked upstream");
            }
            "role" => {
                settings.role = value.get::<QuinnQuicRole>().expect("type checked upstream");
            }
            "url" => {
                settings.url = value.get::<String>().expect("type checked upstream");
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
            "timeout" => settings.timeout.to_value(),
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
            "secure-connection" => settings.secure_conn.to_value(),
            "stats" => {
                let state = self.state.lock().unwrap();
                match *state {
                    State::Started(ref state) => get_stats(Some(state.session.stats())).to_value(),
                    State::Stopped => get_stats(None).to_value(),
                }
            }
            "drop-buffer-for-datagram" => settings.drop_buffer_for_datagram.to_value(),
            "role" => settings.role.to_value(),
            "url" => settings.url.to_value(),
            _ => unimplemented!(),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for QuinnWebTransportSink {
    const NAME: &'static str = "GstQuinnWebTransportSink";
    type Type = super::QuinnWebTransportSink;
    type ParentType = gst_base::BaseSink;
}

impl BaseSinkImpl for QuinnWebTransportSink {
    fn start(&self) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.lock().unwrap();
        if let State::Started { .. } = *state {
            unreachable!("QuinnWebTransportSink is already started");
        }

        let sess_guard = self.session.lock().unwrap();
        let session = match *sess_guard {
            Some(ref s) => {
                // We will end up here if upstream MoQ muxer
                // requested for a Session before we could
                // set up.
                gst::info!(
                    CAT,
                    imp = self,
                    "Using existing connection with ID: {}",
                    s.stable_id()
                );
                s.clone()
            }
            None => self.setup_session()?,
        };
        drop(sess_guard);

        *state = State::Started(Started {
            session,
            stream: None,
            stream_map: HashMap::new(),
            stream_idx: 0,
        });

        gst::info!(CAT, imp = self, "Started");

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        let settings = self.settings.lock().unwrap();
        let timeout = settings.timeout;
        let use_datagram = settings.use_datagram;
        drop(settings);

        let mut state = self.state.lock().unwrap();

        if let State::Started(ref mut state) = *state {
            if !use_datagram {
                if let Some(ref mut send) = state.stream.take() {
                    self.close_stream(send, timeout);
                }
            }

            for stream in state.stream_map.values_mut() {
                self.close_stream(stream, timeout);
            }

            state
                .session
                .close(CONNECTION_CLOSE_CODE, CONNECTION_CLOSE_MSG.as_bytes());
        }

        *state = State::Stopped;

        gst::info!(CAT, imp = self, "Stopped");

        Ok(())
    }

    fn render(&self, buffer: &gst::Buffer) -> Result<gst::FlowSuccess, gst::FlowError> {
        if let State::Stopped = *self.state.lock().unwrap() {
            gst::element_imp_error!(self, gst::CoreError::Failed, ["Not started yet"]);
            return Err(gst::FlowError::Error);
        }

        gst::trace!(CAT, imp = self, "Rendering {:?}", buffer);

        let map = buffer.map_readable().map_err(|_| {
            gst::element_imp_error!(self, gst::CoreError::Failed, ["Failed to map buffer"]);
            gst::FlowError::Error
        })?;

        let meta = buffer.meta::<QuinnQuicMeta>();

        match self.send_buffer(&map, meta) {
            Ok(_) => Ok(gst::FlowSuccess::Ok),
            Err(err) => match err {
                Some(error_message) => {
                    gst::error!(CAT, imp = self, "Data sending failed: {}", error_message);
                    self.post_error_message(error_message);
                    Err(gst::FlowError::Error)
                }
                _ => {
                    gst::info!(CAT, imp = self, "Send interrupted. Flushing...");
                    Err(gst::FlowError::Flushing)
                }
            },
        }
    }

    fn query(&self, query: &mut gst::QueryRef) -> bool {
        match query.view_mut() {
            gst::QueryViewMut::Custom(q) => self.sink_query(q),
            gst::QueryViewMut::Context(c) => {
                gst::debug!(CAT, imp = self, "Handling Context query");

                if let Some(context) = self.set_session_context() {
                    gst::info!(CAT, imp = self, "Setting Quinn Session Context");
                    c.set_context(&context);
                    return true;
                }

                match self.setup_session() {
                    Ok(session) => {
                        let mut conn_guard = self.session.lock().unwrap();
                        *conn_guard = Some(session);
                        drop(conn_guard);

                        match self.set_session_context() {
                            Some(context) => {
                                gst::info!(CAT, imp = self, "Setting Quinn Session Context");
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
                        gst::error!(CAT, imp = self, "Failed to setup session, {e:?}");
                        false
                    }
                }
            }
            _ => BaseSinkImplExt::parent_query(self, query),
        }
    }

    fn unlock(&self) -> Result<(), gst::ErrorMessage> {
        let mut canceller = self.canceller.lock().unwrap();
        canceller.abort();
        Ok(())
    }

    fn unlock_stop(&self) -> Result<(), gst::ErrorMessage> {
        let mut canceller = self.canceller.lock().unwrap();
        if matches!(&*canceller, utils::Canceller::Cancelled) {
            *canceller = utils::Canceller::None;
        }
        Ok(())
    }

    fn event(&self, event: gst::Event) -> bool {
        use gst::EventView;

        gst::debug!(CAT, imp = self, "Handling event {:?}", event);

        let settings = self.settings.lock().unwrap();
        let timeout = settings.timeout;
        drop(settings);

        let mut state = self.state.lock().unwrap();
        if let State::Started(ref mut state) = *state {
            if let EventView::CustomDownstream(ev) = event.view() {
                if let Some(s) = ev.structure() {
                    if s.name() == QUIC_STREAM_CLOSE_CUSTOMDOWNSTREAM_EVENT {
                        if let Ok(stream_id) = s.get::<u64>(QUIC_STREAM_ID) {
                            if let Some(mut stream) = state.stream_map.remove(&stream_id) {
                                self.close_stream(&mut stream, timeout);
                                return true;
                            }
                        }
                    }
                }
            }
        }

        self.parent_event(event)
    }
}

impl QuinnWebTransportSink {
    fn send_buffer(
        &self,
        src: &[u8],
        meta: Option<gst::MetaRef<'_, QuinnQuicMeta>>,
    ) -> Result<(), Option<gst::ErrorMessage>> {
        let settings = self.settings.lock().unwrap();
        let timeout = settings.timeout;
        let use_datagram = settings.use_datagram;
        let drop_buffer_for_datagram = settings.drop_buffer_for_datagram;
        drop(settings);

        let mut state = self.state.lock().unwrap();

        let started = match *state {
            State::Started(ref mut started) => started,
            State::Stopped => {
                return Err(Some(gst::error_msg!(
                    gst::LibraryError::Failed,
                    ["Cannot send before start()"]
                )));
            }
        };
        let session = started.session.clone();

        if let Some(m) = meta {
            if m.is_datagram() {
                self.write_datagram(session, src, drop_buffer_for_datagram)
            } else {
                let stream_id = m.stream_id();

                if let Some(send) = started.stream_map.get_mut(&stream_id) {
                    gst::trace!(CAT, imp = self, "Writing buffer for stream {stream_id:?}");
                    self.write_stream(send, src, timeout)
                } else {
                    Err(Some(gst::error_msg!(
                        gst::ResourceError::Failed,
                        ["No stream for buffer with stream id {}", stream_id]
                    )))
                }
            }
        } else if use_datagram {
            self.write_datagram(session, src, drop_buffer_for_datagram)
        } else {
            {
                if started.stream.is_none() {
                    match self.open_stream(session, timeout) {
                        Ok(stream) => {
                            gst::debug!(CAT, imp = self, "Opened stream: {:?}", stream);
                            started.stream = Some(stream);
                        }
                        Err(err) => return Err(Some(err)),
                    }
                }
            }

            let send = started.stream.as_mut().expect("Stream must be valid here");
            self.write_stream(send, src, timeout)
        }
    }

    fn get_epconfig(
        &self,
    ) -> Result<(QuinnQuicRole, Option<url::Url>, QuinnQuicEndpointConfig), gst::ErrorMessage> {
        let settings = self.settings.lock().unwrap();

        let role = settings.role;
        let timeout = settings.timeout;
        let secure_conn = settings.secure_conn;
        let server_name = settings.server_name.clone();
        let certificate_file = settings.certificate_file.clone();
        let private_key_file = settings.private_key_file.clone();
        let transport_config = settings.transport_config;

        let client_addr = if role == QuinnQuicRole::Client {
            Some(
                make_socket_addr(
                    format!("{}:{}", settings.bind_address, settings.bind_port).as_str(),
                )
                .unwrap(),
            )
        } else {
            None
        };

        let (server_addr, url) = if role == QuinnQuicRole::Client {
            let url = url::Url::parse(&settings.url).map_err(|err| {
                gst::error_msg!(gst::ResourceError::Failed, ["Failed to parse URL: {}", err])
            })?;
            drop(settings);

            let server_port = url.port().unwrap_or(443);
            if !url.has_host() {
                return Err(gst::error_msg!(
                    gst::ResourceError::Failed,
                    ["Cannot parse host for URL"]
                ));
            }
            let host = url.host_str().unwrap().to_owned();

            // Look up the DNS entry.
            let mut remotes = match wait(&self.canceller, lookup_host((host, server_port)), timeout)
            {
                Ok(Ok(remotes)) => remotes,
                Ok(Err(err)) => {
                    return Err(gst::error_msg!(
                        gst::ResourceError::Failed,
                        ["Host lookup request failed: {err}"]
                    ));
                }
                Err(err) => match err {
                    WaitError::FutureAborted => {
                        return Err(gst::error_msg!(
                            gst::ResourceError::Failed,
                            ["Host lookup request aborted"]
                        ));
                    }
                    WaitError::FutureError(err) => {
                        return Err(gst::error_msg!(
                            gst::ResourceError::Failed,
                            ["Host lookup request failed: {err}"]
                        ));
                    }
                },
            };

            let server_addr = match remotes.next() {
                Some(remote) => Ok(remote),
                None => Err(gst::error_msg!(
                    gst::ResourceError::Failed,
                    ["Cannot resolve host name for URL"]
                )),
            }?;

            drop(remotes);

            (server_addr, Some(url))
        } else {
            (
                make_socket_addr(format!("{}:{}", settings.address, settings.port).as_str())
                    .unwrap(),
                None,
            )
        };

        Ok((
            role,
            url,
            QuinnQuicEndpointConfig {
                server_addr,
                server_name,
                client_addr,
                secure_conn,
                alpns: vec![HTTP3_ALPN.to_string()],
                certificate_file,
                private_key_file,
                keep_alive_interval: 0,
                transport_config,
                with_client_auth: role == QuinnQuicRole::Client,
                webtransport: true,
            },
        ))
    }

    fn sink_query(&self, query: &mut gst::QueryRef) -> bool {
        gst::debug!(CAT, imp = self, "Handling sink query: {query:?}");

        let s = query.structure_mut();

        match s.name().as_str() {
            QUIC_DATAGRAM_PROBE => self.handle_datagram_query(s),
            QUIC_STREAM_OPEN => self.handle_open_stream_query(s),
            _ => false,
        }
    }

    fn handle_open_stream_query(&self, s: &mut gst::StructureRef) -> bool {
        gst::debug!(CAT, imp = self, "Handling open stream query: {s:?}");

        let settings = self.settings.lock().unwrap();
        let timeout = settings.timeout;
        drop(settings);

        let mut state = self.state.lock().unwrap();
        if let State::Started(ref mut state) = *state {
            let session = state.session.clone();

            gst::debug!(
                CAT,
                imp = self,
                "Attempting to open stream for stream query: {s:?}"
            );

            match self.open_stream(session, timeout) {
                Ok(stream) => {
                    let index = state.stream_idx;

                    if let Ok(priority) = s.get::<i32>(QUIC_STREAM_PRIORITY) {
                        // Default value of priority for Stream is already 0.
                        if priority != 0 {
                            let _ = stream.set_priority(priority);
                        }
                    }

                    gst::debug!(
                        CAT,
                        imp = self,
                        "Opened stream for query: {s:?}, stream: {:?}, priority: {:?}",
                        stream,
                        stream.priority()
                    );

                    state.stream_map.insert(index, stream);
                    s.set_value(QUIC_STREAM_ID, index.to_send_value());

                    state.stream_idx += 1;

                    return true;
                }
                Err(err) => {
                    gst::error!(
                        CAT,
                        imp = self,
                        "Failed to handle open stream query, {err:?}"
                    );
                    return false;
                }
            }
        }

        false
    }

    fn handle_datagram_query(&self, s: &mut gst::StructureRef) -> bool {
        gst::debug!(CAT, imp = self, "Handling datagram query: {s:?}");

        let state = self.state.lock().unwrap();
        if let State::Started(ref state) = *state {
            if state.session.max_datagram_size() > 0 {
                return true;
            }

            gst::warning!(CAT, imp = self, "Datagram unsupported by peer");
        }

        false
    }

    fn open_stream(&self, session: Session, timeout: u32) -> Result<SendStream, gst::ErrorMessage> {
        match wait(&self.canceller, session.open_uni(), timeout) {
            Ok(Ok(stream)) => {
                gst::debug!(CAT, imp = self, "Opened stream: {:?}", stream);

                Ok(stream)
            }
            Ok(Err(err)) => {
                gst::error!(CAT, imp = self, "Failed to open stream {err}");
                Err(gst::error_msg!(
                    gst::ResourceError::Failed,
                    ["Failed to open stream, {err}"]
                ))
            }
            Err(err) => {
                gst::error!(CAT, imp = self, "Failed to open stream {err}");
                Err(gst::error_msg!(
                    gst::ResourceError::Failed,
                    ["Failed to open stream, {err}"]
                ))
            }
        }
    }

    fn write_datagram(
        &self,
        session: Session,
        src: &[u8],
        drop_buffer_for_datagram: bool,
    ) -> Result<(), Option<gst::ErrorMessage>> {
        let size = session.max_datagram_size();
        if src.len() > size {
            if drop_buffer_for_datagram {
                gst::warning!(
                    CAT,
                    imp = self,
                    "Buffer dropped, current max datagram size: {size} > buffer size: {}",
                    src.len()
                );
                return Ok(());
            } else {
                return Err(Some(gst::error_msg!(
                    gst::ResourceError::Failed,
                    [
                        "Sending data failed, current max datagram size: {size}, buffer size: {}",
                        src.len()
                    ]
                )));
            }
        }

        match session.send_datagram(Bytes::copy_from_slice(src)) {
            Ok(_) => Ok(()),
            Err(e) => Err(Some(gst::error_msg!(
                gst::ResourceError::Failed,
                ["Sending data failed: {}", e]
            ))),
        }
    }

    fn write_stream(
        &self,
        send: &mut SendStream,
        src: &[u8],
        timeout: u32,
    ) -> Result<(), Option<gst::ErrorMessage>> {
        match wait(&self.canceller, send.write_all(src), timeout) {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(Some(gst::error_msg!(
                gst::ResourceError::Failed,
                ["Sending data failed: {}", e]
            ))),
            Err(e) => match e {
                WaitError::FutureAborted => {
                    gst::warning!(CAT, imp = self, "Sending aborted");
                    Ok(())
                }
                WaitError::FutureError(e) => Err(Some(gst::error_msg!(
                    gst::ResourceError::Failed,
                    ["Sending data failed: {}", e]
                ))),
            },
        }
    }

    fn close_stream(&self, stream: &mut SendStream, timeout: u32) {
        /*
         * Shutdown stream gracefully
         * send.finish() may fail, but the error is harmless.
         */
        let _ = stream.finish();

        match wait(&self.canceller, stream.stopped(), timeout) {
            Ok(r) => {
                if let Err(e) = r {
                    let err_msg = format!("Stream finish request error: {e}");
                    gst::error!(CAT, imp = self, "{}", err_msg);
                } else {
                    gst::info!(CAT, imp = self, "Stream {:?} finished", stream);
                }
            }
            Err(e) => match e {
                WaitError::FutureAborted => {
                    let err_msg = "Stream finish request aborted".to_string();
                    gst::warning!(CAT, imp = self, "{}", err_msg);
                }
                WaitError::FutureError(e) => {
                    let err_msg = format!("Stream finish request future error: {e}");
                    gst::error!(CAT, imp = self, "{}", err_msg);
                }
            },
        }
    }

    fn setup_session(&self) -> Result<Session, gst::ErrorMessage> {
        let settings = self.settings.lock().unwrap();
        let timeout = settings.timeout;
        drop(settings);

        let (role, url, endpoint_config) = self.get_epconfig()?;
        let mut shared_connection = SharedConnection::get_or_init(endpoint_config.server_addr);

        shared_connection.set_endpoint_config(endpoint_config);

        let session = match shared_connection.connection() {
            Some(c) => match c {
                QuinnConnection::WebTransport(sess) => {
                    gst::info!(
                        CAT,
                        imp = self,
                        "Using existing session with ID: {}",
                        sess.stable_id()
                    );

                    sess
                }
                QuinnConnection::Quic(_) => unreachable!(),
            },
            None => match wait(
                &self.canceller,
                shared_connection.connect(role, url),
                timeout,
            ) {
                Ok(Ok(_)) => {
                    let c = shared_connection
                        .connection()
                        .expect("Connection should be valid here");
                    match c {
                        QuinnConnection::Quic(_) => unreachable!(),
                        QuinnConnection::WebTransport(sess) => {
                            gst::info!(
                                CAT,
                                imp = self,
                                "Using existing session with ID: {}",
                                sess.stable_id()
                            );

                            sess
                        }
                    }
                }
                Ok(Err(e)) | Err(e) => match e {
                    WaitError::FutureAborted => {
                        gst::warning!(CAT, imp = self, "Session aborted");
                        return Err(gst::error_msg!(
                            gst::ResourceError::Failed,
                            ["Session request failed"]
                        ));
                    }
                    WaitError::FutureError(err) => {
                        gst::error!(CAT, imp = self, "Session request failed: {}", err);
                        return Err(gst::error_msg!(
                            gst::ResourceError::Failed,
                            ["Session request failed: {err}"]
                        ));
                    }
                },
            },
        };

        Ok(session)
    }

    fn set_session_context(&self) -> Option<gst::Context> {
        let sess_guard = self.session.lock().unwrap();

        if let Some(ref session) = *sess_guard {
            gst::debug!(CAT, imp = self, "Using already established Connection");

            let conn = QuinnConnectionContext(Arc::new(QuinnConnectionContextInner {
                connection: QuinnConnection::WebTransport(session.clone()),
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
