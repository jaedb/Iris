// Copyright (C) 2021 Rafael Caricio <rafael@caricio.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use m3u8_rs::{MediaPlaylist, MediaPlaylistType, MediaSegment};
use std::io::Write;

/// An HLS playlist.
///
/// Controls the changes that needs to happen in the playlist as new segments are added. This
/// includes maintaining the number of segments present in the playlist and setting the final
/// state of the playlist.
#[derive(Debug, Clone)]
pub struct Playlist {
    inner: MediaPlaylist,
    playlist_index: u64,
    status: PlaylistRenderState,
    turn_vod: bool,
    is_cmaf: bool,
}

impl Playlist {
    pub fn new(playlist: MediaPlaylist, turn_vod: bool, is_cmaf: bool) -> Self {
        Self {
            inner: playlist,
            playlist_index: 0,
            status: PlaylistRenderState::Init,
            turn_vod,
            is_cmaf,
        }
    }

    /// Adds a new segment to the playlist.
    pub fn add_segment(&mut self, segment: MediaSegment) {
        self.start();
        self.inner.segments.push(segment);
    }

    /// Updates the playlist based on current state.
    ///
    /// The playlist will be updated based on it's type. The playlist status is set to started.
    /// When a playlist type is defined, the number of segments is updated to match the max
    /// playlist length value. The playlist index and current media sequence is also kept up
    /// to date.
    pub fn update_playlist_state(&mut self, max_playlist_length: usize) {
        if !self.is_type_undefined() {
            return;
        }

        // Remove oldest segments if playlist is at maximum expected capacity
        if max_playlist_length > 0 {
            if self.is_cmaf {
                // init segment uri will be specified only if it's updated
                // or in case of the very first segment.
                while self.inner.segments.len() > max_playlist_length {
                    let to_remove = self.inner.segments.remove(0);
                    if self.inner.segments[0].map.is_none() {
                        self.inner.segments[0].map.clone_from(&to_remove.map)
                    }
                }
            } else if self.inner.segments.len() > max_playlist_length {
                let remove_len = self.inner.segments.len() - max_playlist_length;
                self.inner.segments.drain(0..remove_len);
            }
        }

        self.playlist_index += 1;
        self.inner.media_sequence = self.playlist_index - self.inner.segments.len() as u64;
    }

    /// Sets the playlist to started state.
    fn start(&mut self) {
        self.status = PlaylistRenderState::Started;
        self.inner.end_list = false;
    }

    /// Sets the playlist to stopped state.
    pub fn stop(&mut self, write_endlist: bool) {
        self.inner.end_list = write_endlist;
        if self.turn_vod {
            self.inner.playlist_type = Some(MediaPlaylistType::Vod);
        }
    }

    /// Returns true if the playlist type is not specified.
    pub fn is_type_undefined(&self) -> bool {
        self.inner.playlist_type.is_none()
    }

    /// Returns true if the playlist internal status started.
    pub fn is_rendering(&self) -> bool {
        self.status == PlaylistRenderState::Started
    }

    /// Returns the number of segments in the playlist.
    pub fn len(&self) -> usize {
        self.inner.segments.len()
    }

    /// Writes the playlist in textual format to the provided `Write` reference.
    pub fn write_to<T: Write>(&self, w: &mut T) -> std::io::Result<()> {
        self.inner.write_to(w)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PlaylistRenderState {
    Init,
    Started,
}
