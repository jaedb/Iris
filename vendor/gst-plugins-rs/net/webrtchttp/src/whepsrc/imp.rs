// Copyright (C) 2022, Asymptotic Inc.
//      Author: Sanchayan Maity <sanchayan@asymptotic.io>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use crate::utils::{
    self, build_reqwest_client, parse_redirect_location, set_ice_servers, wait, wait_async,
    WaitError, RUNTIME,
};
use crate::IceTransportPolicy;
use async_recursion::async_recursion;
use bytes::Bytes;
use gst::{glib, prelude::*, subclass::prelude::*};
use gst_sdp::*;
use gst_webrtc::*;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::StatusCode;
use std::sync::LazyLock;
use std::sync::Mutex;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "whepsrc",
        gst::DebugColorFlags::empty(),
        Some("WHEP Source"),
    )
});

const DEFAULT_ICE_TRANSPORT_POLICY: IceTransportPolicy = IceTransportPolicy::All;
const MAX_REDIRECTS: u8 = 10;
const DEFAULT_TIMEOUT: u32 = 15;

#[derive(Debug, Clone)]
struct Settings {
    video_caps: Option<gst::Caps>,
    audio_caps: Option<gst::Caps>,
    turn_server: Option<String>,
    stun_server: Option<String>,
    whep_endpoint: Option<String>,
    auth_token: Option<String>,
    use_link_headers: bool,
    ice_transport_policy: IceTransportPolicy,
    timeout: u32,
}

#[allow(clippy::derivable_impls)]
impl Default for Settings {
    fn default() -> Self {
        let video_caps = {
            let video = [
                ("VP8", 101),
                ("VP9", 102),
                ("H264", 103),
                ("H265", 104),
                ("AV1", 105),
            ];

            let mut video_caps = gst::Caps::new_empty();
            let caps = video_caps.get_mut().unwrap();

            for (encoding, pt) in video {
                let s = gst::Structure::builder("application/x-rtp")
                    .field("media", "video")
                    .field("payload", pt)
                    .field("encoding-name", encoding)
                    .field("clock-rate", 90000)
                    .build();
                caps.append_structure(s);
            }

            Some(video_caps)
        };

        let audio_caps = Some(
            gst::Caps::builder("application/x-rtp")
                .field("media", "audio")
                .field("encoding-name", "OPUS")
                .field("payload", 96)
                .field("clock-rate", 48000)
                .build(),
        );

        Self {
            video_caps,
            audio_caps,
            stun_server: None,
            turn_server: None,
            whep_endpoint: None,
            auth_token: None,
            use_link_headers: false,
            ice_transport_policy: DEFAULT_ICE_TRANSPORT_POLICY,
            timeout: DEFAULT_TIMEOUT,
        }
    }
}

#[derive(Debug, Default)]
enum State {
    #[default]
    Stopped,
    Post {
        redirects: u8,
    },
    Running {
        whep_resource: String,
    },
}

pub struct WhepSrc {
    settings: Mutex<Settings>,
    state: Mutex<State>,
    webrtcbin: gst::Element,
    canceller: Mutex<utils::Canceller>,
    client: reqwest::Client,
}

impl Default for WhepSrc {
    fn default() -> Self {
        let webrtcbin = gst::ElementFactory::make("webrtcbin")
            .build()
            .expect("Failed to create webrtcbin");

        // We'll handle redirects manually since the default redirect handler does not
        // reuse the authentication token on the redirected server
        let pol = reqwest::redirect::Policy::none();
        let client = build_reqwest_client(pol);

        Self {
            settings: Mutex::new(Settings::default()),
            state: Mutex::new(State::default()),
            webrtcbin,
            canceller: Mutex::new(utils::Canceller::default()),
            client,
        }
    }
}

impl GstObjectImpl for WhepSrc {}

impl ElementImpl for WhepSrc {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "WHEP Source Bin",
                "Source/Network/WebRTC",
                "A bin to stream media using the WebRTC HTTP Egress Protocol (WHEP)",
                "Sanchayan Maity <sanchayan@asymptotic.io>",
            )
        });
        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let src_pad_template = gst::PadTemplate::new(
                "src_%u",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &gst::Caps::new_empty_simple("application/x-rtp"),
            )
            .unwrap();

            vec![src_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    #[allow(clippy::single_match)]
    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        match transition {
            gst::StateChange::NullToReady => {
                /*
                 * Fail the state change if WHEP endpoint has not been set by the
                 * time ReadyToPaused transition happens. This prevents us from
                 * having to check this everywhere else.
                 */
                let settings = self.settings.lock().unwrap();

                if settings.whep_endpoint.is_none() {
                    gst::error!(CAT, imp = self, "WHEP endpoint URL must be set");
                    return Err(gst::StateChangeError);
                }

                /*
                 * Check if we have a valid URL. We can be assured any further URL
                 * handling won't fail due to invalid URLs.
                 */
                if let Err(e) =
                    reqwest::Url::parse(settings.whep_endpoint.as_ref().unwrap().as_str())
                {
                    gst::error!(
                        CAT,
                        imp = self,
                        "WHEP endpoint URL could not be parsed: {}",
                        e
                    );
                    return Err(gst::StateChangeError);
                }

                drop(settings);
            }

            gst::StateChange::PausedToReady => {
                let mut canceller = self.canceller.lock().unwrap();
                canceller.abort();
            }
            _ => (),
        }

        let res = self.parent_change_state(transition)?;

        match transition {
            gst::StateChange::PausedToReady => {
                {
                    let mut canceller = self.canceller.lock().unwrap();
                    *canceller = utils::Canceller::None;
                }

                let state = self.state.lock().unwrap();
                if let State::Running { .. } = *state {
                    drop(state);
                    self.terminate_session();
                }

                for pad in self.obj().src_pads() {
                    gst::debug!(CAT, imp = self, "Removing pad: {}", pad.name());

                    // No need to deactivate pad here. Parent GstBin will deactivate
                    // the pad. Only remove the pad.
                    if let Err(e) = self.obj().remove_pad(&pad) {
                        gst::error!(
                            CAT,
                            imp = self,
                            "Failed to remove pad {}: {}",
                            pad.name(),
                            e
                        );
                    }
                }
            }
            _ => (),
        }

        Ok(res)
    }
}

impl ObjectImpl for WhepSrc {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecBoxed::builder::<gst::Caps>("video-caps")
                    .nick("Video caps")
                    .blurb("Governs what video codecs will be proposed")
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Caps>("audio-caps")
                    .nick("Audio caps")
                    .blurb("Governs what audio codecs will be proposed")
                    .build(),
                glib::ParamSpecString::builder("stun-server")
                    .nick("STUN Server")
                    .blurb("The STUN server of the form stun://hostname:port")
                    .build(),
                glib::ParamSpecString::builder("turn-server")
                    .nick("TURN Server")
                    .blurb("The TURN server of the form turn(s)://username:password@host:port.")
                    .build(),
                glib::ParamSpecString::builder("whep-endpoint")
                    .nick("WHEP Endpoint")
                    .blurb("The WHEP server endpoint to POST SDP offer to.")
                    .build(),
                glib::ParamSpecBoolean::builder("use-link-headers")
                    .nick("Use Link Headers")
                    .blurb("Use link headers to configure STUN/TURN servers if present in WHEP endpoint response.")
                    .build(),
                glib::ParamSpecString::builder("auth-token")
                    .nick("Authorization Token")
                    .blurb("Authentication token to use, will be sent in the HTTP Header as 'Bearer <auth-token>'")
                    .build(),
                glib::ParamSpecEnum::builder_with_default("ice-transport-policy", DEFAULT_ICE_TRANSPORT_POLICY)
                    .nick("ICE transport policy")
                    .blurb("The policy to apply for ICE transport")
                    .build(),
                glib::ParamSpecUInt::builder("timeout")
                    .nick("Timeout")
                    .blurb("Value in seconds to timeout WHEP endpoint requests (0 = No timeout).")
                    .maximum(3600)
                    .default_value(DEFAULT_TIMEOUT)
                    .readwrite()
                    .build(),
            ]
        });
        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "video-caps" => {
                let mut settings = self.settings.lock().unwrap();
                settings.video_caps = value
                    .get::<Option<gst::Caps>>()
                    .expect("type checked upstream");
            }
            "audio-caps" => {
                let mut settings = self.settings.lock().unwrap();
                settings.audio_caps = value
                    .get::<Option<gst::Caps>>()
                    .expect("type checked upstream");
            }
            "stun-server" => {
                let mut settings = self.settings.lock().unwrap();
                settings.stun_server = value
                    .get::<Option<String>>()
                    .expect("type checked upstream");
                self.webrtcbin
                    .set_property("stun-server", settings.stun_server.as_ref());
            }
            "turn-server" => {
                let mut settings = self.settings.lock().unwrap();
                settings.turn_server = value
                    .get::<Option<String>>()
                    .expect("type checked upstream");
                self.webrtcbin
                    .set_property("turn-server", settings.turn_server.as_ref());
            }
            "whep-endpoint" => {
                let mut settings = self.settings.lock().unwrap();
                settings.whep_endpoint = value.get().expect("WHEP endpoint should be a string");
            }
            "use-link-headers" => {
                let mut settings = self.settings.lock().unwrap();
                settings.use_link_headers = value
                    .get()
                    .expect("use-link-headers should be a boolean value");
            }
            "auth-token" => {
                let mut settings = self.settings.lock().unwrap();
                settings.auth_token = value.get().expect("Auth token should be a string");
            }
            "ice-transport-policy" => {
                let mut settings = self.settings.lock().unwrap();
                settings.ice_transport_policy = value
                    .get::<IceTransportPolicy>()
                    .expect("ice-transport-policy should be an enum value");

                if settings.ice_transport_policy == IceTransportPolicy::Relay {
                    self.webrtcbin
                        .set_property_from_str("ice-transport-policy", "relay");
                } else {
                    self.webrtcbin
                        .set_property_from_str("ice-transport-policy", "all");
                }
            }
            "timeout" => {
                let mut settings = self.settings.lock().unwrap();
                settings.timeout = value.get().expect("type checked upstream");
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "video-caps" => {
                let settings = self.settings.lock().unwrap();
                settings.video_caps.to_value()
            }
            "audio-caps" => {
                let settings = self.settings.lock().unwrap();
                settings.audio_caps.to_value()
            }
            "stun-server" => {
                let settings = self.settings.lock().unwrap();
                settings.stun_server.to_value()
            }
            "turn-server" => {
                let settings = self.settings.lock().unwrap();
                settings.turn_server.to_value()
            }
            "whep-endpoint" => {
                let settings = self.settings.lock().unwrap();
                settings.whep_endpoint.to_value()
            }
            "use-link-headers" => {
                let settings = self.settings.lock().unwrap();
                settings.use_link_headers.to_value()
            }
            "auth-token" => {
                let settings = self.settings.lock().unwrap();
                settings.auth_token.to_value()
            }
            "ice-transport-policy" => {
                let settings = self.settings.lock().unwrap();
                settings.ice_transport_policy.to_value()
            }
            "timeout" => {
                let settings = self.settings.lock().unwrap();
                settings.timeout.to_value()
            }
            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        self.obj()
            .set_suppressed_flags(gst::ElementFlags::SINK | gst::ElementFlags::SOURCE);
        self.obj().set_element_flags(gst::ElementFlags::SOURCE);

        self.setup_webrtcbin();

        self.obj().add(&self.webrtcbin).unwrap();
    }
}

#[glib::object_subclass]
impl ObjectSubclass for WhepSrc {
    const NAME: &'static str = "GstWhepSrc";
    type Type = super::WhepSrc;
    type ParentType = gst::Bin;
}

impl BinImpl for WhepSrc {
    fn handle_message(&self, message: gst::Message) {
        use gst::MessageView;
        match message.view() {
            MessageView::Eos(_) | MessageView::Error(_) => {
                self.terminate_session();
                self.parent_handle_message(message)
            }
            _ => self.parent_handle_message(message),
        }
    }
}

impl WhepSrc {
    fn raise_error(&self, resource_error: gst::ResourceError, msg: String) {
        gst::error_msg!(resource_error, ["{msg}"]);
        gst::element_imp_error!(self, resource_error, ["{msg}"]);
    }

    fn handle_future_error(&self, err: WaitError) {
        match err {
            WaitError::FutureAborted => {
                gst::warning!(CAT, imp = self, "Future aborted")
            }
            WaitError::FutureError(err) => {
                self.raise_error(gst::ResourceError::Failed, err.to_string())
            }
        };
    }

    fn setup_webrtcbin(&self) {
        // The specification requires all m= lines to be bundled (section 4.5)
        self.webrtcbin
            .set_property("bundle-policy", WebRTCBundlePolicy::MaxBundle);

        let self_weak = self.downgrade();
        self.webrtcbin
            .connect_notify(Some("ice-gathering-state"), move |webrtcbin, _pspec| {
                let Some(self_) = self_weak.upgrade() else {
                    return;
                };

                let state = webrtcbin.property::<WebRTCICEGatheringState>("ice-gathering-state");

                match state {
                    WebRTCICEGatheringState::Gathering => {
                        gst::info!(CAT, imp = self_, "ICE gathering started")
                    }
                    WebRTCICEGatheringState::Complete => {
                        gst::info!(CAT, imp = self_, "ICE gathering completed");

                        let self_ref = self_.ref_counted();

                        // With tokio's spawn one does not have to .await the
                        // returned JoinHandle to make the provided future start
                        // execution. It will start running in the background
                        // immediately when spawn is called.
                        RUNTIME.spawn(async move {
                            /* Note that we check for a valid WHEP endpoint in change_state */
                            self_ref.whep_offer().await
                        });
                    }
                    _ => (),
                }
            });

        let self_weak = self.downgrade();
        self.webrtcbin
            .connect_notify(Some("ice-connection-state"), move |webrtcbin, _pspec| {
                let Some(self_) = self_weak.upgrade() else {
                    return;
                };

                let state = webrtcbin.property::<WebRTCICEConnectionState>("ice-connection-state");

                match state {
                    WebRTCICEConnectionState::New => (),
                    WebRTCICEConnectionState::Checking => {
                        gst::info!(CAT, imp = self_, "ICE connecting...")
                    }
                    WebRTCICEConnectionState::Connected => {
                        gst::info!(CAT, imp = self_, "ICE connected")
                    }
                    WebRTCICEConnectionState::Completed => {
                        gst::info!(CAT, imp = self_, "ICE completed")
                    }
                    WebRTCICEConnectionState::Failed => {
                        self_.terminate_session();
                        gst::element_imp_error!(self_, gst::ResourceError::Failed, ["ICE failed"]);
                    }
                    WebRTCICEConnectionState::Disconnected => (),
                    WebRTCICEConnectionState::Closed => (),
                    _ => (),
                }
            });

        let self_weak = self.downgrade();
        self.webrtcbin
            .connect_notify(Some("connection-state"), move |webrtcbin, _pspec| {
                let Some(self_) = self_weak.upgrade() else {
                    return;
                };

                let state = webrtcbin.property::<WebRTCPeerConnectionState>("connection-state");

                match state {
                    WebRTCPeerConnectionState::New => (),
                    WebRTCPeerConnectionState::Connecting => {
                        gst::info!(CAT, imp = self_, "PeerConnection connecting...")
                    }
                    WebRTCPeerConnectionState::Connected => {
                        gst::info!(CAT, imp = self_, "PeerConnection connected")
                    }
                    WebRTCPeerConnectionState::Disconnected => (),
                    WebRTCPeerConnectionState::Failed => {
                        self_.terminate_session();
                        gst::element_imp_error!(
                            self_,
                            gst::ResourceError::Failed,
                            ["PeerConnection failed"]
                        );
                    }
                    WebRTCPeerConnectionState::Closed => (),
                    _ => (),
                }
            });

        let self_weak = self.downgrade();
        self.webrtcbin.connect_pad_added(move |_, pad| {
            let Some(self_) = self_weak.upgrade() else {
                return;
            };

            gst::debug!(
                CAT,
                imp = self_,
                "Pad added with name: {} and caps: {:?}",
                pad.name(),
                pad.current_caps()
            );

            let templ = self_.obj().pad_template("src_%u").unwrap();
            let src_pad = gst::GhostPad::from_template_with_target(&templ, pad).unwrap();

            src_pad.set_target(Some(pad)).unwrap();
            src_pad
                .set_active(true)
                .expect("Pad activation should succeed");

            self_.obj().add_pad(&src_pad).expect("Failed to add pad");
        });

        let self_weak = self.downgrade();
        self.webrtcbin.connect("on-negotiation-needed", false, {
            move |_| {
                let self_ = self_weak.upgrade()?;
                let settings = self_.settings.lock().unwrap();

                let endpoint =
                    reqwest::Url::parse(settings.whep_endpoint.as_ref().unwrap().as_str());
                if let Err(e) = endpoint {
                    gst::element_imp_error!(
                        self_,
                        gst::ResourceError::Failed,
                        ["Could not parse WHEP endpoint URL :{}", e]
                    );
                    return None;
                }

                drop(settings);

                let mut state = self_.state.lock().unwrap();
                *state = State::Post { redirects: 0 };
                drop(state);

                self_.initial_post_request(endpoint.unwrap());

                None
            }
        });
    }

    fn sdp_message_parse(&self, sdp_bytes: Bytes) {
        let sdp = match sdp_message::SDPMessage::parse_buffer(&sdp_bytes) {
            Ok(sdp) => sdp,
            Err(_) => {
                self.raise_error(
                    gst::ResourceError::Failed,
                    "Could not parse answer SDP".to_string(),
                );
                return;
            }
        };

        let remote_sdp = WebRTCSessionDescription::new(WebRTCSDPType::Answer, sdp);

        gst::debug!(
            CAT,
            imp = self,
            "Setting remote description: {:?}",
            remote_sdp.sdp().as_text()
        );

        self.webrtcbin.emit_by_name::<()>(
            "set-remote-description",
            &[&remote_sdp, &None::<gst::Promise>],
        );

        for media in remote_sdp.sdp().medias() {
            let c = media.attribute_val("candidate");

            if let Some(candidate) = c {
                let m_line_index = 0u32;
                let c = format!("candidate:{candidate}");

                gst::debug!(CAT, imp = self, "Adding ICE candidate from offer: {:?}", c);

                self.webrtcbin
                    .emit_by_name::<()>("add-ice-candidate", &[&m_line_index, &c]);
            }
        }
    }

    async fn parse_endpoint_response(
        &self,
        sess_desc: WebRTCSessionDescription,
        resp: reqwest::Response,
        redirects: u8,
    ) {
        let endpoint;
        let use_link_headers;

        {
            let settings = self.settings.lock().unwrap();
            endpoint =
                reqwest::Url::parse(settings.whep_endpoint.as_ref().unwrap().as_str()).unwrap();
            use_link_headers = settings.use_link_headers;
            drop(settings);
        }

        match resp.status() {
            StatusCode::OK | StatusCode::NO_CONTENT => {
                gst::info!(CAT, imp = self, "SDP offer successfully send");
            }

            StatusCode::CREATED => {
                gst::debug!(CAT, imp = self, "Response headers: {:?}", resp.headers());

                if use_link_headers {
                    if let Err(e) = set_ice_servers(&self.webrtcbin, resp.headers()) {
                        self.raise_error(gst::ResourceError::Failed, e.to_string());
                        return;
                    };
                }

                /* See section 4.2 of the WHEP specification */
                let location = match resp.headers().get(reqwest::header::LOCATION) {
                    Some(location) => location,
                    None => {
                        self.raise_error(
                            gst::ResourceError::Failed,
                            "Location header field should be present for WHEP resource URL"
                                .to_string(),
                        );
                        return;
                    }
                };

                let location = match location.to_str() {
                    Ok(loc) => loc,
                    Err(e) => {
                        self.raise_error(
                            gst::ResourceError::Failed,
                            format!("Failed to convert location to string: {e}"),
                        );
                        return;
                    }
                };

                let url = reqwest::Url::parse(endpoint.as_str()).unwrap();

                gst::debug!(CAT, imp = self, "WHEP resource: {:?}", location);

                let url = match url.join(location) {
                    Ok(joined_url) => joined_url,
                    Err(err) => {
                        self.raise_error(
                            gst::ResourceError::Failed,
                            format!("URL join operation failed: {err:?}"),
                        );
                        return;
                    }
                };

                match resp.bytes().await {
                    Ok(ans_bytes) => {
                        let mut state = self.state.lock().unwrap();
                        *state = match *state {
                            State::Post { redirects: _r } => State::Running {
                                whep_resource: url.to_string(),
                            },
                            _ => {
                                self.raise_error(
                                    gst::ResourceError::Failed,
                                    "Expected to be in POST state".to_string(),
                                );
                                return;
                            }
                        };
                        drop(state);

                        self.sdp_message_parse(ans_bytes)
                    }
                    Err(err) => self.raise_error(gst::ResourceError::Failed, err.to_string()),
                }
            }

            status if status.is_redirection() => {
                if redirects < MAX_REDIRECTS {
                    match parse_redirect_location(resp.headers(), &endpoint) {
                        Ok(redirect_url) => {
                            {
                                let mut state = self.state.lock().unwrap();
                                *state = match *state {
                                    State::Post { redirects: _r } => State::Post {
                                        redirects: redirects + 1,
                                    },
                                    /*
                                     * As per section 4.6 of the specification, redirection is
                                     * not required to be supported for the PATCH and DELETE
                                     * requests to the final WHEP resource URL. Only the initial
                                     * POST request may support redirection.
                                     */
                                    State::Running { .. } => {
                                        self.raise_error(
                                            gst::ResourceError::Failed,
                                            "Unexpected redirection in RUNNING state".to_string(),
                                        );
                                        return;
                                    }
                                    State::Stopped => unreachable!(),
                                };
                                drop(state);
                            }

                            gst::warning!(
                                CAT,
                                imp = self,
                                "Redirecting endpoint to {}",
                                redirect_url.as_str()
                            );

                            self.do_post(sess_desc, redirect_url).await
                        }
                        Err(e) => self.raise_error(gst::ResourceError::Failed, e.to_string()),
                    }
                } else {
                    self.raise_error(
                        gst::ResourceError::Failed,
                        "Too many redirects. Unable to connect.".to_string(),
                    );
                }
            }

            s => {
                match resp.bytes().await {
                    Ok(r) => {
                        let res = r.escape_ascii().to_string();

                        // FIXME: Check and handle 'Retry-After' header in case of server error
                        self.raise_error(
                            gst::ResourceError::Failed,
                            format!("Unexpected response: {} - {}", s.as_str(), res),
                        );
                    }
                    Err(err) => self.raise_error(gst::ResourceError::Failed, err.to_string()),
                }
            }
        }
    }

    fn generate_offer(&self) {
        let self_weak = self.downgrade();
        let promise = gst::Promise::with_change_func(move |reply| {
            let Some(self_) = self_weak.upgrade() else {
                return;
            };

            let reply = match reply {
                Ok(Some(reply)) => reply,
                Ok(None) => {
                    gst::element_imp_error!(
                        self_,
                        gst::LibraryError::Failed,
                        ["generate offer::Promise returned with no reply"]
                    );
                    return;
                }
                Err(e) => {
                    gst::element_imp_error!(
                        self_,
                        gst::LibraryError::Failed,
                        ["generate offer::Promise returned with error {:?}", e]
                    );
                    return;
                }
            };

            if let Ok(offer_sdp) = reply
                .value("offer")
                .map(|offer| offer.get::<gst_webrtc::WebRTCSessionDescription>().unwrap())
            {
                gst::debug!(
                    CAT,
                    imp = self_,
                    "Setting local description: {:?}",
                    offer_sdp.sdp().as_text()
                );

                self_.webrtcbin.emit_by_name::<()>(
                    "set-local-description",
                    &[&offer_sdp, &None::<gst::Promise>],
                );
            } else {
                let error = reply
                    .value("error")
                    .expect("structure must have an error value")
                    .get::<glib::Error>()
                    .expect("value must be a GLib error");

                gst::element_imp_error!(
                    self_,
                    gst::LibraryError::Failed,
                    ["generate offer::Promise returned with error: {}", error]
                );
            }
        });

        let settings = self.settings.lock().unwrap();

        gst::debug!(
            CAT,
            imp = self,
            "Audio caps: {:?} Video caps: {:?}",
            settings.audio_caps,
            settings.video_caps
        );

        if settings.audio_caps.is_none() && settings.video_caps.is_none() {
            self.raise_error(
                gst::ResourceError::Failed,
                "One of audio-caps or video-caps must be set".to_string(),
            );
            return;
        }

        /*
         * Since we will be recvonly we need to add a transceiver without which
         * WebRTC bin does not generate ICE candidates.
         */
        if let Some(audio_caps) = &settings.audio_caps {
            self.webrtcbin.emit_by_name::<WebRTCRTPTransceiver>(
                "add-transceiver",
                &[&WebRTCRTPTransceiverDirection::Recvonly, &audio_caps],
            );
        }

        if let Some(video_caps) = &settings.video_caps {
            self.webrtcbin.emit_by_name::<WebRTCRTPTransceiver>(
                "add-transceiver",
                &[&WebRTCRTPTransceiverDirection::Recvonly, &video_caps],
            );
        }

        drop(settings);

        self.webrtcbin
            .emit_by_name::<()>("create-offer", &[&None::<gst::Structure>, &promise]);
    }

    fn initial_post_request(&self, endpoint: reqwest::Url) {
        let state = self.state.lock().unwrap();

        gst::info!(CAT, imp = self, "WHEP endpoint url: {}", endpoint.as_str());

        match *state {
            State::Post { .. } => (),
            _ => {
                self.raise_error(
                    gst::ResourceError::Failed,
                    "Trying to do POST in unexpected state".to_string(),
                );
                return;
            }
        };
        drop(state);

        self.generate_offer()
    }

    async fn whep_offer(&self) {
        let local_desc = self
            .webrtcbin
            .property::<Option<WebRTCSessionDescription>>("local-description");

        let sess_desc = match local_desc {
            None => {
                gst::element_imp_error!(
                    self,
                    gst::ResourceError::Failed,
                    ["Local description is not set"]
                );
                return;
            }
            Some(mut local_desc) => {
                local_desc.set_type(WebRTCSDPType::Offer);
                local_desc
            }
        };

        gst::debug!(
            CAT,
            imp = self,
            "Sending offer SDP: {:?}",
            sess_desc.sdp().as_text()
        );

        let timeout;
        let endpoint;

        {
            let settings = self.settings.lock().unwrap();
            timeout = settings.timeout;
            endpoint =
                reqwest::Url::parse(settings.whep_endpoint.as_ref().unwrap().as_str()).unwrap();
            drop(settings);
        }

        if let Err(e) =
            wait_async(&self.canceller, self.do_post(sess_desc, endpoint), timeout).await
        {
            self.handle_future_error(e);
        }
    }

    #[async_recursion]
    async fn do_post(&self, offer: WebRTCSessionDescription, endpoint: reqwest::Url) {
        let auth_token;

        {
            let settings = self.settings.lock().unwrap();
            auth_token = settings.auth_token.clone();
            drop(settings);
        }

        let sdp = offer.sdp();
        let body = sdp.as_text().unwrap();

        gst::info!(CAT, imp = self, "Using endpoint {}", endpoint.as_str());

        let mut headermap = HeaderMap::new();
        headermap.insert(
            reqwest::header::CONTENT_TYPE,
            HeaderValue::from_static("application/sdp"),
        );

        if let Some(token) = auth_token.as_ref() {
            let bearer_token = "Bearer ".to_owned() + token.as_str();
            headermap.insert(
                reqwest::header::AUTHORIZATION,
                HeaderValue::from_str(bearer_token.as_str())
                    .expect("Failed to set auth token to header"),
            );
        }

        gst::debug!(
            CAT,
            imp = self,
            "Url for HTTP POST request: {}",
            endpoint.as_str()
        );

        let resp = self
            .client
            .request(reqwest::Method::POST, endpoint.clone())
            .headers(headermap)
            .body(body)
            .send()
            .await;

        match resp {
            Ok(r) => {
                #[allow(unused_mut)]
                let mut redirects;

                {
                    let state = self.state.lock().unwrap();
                    redirects = match *state {
                        State::Post { redirects } => redirects,
                        _ => {
                            self.raise_error(
                                gst::ResourceError::Failed,
                                "Trying to do POST in unexpected state".to_string(),
                            );
                            return;
                        }
                    };
                    drop(state);
                }

                self.parse_endpoint_response(offer, r, redirects).await
            }
            Err(err) => self.raise_error(gst::ResourceError::Failed, err.to_string()),
        }
    }

    fn terminate_session(&self) {
        let settings = self.settings.lock().unwrap();
        let state = self.state.lock().unwrap();
        let timeout = settings.timeout;

        let resource_url = match *state {
            State::Running {
                whep_resource: ref whep_resource_url,
            } => whep_resource_url.clone(),
            _ => {
                gst::element_imp_error!(
                    self,
                    gst::ResourceError::Failed,
                    ["Terminated in unexpected state"]
                );
                return;
            }
        };

        drop(state);

        let mut headermap = HeaderMap::new();
        if let Some(token) = &settings.auth_token {
            let bearer_token = "Bearer ".to_owned() + token.as_str();
            headermap.insert(
                reqwest::header::AUTHORIZATION,
                HeaderValue::from_str(bearer_token.as_str())
                    .expect("Failed to set auth token to header"),
            );
        }

        drop(settings);

        gst::debug!(CAT, imp = self, "DELETE request on {}", resource_url);

        /* DELETE request goes to the WHEP resource URL. See section 3 of the specification. */
        let client = build_reqwest_client(reqwest::redirect::Policy::default());
        let future = async {
            client
                .delete(resource_url.clone())
                .headers(headermap)
                .send()
                .await
                .map_err(|err| {
                    gst::error_msg!(
                        gst::ResourceError::Failed,
                        ["DELETE request failed {}: {:?}", resource_url, err]
                    )
                })
        };

        let res = wait(&self.canceller, future, timeout);
        match res {
            Ok(r) => {
                gst::debug!(CAT, imp = self, "Response to DELETE : {}", r.status());
            }
            Err(e) => match e {
                WaitError::FutureAborted => {
                    gst::warning!(CAT, imp = self, "DELETE request aborted")
                }
                WaitError::FutureError(e) => {
                    gst::error!(CAT, imp = self, "Error on DELETE request : {}", e)
                }
            },
        };
    }
}
