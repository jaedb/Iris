// Copyright (C) 2024 Thibault Saunier <tsaunier@igalia.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * tracer-memory-tracer:
 *
 * This tracer provides an easy way to track memory allocations over time in a pipeline.
 *
 * ## Example:
 *
 * ### Log GstMemory allocation releases into the `tmp.memory.csv` file
 *
 * ```
 * # Dropping allocation query so we can see both the GLMemory and the SystemMemory on the graph
 *
 * $ GST_TRACERS="memory-tracer(file=tmp.memory.cvs)" gst-launch-1.0 videotestsrc num-buffers=30 ! identity drop-allocation=true ! glimagesink
 * ```
 *
 * ### lot memory usage per type
 *
 * ```
 * python3 utils/tracers/scripts/memory_usage.py tmp.memory.cvs
 * ```
 *
 * ### Result
 *
 * ![](images/memory_tracer_plot.png)
 *
 * Since: 0.14
 */
use gst::glib;
use gst::glib::Properties;
use gst::prelude::*;
use gst::subclass::prelude::*;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "memory-tracer",
        gst::DebugColorFlags::empty(),
        Some("Tracer to collect information about GStreamer memory allocations"),
    )
});

struct MemoryEvent {
    timestamp: u64,
    ptr: usize,
    parent: usize,
    size: usize,
    is_alloc: bool,
    memory_type: &'static str,
}

impl MemoryEvent {
    fn event_type(&self) -> &str {
        if self.is_alloc {
            "alloc"
        } else {
            "free"
        }
    }
}

#[derive(Debug)]
struct Settings {
    file: PathBuf,
}

impl Default for Settings {
    fn default() -> Self {
        let mut file = gst::glib::tmp_dir();
        file.push(format!("{:?}-memory_tracer.log", std::process::id()));
        Self { file }
    }
}

#[derive(Default)]
struct State {
    log: Vec<MemoryEvent>,
    logs_written: bool,
}
#[derive(Properties, Default)]
#[properties(wrapper_type = super::MemoryTracer)]
pub struct MemoryTracer {
    state: Mutex<State>,
    #[property(
        name="file",
        set = Self::set_file,
        type = String,
        blurb = "Path to the file to write memory usage information",
    )]
    settings: Mutex<Settings>,
}

#[glib::object_subclass]
impl ObjectSubclass for MemoryTracer {
    const NAME: &'static str = "GstMemoryTracer";
    type Type = super::MemoryTracer;
    type ParentType = gst::Tracer;
}

impl MemoryTracer {
    fn set_file(&self, file: String) {
        let mut settings = self.settings.lock().unwrap();
        settings.file = PathBuf::from(file);
    }

    fn write_log(&self, file_path: Option<String>) {
        use std::io::prelude::*;

        let settings = self.settings.lock().unwrap();
        let mut file = match file_path.map_or_else(
            || std::fs::File::create(&settings.file),
            |path| std::fs::File::create(PathBuf::from(path)),
        ) {
            Ok(file) => file,
            Err(err) => {
                gst::error!(CAT, imp = self, "Failed to create file: {err}");
                return;
            }
        };

        gst::info!(CAT, imp = self, "Writing file {:?}", file);

        drop(settings);

        let mut state = self.state.lock().unwrap();
        let log = std::mem::take(&mut state.log);
        state.logs_written = true;
        drop(state);

        for event in &log {
            if let Err(err) = writeln!(
                &mut file,
                "{},{},{},0x{:08x},{:?},{}",
                event.timestamp,
                event.event_type(),
                event.ptr,
                event.parent,
                event.memory_type,
                event.size
            ) {
                gst::error!(CAT, imp = self, "Failed to write to file: {err}");
            }
        }
    }
}

#[glib::derived_properties]
impl ObjectImpl for MemoryTracer {
    fn signals() -> &'static [glib::subclass::Signal] {
        static SIGNALS: LazyLock<Vec<glib::subclass::Signal>> = LazyLock::new(|| {
            vec![glib::subclass::Signal::builder("write-log")
                .action()
                .param_types([Option::<String>::static_type()])
                .class_handler(|args| {
                    let obj = args[0].get::<super::MemoryTracer>().unwrap();
                    let file = args[1].get::<Option<String>>().unwrap();

                    obj.imp().write_log(file);

                    None
                })
                .build()]
        });

        SIGNALS.as_ref()
    }

    fn constructed(&self) {
        self.parent_constructed();

        self.register_hook(TracerHook::MemoryInit);
        self.register_hook(TracerHook::MemoryFreePre);
    }

    fn dispose(&self) {
        if self.state.lock().unwrap().logs_written {
            gst::info!(
                CAT,
                "Logs were written manually, not overwriting on dispose"
            );
            return;
        }

        self.write_log(None);
    }
}

impl TracerImpl for MemoryTracer {
    const USE_STRUCTURE_PARAMS: bool = true;

    fn memory_init(&self, ts: u64, memory: &gst::MemoryRefTrace) {
        let mut state = self.state.lock().unwrap();
        let size = memory.maxsize();
        let ptr = memory.as_ptr() as usize;

        let parent = memory.parent().map_or(0_usize, |p| p.as_ptr() as usize);

        state.log.push(MemoryEvent {
            timestamp: ts,
            ptr,
            parent,
            is_alloc: true,
            memory_type: memory
                .allocator()
                .map_or("unknown", |alloc| alloc.memory_type()),
            size,
        });
    }

    fn memory_free_pre(&self, ts: u64, memory: &gst::MemoryRef) {
        let mut state = self.state.lock().unwrap();
        let ptr = memory.as_ptr() as usize;

        let parent = memory.parent().map_or(0_usize, |p| p.as_ptr() as usize);
        state.log.push(MemoryEvent {
            timestamp: ts,
            parent,
            ptr,
            is_alloc: false,
            memory_type: memory
                .allocator()
                .map_or("unknown", |alloc| alloc.memory_type()),
            size: memory.maxsize(),
        });
    }
}

impl GstObjectImpl for MemoryTracer {}
