// Copyright (C) 2018 Sebastian Dröge <sebastian@centricular.com>
// Copyright (C) 2019-2022 François Laignel <fengalin@free.fr>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use futures::channel::mpsc;
use futures::prelude::*;

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use std::sync::LazyLock;

use std::sync::Mutex;
use std::time::Duration;

use crate::runtime::prelude::*;
use crate::runtime::{Context, PadSrc, Task, TaskState};

const DEFAULT_CONTEXT: &str = "";
const DEFAULT_CONTEXT_WAIT: Duration = Duration::ZERO;
const DEFAULT_CAPS: Option<gst::Caps> = None;
const DEFAULT_MAX_BUFFERS: u32 = 10;
const DEFAULT_DO_TIMESTAMP: bool = false;

#[derive(Debug, Clone)]
struct Settings {
    context: String,
    context_wait: Duration,
    caps: Option<gst::Caps>,
    max_buffers: u32,
    do_timestamp: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            context: DEFAULT_CONTEXT.into(),
            context_wait: DEFAULT_CONTEXT_WAIT,
            caps: DEFAULT_CAPS,
            max_buffers: DEFAULT_MAX_BUFFERS,
            do_timestamp: DEFAULT_DO_TIMESTAMP,
        }
    }
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "ts-appsrc",
        gst::DebugColorFlags::empty(),
        Some("Thread-sharing app source"),
    )
});

#[derive(Debug)]
enum StreamItem {
    Buffer(gst::Buffer),
    Event(gst::Event),
}

#[derive(Clone, Debug)]
struct AppSrcPadHandler;

impl PadSrcHandler for AppSrcPadHandler {
    type ElementImpl = AppSrc;

    fn src_event(self, pad: &gst::Pad, imp: &AppSrc, event: gst::Event) -> bool {
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

    fn src_query(self, pad: &gst::Pad, imp: &AppSrc, query: &mut gst::QueryRef) -> bool {
        gst::log!(CAT, obj = pad, "Handling {:?}", query);

        use gst::QueryViewMut;
        let ret = match query.view_mut() {
            QueryViewMut::Latency(q) => {
                q.set(true, gst::ClockTime::ZERO, gst::ClockTime::NONE);
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

#[derive(Debug)]
struct AppSrcTask {
    element: super::AppSrc,
    receiver: mpsc::Receiver<StreamItem>,
    need_initial_events: bool,
    need_segment: bool,
}

impl AppSrcTask {
    fn new(element: super::AppSrc, receiver: mpsc::Receiver<StreamItem>) -> Self {
        AppSrcTask {
            element,
            receiver,
            need_initial_events: true,
            need_segment: true,
        }
    }
}

impl AppSrcTask {
    fn flush(&mut self) {
        // Purge the channel
        while let Ok(Some(_item)) = self.receiver.try_next() {}
    }

    async fn push_item(&mut self, item: StreamItem) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(CAT, obj = self.element, "Handling {:?}", item);
        let appsrc = self.element.imp();

        if self.need_initial_events {
            gst::debug!(CAT, obj = self.element, "Pushing initial events");

            let stream_id = format!("{:08x}{:08x}", rand::random::<u32>(), rand::random::<u32>());
            let stream_start_evt = gst::event::StreamStart::builder(&stream_id)
                .group_id(gst::GroupId::next())
                .build();
            appsrc.src_pad.push_event(stream_start_evt).await;

            let caps = appsrc.settings.lock().unwrap().caps.clone();
            if let Some(caps) = caps {
                appsrc
                    .src_pad
                    .push_event(gst::event::Caps::new(&caps))
                    .await;
                *appsrc.configured_caps.lock().unwrap() = Some(caps.clone());
            }

            self.need_initial_events = false;
        }

        if self.need_segment {
            let segment_evt =
                gst::event::Segment::new(&gst::FormattedSegment::<gst::format::Time>::new());
            appsrc.src_pad.push_event(segment_evt).await;

            self.need_segment = false;
        }

        match item {
            StreamItem::Buffer(buffer) => {
                gst::log!(CAT, obj = self.element, "Forwarding {:?}", buffer);
                appsrc.src_pad.push(buffer).await
            }
            StreamItem::Event(event) => {
                match event.view() {
                    gst::EventView::Eos(_) => {
                        // Let the caller push the event
                        Err(gst::FlowError::Eos)
                    }
                    _ => {
                        gst::log!(CAT, obj = self.element, "Forwarding {:?}", event);
                        appsrc.src_pad.push_event(event).await;
                        Ok(gst::FlowSuccess::Ok)
                    }
                }
            }
        }
    }
}

impl TaskImpl for AppSrcTask {
    type Item = StreamItem;

    fn obj(&self) -> &impl IsA<glib::Object> {
        &self.element
    }

    async fn try_next(&mut self) -> Result<StreamItem, gst::FlowError> {
        self.receiver
            .next()
            .await
            .ok_or_else(|| panic!("Internal channel sender dropped while Task is Started"))
    }

    async fn handle_item(&mut self, item: StreamItem) -> Result<(), gst::FlowError> {
        let res = self.push_item(item).await;
        match res {
            Ok(_) => {
                gst::log!(CAT, obj = self.element, "Successfully pushed item");
            }
            Err(gst::FlowError::Eos) => {
                gst::debug!(CAT, obj = self.element, "EOS");
                let appsrc = self.element.imp();
                appsrc.src_pad.push_event(gst::event::Eos::new()).await;
            }
            Err(gst::FlowError::Flushing) => {
                gst::debug!(CAT, obj = self.element, "Flushing");
            }
            Err(err) => {
                gst::error!(CAT, obj = self.element, "Got error {}", err);
                gst::element_error!(
                    &self.element,
                    gst::StreamError::Failed,
                    ("Internal data stream error"),
                    ["streaming stopped, reason {}", err]
                );
            }
        }

        res.map(drop)
    }

    async fn stop(&mut self) -> Result<(), gst::ErrorMessage> {
        gst::log!(CAT, obj = self.element, "Stopping task");

        self.flush();
        self.need_initial_events = true;
        self.need_segment = true;

        gst::log!(CAT, obj = self.element, "Task stopped");
        Ok(())
    }

    async fn flush_start(&mut self) -> Result<(), gst::ErrorMessage> {
        gst::log!(CAT, obj = self.element, "Starting task flush");

        self.flush();
        self.need_segment = true;

        gst::log!(CAT, obj = self.element, "Task flush started");
        Ok(())
    }
}

#[derive(Debug)]
pub struct AppSrc {
    src_pad: PadSrc,
    task: Task,
    sender: Mutex<Option<mpsc::Sender<StreamItem>>>,
    configured_caps: Mutex<Option<gst::Caps>>,
    settings: Mutex<Settings>,
}

impl AppSrc {
    fn push_buffer(&self, mut buffer: gst::Buffer) -> bool {
        let state = self.task.lock_state();
        if *state != TaskState::Started && *state != TaskState::Paused {
            gst::debug!(CAT, imp = self, "Rejecting buffer due to element state");
            return false;
        }

        let do_timestamp = self.settings.lock().unwrap().do_timestamp;
        if do_timestamp {
            let elem = self.obj();
            if let Some(clock) = elem.clock() {
                let base_time = elem.base_time();
                let now = clock.time();

                let buffer = buffer.make_mut();
                buffer.set_dts(now.opt_checked_sub(base_time).ok().flatten());
                buffer.set_pts(None);
            } else {
                gst::error!(CAT, imp = self, "Don't have a clock yet");
                return false;
            }
        }

        let mut sender = self.sender.lock().unwrap();
        match sender
            .as_mut()
            .unwrap()
            .try_send(StreamItem::Buffer(buffer))
        {
            Ok(_) => true,
            Err(err) => {
                gst::error!(CAT, imp = self, "Failed to queue buffer: {}", err);
                false
            }
        }
    }

    fn end_of_stream(&self) -> bool {
        let mut sender = self.sender.lock().unwrap();
        let sender = match sender.as_mut() {
            Some(sender) => sender,
            None => return false,
        };

        match sender.try_send(StreamItem::Event(gst::event::Eos::new())) {
            Ok(_) => true,
            Err(err) => {
                gst::error!(CAT, imp = self, "Failed to queue EOS: {}", err);
                false
            }
        }
    }

    fn prepare(&self) -> Result<(), gst::ErrorMessage> {
        gst::debug!(CAT, imp = self, "Preparing");

        let settings = self.settings.lock().unwrap();
        let context =
            Context::acquire(&settings.context, settings.context_wait).map_err(|err| {
                gst::error_msg!(
                    gst::ResourceError::OpenRead,
                    ["Failed to acquire Context: {}", err]
                )
            })?;
        let max_buffers = settings.max_buffers.try_into().map_err(|err| {
            gst::error_msg!(
                gst::ResourceError::Settings,
                ["Invalid max-buffers: {}, {}", settings.max_buffers, err]
            )
        })?;
        drop(settings);

        *self.configured_caps.lock().unwrap() = None;

        let (sender, receiver) = mpsc::channel(max_buffers);
        *self.sender.lock().unwrap() = Some(sender);
        self.task
            .prepare(AppSrcTask::new(self.obj().clone(), receiver), context)
            .block_on_or_add_subtask_then(self.obj(), |elem, res| {
                if res.is_ok() {
                    gst::debug!(CAT, obj = elem, "Prepared");
                }
            })
    }

    fn unprepare(&self) {
        gst::debug!(CAT, imp = self, "Unpreparing");

        *self.sender.lock().unwrap() = None;
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
}

#[glib::object_subclass]
impl ObjectSubclass for AppSrc {
    const NAME: &'static str = "GstTsAppSrc";
    type Type = super::AppSrc;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        Self {
            src_pad: PadSrc::new(
                gst::Pad::from_template(&klass.pad_template("src").unwrap()),
                AppSrcPadHandler,
            ),
            task: Task::default(),
            sender: Default::default(),
            configured_caps: Default::default(),
            settings: Default::default(),
        }
    }
}

impl ObjectImpl for AppSrc {
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
                glib::ParamSpecUInt::builder("max-buffers")
                    .nick("Max Buffers")
                    .blurb("Maximum number of buffers to queue up")
                    .minimum(1)
                    .default_value(DEFAULT_MAX_BUFFERS)
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Caps>("caps")
                    .nick("Caps")
                    .blurb("Caps to use")
                    .build(),
                glib::ParamSpecBoolean::builder("do-timestamp")
                    .nick("Do Timestamp")
                    .blurb("Timestamp buffers with the current running time on arrival")
                    .default_value(DEFAULT_DO_TIMESTAMP)
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn signals() -> &'static [glib::subclass::Signal] {
        static SIGNALS: LazyLock<Vec<glib::subclass::Signal>> = LazyLock::new(|| {
            vec![
                glib::subclass::Signal::builder("push-buffer")
                    .param_types([gst::Buffer::static_type()])
                    .return_type::<bool>()
                    .action()
                    .class_handler(|args| {
                        let elem = args[0].get::<super::AppSrc>().expect("signal arg");
                        let buffer = args[1].get::<gst::Buffer>().expect("signal arg");

                        Some(elem.imp().push_buffer(buffer).to_value())
                    })
                    .build(),
                /**
                 * ts-appsrc::end-of-stream:
                 * @self: A ts-appsrc
                 *
                 * Returns: %TRUE if the EOS could be queued, %FALSE otherwise
                 */
                glib::subclass::Signal::builder("end-of-stream")
                    .return_type::<bool>()
                    .action()
                    .class_handler(|args| {
                        let elem = args[0].get::<super::AppSrc>().expect("signal arg");

                        Some(elem.imp().end_of_stream().to_value())
                    })
                    .build(),
            ]
        });

        SIGNALS.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().unwrap();
        match pspec.name() {
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
            "caps" => {
                settings.caps = value.get().expect("type checked upstream");
            }
            "max-buffers" => {
                settings.max_buffers = value.get().expect("type checked upstream");
            }
            "do-timestamp" => {
                settings.do_timestamp = value.get().expect("type checked upstream");
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().unwrap();
        match pspec.name() {
            "context" => settings.context.to_value(),
            "context-wait" => (settings.context_wait.as_millis() as u32).to_value(),
            "caps" => settings.caps.to_value(),
            "max-buffers" => settings.max_buffers.to_value(),
            "do-timestamp" => settings.do_timestamp.to_value(),
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

impl GstObjectImpl for AppSrc {}

impl ElementImpl for AppSrc {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Thread-sharing app source",
                "Source/Generic",
                "Thread-sharing app source",
                "Sebastian Dröge <sebastian@centricular.com>",
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
}
