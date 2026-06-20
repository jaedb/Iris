// Copyright (C) 2024 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * tracer-pad-push-timings:
 *
 * This tracer measures how long it takes to push a buffer or buffer list out of a pad.
 *
 * Example:
 *
 * ```console
 * $ GST_TRACERS='pad-push-timings(file="/tmp/pad_push_timings.log")' gst-launch-1.0 audiotestsrc ! queue ! fakesink
 * ```
 *
 * The generated file is a CSV file of the format
 *
 * ```csv
 * timestamp,element:pad name,pad pointer,push duration
 * ```
 *
 * ## Parameters
 *
 * ### `file`
 *
 * Specifies the path to the file that will collect the CSV file with the push timings.
 *
 * By default the file is written to `/tmp/pad_push_timings.log`.
 *
 * ### `include-filter`
 *
 * Specifies a regular expression for the `element:pad` names that should be included.
 *
 * By default this is not set.
 *
 * ### `exclude-filter`
 *
 * Specifies a regular expression for the `element:pad` names that should **not** be included.
 *
 * By default this is not set.
 */
use std::collections::HashMap;
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
        "pad-push-timings",
        gst::DebugColorFlags::empty(),
        Some("Tracer to pad push timings"),
    )
});

#[derive(Debug)]
struct Settings {
    file: PathBuf,
    include_filter: Option<Regex>,
    exclude_filter: Option<Regex>,
}

impl Default for Settings {
    fn default() -> Self {
        let mut file = glib::tmp_dir();
        file.push("pad_push_timings.log");

        Self {
            file,
            include_filter: None,
            exclude_filter: None,
        }
    }
}

impl Settings {
    fn update_from_params(&mut self, imp: &PadPushTimings, params: String) {
        let s = match gst::Structure::from_str(&format!("pad-push-timings,{params}")) {
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
    pads: HashMap<usize, Pad>,
    log: Vec<LogLine>,
    settings: Settings,
}

struct Pad {
    parent_name: Option<Arc<glib::GString>>,
    pad_name: Arc<glib::GString>,
    pending_push_start: Option<u64>,
    include: bool,
}

struct LogLine {
    timestamp: u64,
    parent_name: Option<Arc<glib::GString>>,
    pad_name: Arc<glib::GString>,
    ptr: usize,
    push_duration: u64,
}

#[derive(Default)]
pub struct PadPushTimings {
    state: Mutex<State>,
}

#[glib::object_subclass]
impl ObjectSubclass for PadPushTimings {
    const NAME: &'static str = "GstPadPushTimings";
    type Type = super::PadPushTimings;
    type ParentType = gst::Tracer;
}

impl ObjectImpl for PadPushTimings {
    fn constructed(&self) {
        self.parent_constructed();

        if let Some(params) = self.obj().property::<Option<String>>("params") {
            let mut state = self.state.lock().unwrap();
            state.settings.update_from_params(self, params);
        }

        self.register_hook(TracerHook::PadPushPre);
        self.register_hook(TracerHook::PadPushListPre);
        self.register_hook(TracerHook::PadPushPost);
        self.register_hook(TracerHook::PadPushListPost);
        self.register_hook(TracerHook::ObjectDestroyed);
    }

    fn dispose(&self) {
        use std::io::prelude::*;

        let state = self.state.lock().unwrap();

        let mut file = match std::fs::File::create(&state.settings.file) {
            Ok(file) => file,
            Err(err) => {
                gst::error!(CAT, imp = self, "Failed to create file: {err}");
                return;
            }
        };

        gst::debug!(
            CAT,
            imp = self,
            "Writing file {}",
            state.settings.file.display()
        );

        for LogLine {
            timestamp,
            parent_name,
            pad_name,
            ptr,
            push_duration,
        } in &state.log
        {
            let res = if let Some(parent_name) = parent_name {
                writeln!(
                    &mut file,
                    "{timestamp},{parent_name}:{pad_name},0x{ptr:08x},{push_duration}"
                )
            } else {
                writeln!(
                    &mut file,
                    "{timestamp},:{pad_name},0x{ptr:08x},{push_duration}"
                )
            };
            if let Err(err) = res {
                gst::error!(CAT, imp = self, "Failed to write to file: {err}");
                return;
            }
        }
    }
}

impl GstObjectImpl for PadPushTimings {}

impl TracerImpl for PadPushTimings {
    fn pad_push_pre(&self, ts: u64, pad: &gst::Pad, _buffer: &gst::Buffer) {
        self.push_pre(ts, pad);
    }

    fn pad_push_list_pre(&self, ts: u64, pad: &gst::Pad, _list: &gst::BufferList) {
        self.push_pre(ts, pad);
    }

    fn pad_push_post(
        &self,
        ts: u64,
        pad: &gst::Pad,
        _result: Result<gst::FlowSuccess, gst::FlowError>,
    ) {
        self.push_post(ts, pad);
    }

    fn pad_push_list_post(
        &self,
        ts: u64,
        pad: &gst::Pad,
        _result: Result<gst::FlowSuccess, gst::FlowError>,
    ) {
        self.push_post(ts, pad);
    }

    fn object_destroyed(&self, _ts: u64, object: std::ptr::NonNull<gst::ffi::GstObject>) {
        let ptr = object.as_ptr() as usize;
        let mut state = self.state.lock().unwrap();
        state.pads.remove(&ptr);
    }
}

impl PadPushTimings {
    fn push_pre(&self, ts: u64, pad: &gst::Pad) {
        let ptr = pad.as_ptr() as usize;
        let mut state = self.state.lock().unwrap();

        let State {
            ref mut pads,
            ref settings,
            ..
        } = &mut *state;

        let pad = pads.entry(ptr).or_insert_with(|| {
            let parent_name = pad.parent().map(|p| p.name());
            let pad_name = pad.name();

            let mut include = true;
            let name = if let Some(ref parent_name) = parent_name {
                format!("{parent_name}:{pad_name}")
            } else {
                format!(":{pad_name}")
            };

            if let Some(ref filter) = settings.include_filter {
                if !filter.is_match(&name) {
                    include = false;
                }
            }
            if let Some(ref filter) = settings.exclude_filter {
                if filter.is_match(&name) {
                    include = false;
                }
            }

            Pad {
                parent_name: parent_name.map(Arc::new),
                pad_name: Arc::new(pad_name),
                pending_push_start: None,
                include,
            }
        });

        if !pad.include {
            return;
        }

        assert!(pad.pending_push_start.is_none());
        pad.pending_push_start = Some(ts);
    }

    fn push_post(&self, ts: u64, pad: &gst::Pad) {
        let ptr = pad.as_ptr() as usize;
        let mut state = self.state.lock().unwrap();

        let State {
            ref mut pads,
            ref mut log,
            ..
        } = &mut *state;

        let Some(pad) = pads.get_mut(&ptr) else {
            return;
        };
        if !pad.include {
            return;
        }

        let push_start = pad.pending_push_start.take().unwrap();

        log.push(LogLine {
            timestamp: push_start,
            parent_name: pad.parent_name.clone(),
            pad_name: pad.pad_name.clone(),
            ptr,
            push_duration: ts - push_start,
        });
    }
}
