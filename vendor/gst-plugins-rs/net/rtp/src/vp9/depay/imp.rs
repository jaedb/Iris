//
// Copyright (C) 2023 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-rtpvp9depay2
 * @see_also: rtpvp9pay2, vp9enc, vp9dec
 *
 * Depayload a VP9 video stream from RTP packets as per [draft-ietf-payload-vp9][draft-ietf-payload-vp9].
 *
 * [draft-ietf-payload-vp9]:https://datatracker.ietf.org/doc/html/draft-ietf-payload-vp9-16#section-4
 *
 * ## Example pipeline
 *
 * ```shell
 * gst-launch-1.0 udpsrc address=127.0.0.1 port=5555 caps='application/x-rtp,media=video,clock-rate=90000,encoding-name=VP9' ! rtpjitterbuffer latency=100 ! rtpvp9depay2 ! decodebin3 ! videoconvertscale ! autovideosink
 * ```
 *
 * This will depayload and decode an incoming RTP VP9 video stream. You can use the #rtpvp9pay2
 * and #vp9enc elements to create such an RTP stream.
 *
 * Since: plugins-rs-0.13.0
 */
use std::{io::Cursor, mem, sync::Mutex};

use atomic_refcell::AtomicRefCell;
use bitstream_io::{BigEndian, BitRead as _, BitReader, ByteRead as _, ByteReader};

use gst::{glib, prelude::*, subclass::prelude::*};

use std::sync::LazyLock;

use crate::basedepay::{PacketToBufferRelation, RtpBaseDepay2Ext};
use crate::vp9::frame_header::FrameHeader;
use crate::vp9::payload_descriptor::{PayloadDescriptor, PictureId};

#[derive(Clone, Default)]
struct Settings {
    request_keyframe: bool,
    wait_for_keyframe: bool,
}

struct State {
    /// Last extended RTP timestamp.
    last_timestamp: Option<u64>,

    /// Last picture ID, if any.
    ///
    /// This is the picture ID from the last picture and is reset
    /// to `None` also if a picture doesn't have any ID.
    last_picture_id: Option<PictureId>,

    /// Payload descriptor of the first packet of the last key picture.
    ///
    /// If this is not set then we did not see a keyframe yet.
    last_key_picture_payload_descriptor: Option<PayloadDescriptor>,

    /// Frame header of the last keyframe.
    ///
    /// For scalable streams this is set to the last frame of the picture.
    last_keyframe_frame_header: Option<FrameHeader>,

    /// Frame header of the current keyframe, if any.
    ///
    /// For scalable streams this is set to the last frame of the picture.
    ///
    /// This is only set if the current picture is a key picture and is reset whenever a picture is
    /// pushed downstream.
    current_keyframe_frame_header: Option<FrameHeader>,

    /// Payload descriptor of the first packet of the current picture.
    ///
    /// This is reset whenever the current picture is pushed downstream.
    current_picture_payload_descriptor: Option<PayloadDescriptor>,

    /// Currently queued data for the current picture.
    pending_picture_ext_seqnum: u64,
    pending_picture: Vec<u8>,

    /// Set to `true` if the next outgoing buffer should have the `DISCONT` flag set.
    needs_discont: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            last_timestamp: None,
            last_picture_id: None,
            last_key_picture_payload_descriptor: None,
            last_keyframe_frame_header: None,
            current_keyframe_frame_header: None,
            current_picture_payload_descriptor: None,
            pending_picture_ext_seqnum: 0,
            pending_picture: Vec::default(),
            needs_discont: true,
        }
    }
}

#[derive(Default)]
pub struct RtpVp9Depay {
    state: AtomicRefCell<State>,
    settings: Mutex<Settings>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpvp9depay2",
        gst::DebugColorFlags::empty(),
        Some("RTP VP9 Depayloader"),
    )
});

impl RtpVp9Depay {
    fn reset(&self, state: &mut State) {
        gst::debug!(CAT, imp = self, "resetting state");

        *state = State::default()
    }
}

#[glib::object_subclass]
impl ObjectSubclass for RtpVp9Depay {
    const NAME: &'static str = "GstRtpVp9Depay2";
    type Type = super::RtpVp9Depay;
    type ParentType = crate::basedepay::RtpBaseDepay2;
}

impl ObjectImpl for RtpVp9Depay {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecBoolean::builder("request-keyframe")
                    .nick("Request Keyframe")
                    .blurb("Request new keyframe when packet loss is detected")
                    .default_value(Settings::default().request_keyframe)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("wait-for-keyframe")
                    .nick("Wait For Keyframe")
                    .blurb("Wait for the next keyframe after packet loss")
                    .default_value(Settings::default().wait_for_keyframe)
                    .mutable_ready()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "request-keyframe" => {
                self.settings.lock().unwrap().request_keyframe = value.get().unwrap();
            }
            "wait-for-keyframe" => {
                self.settings.lock().unwrap().wait_for_keyframe = value.get().unwrap();
            }
            _ => unimplemented!(),
        };
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "request-keyframe" => self.settings.lock().unwrap().request_keyframe.to_value(),
            "wait-for-keyframe" => self.settings.lock().unwrap().wait_for_keyframe.to_value(),
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for RtpVp9Depay {}

impl ElementImpl for RtpVp9Depay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP VP9 Depayloader",
                "Codec/Depayloader/Network/RTP",
                "Depayload VP9 from RTP packets",
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

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("video/x-vp9").build(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl crate::basedepay::RtpBaseDepay2Impl for RtpVp9Depay {
    const ALLOWED_META_TAGS: &'static [&'static str] = &["video"];

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.borrow_mut();
        self.reset(&mut state);

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.borrow_mut();
        self.reset(&mut state);

        Ok(())
    }

    fn drain(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        // TODO: Could forward all complete layers here if any are queued up
        Ok(gst::FlowSuccess::Ok)
    }

    fn flush(&self) {
        let mut state = self.state.borrow_mut();
        self.reset(&mut state);
    }

    // TODO: Might want to send lost events (and possibly ignore the ones from upstream) if there
    // are discontinuities (either in the seqnum or otherwise detected). This is especially useful
    // in case of ULPFEC as that breaks seqnum-based discontinuity detecetion.
    //
    // rtpvp9depay does this but it feels like the whole approach needs some redesign.
    fn handle_packet(
        &self,
        packet: &crate::basedepay::Packet,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let settings = self.settings.lock().unwrap().clone();

        gst::trace!(CAT, imp = self, "Handling RTP packet {packet:?}");
        let mut state = self.state.borrow_mut();

        let payload = packet.payload();
        let mut cursor = Cursor::new(payload);

        let mut r = ByteReader::endian(&mut cursor, BigEndian);
        let payload_descriptor = match r.parse::<PayloadDescriptor>() {
            Ok(payload_descriptor) => payload_descriptor,
            Err(err) => {
                gst::warning!(CAT, imp = self, "Invalid VP9 RTP packet: {err}");
                // TODO: Could potentially drain here?
                self.reset(&mut state);
                self.obj().drop_packet(packet);
                return Ok(gst::FlowSuccess::Ok);
            }
        };

        let payload_start_index = cursor.position() as usize;

        gst::trace!(
            CAT,
            imp = self,
            "VP9 RTP payload descriptor size: {}",
            payload_start_index
        );
        gst::trace!(
            CAT,
            imp = self,
            "Received VP9 RTP payload descriptor: {payload_descriptor:?}"
        );

        // This is the start of a picture if this is the start of the frame and either there is no
        // layer information or this is the first spatial layer.
        let is_start_of_picture = payload_descriptor.start_of_frame
            && payload_descriptor
                .layer_index
                .as_ref()
                .is_none_or(|layer_index| layer_index.spatial_layer_id == 0);

        // Additionally, this is a key picture if it is not an inter predicted picture.
        let is_key_picture =
            !payload_descriptor.inter_picture_predicted_frame && is_start_of_picture;

        // If the timestamp or picture ID is changing we assume that a new picture is starting.
        // Any previously queued picture data needs to be drained now.
        if is_start_of_picture
            || state.last_timestamp != Some(packet.ext_timestamp())
            || state
                .last_picture_id
                .is_some_and(|picture_id| Some(picture_id) != payload_descriptor.picture_id)
        {
            // Missed the marker packet for the last picture
            if state.current_picture_payload_descriptor.is_some() {
                gst::warning!(CAT, imp = self, "Packet is part of a new picture but didn't receive last packet of previous picture");
                // TODO: Could potentially drain here?
                self.reset(&mut state);
            }
            // Else cleanly starting a new picture here
        }

        // Validate payload descriptor
        if let Some(ref last_keyframe_payloader_descriptor) =
            state.last_key_picture_payload_descriptor
        {
            // Section 4.2, I flag
            //
            // > If the V bit was set in the stream's most recent start of a keyframe (i.e. the SS
            // > field was present) and the F bit is set to 0 (i.e. non-flexible scalability mode is
            // > in use), then this bit MUST be set on every packet.
            //
            // This check is extended here to not just check for presence of the SS field but check
            // that there are multiple spatial layers. If there is only one then we treat it as if
            // the field wasn't set.
            if last_keyframe_payloader_descriptor
                .scalability_structure
                .as_ref()
                .is_some_and(|scalability_structure| scalability_structure.num_spatial_layers > 1)
                && !payload_descriptor.flexible_mode
                && payload_descriptor.picture_id.is_none()
            {
                gst::warning!(
                    CAT,
                    imp = self,
                    "Scalability structure present and non-flexible scalability mode used but no picture ID present",
                );
                // TODO: Could potentially drain here?
                self.reset(&mut state);
                self.obj().drop_packet(packet);
                return Ok(gst::FlowSuccess::Ok);
            }

            // In other words, picture IDs are only optional if non-flexible scalability mode is
            // used and there was no scalability structure in the keyframe.

            // Section 4.2, F flag
            //
            // > The value of this F bit MUST only change on the first packet of a key picture.  A
            // > key picture is a picture whose base spatial layer frame is a key frame, and which
            // > thus completely resets the encoder state.  This packet will have its P bit equal to
            // > zero, SID or L bit (described below) equal to zero, and B bit (described below)
            // > equal to 1.
            if !is_key_picture
                && last_keyframe_payloader_descriptor.flexible_mode
                    != payload_descriptor.flexible_mode
            {
                gst::warning!(
                    CAT,
                    imp = self,
                    "Flexible scalability mode can only change on key pictures"
                );

                // TODO: Could potentially drain here?
                self.reset(&mut state);
                self.obj().drop_packet(packet);
                return Ok(gst::FlowSuccess::Ok);
            }
        }

        // Section 4.2, P flag
        //
        // > When P is set to zero, the TID field (described below) MUST also be set to 0 (if
        // > present).
        if !payload_descriptor.inter_picture_predicted_frame
            && payload_descriptor
                .layer_index
                .as_ref()
                .is_some_and(|layer_index| layer_index.temporal_layer_id != 0)
        {
            gst::warning!(
                CAT,
                imp = self,
                "Temporal layer ID of non-inter-predicted frame must be 0"
            );
            // TODO: Could potentially drain here?
            self.reset(&mut state);
            self.obj().drop_packet(packet);
            return Ok(gst::FlowSuccess::Ok);
        }

        // Section 4.2, F flag
        //
        // > This MUST only be set to 1 if the I bit is also set to one; if the I bit is set to
        // > zero, then this MUST also be set to zero and ignored by receivers.
        if payload_descriptor.flexible_mode && payload_descriptor.picture_id.is_none() {
            gst::warning!(
                CAT,
                imp = self,
                "Flexible scalability mode but no picture ID present"
            );
            // TODO: Could potentially drain here?
            self.reset(&mut state);
            self.obj().drop_packet(packet);
            return Ok(gst::FlowSuccess::Ok);
        }

        // If this is not the start of a picture then we have to wait for one
        if state.current_picture_payload_descriptor.is_none() && !is_start_of_picture {
            if state.last_timestamp.is_some() {
                gst::warning!(CAT, imp = self, "Waiting for start of picture");
            } else {
                gst::trace!(CAT, imp = self, "Waiting for start of picture");
            }
            // TODO: Could potentially drain here?
            self.obj().drop_packet(packet);
            self.reset(&mut state);
            return Ok(gst::FlowSuccess::Ok);
        }

        // If necessary wait for a key picture if we never saw one so far and/or request one
        // from upstream.
        if is_start_of_picture
            && !is_key_picture
            && state.last_key_picture_payload_descriptor.is_none()
        {
            if settings.request_keyframe {
                gst::debug!(CAT, imp = self, "Requesting keyframe from upstream");
                let event = gst_video::UpstreamForceKeyUnitEvent::builder()
                    .all_headers(true)
                    .build();
                let _ = self.obj().sink_pad().push_event(event);
            }

            if settings.wait_for_keyframe {
                gst::trace!(CAT, imp = self, "Waiting for keyframe");
                // TODO: Could potentially drain here?
                self.reset(&mut state);
                self.obj().drop_packet(packet);
                return Ok(gst::FlowSuccess::Ok);
            }
        }

        // Update state tracking
        if is_start_of_picture {
            assert!(state.pending_picture.is_empty());
            state.pending_picture_ext_seqnum = packet.ext_seqnum();
            state.current_picture_payload_descriptor = Some(payload_descriptor.clone());
            state.last_timestamp = Some(packet.ext_timestamp());
            if let Some(picture_id) = payload_descriptor.picture_id {
                state.last_picture_id = Some(picture_id);
            } else {
                state.last_picture_id = None;
            }

            if is_key_picture {
                state.last_key_picture_payload_descriptor = Some(payload_descriptor.clone());
            }
        }

        // If this is the start of a frame in a key picture then parse the frame header. We always
        // keep the last one around as that should theoretically be the one with the highest
        // resolution and profile.
        if payload_descriptor.start_of_frame
            && state
                .current_picture_payload_descriptor
                .as_ref()
                .is_some_and(|current_picture_payload_descriptor| {
                    !current_picture_payload_descriptor.inter_picture_predicted_frame
                })
        {
            let mut r = BitReader::endian(&mut cursor, BigEndian);
            // We assume that the beginning of the frame header fits into the first packet
            match r.parse::<FrameHeader>() {
                Ok(frame_header) => {
                    gst::trace!(CAT, imp = self, "Parsed frame header: {frame_header:?}");
                    state.current_keyframe_frame_header = Some(frame_header);
                }
                Err(err) => {
                    // Don't consider this a fatal error
                    gst::warning!(CAT, imp = self, "Failed to read frame header: {err}");
                }
            };
        }

        state
            .pending_picture
            .extend_from_slice(&payload[payload_start_index..]);

        // The marker bit is set for the last packet of a picture.
        if !packet.marker_bit() {
            return Ok(gst::FlowSuccess::Ok);
        }

        let current_picture_payload_descriptor =
            state.current_picture_payload_descriptor.take().unwrap();

        if let Some(current_keyframe_frame_header) = state.current_keyframe_frame_header.take() {
            // TODO: Could also add more information to the caps
            if current_keyframe_frame_header.keyframe_info.is_some()
                && state.last_keyframe_frame_header.as_ref().is_none_or(
                    |last_keyframe_frame_header| {
                        last_keyframe_frame_header.profile != current_keyframe_frame_header.profile
                            || last_keyframe_frame_header
                                .keyframe_info
                                .as_ref()
                                .map(|keyframe_info| keyframe_info.render_size())
                                != current_keyframe_frame_header
                                    .keyframe_info
                                    .as_ref()
                                    .map(|keyframe_info| keyframe_info.render_size())
                    },
                )
            {
                let render_size = current_keyframe_frame_header
                    .keyframe_info
                    .as_ref()
                    .map(|keyframe_info| keyframe_info.render_size())
                    .unwrap();

                let caps = gst::Caps::builder("video/x-vp9")
                    .field(
                        "profile",
                        format!("{}", current_keyframe_frame_header.profile),
                    )
                    .field("width", render_size.0 as i32)
                    .field("height", render_size.1 as i32)
                    .build();

                self.obj().set_src_caps(&caps);
            }
            state.last_keyframe_frame_header = Some(current_keyframe_frame_header);
        }

        let mut buffer = gst::Buffer::from_mut_slice(mem::take(&mut state.pending_picture));
        {
            let buffer = buffer.get_mut().unwrap();

            if current_picture_payload_descriptor.inter_picture_predicted_frame {
                buffer.set_flags(gst::BufferFlags::DELTA_UNIT);
                gst::trace!(CAT, imp = self, "Finishing delta-frame");
            } else {
                gst::trace!(CAT, imp = self, "Finishing keyframe");
            }

            if state.needs_discont {
                gst::trace!(CAT, imp = self, "Setting DISCONT");
                buffer.set_flags(gst::BufferFlags::DISCONT);
                state.needs_discont = false;
            }

            // Set MARKER flag on the output so that the parser knows that this buffer ends a full
            // picture and potentially can operate a bit faster.
            buffer.set_flags(gst::BufferFlags::MARKER);
        }

        state.current_picture_payload_descriptor = None;
        state.current_keyframe_frame_header = None;

        // Set fallback caps if the first complete frame we have is not a keyframe. For keyframes,
        // caps with profile and resolution would've been set above already.
        //
        // If a keyframe is received in the future then the caps are updated above.
        if !self.obj().src_pad().has_current_caps() {
            self.obj()
                .set_src_caps(&self.obj().src_pad().pad_template_caps());
        }

        self.obj().queue_buffer(
            PacketToBufferRelation::Seqnums(state.pending_picture_ext_seqnum..=packet.ext_seqnum()),
            buffer,
        )?;

        Ok(gst::FlowSuccess::Ok)
    }
}
