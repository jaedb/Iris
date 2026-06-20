// Copyright (C) 2020 Markus Ebner <info@ebner-markus.de>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use atomic_refcell::AtomicRefCell;
use gst::glib;
use gst::subclass::prelude::*;
use gst_video::prelude::*;
use gst_video::subclass::prelude::*;
use gst_video::VideoFormat;
use std::sync::LazyLock;
use std::{
    io,
    io::Write,
    sync::{Arc, Mutex},
};

const DEFAULT_REPEAT: i32 = 0;
const DEFAULT_SPEED: i32 = 10;

/// The gif::Encoder requires a std::io::Write implementation, to which it
/// can save the generated gif. This struct is used as a temporary cache, into
/// which the encoder can write encoded frames, such that we can read them back
/// and commit them to the gstreamer pipeline.
struct CacheBuffer {
    buffer: AtomicRefCell<Vec<u8>>,
}

impl CacheBuffer {
    pub fn new() -> Self {
        Self {
            buffer: AtomicRefCell::new(Vec::new()),
        }
    }
    pub fn clear(&self) {
        self.buffer.borrow_mut().clear();
    }
    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        let mut buffer = self.buffer.borrow_mut();
        buffer.write(buf)
    }
    pub fn consume(&self) -> Vec<u8> {
        let mut buffer = self.buffer.borrow_mut();
        std::mem::take(&mut *buffer)
    }
}

/// Writer for a CacheBuffer instance. This class is passed to the gif::Encoder.
/// Everything written to the CacheBufferWriter is stored in the underlying CacheBuffer.
struct CacheBufferWriter {
    cache_buffer: Arc<CacheBuffer>,
}
impl CacheBufferWriter {
    pub fn new(cache_buffer: Arc<CacheBuffer>) -> Self {
        Self { cache_buffer }
    }
}
impl Write for CacheBufferWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.cache_buffer.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct Settings {
    repeat: i32,
    speed: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            repeat: DEFAULT_REPEAT,
            speed: DEFAULT_SPEED,
        }
    }
}

struct State {
    video_info: gst_video::VideoInfo,
    cache: Arc<CacheBuffer>,
    gif_pts: Option<gst::ClockTime>,
    last_actual_pts: Option<gst::ClockTime>,
    context: Option<gif::Encoder<CacheBufferWriter>>,
}

impl State {
    pub fn new(video_info: gst_video::VideoInfo) -> Self {
        Self {
            video_info,
            cache: Arc::new(CacheBuffer::new()),
            gif_pts: None,
            last_actual_pts: None,
            context: None,
        }
    }

    /// Check if the essential encoding parameters (width, height, format) have changed
    /// compared to the current state. Returns true if a reset is needed.
    pub fn needs_reset(&self, new_video_info: &gst_video::VideoInfo) -> bool {
        self.video_info.width() != new_video_info.width()
            || self.video_info.height() != new_video_info.height()
            || self.video_info.format() != new_video_info.format()
    }
    pub fn reset(&mut self, settings: Settings) {
        self.cache.clear();
        self.gif_pts = None;
        self.last_actual_pts = None;
        // initialize and configure encoder with a CacheBufferWriter pointing
        // to our CacheBuffer instance
        let mut encoder = gif::Encoder::new(
            CacheBufferWriter::new(self.cache.clone()),
            self.video_info.width() as u16,
            self.video_info.height() as u16,
            &[],
        )
        .expect("Failed to initialize GIF encoder");
        match settings.repeat {
            -1 => encoder.set_repeat(gif::Repeat::Infinite),
            _ => encoder.set_repeat(gif::Repeat::Finite(settings.repeat as u16)),
        }
        .expect("Failed to configure encoder");
        self.context = Some(encoder);
    }
}

#[derive(Default)]
pub struct GifEnc {
    state: AtomicRefCell<Option<State>>,
    settings: Mutex<Settings>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new("gifenc", gst::DebugColorFlags::empty(), Some("GIF encoder"))
});

#[glib::object_subclass]
impl ObjectSubclass for GifEnc {
    const NAME: &'static str = "GstGifEnc";
    type Type = super::GifEnc;
    type ParentType = gst_video::VideoEncoder;
}

impl ObjectImpl for GifEnc {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecInt::builder("repeat")
                    .nick("Repeat")
                    .blurb("Repeat (-1 to loop forever, 0 .. n finite repetitions)")
                    .minimum(-1)
                    .maximum(u16::MAX as i32)
                    .default_value(DEFAULT_REPEAT)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecInt::builder("speed")
                    .nick("Speed")
                    .blurb("Speed (1 .. 30; higher value yields faster encoding)")
                    .minimum(1)
                    .maximum(30)
                    .default_value(DEFAULT_SPEED)
                    .mutable_ready()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "repeat" => {
                let mut settings = self.settings.lock().unwrap();
                settings.repeat = value.get().expect("type checked upstream");
            }
            "speed" => {
                let mut settings = self.settings.lock().unwrap();
                settings.speed = value.get().expect("type checked upstream");
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "repeat" => {
                let settings = self.settings.lock().unwrap();
                settings.repeat.to_value()
            }
            "speed" => {
                let settings = self.settings.lock().unwrap();
                settings.speed.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for GifEnc {}

impl ElementImpl for GifEnc {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "GIF encoder",
                "Encoder/Video",
                "GIF encoder",
                "Markus Ebner <info@ebner-markus.de>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let sink_caps = gst_video::VideoCapsBuilder::new()
                .format_list([VideoFormat::Rgb, VideoFormat::Rgba])
                // frame-delay timing in gif is a multiple of 10ms -> max 100fps
                .framerate_range(gst::Fraction::from(1)..gst::Fraction::from(100))
                .build();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &sink_caps,
            )
            .unwrap();

            let src_caps = gst::Caps::builder("image/gif").build();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &src_caps,
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl VideoEncoderImpl for GifEnc {
    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        *self.state.borrow_mut() = None;
        Ok(())
    }

    fn propose_allocation(
        &self,
        query: &mut gst::query::Allocation,
    ) -> Result<(), gst::LoggableError> {
        query.add_allocation_meta::<gst_video::VideoMeta>(None);
        self.parent_propose_allocation(query)
    }

    fn set_format(
        &self,
        state: &gst_video::VideoCodecState<'static, gst_video::video_codec_state::Readable>,
    ) -> Result<(), gst::LoggableError> {
        let video_info = state.info();
        gst::debug!(CAT, imp = self, "Setting format {:?}", video_info);

        // Check if we need to reset the encoder based on essential parameters
        let needs_reset = self
            .state
            .borrow()
            .as_ref()
            .is_none_or(|current_state| current_state.needs_reset(video_info));

        if needs_reset {
            gst::debug!(
                CAT,
                imp = self,
                "Essential format parameters changed, flushing and resetting encoder"
            );
            self.flush_encoder()
                .map_err(|_| gst::loggable_error!(CAT, "Failed to drain"))?;

            let mut new_state = State::new(video_info.clone());
            let settings = self.settings.lock().unwrap();
            new_state.reset(*settings);
            *self.state.borrow_mut() = Some(new_state);
        } else {
            gst::debug!(CAT, imp = self, "Only non-essential parameters changed (colorimetry, framerate, etc.), keeping encoder state");
            // Update the video_info in the existing state without resetting the encoder
            if let Some(ref mut current_state) = *self.state.borrow_mut() {
                current_state.video_info = video_info.clone();
            }
        }

        let instance = self.obj();
        let output_state = instance
            .set_output_state(gst::Caps::builder("image/gif").build(), Some(state))
            .map_err(|_| gst::loggable_error!(CAT, "Failed to set output state"))?;
        instance
            .negotiate(output_state)
            .map_err(|_| gst::loggable_error!(CAT, "Failed to negotiate"))?;

        self.parent_set_format(state)
    }

    fn finish(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        self.flush_encoder()
    }

    fn handle_frame(
        &self,
        mut frame: gst_video::VideoCodecFrame,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state_guard = self.state.borrow_mut();
        let state = state_guard.as_mut().ok_or(gst::FlowError::NotNegotiated)?;

        gst::debug!(
            CAT,
            imp = self,
            "Sending frame {}",
            frame.system_frame_number()
        );

        let input_buffer = frame.input_buffer().expect("frame without input buffer");

        {
            let in_frame =
                gst_video::VideoFrameRef::from_buffer_ref_readable(input_buffer, &state.video_info)
                    .map_err(|_| {
                        gst::element_imp_error!(
                            self,
                            gst::CoreError::Failed,
                            ["Failed to map output buffer readable"]
                        );
                        gst::FlowError::Error
                    })?;

            let frame_width = in_frame.info().width();
            let frame_height = in_frame.info().height();

            // Calculate delay to new frame by calculating the difference between the current actual
            // presentation timestamp of the last frame within the gif, and the pts of the new frame.
            // This results in variable frame delays in the gif - but an overall constant fps.
            let pts = in_frame.buffer().pts();
            state.last_actual_pts = pts;
            if state.gif_pts.is_none() {
                // First frame: use pts of first input frame as origin
                state.gif_pts = pts;
            }
            let pts = pts.ok_or_else(|| {
                gst::element_imp_error!(
                    self,
                    gst::CoreError::Failed,
                    ["No PTS set on input frame. Unable to calculate proper frame timing."]
                );
                gst::FlowError::Error
            })?;
            let frame_delay = pts
                .checked_sub(state.gif_pts.expect("checked above"))
                .ok_or_else(|| {
                    gst::element_imp_error!(
                        self,
                        gst::CoreError::Failed,
                        ["Input frame PTS is greater than gif_pts. Unable to calculate proper frame timing."]
                    );
                    gst::FlowError::Error
                })?;

            let settings = self.settings.lock().unwrap();

            let mut raw_frame = tightly_packed_framebuffer(&in_frame);
            let mut gif_frame = match in_frame.info().format() {
                gst_video::VideoFormat::Rgb => gif::Frame::from_rgb_speed(
                    frame_width as u16,
                    frame_height as u16,
                    &raw_frame,
                    settings.speed,
                ),
                gst_video::VideoFormat::Rgba => gif::Frame::from_rgba_speed(
                    frame_width as u16,
                    frame_height as u16,
                    &mut raw_frame,
                    settings.speed,
                ),
                _ => unreachable!(),
            };

            // apply encoding settings to frame (gif uses multiples of 10ms as frame_delay)
            // use float arithmetic with rounding for this calculation, since small stuttering
            // is probably less visible than the large stuttering when a complete 10ms have to
            // "catch up".
            gif_frame.delay = (frame_delay.mseconds() as f32 / 10.0).round() as u16;
            state.gif_pts = state
                .gif_pts
                .opt_add((gif_frame.delay as u64 * 10).mseconds());

            // encode new frame
            let context = state.context.as_mut().unwrap();
            if let Err(e) = context.write_frame(&gif_frame) {
                gst::element_imp_error!(self, gst::CoreError::Failed, ["{e}"]);
                return Err(gst::FlowError::Error);
            }
        }

        // The encoder directly outputs one frame for each input frame
        // Since the output is directly available, we can re-use the input frame
        // to push results to the pipeline
        let buffer = state.cache.consume();

        // Avoid keeping the state locked while calling finish_frame()
        drop(state_guard);

        let output_buffer = gst::Buffer::from_mut_slice(buffer);
        // Currently not using incremental frames -> every frame is a keyframe
        frame.set_flags(gst_video::VideoCodecFrameFlags::SYNC_POINT);
        frame.set_output_buffer(output_buffer);
        self.obj().finish_frame(frame)
    }
}

impl GifEnc {
    fn flush_encoder(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::debug!(CAT, imp = self, "Flushing");

        let trailer_buffer = self.state.borrow_mut().as_mut().and_then(|state| {
            // Drop encoder to flush and take flushed data (gif trailer)
            state.context = None;
            let buffer = state.cache.consume();
            // reset internal state
            let settings = self.settings.lock().unwrap();
            // manually produce a
            let mut trailer_buffer = gst::Buffer::from_mut_slice(buffer);
            {
                let trailer_buffer = trailer_buffer.get_mut().unwrap();
                trailer_buffer.set_pts(state.last_actual_pts);
            }

            // Initialize the encoder again, to be ready for a new round without format change
            let last_actual_pts = state.last_actual_pts;
            state.reset(*settings);
            if last_actual_pts.is_some() {
                // return the constructed buffer containing the gif trailer
                Some(trailer_buffer)
            } else {
                gst::debug!(
                    CAT,
                    imp = self,
                    "No actual frames encoded, skipping trailer"
                );
                None
            }
        });
        if let Some(trailer_buffer) = trailer_buffer {
            // manually push GIF trailer to the encoder's src pad
            self.obj().src_pad().push(trailer_buffer)?;
        }

        Ok(gst::FlowSuccess::Ok)
    }
}

/// Helper method that takes a gstreamer video-frame and copies it into a
/// tightly packed rgb(a) buffer, ready for consumption by the gif encoder.
fn tightly_packed_framebuffer(frame: &gst_video::VideoFrameRef<&gst::BufferRef>) -> Vec<u8> {
    assert_eq!(frame.n_planes(), 1); // RGB and RGBA are tightly packed
    let line_size = (frame.width() * frame.n_components()) as usize;
    let line_stride = frame.plane_stride()[0] as usize;
    let mut raw_frame: Vec<u8> = Vec::with_capacity(line_size * frame.info().height() as usize);

    // copy gstreamer frame to tightly packed rgb(a) frame.
    frame
        .plane_data(0)
        .unwrap()
        .chunks_exact(line_stride)
        .map(|padded_line| &padded_line[..line_size])
        .for_each(|line| raw_frame.extend_from_slice(line));

    raw_frame
}
