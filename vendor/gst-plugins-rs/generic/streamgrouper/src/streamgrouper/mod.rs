// Copyright (C) 2024 Igalia S.L. <aboya@igalia.com>
// Copyright (C) 2024 Comcast <aboya@igalia.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;

mod imp;

/**
* SECTION:element-streamgrouper
*
* #streamgrouper takes any number of streams in the sinkpads and patches the STREAM_START
* events so that they all belong to the same group-id.
*
* This is useful for constructing simple pipelines where different sources push buffers
* into an element that contains a streamsynchronizer element, like playsink.
*
* Notice that because of this group-id merging, using streamgrouper is incompatible with
* gapless playback. However, this is not a problem, since streamgrouper is currently
* intended only for use cases in which only one stream group will be played.
*
* ## Example
*
* This is a simple pipeline where, because the audio and video streams come from
* unrelated source elements, they end up with different group-ids and therefore get stuck
* forever waiting inside the streamsynchronizer inside playsink, and never play:
*
* |[
* # Will get stuck! The streams from audiotestsrc and videotestsrc don't
* # share a group-id.
* gst-launch-1.0 \
*   playsink name=myplaysink \
*   audiotestsrc ! myplaysink.audio_sink \
*   videotestsrc ! myplaysink.video_sink
* ]|
*
* By adding streamgrouper to the pipeline, the streams are become part of the same group
* and playback is possible.
*
* |[
 gst-launch-1.0 \
   playsink name=myplaysink \
   streamgrouper name=grouper \
   audiotestsrc ! grouper.sink_0   grouper.src_0 ! myplaysink.audio_sink \
   videotestsrc ! grouper.sink_1   grouper.src_1 ! myplaysink.video_sink
* ]|
*
* Since: plugins-rs-0.14.0
*/

glib::wrapper! {
    pub struct StreamGrouper(ObjectSubclass<imp::StreamGrouper>)
        @extends
            gst::Element,
            gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "streamgrouper",
        gst::Rank::NONE,
        StreamGrouper::static_type(),
    )
}
