use anyhow::{anyhow, bail, Context};
use async_tungstenite::tungstenite::Message;
use futures::prelude::*;
use futures::{pin_mut, select, select_biased};
use gst::prelude::*;
use tracing::{debug, error, info, trace};
use url::Url;

use std::{ops::ControlFlow, pin::Pin, sync::Arc, thread, time::Duration};

use gst_plugin_webrtc_protocol::{
    IncomingMessage as ToSignaller, OutgoingMessage as FromSignaller, PeerRole, PeerStatus,
};

#[derive(Debug, Default, Clone, clap::Parser)]
struct Args {
    #[clap(long, help = "Initial clock type", default_value = "ntp")]
    pub clock: Clock,

    #[clap(
        long,
        help = "Maximum duration in seconds to wait for clock synchronization",
        default_value = "5"
    )]
    pub clock_sync_timeout: u64,

    #[clap(
        long,
        help = "Expect RFC 7273 PTP or NTP clock & RTP/clock offset signalling"
    )]
    pub expect_clock_signalling: bool,

    #[clap(long, help = "NTP server host", default_value = "pool.ntp.org")]
    pub ntp_server: String,

    #[clap(long, help = "PTP domain", default_value = "0")]
    pub ptp_domain: u32,

    #[clap(long, help = "Pipeline latency (ms)", default_value = "1000")]
    pub pipeline_latency: u64,

    #[clap(long, help = "RTP jitterbuffer latency (ms)", default_value = "40")]
    pub rtp_latency: u32,

    #[clap(
        long,
        help = "Force accepted audio codecs. See 'webrtcsrc' 'audio-codecs' property (ex. 'OPUS'). Accepts several occurrences."
    )]
    pub audio_codecs: Vec<String>,

    #[clap(
        long,
        help = "Force accepted video codecs. See 'webrtcsrc' 'video-codecs' property (ex. 'VP8'). Accepts several occurrences."
    )]
    pub video_codecs: Vec<String>,

    #[clap(long, help = "Signalling server host", default_value = "localhost")]
    pub server: String,

    #[clap(long, help = "Signalling server port", default_value = "8443")]
    pub port: u32,

    #[clap(long, help = "use tls")]
    pub use_tls: bool,
}

impl Args {
    pub fn scheme(&self) -> &str {
        if self.use_tls {
            "wss"
        } else {
            "ws"
        }
    }

    async fn get_synced_clock(&self) -> anyhow::Result<gst::Clock> {
        debug!("Syncing to {:?}", self.clock);

        // Create the requested clock and wait for synchronization.
        let clock = match self.clock {
            Clock::System => gst::SystemClock::obtain(),
            Clock::Ntp => gst_net::NtpClock::new(None, &self.ntp_server, 123, gst::ClockTime::ZERO)
                .upcast::<gst::Clock>(),
            Clock::Ptp => {
                gst_net::PtpClock::init(None, &[])?;
                gst_net::PtpClock::new(None, self.ptp_domain)?.upcast()
            }
        };

        let clock_sync_timeout = gst::ClockTime::from_seconds(self.clock_sync_timeout);
        let clock =
            tokio::task::spawn_blocking(move || -> Result<gst::Clock, gst::glib::BoolError> {
                clock.wait_for_sync(clock_sync_timeout)?;
                Ok(clock)
            })
            .await
            .with_context(|| format!("Syncing to {:?}", self.clock))?
            .with_context(|| format!("Syncing to {:?}", self.clock))?;

        info!("Synced to {:?}", self.clock);

        Ok(clock)
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum Clock {
    #[default]
    Ntp,
    Ptp,
    System,
}

fn spawn_consumer(
    signaller_url: &Url,
    pipeline: &gst::Pipeline,
    args: Arc<Args>,
    peer_id: String,
    meta: Option<serde_json::Value>,
) -> anyhow::Result<()> {
    info!(%peer_id, ?meta, "Spawning consumer");

    let bin = gst::Bin::with_name(&peer_id);
    pipeline.add(&bin).context("Adding consumer bin")?;

    let webrtcsrc = gst::ElementFactory::make("webrtcsrc")
        .name(
            meta.as_ref()
                .map_or_else(|| peer_id.clone(), serde_json::Value::to_string),
        )
        // Discard retransmission in RFC 7273 mode. See:
        // * https://gitlab.freedesktop.org/gstreamer/gst-plugins-good/-/issues/914
        // * https://gitlab.freedesktop.org/gstreamer/gstreamer/-/issues/1574
        .property_if("do-retransmission", false, args.expect_clock_signalling)
        .property_if_not_empty::<gst::Array, _>("audio-codecs", &args.audio_codecs)
        .property_if_not_empty::<gst::Array, _>("video-codecs", &args.video_codecs)
        .build()
        .context("Creating webrtcsrc")?;

    bin.add(&webrtcsrc).context("Adding webrtcsrc")?;

    let signaller = webrtcsrc.property::<gst::glib::Object>("signaller");
    signaller.set_property("uri", signaller_url.as_str());
    signaller.set_property("producer-peer-id", &peer_id);

    signaller.connect("webrtcbin-ready", false, {
        let cli_args = args.clone();
        let peer_id = peer_id.clone();
        let meta = meta.clone();
        move |args| {
            let webrtcbin = args[2].get::<gst::Element>().unwrap();
            webrtcbin.set_property("latency", cli_args.rtp_latency);

            let rtpbin = webrtcbin
                .downcast_ref::<gst::Bin>()
                .unwrap()
                .by_name("rtpbin")
                .unwrap();

            rtpbin.set_property("add-reference-timestamp-meta", true);

            // Configure for network synchronization via the RTP NTP timestamps.
            // This requires that sender and receiver are synchronized to the same
            // clock.
            rtpbin.set_property_from_str("buffer-mode", "synced");

            if cli_args.expect_clock_signalling {
                // Synchronize incoming packets using signalled RFC 7273 clock.
                rtpbin.set_property("rfc7273-sync", true);
            } else if cli_args.clock == Clock::Ntp {
                rtpbin.set_property("ntp-sync", true);
            }

            // Don't bother updating inter-stream offsets if the difference to the previous
            // configuration is less than 1ms. The synchronization will have rounding errors
            // in the range of the RTP clock rate, i.e. 1/90000s and 1/48000s in this case
            rtpbin.set_property("min-ts-offset", gst::ClockTime::MSECOND);

            // Log a couple of media details
            rtpbin.call_async({
                let cli_args = cli_args.clone();
                let peer_id = peer_id.clone();
                let meta = meta.clone();
                move |rtpbin| {
                    // Wait for actual streaming and clocks to sync
                    thread::sleep(Duration::from_secs(cli_args.clock_sync_timeout));

                    let _ = rtpbin
                        .downcast_ref::<gst::Bin>()
                        .unwrap()
                        .iterate_all_by_element_factory_name("rtpjitterbuffer")
                        .foreach(|jbuf| {
                            let stats = jbuf.property::<gst::Structure>("stats");
                            // stats.rfc7273-active is available since GStreamer 1.26
                            let rfc7273_active =
                                stats.get_optional::<bool>("rfc7273-active").ok().flatten();

                            let ptdemux = jbuf
                                .static_pad("src")
                                .unwrap()
                                .peer()
                                .expect("rtpptdemux linked at this point")
                                .parent_element()
                                .unwrap();
                            ptdemux.foreach_src_pad(|_, pad| {
                                if let Some(caps) = pad.current_caps() {
                                    let s = caps.structure(0).unwrap();
                                    let ssrc = s.get::<u32>("ssrc").unwrap();
                                    let mid = s.get::<&str>("a-mid").unwrap();
                                    let encoding_name = s.get::<&str>("encoding-name").unwrap();

                                    if let Some(rfc7273_active) = rfc7273_active {
                                        let ts_refclk =
                                            s.get_optional::<&str>("a-ts-refclk").unwrap();
                                        let mediaclk =
                                            s.get_optional::<&str>("a-mediaclk").unwrap();
                                        info!(
                                            %peer_id, ?meta, %ssrc, %mid, %encoding_name,
                                            %rfc7273_active, ?ts_refclk, ?mediaclk,
                                            "media info"
                                        );
                                    } else {
                                        info!(%peer_id, ?meta, %ssrc, %mid, %encoding_name, "media info");
                                    }
                                }

                                ControlFlow::Continue(())
                            });
                        });
                }
            });

            None
        }
    });

    webrtcsrc.connect_pad_added({
        move |webrtcsrc, pad| {
            let Some(bin) = webrtcsrc
                .parent()
                .map(|b| b.downcast::<gst::Bin>().expect("webrtcsrc added to a bin"))
            else {
                return;
            };

            if pad.name().starts_with("audio") {
                let audioconvert = gst::ElementFactory::make("audioconvert")
                    .build()
                    .expect("Checked in prepare()");
                let audioresample = gst::ElementFactory::make("audioresample")
                    .build()
                    .expect("Checked in prepare()");
                // Decouple processing from sync a bit
                let queue = gst::ElementFactory::make("queue")
                    .property("max-size-buffers", 1u32)
                    .property("max-size-bytes", 0u32)
                    .property("max-size-time", 0u64)
                    .build()
                    .expect("Checked in prepare()");
                let audiosink = gst::ElementFactory::make("autoaudiosink")
                    .build()
                    .expect("Checked in prepare()");
                bin.add_many([&audioconvert, &audioresample, &queue, &audiosink])
                    .unwrap();

                pad.link(&audioconvert.static_pad("sink").unwrap()).unwrap();
                gst::Element::link_many([&audioconvert, &audioresample, &queue, &audiosink])
                    .unwrap();

                audiosink.sync_state_with_parent().unwrap();
                queue.sync_state_with_parent().unwrap();
                audioresample.sync_state_with_parent().unwrap();
                audioconvert.sync_state_with_parent().unwrap();
            } else if pad.name().starts_with("video") {
                use std::sync::atomic::{AtomicBool, Ordering};

                // Create a timeoverlay element to render the timestamps from
                // the reference timestamp metadata on top of the video frames
                // in the bottom left.
                //
                // Also add a pad probe on its sink pad to log the same timestamp to
                // stdout on each frame.
                let timeoverlay = gst::ElementFactory::make("timeoverlay")
                    .property_from_str("time-mode", "reference-timestamp")
                    .property_from_str("valignment", "bottom")
                    .build()
                    .expect("Checked in prepare()");
                let sinkpad = timeoverlay
                    .static_pad("video_sink")
                    .expect("Failed to get timeoverlay sinkpad");
                let ref_ts_caps_set = AtomicBool::new(false);
                sinkpad
                    .add_probe(gst::PadProbeType::BUFFER, {
                        let timeoverlay = timeoverlay.downgrade();
                        move |_pad, info| {
                        if let Some(gst::PadProbeData::Buffer(ref buffer)) = info.data {
                            if let Some(meta) = buffer.meta::<gst::ReferenceTimestampMeta>() {
                                if !ref_ts_caps_set.fetch_or(true, Ordering::SeqCst) {
                                    if let Some(timeoverlay) = timeoverlay.upgrade() {
                                        let reference = meta.reference();
                                        timeoverlay.set_property("reference-timestamp-caps", reference.to_owned());

                                        info!(%reference, timestamp = %meta.timestamp(), "Have sender clock time");
                                    }
                                } else {
                                    trace!(timestamp = %meta.timestamp(), "Have sender clock time");
                                }
                            } else {
                                trace!("Have no sender clock time");
                            }
                        }

                        gst::PadProbeReturn::Ok
                    }})
                    .expect("Failed to add timeoverlay pad probe");

                let videoconvert = gst::ElementFactory::make("videoconvert")
                    .build()
                    .expect("Checked in prepare()");
                // Decouple processing from sync a bit
                let queue = gst::ElementFactory::make("queue")
                    .property("max-size-buffers", 1u32)
                    .property("max-size-bytes", 0u32)
                    .property("max-size-time", 0u64)
                    .build()
                    .expect("Checked in prepare()");
                let videosink = gst::ElementFactory::make("autovideosink")
                    .build()
                    .expect("Checked in prepare()");

                bin.add_many([&timeoverlay, &videoconvert, &queue, &videosink])
                    .unwrap();

                pad.link(&sinkpad).unwrap();
                gst::Element::link_many([&timeoverlay, &videoconvert, &queue, &videosink]).unwrap();

                videosink.sync_state_with_parent().unwrap();
                queue.sync_state_with_parent().unwrap();
                videoconvert.sync_state_with_parent().unwrap();
                timeoverlay.sync_state_with_parent().unwrap();
            }
        }
    });

    signaller.connect("session-ended", true, {
        let bin = bin.downgrade();
        move |_| {
            info!(%peer_id, ?meta, "Session ended");

            let Some(bin) = bin.upgrade() else {
                return Some(false.into());
            };

            bin.call_async(|bin| {
                let _ = bin.set_state(gst::State::Null);

                if let Some(pipeline) = bin.parent().map(|p| {
                    p.downcast::<gst::Pipeline>()
                        .expect("Bin added to the pipeline")
                }) {
                    let _ = pipeline.remove(bin);
                }
            });

            Some(false.into())
        }
    });

    bin.sync_state_with_parent()
        .context("Syncing consumer bin with pipeline")?;

    Ok(())
}

#[derive(Debug, Default)]
struct App {
    args: Arc<Args>,
    pipeline: Option<gst::Pipeline>,
    listener_abort_hdl: Option<future::AbortHandle>,
    listener_task_hdl: Option<future::Fuse<tokio::task::JoinHandle<anyhow::Result<()>>>>,
}

impl App {
    fn new(args: Args) -> Self {
        App {
            args: args.into(),
            ..Default::default()
        }
    }

    #[inline(always)]
    fn pipeline(&self) -> &gst::Pipeline {
        self.pipeline.as_ref().expect("Set in prepare")
    }

    async fn prepare_and_run(&mut self) -> anyhow::Result<()> {
        self.prepare().await.context("Preparing")?;
        self.run().await.context("Running")?;

        Ok(())
    }

    async fn prepare(&mut self) -> anyhow::Result<()> {
        debug!("Preparing");

        // Check availability of elements which might be created in webrtcsrc.connect_pad_added()
        let mut missing_elements = String::new();
        for elem in [
            "queue",
            "audioconvert",
            "audioresample",
            "autovideosink",
            "timeoverlay",
            "videoconvert",
            "autovideosink",
        ] {
            if gst::ElementFactory::find(elem).is_none() {
                missing_elements.push_str("\n\t- ");
                missing_elements.push_str(elem);
            }
        }
        if !missing_elements.is_empty() {
            bail!("Missing elements:{}", missing_elements);
        }

        self.pipeline = Some(gst::Pipeline::new());
        self.pipeline()
            .use_clock(Some(&self.args.get_synced_clock().await?));

        // Set the base time of the pipeline statically to zero so that running
        // time and clock time are the same. This easies debugging.
        self.pipeline().set_base_time(gst::ClockTime::ZERO);
        self.pipeline().set_start_time(gst::ClockTime::NONE);

        // Configure a static latency (1s by default).
        // This needs to be higher than the sum of the sender latency and
        // the receiver latency of the receiver with the highest latency.
        // As this can't be known automatically and depends on many factors,
        // this has to be known for the overall system and configured accordingly.
        self.pipeline()
            .set_latency(gst::ClockTime::from_mseconds(self.args.pipeline_latency));

        let signaller_url = Url::parse(&format!(
            "{}://{}:{}",
            self.args.scheme(),
            self.args.server,
            self.args.port,
        ))?;

        let (signaller_tx, signaller_rx) = connect_as_listener(&signaller_url)
            .await
            .context("Connecting as listener")?;

        let (listener_abort_hdl, listener_abort_reg) = future::AbortHandle::new_pair();
        self.listener_abort_hdl = Some(listener_abort_hdl);

        self.listener_task_hdl = Some(
            tokio::task::spawn(listener_task(
                listener_abort_reg,
                signaller_tx,
                signaller_rx,
                signaller_url,
                self.pipeline().clone(),
                self.args.clone(),
            ))
            .fuse(),
        );

        Ok(())
    }

    async fn run(&mut self) -> anyhow::Result<()> {
        debug!("Running");

        let bus = self.pipeline().bus().context("Getting the pipeline bus")?;
        let mut bus_stream = bus.stream();

        self.pipeline()
            .call_async_future(|pipeline| pipeline.set_state(gst::State::Playing))
            .await
            .context("Setting pipeline to Playing")?;

        loop {
            select_biased! {
                // Don't take listener_task_hdl: we will need it in teardown()
                listener_res = self.listener_task_hdl.as_mut().expect("defined in prepare") => {
                    listener_res.context("Signaller listener task")??;

                    info!("Breaking due to signaller listener termination");
                    break;
                },
                bus_msg = bus_stream.next() => {
                    use gst::MessageView::*;

                    let Some(msg) = bus_msg else { break };
                    match msg.view() {
                        Error(msg) => {
                            let err = msg.error();
                            let src_name = msg.src().map(|src| src.name());

                            bail!(
                                "Element {} error message: {err:#}",
                                src_name.as_deref().unwrap_or("UNKNOWN"),
                            );
                        }
                        Latency(msg) => {
                            info!(
                                "Latency requirements have changed for element {}",
                                msg.src().map(|src| src.name()).as_deref().unwrap_or("UNKNOWN"),
                            );
                            if let Err(err) = self.pipeline().recalculate_latency() {
                                error!(%err, "Error recalculating latency");
                            }
                        }
                        _ => (),
                    }
                }
            }
        }

        Ok(())
    }

    /// Tears this `App` down and deallocates all its resources by consuming `self`.
    async fn teardown(mut self) {
        debug!("Tearing down");

        if let Some(ref pipeline) = self.pipeline {
            // For debugging purposes:
            // define the GST_DEBUG_DUMP_DOT_DIR env var to generate a dot file.
            pipeline.debug_to_dot_file(
                gst::DebugGraphDetails::all(),
                "webrtc-precise-sync-recv-tearing-down",
            );
        }

        if let Some(listener_abort_hdl) = self.listener_abort_hdl.take() {
            listener_abort_hdl.abort();
        }

        if let Some(pipeline) = self.pipeline.take() {
            let _ = pipeline
                .call_async_future(|pipeline| pipeline.set_state(gst::State::Null))
                .await;
        }

        if let Some(listener_task_hdl) = self.listener_task_hdl.take() {
            use future::FusedFuture;
            if !listener_task_hdl.is_terminated() {
                let _ = listener_task_hdl.await;
            }
        }
    }
}

async fn connect_as_listener(
    signaller_url: &Url,
) -> anyhow::Result<(
    Pin<Box<impl Sink<ToSignaller, Error = anyhow::Error>>>,
    Pin<Box<impl Stream<Item = anyhow::Result<FromSignaller>>>>,
)> {
    async fn register(
        mut signaller_tx: Pin<&mut impl Sink<ToSignaller, Error = anyhow::Error>>,
        mut signaller_rx: Pin<&mut impl Stream<Item = anyhow::Result<FromSignaller>>>,
    ) -> anyhow::Result<()> {
        match signaller_rx
            .next()
            .await
            .unwrap_or_else(|| Err(anyhow!("Signaller ended session")))
            .context("Expecting Welcome")?
        {
            FromSignaller::Welcome { peer_id } => {
                info!(%peer_id, "Got Welcomed by signaller");
            }
            other => bail!("Expected Welcome, got {other:?}"),
        }

        debug!("Registering as listener");

        signaller_tx
            .send(ToSignaller::SetPeerStatus(PeerStatus {
                roles: vec![PeerRole::Listener],
                ..Default::default()
            }))
            .await
            .context("Sending SetPeerStatus")?;

        loop {
            let msg = signaller_rx
                .next()
                .await
                .unwrap_or_else(|| Err(anyhow!("Signaller ended session")))
                .context("SetPeerStatus response")?;

            match msg {
                FromSignaller::PeerStatusChanged(peer_status) if peer_status.listening() => break,
                FromSignaller::EndSession(_) => bail!("Signaller ended session unexpectedly"),
                _ => (),
            }
        }

        Ok(())
    }

    debug!("Connecting to Signaller");

    let (ws, _) = async_tungstenite::tokio::connect_async(signaller_url)
        .await
        .context("Connecting to signaller")?;
    let (ws_tx, ws_rx) = ws.split();

    let mut signaller_tx = Box::pin(ws_tx.sink_err_into::<anyhow::Error>().with(
        |msg: ToSignaller| {
            future::ok(Message::text(
                serde_json::to_string(&msg).expect("msg is serializable"),
            ))
        },
    ));

    let mut signaller_rx = Box::pin(ws_rx.filter_map(|msg| {
        future::ready(match msg {
            Ok(Message::Text(msg)) => match serde_json::from_str::<FromSignaller>(&msg) {
                Ok(message) => Some(Ok(message)),
                Err(err) => Some(Err(anyhow!(
                    "Failed to deserialize signaller message: {err:#}",
                ))),
            },
            Ok(Message::Close(_)) => Some(Err(anyhow!("Connection closed"))),
            Ok(Message::Ping(_)) => None,
            Ok(other) => Some(Err(anyhow!("Unexpected message {other:?}"))),
            Err(err) => Some(Err(err.into())),
        })
    }));

    if let Err(err) = register(signaller_tx.as_mut(), signaller_rx.as_mut())
        .await
        .context("Registering as listener")
    {
        debug!("Closing signaller websocket due to {err:#}");
        let _ = signaller_tx.close().await;

        return Err(err);
    }

    Ok((signaller_tx, signaller_rx))
}

async fn listen(
    signaller_tx: &mut Pin<Box<impl Sink<ToSignaller, Error = anyhow::Error>>>,
    mut signaller_rx: Pin<Box<impl Stream<Item = anyhow::Result<FromSignaller>>>>,
    signaller_url: Url,
    pipeline: gst::Pipeline,
    args: Arc<Args>,
) -> anyhow::Result<()> {
    debug!("Looking for already registered producers");

    signaller_tx
        .send(ToSignaller::List)
        .await
        .context("Sending List")?;

    loop {
        match signaller_rx
            .next()
            .await
            .unwrap_or_else(|| Err(anyhow!("Signaller ended session")))
            .context("List response")?
        {
            FromSignaller::List { producers } => {
                for peer in producers {
                    spawn_consumer(&signaller_url, &pipeline, args.clone(), peer.id, peer.meta)
                        .context("Spawning consumer")?;
                }
                break;
            }
            FromSignaller::EndSession(_) => bail!("Signaller ended session unexpectedly"),
            _ => (),
        }
    }

    debug!("Listening to signaller");

    loop {
        match signaller_rx
            .next()
            .await
            .unwrap_or_else(|| Err(anyhow!("Signaller ended session")))
            .context("Listening to signaller")?
        {
            FromSignaller::PeerStatusChanged(peer_status) if peer_status.producing() => {
                spawn_consumer(
                    &signaller_url,
                    &pipeline,
                    args.clone(),
                    peer_status.peer_id.expect("producer with peer_id"),
                    peer_status.meta,
                )
                .context("Spawning consumer")?;
            }
            FromSignaller::EndSession(_) => {
                info!("Signaller ended session");
                break;
            }
            other => trace!(msg = ?other, "Ignoring signaller message"),
        }
    }

    Ok(())
}

/// Wrapper around the listener.
///
/// Ensures the websocket is properly closed even if an error occurs or
/// the listener is aborted.
async fn listener_task(
    abort_reg: future::AbortRegistration,
    mut signaller_tx: Pin<Box<impl Sink<ToSignaller, Error = anyhow::Error>>>,
    signaller_rx: Pin<Box<impl Stream<Item = anyhow::Result<FromSignaller>>>>,
    signaller_url: Url,
    pipeline: gst::Pipeline,
    args: Arc<Args>,
) -> anyhow::Result<()> {
    let res = future::Abortable::new(
        listen(
            &mut signaller_tx,
            signaller_rx,
            signaller_url,
            pipeline,
            args,
        ),
        abort_reg,
    )
    .await;

    debug!("Closing signaller websocket");
    let _ = signaller_tx.close().await;

    if let Ok(listener_res) = res {
        if listener_res.is_err() {
            listener_res?;
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use clap::Parser;
    use tracing_subscriber::prelude::*;

    let args = Args::parse();

    tracing_log::LogTracer::init().context("Setting logger")?;
    let env_filter = tracing_subscriber::EnvFilter::try_from_env("WEBRTC_PRECISE_SYNC_RECV_LOG")
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
    tracing::subscriber::set_global_default(subscriber).context("Setting tracing subscriber")?;

    gst::init()?;
    gstrswebrtc::plugin_register_static()?;
    gstrsrtp::plugin_register_static()?;

    debug!("Starting");

    let mut res = Ok(());
    let mut app = App::new(args);

    {
        let ctrl_c = tokio::signal::ctrl_c().fuse();
        pin_mut!(ctrl_c);

        let prepare_and_run = app.prepare_and_run().fuse();
        pin_mut!(prepare_and_run);

        select! {
            _ctrl_c_res = ctrl_c => {
                info!("Shutting down due to user request");
            }
            app_res = prepare_and_run => {
                if let Err(ref err) = app_res {
                    error!("Shutting down due to application error: {err:#}");
                } else {
                    info!("Shutting down due to application termination");
                }

                res = app_res;
            }
        }
    }

    app.teardown().await;

    debug!("Quitting");

    unsafe {
        // Needed for certain tracers to write data
        gst::deinit();
    }

    res
}
