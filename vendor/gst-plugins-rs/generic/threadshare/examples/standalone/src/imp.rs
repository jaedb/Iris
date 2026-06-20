// Copyright (C) 2022 François Laignel <fengalin@free.fr>
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

use std::sync::Mutex;
use std::time::Duration;

use gstthreadshare::runtime::prelude::*;
use gstthreadshare::runtime::{task, timer, Context, PadSrc, Task};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        super::ELEMENT_NAME,
        gst::DebugColorFlags::empty(),
        Some("Thread-sharing standalone test src"),
    )
});

const DEFAULT_CONTEXT: &str = "";
const DEFAULT_CONTEXT_WAIT: Duration = Duration::from_millis(20);
const DEFAULT_PUSH_PERIOD: gst::ClockTime = gst::ClockTime::from_mseconds(20);
const DEFAULT_NUM_BUFFERS: i32 = 50 * 100;

#[derive(Debug, Clone)]
struct Settings {
    context: String,
    context_wait: Duration,
    push_period: gst::ClockTime,
    is_main_elem: bool,
    num_buffers: Option<u32>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            context: DEFAULT_CONTEXT.into(),
            context_wait: DEFAULT_CONTEXT_WAIT,
            push_period: DEFAULT_PUSH_PERIOD,
            is_main_elem: false,
            num_buffers: Some(DEFAULT_NUM_BUFFERS as u32),
        }
    }
}

#[derive(Clone, Debug)]
struct TestSrcPadHandler;
impl PadSrcHandler for TestSrcPadHandler {
    type ElementImpl = TestSrc;
}

#[derive(Debug)]
struct SrcTask {
    elem: super::TestSrc,
    buffer_pool: gst::BufferPool,
    timer: Option<timer::Interval>,
    is_main_elem: bool,
    push_period: gst::ClockTime,
    need_initial_events: bool,
    num_buffers: Option<u32>,
    buffer_count: u32,
}

impl SrcTask {
    fn new(elem: super::TestSrc) -> Self {
        let buffer_pool = gst::BufferPool::new();
        let mut pool_config = buffer_pool.config();
        pool_config
            .as_mut()
            .set_params(Some(&gst::Caps::builder("foo/bar").build()), 10, 10, 10);
        buffer_pool.set_config(pool_config).unwrap();

        SrcTask {
            elem,
            buffer_pool,
            timer: None,
            is_main_elem: false,
            push_period: gst::ClockTime::ZERO,
            need_initial_events: true,
            num_buffers: Some(DEFAULT_NUM_BUFFERS as u32),
            buffer_count: 0,
        }
    }
}

impl TaskImpl for SrcTask {
    type Item = ();

    fn obj(&self) -> &impl IsA<glib::Object> {
        &self.elem
    }

    async fn prepare(&mut self) -> Result<(), gst::ErrorMessage> {
        let imp = self.elem.imp();
        let settings = imp.settings.lock().unwrap();
        self.is_main_elem = settings.is_main_elem;

        log_or_trace!(CAT, self.is_main_elem, imp = imp, "Preparing Task");

        self.push_period = settings.push_period;
        self.num_buffers = settings.num_buffers;

        Ok(())
    }

    async fn start(&mut self) -> Result<(), gst::ErrorMessage> {
        log_or_trace!(CAT, self.is_main_elem, obj = self.elem, "Starting Task");

        if self.need_initial_events {
            let imp = self.elem.imp();

            debug_or_trace!(
                CAT,
                self.is_main_elem,
                obj = self.elem,
                "Pushing initial events"
            );

            let stream_id = format!("{:08x}{:08x}", rand::random::<u32>(), rand::random::<u32>());
            let stream_start_evt = gst::event::StreamStart::builder(&stream_id)
                .group_id(gst::GroupId::next())
                .build();
            imp.src_pad.push_event(stream_start_evt).await;

            imp.src_pad
                .push_event(gst::event::Caps::new(
                    &gst::Caps::builder("foo/bar").build(),
                ))
                .await;

            let segment_evt =
                gst::event::Segment::new(&gst::FormattedSegment::<gst::format::Time>::new());
            imp.src_pad.push_event(segment_evt).await;

            self.need_initial_events = false;
        }

        self.timer = Some(
            timer::interval_delayed_by(
                // Delay first buffer push so as to let others start.
                Duration::from_secs(2),
                self.push_period.into(),
            )
            .expect("push period must be greater than 0"),
        );
        self.buffer_count = 0;
        self.buffer_pool.set_active(true).unwrap();

        Ok(())
    }

    async fn stop(&mut self) -> Result<(), gst::ErrorMessage> {
        log_or_trace!(CAT, self.is_main_elem, obj = self.elem, "Stopping Task");
        self.buffer_pool.set_active(false).unwrap();
        self.timer = None;
        self.need_initial_events = true;

        Ok(())
    }

    async fn try_next(&mut self) -> Result<(), gst::FlowError> {
        log_or_trace!(CAT, self.is_main_elem, obj = self.elem, "Awaiting timer");
        self.timer.as_mut().unwrap().next().await;
        log_or_trace!(CAT, self.is_main_elem, obj = self.elem, "Timer ticked");

        Ok(())
    }

    async fn handle_item(&mut self, _: ()) -> Result<(), gst::FlowError> {
        let buffer = self
            .buffer_pool
            .acquire_buffer(None)
            .map(|mut buffer| {
                {
                    let buffer = buffer.get_mut().unwrap();
                    let rtime = self.elem.current_running_time().unwrap();
                    buffer.set_pts(rtime);
                }
                buffer
            })
            .inspect_err(|&err| {
                gst::error!(CAT, obj = self.elem, "Failed to acquire buffer {err}");
            })?;

        debug_or_trace!(CAT, self.is_main_elem, obj = self.elem, "Forwarding buffer");
        self.elem.imp().src_pad.push(buffer).await?;
        log_or_trace!(
            CAT,
            self.is_main_elem,
            obj = self.elem,
            "Successfully pushed buffer"
        );

        self.buffer_count += 1;

        if self.num_buffers.opt_eq(self.buffer_count).unwrap_or(false) {
            gst::debug!(CAT, obj = self.elem, "Pushing EOS");

            let imp = self.elem.imp();
            if !imp.src_pad.push_event(gst::event::Eos::new()).await {
                gst::error!(CAT, imp = imp, "Error pushing EOS");
            }

            return Err(gst::FlowError::Eos);
        }

        Ok(())
    }

    async fn handle_loop_error(&mut self, err: gst::FlowError) -> task::Trigger {
        match err {
            gst::FlowError::Eos => task::Trigger::Stop,
            gst::FlowError::Flushing => {
                debug_or_trace!(CAT, self.is_main_elem, obj = self.elem, "Flushing");

                task::Trigger::FlushStart
            }
            err => {
                gst::error!(CAT, obj = self.elem, "Got error {err}");
                gst::element_error!(
                    &self.elem,
                    gst::StreamError::Failed,
                    ("Internal data stream error"),
                    ["streaming stopped, reason {}", err]
                );

                task::Trigger::Error
            }
        }
    }
}

#[derive(Debug)]
pub struct TestSrc {
    src_pad: PadSrc,
    task: Task,
    settings: Mutex<Settings>,
}

impl TestSrc {
    fn prepare(&self) -> Result<(), gst::ErrorMessage> {
        let is_main_elem = self.settings.lock().unwrap().is_main_elem;
        debug_or_trace!(CAT, is_main_elem, imp = self, "Preparing");

        let settings = self.settings.lock().unwrap();
        let ts_ctx = Context::acquire(&settings.context, settings.context_wait).map_err(|err| {
            gst::error_msg!(
                gst::ResourceError::OpenRead,
                ["Failed to acquire Context: {}", err]
            )
        })?;
        drop(settings);

        self.task
            .prepare(SrcTask::new(self.obj().clone()), ts_ctx)
            .block_on_or_add_subtask_then(self.obj(), move |elem, res| {
                if res.is_ok() {
                    debug_or_trace!(CAT, is_main_elem, obj = elem, "Prepared");
                }
            })
    }

    fn unprepare(&self) {
        let is_main_elem = self.settings.lock().unwrap().is_main_elem;
        debug_or_trace!(CAT, is_main_elem, imp = self, "Unpreparing");
        let _ = self
            .task
            .unprepare()
            .block_on_or_add_subtask_then(self.obj(), move |elem, _| {
                debug_or_trace!(CAT, is_main_elem, obj = elem, "Unprepared");
            });
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        let is_main_elem = self.settings.lock().unwrap().is_main_elem;
        debug_or_trace!(CAT, is_main_elem, imp = self, "Stopping");
        self.task
            .stop()
            .block_on_or_add_subtask_then(self.obj(), move |elem, res| {
                if res.is_ok() {
                    debug_or_trace!(CAT, is_main_elem, obj = elem, "Stopped");
                }
            })
    }

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        let is_main_elem = self.settings.lock().unwrap().is_main_elem;
        debug_or_trace!(CAT, is_main_elem, imp = self, "Starting");
        self.task
            .start()
            .block_on_or_add_subtask_then(self.obj(), move |elem, res| {
                if res.is_ok() {
                    debug_or_trace!(CAT, is_main_elem, obj = elem, "Started");
                }
            })
    }
}

#[glib::object_subclass]
impl ObjectSubclass for TestSrc {
    const NAME: &'static str = "StandaloneTestSrc";
    type Type = super::TestSrc;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        Self {
            src_pad: PadSrc::new(
                gst::Pad::from_template(&klass.pad_template("src").unwrap()),
                TestSrcPadHandler,
            ),
            task: Task::default(),
            settings: Default::default(),
        }
    }
}

impl ObjectImpl for TestSrc {
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
                glib::ParamSpecUInt::builder("push-period")
                    .nick("Buffer Push Period")
                    .blurb("Push a new buffer every this many ms")
                    .default_value(DEFAULT_PUSH_PERIOD.mseconds() as u32)
                    .build(),
                glib::ParamSpecBoolean::builder("main-elem")
                    .nick("Main Element")
                    .blurb("Declare this element as the main one")
                    .write_only()
                    .build(),
                glib::ParamSpecInt::builder("num-buffers")
                    .nick("Num Buffers")
                    .blurb("Number of buffers to output before sending EOS (-1 = unlimited)")
                    .minimum(-1i32)
                    .default_value(DEFAULT_NUM_BUFFERS)
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().unwrap();
        match pspec.name() {
            "context" => {
                settings.context = value
                    .get::<Option<String>>()
                    .unwrap()
                    .unwrap_or_else(|| DEFAULT_CONTEXT.into());
            }
            "context-wait" => {
                settings.context_wait = Duration::from_millis(value.get::<u32>().unwrap().into());
            }
            "push-period" => {
                let value: u64 = value.get::<u32>().unwrap().into();
                settings.push_period = value.mseconds();
            }
            "main-elem" => {
                settings.is_main_elem = value.get::<bool>().unwrap();
            }
            "num-buffers" => {
                let value = value.get::<i32>().unwrap();
                settings.num_buffers = if value > 0 { Some(value as u32) } else { None };
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().unwrap();
        match pspec.name() {
            "context" => settings.context.to_value(),
            "context-wait" => (settings.context_wait.as_millis() as u32).to_value(),
            "push-period" => (settings.push_period.mseconds() as u32).to_value(),
            "main-elem" => settings.is_main_elem.to_value(),
            "num-buffers" => settings
                .num_buffers
                .and_then(|val| val.try_into().ok())
                .unwrap_or(-1i32)
                .to_value(),
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

impl GstObjectImpl for TestSrc {}

impl ElementImpl for TestSrc {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Thread-sharing standalone test source",
                "Source/Test",
                "Thread-sharing standalone test source",
                "François Laignel <fengalin@free.fr>",
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
        gst::trace!(CAT, imp = self, "Changing state {transition:?}");

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
