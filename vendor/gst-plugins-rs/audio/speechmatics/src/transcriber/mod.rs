// Copyright (C) 2024 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-speechmaticstranscriber
 *
 * `speechmaticstranscriber` is an element that uses the [speechmatics
 * realtime API](https://docs.speechmatics.com/api-ref/realtime-transcription-websocket)
 * to transcribe audio speech into text.
 *
 * The element can work with both live and non-live data.
 *
 * With non-live data, the speechmatics endpoint can process audio data much faster
 * than real time. This makes the element useful for quickly transcribing large audio
 * samples, but can also use up credit at a similar rate. It is thus advisable to exercise
 * restraint when using the element with non-live data for the sole purpose of testing it.
 *
 * With live data, it is up to the user to determine the appropriate values for both the
 * `latency` property (which is the latency the element will advertise to the pipeline), and
 * optionally for the `max_delay` property, which is a value that can be passed to the speechmatics
 * API to request a latency from it.
 *
 * In practice, the speechmatics API seems to take this value into account, but the actual observed
 * delay from the sending of an audio sample to the reception of the matching text items may often
 * exceed it.
 *
 * It is important for that observed delay to remain lesser than the latency that was selected.
 *
 * For this reason the element tracks a `max-observed-delay`, available as a readonly, notified
 * property. The user can then monitor the property to tune the latency property as desired.
 *
 * In addition, a warning message will be posted by the element whenever the maximum observed delay
 * is greater than the latency that was selected.
 *
 * Note that application users can also use the `lateness` property in combination with or instead
 * of the latency property. Setting this property will result in the running times of the output items
 * being shifted forward, thus desynchronizing them with the audio by a fixed offset. This is
 * useful when loss of synchronization is an acceptable tradeoff for a decreased latency.
 *
 * Finally, the element will push gaps at regular intervals as empty transcripts are received
 * from the speechmatics API. In practice the average duration of such gaps has been observed to
 * be roughly two seconds, this can serve as a basis for sizing queues on parallel branches when
 * applicable.
 */
use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct Transcriber(ObjectSubclass<imp::Transcriber>) @extends gst::Element, gst::Object, @implements gst::ChildProxy;
}

glib::wrapper! {
    pub struct TranscriberSrcPad(ObjectSubclass<imp::TranscriberSrcPad>) @extends gst::Pad, gst::Object;
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstSpeechmaticsTranscriberDiarization")]
#[non_exhaustive]
pub enum SpeechmaticsTranscriberDiarization {
    #[enum_value(name = "None: no diarization", nick = "none")]
    None = 0,
    #[enum_value(name = "Speaker: identify speakers by their voices", nick = "speaker")]
    Speaker = 1,
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    {
        TranscriberSrcPad::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
        SpeechmaticsTranscriberDiarization::static_type()
            .mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }
    gst::Element::register(
        Some(plugin),
        "speechmaticstranscriber",
        gst::Rank::NONE,
        Transcriber::static_type(),
    )
}
