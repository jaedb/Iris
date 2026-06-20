// Copyright (C) 2021-2024 Guillaume Desmottes <guillaume@desmottes.be>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::sync::{Arc, LazyLock, Mutex};

use futures::future::{AbortHandle, Abortable};
use tokio::runtime;

use gst::glib;
use gst::subclass::prelude::*;
use gst_base::prelude::*;
use gst_base::subclass::{base_src::CreateSuccess, prelude::*};

use librespot_core::SpotifyId;
use librespot_metadata::lyrics;

use crate::common::SetupThread;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "spotifylyricssrc",
        gst::DebugColorFlags::empty(),
        Some("Spotify lyrics source"),
    )
});

static RUNTIME: LazyLock<runtime::Runtime> = LazyLock::new(|| {
    runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(1)
        .build()
        .unwrap()
});

#[derive(Default, Debug)]
struct Settings {
    common: crate::common::Settings,
    background_color: u32,
    highlight_text_color: u32,
    text_color: u32,
}

struct State {
    /// pending buffers, in reverse order so pop() produces the next one to push
    buffers: Vec<gst::Buffer>,
}

#[derive(Default)]
pub struct SpotifyLyricsSrc {
    setup_thread: Mutex<SetupThread>,
    state: Arc<Mutex<Option<State>>>,
    settings: Mutex<Settings>,
}

#[glib::object_subclass]
impl ObjectSubclass for SpotifyLyricsSrc {
    const NAME: &'static str = "GstSpotifyLyricsSrc";
    type Type = super::SpotifyLyricsSrc;
    type ParentType = gst_base::PushSrc;
}

impl ObjectImpl for SpotifyLyricsSrc {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            let mut props = crate::common::Settings::properties();

            props.push(
                glib::ParamSpecUInt::builder("background-color")
                    .nick("Background color")
                    .blurb("The background color of the lyrics, in ARGB")
                    .default_value(0)
                    .read_only()
                    .build(),
            );

            props.push(
                glib::ParamSpecUInt::builder("highlight-text-color")
                    .nick("Highlight Text color")
                    .blurb("The text color of the highlighted lyrics, in ARGB")
                    .default_value(0)
                    .read_only()
                    .build(),
            );

            props.push(
                glib::ParamSpecUInt::builder("text-color")
                    .nick("Text color")
                    .blurb("The text color of the lyrics, in ARGB")
                    .default_value(0)
                    .read_only()
                    .build(),
            );

            props
        });
        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().unwrap();
        settings.common.set_property(value, pspec);
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().unwrap();

        match pspec.name() {
            "background-color" => settings.background_color.to_value(),
            "highlight-text-color" => settings.highlight_text_color.to_value(),
            "text-color" => settings.text_color.to_value(),
            _ => settings.common.property(pspec),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        self.obj().set_format(gst::Format::Time);
    }
}

impl GstObjectImpl for SpotifyLyricsSrc {}

impl ElementImpl for SpotifyLyricsSrc {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Spotify lyrics source",
                "Source/Text",
                "Spotify lyrics source",
                "Guillaume Desmottes <guillaume@desmottes.be>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::builder("text/x-raw")
                .field("format", "utf8")
                .build();

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
}

impl BaseSrcImpl for SpotifyLyricsSrc {
    fn start(&self) -> Result<(), gst::ErrorMessage> {
        {
            let state = self.state.lock().unwrap();
            if state.is_some() {
                // already started
                return Ok(());
            }
        }

        {
            // If not started yet and not cancelled, start the setup
            let mut setup_thread = self.setup_thread.lock().unwrap();
            assert!(!matches!(&*setup_thread, SetupThread::Cancelled));
            if matches!(&*setup_thread, SetupThread::None) {
                self.start_setup(&mut setup_thread);
            }
        }

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        if let Some(_state) = self.state.lock().unwrap().take() {
            gst::debug!(CAT, imp = self, "stopping");
        }

        Ok(())
    }

    fn unlock(&self) -> Result<(), gst::ErrorMessage> {
        let mut setup_thread = self.setup_thread.lock().unwrap();
        setup_thread.abort();
        Ok(())
    }

    fn unlock_stop(&self) -> Result<(), gst::ErrorMessage> {
        let mut setup_thread = self.setup_thread.lock().unwrap();
        if matches!(&*setup_thread, SetupThread::Cancelled) {
            *setup_thread = SetupThread::None;
        }
        Ok(())
    }
}

impl PushSrcImpl for SpotifyLyricsSrc {
    fn create(
        &self,
        _buffer: Option<&mut gst::BufferRef>,
    ) -> Result<CreateSuccess, gst::FlowError> {
        let state_set = {
            let state = self.state.lock().unwrap();
            state.is_some()
        };

        if !state_set {
            // If not started yet and not cancelled, start the setup
            let mut setup_thread = self.setup_thread.lock().unwrap();
            if matches!(&*setup_thread, SetupThread::Cancelled) {
                return Err(gst::FlowError::Flushing);
            }

            if matches!(&*setup_thread, SetupThread::None) {
                self.start_setup(&mut setup_thread);
            }
        }

        {
            // wait for the setup to be completed
            let mut setup_thread = self.setup_thread.lock().unwrap();
            if let SetupThread::Pending {
                ref mut thread_handle,
                ..
            } = *setup_thread
            {
                let thread_handle = thread_handle.take().expect("Waiting multiple times");
                drop(setup_thread);
                let res = thread_handle.join().unwrap();

                match res {
                    Err(_aborted) => {
                        gst::debug!(CAT, imp = self, "setup has been cancelled");
                        setup_thread = self.setup_thread.lock().unwrap();
                        *setup_thread = SetupThread::Cancelled;
                        return Err(gst::FlowError::Flushing);
                    }
                    Ok(Err(err)) => {
                        gst::error!(CAT, imp = self, "failed to start: {err:?}");
                        gst::element_imp_error!(self, gst::ResourceError::Settings, ["{err:?}"]);
                        setup_thread = self.setup_thread.lock().unwrap();
                        *setup_thread = SetupThread::None;
                        return Err(gst::FlowError::Error);
                    }
                    Ok(Ok(_)) => {
                        setup_thread = self.setup_thread.lock().unwrap();
                        *setup_thread = SetupThread::Done;
                    }
                }
            }
        }

        let mut state = self.state.lock().unwrap();
        let state = state.as_mut().unwrap();

        match state.buffers.pop() {
            Some(buffer) => {
                gst::log!(CAT, imp = self, "created {:?}", buffer);
                Ok(CreateSuccess::NewBuffer(buffer))
            }
            None => {
                gst::debug!(CAT, imp = self, "eos");
                Err(gst::FlowError::Eos)
            }
        }
    }
}

impl SpotifyLyricsSrc {
    fn start_setup(&self, setup_thread: &mut SetupThread) {
        assert!(matches!(setup_thread, SetupThread::None));

        let self_ = self.to_owned();

        // run the runtime from another thread to prevent the "start a runtime from within a runtime" panic
        // when the plugin is statically linked.
        let (abort_handle, abort_registration) = AbortHandle::new_pair();
        let thread_handle = std::thread::spawn(move || {
            RUNTIME.block_on(async move {
                let future = Abortable::new(self_.setup(), abort_registration);
                future.await
            })
        });

        *setup_thread = SetupThread::Pending {
            thread_handle: Some(thread_handle),
            abort_handle,
        };
    }
    async fn setup(&self) -> anyhow::Result<()> {
        {
            let state = self.state.lock().unwrap();

            if state.is_some() {
                // already setup
                return Ok(());
            }
        }

        let src = self.obj();

        let (session, track_id) = {
            let common = {
                let settings = self.settings.lock().unwrap();
                settings.common.clone()
            };

            let session = common.connect_session(src.clone(), &CAT).await?;
            let track_id = SpotifyId::from_base62(&common.track_uri()?.to_id()?);

            (session, track_id)
        };

        let track_id_unpacked = track_id?;
        let reply = lyrics::Lyrics::get(&session, &track_id_unpacked)
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "failed to get lyrics for track {} ({})",
                    track_id_unpacked,
                    e
                )
            })?;

        gst::debug!(
            CAT,
            imp = self,
            "got lyrics for track {}",
            track_id_unpacked
        );

        match reply.lyrics.sync_type {
            lyrics::SyncType::Unsynced => {
                // lyrics are not synced so we can't generate timestamps
                anyhow::bail!("lyrics are not synced")
            }
            lyrics::SyncType::LineSynced => {}
        }

        let mut buffers = vec![];

        // create all pending buffers, in reverse order.
        let mut lines = reply.lyrics.lines.into_iter().rev();

        // The last line is always empty as it's meant to calculate the last actual line duration,
        // so we can safely ignore it.
        if let Some(last_line) = lines.next() {
            // ts of the next buffer in chronological order, so the previous one when iterating
            let mut next_ts = start_time(&last_line)?;

            for line in lines {
                let ts = start_time(&line)?;
                // skip consecutive empty lines
                if !line.words.is_empty() {
                    let txt = line.words;
                    let duration = next_ts - ts;

                    gst::trace!(CAT, imp = self, "{}: {} (duration: {})", ts, txt, duration);

                    let mut buffer = gst::Buffer::from_slice(txt);
                    {
                        let buffer = buffer.get_mut().unwrap();

                        buffer.set_pts(ts);
                        buffer.set_duration(duration);
                    }

                    buffers.push(buffer);
                    next_ts = ts;
                }
            }
        } else {
            // no lyrics
        }

        // update color properties
        {
            let mut settings = self.settings.lock().unwrap();
            settings.background_color = reply.colors.background as u32;
            settings.highlight_text_color = reply.colors.highlight_text as u32;
            settings.text_color = reply.colors.text as u32;
        }
        self.obj().notify("background-color");
        self.obj().notify("highlight-text-color");
        self.obj().notify("text-color");

        let mut state = self.state.lock().unwrap();
        state.replace(State { buffers });

        Ok(())
    }
}

fn start_time(line: &lyrics::Line) -> anyhow::Result<gst::ClockTime> {
    let ms = line.start_time_ms.parse()?;
    let ts = gst::ClockTime::from_mseconds(ms);
    Ok(ts)
}
