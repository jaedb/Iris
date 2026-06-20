// Copyright (C) 2022 OneStream Live <guillaume.desmottes@onestream.live>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * tracer-pipeline-snapshot:
 *
 * This tracer provides an easy way to take a snapshot of all the pipelines without
 * having to modify the application.
 * One just has to load the tracer and send the `SIGUSR1` UNIX signal to take snapshots.
 * It currently only works on UNIX systems.
 *
 * When taking a snapshot pipelines are saved to DOT files, but the tracer may be
 * extended in the future to dump more information.
 *
 * Example:
 *
 * ```console
 * $ GST_TRACERS="pipeline-snapshot" GST_DEBUG_DUMP_DOT_DIR=. gst-launch-1.0 audiotestsrc ! fakesink
 * ```
 * You can then trigger a snapshot using:
 * ```console
 * $ kill -SIGUSR1 $(pidof gst-launch-1.0)
 * ```
 *
 * Parameters can be passed to configure the tracer:
 * - `dot-dir` (string, default: None): directory where to place dot files (overriding `GST_DEBUG_DUMP_DOT_DIR`).
 * - `xdg-cache`: Instead of using `GST_DEBUG_DUMP_DOT_DIR` or `dot-dir`, use `$XDG_CACHE_DIR/gstreamer-dots` to save dot files.
 * - `dot-prefix` (string, default: "pipeline-snapshot-"): when dumping pipelines to a `dot` file each file is named `$prefix$pipeline_name.dot`.
 * - `dot-ts` (boolean, default: "true"): if the current timestamp should be added as a prefix to each pipeline `dot` file.
 * - `cleanup-mode` (enum, default: "none"): Determines how .dot files are cleaned up:
 *     - "initial": Removes all existing .dot files from the target folder when the tracer starts
 *     - "automatic": Performs cleanup before each snapshot. If folder-mode is enabled, cleans up .dot files within folders.
 *                    If folder-mode is None, cleans up .dot files directly in the target directory
 *     - "none": Never removes any .dot files
 * - `folder-mode` (enum, default: "none"): Controls how .dot files are organized in folders:
 *     - "none": All .dot files are stored directly in the target directory without subfolder organization
 *     - "numbered": Creates a new numbered folder (starting from 0) for each snapshot operation
 *     - "timed": Creates a new folder named with the current timestamp for each snapshot operation
 * - `dots-viewer-websocket-url`: A websocket URL to connect to a dots-viewer server instance,
 *                                allowing the user to snapshot running pipelines from the web
*                                page. To trigger a snapshot, the user should send a json message
*                                with the following format:
*                                ```json
*                                {
*                                    "type": "Snapshot"
*                                }
*                                ```
 *
 * Examples:
 *
 * Basic usage with custom prefix and timestamp:
 * ```console
 * $ GST_TRACERS="pipeline-snapshot(dot-prefix="badger-",dot-ts=true,xdg-cache=true)" GST_DEBUG_DUMP_DOT_DIR=. gst-launch-1.0 audiotestsrc ! fakesink
 * ```
 *
 * Using numbered folders with automatic cleanup:
 * ```console
 * $ GST_TRACERS="pipeline-snapshot(folder-mode=numbered,cleanup-mode=automatic)" GST_DEBUG_DUMP_DOT_DIR=. gst-launch-1.0 audiotestsrc ! fakesink
 * ```
 *
 * Using timestamped folders with initial cleanup:
 * ```console
 * $ GST_TRACERS="pipeline-snapshot(folder-mode=timed,cleanup-mode=initial)" GST_DEBUG_DUMP_DOT_DIR=. gst-launch-1.0 audiotestsrc ! fakesink
 * ```
 */
use futures::prelude::*;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex, RwLock};
use tokio::runtime;

use async_tungstenite::tungstenite::Message;
use gst::glib;
use gst::glib::translate::ToGlibPtr;
use gst::glib::Properties;
use gst::prelude::*;
use gst::subclass::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "pipeline-snapshot",
        gst::DebugColorFlags::empty(),
        Some("pipeline snapshot tracer"),
    )
});

pub static RUNTIME: LazyLock<runtime::Runtime> = LazyLock::new(|| {
    runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(1)
        .build()
        .unwrap()
});

static START_TIME: LazyLock<gst::ClockTime> = LazyLock::new(gst::get_timestamp);

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Clone)]
struct ElementPtr(std::ptr::NonNull<gst::ffi::GstElement>);

unsafe impl Send for ElementPtr {}
unsafe impl Sync for ElementPtr {}

impl ElementPtr {
    fn from_ref(element: &gst::Element) -> Self {
        let p = element.to_glib_none().0;
        Self(std::ptr::NonNull::new(p).unwrap())
    }

    fn from_object_ptr(p: std::ptr::NonNull<gst::ffi::GstObject>) -> Self {
        let p = p.cast();
        Self(p)
    }
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstPipelineSnapshotCleanupMode")]
#[non_exhaustive]
pub enum CleanupMode {
    #[enum_value(
        name = "CleanupInitial: Remove all .dot files from folder when starting",
        nick = "initial"
    )]
    Initial,
    #[enum_value(
        name = "CleanupAutomatic: cleanup .dot files before each snapshots if pipeline-snapshot::folder-mode is not None \
                otherwise cleanup `.dot` files in folders",
        nick = "automatic"
    )]
    Automatic,
    #[enum_value(name = "None: Never remove any dot file", nick = "none")]
    None,
}

impl std::str::FromStr for CleanupMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "initial" => Ok(CleanupMode::Initial),
            "automatic" => Ok(CleanupMode::Automatic),
            "none" => Ok(CleanupMode::None),
            _ => Err(format!("unknown cleanup mode: {s}")),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstPipelineSnapshotFolderMode")]
#[non_exhaustive]
pub enum FolderMode {
    #[enum_value(name = "None: Do not use folders to store dot files", nick = "none")]
    None,
    #[enum_value(
        name = "Numbered: Use folders to store dot files, each time `.snapshot()` is called a new folder is created \
                and named with a number starting from 0.",
        nick = "numbered"
    )]
    Numbered,
    #[enum_value(
        name = "Timed: Use folders to store dot files, each time `.snapshot()` is called a new folder is created \
                         and named with the current timestamp.",
        nick = "timed"
    )]
    Timed,
}

impl std::str::FromStr for FolderMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(FolderMode::None),
            "numbered" => Ok(FolderMode::Numbered),
            "timed" => Ok(FolderMode::Timed),
            _ => Err(format!("unknown folder mode: {s}")),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
enum DotViewerMessageType {
    Snapshot,
}

#[derive(Debug, Serialize, Deserialize)]
struct DotViewerMessage {
    #[serde(rename = "type")]
    pub type_: DotViewerMessageType,
}

#[derive(Debug)]
struct Settings {
    dot_prefix: Option<String>,
    dot_ts: bool,
    dot_pipeline_ptr: bool,
    dot_dir: Option<String>,
    xdg_cache: bool,
    cleanup_mode: CleanupMode,
    folder_mode: FolderMode,
    dots_viewer_ws_url: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            dot_dir: None,
            dot_prefix: Some("pipeline-snapshot-".to_string()),
            dot_ts: true,
            xdg_cache: false,
            cleanup_mode: CleanupMode::None,
            dot_pipeline_ptr: false,
            folder_mode: FolderMode::None,
            dots_viewer_ws_url: None,
        }
    }
}

impl Settings {
    fn set_xdg_cache(&mut self, xdg_cache: bool) {
        self.xdg_cache = xdg_cache;
        if xdg_cache {
            let mut path = gst::glib::user_cache_dir();
            path.push("gstreamer-dots");
            self.dot_dir = path.to_str().map(|s| s.to_string());
        }
    }

    fn set_dot_dir(&mut self, dot_dir: Option<String>) {
        if self.xdg_cache {
            if dot_dir.is_some() {
                gst::warning!(CAT, "Trying to set a dot dir while using XDG cache");
            }
        } else if let Some(dot_dir) = dot_dir {
            self.dot_dir = Some(dot_dir);
        } else {
            self.dot_dir = std::env::var("GST_DEBUG_DUMP_DOT_DIR").ok();
        }
    }

    fn update_from_params(&mut self, imp: &PipelineSnapshot, params: String) {
        let s = match gst::Structure::from_str(&format!("pipeline-snapshot,{params}")) {
            Ok(s) => s,
            Err(err) => {
                gst::warning!(CAT, imp = imp, "failed to parse tracer parameters: {}", err);
                return;
            }
        };

        if let Ok(xdg_cache) = s.get("xdg-cache") {
            self.set_xdg_cache(xdg_cache);
            gst::log!(
                CAT,
                imp = imp,
                "Using xdg_cache -> dot-dir = {:?}",
                self.dot_dir
            );
        }

        if let Ok(dot_dir) = s.get("dot-dir") {
            self.set_dot_dir(dot_dir);
            gst::log!(CAT, imp = imp, "dot-dir = {:?}", self.dot_dir);
        }

        if let Ok(dot_prefix) = s.get("dot-prefix") {
            gst::log!(CAT, imp = imp, "dot-prefix = {:?}", dot_prefix);
            self.dot_prefix = dot_prefix;
        }

        if let Ok(dot_ts) = s.get("dot-ts") {
            gst::log!(CAT, imp = imp, "dot-ts = {}", dot_ts);
            self.dot_ts = dot_ts;
        }

        if let Ok(dot_pipeline_ptr) = s.get("dot-pipeline-ptr") {
            gst::log!(CAT, imp = imp, "dot-pipeline-ptr = {}", dot_pipeline_ptr);
            self.dot_pipeline_ptr = dot_pipeline_ptr;
        }

        if let Ok(cleanup_mod) = s.get::<&str>("cleanup-mode") {
            self.cleanup_mode = match cleanup_mod.parse() {
                Ok(mode) => mode,
                Err(err) => {
                    gst::warning!(CAT, imp = imp, "unknown cleanup-mode: {}", err);
                    CleanupMode::None
                }
            };
        }

        if let Ok(websocket) = s.get::<&str>("dots-viewer-ws-url") {
            gst::debug!(CAT, imp = imp, "dots-viewer-websocket-url = {}", websocket);
            self.dots_viewer_ws_url = Some(websocket.to_string());
        }

        if let Ok(folder_mode) = s.get::<&str>("folder-mode") {
            self.folder_mode = match folder_mode.parse() {
                Ok(mode) => mode,
                Err(err) => {
                    gst::warning!(CAT, imp = imp, "unknown folder-mode: {}", err);
                    FolderMode::None
                }
            };
        }
    }
}

#[derive(Debug, Default)]
struct State {
    current_folder: u32,
    pipelines: HashMap<ElementPtr, glib::WeakRef<gst::Element>>,
}

#[derive(Properties, Debug, Default)]
#[properties(wrapper_type = super::PipelineSnapshot)]
pub struct PipelineSnapshot {
    #[property(name="dot-dir", get, set = Self::set_dot_dir, construct_only, type = String, member = dot_dir, blurb = "Directory where to place dot files")]
    #[property(name="dots-viewer-ws-url", get, set = Self::set_dot_viewer_ws_url, construct_only, type = String, member = dots_viewer_ws_url, blurb = "gst-dots-viewer websocket URL")]
    #[property(name="xdg-cache", get, set = Self::set_xdg_cache, construct_only, type = bool, member = xdg_cache, blurb = "Use $XDG_CACHE_DIR/gstreamer-dots")]
    #[property(name="dot-prefix", get, set, type = String, member = dot_prefix, blurb = "Prefix for dot files")]
    #[property(name="dot-ts", get, set, type = bool, member = dot_ts, blurb = "Add timestamp to dot files")]
    #[property(name="dot-pipeline-ptr", get, set, type = bool, member = dot_pipeline_ptr, blurb = "Add pipeline ptr value to dot files")]
    #[property(name="cleanup-mode", get  = |s: &Self| s.settings.read().unwrap().cleanup_mode, set, type = CleanupMode, member = cleanup_mode, blurb = "Cleanup mode", builder(CleanupMode::None))]
    #[property(name="folder-mode",
               get=|s: &Self| s.settings.read().unwrap().folder_mode,
               set,
               type = FolderMode, member = folder_mode, blurb = "How to create folder each time a snapshot of all pipelines is made", builder(FolderMode::None))]
    settings: RwLock<Settings>,
    handles: Mutex<Option<Handles>>,
    state: Arc<Mutex<State>>,
}

#[derive(Debug)]
struct Handles {
    #[cfg(unix)]
    signal: signal_hook::iterator::Handle,
    thread: std::thread::JoinHandle<()>,
    websocket: Option<tokio::task::JoinHandle<()>>,
}

#[glib::object_subclass]
impl ObjectSubclass for PipelineSnapshot {
    const NAME: &'static str = "GstPipelineSnapshot";
    type Type = super::PipelineSnapshot;
    type ParentType = gst::Tracer;
}

#[glib::derived_properties]
impl ObjectImpl for PipelineSnapshot {
    fn constructed(&self) {
        let _ = START_TIME.as_ref();
        self.parent_constructed();

        let mut settings = self.settings.write().unwrap();
        if let Some(params) = self.obj().property::<Option<String>>("params") {
            settings.update_from_params(self, params);
        }

        if settings.cleanup_mode == CleanupMode::Initial {
            drop(settings);
            self.cleanup_dots(&self.settings.read().unwrap().dot_dir.as_ref(), true);
        } else {
            drop(settings);
        }

        self.register_hook(TracerHook::ElementNew);
        self.register_hook(TracerHook::ObjectDestroyed);

        if let Err(err) = self.setup_signal() {
            gst::warning!(CAT, imp = self, "failed to setup UNIX signals: {}", err);
        }

        self.setup_websocket();
    }

    fn signals() -> &'static [glib::subclass::Signal] {
        static SIGNALS: LazyLock<Vec<glib::subclass::Signal>> = LazyLock::new(|| {
            vec![glib::subclass::Signal::builder("snapshot")
                .action()
                .class_handler(|args| {
                    args[0].get::<super::PipelineSnapshot>().unwrap().snapshot();

                    None
                })
                .build()]
        });

        SIGNALS.as_ref()
    }

    fn dispose(&self) {
        let mut handles = self.handles.lock().unwrap();
        if let Some(handles) = handles.take() {
            #[cfg(unix)]
            handles.signal.close();
            handles.thread.join().unwrap();
        }
    }
}

impl GstObjectImpl for PipelineSnapshot {}

impl TracerImpl for PipelineSnapshot {
    fn element_new(&self, _ts: u64, element: &gst::Element) {
        if element.is::<gst::Pipeline>() {
            let pipeline_ptr = ElementPtr::from_ref(element);

            let weak = element.downgrade();
            let mut state = self.state.lock().unwrap();
            state.pipelines.insert(pipeline_ptr, weak);
            gst::debug!(
                CAT,
                imp = self,
                "new pipeline: {} ({:?}) got {} now",
                element.name(),
                pipeline_ptr,
                state.pipelines.len()
            );
        }
    }

    fn object_destroyed(&self, _ts: u64, object: std::ptr::NonNull<gst::ffi::GstObject>) {
        let mut state = self.state.lock().unwrap();
        let object = ElementPtr::from_object_ptr(object);
        if state.pipelines.remove(&object).is_some() {
            gst::debug!(
                CAT,
                imp = self,
                "Pipeline removed: {:?} - {} remaining",
                object,
                state.pipelines.len()
            );
        }
    }
}

impl PipelineSnapshot {
    async fn handle_websocket(
        weak_self: glib::WeakRef<super::PipelineSnapshot>,
        host: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let url = url::Url::parse(&host)?;
        let (ws_stream, _) = async_tungstenite::tokio::connect_async(url).await?;
        let (mut write, mut read) = ws_stream.split();

        gst::debug!(CAT, "Connected to WebSocket server at {}", host);
        write
            .send(Message::Text(
                serde_json::json!({
                    "type": "Hello",
                })
                .to_string()
                .into(),
            ))
            .await?;

        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    let msg: DotViewerMessage = match serde_json::from_str(&text) {
                        Ok(s) => s,
                        Err(e) => {
                            gst::error!(CAT, "Failed to parse message: {}", e);
                            continue;
                        }
                    };

                    match msg.type_ {
                        DotViewerMessageType::Snapshot => {
                            if let Some(this) = weak_self.upgrade() {
                                gst::info!(CAT, "Received dot-pipeline request from the WebSocket");
                                this.snapshot();
                            }
                        }
                    }
                }
                Ok(Message::Close(_)) => break,
                Err(e) => {
                    gst::warning!(CAT, "WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }

        let _ = write.close(None).await;
        Ok(())
    }

    fn setup_websocket(&self) {
        let settings = self.settings.read().unwrap();

        if let Some(dots_viewer_websocket_url) = &settings.dots_viewer_ws_url {
            let host = dots_viewer_websocket_url.clone();
            let obj = self.obj().downgrade();

            let handle = RUNTIME.spawn(async move {
                loop {
                    gst::debug!(CAT, "Connecting to WebSocket server at {}", host);
                    if let Err(e) = Self::handle_websocket(obj.clone(), host.clone()).await {
                        gst::debug!(
                            CAT,
                            "WebSocket {} connection failed: {}. Retrying in 5 seconds...",
                            host,
                            e
                        );
                    }

                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
            });

            let mut handles = self.handles.lock().unwrap();
            if let Some(handles) = handles.as_mut() {
                handles.websocket = Some(handle);
            }
        }
    }

    fn set_dot_dir(&self, dot_dir: Option<String>) {
        let mut settings = self.settings.write().unwrap();
        settings.set_dot_dir(dot_dir);
    }

    fn set_dot_viewer_ws_url(&self, url: Option<String>) {
        let mut settings = self.settings.write().unwrap();
        settings.dots_viewer_ws_url = url;
    }

    fn set_xdg_cache(&self, use_xdg_cache: bool) {
        let mut settings = self.settings.write().unwrap();
        settings.set_xdg_cache(use_xdg_cache);
    }

    pub(crate) fn snapshot(&self) {
        let settings = self.settings.read().unwrap();

        let dot_dir = if let Some(dot_dir) = settings.dot_dir.as_ref() {
            if !matches!(settings.folder_mode, FolderMode::None) {
                let dot_dir = match settings.folder_mode {
                    FolderMode::Numbered => {
                        let mut state = self.state.lock().unwrap();
                        let res = state.current_folder;
                        state.current_folder += 1;

                        format!("{dot_dir}/{res}")
                    }
                    FolderMode::Timed => {
                        let datetime: chrono::DateTime<chrono::Local> = chrono::Local::now();
                        format!("{dot_dir}/{}", datetime.format("%Y-%m-%d %H:%M:%S"))
                    }
                    _ => unreachable!(),
                };

                if let Err(err) = std::fs::create_dir_all(&dot_dir) {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Failed to create folder {}: {}",
                        dot_dir,
                        err
                    );
                    return;
                }

                dot_dir
            } else {
                dot_dir.clone()
            }
        } else {
            gst::info!(CAT, imp = self, "No dot-dir set, not dumping pipelines");
            return;
        };

        if matches!(settings.cleanup_mode, CleanupMode::Automatic) {
            self.cleanup_dots(&Some(&dot_dir), false);
        }

        let ts = if settings.dot_ts {
            format!("{:?}-", gst::get_timestamp() - *START_TIME)
        } else {
            "".to_string()
        };

        let pipelines = {
            let state = self.state.lock().unwrap();
            gst::log!(
                CAT,
                imp = self,
                "dumping {} pipelines",
                state.pipelines.len()
            );

            state
                .pipelines
                .iter()
                .filter_map(|(ptr, w)| {
                    let pipeline = w.upgrade();

                    if pipeline.is_none() {
                        gst::warning!(CAT, imp = self, "Pipeline {ptr:?} disappeared");
                    }
                    pipeline
                })
                .collect::<Vec<_>>()
        };

        for pipeline in pipelines.into_iter() {
            let pipeline = pipeline.downcast::<gst::Pipeline>().unwrap();
            gst::debug!(CAT, imp = self, "dump {}", pipeline.name());

            let pipeline_ptr = if settings.dot_pipeline_ptr {
                let pipeline_ptr: *const gst::ffi::GstPipeline = pipeline.to_glib_none().0;

                format!("-{pipeline_ptr:?}")
            } else {
                "".to_string()
            };
            let dot_path = format!(
                "{dot_dir}/{ts}{}{}{pipeline_ptr}.dot",
                settings.dot_prefix.as_ref().map_or("", |s| s.as_str()),
                pipeline.name(),
            );

            let data = pipeline.debug_to_dot_data(gst::DebugGraphDetails::all());

            if let Err(e) = self.write_dot_file_atomically(Path::new(&dot_path), data.as_bytes()) {
                gst::warning!(CAT, imp = self, "Failed to write {}: {}", dot_path, e);
            }
        }
    }

    fn write_dot_file_atomically(&self, path: &Path, data: &[u8]) -> std::io::Result<()> {
        // Create a temporary file in the same directory
        let tmp_path = path.with_extension("dot.tmp");

        // Write data to temporary file
        {
            let mut tmp_file = std::fs::File::create(&tmp_path)?;
            tmp_file.write_all(data)?;

            // Ensure all data is written to disk
            tmp_file.sync_all()?;
        }

        // Atomically rename temporary file to target path
        std::fs::rename(tmp_path, path)?;

        Ok(())
    }

    #[cfg(unix)]
    fn setup_signal(&self) -> anyhow::Result<()> {
        use signal_hook::consts::signal::*;
        use signal_hook::iterator::Signals;

        let mut signals = Signals::new([SIGUSR1])?;
        let signal_handle = signals.handle();

        let this_weak = self.obj().downgrade();
        let thread_handle = std::thread::spawn(move || {
            for signal in &mut signals {
                match signal {
                    SIGUSR1 => {
                        if let Some(this) = this_weak.upgrade() {
                            this.snapshot();
                        } else {
                            break;
                        };
                    }
                    _ => unreachable!(),
                }
            }
        });

        let mut handles = self.handles.lock().unwrap();
        *handles = Some(Handles {
            signal: signal_handle,
            thread: thread_handle,
            websocket: None,
        });

        Ok(())
    }

    #[cfg(not(unix))]
    fn setup_signal(&self) -> anyhow::Result<()> {
        anyhow::bail!("only supported on UNIX system");
    }

    fn cleanup_dots(&self, dot_dir: &Option<&String>, recurse: bool) {
        if let Some(dot_dir) = dot_dir {
            gst::info!(CAT, imp = self, "Cleaning up {}", dot_dir);
            let mut paths = match std::fs::read_dir(dot_dir) {
                Ok(entries) => {
                    entries
                        .filter_map(|entry| {
                            let entry = entry.ok()?; // Handle possible errors when reading directory entries
                            let path = entry.path();
                            let extension = path.extension()?.to_str()?; // Get the extension as a string
                            if extension.ends_with(".dot") {
                                Some(path.to_path_buf())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<PathBuf>>()
                }
                Err(e) => {
                    gst::warning!(CAT, imp = self, "Failed to read {}: {}", dot_dir, e);
                    return;
                }
            };

            if recurse {
                paths.append(
                    &mut walkdir::WalkDir::new(dot_dir)
                        .into_iter()
                        .filter_map(|entry| {
                            let entry = entry.ok()?;
                            let path = entry.path();
                            let extension = path.extension()?.to_str()?;
                            if extension == "dot" {
                                Some(path.to_path_buf())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<PathBuf>>(),
                )
            }

            for path in paths {
                if let Err(e) = std::fs::remove_file(&path) {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Failed to remove {}: {}",
                        path.display(),
                        e
                    );
                }
            }
        }
    }
}
