// Copyright (C) 2022 Rafael Caricio <rafael@caricio.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0
/**
 * element-videocompare:
 * @short_description: Compares multiple video feeds and post message to the application when frames are similar.
 *
 * Compare multiple video pad buffers, with the first added pad being the reference. All video
 * streams must use the same geometry. A message is posted to the application when the measurement
 * of distance between frames falls under the desired threshold.
 *
 * The application can decide what to do next with the comparison result. This element can be used
 * to detect badges or slates in the video stream.
 *
 * The src pad passthrough buffers from the reference sink pad.
 *
 * ## Example pipeline
 * ```bash
 * gst-launch-1.0 videotestsrc pattern=red ! videocompare name=compare ! videoconvert \
 *   ! autovideosink  videotestsrc pattern=red ! imagefreeze ! compare. -m
 * ```
 *
 * The message posted to the application contains a structure that looks like:
 *
 * ```ignore
 * videocompare,
 *   pad-distances=(structure)< "pad-distance\,\ pad\=\(GstPad\)\"\\\(GstVideoAggregatorPad\\\)\\\ sink_1\"\,\ distance\=\(double\)0\;" >,
 *   running-time=(guint64)0;
 * ```
 *
 * Since: plugins-rs-0.9.0
 */
use gst::glib;
use gst::prelude::*;
use std::fmt::Debug;

mod hashed_image;
mod imp;

glib::wrapper! {
    pub struct VideoCompare(ObjectSubclass<imp::VideoCompare>) @extends gst_video::VideoAggregator, gst_base::Aggregator, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "videocompare",
        gst::Rank::NONE,
        VideoCompare::static_type(),
    )
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstVideoCompareHashAlgorithm")]
#[non_exhaustive]
pub enum HashAlgorithm {
    #[enum_value(name = "Mean: The Mean hashing algorithm.", nick = "mean")]
    Mean = 0,

    #[enum_value(name = "Gradient: The Gradient hashing algorithm.", nick = "gradient")]
    Gradient = 1,

    #[enum_value(
        name = "VertGradient: The Vertical-Gradient hashing algorithm.",
        nick = "vertgradient"
    )]
    VertGradient = 2,

    #[enum_value(
        name = "DoubleGradient: The Double-Gradient hashing algorithm.",
        nick = "doublegradient"
    )]
    DoubleGradient = 3,

    #[enum_value(
        name = "Blockhash: The Blockhash (block median value perceptual hash) algorithm.",
        nick = "blockhash"
    )]
    Blockhash = 4,

    #[cfg(feature = "dssim")]
    #[enum_value(
        name = "Dssim: Image similarity comparison simulating human perception.",
        nick = "dssim"
    )]
    Dssim = 5,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct VideoCompareMessage {
    pad_distances: Vec<PadDistance>,
    running_time: Option<gst::ClockTime>,
}

impl VideoCompareMessage {
    pub fn pad_distances(&self) -> &[PadDistance] {
        self.pad_distances.as_slice()
    }

    pub fn running_time(&self) -> Option<gst::ClockTime> {
        self.running_time
    }
}

impl From<VideoCompareMessage> for gst::Structure {
    fn from(msg: VideoCompareMessage) -> Self {
        gst::Structure::builder("videocompare")
            .field(
                "pad-distances",
                gst::Array::new(msg.pad_distances.into_iter().map(|v| {
                    let s: gst::Structure = v.into();
                    s.to_send_value()
                })),
            )
            .field("running-time", msg.running_time)
            .build()
    }
}

impl TryFrom<gst::Structure> for VideoCompareMessage {
    type Error = Box<dyn std::error::Error>;

    fn try_from(structure: gst::Structure) -> Result<Self, Self::Error> {
        let mut pad_distances = Vec::<PadDistance>::new();
        for value in structure.get::<gst::Array>("pad-distances")?.iter() {
            let s = value.get::<gst::Structure>()?;
            pad_distances.push(s.try_into()?)
        }

        Ok(VideoCompareMessage {
            running_time: structure.get("running-time")?,
            pad_distances,
        })
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct PadDistance {
    pad: gst::Pad,
    distance: f64,
}

impl PadDistance {
    pub fn new(pad: gst::Pad, distance: f64) -> Self {
        Self { pad, distance }
    }

    pub fn pad(&self) -> &gst::Pad {
        &self.pad
    }

    pub fn distance(&self) -> f64 {
        self.distance
    }
}

impl From<PadDistance> for gst::Structure {
    fn from(pad_distance: PadDistance) -> Self {
        gst::Structure::builder("pad-distance")
            .field("pad", pad_distance.pad)
            .field("distance", pad_distance.distance)
            .build()
    }
}

impl TryFrom<gst::Structure> for PadDistance {
    type Error = Box<dyn std::error::Error>;

    fn try_from(structure: gst::Structure) -> Result<Self, Self::Error> {
        Ok(PadDistance {
            pad: structure.get("pad")?,
            distance: structure.get("distance")?,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn init() {
        use std::sync::Once;
        static INIT: Once = Once::new();

        INIT.call_once(|| {
            gst::init().unwrap();
        });
    }

    #[test]
    fn reverse_order_serialization_of_structure() {
        init();

        let running_time = gst::ClockTime::from_seconds(2);

        let mut message = VideoCompareMessage::default();
        message.pad_distances.push(PadDistance {
            pad: gst::Pad::builder(gst::PadDirection::Sink)
                .name("sink_0")
                .build(),
            distance: 42_f64,
        });
        message.running_time = Some(running_time);

        let structure: gst::Structure = message.into();

        let pad_distances = structure.get::<gst::Array>("pad-distances").unwrap();
        let first = pad_distances
            .first()
            .unwrap()
            .get::<gst::Structure>()
            .unwrap();
        assert_eq!(first.get::<gst::Pad>("pad").unwrap().name(), "sink_0");
        assert_eq!(first.get::<f64>("distance").unwrap(), 42_f64);

        assert_eq!(
            structure
                .get_optional::<gst::ClockTime>("running-time")
                .unwrap(),
            Some(running_time)
        );
    }

    #[test]
    fn from_structure_to_message_struct() {
        init();

        let running_time = gst::ClockTime::from_seconds(2);

        let structure = gst::Structure::builder("videocompare")
            .field(
                "pad-distances",
                gst::Array::from_iter([gst::Structure::builder("pad-distance")
                    .field(
                        "pad",
                        gst::Pad::builder(gst::PadDirection::Sink)
                            .name("sink_0")
                            .build(),
                    )
                    .field("distance", 42f64)
                    .build()
                    .to_send_value()]),
            )
            .field("running-time", running_time)
            .build();

        let message: VideoCompareMessage = structure.try_into().unwrap();
        assert_eq!(message.running_time, Some(running_time));

        let pad_distance = message.pad_distances.first().unwrap();
        assert_eq!(pad_distance.pad.name().as_str(), "sink_0");
        assert_eq!(pad_distance.distance, 42f64);
    }
}
