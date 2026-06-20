// Copyright (C) 2022, Asymptotic Inc.
//      Author: Sanchayan Maity <sanchayan@asymptotic.io>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use crate::signaller::{Signallable, SignallableImpl};
use crate::utils::{
    build_reqwest_client, parse_redirect_location, set_ice_servers, wait, wait_async, WaitError,
};
use crate::RUNTIME;
use bytes::Bytes;
use futures::future;
use gst::glib::RustClosure;
use gst::{glib, prelude::*, subclass::prelude::*};
use gst_sdp::*;
use gst_webrtc::*;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::StatusCode;
use std::sync::LazyLock;
use std::sync::Mutex;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "whep-client-signaller",
        gst::DebugColorFlags::empty(),
        Some("WHEP Client Signaller"),
    )
});

const MAX_REDIRECTS: u8 = 10;
const DEFAULT_TIMEOUT: u32 = 15;
const SESSION_ID: &str = "whep-client";

#[derive(Debug, Clone)]
struct Settings {
    whep_endpoint: Option<String>,
    auth_token: Option<String>,
    use_link_headers: bool,
    timeout: u32,
}

#[allow(clippy::derivable_impls)]
impl Default for Settings {
    fn default() -> Self {
        Self {
            whep_endpoint: None,
            auth_token: None,
            use_link_headers: false,
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

pub struct WhepClient {
    settings: Mutex<Settings>,
    state: Mutex<State>,
    canceller: Mutex<Option<future::AbortHandle>>,
    client: reqwest::Client,
}

impl Default for WhepClient {
    fn default() -> Self {
        // We'll handle redirects manually since the default redirect handler does not
        // reuse the authentication token on the redirected server
        let pol = reqwest::redirect::Policy::none();
        let client = build_reqwest_client(pol);

        Self {
            settings: Mutex::new(Settings::default()),
            state: Mutex::new(State::default()),
            canceller: Mutex::new(None),
            client,
        }
    }
}

impl ObjectImpl for WhepClient {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
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
                glib::ParamSpecUInt::builder("timeout")
                    .nick("Timeout")
                    .blurb("Value in seconds to timeout WHEP endpoint requests (0 = No timeout).")
                    .maximum(3600)
                    .default_value(DEFAULT_TIMEOUT)
                    .readwrite()
                    .build(),
                glib::ParamSpecBoolean::builder("manual-sdp-munging")
                    .nick("Manual SDP munging")
                    .blurb("Whether the signaller manages SDP munging itself")
                    .default_value(false)
                    .read_only()
                    .build(),
            ]
        });
        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
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
            "timeout" => {
                let mut settings = self.settings.lock().unwrap();
                settings.timeout = value.get().expect("type checked upstream");
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
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
            "timeout" => {
                let settings = self.settings.lock().unwrap();
                settings.timeout.to_value()
            }
            "manual-sdp-munging" => false.to_value(),
            _ => unimplemented!(),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for WhepClient {
    const NAME: &'static str = "GstWhepClientSignaller";
    type Type = super::WhepClientSignaller;
    type ParentType = glib::Object;
    type Interfaces = (Signallable,);
}

impl WhepClient {
    fn raise_error(&self, msg: String) {
        self.obj()
            .emit_by_name::<()>("error", &[&format!("Error: {msg}")]);
    }

    fn handle_future_error(&self, err: WaitError) {
        match err {
            WaitError::FutureAborted => {
                gst::warning!(CAT, imp = self, "Future aborted")
            }
            WaitError::FutureError(err) => self.raise_error(err.to_string()),
        };
    }

    fn sdp_message_parse(&self, sdp_bytes: Bytes, _webrtcbin: &gst::Element) {
        let sdp = match sdp_message::SDPMessage::parse_buffer(&sdp_bytes) {
            Ok(sdp) => sdp,
            Err(_) => {
                self.raise_error("Could not parse answer SDP".to_string());
                return;
            }
        };

        let remote_sdp = WebRTCSessionDescription::new(WebRTCSDPType::Answer, sdp);

        self.obj()
            .emit_by_name::<()>("session-description", &[&SESSION_ID, &remote_sdp]);
    }

    async fn parse_endpoint_response(
        &self,
        sess_desc: WebRTCSessionDescription,
        resp: reqwest::Response,
        redirects: u8,
        webrtcbin: gst::Element,
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
                    if let Err(e) = set_ice_servers(&webrtcbin, resp.headers()) {
                        self.raise_error(e.to_string());
                        return;
                    };
                }

                /* See section 4.2 of the WHEP specification */
                let location = match resp.headers().get(reqwest::header::LOCATION) {
                    Some(location) => location,
                    None => {
                        self.raise_error(
                            "Location header field should be present for WHEP resource URL"
                                .to_string(),
                        );
                        return;
                    }
                };

                let location = match location.to_str() {
                    Ok(loc) => loc,
                    Err(e) => {
                        self.raise_error(format!("Failed to convert location to string: {e}"));
                        return;
                    }
                };

                let url = reqwest::Url::parse(endpoint.as_str()).unwrap();

                gst::debug!(CAT, imp = self, "WHEP resource: {:?}", location);

                let url = match url.join(location) {
                    Ok(joined_url) => joined_url,
                    Err(err) => {
                        self.raise_error(format!("URL join operation failed: {err:?}"));
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
                                self.raise_error("Expected to be in POST state".to_string());
                                return;
                            }
                        };
                        drop(state);

                        self.sdp_message_parse(ans_bytes, &webrtcbin)
                    }
                    Err(err) => self.raise_error(err.to_string()),
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

                            Box::pin(self.do_post(sess_desc, webrtcbin, redirect_url)).await
                        }
                        Err(e) => self.raise_error(e.to_string()),
                    }
                } else {
                    self.raise_error("Too many redirects. Unable to connect.".to_string());
                }
            }

            s => {
                match resp.bytes().await {
                    Ok(r) => {
                        let res = r.escape_ascii().to_string();

                        // FIXME: Check and handle 'Retry-After' header in case of server error
                        self.raise_error(format!("Unexpected response: {} - {}", s.as_str(), res));
                    }
                    Err(err) => self.raise_error(err.to_string()),
                }
            }
        }
    }

    async fn whep_offer(&self, webrtcbin: gst::Element) {
        let local_desc =
            webrtcbin.property::<Option<WebRTCSessionDescription>>("local-description");

        let sess_desc = match local_desc {
            None => {
                self.raise_error("Local description is not set".to_string());
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
            "Sending offer SDP: {}",
            sess_desc.sdp().to_string()
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

        if let Err(e) = wait_async(
            &self.canceller,
            self.do_post(sess_desc, webrtcbin, endpoint),
            timeout,
        )
        .await
        {
            self.handle_future_error(e);
        }
    }

    async fn do_post(
        &self,
        offer: WebRTCSessionDescription,
        webrtcbin: gst::Element,
        endpoint: reqwest::Url,
    ) {
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
                            self.raise_error("Trying to do POST in unexpected state".to_string());
                            return;
                        }
                    };
                    drop(state);
                }

                self.parse_endpoint_response(offer, r, redirects, webrtcbin)
                    .await
            }
            Err(err) => self.raise_error(err.to_string()),
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
                self.raise_error("Terminated in unexpected state".to_string());
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

    pub fn on_webrtcbin_ready(&self) -> RustClosure {
        glib::closure!(|signaller: &super::WhepClientSignaller,
                        _consumer_identifier: &str,
                        webrtcbin: &gst::Element| {
            webrtcbin.connect_notify(
                Some("ice-gathering-state"),
                glib::clone!(
                    #[weak]
                    signaller,
                    move |webrtcbin, _pspec| {
                        let state =
                            webrtcbin.property::<WebRTCICEGatheringState>("ice-gathering-state");

                        match state {
                            WebRTCICEGatheringState::Gathering => {
                                gst::info!(CAT, obj = signaller, "ICE gathering started");
                            }
                            WebRTCICEGatheringState::Complete => {
                                gst::info!(CAT, obj = signaller, "ICE gathering complete");

                                let webrtcbin = webrtcbin.clone();

                                RUNTIME.spawn(async move {
                                    signaller.imp().whep_offer(webrtcbin).await
                                });
                            }
                            _ => (),
                        }
                    }
                ),
            );
        })
    }
}

impl SignallableImpl for WhepClient {
    fn start(&self) {
        if self.settings.lock().unwrap().whep_endpoint.is_none() {
            self.raise_error("WHEP endpoint URL must be set".to_string());
            return;
        }

        let mut state = self.state.lock().unwrap();
        *state = State::Post { redirects: 0 };
        drop(state);

        let this_weak = self.downgrade();

        // the state lock will be already held at this point in the `maybe_start_signaller` of BaseWebRTCSrc
        // and the `start_session` will also need the state mutex to check for existing sessions with same id
        // so spawn a new thread and emit the signals inside it, so that there won't be a deadlock
        RUNTIME.spawn(async move {
            if let Some(this) = this_weak.upgrade() {
                this.obj()
                    .emit_by_name::<()>("session-started", &[&SESSION_ID, &SESSION_ID]);

                this.obj().emit_by_name::<()>(
                    "session-requested",
                    &[
                        &SESSION_ID,
                        &SESSION_ID,
                        &None::<gst_webrtc::WebRTCSessionDescription>,
                    ],
                );
            }
        });
    }

    fn stop(&self) {}

    fn end_session(&self, _session_id: &str) {
        // Interrupt requests in progress, if any
        if let Some(canceller) = &*self.canceller.lock().unwrap() {
            canceller.abort();
        }

        let state = self.state.lock().unwrap();
        if let State::Running { .. } = *state {
            // Release server-side resources
            drop(state);
            self.terminate_session();
        }
    }
}
