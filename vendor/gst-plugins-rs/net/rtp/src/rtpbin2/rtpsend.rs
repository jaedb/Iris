// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-rtpsend
 * @see_also: rtprecv, rtpbin, rtpsession, rtpjitterbuffer.
 *
 * RTP session management (sender).
 *
 * ## Environment variables
 *
 * The underlying Runtime can be fine-tuned process-wide using the following env vars:
 *
 * * 'GST_RTPBIN2_RT_WORKER_THREADS': number of worker threads the Runtime will use (default: 1)
 *   0 => number of cores available on the system.
 * * 'GST_RTPBIN2_RT_MAX_BLOCKING_THREADS': limit for the number of threads in the blocking pool (default: 512).
 *   When they push buffers, downstream events or handle downstream queries, rtpbin2 elements spawn
 *   a thread from the blocking pool. This is to avoid blocking the worker thread which is shared
 *   with other elements running on the same runtime.
 * * 'GST_RTPBIN2_RT_THREAD_KEEP_ALIVE': timeout for a thread in the blocking pool in ms (default: 10s).
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 \
 *   audiotestsrc is-live=true ! opusenc ! rtpopuspay2 ! queue max-size-buffers=0 max-size-bytes=0 max-size-time=100000000 \
 *   ! rtpsend name=send rtp-id=example-rtp-id \
 *       send.rtp_src_0 ! udpsink port=5004 host=127.0.0.1 \
 *       send.rtcp_src_0 ! udpsink port=5005 host=127.0.0.1 async=false \
 *   rtprecv name=recv rtp-id=example-rtp-id \
 *   udpsrc port=5007 caps='application/x-rtcp' ! recv.rtcp_sink_0
 * ]| This will produce an Opus encoded audio stream and send it as RTP packets with RTCP
 * over UDP ports 5004 & 5005. The pipeline expects RTCP to be sent back on UDP port 5007.
 * See #rtprecv for an example of how to process such packets.
 *
 * Since: plugins-rs-0.13.0
 */
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::Poll;
use std::time::{Duration, Instant, SystemTime};

use futures::future::{AbortHandle, Abortable};
use futures::StreamExt;
use gst::{glib, prelude::*, subclass::prelude::*};
use std::sync::LazyLock;

use super::internal::{pt_clock_rate_from_caps, GstRustLogger, SharedRtpState, SharedSession};
use super::session::{RtcpSendReply, RtpProfile, SendReply, RTCP_MIN_REPORT_INTERVAL};
use super::source::SourceState;

use crate::rtpbin2;

const DEFAULT_MIN_RTCP_INTERVAL: Duration = RTCP_MIN_REPORT_INTERVAL;
const DEFAULT_REDUCED_SIZE_RTCP: bool = false;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpsend",
        gst::DebugColorFlags::empty(),
        Some("RTP Sending"),
    )
});

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstRtpSendProfile")]
pub enum Profile {
    #[default]
    #[enum_value(name = "AVP profile as specified in RFC 3550", nick = "avp")]
    Avp,
    #[enum_value(name = "AVPF profile as specified in RFC 4585", nick = "avpf")]
    Avpf,
}

impl From<RtpProfile> for Profile {
    fn from(value: RtpProfile) -> Self {
        match value {
            RtpProfile::Avp => Self::Avp,
            RtpProfile::Avpf => Self::Avpf,
        }
    }
}

impl From<Profile> for RtpProfile {
    fn from(value: Profile) -> Self {
        match value {
            Profile::Avp => Self::Avp,
            Profile::Avpf => Self::Avpf,
        }
    }
}

#[derive(Debug, Clone)]
struct Settings {
    rtp_id: String,
    min_rtcp_interval: Duration,
    profile: Profile,
    reduced_size_rtcp: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            rtp_id: String::from("rtp-id"),
            min_rtcp_interval: DEFAULT_MIN_RTCP_INTERVAL,
            profile: Profile::default(),
            reduced_size_rtcp: DEFAULT_REDUCED_SIZE_RTCP,
        }
    }
}

#[derive(Debug)]
#[must_use = "futures/streams/sinks do nothing unless you `.await` or poll them"]
struct RtcpSendStream {
    state: Arc<Mutex<State>>,
    session_id: usize,
    sleep: Pin<Box<tokio::time::Sleep>>,
}

impl RtcpSendStream {
    fn new(state: Arc<Mutex<State>>, session_id: usize) -> Self {
        Self {
            state,
            session_id,
            sleep: Box::pin(tokio::time::sleep(Duration::from_secs(1))),
        }
    }
}

impl futures::stream::Stream for RtcpSendStream {
    type Item = RtcpSendReply;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let mut state = self.state.lock().unwrap();
        let now = Instant::now();
        let ntp_now = SystemTime::now();
        let mut lowest_wait = None;
        if let Some(session) = state.mut_session_by_id(self.session_id) {
            let mut session_inner = session.internal_session.inner.lock().unwrap();
            if let Some(reply) = session_inner.session.poll_rtcp_send(now, ntp_now) {
                return Poll::Ready(Some(reply));
            }
            if let Some(wait) = session_inner.session.poll_rtcp_send_timeout(now) {
                if lowest_wait.is_none_or(|lowest_wait| wait < lowest_wait) {
                    lowest_wait = Some(wait);
                }
            }
            session_inner.rtcp_waker = Some(cx.waker().clone());
        }
        drop(state);

        // default to the minimum initial rtcp delay so we don't busy loop if there are no sessions or no
        // timeouts available
        let lowest_wait =
            lowest_wait.unwrap_or(now + crate::rtpbin2::session::RTCP_MIN_REPORT_INTERVAL / 2);
        let this = self.get_mut();
        this.sleep.as_mut().reset(lowest_wait.into());
        if !std::future::Future::poll(this.sleep.as_mut(), cx).is_pending() {
            // wake us again if the delay is not pending for another go at finding the next timeout
            // value
            cx.waker().wake_by_ref();
        }
        Poll::Pending
    }
}

#[derive(Debug)]
struct SendSession {
    internal_session: SharedSession,

    rtcp_task: Mutex<Option<RtcpTask>>,

    // State for sending RTP streams
    rtp_send_sinkpad: Option<gst::Pad>,
    rtp_send_srcpad: Option<gst::Pad>,

    rtcp_send_srcpad: Option<gst::Pad>,
}

impl SendSession {
    fn new(shared_state: &SharedRtpState, id: usize, settings: &Settings) -> Self {
        let internal_session = shared_state.session_get_or_init(id, || {
            SharedSession::new(
                id,
                settings.profile.into(),
                settings.min_rtcp_interval,
                settings.reduced_size_rtcp,
            )
        });
        let mut inner = internal_session.inner.lock().unwrap();
        inner.session.set_profile(settings.profile.into());
        inner
            .session
            .set_min_rtcp_interval(settings.min_rtcp_interval);
        inner
            .session
            .set_reduced_size_rtcp(settings.reduced_size_rtcp);
        drop(inner);

        Self {
            internal_session,

            rtcp_task: Mutex::new(None),
            rtp_send_sinkpad: None,
            rtp_send_srcpad: None,
            rtcp_send_srcpad: None,
        }
    }

    fn start_rtcp_task(&self, state: Arc<Mutex<State>>) {
        let mut rtcp_task = self.rtcp_task.lock().unwrap();

        if rtcp_task.is_some() {
            return;
        }

        // run the runtime from another task to prevent the "start a runtime from within a runtime" panic
        // when the plugin is statically linked.
        let (abort_handle, abort_registration) = AbortHandle::new_pair();
        let session_id = self.internal_session.id;
        rtpbin2::get_or_init_runtime()
            .expect("initialized in change_state()")
            .spawn(async move {
                let future = Abortable::new(Self::rtcp_task(state, session_id), abort_registration);
                future.await
            });

        rtcp_task.replace(RtcpTask { abort_handle });
    }

    async fn rtcp_task(state: Arc<Mutex<State>>, session_id: usize) {
        let mut stream = RtcpSendStream::new(state.clone(), session_id);
        // we use a semaphore instead of mutex for ordering
        // i.e. we should only allow a single pad push, but still allow other rtcp tasks to
        // continue operating
        let sem = Arc::new(tokio::sync::Semaphore::new(1));
        while let Some(reply) = stream.next().await {
            let send = {
                let state = state.lock().unwrap();
                let Some(session) = state.session_by_id(session_id) else {
                    continue;
                };
                match reply {
                    RtcpSendReply::Data(data) => {
                        session.rtcp_send_srcpad.clone().map(|pad| (pad, data))
                    }
                    RtcpSendReply::SsrcBye(ssrc) => {
                        session
                            .internal_session
                            .config
                            .emit_by_name::<()>("bye-ssrc", &[&ssrc]);
                        None
                    }
                }
            };

            if let Some((rtcp_srcpad, data)) = send {
                let acquired = sem.clone().acquire_owned().await;
                rtpbin2::get_or_init_runtime()
                    .expect("initialized in change_state()")
                    .spawn_blocking(move || {
                        let buffer = gst::Buffer::from_mut_slice(data);
                        if let Err(e) = rtcp_srcpad.push(buffer) {
                            gst::warning!(
                                CAT,
                                obj = rtcp_srcpad,
                                "Failed to send rtcp data: flow return {e:?}"
                            );
                        }
                        drop(acquired);
                    });
            }
        }
    }

    fn stop_rtcp_task(&self) {
        let mut rtcp_task = self.rtcp_task.lock().unwrap();

        if let Some(rtcp) = rtcp_task.take() {
            rtcp.abort_handle.abort();
        }
    }
}

#[derive(Debug, Default)]
struct State {
    shared_state: Option<SharedRtpState>,
    sessions: Vec<SendSession>,
    max_session_id: usize,
    pads_session_id_map: HashMap<gst::Pad, usize>,
}

impl State {
    fn session_by_id(&self, id: usize) -> Option<&SendSession> {
        self.sessions
            .iter()
            .find(|session| session.internal_session.id == id)
    }

    fn mut_session_by_id(&mut self, id: usize) -> Option<&mut SendSession> {
        self.sessions
            .iter_mut()
            .find(|session| session.internal_session.id == id)
    }

    fn stats(&self) -> gst::Structure {
        let mut ret = gst::Structure::builder("application/x-rtp2-stats");
        for session in self.sessions.iter() {
            let sess_id = session.internal_session.id;
            let session = session.internal_session.inner.lock().unwrap();

            ret = ret.field(sess_id.to_string(), session.stats());
        }
        ret.build()
    }
}

pub struct RtpSend {
    settings: Mutex<Settings>,
    state: Arc<Mutex<State>>,
}

#[derive(Debug)]
struct RtcpTask {
    abort_handle: AbortHandle,
}

impl RtpSend {
    fn iterate_internal_links(&self, pad: &gst::Pad) -> gst::Iterator<gst::Pad> {
        let state = self.state.lock().unwrap();
        if let Some(&id) = state.pads_session_id_map.get(pad) {
            if let Some(session) = state.session_by_id(id) {
                if let Some(ref sinkpad) = session.rtp_send_sinkpad {
                    if let Some(ref srcpad) = session.rtp_send_srcpad {
                        if sinkpad == pad {
                            return gst::Iterator::from_vec(vec![srcpad.clone()]);
                        } else if srcpad == pad {
                            return gst::Iterator::from_vec(vec![sinkpad.clone()]);
                        }
                    }
                }
                // nothing to do for rtcp pads
            }
        }
        gst::Iterator::from_vec(vec![])
    }

    fn handle_buffer(
        &self,
        sinkpad: &gst::Pad,
        srcpad: &gst::Pad,
        internal_session: &SharedSession,
        buffer: gst::Buffer,
        now: Instant,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mapped = buffer.map_readable().map_err(|e| {
            gst::error!(CAT, imp = self, "Failed to map input buffer {e:?}");
            gst::FlowError::Error
        })?;
        let rtp = match rtp_types::RtpPacket::parse(&mapped) {
            Ok(rtp) => rtp,
            Err(e) => {
                gst::error!(
                    CAT,
                    imp = self,
                    "Failed to parse input as valid rtp packet: {e:?}"
                );
                return Ok(gst::FlowSuccess::Ok);
            }
        };

        let mut session_inner = internal_session.inner.lock().unwrap();

        let mut ssrc_collision: smallvec::SmallVec<[u32; 4]> = smallvec::SmallVec::new();
        loop {
            match session_inner.session.handle_send(&rtp, now) {
                SendReply::SsrcCollision(ssrc) => {
                    if !ssrc_collision.contains(&ssrc) {
                        ssrc_collision.push(ssrc);
                    }
                }
                SendReply::NewSsrc(ssrc, _pt) => {
                    drop(session_inner);
                    internal_session
                        .config
                        .emit_by_name::<()>("new-ssrc", &[&ssrc]);
                    session_inner = internal_session.inner.lock().unwrap();
                }
                SendReply::Passthrough => break,
                SendReply::Drop => return Ok(gst::FlowSuccess::Ok),
            }
        }
        // TODO: handle other processing
        drop(mapped);
        drop(session_inner);

        for ssrc in ssrc_collision {
            // XXX: Another option is to have us rewrite ssrc's instead of asking upstream to do
            // so.
            sinkpad.send_event(
                gst::event::CustomUpstream::builder(
                    gst::Structure::builder("GstRTPCollision")
                        .field("ssrc", ssrc)
                        .build(),
                )
                .build(),
            );
        }

        srcpad.push(buffer)
    }

    fn rtp_sink_chain_list(
        &self,
        pad: &gst::Pad,
        id: usize,
        list: gst::BufferList,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let state = self.state.lock().unwrap();
        let Some(session) = state.session_by_id(id) else {
            gst::error!(CAT, "No session?");
            return Err(gst::FlowError::Error);
        };

        let srcpad = session.rtp_send_srcpad.clone().unwrap();
        let internal_session = session.internal_session.clone();
        drop(state);

        let now = Instant::now();
        for buffer in list.iter_owned() {
            self.handle_buffer(pad, &srcpad, &internal_session, buffer, now)?;
        }
        Ok(gst::FlowSuccess::Ok)
    }

    fn rtp_sink_chain(
        &self,
        pad: &gst::Pad,
        id: usize,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let state = self.state.lock().unwrap();
        let Some(session) = state.session_by_id(id) else {
            gst::error!(CAT, "No session?");
            return Err(gst::FlowError::Error);
        };

        let srcpad = session.rtp_send_srcpad.clone().unwrap();
        let internal_session = session.internal_session.clone();
        drop(state);

        let now = Instant::now();
        self.handle_buffer(pad, &srcpad, &internal_session, buffer, now)
    }

    fn rtp_sink_event(&self, pad: &gst::Pad, event: gst::Event, id: usize) -> bool {
        match event.view() {
            gst::EventView::Caps(caps) => {
                if let Some((pt, clock_rate)) = pt_clock_rate_from_caps(caps.caps()) {
                    let state = self.state.lock().unwrap();
                    if let Some(session) = state.session_by_id(id) {
                        let mut session = session.internal_session.inner.lock().unwrap();
                        session.session.set_pt_clock_rate(pt, clock_rate);
                        session.add_caps(caps.caps_owned());
                    }
                } else {
                    gst::debug!(
                        CAT,
                        obj = pad,
                        "input caps are missing payload or clock-rate fields, need to use pt-map"
                    );
                }
                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            gst::EventView::Eos(_eos) => {
                let now = Instant::now();
                let state = self.state.lock().unwrap();
                if let Some(session) = state.session_by_id(id) {
                    let mut session = session.internal_session.inner.lock().unwrap();
                    let ssrcs = session.session.ssrcs().collect::<Vec<_>>();
                    // We want to bye all relevant ssrc's here.
                    // Relevant means they will not be used by something else which means that any
                    // local send ssrc that is not being used for Sr/Rr reports (internal_ssrc) can
                    // have the Bye state applied.
                    let mut all_local = true;
                    let internal_ssrc = session.session.internal_ssrc();
                    for ssrc in ssrcs {
                        let Some(local_send) = session.session.mut_local_send_source_by_ssrc(ssrc)
                        else {
                            if let Some(local_recv) =
                                session.session.local_receive_source_by_ssrc(ssrc)
                            {
                                if local_recv.state() != SourceState::Bye
                                    && Some(ssrc) != internal_ssrc
                                {
                                    all_local = false;
                                }
                            }
                            continue;
                        };
                        if Some(ssrc) != internal_ssrc {
                            local_send.mark_bye("End of Stream")
                        }
                    }
                    if all_local {
                        // if there are no non-local send ssrc's, then we can Bye the entire
                        // session.
                        session.session.schedule_bye("End of Stream", now);
                    }
                    if let Some(waker) = session.rtcp_waker.take() {
                        waker.wake();
                    }
                    drop(session);
                }
                drop(state);
                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for RtpSend {
    const NAME: &'static str = "GstRtpSend";
    type Type = super::RtpSend;
    type ParentType = gst::Element;

    fn new() -> Self {
        GstRustLogger::install();
        Self {
            settings: Default::default(),
            state: Default::default(),
        }
    }
}

impl ObjectImpl for RtpSend {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecString::builder("rtp-id")
                    .nick("The RTP Connection ID")
                    .blurb("A connection ID shared with a rtprecv element for implementing both sending and receiving using the same RTP context")
                    .default_value("rtp-id")
                    .build(),
                glib::ParamSpecUInt::builder("min-rtcp-interval")
                    .nick("Minimum RTCP interval in ms")
                    .blurb("Minimum time (in ms) between RTCP reports")
                    .default_value(DEFAULT_MIN_RTCP_INTERVAL.as_millis() as u32)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Structure>("stats")
                    .nick("Statistics")
                    .blurb("Statistics about the session")
                    .read_only()
                    .build(),
                glib::ParamSpecEnum::builder::<Profile>("rtp-profile")
                    .nick("RTP Profile")
                    .blurb("RTP Profile to use")
                    .default_value(Profile::default())
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("reduced-size-rtcp")
                    .nick("Reduced Size RTCP")
                    .blurb("Use reduced size RTCP. Only has an effect if rtp-profile=avpf")
                    .default_value(DEFAULT_REDUCED_SIZE_RTCP)
                    .mutable_ready()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "rtp-id" => {
                let mut settings = self.settings.lock().unwrap();
                settings.rtp_id = value.get::<String>().expect("type checked upstream");
            }
            "min-rtcp-interval" => {
                let mut settings = self.settings.lock().unwrap();
                settings.min_rtcp_interval = Duration::from_millis(
                    value.get::<u32>().expect("type checked upstream").into(),
                );
            }
            "rtp-profile" => {
                let mut settings = self.settings.lock().unwrap();
                settings.profile = value.get::<Profile>().expect("Type checked upstream");
            }
            "reduced-size-rtcp" => {
                let mut settings = self.settings.lock().unwrap();
                settings.reduced_size_rtcp = value.get::<bool>().expect("Type checked upstream");
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "rtp-id" => {
                let settings = self.settings.lock().unwrap();
                settings.rtp_id.to_value()
            }
            "min-rtcp-interval" => {
                let settings = self.settings.lock().unwrap();
                (settings.min_rtcp_interval.as_millis() as u32).to_value()
            }
            "stats" => {
                let state = self.state.lock().unwrap();
                state.stats().to_value()
            }
            "rtp-profile" => {
                let settings = self.settings.lock().unwrap();
                settings.profile.to_value()
            }
            "reduced-size-rtcp" => {
                let settings = self.settings.lock().unwrap();
                settings.reduced_size_rtcp.to_value()
            }
            _ => unimplemented!(),
        }
    }

    fn signals() -> &'static [glib::subclass::Signal] {
        static SIGNALS: LazyLock<Vec<glib::subclass::Signal>> = LazyLock::new(|| {
            vec![glib::subclass::Signal::builder("get-session")
                .param_types([u32::static_type()])
                .return_type::<crate::rtpbin2::config::Rtp2Session>()
                .action()
                .class_handler(|args| {
                    let element = args[0].get::<super::RtpSend>().expect("signal arg");
                    let id = args[1].get::<u32>().expect("signal arg");
                    let send = element.imp();
                    let state = send.state.lock().unwrap();
                    state
                        .session_by_id(id as usize)
                        .map(|sess| sess.internal_session.config.to_value())
                })
                .build()]
        });

        SIGNALS.as_ref()
    }
}

impl GstObjectImpl for RtpSend {}

impl ElementImpl for RtpSend {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP Session Sender",
                "Network/RTP/Filter",
                "RTP session management (sender)",
                "Matthew Waters <matthew@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let rtp_caps = gst::Caps::builder_full()
                .structure(gst::Structure::builder("application/x-rtp").build())
                .build();
            let rtcp_caps = gst::Caps::builder_full()
                .structure(gst::Structure::builder("application/x-rtcp").build())
                .build();

            vec![
                gst::PadTemplate::new(
                    "rtp_sink_%u",
                    gst::PadDirection::Sink,
                    gst::PadPresence::Request,
                    &rtp_caps,
                )
                .unwrap(),
                gst::PadTemplate::new(
                    "rtp_src_%u",
                    gst::PadDirection::Src,
                    gst::PadPresence::Sometimes,
                    &rtp_caps,
                )
                .unwrap(),
                gst::PadTemplate::new(
                    "rtcp_src_%u",
                    gst::PadDirection::Src,
                    gst::PadPresence::Request,
                    &rtcp_caps,
                )
                .unwrap(),
            ]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn request_new_pad(
        &self,
        templ: &gst::PadTemplate,
        name: Option<&str>,
        _caps: Option<&gst::Caps>, // XXX: do something with caps?
    ) -> Option<gst::Pad> {
        let settings = self.settings.lock().unwrap().clone();
        let state_clone = self.state.clone();
        let mut state = self.state.lock().unwrap();
        let max_session_id = state.max_session_id;
        let rtp_id = settings.rtp_id.clone();

        // parse the possibly provided name into a session id or use the default
        let sess_parse = move |name: Option<&str>, prefix, default_id| -> Option<usize> {
            if let Some(name) = name {
                name.strip_prefix(prefix).and_then(|suffix| {
                    if suffix.starts_with("%u") {
                        Some(default_id)
                    } else {
                        suffix.parse::<usize>().ok()
                    }
                })
            } else {
                Some(default_id)
            }
        };

        match templ.name_template() {
            "rtp_sink_%u" => sess_parse(name, "rtp_sink_", max_session_id).and_then(|id| {
                let new_pad = move |session: &mut SendSession| -> Option<(
                    gst::Pad,
                    Option<gst::Pad>,
                    usize,
                    Vec<gst::Event>,
                )> {
                    let sinkpad = gst::Pad::builder_from_template(templ)
                        .chain_function(move |pad, parent, buffer| {
                            RtpSend::catch_panic_pad_function(
                                parent,
                                || Err(gst::FlowError::Error),
                                |this| this.rtp_sink_chain(pad, id, buffer),
                            )
                        })
                        .chain_list_function(move |pad, parent, list| {
                            RtpSend::catch_panic_pad_function(
                                parent,
                                || Err(gst::FlowError::Error),
                                |this| this.rtp_sink_chain_list(pad, id, list),
                            )
                        })
                        .iterate_internal_links_function(|pad, parent| {
                            RtpSend::catch_panic_pad_function(
                                parent,
                                || gst::Iterator::from_vec(vec![]),
                                |this| this.iterate_internal_links(pad),
                            )
                        })
                        .event_function(move |pad, parent, event| {
                            RtpSend::catch_panic_pad_function(
                                parent,
                                || false,
                                |this| this.rtp_sink_event(pad, event, id),
                            )
                        })
                        .flags(gst::PadFlags::PROXY_CAPS)
                        .name(format!("rtp_sink_{id}"))
                        .build();
                    let src_templ = self.obj().pad_template("rtp_src_%u").unwrap();
                    let srcpad = gst::Pad::builder_from_template(&src_templ)
                        .iterate_internal_links_function(|pad, parent| {
                            RtpSend::catch_panic_pad_function(
                                parent,
                                || gst::Iterator::from_vec(vec![]),
                                |this| this.iterate_internal_links(pad),
                            )
                        })
                        .name(format!("rtp_src_{id}"))
                        .build();
                    session.rtp_send_sinkpad = Some(sinkpad.clone());
                    session.rtp_send_srcpad = Some(srcpad.clone());
                    session
                        .internal_session
                        .inner
                        .lock()
                        .unwrap()
                        .rtp_send_sinkpad = Some(sinkpad.clone());
                    Some((sinkpad, Some(srcpad), id, vec![]))
                };

                let session = state.mut_session_by_id(id);
                if let Some(session) = session {
                    if session.rtp_send_sinkpad.is_some() {
                        None
                    } else {
                        new_pad(session)
                    }
                } else {
                    let shared_state = state
                        .shared_state
                        .get_or_insert_with(|| SharedRtpState::send_get_or_init(rtp_id));
                    let mut session = SendSession::new(shared_state, id, &settings);
                    let ret = new_pad(&mut session);
                    state.sessions.push(session);
                    ret
                }
            }),
            "rtcp_src_%u" => sess_parse(name, "rtcp_src_", max_session_id).and_then(|id| {
                let new_pad = move |session: &mut SendSession| -> Option<(
                    gst::Pad,
                    Option<gst::Pad>,
                    usize,
                    Vec<gst::Event>,
                )> {
                    let srcpad = gst::Pad::builder_from_template(templ)
                        .iterate_internal_links_function(|pad, parent| {
                            RtpSend::catch_panic_pad_function(
                                parent,
                                || gst::Iterator::from_vec(vec![]),
                                |this| this.iterate_internal_links(pad),
                            )
                        })
                        .name(format!("rtcp_src_{id}"))
                        .build();

                    let stream_id = format!("{id}/rtcp");
                    let stream_start = gst::event::StreamStart::builder(&stream_id).build();
                    let seqnum = stream_start.seqnum();

                    let caps = gst::Caps::new_empty_simple("application/x-rtcp");
                    let caps = gst::event::Caps::builder(&caps).seqnum(seqnum).build();

                    let segment = gst::FormattedSegment::<gst::ClockTime>::new();
                    let segment = gst::event::Segment::new(&segment);

                    session.rtcp_send_srcpad = Some(srcpad.clone());
                    session.start_rtcp_task(state_clone);
                    Some((srcpad, None, id, vec![stream_start, caps, segment]))
                };

                let session = state.mut_session_by_id(id);
                if let Some(session) = session {
                    if session.rtcp_send_srcpad.is_some() {
                        None
                    } else {
                        new_pad(session)
                    }
                } else {
                    let shared_state = state
                        .shared_state
                        .get_or_insert_with(|| SharedRtpState::send_get_or_init(rtp_id));
                    let mut session = SendSession::new(shared_state, id, &settings);
                    let ret = new_pad(&mut session);
                    state.sessions.push(session);
                    ret
                }
            }),
            _ => None,
        }
        .map(|(pad, otherpad, id, sticky_events)| {
            state.max_session_id = (id + 1).max(state.max_session_id);
            state.pads_session_id_map.insert(pad.clone(), id);
            if let Some(ref pad) = otherpad {
                state.pads_session_id_map.insert(pad.clone(), id);
            }

            drop(state);

            pad.set_active(true).unwrap();
            for event in sticky_events {
                let _ = pad.store_sticky_event(&event);
            }
            self.obj().add_pad(&pad).unwrap();

            if let Some(pad) = otherpad {
                pad.set_active(true).unwrap();
                self.obj().add_pad(&pad).unwrap();
            }

            pad
        })
    }

    fn release_pad(&self, pad: &gst::Pad) {
        let mut state = self.state.lock().unwrap();
        let mut removed_pads = vec![];
        let mut removed_session_ids = vec![];
        if let Some(&id) = state.pads_session_id_map.get(pad) {
            removed_pads.push(pad.clone());
            if let Some(session) = state.mut_session_by_id(id) {
                if Some(pad) == session.rtp_send_sinkpad.as_ref() {
                    session.rtp_send_sinkpad = None;
                    session
                        .internal_session
                        .inner
                        .lock()
                        .unwrap()
                        .rtp_send_sinkpad = None;

                    if let Some(srcpad) = session.rtp_send_srcpad.take() {
                        removed_pads.push(srcpad);
                    }
                }

                if Some(pad) == session.rtcp_send_srcpad.as_ref() {
                    session.rtcp_send_srcpad = None;
                }

                if session.rtp_send_sinkpad.is_none() && session.rtcp_send_srcpad.is_none() {
                    removed_session_ids.push(session.internal_session.id);
                }
            }
        }
        drop(state);

        for pad in removed_pads.iter() {
            let _ = pad.set_active(false);
            // Pad might not have been added yet if it's a RTP recv srcpad
            if pad.has_as_parent(&*self.obj()) {
                let _ = self.obj().remove_pad(pad);
            }
        }

        {
            let mut state = self.state.lock().unwrap();

            for pad in removed_pads.iter() {
                state.pads_session_id_map.remove(pad);
            }
            for id in removed_session_ids {
                if let Some(session) = state.session_by_id(id) {
                    if session.rtp_send_sinkpad.is_none() && session.rtcp_send_srcpad.is_none() {
                        session.stop_rtcp_task();
                        state.sessions.retain(|s| s.internal_session.id != id);
                    }
                }
            }
        }

        self.parent_release_pad(pad)
    }

    #[allow(clippy::single_match)]
    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        match transition {
            gst::StateChange::NullToReady => {
                if let Err(err) = rtpbin2::get_or_init_runtime() {
                    self.post_error_message(gst::error_msg!(
                        gst::LibraryError::Settings,
                        ["Error initializing runtime: {err}"]
                    ));
                    return Err(gst::StateChangeError);
                }

                let settings = self.settings.lock().unwrap();
                let rtp_id = settings.rtp_id.clone();
                drop(settings);

                let state_clone = self.state.clone();
                let mut state = self.state.lock().unwrap();
                let empty_sessions = state.sessions.is_empty();
                match state.shared_state.as_mut() {
                    Some(shared) => {
                        if !empty_sessions && shared.name() != rtp_id {
                            let other_name = shared.name().to_owned();
                            drop(state);
                            self.post_error_message(gst::error_msg!(gst::LibraryError::Settings, ["rtp-id {rtp_id} does not match the currently set value {other_name}"]));
                            return Err(gst::StateChangeError);
                        }
                    }
                    None => {
                        state.shared_state = Some(SharedRtpState::send_get_or_init(rtp_id.clone()));
                    }
                }
                for session in state.sessions.iter_mut() {
                    if session.rtcp_send_srcpad.is_some() {
                        session.start_rtcp_task(state_clone.clone());
                    }
                }
            }
            _ => (),
        }
        let success = self.parent_change_state(transition)?;

        match transition {
            gst::StateChange::ReadyToNull => {
                let mut state = self.state.lock().unwrap();
                for session in state.sessions.iter_mut() {
                    session.stop_rtcp_task();
                }
            }
            _ => (),
        }

        Ok(success)
    }
}

impl Drop for RtpSend {
    fn drop(&mut self) {
        if let Some(ref shared_state) = self.state.lock().unwrap().shared_state {
            shared_state.unmark_send_outstanding();
        }
    }
}
