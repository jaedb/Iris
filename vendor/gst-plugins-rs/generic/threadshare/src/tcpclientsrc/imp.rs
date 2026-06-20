// Copyright (C) 2018 Sebastian Dröge <sebastian@centricular.com>
// Copyright (C) 2018 LEE Dongjun <redongjun@gmail.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use futures::prelude::*;

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use std::sync::LazyLock;

use std::io;
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::sync::Mutex;
use std::time::Duration;

use crate::runtime::prelude::*;
use crate::runtime::task;
use crate::runtime::{Context, PadSrc, Task, TaskState};

use crate::runtime::Async;
use crate::socket::{Socket, SocketError, SocketRead};
use futures::channel::mpsc::{channel, Receiver, Sender};
use futures::pin_mut;

const DEFAULT_HOST: Option<&str> = Some("127.0.0.1");
const DEFAULT_PORT: i32 = 4953;
const DEFAULT_CAPS: Option<gst::Caps> = None;
const DEFAULT_BLOCKSIZE: u32 = 4096;
const DEFAULT_CONTEXT: &str = "";
const DEFAULT_CONTEXT_WAIT: Duration = Duration::ZERO;

#[derive(Debug, Default)]
struct State {
    event_sender: Option<Sender<gst::Event>>,
}

#[derive(Debug, Clone)]
struct Settings {
    host: Option<String>,
    port: i32,
    caps: Option<gst::Caps>,
    blocksize: u32,
    context: String,
    context_wait: Duration,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            host: DEFAULT_HOST.map(Into::into),
            port: DEFAULT_PORT,
            caps: DEFAULT_CAPS,
            blocksize: DEFAULT_BLOCKSIZE,
            context: DEFAULT_CONTEXT.into(),
            context_wait: DEFAULT_CONTEXT_WAIT,
        }
    }
}

struct TcpClientReader(Async<TcpStream>);

impl TcpClientReader {
    pub fn new(socket: Async<TcpStream>) -> Self {
        TcpClientReader(socket)
    }
}

impl SocketRead for TcpClientReader {
    const DO_TIMESTAMP: bool = false;

    async fn read<'buf>(
        &'buf mut self,
        buffer: &'buf mut [u8],
    ) -> io::Result<(usize, Option<std::net::SocketAddr>)> {
        Ok((self.0.read(buffer).await?, None))
    }
}

#[derive(Clone, Debug)]
struct TcpClientSrcPadHandler;

impl PadSrcHandler for TcpClientSrcPadHandler {
    type ElementImpl = TcpClientSrc;

    fn src_event(self, pad: &gst::Pad, imp: &TcpClientSrc, event: gst::Event) -> bool {
        gst::log!(CAT, obj = pad, "Handling {:?}", event);

        use gst::EventView;
        let ret = match event.view() {
            EventView::FlushStart(..) => {
                imp.task.flush_start().block_on_or_add_subtask(pad).is_ok()
            }
            EventView::FlushStop(..) => imp.task.flush_stop().block_on_or_add_subtask(pad).is_ok(),
            EventView::Reconfigure(..) => true,
            EventView::Latency(..) => true,
            _ => false,
        };

        if ret {
            gst::log!(CAT, obj = pad, "Handled {:?}", event);
        } else {
            gst::log!(CAT, obj = pad, "Didn't handle {:?}", event);
        }

        ret
    }

    fn src_query(self, pad: &gst::Pad, imp: &TcpClientSrc, query: &mut gst::QueryRef) -> bool {
        gst::log!(CAT, obj = pad, "Handling {:?}", query);

        use gst::QueryViewMut;
        let ret = match query.view_mut() {
            QueryViewMut::Latency(q) => {
                let latency =
                    gst::ClockTime::try_from(imp.settings.lock().unwrap().context_wait).unwrap();
                gst::debug!(CAT, obj = pad, "Reporting latency {latency}");
                q.set(true, latency, gst::ClockTime::NONE);
                true
            }
            QueryViewMut::Scheduling(q) => {
                q.set(gst::SchedulingFlags::SEQUENTIAL, 1, -1, 0);
                q.add_scheduling_modes([gst::PadMode::Push]);
                true
            }
            QueryViewMut::Caps(q) => {
                let caps = if let Some(caps) = imp.configured_caps.lock().unwrap().as_ref() {
                    q.filter()
                        .map(|f| f.intersect_with_mode(caps, gst::CapsIntersectMode::First))
                        .unwrap_or_else(|| caps.clone())
                } else {
                    q.filter()
                        .map(|f| f.to_owned())
                        .unwrap_or_else(gst::Caps::new_any)
                };

                q.set_result(&caps);

                true
            }
            _ => false,
        };

        if ret {
            gst::log!(CAT, obj = pad, "Handled {:?}", query);
        } else {
            gst::log!(CAT, obj = pad, "Didn't handle {:?}", query);
        }

        ret
    }
}

struct TcpClientSrcTask {
    element: super::TcpClientSrc,
    saddr: SocketAddr,
    buffer_pool: Option<gst::BufferPool>,
    socket: Option<Socket<TcpClientReader>>,
    need_initial_events: bool,
    need_segment: bool,
    event_receiver: Receiver<gst::Event>,
}

impl TcpClientSrcTask {
    fn new(
        element: super::TcpClientSrc,
        saddr: SocketAddr,
        buffer_pool: gst::BufferPool,
        event_receiver: Receiver<gst::Event>,
    ) -> Self {
        TcpClientSrcTask {
            element,
            saddr,
            buffer_pool: Some(buffer_pool),
            socket: None,
            need_initial_events: true,
            need_segment: true,
            event_receiver,
        }
    }

    async fn push_buffer(
        &mut self,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(CAT, obj = self.element, "Handling {:?}", buffer);

        let tcpclientsrc = self.element.imp();

        if self.need_initial_events {
            gst::debug!(CAT, obj = self.element, "Pushing initial events");

            let stream_id = format!("{:08x}{:08x}", rand::random::<u32>(), rand::random::<u32>());
            let stream_start_evt = gst::event::StreamStart::builder(&stream_id)
                .group_id(gst::GroupId::next())
                .build();
            tcpclientsrc.src_pad.push_event(stream_start_evt).await;

            let caps = tcpclientsrc.settings.lock().unwrap().caps.clone();
            if let Some(caps) = caps {
                tcpclientsrc
                    .src_pad
                    .push_event(gst::event::Caps::new(&caps))
                    .await;
                *tcpclientsrc.configured_caps.lock().unwrap() = Some(caps);
            }

            self.need_initial_events = false;
        }

        if self.need_segment {
            let segment_evt =
                gst::event::Segment::new(&gst::FormattedSegment::<gst::format::Time>::new());
            tcpclientsrc.src_pad.push_event(segment_evt).await;

            self.need_segment = false;
        }

        if buffer.size() == 0 {
            tcpclientsrc
                .src_pad
                .push_event(gst::event::Eos::new())
                .await;
            return Ok(gst::FlowSuccess::Ok);
        }

        let res = tcpclientsrc.src_pad.push(buffer).await;
        match res {
            Ok(_) => {
                gst::log!(CAT, obj = self.element, "Successfully pushed buffer");
            }
            Err(gst::FlowError::Flushing) => {
                gst::debug!(CAT, obj = self.element, "Flushing");
            }
            Err(gst::FlowError::Eos) => {
                gst::debug!(CAT, obj = self.element, "EOS");
                tcpclientsrc
                    .src_pad
                    .push_event(gst::event::Eos::new())
                    .await;
            }
            Err(err) => {
                gst::error!(CAT, obj = self.element, "Got error {}", err);
                gst::element_error!(
                    self.element,
                    gst::StreamError::Failed,
                    ("Internal data stream error"),
                    ["streaming stopped, reason {}", err]
                );
            }
        }

        res
    }
}

impl TaskImpl for TcpClientSrcTask {
    type Item = gst::Buffer;

    fn obj(&self) -> &impl IsA<glib::Object> {
        &self.element
    }

    async fn prepare(&mut self) -> Result<(), gst::ErrorMessage> {
        gst::log!(
            CAT,
            obj = self.element,
            "Preparing task connecting to {:?}",
            self.saddr
        );

        let socket = Async::<TcpStream>::connect(self.saddr)
            .await
            .map_err(|err| {
                gst::error_msg!(
                    gst::ResourceError::OpenRead,
                    ["Failed to connect to {:?}: {:?}", self.saddr, err]
                )
            })?;

        self.socket = Some(
            Socket::try_new(
                self.element.clone().upcast(),
                self.buffer_pool.take().unwrap(),
                TcpClientReader::new(socket),
            )
            .map_err(|err| {
                gst::error_msg!(
                    gst::ResourceError::OpenRead,
                    ["Failed to prepare socket {:?}", err]
                )
            })?,
        );

        gst::log!(CAT, obj = self.element, "Task prepared");
        Ok(())
    }

    async fn handle_action_error(
        &mut self,
        trigger: task::Trigger,
        state: TaskState,
        err: gst::ErrorMessage,
    ) -> task::Trigger {
        match trigger {
            task::Trigger::Prepare => {
                gst::error!(CAT, "Task preparation failed: {:?}", err);
                self.element.post_error_message(err);

                task::Trigger::Error
            }
            other => unreachable!("Action error for {:?} in state {:?}", other, state),
        }
    }

    async fn try_next(&mut self) -> Result<gst::Buffer, gst::FlowError> {
        let event_fut = self.event_receiver.next().fuse();
        let socket_fut = self.socket.as_mut().unwrap().try_next().fuse();

        pin_mut!(event_fut);
        pin_mut!(socket_fut);

        futures::select! {
            event_res = event_fut => match event_res {
                Some(event) => {
                    gst::debug!(CAT, obj = self.element, "Handling element level event {event:?}");

                    match event.view() {
                        gst::EventView::Eos(_) => Err(gst::FlowError::Eos),
                        ev => {
                            gst::error!(CAT, obj = self.element, "Unexpected event {ev:?} on channel");
                            Err(gst::FlowError::Error)
                        }
                    }
                }
                None => {
                    gst::error!(CAT, obj = self.element, "Unexpected return on event channel");
                    Err(gst::FlowError::Error)
                }
            },
            socket_res = socket_fut => match socket_res {
                Ok((buffer, _saddr)) => Ok(buffer),
                Err(err) => {
                    gst::error!(CAT, obj = self.element, "Got error {err:#}");

                    match err {
                        SocketError::Gst(err) => {
                            gst::element_error!(
                                self.element,
                                gst::StreamError::Failed,
                                ("Internal data stream error"),
                                ["streaming stopped, reason {err}"]
                            );
                        }
                        SocketError::Io(err) => {
                            gst::element_error!(
                                self.element,
                                gst::StreamError::Failed,
                                ("I/O error"),
                                ["streaming stopped, I/O error {err}"]
                            );
                        }
                    }

                    Err(gst::FlowError::Error)
                }
            },
        }
    }

    async fn handle_item(&mut self, buffer: gst::Buffer) -> Result<(), gst::FlowError> {
        let _ = self.push_buffer(buffer).await?;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), gst::ErrorMessage> {
        gst::log!(CAT, obj = self.element, "Stopping task");
        self.need_initial_events = true;
        gst::log!(CAT, obj = self.element, "Task stopped");
        Ok(())
    }

    async fn flush_stop(&mut self) -> Result<(), gst::ErrorMessage> {
        gst::log!(CAT, obj = self.element, "Stopping task flush");
        self.need_initial_events = true;
        gst::log!(CAT, obj = self.element, "Task flush stopped");
        Ok(())
    }

    async fn handle_loop_error(&mut self, err: gst::FlowError) -> task::Trigger {
        match err {
            gst::FlowError::Flushing => {
                gst::debug!(CAT, obj = self.element, "Flushing");

                task::Trigger::FlushStart
            }
            gst::FlowError::Eos => {
                gst::debug!(CAT, obj = self.element, "EOS");
                self.element
                    .imp()
                    .src_pad
                    .push_event(gst::event::Eos::new())
                    .await;

                task::Trigger::Stop
            }
            err => {
                gst::error!(CAT, obj = self.element, "Got error {err}");
                gst::element_error!(
                    &self.element,
                    gst::StreamError::Failed,
                    ("Internal data stream error"),
                    ["streaming stopped, reason {}", err]
                );

                task::Trigger::Error
            }
        }
    }
}

pub struct TcpClientSrc {
    src_pad: PadSrc,
    task: Task,
    configured_caps: Mutex<Option<gst::Caps>>,
    settings: Mutex<Settings>,
    state: Mutex<State>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "ts-tcpclientsrc",
        gst::DebugColorFlags::empty(),
        Some("Thread-sharing TCP Client source"),
    )
});

impl TcpClientSrc {
    fn prepare(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp = self, "Preparing");
        let settings = self.settings.lock().unwrap().clone();

        let context =
            Context::acquire(&settings.context, settings.context_wait).map_err(|err| {
                gst::error_msg!(
                    gst::ResourceError::OpenRead,
                    ["Failed to acquire Context: {}", err]
                )
            })?;

        *self.configured_caps.lock().unwrap() = None;

        let host: IpAddr = match settings.host {
            None => {
                return Err(gst::error_msg!(
                    gst::ResourceError::Settings,
                    ["No host set"]
                ));
            }
            Some(ref host) => match host.parse() {
                Err(err) => {
                    return Err(gst::error_msg!(
                        gst::ResourceError::Settings,
                        ["Invalid host '{}' set: {}", host, err]
                    ));
                }
                Ok(host) => host,
            },
        };
        let port = settings.port;

        let buffer_pool = gst::BufferPool::new();
        let mut config = buffer_pool.config();
        config.set_params(None, settings.blocksize, 0, 0);
        buffer_pool.set_config(config).map_err(|_| {
            gst::error_msg!(
                gst::ResourceError::Settings,
                ["Failed to configure buffer pool"]
            )
        })?;

        let saddr = SocketAddr::new(host, port as u16);

        let (sender, receiver) = channel(1);

        // Don't block on `prepare` as the socket connection takes time.
        // This will be performed in the background and we'll block on
        // `start` which will also ensure `prepare` completed successfully.
        let fut = self
            .task
            .prepare(
                TcpClientSrcTask::new(self.obj().clone(), saddr, buffer_pool, receiver),
                context,
            )
            .check()?;
        drop(fut);

        let mut state = self.state.lock().unwrap();
        state.event_sender = Some(sender);
        drop(state);

        gst::debug!(CAT, imp = self, "Preparing asynchronously");

        Ok(())
    }

    fn unprepare(&self) {
        gst::debug!(CAT, imp = self, "Unpreparing");
        let _ = self
            .task
            .unprepare()
            .block_on_or_add_subtask_then(self.obj(), |elem, _| {
                gst::debug!(CAT, obj = elem, "Unprepared");
            });
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp = self, "Stopping");
        self.task
            .stop()
            .block_on_or_add_subtask_then(self.obj(), |elem, res| {
                if res.is_ok() {
                    gst::debug!(CAT, obj = elem, "Stopped");
                }
            })
    }

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp = self, "Starting");
        self.task
            .start()
            .block_on_or_add_subtask_then(self.obj(), |elem, res| {
                if res.is_ok() {
                    gst::debug!(CAT, obj = elem, "Started");
                }
            })
    }

    fn state(&self) -> TaskState {
        self.task.state()
    }
}

#[glib::object_subclass]
impl ObjectSubclass for TcpClientSrc {
    const NAME: &'static str = "GstTsTcpClientSrc";
    type Type = super::TcpClientSrc;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        Self {
            src_pad: PadSrc::new(
                gst::Pad::from_template(&klass.pad_template("src").unwrap()),
                TcpClientSrcPadHandler,
            ),
            task: Task::default(),
            configured_caps: Default::default(),
            settings: Default::default(),
            state: Default::default(),
        }
    }
}

impl ObjectImpl for TcpClientSrc {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecString::builder("context")
                    .nick("Context")
                    .blurb("Context name to share threads with")
                    .default_value(Some(DEFAULT_CONTEXT))
                    .build(),
                glib::ParamSpecUInt::builder("context-wait")
                    .nick("Context Wait")
                    .blurb("Throttle poll loop to run at most once every this many ms")
                    .maximum(1000)
                    .default_value(DEFAULT_CONTEXT_WAIT.as_millis() as u32)
                    .build(),
                glib::ParamSpecString::builder("host")
                    .nick("Host")
                    .blurb("The host IP address to receive packets from")
                    .default_value(DEFAULT_HOST)
                    .build(),
                glib::ParamSpecInt::builder("port")
                    .nick("Port")
                    .blurb("Port to receive packets from")
                    .minimum(0)
                    .maximum(u16::MAX as i32)
                    .default_value(DEFAULT_PORT)
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Caps>("caps")
                    .nick("Caps")
                    .blurb("Caps to use")
                    .build(),
                glib::ParamSpecUInt::builder("blocksize")
                    .nick("Blocksize")
                    .blurb("Size in bytes to read per buffer (-1 = default)")
                    .default_value(DEFAULT_BLOCKSIZE)
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().unwrap();
        match pspec.name() {
            "host" => {
                settings.host = value.get().expect("type checked upstream");
            }
            "port" => {
                settings.port = value.get().expect("type checked upstream");
            }
            "caps" => {
                settings.caps = value.get().expect("type checked upstream");
            }
            "blocksize" => {
                settings.blocksize = value.get().expect("type checked upstream");
            }
            "context" => {
                settings.context = value
                    .get::<Option<String>>()
                    .expect("type checked upstream")
                    .unwrap_or_else(|| DEFAULT_CONTEXT.into());
            }
            "context-wait" => {
                settings.context_wait = Duration::from_millis(
                    value.get::<u32>().expect("type checked upstream").into(),
                );
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().unwrap();
        match pspec.name() {
            "host" => settings.host.to_value(),
            "port" => settings.port.to_value(),
            "caps" => settings.caps.to_value(),
            "blocksize" => settings.blocksize.to_value(),
            "context" => settings.context.to_value(),
            "context-wait" => (settings.context_wait.as_millis() as u32).to_value(),
            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(self.src_pad.gst_pad()).unwrap();
        obj.set_element_flags(gst::ElementFlags::SOURCE);
    }
}

impl GstObjectImpl for TcpClientSrc {}

impl ElementImpl for TcpClientSrc {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Thread-sharing TCP client source",
                "Source/Network",
                "Receives data over the network via TCP",
                "Sebastian Dröge <sebastian@centricular.com>, LEE Dongjun <redongjun@gmail.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::new_any();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
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
        gst::trace!(CAT, imp = self, "Changing state {:?}", transition);

        match transition {
            gst::StateChange::NullToReady => {
                self.prepare().map_err(|err| {
                    self.post_error_message(err);
                    gst::StateChangeError
                })?;
            }
            gst::StateChange::PausedToReady => {
                self.stop().map_err(|_| gst::StateChangeError)?;
            }
            gst::StateChange::ReadyToNull => {
                self.unprepare();
            }
            _ => (),
        }

        let mut success = self.parent_change_state(transition)?;

        match transition {
            gst::StateChange::ReadyToPaused => {
                success = gst::StateChangeSuccess::NoPreroll;
            }
            gst::StateChange::PausedToPlaying => {
                self.start().map_err(|_| gst::StateChangeError)?;
            }
            gst::StateChange::PlayingToPaused => {
                success = gst::StateChangeSuccess::NoPreroll;
            }
            _ => (),
        }

        Ok(success)
    }

    fn send_event(&self, event: gst::Event) -> bool {
        use gst::EventView;

        gst::debug!(CAT, imp = self, "Handling element level event {event:?}");

        match event.view() {
            EventView::Eos(_) => {
                if self.state() != TaskState::Started {
                    if let Err(err) = self.start() {
                        gst::error!(CAT, imp = self, "Failed to start task thread {err:?}");
                    }
                }

                if self.state() == TaskState::Started {
                    let mut state = self.state.lock().unwrap();

                    if let Some(event_tx) = state.event_sender.as_mut() {
                        return event_tx.try_send(event.clone()).is_ok();
                    }
                }

                false
            }
            _ => self.parent_send_event(event),
        }
    }
}
