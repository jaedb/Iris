// Copyright (C) 2022 Rafael Caricio <rafael@caricio.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use crate::videocompare::hashed_image::HasherEngine;
use crate::videocompare::HashAlgorithm;
use crate::{PadDistance, VideoCompareMessage};
use gst::subclass::prelude::*;
use gst::{glib, glib::prelude::*, prelude::*};
use gst_base::prelude::*;
use gst_base::AggregatorPad;
use gst_video::prelude::*;
use gst_video::subclass::prelude::*;
use gst_video::subclass::AggregateFramesToken;
use gst_video::VideoFormat;
use std::sync::LazyLock;
use std::sync::{Arc, Mutex};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "videocompare",
        gst::DebugColorFlags::empty(),
        Some("Video frames comparison"),
    )
});

const DEFAULT_HASH_ALGO: HashAlgorithm = HashAlgorithm::Blockhash;
const DEFAULT_MAX_DISTANCE_THRESHOLD: f64 = 0.0;

struct Settings {
    hash_algo: HashAlgorithm,
    max_distance_threshold: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            hash_algo: DEFAULT_HASH_ALGO,
            max_distance_threshold: DEFAULT_MAX_DISTANCE_THRESHOLD,
        }
    }
}

struct State {
    hasher: HasherEngine,
}

impl Default for State {
    fn default() -> Self {
        Self {
            hasher: DEFAULT_HASH_ALGO.into(),
        }
    }
}

#[derive(Default)]
pub struct VideoCompare {
    settings: Arc<Mutex<Settings>>,
    state: Arc<Mutex<State>>,
    reference_pad: Mutex<Option<gst::Pad>>,
}

#[glib::object_subclass]
impl ObjectSubclass for VideoCompare {
    const NAME: &'static str = "GstVideoCompare";
    type Type = super::VideoCompare;
    type ParentType = gst_video::VideoAggregator;
}

impl ObjectImpl for VideoCompare {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecEnum::builder_with_default("hash-algo", DEFAULT_HASH_ALGO)
                    .nick("Hashing Algorithm")
                    .blurb("Which hashing algorithm to use for image comparisons")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecDouble::builder("max-dist-threshold")
                    .nick("Maximum Distance Threshold")
                    .blurb("Maximum distance threshold to emit messages when an image is detected, by default emits only on exact match")
                    .minimum(0f64)
                    .default_value(DEFAULT_MAX_DISTANCE_THRESHOLD)
                    .mutable_ready()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().unwrap();
        match pspec.name() {
            "hash-algo" => {
                let hash_algo = value.get().expect("type checked upstream");
                if settings.hash_algo != hash_algo {
                    gst::info!(
                        CAT,
                        imp = self,
                        "Changing hash-algo from {:?} to {:?}",
                        settings.hash_algo,
                        hash_algo
                    );
                    settings.hash_algo = hash_algo;

                    let mut state = self.state.lock().unwrap();
                    state.hasher = hash_algo.into();
                }
            }
            "max-dist-threshold" => {
                let max_distance_threshold = value.get().expect("type checked upstream");
                if settings.max_distance_threshold != max_distance_threshold {
                    gst::info!(
                        CAT,
                        imp = self,
                        "Changing max-dist-threshold from {} to {}",
                        settings.max_distance_threshold,
                        max_distance_threshold
                    );
                    settings.max_distance_threshold = max_distance_threshold;
                }
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().unwrap();
        match pspec.name() {
            "hash-algo" => settings.hash_algo.to_value(),
            "max-dist-threshold" => settings.max_distance_threshold.to_value(),
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for VideoCompare {}

impl ElementImpl for VideoCompare {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Image comparison",
                "Filter/Video",
                "Compare similarity of video frames",
                "Rafael Caricio <rafael@caricio.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst_video::VideoCapsBuilder::new()
                .format_list([VideoFormat::Rgb, VideoFormat::Rgba])
                .build();

            let sink_pad_template = gst::PadTemplate::with_gtype(
                "sink_%u",
                gst::PadDirection::Sink,
                gst::PadPresence::Request,
                &caps,
                gst_video::VideoAggregatorPad::static_type(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::with_gtype(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
                gst_video::VideoAggregatorPad::static_type(),
            )
            .unwrap();

            vec![sink_pad_template, src_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn release_pad(&self, pad: &gst::Pad) {
        let mut reference_pad = self.reference_pad.lock().unwrap();
        if let Some(current_reference_pad) = reference_pad.to_owned() {
            if pad != &current_reference_pad {
                // We don't worry if any other pads get released
                return;
            }

            // Since we are releasing the reference pad, we need to select a new pad for the
            // comparisons. At the moment we have no defined criteria to select the next
            // reference sink pad, so we choose the first that comes.
            for sink_pad in self.obj().sink_pads() {
                if current_reference_pad != sink_pad {
                    // Choose the first available left sink pad
                    *reference_pad = Some(sink_pad);
                }
            }
        }
    }
}

impl AggregatorImpl for VideoCompare {
    fn create_new_pad(
        &self,
        templ: &gst::PadTemplate,
        req_name: Option<&str>,
        caps: Option<&gst::Caps>,
    ) -> Option<AggregatorPad> {
        let pad = self.parent_create_new_pad(templ, req_name, caps);
        if let Some(pad) = &pad {
            let mut reference_pad = self.reference_pad.lock().unwrap();
            // We store the first pad added to the element for later use, this way we guarantee that
            // the first pad is used as the reference for comparisons
            if reference_pad.is_none() && pad.direction() == gst::PadDirection::Sink {
                let pad = pad.clone().upcast::<gst::Pad>();
                gst::info!(
                    CAT,
                    imp = self,
                    "Reference sink pad selected: {}",
                    pad.name()
                );
                *reference_pad = Some(pad);
            }
        }
        pad
    }

    fn update_src_caps(&self, caps: &gst::Caps) -> Result<gst::Caps, gst::FlowError> {
        let reference_pad = self.reference_pad.lock().unwrap();
        let sink_caps = reference_pad
            .as_ref()
            .and_then(|pad| pad.current_caps())
            .unwrap_or_else(|| caps.to_owned()); // Allow any caps for now

        if !sink_caps.can_intersect(caps) {
            gst::error!(
                CAT,
                imp = self,
                "Proposed src caps ({:?}) not supported, needs to intersect with the reference sink caps ({:?})",
                caps,
                sink_caps
            );
            return Err(gst::FlowError::NotNegotiated);
        }

        gst::info!(CAT, imp = self, "Caps for src pad: {:?}", sink_caps);
        Ok(sink_caps)
    }
}

impl VideoAggregatorImpl for VideoCompare {
    fn aggregate_frames(
        &self,
        token: &AggregateFramesToken,
        outbuf: &mut gst::BufferRef,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let state = self.state.lock().unwrap();

        let reference_pad = {
            let reference_pad = self.reference_pad.lock().unwrap();
            reference_pad
                .as_ref()
                .map(|pad| {
                    pad.clone()
                        .downcast::<gst_video::VideoAggregatorPad>()
                        .unwrap()
                })
                .ok_or_else(|| {
                    gst::warning!(CAT, imp = self, "No reference sink pad exists");
                    gst::FlowError::Eos
                })?
        };

        let reference_frame = match reference_pad.prepared_frame(token) {
            Some(f) => f,
            None => {
                return if reference_pad.is_eos() {
                    Err(gst::FlowError::Eos)
                } else {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "The reference sink pad '{}' has not produced a buffer, image comparison not possible",
                        reference_pad.name()
                    );
                    Ok(gst::FlowSuccess::Ok)
                }
            }
        };

        let mut message = VideoCompareMessage::default();
        let buffer = reference_frame.buffer();

        // Add running time to message when possible
        message.running_time = reference_pad
            .segment()
            .downcast::<gst::ClockTime>()
            .ok()
            .and_then(|segment| buffer.pts().map(|pts| segment.to_running_time(pts)))
            .flatten();

        // output the reference buffer
        outbuf.remove_all_memory();
        buffer
            .copy_into(outbuf, gst::BufferCopyFlags::all(), ..)
            .map_err(|_| gst::FlowError::Error)?;

        // Use current frame as the reference to the comparison
        let reference_hash = state.hasher.hash_image(&reference_frame)?;

        // Loop through all remaining sink pads and compare the latest available buffer
        for pad in self
            .obj()
            .sink_pads()
            .into_iter()
            .map(|pad| pad.downcast::<gst_video::VideoAggregatorPad>().unwrap())
            .collect::<Vec<_>>()
        {
            // Do not compare the reference pad with itself
            if pad == reference_pad {
                continue;
            }

            let frame = match pad.prepared_frame(token) {
                Some(f) => f,
                None => return Ok(gst::FlowSuccess::Ok),
            };

            // Make sure the sizes are the same
            if reference_frame.width() != frame.width()
                || reference_frame.height() != frame.height()
            {
                gst::error!(
                    CAT,
                    imp = self,
                    "Video streams do not have the same sizes (add videoscale and force the sizes to be equal on all sink pads)",
                );
                return Err(gst::FlowError::NotNegotiated);
            }

            // compare current frame with the reference and add to the results to the structure
            let frame_hash = state.hasher.hash_image(&frame)?;
            let distance = state.hasher.compare(&reference_hash, &frame_hash);
            message
                .pad_distances
                .push(PadDistance::new(pad.upcast(), distance));
        }

        let max_distance_threshold = {
            let settings = self.settings.lock().unwrap();
            settings.max_distance_threshold
        };

        if message
            .pad_distances
            .iter()
            .any(|p| p.distance <= max_distance_threshold)
        {
            gst::debug!(
                CAT,
                imp = self,
                "Image detected {}",
                message.running_time.unwrap().display()
            );
            let element = self.obj();
            let _ = element.post_message(
                gst::message::Element::builder(message.into())
                    .src(element.as_ref())
                    .build(),
            );
        } else {
            gst::debug!(
                CAT,
                imp = self,
                "Compared images and could not find any frame with distance lower than the threshold of {}: {:?}",
                max_distance_threshold,
                message
            );
        }

        Ok(gst::FlowSuccess::Ok)
    }
}
