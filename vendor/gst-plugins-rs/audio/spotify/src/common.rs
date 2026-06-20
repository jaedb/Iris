// Copyright (C) 2021 Guillaume Desmottes <guillaume@desmottes.be>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use anyhow::bail;

use gst::glib;
use gst::prelude::*;

use futures::future::{AbortHandle, Aborted};
use librespot_core::{
    authentication::Credentials, cache::Cache, config::SessionConfig, session::Session, SpotifyUri, version,
};

#[derive(Default, Debug, Clone)]
pub struct Settings {
    access_token: String,
    cache_credentials: String,
    cache_files: String,
    cache_max_size: u64,
    pub track: String,
}

impl Settings {
    pub fn properties() -> Vec<glib::ParamSpec> {
        vec![
            glib::ParamSpecString::builder("access-token")
                .nick("Access token")
                .blurb("Spotify access token, requires 'streaming' scope")
                .default_value(Some(""))
                .mutable_ready()
                .build(),
            glib::ParamSpecString::builder("cache-credentials")
                .nick("Credentials cache")
                .blurb("Directory where to cache Spotify credentials")
                .default_value(Some(""))
                .mutable_ready()
                .build(),
            glib::ParamSpecString::builder("cache-files")
                .nick("Files cache")
                .blurb("Directory where to cache downloaded files from Spotify")
                .default_value(Some(""))
                .mutable_ready()
                .build(),
            glib::ParamSpecUInt64::builder("cache-max-size")
                .nick("Cache max size")
                .blurb(
                    "The max allowed size of the cache, in bytes, or 0 to disable the cache limit",
                )
                .default_value(0)
                .mutable_ready()
                .build(),
            glib::ParamSpecString::builder("track")
                .nick("Spotify URI")
                .blurb("Spotify track URI, in the form 'spotify:track:$SPOTIFY_ID'")
                .default_value(Some(""))
                .mutable_ready()
                .build(),
        ]
    }

    pub fn set_property(&mut self, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "access-token" => {
                self.access_token = value.get().expect("type checked upstream");
            }
            "cache-credentials" => {
                self.cache_credentials = value.get().expect("type checked upstream");
            }
            "cache-files" => {
                self.cache_files = value.get().expect("type checked upstream");
            }
            "cache-max-size" => {
                self.cache_max_size = value.get().expect("type checked upstream");
            }
            "track" => {
                self.track = value.get().expect("type checked upstream");
            }
            _ => unimplemented!(),
        }
    }

    pub fn property(&self, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "access-token" => self.access_token.to_value(),
            "cache-credentials" => self.cache_credentials.to_value(),
            "cache-files" => self.cache_files.to_value(),
            "cache-max-size" => self.cache_max_size.to_value(),
            "track" => self.track.to_value(),
            _ => unimplemented!(),
        }
    }

    pub async fn connect_session<T>(
        &self,
        src: T,
        cat: &gst::DebugCategory,
    ) -> anyhow::Result<Session>
    where
        T: IsA<glib::Object>,
    {
        let credentials_cache = if self.cache_credentials.is_empty() {
            None
        } else {
            Some(&self.cache_credentials)
        };

        let files_cache = if self.cache_files.is_empty() {
            None
        } else {
            Some(&self.cache_files)
        };

        let max_size = if self.cache_max_size != 0 {
            Some(self.cache_max_size)
        } else {
            None
        };

        gst::info!(
            cat,
            obj = &src,
            "Using librespot {} ({})",
            version::SEMVER,
            version::SHA_SHORT
        );

        let cache = Cache::new(credentials_cache, None, files_cache, max_size)?;

        if let Some(cached_cred) = cache.credentials() {
            let cached_username = cached_cred
                .username
                .as_ref()
                .map_or("UNKNOWN", |s| s.as_str());
            gst::debug!(
                cat,
                obj = &src,
                "reuse cached credentials for user {}",
                cached_username
            );

            let session = Session::new(SessionConfig::default(), Some(cache));
            session.connect(cached_cred, true).await?;
            return Ok(session);
        }

        gst::debug!(
            cat,
            obj = &src,
            "credentials not in cache or cached credentials invalid",
        );

        if self.access_token.is_empty() {
            bail!("access-token is not set and credentials are not in cache");
        }

        let cred = Credentials::with_access_token(&self.access_token);

        let session = Session::new(SessionConfig::default(), Some(cache));
        session.connect(cred, true).await?;
        
        Ok(session)
    }

    pub fn track_uri(&self) -> anyhow::Result<SpotifyUri> {
        if self.track.is_empty() {
            bail!("track is not set");
        }
        let track = SpotifyUri::from_uri(&self.track)?;

        Ok(track)
    }
}

#[derive(Default)]
pub enum SetupThread {
    #[default]
    None,
    Pending {
        thread_handle: Option<std::thread::JoinHandle<Result<anyhow::Result<()>, Aborted>>>,
        abort_handle: AbortHandle,
    },
    Cancelled,
    Done,
}

impl SetupThread {
    pub fn abort(&mut self) {
        // Cancel setup thread if it is pending and not done yet
        if matches!(self, SetupThread::None | SetupThread::Done) {
            return;
        }

        if let SetupThread::Pending {
            ref abort_handle, ..
        } = *self
        {
            abort_handle.abort();
        }
        *self = SetupThread::Cancelled;
    }
}
