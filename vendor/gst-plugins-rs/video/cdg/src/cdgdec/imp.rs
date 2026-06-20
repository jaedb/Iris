// Copyright (C) 2019 Guillaume Desmottes <guillaume.desmottes@collabora.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use gst::glib;
use gst::subclass::prelude::*;
use gst_video::prelude::*;
use gst_video::subclass::prelude::*;
use image::GenericImageView;
use std::sync::LazyLock;
use std::sync::Mutex;

use crate::constants::{CDG_HEIGHT, CDG_WIDTH};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new("cdgdec", gst::DebugColorFlags::empty(), Some("CDG decoder"))
});

#[derive(Default)]
pub struct CdgDec {
    cdg_inter: Mutex<Box<cdg_renderer::CdgInterpreter>>,
    output_info: Mutex<Option<gst_video::VideoInfo>>,
}

#[glib::object_subclass]
impl ObjectSubclass for CdgDec {
    const NAME: &'static str = "GstCdgDec";
    type Type = super::CdgDec;
    type ParentType = gst_video::VideoDecoder;
}

impl ObjectImpl for CdgDec {}

impl GstObjectImpl for CdgDec {}

impl ElementImpl for CdgDec {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "CDG decoder",
                "Decoder/Video",
                "CDG decoder",
                "Guillaume Desmottes <guillaume.desmottes@collabora.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let sink_caps = gst::Caps::builder("video/x-cdg")
                .field("parsed", true)
                .build();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &sink_caps,
            )
            .unwrap();

            let src_caps = gst_video::VideoCapsBuilder::new()
                .format(gst_video::VideoFormat::Rgba)
                .width(CDG_WIDTH as i32)
                .height(CDG_HEIGHT as i32)
                .framerate((0, 1).into())
                .build();
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

impl VideoDecoderImpl for CdgDec {
    fn start(&self) -> Result<(), gst::ErrorMessage> {
        let mut out_info = self.output_info.lock().unwrap();
        *out_info = None;

        self.parent_start()
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        {
            let mut cdg_inter = self.cdg_inter.lock().unwrap();
            cdg_inter.reset(true);
        }
        self.parent_stop()
    }

    fn handle_frame(
        &self,
        mut frame: gst_video::VideoCodecFrame,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        {
            let mut out_info = self.output_info.lock().unwrap();
            if out_info.is_none() {
                let instance = self.obj();
                let output_state = instance.set_output_state(
                    gst_video::VideoFormat::Rgba,
                    CDG_WIDTH,
                    CDG_HEIGHT,
                    None,
                )?;

                instance.negotiate(output_state)?;

                let out_state = instance.output_state().unwrap();
                *out_info = Some(out_state.info().clone());
            }
        }

        let cmd = {
            let input = frame.input_buffer().unwrap();
            let map = input.map_readable().map_err(|_| {
                gst::element_imp_error!(
                    self,
                    gst::CoreError::Failed,
                    ["Failed to map input buffer readable"]
                );
                gst::FlowError::Error
            })?;
            let data = map.as_slice();

            cdg::decode_subchannel_cmd(data)
        };

        let cmd = match cmd {
            Some(cmd) => cmd,
            None => {
                // Not a CDG command
                self.obj().release_frame(frame);
                return Ok(gst::FlowSuccess::Ok);
            }
        };

        let mut cdg_inter = self.cdg_inter.lock().unwrap();
        cdg_inter.handle_cmd(cmd);

        self.obj().allocate_output_frame(&mut frame, None)?;
        {
            let output = frame.output_buffer_mut().unwrap();
            let info = self.output_info.lock().unwrap();

            let mut out_frame =
                gst_video::VideoFrameRef::from_buffer_ref_writable(output, info.as_ref().unwrap())
                    .map_err(|_| {
                        gst::element_imp_error!(
                            self,
                            gst::CoreError::Failed,
                            ["Failed to map output buffer writable"]
                        );
                        gst::FlowError::Error
                    })?;

            let out_stride = out_frame.plane_stride()[0] as usize;

            for (y, line) in out_frame
                .plane_data_mut(0)
                .unwrap()
                .chunks_exact_mut(out_stride)
                .take(CDG_HEIGHT as usize)
                .enumerate()
            {
                for (x, pixel) in line
                    .chunks_exact_mut(4)
                    .take(CDG_WIDTH as usize)
                    .enumerate()
                {
                    let p = cdg_inter.get_pixel(x as u32, y as u32);
                    pixel.copy_from_slice(&p.0);
                }
            }
        }

        gst::debug!(
            CAT,
            imp = self,
            "Finish frame pts={}",
            frame.pts().display()
        );

        self.obj().finish_frame(frame)
    }

    fn decide_allocation(
        &self,
        query: &mut gst::query::Allocation,
    ) -> Result<(), gst::LoggableError> {
        if query
            .find_allocation_meta::<gst_video::VideoMeta>()
            .is_some()
        {
            if let Some((Some(ref pool), _, _, _)) = query.allocation_pools().next() {
                let mut config = pool.config();
                config.add_option(gst_video::BUFFER_POOL_OPTION_VIDEO_META);
                pool.set_config(config)
                    .map_err(|_| gst::loggable_error!(CAT, "Failed to configure buffer pool"))?;
            }
        }

        self.parent_decide_allocation(query)
    }

    fn flush(&self) -> bool {
        gst::debug!(CAT, imp = self, "flushing, reset CDG interpreter");

        let mut cdg_inter = self.cdg_inter.lock().unwrap();
        cdg_inter.reset(false);
        true
    }
}
