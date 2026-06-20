// Copyright (C) 2021 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use libwebp_sys as ffi;
use std::sync::LazyLock;

use std::sync::Mutex;

use std::marker::PhantomData;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rswebpdec",
        gst::DebugColorFlags::empty(),
        Some("Rust WebP decoder"),
    )
});

#[derive(Default)]
struct State {
    buffers: Vec<gst::Buffer>,
    total_size: usize,
}

struct Decoder<'a> {
    decoder: *mut ffi::WebPAnimDecoder,
    phantom: PhantomData<&'a [u8]>,
}

struct Frame<'a> {
    buf: &'a [u8],
    timestamp: i32,
}

struct Info {
    width: u32,
    height: u32,
    frame_count: u32,
}

impl Decoder<'_> {
    fn from_data(data: &[u8]) -> Option<Self> {
        unsafe {
            let mut options = std::mem::MaybeUninit::zeroed();
            if ffi::WebPAnimDecoderOptionsInit(options.as_mut_ptr()) == 0 {
                return None;
            }
            let mut options = options.assume_init();

            options.use_threads = 1;
            // TODO: negotiate this with downstream, bearing in mind that
            // we should be able to tell whether an image contains alpha
            // using WebPDemuxGetI
            options.color_mode = ffi::MODE_RGBA;

            let ptr = ffi::WebPAnimDecoderNew(
                &ffi::WebPData {
                    bytes: data.as_ptr(),
                    size: data.len(),
                },
                &options,
            );

            Some(Self {
                decoder: ptr,
                phantom: PhantomData,
            })
        }
    }

    fn has_more_frames(&self) -> bool {
        unsafe { ffi::WebPAnimDecoderHasMoreFrames(self.decoder) != 0 }
    }

    fn info(&self) -> Option<Info> {
        let mut info = std::mem::MaybeUninit::zeroed();
        unsafe {
            if ffi::WebPAnimDecoderGetInfo(self.decoder, info.as_mut_ptr()) == 0 {
                return None;
            }
            let info = info.assume_init();
            Some(Info {
                width: info.canvas_width,
                height: info.canvas_height,
                frame_count: info.frame_count,
            })
        }
    }

    fn next(&mut self) -> Option<Frame<'_>> {
        let mut buf = std::ptr::null_mut();
        let buf_ptr: *mut *mut u8 = &mut buf;
        let mut timestamp: i32 = 0;

        if let Some(info) = self.info() {
            unsafe {
                if ffi::WebPAnimDecoderGetNext(self.decoder, buf_ptr, &mut timestamp) == 0 {
                    return None;
                }

                assert!(!buf.is_null());

                Some(Frame {
                    buf: std::slice::from_raw_parts(buf, (info.width * info.height * 4) as usize),
                    timestamp,
                })
            }
        } else {
            None
        }
    }
}

impl Drop for Decoder<'_> {
    fn drop(&mut self) {
        unsafe { ffi::WebPAnimDecoderDelete(self.decoder) }
    }
}

pub struct WebPDec {
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
    state: Mutex<State>,
}

impl WebPDec {
    fn sink_chain(
        &self,
        pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(CAT, obj = pad, "Handling buffer {:?}", buffer);

        let mut state = self.state.lock().unwrap();

        state.total_size += buffer.size();
        state.buffers.push(buffer);

        Ok(gst::FlowSuccess::Ok)
    }

    fn decode(&self) -> Result<(), gst::ErrorMessage> {
        let mut prev_timestamp: Option<gst::ClockTime> = Some(gst::ClockTime::ZERO);
        let mut state = self.state.lock().unwrap();

        if state.buffers.is_empty() {
            return Err(gst::error_msg!(
                gst::StreamError::Decode,
                ["No valid frames decoded before end of stream"]
            ));
        }

        let mut buf = Vec::with_capacity(state.total_size);

        for buffer in state.buffers.drain(..) {
            buf.extend_from_slice(&buffer.map_readable().expect("Failed to map buffer"));
        }

        drop(state);

        let mut decoder = Decoder::from_data(&buf).ok_or_else(|| {
            gst::error_msg!(gst::StreamError::Decode, ["Failed to decode picture"])
        })?;

        let info = decoder.info().ok_or_else(|| {
            gst::error_msg!(gst::StreamError::Decode, ["Failed to get animation info"])
        })?;

        if info.frame_count == 0 {
            return Err(gst::error_msg!(
                gst::StreamError::Decode,
                ["No valid frames decoded before end of stream"]
            ));
        }

        let caps =
            gst_video::VideoInfo::builder(gst_video::VideoFormat::Rgba, info.width, info.height)
                .fps((0, 1))
                .build()
                .unwrap()
                .to_caps()
                .unwrap();

        // We push our own time segment, regardless of what the
        // input Segment may have contained. WebP is self-contained,
        // and its timestamps are our only time source
        let segment = gst::FormattedSegment::<gst::ClockTime>::new();

        let _ = self.srcpad.push_event(gst::event::Caps::new(&caps));
        let _ = self.srcpad.push_event(gst::event::Segment::new(&segment));

        while decoder.has_more_frames() {
            let frame = decoder.next().ok_or_else(|| {
                gst::error_msg!(gst::StreamError::Decode, ["Failed to get next frame"])
            })?;

            let timestamp = (frame.timestamp as u64).mseconds();
            let duration =
                prev_timestamp.and_then(|prev_timestamp| timestamp.checked_sub(prev_timestamp));

            let mut out_buf =
                gst::Buffer::with_size((info.width * info.height * 4) as usize).unwrap();
            {
                let out_buf_mut = out_buf.get_mut().unwrap();
                out_buf_mut.copy_from_slice(0, frame.buf).unwrap();
                out_buf_mut.set_pts(prev_timestamp);
                out_buf_mut.set_duration(duration);
            }

            prev_timestamp = Some(timestamp);

            match self.srcpad.push(out_buf) {
                Ok(_) => (),
                Err(gst::FlowError::Flushing) | Err(gst::FlowError::Eos) => break,
                Err(flow) => {
                    return Err(gst::error_msg!(
                        gst::StreamError::Failed,
                        ["Failed to push buffers: {:?}", flow]
                    ));
                }
            }
        }

        Ok(())
    }

    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::log!(CAT, obj = pad, "Handling event {:?}", event);
        match event.view() {
            EventView::FlushStop(..) => {
                let mut state = self.state.lock().unwrap();
                *state = State::default();
                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            EventView::Eos(..) => {
                if let Err(err) = self.decode() {
                    self.post_error_message(err);
                }
                gst::Pad::event_default(pad, Some(&*self.obj()), event)
            }
            EventView::Segment(..) => true,
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }

    fn src_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::log!(CAT, obj = pad, "Handling event {:?}", event);
        match event.view() {
            EventView::Seek(..) => false,
            _ => gst::Pad::event_default(pad, Some(&*self.obj()), event),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for WebPDec {
    const NAME: &'static str = "GstRsWebPDec";
    type Type = super::WebPDec;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                WebPDec::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |dec| dec.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                WebPDec::catch_panic_pad_function(
                    parent,
                    || false,
                    |dec| dec.sink_event(pad, event),
                )
            })
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::Pad::builder_from_template(&templ)
            .event_function(|pad, parent, event| {
                WebPDec::catch_panic_pad_function(parent, || false, |dec| dec.src_event(pad, event))
            })
            .build();

        Self {
            srcpad,
            sinkpad,
            state: Mutex::new(State::default()),
        }
    }
}

impl ObjectImpl for WebPDec {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }
}

impl GstObjectImpl for WebPDec {}

impl ElementImpl for WebPDec {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "WebP decoder",
                "Codec/Decoder/Video",
                "Decodes potentially animated WebP images",
                "Mathieu Duponchelle <mathieu@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::builder("image/webp").build();

            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let caps = gst_video::VideoCapsBuilder::new()
                .format(gst_video::VideoFormat::Rgba)
                .build();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst::trace!(CAT, imp = self, "Changing state {:?}", transition);

        if transition == gst::StateChange::PausedToReady {
            *self.state.lock().unwrap() = State::default();
        }

        self.parent_change_state(transition)
    }
}
