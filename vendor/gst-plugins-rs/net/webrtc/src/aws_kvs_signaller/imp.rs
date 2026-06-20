// SPDX-License-Identifier: MPL-2.0

use super::protocol as p;
use crate::signaller::{Signallable, SignallableImpl};
use crate::utils::create_tls_connector;
use crate::RUNTIME;
use anyhow::{anyhow, Error};
use async_tungstenite::tungstenite::Message as WsMessage;
use futures::channel::mpsc;
use futures::prelude::*;
use gst::glib;
use gst::glib::prelude::*;
use gst::subclass::prelude::*;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::Mutex;
use tokio::task;

use aws_config::default_provider::credentials::DefaultCredentialsChain;
use aws_credential_types::{provider::ProvideCredentials, Credentials};
use aws_sdk_kinesisvideo::{
    types::{ChannelProtocol, ChannelRole, SingleMasterChannelEndpointConfiguration},
    Client,
};
use aws_sdk_kinesisvideosignaling::Client as SignalingClient;
use aws_sigv4::http_request::{
    sign, SignableBody, SignableRequest, SignatureLocation, SigningSettings,
};
use aws_sigv4::sign::v4;
use data_encoding::BASE64;
use http::Uri;
use std::time::{Duration, SystemTime};

const DEFAULT_AWS_REGION: &str = "us-east-1";
const DEFAULT_PING_TIMEOUT: i32 = 30;

#[allow(deprecated)]
pub static AWS_BEHAVIOR_VERSION: LazyLock<aws_config::BehaviorVersion> =
    LazyLock::new(aws_config::BehaviorVersion::v2023_11_09);

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "webrtc-aws-kvs-signaller",
        gst::DebugColorFlags::empty(),
        Some("WebRTC AWS KVS signaller"),
    )
});

#[derive(Default)]
struct State {
    /// Sender for the websocket messages
    websocket_sender: Option<mpsc::Sender<p::OutgoingMessage>>,
    send_task_handle: Option<task::JoinHandle<Result<(), Error>>>,
    receive_task_handle: Option<task::JoinHandle<()>>,
}

#[derive(Clone)]
struct Settings {
    address: Option<String>,
    cafile: Option<PathBuf>,
    access_key: Option<String>,
    secret_access_key: Option<String>,
    session_token: Option<String>,
    channel_name: Option<String>,
    ping_timeout: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            address: Some("ws://127.0.0.1:8443".to_string()),
            cafile: None,
            access_key: None,
            secret_access_key: None,
            session_token: None,
            channel_name: None,
            ping_timeout: DEFAULT_PING_TIMEOUT,
        }
    }
}

#[derive(Default)]
pub struct Signaller {
    state: Mutex<State>,
    settings: Mutex<Settings>,
}

impl Signaller {
    fn handle_message(&self, msg: async_tungstenite::tungstenite::Utf8Bytes) {
        if let Ok(msg) = serde_json::from_str::<p::IncomingMessage>(&msg) {
            match BASE64.decode(&msg.message_payload.into_bytes()) {
                Ok(payload) => {
                    let payload = String::from_utf8_lossy(&payload);
                    match msg.message_type.as_str() {
                        "SDP_OFFER" => {
                            if let Ok(sdp_msg) = serde_json::from_str::<p::SdpOffer>(&payload) {
                                gst::log!(
                                    CAT,
                                    "Consumer {} got SDP offer: {}",
                                    msg.sender_client_id,
                                    sdp_msg.sdp
                                );
                                self.obj().emit_by_name::<()>(
                                    "session-requested",
                                    &[
                                        &msg.sender_client_id,
                                        &msg.sender_client_id,
                                        &Some(gst_webrtc::WebRTCSessionDescription::new(
                                            gst_webrtc::WebRTCSDPType::Offer,
                                            gst_sdp::SDPMessage::parse_buffer(
                                                sdp_msg.sdp.as_bytes(),
                                            )
                                            .unwrap(),
                                        )),
                                    ],
                                );
                            } else {
                                gst::warning!(
                                    CAT,
                                    imp = self,
                                    "Failed to parse SDP_OFFER: {payload}"
                                );
                            }
                        }
                        "ICE_CANDIDATE" => {
                            if let Ok(ice_msg) = serde_json::from_str::<p::IceCandidate>(&payload) {
                                gst::log!(
                                    CAT,
                                    "Consumer {} got candidate {} for m_line {} and mid {}",
                                    msg.sender_client_id,
                                    ice_msg.candidate,
                                    ice_msg.sdp_m_line_index,
                                    ice_msg.sdp_mid
                                );
                                self.obj().emit_by_name::<()>(
                                    "handle-ice",
                                    &[
                                        &msg.sender_client_id,
                                        &ice_msg.sdp_m_line_index,
                                        &Some(ice_msg.sdp_mid),
                                        &ice_msg.candidate,
                                    ],
                                );
                            } else {
                                gst::warning!(
                                    CAT,
                                    imp = self,
                                    "Failed to parse ICE_CANDIDATE: {payload}"
                                );
                            }
                        }
                        _ => {
                            gst::log!(
                                CAT,
                                imp = self,
                                "Ignoring unsupported message type {}",
                                msg.message_type
                            );
                        }
                    }
                }
                Err(e) => {
                    gst::error!(
                        CAT,
                        imp = self,
                        "Failed to decode message payload from server: {e}"
                    );
                    self.obj().emit_by_name::<()>(
                        "error",
                        &[&format!(
                            "{:?}",
                            anyhow!("Failed to decode message payload from server: {e}")
                        )],
                    );
                }
            }
        } else {
            gst::log!(CAT, imp = self, "Unknown message from server: [{msg}]");
        }
    }

    async fn connect(&self) -> Result<(), Error> {
        let settings = self.settings.lock().unwrap().clone();

        let connector = create_tls_connector(settings.cafile.as_ref(), false)
            .map_ok(Some)
            .await?;

        let region = aws_config::meta::region::RegionProviderChain::default_provider()
            .or_else(DEFAULT_AWS_REGION)
            .region()
            .await
            .unwrap();
        let access_key = settings.access_key.as_ref();
        let secret_access_key = settings.secret_access_key.as_ref();
        let session_token = settings.session_token.clone();

        let credentials = match (access_key, secret_access_key) {
            (Some(key), Some(secret_key)) => {
                gst::debug!(
                    CAT,
                    imp = self,
                    "Using provided access and secret access key"
                );
                Ok(Credentials::new(
                    key.clone(),
                    secret_key.clone(),
                    session_token,
                    None,
                    "kvs",
                ))
            }
            _ => {
                gst::debug!(CAT, imp = self, "Using default AWS credentials");
                let cred = DefaultCredentialsChain::builder()
                    .region(region.clone())
                    .build()
                    .await;
                cred.provide_credentials().await
            }
        };

        let credentials = match credentials {
            Err(e) => {
                anyhow::bail!("Failed to retrieve credentials with error {e}");
            }
            Ok(credentials) => credentials,
        };

        let Some(channel_name) = settings.channel_name else {
            anyhow::bail!("Channel name cannot be None!");
        };

        let client = Client::new(
            &aws_config::defaults(*AWS_BEHAVIOR_VERSION)
                .credentials_provider(credentials.clone())
                .load()
                .await,
        );

        let resp = client
            .describe_signaling_channel()
            .set_channel_name(Some(channel_name.clone()))
            .send()
            .await?;

        let Some(cinfo) = resp.channel_info() else {
            anyhow::bail!("No description found for {channel_name}");
        };

        gst::debug!(CAT, "Channel description: {cinfo:?}");

        let Some(channel_arn) = cinfo.channel_arn() else {
            anyhow::bail!("No channel ARN found for {channel_name}");
        };

        let config = SingleMasterChannelEndpointConfiguration::builder()
            .set_protocols(Some(vec![ChannelProtocol::Wss, ChannelProtocol::Https]))
            .set_role(Some(ChannelRole::Master))
            .build();

        let resp = client
            .get_signaling_channel_endpoint()
            .set_channel_arn(Some(channel_arn.to_string()))
            .set_single_master_channel_endpoint_configuration(Some(config))
            .send()
            .await?;

        gst::debug!(CAT, "Endpoints: {:?}", resp.resource_endpoint_list());

        let endpoint_wss_uri = match resp.resource_endpoint_list().iter().find_map(|endpoint| {
            if endpoint.protocol == Some(ChannelProtocol::Wss) {
                Some(endpoint.resource_endpoint().unwrap().to_owned())
            } else {
                None
            }
        }) {
            Some(endpoint_uri_str) => Uri::from_maybe_shared(endpoint_uri_str).unwrap(),
            None => {
                anyhow::bail!("No WSS endpoint found for {channel_name}");
            }
        };

        let endpoint_https_uri = match resp.resource_endpoint_list().iter().find_map(|endpoint| {
            if endpoint.protocol == Some(ChannelProtocol::Https) {
                Some(endpoint.resource_endpoint().unwrap().to_owned())
            } else {
                None
            }
        }) {
            Some(endpoint_uri_str) => endpoint_uri_str,
            None => {
                anyhow::bail!("No HTTPS endpoint found for {channel_name}");
            }
        };

        gst::debug!(
            CAT,
            "Endpoints: {:?} {:?}",
            endpoint_wss_uri,
            endpoint_https_uri
        );

        let signaling_config = aws_sdk_kinesisvideosignaling::config::Builder::from(
            &aws_config::defaults(*AWS_BEHAVIOR_VERSION)
                .credentials_provider(credentials.clone())
                .load()
                .await,
        )
        .endpoint_url(endpoint_https_uri)
        .build();

        let signaling_client = SignalingClient::from_conf(signaling_config);

        let resp = signaling_client
            .get_ice_server_config()
            .set_channel_arn(Some(channel_arn.to_string()))
            .send()
            .await?;

        let ice_servers: Vec<String> = resp
            .ice_server_list()
            .iter()
            .filter_map(|server| {
                Option::zip(server.username(), server.password())
                    .map(|(username, password)| (username, password, server))
            })
            .flat_map(|(username, password, server)| {
                server
                    .uris()
                    .iter()
                    .filter_map(move |uri| {
                        uri.split_once(':').map(|(protocol, host)| {
                            let (timestamp, username) = username.split_once(':').unwrap();

                            format!("{protocol}://{timestamp}%3A{encoded_user_name}:{encoded_password}@{host}",
                                encoded_user_name=url_escape::encode_userinfo(username),
                                encoded_password=url_escape::encode_userinfo(password),
                            )
                        })
                    })
            })
            .collect();

        gst::info!(CAT, "Ice servers: {:?}", ice_servers);
        self.obj().connect_closure(
            "webrtcbin-ready",
            false,
            glib::closure!(|_signaller: &super::AwsKvsSignaller,
                            _consumer_identifier: &str,
                            webrtcbin: &gst::Element| {
                webrtcbin.set_property(
                    "stun-server",
                    format!("stun://stun.kinesisvideo.{DEFAULT_AWS_REGION}.amazonaws.com:443"),
                );
                for ice_server in &ice_servers {
                    let res = webrtcbin.emit_by_name::<bool>("add-turn-server", &[&ice_server]);
                    gst::debug!(CAT, "Added ICE server {ice_server}, res: {res}");
                }
            }),
        );

        let mut signing_settings = SigningSettings::default();
        signing_settings.signature_location = SignatureLocation::QueryParams;
        signing_settings.expires_in = Some(Duration::from_secs(5 * 60));
        let identity = credentials.clone().into();
        let region_string = region.to_string();
        let signing_params = v4::SigningParams::builder()
            .identity(&identity)
            .region(&region_string)
            .name("kinesisvideo")
            .time(SystemTime::now())
            .settings(signing_settings)
            .build()
            .unwrap()
            .into();
        let transcribe_uri = Uri::builder()
            .scheme("wss")
            .authority(endpoint_wss_uri.authority().unwrap().to_owned())
            .path_and_query(format!(
                "/?X-Amz-ChannelARN={}",
                aws_smithy_http::query::fmt_string(channel_arn)
            ))
            .build()
            .map_err(|err| {
                gst::error!(CAT, imp = self, "Failed to build HTTP request URI: {err}");
                anyhow!("Failed to build HTTP request URI: {err}")
            })?;

        // Convert the HTTP request into a signable request
        let signable_request = SignableRequest::new(
            "GET",
            transcribe_uri.to_string(),
            std::iter::empty(),
            SignableBody::Bytes(&[]),
        )
        .expect("signable request");

        let mut request = http::Request::builder()
            .uri(transcribe_uri)
            .body(aws_smithy_types::body::SdkBody::empty())
            .expect("Failed to build valid request");
        let (signing_instructions, _signature) =
            sign(signable_request, &signing_params)?.into_parts();
        signing_instructions.apply_to_request_http1x(&mut request);

        let url = request.uri().to_string();

        gst::debug!(CAT, "Signed URL: {url}");

        let (ws, _) =
            async_tungstenite::tokio::connect_async_with_tls_connector(url, connector).await?;

        gst::info!(CAT, imp = self, "connected");

        // Channel for asynchronously sending out websocket message
        let (mut ws_sink, mut ws_stream) = ws.split();

        // 1000 is completely arbitrary, we simply don't want infinite piling
        // up of messages as with unbounded
        let (mut _websocket_sender, mut websocket_receiver) =
            mpsc::channel::<p::OutgoingMessage>(1000);
        let imp = self.downgrade();
        let ping_timeout = settings.ping_timeout;
        let send_task_handle = task::spawn(async move {
            let mut res = Ok(());
            loop {
                match tokio::time::timeout(
                    std::time::Duration::from_secs(ping_timeout as u64),
                    websocket_receiver.next(),
                )
                .await
                {
                    Ok(Some(msg)) => {
                        if let Some(imp) = imp.upgrade() {
                            gst::trace!(
                                CAT,
                                imp = imp,
                                "Sending websocket message {}",
                                serde_json::to_string(&msg).unwrap()
                            );
                        }
                        res = ws_sink
                            .send(WsMessage::text(serde_json::to_string(&msg).unwrap()))
                            .await;
                    }
                    Ok(None) => {
                        break;
                    }
                    Err(_) => {
                        res = ws_sink.send(WsMessage::Ping(Default::default())).await;
                    }
                }

                if let Err(ref err) = res {
                    if let Some(imp) = imp.upgrade() {
                        gst::error!(CAT, imp = imp, "Quitting send loop: {err}");
                    } else {
                        gst::error!(CAT, "Quitting send loop: {err}");
                    }

                    break;
                }
            }

            if let Some(imp) = imp.upgrade() {
                gst::debug!(CAT, imp = imp, "Done sending");
            } else {
                gst::debug!(CAT, "Done sending");
            }

            let _ = ws_sink.close(None).await;

            res.map_err(Into::into)
        });

        let imp = self.downgrade();
        let receive_task_handle = task::spawn(async move {
            while let Some(msg) = tokio_stream::StreamExt::next(&mut ws_stream).await {
                if let Some(imp) = imp.upgrade() {
                    match msg {
                        Ok(WsMessage::Text(msg)) => {
                            gst::trace!(CAT, "received message [{msg}]");
                            imp.handle_message(msg);
                        }
                        Ok(WsMessage::Close(reason)) => {
                            gst::info!(CAT, imp = imp, "websocket connection closed: {:?}", reason);
                            imp.obj().emit_by_name::<()>("shutdown", &[]);
                            break;
                        }
                        Ok(_) => (),
                        Err(err) => {
                            imp.obj().emit_by_name::<()>(
                                "error",
                                &[&format!("{:?}", anyhow!("Error receiving: {err}"))],
                            );
                            break;
                        }
                    }
                } else {
                    break;
                }
            }

            if let Some(imp) = imp.upgrade() {
                gst::info!(CAT, imp = imp, "Stopped websocket receiving");
            }
        });

        let mut state = self.state.lock().unwrap();
        state.websocket_sender = Some(_websocket_sender);
        state.send_task_handle = Some(send_task_handle);
        state.receive_task_handle = Some(receive_task_handle);

        Ok(())
    }
}

impl SignallableImpl for Signaller {
    fn start(&self) {
        let this = self.obj().clone();
        let imp = self.downgrade();
        task::spawn(async move {
            if let Some(imp) = imp.upgrade() {
                if let Err(err) = imp.connect().await {
                    this.emit_by_name::<()>("error", &[&format!("{:?}", anyhow!(err))]);
                }
            }
        });
    }

    fn send_sdp(&self, session_id: &str, sdp: &gst_webrtc::WebRTCSessionDescription) {
        let state = self.state.lock().unwrap();

        let msg = p::OutgoingMessage {
            action: "SDP_ANSWER".to_string(),
            message_payload: BASE64.encode(
                &serde_json::to_string(&p::SdpAnswer {
                    type_: "answer".to_string(),
                    sdp: sdp.sdp().as_text().unwrap(),
                })
                .unwrap()
                .into_bytes(),
            ),
            recipient_client_id: session_id.to_string(),
        };

        if let Some(mut sender) = state.websocket_sender.clone() {
            let imp = self.downgrade();
            RUNTIME.spawn(async move {
                if let Err(err) = sender.send(msg).await {
                    if let Some(imp) = imp.upgrade() {
                        imp.obj().emit_by_name::<()>(
                            "error",
                            &[&format!("{:?}", anyhow!("Error: {err}"))],
                        );
                    }
                }
            });
        }
    }

    fn add_ice(
        &self,
        session_id: &str,
        candidate: &str,
        sdp_m_line_index: u32,
        _sdp_mid: Option<String>,
    ) {
        let state = self.state.lock().unwrap();

        let msg = p::OutgoingMessage {
            action: "ICE_CANDIDATE".to_string(),
            message_payload: BASE64.encode(
                &serde_json::to_string(&p::OutgoingIceCandidate {
                    candidate: candidate.to_string(),
                    sdp_mid: sdp_m_line_index.to_string(),
                    sdp_m_line_index,
                })
                .unwrap()
                .into_bytes(),
            ),
            recipient_client_id: session_id.to_string(),
        };

        if let Some(mut sender) = state.websocket_sender.clone() {
            let imp = self.downgrade();
            RUNTIME.spawn(async move {
                if let Err(err) = sender.send(msg).await {
                    if let Some(imp) = imp.upgrade() {
                        imp.obj().emit_by_name::<()>(
                            "error",
                            &[&format!("{:?}", anyhow!("Error: {err}"))],
                        );
                    }
                }
            });
        }
    }

    fn stop(&self) {
        gst::info!(CAT, imp = self, "Stopping now");

        let mut state = self.state.lock().unwrap();
        let send_task_handle = state.send_task_handle.take();
        let receive_task_handle = state.receive_task_handle.take();
        if let Some(mut sender) = state.websocket_sender.take() {
            let imp = self.downgrade();
            RUNTIME.block_on(async move {
                sender.close_channel();

                if let Some(handle) = send_task_handle {
                    if let Err(err) = handle.await {
                        if let Some(imp) = imp.upgrade() {
                            gst::warning!(CAT, imp = imp, "Error while joining send task: {err}");
                        }
                    }
                }

                if let Some(handle) = receive_task_handle {
                    if let Err(err) = handle.await {
                        if let Some(imp) = imp.upgrade() {
                            gst::warning!(
                                CAT,
                                imp = imp,
                                "Error while joining receive task: {err}"
                            );
                        }
                    }
                }
            });
        }
    }

    fn end_session(&self, session_id: &str) {
        gst::info!(CAT, imp = self, "Signalling session {session_id} ended");

        // We can seemingly not do anything beyond that
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Signaller {
    const NAME: &'static str = "GstAwsKvsWebRTCSinkSignaller";
    type Type = super::AwsKvsSignaller;
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
                glib::ParamSpecString::builder("address")
                    .nick("Address")
                    .blurb("Address of the signalling server")
                    .default_value("ws://127.0.0.1:8443")
                    .build(),
                glib::ParamSpecString::builder("cafile")
                    .nick("CA file")
                    .blurb("Path to a Certificate file to add to the set of roots the TLS connector will trust")
                    .build(),
                glib::ParamSpecString::builder("channel-name")
                    .nick("Channel name")
                    .blurb("Name of the channel to connect as master to")
                    .build(),
                glib::ParamSpecInt::builder("ping-timeout")
                    .nick("Ping Timeout")
                    .blurb("How often (in seconds) to send pings to keep the websocket alive")
                    .default_value(DEFAULT_PING_TIMEOUT)
                    .minimum(1)
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "address" => {
                let address: Option<_> = value.get().unwrap();

                if let Some(address) = address {
                    gst::info!(CAT, "Signaller address set to {address}");

                    let mut settings = self.settings.lock().unwrap();
                    settings.address = Some(address);
                } else {
                    gst::error!(CAT, "address can't be None");
                }
            }
            "cafile" => {
                let value: String = value.get().unwrap();
                let mut settings = self.settings.lock().unwrap();
                settings.cafile = Some(value.into());
            }
            "access-key" => {
                let mut settings = self.settings.lock().unwrap();
                settings.access_key = value.get().unwrap();
            }
            "secret-access-key" => {
                let mut settings = self.settings.lock().unwrap();
                settings.secret_access_key = value.get().unwrap();
            }
            "session-token" => {
                let mut settings = self.settings.lock().unwrap();
                settings.session_token = value.get().unwrap();
            }
            "channel-name" => {
                let mut settings = self.settings.lock().unwrap();
                settings.channel_name = value.get().unwrap();
            }
            "ping-timeout" => {
                let mut settings = self.settings.lock().unwrap();
                settings.ping_timeout = value.get().unwrap();
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "manual-sdp-munging" => false.to_value(),
            "address" => self.settings.lock().unwrap().address.to_value(),
            "cafile" => {
                let settings = self.settings.lock().unwrap();
                let cafile = settings.cafile.as_ref();
                cafile.and_then(|file| file.to_str()).to_value()
            }
            "access-key" => {
                let settings = self.settings.lock().unwrap();
                settings.access_key.to_value()
            }
            "secret-access-key" => {
                let settings = self.settings.lock().unwrap();
                settings.secret_access_key.to_value()
            }
            "session-token" => {
                let settings = self.settings.lock().unwrap();
                settings.session_token.to_value()
            }
            "channel-name" => self.settings.lock().unwrap().channel_name.to_value(),
            "ping-timeout" => self.settings.lock().unwrap().ping_timeout.to_value(),
            _ => unimplemented!(),
        }
    }
}
