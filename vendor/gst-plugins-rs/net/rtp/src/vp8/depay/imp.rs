//
// Copyright (C) 2023 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-rtpvp8depay2
 * @see_also: rtpvp8pay2, vp8enc, vp8dec
 *
 * Depayload a VP8 video stream from RTP packets as per [RFC 7741][rfc-7741].
 *
 * [rfc-7741]: https://www.rfc-editor.org/rfc/rfc7741#section-4
 *
 * ## Example pipeline
 *
 * ```shell
 * gst-launch-1.0 udpsrc address=127.0.0.1 port=5555 caps='application/x-rtp,media=video,clock-rate=90000,encoding-name=VP8' ! rtpjitterbuffer latency=100 ! rtpvp8depay2 ! decodebin3 ! videoconvertscale ! autovideosink
 * ```
 *
 * This will depayload and decode an incoming RTP VP8 video stream. You can use the #rtpvp8pay2
 * and #vp8enc elements to create such an RTP stream.
 *
 * Since: plugins-rs-0.13.0
 */
use std::{io::Cursor, mem, sync::Mutex};

use atomic_refcell::AtomicRefCell;
use bitstream_io::{BigEndian, ByteRead as _, ByteReader};

use gst::{glib, prelude::*, subclass::prelude::*};

use std::sync::LazyLock;

use crate::basedepay::{PacketToBufferRelation, RtpBaseDepay2Ext};
use crate::vp8::frame_header::UncompressedFrameHeader;
use crate::vp8::payload_descriptor::{PayloadDescriptor, PictureId};

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
    /// This is the picture ID from the last frame and is reset
    /// to `None` also if a picture doesn't have any ID.
    last_picture_id: Option<PictureId>,

    /// Payload descriptor of the first packet of the current frame.
    ///
    /// This is reset whenever the current frame is pushed downstream.
    current_frame_payload_descriptor: Option<PayloadDescriptor>,

    /// Last keyframe frame header
    last_keyframe_frame_header: Option<UncompressedFrameHeader>,

    /// Currently queued data for the current frame.
    pending_frame_ext_seqnum: u64,
    pending_frame_is_keyframe: bool,
    pending_frame: Vec<u8>,

    /// Set to `true` if the next outgoing buffer should have the `DISCONT` flag set.
    needs_discont: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            last_timestamp: None,
            last_picture_id: None,
            current_frame_payload_descriptor: None,
            last_keyframe_frame_header: None,
            pending_frame_ext_seqnum: 0,
            pending_frame: Vec::default(),
            pending_frame_is_keyframe: false,
            needs_discont: true,
        }
    }
}

#[derive(Default)]
pub struct RtpVp8Depay {
    state: AtomicRefCell<State>,
    settings: Mutex<Settings>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpvp8depay2",
        gst::DebugColorFlags::empty(),
        Some("RTP VP8 Depayloader"),
    )
});

impl RtpVp8Depay {
    fn reset(&self, state: &mut State) {
        gst::debug!(CAT, imp = self, "resetting state");

        *state = State::default()
    }
}

#[glib::object_subclass]
impl ObjectSubclass for RtpVp8Depay {
    const NAME: &'static str = "GstRtpVp8Depay2";
    type Type = super::RtpVp8Depay;
    type ParentType = crate::basedepay::RtpBaseDepay2;
}

impl ObjectImpl for RtpVp8Depay {
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

impl GstObjectImpl for RtpVp8Depay {}

impl ElementImpl for RtpVp8Depay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP VP8 Depayloader",
                "Codec/Depayloader/Network/RTP",
                "Depayload VP8 from RTP packets",
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
                        gst::List::new(["VP8", "VP8-DRAFT-IETF-01"]),
                    )
                    .build(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("video/x-vp8").build(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl crate::basedepay::RtpBaseDepay2Impl for RtpVp8Depay {
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

    fn flush(&self) {
        let mut state = self.state.borrow_mut();
        self.reset(&mut state);
    }

    // TODO: Might want to send lost events (and possibly ignore the ones from upstream) if there
    // are discontinuities (either in the seqnum or otherwise detected). This is especially useful
    // in case of ULPFEC as that breaks seqnum-based discontinuity detecetion.
    //
    // rtpvp8depay does this but it feels like the whole approach needs some redesign.
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
                gst::warning!(CAT, imp = self, "Invalid VP8 RTP packet: {err}");
                self.reset(&mut state);
                self.obj().drop_packet(packet);
                return Ok(gst::FlowSuccess::Ok);
            }
        };

        let payload_start_index = cursor.position() as usize;
        gst::trace!(
            CAT,
            imp = self,
            "VP8 RTP payload descriptor size: {}",
            payload_start_index
        );
        gst::trace!(
            CAT,
            imp = self,
            "Received VP8 RTP payload descriptor: {payload_descriptor:?}"
        );

        // This is the start of a frame if it is the start of a partition and the partition index
        // is 0.
        let is_start_of_frame =
            payload_descriptor.start_of_partition && payload_descriptor.partition_index == 0;

        // If this is not the start of a picture then we have to wait for one
        if state.current_frame_payload_descriptor.is_none() && !is_start_of_frame {
            if state.last_timestamp.is_some() {
                gst::warning!(CAT, imp = self, "Waiting for start of picture");
            } else {
                gst::trace!(CAT, imp = self, "Waiting for start of picture");
            }
            self.obj().drop_packet(packet);
            self.reset(&mut state);
            return Ok(gst::FlowSuccess::Ok);
        }

        // Update state tracking
        if is_start_of_frame {
            let mut r = ByteReader::endian(&mut cursor, BigEndian);
            // We assume that the 10 bytes of frame header are in the first packet
            let frame_header = match r.parse::<UncompressedFrameHeader>() {
                Ok(frame_header) => frame_header,
                Err(err) => {
                    gst::warning!(CAT, imp = self, "Failed to read frame header: {err}");
                    self.obj().drop_packet(packet);
                    self.reset(&mut state);
                    return Ok(gst::FlowSuccess::Ok);
                }
            };

            // If necessary wait for a key frame if we never saw one so far and/or request one
            // from upstream.
            if !frame_header.is_keyframe && state.last_keyframe_frame_header.is_none() {
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

            assert!(state.pending_frame.is_empty());
            state.pending_frame_ext_seqnum = packet.ext_seqnum();
            state.pending_frame_is_keyframe = frame_header.is_keyframe;
            state.current_frame_payload_descriptor = Some(payload_descriptor.clone());
            state.last_timestamp = Some(packet.ext_timestamp());
            if let Some(picture_id) = payload_descriptor.picture_id {
                state.last_picture_id = Some(picture_id);
            } else {
                state.last_picture_id = None;
            }

            if frame_header.is_keyframe {
                // Update caps with profile and resolution now that we know it
                if state
                    .last_keyframe_frame_header
                    .as_ref()
                    .is_none_or(|last_frame_header| {
                        last_frame_header.profile != frame_header.profile
                            || last_frame_header.resolution != frame_header.resolution
                    })
                {
                    let resolution = frame_header.resolution.unwrap();

                    let caps = gst::Caps::builder("video/x-vp8")
                        .field("profile", format!("{}", frame_header.profile))
                        .field("width", resolution.0 as i32)
                        .field("height", resolution.1 as i32)
                        .build();

                    self.obj().set_src_caps(&caps);
                }
                state.last_keyframe_frame_header = Some(frame_header);
            }
        }

        state
            .pending_frame
            .extend_from_slice(&payload[payload_start_index..]);

        // The marker bit is set for the last packet of a frame.
        if !packet.marker_bit() {
            return Ok(gst::FlowSuccess::Ok);
        }

        let mut buffer = gst::Buffer::from_mut_slice(mem::take(&mut state.pending_frame));
        {
            let buffer = buffer.get_mut().unwrap();

            if !state.pending_frame_is_keyframe {
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
            // frame and potentially can operate a bit faster.
            buffer.set_flags(gst::BufferFlags::MARKER);

            // TODO: Could add VP8 custom meta about scalability here
        }

        state.current_frame_payload_descriptor = None;
        state.pending_frame_is_keyframe = false;

        // Set fallback caps if the first complete frame we have is not a keyframe. For keyframes,
        // caps with profile and resolution would've been set above already.
        //
        // If a keyframe is received in the future then the caps are updated above.
        if !self.obj().src_pad().has_current_caps() {
            self.obj()
                .set_src_caps(&self.obj().src_pad().pad_template_caps());
        }

        self.obj().queue_buffer(
            PacketToBufferRelation::Seqnums(state.pending_frame_ext_seqnum..=packet.ext_seqnum()),
            buffer,
        )?;

        Ok(gst::FlowSuccess::Ok)
    }
}
