//
// Copyright (C) 2023 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-rtpvp9pay2
 * @see_also: rtpvp9depay2, vp9dec, vp9enc
 *
 * Payload a VP9 video stream into RTP packets as per [draft-ietf-payload-vp9][draft-ietf-payload-vp9].
 *
 * [draft-ietf-payload-vp9]:https://datatracker.ietf.org/doc/html/draft-ietf-payload-vp9-16#section-4
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 videotestsrc ! video/x-raw,width=1280,height=720,format=I420 ! timeoverlay font-desc=Sans,22 ! vp9enc ! rtpvp9pay2 ! udpsink host=127.0.0.1 port=5004
 * ]| This will create and payload a VP9 video stream with a test pattern and
 * send it out via UDP to localhost port 5004.
 *
 * Since: plugins-rs-0.13.0
 */
use gst::{glib, prelude::*, subclass::prelude::*};
use smallvec::SmallVec;
use std::{cmp, sync::Mutex};

use bitstream_io::{BigEndian, BitRead as _, BitReader, ByteWrite as _, ByteWriter};
use std::sync::LazyLock;

use crate::{
    basepay::{RtpBasePay2Ext, RtpBasePay2ImplExt},
    vp9::{
        frame_header::FrameHeader,
        payload_descriptor::{PayloadDescriptor, PictureId},
    },
};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpvp9pay2",
        gst::DebugColorFlags::empty(),
        Some("RTP VP9 Payloader"),
    )
});

#[derive(Clone, Default)]
struct Settings {
    picture_id_mode: super::PictureIdMode,
    picture_id_offset: Option<u16>,
}

#[derive(Default)]
pub struct RtpVp9Pay {
    settings: Mutex<Settings>,
    /// Current picture ID.
    ///
    /// Reset to `None` in `Null` / `Ready` state and initialized to the offset when going to
    /// `Paused`.
    picture_id: Mutex<Option<PictureId>>,
}

#[glib::object_subclass]
impl ObjectSubclass for RtpVp9Pay {
    const NAME: &'static str = "GstRtpVp9Pay2";
    type Type = super::RtpVp9Pay;
    type ParentType = crate::basepay::RtpBasePay2;
}

impl ObjectImpl for RtpVp9Pay {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecEnum::builder::<super::PictureIdMode>("picture-id-mode")
                    .nick("Picture ID Mode")
                    .blurb("The picture ID mode for payloading")
                    .default_value(Settings::default().picture_id_mode)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecInt::builder("picture-id-offset")
                    .nick("Picture ID Offset")
                    .blurb("Offset to add to the initial picture-id (-1 = random)")
                    .default_value(
                        Settings::default()
                            .picture_id_offset
                            .map(i32::from)
                            .unwrap_or(-1),
                    )
                    .minimum(-1)
                    .maximum(0x7fff)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecInt::builder("picture-id")
                    .nick("Picture ID")
                    .blurb("Current Picture ID")
                    .default_value(-1)
                    .minimum(-1)
                    .maximum(0x7fff)
                    .read_only()
                    .build(),
            ]
        });

        &PROPERTIES
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "picture-id-mode" => {
                self.settings.lock().unwrap().picture_id_mode = value.get().unwrap();
            }
            "picture-id-offset" => {
                let v = value.get::<i32>().unwrap();
                self.settings.lock().unwrap().picture_id_offset =
                    (v != -1).then_some((v & 0x7fff) as u16);
            }
            _ => unimplemented!(),
        };
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "picture-id-mode" => self.settings.lock().unwrap().picture_id_mode.to_value(),
            "picture-id-offset" => self
                .settings
                .lock()
                .unwrap()
                .picture_id_offset
                .map(i32::from)
                .unwrap_or(-1)
                .to_value(),
            "picture-id" => {
                let picture_id = self.picture_id.lock().unwrap();
                picture_id
                    .map(u16::from)
                    .map(i32::from)
                    .unwrap_or(-1)
                    .to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for RtpVp9Pay {}

impl ElementImpl for RtpVp9Pay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP VP9 payloader",
                "Codec/Payloader/Network/RTP",
                "Payload VP9 as RTP packets",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &gst::Caps::builder("video/x-vp9").build(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("application/x-rtp")
                    .field("media", "video")
                    .field("clock-rate", 90_000i32)
                    .field(
                        "encoding-name",
                        gst::List::new(["VP9", "VP9-DRAFT-IETF-01"]),
                    )
                    .build(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl crate::basepay::RtpBasePay2Impl for RtpVp9Pay {
    const ALLOWED_META_TAGS: &'static [&'static str] = &["video"];

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        let settings = self.settings.lock().unwrap().clone();

        let picture_id_offset = settings.picture_id_offset.unwrap_or_else(|| {
            use rand::Rng as _;

            let mut rng = rand::rng();
            rng.random::<u16>()
        });

        let picture_id = PictureId::new(settings.picture_id_mode, picture_id_offset);
        *self.picture_id.lock().unwrap() = picture_id;

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        *self.picture_id.lock().unwrap() = None;

        Ok(())
    }

    fn set_sink_caps(&self, caps: &gst::Caps) -> bool {
        gst::debug!(CAT, imp = self, "received caps {caps:?}");

        let caps_builder = gst::Caps::builder("application/x-rtp")
            .field("media", "video")
            .field("clock-rate", 90_000i32)
            .field(
                "encoding-name",
                gst::List::new(["VP9", "VP9-DRAFT-IETF-01"]),
            );

        self.obj().set_src_caps(&caps_builder.build());

        true
    }

    fn negotiate(&self, mut src_caps: gst::Caps) {
        // Fixate the encoding-name with preference to "VP9"

        src_caps.truncate();
        {
            let src_caps = src_caps.get_mut().unwrap();
            let s = src_caps.structure_mut(0).unwrap();
            s.fixate_field_str("encoding-name", "VP9");
        }

        self.parent_negotiate(src_caps);
    }

    fn handle_buffer(
        &self,
        buffer: &gst::Buffer,
        id: u64,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let max_payload_size = self.obj().max_payload_size();

        gst::trace!(CAT, imp = self, "received buffer of size {}", buffer.size());

        let map = buffer.map_readable().map_err(|_| {
            gst::element_imp_error!(
                self,
                gst::ResourceError::Read,
                ["Failed to map buffer readable"]
            );

            gst::FlowError::Error
        })?;

        // TODO: We assume 1 spatial and 1 temporal layer. Scalable VP9 streams are not really
        // supported by GStreamer so far and require further design work.
        // FIXME: We also assume that each buffer contains a single VP9 frame. The VP9 caps are
        // misdesigned unfortunately and there's no enforced alignment so this could theoretically
        // also contain a whole superframe. A receiver is likely not going to fail on this.

        let picture_id = *self.picture_id.lock().unwrap();

        // For now we're only getting the keyframe information from the frame header. We could also
        // get information like the frame size from here but it's optional in the RTP payload
        // descriptor and only required for scalable streams.
        //
        // We parse the frame header for the keyframe information because upstream is not
        // necessarily providing correctly parsed information. This is mostly for compatibility
        // with `rtpvp9pay`.
        let mut r = BitReader::endian(map.as_slice(), BigEndian);
        let key_frame = match r.parse::<FrameHeader>() {
            Ok(frame_header) => {
                gst::trace!(CAT, imp = self, "Parsed frame header: {frame_header:?}");
                // show_existing_frame assumes that there is an existing frame to show so this is
                // clearly not a keyframe
                frame_header.is_keyframe.unwrap_or(false)
            }
            Err(err) => {
                gst::trace!(CAT, imp = self, "Failed parsing frame header: {err:?}");
                !buffer.flags().contains(gst::BufferFlags::DELTA_UNIT)
            }
        };

        let mut first = true;
        let mut data = map.as_slice();
        while !data.is_empty() {
            let mut payload_descriptor = PayloadDescriptor {
                picture_id,
                layer_index: None,
                inter_picture_predicted_frame: !key_frame,
                flexible_mode: false,
                reference_indices: Default::default(),
                start_of_frame: first,
                end_of_frame: false, // reset later
                scalability_structure: None,
                not_reference_frame_for_upper_layers: true,
            };

            let payload_descriptor_size = payload_descriptor.size().map_err(|err| {
                gst::error!(
                    CAT,
                    imp = self,
                    "Failed to write payload descriptor: {err:?}"
                );
                gst::FlowError::Error
            })?;
            let overhead = payload_descriptor_size;
            let payload_size = (max_payload_size as usize)
                .checked_sub(overhead + 1)
                .ok_or_else(|| {
                    gst::error!(CAT, imp = self, "Too small MTU configured for stream");
                    gst::element_imp_error!(
                        self,
                        gst::LibraryError::Settings,
                        ["Too small MTU configured for stream"]
                    );
                    gst::FlowError::Error
                })?
                + 1;
            let payload_size = cmp::min(payload_size, data.len());

            payload_descriptor.end_of_frame = data.len() == payload_size;

            gst::trace!(
                CAT,
                imp = self,
                "Writing packet with payload descriptor {payload_descriptor:?} and payload size {payload_size}",
            );

            let mut payload_descriptor_buffer =
                SmallVec::<[u8; 256]>::with_capacity(payload_descriptor_size);
            let mut w = ByteWriter::endian(&mut payload_descriptor_buffer, BigEndian);
            w.build::<PayloadDescriptor>(&payload_descriptor)
                .map_err(|err| {
                    gst::error!(
                        CAT,
                        imp = self,
                        "Failed to write payload descriptor: {err:?}"
                    );
                    gst::FlowError::Error
                })?;
            assert_eq!(payload_descriptor_buffer.len(), payload_descriptor_size);

            self.obj().queue_packet(
                id.into(),
                rtp_types::RtpPacketBuilder::new()
                    .marker_bit(data.len() == payload_size)
                    .payload(payload_descriptor_buffer.as_slice())
                    .payload(&data[..payload_size]),
            )?;

            data = &data[payload_size..];
            first = false;
        }

        let next_picture_id = picture_id.map(PictureId::increment);
        *self.picture_id.lock().unwrap() = next_picture_id;

        Ok(gst::FlowSuccess::Ok)
    }
}
