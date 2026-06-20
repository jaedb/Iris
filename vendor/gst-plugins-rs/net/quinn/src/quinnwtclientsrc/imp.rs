// Copyright (C) 2024, Fluendo S.A.
//      Author: Andoni Morales Alastruey <amorales@fluendo.com>
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
use crate::quinnquicmeta::QuinnQuicMeta;
use crate::quinnquicquery::*;
use crate::utils::{
    client, client_endpoint, get_stats, make_socket_addr, server_endpoint, wait, Canceller,
    WaitError, CONNECTION_CLOSE_CODE, CONNECTION_CLOSE_MSG, RUNTIME,
};
use crate::{common::*, utils};
use async_channel::{unbounded, Receiver, Sender};
use bytes::{buf, Bytes};
use futures::{future::BoxFuture, stream::FuturesUnordered, StreamExt};
use gst::{glib, prelude::*, subclass::prelude::*};
use gst_base::prelude::*;
use gst_base::subclass::base_src::CreateSuccess;
use gst_base::subclass::prelude::*;
use quinn::{Connection, ConnectionError, TransportConfig};
use rustls::server;
use std::borrow::Borrow;
use std::fmt::Error;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::thread::{spawn, Builder, JoinHandle};
use tokio::net::lookup_host;
use tokio::sync::oneshot;
use web_transport_quinn::*;

const DATA_HANDLER_THREAD: &str = "data-handler";

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "quinnwtclientsrc",
        gst::DebugColorFlags::empty(),
        Some("Quinn WebTransport client source"),
    )
});

enum QuinnData {
    Datagram(Bytes),
    Stream(u64, Bytes),
    Closed(u64),
    Eos,
}

struct Started {
    session: Session,
    data_handler: Option<JoinHandle<()>>,
    // TODO: Use tokio channel
    //
    // We use async-channel to keep a clone of the receive channel around
    // for use in every `create` call. tokio's UnboundedReceiver does not
    // implement clone.
    data_rx: Option<Receiver<QuinnData>>,
    thread_quit: Option<oneshot::Sender<()>>,
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
    certificate_file: Option<PathBuf>,
    keep_alive_interval: u64,
    secure_conn: bool,
    timeout: u32,
    transport_config: QuinnQuicTransportConfig,
    url: String,
}

impl Default for Settings {
    fn default() -> Self {
        let transport_config = QuinnQuicTransportConfig::default();
        Settings {
            bind_address: DEFAULT_BIND_ADDR.to_string(),
            bind_port: DEFAULT_BIND_PORT,
            certificate_file: None,
            keep_alive_interval: 0,
            secure_conn: DEFAULT_SECURE_CONNECTION,
            timeout: DEFAULT_TIMEOUT,
            transport_config,
            url: DEFAULT_ADDR.to_string(),
        }
    }
}

pub struct QuinnWebTransportClientSrc {
    settings: Mutex<Settings>,
    state: Mutex<State>,
    canceller: Mutex<utils::Canceller>,
    session: Mutex<Option<Session>>,
}

impl Default for QuinnWebTransportClientSrc {
    fn default() -> Self {
        Self {
            settings: Mutex::new(Settings::default()),
            state: Mutex::new(State::default()),
            canceller: Mutex::new(utils::Canceller::default()),
            session: Mutex::new(None),
        }
    }
}

impl GstObjectImpl for QuinnWebTransportClientSrc {}

impl ElementImpl for QuinnWebTransportClientSrc {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Quinn WebTransport Client Source",
                "Source/Network/QUIC",
                "Receive data over the network via WebTransport",
                "Andoni Morales Alastruey <amorales@fluendo.com>",
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
            if settings.secure_conn && settings.certificate_file.is_none() {
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

impl ObjectImpl for QuinnWebTransportClientSrc {
    fn constructed(&self) {
        self.parent_constructed();
        self.obj().set_format(gst::Format::Time);
        self.obj().set_live(true);
        self.obj().set_do_timestamp(true);
    }

    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecString::builder("certificate-file")
                    .nick("Certificate file")
                    .blurb("Path to certificate chain in single file")
                    .build(),
		glib::ParamSpecUInt64::builder("keep-alive-interval")
                    .nick("QUIC connection keep alive interval in ms")
                    .blurb("Keeps QUIC connection alive by periodically pinging the server. Value set in ms, 0 disables this feature")
		    .default_value(0)
                    .readwrite()
                    .build(),
                glib::ParamSpecUInt::builder("timeout")
                    .nick("Timeout")
                    .blurb("Value in seconds to timeout WebTransport endpoint requests (0 = No timeout).")
                    .maximum(3600)
                    .default_value(DEFAULT_TIMEOUT)
                    .readwrite()
                    .build(),
                glib::ParamSpecString::builder("url")
                    .nick("Server URL")
                    .blurb("URL of the HTTP/3 server to connect to.")
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Structure>("stats")
                    .nick("Connection statistics")
                    .blurb("Connection statistics")
                    .read_only()
                    .build(),
                glib::ParamSpecBoolean::builder("secure-connection")
                    .nick("Use secure connection.")
                    .blurb("Use certificates for QUIC connection. False: Insecure connection, True: Secure connection.")
                    .default_value(DEFAULT_SECURE_CONNECTION)
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().unwrap();

        match pspec.name() {
            "certificate-file" => {
                let value: String = value.get().unwrap();
                settings.certificate_file = Some(value.into());
            }
            "keep-alive-interval" => {
                settings.keep_alive_interval = value.get().expect("type checked upstream");
            }
            "timeout" => {
                settings.timeout = value.get().expect("type checked upstream");
            }
            "secure-connection" => {
                settings.secure_conn = value.get().expect("type checked upstream");
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
            "certificate-file" => {
                let certfile = settings.certificate_file.as_ref();
                certfile.and_then(|file| file.to_str()).to_value()
            }
            "keep-alive-interval" => settings.keep_alive_interval.to_value(),
            "timeout" => settings.timeout.to_value(),
            "url" => settings.url.to_value(),
            "secure-connection" => settings.secure_conn.to_value(),
            "stats" => {
                let state = self.state.lock().unwrap();
                match *state {
                    State::Started(ref state) => get_stats(Some(state.session.stats())).to_value(),
                    State::Stopped => get_stats(None).to_value(),
                }
            }
            _ => unimplemented!(),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for QuinnWebTransportClientSrc {
    const NAME: &'static str = "GstQuinnWebTransportClientSrc";
    type Type = super::QuinnWebTransportClientSrc;
    type ParentType = gst_base::PushSrc;
}

impl BaseSrcImpl for QuinnWebTransportClientSrc {
    fn is_seekable(&self) -> bool {
        false
    }

    fn query(&self, query: &mut gst::QueryRef) -> bool {
        use gst::QueryViewMut;

        match query.view_mut() {
            QueryViewMut::Context(c) => {
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
            _ => BaseSrcImplExt::parent_query(self, query),
        }
    }

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        let state = self.state.lock().unwrap();
        if let State::Started { .. } = *state {
            unreachable!("QuinnWebTransportClientSrc already started");
        }
        drop(state);

        let sess_guard = self.session.lock().unwrap();
        let session = match *sess_guard {
            Some(ref s) => {
                // We will end up here if downstream MoQ demuxer
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

        let sess_clone = session.clone();
        let (tx_quit, rx_quit): (oneshot::Sender<()>, oneshot::Receiver<()>) = oneshot::channel();
        let (data_tx, data_rx): (Sender<QuinnData>, Receiver<QuinnData>) = unbounded();

        let self_ = self.ref_counted();
        let data_handler = Builder::new()
            .name(DATA_HANDLER_THREAD.to_string())
            .spawn(move || {
                self_.handle_data(sess_clone, data_tx, rx_quit);
                gst::debug!(CAT, imp = self_, "Data handler thread exit");
            })
            .unwrap();

        let mut state = self.state.lock().unwrap();
        *state = State::Started(Started {
            session,
            data_handler: Some(data_handler),
            data_rx: Some(data_rx),
            thread_quit: Some(tx_quit),
        });
        drop(state);

        gst::info!(CAT, imp = self, "Started");

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
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

            state
                .session
                .close(CONNECTION_CLOSE_CODE, CONNECTION_CLOSE_MSG.as_bytes());
        }

        *state = State::Stopped;

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
}

impl PushSrcImpl for QuinnWebTransportClientSrc {
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

impl QuinnWebTransportClientSrc {
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
                    Err(None)
                }
                WaitError::FutureError(e) => {
                    gst::error!(CAT, imp = self, "Failed to read from stream: {}", e);
                    Err(Some(e))
                }
            },
        }
    }

    fn get_epconfig(&self) -> Result<(url::Url, QuinnQuicEndpointConfig), gst::ErrorMessage> {
        let settings = self.settings.lock().unwrap();
        let timeout = settings.timeout;
        let secure_conn = settings.secure_conn;
        let certificate_file = settings.certificate_file.clone();
        let keep_alive_interval = settings.keep_alive_interval;
        let transport_config = settings.transport_config;

        let client_addr =
            make_socket_addr(format!("{}:{}", settings.bind_address, settings.bind_port).as_str())?;

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
        let mut remotes = match wait(&self.canceller, lookup_host((host, server_port)), timeout) {
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

        Ok((
            url,
            QuinnQuicEndpointConfig {
                server_addr,
                server_name: DEFAULT_SERVER_NAME.to_string(),
                client_addr: Some(client_addr),
                secure_conn,
                alpns: vec![HTTP3_ALPN.to_string()],
                certificate_file,
                private_key_file: None,
                keep_alive_interval,
                transport_config,
                with_client_auth: false,
                webtransport: true,
            },
        ))
    }

    fn handle_connection_error(&self, session_err: SessionError) {
        match session_err {
            SessionError::ConnectionError(err) => {
                gst::error!(CAT, imp = self, "Connection error: {err:?}");
            }
            SessionError::WebTransportError(err) => {
                gst::error!(CAT, imp = self, "WebTransport error: {err:?}");
            }
            SessionError::SendDatagramError(err) => {
                gst::error!(CAT, imp = self, "Send Datagram error: {err:?}");
            }
        }
    }

    fn handle_data(
        &self,
        session: Session,
        sender: Sender<QuinnData>,
        receiver: oneshot::Receiver<()>,
    ) {
        // Unifies the Future return types
        enum QuinnFuture {
            Datagram(Bytes),
            StreamData(RecvStream, QuinnData),
            Stream(RecvStream, u64),
            Stop,
        }

        let blocksize = self.obj().blocksize() as usize;
        gst::info!(CAT, imp = self, "Using a blocksize of {blocksize} for read",);

        let stream_idx = AtomicU64::new(0);

        let incoming_stream = |sess: Session, sidx: u64| async move {
            match sess.accept_uni().await {
                Ok(recv_stream) => QuinnFuture::Stream(recv_stream, sidx),
                Err(err) => {
                    self.handle_connection_error(err);
                    QuinnFuture::Stop
                }
            }
        };

        let datagram = |sess: Session| async move {
            match sess.read_datagram().await {
                Ok(bytes) => QuinnFuture::Datagram(bytes),
                Err(err) => {
                    self.handle_connection_error(err);
                    QuinnFuture::Stop
                }
            }
        };

        let recv_stream = |mut s: RecvStream, stream_id: u64| async move {
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
                    ReadError::SessionError(err) => {
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

        tasks.push(Box::pin(datagram(session.clone())));

        let idx = stream_idx.load(Ordering::Relaxed);
        stream_idx.fetch_add(1, Ordering::Relaxed);

        tasks.push(Box::pin(incoming_stream(session.clone(), idx)));
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
                            tasks.push(Box::pin(recv_stream(s, stream_id)));
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
                    QuinnFuture::Stream(s, stream_id) => {
                        gst::trace!(
                            CAT,
                            imp = self,
                            "Incoming stream connection, {stream_id}, {s:?}"
                        );
                        tasks.push(Box::pin(recv_stream(s, stream_id)));

                        let idx = stream_idx.load(Ordering::Relaxed);
                        stream_idx.fetch_add(1, Ordering::Relaxed);

                        tasks.push(Box::pin(incoming_stream(session.clone(), idx)));
                    }
                    QuinnFuture::Datagram(b) => {
                        gst::trace!(CAT, imp = self, "Received {} bytes on datagram", b.len());
                        tx_send(sender.clone(), QuinnData::Datagram(b)).await;
                        tasks.push(Box::pin(datagram(session.clone())));
                    }
                }
            }
        });

        gst::info!(CAT, imp = self, "Quit data handler thread");
    }

    fn setup_session(&self) -> Result<Session, gst::ErrorMessage> {
        let settings = self.settings.lock().unwrap();
        let timeout = settings.timeout;
        drop(settings);

        let (url, endpoint_config) = self.get_epconfig()?;

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
                shared_connection.connect(QuinnQuicRole::Client, Some(url)),
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
                            ["Session request aborted"]
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

        gst::info!(
            CAT,
            imp = self,
            "Remote session established: {}",
            session.remote_address()
        );

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
