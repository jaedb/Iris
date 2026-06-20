// Copyright (C) 2021 OneStream Live <guillaume.desmottes@onestream.live>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
};

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use std::sync::LazyLock;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "uriplaylistbin",
        gst::DebugColorFlags::empty(),
        Some("Uri Playlist Bin"),
    )
});

#[derive(Debug, thiserror::Error)]
enum PlaylistError {
    #[error("plugin missing: {error}")]
    PluginMissing { error: anyhow::Error },
}

#[derive(Debug, Clone)]
struct Settings {
    uris: Vec<String>,
    iterations: u32,
    cache: bool,
    cache_dir: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            uris: vec![],
            iterations: 1,
            cache: false,
            cache_dir: None,
        }
    }
}

#[derive(Debug)]
struct State {
    uridecodebin: gst::Element,
    playlist: Playlist,
    /// next current items, updated when uridecodebin updates its current-uri property
    pending_current_items: VecDeque<Option<Item>>,
    current_item: Option<Item>,
    /// key are src pads from uridecodebin
    pads: HashMap<gst::Pad, Pads>,
    /// URIs cached on disk, only used if `cache` property is enabled.
    cached_uris: HashMap<String, PathBuf>,

    // read-only properties
    current_iteration: u32,
    current_uri_index: u64,
}

#[derive(Debug)]
struct Pads {
    /// requested streamsynchronizer sink pad
    ss_sink: gst::Pad,
    /// ghost pad of the associated src pad
    ghost_src: gst::GhostPad,
}

impl State {
    fn new(uris: Vec<String>, iterations: u32, uridecodebin: gst::Element) -> Self {
        Self {
            uridecodebin,
            playlist: Playlist::new(uris, iterations),
            pending_current_items: VecDeque::new(),
            current_item: None,
            pads: HashMap::new(),
            cached_uris: HashMap::new(),
            current_iteration: 0,
            current_uri_index: 0,
        }
    }

    fn update_iterations(&mut self, iterations: u32) {
        self.playlist.iterations = iterations;
    }
}

#[derive(Default)]
pub struct UriPlaylistBin {
    settings: Mutex<Settings>,
    state: Mutex<Option<State>>,
}

#[derive(Debug, Clone)]
struct Item {
    inner: Arc<Mutex<ItemInner>>,
}

impl Item {
    fn new(uri: String, index: usize) -> Self {
        let inner = ItemInner { uri, index };

        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    fn uri(&self) -> String {
        let inner = self.inner.lock().unwrap();
        inner.uri.clone()
    }

    fn index(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.index
    }
}

#[derive(Debug, Clone)]
struct ItemInner {
    uri: String,
    index: usize,
}

struct Playlist {
    uris: Vec<String>,
    iterations: u32,

    next_index: usize,
}

impl Playlist {
    fn new(uris: Vec<String>, iterations: u32) -> Self {
        Self {
            uris,
            iterations,
            next_index: 0,
        }
    }

    fn next(&mut self) -> Option<Item> {
        let uris_len = self.uris.len();
        let (iteration, uri_index) = (
            (self.next_index / uris_len) as u32,
            (self.next_index % uris_len),
        );

        if self.iterations != 0 && iteration >= self.iterations {
            // playlist is done
            return None;
        }

        let uri = self.uris[uri_index].clone();
        let item = Item::new(uri, self.next_index);

        self.next_index += 1;
        if self.next_index == usize::MAX {
            // prevent overflow with infinite playlist
            self.next_index = 0;
        }

        Some(item)
    }
}

impl std::fmt::Debug for Playlist {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Playlist")
            .field("uris", &self.uris)
            .finish()
    }
}

#[glib::object_subclass]
impl ObjectSubclass for UriPlaylistBin {
    const NAME: &'static str = "GstUriPlaylistBin";
    type Type = super::UriPlaylistBin;
    type ParentType = gst::Bin;
}

impl ObjectImpl for UriPlaylistBin {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecBoxed::builder::<Vec<String>>("uris")
                    .nick("URIs")
                    .blurb("URIs of the medias to play")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder("iterations")
                    .nick("Iterations")
                    .blurb("Number of time the playlist items should be played each (0 = unlimited)")
                    .default_value(1)
                    .mutable_playing()
                    .build(),
                /**
                 * GstUriPlaylistBin:cache:
                 *
                 * Cache playlist items from the network to disk so they are downloaded only once when playing multiple iterations.
                 *
                 * Since: plugins-rs-0.14.0
                 */
                glib::ParamSpecBoolean::builder("cache")
                    .nick("Cache")
                    .blurb("Cache playlist items from the network to disk so they are downloaded only once when playing multiple iterations.")
                    .mutable_ready()
                    .build(),
                /**
                 * GstUriPlaylistBin:cache-dir:
                 *
                 * The directory where playlist items are downloaded to, if 'cache' is enabled. If not set (default), the XDG cache directory is used.
                 *
                 * Since: plugins-rs-0.14.0
                 */
                glib::ParamSpecString::builder("cache-dir")
                    .nick("Cache directory")
                    .blurb("The directory where playlist items are downloaded to, if 'cache' is enabled. If not set (default), the XDG cache directory is used.")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder("current-iteration")
                    .nick("Current iteration")
                    .blurb("The index of the current playlist iteration, or 0 if the iterations property is 0 (unlimited playlist)")
                    .read_only()
                    .build(),
                glib::ParamSpecUInt64::builder("current-uri-index")
                    .nick("Current URI")
                    .blurb("The index from the uris property of the current URI being played")
                    .read_only()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "uris" => {
                let mut settings = self.settings.lock().unwrap();
                let new_value = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing uris from {:?} to {:?}",
                    settings.uris,
                    new_value,
                );
                settings.uris = new_value;
            }
            "iterations" => {
                let new_value = value.get().expect("type checked upstream");
                {
                    let mut settings = self.settings.lock().unwrap();
                    gst::info!(
                        CAT,
                        imp = self,
                        "Changing iterations from {:?} to {:?}",
                        settings.iterations,
                        new_value,
                    );
                    settings.iterations = new_value;
                }

                {
                    let mut state = self.state.lock().unwrap();
                    if let Some(state) = state.as_mut() {
                        state.update_iterations(new_value);
                    }
                }
            }
            "cache" => {
                let mut settings = self.settings.lock().unwrap();
                let new_value = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing cache from {:?} to {:?}",
                    settings.cache,
                    new_value,
                );
                settings.cache = new_value;
            }
            "cache-dir" => {
                let mut settings = self.settings.lock().unwrap();
                let new_value = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing cache-dir from {:?} to {:?}",
                    settings.cache_dir,
                    new_value,
                );
                settings.cache_dir = new_value;
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "uris" => {
                let settings = self.settings.lock().unwrap();
                settings.uris.to_value()
            }
            "iterations" => {
                let settings = self.settings.lock().unwrap();
                settings.iterations.to_value()
            }
            "current-iteration" => {
                let state = self.state.lock().unwrap();
                state
                    .as_ref()
                    .map(|state| state.current_iteration)
                    .unwrap_or(0)
                    .to_value()
            }
            "current-uri-index" => {
                let state = self.state.lock().unwrap();
                state
                    .as_ref()
                    .map(|state| state.current_uri_index)
                    .unwrap_or(0)
                    .to_value()
            }
            "cache" => {
                let settings = self.settings.lock().unwrap();
                settings.cache.to_value()
            }
            "cache-dir" => {
                let settings = self.settings.lock().unwrap();
                settings.cache_dir.to_value()
            }
            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.set_suppressed_flags(gst::ElementFlags::SOURCE | gst::ElementFlags::SINK);
        obj.set_element_flags(gst::ElementFlags::SOURCE);
    }
}

impl GstObjectImpl for UriPlaylistBin {}

impl BinImpl for UriPlaylistBin {
    fn handle_message(&self, mut msg: gst::Message) {
        match msg.view() {
            gst::MessageView::Error(_) => {
                let item = {
                    let state_guard = self.state.lock().unwrap();
                    let state = state_guard.as_ref().unwrap();

                    // uridecodebin likely failed because of the last URI we set
                    match state.pending_current_items.iter().last() {
                        Some(Some(item)) => Some(item.clone()),
                        _ => state.current_item.clone(),
                    }
                };

                if let Some(item) = item {
                    // add the URI of the failed item
                    let msg_ref = msg.make_mut();
                    let details = msg_ref.details_mut();
                    details.set("uri", item.uri());
                    details.set("index", item.index() as u64);
                    self.parent_handle_message(msg);
                } else {
                    self.parent_handle_message(msg)
                }
            }
            _ => self.parent_handle_message(msg),
        }
    }
}

impl ElementImpl for UriPlaylistBin {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Playlist Source",
                "Generic/Source",
                "Sequentially play uri streams",
                "Guillaume Desmottes <guillaume.desmottes@onestream.live>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let audio_src_pad_template = gst::PadTemplate::new(
                "audio_%u",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &gst::Caps::new_any(),
            )
            .unwrap();

            let video_src_pad_template = gst::PadTemplate::new(
                "video_%u",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &gst::Caps::new_any(),
            )
            .unwrap();

            let text_src_pad_template = gst::PadTemplate::new(
                "text_%u",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &gst::Caps::new_any(),
            )
            .unwrap();

            vec![
                audio_src_pad_template,
                video_src_pad_template,
                text_src_pad_template,
            ]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        if transition == gst::StateChange::NullToReady {
            if let Err(e) = self.start() {
                self.failed(e);
                return Err(gst::StateChangeError);
            }
        }

        let res = self.parent_change_state(transition);

        if transition == gst::StateChange::ReadyToNull {
            self.stop();
        }

        res
    }
}

impl UriPlaylistBin {
    fn start(&self) -> Result<(), PlaylistError> {
        gst::debug!(CAT, imp = self, "Starting");
        {
            let mut state_guard = self.state.lock().unwrap();
            assert!(state_guard.is_none());

            let settings = self.settings.lock().unwrap();
            // No need to enable caching if we play only one iteration
            let download = settings.cache && settings.iterations != 1;
            let uridecodebin = gst::ElementFactory::make("uridecodebin3")
                .name("playlist-uridecodebin")
                .property("download", download)
                .property("download-dir", &settings.cache_dir)
                .build()
                .map_err(|e| PlaylistError::PluginMissing { error: e.into() })?;
            drop(settings);

            let streamsynchronizer = gst::ElementFactory::make("streamsynchronizer")
                .name("playlist-streamsynchronizer")
                .build()
                .map_err(|e| PlaylistError::PluginMissing { error: e.into() })?;

            self.obj().add(&uridecodebin).unwrap();
            self.obj().add(&streamsynchronizer).unwrap();

            let bin_weak = self.obj().downgrade();
            let streamsynchronizer_clone = streamsynchronizer.clone();
            uridecodebin.connect_pad_added(move |_uridecodebin, src_pad| {
                let Some(bin) = bin_weak.upgrade() else {
                    return;
                };

                gst::debug!(
                    CAT,
                    obj = bin,
                    "uridecodebin src pad added: {}",
                    src_pad.name()
                );

                // connect the pad to streamsynchronizer
                let ss_sink = streamsynchronizer_clone
                    .request_pad_simple("sink_%u")
                    .unwrap();
                src_pad.link(&ss_sink).unwrap();
                let src_pad_name = ss_sink.name().to_string().replace("sink", "src");

                // ghost the associated streamsynchronizer src pad
                let ss_src = streamsynchronizer_clone.static_pad(&src_pad_name).unwrap();

                let ghost_src = gst::GhostPad::builder_with_target(&ss_src)
                    .unwrap()
                    .name(src_pad.name().as_str())
                    .build();

                ghost_src.set_active(true).unwrap();
                bin.add_pad(&ghost_src).unwrap();

                {
                    let mut state_guard = bin.imp().state.lock().unwrap();
                    let state = state_guard.as_mut().unwrap();

                    state
                        .pads
                        .insert(src_pad.clone(), Pads { ss_sink, ghost_src });
                }
            });

            let bin_weak = self.obj().downgrade();
            uridecodebin.connect_pad_removed(move |_uridecodebin, src_pad| {
                let Some(bin) = bin_weak.upgrade() else {
                    return;
                };

                gst::debug!(
                    CAT,
                    obj = bin,
                    "uridecodebin src pad removed: {}",
                    src_pad.name()
                );

                {
                    let mut state_guard = bin.imp().state.lock().unwrap();
                    let state = state_guard.as_mut().unwrap();

                    if let Some(pads) = state.pads.remove(src_pad) {
                        streamsynchronizer.release_request_pad(&pads.ss_sink);

                        pads.ghost_src.set_active(false).unwrap();
                        let _ = pads.ghost_src.set_target(None::<&gst::Pad>);
                        let _ = bin.remove_pad(&pads.ghost_src);
                    }
                }
            });

            let bin_weak = self.obj().downgrade();
            uridecodebin.connect("about-to-finish", false, move |args| {
                let uridecodebin = args[0].get::<gst::Bin>().unwrap();
                let bin = bin_weak.upgrade()?;
                let self_ = bin.imp();

                gst::debug!(CAT, obj = bin, "current URI about to finish");

                let cache = self_.settings.lock().unwrap().cache;

                // `about-to-finish` is emitted when the file has been fully buffered so we are sure it has been fully written to disk.
                if cache {
                    // retrieve cached path of the current item
                    let download_path = uridecodebin
                        .iterate_recurse()
                        .find(|e| {
                            e.factory()
                                .map(|factory| factory.name())
                                .unwrap_or_default()
                                == "downloadbuffer"
                        })
                        .map(|downloadbuffer| downloadbuffer.property::<String>("temp-location"))
                        .map(PathBuf::from)
                        .and_then(|path| path.canonicalize().ok());

                    // urisourcebin uses downloadbuffer only with some specific URI scheme (http, etc).
                    // So if it has not been used assume it's a local file and loop using the original (or already cached) URI.
                    if let Some(path) = download_path {
                        let mut state = self_.state.lock().unwrap();
                        if let Some(state) = state.as_mut() {
                            let uri = uridecodebin.property::<String>("uri");
                            // downloadbuffer will remove the file as soon as it's done with it so we need to make a copy.
                            let mut link_path = path.clone();
                            link_path.set_file_name(format!(
                                "item-{}-{}",
                                state.current_uri_index,
                                path.file_name()
                                    .and_then(|name| name.to_str())
                                    .unwrap_or_default()
                            ));

                            let mut cached = true;

                            // Try first creating a hard link to prevent a full copy.
                            if let Err(err) = std::fs::hard_link(&path, &link_path) {
                                gst::warning!(
                                    CAT,
                                    imp = self_,
                                    "Failed to hard link cached item, try copy: '{err}'"
                                );

                                if let Err(err) = std::fs::copy(&path, &link_path) {
                                    // Hard links are only supported with NTFS on Windows so fallback to copy.
                                    gst::warning!(
                                        CAT,
                                        imp = self_,
                                        "Failed to copy cached item: '{err}'"
                                    );

                                    cached = false;
                                }
                            }

                            if cached {
                                gst::log!(
                                    CAT,
                                    imp = self_,
                                    "URI {uri} cached to {}",
                                    link_path.display()
                                );
                                state.cached_uris.insert(uri, link_path);
                            }
                        }
                    }
                }

                let _ = self_.start_next_item();

                None
            });

            let bin_weak = self.obj().downgrade();
            uridecodebin.connect_notify(Some("current-uri"), move |_uridecodebin, _param| {
                // new current URI, update pending current item if needed
                let Some(bin) = bin_weak.upgrade() else {
                    return;
                };
                let self_ = bin.imp();

                let mut state_guard = self_.state.lock().unwrap();
                let state = state_guard.as_mut().unwrap();

                if let Some(new_current_item) = state.pending_current_items.pop_front() {
                    self_.update_current(state_guard, new_current_item);
                }
            });

            let settings = self.settings.lock().unwrap();

            *state_guard = Some(State::new(
                settings.uris.clone(),
                settings.iterations,
                uridecodebin,
            ));
        }

        self.start_next_item()?;

        Ok(())
    }

    fn start_next_item(&self) -> Result<(), PlaylistError> {
        let mut state_guard = self.state.lock().unwrap();
        let state = state_guard.as_mut().unwrap();

        let item = match state.playlist.next() {
            Some(item) => item,
            None => {
                gst::debug!(CAT, imp = self, "no more item to queue",);

                state.pending_current_items.push_back(None);
                return Ok(());
            }
        };

        let mut uri = item.uri();
        if let Some(path) = state.cached_uris.get(&uri) {
            uri = gst::glib::filename_to_uri(path, None).unwrap().to_string();
            gst::debug!(
                CAT,
                imp = self,
                "start next item from cache #{}: {uri}",
                item.index(),
            );
        } else {
            gst::debug!(CAT, imp = self, "start next item #{}: {uri}", item.index(),);
        }

        // don't hold the mutex when updating `uri` to prevent deadlocks.
        let uridecodebin = state.uridecodebin.clone();

        state.pending_current_items.push_back(Some(item));

        drop(state_guard);
        uridecodebin.set_property("uri", uri);

        Ok(())
    }

    fn update_current(&self, mut state_guard: MutexGuard<Option<State>>, current: Option<Item>) {
        let (uris_len, infinite) = {
            let settings = self.settings.lock().unwrap();

            (settings.uris.len(), settings.iterations == 0)
        };

        if let Some(state) = state_guard.as_mut() {
            state.current_item = current;

            if let Some(current) = state.current_item.as_ref() {
                let (mut current_iteration, current_uri_index) = (
                    (current.index() / uris_len) as u32,
                    (current.index() % uris_len) as u64,
                );

                if infinite {
                    current_iteration = 0;
                }

                let element = self.obj();
                let mut notify_iteration = false;
                let mut notify_index = false;

                if current_iteration != state.current_iteration {
                    state.current_iteration = current_iteration;
                    notify_iteration = true;
                }
                if current_uri_index != state.current_uri_index {
                    state.current_uri_index = current_uri_index;
                    notify_index = true;
                }

                // drop mutex before notifying changes as the callback will likely try to fetch the updated values
                // which would deadlock.
                drop(state_guard);

                if notify_iteration {
                    element.notify("current-iteration");
                }
                if notify_index {
                    element.notify("current-uri-index");
                }
            }
        }
    }

    fn failed(&self, error: PlaylistError) {
        let error_msg = error.to_string();
        gst::error!(CAT, imp = self, "{}", error_msg);

        match error {
            PlaylistError::PluginMissing { .. } => {
                gst::element_imp_error!(self, gst::CoreError::MissingPlugin, ["{}", &error_msg]);
            }
        }

        self.update_current(self.state.lock().unwrap(), None);
    }

    fn stop(&self) {
        // remove all children and pads
        let children = self.obj().children();
        let children_ref = children.iter().collect::<Vec<_>>();
        self.obj().remove_many(children_ref).unwrap();

        for pad in self.obj().src_pads() {
            self.obj().remove_pad(&pad).unwrap();
        }

        let mut state_guard = self.state.lock().unwrap();

        if let Some(state) = state_guard.as_ref() {
            for cached in state.cached_uris.values() {
                let _ = std::fs::remove_file(cached);
            }
        }

        *state_guard = None;
    }
}
