// SPDX-License-Identifier: MPL-2.0

use glib::prelude::*;
use glib::subclass::prelude::*;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_base::prelude::*;
use gst_base::subclass::prelude::*;

use std::sync::LazyLock;

use std::mem;
use std::sync::Mutex;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "ndisinkcombiner",
        gst::DebugColorFlags::empty(),
        Some("NewTek NDI sink audio/video combiner"),
    )
});

struct State {
    // Note that this applies to the currently pending buffer on the pad and *not*
    // to the current_video_buffer below!
    video_info: Option<gst_video::VideoInfo>,
    audio_info: Option<gst_audio::AudioInfo>,
    // These are only ever set when a change is pending mid-stream. They apply to the currently
    // pending buffer on the pad and not to the current_video_buffer.
    pending_caps: Option<gst::Caps>,
    pending_segment: Option<gst::Segment>,
    current_video_buffer: Option<(
        gst::Buffer,
        gst::ClockTime,
        Option<gst::Caps>,
        Option<gst::Segment>,
    )>,
    current_audio_buffers: Vec<(gst::Buffer, gst_audio::AudioInfo, i64)>,
}

pub struct NdiSinkCombiner {
    video_pad: gst_base::AggregatorPad,
    audio_pad: Mutex<Option<gst_base::AggregatorPad>>,
    state: Mutex<Option<State>>,
}

#[glib::object_subclass]
impl ObjectSubclass for NdiSinkCombiner {
    const NAME: &'static str = "GstNdiSinkCombiner";
    type Type = super::NdiSinkCombiner;
    type ParentType = gst_base::Aggregator;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("video").unwrap();
        let video_pad = gst::PadBuilder::<gst_base::AggregatorPad>::from_template(&templ).build();

        Self {
            video_pad,
            audio_pad: Mutex::new(None),
            state: Mutex::new(None),
        }
    }
}

impl ObjectImpl for NdiSinkCombiner {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.video_pad).unwrap();
    }
}

impl GstObjectImpl for NdiSinkCombiner {}

impl ElementImpl for NdiSinkCombiner {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "NewTek NDI Sink Combiner",
                "Combiner/Audio/Video",
                "NewTek NDI sink audio/video combiner",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst_video::VideoCapsBuilder::new()
                .format_list([
                    gst_video::VideoFormat::Uyvy,
                    gst_video::VideoFormat::I420,
                    gst_video::VideoFormat::Nv12,
                    gst_video::VideoFormat::Nv21,
                    gst_video::VideoFormat::Yv12,
                    gst_video::VideoFormat::Bgra,
                    gst_video::VideoFormat::Bgrx,
                    gst_video::VideoFormat::Rgba,
                    gst_video::VideoFormat::Rgbx,
                ])
                .framerate_range(gst::Fraction::new(1, i32::MAX)..gst::Fraction::new(i32::MAX, 1))
                .build();
            let src_pad_template = gst::PadTemplate::with_gtype(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
                gst_base::AggregatorPad::static_type(),
            )
            .unwrap();

            let video_sink_pad_template = gst::PadTemplate::with_gtype(
                "video",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
                gst_base::AggregatorPad::static_type(),
            )
            .unwrap();

            let caps = gst_audio::AudioCapsBuilder::new_interleaved()
                .format(gst_audio::AUDIO_FORMAT_F32)
                .rate_range(1..i32::MAX)
                .build();
            let audio_sink_pad_template = gst::PadTemplate::with_gtype(
                "audio",
                gst::PadDirection::Sink,
                gst::PadPresence::Request,
                &caps,
                gst_base::AggregatorPad::static_type(),
            )
            .unwrap();
            vec![
                src_pad_template,
                video_sink_pad_template,
                audio_sink_pad_template,
            ]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn release_pad(&self, pad: &gst::Pad) {
        let mut audio_pad_storage = self.audio_pad.lock().unwrap();

        if audio_pad_storage.as_ref().map(|p| p.upcast_ref()) == Some(pad) {
            gst::debug!(CAT, obj = pad, "Release audio pad");
            self.parent_release_pad(pad);
            *audio_pad_storage = None;
        }
    }
}

impl AggregatorImpl for NdiSinkCombiner {
    fn create_new_pad(
        &self,
        templ: &gst::PadTemplate,
        _req_name: Option<&str>,
        _caps: Option<&gst::Caps>,
    ) -> Option<gst_base::AggregatorPad> {
        let mut audio_pad_storage = self.audio_pad.lock().unwrap();

        if audio_pad_storage.is_some() {
            gst::error!(CAT, imp = self, "Audio pad already requested");
            return None;
        }

        let sink_templ = self.obj().pad_template("audio").unwrap();
        if templ != &sink_templ {
            gst::error!(CAT, imp = self, "Wrong pad template");
            return None;
        }

        let pad = gst::PadBuilder::<gst_base::AggregatorPad>::from_template(templ).build();
        *audio_pad_storage = Some(pad.clone());

        gst::debug!(CAT, imp = self, "Requested audio pad");

        Some(pad)
    }

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        let mut state_storage = self.state.lock().unwrap();
        *state_storage = Some(State {
            audio_info: None,
            video_info: None,
            pending_caps: None,
            pending_segment: None,
            current_video_buffer: None,
            current_audio_buffers: Vec::new(),
        });

        gst::debug!(CAT, imp = self, "Started");

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        // Drop our state now
        let _ = self.state.lock().unwrap().take();

        gst::debug!(CAT, imp = self, "Stopped");

        Ok(())
    }

    fn next_time(&self) -> Option<gst::ClockTime> {
        // FIXME: What to do here? We don't really know when the next buffer is expected
        gst::ClockTime::NONE
    }

    fn clip(
        &self,
        agg_pad: &gst_base::AggregatorPad,
        mut buffer: gst::Buffer,
    ) -> Option<gst::Buffer> {
        let segment = match agg_pad.segment().downcast::<gst::ClockTime>() {
            Ok(segment) => segment,
            Err(_) => {
                gst::error!(CAT, obj = agg_pad, "Only TIME segments supported");
                return Some(buffer);
            }
        };

        let pts = buffer.pts();
        if pts.is_none() {
            gst::error!(CAT, obj = agg_pad, "Only buffers with PTS supported");
            return Some(buffer);
        }

        let duration = buffer.duration();

        gst::trace!(
            CAT,
            obj = agg_pad,
            "Clipping buffer {:?} with PTS {} and duration {}",
            buffer,
            pts.display(),
            duration.display(),
        );

        let state_storage = self.state.lock().unwrap();
        let state = match &*state_storage {
            Some(ref state) => state,
            None => return None,
        };

        let duration = if duration.is_some() {
            duration
        } else if let Some(ref audio_info) = state.audio_info {
            gst::ClockTime::SECOND.mul_div_floor(
                buffer.size() as u64,
                audio_info.rate() as u64 * audio_info.bpf() as u64,
            )
        } else if let Some(ref video_info) = state.video_info {
            if video_info.fps().numer() > 0 {
                gst::ClockTime::SECOND.mul_div_floor(
                    video_info.fps().denom() as u64,
                    video_info.fps().numer() as u64,
                )
            } else {
                gst::ClockTime::NONE
            }
        } else {
            unreachable!()
        };

        gst::debug!(
            CAT,
            obj = agg_pad,
            "Clipping buffer {:?} with PTS {} and duration {}",
            buffer,
            pts.display(),
            duration.display(),
        );

        if agg_pad == &self.video_pad {
            let end_pts = pts
                .zip(duration)
                .and_then(|(pts, duration)| pts.checked_add(duration));

            segment.clip(pts, end_pts).map(|(start, stop)| {
                {
                    let buffer = buffer.make_mut();
                    buffer.set_pts(start);
                    buffer.set_duration(
                        stop.zip(start)
                            .and_then(|(stop, start)| stop.checked_sub(start)),
                    );
                }

                buffer
            })
        } else if let Some(ref audio_info) = state.audio_info {
            gst_audio::audio_buffer_clip(
                buffer,
                segment.upcast_ref(),
                audio_info.rate(),
                audio_info.bpf(),
            )
        } else {
            // Can't really have audio buffers without caps
            unreachable!();
        }
    }

    fn aggregate(&self, timeout: bool) -> Result<gst::FlowSuccess, gst::FlowError> {
        // FIXME: Can't really happen because we always return NONE from get_next_time() but that
        // should be improved!
        assert!(!timeout);

        // Because peek_buffer() can call into clip() and that would take the state lock again,
        // first try getting buffers from both pads here
        let video_buffer_and_segment = match self.video_pad.peek_buffer() {
            Some(video_buffer) => {
                let video_segment = self.video_pad.segment();
                let video_segment = match video_segment.downcast::<gst::ClockTime>() {
                    Ok(video_segment) => video_segment,
                    Err(video_segment) => {
                        gst::error!(
                            CAT,
                            imp = self,
                            "Video segment of wrong format {:?}",
                            video_segment.format()
                        );
                        return Err(gst::FlowError::Error);
                    }
                };

                Some((video_buffer, video_segment))
            }
            None if !self.video_pad.is_eos() => {
                gst::trace!(CAT, imp = self, "Waiting for video buffer");
                return Err(gst_base::AGGREGATOR_FLOW_NEED_DATA);
            }
            None => None,
        };

        let audio_buffer_segment_and_pad =
            if let Some(audio_pad) = self.audio_pad.lock().unwrap().clone() {
                match audio_pad.peek_buffer() {
                    Some(audio_buffer) if audio_buffer.size() == 0 => {
                        // Skip empty/gap audio buffer
                        audio_pad.drop_buffer();
                        gst::trace!(CAT, imp = self, "Empty audio buffer, waiting for next");
                        return Err(gst_base::AGGREGATOR_FLOW_NEED_DATA);
                    }
                    Some(audio_buffer) => {
                        let audio_segment = audio_pad.segment();
                        let audio_segment = match audio_segment.downcast::<gst::ClockTime>() {
                            Ok(audio_segment) => audio_segment,
                            Err(audio_segment) => {
                                gst::error!(
                                    CAT,
                                    imp = self,
                                    "Audio segment of wrong format {:?}",
                                    audio_segment.format()
                                );
                                return Err(gst::FlowError::Error);
                            }
                        };

                        Some((audio_buffer, audio_segment, audio_pad))
                    }
                    None if !audio_pad.is_eos() => {
                        gst::trace!(CAT, imp = self, "Waiting for audio buffer");
                        return Err(gst_base::AGGREGATOR_FLOW_NEED_DATA);
                    }
                    None => None,
                }
            } else {
                None
            };

        let mut state_storage = self.state.lock().unwrap();
        let state = match &mut *state_storage {
            Some(ref mut state) => state,
            None => return Err(gst::FlowError::Flushing),
        };

        let (
            mut current_video_buffer,
            current_video_running_time_end,
            pending_caps,
            pending_segment,
            next_video_buffer,
        ) = if let Some((video_buffer, video_segment)) = video_buffer_and_segment {
            let video_running_time = video_segment.to_running_time(video_buffer.pts()).unwrap();
            if let Some(pending_segment) = &state.pending_segment {
                assert_eq!(video_segment.upcast_ref(), pending_segment);
            }

            match &state.current_video_buffer {
                None => {
                    gst::trace!(CAT, imp = self, "First video buffer, waiting for second");
                    state.current_video_buffer = Some((
                        video_buffer,
                        video_running_time,
                        state.pending_caps.take(),
                        state.pending_segment.take(),
                    ));
                    drop(state_storage);
                    self.video_pad.drop_buffer();
                    return Err(gst_base::AGGREGATOR_FLOW_NEED_DATA);
                }
                Some((ref buffer, _, pending_caps, pending_segment)) => (
                    buffer.clone(),
                    Some(video_running_time),
                    pending_caps.clone(),
                    pending_segment.clone(),
                    Some((video_buffer, video_running_time)),
                ),
            }
        } else {
            match (&state.current_video_buffer, &audio_buffer_segment_and_pad) {
                (None, None) => {
                    gst::trace!(
                        CAT,
                        imp = self,
                        "All pads are EOS and no buffers are queued, finishing"
                    );
                    return Err(gst::FlowError::Eos);
                }
                (None, Some((ref audio_buffer, ref audio_segment, _))) => {
                    // Create an empty dummy buffer for attaching the audio. This is going to
                    // be dropped by the sink later.
                    let audio_running_time =
                        audio_segment.to_running_time(audio_buffer.pts()).unwrap();

                    let video_segment = self.video_pad.segment();
                    let video_segment = match video_segment.downcast::<gst::ClockTime>() {
                        Ok(video_segment) => video_segment,
                        Err(video_segment) => {
                            gst::error!(
                                CAT,
                                imp = self,
                                "Video segment of wrong format {:?}",
                                video_segment.format()
                            );
                            return Err(gst::FlowError::Error);
                        }
                    };
                    let video_pts = video_segment.position_from_running_time(audio_running_time);
                    if video_pts.is_none() {
                        gst::warning!(CAT, imp = self, "Can't output more audio after video EOS");
                        return Err(gst::FlowError::Eos);
                    }

                    let mut buffer = gst::Buffer::new();
                    {
                        let buffer = buffer.get_mut().unwrap();
                        buffer.set_pts(video_pts);
                    }

                    (buffer, gst::ClockTime::NONE, None, None, None)
                }
                (Some((ref buffer, _, _, _)), _) => {
                    (buffer.clone(), gst::ClockTime::NONE, None, None, None)
                }
            }
        };

        if let Some((audio_buffer, audio_segment, audio_pad)) = audio_buffer_segment_and_pad {
            let audio_info = match state.audio_info {
                Some(ref audio_info) => audio_info,
                None => {
                    gst::error!(CAT, imp = self, "Have no audio caps");
                    return Err(gst::FlowError::NotNegotiated);
                }
            };

            let audio_running_time = audio_segment.to_running_time(audio_buffer.pts());
            let duration = gst::ClockTime::SECOND.mul_div_floor(
                audio_buffer.size() as u64 / audio_info.bpf() as u64,
                audio_info.rate() as u64,
            );
            let audio_running_time_end = audio_running_time
                .zip(duration)
                .and_then(|(running_time, duration)| running_time.checked_add(duration));

            if audio_running_time_end
                .zip(current_video_running_time_end)
                .map(|(audio, video)| audio <= video)
                .unwrap_or(true)
            {
                let timecode = self
                    .obj()
                    .base_time()
                    .zip(audio_running_time)
                    .map(|(base_time, audio_running_time)| {
                        ((base_time.nseconds() + audio_running_time.nseconds()) / 100) as i64
                    })
                    .unwrap_or(crate::ndisys::NDIlib_send_timecode_synthesize);

                gst::trace!(
                    CAT,
                    imp = self,
                    "Including audio buffer {:?} with timecode {}: {} <= {}",
                    audio_buffer,
                    timecode,
                    audio_running_time_end.display(),
                    current_video_running_time_end.display(),
                );
                state
                    .current_audio_buffers
                    .push((audio_buffer, audio_info.clone(), timecode));
                audio_pad.drop_buffer();

                // If there is still video data, wait for the next audio buffer or EOS,
                // otherwise just output the dummy video buffer directly.
                if current_video_running_time_end.is_some() {
                    return Err(gst_base::AGGREGATOR_FLOW_NEED_DATA);
                }
            }

            // Otherwise finish this video buffer with all audio that has accumulated so
            // far
        }

        let audio_buffers = mem::take(&mut state.current_audio_buffers);

        if !audio_buffers.is_empty() {
            let current_video_buffer = current_video_buffer.make_mut();
            crate::ndisinkmeta::NdiSinkAudioMeta::add(current_video_buffer, audio_buffers);
        }

        if let Some((video_buffer, video_running_time)) = next_video_buffer {
            state.current_video_buffer = Some((video_buffer, video_running_time, None, None));
            drop(state_storage);
            self.video_pad.drop_buffer();
        } else {
            state.current_video_buffer = None;
            drop(state_storage);
        }

        gst::trace!(
            CAT,
            imp = self,
            "Finishing video buffer {:?}",
            current_video_buffer
        );
        if let Some(caps) = pending_caps {
            self.obj().set_src_caps(&caps);
        }
        if let Some(segment) = pending_segment {
            self.obj().update_segment(&segment);
        }
        let ret = self.obj().finish_buffer(current_video_buffer);
        ret
    }

    fn sink_event(&self, pad: &gst_base::AggregatorPad, event: gst::Event) -> bool {
        use gst::EventView;

        match event.view() {
            EventView::Caps(caps) => {
                let caps = caps.caps_owned();

                let mut state_storage = self.state.lock().unwrap();
                let state = match &mut *state_storage {
                    Some(ref mut state) => state,
                    None => return false,
                };

                if pad == &self.video_pad {
                    let mut send_caps_immediately = true;
                    let info = match gst_video::VideoInfo::from_caps(&caps) {
                        Ok(info) => info,
                        Err(_) => {
                            gst::error!(CAT, obj = pad, "Failed to parse caps {:?}", caps);
                            return false;
                        }
                    };

                    // 2 frames latency because we queue 1 frame and wait until audio
                    // up to the end of that frame has arrived.
                    let latency = if info.fps().numer() > 0 {
                        gst::ClockTime::SECOND
                            .mul_div_floor(2 * info.fps().denom() as u64, info.fps().numer() as u64)
                            .unwrap_or(80 * gst::ClockTime::MSECOND)
                    } else {
                        // let's assume 25fps and 2 frames latency
                        80 * gst::ClockTime::MSECOND
                    };

                    state.video_info = Some(info);
                    if state.current_video_buffer.is_some() {
                        state.pending_caps = Some(caps.clone());
                        send_caps_immediately = false;
                    } else {
                        state.pending_caps = None;
                    }

                    drop(state_storage);

                    self.obj().set_latency(latency, gst::ClockTime::NONE);

                    // The video caps are passed through as the audio is included only in a meta
                    if send_caps_immediately {
                        self.obj().set_src_caps(&caps);
                    }
                } else {
                    let info = match gst_audio::AudioInfo::from_caps(&caps) {
                        Ok(info) => info,
                        Err(_) => {
                            gst::error!(CAT, obj = pad, "Failed to parse caps {:?}", caps);
                            return false;
                        }
                    };

                    state.audio_info = Some(info);
                }
            }
            // The video segment is passed through as-is and the video timestamps are preserved
            EventView::Segment(segment) if pad == &self.video_pad => {
                let segment = segment.segment();
                gst::debug!(CAT, obj = pad, "Updating segment {:?}", segment);
                let mut state_storage = self.state.lock().unwrap();
                let state = match &mut *state_storage {
                    Some(ref mut state) => state,
                    None => return false,
                };
                let mut send_segment_immediately = true;
                if state.current_video_buffer.is_some() {
                    state.pending_segment = Some(segment.clone());
                    send_segment_immediately = false;
                } else {
                    state.pending_caps = None;
                }
                drop(state_storage);

                if send_segment_immediately {
                    self.obj().update_segment(segment);
                }
            }

            EventView::FlushStop(_) if pad == &self.video_pad => {
                let mut state_storage = self.state.lock().unwrap();
                let state = match &mut *state_storage {
                    Some(ref mut state) => state,
                    None => return false,
                };
                state.pending_segment = None;
                state.pending_caps = None;
            }
            _ => (),
        }

        self.parent_sink_event(pad, event)
    }

    fn sink_query(&self, pad: &gst_base::AggregatorPad, query: &mut gst::QueryRef) -> bool {
        use gst::QueryViewMut;

        match query.view_mut() {
            QueryViewMut::Caps(_) if pad == &self.video_pad => {
                // Directly forward caps queries
                let srcpad = self.obj().static_pad("src").unwrap();
                return srcpad.peer_query(query);
            }
            _ => (),
        }

        self.parent_sink_query(pad, query)
    }

    fn negotiate(&self) -> bool {
        // No negotiation needed as the video caps are just passed through
        true
    }
}
