//
// Copyright (C) 2023 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-rtpvp8pay2
 * @see_also: rtpvp8depay2, vp8dec, vp8enc
 *
 * Payload a VP8 video stream into RTP packets as per [RFC 7741][rfc-7741].
 *
 * [rfc-7741]: https://www.rfc-editor.org/rfc/rfc7741#section-4
 *
 * ## Example pipeline
 *
 * |[
 * gst-launch-1.0 videotestsrc ! video/x-raw,width=1280,height=720,format=I420 ! timeoverlay font-desc=Sans,22 ! vp8enc ! rtpvp8pay2 ! udpsink host=127.0.0.1 port=5004
 * ]| This will create and payload a VP8 video stream with a test pattern and
 * send it out via UDP to localhost port 5004.
 *
 * Since: plugins-rs-0.13.0
 */
use atomic_refcell::AtomicRefCell;
use gst::{glib, prelude::*, subclass::prelude::*};
use smallvec::SmallVec;
use std::{cmp, sync::Mutex};

use bitstream_io::{BigEndian, ByteWrite as _, ByteWriter};
use std::sync::LazyLock;

use crate::{
    basepay::{RtpBasePay2Ext, RtpBasePay2ImplExt},
    vp8::{
        frame_header::FrameInfo,
        payload_descriptor::{LayerId, PayloadDescriptor, PictureId},
    },
};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpvp8pay2",
        gst::DebugColorFlags::empty(),
        Some("RTP VP8 Payloader"),
    )
});

#[derive(Clone, Default)]
struct Settings {
    picture_id_mode: super::PictureIdMode,
    picture_id_offset: Option<u16>,
    fragmentation_mode: super::FragmentationMode,
}

#[derive(Default)]
struct State {
    /// Only set if a VP8 custom meta was ever received for this stream. Incremented whenever a
    /// frame with layer-id=0 or no meta is received.
    temporal_layer_zero_index: Option<u8>,
}

#[derive(Default)]
pub struct RtpVp8Pay {
    settings: Mutex<Settings>,
    state: AtomicRefCell<State>,
    /// Current picture ID.
    ///
    /// Reset to `None` in `Null` / `Ready` state and initialized to the offset when going to
    /// `Paused`.
    picture_id: Mutex<Option<PictureId>>,
}

#[glib::object_subclass]
impl ObjectSubclass for RtpVp8Pay {
    const NAME: &'static str = "GstRtpVp8Pay2";
    type Type = super::RtpVp8Pay;
    type ParentType = crate::basepay::RtpBasePay2;
}

impl ObjectImpl for RtpVp8Pay {
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
                glib::ParamSpecEnum::builder::<super::FragmentationMode>("fragmentation-mode")
                    .nick("Fragmentation Mode")
                    .blurb("Fragmentation Mode")
                    .default_value(Settings::default().fragmentation_mode)
                    .mutable_ready()
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
            "fragmentation-mode" => {
                self.settings.lock().unwrap().fragmentation_mode = value.get().unwrap();
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
            "fragmentation-mode" => self.settings.lock().unwrap().fragmentation_mode.to_value(),
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for RtpVp8Pay {}

impl ElementImpl for RtpVp8Pay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP VP8 payloader",
                "Codec/Payloader/Network/RTP",
                "Payload VP8 as RTP packets",
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
                &gst::Caps::builder("video/x-vp8").build(),
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
                        gst::List::new(["VP8", "VP8-DRAFT-IETF-01"]),
                    )
                    .build(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl crate::basepay::RtpBasePay2Impl for RtpVp8Pay {
    const ALLOWED_META_TAGS: &'static [&'static str] = &["video"];

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        *self.state.borrow_mut() = State::default();

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
        *self.state.borrow_mut() = State::default();
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
                gst::List::new(["VP8", "VP8-DRAFT-IETF-01"]),
            );

        self.obj().set_src_caps(&caps_builder.build());

        true
    }

    fn negotiate(&self, mut src_caps: gst::Caps) {
        // Fixate the encoding-name with preference to "VP8"

        src_caps.truncate();
        {
            let src_caps = src_caps.get_mut().unwrap();
            let s = src_caps.structure_mut(0).unwrap();
            s.fixate_field_str("encoding-name", "VP8");
        }

        self.parent_negotiate(src_caps);
    }

    fn handle_buffer(
        &self,
        buffer: &gst::Buffer,
        id: u64,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.borrow_mut();

        let settings = self.settings.lock().unwrap();
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

        let picture_id = *self.picture_id.lock().unwrap();

        // If this flag is not set by upstream then we don't set the corresponding value in the
        // payload descriptor. That's not a problem because it only means that the frame can't be
        // dropped safely, which is what has to be assumed anyway if there's no other information.
        let non_reference_frame = buffer.flags().contains(gst::BufferFlags::DROPPABLE);

        let meta = VP8Meta::from_buffer(buffer);

        // Initialize temporal layer zero index the first time we receive a meta with temporal
        // scaling enabled.
        if meta.as_ref().map(|meta| meta.layer_id.is_some()) == Some(true)
            && state.temporal_layer_zero_index.is_none()
        {
            gst::trace!(CAT, imp = self, "Detected stream with temporal scalability");
            state.temporal_layer_zero_index = Some(0);
        }

        // Can't work with partition indices if temporal scalability is enabled
        let partition_offsets = if state.temporal_layer_zero_index.is_none() {
            match FrameInfo::parse(&map) {
                Ok(frame_info) => {
                    gst::trace!(CAT, imp = self, "Parsed frame info {frame_info:?}");

                    Some(frame_info.partition_offsets)
                }
                Err(err) => {
                    gst::error!(CAT, imp = self, "Failed parsing frame info: {err}");
                    None
                }
            }
        } else {
            None
        };

        let mut first = true;
        let mut current_offset = 0;
        let mut data = map.as_slice();
        while !data.is_empty() {
            let mut payload_descriptor = PayloadDescriptor {
                picture_id,
                non_reference_frame,
                start_of_partition: first,
                partition_index: 0, // filled later
                temporal_layer_zero_index: state.temporal_layer_zero_index,
                temporal_layer_id: if state.temporal_layer_zero_index.is_some() {
                    let (temporal_layer, layer_sync) = meta
                        .as_ref()
                        .and_then(|meta| meta.layer_id)
                        .unwrap_or((0, false));

                    Some(LayerId {
                        id: temporal_layer as u8,
                        sync: layer_sync,
                    })
                } else {
                    None
                },
                key_index: None,
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

            let mut payload_size = cmp::min(payload_size, data.len());

            if let Some(ref partition_offsets) = partition_offsets {
                let (start_partition_index, start_partition_start, start_partition_end) =
                    find_partition_for_offset(partition_offsets, current_offset);

                // FIXME: Partition indices go from 0 to 8 inclusive, but there are only 3 bits
                // available. The first two partitions are considered partition index 0.
                //
                // If there are 8 DCT token partitions then there are 9 partitions overall because
                // there's always the first partition with motion vectors etc.
                if start_partition_index <= 1 {
                    payload_descriptor.partition_index = 0;
                } else {
                    payload_descriptor.partition_index = (start_partition_index - 1) as u8 & 0b111;

                    // For the first partition this is set above when creating the payload
                    // descriptor.
                    if start_partition_start == current_offset {
                        payload_descriptor.start_of_partition = true;
                    }
                }

                let (end_partition_index, end_partition_start, end_partition_end) =
                    find_partition_for_offset(
                        partition_offsets,
                        // -1 so we have the last byte that is still part of the payload
                        current_offset + payload_size as u32 - 1,
                    );

                // Check if the payload size has to be reduced to fit the selected fragmentation mode.
                match settings.fragmentation_mode {
                    crate::vp8::pay::FragmentationMode::None => (),
                    crate::vp8::pay::FragmentationMode::PartitionStart => {
                        // If start and end partition are different then set the payload size in
                        // such a way that it ends just before the end partition, i.e. the next
                        // packet would start with that partition.
                        //
                        // If the end partition index is partition 1 then don't do anything: as
                        // explained above we consider partition 0 and 1 as one.
                        if start_partition_index != end_partition_index
                            && end_partition_index != 1
                            && end_partition_end > (current_offset + payload_size as u32)
                        {
                            payload_size = (end_partition_start - current_offset) as usize;
                        }
                    }
                    crate::vp8::pay::FragmentationMode::EveryPartition => {
                        // If the end offset is after the end of the current partition then reduce
                        // it to the end of the current partition.
                        //
                        // If the end partition index is partition 1 then consider the end of that
                        // one instead: as explained above we consider partition 0 and 1 as one.
                        //
                        // If the end partition is partition 0 then there's nothing to be done.
                        // Start and end of this packet are inside partition 0.
                        if end_partition_index > 1
                            && current_offset + payload_size as u32 > start_partition_end
                        {
                            payload_size = (start_partition_end - current_offset) as usize;
                        } else if end_partition_index == 1
                            && current_offset + payload_size as u32 > end_partition_end
                        {
                            payload_size = (end_partition_end - current_offset) as usize;
                        }
                    }
                }
            }

            gst::trace!(
                CAT,
                imp = self,
                "Writing packet with payload descriptor {payload_descriptor:?} and payload size {payload_size} at offset {current_offset}",
            );

            assert!(payload_size > 0);

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
            current_offset += payload_size as u32;
            first = false;
        }

        // If this temporal layer zero then increment the temporal layer zero index.
        //
        // FIXME: This is only correct for prediction structures where higher layers always refers
        // to the previous base layer frame.
        if meta.is_none_or(|meta| matches!(meta.layer_id, Some((0, _)))) {
            if let Some(ref mut temporal_layer_zero_index) = state.temporal_layer_zero_index {
                *temporal_layer_zero_index = temporal_layer_zero_index.wrapping_add(1);
                gst::trace!(
                    CAT,
                    imp = self,
                    "Updated temporal layer zero index to {temporal_layer_zero_index}"
                );
            }
        }

        let next_picture_id = picture_id.map(PictureId::increment);
        *self.picture_id.lock().unwrap() = next_picture_id;

        Ok(gst::FlowSuccess::Ok)
    }

    fn transform_meta(
        &self,
        in_buf: &gst::BufferRef,
        meta: &gst::MetaRef<gst::Meta>,
        out_buf: &mut gst::BufferRef,
    ) {
        // Drop VP8 custom meta, handle all other metas normally.
        if meta
            .try_as_custom_meta()
            .is_some_and(|meta| meta.has_name("GstVP8Meta"))
        {
            return;
        }

        self.parent_transform_meta(in_buf, meta, out_buf)
    }
}

struct VP8Meta {
    layer_id: Option<(u32, bool)>,
}

impl VP8Meta {
    fn from_buffer(buffer: &gst::BufferRef) -> Option<Self> {
        let meta = gst::meta::CustomMeta::from_buffer(buffer, "GstVP8Meta").ok()?;

        let s = meta.structure();

        let layer_id = if s.get::<bool>("use-temporal-scaling") == Ok(true) {
            let layer_id = s.get::<u32>("layer-id").ok()?;
            let layer_sync = s.get::<bool>("layer-sync").ok()?;

            Some((layer_id, layer_sync))
        } else {
            None
        };

        Some(VP8Meta { layer_id })
    }
}

/// Returns the partition for a given offset, the start offset of that partition and its end
/// offset.
fn find_partition_for_offset(partition_offsets: &[u32], offset: u32) -> (usize, u32, u32) {
    // There are always at least two items: 0 and the whole frame length.

    assert!(!partition_offsets.is_empty());

    for (idx, offsets) in partition_offsets.windows(2).enumerate() {
        let [start, end] = [offsets[0], offsets[1]];

        if (start..end).contains(&offset) {
            return (idx, start, end);
        }
    }

    // Can't get here because the last offset is always the length of the whole frame.
    unreachable!();
}
