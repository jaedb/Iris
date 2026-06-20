// SPDX-License-Identifier: MPL-2.0

use gst::prelude::*;

use crate::signaller::{prelude::*, Signallable, Signaller};
use crate::utils::{self, Codec, Codecs, NavigationEvent, AUDIO_CAPS, RTP_CAPS, VIDEO_CAPS};
use crate::webrtcsrc::WebRTCSrcPad;
use crate::RUNTIME;
use anyhow::{Context, Error};
use gst::glib;
use gst::subclass::prelude::*;
use gst_webrtc::WebRTCDataChannel;
use itertools::Itertools;
use std::borrow::BorrowMut;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::atomic::AtomicU16;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Condvar, LazyLock, Mutex};
use url::Url;

const DEFAULT_STUN_SERVER: Option<&str> = Some("stun://stun.l.google.com:19302");
const DEFAULT_ENABLE_DATA_CHANNEL_NAVIGATION: bool = false;
const DEFAULT_ENABLE_CONTROL_DATA_CHANNEL: bool = false;
const DEFAULT_DO_RETRANSMISSION: bool = true;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "webrtcsrc",
        gst::DebugColorFlags::empty(),
        Some("WebRTC src"),
    )
});

struct Settings {
    stun_server: Option<String>,
    turn_servers: gst::Array,
    signaller: Signallable,
    meta: Option<gst::Structure>,
    video_codecs: Vec<Codec>,
    audio_codecs: Vec<Codec>,
    enable_data_channel_navigation: bool,
    enable_control_data_channel: bool,
    do_retransmission: bool,
}

#[derive(Default)]
pub struct BaseWebRTCSrc {
    settings: Mutex<Settings>,
    state: Mutex<State>,
}

#[glib::object_subclass]
impl ObjectSubclass for BaseWebRTCSrc {
    const NAME: &'static str = "GstBaseWebRTCSrc";
    type Type = super::BaseWebRTCSrc;
    type ParentType = gst::Bin;
    type Interfaces = (gst::ChildProxy,);
}

unsafe impl<T: BaseWebRTCSrcImpl> IsSubclassable<T> for super::BaseWebRTCSrc {
    fn class_init(class: &mut glib::Class<Self>) {
        Self::parent_class_init::<T>(class);
    }
}
pub(crate) trait BaseWebRTCSrcImpl:
    BinImpl + ObjectSubclass<Type: IsA<super::BaseWebRTCSrc>>
{
}

impl ObjectImpl for BaseWebRTCSrc {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPS: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
               glib::ParamSpecString::builder("stun-server")
                   .nick("The STUN server to use")
                   .blurb("The STUN server of the form stun://host:port")
                   .flags(glib::ParamFlags::READWRITE)
                   .default_value(DEFAULT_STUN_SERVER)
                   .mutable_ready()
                   .build(),
               gst::ParamSpecArray::builder("turn-servers")
                   .nick("List of TURN servers to use")
                   .blurb("The TURN servers of the form <\"turn(s)://username:password@host:port\", \"turn(s)://username1:password1@host1:port1\">")
                   .element_spec(&glib::ParamSpecString::builder("turn-server")
                       .nick("TURN Server")
                       .blurb("The TURN server of the form turn(s)://username:password@host:port.")
                       .build()
                   )
                   .mutable_ready()
                   .build(),
               glib::ParamSpecObject::builder::<Signallable>("signaller")
                   .flags(glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY)
                   .blurb("The Signallable object to use to handle WebRTC Signalling")
                   .build(),
               glib::ParamSpecBoxed::builder::<gst::Structure>("meta")
                   .flags(glib::ParamFlags::READWRITE | gst::PARAM_FLAG_MUTABLE_READY)
                   .blurb("Free form metadata about the consumer")
                   .build(),
               gst::ParamSpecArray::builder("video-codecs")
                   .flags(glib::ParamFlags::READWRITE | gst::PARAM_FLAG_MUTABLE_READY)
                   .blurb(&format!("Names of video codecs to be be used during the SDP negotiation. Valid values: [{}]",
                       Codecs::video_codecs()
                           .map(|c| c.name.as_str())
                           .join(", ")
                   ))
                   .element_spec(&glib::ParamSpecString::builder("video-codec-name").build())
                   .build(),
               gst::ParamSpecArray::builder("audio-codecs")
                   .flags(glib::ParamFlags::READWRITE | gst::PARAM_FLAG_MUTABLE_READY)
                   .blurb(&format!("Names of audio codecs to be be used during the SDP negotiation. Valid values: [{}]",
                       Codecs::audio_codecs()
                           .map(|c| c.name.as_str())
                           .join(", ")
                   ))
                   .element_spec(&glib::ParamSpecString::builder("audio-codec-name").build())
                   .build(),
               /**
                * GstBaseWebRTCSrc:enable-data-channel-navigation:
                *
                * Enable navigation events through a dedicated WebRTCDataChannel.
                *
                * Deprecated:plugins-rs-0.14.0: Use #GstBaseWebRTCSrc:enable-control-data-channel
                */
               glib::ParamSpecBoolean::builder("enable-data-channel-navigation")
                   .nick("Enable data channel navigation")
                   .blurb("Enable navigation events through a dedicated WebRTCDataChannel")
                   .default_value(DEFAULT_ENABLE_DATA_CHANNEL_NAVIGATION)
                   .mutable_ready()
                   .build(),
               /**
                * GstBaseWebRTCSink:enable-control-data-channel:
                *
                * Enable sending control requests through data channel.
                * This includes but is not limited to the forwarding of navigation events.
                *
                * Since: plugins-rs-0.14.0
                */
               glib::ParamSpecBoolean::builder("enable-control-data-channel")
                   .nick("Enable control data channel")
                   .blurb("Enable sending control requests through a dedicated WebRTCDataChannel")
                   .default_value(DEFAULT_ENABLE_CONTROL_DATA_CHANNEL)
                   .mutable_ready()
                   .build(),
               glib::ParamSpecBoolean::builder("do-retransmission")
                   .nick("Enable retransmission")
                   .blurb("Send retransmission events upstream when a packet is late")
                   .default_value(DEFAULT_DO_RETRANSMISSION)
                   .mutable_ready()
                   .build(),
            ]
        });

        PROPS.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "signaller" => {
                let signaller = value
                    .get::<Option<Signallable>>()
                    .expect("type checked upstream");
                if let Some(signaller) = signaller {
                    self.settings.lock().unwrap().signaller = signaller;
                }
                // else: signaller not set as a construct property
                //       => use default Signaller
            }
            "video-codecs" => {
                self.settings.lock().unwrap().video_codecs = value
                    .get::<gst::ArrayRef>()
                    .expect("type checked upstream")
                    .as_slice()
                    .iter()
                    .filter_map(|codec_name| {
                        Codecs::find(codec_name.get::<&str>().expect("type checked upstream"))
                    })
                    .collect::<Vec<Codec>>()
            }
            "audio-codecs" => {
                self.settings.lock().unwrap().audio_codecs = value
                    .get::<gst::ArrayRef>()
                    .expect("type checked upstream")
                    .as_slice()
                    .iter()
                    .filter_map(|codec_name| {
                        Codecs::find(codec_name.get::<&str>().expect("type checked upstream"))
                    })
                    .collect::<Vec<Codec>>()
            }
            "stun-server" => {
                self.settings.lock().unwrap().stun_server = value
                    .get::<Option<String>>()
                    .expect("type checked upstream");
            }
            "turn-servers" => {
                let mut settings = self.settings.lock().unwrap();
                settings.turn_servers = value.get::<gst::Array>().expect("type checked upstream");
            }
            "meta" => {
                self.settings.lock().unwrap().meta = value
                    .get::<Option<gst::Structure>>()
                    .expect("type checked upstream")
            }
            "enable-data-channel-navigation" => {
                let mut settings = self.settings.lock().unwrap();
                settings.enable_data_channel_navigation = value.get::<bool>().unwrap();
            }
            "enable-control-data-channel" => {
                let mut settings = self.settings.lock().unwrap();
                settings.enable_control_data_channel = value.get::<bool>().unwrap();
            }
            "do-retransmission" => {
                let mut settings = self.settings.lock().unwrap();
                settings.do_retransmission = value.get::<bool>().unwrap();
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "signaller" => self.settings.lock().unwrap().signaller.to_value(),
            "video-codecs" => gst::Array::new(
                self.settings
                    .lock()
                    .unwrap()
                    .video_codecs
                    .iter()
                    .map(|v| &v.name),
            )
            .to_value(),
            "audio-codecs" => gst::Array::new(
                self.settings
                    .lock()
                    .unwrap()
                    .audio_codecs
                    .iter()
                    .map(|v| &v.name),
            )
            .to_value(),
            "stun-server" => self.settings.lock().unwrap().stun_server.to_value(),
            "turn-servers" => self.settings.lock().unwrap().turn_servers.to_value(),
            "meta" => self.settings.lock().unwrap().meta.to_value(),
            "enable-data-channel-navigation" => {
                let settings = self.settings.lock().unwrap();
                settings.enable_data_channel_navigation.to_value()
            }
            "enable-control-data-channel" => {
                let settings = self.settings.lock().unwrap();
                settings.enable_control_data_channel.to_value()
            }
            "do-retransmission" => self.settings.lock().unwrap().do_retransmission.to_value(),
            name => panic!("{name} getter not implemented"),
        }
    }

    fn signals() -> &'static [glib::subclass::Signal] {
        static SIGNALS: LazyLock<Vec<glib::subclass::Signal>> = LazyLock::new(|| {
            vec![
                /**
                 * GstBaseWebRTCSrc::request-encoded-filter:
                 * @producer_id: (nullable): Identifier of the producer
                 * @pad_name: The name of the output pad
                 * @allowed_caps: the allowed caps for the output pad
                 *
                 * This signal can be used to insert a filter
                 * element between:
                 *
                 * - the depayloader and the decoder.
                 * - the depayloader and downstream element if
                 *   no decoders are used.
                 *
                 * Returns: the element to insert.
                 */
                glib::subclass::Signal::builder("request-encoded-filter")
                    .param_types([
                        Option::<String>::static_type(),
                        String::static_type(),
                        Option::<gst::Caps>::static_type(),
                    ])
                    .return_type::<gst::Element>()
                    .build(),
            ]
        });

        SIGNALS.as_ref()
    }

    fn constructed(&self) {
        self.parent_constructed();
        let signaller = self.settings.lock().unwrap().signaller.clone();

        self.connect_signaller(&signaller);

        let obj = &*self.obj();

        obj.set_suppressed_flags(gst::ElementFlags::SINK | gst::ElementFlags::SOURCE);
        obj.set_element_flags(gst::ElementFlags::SOURCE);
    }
}

impl Default for Settings {
    fn default() -> Self {
        let signaller = Signaller::default();

        Self {
            stun_server: DEFAULT_STUN_SERVER.map(|v| v.to_string()),
            turn_servers: Default::default(),
            signaller: signaller.upcast(),
            meta: Default::default(),
            audio_codecs: Codecs::audio_codecs()
                .filter(|codec| !codec.is_raw)
                .cloned()
                .collect(),
            video_codecs: Codecs::video_codecs()
                .filter(|codec| !codec.is_raw)
                .cloned()
                .collect(),
            enable_data_channel_navigation: DEFAULT_ENABLE_DATA_CHANNEL_NAVIGATION,
            enable_control_data_channel: DEFAULT_ENABLE_CONTROL_DATA_CHANNEL,
            do_retransmission: DEFAULT_DO_RETRANSMISSION,
        }
    }
}

#[allow(dead_code)]
struct SignallerSignals {
    error: glib::SignalHandlerId,
    session_started: glib::SignalHandlerId,
    session_ended: glib::SignalHandlerId,
    request_meta: glib::SignalHandlerId,
    session_description: glib::SignalHandlerId,
    handle_ice: glib::SignalHandlerId,
    session_requested: glib::SignalHandlerId,
}

impl SessionInner {
    fn webrtcbin(&self) -> gst::Bin {
        self.webrtcbin.clone().downcast::<gst::Bin>().unwrap()
    }

    fn new(session_id: &str) -> Result<Self, Error> {
        let webrtcbin = gst::ElementFactory::make("webrtcbin")
            .property("bundle-policy", gst_webrtc::WebRTCBundlePolicy::MaxBundle)
            .build()
            .with_context(|| "Failed to make element webrtcbin".to_string())?;

        Ok(Self {
            id: session_id.to_string(),
            webrtcbin,
            data_channel: None,
            n_video_pads: AtomicU16::new(0),
            n_audio_pads: AtomicU16::new(0),
            flow_combiner: Mutex::new(gst_base::UniqueFlowCombiner::new()),
            request_counter: 0,
            pending_srcpads: HashMap::new(),
        })
    }

    fn get_stream_id(
        &self,
        transceiver: Option<gst_webrtc::WebRTCRTPTransceiver>,
        mline: Option<u32>,
    ) -> Option<String> {
        let mline = transceiver.map_or(mline, |t| Some(t.mlineindex()));

        // making a hash of the session ID and adding `:<some-id>`,
        // here the ID is the mline of the stream in the SDP.
        mline.map(|mline| {
            let mut cs = glib::Checksum::new(glib::ChecksumType::Sha256).unwrap();
            cs.update(self.id.as_bytes());
            format!("{}:{mline}", cs.string().unwrap())
        })
    }

    // Maps the `webrtcbin` pad to our exposed source pad using the pad stream ID.
    fn take_pending_src_pad(
        &mut self,
        webrtcbin_src: &gst::Pad,
    ) -> Option<(WebRTCSrcPad, gst::Caps)> {
        self.get_stream_id(
            Some(webrtcbin_src.property::<gst_webrtc::WebRTCRTPTransceiver>("transceiver")),
            None,
        )
        .and_then(|stream_id| self.pending_srcpads.remove(&stream_id))
    }

    // Maps the `webrtcbin` pad to our exposed source pad using the pad stream ID.
    fn get_src_pad_from_webrtcbin_pad(
        &self,
        webrtcbin_src: &gst::Pad,
        element: &super::BaseWebRTCSrc,
    ) -> Option<WebRTCSrcPad> {
        self.get_stream_id(
            Some(webrtcbin_src.property::<gst_webrtc::WebRTCRTPTransceiver>("transceiver")),
            None,
        )
        .and_then(|stream_id| {
            element.iterate_src_pads().into_iter().find_map(|s| {
                let pad = s.ok()?.downcast::<WebRTCSrcPad>().unwrap();
                if pad.imp().stream_id() == stream_id {
                    Some(pad)
                } else {
                    None
                }
            })
        })
    }

    fn send_navigation_event(
        &mut self,
        evt: gst_video::NavigationEvent,
        element: &super::BaseWebRTCSrc,
    ) {
        if let Some(data_channel) = &self.data_channel.borrow_mut() {
            let nav_event = NavigationEvent {
                mid: None,
                event: evt,
            };
            match serde_json::to_string(&nav_event).ok() {
                Some(str) => {
                    gst::trace!(
                        CAT,
                        obj = element,
                        "Sending navigation event to peer for session {}",
                        self.id
                    );
                    if let Err(err) = data_channel.send_string_full(Some(str.as_str())) {
                        gst::error!(
                            CAT,
                            obj = element,
                            "Failed sending navigation event to peer for session {}: {err}",
                            self.id,
                        );
                    }
                }
                None => {
                    gst::error!(
                        CAT,
                        obj = element,
                        "Could not serialize navigation event for session {}",
                        self.id
                    );
                }
            }
        }
    }

    fn send_control_request(
        &mut self,
        request: utils::ControlRequest,
        element: &super::BaseWebRTCSrc,
        mid: Option<String>,
    ) {
        if let Some(data_channel) = &self.data_channel.borrow_mut() {
            let msg = utils::ControlRequestMessage {
                id: self.request_counter,
                mid,
                request: utils::StringOrRequest::Request(request),
            };
            self.request_counter += 1;
            match serde_json::to_string(&msg).ok() {
                Some(str) => {
                    gst::trace!(
                        CAT,
                        obj = element,
                        "Sending control request to peer for session {}",
                        self.id
                    );
                    if let Err(err) = data_channel.send_string_full(Some(str.as_str())) {
                        gst::error!(
                            CAT,
                            obj = element,
                            "Failed sending control request to peer for session {}: {err}",
                            self.id,
                        );
                    }
                }
                None => {
                    gst::error!(
                        CAT,
                        obj = element,
                        "Could not serialize navigation event for session {}",
                        self.id
                    );
                }
            }
        }
    }

    // Creates a bin which contains the webrtcbin, encoded filter (if requested) plus parser
    // and decoder (if needed) for every session
    //
    // The ghostpad of the session's bin will be the target pad of the webrtcsrc's srcpad
    // corresponding to the session.
    //
    // The target pad for the session's bin ghostpad will be
    //      - the decoder's srcpad, if decoder is needed
    //      - otherwise, encoded filter's srcpad, if requested
    //      - otherwise, webrtcbin's src pad.
    fn handle_webrtc_src_pad(
        &mut self,
        bin: &gst::Bin,
        webrtcbin_pad: &gst::Pad,
        element: &super::BaseWebRTCSrc,
    ) -> gst::GhostPad {
        let srcpad_and_caps = self.take_pending_src_pad(webrtcbin_pad);
        if let Some((ref srcpad, ref caps)) = srcpad_and_caps {
            let stream_id = srcpad.imp().stream_id();
            let mut builder = gst::event::StreamStart::builder(&stream_id);
            if let Some(stream_start) = webrtcbin_pad.sticky_event::<gst::event::StreamStart>(0) {
                builder = builder
                    .seqnum(stream_start.seqnum())
                    .group_id(stream_start.group_id().unwrap_or_else(gst::GroupId::next));
            }

            gst::debug!(
                CAT,
                obj = element,
                "Storing id {stream_id} on {webrtcbin_pad:?} for session {}",
                self.id
            );
            webrtcbin_pad.store_sticky_event(&builder.build()).ok();

            srcpad.imp().set_webrtc_pad(webrtcbin_pad.downgrade());

            element
                .add_pad(srcpad)
                .expect("Adding ghost pad should never fail");
            let media_type = caps
                .structure(0)
                .expect("Passing empty caps is invalid")
                .get::<&str>("media")
                .expect("Only caps with a `media` field are expected when creating the pad");

            let raw_caps = if media_type == "video" {
                VIDEO_CAPS.to_owned()
            } else if media_type == "audio" {
                AUDIO_CAPS.to_owned()
            } else {
                unreachable!()
            };

            let caps_with_raw = [caps.clone(), raw_caps.clone()]
                .into_iter()
                .collect::<gst::Caps>();

            let downstream_caps = srcpad.peer_query_caps(Some(&caps_with_raw));
            if let Some(first_struct) = downstream_caps.structure(0) {
                if first_struct.has_name(raw_caps.structure(0).unwrap().name()) {
                    srcpad.imp().set_needs_decoding(true)
                }
            }
        }

        let ghostpad = gst::GhostPad::builder(gst::PadDirection::Src)
            .proxy_pad_chain_function(glib::clone!(
                #[weak]
                element,
                #[strong(rename_to = sess_id)]
                self.id,
                #[upgrade_or_panic]
                move |pad, parent, buffer| {
                    let padret = gst::ProxyPad::chain_default(pad, parent, buffer);
                    let state = element.imp().state.lock().unwrap();
                    if let Some(session) = state.sessions.get(&sess_id) {
                        let session = session.0.lock().unwrap();
                        let f = session.flow_combiner.lock().unwrap().update_flow(padret);
                        f
                    } else {
                        padret
                    }
                }
            ))
            .proxy_pad_event_function(glib::clone!(
                #[weak]
                element,
                #[weak(rename_to = webrtcpad)]
                webrtcbin_pad,
                #[strong(rename_to = sess_id)]
                self.id,
                #[upgrade_or_panic]
                move |pad, parent, event| {
                    let event = if let gst::EventView::StreamStart(stream_start) = event.view() {
                        let state = element.imp().state.lock().unwrap();
                        if let Some(session) = state.sessions.get(&sess_id) {
                            session
                                .0
                                .lock()
                                .unwrap()
                                .get_src_pad_from_webrtcbin_pad(&webrtcpad, &element)
                                .map(|srcpad| {
                                    gst::event::StreamStart::builder(&srcpad.imp().stream_id())
                                        .seqnum(stream_start.seqnum())
                                        .group_id(
                                            stream_start
                                                .group_id()
                                                .unwrap_or_else(gst::GroupId::next),
                                        )
                                        .build()
                                })
                                .unwrap_or(event)
                        } else {
                            gst::error!(CAT, obj = element, "session {sess_id:?} does not exist");
                            event
                        }
                    } else {
                        event
                    };

                    gst::Pad::event_default(pad, parent, event)
                }
            ))
            .build();

        let (enable_data_channel_navigation, enable_control_data_channel) = {
            let settings = element.imp().settings.lock().unwrap();
            (
                settings.enable_data_channel_navigation,
                settings.enable_control_data_channel,
            )
        };

        if enable_data_channel_navigation || enable_control_data_channel {
            webrtcbin_pad.add_probe(
                gst::PadProbeType::EVENT_UPSTREAM,
                glib::clone!(
                    #[weak]
                    element,
                    #[strong(rename_to = sess_id)]
                    self.id,
                    #[upgrade_or]
                    gst::PadProbeReturn::Remove,
                    move |pad, info| {
                        let Some(ev) = info.event() else {
                            return gst::PadProbeReturn::Ok;
                        };

                        match ev.view() {
                            gst::EventView::Navigation(nav) => {
                                let mut state = element.imp().state.lock().unwrap();
                                if let Some(session) = state.sessions.get_mut(&sess_id) {
                                    let mut session = session.0.lock().unwrap();
                                    if enable_data_channel_navigation {
                                        session.send_navigation_event(
                                            gst_video::NavigationEvent::parse(nav).unwrap(),
                                            &element,
                                        );
                                    }
                                    if enable_control_data_channel {
                                        let transceiver = pad
                                            .property::<gst_webrtc::WebRTCRTPTransceiver>(
                                                "transceiver",
                                            );
                                        let request = utils::ControlRequest::NavigationEvent {
                                            event: gst_video::NavigationEvent::parse(nav).unwrap(),
                                        };
                                        session.send_control_request(
                                            request,
                                            &element,
                                            transceiver.mid().map(|s| s.to_string()),
                                        );
                                    }
                                }
                            }
                            gst::EventView::CustomUpstream(cup) => {
                                let transceiver =
                                    pad.property::<gst_webrtc::WebRTCRTPTransceiver>("transceiver");
                                let mut state = element.imp().state.lock().unwrap();
                                if let Some(session) = state.sessions.get_mut(&sess_id) {
                                    let mut session = session.0.lock().unwrap();
                                    if enable_control_data_channel {
                                        if let Some(s) = cup.structure() {
                                            if let Some(structure) =
                                                utils::gvalue_to_json(&s.to_value())
                                            {
                                                let request =
                                                    utils::ControlRequest::CustomUpstreamEvent {
                                                        structure_name: s.name().to_string(),
                                                        structure,
                                                    };
                                                session.send_control_request(
                                                    request,
                                                    &element,
                                                    transceiver.mid().map(|s| s.to_string()),
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                            _ => (),
                        }

                        gst::PadProbeReturn::Ok
                    }
                ),
            );
        }

        if let Some((srcpad, _)) = srcpad_and_caps {
            let signaller = element.imp().signaller();

            // Signalers like WhipServer do not need a peer producer id as they run as a server
            // waiting for a peer connection so they don't have that property. In that case use
            // the session id as producer id

            // In order to avoid breaking any existing signallers that depend on peer-producer-id,
            // continue to use that or fallback to webrtcbin pad's msid if the
            // peer-producer-id is None.
            let producer_id = if signaller
                .has_property_with_type("producer-peer-id", Option::<String>::static_type())
            {
                signaller
                    .property::<Option<String>>("producer-peer-id")
                    .or_else(|| webrtcbin_pad.property("msid"))
            } else {
                Some(self.id.clone())
            };

            let encoded_filter = element.emit_by_name::<Option<gst::Element>>(
                "request-encoded-filter",
                &[&producer_id, &srcpad.name(), &srcpad.allowed_caps()],
            );

            if srcpad.imp().needs_decoding() {
                let decodebin = gst::ElementFactory::make("decodebin3")
                    .build()
                    .expect("decodebin3 needs to be present!");
                bin.add(&decodebin).unwrap();
                decodebin.sync_state_with_parent().unwrap();
                decodebin.connect_pad_added(glib::clone!(
                    #[weak]
                    ghostpad,
                    move |_webrtcbin, pad| {
                        if pad.direction() == gst::PadDirection::Sink {
                            return;
                        }

                        ghostpad.set_target(Some(pad)).unwrap();
                    }
                ));

                gst::debug!(
                    CAT,
                    obj = element,
                    "Decoding for {}",
                    srcpad.imp().stream_id()
                );

                if let Some(encoded_filter) = encoded_filter {
                    let filter_sink_pad = encoded_filter
                        .static_pad("sink")
                        .expect("encoded filter must expose a static sink pad");

                    let parsebin = gst::ElementFactory::make("parsebin")
                        .build()
                        .expect("parsebin needs to be present!");
                    bin.add_many([&parsebin, &encoded_filter]).unwrap();

                    parsebin.connect_pad_added(move |_, pad| {
                        pad.link(&filter_sink_pad)
                            .expect("parsebin ! encoded_filter linking failed");
                        encoded_filter
                            .link(&decodebin)
                            .expect("encoded_filter ! decodebin3 linking failed");

                        encoded_filter.sync_state_with_parent().unwrap();
                    });

                    webrtcbin_pad
                        .link(&parsebin.static_pad("sink").unwrap())
                        .expect("webrtcbin ! parsebin linking failed");

                    parsebin.sync_state_with_parent().unwrap();
                } else {
                    let sinkpad = decodebin
                        .static_pad("sink")
                        .expect("decodebin has a sink pad");
                    webrtcbin_pad
                        .link(&sinkpad)
                        .expect("webrtcbin ! decodebin3 linking failed");
                }
            } else {
                gst::debug!(
                    CAT,
                    obj = element,
                    "NO decoding for {}",
                    srcpad.imp().stream_id()
                );

                if let Some(encoded_filter) = encoded_filter {
                    let filter_sink_pad = encoded_filter
                        .static_pad("sink")
                        .expect("encoded filter must expose a static sink pad");
                    let filter_src_pad = encoded_filter
                        .static_pad("src")
                        .expect("encoded filter must expose a static src pad");

                    bin.add(&encoded_filter).unwrap();

                    webrtcbin_pad
                        .link(&filter_sink_pad)
                        .expect("webrtcbin ! encoded_filter linking failed");

                    encoded_filter.sync_state_with_parent().unwrap();
                    ghostpad.set_target(Some(&filter_src_pad)).unwrap();
                } else {
                    // No decoder or filter
                    ghostpad.set_target(Some(webrtcbin_pad)).unwrap();
                }
            }
            srcpad.set_target(Some(&ghostpad)).unwrap();
        } else {
            gst::debug!(
                CAT,
                obj = element,
                "Unused webrtcbin pad {} {:?}",
                webrtcbin_pad.name(),
                webrtcbin_pad.current_caps(),
            );
        }
        ghostpad
    }

    fn generate_offer(&self, element: &super::BaseWebRTCSrc) {
        let sess_id = self.id.clone();
        let webrtcbin = self.webrtcbin();
        let direction = gst_webrtc::WebRTCRTPTransceiverDirection::Recvonly;
        let mut pt = 96..127;
        let settings = element.imp().settings.lock().unwrap();
        let caps = settings
            .video_codecs
            .iter()
            .chain(settings.audio_codecs.iter())
            .map(|codec| {
                let name = &codec.name;

                let Some(pt) = pt.next() else {
                    gst::warning!(
                        CAT,
                        obj = element,
                        "exhausted the list of dynamic payload types, not adding transceiver for {name}"
                    );
                    return None;
                };

                let (media, clock_rate) = if codec.is_video() {
                    ("video", codec.clock_rate().unwrap_or(90000))
                } else {
                    ("audio", codec.clock_rate().unwrap_or(48000))
                };

                let mut caps = gst::Caps::new_empty();
                {
                    let caps = caps.get_mut().unwrap();
                    let mut s = gst::Structure::builder("application/x-rtp")
                        .field("media", media)
                        .field("payload", pt)
                        .field("encoding-name", name.as_str())
                        .field("clock-rate", clock_rate)
                        .build();

                    if name.eq_ignore_ascii_case("H264") {
                        // support the constrained-baseline profile for now
                        // TODO: extend this to other supported profiles by querying the decoders
                        s.set("profile-level-id", "42e016");
                    }

                    caps.append_structure(s);
                }
                Some(caps)
            });

        for c in caps.flatten() {
            gst::info!(CAT, obj = element, "Adding transceiver with caps: {c:#?}");
            let transceiver = webrtcbin.emit_by_name::<gst_webrtc::WebRTCRTPTransceiver>(
                "add-transceiver",
                &[&direction, &c],
            );

            transceiver.set_property("do_nack", settings.do_retransmission);
            transceiver.set_property("fec-type", gst_webrtc::WebRTCFECType::UlpRed);
        }

        let webrtcbin_weak = webrtcbin.downgrade();
        let promise = gst::Promise::with_change_func(glib::clone!(
            #[weak (rename_to = ele)]
            element,
            move |reply| {
                let Some(webrtcbin) = webrtcbin_weak.upgrade() else {
                    gst::error!(CAT, obj = ele, "generate offer::failed to get webrtcbin");
                    ele.imp().signaller().end_session(sess_id.as_str());
                    return;
                };

                let reply = match reply {
                    Ok(Some(reply)) => reply,
                    Ok(None) => {
                        gst::error!(
                            CAT,
                            obj = ele,
                            "generate offer::Promise returned with no reply"
                        );
                        ele.imp().signaller().end_session(sess_id.as_str());
                        return;
                    }
                    Err(e) => {
                        gst::error!(
                            CAT,
                            obj = ele,
                            "generate offer::Promise returned with error {:?}",
                            e
                        );
                        ele.imp().signaller().end_session(sess_id.as_str());
                        return;
                    }
                };

                if let Ok(offer_sdp) = reply
                    .value("offer")
                    .map(|offer| offer.get::<gst_webrtc::WebRTCSessionDescription>().unwrap())
                {
                    webrtcbin.emit_by_name::<()>(
                        "set-local-description",
                        &[&offer_sdp, &None::<gst::Promise>],
                    );

                    gst::log!(
                        CAT,
                        obj = ele,
                        "Sending SDP, {}",
                        offer_sdp.sdp().to_string()
                    );

                    let signaller = ele.imp().signaller();
                    signaller.send_sdp(sess_id.as_str(), &offer_sdp);
                } else {
                    let error = reply
                        .value("error")
                        .expect("structure must have an error value")
                        .get::<glib::Error>()
                        .expect("value must be a GLib error");

                    gst::error!(
                        CAT,
                        obj = ele,
                        "generate offer::Promise returned with error: {}",
                        error
                    );
                    ele.imp().signaller().end_session(sess_id.as_str());
                }
            }
        ));

        webrtcbin
            .clone()
            .emit_by_name::<()>("create-offer", &[&None::<gst::Structure>, &promise]);
    }

    fn remote_description_set(
        &mut self,
        element: &super::BaseWebRTCSrc,
        desc: &gst_webrtc::WebRTCSessionDescription,
    ) -> (gst::Promise, gst::Bin) {
        let sdp = desc.sdp();
        let desc_type = desc.type_();
        let webrtcbin = self.webrtcbin();
        for (i, media) in sdp.medias().enumerate() {
            let (codec_names, do_retransmission) = {
                let settings = element.imp().settings.lock().unwrap();
                (
                    settings
                        .video_codecs
                        .iter()
                        .chain(settings.audio_codecs.iter())
                        .map(|codec| codec.name.clone())
                        .collect::<HashSet<String>>(),
                    settings.do_retransmission,
                )
            };
            let caps = media
                .formats()
                .filter_map(|format| {
                    format.parse::<i32>().ok().and_then(|pt| {
                        let mut mediacaps = media.caps_from_media(pt)?;
                        let s = mediacaps.structure(0).unwrap();
                        if !codec_names.contains(s.get::<&str>("encoding-name").ok()?) {
                            return None;
                        }

                        let mut filtered_s = gst::Structure::new_empty("application/x-rtp");
                        filtered_s.extend(s.iter().filter_map(|(key, value)| {
                            if key.starts_with("rtcp-") {
                                None
                            } else {
                                Some((key, value.to_owned()))
                            }
                        }));

                        if media
                            .attributes_to_caps(mediacaps.get_mut().unwrap())
                            .is_err()
                        {
                            gst::warning!(
                                CAT,
                                obj = element,
                                "Failed to retrieve attributes from media!"
                            );
                            return None;
                        }

                        let s = mediacaps.structure(0).unwrap();

                        filtered_s.extend(s.iter().filter_map(|(key, value)| {
                            if key.starts_with("extmap-") {
                                return Some((key, value.to_owned()));
                            }

                            None
                        }));

                        Some(filtered_s)
                    })
                })
                .collect::<gst::Caps>();

            if !caps.is_empty() {
                let stream_id = self.get_stream_id(None, Some(i as u32)).unwrap();
                if element
                    .imp()
                    .create_and_probe_src_pad(&caps, &stream_id, self)
                {
                    if desc_type == gst_webrtc::WebRTCSDPType::Offer {
                        gst::info!(
                        CAT,
                        obj = element,
                        "Getting transceiver for {stream_id} and index {i} with caps: {caps:#?}"
                        );

                        let mut transceiver = None;
                        let mut idx = 0i32;
                        // find the transceiver with this mline
                        loop {
                            let Some(to_check) = webrtcbin
                                .emit_by_name::<Option<gst_webrtc::WebRTCRTPTransceiver>>(
                                    "get-transceiver",
                                    &[&idx],
                                )
                            else {
                                break;
                            };
                            let mline = to_check.property::<u32>("mlineindex");
                            if mline as usize == i {
                                transceiver = Some(to_check);
                                break;
                            }
                            idx += 1;
                        }
                        let transceiver = transceiver.unwrap_or_else(|| {
                        gst::warning!(CAT, "Transceiver for idx {i} does not exist, GStreamer <= 1.24, adding it ourself");
                        webrtcbin.emit_by_name::<gst_webrtc::WebRTCRTPTransceiver>(
                            "add-transceiver",
                            &[&gst_webrtc::WebRTCRTPTransceiverDirection::Recvonly, &caps])
                        });

                        transceiver.set_property("do_nack", do_retransmission);
                        transceiver.set_property("fec-type", gst_webrtc::WebRTCFECType::UlpRed);
                        transceiver.set_property("codec-preferences", caps);
                    } else {
                        // SDP type is answer,
                        // so the transceiver must have already been created while sending offer
                    }
                } else {
                    gst::error!(
                        CAT,
                        obj = element,
                        "Failed to create src pad with caps {:?}",
                        caps
                    );
                }
            } else {
                gst::info!(
                    CAT,
                    obj = element,
                    "Not using media: {media:#?} as it doesn't match our codec restrictions"
                );
            }
        }

        let promise = gst::Promise::with_change_func(glib::clone!(
            #[weak]
            element,
            #[strong(rename_to = session_id)]
            self.id,
            move |reply| {
                if desc_type == gst_webrtc::WebRTCSDPType::Offer {
                    let state = element.imp().state.lock().unwrap();
                    gst::info!(CAT, obj = element, "got answer for session {session_id:?}");
                    let Some(session) = state.sessions.get(&session_id) else {
                        gst::error!(CAT, obj = element, "no session {session_id:?}");
                        return;
                    };
                    session.0.lock().unwrap().on_answer_created(reply, &element);
                } else {
                    gst::log!(
                        CAT,
                        obj = element,
                        "Nothing to do in the promise in case of an answer"
                    );
                    return;
                }
            }
        ));

        (promise, webrtcbin.clone())
    }

    fn handle_remote_description(
        &mut self,
        desc: &gst_webrtc::WebRTCSessionDescription,
        element: &super::BaseWebRTCSrc,
    ) -> (gst::Promise, gst::Bin) {
        gst::debug!(
            CAT,
            obj = element,
            "Got remote description: {}",
            desc.sdp().to_string()
        );

        let promise = gst::Promise::with_change_func(glib::clone!(
            #[weak]
            element,
            #[strong]
            desc,
            #[strong(rename_to = session_id)]
            self.id,
            move |_| {
                let mut state = element.imp().state.lock().unwrap();
                gst::info!(
                    CAT,
                    obj = element,
                    "got {:?} for session {session_id:?}",
                    desc.type_()
                );
                let Some(session) = state.sessions.get_mut(&session_id) else {
                    gst::error!(CAT, obj = element, "no session {session_id:?}");
                    return;
                };

                let (promise, webrtcbin) = session
                    .0
                    .lock()
                    .unwrap()
                    .remote_description_set(&element, &desc);

                drop(state);

                if desc.type_() == gst_webrtc::WebRTCSDPType::Offer {
                    webrtcbin
                        .emit_by_name::<()>("create-answer", &[&None::<gst::Structure>, &promise]);
                } else {
                    // Nothing to do with the promise in case of an answer
                    promise.reply(None);
                }
            }
        ));

        // We cannot emit `set-remote-description` from here. The promise
        // function needs the state lock which is held by the caller
        // of `handle_remote_description`. So return the promise to the caller so that
        // it can drop the `state` and safely emit `set-remote-description`

        (promise, self.webrtcbin().clone())
    }

    fn on_answer_created(
        &self,
        reply: Result<Option<&gst::StructureRef>, gst::PromiseError>,
        element: &super::BaseWebRTCSrc,
    ) {
        let reply = match reply {
            Ok(Some(reply)) => {
                if !reply.has_field_with_type(
                    "answer",
                    gst_webrtc::WebRTCSessionDescription::static_type(),
                ) {
                    gst::element_error!(
                        element,
                        gst::StreamError::Failed,
                        ["create-answer::Promise returned with no reply"]
                    );
                    return;
                } else if reply.has_field_with_type("error", glib::Error::static_type()) {
                    gst::element_error!(
                        element,
                        gst::LibraryError::Failed,
                        ["create-offer::Promise returned with error: {:?}", reply]
                    );
                    return;
                }

                reply
            }
            Ok(None) => {
                gst::element_error!(
                    element,
                    gst::StreamError::Failed,
                    ["create-answer::Promise returned with no reply"]
                );

                return;
            }
            Err(err) => {
                gst::element_error!(
                    element,
                    gst::LibraryError::Failed,
                    ["create-answer::Promise returned with error {:?}", err]
                );

                return;
            }
        };

        let answer = reply
            .value("answer")
            .unwrap()
            .get::<gst_webrtc::WebRTCSessionDescription>()
            .expect("Invalid argument");

        self.webrtcbin
            .emit_by_name::<()>("set-local-description", &[&answer, &None::<gst::Promise>]);

        gst::log!(
            CAT,
            obj = element,
            "Sending SDP, {}",
            answer.sdp().to_string()
        );
        let signaller = element.imp().signaller();
        signaller.send_sdp(&self.id, &answer);
    }

    fn on_data_channel(&mut self, data_channel: glib::Object, element: &super::BaseWebRTCSrc) {
        gst::info!(CAT, obj = element, "Received data channel {data_channel:?}");
        self.data_channel = data_channel.dynamic_cast::<WebRTCDataChannel>().ok();
    }

    fn on_ice_candidate(
        &self,
        sdp_m_line_index: u32,
        candidate: String,
        element: &super::BaseWebRTCSrc,
    ) {
        let signaller = element.imp().signaller();
        signaller.add_ice(&self.id, &candidate, sdp_m_line_index, None::<String>);
    }

    /// Called by the signaller with an ice candidate
    fn handle_ice(
        &self,
        sdp_m_line_index: Option<u32>,
        _sdp_mid: Option<String>,
        candidate: &str,
        element: &super::BaseWebRTCSrc,
    ) {
        let sdp_m_line_index = match sdp_m_line_index {
            Some(m_line) => m_line,
            None => {
                gst::error!(CAT, obj = element, "No mandatory mline");
                return;
            }
        };
        gst::log!(
            CAT,
            obj = element,
            "Got ice candidate for {}: {candidate}",
            self.id
        );

        self.webrtcbin()
            .emit_by_name::<()>("add-ice-candidate", &[&sdp_m_line_index, &candidate]);
    }
}

impl BaseWebRTCSrc {
    fn signaller(&self) -> Signallable {
        self.settings.lock().unwrap().signaller.clone()
    }

    fn unprepare(&self) -> Result<(), Error> {
        gst::info!(CAT, imp = self, "unpreparing");

        let mut state = self.state.lock().unwrap();
        let session_ids: Vec<_> = state.sessions.keys().map(|k| k.to_owned()).collect();
        let sessions: Vec<_> = session_ids
            .iter()
            .filter_map(|id| state.end_session(&self.obj(), id))
            .collect();
        drop(state);

        gst::debug!(CAT, imp = self, "Ending sessions");
        for session in sessions {
            self.signaller().end_session(&session.0.lock().unwrap().id);
        }
        gst::debug!(CAT, imp = self, "All sessions have started finalizing");

        self.maybe_stop_signaller();

        let finalizing_sessions = self.state.lock().unwrap().finalizing_sessions.clone();

        let (sessions, cvar) = &*finalizing_sessions;
        let mut sessions = sessions.lock().unwrap();
        while !sessions.is_empty() {
            sessions = cvar.wait(sessions).unwrap();
        }

        gst::debug!(CAT, imp = self, "All sessions are done finalizing");

        Ok(())
    }

    fn connect_signaller(&self, signaller: &Signallable) {
        let instance = &*self.obj();

        let _ = self
            .state
            .lock()
            .unwrap()
            .signaller_signals
            .insert(SignallerSignals {
                error: signaller.connect_closure(
                    "error",
                    false,
                    glib::closure!(
                        #[watch]
                        instance,
                        move |_signaller: glib::Object, error: String| {
                            gst::element_error!(
                                instance,
                                gst::StreamError::Failed,
                                ["Signalling error: {}", error]
                            );
                        }
                    ),
                ),

                session_started: signaller.connect_closure(
                    "session-started",
                    false,
                    glib::closure!(
                        #[watch]
                        instance,
                        move |_signaller: glib::Object, session_id: &str, _peer_id: &str| {
                            let imp = instance.imp();
                            gst::info!(CAT, imp = imp, "Session started: {session_id}");
                            let _ = imp.start_session(session_id);
                        }
                    ),
                ),

                session_ended: signaller.connect_closure(
                    "session-ended",
                    false,
                    glib::closure!(
                        #[watch]
                        instance,
                        move |_signaller: glib::Object, session_id: &str| {
                            let this = instance.imp();

                            if let Err(e) = this.remove_session(session_id) {
                                gst::error!(
                                    CAT,
                                    imp = this,
                                    " Failed to end session {session_id}: {e}"
                                );
                                return false;
                            }

                            gst::log!(CAT, imp = this, "Session {session_id} cleaned up");

                            true
                        }
                    ),
                ),

                session_requested: signaller.connect_closure(
                    "session-requested",
                    false,
                    glib::closure!(#[watch] instance, move |_signaler: glib::Object, session_id: &str, _peer_id: &str, offer: Option<&gst_webrtc::WebRTCSessionDescription>|{
                        if offer.is_none() {
                            let this = instance.imp();
                            let state = this.state.lock().unwrap();
                            let Some(session) = state.sessions.get(session_id) else {
                                gst::error!(CAT, imp = this, "session {session_id:?} not found");
                                return
                            };
                            session.0.lock().unwrap().generate_offer(&this.obj());
                        }
                    }),
                ),

                request_meta: signaller.connect_closure(
                    "request-meta",
                    false,
                    glib::closure!(
                        #[watch]
                        instance,
                        move |_signaller: glib::Object| -> Option<gst::Structure> {
                            instance.imp().settings.lock().unwrap().meta.clone()
                        }
                    ),
                ),

                session_description: signaller.connect_closure(
                    "session-description",
                    false,
                    glib::closure!(
                        #[watch]
                        instance,
                        move |_signaller: glib::Object,
                              session_id: &str,
                              desc: &gst_webrtc::WebRTCSessionDescription| {
                        match desc.type_() {
                            gst_webrtc::WebRTCSDPType::Offer | gst_webrtc::WebRTCSDPType::Answer => {
                                let this = instance.imp();
                                gst::info!(CAT, imp = this, "got sdp : {:?}", desc.type_());
                                let mut state = this.state.lock().unwrap();
                                let Some(session) = state.sessions.get_mut(session_id) else {
                                    gst::error!(CAT, imp = this, "session {session_id:?} not found");
                                    return;
                                };

                                let (promise, webrtcbin) = session.0.lock().unwrap().handle_remote_description(desc, &this.obj());
                                drop(state);
                                webrtcbin
                                    .emit_by_name::<()>("set-remote-description", &[&desc, &promise]);
                            },
                            _ => {
                                unimplemented!("{:?} type remote description not handled", desc.type_());
                            },
                        }
                    }
                    ),
                ),

                // sdp_mid is exposed for future proofing, see
                // https://gitlab.freedesktop.org/gstreamer/gst-plugins-bad/-/issues/1174,
                // at the moment sdp_m_line_index must be Some
                handle_ice: signaller.connect_closure(
                    "handle-ice",
                    false,
                    glib::closure!(
                        #[watch]
                        instance,
                        move |_signaller: glib::Object,
                              session_id: &str,
                              sdp_m_line_index: u32,
                              _sdp_mid: Option<String>,
                              candidate: &str| {
                            let this = instance.imp();
                            let state = this.state.lock().unwrap();
                            let Some(session) = state.sessions.get(session_id) else {
                                gst::error!(CAT, imp = this, "session {session_id:?} not found");
                                return;
                            };
                            session.0.lock().unwrap().handle_ice(
                                Some(sdp_m_line_index),
                                None,
                                candidate,
                                &this.obj(),
                            );
                        }
                    ),
                ),
            });

        // previous signals are disconnected when dropping the old structure
    }

    // Creates and adds our `WebRTCSrcPad` source pad, returning caps accepted
    // downstream
    fn create_and_probe_src_pad(
        &self,
        caps: &gst::Caps,
        stream_id: &str,
        session: &mut SessionInner,
    ) -> bool {
        gst::log!(CAT, "Creating pad for {caps:?}, stream: {stream_id}");

        let obj = self.obj();
        let media_type = caps
            .structure(0)
            .expect("Passing empty caps is invalid")
            .get::<&str>("media")
            .expect("Only caps with a `media` field are expected when creating the pad");

        let (template, name) = if media_type == "video" {
            (
                obj.pad_template("video_%s_%u").unwrap(),
                format!(
                    "video_{}_{}",
                    session.id,
                    session.n_video_pads.fetch_add(1, Ordering::SeqCst)
                ),
            )
        } else if media_type == "audio" {
            (
                obj.pad_template("audio_%s_%u").unwrap(),
                format!(
                    "audio_{}_{}",
                    session.id,
                    session.n_audio_pads.fetch_add(1, Ordering::SeqCst)
                ),
            )
        } else {
            gst::info!(
                CAT,
                imp = self,
                "Not an audio or video media {media_type:?}"
            );

            return false;
        };

        let ghost = gst::GhostPad::builder_from_template(&template)
            .name(name)
            .build()
            .downcast::<WebRTCSrcPad>()
            .unwrap();
        ghost.imp().set_stream_id(stream_id);
        session
            .pending_srcpads
            .insert(stream_id.to_string(), (ghost.clone(), caps.clone()));

        true
    }

    fn maybe_start_signaller(&self) {
        let obj = self.obj();
        let mut state = self.state.lock().unwrap();
        if state.signaller_state == SignallerState::Stopped
            && obj.current_state() >= gst::State::Paused
        {
            self.signaller().start();

            gst::info!(CAT, imp = self, "Started signaller");
            state.signaller_state = SignallerState::Started;
        }
    }

    fn maybe_stop_signaller(&self) {
        let mut state = self.state.lock().unwrap();
        if state.signaller_state == SignallerState::Started {
            state.signaller_state = SignallerState::Stopped;
            drop(state);
            self.signaller().stop();
            gst::info!(CAT, imp = self, "Stopped signaller");
        }
    }

    pub fn set_signaller(&self, signaller: Signallable) -> Result<(), Error> {
        let sigobj = signaller.clone();
        let mut settings = self.settings.lock().unwrap();

        self.connect_signaller(&sigobj);
        settings.signaller = signaller;

        Ok(())
    }

    fn start_session(&self, session_id: &str) -> Result<(), Error> {
        let state = self.state.lock().unwrap();
        if state.sessions.contains_key(session_id) {
            return Err(anyhow::anyhow!(
                "session with id {session_id} already exists"
            ));
        };
        drop(state);

        let session = SessionInner::new(session_id)?;

        let webrtcbin = session.webrtcbin();

        {
            let settings = self.settings.lock().unwrap();

            if let Some(stun_server) = settings.stun_server.as_ref() {
                webrtcbin.set_property("stun-server", stun_server);
            }

            for turn_server in settings.turn_servers.iter() {
                webrtcbin.emit_by_name::<bool>("add-turn-server", &[&turn_server]);
            }
        }

        let bin = gst::Bin::with_name(session_id);

        bin.connect_pad_removed(glib::clone!(
            #[weak(rename_to = this)]
            self,
            #[to_owned]
            session_id,
            move |_, pad| {
                let mut state = this.state.lock().unwrap();
                if let Some(session) = state.sessions.get_mut(&session_id) {
                    let session = session.0.lock().unwrap();
                    session.flow_combiner.lock().unwrap().remove_pad(pad);
                };
            }
        ));
        bin.connect_pad_added(glib::clone!(
            #[weak(rename_to = this)]
            self,
            #[to_owned]
            session_id,
            move |_, pad| {
                let mut state = this.state.lock().unwrap();
                if let Some(session) = state.sessions.get_mut(&session_id) {
                    let session = session.0.lock().unwrap();
                    session.flow_combiner.lock().unwrap().add_pad(pad);
                };
            }
        ));

        webrtcbin.connect_pad_added(glib::clone!(
            #[weak(rename_to = this)]
            self,
            #[weak]
            bin,
            #[to_owned]
            session_id,
            move |_webrtcbin, pad| {
                if pad.direction() == gst::PadDirection::Sink {
                    return;
                }
                let mut state = this.state.lock().unwrap();
                let Some(session) = state.sessions.get_mut(&session_id) else {
                    gst::error!(CAT, imp = this, "session {session_id:?} not found");
                    return;
                };
                let bin_ghostpad =
                    session
                        .0
                        .lock()
                        .unwrap()
                        .handle_webrtc_src_pad(&bin, pad, &this.obj());
                drop(state);
                bin.add_pad(&bin_ghostpad)
                    .expect("Adding ghostpad to the bin should always work");
            }
        ));

        webrtcbin.connect_pad_removed(glib::clone!(
            #[weak(rename_to = this)]
            self,
            #[to_owned]
            session_id,
            move |_webrtcbin, pad| {
                let mut state = this.state.lock().unwrap();
                if let Some(session) = state.sessions.get_mut(&session_id) {
                    let session = session.0.lock().unwrap();
                    session.flow_combiner.lock().unwrap().remove_pad(pad);
                };
            }
        ));

        webrtcbin.connect_closure(
            "on-ice-candidate",
            false,
            glib::closure!(
                #[weak(rename_to = this)]
                self,
                #[to_owned]
                session_id,
                move |_webrtcbin: gst::Bin, sdp_m_line_index: u32, candidate: String| {
                    let mut state = this.state.lock().unwrap();
                    let Some(session) = state.sessions.get_mut(&session_id) else {
                        gst::error!(CAT, imp = this, "session {session_id:?} not found");
                        return;
                    };
                    let session = session.0.lock().unwrap();
                    session.on_ice_candidate(sdp_m_line_index, candidate, &this.obj());
                }
            ),
        );

        webrtcbin.connect_closure(
            "on-data-channel",
            false,
            glib::closure!(
                #[weak(rename_to = this)]
                self,
                #[to_owned]
                session_id,
                move |_webrtcbin: gst::Bin, data_channel: glib::Object| {
                    let mut state = this.state.lock().unwrap();

                    let Some(session) = state.sessions.get_mut(&session_id) else {
                        gst::error!(CAT, imp = this, "session {session_id:?} not found");
                        return;
                    };
                    let mut session = session.0.lock().unwrap();
                    session.on_data_channel(data_channel, &this.obj());
                }
            ),
        );

        bin.add(&webrtcbin).unwrap();
        self.obj().add(&bin).context("Could not add `webrtcbin`")?;
        bin.sync_state_with_parent().unwrap();

        self.signaller()
            .emit_by_name::<()>("webrtcbin-ready", &[&session_id, &webrtcbin]);

        let mut state = self.state.lock().unwrap();
        state.sessions.insert(
            session_id.to_string(),
            Session(Arc::new(Mutex::new(session))),
        );

        Ok(())
    }

    fn remove_session(&self, session_id: &str) -> Result<(), Error> {
        let mut state = self.state.lock().unwrap();
        if !state.sessions.contains_key(session_id) {
            return Err(anyhow::anyhow!("No session with id: {session_id}"));
        }

        // We need to do a flush start, without which the pad chain
        // function trying to take the state lock causes a deadlock
        // with the state lock already acquired here.
        self.flush_src_pad(session_id)?;

        if let Some(session) = state.end_session(&self.obj(), session_id) {
            drop(state);
            let session = session.0.lock().unwrap();
            self.signaller().end_session(&session.id);
        }

        self.remove_src_pad(session_id)?;

        Ok(())
    }

    fn flush_src_pad(&self, session_id: &str) -> Result<(), Error> {
        let obj = self.obj();

        for pad in obj
            .src_pads()
            .iter()
            .filter(|pad| pad.name().contains(session_id))
        {
            if !pad.push_event(gst::event::FlushStart::new()) {
                return Err(anyhow::anyhow!(
                    "Failed to send flush-start for pad: {}",
                    pad.name()
                ));
            }
        }

        Ok(())
    }

    fn remove_src_pad(&self, session_id: &str) -> Result<(), Error> {
        let obj = self.obj();

        for pad in obj
            .src_pads()
            .iter()
            .filter(|pad| pad.name().contains(session_id))
        {
            obj.remove_pad(pad)
                .map_err(|err| anyhow::anyhow!("Couldn't remove pad? {err:?}"))?;

            gst::log!(CAT, imp = self, "Removed pad {}", pad.name());
        }

        Ok(())
    }
}

impl ElementImpl for BaseWebRTCSrc {
    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            // Ignore specific raw caps from Codecs: they are covered by VIDEO_CAPS & AUDIO_CAPS

            let mut video_caps_builder = gst::Caps::builder_full()
                .structure_with_any_features(VIDEO_CAPS.structure(0).unwrap().to_owned())
                .structure(RTP_CAPS.structure(0).unwrap().to_owned());

            for codec in Codecs::video_codecs().filter(|codec| !codec.is_raw) {
                video_caps_builder =
                    video_caps_builder.structure(codec.caps.structure(0).unwrap().to_owned());
            }

            let mut audio_caps_builder = gst::Caps::builder_full()
                .structure_with_any_features(AUDIO_CAPS.structure(0).unwrap().to_owned())
                .structure(RTP_CAPS.structure(0).unwrap().to_owned());

            for codec in Codecs::audio_codecs().filter(|codec| !codec.is_raw) {
                audio_caps_builder =
                    audio_caps_builder.structure(codec.caps.structure(0).unwrap().to_owned());
            }

            vec![
                gst::PadTemplate::with_gtype(
                    "video_%s_%u",
                    gst::PadDirection::Src,
                    gst::PadPresence::Sometimes,
                    &video_caps_builder.build(),
                    WebRTCSrcPad::static_type(),
                )
                .unwrap(),
                gst::PadTemplate::with_gtype(
                    "audio_%s_%u",
                    gst::PadDirection::Src,
                    gst::PadPresence::Sometimes,
                    &audio_caps_builder.build(),
                    WebRTCSrcPad::static_type(),
                )
                .unwrap(),
            ]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        let obj = &*self.obj();

        let mut ret = self.parent_change_state(transition);

        match transition {
            gst::StateChange::PausedToReady => {
                if let Err(err) = self.unprepare() {
                    gst::element_error!(
                        obj,
                        gst::StreamError::Failed,
                        ["Failed to unprepare: {}", err]
                    );
                    return Err(gst::StateChangeError);
                }
            }
            gst::StateChange::ReadyToPaused => {
                ret = Ok(gst::StateChangeSuccess::NoPreroll);
            }
            gst::StateChange::PlayingToPaused => {
                ret = Ok(gst::StateChangeSuccess::NoPreroll);
            }
            gst::StateChange::PausedToPlaying => {
                self.maybe_start_signaller();
            }
            _ => (),
        }

        ret
    }

    fn send_event(&self, event: gst::Event) -> bool {
        match event.view() {
            gst::EventView::Navigation(ev) => {
                let settings = self.settings.lock().unwrap();
                let mut state = self.state.lock().unwrap();

                // Return without potentially warning
                if !settings.enable_data_channel_navigation && !settings.enable_control_data_channel
                {
                    return true;
                }

                if state.sessions.len() != 1 {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Navigation event can only be sent on the element if there is a single \
                        session. For multiple sessions, send the event on the desired source \
                        pad(s)"
                    );
                    false
                } else {
                    if settings.enable_data_channel_navigation {
                        state
                            .sessions
                            .values_mut()
                            .next()
                            .unwrap()
                            .0
                            .lock()
                            .unwrap()
                            .send_navigation_event(
                                gst_video::NavigationEvent::parse(ev).unwrap(),
                                &self.obj(),
                            );
                    }

                    if settings.enable_control_data_channel {
                        let request = utils::ControlRequest::NavigationEvent {
                            event: gst_video::NavigationEvent::parse(ev).unwrap(),
                        };
                        state
                            .sessions
                            .values_mut()
                            .next()
                            .unwrap()
                            .0
                            .lock()
                            .unwrap()
                            .send_control_request(request, &self.obj(), None);
                    }
                    true
                }
            }
            _ => true,
        }
    }
}

impl GstObjectImpl for BaseWebRTCSrc {}

impl BinImpl for BaseWebRTCSrc {}

impl ChildProxyImpl for BaseWebRTCSrc {
    fn child_by_index(&self, index: u32) -> Option<glib::Object> {
        if index == 0 {
            Some(self.signaller().upcast())
        } else {
            None
        }
    }

    fn children_count(&self) -> u32 {
        1
    }

    fn child_by_name(&self, name: &str) -> Option<glib::Object> {
        match name {
            "signaller" => {
                gst::info!(CAT, imp = self, "Getting signaller");
                Some(self.signaller().upcast())
            }
            _ => None,
        }
    }
}

#[derive(PartialEq)]
enum SignallerState {
    Started,
    Stopped,
}

struct SessionInner {
    id: String,
    webrtcbin: gst::Element,
    data_channel: Option<WebRTCDataChannel>,
    n_video_pads: AtomicU16,
    n_audio_pads: AtomicU16,
    flow_combiner: Mutex<gst_base::UniqueFlowCombiner>,
    request_counter: u64,
    pending_srcpads: HashMap<String, (WebRTCSrcPad, gst::Caps)>,
}

#[derive(Clone)]
struct Session(Arc<Mutex<SessionInner>>);

struct State {
    sessions: HashMap<String, Session>,
    signaller_state: SignallerState,
    signaller_signals: Option<SignallerSignals>,
    finalizing_sessions: Arc<(Mutex<HashMap<String, Session>>, Condvar)>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            signaller_state: SignallerState::Stopped,
            sessions: HashMap::new(),
            signaller_signals: Default::default(),
            finalizing_sessions: Arc::new((Mutex::new(HashMap::new()), Condvar::new())),
        }
    }
}

impl State {
    fn finalize_session(&mut self, element: &super::BaseWebRTCSrc, session: Session) {
        let inner = session.0.lock().unwrap();
        let session_id = inner.id.clone();
        drop(inner);

        gst::info!(CAT, "Ending session {}", session_id);

        let finalizing_sessions = self.finalizing_sessions.clone();
        let (sessions, _cvar) = &*finalizing_sessions;
        sessions.lock().unwrap().insert(session_id.clone(), session);

        let element = element.clone();
        RUNTIME.spawn_blocking(move || {
            let (sessions, cvar) = &*finalizing_sessions;
            let mut sessions = sessions.lock().unwrap();
            let session = sessions.remove(&session_id).unwrap();
            let session = session.0.lock().unwrap();

            let bin = session
                .webrtcbin()
                .parent()
                .and_downcast::<gst::Bin>()
                .unwrap();

            let _ = bin.set_state(gst::State::Null);

            if let Some(webrtcsrc) = bin.parent() {
                if let Err(e) = webrtcsrc.downcast_ref::<gst::Bin>().unwrap().remove(&bin) {
                    gst::error!(
                        CAT,
                        obj = bin,
                        "Failed to remove bin for session {} :{e}",
                        session.id
                    );
                }
            }

            cvar.notify_one();

            gst::debug!(CAT, obj = element, "Session {session_id} ended");
        });
    }

    fn end_session(&mut self, element: &super::BaseWebRTCSrc, session_id: &str) -> Option<Session> {
        if let Some(session) = self.sessions.remove(session_id) {
            self.finalize_session(element, session.clone());
            return Some(session);
        }

        None
    }
}

#[derive(Default)]
pub struct WebRTCSrc {}

impl ObjectImpl for WebRTCSrc {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![glib::ParamSpecBoolean::builder("connect-to-first-producer")
                .nick("Connect to first peer")
                .blurb(
                    "When enabled, automatically connect to the first peer that becomes available \
                     if no 'peer-id' is specified.",
                )
                .default_value(false)
                .mutable_ready()
                .build()]
        });
        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let obj = self.obj();
        let base = obj.upcast_ref::<super::BaseWebRTCSrc>().imp();
        match pspec.name() {
            "connect-to-first-producer" => base
                .signaller()
                .downcast::<Signaller>()
                .unwrap()
                .imp()
                .set_connect_to_first_producer(value.get().unwrap()),
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let obj = self.obj();
        let base = obj.upcast_ref::<super::BaseWebRTCSrc>().imp();
        match pspec.name() {
            "connect-to-first-producer" => base
                .signaller()
                .downcast::<Signaller>()
                .unwrap()
                .imp()
                .connect_to_first_producer()
                .into(),
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for WebRTCSrc {}

impl BinImpl for WebRTCSrc {}

impl ElementImpl for WebRTCSrc {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "WebRTCSrc",
                "Source/Network/WebRTC",
                "WebRTC src",
                "Thibault Saunier <tsaunier@igalia.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }
}

impl BaseWebRTCSrcImpl for WebRTCSrc {}

impl URIHandlerImpl for WebRTCSrc {
    const URI_TYPE: gst::URIType = gst::URIType::Src;

    fn protocols() -> &'static [&'static str] {
        &["gstwebrtc", "gstwebrtcs"]
    }

    fn uri(&self) -> Option<String> {
        let obj = self.obj();
        let base = obj.upcast_ref::<super::BaseWebRTCSrc>().imp();
        base.signaller().property::<Option<String>>("uri")
    }

    fn set_uri(&self, uri: &str) -> Result<(), glib::Error> {
        let uri = Url::from_str(uri)
            .map_err(|err| glib::Error::new(gst::URIError::BadUri, &format!("{err:?}")))?;

        let socket_scheme = match uri.scheme() {
            "gstwebrtc" => Ok("ws"),
            "gstwebrtcs" => Ok("wss"),
            _ => Err(glib::Error::new(
                gst::URIError::BadUri,
                &format!("Invalid protocol: {}", uri.scheme()),
            )),
        }?;

        let mut url_str = uri.to_string();

        // Not using `set_scheme()` because it doesn't work with `http`
        // See https://github.com/servo/rust-url/pull/768 for a PR implementing that
        url_str.replace_range(0..uri.scheme().len(), socket_scheme);

        let obj = self.obj();
        let base = obj.upcast_ref::<super::BaseWebRTCSrc>().imp();
        base.signaller().set_property("uri", &url_str);

        Ok(())
    }
}

#[glib::object_subclass]
impl ObjectSubclass for WebRTCSrc {
    const NAME: &'static str = "GstWebRTCSrc";
    type Type = super::WebRTCSrc;
    type ParentType = super::BaseWebRTCSrc;
    type Interfaces = (gst::URIHandler,);
}

#[cfg(feature = "whip")]
pub(super) mod whip {
    use super::*;
    use crate::whip_signaller::WhipServerSignaller;

    #[derive(Default)]
    pub struct WhipServerSrc {}

    impl ObjectImpl for WhipServerSrc {
        fn constructed(&self) {
            self.parent_constructed();
            let element = self.obj();
            let ws = element
                .upcast_ref::<crate::webrtcsrc::BaseWebRTCSrc>()
                .imp();

            let _ = ws.set_signaller(WhipServerSignaller::default().upcast());

            let settings = ws.settings.lock().unwrap();
            element
                .bind_property("stun-server", &settings.signaller, "stun-server")
                .build();
            element
                .bind_property("turn-servers", &settings.signaller, "turn-servers")
                .build();
        }
    }

    impl GstObjectImpl for WhipServerSrc {}

    impl BinImpl for WhipServerSrc {}

    impl ElementImpl for WhipServerSrc {
        fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
            static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> =
                LazyLock::new(|| {
                    gst::subclass::ElementMetadata::new(
                        "WhipServerSrc",
                        "Source/Network/WebRTC",
                        "WebRTC source element using WHIP Server as the signaller",
                        "Taruntej Kanakamalla <taruntej@asymptotic.io>",
                    )
                });

            Some(&*ELEMENT_METADATA)
        }
    }

    impl BaseWebRTCSrcImpl for WhipServerSrc {}

    #[glib::object_subclass]
    impl ObjectSubclass for WhipServerSrc {
        const NAME: &'static str = "GstWhipServerSrc";
        type Type = crate::webrtcsrc::WhipServerSrc;
        type ParentType = crate::webrtcsrc::BaseWebRTCSrc;
    }
}

#[cfg(feature = "livekit")]
pub(super) mod livekit {
    use super::*;
    use crate::{livekit_signaller::LiveKitSignaller, webrtcsrc::pad::WebRTCSrcPadImpl};

    #[derive(Default)]
    pub struct LiveKitWebRTCSrc;

    impl ObjectImpl for LiveKitWebRTCSrc {
        fn constructed(&self) {
            self.parent_constructed();
            let element = self.obj();
            let ws = element
                .upcast_ref::<crate::webrtcsrc::BaseWebRTCSrc>()
                .imp();

            let _ = ws.set_signaller(LiveKitSignaller::new_consumer().upcast());
        }
    }

    impl GstObjectImpl for LiveKitWebRTCSrc {}

    impl BinImpl for LiveKitWebRTCSrc {}

    impl ElementImpl for LiveKitWebRTCSrc {
        fn pad_templates() -> &'static [gst::PadTemplate] {
            static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
                super::BaseWebRTCSrc::pad_templates()
                    .iter()
                    .map(|pad_templ| {
                        gst::PadTemplate::with_gtype(
                            pad_templ.name_template(),
                            pad_templ.direction(),
                            pad_templ.presence(),
                            pad_templ.caps(),
                            super::super::LiveKitWebRTCSrcPad::static_type(),
                        )
                        .unwrap()
                    })
                    .collect()
            });

            PAD_TEMPLATES.as_ref()
        }

        fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
            static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> =
                LazyLock::new(|| {
                    gst::subclass::ElementMetadata::new(
                        "LiveKitWebRTCSrc",
                        "Source/Network/WebRTC",
                        "WebRTC source with LiveKit signaller",
                        "Jordan Yelloz <jordan.yelloz@collabora.com>",
                    )
                });

            Some(&*ELEMENT_METADATA)
        }
    }

    impl BaseWebRTCSrcImpl for LiveKitWebRTCSrc {}

    #[glib::object_subclass]
    impl ObjectSubclass for LiveKitWebRTCSrc {
        const NAME: &'static str = "GstLiveKitWebRTCSrc";
        type Type = crate::webrtcsrc::LiveKitWebRTCSrc;
        type ParentType = crate::webrtcsrc::BaseWebRTCSrc;
    }

    #[derive(Default)]
    pub struct LiveKitWebRTCSrcPad {}

    fn participant_permission_to_structure(
        participant_permission: &livekit_protocol::ParticipantPermission,
    ) -> gst::Structure {
        gst::Structure::builder("livekit/participant-permission")
            .field("can-subscribe", participant_permission.can_subscribe)
            .field("can-publish", participant_permission.can_publish)
            .field("can-publish-data", participant_permission.can_publish_data)
            .field_from_iter::<gst::Array, _>(
                "can-publish-sources",
                participant_permission
                    .can_publish_sources
                    .iter()
                    .map(|v| v.to_send_value()),
            )
            .field("hidden", participant_permission.hidden)
            .field("recorder", participant_permission.recorder)
            .field(
                "can-update-metadata",
                participant_permission.can_update_metadata,
            )
            .field("agent", participant_permission.agent)
            .build()
    }

    fn track_info_to_structure(track_info: &livekit_protocol::TrackInfo) -> gst::Structure {
        gst::Structure::builder("livekit/track-info")
            .field("sid", &track_info.sid)
            .field(
                "type",
                livekit_protocol::TrackType::try_from(track_info.r#type)
                    .map(|t| t.as_str_name())
                    .unwrap_or("unknown"),
            )
            .field("name", &track_info.name)
            .field("muted", track_info.muted)
            .field("width", track_info.width)
            .field("height", track_info.height)
            .field("simulcast", track_info.simulcast)
            .field("disable-dtx", track_info.disable_dtx)
            .field(
                "source",
                livekit_protocol::TrackSource::try_from(track_info.source)
                    .map(|s| s.as_str_name())
                    .unwrap_or("unknown"),
            )
            //.field_from_iter::<gst::Array>("layers", track_info.layers.iter().todo!())
            .field("mime-type", &track_info.mime_type)
            .field("mid", &track_info.mid)
            //.field_from_iter::<gst::Array>("codecs", track_info.codecs.iter().todo!())
            .field("stereo", track_info.stereo)
            .field("disable-red", track_info.disable_red)
            .field(
                "encryption",
                livekit_protocol::encryption::Type::try_from(track_info.encryption)
                    .map(|e| e.as_str_name())
                    .unwrap_or("unknown"),
            )
            .field("stream", &track_info.stream)
            //.field("version", todo!())
            .build()
    }

    fn participant_info_to_structure(
        participant_info: &livekit_protocol::ParticipantInfo,
    ) -> gst::Structure {
        gst::Structure::builder("livekit/participant-info")
            .field("sid", &participant_info.sid)
            .field("identity", &participant_info.identity)
            .field(
                "state",
                livekit_protocol::participant_info::State::try_from(participant_info.state)
                    .map(|s| s.as_str_name())
                    .unwrap_or("unknown"),
            )
            .field_from_iter::<gst::Array, _>(
                "tracks",
                participant_info.tracks.iter().map(track_info_to_structure),
            )
            .field("metadata", &participant_info.metadata)
            .field("joined-at", participant_info.joined_at)
            .field("name", &participant_info.name)
            .field("version", participant_info.version)
            .field_if_some(
                "permission",
                participant_info
                    .permission
                    .as_ref()
                    .map(participant_permission_to_structure),
            )
            .field("region", &participant_info.region)
            .field("is-publisher", participant_info.is_publisher)
            .field(
                "kind",
                livekit_protocol::participant_info::Kind::try_from(participant_info.kind)
                    .map(|k| k.as_str_name())
                    .unwrap_or("unknown"),
            )
            .build()
    }

    impl LiveKitWebRTCSrcPad {
        fn participant_track_sid(&self) -> Option<(String, String)> {
            // msid format is "participant_sid|track_sid"
            let msid = self.obj().property::<Option<String>>("msid")?;
            let (participant_sid, track_sid) = msid.split_once('|')?;
            Some((String::from(participant_sid), String::from(track_sid)))
        }

        fn participant_info(&self) -> Option<gst::Structure> {
            let participant_sid = self.participant_sid()?;
            let webrtcbin = self
                .obj()
                .parent()
                .and_downcast::<super::super::BaseWebRTCSrc>()?;
            let signaller = webrtcbin.property::<LiveKitSignaller>("signaller");
            let participant_info = signaller.imp().participant_info(&participant_sid)?;
            Some(participant_info_to_structure(&participant_info))
        }

        fn track_info(&self) -> Option<gst::Structure> {
            let (participant_sid, track_sid) = self.participant_track_sid()?;
            let webrtcbin = self
                .obj()
                .parent()
                .and_downcast::<super::super::BaseWebRTCSrc>()?;
            let signaller = webrtcbin.property::<LiveKitSignaller>("signaller");
            let participant_info = signaller.imp().participant_info(&participant_sid)?;
            participant_info
                .tracks
                .iter()
                .find(|t| t.sid == track_sid)
                .map(track_info_to_structure)
        }

        fn participant_sid(&self) -> Option<String> {
            self.participant_track_sid()
                .map(|(participant_sid, _track_sid)| participant_sid)
        }

        fn track_sid(&self) -> Option<String> {
            self.participant_track_sid()
                .map(|(_participant_sid, track_sid)| track_sid)
        }

        fn set_track_disabled(&self, disabled: bool) {
            let Some(track_sid) = self.track_sid() else {
                return;
            };
            let Some(webrtcbin) = self
                .obj()
                .parent()
                .and_downcast::<super::super::BaseWebRTCSrc>()
            else {
                return;
            };
            let signaller = webrtcbin.property::<LiveKitSignaller>("signaller");
            signaller.imp().set_track_disabled(&track_sid, disabled)
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LiveKitWebRTCSrcPad {
        const NAME: &'static str = "GstLiveKitWebRTCSrcPad";
        type Type = super::super::LiveKitWebRTCSrcPad;
        type ParentType = super::super::WebRTCSrcPad;
    }

    impl ObjectImpl for LiveKitWebRTCSrcPad {
        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: LazyLock<Vec<glib::subclass::Signal>> = LazyLock::new(|| {
                vec![glib::subclass::Signal::builder("set-track-disabled")
                    .param_types([bool::static_type()])
                    .action()
                    .class_handler(|values| {
                        let pad = values[0]
                            .get::<&super::super::LiveKitWebRTCSrcPad>()
                            .unwrap();
                        let disabled = values[1].get::<bool>().unwrap();

                        pad.imp().set_track_disabled(disabled);

                        None
                    })
                    .build()]
            });

            SIGNALS.as_ref()
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPS: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
                vec![
                    glib::ParamSpecBoxed::builder::<gst::Structure>("participant-info")
                        .flags(glib::ParamFlags::READABLE)
                        .blurb("Participant Information")
                        .build(),
                    glib::ParamSpecBoxed::builder::<gst::Structure>("track-info")
                        .flags(glib::ParamFlags::READABLE)
                        .blurb("Track Information")
                        .build(),
                    glib::ParamSpecString::builder("participant-sid")
                        .flags(glib::ParamFlags::READABLE)
                        .blurb("Participant ID")
                        .build(),
                    glib::ParamSpecString::builder("track-sid")
                        .flags(glib::ParamFlags::READABLE)
                        .blurb("Track ID")
                        .build(),
                ]
            });
            PROPS.as_ref()
        }
        fn property(&self, _sid: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "participant-info" => self.participant_info().into(),
                "track-info" => self.track_info().into(),
                "participant-sid" => self.participant_sid().into(),
                "track-sid" => self.track_sid().into(),
                name => panic!("no readable property {name:?}"),
            }
        }
    }
    impl GstObjectImpl for LiveKitWebRTCSrcPad {}
    impl PadImpl for LiveKitWebRTCSrcPad {}
    impl ProxyPadImpl for LiveKitWebRTCSrcPad {}
    impl GhostPadImpl for LiveKitWebRTCSrcPad {}
    impl WebRTCSrcPadImpl for LiveKitWebRTCSrcPad {}
}

#[cfg(feature = "janus")]
pub(super) mod janus {
    use super::*;
    use crate::{
        janusvr_signaller::{JanusVRSignallerStr, JanusVRSignallerU64},
        webrtcsink::JanusVRSignallerState,
        webrtcsrc::WebRTCSignallerRole,
    };

    static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
        gst::DebugCategory::new(
            "janusvrwebrtcsrc",
            gst::DebugColorFlags::empty(),
            Some("WebRTC Janus Video Room src"),
        )
    });

    #[derive(Debug, Clone, Default)]
    struct JanusSettings {
        use_string_ids: bool,
    }

    #[derive(Debug, Default)]
    struct JanusState {
        janus_state: JanusVRSignallerState,
    }

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = crate::webrtcsrc::JanusVRWebRTCSrc)]
    pub struct JanusVRWebRTCSrc {
        /**
         * GstJanusVRWebRTCSrc:use-string-ids:
         *
         * By default Janus uses `u64` ids to identify the room, the feed, etc.
         * But it can be changed to strings using the `strings_ids` option in `janus.plugin.videoroom.jcfg`.
         * In such case, `janusvrwebrtcsrc` has to be created using `use-string-ids=true` so its signaller
         * uses the right types for such ids and properties.
         *
         * Since: plugins-rs-0.14.0
         */
        #[property(name="use-string-ids", get, construct_only, type = bool, member = use_string_ids, blurb = "Use strings instead of u64 for Janus IDs, see strings_ids config option in janus.plugin.videoroom.jcfg")]
        settings: Mutex<JanusSettings>,
        /**
         * GstJanusVRWebRTCSrc:janus-state:
         *
         * The current state of the signaller.
         * Since: plugins-rs-0.14.0
         */
        #[property(
        name = "janus-state",
        get,
        member = janus_state,
        type = JanusVRSignallerState,
        blurb = "The current state of the signaller",
        builder(JanusVRSignallerState::Initialized)
    )]
        state: Mutex<JanusState>,
    }

    #[glib::derived_properties]
    impl ObjectImpl for JanusVRWebRTCSrc {
        fn constructed(&self) {
            self.parent_constructed();

            let settings = self.settings.lock().unwrap();
            let element = self.obj();
            let ws = element
                .upcast_ref::<crate::webrtcsrc::BaseWebRTCSrc>()
                .imp();

            let signaller: Signallable = if settings.use_string_ids {
                JanusVRSignallerStr::new(WebRTCSignallerRole::Consumer).upcast()
            } else {
                JanusVRSignallerU64::new(WebRTCSignallerRole::Consumer).upcast()
            };

            let self_weak = self.downgrade();
            signaller.connect("state-updated", false, move |args| {
                let self_ = self_weak.upgrade()?;
                let janus_state = args[1].get::<JanusVRSignallerState>().unwrap();

                {
                    let mut state = self_.state.lock().unwrap();
                    state.janus_state = janus_state;
                }

                gst::debug!(
                    CAT,
                    imp = self_,
                    "signaller state updated: {:?}",
                    janus_state
                );

                self_.obj().notify("janus-state");

                None
            });

            let _ = ws.set_signaller(signaller);
        }
    }

    impl GstObjectImpl for JanusVRWebRTCSrc {}

    impl ElementImpl for JanusVRWebRTCSrc {
        fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
            static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> =
                LazyLock::new(|| {
                    gst::subclass::ElementMetadata::new(
                        "JanusVRWebRTCSrc",
                        "Source/Network/WebRTC",
                        "WebRTC source with Janus Video Room signaller",
                        "Eva Pace <epace@igalia.com>",
                    )
                });

            Some(&*ELEMENT_METADATA)
        }
    }

    impl BinImpl for JanusVRWebRTCSrc {}

    impl BaseWebRTCSrcImpl for JanusVRWebRTCSrc {}

    #[glib::object_subclass]
    impl ObjectSubclass for JanusVRWebRTCSrc {
        const NAME: &'static str = "GstJanusVRWebRTCSrc";
        type Type = crate::webrtcsrc::JanusVRWebRTCSrc;
        type ParentType = crate::webrtcsrc::BaseWebRTCSrc;
        type Interfaces = (gst::URIHandler,);
    }

    impl URIHandlerImpl for JanusVRWebRTCSrc {
        const URI_TYPE: gst::URIType = gst::URIType::Src;

        fn protocols() -> &'static [&'static str] {
            &["gstjanusvr", "gstjanusvrs"]
        }

        fn uri(&self) -> Option<String> {
            {
                let settings = self.settings.lock().unwrap();
                if settings.use_string_ids {
                    // URI not supported for string ids
                    return None;
                }
            }

            let obj = self.obj();
            let base = obj.upcast_ref::<crate::webrtcsrc::BaseWebRTCSrc>().imp();
            let signaller = base.signaller();

            let janus_endpoint = signaller.property::<String>("janus-endpoint");
            let uri = janus_endpoint
                .replace("wss://", "gstjansvrs://")
                .replace("ws://", "gstjanusvr://");
            let room_id = signaller.property::<u64>("room-id");
            let producer_peer_id = signaller.property::<u64>("producer-peer-id");

            Some(format!(
                "{uri}?use-string-ids=false&room-id={room_id}&producer-peer-id={producer_peer_id}"
            ))
        }

        fn set_uri(&self, uri: &str) -> Result<(), glib::Error> {
            gst::debug!(CAT, imp = self, "parsing URI {uri}");

            let uri = Url::from_str(uri)
                .map_err(|err| glib::Error::new(gst::URIError::BadUri, &format!("{err:?}")))?;

            let socket_scheme = match uri.scheme() {
                "gstjanusvr" => Ok("ws"),
                "gstjanusvrs" => Ok("wss"),
                _ => Err(glib::Error::new(
                    gst::URIError::BadUri,
                    &format!("Invalid protocol: {}", uri.scheme()),
                )),
            }?;

            let port = uri
                .port()
                .map(|port| format!(":{port}"))
                .unwrap_or_default();

            let janus_endpoint = format!(
                "{socket_scheme}://{}{port}{}",
                uri.host_str().unwrap_or("127.0.0.1"),
                uri.path()
            );

            let use_strings_ids = uri
                .query_pairs()
                .find(|(k, _v)| k == "use-string-ids")
                .map(|(_k, v)| v.to_lowercase() == "true")
                .unwrap_or_default();
            if use_strings_ids {
                // TODO: we'd have to instantiate a JanusVRSignallerStr and set it on the src element
                // but "signaller" is a construct-only property.
                return Err(glib::Error::new(
                    gst::URIError::BadUri,
                    "use-string-ids=true not yet supported in URI",
                ));
            }

            let room_id = uri
                .query_pairs()
                .find(|(k, _v)| k == "room-id")
                .map(|(_k, v)| v)
                .ok_or(glib::Error::new(gst::URIError::BadUri, "room-id missing"))?;
            let producer_peer_id = uri
                .query_pairs()
                .find(|(k, _v)| k == "producer-peer-id")
                .map(|(_k, v)| v)
                .ok_or(glib::Error::new(
                    gst::URIError::BadUri,
                    "producer-peer-id missing",
                ))?;

            let obj = self.obj();
            let base = obj.upcast_ref::<crate::webrtcsrc::BaseWebRTCSrc>().imp();
            let signaller = base.signaller();

            let room_id = room_id.parse::<u64>().map_err(|err| {
                glib::Error::new(gst::URIError::BadUri, &format!("Invalid room-id: {err}"))
            })?;
            let producer_peer_id = producer_peer_id.parse::<u64>().map_err(|err| {
                glib::Error::new(
                    gst::URIError::BadUri,
                    &format!("Invalid producer-peer-id: {err}"),
                )
            })?;

            signaller.set_property("janus-endpoint", &janus_endpoint);
            signaller.set_property("room-id", room_id);
            signaller.set_property("producer-peer-id", producer_peer_id);

            Ok(())
        }
    }
}

#[cfg(feature = "whep")]
pub(super) mod whep {
    use super::*;
    use crate::whep_signaller::WhepClientSignaller;

    #[derive(Default)]
    pub struct WhepClientSrc {}

    impl ObjectImpl for WhepClientSrc {
        fn constructed(&self) {
            self.parent_constructed();
            let element = self.obj();
            let ws = element
                .upcast_ref::<crate::webrtcsrc::BaseWebRTCSrc>()
                .imp();

            let _ = ws.set_signaller(WhepClientSignaller::default().upcast());

            let obj = &*self.obj();

            obj.set_suppressed_flags(gst::ElementFlags::SINK | gst::ElementFlags::SOURCE);
            obj.set_element_flags(gst::ElementFlags::SOURCE);
        }
    }

    impl GstObjectImpl for WhepClientSrc {}

    impl BinImpl for WhepClientSrc {}

    impl ElementImpl for WhepClientSrc {
        fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
            static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> =
                LazyLock::new(|| {
                    gst::subclass::ElementMetadata::new(
                        "WhepClientSrc",
                        "Source/Network/WebRTC",
                        "WebRTC source element using WHEP Client as the signaller",
                        "Sanchayan Maity <sanchayan@asymptotic.io>",
                    )
                });

            Some(&*ELEMENT_METADATA)
        }
    }

    impl BaseWebRTCSrcImpl for WhepClientSrc {}

    #[glib::object_subclass]
    impl ObjectSubclass for WhepClientSrc {
        const NAME: &'static str = "GstWhepClientSrc";
        type Type = crate::webrtcsrc::WhepClientSrc;
        type ParentType = crate::webrtcsrc::BaseWebRTCSrc;
    }
}
