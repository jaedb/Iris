// SPDX-License-Identifier: MPL-2.0

use crate::signaller::{Signallable, SignallableImpl, WebRTCSignallerRole};

use crate::utils::{wait_async, WaitError};
use crate::RUNTIME;

use anyhow::anyhow;
use futures::executor::block_on;
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::LazyLock;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::{Duration, Instant};

use livekit_api::access_token::{AccessToken, VideoGrants};
use livekit_api::signal_client;
use livekit_protocol::{self as proto, UpdateTrackSettings};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "webrtc-livekit-signaller",
        gst::DebugColorFlags::empty(),
        Some("WebRTC LiveKit signaller"),
    )
});

const DEFAULT_TRACK_PUBLISH_TIMEOUT: u32 = 10;

#[derive(Clone)]
struct Settings {
    wsurl: Option<String>,
    api_key: Option<String>,
    secret_key: Option<String>,
    participant_name: Option<String>,
    identity: Option<String>,
    room_name: Option<String>,
    auth_token: Option<String>,
    role: WebRTCSignallerRole,
    producer_peer_id: Option<String>,
    excluded_produder_peer_ids: Vec<String>,
    timeout: u32,
    room_timeout: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            wsurl: Some("ws://127.0.0.1:7880".to_string()),
            api_key: None,
            secret_key: None,
            participant_name: Some("GStreamer".to_string()),
            identity: Some("gstreamer".to_string()),
            room_name: None,
            auth_token: None,
            role: WebRTCSignallerRole::default(),
            producer_peer_id: None,
            excluded_produder_peer_ids: vec![],
            timeout: DEFAULT_TRACK_PUBLISH_TIMEOUT,
            room_timeout: 0,
        }
    }
}

#[derive(Default)]
pub struct Signaller {
    settings: Mutex<Settings>,
    connection: Mutex<Option<Connection>>,
    join_canceller: Mutex<Option<futures::future::AbortHandle>>,
    signal_task_canceller: Mutex<Option<futures::future::AbortHandle>>,
    room_timeout_task_canceller: Mutex<Option<futures::future::AbortHandle>>,
}

struct Channels {
    reliable_channel: gst_webrtc::WebRTCDataChannel,
    lossy_channel: gst_webrtc::WebRTCDataChannel,
}

struct Connection {
    signal_client: Arc<signal_client::SignalClient>,
    pending_tracks: HashMap<String, oneshot::Sender<proto::TrackInfo>>,
    signal_task: JoinHandle<()>,
    early_candidates: Option<Vec<String>>,
    channels: Option<Channels>,
    participants: HashMap<String, proto::ParticipantInfo>,
    state: ConnectionState,
    room_timeout_task: Option<JoinHandle<()>>,
    last_participant_lost_at: Option<Instant>,
    our_participant_sid: String,
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, glib::Enum)]
#[enum_type(name = "GstWebRTCLiveKitConnectionState")]
enum ConnectionState {
    #[default]
    #[enum_value(name = "server-disconnected")]
    ServerDisconnected,
    #[enum_value(name = "server-connected")]
    ServerConnected,
    #[enum_value(name = "publishing")]
    Publishing,
    #[enum_value(name = "published")]
    Published,
    #[enum_value(name = "subscribed")]
    Subscribed,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IceCandidateJson {
    pub sdp_mid: String,
    pub sdp_m_line_index: i32,
    pub candidate: String,
}

impl Signaller {
    fn raise_error(&self, msg: String) {
        self.obj()
            .emit_by_name::<()>("error", &[&format!("Error: {msg}")]);
    }

    fn role(&self) -> Option<WebRTCSignallerRole> {
        self.settings.lock().map(|s| s.role).ok()
    }

    fn is_subscriber(&self) -> bool {
        matches!(self.role(), Some(WebRTCSignallerRole::Consumer))
    }

    fn producer_peer_id(&self) -> Option<String> {
        assert!(self.is_subscriber());
        let settings = self.settings.lock().ok()?;
        settings.producer_peer_id.clone()
    }

    fn auto_subscribe(&self) -> bool {
        self.is_subscriber()
            && self.producer_peer_id().is_none()
            && self.excluded_producer_peer_ids_is_empty()
    }

    fn signal_target(&self) -> Option<proto::SignalTarget> {
        match self.role()? {
            WebRTCSignallerRole::Consumer => Some(proto::SignalTarget::Subscriber),
            WebRTCSignallerRole::Producer => Some(proto::SignalTarget::Publisher),
            _ => None,
        }
    }

    fn excluded_producer_peer_ids_is_empty(&self) -> bool {
        assert!(self.is_subscriber());
        self.settings
            .lock()
            .unwrap()
            .excluded_produder_peer_ids
            .is_empty()
    }

    fn is_peer_excluded(&self, peer_id: &str) -> bool {
        self.settings
            .lock()
            .unwrap()
            .excluded_produder_peer_ids
            .iter()
            .any(|id| id == peer_id)
    }

    fn signal_client(&self) -> Option<Arc<signal_client::SignalClient>> {
        let connection = self.connection.lock().unwrap();
        Some(connection.as_ref()?.signal_client.clone())
    }

    fn require_signal_client(&self) -> Arc<signal_client::SignalClient> {
        self.signal_client().unwrap()
    }

    async fn send_trickle_request(&self, candidate_init: &str) {
        let Some(signal_client) = self.signal_client() else {
            return;
        };
        let Some(target) = self.signal_target() else {
            return;
        };
        signal_client
            .send(proto::signal_request::Message::Trickle(
                proto::TrickleRequest {
                    candidate_init: candidate_init.to_string(),
                    target: target as i32,
                },
            ))
            .await;
    }

    async fn send_delayed_ice_candidates(&self) {
        let Some(mut early_candidates) = self
            .connection
            .lock()
            .unwrap()
            .as_mut()
            .and_then(|c| c.early_candidates.take())
        else {
            return;
        };

        while let Some(candidate_str) = early_candidates.pop() {
            gst::debug!(
                CAT,
                imp = self,
                "Sending delayed ice candidate {candidate_str:?}"
            );
            self.send_trickle_request(&candidate_str).await;
        }
    }

    async fn signal_task(&self, mut signal_events: signal_client::SignalEvents) {
        loop {
            match wait_async(&self.signal_task_canceller, signal_events.recv(), 0).await {
                Ok(Some(signal)) => match signal {
                    signal_client::SignalEvent::Message(signal) => {
                        self.on_signal_event(*signal).await;
                    }
                    signal_client::SignalEvent::Close(reason) => {
                        gst::debug!(CAT, imp = self, "Close: {reason}");
                        self.raise_error("Server disconnected".to_string());
                        break;
                    }
                },
                Ok(None) => {}
                Err(err) => match err {
                    WaitError::FutureAborted => {
                        gst::debug!(CAT, imp = self, "Closing signal_task");
                        break;
                    }
                    WaitError::FutureError(err) => self.raise_error(err.to_string()),
                },
            }
        }
    }

    async fn on_signal_event(&self, event: proto::signal_response::Message) {
        match event {
            proto::signal_response::Message::Answer(answer) => {
                gst::debug!(CAT, imp = self, "Received publisher answer: {:?}", answer);
                let sdp = match gst_sdp::SDPMessage::parse_buffer(answer.sdp.as_bytes()) {
                    Ok(sdp) => sdp,
                    Err(_) => {
                        self.raise_error("Couldn't parse Answer SDP".to_string());
                        return;
                    }
                };
                let answer = gst_webrtc::WebRTCSessionDescription::new(
                    gst_webrtc::WebRTCSDPType::Answer,
                    sdp,
                );
                self.obj()
                    .emit_by_name::<()>("session-description", &[&"unique", &answer]);
                if let Some(connection) = &mut *self.connection.lock().unwrap() {
                    connection.state = ConnectionState::Published;
                }
                self.obj().notify("connection-state");
            }

            proto::signal_response::Message::Offer(offer) => {
                if !self.is_subscriber() {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Ignoring subscriber offer in non-subscriber mode: {:?}",
                        offer
                    );
                    return;
                }
                gst::debug!(CAT, imp = self, "Received subscriber offer: {:?}", offer);
                let sdp = match gst_sdp::SDPMessage::parse_buffer(offer.sdp.as_bytes()) {
                    Ok(sdp) => sdp,
                    Err(_) => {
                        self.raise_error("Couldn't parse Offer SDP".to_string());
                        return;
                    }
                };
                let offer = gst_webrtc::WebRTCSessionDescription::new(
                    gst_webrtc::WebRTCSDPType::Offer,
                    sdp,
                );
                self.obj()
                    .emit_by_name::<()>("session-description", &[&"unique", &offer]);
            }

            proto::signal_response::Message::Trickle(trickle) => {
                gst::debug!(CAT, imp = self, "Received ice_candidate {:?}", trickle);

                let Some(target) = self.signal_target() else {
                    return;
                };

                if target == trickle.target() {
                    if let Ok(json) =
                        serde_json::from_str::<IceCandidateJson>(&trickle.candidate_init)
                    {
                        let mline = json.sdp_m_line_index as u32;
                        self.obj().emit_by_name::<()>(
                            "handle-ice",
                            &[&"unique", &mline, &Some(json.sdp_mid), &json.candidate],
                        );
                    }
                }
            }

            proto::signal_response::Message::ConnectionQuality(quality) => {
                gst::debug!(CAT, imp = self, "Connection quality: {:?}", quality);
            }

            proto::signal_response::Message::TrackPublished(publish_res) => {
                gst::debug!(CAT, imp = self, "Track published: {:?}", publish_res);
                if let Some(connection) = &mut *self.connection.lock().unwrap() {
                    if let Some(tx) = connection.pending_tracks.remove(&publish_res.cid) {
                        let _ = tx.send(publish_res.track.unwrap());
                    }
                }
            }

            proto::signal_response::Message::Update(update) => {
                gst::debug!(CAT, imp = self, "Update: {:?}", update);
                for participant in update.participants {
                    self.on_participant(&participant, true)
                }
            }

            proto::signal_response::Message::Leave(leave) => {
                gst::debug!(CAT, imp = self, "Leave: {:?}", leave);
            }

            _ => {}
        }
    }

    fn start_room_timeout_task(&self) {
        let Some(connection) = &mut *self.connection.lock().unwrap() else {
            return;
        };

        let weak_imp = self.downgrade();
        let room_timeout_task = RUNTIME.spawn(async move {
            if let Some(imp) = weak_imp.upgrade() {
                imp.room_timeout_task().await;
            }
        });
        connection.room_timeout_task = Some(room_timeout_task);
    }

    fn stop_room_timeout_task(&self) {
        if let Some(canceller) = self.room_timeout_task_canceller.lock().unwrap().take() {
            canceller.abort();
        }
        let room_timeout_task = self
            .connection
            .lock()
            .unwrap()
            .as_mut()
            .and_then(|connection| connection.room_timeout_task.take());
        if let Some(room_timeout_task) = room_timeout_task {
            block_on(room_timeout_task).unwrap();
        }
    }

    async fn room_timeout_task(&self) {
        loop {
            let room_timeout = self.settings.lock().unwrap().clone().room_timeout;
            if room_timeout == 0 {
                return;
            }
            let room_timeout = Duration::from_millis(room_timeout as u64);
            let now = Instant::now();
            let end = {
                let Some(state) = &*self.connection.lock().unwrap() else {
                    return;
                };
                state
                    .last_participant_lost_at
                    .map(|initial| initial + room_timeout)
                    .unwrap_or(now + Duration::from_secs(60))
            };

            if end < now {
                self.raise_error("Room timeout reached with no other participants".to_owned());
                return;
            }
            let fut = tokio::time::sleep_until(end);

            match wait_async(&self.room_timeout_task_canceller, fut, 0).await {
                Ok(()) => (),
                Err(err) => match err {
                    WaitError::FutureAborted => {
                        gst::debug!(CAT, imp = self, "Closing room_timeout_task");
                        break;
                    }
                    WaitError::FutureError(err) => self.raise_error(err.to_string()),
                },
            }
        }
    }

    fn send_sdp_answer(&self, _session_id: &str, sessdesc: &gst_webrtc::WebRTCSessionDescription) {
        let weak_imp = self.downgrade();
        let sessdesc = sessdesc.clone();

        RUNTIME.spawn(async move {
            if let Some(imp) = weak_imp.upgrade() {
                let sdp = sessdesc.sdp();
                gst::debug!(CAT, imp = imp, "Sending SDP {:?} now", &sdp);
                let signal_client = imp.require_signal_client();
                signal_client
                    .send(proto::signal_request::Message::Answer(
                        proto::SessionDescription {
                            r#type: "answer".to_string(),
                            sdp: sdp.to_string(),
                        },
                    ))
                    .await;
                imp.send_delayed_ice_candidates().await;

                if let Some(connection) = &mut *imp.connection.lock().unwrap() {
                    connection.state = ConnectionState::Subscribed;
                }
                imp.obj().notify("connection-state");
            }
        });
    }

    fn send_sdp_offer(&self, _session_id: &str, sessdesc: &gst_webrtc::WebRTCSessionDescription) {
        let weak_imp = self.downgrade();
        let sessdesc = sessdesc.clone();
        RUNTIME.spawn(async move {
            if let Some(imp) = weak_imp.upgrade() {
                let sdp = sessdesc.sdp();
                let signal_client = imp.require_signal_client();
                let timeout = imp.settings.lock().unwrap().timeout;

                for media in sdp.medias() {
                    if let Some(mediatype) = media.media() {
                        let (mtype, msource) = if mediatype == "audio" {
                            (
                                proto::TrackType::Audio,
                                proto::TrackSource::Microphone as i32,
                            )
                        } else if mediatype == "video" {
                            (proto::TrackType::Video, proto::TrackSource::Camera as i32)
                        } else {
                            continue;
                        };

                        let mut disable_red = true;

                        if mtype == proto::TrackType::Audio {
                            for format in media.formats() {
                                if let Ok(pt) = format.parse::<i32>() {
                                    if let Some(caps) = media.caps_from_media(pt) {
                                        let s = caps.structure(0).unwrap();
                                        let encoding_name = s.get::<&str>("encoding-name").unwrap();
                                        if encoding_name == "RED" {
                                            disable_red = false;
                                        }
                                    }
                                }
                            }
                        }

                        // Our SDP should always have a mid
                        let mid = media.attribute_val("mid").unwrap().to_string();

                        let mut trackid = "";
                        for attr in media.attributes() {
                            if attr.key() == "ssrc" {
                                if let Some(val) = attr.value() {
                                    let split: Vec<&str> = val.split_whitespace().collect();
                                    if split.len() == 3 && split[1].starts_with("msid:") {
                                        trackid = split[2];
                                        break;
                                    }
                                }
                            }
                        }

                        let layers = if mtype == proto::TrackType::Video {
                            vec![proto::VideoLayer {
                                quality: proto::VideoQuality::High as i32,
                                ..Default::default()
                            }]
                        } else {
                            vec![]
                        };

                        let req = proto::AddTrackRequest {
                            cid: trackid.to_string(),
                            name: mid.clone(),
                            r#type: mtype as i32,
                            muted: false,
                            source: msource,
                            disable_dtx: true,
                            disable_red,
                            layers,
                            ..Default::default()
                        };

                        let (tx, rx) = oneshot::channel();
                        if let Some(connection) = &mut *imp.connection.lock().unwrap() {
                            let pendings_tracks = &mut connection.pending_tracks;
                            if pendings_tracks.contains_key(&req.cid) {
                                panic!("track already published");
                            }
                            pendings_tracks.insert(req.cid.clone(), tx);
                        }

                        let cid = req.cid.clone();

                        if let Some(connection) = &mut *imp.connection.lock().unwrap() {
                            connection.state = ConnectionState::Publishing;
                        }
                        imp.obj().notify("connection-state");

                        signal_client
                            .send(proto::signal_request::Message::AddTrack(req))
                            .await;

                        if let Err(err) = wait_async(&imp.join_canceller, rx, timeout).await {
                            if let Some(connection) = &mut *imp.connection.lock().unwrap() {
                                connection.pending_tracks.remove(&cid);
                            }

                            match err {
                                WaitError::FutureAborted => {
                                    gst::warning!(CAT, imp = imp, "Future aborted")
                                }
                                WaitError::FutureError(err) => imp.raise_error(err.to_string()),
                            };
                        }
                    }
                }

                gst::debug!(CAT, imp = imp, "Sending SDP now");
                signal_client
                    .send(proto::signal_request::Message::Offer(
                        proto::SessionDescription {
                            r#type: "offer".to_string(),
                            sdp: sessdesc.sdp().to_string(),
                        },
                    ))
                    .await;

                if let Some(imp) = weak_imp.upgrade() {
                    imp.send_delayed_ice_candidates().await;
                }
            }
        });
    }

    fn on_participant(&self, participant: &proto::ParticipantInfo, new_connection: bool) {
        gst::debug!(CAT, imp = self, "{:?}", participant);
        let peer_sid = &participant.sid;
        let peer_identity = &participant.identity;

        if participant.is_publisher && self.is_subscriber() {
            match self.producer_peer_id() {
                Some(id) if id == *peer_sid => {
                    gst::debug!(CAT, imp = self, "matching peer sid {id:?}");
                }
                Some(id) if id == *peer_identity => {
                    gst::debug!(CAT, imp = self, "matching peer identity {id:?}");
                }
                None => {
                    if self.is_peer_excluded(peer_sid) || self.is_peer_excluded(peer_identity) {
                        gst::debug!(CAT, imp = self, "ignoring excluded peer {participant:?}");
                        return;
                    }
                    gst::debug!(CAT, imp = self, "catch-all mode, matching {participant:?}");
                }
                _ => return,
            }
            return;
        }

        let mut connection = self.connection.lock().unwrap();
        if let Some(ref mut connection) = &mut *connection {
            if participant.state == proto::participant_info::State::Disconnected as i32 {
                connection.participants.remove(&participant.sid);
                if connection
                    .participants
                    .iter()
                    .next()
                    .is_none_or(|(id, _part)| id == &connection.our_participant_sid)
                {
                    gst::debug!(
                        CAT,
                        imp = self,
                        "no other participants, starting room timeout"
                    );
                    connection.last_participant_lost_at = Some(Instant::now());
                }
            } else if participant.sid != connection.our_participant_sid {
                if !connection.participants.contains_key(&participant.sid) {
                    connection
                        .participants
                        .insert(participant.sid.clone(), participant.clone());
                }
                gst::debug!(
                    CAT,
                    imp = self,
                    "have at least one other participant, unsetting room-timeout"
                );
                connection.last_participant_lost_at = None;
            }
        }

        if !participant.is_publisher || !self.is_subscriber() {
            return;
        }

        let meta = Some(&participant.metadata)
            .filter(|meta| !meta.is_empty())
            .and_then(|meta| gst::Structure::from_str(meta).ok());
        match participant.state {
            x if x == proto::participant_info::State::Active as i32 => {
                let track_sids = participant
                    .tracks
                    .iter()
                    .filter(|t| !t.muted)
                    .map(|t| t.sid.clone())
                    .collect::<Vec<_>>();
                let update = proto::UpdateSubscription {
                    track_sids: track_sids.clone(),
                    subscribe: true,
                    participant_tracks: vec![proto::ParticipantTracks {
                        participant_sid: participant.sid.clone(),
                        track_sids: track_sids.clone(),
                    }],
                };
                let update = proto::signal_request::Message::Subscription(update);
                let weak_imp = self.downgrade();
                let peer_sid = peer_sid.clone();
                RUNTIME.spawn(async move {
                    let imp = match weak_imp.upgrade() {
                        Some(imp) => imp,
                        None => return,
                    };
                    let signal_client = imp.require_signal_client();
                    signal_client.send(update).await;
                    imp.obj()
                        .emit_by_name::<()>("producer-added", &[&peer_sid, &meta, &new_connection]);
                });
            }
            _ => {
                self.obj()
                    .emit_by_name::<()>("producer-removed", &[&peer_sid, &meta]);
            }
        }
    }

    async fn close_signal_client(signal_client: &signal_client::SignalClient) {
        signal_client
            .send(proto::signal_request::Message::Leave(proto::LeaveRequest {
                can_reconnect: false,
                reason: proto::DisconnectReason::ClientInitiated as i32,
                ..Default::default()
            }))
            .await;
        signal_client.close().await;
    }

    pub(crate) fn participant_info(&self, participant_sid: &str) -> Option<proto::ParticipantInfo> {
        let connection = self.connection.lock().unwrap();
        let connection = connection.as_ref()?;
        let participant = connection.participants.get(participant_sid)?;
        Some(participant.clone())
    }

    pub(crate) fn set_track_disabled(&self, track_sid: &str, disabled: bool) {
        let update = proto::signal_request::Message::TrackSetting(UpdateTrackSettings {
            track_sids: vec![String::from(track_sid)],
            disabled,
            ..Default::default()
        });
        let weak_imp = self.downgrade();
        RUNTIME.spawn(async move {
            let imp = match weak_imp.upgrade() {
                Some(imp) => imp,
                None => return,
            };
            let signal_client = imp.require_signal_client();
            signal_client.send(update).await;
        });
    }
}

impl SignallableImpl for Signaller {
    fn start(&self) {
        gst::debug!(CAT, imp = self, "Connecting");

        let wsurl = if let Some(wsurl) = &self.settings.lock().unwrap().wsurl {
            wsurl.clone()
        } else {
            self.raise_error("WebSocket URL must be set".to_string());
            return;
        };

        let auth_token = {
            let settings = self.settings.lock().unwrap();
            let role = settings.role;

            if let Some(auth_token) = &settings.auth_token {
                auth_token.clone()
            } else if let (
                Some(api_key),
                Some(secret_key),
                Some(identity),
                Some(participant_name),
                Some(room_name),
            ) = (
                &settings.api_key,
                &settings.secret_key,
                &settings.identity,
                &settings.participant_name,
                &settings.room_name,
            ) {
                let grants = VideoGrants {
                    room_join: true,
                    can_subscribe: role == WebRTCSignallerRole::Consumer,
                    room: room_name.clone(),
                    ..Default::default()
                };
                let access_token = AccessToken::with_api_key(api_key, secret_key)
                    .with_name(participant_name)
                    .with_identity(identity)
                    .with_grants(grants);
                match access_token.to_jwt() {
                    Ok(token) => token,
                    Err(err) => {
                        self.raise_error(format!(
                            "{:?}",
                            anyhow!("Could not create auth token {err}")
                        ));
                        return;
                    }
                }
            } else {
                self.raise_error("Either auth-token or (api-key and secret-key and identity and room-name) must be set".to_string());
                return;
            }
        };

        gst::debug!(CAT, imp = self, "We have an authentication token");

        let weak_imp = self.downgrade();
        RUNTIME.spawn(async move {
            let Some(imp) = weak_imp.upgrade() else {
                return;
            };

            let options = signal_client::SignalOptions {
                auto_subscribe: imp.auto_subscribe(),
                ..Default::default()
            };
            gst::debug!(CAT, imp = imp, "Connecting to {}", wsurl);

            let res = signal_client::SignalClient::connect(&wsurl, &auth_token, options).await;
            let (signal_client, join_response, signal_events) = match res {
                Err(err) => {
                    imp.obj()
                        .emit_by_name::<()>("error", &[&format!("{:?}", anyhow!("Error: {err}"))]);
                    return;
                }
                Ok(ok) => ok,
            };
            let signal_client = Arc::new(signal_client);

            gst::debug!(
                CAT,
                imp = imp,
                "Connected with JoinResponse: {:?}",
                join_response
            );

            let weak_imp = imp.downgrade();
            let signal_task = RUNTIME.spawn(async move {
                if let Some(imp) = weak_imp.upgrade() {
                    imp.signal_task(signal_events).await;
                }
            });

            let mut connection = Connection {
                signal_client,
                signal_task,
                pending_tracks: Default::default(),
                early_candidates: Some(Vec::new()),
                channels: None,
                participants: HashMap::default(),
                state: ConnectionState::ServerConnected,
                last_participant_lost_at: Some(Instant::now()),
                room_timeout_task: None,
                our_participant_sid: join_response
                    .participant
                    .map(|p| p.sid.clone())
                    .unwrap_or_default(),
            };
            if !join_response.other_participants.is_empty() {
                connection.last_participant_lost_at = None;
            }
            *imp.connection.lock().unwrap() = Some(connection);
            imp.start_room_timeout_task();
            imp.obj().notify("connection-state");

            if imp.is_subscriber() {
                imp.obj()
                    .emit_by_name::<()>("session-started", &[&"unique", &"unique"]);
                for participant in &join_response.other_participants {
                    imp.on_participant(participant, false)
                }
            }

            imp.obj().connect_closure(
                "webrtcbin-ready",
                false,
                glib::closure!(
                    #[watch(rename_to = obj)]
                    imp.obj(),
                    move |_signaller: &super::LiveKitSignaller,
                          _consumer_identifier: &str,
                          webrtcbin: &gst::Element| {
                        let imp = obj.imp();
                        gst::info!(CAT, "Adding data channels");
                        let reliable_channel = webrtcbin
                            .emit_by_name::<gst_webrtc::WebRTCDataChannel>(
                                "create-data-channel",
                                &[
                                    &"_reliable",
                                    &gst::Structure::builder("config")
                                        .field("ordered", true)
                                        .build(),
                                ],
                            );
                        let lossy_channel = webrtcbin
                            .emit_by_name::<gst_webrtc::WebRTCDataChannel>(
                                "create-data-channel",
                                &[
                                    &"_lossy",
                                    &gst::Structure::builder("config")
                                        .field("ordered", true)
                                        .field("max-retransmits", 0)
                                        .build(),
                                ],
                            );

                        let mut connection = imp.connection.lock().unwrap();
                        if let Some(connection) = connection.as_mut() {
                            connection.channels = Some(Channels {
                                reliable_channel,
                                lossy_channel,
                            });
                        }
                    }
                ),
            );

            imp.obj().emit_by_name::<()>(
                "session-requested",
                &[
                    &"unique",
                    &"unique",
                    &None::<gst_webrtc::WebRTCSessionDescription>,
                ],
            );
        });
    }

    fn send_sdp(&self, session_id: &str, sessdesc: &gst_webrtc::WebRTCSessionDescription) {
        gst::debug!(CAT, imp = self, "Created SDP {:?}", sessdesc.sdp());

        match sessdesc.type_() {
            gst_webrtc::WebRTCSDPType::Offer => {
                self.send_sdp_offer(session_id, sessdesc);
            }
            gst_webrtc::WebRTCSDPType::Answer => {
                self.send_sdp_answer(session_id, sessdesc);
            }
            _ => {
                gst::debug!(CAT, imp = self, "Ignoring SDP {:?}", sessdesc.sdp());
            }
        }
    }

    fn add_ice(
        &self,
        _session_id: &str,
        candidate: &str,
        sdp_m_line_index: u32,
        sdp_mid: Option<String>,
    ) {
        let candidate_str = serde_json::to_string(&IceCandidateJson {
            sdp_mid: sdp_mid.unwrap_or("".to_string()),
            sdp_m_line_index: sdp_m_line_index as i32,
            candidate: candidate.to_string(),
        })
        .unwrap();

        if let Some(connection) = &mut *self.connection.lock().unwrap() {
            if let Some(early_candidates) = connection.early_candidates.as_mut() {
                gst::debug!(CAT, imp = self, "Delaying ice candidate {candidate_str:?}");

                early_candidates.push(candidate_str);
                return;
            }
        };

        gst::debug!(CAT, imp = self, "Sending ice candidate {candidate_str:?}");

        let imp = self.downgrade();
        RUNTIME.spawn(async move {
            if let Some(imp) = imp.upgrade() {
                imp.send_trickle_request(&candidate_str).await;
            };
        });
    }

    fn stop(&self) {
        if let Some(canceller) = &*self.join_canceller.lock().unwrap() {
            canceller.abort();
        }
        if let Some(canceller) = &*self.signal_task_canceller.lock().unwrap() {
            canceller.abort();
        }
        if let Some(canceller) = &*self.room_timeout_task_canceller.lock().unwrap() {
            canceller.abort();
        }

        let connection = self.connection.lock().unwrap().take();
        if let Some(connection) = connection {
            block_on(connection.signal_task).unwrap();
            block_on(Self::close_signal_client(&connection.signal_client));
        }
        self.obj().notify("connection-state");
    }

    fn end_session(&self, session_id: &str) {
        assert_eq!(session_id, "unique");
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Signaller {
    const NAME: &'static str = "GstLiveKitWebRTCSinkSignaller";
    type Type = super::LiveKitSignaller;
    type ParentType = glib::Object;
    type Interfaces = (Signallable,);
}

impl ObjectImpl for Signaller {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecBoolean::builder("manual-sdp-munging")
                    .nick("Manual SDP munging")
                    .blurb("Whether the signaller manages SDP munging itself")
                    .default_value(false)
                    .read_only()
                    .build(),
                glib::ParamSpecString::builder("ws-url")
                    .nick("WebSocket URL")
                    .blurb("The URL of the websocket of the LiveKit server")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("api-key")
                    .nick("API key")
                    .blurb("API key (combined into auth-token)")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("secret-key")
                    .nick("Secret Key")
                    .blurb("Secret key (combined into auth-token)")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("participant-name")
                    .nick("Participant name")
                    .blurb("Human readable name of the participant (combined into auth-token)")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("identity")
                    .nick("Participant Identity")
                    .blurb("Identity of the participant (combined into auth-token)")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("auth-token")
                    .nick("Authorization Token")
                    .blurb("Authentication token to use (contains api_key/secret/name/identity)")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("room-name")
                    .nick("Room Name")
                    .blurb("Name of the room to join (mandatory)")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder("timeout")
                    .nick("Timeout")
                    .blurb("Value in seconds to timeout join requests.")
                    .maximum(3600)
                    .minimum(1)
                    .default_value(DEFAULT_TRACK_PUBLISH_TIMEOUT)
                    .build(),
                glib::ParamSpecObject::builder::<gst_webrtc::WebRTCDataChannel>("reliable-channel")
                    .nick("Reliable Channel")
                    .blurb("Reliable Data Channel object.")
                    .flags(glib::ParamFlags::READABLE)
                    .build(),
                glib::ParamSpecObject::builder::<gst_webrtc::WebRTCDataChannel>("lossy-channel")
                    .nick("Lossy Channel")
                    .blurb("Lossy Data Channel object.")
                    .flags(glib::ParamFlags::READABLE)
                    .build(),
                glib::ParamSpecEnum::builder_with_default("role", WebRTCSignallerRole::default())
                    .nick("Sigaller Role")
                    .blurb("Whether this signaller acts as either a Consumer or Producer. Listener is not currently supported.")
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
                glib::ParamSpecString::builder("producer-peer-id")
                    .nick("Producer Peer ID")
                    .blurb("When in Consumer Role, the signaller will subscribe to this peer's tracks.")
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
                gst::ParamSpecArray::builder("excluded-producer-peer-ids")
                    .nick("Excluded Producer Peer IDs")
                    .blurb("When in Consumer Role, the signaller will not subscribe to these peers' tracks.")
                    .flags(glib::ParamFlags::READWRITE)
                    .element_spec(&glib::ParamSpecString::builder("producer-peer-id").build())
                    .build(),
                glib::ParamSpecEnum::builder::<ConnectionState>("connection-state")
                    .nick("Connection State")
                    .blurb("Connection State with the LiveKit server")
                    .default_value(ConnectionState::ServerDisconnected)
                    .read_only()
                    .build(),
                glib::ParamSpecUInt::builder("room-timeout")
                    .nick("Room timeout")
                    .blurb("How many milliseconds to stay in the room if there are no other participants in the room (0 = disabled)")
                    .minimum(0)
                    .maximum(u32::MAX)
                    .default_value(0)
                    .flags(glib::ParamFlags::READWRITE)
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().unwrap();
        match pspec.name() {
            "ws-url" => {
                settings.wsurl = value.get().unwrap();
            }
            "api-key" => {
                settings.api_key = value.get().unwrap();
            }
            "secret-key" => {
                settings.secret_key = value.get().unwrap();
            }
            "participant-name" => {
                settings.participant_name = value.get().unwrap();
            }
            "identity" => {
                settings.identity = value.get().unwrap();
            }
            "room-name" => {
                settings.room_name = value.get().unwrap();
            }
            "auth-token" => {
                settings.auth_token = value.get().unwrap();
            }
            "timeout" => {
                settings.timeout = value.get().unwrap();
            }
            "role" => settings.role = value.get().unwrap(),
            "producer-peer-id" => settings.producer_peer_id = value.get().unwrap(),
            "excluded-producer-peer-ids" => {
                settings.excluded_produder_peer_ids = value
                    .get::<gst::ArrayRef>()
                    .expect("type checked upstream")
                    .as_slice()
                    .iter()
                    .filter_map(|id| id.get::<&str>().ok())
                    .map(|id| id.to_string())
                    .collect::<Vec<String>>()
            }
            "room-timeout" => {
                let val = value.get().unwrap();
                if settings.room_timeout != val {
                    settings.room_timeout = val;
                    drop(settings);
                    self.stop_room_timeout_task();
                    self.start_room_timeout_task();
                }
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().unwrap();
        match pspec.name() {
            "manual-sdp-munging" => false.to_value(),
            "ws-url" => settings.wsurl.to_value(),
            "api-key" => settings.api_key.to_value(),
            "secret-key" => settings.secret_key.to_value(),
            "participant-name" => settings.participant_name.to_value(),
            "identity" => settings.identity.to_value(),
            "room-name" => settings.room_name.to_value(),
            "auth-token" => settings.auth_token.to_value(),
            "timeout" => settings.timeout.to_value(),
            channel @ ("reliable-channel" | "lossy-channel") => {
                let channel = if let Some(connection) = &*self.connection.lock().unwrap() {
                    if let Some(channels) = &connection.channels {
                        if channel == "reliable-channel" {
                            Some(channels.reliable_channel.clone())
                        } else {
                            Some(channels.lossy_channel.clone())
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };
                channel.to_value()
            }
            "role" => settings.role.to_value(),
            "producer-peer-id" => settings.producer_peer_id.to_value(),
            "excluded-producer-peer-ids" => {
                gst::Array::new(&settings.excluded_produder_peer_ids).to_value()
            }
            "connection-state" => {
                drop(settings);
                let connection = self.connection.lock().unwrap();
                let Some(connection) = connection.as_ref() else {
                    return ConnectionState::ServerDisconnected.to_value();
                };
                connection.state.to_value()
            }
            "room-timeout" => settings.room_timeout.to_value(),
            _ => unimplemented!(),
        }
    }
}
