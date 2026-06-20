// SPDX-License-Identifier: MPL-2.0

use crate::utils::{
    cleanup_codec_caps, has_raw_caps, make_element, Codec, Codecs, NavigationEvent,
};
use anyhow::Context;
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_plugin_webrtc_signalling::handlers::Handler;
use gst_plugin_webrtc_signalling::server::{Server, ServerError};
use gst_rtp::prelude::*;
use gst_utils::StreamProducer;
use gst_video::subclass::prelude::*;
use gst_video::{VideoInfo, VideoMultiviewMode};
use gst_webrtc::{WebRTCDataChannel, WebRTCICETransportPolicy};

use tokio::net::TcpListener;

use futures::prelude::*;

use anyhow::{anyhow, Error};
use itertools::Itertools;
use std::collections::HashMap;

use std::ops::DerefMut;
use std::ops::Mul;
use std::sync::{mpsc, Arc, Condvar, LazyLock, Mutex};

use super::homegrown_cc::CongestionController;
use super::{
    WebRTCSinkCongestionControl, WebRTCSinkError, WebRTCSinkMitigationMode, WebRTCSinkPad,
};
use crate::signaller::{prelude::*, Signallable, Signaller, WebRTCSignallerRole};
use crate::utils::create_tls_acceptor;
use crate::{utils, RUNTIME};
use std::collections::{BTreeMap, HashSet};
use tracing_subscriber::prelude::*;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "webrtcsink",
        gst::DebugColorFlags::empty(),
        Some("WebRTC sink"),
    )
});

const CUDA_MEMORY_FEATURE: &str = "memory:CUDAMemory";
const GL_MEMORY_FEATURE: &str = "memory:GLMemory";
const NVMM_MEMORY_FEATURE: &str = "memory:NVMM";
const D3D11_MEMORY_FEATURE: &str = "memory:D3D11Memory";
const VA_MEMORY_FEATURE: &str = "memory:VAMemory";

const RTP_TWCC_URI: &str =
    "http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01";

const TLS_HANDSHAKE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

const DEFAULT_STUN_SERVER: Option<&str> = Some("stun://stun.l.google.com:19302");
const DEFAULT_MIN_BITRATE: u32 = 1000;

/* I have found higher values to cause packet loss *somewhere* in
 * my local network, possibly related to chrome's pretty low UDP
 * buffer sizes */
const DEFAULT_MAX_BITRATE: u32 = 8192000;
const DEFAULT_CONGESTION_CONTROL: WebRTCSinkCongestionControl =
    WebRTCSinkCongestionControl::GoogleCongestionControl;
const DEFAULT_DO_FEC: bool = true;
const DEFAULT_DO_RETRANSMISSION: bool = true;
const DEFAULT_DO_CLOCK_SIGNALLING: bool = false;
const DEFAULT_ENABLE_DATA_CHANNEL_NAVIGATION: bool = false;
const DEFAULT_ENABLE_CONTROL_DATA_CHANNEL: bool = false;
const DEFAULT_ICE_TRANSPORT_POLICY: WebRTCICETransportPolicy = WebRTCICETransportPolicy::All;
const DEFAULT_START_BITRATE: u32 = 2048000;
#[cfg(feature = "web_server")]
const DEFAULT_RUN_WEB_SERVER: bool = false;
#[cfg(feature = "web_server")]
const DEFAULT_WEB_SERVER_CERT: Option<&str> = None;
#[cfg(feature = "web_server")]
const DEFAULT_WEB_SERVER_KEY: Option<&str> = None;
#[cfg(feature = "web_server")]
const DEFAULT_WEB_SERVER_PATH: Option<&str> = None;
#[cfg(feature = "web_server")]
const DEFAULT_WEB_SERVER_DIRECTORY: &str = "gstwebrtc-api/dist";
#[cfg(feature = "web_server")]
const DEFAULT_WEB_SERVER_HOST_ADDR: &str = "http://127.0.0.1:8080";
const DEFAULT_FORWARD_METAS: &str = "";
const DEFAULT_ENABLE_MITIGATION_MODES: WebRTCSinkMitigationMode = WebRTCSinkMitigationMode::all();
/* Start adding some FEC when the bitrate > 2Mbps as we found experimentally
 * that it is not worth it below that threshold */
const DO_FEC_THRESHOLD: u32 = 2000000;

#[derive(Debug, Clone, Copy)]
struct CCInfo {
    heuristic: WebRTCSinkCongestionControl,
    min_bitrate: u32,
    max_bitrate: u32,
    start_bitrate: u32,
}

/// User configuration
#[derive(Clone)]
struct Settings {
    video_caps: gst::Caps,
    audio_caps: gst::Caps,
    turn_servers: gst::Array,
    stun_server: Option<String>,
    cc_info: CCInfo,
    do_fec: bool,
    do_retransmission: bool,
    do_clock_signalling: bool,
    enable_data_channel_navigation: bool,
    enable_control_data_channel: bool,
    meta: Option<gst::Structure>,
    ice_transport_policy: WebRTCICETransportPolicy,
    signaller: Signallable,
    #[cfg(feature = "web_server")]
    run_web_server: bool,
    #[cfg(feature = "web_server")]
    web_server_cert: Option<String>,
    #[cfg(feature = "web_server")]
    web_server_key: Option<String>,
    #[cfg(feature = "web_server")]
    web_server_path: Option<String>,
    #[cfg(feature = "web_server")]
    web_server_directory: String,
    #[cfg(feature = "web_server")]
    web_server_host_addr: url::Url,
    forward_metas: HashSet<String>,
    enabled_mitigation_modes: WebRTCSinkMitigationMode,
}

use std::sync::atomic::{AtomicU32, Ordering};

static BD_SEQ: AtomicU32 = AtomicU32::new(0);
fn get_bdseq() -> u32 {
    BD_SEQ.fetch_and(1, Ordering::SeqCst) + 1
}

#[derive(Debug, Clone)]
struct DiscoveryInfo {
    id: u32,
    caps: gst::Caps,
    srcs: Arc<Mutex<Vec<gst_app::AppSrc>>>,
}

impl DiscoveryInfo {
    fn new(caps: gst::Caps) -> Self {
        Self {
            id: get_bdseq(),
            caps,
            srcs: Default::default(),
        }
    }

    fn srcs(&self) -> Vec<gst_app::AppSrc> {
        self.srcs.lock().unwrap().clone()
    }

    fn create_src(&self) -> gst_app::AppSrc {
        let src = gst_app::AppSrc::builder()
            .caps(&self.caps)
            .format(gst::Format::Time)
            .build();

        self.srcs.lock().unwrap().push(src.clone());

        src
    }
}

// Same gst::bus::BusStream but hooking context message from the thread
// where the message is posted, so that GstContext can be shared
#[derive(Debug)]
struct CustomBusStream {
    bus: glib::WeakRef<gst::Bus>,
    receiver: futures::channel::mpsc::UnboundedReceiver<gst::Message>,
}

impl CustomBusStream {
    fn new(bin: &super::BaseWebRTCSink, pipeline: &gst::Pipeline, prefix: &str) -> Self {
        let (sender, receiver) = futures::channel::mpsc::unbounded();
        let bus = pipeline.bus().unwrap();

        let bin_weak = bin.downgrade();
        let pipeline_weak = pipeline.downgrade();
        let prefix_clone = prefix.to_string();
        bus.set_sync_handler(move |_, msg| {
            match msg.view() {
                gst::MessageView::NeedContext(..) | gst::MessageView::HaveContext(..) => {
                    if let Some(bin) = bin_weak.upgrade() {
                        let _ = bin.post_message(msg.clone());
                    }
                }
                gst::MessageView::StateChanged(state_changed) => {
                    if let Some(pipeline) = pipeline_weak.upgrade() {
                        if state_changed.src() == Some(pipeline.upcast_ref()) {
                            pipeline.debug_to_dot_file_with_ts(
                                gst::DebugGraphDetails::all(),
                                format!(
                                    "{}-{:?}-to-{:?}",
                                    prefix_clone,
                                    state_changed.old(),
                                    state_changed.current()
                                ),
                            );
                        }
                    }
                    let _ = sender.unbounded_send(msg.clone());
                }
                _ => {
                    let _ = sender.unbounded_send(msg.clone());
                }
            }

            gst::BusSyncReply::Drop
        });

        Self {
            bus: bus.downgrade(),
            receiver,
        }
    }
}

impl Drop for CustomBusStream {
    fn drop(&mut self) {
        if let Some(bus) = self.bus.upgrade() {
            bus.unset_sync_handler();
        }
    }
}

impl futures::Stream for CustomBusStream {
    type Item = gst::Message;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.receiver.poll_next_unpin(context)
    }
}

impl futures::stream::FusedStream for CustomBusStream {
    fn is_terminated(&self) -> bool {
        self.receiver.is_terminated()
    }
}

/// Wrapper around our sink pads
#[derive(Debug, Clone)]
struct InputStream {
    sink_pad: WebRTCSinkPad,
    producer: Option<StreamProducer>,
    /// The (fixed) caps coming in
    in_caps: Option<gst::Caps>,
    /// The caps we will offer, as a set of fixed structures
    out_caps: Option<gst::Caps>,
    /// Pace input data
    clocksync: Option<gst::Element>,
    /// The serial number picked for this stream
    serial: u32,
    /// Whether the input stream is video or not
    is_video: bool,
    /// Whether initial discovery has started
    initial_discovery_started: bool,
}

/// Wrapper around webrtcbin pads
#[derive(Clone, Debug)]
struct WebRTCPad {
    pad: gst::Pad,
    /// The (fixed) caps of the corresponding input stream
    in_caps: gst::Caps,
    /// The m= line index in the SDP
    media_idx: u32,
    ssrc: u32,
    /// The name of the corresponding InputStream's sink_pad.
    /// When None, the pad was only created to mark its transceiver
    /// as inactive (in the case where we answer an offer).
    stream_name: Option<String>,
    /// The payload selected in the answer, None at first
    payload: Option<i32>,
}

/// Wrapper around GStreamer encoder element, keeps track of factory
/// name in order to provide a unified set / get bitrate API, also
/// tracks a raw capsfilter used to resize / decimate the input video
/// stream according to the bitrate, thresholds hardcoded for now
pub struct VideoEncoder {
    factory_name: String,
    codec_name: String,
    element: gst::Element,
    filter: gst::Element,
    halved_framerate: gst::Fraction,
    video_info: gst_video::VideoInfo,
    session_id: String,
    mitigation_mode: WebRTCSinkMitigationMode,
    pub transceiver: gst_webrtc::WebRTCRTPTransceiver,
    /// name of the sink pad feeding this encoder
    stream_name: String,
}

struct SessionInner {
    id: String,

    pipeline: gst::Pipeline,
    webrtcbin: gst::Element,
    rtprtxsend: Option<gst::Element>,
    webrtc_pads: HashMap<u32, WebRTCPad>,
    peer_id: String,
    encoders: Vec<VideoEncoder>,

    // Our Homegrown controller (if cc_info.heuristic == Homegrown)
    congestion_controller: Option<CongestionController>,
    // Our BandwidthEstimator (if cc_info.heuristic == GoogleCongestionControl)
    rtpgccbwe: Option<gst::Element>,

    sdp: Option<gst_sdp::SDPMessage>,
    stats: gst::Structure,

    cc_info: CCInfo,

    links: HashMap<u32, gst_utils::ConsumptionLink>,
    stats_sigid: Option<glib::SignalHandlerId>,

    // When not None, constructed from offer SDP
    codecs: Option<BTreeMap<i32, Codec>>,

    stats_collection_handle: Option<tokio::task::JoinHandle<()>>,

    navigation_handler: Option<NavigationEventHandler>,
    control_events_handler: Option<ControlRequestHandler>,
    enabled_mitigation_modes: WebRTCSinkMitigationMode,
}

#[derive(Clone)]
struct Session(Arc<Mutex<SessionInner>>);

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
enum SignallerState {
    Started,
    Stopped,
}

// Used to ensure signal are disconnected when a new signaller is is
#[allow(dead_code)]
struct SignallerSignals {
    error: glib::SignalHandlerId,
    request_meta: glib::SignalHandlerId,
    session_requested: glib::SignalHandlerId,
    session_ended: glib::SignalHandlerId,
    session_description: glib::SignalHandlerId,
    handle_ice: glib::SignalHandlerId,
    shutdown: glib::SignalHandlerId,
}

/* Our internal state */
struct State {
    signaller_state: SignallerState,
    sessions: HashMap<String, Session>,
    codecs: BTreeMap<i32, Codec>,
    /// Used to abort codec discovery
    codecs_abort_handles: Vec<futures::future::AbortHandle>,
    /// Used to wait for the discovery task to fully stop
    codecs_done_receivers: Vec<futures::channel::oneshot::Receiver<()>>,
    /// Used to determine whether we can start the signaller when going to Playing,
    /// or whether we should wait
    codec_discovery_done: bool,
    audio_serial: u32,
    video_serial: u32,
    streams: HashMap<String, InputStream>,
    discoveries: HashMap<String, Vec<DiscoveryInfo>>,
    signaller_signals: Option<SignallerSignals>,
    finalizing_sessions: Arc<(Mutex<HashMap<String, Session>>, Condvar)>,
    #[cfg(feature = "web_server")]
    web_shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    #[cfg(feature = "web_server")]
    web_join_handle: Option<tokio::task::JoinHandle<()>>,
    session_mids: HashMap<String, HashMap<String, String>>,
    session_stream_names: HashMap<String, HashMap<String, String>>,
}

fn create_navigation_event(sink: &super::BaseWebRTCSink, msg: &str, session_id: &str) {
    let event: Result<NavigationEvent, _> = serde_json::from_str(msg);

    if let Ok(event) = event {
        gst::log!(CAT, obj = sink, "Processing navigation event: {:?}", event);

        if let Some(mid) = event.mid {
            let this = sink.imp();

            let state = this.state.lock().unwrap();
            let Some(stream) = state
                .session_mids
                .get(session_id)
                .and_then(|mids| mids.get(&mid))
                .and_then(|name| state.streams.get(name))
            else {
                return;
            };
            let event = gst::event::Navigation::new(event.event.structure());

            if !stream.sink_pad.push_event(event.clone()) {
                gst::info!(CAT, obj = sink, "Could not send event: {:?}", event);
            }
        } else {
            let this = sink.imp();

            let state = this.state.lock().unwrap();
            let event = gst::event::Navigation::new(event.event.structure());
            state.streams.iter().for_each(|(_, stream)| {
                if stream.sink_pad.name().starts_with("video_") {
                    gst::log!(CAT, "Navigating to: {:?}", event);
                    if !stream.sink_pad.push_event(event.clone()) {
                        gst::info!(CAT, obj = sink, "Could not send event: {:?}", event);
                    }
                }
            });
        }
    } else {
        gst::error!(CAT, obj = sink, "Invalid navigation event: {:?}", msg);
    }
}

fn handle_control_event(
    sink: &super::BaseWebRTCSink,
    msg: &str,
    session_id: &str,
) -> Result<utils::ControlResponseMessage, Error> {
    let msg: utils::ControlRequestMessage = serde_json::from_str(msg)?;

    let request = match msg.request {
        utils::StringOrRequest::String(s) => serde_json::from_str(&s)?,
        utils::StringOrRequest::Request(r) => r,
    };

    let event = match request {
        utils::ControlRequest::NavigationEvent { event } => {
            gst::event::Navigation::new(event.structure())
        }
        utils::ControlRequest::CustomUpstreamEvent {
            structure_name,
            structure,
        } => gst::event::CustomUpstream::new(utils::deserialize_serde_object(
            &structure,
            &structure_name,
        )?),
    };

    gst::log!(CAT, obj = sink, "Processing control event: {:?}", event);

    let mut ret = false;

    if let Some(mid) = msg.mid {
        let this = sink.imp();

        let state = this.state.lock().unwrap();
        let Some(stream) = state
            .session_mids
            .get(session_id)
            .and_then(|mids| mids.get(&mid))
            .and_then(|name| state.streams.get(name))
        else {
            return Err(anyhow!("No relevant stream to forward event to"));
        };
        if !stream.sink_pad.push_event(event.clone()) {
            gst::info!(CAT, obj = sink, "Could not send event: {:?}", event);
        } else {
            ret = true;
        }
    } else {
        let this = sink.imp();

        let state = this.state.lock().unwrap();
        state.streams.iter().for_each(|(_, stream)| {
            if !stream.sink_pad.push_event(event.clone()) {
                gst::info!(CAT, obj = sink, "Could not send event: {:?}", event);
            } else {
                ret = true;
            }
        });
    }

    Ok(utils::ControlResponseMessage {
        id: msg.id,
        error: if ret {
            None
        } else {
            Some("No sink pad could handle the request".to_string())
        },
    })
}

/// Simple utility for tearing down a pipeline cleanly
struct PipelineWrapper(gst::Pipeline);

// Structure to generate GstNavigation event from a WebRTCDataChannel
// This is simply used to hold references to the inner items.
#[allow(dead_code)]
#[derive(Debug)]
struct NavigationEventHandler((Option<glib::SignalHandlerId>, WebRTCDataChannel));

// Structure to generate arbitrary upstream events from a WebRTCDataChannel
#[allow(dead_code)]
#[derive(Debug)]
struct ControlRequestHandler((Option<glib::SignalHandlerId>, WebRTCDataChannel));

/// Our instance structure
#[derive(Default)]
pub struct BaseWebRTCSink {
    state: Mutex<State>,
    settings: Mutex<Settings>,
}

impl Default for Settings {
    fn default() -> Self {
        let signaller = Signaller::new(WebRTCSignallerRole::Producer);

        Self {
            video_caps: Codecs::video_codecs()
                .filter(|codec| !codec.is_raw)
                .flat_map(|codec| codec.caps.iter().map(ToOwned::to_owned))
                .collect::<gst::Caps>(),
            audio_caps: Codecs::audio_codecs()
                .filter(|codec| !codec.is_raw)
                .flat_map(|codec| codec.caps.iter().map(ToOwned::to_owned))
                .collect::<gst::Caps>(),
            stun_server: DEFAULT_STUN_SERVER.map(String::from),
            turn_servers: gst::Array::new(Vec::new() as Vec<glib::SendValue>),
            cc_info: CCInfo {
                heuristic: DEFAULT_CONGESTION_CONTROL,
                min_bitrate: DEFAULT_MIN_BITRATE,
                max_bitrate: DEFAULT_MAX_BITRATE,
                start_bitrate: DEFAULT_START_BITRATE,
            },
            do_fec: DEFAULT_DO_FEC,
            do_retransmission: DEFAULT_DO_RETRANSMISSION,
            do_clock_signalling: DEFAULT_DO_CLOCK_SIGNALLING,
            enable_data_channel_navigation: DEFAULT_ENABLE_DATA_CHANNEL_NAVIGATION,
            enable_control_data_channel: DEFAULT_ENABLE_CONTROL_DATA_CHANNEL,
            meta: None,
            ice_transport_policy: DEFAULT_ICE_TRANSPORT_POLICY,
            signaller: signaller.upcast(),
            #[cfg(feature = "web_server")]
            run_web_server: DEFAULT_RUN_WEB_SERVER,
            #[cfg(feature = "web_server")]
            web_server_cert: DEFAULT_WEB_SERVER_CERT.map(String::from),
            #[cfg(feature = "web_server")]
            web_server_key: DEFAULT_WEB_SERVER_KEY.map(String::from),
            #[cfg(feature = "web_server")]
            web_server_path: DEFAULT_WEB_SERVER_PATH.map(String::from),
            #[cfg(feature = "web_server")]
            web_server_directory: String::from(DEFAULT_WEB_SERVER_DIRECTORY),
            #[cfg(feature = "web_server")]
            web_server_host_addr: url::Url::parse(DEFAULT_WEB_SERVER_HOST_ADDR).unwrap(),
            forward_metas: HashSet::new(),
            enabled_mitigation_modes: DEFAULT_ENABLE_MITIGATION_MODES,
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            signaller_state: SignallerState::Stopped,
            sessions: HashMap::new(),
            codecs: BTreeMap::new(),
            codecs_abort_handles: Vec::new(),
            codecs_done_receivers: Vec::new(),
            codec_discovery_done: false,
            audio_serial: 0,
            video_serial: 0,
            streams: HashMap::new(),
            discoveries: HashMap::new(),
            signaller_signals: Default::default(),
            finalizing_sessions: Arc::new((Mutex::new(HashMap::new()), Condvar::new())),
            #[cfg(feature = "web_server")]
            web_shutdown_tx: None,
            #[cfg(feature = "web_server")]
            web_join_handle: None,
            session_mids: HashMap::new(),
            session_stream_names: HashMap::new(),
        }
    }
}

fn make_converter_for_video_caps(caps: &gst::Caps, codec: &Codec) -> Result<gst::Element, Error> {
    assert!(caps.is_fixed());

    let video_info = gst_video::VideoInfo::from_caps(caps)?;

    let ret = gst::Bin::default();

    let (mut head, tail) = {
        if let Some(feature) = caps.features(0) {
            if feature.contains(NVMM_MEMORY_FEATURE)
                // NVIDIA V4L2 encoders require NVMM memory as input and that requires using the
                // corresponding converter
                || codec
                .encoder_factory()
                .is_some_and(|factory| factory.name().starts_with("nvv4l2"))
            {
                let queue = make_element("queue", None)?;
                let nvconvert = if let Ok(nvconvert) = make_element("nvvideoconvert", None) {
                    nvconvert.set_property_from_str("compute-hw", "Default");
                    nvconvert.set_property_from_str("nvbuf-memory-type", "nvbuf-mem-default");
                    nvconvert
                } else {
                    make_element("nvvidconv", None)?
                };

                ret.add_many([&queue, &nvconvert])?;
                gst::Element::link_many([&queue, &nvconvert])?;

                (queue, nvconvert)
            } else if feature.contains(D3D11_MEMORY_FEATURE) {
                let d3d11upload = make_element("d3d11upload", None)?;
                let d3d11convert = make_element("d3d11convert", None)?;

                ret.add_many([&d3d11upload, &d3d11convert])?;
                d3d11upload.link(&d3d11convert)?;

                (d3d11upload, d3d11convert)
            } else if feature.contains(CUDA_MEMORY_FEATURE) {
                if let Some(convert_factory) = gst::ElementFactory::find("cudaconvert") {
                    let cudaupload = make_element("cudaupload", None)?;
                    let cudaconvert = convert_factory.create().build()?;
                    let cudascale = make_element("cudascale", None)?;

                    ret.add_many([&cudaupload, &cudaconvert, &cudascale])?;
                    gst::Element::link_many([&cudaupload, &cudaconvert, &cudascale])?;

                    (cudaupload, cudascale)
                } else {
                    let cudadownload = make_element("cudadownload", None)?;
                    let convert = make_element("videoconvert", None)?;
                    let scale = make_element("videoscale", None)?;

                    gst::warning!(
                        CAT,
                        "No cudaconvert factory available, falling back to software"
                    );

                    ret.add_many([&cudadownload, &convert, &scale])?;
                    gst::Element::link_many([&cudadownload, &convert, &scale])?;

                    (cudadownload, scale)
                }
            } else if feature.contains(GL_MEMORY_FEATURE) {
                let glupload = make_element("glupload", None)?;
                let glconvert = make_element("glcolorconvert", None)?;
                let glscale = make_element("glcolorscale", None)?;

                ret.add_many([&glupload, &glconvert, &glscale])?;
                gst::Element::link_many([&glupload, &glconvert, &glscale])?;

                (glupload, glscale)
            } else if feature.contains(VA_MEMORY_FEATURE) {
                let vapostproc = make_element("vapostproc", None)?;

                ret.add_many([&vapostproc])?;
                (vapostproc.clone(), vapostproc)
            } else {
                let convert = make_element("videoconvert", None)?;
                let scale = make_element("videoscale", None)?;

                ret.add_many([&convert, &scale])?;
                gst::Element::link_many([&convert, &scale])?;

                (convert, scale)
            }
        } else {
            let convert = make_element("videoconvert", None)?;
            let scale = make_element("videoscale", None)?;

            ret.add_many([&convert, &scale])?;
            gst::Element::link_many([&convert, &scale])?;

            (convert, scale)
        }
    };

    if video_info.fps().numer() != 0 {
        let vrate = make_element("videorate", None)?;
        vrate.set_property("drop-only", true);
        vrate.set_property("skip-to-first", true);

        ret.add(&vrate)?;
        vrate.link(&head)?;
        head = vrate;
    }

    ret.add_pad(&gst::GhostPad::with_target(&head.static_pad("sink").unwrap()).unwrap())
        .unwrap();

    ret.add_pad(&gst::GhostPad::with_target(&tail.static_pad("src").unwrap()).unwrap())
        .unwrap();

    Ok(ret.upcast())
}

/// Add a pad probe to convert force-keyunit events to the custom action signal based NVIDIA
/// encoder API.
fn add_nv4l2enc_force_keyunit_workaround(enc: &gst::Element) {
    use std::sync::atomic::{self, AtomicBool};

    let srcpad = enc.static_pad("src").unwrap();
    let saw_buffer = AtomicBool::new(false);
    srcpad
        .add_probe(
            gst::PadProbeType::BUFFER
                | gst::PadProbeType::BUFFER_LIST
                | gst::PadProbeType::EVENT_UPSTREAM,
            move |pad, info| {
                match info.data {
                    Some(gst::PadProbeData::Buffer(..))
                    | Some(gst::PadProbeData::BufferList(..)) => {
                        saw_buffer.store(true, atomic::Ordering::SeqCst);
                    }
                    Some(gst::PadProbeData::Event(ref ev))
                        if gst_video::ForceKeyUnitEvent::is(ev)
                            && saw_buffer.load(atomic::Ordering::SeqCst) =>
                    {
                        let enc = pad.parent().unwrap();
                        enc.emit_by_name::<()>("force-IDR", &[]);
                    }
                    _ => {}
                }

                gst::PadProbeReturn::Ok
            },
        )
        .unwrap();
}

/// Default configuration for known encoders, can be disabled
/// by returning True from an encoder-setup handler.
fn configure_encoder(enc: &gst::Element, start_bitrate: u32) {
    let audio_encoder = enc.is::<gst_audio::AudioEncoder>();
    if audio_encoder {
        // Chrome audio decoder expects perfect timestamps
        enc.set_property("perfect-timestamp", true);
    }

    if let Some(factory) = enc.factory() {
        match factory.name().as_str() {
            "vp8enc" | "vp9enc" => {
                enc.set_property("deadline", 1i64);
                enc.set_property("target-bitrate", start_bitrate as i32);
                enc.set_property("cpu-used", -16i32);
                enc.set_property("keyframe-max-dist", 2000i32);
                enc.set_property_from_str("keyframe-mode", "disabled");
                enc.set_property_from_str("end-usage", "cbr");
                enc.set_property("buffer-initial-size", 100i32);
                enc.set_property("buffer-optimal-size", 120i32);
                enc.set_property("buffer-size", 150i32);
                enc.set_property("max-intra-bitrate", 250i32);
                enc.set_property_from_str("error-resilient", "default");
                enc.set_property("lag-in-frames", 0i32);
            }
            "x264enc" => {
                enc.set_property("bitrate", start_bitrate / 1000);
                enc.set_property_from_str("tune", "zerolatency");
                enc.set_property_from_str("speed-preset", "ultrafast");
                enc.set_property("threads", 4u32);
                enc.set_property("key-int-max", 2560u32);
                enc.set_property("b-adapt", false);
                enc.set_property("vbv-buf-capacity", 120u32);
            }
            "openh264enc" => {
                enc.set_property("bitrate", start_bitrate);
                enc.set_property("gop-size", 2560u32);
                enc.set_property_from_str("rate-control", "bitrate");
                enc.set_property_from_str("complexity", "low");
                enc.set_property("background-detection", false);
                enc.set_property("scene-change-detection", false);
            }
            "nvh264enc" | "nvh265enc" => {
                enc.set_property("bitrate", start_bitrate / 1000);
                enc.set_property("gop-size", 2560i32);
                enc.set_property_from_str("rc-mode", "cbr-ld-hq");
                enc.set_property("zerolatency", true);
            }
            "vaapih264enc" | "vaapivp8enc" | "vaapivp9enc" => {
                enc.set_property("bitrate", start_bitrate / 1000);
                enc.set_property("keyframe-period", 2560u32);
                enc.set_property_from_str("rate-control", "cbr");
            }
            "vavp8enc" | "vavp8lpenc" | "vavp9enc" | "vavp9lpenc" | "vah264enc" | "vah264lpenc"
            | "vah265enc" | "vah265lpenc" | "vaav1enc" | "vaav1lpenc" => {
                enc.set_property("bitrate", start_bitrate / 1000);
                enc.set_property("key-int-max", 256u32);
                enc.set_property_from_str("rate-control", "cbr");
            }
            "nvv4l2h264enc" => {
                enc.set_property("bitrate", start_bitrate);
                enc.set_property_from_str("preset-level", "UltraFastPreset");
                enc.set_property("maxperf-enable", true);
                enc.set_property("insert-vui", true);
                enc.set_property("idrinterval", 256u32);
                enc.set_property("insert-sps-pps", true);
                enc.set_property("insert-aud", true);
                enc.set_property_from_str("control-rate", "constant_bitrate");
                add_nv4l2enc_force_keyunit_workaround(enc);
            }
            "nvv4l2vp8enc" | "nvv4l2vp9enc" => {
                enc.set_property("bitrate", start_bitrate);
                enc.set_property_from_str("preset-level", "UltraFastPreset");
                enc.set_property("maxperf-enable", true);
                enc.set_property("idrinterval", 256u32);
                enc.set_property_from_str("control-rate", "constant_bitrate");
                add_nv4l2enc_force_keyunit_workaround(enc);
            }
            "qsvh264enc" => {
                enc.set_property("bitrate", start_bitrate / 1000);
                enc.set_property("gop-size", 2560u32);
                enc.set_property("low-latency", true);
                enc.set_property("disable-hrd-conformance", true);
                enc.set_property_from_str("rate-control", "cbr");
            }
            "nvav1enc" => {
                enc.set_property("bitrate", start_bitrate / 1000);
                enc.set_property("gop-size", -1i32);
                enc.set_property_from_str("rc-mode", "cbr");
                enc.set_property("zerolatency", true);
            }
            "av1enc" => {
                enc.set_property("target-bitrate", start_bitrate / 1000);
                enc.set_property_from_str("end-usage", "cbr");
                enc.set_property("keyframe-max-dist", i32::MAX);
                enc.set_property_from_str("usage-profile", "realtime");
            }
            "rav1enc" => {
                enc.set_property("bitrate", start_bitrate as i32);
                enc.set_property("low-latency", true);
                enc.set_property("max-key-frame-interval", 715827882u64);
                enc.set_property("speed-preset", 10u32);
            }
            "vpuenc_h264" => {
                enc.set_property("bitrate", start_bitrate / 1000);
                enc.set_property("gop-size", 2560u32);
            }
            "nvv4l2av1enc" => {
                enc.set_property("bitrate", start_bitrate);
                enc.set_property("maxperf-enable", true);
                enc.set_property_from_str("control-rate", "constant_bitrate");
                enc.set_property_from_str("preset-level", "UltraFastPreset");
                add_nv4l2enc_force_keyunit_workaround(enc);
            }
            _ => (),
        }
    }
}

/// Default configuration for known payloaders, can be disabled
/// by returning True from a payloader-setup handler.
fn configure_payloader(pay: &gst::Element) {
    pay.set_property("mtu", 1200_u32);

    if let Some(factory) = pay.factory() {
        match factory.name().as_str() {
            "rtpvp8pay" | "rtpvp9pay" => {
                pay.set_property_from_str("picture-id-mode", "15-bit");
            }
            "rtph264pay" | "rtph265pay" => {
                pay.set_property_from_str("aggregate-mode", "zero-latency");
                pay.set_property("config-interval", -1i32);
            }
            _ => (),
        }
    }
}

fn setup_signal_accumulator(
    _hint: &glib::subclass::SignalInvocationHint,
    _acc: glib::Value,
    value: &glib::Value,
) -> std::ops::ControlFlow<glib::Value, glib::Value> {
    let is_configured = value.get::<bool>().unwrap();
    if !is_configured {
        std::ops::ControlFlow::Continue(value.clone())
    } else {
        std::ops::ControlFlow::Break(value.clone())
    }
}

/// Set of elements used in an EncodingChain
struct EncodingChain {
    raw_filter: Option<gst::Element>,
    encoder: Option<gst::Element>,
    pay_filter: gst::Element,
}

/// A set of elements that transform raw data into RTP packets
struct PayloadChain {
    encoding_chain: EncodingChain,
    payloader: gst::Element,
}

struct PayloadChainBuilder {
    /// Caps of the input chain
    input_caps: gst::Caps,
    /// Caps expected after the payloader
    output_caps: gst::Caps,
    ///  The Codec representing wanted encoding
    codec: Codec,
    /// Filter element between the encoder and the payloader.
    encoded_filter: Option<gst::Element>,
}

impl PayloadChainBuilder {
    fn new(
        input_caps: &gst::Caps,
        output_caps: &gst::Caps,
        codec: &Codec,
        encoded_filter: Option<gst::Element>,
    ) -> Self {
        Self {
            input_caps: input_caps.clone(),
            output_caps: output_caps.clone(),
            codec: codec.clone(),
            encoded_filter,
        }
    }

    fn build(self, pipeline: &gst::Pipeline, src: &gst::Element) -> Result<PayloadChain, Error> {
        gst::trace!(
            CAT,
            obj = pipeline,
            "Setting up encoding, input caps: {input_caps}, \
                    output caps: {output_caps}, codec: {codec:?}",
            input_caps = self.input_caps,
            output_caps = self.output_caps,
            codec = self.codec,
        );

        let needs_encoding = if self.codec.is_raw {
            !self.codec.caps.can_intersect(&self.input_caps)
        } else {
            has_raw_caps(&self.input_caps)
        };

        let mut elements: Vec<gst::Element> = Vec::new();

        let (raw_filter, encoder) = if needs_encoding {
            elements.push(match self.codec.is_video() {
                true => make_converter_for_video_caps(&self.input_caps, &self.codec)?.upcast(),
                false => {
                    gst::parse::bin_from_description("audioresample ! audioconvert", true)?.upcast()
                }
            });

            let raw_filter = self.codec.raw_converter_filter()?;
            elements.push(raw_filter.clone());

            let encoder = if self.codec.is_raw {
                None
            } else {
                let encoder = self
                    .codec
                    .build_encoder()
                    .expect("We should always have an encoder for negotiated codecs")?;
                elements.push(encoder.clone());
                elements.push(make_element("capsfilter", None)?);

                Some(encoder)
            };

            (Some(raw_filter), encoder)
        } else {
            (None, None)
        };

        if let Some(parser) = self.codec.build_parser()? {
            elements.push(parser);
        }

        // Only force the profile when output caps were not specified, either
        // through input caps or because we are answering an offer
        let force_profile = self.output_caps.is_any() && needs_encoding;
        elements.push(
            gst::ElementFactory::make("capsfilter")
                .property("caps", self.codec.parser_caps(force_profile))
                .build()
                .with_context(|| "Failed to make element capsfilter")?,
        );

        if let Some(ref encoded_filter) = self.encoded_filter {
            elements.push(encoded_filter.clone());
        }

        let pay = self
            .codec
            .create_payloader()
            .expect("Payloaders should always have been set in the CodecInfo we handle");

        elements.push(pay.clone());

        let pay_filter = gst::ElementFactory::make("capsfilter")
            .property("caps", self.output_caps)
            .build()
            .with_context(|| "Failed to make payloader")?;
        elements.push(pay_filter.clone());

        for element in &elements {
            pipeline.add(element).unwrap();
        }

        elements.insert(0, src.clone());
        gst::Element::link_many(elements.iter().collect::<Vec<&gst::Element>>().as_slice())
            .with_context(|| "Linking encoding elements")?;

        Ok(PayloadChain {
            encoding_chain: EncodingChain {
                raw_filter,
                encoder,
                pay_filter,
            },
            payloader: pay,
        })
    }
}

fn default_configure_mitigation_mode(
    video_info: &VideoInfo,
    bitrate: i32,
    enabled_mitigation_modes: WebRTCSinkMitigationMode,
) -> gst::Caps {
    let mut s = gst::Structure::new_empty("video/x-raw");
    if enabled_mitigation_modes.is_empty() {
        return gst::Caps::builder_full_with_any_features()
            .structure(s)
            .build();
    }

    if bitrate < 500000 && enabled_mitigation_modes.contains(WebRTCSinkMitigationMode::DOWNSAMPLED)
    {
        let scaled_framerate = video_info.fps().mul(gst::Fraction::new(1, 2));
        if scaled_framerate.numer() != 0 {
            s.set("framerate", scaled_framerate);
        }
    }

    if enabled_mitigation_modes.contains(WebRTCSinkMitigationMode::DOWNSCALED) {
        let height = match bitrate {
            b if b < 1_000_000 => 360,
            b if b < 2_000_000 => 720,
            _ => video_info.height() as i32,
        };
        let width = VideoEncoder::scale_height_round_2(video_info, height);

        s.set("height", height);
        s.set("width", width);
    }

    gst::Caps::builder_full_with_any_features()
        .structure(s)
        .build()
}

impl VideoEncoder {
    fn new(
        encoding_elements: &EncodingChain,
        video_info: gst_video::VideoInfo,
        session_id: &str,
        codec_name: &str,
        transceiver: gst_webrtc::WebRTCRTPTransceiver,
        stream_name: String,
    ) -> Option<Self> {
        let halved_framerate = video_info.fps().mul(gst::Fraction::new(1, 2));
        Some(Self {
            factory_name: encoding_elements.encoder.as_ref()?.factory()?.name().into(),
            codec_name: codec_name.to_string(),
            element: encoding_elements.encoder.as_ref()?.clone(),
            filter: encoding_elements.raw_filter.as_ref()?.clone(),
            halved_framerate,
            video_info,
            session_id: session_id.to_string(),
            mitigation_mode: WebRTCSinkMitigationMode::NONE,
            transceiver,
            stream_name,
        })
    }

    fn is_bitrate_supported(factory_name: &str) -> bool {
        matches!(
            factory_name,
            "vp8enc"
                | "vp9enc"
                | "x264enc"
                | "openh264enc"
                | "nvh264enc"
                | "nvh265enc"
                | "vaapih264enc"
                | "vaapivp8enc"
                | "vaapivp9enc"
                | "qsvh264enc"
                | "nvv4l2h264enc"
                | "nvv4l2vp8enc"
                | "nvv4l2vp9enc"
                | "nvav1enc"
                | "av1enc"
                | "rav1enc"
                | "vpuenc_h264"
                | "nvv4l2av1enc"
                | "vavp8enc"
                | "vavp8lpenc"
                | "vavp9enc"
                | "vavp9lpenc"
                | "vah264enc"
                | "vah264lpenc"
                | "vah265enc"
                | "vah265lpenc"
                | "vaav1enc"
                | "vaav1lpenc"
        )
    }

    fn bitrate(&self) -> Result<i32, WebRTCSinkError> {
        let bitrate = match self.factory_name.as_str() {
            "vp8enc" | "vp9enc" => self.element.property::<i32>("target-bitrate"),
            "av1enc" => (self.element.property::<u32>("target-bitrate") * 1000) as i32,
            "x264enc" | "nvh264enc" | "nvh265enc" | "vaapih264enc" | "vaapivp8enc"
            | "vaapivp9enc" | "qsvh264enc" | "nvav1enc" | "vpuenc_h264" | "vavp8enc"
            | "vavp8lpenc" | "vavp9enc" | "vavp9lpenc" | "vah264enc" | "vah264lpenc"
            | "vah265enc" | "vah265lpenc" | "vaav1enc" | "vaav1lpenc" => {
                (self.element.property::<u32>("bitrate") * 1000) as i32
            }
            "openh264enc" | "nvv4l2h264enc" | "nvv4l2vp8enc" | "nvv4l2vp9enc" | "rav1enc"
            | "nvv4l2av1enc" => (self.element.property::<u32>("bitrate")) as i32,
            _ => return Err(WebRTCSinkError::BitrateNotSupported),
        };

        Ok(bitrate)
    }

    fn scale_height_round_2(video_info: &VideoInfo, height: i32) -> i32 {
        let ratio = gst_video::calculate_display_ratio(
            video_info.width(),
            video_info.height(),
            video_info.par(),
            gst::Fraction::new(1, 1),
        )
        .unwrap();

        let width = height.mul_div_ceil(ratio.numer(), ratio.denom()).unwrap();

        (width + 1) & !1
    }

    pub(crate) fn set_bitrate(
        &mut self,
        element: &super::BaseWebRTCSink,
        bitrate: i32,
        enabled_mitigation_modes: WebRTCSinkMitigationMode,
    ) -> Result<(), WebRTCSinkError> {
        match self.factory_name.as_str() {
            "vp8enc" | "vp9enc" => self.element.set_property("target-bitrate", bitrate),
            "av1enc" => self
                .element
                .set_property("target-bitrate", (bitrate / 1000) as u32),
            "x264enc" | "nvh264enc" | "nvh265enc" | "vaapih264enc" | "vaapivp8enc"
            | "vaapivp9enc" | "qsvh264enc" | "nvav1enc" | "vpuenc_h264" | "vavp8enc"
            | "vavp8lpenc" | "vavp9enc" | "vavp9lpenc" | "vah264enc" | "vah264lpenc"
            | "vah265enc" | "vah265lpenc" | "vaav1enc" | "vaav1lpenc" => {
                self.element
                    .set_property("bitrate", (bitrate / 1000) as u32);
            }
            "openh264enc" | "nvv4l2h264enc" | "nvv4l2vp8enc" | "nvv4l2vp9enc" | "nvv4l2av1enc" => {
                self.element.set_property("bitrate", bitrate as u32)
            }
            "rav1enc" => self.element.set_property("bitrate", bitrate),
            _ => return Err(WebRTCSinkError::BitrateNotSupported),
        }

        let current_caps = self.filter.property::<gst::Caps>("caps");

        let mitigation_mode_caps = element.emit_by_name::<gst::Caps>(
            "configure-mitigation-caps",
            &[
                &self.stream_name,
                &self.video_info,
                &bitrate,
                &enabled_mitigation_modes,
            ],
        );
        let mitigation_mode_caps_structure = mitigation_mode_caps.structure(0).unwrap().to_owned();

        self.mitigation_mode = WebRTCSinkMitigationMode::NONE;

        if mitigation_mode_caps_structure.get::<i32>("height").is_ok() {
            self.mitigation_mode = WebRTCSinkMitigationMode::DOWNSCALED;
        }

        if mitigation_mode_caps_structure
            .get::<gst::Fraction>("framerate")
            .is_ok()
        {
            self.mitigation_mode |= WebRTCSinkMitigationMode::DOWNSAMPLED;
        }

        if !mitigation_mode_caps.is_strictly_equal(&current_caps) {
            gst::log!(
                CAT,
                obj = element,
                "session {}: setting bitrate {} and caps {} on encoder {:?}",
                self.session_id,
                bitrate,
                mitigation_mode_caps,
                self.element
            );

            self.filter.set_property("caps", mitigation_mode_caps);
        }

        Ok(())
    }

    fn gather_stats(&self) -> gst::Structure {
        gst::Structure::builder("application/x-webrtcsink-video-encoder-stats")
            .field("bitrate", self.bitrate().unwrap_or(0i32))
            .field("mitigation-mode", self.mitigation_mode)
            .field("codec-name", self.codec_name.as_str())
            .field(
                "fec-percentage",
                self.transceiver.property::<u32>("fec-percentage"),
            )
            .build()
    }
}

impl State {
    fn finalize_session(&mut self, element: &super::BaseWebRTCSink, session: Session) {
        let mut inner = session.0.lock().unwrap();

        gst::info!(CAT, "Ending session {}", inner.id);
        inner.pipeline.debug_to_dot_file_with_ts(
            gst::DebugGraphDetails::all(),
            format!("removing-session-{}-", inner.id),
        );

        let webrtc_pads: HashMap<_, _> = inner.webrtc_pads.drain().collect();

        for ssrc in webrtc_pads.keys() {
            inner.links.remove(ssrc);
        }

        let stats_collection_handle = inner.stats_collection_handle.take();
        let pipeline = inner.pipeline.clone();
        let session_id = inner.id.clone();
        drop(inner);

        let finalizing_sessions = self.finalizing_sessions.clone();
        let (sessions, _cvar) = &*finalizing_sessions;
        sessions.lock().unwrap().insert(session_id.clone(), session);

        let element = element.clone();
        RUNTIME.spawn_blocking(move || {
            if let Some(stats_collection_handle) = stats_collection_handle {
                stats_collection_handle.abort();
                let _ = RUNTIME.block_on(stats_collection_handle);
            }

            let _ = pipeline.set_state(gst::State::Null);
            drop(pipeline);

            let (sessions, cvar) = &*finalizing_sessions;
            let mut sessions = sessions.lock().unwrap();
            let session = sessions.remove(&session_id).unwrap();

            let _ = session
                .0
                .lock()
                .unwrap()
                .pipeline
                .set_state(gst::State::Null);

            cvar.notify_one();

            gst::debug!(CAT, obj = element, "Session {session_id} ended");
        });
    }

    fn end_session(
        &mut self,
        element: &super::BaseWebRTCSink,
        session_id: &str,
    ) -> Option<Session> {
        if let Some(session) = self.sessions.remove(session_id) {
            self.finalize_session(element, session.clone());
            Some(session)
        } else {
            None
        }
    }

    fn should_start_signaller(&mut self, element: &super::BaseWebRTCSink) -> bool {
        self.signaller_state == SignallerState::Stopped
            && element.current_state() >= gst::State::Paused
            && self.codec_discovery_done
    }

    fn queue_discovery(&mut self, stream_name: &str, discovery_info: DiscoveryInfo) {
        if let Some(discos) = self.discoveries.get_mut(stream_name) {
            discos.push(discovery_info);
        } else {
            self.discoveries
                .insert(stream_name.to_string(), vec![discovery_info]);
        }
    }

    fn remove_discovery(&mut self, stream_name: &str, discovery_info: &DiscoveryInfo) {
        if let Some(discos) = self.discoveries.get_mut(stream_name) {
            let position = discos
                .iter()
                .position(|d| d.id == discovery_info.id)
                .expect(
                    "We expect discovery to always be in the list of discoverers when removing",
                );
            discos.remove(position);
        }
    }
}

impl SessionInner {
    #[allow(clippy::too_many_arguments)]
    fn new(
        id: String,
        pipeline: gst::Pipeline,
        webrtcbin: gst::Element,
        peer_id: String,
        congestion_controller: Option<CongestionController>,
        rtpgccbwe: Option<gst::Element>,
        cc_info: CCInfo,
        enabled_mitigation_modes: WebRTCSinkMitigationMode,
    ) -> Self {
        Self {
            id,
            pipeline,
            webrtcbin,
            peer_id,
            cc_info,
            rtprtxsend: None,
            congestion_controller,
            rtpgccbwe,
            stats: gst::Structure::new_empty("application/x-webrtc-stats"),
            sdp: None,
            webrtc_pads: HashMap::new(),
            encoders: Vec::new(),
            links: HashMap::new(),
            stats_sigid: None,
            codecs: None,
            stats_collection_handle: None,
            navigation_handler: None,
            control_events_handler: None,
            enabled_mitigation_modes,
        }
    }

    fn gather_stats(&self) -> gst::Structure {
        let mut ret = self.stats.to_owned();

        let encoder_stats = self
            .encoders
            .iter()
            .map(VideoEncoder::gather_stats)
            .map(|s| s.to_send_value())
            .collect::<gst::Array>();

        let our_stats = gst::Structure::builder("application/x-webrtcsink-consumer-stats")
            .field("video-encoders", encoder_stats)
            .build();

        ret.set("consumer-stats", our_stats);

        ret
    }

    /// Called when we have received an answer, connects an InputStream
    /// to a given WebRTCPad
    fn connect_input_stream(
        &mut self,
        element: &super::BaseWebRTCSink,
        producer: &StreamProducer,
        webrtc_pad: &WebRTCPad,
        session_setup_result: SessionSetupResult,
    ) -> Result<(), Error> {
        let (appsrc, encoding_chain, caps, codec, stream_name) = session_setup_result;

        gst::info!(
            CAT,
            obj = element,
            "Connecting input stream {} for consumer {} and media {}",
            stream_name,
            self.peer_id,
            webrtc_pad.media_idx
        );

        let pay_filter = make_element("capsfilter", None)?;
        self.pipeline.add(&pay_filter).unwrap();

        // At this point, the peer has provided its answer, and we want to
        // let the payloader / encoder perform negotiation according to that.
        //
        // This means we need to unset our codec preferences, as they would now
        // conflict with what the peer actually requested (see webrtcbin's
        // caps query implementation), and instead install a capsfilter downstream
        // of the payloader with caps constructed from the relevant SDP media.
        let transceiver = webrtc_pad
            .pad
            .property::<gst_webrtc::WebRTCRTPTransceiver>("transceiver");
        transceiver.set_property("codec-preferences", None::<gst::Caps>);

        let s = caps.structure(0).unwrap();
        let mut filtered_s = gst::Structure::new_empty("application/x-rtp");

        filtered_s.extend(s.iter().filter_map(|(key, value)| {
            if key.starts_with("a-") {
                None
            } else if key.starts_with("extmap-")
                && value
                    .get::<&str>()
                    .is_ok_and(|v| v == "urn:ietf:params:rtp-hdrext:ssrc-audio-level")
            {
                // Workaround for the audio-level header extension: our extmap for this will usually be an array
                // with "vad=on", but browsers strip that (because it's the default) and just give us a string
                // with the uri. To make the capsfilter work, lets re-add the vad=on variant to the caps.
                let vad_on_array =
                    gst::Array::new(["", "urn:ietf:params:rtp-hdrext:ssrc-audio-level", "vad=on"])
                        .to_send_value();
                let list = gst::List::new([vad_on_array, value.to_owned()]).to_send_value();
                Some((key, list))
            } else {
                Some((key, value.to_owned()))
            }
        }));
        filtered_s.set("ssrc", webrtc_pad.ssrc);

        let caps = gst::Caps::builder_full().structure(filtered_s).build();

        pay_filter.set_property("caps", caps);

        if codec.is_video() {
            let video_info = gst_video::VideoInfo::from_caps(&webrtc_pad.in_caps)?;
            if let Some(mut enc) = VideoEncoder::new(
                &encoding_chain,
                video_info,
                &self.id,
                codec.caps.structure(0).unwrap().name(),
                transceiver,
                stream_name.to_string(),
            ) {
                match self.cc_info.heuristic {
                    WebRTCSinkCongestionControl::Disabled => {
                        // If congestion control is disabled, we simply use the highest
                        // known "safe" value for the bitrate.
                        let _ = enc.set_bitrate(
                            element,
                            self.cc_info.max_bitrate as i32,
                            self.enabled_mitigation_modes,
                        );
                        enc.transceiver.set_property("fec-percentage", 50u32);
                    }
                    WebRTCSinkCongestionControl::Homegrown => {
                        if let Some(congestion_controller) = self.congestion_controller.as_mut() {
                            if let Ok(bitrate) = enc.bitrate() {
                                congestion_controller.target_bitrate_on_delay += bitrate;
                                congestion_controller.target_bitrate_on_loss =
                                    congestion_controller.target_bitrate_on_delay;
                                enc.transceiver.set_property("fec-percentage", 0u32);
                            }
                        } else {
                            /* If congestion control is disabled, we simply use the highest
                             * known "safe" value for the bitrate. */
                            let _ = enc.set_bitrate(
                                element,
                                self.cc_info.max_bitrate as i32,
                                self.enabled_mitigation_modes,
                            );
                            enc.transceiver.set_property("fec-percentage", 50u32);
                        }
                    }
                    _ => enc.transceiver.set_property("fec-percentage", 0u32),
                }

                self.encoders.push(enc);

                if let Some(rtpgccbwe) = self.rtpgccbwe.as_ref() {
                    let max_bitrate = self.cc_info.max_bitrate * (self.encoders.len() as u32);
                    let min_bitrate = self.cc_info.min_bitrate * (self.encoders.len() as u32);
                    rtpgccbwe.set_property("max-bitrate", max_bitrate);
                    rtpgccbwe.set_property("min-bitrate", min_bitrate);
                }
            }
        }

        let appsrc = appsrc.downcast::<gst_app::AppSrc>().unwrap();
        gst_utils::StreamProducer::configure_consumer(&appsrc);
        self.pipeline
            .sync_children_states()
            .with_context(|| format!("Connecting input stream for {}", self.peer_id))?;

        encoding_chain.pay_filter.link(&pay_filter)?;

        let srcpad = pay_filter.static_pad("src").unwrap();

        srcpad
            .link(&webrtc_pad.pad)
            .with_context(|| format!("Connecting input stream for {}", self.peer_id))?;

        match producer.add_consumer(&appsrc) {
            Ok(link) => {
                self.links.insert(webrtc_pad.ssrc, link);
                Ok(())
            }
            Err(err) => Err(anyhow!("Could not link producer: {:?}", err)),
        }
    }
}

impl Drop for PipelineWrapper {
    fn drop(&mut self) {
        let _ = self.0.set_state(gst::State::Null);
    }
}

impl InputStream {
    /// Called when transitioning state up to Paused
    fn prepare(&mut self, element: &super::BaseWebRTCSink) -> Result<(), Error> {
        let clocksync = make_element("clocksync", None)?;
        let appsink = make_element("appsink", None)?
            .downcast::<gst_app::AppSink>()
            .unwrap();

        element.add(&clocksync).unwrap();
        element.add(&appsink).unwrap();

        clocksync
            .link(&appsink)
            .with_context(|| format!("Linking input stream {}", self.sink_pad.name()))?;

        element
            .sync_children_states()
            .with_context(|| format!("Linking input stream {}", self.sink_pad.name()))?;

        self.sink_pad
            .set_target(Some(&clocksync.static_pad("sink").unwrap()))
            .unwrap();

        self.producer = Some(StreamProducer::from(&appsink));

        Ok(())
    }

    /// Called when transitioning state back down to Ready
    fn unprepare(&mut self, element: &super::BaseWebRTCSink) {
        self.sink_pad.set_target(None::<&gst::Pad>).unwrap();

        if let Some(clocksync) = self.clocksync.take() {
            element.remove(&clocksync).unwrap();
            clocksync.set_state(gst::State::Null).unwrap();
        }

        if let Some(producer) = self.producer.take() {
            let appsink = producer.appsink().upcast_ref::<gst::Element>();
            element.remove(appsink).unwrap();
            appsink.set_state(gst::State::Null).unwrap();
        }
    }

    fn create_discovery(&self) -> DiscoveryInfo {
        DiscoveryInfo::new(
            self.in_caps.clone().expect(
                "We should never create a discovery for a stream that doesn't have caps set",
            ),
        )
    }

    fn msid(&self) -> Option<String> {
        self.sink_pad.property("msid")
    }
}

impl NavigationEventHandler {
    fn new(element: &super::BaseWebRTCSink, webrtcbin: &gst::Element, session_id: &str) -> Self {
        gst::info!(CAT, obj = element, "Creating navigation data channel");
        let channel = webrtcbin.emit_by_name::<WebRTCDataChannel>(
            "create-data-channel",
            &[
                &"input",
                &gst::Structure::builder("config")
                    .field("priority", gst_webrtc::WebRTCPriorityType::High)
                    .build(),
            ],
        );

        let session_id = session_id.to_string();

        Self((
            Some(channel.connect_closure(
                "on-message-string",
                false,
                glib::closure!(
                    #[watch]
                    element,
                    #[strong]
                    session_id,
                    move |_channel: &WebRTCDataChannel, msg: &str| {
                        create_navigation_event(element, msg, &session_id);
                    }
                ),
            )),
            channel,
        ))
    }
}

impl Drop for NavigationEventHandler {
    fn drop(&mut self) {
        self.0 .1.disconnect(self.0 .0.take().unwrap());
        self.0 .1.close();
    }
}

impl ControlRequestHandler {
    fn new(element: &super::BaseWebRTCSink, webrtcbin: &gst::Element, session_id: &str) -> Self {
        let channel = webrtcbin.emit_by_name::<WebRTCDataChannel>(
            "create-data-channel",
            &[
                &"control",
                &gst::Structure::builder("config")
                    .field("priority", gst_webrtc::WebRTCPriorityType::High)
                    .build(),
            ],
        );

        let session_id = session_id.to_string();

        Self((
            Some(channel.connect_closure(
                "on-message-string",
                false,
                glib::closure!(
                    #[watch]
                    element,
                    #[strong]
                    session_id,
                    move |channel: &WebRTCDataChannel, msg: &str| {
                        match handle_control_event(element, msg, &session_id) {
                            Err(err) => {
                                gst::error!(CAT, "Failed to handle control event: {err:?}");
                            }
                            Ok(msg) => match serde_json::to_string(&msg).ok() {
                                Some(s) => {
                                    if let Err(err) = channel.send_string_full(Some(s.as_str())) {
                                        gst::error!(
                                            CAT,
                                            obj = element,
                                            "Failed sending control request to peer: {err}",
                                        );
                                    }
                                }
                                None => {
                                    gst::error!(
                                        CAT,
                                        obj = element,
                                        "Failed to serialize control response",
                                    );
                                }
                            },
                        }
                    }
                ),
            )),
            channel,
        ))
    }
}

impl Drop for ControlRequestHandler {
    fn drop(&mut self) {
        self.0 .1.disconnect(self.0 .0.take().unwrap());
        self.0 .1.close();
    }
}

/// How to configure RTP extensions for payloaders, if at all
enum ExtensionConfigurationType {
    /// Skip configuration, do not add any extensions
    Skip,
    /// Configure extensions and assign IDs automatically, based on already enabled extensions
    Auto,
    /// Configure extensions, use specific ids that were provided
    Apply { twcc_id: u32 },
}

type SessionSetupResult = (gst::Element, EncodingChain, gst::Caps, Codec, String);

impl BaseWebRTCSink {
    fn configure_congestion_control(
        &self,
        payloader: &gst::Element,
        codec: &Codec,
        extension_configuration_type: ExtensionConfigurationType,
    ) -> Result<(), Error> {
        if let ExtensionConfigurationType::Skip = extension_configuration_type {
            return Ok(());
        }

        let settings = self.settings.lock().unwrap();

        if codec.is_video() {
            if let Some(enc_name) = codec.encoder_name().as_deref() {
                if !VideoEncoder::is_bitrate_supported(enc_name) {
                    gst::error!(
                        CAT,
                        imp = self,
                        "Bitrate handling is not supported yet for {enc_name}"
                    );

                    return Ok(());
                }
            }
        }

        if settings.cc_info.heuristic == WebRTCSinkCongestionControl::Disabled {
            return Ok(());
        }

        let Some(twcc_id) = self.pick_twcc_extension_id(payloader, extension_configuration_type)
        else {
            return Ok(());
        };

        gst::debug!(
            CAT,
            obj = payloader,
            "Mapping TWCC extension to ID {}",
            twcc_id
        );

        /* We only enforce TWCC in the offer caps, once a remote description
         * has been set it will get automatically negotiated. This is necessary
         * because the implementor in Firefox had apparently not understood the
         * concept of *transport-wide* congestion control, and firefox doesn't
         * provide feedback for audio packets.
         */
        if let Some(twcc_extension) = gst_rtp::RTPHeaderExtension::create_from_uri(RTP_TWCC_URI) {
            twcc_extension.set_id(twcc_id);
            payloader.emit_by_name::<()>("add-extension", &[&twcc_extension]);
        } else {
            anyhow::bail!("Failed to add TWCC extension, make sure 'gst-plugins-good:rtpmanager' is installed");
        }

        Ok(())
    }

    fn has_connected_payloader_setup_slots(&self) -> bool {
        use glib::{signal, subclass};

        let signal_id =
            subclass::signal::SignalId::lookup("payloader-setup", BaseWebRTCSink::type_()).unwrap();

        signal::signal_has_handler_pending(
            self.obj().upcast_ref::<gst::Object>(),
            signal_id,
            None,
            false,
        )
    }

    /// Returns Some with an available ID for TWCC extension or None if it's already configured
    fn pick_twcc_extension_id(
        &self,
        payloader: &gst::Element,
        extension_configuration_type: ExtensionConfigurationType,
    ) -> Option<u32> {
        match extension_configuration_type {
            ExtensionConfigurationType::Auto => {
                // GstRTPBasePayload::extensions property is only available since GStreamer 1.24
                if !payloader.has_property_with_type("extensions", gst::Array::static_type()) {
                    if self.has_connected_payloader_setup_slots() {
                        gst::warning!(CAT, imp = self, "'extensions' property is not available: TWCC extension ID will default to 1. \
        Application code must ensure to pick non-conflicting IDs for any additionally configured extensions. \
        Please consider updating GStreamer to 1.24.");
                    }

                    return Some(1);
                }

                let enabled_extensions: gst::Array = payloader.property("extensions");

                let twcc = enabled_extensions
                    .iter()
                    .find(|value| {
                        let value = value.get::<gst_rtp::RTPHeaderExtension>().unwrap();

                        match value.uri() {
                            Some(v) => v == RTP_TWCC_URI,
                            None => false,
                        }
                    })
                    .map(|value| value.get::<gst_rtp::RTPHeaderExtension>().unwrap());

                if let Some(ext) = twcc {
                    gst::debug!(
                        CAT,
                        obj = payloader,
                        "TWCC extension is already mapped to id {} by application",
                        ext.id()
                    );
                    return None;
                }

                let ext_id = utils::find_smallest_available_ext_id(
                    enabled_extensions
                        .iter()
                        .map(|value| value.get::<gst_rtp::RTPHeaderExtension>().unwrap().id()),
                );

                Some(ext_id)
            }
            ExtensionConfigurationType::Apply { twcc_id } => Some(twcc_id),
            ExtensionConfigurationType::Skip => unreachable!(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn configure_payloader(
        &self,
        peer_id: &str,
        stream_name: &str,
        payloader: &gst::Element,
        codec: &Codec,
        ssrc: Option<u32>,
        caps: Option<&gst::Caps>,
        extension_configuration_type: ExtensionConfigurationType,
    ) -> Result<(), Error> {
        self.obj()
            .emit_by_name::<bool>("payloader-setup", &[&peer_id, &stream_name, &payloader]);

        payloader.set_property(
            "pt",
            codec
                .payload()
                .expect("Negotiated codec should always have pt set") as u32,
        );

        if let Some(ssrc) = ssrc {
            if let Some(pspec) = payloader.find_property("ssrc") {
                match pspec.value_type() {
                    glib::Type::I64 => {
                        payloader.set_property("ssrc", ssrc as i64);
                    }
                    glib::Type::U32 => {
                        payloader.set_property("ssrc", ssrc);
                    }
                    _ => {
                        gst::warning!(
                            CAT,
                            imp = self,
                            "Unsupported ssrc type (expected i64 or u32)"
                        );
                    }
                }
            } else {
                gst::warning!(
                    CAT,
                    imp = self,
                    "Failed to find 'ssrc' property on payloader"
                );
            }
        }

        if self.settings.lock().unwrap().do_clock_signalling {
            if let Some(caps) = caps {
                let clock = self
                    .obj()
                    .clock()
                    .expect("element added & pipeline Playing");

                if clock.is::<gst_net::NtpClock>() || clock.is::<gst_net::PtpClock>() {
                    // RFC 7273 defines the "mediaclk:direct" attribute as the RTP timestamp
                    // value at the clock's epoch (time of origin). It was initialised to
                    // 0 in the SDP offer.
                    //
                    // Let's set the payloader's offset so the RTP timestamps
                    // are generated accordingly.
                    let clock_rate =
                        caps.structure(0)
                            .unwrap()
                            .get::<i32>("clock-rate")
                            .context("Setting payloader offset")? as u64;
                    let basetime = self
                        .obj()
                        .base_time()
                        .expect("element added & pipeline Playing");
                    let Some(rtp_basetime) = basetime
                        .nseconds()
                        .mul_div_ceil(clock_rate, *gst::ClockTime::SECOND)
                    else {
                        anyhow::bail!("Failed to compute RTP base time. clock-rate: {clock_rate}");
                    };

                    payloader.set_property("timestamp-offset", (rtp_basetime & 0xffff_ffff) as u32);
                }
            }
        }

        self.configure_congestion_control(payloader, codec, extension_configuration_type)
    }

    fn generate_ssrc(&self, webrtc_pads: &HashMap<u32, WebRTCPad>) -> u32 {
        loop {
            let ret = fastrand::u32(..);

            if !webrtc_pads.contains_key(&ret) {
                gst::trace!(CAT, imp = self, "Selected ssrc {}", ret);
                return ret;
            }
        }
    }

    fn request_inactive_webrtcbin_pad(
        &self,
        webrtcbin: &gst::Element,
        webrtc_pads: &mut HashMap<u32, WebRTCPad>,
        is_video: bool,
    ) {
        let ssrc = self.generate_ssrc(webrtc_pads);
        let media_idx = webrtc_pads.len() as i32;

        let Some(pad) = webrtcbin.request_pad_simple(&format!("sink_{media_idx}")) else {
            gst::error!(CAT, imp = self, "Failed to request pad from webrtcbin");
            gst::element_imp_error!(
                self,
                gst::StreamError::Failed,
                ["Failed to request pad from webrtcbin"]
            );
            return;
        };

        let transceiver = pad.property::<gst_webrtc::WebRTCRTPTransceiver>("transceiver");

        transceiver.set_property(
            "direction",
            gst_webrtc::WebRTCRTPTransceiverDirection::Inactive,
        );

        let payloader_caps = gst::Caps::builder("application/x-rtp")
            .field("media", if is_video { "video" } else { "audio" })
            .build();

        transceiver.set_property("codec-preferences", &payloader_caps);

        webrtc_pads.insert(
            ssrc,
            WebRTCPad {
                pad,
                in_caps: gst::Caps::new_empty(),
                media_idx: media_idx as u32,
                ssrc,
                stream_name: None,
                payload: None,
            },
        );
    }

    async fn request_webrtcbin_pad(
        &self,
        webrtcbin: &gst::Element,
        stream: &mut InputStream,
        media: Option<&gst_sdp::SDPMediaRef>,
        settings: &Settings,
        webrtc_pads: &mut HashMap<u32, WebRTCPad>,
        codecs: &mut BTreeMap<i32, Codec>,
    ) {
        let ssrc = self.generate_ssrc(webrtc_pads);
        let media_idx = webrtc_pads.len() as i32;

        let mut payloader_caps = match media {
            Some(media) => {
                let discovery_info = stream.create_discovery();

                let codec = self
                    .select_codec(
                        &discovery_info,
                        media,
                        &stream.in_caps.as_ref().unwrap().clone(),
                        &stream.sink_pad.name(),
                        settings,
                    )
                    .await;

                match codec {
                    Some(codec) => {
                        gst::debug!(CAT, imp = self, "Selected {codec:?} for media {media_idx}");

                        codecs.insert(codec.payload().unwrap(), codec.clone());
                        codec.output_filter().unwrap()
                    }
                    None => {
                        gst::error!(CAT, imp = self, "No codec selected for media {media_idx}");

                        gst::Caps::new_empty()
                    }
                }
            }
            None => stream.out_caps.as_ref().unwrap().to_owned(),
        };

        if payloader_caps.is_empty() {
            self.request_inactive_webrtcbin_pad(webrtcbin, webrtc_pads, stream.is_video);
        } else {
            let payloader_caps_mut = payloader_caps.make_mut();
            payloader_caps_mut.set("ssrc", ssrc);

            if self.settings.lock().unwrap().do_clock_signalling {
                // Add RFC7273 attributes when using an NTP or PTP clock
                let clock = self
                    .obj()
                    .clock()
                    .expect("element added and pipeline playing");

                let ts_refclk = if clock.is::<gst_net::NtpClock>() {
                    gst::debug!(CAT, imp = self, "Found NTP clock");

                    let addr = clock.property::<String>("address");
                    let port = clock.property::<i32>("port");

                    Some(if port == 123 {
                        format!("ntp={addr}")
                    } else {
                        format!("ntp={addr}:{port}")
                    })
                } else if clock.is::<gst_net::PtpClock>() {
                    gst::debug!(CAT, imp = self, "Found PTP clock");

                    let clock_id = clock.property::<u64>("grandmaster-clock-id");
                    let domain = clock.property::<u32>("domain");

                    Some(format!(
                        "ptp=IEEE1588-2008:{:02x}-{:02x}-{:02x}-{:02x}-{:02x}-{:02x}-{:02x}-{:02x}{}",
                        (clock_id >> 56) & 0xff,
                        (clock_id >> 48) & 0xff,
                        (clock_id >> 40) & 0xff,
                        (clock_id >> 32) & 0xff,
                        (clock_id >> 24) & 0xff,
                        (clock_id >> 16) & 0xff,
                        (clock_id >> 8) & 0xff,
                        clock_id & 0xff,
                        if domain == 0 {
                            "".to_string()
                        } else {
                            format!(":{domain}")
                        },
                    ))
                } else {
                    None
                };

                if let Some(ts_refclk) = ts_refclk.as_deref() {
                    payloader_caps_mut.set("a-ts-refclk", Some(ts_refclk));
                    // Set the offset to 0, we will adjust the payloader offsets
                    // when the payloaders are available.
                    payloader_caps_mut.set("a-mediaclk", Some("direct=0"));
                } else {
                    payloader_caps_mut.set("a-ts-refclk", Some("local"));
                    payloader_caps_mut.set("a-mediaclk", Some("sender"));
                }
            }

            gst::info!(
                CAT,
                imp = self,
                "Requesting WebRTC pad with caps {}",
                payloader_caps
            );

            let Some(pad) = webrtcbin.request_pad_simple(&format!("sink_{media_idx}")) else {
                gst::error!(CAT, imp = self, "Failed to request pad from webrtcbin");
                gst::element_imp_error!(
                    self,
                    gst::StreamError::Failed,
                    ["Failed to request pad from webrtcbin"]
                );
                return;
            };

            if let Some(msid) = stream.msid() {
                gst::trace!(
                    CAT,
                    imp = self,
                    "forwarding msid={msid:?} to webrtcbin sinkpad"
                );
                pad.set_property("msid", &msid);
            }

            let transceiver = pad.property::<gst_webrtc::WebRTCRTPTransceiver>("transceiver");

            transceiver.set_property(
                "direction",
                gst_webrtc::WebRTCRTPTransceiverDirection::Sendonly,
            );

            transceiver.set_property("codec-preferences", &payloader_caps);

            if stream.sink_pad.name().starts_with("video_") {
                if settings.do_fec {
                    transceiver.set_property("fec-type", gst_webrtc::WebRTCFECType::UlpRed);
                }

                transceiver.set_property("do-nack", settings.do_retransmission);
            }

            webrtc_pads.insert(
                ssrc,
                WebRTCPad {
                    pad,
                    in_caps: stream.in_caps.as_ref().unwrap().clone(),
                    media_idx: media_idx as u32,
                    ssrc,
                    stream_name: Some(stream.sink_pad.name().to_string()),
                    payload: None,
                },
            );
        }
    }

    #[cfg(feature = "web_server")]
    fn spawn_web_server(
        settings: &Settings,
    ) -> Result<
        (
            tokio::sync::oneshot::Sender<()>,
            tokio::task::JoinHandle<()>,
        ),
        Error,
    > {
        use hyper_util::{rt::TokioExecutor, rt::TokioIo, server::conn::auto};

        let (tx, mut rx) = tokio::sync::oneshot::channel::<()>();

        let addr = settings.web_server_host_addr.socket_addrs(|| None).unwrap()[0];

        let settings = settings.clone();

        let jh = RUNTIME.spawn(async move {
            let route = match settings.web_server_path {
                Some(path) => warp::path(path)
                    .and(warp::fs::dir(settings.web_server_directory))
                    .boxed(),
                None => warp::get()
                    .and(warp::fs::dir(settings.web_server_directory))
                    .boxed(),
            };

            if let (Some(cert), Some(key)) = (settings.web_server_cert, settings.web_server_key) {
                let listener = TcpListener::bind(&addr).await.unwrap();
                let acceptor = create_tls_acceptor(&cert, &key).await.unwrap();
                let warp_service = warp::service(route);

                while let Ok((stream, address)) = listener.accept().await {
                    gst::debug!(CAT, "Connection accepted from address: {address:?}");

                    match tokio::time::timeout(TLS_HANDSHAKE_TIMEOUT, acceptor.accept(stream)).await
                    {
                        Ok(Ok(stream)) => {
                            let warp_service = warp_service.clone();
                            let stream_io = TokioIo::new(stream);
                            let builder = auto::Builder::new(TokioExecutor::new());
                            let service =
                                hyper_util::service::TowerToHyperService::new(warp_service);
                            let connection = builder.serve_connection(stream_io, service);

                            tokio::select! {
                                biased;
                                res = &mut rx => match res {
                                    Ok(_) => gst::debug!(CAT, "Server shut down signal received"),
                                    Err(e) => gst::error!(CAT, "{e:?}: Sender dropped"),
                                },
                                conn_res = connection => match conn_res {
                                    Ok(_) => (),
                                    Err(err) => {
                                        gst::error!(CAT, "Connection error: {err:?}");
                                        continue;
                                    }
                                }
                            }
                        }
                        Ok(Err(err)) => {
                            gst::error!(CAT, "TLS error: {err:?}");
                            break;
                        }
                        Err(elapsed) => {
                            gst::error!(CAT, "TLS timeout: {elapsed:?}");
                            break;
                        }
                    }
                }
            } else {
                warp::serve(route)
                    .bind(addr)
                    .await
                    .graceful(async move {
                        match rx.await {
                            Ok(_) => gst::debug!(CAT, "Server shut down signal received"),
                            Err(e) => gst::error!(CAT, "{e:?}: Sender dropped"),
                        }
                    })
                    .run()
                    .await;
            }
        });

        Ok((tx, jh))
    }

    /// Prepare for accepting consumers, by setting
    /// up StreamProducers for each of our sink pads
    fn prepare(&self) -> Result<(), Error> {
        gst::debug!(CAT, imp = self, "preparing");

        #[cfg(feature = "web_server")]
        let settings = self.settings.lock().unwrap();
        let mut state = self.state.lock().unwrap();

        state
            .streams
            .iter_mut()
            .try_for_each(|(_, stream)| stream.prepare(&self.obj()))?;

        #[cfg(feature = "web_server")]
        if settings.run_web_server {
            let (web_shutdown_tx, web_join_handle) = BaseWebRTCSink::spawn_web_server(&settings)?;
            state.web_shutdown_tx = Some(web_shutdown_tx);
            state.web_join_handle = Some(web_join_handle);
        }

        Ok(())
    }

    /// Unprepare by stopping consumers, then the signaller object.
    /// Might abort codec discovery
    fn unprepare(&self) -> Result<(), Error> {
        gst::info!(CAT, imp = self, "unpreparing");

        let settings = self.settings.lock().unwrap();
        let signaller = settings.signaller.clone();
        drop(settings);
        let mut state = self.state.lock().unwrap();

        let mut join_handles = vec![];

        #[cfg(feature = "web_server")]
        if let Some(web_shutdown_tx) = state.web_shutdown_tx.take() {
            let _ = web_shutdown_tx.send(());
            let web_join_handle = state.web_join_handle.take().expect("no web join handle");
            // wait for this later
            join_handles.push(
                async {
                    let _ = web_join_handle.await;
                }
                .boxed_local(),
            );
        }

        let session_ids: Vec<_> = state.sessions.keys().map(|k| k.to_owned()).collect();

        let sessions: Vec<_> = session_ids
            .iter()
            .filter_map(|id| state.end_session(&self.obj(), id))
            .collect();

        state
            .streams
            .iter_mut()
            .for_each(|(_, stream)| stream.unprepare(&self.obj()));

        let codecs_abort_handle = std::mem::take(&mut state.codecs_abort_handles);
        codecs_abort_handle.into_iter().for_each(|handle| {
            handle.abort();
        });

        gst::debug!(CAT, imp = self, "Waiting for codec discoveries to finish");
        let codecs_done_receiver = std::mem::take(&mut state.codecs_done_receivers);
        codecs_done_receiver.into_iter().for_each(|receiver| {
            join_handles.push(
                async {
                    let _ = receiver.await;
                }
                .boxed_local(),
            );
        });
        gst::debug!(CAT, imp = self, "No codec discovery is running anymore");

        state.codec_discovery_done = false;
        state.codecs = BTreeMap::new();

        let signaller_state = state.signaller_state;
        if state.signaller_state == SignallerState::Started {
            state.signaller_state = SignallerState::Stopped;
        }

        drop(state);

        // only wait for all handles after the state lock has been dropped.  Some of the futures may
        // be waiting on the state lock to make forward progress before being able to be cancelled
        // from calls above.
        for handle in join_handles {
            RUNTIME.block_on(handle);
        }

        gst::debug!(CAT, imp = self, "Ending sessions");
        for session in sessions {
            signaller.end_session(&session.0.lock().unwrap().id);
        }
        gst::debug!(CAT, imp = self, "All sessions have started finalizing");

        if signaller_state == SignallerState::Started {
            gst::info!(CAT, imp = self, "Stopping signaller");
            signaller.stop();
            gst::info!(CAT, imp = self, "Stopped signaller");
        }

        let finalizing_sessions = self.state.lock().unwrap().finalizing_sessions.clone();

        let (sessions, cvar) = &*finalizing_sessions;
        let mut sessions = sessions.lock().unwrap();
        while !sessions.is_empty() {
            sessions = cvar.wait(sessions).unwrap();
        }

        gst::debug!(CAT, imp = self, "All sessions are done finalizing");

        Ok(())
    }

    fn connect_signaller(&self, signaler: &Signallable) {
        let instance = &*self.obj();

        let _ = self.state.lock().unwrap().signaller_signals.insert(SignallerSignals {
            error: signaler.connect_closure(
                "error",
                false,
                glib::closure!(#[watch] instance, move |_signaler: glib::Object, error: String| {
                    gst::element_error!(
                        instance,
                        gst::StreamError::Failed,
                        ["Signalling error: {}", error]
                    );
                }),
            ),

            request_meta: signaler.connect_closure(
                "request-meta",
                false,
                glib::closure!(#[watch] instance, move |_signaler: glib::Object| -> Option<gst::Structure> {
                    let meta = instance.imp().settings.lock().unwrap().meta.clone();

                    meta
                }),
            ),

            session_requested: signaler.connect_closure(
                "session-requested",
                false,
                glib::closure!(#[watch] instance, move |_signaler: glib::Object, session_id: &str, peer_id: &str, offer: Option<&gst_webrtc::WebRTCSessionDescription>|{
                    if let Err(err) = instance.imp().start_session(session_id, peer_id, offer) {
                        gst::warning!(CAT, obj = instance, "{:?}", err);
                    }
                }),
            ),

            session_description: signaler.connect_closure(
                "session-description",
                false,
                glib::closure!(#[watch] instance, move |
                        _signaler: glib::Object,
                        session_id: &str,
                        session_description: &gst_webrtc::WebRTCSessionDescription| {

                        if session_description.type_() == gst_webrtc::WebRTCSDPType::Answer {
                            instance.imp().handle_sdp_answer(session_id, session_description);
                        } else {
                            gst::error!(CAT, obj = instance, "Unsupported SDP Type");
                        }
                    }
                ),
            ),

            handle_ice: signaler.connect_closure(
                "handle-ice",
                false,
                glib::closure!(#[watch] instance, move |
                        _signaler: glib::Object,
                        session_id: &str,
                        sdp_m_line_index: u32,
                        _sdp_mid: Option<String>,
                        candidate: &str| {
                        instance
                            .imp()
                            .handle_ice(session_id, Some(sdp_m_line_index), None, candidate);
                    }),
            ),

            session_ended: signaler.connect_closure(
                "session-ended",
                false,
                glib::closure!(#[watch] instance, move |_signaler: glib::Object, session_id: &str|{
                    if let Err(err) = instance.imp().remove_session(session_id, false) {
                        gst::warning!(CAT, obj = instance, "{}", err);
                    }
                    false
                }),
            ),

            shutdown: signaler.connect_closure(
                "shutdown",
                false,
                glib::closure!(#[watch] instance, move |_signaler: glib::Object|{
                    instance.imp().shutdown();
                }),
            ),
        });
    }

    /// When using a custom signaller
    pub fn set_signaller(&self, signaller: Signallable) -> Result<(), Error> {
        let sigobj = signaller.clone();
        let mut settings = self.settings.lock().unwrap();

        self.connect_signaller(&sigobj);
        settings.signaller = signaller;

        Ok(())
    }

    /// Called by the signaller when it wants to shut down gracefully
    fn shutdown(&self) {
        gst::info!(CAT, imp = self, "Shutting down");
        let _ = self
            .obj()
            .post_message(gst::message::Eos::builder().src(&*self.obj()).build());
    }

    fn on_offer_created(&self, offer: gst_webrtc::WebRTCSessionDescription, session_id: &str) {
        let settings = self.settings.lock().unwrap();
        let signaller = settings.signaller.clone();
        drop(settings);
        let state = self.state.lock().unwrap();

        if let Some(session) = state.sessions.get(session_id) {
            session
                .0
                .lock()
                .unwrap()
                .webrtcbin
                .emit_by_name::<()>("set-local-description", &[&offer, &None::<gst::Promise>]);
            drop(state);

            let maybe_munged_offer = if signaller
                .has_property_with_type("manual-sdp-munging", bool::static_type())
                && signaller.property("manual-sdp-munging")
            {
                // Don't munge, signaller will manage this
                offer
            } else {
                // Use the default munging mechanism (signal registered by user)
                signaller.munge_sdp(session_id, &offer)
            };
            signaller.send_sdp(session_id, &maybe_munged_offer);
        }
    }

    fn on_answer_created(&self, answer: gst_webrtc::WebRTCSessionDescription, session_id: &str) {
        let settings = self.settings.lock().unwrap();
        let signaller = settings.signaller.clone();
        drop(settings);

        let session = self.state.lock().unwrap().sessions.get(session_id).cloned();

        if let Some(session) = session {
            let mut session = session.0.lock().unwrap();
            let sdp = answer.sdp();

            session.sdp = Some(sdp.to_owned());

            for webrtc_pad in session.webrtc_pads.values_mut() {
                webrtc_pad.payload = sdp
                    .media(webrtc_pad.media_idx)
                    .and_then(|media| media.format(0))
                    .and_then(|format| format.parse::<i32>().ok());
            }

            session
                .webrtcbin
                .emit_by_name::<()>("set-local-description", &[&answer, &None::<gst::Promise>]);

            let maybe_munged_answer = if signaller
                .has_property_with_type("manual-sdp-munging", bool::static_type())
                && signaller.property("manual-sdp-munging")
            {
                // Don't munge, signaller will manage this
                answer
            } else {
                // Use the default munging mechanism (signal registered by user)
                signaller.munge_sdp(&session.id, &answer)
            };

            signaller.send_sdp(&session.id, &maybe_munged_answer);

            let session_id = session.id.clone();
            drop(session);

            self.on_remote_description_set(&session_id)
        }
    }

    fn on_remote_description_offer_set(&self, session_id: String) {
        let state = self.state.lock().unwrap();

        if let Some(session) = state.sessions.get(&session_id) {
            gst::debug!(
                CAT,
                imp = self,
                "Creating answer for session {}",
                session_id
            );
            let promise = gst::Promise::with_change_func(glib::clone!(
                #[weak(rename_to = this)]
                self,
                #[strong]
                session_id,
                move |reply| {
                    gst::debug!(CAT, imp = this, "Created answer for session {}", session_id);

                    let reply = match reply {
                        Ok(Some(reply)) => reply,
                        Ok(None) => {
                            gst::warning!(
                                CAT,
                                imp = this,
                                "Promise returned without a reply for {}",
                                session_id
                            );
                            let _ = this.remove_session(&session_id, true);
                            return;
                        }
                        Err(err) => {
                            gst::warning!(
                                CAT,
                                imp = this,
                                "Promise returned with an error for {}: {:?}",
                                session_id,
                                err
                            );
                            let _ = this.remove_session(&session_id, true);
                            return;
                        }
                    };

                    if let Ok(answer) = reply.value("answer").map(|answer| {
                        answer
                            .get::<gst_webrtc::WebRTCSessionDescription>()
                            .unwrap()
                    }) {
                        this.on_answer_created(answer, &session_id);
                    } else {
                        gst::warning!(
                            CAT,
                            imp = this,
                            "Reply without an answer for session {}: {:?}",
                            session_id,
                            reply
                        );
                        let _ = this.remove_session(&session_id, true);
                    }
                }
            ));

            let webrtcbin = session.0.lock().unwrap().webrtcbin.clone();

            webrtcbin.emit_by_name::<()>("create-answer", &[&None::<gst::Structure>, &promise]);
        }
    }

    async fn select_codec(
        &self,
        discovery_info: &DiscoveryInfo,
        media: &gst_sdp::SDPMediaRef,
        in_caps: &gst::Caps,
        stream_name: &str,
        settings: &Settings,
    ) -> Option<Codec> {
        let user_caps = match media.media() {
            Some("audio") => &settings.audio_caps,
            Some("video") => &settings.video_caps,
            _ => {
                unreachable!();
            }
        };

        // Here, we want to try the codecs proposed by the remote offerer
        // in the order requested by the user. For instance, if the offer
        // contained VP8, VP9 and H264 (in this order), but the video-caps
        // contained H264 and VP8 (in this order), we want to try H264 first,
        // skip VP9, then try VP8.
        //
        // If the user wants to simply use the offered order, they should be
        // able to set video-caps to ANY caps, though other tweaks are probably
        // required elsewhere to make this work in all cases (eg when we create
        // the offer).

        let mut ordered_codecs_and_caps: Vec<(gst::Caps, Vec<(Codec, gst::Caps)>)> = user_caps
            .iter()
            .map(|s| ([s.to_owned()].into_iter().collect(), Vec::new()))
            .collect();

        for (payload, mut caps) in media
            .formats()
            .filter_map(|format| format.parse::<i32>().ok())
            .filter_map(|payload| Some(payload).zip(media.caps_from_media(payload)))
        {
            let s = caps.make_mut().structure_mut(0).unwrap();

            s.filter_map_in_place(|quark, value| {
                if quark.as_str().starts_with("rtcp-fb-") {
                    None
                } else {
                    Some(value)
                }
            });
            s.set_name("application/x-rtp");

            let encoding_name = s.get::<String>("encoding-name").unwrap();

            if let Some(mut codec) = Codecs::find(&encoding_name) {
                if !codec.can_encode() {
                    continue;
                }

                codec.set_pt(payload);
                for (user_caps, codecs_and_caps) in ordered_codecs_and_caps.iter_mut() {
                    if codec.caps.is_subset(user_caps) {
                        codecs_and_caps.push((codec, caps));
                        break;
                    }
                }
            }
        }

        let mut twcc_idx = None;

        for attribute in media.attributes() {
            if attribute.key() == "extmap" {
                if let Some(value) = attribute.value() {
                    if let Some((idx_str, ext)) = value.split_once(' ') {
                        if ext == RTP_TWCC_URI {
                            if let Ok(idx) = idx_str.parse::<u32>() {
                                twcc_idx = Some(idx);
                            } else {
                                gst::warning!(
                                    CAT,
                                    imp = self,
                                    "Failed to parse twcc index: {idx_str}"
                                );
                            }
                        }
                    }
                }
            }
        }

        let futs = ordered_codecs_and_caps
            .iter()
            .flat_map(|(_, codecs_and_caps)| codecs_and_caps)
            .map(|(codec, caps)| async move {
                let extension_configuration_type = twcc_idx
                    .map(|twcc_id| ExtensionConfigurationType::Apply { twcc_id })
                    .unwrap_or(ExtensionConfigurationType::Skip);

                self.run_discovery_pipeline(
                    stream_name,
                    discovery_info,
                    codec.clone(),
                    in_caps.clone(),
                    caps,
                    extension_configuration_type,
                )
                .await
                .map(|s| {
                    let mut codec = codec.clone();
                    codec.set_output_filter([s].into_iter().collect());
                    codec
                })
            });

        /* Run sequentially to avoid NVENC collisions */
        for fut in futs {
            if let Ok(codec) = fut.await {
                return Some(codec);
            }
        }

        None
    }

    fn negotiate(&self, session_id: &str, offer: Option<&gst_webrtc::WebRTCSessionDescription>) {
        let state = self.state.lock().unwrap();

        gst::debug!(CAT, imp = self, "Negotiating for session {}", session_id);

        if let Some(session) = state.sessions.get(session_id) {
            let session = session.0.lock().unwrap();
            gst::trace!(CAT, imp = self, "WebRTC pads: {:?}", session.webrtc_pads);
            let webrtcbin = session.webrtcbin.clone();
            drop(session);

            if let Some(offer) = offer {
                let promise = gst::Promise::with_change_func(glib::clone!(
                    #[weak(rename_to = this)]
                    self,
                    #[to_owned]
                    session_id,
                    move |reply| {
                        gst::debug!(CAT, imp = this, "received reply {:?}", reply);
                        this.on_remote_description_offer_set(session_id);
                    }
                ));

                webrtcbin.emit_by_name::<()>("set-remote-description", &[&offer, &promise]);
            } else {
                gst::debug!(CAT, imp = self, "Creating offer for session {}", session_id);
                let promise = gst::Promise::with_change_func(glib::clone!(
                    #[weak(rename_to = this)]
                    self,
                    #[to_owned]
                    session_id,
                    move |reply| {
                        gst::debug!(CAT, imp = this, "Created offer for session {}", session_id);

                        let reply = match reply {
                            Ok(Some(reply)) => reply,
                            Ok(None) => {
                                gst::warning!(
                                    CAT,
                                    imp = this,
                                    "Promise returned without a reply for {}",
                                    session_id
                                );
                                let _ = this.remove_session(&session_id, true);
                                return;
                            }
                            Err(err) => {
                                gst::warning!(
                                    CAT,
                                    imp = this,
                                    "Promise returned with an error for {}: {:?}",
                                    session_id,
                                    err
                                );
                                let _ = this.remove_session(&session_id, true);
                                return;
                            }
                        };

                        if let Ok(offer) = reply.value("offer").map(|offer| {
                            offer.get::<gst_webrtc::WebRTCSessionDescription>().unwrap()
                        }) {
                            this.on_offer_created(offer, &session_id);
                        } else {
                            gst::warning!(
                                CAT,
                                "Reply without an offer for session {}: {:?}",
                                session_id,
                                reply
                            );
                            let _ = this.remove_session(&session_id, true);
                        }
                    }
                ));

                webrtcbin.emit_by_name::<()>("create-offer", &[&None::<gst::Structure>, &promise]);
            }
        } else {
            gst::debug!(
                CAT,
                imp = self,
                "consumer for session {} no longer exists (sessions: {:?}",
                session_id,
                state.sessions.keys()
            );
        }
    }

    fn on_ice_candidate(&self, session_id: String, sdp_m_line_index: u32, candidate: String) {
        let settings = self.settings.lock().unwrap();
        let signaller = settings.signaller.clone();
        drop(settings);
        signaller.add_ice(&session_id, &candidate, sdp_m_line_index, None)
    }

    fn send_buffer_metas(
        &self,
        state: &State,
        settings: &Settings,
        session_id: &str,
        buffer: &gst::BufferRef,
    ) {
        // This should probably never happen as the transform
        // function for the dye meta always copies, but we can't
        // rely on the behavior of all encoders / payloaders.
        let Ok(dye_meta) = gst::meta::CustomMeta::from_buffer(buffer, "webrtcsink-dye") else {
            return;
        };
        let stream_name = dye_meta.structure().get::<String>("stream-name").unwrap();
        let Some(mid) = state
            .session_stream_names
            .get(session_id)
            .and_then(|names| names.get(&stream_name))
        else {
            return;
        };

        let Some(session) = state.sessions.get(session_id) else {
            return;
        };
        let session = session.0.lock().unwrap();

        if let Some(ref handler) = session.control_events_handler {
            if handler.0 .1.ready_state() != gst_webrtc::WebRTCDataChannelState::Open {
                return;
            }

            for meta in utils::serialize_meta(buffer, &settings.forward_metas) {
                match serde_json::to_string(&utils::InfoMessage {
                    mid: mid.to_owned(),
                    info: utils::Info::Meta(meta),
                }) {
                    Ok(msg) => {
                        if let Err(err) = handler.0 .1.send_string_full(Some(msg.as_str())) {
                            gst::error!(CAT, imp = self, "Failed sending meta to peer: {err}",);
                        }
                    }
                    Err(err) => {
                        gst::warning!(CAT, imp = self, "Failed to serialize info message: {err:?}",);
                    }
                }
            }
        }
    }

    /// Called by the signaller to add a new session
    fn start_session(
        &self,
        session_id: &str,
        peer_id: &str,
        offer: Option<&gst_webrtc::WebRTCSessionDescription>,
    ) -> Result<(), WebRTCSinkError> {
        let pipeline = gst::Pipeline::builder()
            .name(format!("session-pipeline-{session_id}"))
            .build();

        self.obj()
            .emit_by_name::<()>("consumer-pipeline-created", &[&peer_id, &pipeline]);

        let settings = self.settings.lock().unwrap();
        let mut state = self.state.lock().unwrap();
        let peer_id = peer_id.to_string();
        let session_id = session_id.to_string();
        let element = self.obj().clone();

        if state.sessions.contains_key(&session_id) {
            return Err(WebRTCSinkError::DuplicateSessionId(session_id));
        }

        gst::info!(
            CAT,
            obj = element,
            "Adding session: {} for peer: {}",
            session_id,
            peer_id,
        );

        let webrtcbin = make_element("webrtcbin", Some(&format!("webrtcbin-{session_id}")))
            .map_err(|err| WebRTCSinkError::SessionPipelineError {
                session_id: session_id.clone(),
                peer_id: peer_id.clone(),
                details: err.to_string(),
            })?;

        webrtcbin.set_property_from_str("bundle-policy", "max-bundle");
        webrtcbin.set_property("ice-transport-policy", settings.ice_transport_policy);

        if let Some(stun_server) = settings.stun_server.as_ref() {
            webrtcbin.set_property("stun-server", stun_server);
        }

        for turn_server in settings.turn_servers.iter() {
            webrtcbin.emit_by_name::<bool>("add-turn-server", &[&turn_server]);
        }

        let rtpgccbwe = match settings.cc_info.heuristic {
            WebRTCSinkCongestionControl::GoogleCongestionControl => {
                let rtpgccbwe = match gst::ElementFactory::make("rtpgccbwe").build() {
                    Err(err) => {
                        glib::g_warning!(
                            "webrtcsink",
                            "The `rtpgccbwe` element is not available \
                            not doing any congestion control: {err:?}"
                        );
                        None
                    }
                    Ok(cc) => {
                        webrtcbin.connect_closure(
                            "request-aux-sender",
                            false,
                            glib::closure!(
                                #[watch]
                                element,
                                #[strong]
                                session_id,
                                #[strong]
                                cc,
                                move |_webrtcbin: gst::Element, _transport: gst::Object| {
                                    let settings = element.imp().settings.lock().unwrap();

                                    // TODO: Bind properties with @element's
                                    cc.set_properties(&[
                                        ("min-bitrate", &settings.cc_info.min_bitrate),
                                        ("estimated-bitrate", &settings.cc_info.start_bitrate),
                                        ("max-bitrate", &settings.cc_info.max_bitrate),
                                    ]);

                                    cc.connect_notify(
                                        Some("estimated-bitrate"),
                                        glib::clone!(
                                            #[weak]
                                            element,
                                            #[strong]
                                            session_id,
                                            move |bwe, pspec| {
                                                element.imp().set_bitrate(
                                                    &session_id,
                                                    bwe.property::<u32>(pspec.name()),
                                                );
                                            }
                                        ),
                                    );

                                    cc.clone()
                                }
                            ),
                        );

                        Some(cc)
                    }
                };

                webrtcbin.connect_closure(
                    "deep-element-added",
                    false,
                    glib::closure!(
                        #[watch]
                        element,
                        #[strong]
                        session_id,
                        move |_webrtcbin: gst::Element, _bin: gst::Bin, e: gst::Element| {
                            if e.factory().is_some_and(|f| f.name() == "rtprtxsend") {
                                if e.has_property_with_type("stuffing-kbps", i32::static_type()) {
                                    element.imp().set_rtptrxsend(&session_id, e);
                                } else {
                                    gst::warning!(
                                        CAT,
                                        "rtprtxsend doesn't have a `stuffing-kbps` \
                                    property, stuffing disabled"
                                    );
                                }
                            }
                        }
                    ),
                );

                rtpgccbwe
            }
            _ => None,
        };

        webrtcbin.connect_closure(
            "deep-element-added",
            false,
            glib::closure!(
                #[strong]
                session_id,
                #[watch]
                element,
                move |_webrtcbin: gst::Element, _bin: gst::Bin, e: gst::Element| {
                    if e.factory().is_some_and(|f| f.name() == "nicesink") {
                        let sinkpad = e.static_pad("sink").unwrap();

                        let session_id = session_id.clone();
                        let element_clone = element.downgrade();
                        sinkpad.add_probe(
                            gst::PadProbeType::BUFFER
                                | gst::PadProbeType::BUFFER_LIST
                                | gst::PadProbeType::EVENT_DOWNSTREAM,
                            move |_pad, info| {
                                let Some(element) = element_clone.upgrade() else {
                                    return gst::PadProbeReturn::Remove;
                                };
                                let this = element.imp();
                                let settings = this.settings.lock().unwrap();
                                if settings.forward_metas.is_empty() {
                                    return gst::PadProbeReturn::Ok;
                                }

                                let state = this.state.lock().unwrap();
                                match info.data {
                                    Some(gst::PadProbeData::Buffer(ref buffer)) => {
                                        this.send_buffer_metas(
                                            &state,
                                            &settings,
                                            &session_id,
                                            buffer,
                                        );
                                    }
                                    Some(gst::PadProbeData::BufferList(ref list)) => {
                                        for buffer in list.iter() {
                                            this.send_buffer_metas(
                                                &state,
                                                &settings,
                                                &session_id,
                                                buffer,
                                            );
                                        }
                                    }
                                    _ => (),
                                }
                                gst::PadProbeReturn::Ok
                            },
                        );
                    }
                }
            ),
        );

        pipeline.add(&webrtcbin).unwrap();

        webrtcbin.connect_closure(
            "on-ice-candidate",
            false,
            glib::closure!(
                #[watch]
                element,
                #[strong]
                session_id,
                move |_webrtcbin: &gst::Element, sdp_m_line_index: u32, candidate: String| {
                    let this = element.imp();
                    this.on_ice_candidate(session_id.to_string(), sdp_m_line_index, candidate);
                }
            ),
        );

        webrtcbin.connect_notify(
            Some("connection-state"),
            glib::clone!(
                #[weak]
                element,
                #[strong]
                peer_id,
                #[strong]
                session_id,
                move |webrtcbin, _pspec| {
                    let state = webrtcbin
                        .property::<gst_webrtc::WebRTCPeerConnectionState>("connection-state");

                    match state {
                        gst_webrtc::WebRTCPeerConnectionState::Failed => {
                            let this = element.imp();
                            gst::warning!(
                                CAT,
                                obj = element,
                                "Connection state for in session {} (peer {}) failed",
                                session_id,
                                peer_id
                            );
                            let _ = this.remove_session(&session_id, true);
                        }
                        _ => {
                            gst::log!(
                                CAT,
                                obj = element,
                                "Connection state in session {} (peer {}) changed: {:?}",
                                session_id,
                                peer_id,
                                state
                            );
                        }
                    }
                }
            ),
        );

        webrtcbin.connect_notify(
            Some("ice-connection-state"),
            glib::clone!(
                #[weak]
                element,
                #[strong]
                peer_id,
                #[strong]
                session_id,
                move |webrtcbin, _pspec| {
                    let state = webrtcbin
                        .property::<gst_webrtc::WebRTCICEConnectionState>("ice-connection-state");
                    let this = element.imp();

                    match state {
                        gst_webrtc::WebRTCICEConnectionState::Failed => {
                            gst::warning!(
                                CAT,
                                obj = element,
                                "Ice connection state in session {} (peer {}) failed",
                                session_id,
                                peer_id,
                            );
                            let _ = this.remove_session(&session_id, true);
                        }
                        _ => {
                            gst::log!(
                                CAT,
                                obj = element,
                                "Ice connection state in session {} (peer {}) changed: {:?}",
                                session_id,
                                peer_id,
                                state
                            );
                        }
                    }

                    if state == gst_webrtc::WebRTCICEConnectionState::Completed {
                        let state = this.state.lock().unwrap();

                        if let Some(session) = state.sessions.get(&session_id) {
                            let pads: Vec<gst::Pad> = session
                                .0
                                .lock()
                                .unwrap()
                                .webrtc_pads
                                .values()
                                .map(|p| p.pad.clone())
                                .collect();
                            for pad in pads {
                                if let Some(srcpad) = pad.peer() {
                                    srcpad.send_event(
                                        gst_video::UpstreamForceKeyUnitEvent::builder()
                                            .all_headers(true)
                                            .build(),
                                    );
                                }
                            }
                        }
                    }
                }
            ),
        );

        webrtcbin.connect_notify(
            Some("ice-gathering-state"),
            glib::clone!(
                #[weak]
                element,
                #[strong]
                peer_id,
                #[strong]
                session_id,
                move |webrtcbin, _pspec| {
                    let state = webrtcbin
                        .property::<gst_webrtc::WebRTCICEGatheringState>("ice-gathering-state");

                    gst::log!(
                        CAT,
                        obj = element,
                        "Ice gathering state in session {} (peer {}) changed: {:?}",
                        session_id,
                        peer_id,
                        state
                    );
                }
            ),
        );

        let session = SessionInner::new(
            session_id.clone(),
            pipeline.clone(),
            webrtcbin.clone(),
            peer_id.clone(),
            match settings.cc_info.heuristic {
                WebRTCSinkCongestionControl::Homegrown => Some(CongestionController::new(
                    &peer_id,
                    settings.cc_info.min_bitrate,
                    settings.cc_info.max_bitrate,
                )),
                _ => None,
            },
            rtpgccbwe,
            settings.cc_info,
            settings.enabled_mitigation_modes,
        );

        let rtpbin = webrtcbin
            .dynamic_cast_ref::<gst::ChildProxy>()
            .unwrap()
            .child_by_name("rtpbin")
            .unwrap();

        if session.congestion_controller.is_some() {
            let session_id_str = session_id.to_string();
            rtpbin.connect_closure(
                "on-new-ssrc",
                true,
                glib::closure!(
                    #[watch]
                    element,
                    move |rtpbin: gst::Object, session_id: u32, _src: u32| {
                        let rtp_session =
                            rtpbin.emit_by_name::<gst::Element>("get-session", &[&session_id]);

                        let mut state = element.imp().state.lock().unwrap();
                        if let Some(session) = state.sessions.get_mut(&session_id_str) {
                            let mut session = session.0.lock().unwrap();
                            if session.stats_sigid.is_none() {
                                let session_id_str = session_id_str.clone();
                                session.stats_sigid = Some(rtp_session.connect_notify(
                                    Some("twcc-stats"),
                                    glib::clone!(
                                        #[weak]
                                        element,
                                        move |sess, pspec| {
                                            // Run the Loss-based control algorithm on new peer TWCC feedbacks
                                            element.imp().process_loss_stats(
                                                &session_id_str,
                                                &sess.property::<gst::Structure>(pspec.name()),
                                            );
                                        }
                                    ),
                                ));
                            }
                        }
                    }
                ),
            );
        }

        let clock = element.clock();

        pipeline.use_clock(clock.as_ref());
        pipeline.set_start_time(gst::ClockTime::NONE);
        pipeline.set_base_time(element.base_time().unwrap());

        let mut bus_stream = CustomBusStream::new(
            &element,
            &pipeline,
            &format!("webrtcsink-session-{session_id}"),
        );
        let element_clone = element.downgrade();
        let pipeline_clone = pipeline.downgrade();
        let session_id_clone = session_id.clone();
        let offer_clone = offer.cloned();
        let peer_id_clone = peer_id.clone();

        RUNTIME.spawn(async move {
            while let Some(msg) = bus_stream.next().await {
                let Some(element) = element_clone.upgrade() else {
                    break;
                };
                let Some(pipeline) = pipeline_clone.upgrade() else {
                    break;
                };
                let this = element.imp();
                match msg.view() {
                    gst::MessageView::Error(err) => {
                        gst::error!(
                            CAT,
                            "session {} error: {}, details: {:?}",
                            session_id_clone,
                            err.error(),
                            err.debug()
                        );
                        let _ = this.remove_session(&session_id_clone, true);
                    }
                    gst::MessageView::Latency(..) => {
                        gst::info!(CAT, obj = pipeline, "Recalculating latency");
                        let _ = pipeline.recalculate_latency();
                    }
                    gst::MessageView::Eos(..) => {
                        gst::error!(
                            CAT,
                            "Unexpected end of stream in session {}",
                            session_id_clone,
                        );
                        let _ = this.remove_session(&session_id_clone, true);
                    }
                    gst::MessageView::StateChanged(state_changed) => {
                        if state_changed.src() == Some(pipeline.upcast_ref())
                            && state_changed.old() == gst::State::Ready
                            && state_changed.current() == gst::State::Paused
                        {
                            gst::info!(
                                CAT,
                                obj = pipeline,
                                "{peer_id_clone} pipeline reached PAUSED, negotiating"
                            );
                            // We don't connect to on-negotiation-needed, this in order to call the above
                            // signal without holding the state lock:
                            //
                            // Going to Ready triggers synchronous emission of the on-negotiation-needed
                            // signal, during which time the application may add a data channel, causing
                            // renegotiation, which we do not support at this time.
                            //
                            // This is completely safe, as we know that by now all conditions are gathered:
                            // webrtcbin is in the Paused state, and all its transceivers have codec_preferences.
                            this.negotiate(&session_id_clone, offer_clone.as_ref());

                            if let Err(err) = pipeline.set_state(gst::State::Playing) {
                                gst::warning!(
                                    CAT,
                                    obj = element,
                                    "Failed to bring {peer_id_clone} pipeline to PLAYING: {}",
                                    err
                                );
                                let _ = this.remove_session(&session_id_clone, true);
                            }
                        }
                    }
                    _ => (),
                }
            }
        });

        state.sessions.insert(
            session_id.to_string(),
            Session(Arc::new(Mutex::new(session))),
        );

        let mut streams: Vec<InputStream> = state.streams.values().cloned().collect();

        streams.sort_by_key(|s| s.serial);

        RUNTIME.spawn(glib::clone!(
            #[to_owned(rename_to = this)]
            self,
            #[strong(rename_to = offer)]
            offer.cloned(),
            async move {
                let settings_clone = this.settings.lock().unwrap().clone();
                let signaller = settings_clone.signaller.clone();

                let mut webrtc_pads: HashMap<u32, WebRTCPad> = HashMap::new();
                let mut codecs: BTreeMap<i32, Codec> = BTreeMap::new();

                if let Some(ref offer) = offer {
                    for media in offer.sdp().medias() {
                        let media_is_video = match media.media() {
                            Some("audio") => false,
                            Some("video") => true,
                            _ => {
                                continue;
                            }
                        };

                        if let Some(idx) = streams.iter().position(|s| {
                            let structname =
                                s.in_caps.as_ref().unwrap().structure(0).unwrap().name();
                            let stream_is_video = structname.starts_with("video/");

                            if !stream_is_video {
                                assert!(structname.starts_with("audio/"));
                            }

                            media_is_video == stream_is_video
                        }) {
                            let mut stream = streams.remove(idx);
                            this.request_webrtcbin_pad(
                                &webrtcbin,
                                &mut stream,
                                Some(media),
                                &settings_clone,
                                &mut webrtc_pads,
                                &mut codecs,
                            )
                            .await;
                        } else {
                            this.request_inactive_webrtcbin_pad(
                                &webrtcbin,
                                &mut webrtc_pads,
                                media_is_video,
                            );
                        }
                    }
                } else {
                    for mut stream in streams {
                        this.request_webrtcbin_pad(
                            &webrtcbin,
                            &mut stream,
                            None,
                            &settings_clone,
                            &mut webrtc_pads,
                            &mut codecs,
                        )
                        .await;
                    }
                }

                let enable_data_channel_navigation = settings_clone.enable_data_channel_navigation;
                let enable_control_data_channel = settings_clone.enable_control_data_channel;

                drop(settings_clone);

                {
                    let mut state = this.state.lock().unwrap();
                    if let Some(session) = state.sessions.get_mut(&session_id) {
                        let mut session = session.0.lock().unwrap();
                        session.webrtc_pads = webrtc_pads;
                        if offer.is_some() {
                            session.codecs = Some(codecs);
                        }
                    }
                }

                if let Err(err) = pipeline.set_state(gst::State::Ready) {
                    gst::warning!(
                        CAT,
                        obj = element,
                        "Failed to bring {peer_id} pipeline to READY: {}",
                        err
                    );
                    let _ = this.remove_session(&session_id, true);
                    return;
                }

                if enable_data_channel_navigation {
                    let mut state = this.state.lock().unwrap();
                    if let Some(session) = state.sessions.get_mut(&session_id) {
                        let mut session = session.0.lock().unwrap();
                        session.navigation_handler = Some(NavigationEventHandler::new(
                            &element,
                            &webrtcbin,
                            &session_id,
                        ));
                    }
                }

                if enable_control_data_channel {
                    let mut state = this.state.lock().unwrap();
                    if let Some(session) = state.sessions.get_mut(&session_id) {
                        let mut session = session.0.lock().unwrap();
                        session.control_events_handler = Some(ControlRequestHandler::new(
                            &element,
                            &webrtcbin,
                            &session_id,
                        ));
                    }
                }

                // This is intentionally emitted with the pipeline in the Ready state,
                // so that application code can create data channels at the correct
                // moment.
                element.emit_by_name::<()>("consumer-added", &[&peer_id, &webrtcbin]);
                signaller.emit_by_name::<()>("consumer-added", &[&peer_id, &webrtcbin]);
                signaller.emit_by_name::<()>("webrtcbin-ready", &[&peer_id, &webrtcbin]);

                // We now bring the state to PAUSED before negotiating, as connecting
                // input streams before that can create a race condition with our
                // configuring of the format on the app sources and the start()
                // implementation of base source resetting the format to bytes.
                if let Err(err) = pipeline.set_state(gst::State::Paused) {
                    gst::warning!(
                        CAT,
                        obj = element,
                        "Failed to bring {peer_id} pipeline to PAUSED: {}",
                        err
                    );
                    let _ = this.remove_session(&session_id, true);
                }
            }
        ));

        Ok(())
    }

    /// Called by the signaller to remove a consumer
    fn remove_session(&self, session_id: &str, signal: bool) -> Result<(), WebRTCSinkError> {
        let settings = self.settings.lock().unwrap();
        let signaller = settings.signaller.clone();
        drop(settings);
        let mut state = self.state.lock().unwrap();

        if !state.sessions.contains_key(session_id) {
            return Err(WebRTCSinkError::NoSessionWithId(session_id.to_string()));
        }

        if let Some(session) = state.end_session(&self.obj(), session_id) {
            drop(state);
            let session = session.0.lock().unwrap();
            signaller
                .emit_by_name::<()>("consumer-removed", &[&session.peer_id, &session.webrtcbin]);
            if signal {
                signaller.end_session(session_id);
            }
            self.obj()
                .emit_by_name::<()>("consumer-removed", &[&session.peer_id, &session.webrtcbin]);
        }

        Ok(())
    }

    fn process_loss_stats(&self, session_id: &str, stats: &gst::Structure) {
        let mut state = self.state.lock().unwrap();
        if let Some(session) = state.sessions.get_mut(session_id) {
            /* We need this two-step approach for split-borrowing */
            let mut session_guard = session.0.lock().unwrap();
            let session = session_guard.deref_mut();
            if let Some(congestion_controller) = session.congestion_controller.as_mut() {
                congestion_controller.loss_control(&self.obj(), stats, &mut session.encoders);
            }
            stats.clone_into(&mut session.stats);
        }
    }

    fn process_stats(&self, webrtcbin: gst::Element, session_id: &str) {
        let session_id = session_id.to_string();
        let promise = gst::Promise::with_change_func(glib::clone!(
            #[weak(rename_to = this)]
            self,
            #[strong]
            session_id,
            move |reply| {
                if let Ok(Some(stats)) = reply {
                    let mut state = this.state.lock().unwrap();
                    if let Some(session) = state.sessions.get_mut(&session_id) {
                        /* We need this two-step approach for split-borrowing */
                        let mut session_guard = session.0.lock().unwrap();
                        let session = session_guard.deref_mut();
                        if let Some(congestion_controller) = session.congestion_controller.as_mut()
                        {
                            congestion_controller.delay_control(
                                &this.obj(),
                                stats,
                                &mut session.encoders,
                            );
                        }
                        session.stats = stats.to_owned();
                    }
                }
            }
        ));

        webrtcbin.emit_by_name::<()>("get-stats", &[&None::<gst::Pad>, &promise]);
    }

    fn set_rtptrxsend(&self, session_id: &str, rtprtxsend: gst::Element) {
        let mut state = self.state.lock().unwrap();

        if let Some(session) = state.sessions.get_mut(session_id) {
            session.0.lock().unwrap().rtprtxsend = Some(rtprtxsend);
        }
    }

    fn set_bitrate(&self, session_id: &str, bitrate: u32) {
        let settings = self.settings.lock().unwrap();
        let mut state = self.state.lock().unwrap();

        if let Some(session) = state.sessions.get_mut(session_id) {
            let mut session = session.0.lock().unwrap();

            let n_encoders = session.encoders.len();

            let fec_ratio = {
                if settings.do_fec && bitrate > DO_FEC_THRESHOLD {
                    (bitrate as f64 - DO_FEC_THRESHOLD as f64)
                        / ((session.cc_info.max_bitrate as usize * n_encoders) as f64
                            - DO_FEC_THRESHOLD as f64)
                } else {
                    0f64
                }
            };

            let fec_percentage = fec_ratio * 50f64;
            let encoders_bitrate = (bitrate as f64) / (1. + (fec_percentage / 100.));

            let encoder_bitrate = (encoders_bitrate / (n_encoders as f64)) as i32;

            if let Some(rtpxsend) = session.rtprtxsend.as_ref() {
                rtpxsend.set_property("stuffing-kbps", (bitrate as f64 / 1000.) as i32);
            }

            let mut s_builder = gst::Structure::builder("webrtcsink/encoder-bitrates");
            for encoder in session.encoders.iter() {
                s_builder = s_builder.field(&encoder.stream_name, encoder_bitrate);
            }
            let s = s_builder.build();

            let updated_bitrates = self.obj().emit_by_name::<gst::Structure>(
                "define-encoder-bitrates",
                &[&session.peer_id, &(encoders_bitrate as i32), &s],
            );

            for encoder in session.encoders.iter_mut() {
                let defined_encoder_bitrate =
                    match updated_bitrates.get::<i32>(&encoder.stream_name) {
                        Ok(bitrate) => {
                            gst::log!(
                                CAT,
                                imp = self,
                                "using defined bitrate {bitrate} for encoder {}",
                                encoder.stream_name
                            );
                            bitrate
                        }
                        Err(e) => {
                            gst::log!(
                                CAT,
                                imp = self,
                                "Error in defined bitrate: {e}, falling back to default bitrate \
                            {encoder_bitrate} for encoder {}",
                                encoder.stream_name
                            );
                            encoder_bitrate
                        }
                    };

                if encoder
                    .set_bitrate(
                        &self.obj(),
                        defined_encoder_bitrate,
                        settings.enabled_mitigation_modes,
                    )
                    .is_ok()
                {
                    encoder
                        .transceiver
                        .set_property("fec-percentage", (fec_percentage as u32).min(100));
                }
            }
        }
    }

    fn setup_session_payloading_chain(
        &self,
        peer_id: &str,
        webrtc_pad: &WebRTCPad,
        codecs: &BTreeMap<i32, Codec>,
        sdp: &gst_sdp::SDPMessage,
        pipeline: &gst::Pipeline,
    ) -> Result<Option<SessionSetupResult>, Error> {
        let stream_name = match webrtc_pad.stream_name {
            Some(ref name) => name,
            None => {
                gst::info!(
                    CAT,
                    imp = self,
                    "Consumer {} not connecting any input stream for inactive media {}",
                    peer_id,
                    webrtc_pad.media_idx
                );
                return Ok(None);
            }
        };

        let appsrc = make_element("appsrc", Some(stream_name))?;
        pipeline.add(&appsrc).unwrap();

        let payload = webrtc_pad.payload.unwrap();

        let codec = codecs
            .get(&payload)
            .cloned()
            .ok_or_else(|| anyhow!("No codec for payload {}", payload))?;

        let output_caps = codec.output_filter().unwrap_or_else(gst::Caps::new_any);

        let PayloadChain {
            payloader,
            encoding_chain,
        } = PayloadChainBuilder::new(
            &webrtc_pad.in_caps,
            &output_caps,
            &codec,
            self.obj().emit_by_name::<Option<gst::Element>>(
                "request-encoded-filter",
                &[&Some(peer_id), &stream_name, &codec.caps],
            ),
        )
        .build(pipeline, &appsrc)?;

        if let Some(ref enc) = encoding_chain.encoder {
            self.obj()
                .emit_by_name::<bool>("encoder-setup", &[&peer_id.to_string(), &stream_name, &enc]);
        }

        let sdp_media = sdp.media(webrtc_pad.media_idx).unwrap();

        let mut global_caps = gst::Caps::new_empty_simple("application/x-unknown");

        sdp.attributes_to_caps(global_caps.get_mut().unwrap())
            .unwrap();
        sdp_media
            .attributes_to_caps(global_caps.get_mut().unwrap())
            .unwrap();

        let caps = sdp_media
            .caps_from_media(payload)
            .unwrap()
            .intersect(&global_caps);

        self.configure_payloader(
            peer_id,
            stream_name,
            &payloader,
            &codec,
            Some(webrtc_pad.ssrc),
            Some(&caps),
            ExtensionConfigurationType::Skip,
        )?;

        Ok(Some((
            appsrc,
            encoding_chain,
            caps,
            codec.clone(),
            stream_name.to_owned(),
        )))
    }

    fn on_remote_description_set(&self, session_id: &str) {
        let mut state_guard = self.state.lock().unwrap();
        let mut state = state_guard.deref_mut();
        let Some(session_clone) = state.sessions.get(session_id).map(|s| s.0.clone()) else {
            return;
        };
        let mut remove = false;
        let codecs = state.codecs.clone();

        let mut session = session_clone.lock().unwrap();

        for webrtc_pad in session.webrtc_pads.clone().values() {
            let transceiver = webrtc_pad
                .pad
                .property::<gst_webrtc::WebRTCRTPTransceiver>("transceiver");

            let Some(ref stream_name) = webrtc_pad.stream_name else {
                continue;
            };

            if let Some(mid) = transceiver.mid() {
                state
                    .session_mids
                    .entry(session.id.clone())
                    .or_default()
                    .insert(mid.to_string(), stream_name.clone());
                state
                    .session_stream_names
                    .entry(session.id.clone())
                    .or_default()
                    .insert(stream_name.clone(), mid.to_string());
            }

            if let Some(producer) = state
                .streams
                .get(stream_name)
                .and_then(|stream| stream.producer.clone())
            {
                drop(state_guard);

                let peer_id = session.peer_id.clone();
                let session_codecs = session.codecs.clone().unwrap_or_else(|| codecs.clone());
                let sdp = session.sdp.clone();
                let pipeline = session.pipeline.clone();

                drop(session);

                let res = match self.setup_session_payloading_chain(
                    &peer_id,
                    webrtc_pad,
                    &session_codecs,
                    sdp.as_ref().unwrap(),
                    &pipeline,
                ) {
                    Err(err) => {
                        gst::error!(
                            CAT,
                            imp = self,
                            "Failed to setup elements {} for session {}: {}",
                            stream_name,
                            session_id,
                            err
                        );
                        remove = true;
                        state_guard = self.state.lock().unwrap();
                        session = session_clone.lock().unwrap();
                        state = state_guard.deref_mut();
                        break;
                    }
                    Ok(Some(res)) => res,
                    Ok(None) => {
                        state_guard = self.state.lock().unwrap();
                        session = session_clone.lock().unwrap();
                        state = state_guard.deref_mut();
                        continue;
                    }
                };

                state_guard = self.state.lock().unwrap();
                session = session_clone.lock().unwrap();
                state = state_guard.deref_mut();

                if let Err(err) =
                    session.connect_input_stream(&self.obj(), &producer, webrtc_pad, res)
                {
                    gst::error!(
                        CAT,
                        imp = self,
                        "Failed to connect input stream {} for session {}: {}",
                        stream_name,
                        session.id,
                        err
                    );
                    remove = true;
                    break;
                }
            } else {
                gst::error!(
                    CAT,
                    imp = self,
                    "No producer to connect session {} to",
                    session.id,
                );
                remove = true;
                break;
            }
        }

        session.pipeline.debug_to_dot_file_with_ts(
            gst::DebugGraphDetails::all(),
            format!("webrtcsink-peer-{}-remote-description-set", session.id),
        );

        let this_weak = self.downgrade();
        let webrtcbin = session.webrtcbin.downgrade();
        let session_id_clone = session.id.clone();
        session.stats_collection_handle = Some(RUNTIME.spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));

            loop {
                interval.tick().await;
                if let (Some(webrtcbin), Some(this)) = (webrtcbin.upgrade(), this_weak.upgrade()) {
                    this.process_stats(webrtcbin, &session_id_clone);
                } else {
                    break;
                }
            }
        }));

        if remove {
            let _ = state.sessions.remove(&session.id);
            let session_id = session.id.clone();
            drop(session);
            state.finalize_session(&self.obj(), Session(session_clone.clone()));
            drop(state_guard);
            let settings = self.settings.lock().unwrap();
            let signaller = settings.signaller.clone();
            drop(settings);
            signaller.end_session(&session_id);
        }
    }

    /// Called by the signaller with an ice candidate
    fn handle_ice(
        &self,
        session_id: &str,
        sdp_m_line_index: Option<u32>,
        _sdp_mid: Option<String>,
        candidate: &str,
    ) {
        let state = self.state.lock().unwrap();

        let sdp_m_line_index = match sdp_m_line_index {
            Some(sdp_m_line_index) => sdp_m_line_index,
            None => {
                gst::warning!(CAT, imp = self, "No mandatory SDP m-line index");
                return;
            }
        };

        if let Some(session) = state.sessions.get(session_id) {
            let webrtcbin = session.0.lock().unwrap().webrtcbin.clone();
            gst::trace!(
                CAT,
                imp = self,
                "adding ice candidate for session {session_id}"
            );
            webrtcbin.emit_by_name::<()>("add-ice-candidate", &[&sdp_m_line_index, &candidate]);
        } else {
            gst::warning!(CAT, imp = self, "No consumer with ID {session_id}");
        }
    }

    fn handle_sdp_answer(&self, session_id: &str, desc: &gst_webrtc::WebRTCSessionDescription) {
        let mut state = self.state.lock().unwrap();

        if let Some(session) = state.sessions.get(session_id).map(|s| s.0.clone()) {
            let mut session = session.lock().unwrap();

            let sdp = desc.sdp();

            session.sdp = Some(sdp.to_owned());

            for webrtc_pad in session.webrtc_pads.values_mut() {
                let media_idx = webrtc_pad.media_idx;
                /* TODO: support partial answer, webrtcbin doesn't seem
                 * very well equipped to deal with this at the moment */
                if let Some(media) = sdp.media(media_idx) {
                    if media.attribute_val("inactive").is_some() {
                        let media_str = sdp
                            .media(webrtc_pad.media_idx)
                            .and_then(|media| media.as_text().ok());

                        gst::warning!(
                            CAT,
                            imp = self,
                            "consumer from session {} refused media {}: {:?}",
                            session_id,
                            media_idx,
                            media_str
                        );

                        drop(session);
                        if let Some(_session) = state.end_session(&self.obj(), session_id) {
                            drop(state);
                            let settings = self.settings.lock().unwrap();
                            let signaller = settings.signaller.clone();
                            drop(settings);
                            signaller.end_session(session_id);
                        }
                        return;
                    }
                }

                if let Some(payload) = sdp
                    .media(webrtc_pad.media_idx)
                    .and_then(|media| media.format(0))
                    .and_then(|format| format.parse::<i32>().ok())
                {
                    webrtc_pad.payload = Some(payload);
                } else {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "consumer from session {} did not provide valid payload for media index {} for session {}",
                        session_id,
                        media_idx,
                        session_id,
                    );

                    drop(session);
                    if let Some(_session) = state.end_session(&self.obj(), session_id) {
                        drop(state);
                        let settings = self.settings.lock().unwrap();
                        let signaller = settings.signaller.clone();
                        drop(settings);
                        signaller.end_session(session_id);
                    }

                    gst::warning!(CAT, imp = self, "Consumer did not provide valid payload for media session: {session_id} media_ix: {media_idx}");
                    return;
                }
            }

            let promise = gst::Promise::with_change_func(glib::clone!(
                #[weak(rename_to = this)]
                self,
                #[to_owned]
                session_id,
                move |reply| {
                    gst::debug!(CAT, imp = this, "received reply {:?}", reply);
                    this.on_remote_description_set(&session_id);
                }
            ));

            let webrtcbin = session.webrtcbin.clone();
            drop(session);
            webrtcbin.emit_by_name::<()>("set-remote-description", &[desc, &promise]);
        } else {
            gst::warning!(CAT, imp = self, "No consumer with ID {session_id}");
        }
    }

    async fn run_discovery_pipeline(
        &self,
        stream_name: &str,
        discovery_info: &DiscoveryInfo,
        codec: Codec,
        input_caps: gst::Caps,
        output_caps: &gst::Caps,
        extension_configuration_type: ExtensionConfigurationType,
    ) -> Result<gst::Structure, Error> {
        let pipe = PipelineWrapper(gst::Pipeline::default());

        let has_raw_input = has_raw_caps(&input_caps);
        let src = discovery_info.create_src();
        let mut elements = vec![src.clone().upcast::<gst::Element>()];
        let encoding_chain_src = if codec.is_video() && has_raw_input {
            elements.push(make_converter_for_video_caps(&input_caps, &codec)?);

            let capsfilter = make_element("capsfilter", Some("raw_capsfilter"))?;
            elements.push(capsfilter.clone());

            capsfilter
        } else {
            src.clone().upcast::<gst::Element>()
        };

        gst::debug!(
            CAT,
            imp = self,
            "Running discovery pipeline for input caps {input_caps} and output caps {output_caps} with codec {codec:?}"
        );

        gst::debug!(CAT, imp = self, "Running discovery pipeline");
        let elements_slice = &elements.iter().collect::<Vec<_>>();
        pipe.0.add_many(elements_slice).unwrap();
        gst::Element::link_many(elements_slice)
            .with_context(|| format!("Running discovery pipeline for caps {input_caps}"))?;

        let payload_chain_builder = PayloadChainBuilder::new(
            &src.caps()
                .expect("Caps should always be set when starting discovery"),
            output_caps,
            &codec,
            self.obj().emit_by_name::<Option<gst::Element>>(
                "request-encoded-filter",
                &[&Option::<String>::None, &stream_name, &codec.caps],
            ),
        );

        let PayloadChain {
            payloader,
            encoding_chain,
        } = payload_chain_builder.build(&pipe.0, &encoding_chain_src)?;

        if let Some(ref enc) = encoding_chain.encoder {
            self.obj().emit_by_name::<bool>(
                "encoder-setup",
                &[&"discovery".to_string(), &stream_name, &enc],
            );
        }

        self.configure_payloader(
            "discovery",
            stream_name,
            &payloader,
            &codec,
            None,
            None,
            extension_configuration_type,
        )?;

        let sink = gst_app::AppSink::builder()
            .callbacks(
                gst_app::AppSinkCallbacks::builder()
                    .new_event(|sink| {
                        let obj = sink.pull_object().ok();
                        if let Some(event) = obj.and_then(|o| o.downcast::<gst::Event>().ok()) {
                            if let gst::EventView::Caps(caps) = event.view() {
                                sink.post_message(gst::message::Application::new(
                                    gst::Structure::builder("payloaded_caps")
                                        .field("caps", caps.caps_owned())
                                        .build(),
                                ))
                                .expect("Could not send message");
                            }
                        }

                        true
                    })
                    .build(),
            )
            .build();
        pipe.0.add(sink.upcast_ref::<gst::Element>()).unwrap();
        encoding_chain
            .pay_filter
            .link(&sink)
            .with_context(|| format!("Running discovery pipeline for caps {input_caps}"))?;

        let mut stream = CustomBusStream::new(
            &self.obj(),
            &pipe.0,
            &format!("webrtcsink-discovery-{}", pipe.0.name()),
        );

        pipe.0
            .set_state(gst::State::Playing)
            .with_context(|| format!("Running discovery pipeline for caps {input_caps}"))?;

        {
            let mut state = self.state.lock().unwrap();
            state.queue_discovery(stream_name, discovery_info.clone());
        }

        let ret = {
            loop {
                if let Some(msg) = stream.next().await {
                    match msg.view() {
                        gst::MessageView::Error(err) => {
                            gst::warning!(CAT, imp = self, "Error in discovery pipeline: {err:#?}");
                            pipe.0.debug_to_dot_file_with_ts(
                                gst::DebugGraphDetails::all(),
                                format!("webrtcsink-discovery-{}-error", pipe.0.name()),
                            );
                            break Err(err.error().into());
                        }
                        gst::MessageView::Application(appmsg) => {
                            let caps = match appmsg.structure() {
                                Some(s) => {
                                    if s.name().as_str() != "payloaded_caps" {
                                        continue;
                                    }

                                    s.get::<gst::Caps>("caps").unwrap()
                                }
                                _ => continue,
                            };

                            gst::info!(CAT, imp = self, "Discovery pipeline got caps {caps:?}");
                            pipe.0.debug_to_dot_file_with_ts(
                                gst::DebugGraphDetails::all(),
                                format!("webrtcsink-discovery-{}-done", pipe.0.name()),
                            );

                            if let Some(s) = caps.structure(0) {
                                let mut s = s.to_owned();
                                s.remove_fields([
                                    "timestamp-offset",
                                    "seqnum-offset",
                                    "ssrc",
                                    "sprop-parameter-sets",
                                    "sprop-vps",
                                    "sprop-pps",
                                    "sprop-sps",
                                    "a-framerate",
                                ]);
                                s.set("payload", codec.payload().unwrap());
                                gst::debug!(
                                    CAT,
                                    imp = self,
                                    "Codec discovery pipeline for caps {input_caps} with codec {codec:?} succeeded: {s}"
                                );
                                break Ok(s);
                            } else {
                                break Err(anyhow!("Discovered empty caps"));
                            }
                        }
                        _ => {
                            continue;
                        }
                    }
                } else {
                    unreachable!()
                }
            }
        };

        let mut state = self.state.lock().unwrap();
        state.remove_discovery(stream_name, discovery_info);

        ret
    }

    async fn lookup_caps(
        &self,
        discovery_info: DiscoveryInfo,
        name: String,
        output_caps: gst::Caps,
        codecs: &Codecs,
    ) -> Result<(), Error> {
        let futs = if has_raw_caps(&discovery_info.caps) {
            let codecs = codecs.list_encoders();
            if codecs.is_empty() {
                return Err(anyhow!(
                    "No codec available for encoding stream {}, \
                                   check the webrtcsink logs and verify that \
                                   the rtp plugin is available",
                    name
                ));
            }

            let sink_caps = discovery_info.caps.clone();

            let is_video = match sink_caps.structure(0).unwrap().name().as_str() {
                "video/x-raw" => true,
                "audio/x-raw" => false,
                _ => anyhow::bail!("expected audio or video raw caps: {sink_caps}"),
            };

            codecs
                .iter()
                .filter(|codec| codec.is_video() == is_video)
                .map(|codec| {
                    self.run_discovery_pipeline(
                        &name,
                        &discovery_info,
                        codec.clone(),
                        sink_caps.clone(),
                        &output_caps,
                        ExtensionConfigurationType::Auto,
                    )
                })
                .collect()
        } else if let Some(codec) = codecs.find_for_payloadable_caps(&discovery_info.caps) {
            let mut caps = discovery_info.caps.clone();

            gst::info!(
                CAT,
                imp = self,
                "Stream is already in the {} format, we still need to payload it",
                codec.name
            );

            caps = cleanup_codec_caps(caps);

            vec![self.run_discovery_pipeline(
                &name,
                &discovery_info,
                codec,
                caps,
                &output_caps,
                ExtensionConfigurationType::Auto,
            )]
        } else {
            anyhow::bail!("Unsupported caps: {}", discovery_info.caps);
        };

        let mut payloader_caps = gst::Caps::new_empty();
        let payloader_caps_mut = payloader_caps.make_mut();

        for ret in futures::future::join_all(futs).await {
            match ret {
                Ok(s) => {
                    payloader_caps_mut.append_structure(s);
                }
                Err(err) => {
                    /* We don't consider this fatal, as long as we end up with one
                     * potential codec for each input stream
                     */
                    gst::warning!(CAT, imp = self, "Codec discovery pipeline failed: {}", err);
                }
            }
        }

        let mut state = self.state.lock().unwrap();
        if let Some(stream) = state.streams.get_mut(&name) {
            stream.out_caps = Some(payloader_caps.clone());
        }

        if payloader_caps.is_empty() {
            anyhow::bail!("No caps found for stream {name}");
        }

        Ok(())
    }

    fn gather_stats(&self) -> gst::Structure {
        gst::Structure::from_iter(
            "application/x-webrtcsink-stats",
            self.state
                .lock()
                .unwrap()
                .sessions
                .iter()
                .map(|(name, consumer)| {
                    (
                        name.as_str(),
                        consumer.0.lock().unwrap().gather_stats().to_send_value(),
                    )
                }),
        )
    }

    /// Check if the caps of a sink pad can be changed from `current` to `new` without requiring a WebRTC renegotiation
    fn input_caps_change_allowed(&self, current: &gst::CapsRef, new: &gst::CapsRef) -> bool {
        let mut current = cleanup_codec_caps(current.to_owned());
        let current = current.get_mut().unwrap();
        let mut new = cleanup_codec_caps(new.to_owned());
        let new = new.get_mut().unwrap();

        let Some(current) = current.structure_mut(0) else {
            return false;
        };
        let Some(new) = new.structure_mut(0) else {
            return false;
        };

        if current.name() != new.name() {
            return false;
        }

        // Allow changes of fields which should not be part of the SDP, and so can be updated without requiring
        // a renegotiation.
        let caps_type = current.name();
        if caps_type.starts_with("video/") {
            const VIDEO_ALLOWED_CHANGES: &[&str] = &[
                "width",
                "height",
                "framerate",
                "pixel-aspect-ratio",
                "colorimetry",
                "chroma-site",
            ];

            current.remove_fields(VIDEO_ALLOWED_CHANGES.iter().copied());
            new.remove_fields(VIDEO_ALLOWED_CHANGES.iter().copied());

            // Multiview can be part of SDP, but missing field should be treated the same as mono view.
            fn remove_multiview(s: &mut gst::StructureRef) {
                let is_mono = match s.get::<VideoMultiviewMode>("multiview-mode") {
                    Ok(mode) => mode == VideoMultiviewMode::Mono,
                    Err(_) => true,
                };
                if is_mono {
                    s.remove_fields(["multiview-mode", "multiview-flags"])
                }
            }
            remove_multiview(current);
            remove_multiview(new);
        } else if caps_type.starts_with("audio/") {
            // TODO
        }

        current == new
    }

    fn sink_event(
        &self,
        pad: &gst::Pad,
        element: &super::BaseWebRTCSink,
        event: gst::Event,
    ) -> bool {
        use gst::EventView;

        if let EventView::Caps(e) = event.view() {
            if let Some(caps) = pad.current_caps() {
                if !self.input_caps_change_allowed(&caps, e.caps()) {
                    gst::error!(
                        CAT,
                        obj = pad,
                        "Renegotiation is not supported (old: {}, new: {})",
                        caps,
                        e.caps()
                    );
                    return false;
                }
            }
            gst::info!(CAT, obj = pad, "Received caps event {:?}", e);

            let mut state = self.state.lock().unwrap();

            state.streams.iter_mut().for_each(|(_, stream)| {
                if stream.sink_pad.upcast_ref::<gst::Pad>() == pad {
                    // We do not want VideoInfo to consider max-framerate
                    // when computing fps, so we strip it away here
                    let mut caps = e.caps().to_owned();
                    {
                        let mut_caps = caps.get_mut().unwrap();
                        if let Some(s) = mut_caps.structure_mut(0) {
                            if s.has_name("video/x-raw") {
                                s.remove_field("max-framerate");
                            }
                        }
                    }
                    stream.in_caps = Some(caps.to_owned());
                }
            });

            if e.caps().structure(0).unwrap().name().starts_with("video/") {
                if let Ok(video_info) = gst_video::VideoInfo::from_caps(e.caps()) {
                    // update video encoder info used when downscaling/downsampling the input
                    let stream_name = pad.name().to_string();

                    for session in state.sessions.values() {
                        for encoder in session.0.lock().unwrap().encoders.iter_mut() {
                            if encoder.stream_name == stream_name {
                                encoder.halved_framerate =
                                    video_info.fps().mul(gst::Fraction::new(1, 2));
                                encoder.video_info = video_info.clone();
                            }
                        }
                    }
                }
            }
        }

        gst::Pad::event_default(pad, Some(element), event)
    }

    fn feed_discoveries(&self, stream_name: &str, buffer: &gst::Buffer) {
        let state = self.state.lock().unwrap();

        if let Some(discos) = state.discoveries.get(stream_name) {
            for discovery_info in discos.iter() {
                for src in discovery_info.srcs() {
                    if let Err(err) = src.push_buffer(buffer.clone()) {
                        gst::log!(CAT, obj = src, "Failed to push buffer: {}", err);
                    }
                }
            }
        }
    }

    fn start_stream_discovery_if_needed(&self, stream_name: &str) {
        let (codecs, discovery_info) = {
            let mut state = self.state.lock().unwrap();

            let discovery_info = {
                let stream = state.streams.get_mut(stream_name).unwrap();

                // Initial discovery already happened... nothing to do here.
                if stream.initial_discovery_started {
                    return;
                }

                stream.initial_discovery_started = true;

                stream.create_discovery()
            };

            let codecs = if !state.codecs.is_empty() {
                Codecs::from_map(&state.codecs)
            } else {
                drop(state);
                let settings = self.settings.lock().unwrap();
                let codecs = Codecs::list_encoders_and_payloaders(
                    settings.video_caps.iter().chain(settings.audio_caps.iter()),
                );

                state = self.state.lock().unwrap();
                state.codecs = codecs.to_map();
                codecs
            };

            (codecs, discovery_info)
        };

        let stream_name_clone = stream_name.to_owned();
        RUNTIME.spawn(glib::clone!(
            #[to_owned(rename_to = this)]
            self,
            #[strong]
            discovery_info,
            async move {
                let (fut, handle) = futures::future::abortable(this.lookup_caps(
                    discovery_info.clone(),
                    stream_name_clone.clone(),
                    gst::Caps::new_any(),
                    &codecs,
                ));

                let (codecs_done_sender, codecs_done_receiver) =
                    futures::channel::oneshot::channel();

                // Compiler isn't budged by dropping state before await,
                // so let's make a new scope instead.
                {
                    let mut state = this.state.lock().unwrap();
                    state.codecs_abort_handles.push(handle);
                    state.codecs_done_receivers.push(codecs_done_receiver);
                }

                match fut.await {
                    Ok(Err(err)) => {
                        gst::error!(CAT, imp = this, "Error running discovery: {err:?}");
                        gst::element_error!(
                            this.obj(),
                            gst::StreamError::CodecNotFound,
                            ["Failed to look up output caps: {err:?}"]
                        );
                    }
                    Ok(Ok(_)) => {
                        let settings = this.settings.lock().unwrap();
                        let mut state = this.state.lock().unwrap();
                        state.codec_discovery_done = state
                            .streams
                            .values()
                            .all(|stream| stream.out_caps.is_some());
                        let signaller = settings.signaller.clone();
                        drop(settings);
                        if state.should_start_signaller(&this.obj()) {
                            state.signaller_state = SignallerState::Started;
                            drop(state);
                            signaller.start();
                        }
                    }
                    _ => (),
                }

                let _ = codecs_done_sender.send(());
            }
        ));
    }

    fn dye_buffer(&self, mut buffer: gst::Buffer, stream_name: &str) -> Result<gst::Buffer, Error> {
        let buf_mut = buffer.make_mut();

        let mut m = gst::meta::CustomMeta::add(buf_mut, "webrtcsink-dye")?;

        m.mut_structure().set("stream-name", stream_name);

        Ok(buffer)
    }

    fn chain(
        &self,
        pad: &gst::GhostPad,
        mut buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        buffer = self
            .dye_buffer(buffer, pad.name().as_str())
            .map_err(|err| {
                gst::error!(CAT, obj = pad, "Failed to dye buffer: {err}");
                gst::FlowError::Error
            })?;
        self.start_stream_discovery_if_needed(pad.name().as_str());
        self.feed_discoveries(pad.name().as_str(), &buffer);

        gst::ProxyPad::chain_default(pad, Some(&*self.obj()), buffer)
    }
}

#[glib::object_subclass]
impl ObjectSubclass for BaseWebRTCSink {
    const NAME: &'static str = "GstBaseWebRTCSink";
    type Type = super::BaseWebRTCSink;
    type ParentType = gst::Bin;
    type Interfaces = (gst::ChildProxy, gst_video::Navigation);

    fn class_init(_klass: &mut Self::Class) {
        register_dye_meta();
    }
}

fn register_dye_meta() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::meta::CustomMeta::register_with_transform(
            "webrtcsink-dye",
            &[],
            move |outbuf, meta, _inbuf, _type| {
                let stream_name = meta.structure().get::<String>("stream-name").unwrap();
                if let Ok(mut m) = gst::meta::CustomMeta::add(outbuf, "webrtcsink-dye") {
                    m.mut_structure().set("stream-name", stream_name);
                    true
                } else {
                    false
                }
            },
        );
    });
}

unsafe impl<T: BaseWebRTCSinkImpl> IsSubclassable<T> for super::BaseWebRTCSink {}

pub(crate) trait BaseWebRTCSinkImpl:
    BinImpl + ObjectSubclass<Type: IsA<super::BaseWebRTCSink>>
{
}

impl ObjectImpl for BaseWebRTCSink {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecBoxed::builder::<gst::Caps>("video-caps")
                    .nick("Video encoder caps")
                    .blurb(&format!("Governs what video codecs will be proposed. Valid values: [{}]",
                        Codecs::video_codecs()
                            .map(|c| c.caps.to_string())
                            .join("; ")
                    ))
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Caps>("audio-caps")
                    .nick("Audio encoder caps")
                    .blurb(&format!("Governs what audio codecs will be proposed. Valid values: [{}]",
                        Codecs::audio_codecs()
                            .map(|c| c.caps.to_string())
                            .join("; ")
                    ))
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("stun-server")
                    .nick("STUN Server")
                    .blurb("The STUN server of the form stun://hostname:port")
                    .default_value(DEFAULT_STUN_SERVER)
                    .build(),
                gst::ParamSpecArray::builder("turn-servers")
                    .nick("List of TURN Servers to use")
                    .blurb("The TURN servers of the form <\"turn(s)://username:password@host:port\", \"turn(s)://username1:password1@host1:port1\">")
                    .element_spec(&glib::ParamSpecString::builder("turn-server")
                        .nick("TURN Server")
                        .blurb("The TURN server of the form turn(s)://username:password@host:port.")
                        .build()
                    )
                    .mutable_ready()
                    .build(),
                glib::ParamSpecEnum::builder_with_default("congestion-control", DEFAULT_CONGESTION_CONTROL)
                    .nick("Congestion control")
                    .blurb("Defines how congestion is controlled, if at all")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder("min-bitrate")
                    .nick("Minimal Bitrate")
                    .blurb("Minimal bitrate to use (in bit/sec) when computing it through the congestion control algorithm")
                    .minimum(1)
                    .maximum(u32::MAX)
                    .default_value(DEFAULT_MIN_BITRATE)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder("max-bitrate")
                    .nick("Maximum Bitrate")
                    .blurb("Maximum bitrate to use (in bit/sec) when computing it through the congestion control algorithm")
                    .minimum(1)
                    .maximum(u32::MAX)
                    .default_value(DEFAULT_MAX_BITRATE)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder("start-bitrate")
                    .nick("Start Bitrate")
                    .blurb("Start bitrate to use (in bit/sec)")
                    .minimum(1)
                    .maximum(u32::MAX)
                    .default_value(DEFAULT_START_BITRATE)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Structure>("stats")
                    .nick("Consumer statistics")
                    .blurb("Statistics for the current consumers")
                    .read_only()
                    .build(),
                glib::ParamSpecBoolean::builder("do-fec")
                    .nick("Do Forward Error Correction")
                    .blurb("Whether the element should negotiate and send FEC data")
                    .default_value(DEFAULT_DO_FEC)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("do-retransmission")
                    .nick("Do retransmission")
                    .blurb("Whether the element should offer to honor retransmission requests")
                    .default_value(DEFAULT_DO_RETRANSMISSION)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("do-clock-signalling")
                    .nick("Do clock signalling")
                    .blurb("Whether PTP or NTP clock & RTP/clock offset should be signalled according to RFC 7273")
                    .default_value(DEFAULT_DO_CLOCK_SIGNALLING)
                    .mutable_ready()
                    .build(),
                /**
                 * GstBaseWebRTCSink:enable-data-channel-navigation:
                 *
                 * Enable navigation events through a dedicated WebRTCDataChannel.
                 *
                 * Deprecated:plugins-rs-0.14.0: Use #GstBaseWebRTCSink:enable-control-data-channel
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
                 * Enable receiving arbitrary events through data channel.
                 *
                 * Since: plugins-rs-0.14.0
                 */
                glib::ParamSpecBoolean::builder("enable-control-data-channel")
                    .nick("Enable control data channel")
                    .blurb("Enable receiving arbitrary events through data channel")
                    .default_value(DEFAULT_ENABLE_CONTROL_DATA_CHANNEL)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Structure>("meta")
                    .nick("Meta")
                    .blurb("Free form metadata about the producer")
                    .build(),
                glib::ParamSpecEnum::builder_with_default("ice-transport-policy", DEFAULT_ICE_TRANSPORT_POLICY)
                    .nick("ICE Transport Policy")
                    .blurb("The policy to apply for ICE transport")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecObject::builder::<Signallable>("signaller")
                    .flags(glib::ParamFlags::READABLE | gst::PARAM_FLAG_MUTABLE_READY)
                    .blurb("The Signallable object to use to handle WebRTC Signalling")
                    .build(),
                /**
                 * GstBaseWebRTCSink:run-web-server:
                 *
                 * Whether the element should use [warp] to serve the folder at
                 * #GstBaseWebRTCSink:web-server-directory.
                 *
                 * Since: plugins-rs-0.14.0
                 */
                #[cfg(feature = "web_server")]
                glib::ParamSpecBoolean::builder("run-web-server")
                    .nick("Run web server")
                    .blurb("Whether the element should run a web server")
                    .default_value(DEFAULT_RUN_WEB_SERVER)
                    .mutable_ready()
                    .build(),
                /**
                 * GstBaseWebRTCSink:web-server-cert:
                 *
                 * The certificate to use when #GstBaseWebRTCSink:run-web-server
                 * is TRUE.
                 *
                 * Since: plugins-rs-0.14.0
                 */
                #[cfg(feature = "web_server")]
                glib::ParamSpecString::builder("web-server-cert")
                    .nick("Web server certificate")
                    .blurb(
                        "Path to TLS certificate the web server should use.
                        The certificate should be formatted as PEM",
                    )
                    .default_value(DEFAULT_WEB_SERVER_CERT)
                    .build(),
                /**
                 * GstBaseWebRTCSink:web-server-key:
                 *
                 * The private key to use when #GstBaseWebRTCSink:run-web-server
                 * is TRUE.
                 *
                 * Since: plugins-rs-0.14.0
                 */
                #[cfg(feature = "web_server")]
                glib::ParamSpecString::builder("web-server-key")
                    .nick("Web server private key")
                    .blurb("Path to private encryption key the web server should use.
                        The key should be formatted as PEM")
                    .default_value(DEFAULT_WEB_SERVER_KEY)
                    .build(),
                /**
                 * GstBaseWebRTCSink:web-server-path:
                 *
                 * The root path for the web server when #GstBaseWebRTCSink:run-web-server
                 * is TRUE.
                 *
                 * Since: plugins-rs-0.14.0
                 */
                #[cfg(feature = "web_server")]
                glib::ParamSpecString::builder("web-server-path")
                    .nick("Web server path")
                    .blurb("The root path for the web server")
                    .default_value(DEFAULT_WEB_SERVER_PATH)
                    .build(),
                /**
                 * GstBaseWebRTCSink:web-server-directory:
                 *
                 * The directory to serve when #GstBaseWebRTCSink:run-web-server
                 * is TRUE.
                 *
                 * Since: plugins-rs-0.14.0
                 */
                #[cfg(feature = "web_server")]
                glib::ParamSpecString::builder("web-server-directory")
                    .nick("Web server directory")
                    .blurb("The directory the web server should serve")
                    .default_value(DEFAULT_WEB_SERVER_DIRECTORY)
                    .build(),
                /**
                 * GstBaseWebRTCSink:web-server-host-addr:
                 *
                 * The address to listen on when #GstBaseWebRTCSink:run-web-server
                 * is TRUE.
                 *
                 * Since: plugins-rs-0.14.0
                 */
                #[cfg(feature = "web_server")]
                glib::ParamSpecString::builder("web-server-host-addr")
                    .nick("Web server host address")
                    .blurb("Address the web server should listen on")
                    .default_value(DEFAULT_WEB_SERVER_HOST_ADDR)
                    .build(),
                /**
                 * GstBaseWebRTCSink:forward-metas:
                 *
                 * Comma-separated list of buffer metas to forward over the
                 * control data channel, if any.
                 *
                 * Currently supported names are:
                 *
                 * - timecode
                 *
                 * Since: plugins-rs-0.14.0
                 */
                glib::ParamSpecString::builder("forward-metas")
                    .nick("Forward metas")
                    .blurb("Comma-separated list of buffer meta names to forward over the control data channel. Currently supported names are: timecode")
                    .default_value(DEFAULT_FORWARD_METAS)
                    .mutable_playing()
                    .build(),
                /**
                 * GstBaseWebRTCSink:enable-mitigation-modes:
                 *
                 * Whether the element should dynamically scale the source resolution
                 * based on the bitrate.
                 *
                 * Since: plugins-rs-0.14.0
                 */
                glib::ParamSpecFlags::builder("enable-mitigation-modes")
                    .nick("Enable dynamic scaling / sampling of the source resolution")
                    .blurb("Flags for whether the element should dynamically scale the source resolution and framerate based on the bitrate")
                    .default_value(DEFAULT_ENABLE_MITIGATION_MODES)
                    .mutable_playing()
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
                    .expect("type checked upstream")
                    .unwrap_or_else(gst::Caps::new_empty);
            }
            "audio-caps" => {
                let mut settings = self.settings.lock().unwrap();
                settings.audio_caps = value
                    .get::<Option<gst::Caps>>()
                    .expect("type checked upstream")
                    .unwrap_or_else(gst::Caps::new_empty);
            }
            "stun-server" => {
                let mut settings = self.settings.lock().unwrap();
                settings.stun_server = value
                    .get::<Option<String>>()
                    .expect("type checked upstream")
            }
            "turn-servers" => {
                let mut settings = self.settings.lock().unwrap();
                settings.turn_servers = value.get::<gst::Array>().expect("type checked upstream")
            }
            "congestion-control" => {
                let mut settings = self.settings.lock().unwrap();
                settings.cc_info.heuristic = value
                    .get::<WebRTCSinkCongestionControl>()
                    .expect("type checked upstream");
            }
            "min-bitrate" => {
                let mut settings = self.settings.lock().unwrap();
                settings.cc_info.min_bitrate = value.get::<u32>().expect("type checked upstream");
            }
            "max-bitrate" => {
                let mut settings = self.settings.lock().unwrap();
                settings.cc_info.max_bitrate = value.get::<u32>().expect("type checked upstream");
            }
            "start-bitrate" => {
                let mut settings = self.settings.lock().unwrap();
                settings.cc_info.start_bitrate = value.get::<u32>().expect("type checked upstream");
            }
            "do-fec" => {
                let mut settings = self.settings.lock().unwrap();
                settings.do_fec = value.get::<bool>().expect("type checked upstream");
            }
            "do-retransmission" => {
                let mut settings = self.settings.lock().unwrap();
                settings.do_retransmission = value.get::<bool>().expect("type checked upstream");
            }
            "do-clock-signalling" => {
                let mut settings = self.settings.lock().unwrap();
                settings.do_clock_signalling = value.get::<bool>().expect("type checked upstream");
            }
            "enable-data-channel-navigation" => {
                let mut settings = self.settings.lock().unwrap();
                settings.enable_data_channel_navigation =
                    value.get::<bool>().expect("type checked upstream");
            }
            "enable-control-data-channel" => {
                let mut settings = self.settings.lock().unwrap();
                settings.enable_control_data_channel =
                    value.get::<bool>().expect("type checked upstream");
            }
            "meta" => {
                let mut settings = self.settings.lock().unwrap();
                settings.meta = value
                    .get::<Option<gst::Structure>>()
                    .expect("type checked upstream")
            }
            "ice-transport-policy" => {
                let mut settings = self.settings.lock().unwrap();
                settings.ice_transport_policy = value
                    .get::<WebRTCICETransportPolicy>()
                    .expect("type checked upstream");
            }
            #[cfg(feature = "web_server")]
            "run-web-server" => {
                let mut settings = self.settings.lock().unwrap();
                settings.run_web_server = value.get::<bool>().expect("type checked upstream");
            }
            #[cfg(feature = "web_server")]
            "web-server-cert" => {
                let mut settings = self.settings.lock().unwrap();
                settings.web_server_cert = value
                    .get::<Option<String>>()
                    .expect("type checked upstream")
            }
            #[cfg(feature = "web_server")]
            "web-server-key" => {
                let mut settings = self.settings.lock().unwrap();
                settings.web_server_key = value
                    .get::<Option<String>>()
                    .expect("type checked upstream")
            }
            #[cfg(feature = "web_server")]
            "web-server-path" => {
                let mut settings = self.settings.lock().unwrap();
                settings.web_server_path = value
                    .get::<Option<String>>()
                    .expect("type checked upstream")
            }
            #[cfg(feature = "web_server")]
            "web-server-directory" => {
                let mut settings = self.settings.lock().unwrap();
                settings.web_server_directory =
                    value.get::<String>().expect("type checked upstream")
            }
            #[cfg(feature = "web_server")]
            "web-server-host-addr" => {
                let mut settings = self.settings.lock().unwrap();

                let host_addr = match url::Url::parse(
                    &value.get::<String>().expect("type checked upstream"),
                ) {
                    Err(e) => {
                        gst::error!(CAT, "Couldn't set the host address as {e:?}, fallback to the default value {DEFAULT_WEB_SERVER_HOST_ADDR:?}");
                        return;
                    }

                    Ok(addr) => addr,
                };
                settings.web_server_host_addr = host_addr;
            }
            "forward-metas" => {
                let mut settings = self.settings.lock().unwrap();
                settings.forward_metas = value
                    .get::<String>()
                    .expect("type checked upstream")
                    .split(",")
                    .map(String::from)
                    .collect();
            }
            "enable-mitigation-modes" => {
                let mut settings = self.settings.lock().unwrap();
                settings.enabled_mitigation_modes = value
                    .get::<WebRTCSinkMitigationMode>()
                    .expect("type checked upstream");
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
            "congestion-control" => {
                let settings = self.settings.lock().unwrap();
                settings.cc_info.heuristic.to_value()
            }
            "stun-server" => {
                let settings = self.settings.lock().unwrap();
                settings.stun_server.to_value()
            }
            "turn-servers" => {
                let settings = self.settings.lock().unwrap();
                settings.turn_servers.to_value()
            }
            "min-bitrate" => {
                let settings = self.settings.lock().unwrap();
                settings.cc_info.min_bitrate.to_value()
            }
            "max-bitrate" => {
                let settings = self.settings.lock().unwrap();
                settings.cc_info.max_bitrate.to_value()
            }
            "start-bitrate" => {
                let settings = self.settings.lock().unwrap();
                settings.cc_info.start_bitrate.to_value()
            }
            "do-fec" => {
                let settings = self.settings.lock().unwrap();
                settings.do_fec.to_value()
            }
            "do-retransmission" => {
                let settings = self.settings.lock().unwrap();
                settings.do_retransmission.to_value()
            }
            "do-clock-signalling" => {
                let settings = self.settings.lock().unwrap();
                settings.do_clock_signalling.to_value()
            }
            "enable-data-channel-navigation" => {
                let settings = self.settings.lock().unwrap();
                settings.enable_data_channel_navigation.to_value()
            }
            "enable-control-data-channel" => {
                let settings = self.settings.lock().unwrap();
                settings.enable_control_data_channel.to_value()
            }
            "stats" => self.gather_stats().to_value(),
            "meta" => {
                let settings = self.settings.lock().unwrap();
                settings.meta.to_value()
            }
            "ice-transport-policy" => {
                let settings = self.settings.lock().unwrap();
                settings.ice_transport_policy.to_value()
            }
            "signaller" => self.settings.lock().unwrap().signaller.to_value(),
            #[cfg(feature = "web_server")]
            "run-web-server" => {
                let settings = self.settings.lock().unwrap();
                settings.run_web_server.to_value()
            }
            #[cfg(feature = "web_server")]
            "web-server-cert" => {
                let settings = self.settings.lock().unwrap();
                settings.web_server_cert.to_value()
            }
            #[cfg(feature = "web_server")]
            "web-server-key" => {
                let settings = self.settings.lock().unwrap();
                settings.web_server_key.to_value()
            }
            #[cfg(feature = "web_server")]
            "web-server-path" => {
                let settings = self.settings.lock().unwrap();
                settings.web_server_path.to_value()
            }
            #[cfg(feature = "web_server")]
            "web-server-directory" => {
                let settings = self.settings.lock().unwrap();
                settings.web_server_directory.to_value()
            }
            #[cfg(feature = "web_server")]
            "web-server-host-addr" => {
                let settings = self.settings.lock().unwrap();
                settings.web_server_host_addr.to_string().to_value()
            }
            "forward-metas" => {
                let settings = self.settings.lock().unwrap();
                settings.forward_metas.iter().join(",").to_value()
            }
            "enable-mitigation-modes" => {
                let settings = self.settings.lock().unwrap();
                settings.enabled_mitigation_modes.to_value()
            }
            _ => unimplemented!(),
        }
    }

    fn signals() -> &'static [glib::subclass::Signal] {
        static SIGNALS: LazyLock<Vec<glib::subclass::Signal>> = LazyLock::new(|| {
            vec![
                /**
                 * GstBaseWebRTCSink::consumer-added:
                 * @consumer_id: Identifier of the consumer added
                 * @webrtcbin: The new webrtcbin
                 *
                 * This signal can be used to tweak @webrtcbin, creating a data
                 * channel for example.
                 */
                glib::subclass::Signal::builder("consumer-added")
                    .param_types([String::static_type(), gst::Element::static_type()])
                    .build(),
                /**
                 * GstBaseWebRTCSink::consumer-pipeline-created:
                 * @consumer_id: Identifier of the consumer
                 * @pipeline: The pipeline that was just created
                 *
                 * This signal is emitted right after the pipeline for a new consumer
                 * has been created, for instance allowing handlers to connect to
                 * #GstBin::deep-element-added and tweak properties of any element used
                 * by the pipeline.
                 *
                 * This provides access to the lower level components of webrtcsink, and
                 * no guarantee is made that its internals will remain stable, use with caution!
                 *
                 * This is emitted *before* #GstBaseWebRTCSink::consumer-added .
                 */
                glib::subclass::Signal::builder("consumer-pipeline-created")
                    .param_types([String::static_type(), gst::Pipeline::static_type()])
                    .build(),
                /**
                 * GstBaseWebRTCSink::consumer_removed:
                 * @consumer_id: Identifier of the consumer that was removed
                 * @webrtcbin: The webrtcbin connected to the newly removed consumer
                 *
                 * This signal is emitted right after the connection with a consumer
                 * has been dropped.
                 */
                glib::subclass::Signal::builder("consumer-removed")
                    .param_types([String::static_type(), gst::Element::static_type()])
                    .build(),
                /**
                 * GstBaseWebRTCSink::get_sessions:
                 *
                 * List all sessions (by ID).
                 */
                glib::subclass::Signal::builder("get-sessions")
                    .action()
                    .class_handler(|args| {
                        let element = args[0].get::<super::BaseWebRTCSink>().expect("signal arg");
                        let this = element.imp();

                        let res = Some(
                            this.state
                                .lock()
                                .unwrap()
                                .sessions
                                .keys()
                                .cloned()
                                .collect::<Vec<String>>()
                                .to_value(),
                        );
                        res
                    })
                    .return_type::<Vec<String>>()
                    .build(),
                /**
                 * GstBaseWebRTCSink::encoder-setup:
                 * @consumer_id: Identifier of the consumer, or "discovery"
                 *   when the encoder is used in a discovery pipeline.
                 * @pad_name: The name of the corresponding input pad
                 * @encoder: The constructed encoder
                 *
                 * This signal can be used to tweak @encoder properties.
                 *
                 * Returns: True if the encoder is entirely configured,
                 * False to let other handlers run
                 */
                glib::subclass::Signal::builder("encoder-setup")
                    .param_types([
                        String::static_type(),
                        String::static_type(),
                        gst::Element::static_type(),
                    ])
                    .return_type::<bool>()
                    .accumulator(setup_signal_accumulator)
                    .class_handler(|args| {
                        let element = args[0].get::<super::BaseWebRTCSink>().expect("signal arg");
                        let enc = args[3].get::<gst::Element>().unwrap();

                        gst::debug!(
                            CAT,
                            obj = element,
                            "applying default configuration on encoder {:?}",
                            enc
                        );

                        let this = element.imp();
                        let start_bitrate = this.settings.lock().unwrap().cc_info.start_bitrate;
                        configure_encoder(&enc, start_bitrate);

                        // Return false here so that latter handlers get called
                        Some(false.to_value())
                    })
                    .build(),
                /**
                 * GstBaseWebRTCSink::payloader-setup:
                 * @consumer_id: Identifier of the consumer, or "discovery"
                 *   when the payloader is used in a discovery pipeline.
                 * @pad_name: The name of the corresponding input pad
                 * @payloader: The constructed payloader for selected codec
                 *
                 * This signal can be used to tweak @payloader properties, in particular, adding
                 * additional extensions.
                 *
                 * Note that payload type and ssrc settings are managed by webrtcsink element and
                 * trying to change them from an application handler will have no effect.
                 *
                 * Returns: True if the encoder is entirely configured,
                 * False to let other handlers run. Note that unless your intent is to enforce
                 * your custom settings, it's recommended to let the default handler run
                 * (by returning true), which would apply the optimal settings.
                 */
                glib::subclass::Signal::builder("payloader-setup")
                    .param_types([
                        String::static_type(),
                        String::static_type(),
                        gst::Element::static_type(),
                    ])
                    .return_type::<bool>()
                    .accumulator(setup_signal_accumulator)
                    .class_handler(|args| {
                        let pay = args[3].get::<gst::Element>().unwrap();

                        configure_payloader(&pay);

                        // The default handler is no-op: the whole configuration logic happens
                        // in BaseWebRTCSink::configure_payloader, which is where this signal
                        // is invoked from
                        Some(false.to_value())
                    })
                    .build(),
                /**
                 * GstBaseWebRTCSink::request-encoded-filter:
                 * @consumer_id: Identifier of the consumer
                 * @pad_name: The name of the corresponding input pad
                 * @encoded_caps: The Caps of the encoded stream
                 *
                 * This signal can be used to insert a filter
                 * element between the encoder and the payloader.
                 *
                 * When called during Caps discovery, the `consumer_id` is `None`.
                 *
                 * Returns: the element to insert.
                 */
                glib::subclass::Signal::builder("request-encoded-filter")
                    .param_types([
                        Option::<String>::static_type(),
                        String::static_type(),
                        gst::Caps::static_type(),
                    ])
                    .return_type::<gst::Element>()
                    .build(),
                /**
                 * GstBaseWebRTCSink::define-encoder-bitrates:
                 * @consumer_id: Identifier of the consumer
                 * @overall_bitrate: The total bitrate to allocate
                 * @structure: A structure describing the default per-encoder bitrates
                 *
                 * When a session carries multiple video streams, the congestion
                 * control mechanism will simply divide the overall allocated bitrate
                 * by the number of encoders and set the result as the bitrate for each
                 * individual encoder.
                 *
                 * With this signal, the application can affect how the overall bitrate
                 * gets allocated.
                 *
                 * The structure is named "webrtcsink/encoder-bitrates" and
                 * contains one gchararray to gint32 mapping per video stream
                 * name, for instance:
                 *
                 * "video_1234": 5000i32
                 * "video_5678": 10000i32
                 *
                 * The total of the bitrates in the returned structure should match
                 * the overall bitrate, as it does in the input structure.
                 *
                 * Returns: the updated encoder bitrates.
                 * Since: plugins-rs-0.14.0
                 */
                glib::subclass::Signal::builder("define-encoder-bitrates")
                    .param_types([
                        String::static_type(),
                        i32::static_type(),
                        gst::Structure::static_type(),
                    ])
                    .return_type::<gst::Structure>()
                    .run_last()
                    .class_handler(|args| {
                        Some(args[3usize].get::<gst::Structure>().expect("wrong argument").to_value())
                    })
                    .accumulator(move |_hint, _acc, value| {
                        // First signal handler wins
                        std::ops::ControlFlow::Break(value.clone())
                    })
                    .build(),
                /**
                 * GstBaseWebRTCSink::configure-mitigation-caps:
                 * @stream_name: name of the sink pad feeding the encoder
                 * @current_bitrate: The current bitrate being applied to the encoder
                 *
                 *
                 * Returns: the mitigation mode to apply.
                 * Since: plugins-rs-0.14.0
                 */
                glib::subclass::Signal::builder("configure-mitigation-caps")
                    .param_types([
                        String::static_type(),
                        VideoInfo::static_type(),
                        i32::static_type(),
                        WebRTCSinkMitigationMode::static_type(),
                    ])
                    .return_type::<gst::Caps>()
                    .run_last()
                    .class_handler(|args| {
                        let video_info = args[2].get::<VideoInfo>().expect("No video info");
                        let current_bitrate = args[3].get::<i32>().expect("No bitrate");
                        let enabled_mitigation_modes = args[4].get::<WebRTCSinkMitigationMode>().expect("No enabled mitigation modes");

                        Some(default_configure_mitigation_mode(&video_info, current_bitrate, enabled_mitigation_modes).to_value())
                    })
                    .accumulator(move |_hint, _acc, value| {
                        std::ops::ControlFlow::Break(value.clone())
                    })
                    .build(),
            ]
        });

        SIGNALS.as_ref()
    }

    fn constructed(&self) {
        self.parent_constructed();
        let signaller = self.settings.lock().unwrap().signaller.clone();

        self.connect_signaller(&signaller);

        let obj = self.obj();
        obj.set_suppressed_flags(gst::ElementFlags::SINK | gst::ElementFlags::SOURCE);
        obj.set_element_flags(gst::ElementFlags::SINK);
    }
}

impl GstObjectImpl for BaseWebRTCSink {}

impl ElementImpl for BaseWebRTCSink {
    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let mut caps_builder = gst::Caps::builder_full()
                .structure(gst::Structure::builder("video/x-raw").build())
                .structure_with_features(
                    gst::Structure::builder("video/x-raw").build(),
                    gst::CapsFeatures::new([CUDA_MEMORY_FEATURE]),
                )
                .structure_with_features(
                    gst::Structure::builder("video/x-raw").build(),
                    gst::CapsFeatures::new([GL_MEMORY_FEATURE]),
                )
                .structure_with_features(
                    gst::Structure::builder("video/x-raw").build(),
                    gst::CapsFeatures::new([NVMM_MEMORY_FEATURE]),
                )
                .structure_with_features(
                    gst::Structure::builder("video/x-raw").build(),
                    gst::CapsFeatures::new([D3D11_MEMORY_FEATURE]),
                )
                .structure_with_features(
                    gst::Structure::builder("video/x-raw").build(),
                    gst::CapsFeatures::new([VA_MEMORY_FEATURE]),
                );

            // Ignore specific raw caps from Codecs: they are covered by video/x-raw & audio/x-raw
            for codec in Codecs::video_codecs().filter(|codec| !codec.is_raw) {
                caps_builder = caps_builder.structure(codec.caps.structure(0).unwrap().to_owned());
            }

            let video_pad_template = gst::PadTemplate::with_gtype(
                "video_%u",
                gst::PadDirection::Sink,
                gst::PadPresence::Request,
                &caps_builder.build(),
                WebRTCSinkPad::static_type(),
            )
            .unwrap();

            let mut caps_builder =
                gst::Caps::builder_full().structure(gst::Structure::builder("audio/x-raw").build());
            for codec in Codecs::audio_codecs().filter(|codec| !codec.is_raw) {
                caps_builder = caps_builder.structure(codec.caps.structure(0).unwrap().to_owned());
            }
            let audio_pad_template = gst::PadTemplate::with_gtype(
                "audio_%u",
                gst::PadDirection::Sink,
                gst::PadPresence::Request,
                &caps_builder.build(),
                WebRTCSinkPad::static_type(),
            )
            .unwrap();

            vec![video_pad_template, audio_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn request_new_pad(
        &self,
        templ: &gst::PadTemplate,
        _name: Option<&str>,
        _caps: Option<&gst::Caps>,
    ) -> Option<gst::Pad> {
        let element = self.obj();
        if element.current_state() > gst::State::Ready {
            gst::error!(
                CAT,
                imp = self,
                "element pads can only be requested before starting"
            );
            return None;
        }

        let mut state = self.state.lock().unwrap();

        let serial;

        let (name, is_video) = if templ.name().starts_with("video_") {
            let name = format!("video_{}", state.video_serial);
            serial = state.video_serial;
            state.video_serial += 1;

            (name, true)
        } else {
            let name = format!("audio_{}", state.audio_serial);
            serial = state.audio_serial;
            state.audio_serial += 1;
            (name, false)
        };

        let sink_pad = gst::PadBuilder::<WebRTCSinkPad>::from_template(templ)
            .name(name.as_str())
            .chain_function(|pad, parent, buffer| {
                BaseWebRTCSink::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |this| this.chain(pad.upcast_ref(), buffer),
                )
            })
            .event_function(|pad, parent, event| {
                BaseWebRTCSink::catch_panic_pad_function(
                    parent,
                    || false,
                    |this| this.sink_event(pad.upcast_ref(), &this.obj(), event),
                )
            })
            .build();

        sink_pad.set_active(true).unwrap();
        sink_pad.use_fixed_caps();
        element.add_pad(&sink_pad).unwrap();

        state.streams.insert(
            name,
            InputStream {
                sink_pad: sink_pad.clone(),
                producer: None,
                in_caps: None,
                out_caps: None,
                clocksync: None,
                is_video,
                serial,
                initial_discovery_started: false,
            },
        );

        Some(sink_pad.upcast())
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        let element = self.obj();
        if let gst::StateChange::ReadyToPaused = transition {
            if let Err(err) = self.prepare() {
                gst::element_error!(
                    element,
                    gst::StreamError::Failed,
                    ["Failed to prepare: {}", err]
                );
                return Err(gst::StateChangeError);
            }
        }

        let mut ret = self.parent_change_state(transition);

        match transition {
            gst::StateChange::PausedToReady => {
                let unprepare_res = match tokio::runtime::Handle::try_current() {
                    Ok(_) => {
                        gst::error!(
                            CAT,
                            obj = element,
                            "Trying to set state to NULL from an async \
                                    tokio context, working around the panic but \
                                    you should refactor your code to make use of \
                                    gst::Element::call_async and set the state to \
                                    NULL from there, without blocking the runtime"
                        );
                        let (tx, rx) = mpsc::channel();
                        element.call_async(move |element| {
                            tx.send(element.imp().unprepare()).unwrap();
                        });
                        rx.recv().unwrap()
                    }
                    Err(_) => self.unprepare(),
                };

                if let Err(err) = unprepare_res {
                    gst::element_error!(
                        element,
                        gst::StreamError::Failed,
                        ["Failed to unprepare: {}", err]
                    );
                    return Err(gst::StateChangeError);
                }
            }
            gst::StateChange::ReadyToPaused => {
                ret = Ok(gst::StateChangeSuccess::NoPreroll);
            }
            gst::StateChange::PausedToPlaying => {
                let settings = self.settings.lock().unwrap();
                let signaller = settings.signaller.clone();
                drop(settings);
                let mut state = self.state.lock().unwrap();
                if state.should_start_signaller(&element) {
                    state.signaller_state = SignallerState::Started;
                    drop(state);
                    signaller.start();
                }
            }
            _ => (),
        }

        ret
    }
}

impl BinImpl for BaseWebRTCSink {}

impl ChildProxyImpl for BaseWebRTCSink {
    fn child_by_index(&self, _index: u32) -> Option<glib::Object> {
        None
    }

    fn children_count(&self) -> u32 {
        0
    }

    fn child_by_name(&self, name: &str) -> Option<glib::Object> {
        match name {
            "signaller" => Some(self.settings.lock().unwrap().signaller.clone().upcast()),
            _ => self.obj().static_pad(name).map(|pad| pad.upcast()),
        }
    }
}

impl NavigationImpl for BaseWebRTCSink {
    fn send_event(&self, event_def: gst::Structure) {
        let mut state = self.state.lock().unwrap();
        let event = gst::event::Navigation::new(event_def);

        state.streams.iter_mut().for_each(|(_, stream)| {
            if stream.sink_pad.name().starts_with("video_") {
                gst::log!(CAT, "Navigating to: {:?}", event);
                // FIXME: Handle multi tracks.
                if !stream.sink_pad.push_event(event.clone()) {
                    gst::info!(CAT, imp = self, "Could not send event: {:?}", event);
                }
            }
        });
    }
}

const DEFAULT_RUN_SIGNALLING_SERVER: bool = false;
const DEFAULT_SIGNALLING_SERVER_HOST: &str = "0.0.0.0";
const DEFAULT_SIGNALLING_SERVER_PORT: u16 = 8443;
const DEFAULT_SIGNALLING_SERVER_CERT: Option<&str> = None;
const DEFAULT_SIGNALLING_SERVER_KEY: Option<&str> = None;

#[derive(Default)]
pub struct WebRTCSinkState {
    signalling_server_handle: Option<tokio::task::JoinHandle<()>>,
}

#[derive(Clone)]
pub struct WebRTCSinkSettings {
    run_signalling_server: bool,
    signalling_server_host: String,
    signalling_server_port: u16,
    signalling_server_cert: Option<String>,
    signalling_server_key: Option<String>,
}

impl Default for WebRTCSinkSettings {
    fn default() -> Self {
        Self {
            run_signalling_server: DEFAULT_RUN_SIGNALLING_SERVER,
            signalling_server_host: DEFAULT_SIGNALLING_SERVER_HOST.to_string(),
            signalling_server_port: DEFAULT_SIGNALLING_SERVER_PORT,
            signalling_server_cert: DEFAULT_SIGNALLING_SERVER_CERT.map(String::from),
            signalling_server_key: DEFAULT_SIGNALLING_SERVER_KEY.map(String::from),
        }
    }
}

#[derive(Default)]
pub struct WebRTCSink {
    state: Mutex<WebRTCSinkState>,
    settings: Mutex<WebRTCSinkSettings>,
}

fn initialize_logging(envvar_name: &str) -> Result<(), Error> {
    tracing_log::LogTracer::init()?;
    let env_filter = tracing_subscriber::EnvFilter::try_from_env(envvar_name)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_thread_ids(true)
        .with_target(true)
        .with_span_events(
            tracing_subscriber::fmt::format::FmtSpan::NEW
                | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
        );
    let subscriber = tracing_subscriber::Registry::default()
        .with(env_filter)
        .with(fmt_layer);
    tracing::subscriber::set_global_default(subscriber)?;

    Ok(())
}

pub static SIGNALLING_LOGGING: LazyLock<Result<(), Error>> =
    LazyLock::new(|| initialize_logging("WEBRTCSINK_SIGNALLING_SERVER_LOG"));

#[cfg(feature = "web_server")]
use warp::Filter;

impl WebRTCSink {
    async fn spawn_signalling_server(settings: &WebRTCSinkSettings) -> Result<(), Error> {
        let server = Server::spawn(Handler::new);
        let addr = format!(
            "{}:{}",
            settings.signalling_server_host, settings.signalling_server_port
        );

        // Create the event loop and TCP listener we'll accept connections on.
        let listener = TcpListener::bind(&addr).await?;

        let acceptor = if let (Some(cert), Some(key)) = (
            &settings.signalling_server_cert,
            &settings.signalling_server_key,
        ) {
            create_tls_acceptor(cert, key).await.ok()
        } else {
            None
        };

        while let Ok((stream, address)) = listener.accept().await {
            let mut server_clone = server.clone();
            gst::info!(CAT, "Accepting connection from {}", address);

            if let Some(acceptor) = acceptor.clone() {
                tokio::spawn(async move {
                    match tokio::time::timeout(TLS_HANDSHAKE_TIMEOUT, acceptor.accept(stream)).await
                    {
                        Ok(Ok(stream)) => server_clone.accept_async(stream).await,
                        Ok(Err(err)) => {
                            gst::warning!(
                                CAT,
                                "Failed to accept TLS connection from {}: {}",
                                address,
                                err
                            );
                            Err(ServerError::TLSHandshake(err))
                        }
                        Err(elapsed) => {
                            gst::warning!(
                                CAT,
                                "TLS connection timed out {} after {}",
                                address,
                                elapsed
                            );
                            Err(ServerError::TLSHandshakeTimeout(elapsed))
                        }
                    }
                });
            } else {
                RUNTIME.spawn(async move { server_clone.accept_async(stream).await });
            }
        }

        Ok(())
    }

    /// Start a signalling server if required
    fn prepare(&self) -> Result<(), Error> {
        gst::debug!(CAT, imp = self, "preparing");

        let mut state = self.state.lock().unwrap();

        let settings = self.settings.lock().unwrap().clone();

        if settings.run_signalling_server {
            // Configure our own signalling server as URI on the signaller
            {
                let signaller = self
                    .obj()
                    .upcast_ref::<super::BaseWebRTCSink>()
                    .imp()
                    .settings
                    .lock()
                    .unwrap()
                    .signaller
                    .clone();

                // Map UNSPECIFIED addresses to localhost
                let host = match settings.signalling_server_host.as_str() {
                    "0.0.0.0" => "127.0.0.1".to_string(),
                    "::" | "[::]" => "[::1]".to_string(),
                    host => host.to_string(),
                };

                let scheme = if settings.signalling_server_cert.is_some()
                    && settings.signalling_server_key.is_some()
                {
                    let cafile = settings.signalling_server_cert.as_ref().unwrap();
                    signaller.set_property("cafile", cafile);
                    "wss"
                } else {
                    "ws"
                };
                let uri = format!("{}://{}:{}", scheme, host, settings.signalling_server_port);

                signaller.set_property("uri", uri);
            }

            if let Err(err) = LazyLock::force(&SIGNALLING_LOGGING) {
                Err(anyhow!(
                    "failed signalling server logging initialization: {}",
                    err
                ))?;
            }
            state.signalling_server_handle = Some(RUNTIME.spawn(glib::clone!(
                #[weak(rename_to = this)]
                self,
                async move {
                    if let Err(err) = WebRTCSink::spawn_signalling_server(&settings).await {
                        gst::error!(
                            CAT,
                            imp = this,
                            "Failed to start signalling server: {}",
                            err
                        );
                        this.post_error_message(gst::error_msg!(
                            gst::StreamError::Failed,
                            ["Failed to start signalling server: {}", err]
                        ));
                    }
                }
            )));
        }

        Ok(())
    }

    fn unprepare(&self) {
        gst::info!(CAT, imp = self, "unpreparing");

        let mut state = self.state.lock().unwrap();

        if let Some(handle) = state.signalling_server_handle.take() {
            handle.abort();
        }
    }
}

impl ObjectImpl for WebRTCSink {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                /**
                 * GstWebRTCSink:run-signalling-server:
                 *
                 * Whether the element should run its own signalling server.
                 *
                 * Since: plugins-rs-0.14.0
                 */
                glib::ParamSpecBoolean::builder("run-signalling-server")
                    .nick("Run signalling server")
                    .blurb("Whether the element should run its own signalling server")
                    .default_value(DEFAULT_RUN_SIGNALLING_SERVER)
                    .mutable_ready()
                    .build(),
                /**
                 * GstWebRTCSink:signalling-server-host:
                 *
                 * The address to listen on when #GstWebRTCSink:run-signalling-server
                 * is TRUE.
                 *
                 * Since: plugins-rs-0.14.0
                 */
                glib::ParamSpecString::builder("signalling-server-host")
                    .nick("Signalling server host")
                    .blurb("Address the signalling server should listen on")
                    .default_value(DEFAULT_SIGNALLING_SERVER_HOST)
                    .build(),
                /**
                 * GstWebRTCSink:signalling-server-port:
                 *
                 * The port to listen on when #GstWebRTCSink:run-signalling-server
                 * is TRUE.
                 *
                 * Since: plugins-rs-0.14.0
                 */
                glib::ParamSpecUInt::builder("signalling-server-port")
                    .nick("Signalling server port")
                    .blurb("Port the signalling server should listen on")
                    .minimum(1)
                    .maximum(u16::MAX as u32)
                    .default_value(DEFAULT_SIGNALLING_SERVER_PORT as u32)
                    .build(),
                /**
                 * GstWebRTCSink:signalling-server-cert:
                 *
                 * Path to TLS certificate to use when #GstWebRTCSink:run-signalling-server
                 * is TRUE.
                 *
                 * The certificate should be formatted as PEM.
                 *
                 * Since: plugins-rs-0.14.0
                 */
                glib::ParamSpecString::builder("signalling-server-cert")
                    .nick("Signalling server certificate")
                    .blurb(
                        "Path to TLS certificate the signalling server should use.
                        The certificate should be formatted as PEM",
                    )
                    .default_value(DEFAULT_SIGNALLING_SERVER_CERT)
                    .build(),
                /**
                 * GstBaseWebRTCSink:signalling-server-key:
                 *
                 * The private key to use when #GstBaseWebRTCSink:run-signalling-server
                 * is TRUE.
                 *
                 * Since: plugins-rs-0.14.0
                 */
                glib::ParamSpecString::builder("signalling-server-key")
                    .nick("Signalling server private key")
                    .blurb("Path to private encryption key the signalling server should use.
                        The key should be formatted as PEM")
                    .default_value(DEFAULT_SIGNALLING_SERVER_KEY)
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "run-signalling-server" => {
                let mut settings = self.settings.lock().unwrap();
                settings.run_signalling_server =
                    value.get::<bool>().expect("type checked upstream");
            }
            "signalling-server-host" => {
                let mut settings = self.settings.lock().unwrap();
                settings.signalling_server_host =
                    value.get::<String>().expect("type checked upstream")
            }
            "signalling-server-port" => {
                let mut settings = self.settings.lock().unwrap();
                settings.signalling_server_port =
                    value.get::<u32>().expect("type checked upstream") as u16;
            }
            "signalling-server-cert" => {
                let mut settings = self.settings.lock().unwrap();
                settings.signalling_server_cert = value
                    .get::<Option<String>>()
                    .expect("type checked upstream")
            }
            "signalling-server-key" => {
                let mut settings = self.settings.lock().unwrap();
                settings.signalling_server_key = value
                    .get::<Option<String>>()
                    .expect("type checked upstream")
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "run-signalling-server" => {
                let settings = self.settings.lock().unwrap();
                settings.run_signalling_server.to_value()
            }
            "signalling-server-host" => {
                let settings = self.settings.lock().unwrap();
                settings.signalling_server_host.to_value()
            }
            "signalling-server-port" => {
                let settings = self.settings.lock().unwrap();
                (settings.signalling_server_port as u32).to_value()
            }
            "signalling-server-cert" => {
                let settings = self.settings.lock().unwrap();
                settings.signalling_server_cert.to_value()
            }
            "signalling-server-key" => {
                let settings = self.settings.lock().unwrap();
                settings.signalling_server_key.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for WebRTCSink {}

impl ElementImpl for WebRTCSink {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "WebRTCSink",
                "Sink/Network/WebRTC",
                "WebRTC sink with custom protocol signaller",
                "Mathieu Duponchelle <mathieu@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        let element = self.obj();
        if let gst::StateChange::ReadyToPaused = transition {
            if let Err(err) = self.prepare() {
                gst::element_error!(
                    element,
                    gst::StreamError::Failed,
                    ["Failed to prepare: {}", err]
                );
                return Err(gst::StateChangeError);
            }
        }

        let ret = self.parent_change_state(transition);

        if let gst::StateChange::PausedToReady = transition {
            self.unprepare();
        }

        ret
    }
}

impl BinImpl for WebRTCSink {}

impl BaseWebRTCSinkImpl for WebRTCSink {}

#[glib::object_subclass]
impl ObjectSubclass for WebRTCSink {
    const NAME: &'static str = "GstWebRTCSink";
    type Type = super::WebRTCSink;
    type ParentType = super::BaseWebRTCSink;
}

#[cfg(feature = "aws")]
pub(super) mod aws {
    use super::*;
    use crate::aws_kvs_signaller::AwsKvsSignaller;

    #[derive(Default)]
    pub struct AwsKvsWebRTCSink {}

    impl ObjectImpl for AwsKvsWebRTCSink {
        fn constructed(&self) {
            self.parent_constructed();

            let element = self.obj();
            let ws = element
                .upcast_ref::<crate::webrtcsink::BaseWebRTCSink>()
                .imp();

            let _ = ws.set_signaller(AwsKvsSignaller::default().upcast());
        }
    }

    impl GstObjectImpl for AwsKvsWebRTCSink {}

    impl ElementImpl for AwsKvsWebRTCSink {
        fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
            static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> =
                LazyLock::new(|| {
                    gst::subclass::ElementMetadata::new(
                        "AwsKvsWebRTCSink",
                        "Sink/Network/WebRTC",
                        "WebRTC sink with kinesis video streams signaller",
                        "Mathieu Duponchelle <mathieu@centricular.com>",
                    )
                });

            Some(&*ELEMENT_METADATA)
        }
    }

    impl BinImpl for AwsKvsWebRTCSink {}

    impl BaseWebRTCSinkImpl for AwsKvsWebRTCSink {}

    #[glib::object_subclass]
    impl ObjectSubclass for AwsKvsWebRTCSink {
        const NAME: &'static str = "GstAwsKvsWebRTCSink";
        type Type = crate::webrtcsink::AwsKvsWebRTCSink;
        type ParentType = crate::webrtcsink::BaseWebRTCSink;
    }
}

#[cfg(feature = "whip")]
pub(super) mod whip {
    use super::*;
    use crate::whip_signaller::WhipClientSignaller;

    #[derive(Default)]
    pub struct WhipWebRTCSink {}

    impl ObjectImpl for WhipWebRTCSink {
        fn constructed(&self) {
            self.parent_constructed();

            let element = self.obj();
            let ws = element
                .upcast_ref::<crate::webrtcsink::BaseWebRTCSink>()
                .imp();

            let _ = ws.set_signaller(WhipClientSignaller::default().upcast());
        }
    }

    impl GstObjectImpl for WhipWebRTCSink {}

    impl ElementImpl for WhipWebRTCSink {
        fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
            static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> =
                LazyLock::new(|| {
                    gst::subclass::ElementMetadata::new(
                        "WhipWebRTCSink",
                        "Sink/Network/WebRTC",
                        "WebRTC sink with WHIP client signaller",
                        "Taruntej Kanakamalla <taruntej@asymptotic.io>",
                    )
                });

            Some(&*ELEMENT_METADATA)
        }
    }

    impl BinImpl for WhipWebRTCSink {}

    impl BaseWebRTCSinkImpl for WhipWebRTCSink {}

    #[glib::object_subclass]
    impl ObjectSubclass for WhipWebRTCSink {
        const NAME: &'static str = "GstWhipWebRTCSink";
        type Type = crate::webrtcsink::WhipWebRTCSink;
        type ParentType = crate::webrtcsink::BaseWebRTCSink;
    }
}

#[cfg(feature = "livekit")]
pub(super) mod livekit {
    use super::*;
    use crate::livekit_signaller::LiveKitSignaller;

    #[derive(Default)]
    pub struct LiveKitWebRTCSink {}

    impl ObjectImpl for LiveKitWebRTCSink {
        fn constructed(&self) {
            self.parent_constructed();

            let element = self.obj();
            let ws = element
                .upcast_ref::<crate::webrtcsink::BaseWebRTCSink>()
                .imp();

            let _ = ws.set_signaller(LiveKitSignaller::new_producer().upcast());
        }
    }

    impl GstObjectImpl for LiveKitWebRTCSink {}

    impl ElementImpl for LiveKitWebRTCSink {
        fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
            static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> =
                LazyLock::new(|| {
                    gst::subclass::ElementMetadata::new(
                        "LiveKitWebRTCSink",
                        "Sink/Network/WebRTC",
                        "WebRTC sink with LiveKit signaller",
                        "Olivier Crête <olivier.crete@collabora.com>",
                    )
                });

            Some(&*ELEMENT_METADATA)
        }
    }

    impl BinImpl for LiveKitWebRTCSink {}

    impl BaseWebRTCSinkImpl for LiveKitWebRTCSink {}

    #[glib::object_subclass]
    impl ObjectSubclass for LiveKitWebRTCSink {
        const NAME: &'static str = "GstLiveKitWebRTCSink";
        type Type = crate::webrtcsink::LiveKitWebRTCSink;
        type ParentType = crate::webrtcsink::BaseWebRTCSink;
    }
}

#[cfg(feature = "janus")]
pub(super) mod janus {
    use super::*;
    use crate::{
        janusvr_signaller::{JanusVRSignallerStr, JanusVRSignallerU64},
        webrtcsink::JanusVRSignallerState,
    };

    static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
        gst::DebugCategory::new(
            "janusvrwebrtcsink",
            gst::DebugColorFlags::empty(),
            Some("WebRTC Janus Video Room sink"),
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
    #[properties(wrapper_type = crate::webrtcsink::JanusVRWebRTCSink)]
    pub struct JanusVRWebRTCSink {
        /**
         * GstJanusVRWebRTCSink:use-string-ids:
         *
         * By default Janus uses `u64` ids to identify the room, the feed, etc.
         * But it can be changed to strings using the `strings_ids` option in `janus.plugin.videoroom.jcfg`.
         * In such case, `janusvrwebrtcsink` has to be created using `use-string-ids=true` so its signaller
         * uses the right types for such ids and properties.
         *
         * Since: plugins-rs-0.13.0
         */
        #[property(name="use-string-ids", get, construct_only, type = bool, member = use_string_ids, blurb = "Use strings instead of u64 for Janus IDs, see strings_ids config option in janus.plugin.videoroom.jcfg")]
        settings: Mutex<JanusSettings>,
        /**
         * GstJanusVRWebRTCSink:janus-state:
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
    impl ObjectImpl for JanusVRWebRTCSink {
        fn constructed(&self) {
            self.parent_constructed();

            let settings = self.settings.lock().unwrap();
            let element = self.obj();
            let ws = element
                .upcast_ref::<crate::webrtcsink::BaseWebRTCSink>()
                .imp();

            let signaller: Signallable = if settings.use_string_ids {
                JanusVRSignallerStr::new(WebRTCSignallerRole::Producer).upcast()
            } else {
                JanusVRSignallerU64::new(WebRTCSignallerRole::Producer).upcast()
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

    impl GstObjectImpl for JanusVRWebRTCSink {}

    impl ElementImpl for JanusVRWebRTCSink {
        fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
            static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> =
                LazyLock::new(|| {
                    gst::subclass::ElementMetadata::new(
                        "JanusVRWebRTCSink",
                        "Sink/Network/WebRTC",
                        "WebRTC sink with Janus Video Room signaller",
                        "Eva Pace <epace@igalia.com>",
                    )
                });

            Some(&*ELEMENT_METADATA)
        }
    }

    impl BinImpl for JanusVRWebRTCSink {}

    impl BaseWebRTCSinkImpl for JanusVRWebRTCSink {}

    #[glib::object_subclass]
    impl ObjectSubclass for JanusVRWebRTCSink {
        const NAME: &'static str = "GstJanusVRWebRTCSink";
        type Type = crate::webrtcsink::JanusVRWebRTCSink;
        type ParentType = crate::webrtcsink::BaseWebRTCSink;
    }
}
