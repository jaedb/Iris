// SPDX-License-Identifier: MPL-2.0

use crate::signaller::{prelude::*, Signallable};
use crate::utils::{create_tls_connector, gvalue_to_json, serialize_json_object};
use crate::RUNTIME;
use anyhow::{anyhow, Error};
use async_tungstenite::tungstenite::client::IntoClientRequest;
use async_tungstenite::tungstenite::http::{HeaderName, HeaderValue};
use async_tungstenite::tungstenite::Message as WsMessage;
use futures::channel::mpsc;
use futures::prelude::*;
use gst::glib;
use gst::glib::prelude::*;
use gst::subclass::prelude::*;
use gst_plugin_webrtc_protocol as p;
use std::collections::{HashMap, HashSet};
use std::ops::ControlFlow;
use std::str::FromStr;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;
use tokio::{task, time::timeout};
use url::Url;

use super::CAT;

const DEFAULT_INSECURE_TLS: bool = false;

#[derive(Debug, Eq, PartialEq, Clone, Copy, glib::Enum, Default)]
#[repr(u32)]
#[enum_type(name = "GstRSWebRTCSignallerRole")]
pub enum WebRTCSignallerRole {
    #[default]
    Consumer,
    Producer,
    Listener,
}

pub struct Settings {
    uri: Url,
    producer_peer_id: Option<String>,
    cafile: Option<String>,
    role: WebRTCSignallerRole,
    headers: Option<gst::Structure>,
    insecure_tls: bool,
    connect_to_first_producer: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            uri: Url::from_str("ws://127.0.0.1:8443").unwrap(),
            producer_peer_id: None,
            cafile: Default::default(),
            role: Default::default(),
            headers: None,
            insecure_tls: DEFAULT_INSECURE_TLS,
            connect_to_first_producer: false,
        }
    }
}

#[derive(Default)]
pub struct Signaller {
    state: Mutex<State>,
    settings: Mutex<Settings>,
}

#[derive(Default)]
struct State {
    /// Sender for the websocket messages
    websocket_sender: Option<mpsc::Sender<p::IncomingMessage>>,
    connect_task_handle: Option<task::JoinHandle<()>>,
    send_task_handle: Option<task::JoinHandle<Result<(), Error>>>,
    receive_task_handle: Option<task::JoinHandle<()>>,
    producers: HashSet<String>,
    client_id: Option<String>,
}

impl Signaller {
    fn uri(&self) -> Url {
        self.settings.lock().unwrap().uri.clone()
    }

    pub fn connect_to_first_producer(&self) -> bool {
        self.settings.lock().unwrap().connect_to_first_producer
    }

    pub fn set_connect_to_first_producer(&self, value: bool) {
        self.settings.lock().unwrap().connect_to_first_producer = value;
    }

    fn set_uri(&self, uri: &str) -> Result<(), Error> {
        let mut settings = self.settings.lock().unwrap();
        let mut uri = Url::from_str(uri).map_err(|err| anyhow!("{err:?}"))?;

        if let Some(peer_id) = uri
            .query_pairs()
            .find(|(k, _)| k == "peer-id")
            .map(|v| v.1.to_string())
        {
            if !matches!(settings.role, WebRTCSignallerRole::Consumer) {
                gst::warning!(
                    CAT,
                    "Setting peer-id doesn't make sense for {:?}",
                    settings.role
                );
            } else {
                settings.producer_peer_id = Some(peer_id);
            }
        }

        if let Some(connect_to_first_producer) = uri
            .query_pairs()
            .find(|(k, _)| k == "connect-to-first-producer")
            .map(|v| matches!(v.1.to_lowercase().as_str(), "true" | "1" | ""))
        {
            if !matches!(settings.role, WebRTCSignallerRole::Consumer) {
                gst::warning!(
                    CAT,
                    "Setting connect-to-first-producer doesn't make sense for {:?}",
                    settings.role
                );
            } else {
                settings.connect_to_first_producer = connect_to_first_producer;
            }
        }

        if let Some(peer_id) = &settings.producer_peer_id {
            uri.query_pairs_mut()
                .clear()
                .append_pair("peer-id", peer_id);
        }

        if settings.connect_to_first_producer {
            uri.query_pairs_mut()
                .clear()
                .append_pair("connect-to-first-producer", "true");
        }

        settings.uri = uri;

        Ok(())
    }

    async fn connect(&self) -> Result<(), Error> {
        let (cafile, insecure_tls, role, connect_to_first_producer) = {
            let settings = self.settings.lock().unwrap();
            (
                settings.cafile.clone(),
                settings.insecure_tls,
                settings.role,
                settings.connect_to_first_producer,
            )
        };

        if let super::WebRTCSignallerRole::Consumer = role {
            if !connect_to_first_producer && self.producer_peer_id().is_none() {
                gst::info!(
                    CAT,
                    imp = self,
                    "No producer peer id set, listening for producer session requests"
                );
            }
        }

        if insecure_tls {
            gst::warning!(CAT, imp = self, "insecure tls connections are allowed");
        }

        let connector = create_tls_connector(cafile, insecure_tls)
            .map_ok(Some)
            .await?;
        let mut uri = self.uri();
        uri.set_query(None);

        gst::info!(CAT, imp = self, "connecting to {}", uri.to_string());

        let mut req = uri.into_client_request()?;
        let req_headers = req.headers_mut();
        if let Some(headers) = self.headers() {
            for (key, value) in headers {
                req_headers.insert(
                    HeaderName::from_bytes(key.as_bytes()).unwrap(),
                    HeaderValue::from_bytes(value.as_bytes()).unwrap(),
                );
            }
        }

        let (ws, _) = timeout(
            // FIXME: Make the timeout configurable
            Duration::from_secs(20),
            async_tungstenite::tokio::connect_async_with_tls_connector(req, connector),
        )
        .await??;

        gst::info!(CAT, imp = self, "connected");

        // Channel for asynchronously sending out websocket message
        let (mut ws_sink, mut ws_stream) = ws.split();

        // 1000 is completely arbitrary, we simply don't want infinite piling
        // up of messages as with unbounded
        let (websocket_sender, mut websocket_receiver) = mpsc::channel::<p::IncomingMessage>(1000);
        let send_task_handle = RUNTIME.spawn(glib::clone!(
            #[to_owned(rename_to = this)]
            self,
            async move {
                let mut res = Ok(());
                while let Some(msg) = websocket_receiver.next().await {
                    gst::log!(CAT, "Sending websocket message {:?}", msg);
                    res = ws_sink
                        .send(WsMessage::text(serde_json::to_string(&msg).unwrap()))
                        .await;

                    if let Err(ref err) = res {
                        gst::error!(CAT, imp = this, "Quitting send loop: {err}");
                        break;
                    }
                }

                gst::debug!(CAT, imp = this, "Done sending");

                let _ = ws_sink.close(None).await;

                res.map_err(Into::into)
            }
        ));

        let obj = self.obj();
        let meta =
            if let Some(meta) = obj.emit_by_name::<Option<gst::Structure>>("request-meta", &[]) {
                gvalue_to_json(&meta.to_value())
            } else {
                None
            };

        let receive_task_handle = RUNTIME.spawn(glib::clone!(
            #[to_owned(rename_to = this)]
            self,
            async move {
                while let Some(msg) = tokio_stream::StreamExt::next(&mut ws_stream).await {
                    if let ControlFlow::Break(_) = this.handle_message(msg, &meta) {
                        break;
                    }
                }

                let msg = "Stopped websocket receiving";
                gst::info!(CAT, imp = this, "{msg}");
            }
        ));

        let mut state = self.state.lock().unwrap();
        state.websocket_sender = Some(websocket_sender);
        state.send_task_handle = Some(send_task_handle);
        state.receive_task_handle = Some(receive_task_handle);

        Ok(())
    }

    fn set_status(&self, meta: &Option<serde_json::Value>, peer_id: &str) {
        self.state.lock().unwrap().client_id = Some(peer_id.to_string());

        let role = self.settings.lock().unwrap().role;
        self.send(p::IncomingMessage::SetPeerStatus(match role {
            super::WebRTCSignallerRole::Consumer => p::PeerStatus {
                meta: meta.clone(),
                peer_id: Some(peer_id.to_string()),
                roles: if self.connect_to_first_producer() || self.producer_peer_id().is_some() {
                    vec![]
                } else {
                    vec![p::PeerRole::Consumer]
                },
            },
            super::WebRTCSignallerRole::Producer => p::PeerStatus {
                meta: meta.clone(),
                peer_id: Some(peer_id.to_string()),
                roles: vec![p::PeerRole::Producer],
            },
            super::WebRTCSignallerRole::Listener => p::PeerStatus {
                meta: meta.clone(),
                peer_id: Some(peer_id.to_string()),
                roles: vec![p::PeerRole::Listener],
            },
        }));

        if matches!(role, super::WebRTCSignallerRole::Listener) {
            self.send(p::IncomingMessage::List);
        }
    }

    fn producer_peer_id(&self) -> Option<String> {
        let settings = self.settings.lock().unwrap();

        settings.producer_peer_id.clone()
    }

    fn headers(&self) -> Option<HashMap<String, String>> {
        self.settings
            .lock()
            .unwrap()
            .headers
            .as_ref()
            .map(|structure| {
                let mut hash = HashMap::new();

                for (key, value) in structure.iter() {
                    if let Ok(Ok(value_str)) = value.transform::<String>().map(|v| v.get()) {
                        gst::log!(CAT, imp = self, "headers '{}' -> '{}'", key, value_str);
                        hash.insert(key.to_string(), value_str);
                    } else {
                        gst::warning!(
                            CAT,
                            imp = self,
                            "Failed to convert headers '{}' to string ('{:?}')",
                            key,
                            value
                        );
                    }
                }

                hash
            })
    }

    fn send(&self, msg: p::IncomingMessage) {
        let state = self.state.lock().unwrap();
        if let Some(mut sender) = state.websocket_sender.clone() {
            RUNTIME.spawn(glib::clone!(
                #[to_owned(rename_to = this)]
                self,
                async move {
                    if let Err(err) = sender.send(msg).await {
                        this.obj()
                            .emit_by_name::<()>("error", &[&format!("Error: {err}")]);
                    }
                }
            ));
        }
    }

    pub fn start_session(&self) {
        let role = self.settings.lock().unwrap().role;
        if matches!(role, super::WebRTCSignallerRole::Consumer) {
            let target_producer = self.producer_peer_id().unwrap();

            self.send(p::IncomingMessage::StartSession(p::StartSessionMessage {
                peer_id: target_producer.clone(),
                offer: None,
            }));

            gst::info!(
                CAT,
                imp = self,
                "Started session with producer peer id {target_producer}",
            );
        }
    }

    fn handle_message(
        &self,
        msg: Result<WsMessage, async_tungstenite::tungstenite::Error>,
        meta: &Option<serde_json::Value>,
    ) -> ControlFlow<()> {
        match msg {
            Ok(WsMessage::Text(msg)) => {
                gst::trace!(CAT, imp = self, "Received message {}", msg);

                if let Ok(msg) = serde_json::from_str::<p::OutgoingMessage>(&msg) {
                    match msg {
                        p::OutgoingMessage::Welcome { peer_id } => {
                            self.set_status(meta, &peer_id);
                            if self.producer_peer_id().is_some() {
                                self.start_session();
                            } else if self.connect_to_first_producer() {
                                self.send(p::IncomingMessage::List);
                            }
                        }
                        p::OutgoingMessage::PeerStatusChanged(p::PeerStatus {
                            meta,
                            roles,
                            peer_id,
                        }) => {
                            let meta = meta.and_then(|m| match m {
                                serde_json::Value::Object(v) => Some(serialize_json_object(&v)),
                                _ => {
                                    gst::error!(CAT, imp = self, "Invalid json value: {m:?}");
                                    None
                                }
                            });

                            let peer_id =
                                peer_id.expect("Status changed should always contain a peer ID");
                            let mut state = self.state.lock().unwrap();
                            if roles.iter().any(|r| matches!(r, p::PeerRole::Producer)) {
                                if !state.producers.contains(&peer_id) {
                                    state.producers.insert(peer_id.clone());
                                    drop(state);

                                    self.obj().emit_by_name::<()>(
                                        "producer-added",
                                        &[&peer_id, &meta, &true],
                                    );
                                }
                            } else if state.producers.remove(&peer_id) {
                                drop(state);

                                self.obj()
                                    .emit_by_name::<()>("producer-removed", &[&peer_id, &meta]);
                            }
                        }
                        p::OutgoingMessage::SessionStarted {
                            peer_id,
                            session_id,
                        } => {
                            self.obj()
                                .emit_by_name::<()>("session-started", &[&session_id, &peer_id]);
                        }
                        p::OutgoingMessage::StartSession {
                            session_id,
                            peer_id,
                            offer,
                        } => {
                            assert!(matches!(
                                self.obj().property::<WebRTCSignallerRole>("role"),
                                super::WebRTCSignallerRole::Producer
                            ));

                            let sdp = {
                                if let Some(offer) = offer {
                                    match gst_sdp::SDPMessage::parse_buffer(offer.as_bytes()) {
                                        Ok(sdp) => Some(sdp),
                                        Err(err) => {
                                            self.obj().emit_by_name::<()>(
                                                "error",
                                                &[&format!("Error parsing SDP: {offer} {err:?}")],
                                            );

                                            return ControlFlow::Break(());
                                        }
                                    }
                                } else {
                                    None
                                }
                            };

                            self.obj().emit_by_name::<()>(
                                "session-requested",
                                &[
                                    &session_id,
                                    &peer_id,
                                    &sdp.map(|sdp| {
                                        gst_webrtc::WebRTCSessionDescription::new(
                                            gst_webrtc::WebRTCSDPType::Offer,
                                            sdp,
                                        )
                                    }),
                                ],
                            );
                        }
                        p::OutgoingMessage::EndSession(p::EndSessionMessage { session_id }) => {
                            gst::info!(CAT, imp = self, "Session {session_id} ended");

                            self.obj()
                                .emit_by_name::<bool>("session-ended", &[&session_id]);
                        }
                        p::OutgoingMessage::Peer(p::PeerMessage {
                            session_id,
                            peer_message,
                        }) => match peer_message {
                            p::PeerMessageInner::Sdp(reply) => {
                                let (sdp, desc_type) = match reply {
                                    p::SdpMessage::Answer { sdp } => {
                                        (sdp, gst_webrtc::WebRTCSDPType::Answer)
                                    }
                                    p::SdpMessage::Offer { sdp } => {
                                        (sdp, gst_webrtc::WebRTCSDPType::Offer)
                                    }
                                };
                                let sdp = match gst_sdp::SDPMessage::parse_buffer(sdp.as_bytes()) {
                                    Ok(sdp) => sdp,
                                    Err(err) => {
                                        self.obj().emit_by_name::<()>(
                                            "error",
                                            &[&format!("Error parsing SDP: {sdp} {err:?}")],
                                        );

                                        return ControlFlow::Break(());
                                    }
                                };

                                let desc =
                                    gst_webrtc::WebRTCSessionDescription::new(desc_type, sdp);
                                self.obj().emit_by_name::<()>(
                                    "session-description",
                                    &[&session_id, &desc],
                                );
                            }
                            p::PeerMessageInner::Ice {
                                candidate,
                                sdp_m_line_index,
                            } => {
                                let sdp_mid: Option<String> = None;
                                self.obj().emit_by_name::<()>(
                                    "handle-ice",
                                    &[&session_id, &sdp_m_line_index, &sdp_mid, &candidate],
                                );
                            }
                        },
                        p::OutgoingMessage::List { producers } => {
                            let mut settings = self.settings.lock().unwrap();
                            let role = settings.role;
                            if matches!(role, super::WebRTCSignallerRole::Consumer)
                                && settings.connect_to_first_producer
                                && settings.producer_peer_id.is_none()
                            {
                                if let Some(producer) = producers.first() {
                                    settings.producer_peer_id = Some(producer.id.clone());

                                    drop(settings);

                                    self.start_session();

                                    return ControlFlow::Continue(());
                                }
                            }
                            for producer in producers {
                                let mut state = self.state.lock().unwrap();
                                if !state.producers.contains(&producer.id) {
                                    state.producers.insert(producer.id.clone());
                                    drop(state);

                                    let meta = producer.meta.and_then(|m| match m {
                                        serde_json::Value::Object(v) => {
                                            Some(serialize_json_object(&v))
                                        }
                                        _ => {
                                            gst::error!(
                                                CAT,
                                                imp = self,
                                                "Invalid json value: {m:?}"
                                            );
                                            None
                                        }
                                    });

                                    self.obj().emit_by_name::<()>(
                                        "producer-added",
                                        &[&producer.id, &meta, &false],
                                    );
                                }
                            }
                        }
                        p::OutgoingMessage::ListConsumers { .. } => todo!(),
                        p::OutgoingMessage::Error { details } => {
                            self.obj().emit_by_name::<()>(
                                "error",
                                &[&format!("Error message from server: {details}")],
                            );
                        }
                    }
                } else {
                    gst::error!(CAT, imp = self, "Unknown message from server: {}", msg);

                    self.obj().emit_by_name::<()>(
                        "error",
                        &[&format!("Unknown message from server: {msg}")],
                    );
                }
            }
            Ok(WsMessage::Close(reason)) => {
                gst::info!(CAT, imp = self, "websocket connection closed: {:?}", reason);
                return ControlFlow::Break(());
            }
            Ok(_) => (),
            Err(err) => {
                self.obj()
                    .emit_by_name::<()>("error", &[&format!("Error receiving: {err}")]);
                return ControlFlow::Break(());
            }
        }
        ControlFlow::Continue(())
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Signaller {
    const NAME: &'static str = "GstWebRTCSignaller";
    type Type = super::Signaller;
    type ParentType = glib::Object;
    type Interfaces = (Signallable,);
}

impl ObjectImpl for Signaller {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPS: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecBoolean::builder("manual-sdp-munging")
                    .nick("Manual SDP munging")
                    .blurb("Whether the signaller manages SDP munging itself")
                    .default_value(false)
                    .read_only()
                    .build(),
                glib::ParamSpecString::builder("uri")
                    .nick("Signaller URI")
                    .blurb("URI for connecting to the signaller server")
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
                glib::ParamSpecString::builder("producer-peer-id")
                    .nick("Producer peer id")
                    .blurb("The peer id of the producer transmitted to the signaller server")
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
                glib::ParamSpecString::builder("cafile")
                    .nick("Certificate Authority (CA) file")
                    .blurb("Certificate file used in TLS session")
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
                glib::ParamSpecEnum::builder_with_default("role", WebRTCSignallerRole::Consumer)
                    .nick("Role")
                    .blurb("Role within the session (Consumer, Producer or Listener)")
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
                glib::ParamSpecString::builder("client-id")
                    .nick("Client id")
                    .blurb("The client id transmitted to the signaller server")
                    .flags(glib::ParamFlags::READABLE)
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Structure>("headers")
                    .nick("HTTP headers")
                    .blurb("HTTP headers sent during the connection handshake")
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
                /**
                 * GstWebRTCSignaller::insecure-tls:
                 *
                 * Enables insecure TLS connections. Disabled by default.
                 */
                glib::ParamSpecBoolean::builder("insecure-tls")
                    .nick("Insecure TLS")
                    .blurb("Whether insecure TLS connections are allowed")
                    .default_value(DEFAULT_INSECURE_TLS)
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
            ]
        });

        PROPS.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "uri" => {
                if let Err(e) = self.set_uri(value.get::<&str>().expect("type checked upstream")) {
                    gst::error!(CAT, "Couldn't set URI: {e:?}");
                }
            }
            "producer-peer-id" => {
                let mut settings = self.settings.lock().unwrap();

                if !matches!(settings.role, WebRTCSignallerRole::Consumer) {
                    gst::warning!(
                        CAT,
                        "Setting `producer-peer-id` doesn't make sense for {:?}",
                        settings.role
                    );
                } else {
                    settings.producer_peer_id = value
                        .get::<Option<String>>()
                        .expect("type checked upstream");
                }
            }
            "cafile" => {
                self.settings.lock().unwrap().cafile = value
                    .get::<Option<String>>()
                    .expect("type checked upstream")
            }
            "role" => {
                self.settings.lock().unwrap().role = value
                    .get::<WebRTCSignallerRole>()
                    .expect("type checked upstream")
            }
            "headers" => {
                self.settings.lock().unwrap().headers = value
                    .get::<Option<gst::Structure>>()
                    .expect("type checked upstream")
            }
            "insecure-tls" => {
                self.settings.lock().unwrap().insecure_tls =
                    value.get::<bool>().expect("type checked upstream")
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().unwrap();
        match pspec.name() {
            "manual-sdp-munging" => false.to_value(),
            "uri" => settings.uri.to_string().to_value(),
            "producer-peer-id" => {
                if !matches!(settings.role, WebRTCSignallerRole::Consumer) {
                    gst::warning!(
                        CAT,
                        "`producer-peer-id` doesn't make sense for {:?}",
                        settings.role
                    );
                }

                settings.producer_peer_id.to_value()
            }
            "cafile" => settings.cafile.to_value(),
            "role" => settings.role.to_value(),
            "client-id" => self.state.lock().unwrap().client_id.to_value(),
            "headers" => settings.headers.to_value(),
            "insecure-tls" => settings.insecure_tls.to_value(),
            _ => unimplemented!(),
        }
    }
}

impl SignallableImpl for Signaller {
    fn start(&self) {
        gst::info!(CAT, imp = self, "Starting");

        let mut state = self.state.lock().unwrap();
        let connect_task_handle = RUNTIME.spawn(glib::clone!(
            #[to_owned(rename_to = this)]
            self,
            async move {
                if let Err(err) = this.connect().await {
                    this.obj()
                        .emit_by_name::<()>("error", &[&format!("Error receiving: {err}")]);
                }
            }
        ));

        state.connect_task_handle = Some(connect_task_handle);
    }

    fn stop(&self) {
        gst::info!(CAT, imp = self, "Stopping now");

        let mut state = self.state.lock().unwrap();

        // First make sure the connect task is stopped if it is still
        // running
        let connect_task_handle = state.connect_task_handle.take();
        if let Some(handle) = connect_task_handle {
            RUNTIME.block_on(async move {
                handle.abort();
                let _ = handle.await;
            });
        }

        let send_task_handle = state.send_task_handle.take();
        let receive_task_handle = state.receive_task_handle.take();
        if let Some(mut sender) = state.websocket_sender.take() {
            RUNTIME.block_on(async move {
                sender.close_channel();

                if let Some(handle) = send_task_handle {
                    if let Err(err) = handle.await {
                        gst::warning!(CAT, imp = self, "Error while joining send task: {}", err);
                    }
                }

                if let Some(handle) = receive_task_handle {
                    handle.abort();
                    let _ = handle.await;
                }
            });
        }
        state.producers.clear();
        state.client_id = None;
    }

    fn send_sdp(&self, session_id: &str, sdp: &gst_webrtc::WebRTCSessionDescription) {
        gst::debug!(CAT, imp = self, "Sending SDP {sdp:#?}");

        let msg = p::IncomingMessage::Peer(p::PeerMessage {
            session_id: session_id.to_owned(),
            peer_message: p::PeerMessageInner::Sdp(
                if sdp.type_() == gst_webrtc::WebRTCSDPType::Offer {
                    p::SdpMessage::Offer {
                        sdp: sdp.sdp().as_text().unwrap(),
                    }
                } else {
                    p::SdpMessage::Answer {
                        sdp: sdp.sdp().as_text().unwrap(),
                    }
                },
            ),
        });

        self.send(msg);
    }

    fn add_ice(
        &self,
        session_id: &str,
        candidate: &str,
        sdp_m_line_index: u32,
        _sdp_mid: Option<String>,
    ) {
        gst::debug!(
            CAT,
            imp = self,
            "Adding ice candidate {candidate:?} for {sdp_m_line_index:?} on session {session_id}"
        );

        let msg = p::IncomingMessage::Peer(p::PeerMessage {
            session_id: session_id.to_string(),
            peer_message: p::PeerMessageInner::Ice {
                candidate: candidate.to_string(),
                sdp_m_line_index,
            },
        });

        self.send(msg);
    }

    fn end_session(&self, session_id: &str) {
        gst::debug!(CAT, imp = self, "Signalling session done {}", session_id);

        let state = self.state.lock().unwrap();
        let session_id = session_id.to_string();
        if let Some(mut sender) = state.websocket_sender.clone() {
            RUNTIME.spawn(glib::clone!(
                #[to_owned(rename_to = this)]
                self,
                async move {
                    if let Err(err) = sender
                        .send(p::IncomingMessage::EndSession(p::EndSessionMessage {
                            session_id,
                        }))
                        .await
                    {
                        this.obj()
                            .emit_by_name::<()>("error", &[&format!("Error: {err}")]);
                    }
                }
            ));
        }
    }
}
