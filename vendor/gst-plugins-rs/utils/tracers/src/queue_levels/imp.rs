// Copyright (C) 2022 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * tracer-queue-levels:
 *
 * This tracer provides an easy way to collect queue levels over time of all queues inside a
 * pipeline.
 *
 * Example:
 *
 * ```console
 * $ GST_TRACERS='queue-levels(file="/tmp/queue_levels.log")' gst-launch-1.0 audiotestsrc ! queue ! fakesink
 * ```
 *
 * The generated file is a CSV file of the format
 *
 * ```csv
 * timestamp,queue name,queue pointer,cur-level-bytes,cur-level-time,cur-level-buffers,max-size-bytes,max-size-time,max-size-buffers
 * ```
 *
 * ## Parameters
 *
 * ### `file`
 *
 * Specifies the path to the file that will collect the CSV file with the queue levels.
 *
 * By default the file is written to `/tmp/queue_levels.log`.
 *
 * ### `include-filter`
 *
 * Specifies a regular expression for the queue object names that should be included.
 *
 * By default this is not set.
 *
 * ### `exclude-filter`
 *
 * Specifies a regular expression for the queue object names that should **not** be included.
 *
 * By default this is not set.
 */
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use regex::Regex;
use std::sync::LazyLock;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "queue-levels",
        gst::DebugColorFlags::empty(),
        Some("Tracer to collect queue levels"),
    )
});

static QUEUE_TYPE: LazyLock<glib::Type> = LazyLock::new(|| {
    if let Some(queue) = gst::ElementFactory::find("queue").and_then(|f| f.load().ok()) {
        queue.element_type()
    } else {
        gst::warning!(CAT, "Can't instantiate queue element");
        glib::Type::INVALID
    }
});

static QUEUE2_TYPE: LazyLock<glib::Type> = LazyLock::new(|| {
    if let Some(queue) = gst::ElementFactory::find("queue2").and_then(|f| f.load().ok()) {
        queue.element_type()
    } else {
        gst::warning!(CAT, "Can't instantiate queue2 element");
        glib::Type::INVALID
    }
});

static MULTIQUEUE_TYPE: LazyLock<glib::Type> = LazyLock::new(|| {
    if let Some(queue) = gst::ElementFactory::find("multiqueue").and_then(|f| f.load().ok()) {
        queue.element_type()
    } else {
        gst::warning!(CAT, "Can't instantiate multiqueue element");
        glib::Type::INVALID
    }
});

static APPSRC_TYPE: LazyLock<glib::Type> = LazyLock::new(|| {
    if let Some(queue) = gst::ElementFactory::find("appsrc").and_then(|f| f.load().ok()) {
        queue.element_type()
    } else {
        gst::warning!(CAT, "Can't instantiate appsrc element");
        glib::Type::INVALID
    }
});

static TS_INTERSRC_TYPE: LazyLock<glib::Type> = LazyLock::new(|| {
    if let Some(queue) = gst::ElementFactory::find("ts-intersrc").and_then(|f| f.load().ok()) {
        queue.element_type()
    } else {
        gst::warning!(CAT, "Can't instantiate ts-intersrc element");
        glib::Type::INVALID
    }
});

static TS_PROXYSRC_TYPE: LazyLock<glib::Type> = LazyLock::new(|| {
    if let Some(queue) = gst::ElementFactory::find("ts-proxysrc").and_then(|f| f.load().ok()) {
        queue.element_type()
    } else {
        gst::warning!(CAT, "Can't instantiate ts-proxysrc element");
        glib::Type::INVALID
    }
});

static TS_QUEUE_TYPE: LazyLock<glib::Type> = LazyLock::new(|| {
    if let Some(queue) = gst::ElementFactory::find("ts-queue").and_then(|f| f.load().ok()) {
        queue.element_type()
    } else {
        gst::warning!(CAT, "Can't instantiate ts-queue element");
        glib::Type::INVALID
    }
});

fn is_queue_type(type_: glib::Type) -> bool {
    [
        *QUEUE_TYPE,
        *QUEUE2_TYPE,
        *MULTIQUEUE_TYPE,
        *APPSRC_TYPE,
        *TS_INTERSRC_TYPE,
        *TS_PROXYSRC_TYPE,
        *TS_QUEUE_TYPE,
    ]
    .contains(&type_)
}

#[derive(Debug)]
struct Settings {
    file: PathBuf,
    include_filter: Option<Regex>,
    exclude_filter: Option<Regex>,
}

impl Default for Settings {
    fn default() -> Self {
        let mut file = glib::tmp_dir();
        file.push("queue_levels.log");

        Self {
            file,
            include_filter: None,
            exclude_filter: None,
        }
    }
}

impl Settings {
    fn update_from_params(&mut self, imp: &QueueLevels, params: String) {
        let s = match gst::Structure::from_str(&format!("queue-levels,{params}")) {
            Ok(s) => s,
            Err(err) => {
                gst::warning!(CAT, imp = imp, "failed to parse tracer parameters: {}", err);
                return;
            }
        };

        if let Ok(file) = s.get::<&str>("file") {
            gst::log!(CAT, imp = imp, "file= {}", file);
            self.file = PathBuf::from(file);
        }

        if let Ok(filter) = s.get::<&str>("include-filter") {
            gst::log!(CAT, imp = imp, "include filter= {}", filter);
            let filter = match Regex::new(filter) {
                Ok(filter) => Some(filter),
                Err(err) => {
                    gst::error!(
                        CAT,
                        imp = imp,
                        "Failed to compile include-filter regex: {}",
                        err
                    );
                    None
                }
            };
            self.include_filter = filter;
        }

        if let Ok(filter) = s.get::<&str>("exclude-filter") {
            gst::log!(CAT, imp = imp, "exclude filter= {}", filter);
            let filter = match Regex::new(filter) {
                Ok(filter) => Some(filter),
                Err(err) => {
                    gst::error!(
                        CAT,
                        imp = imp,
                        "Failed to compile exclude-filter regex: {}",
                        err
                    );
                    None
                }
            };
            self.exclude_filter = filter;
        }
    }
}

#[derive(Default)]
struct State {
    queues: HashMap<usize, Arc<glib::GString>>,
    log: Vec<LogLine>,
    settings: Settings,
    logs_written: HashSet<PathBuf>,
}

struct LogLine {
    timestamp: u64,
    name: Arc<glib::GString>,
    idx: Option<usize>,
    ptr: usize,
    cur_level_bytes: u32,
    cur_level_time: u64,
    cur_level_buffers: u32,
    max_size_bytes: u64,
    max_size_time: u64,
    max_size_buffers: u64,
}

#[derive(Default)]
pub struct QueueLevels {
    state: Mutex<State>,
}

#[glib::object_subclass]
impl ObjectSubclass for QueueLevels {
    const NAME: &'static str = "GstQueueLevels";
    type Type = super::QueueLevels;
    type ParentType = gst::Tracer;
}

impl ObjectImpl for QueueLevels {
    fn constructed(&self) {
        self.parent_constructed();

        if let Some(params) = self.obj().property::<Option<String>>("params") {
            let mut state = self.state.lock().unwrap();
            state.settings.update_from_params(self, params);
        }

        LazyLock::force(&QUEUE_TYPE);
        LazyLock::force(&QUEUE2_TYPE);
        LazyLock::force(&MULTIQUEUE_TYPE);

        self.register_hook(TracerHook::ElementNew);
        self.register_hook(TracerHook::ObjectDestroyed);
        self.register_hook(TracerHook::PadPushPost);
        self.register_hook(TracerHook::PadPushListPost);
        #[cfg(feature = "v1_22")]
        {
            self.register_hook(TracerHook::PadChainPost);
            self.register_hook(TracerHook::PadChainListPost);
        }
        #[cfg(not(feature = "v1_22"))]
        {
            self.register_hook(TracerHook::PadPushPost);
            self.register_hook(TracerHook::PadPushListPost);
        }
        self.register_hook(TracerHook::PadPushPre);
        self.register_hook(TracerHook::PadPushListPre);
        self.register_hook(TracerHook::ElementChangeStatePost);
        self.register_hook(TracerHook::PadPushEventPre);
    }

    fn signals() -> &'static [glib::subclass::Signal] {
        static SIGNALS: LazyLock<Vec<glib::subclass::Signal>> = LazyLock::new(|| {
            vec![glib::subclass::Signal::builder("write-log")
                .action()
                .param_types([Option::<String>::static_type()])
                .class_handler(|args| {
                    let obj = args[0].get::<super::QueueLevels>().unwrap();

                    obj.imp().write_log(args[1].get::<Option<&str>>().unwrap());

                    None
                })
                .build()]
        });

        SIGNALS.as_ref()
    }

    fn dispose(&self) {
        self.write_log(None);
    }
}

impl GstObjectImpl for QueueLevels {}

impl TracerImpl for QueueLevels {
    fn element_new(&self, _ts: u64, element: &gst::Element) {
        if !is_queue_type(element.type_()) {
            return;
        }

        let ptr = element.as_ptr() as usize;
        gst::debug!(
            CAT,
            imp = self,
            "new queue: {} 0x{:08x}",
            element.name(),
            ptr
        );

        let mut state = self.state.lock().unwrap();

        let name = element.name();
        if let Some(ref filter) = state.settings.include_filter {
            if !filter.is_match(&name) {
                return;
            }
        }
        if let Some(ref filter) = state.settings.exclude_filter {
            if filter.is_match(&name) {
                return;
            }
        }

        state.queues.entry(ptr).or_insert_with(|| Arc::new(name));
    }

    fn object_destroyed(&self, _ts: u64, object: std::ptr::NonNull<gst::ffi::GstObject>) {
        let ptr = object.as_ptr() as usize;
        let mut state = self.state.lock().unwrap();
        state.queues.remove(&ptr);
    }

    fn pad_push_pre(&self, ts: u64, pad: &gst::Pad, _buffer: &gst::Buffer) {
        if let Some(parent) = pad.parent().and_then(|p| p.downcast::<gst::Element>().ok()) {
            if is_queue_type(parent.type_()) {
                self.log(&parent, Some(pad), ts);
            }
        }
    }

    fn pad_push_list_pre(&self, ts: u64, pad: &gst::Pad, _list: &gst::BufferList) {
        if let Some(parent) = pad.parent().and_then(|p| p.downcast::<gst::Element>().ok()) {
            if is_queue_type(parent.type_()) {
                self.log(&parent, Some(pad), ts);
            }
        }
    }

    #[cfg(not(feature = "v1_22"))]
    fn pad_push_post(
        &self,
        ts: u64,
        pad: &gst::Pad,
        _result: Result<gst::FlowSuccess, gst::FlowError>,
    ) {
        if let Some(peer) = pad.peer() {
            if let Some(parent) = peer
                .parent()
                .and_then(|p| p.downcast::<gst::Element>().ok())
            {
                if is_queue_type(parent.type_()) {
                    self.log(&parent, Some(&peer), ts);
                }
            }
        }
    }

    #[cfg(not(feature = "v1_22"))]
    fn pad_push_list_post(
        &self,
        ts: u64,
        pad: &gst::Pad,
        _result: Result<gst::FlowSuccess, gst::FlowError>,
    ) {
        if let Some(peer) = pad.peer() {
            if let Some(parent) = peer
                .parent()
                .and_then(|p| p.downcast::<gst::Element>().ok())
            {
                if is_queue_type(parent.type_()) {
                    self.log(&parent, Some(&peer), ts);
                }
            }
        }
    }

    #[cfg(feature = "v1_22")]
    fn pad_chain_post(
        &self,
        ts: u64,
        pad: &gst::Pad,
        _result: Result<gst::FlowSuccess, gst::FlowError>,
    ) {
        if let Some(parent) = pad.parent().and_then(|p| p.downcast::<gst::Element>().ok()) {
            if is_queue_type(parent.type_()) {
                self.log(&parent, Some(pad), ts);
            }
        }
    }

    #[cfg(feature = "v1_22")]
    fn pad_chain_list_post(
        &self,
        ts: u64,
        pad: &gst::Pad,
        _result: Result<gst::FlowSuccess, gst::FlowError>,
    ) {
        if let Some(parent) = pad.parent().and_then(|p| p.downcast::<gst::Element>().ok()) {
            if is_queue_type(parent.type_()) {
                self.log(&parent, Some(pad), ts);
            }
        }
    }

    fn element_change_state_post(
        &self,
        ts: u64,
        element: &gst::Element,
        change: gst::StateChange,
        _result: Result<gst::StateChangeSuccess, gst::StateChangeError>,
    ) {
        if change.next() != gst::State::Null {
            return;
        }

        if !is_queue_type(element.type_()) {
            return;
        }

        self.log(element, None, ts);
    }

    fn pad_push_event_pre(&self, ts: u64, pad: &gst::Pad, ev: &gst::Event) {
        if ev.type_() != gst::EventType::FlushStop {
            return;
        }

        if let Some(parent) = pad.parent().and_then(|p| p.downcast::<gst::Element>().ok()) {
            if is_queue_type(parent.type_()) {
                self.log(&parent, Some(pad), ts);
            }
        }
    }
}

impl QueueLevels {
    fn log(&self, element: &gst::Element, pad: Option<&gst::Pad>, timestamp: u64) {
        let ptr = element.as_ptr() as usize;

        let mut state = self.state.lock().unwrap();
        let name = match state.queues.get(&ptr) {
            Some(name) => name.clone(),
            None => return,
        };

        let (max_size_bytes, max_size_time, max_size_buffers) = if element.type_() == *APPSRC_TYPE {
            (
                element.property::<u64>("max-bytes"),
                element.property::<u64>("max-time"),
                element.property::<u64>("max-buffers"),
            )
        } else {
            (
                element.property::<u32>("max-size-bytes") as u64,
                element.property::<u64>("max-size-time"),
                element.property::<u32>("max-size-buffers") as u64,
            )
        };

        if element.type_() == *MULTIQUEUE_TYPE {
            let get_pad_idx = |pad: &gst::Pad| {
                // SAFETY: Names can't change while there's a strong reference to the object
                unsafe {
                    let name_ptr = (*pad.as_ptr()).object.name;
                    let name = std::ffi::CStr::from_ptr(name_ptr as *const _)
                        .to_str()
                        .unwrap();
                    if let Some(idx) = name.strip_prefix("sink_") {
                        idx.parse::<usize>().unwrap()
                    } else if let Some(idx) = name.strip_prefix("src_") {
                        idx.parse::<usize>().unwrap()
                    } else {
                        unreachable!();
                    }
                }
            };

            if let Some(pad) = pad {
                let cur_level_bytes = pad.property::<u32>("current-level-bytes");
                let cur_level_time = pad.property::<u64>("current-level-time");
                let cur_level_buffers = pad.property::<u32>("current-level-buffers");
                state.log.push(LogLine {
                    timestamp,
                    name,
                    idx: Some(get_pad_idx(pad)),
                    ptr,
                    cur_level_bytes,
                    cur_level_time,
                    cur_level_buffers,
                    max_size_bytes,
                    max_size_time,
                    max_size_buffers,
                });
            } else {
                for pad in element.sink_pads() {
                    let cur_level_bytes = pad.property::<u32>("current-level-bytes");
                    let cur_level_time = pad.property::<u64>("current-level-time");
                    let cur_level_buffers = pad.property::<u32>("current-level-buffers");
                    state.log.push(LogLine {
                        timestamp,
                        name: name.clone(),
                        idx: Some(get_pad_idx(&pad)),
                        ptr,
                        cur_level_bytes,
                        cur_level_time,
                        cur_level_buffers,
                        max_size_bytes,
                        max_size_time,
                        max_size_buffers,
                    });
                }
            }
        } else {
            let (cur_level_bytes, cur_level_time, cur_level_buffers) =
                if element.type_() == *APPSRC_TYPE {
                    (
                        element.property::<u64>("current-level-bytes") as u32,
                        element.property::<u64>("current-level-time"),
                        element.property::<u64>("current-level-buffers") as u32,
                    )
                } else {
                    (
                        element.property::<u32>("current-level-bytes"),
                        element.property::<u64>("current-level-time"),
                        element.property::<u32>("current-level-buffers"),
                    )
                };

            state.log.push(LogLine {
                timestamp,
                name,
                idx: None,
                ptr,
                cur_level_bytes,
                cur_level_time,
                cur_level_buffers,
                max_size_bytes,
                max_size_time,
                max_size_buffers,
            });
        }
    }

    fn write_log(&self, file_path: Option<&str>) {
        use std::io::prelude::*;

        let mut state = self.state.lock().unwrap();
        let path = file_path.map_or_else(|| state.settings.file.clone(), PathBuf::from);
        let first_write = !state.logs_written.contains(&path);

        let mut file = match std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(first_write)
            .append(!first_write)
            .open(&path)
        {
            Ok(file) => file,
            Err(err) => {
                gst::error!(CAT, imp = self, "Failed to open file: {err}");
                return;
            }
        };

        let log = std::mem::take(&mut state.log);
        state.logs_written.insert(path);
        drop(state);

        gst::debug!(CAT, imp = self, "Writing file {:?}", file);

        for LogLine {
            timestamp,
            name,
            idx,
            ptr,
            cur_level_bytes,
            cur_level_time,
            cur_level_buffers,
            max_size_bytes,
            max_size_time,
            max_size_buffers,
        } in &log
        {
            let res = if let Some(idx) = idx {
                writeln!(&mut file, "{timestamp},{name}:{idx},0x{ptr:08x},{cur_level_bytes},{cur_level_time},{cur_level_buffers},{max_size_bytes},{max_size_time},{max_size_buffers}")
            } else {
                writeln!(&mut file, "{timestamp},{name},0x{ptr:08x},{cur_level_bytes},{cur_level_time},{cur_level_buffers},{max_size_bytes},{max_size_time},{max_size_buffers}")
            };
            if let Err(err) = res {
                gst::error!(CAT, imp = self, "Failed to write to file: {err}");
                return;
            }
        }
    }
}
