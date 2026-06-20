// GStreamer RTP MPEG-4 Generic Payloader
//
// Copyright (C) 2023-2024 François Laignel <francois centricular com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-rtpmp4gpay2
 * @see_also: rtpmp4gpay2, rtpmp4gpay, rtpmp4gpay, fdkaacenc, fdkaacdec, avenc_mpeg4, avdec_mpeg4
 *
 * Payload an MPEG-4 Generic elementary stream into RTP packets as per [RFC 3640][rfc-3640].
 * Also see the [IANA media-type page for MPEG-4 Generic][iana-mpeg4-generic].
 *
 * [rfc-3640]: https://www.rfc-editor.org/rfc/rfc3640.html#section-4
 * [iana-mpeg4-generic]: https://www.iana.org/assignments/media-types/application/mpeg4-generic
 *
 * ## Aggregation Modes
 *
 * The default aggregation mode is `auto`: If upstream is live, the payloader will send out
 * AUs immediately, even if they don't completely fill a packet, in order to minimise
 * latency. If upstream is not live, the payloader will by default aggregate AUs until
 * it has completely filled an RTP packet as per the configured MTU size or the `max-ptime`
 * property if it is set (it is not set by default).
 *
 * The aggregation mode can be controlled via the `aggregate-mode` property.
 *
 * ## Example pipeline
 * |[
 * gst-launch-1.0 audiotestsrc ! fdkaacenc ! rtpmp4gpay2 ! udpsink host=127.0.0.1 port=5004
 * ]| This will encode an audio test signal to AAC and then payload the encoded audio
 * into RTP packets and send them out via UDP to localhost (IPv4) port 5004.
 * You can use the #rtpmp4gdepay2 or #rtpmp4gdepay elements to depayload such a stream, and
 * the #fdkaacdec element to decode the depayloaded stream.
 *
 * Since: plugins-rs-0.13.0
 */
use atomic_refcell::AtomicRefCell;
use bitstream_io::{BigEndian, BitRead, BitReader, BitWrite, BitWriter, BitsWritten};
use std::sync::LazyLock;

use gst::{glib, prelude::*, subclass::prelude::*};
use smallvec::SmallVec;

use std::collections::VecDeque;
use std::sync::Mutex;

use crate::basepay::{PacketToBufferRelation, RtpBasePay2Ext, RtpBasePay2Impl, RtpBasePay2ImplExt};

use super::RtpMpeg4GenericPayAggregateMode;
use crate::mp4a::parsers::{AudioSpecificConfig, ProfileLevel};
use crate::mp4g::{AccessUnitIndex, AuHeader, AuHeaderContext, ModeConfig};

const VOS_STARTCODE: u32 = 0x000001B0;

/// The size of the field representing the AU headers section len.
const HEADERS_LEN_SIZE: usize = 2;

/// Access Unit maximum header len in bytes.
/// This depends on the supported mode. In current implementation, 3 is the maximum.
const HEADER_MAX_LEN: usize = 3;

#[derive(Clone)]
struct Settings {
    max_ptime: Option<gst::ClockTime>,
    aggregate_mode: RtpMpeg4GenericPayAggregateMode,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            aggregate_mode: RtpMpeg4GenericPayAggregateMode::Auto,
            max_ptime: None,
        }
    }
}

#[derive(Default)]
pub struct RtpMpeg4GenericPay {
    state: AtomicRefCell<State>,
    settings: Mutex<Settings>,
    is_live: Mutex<Option<bool>>,
}

#[derive(Debug)]
struct AccessUnit {
    id: u64,
    pts: Option<gst::ClockTime>,
    dts_delta: Option<i32>,
    duration: Option<gst::ClockTime>,
    maybe_random_access: Option<bool>,
    buffer: gst::MappedBuffer<gst::buffer::Readable>,
}

#[derive(Default)]
struct State {
    /// Configuration of current Mode.
    mode: ModeConfig,

    /// Maximum bit length needed to store an AU Header.
    max_header_bit_len: usize,

    /// Minimum MTU necessary to handle the outgoing packets.
    min_mtu: usize,

    /// Pending AU (we collect until ptime/max-ptime is hit or the packet is full)
    pending_aus: VecDeque<AccessUnit>,
    pending_size: usize,
    pending_duration: Option<gst::ClockTime>,
    clock_rate: u32,

    /// Desired "packet time", i.e. packet duration, from the downstream caps, if set
    ptime: Option<gst::ClockTime>,
    max_ptime: Option<gst::ClockTime>,
}

impl State {
    fn flush(&mut self) {
        self.pending_aus.clear();
        self.pending_size = 0;
        self.pending_duration = None;
    }
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpmp4gpay2",
        gst::DebugColorFlags::empty(),
        Some("RTP MPEG-4 Generic Payloader"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for RtpMpeg4GenericPay {
    const NAME: &'static str = "GstRtpMpeg4GenericPay";
    type Type = super::RtpMpeg4GenericPay;
    type ParentType = crate::basepay::RtpBasePay2;
}

impl ObjectImpl for RtpMpeg4GenericPay {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecEnum::builder_with_default(
                    "aggregate-mode",
                    Settings::default().aggregate_mode,
                )
                .nick("Aggregate Mode")
                .blurb(
                    "Whether to send out AUs immediately or aggregate them until a packet is full.",
                )
                .build(),
                // Using same type/semantics as C payloaders
                glib::ParamSpecInt64::builder("max-ptime")
                    .nick("Maximum Packet Time")
                    .blurb("Maximum duration of the packet data in ns (-1 = unlimited up to MTU)")
                    .default_value(
                        Settings::default()
                            .max_ptime
                            .map(gst::ClockTime::nseconds)
                            .map(|x| x as i64)
                            .unwrap_or(-1),
                    )
                    .minimum(-1)
                    .maximum(i64::MAX)
                    .mutable_playing()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().unwrap();

        match pspec.name() {
            "aggregate-mode" => {
                settings.aggregate_mode = value
                    .get::<RtpMpeg4GenericPayAggregateMode>()
                    .expect("type checked upstream");
            }
            "max-ptime" => {
                let new_max_ptime = match value.get::<i64>().unwrap() {
                    -1 => None,
                    v @ 0.. => Some(gst::ClockTime::from_nseconds(v as u64)),
                    _ => unreachable!(),
                };
                let changed = settings.max_ptime != new_max_ptime;
                settings.max_ptime = new_max_ptime;
                drop(settings);

                if changed {
                    let _ = self
                        .obj()
                        .post_message(gst::message::Latency::builder().src(&*self.obj()).build());
                }
            }
            _ => unimplemented!(),
        };
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().unwrap();

        match pspec.name() {
            "aggregate-mode" => settings.aggregate_mode.to_value(),
            "max-ptime" => (settings
                .max_ptime
                .map(gst::ClockTime::nseconds)
                .map(|x| x as i64)
                .unwrap_or(-1))
            .to_value(),
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for RtpMpeg4GenericPay {}

impl ElementImpl for RtpMpeg4GenericPay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTP MPEG-4 Generic Payloader",
                "Codec/Payloader/Network/RTP",
                "Payload an MPEG-4 Generic elementary stream into RTP packets (RFC 3640)",
                "François Laignel <francois centricular com>",
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
                &gst::Caps::builder_full()
                    .structure(
                        gst::Structure::builder("video/mpeg")
                            .field("mpegversion", 4i32)
                            .field("systemstream", false)
                            .build(),
                    )
                    .structure(
                        gst::Structure::builder("audio/mpeg")
                            .field("mpegversion", 4i32)
                            .field("stream-format", "raw")
                            .build(),
                    )
                    .build(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("application/x-rtp")
                    // TODO "application" is also present in rtpmp4gpay caps template
                    //       but it doesn't handle it in gst_rtp_mp4g_pay_setcaps
                    .field("media", gst::List::new(["audio", "video"]))
                    .field("clock-rate", gst::IntRange::new(1i32, i32::MAX))
                    .field("encoding-name", "MPEG4-GENERIC")
                    // Required string params:
                    .field("streamtype", gst::List::new(["4", "5"])) // 4 = video, 5 = audio
                    // "profile-level-id = [1,MAX], "
                    // "config = (string)"
                    .field(
                        "mode",
                        gst::List::new(["generic", "AAC-lbr", "AAC-hbr", "aac-hbr"]),
                    )
                    // Optional general parameters:
                    // "objecttype = [1,MAX], "
                    // "constantsize = [1,MAX], "     // constant size of each AU
                    // "constantduration = [1,MAX], " // constant duration of each AU
                    // "maxdisplacement = [1,MAX], "
                    // "de-interleavebuffersize = [1,MAX], "
                    // Optional configuration parameters:
                    // "sizelength = [1, 32], "
                    // "indexlength = [1, 32], "
                    // "indexdeltalength = [1, 32], "
                    // "ctsdeltalength = [1, 32], "
                    // "dtsdeltalength = [1, 32], "
                    // "randomaccessindication = {0, 1}, "
                    // "streamstateindication = [0, 32], "
                    // "auxiliarydatasizelength = [0, 32]" )
                    .build(),
            )
            .unwrap();

            vec![sink_pad_template, src_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

/// Returns the difference between `ClockTime`s `ct1` & `ct2` in RTP scale.
///
/// Returns `None` if at least one of the `ClockTime`s is `None`.
/// Returns `Some(None)` if an overflow occurred, error management is left to the caller.
/// Returns `Some(delta)` if the difference could be computed.
fn ct_delta_to_rtp(
    ct1: Option<gst::ClockTime>,
    ct0: Option<gst::ClockTime>,
    clock_rate: u32,
) -> Option<Option<i32>> {
    ct1.into_positive().opt_sub(ct0).map(|delta_ct| {
        delta_ct
            .into_inner_signed()
            .try_into()
            .ok()
            .and_then(|delta_inner: i64| {
                delta_inner
                    .mul_div_ceil(clock_rate as i64, *gst::ClockTime::SECOND as i64)
                    .and_then(|dts_delta| dts_delta.try_into().ok())
            })
    })
}

impl RtpBasePay2Impl for RtpMpeg4GenericPay {
    const ALLOWED_META_TAGS: &'static [&'static str] = &["audio"];

    fn set_sink_caps(&self, caps: &gst::Caps) -> bool {
        let s = caps.structure(0).unwrap();

        let codec_data = match s.get::<&gst::BufferRef>("codec_data") {
            Ok(codec_data) => codec_data,
            Err(err) => {
                gst::error!(CAT, imp = self, "Error getting codec_data from Caps: {err}");
                return false;
            }
        };

        let Ok(codec_data) = codec_data.map_readable() else {
            gst::error!(CAT, imp = self, "Failed to map codec_data as readable");
            return false;
        };

        let codec_data_str = hex::encode(&codec_data);

        let caps_builder = gst::Caps::builder("application/x-rtp")
            .field("mpegversion", 4i32)
            .field("encoding-name", "MPEG4-GENERIC")
            .field("config", codec_data_str);

        let (clock_rate, mode, caps_builder) = match s.name().as_str() {
            "audio/mpeg" => {
                let mut r = BitReader::endian(codec_data.as_slice(), BigEndian);
                let config = match r.parse::<AudioSpecificConfig>() {
                    Ok(config) => config,
                    Err(err) => {
                        gst::error!(CAT, imp = self, "Error parsing audio codec_data: {err:#}");
                        return false;
                    }
                };

                if config.audio_object_type == 0 || config.audio_object_type > 6 {
                    gst::error!(
                        CAT,
                        imp = self,
                        "Unsupported Audio Object Type {}",
                        config.audio_object_type
                    );
                    return false;
                }

                let profile_level = match ProfileLevel::from_caps(s) {
                    Ok(profile_level) => profile_level,
                    Err(err) => {
                        gst::error!(
                            CAT,
                            imp = self,
                            "Error getting profile level from Caps: {err:#}"
                        );
                        return false;
                    }
                };

                gst::log!(CAT, imp = self, "Using audio codec_data {config:?}");

                // AAC-hbr: also used by rtpmp4gpay
                // RFC 3640 also defines AAC-lbr, with a maximum encoded buffer
                // size of 63 bytes and which can't be fragmented. Only AAC-hbr
                // is used because it is more flexible. We could implement AAC-lbr
                // provided make sure the encoded buffers can't exceed the limit
                // and add a flag to prevent fragmentation in `send_packets()`.
                // See https://www.rfc-editor.org/rfc/rfc3640.html#section-3.3.5
                let mode = ModeConfig {
                    size_len: 13,
                    index_len: 3,
                    index_delta_len: 3,
                    constant_duration: config.frame_len as u32,
                    ..Default::default()
                };

                let caps_builder = mode
                    .add_to_caps(
                        caps_builder
                            .field("media", "audio")
                            .field("streamtype", "5")
                            .field("mode", "AAC-hbr")
                            .field("clock-rate", config.sampling_freq as i32)
                            .field("profile", &profile_level.profile)
                            .field("level", &profile_level.level)
                            .field("profile-level-id", profile_level.id)
                            .field("encoding-params", config.channel_conf as i32),
                    )
                    .expect("invalid audio mode");

                (config.sampling_freq, mode, caps_builder)
            }
            "video/mpeg" => {
                if codec_data.len() < 5 {
                    gst::error!(CAT, imp = self, "Error parsing video codec_data: too short");
                    return false;
                }

                let code = u32::from_be_bytes(codec_data[..4].try_into().unwrap());
                let profile = if code == VOS_STARTCODE {
                    let profile = codec_data[4];
                    gst::log!(CAT, imp = self, "Using video codec_data profile {profile}");

                    profile
                } else {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Unexpected VOS startcode in video codec_data. Assuming profile '1'"
                    );

                    1
                };

                // Use a larger size_len than rtpmp4gpay
                // otherwise some large AU can't be payloaded.
                // rtpmp4gpay uses bit shifts to have the AU data size
                // fit in 13 bits, resulting in an invalid size.
                let mode = ModeConfig {
                    size_len: 16,
                    index_len: 3,
                    index_delta_len: 3,
                    cts_delta_len: 16,
                    dts_delta_len: 16,
                    random_access_indication: true,
                    ..Default::default()
                };

                let caps_builder = mode
                    .add_to_caps(
                        caps_builder
                            .field("media", "video")
                            .field("streamtype", "4")
                            .field("mode", "generic")
                            .field("clock-rate", 90000i32)
                            .field("profile-level-id", profile as i32),
                    )
                    .expect("invalid video mode");

                (90000, mode, caps_builder)
            }
            // TODO handle "application"
            _ => unreachable!(),
        };

        self.obj().set_src_caps(&caps_builder.build());

        let mut state = self.state.borrow_mut();
        state.max_header_bit_len = mode.max_header_bit_len();
        state.min_mtu = rtp_types::RtpPacket::MIN_RTP_PACKET_LEN
            + HEADERS_LEN_SIZE
            + state.max_header_bit_len.div_ceil(8)
            + 1;
        state.mode = mode;
        state.clock_rate = clock_rate;

        true
    }

    fn negotiate(&self, mut src_caps: gst::Caps) {
        // Fixate as a first step
        src_caps.fixate();

        let s = src_caps.structure(0).unwrap();

        // Negotiate ptime/maxptime with downstream and use them in combination with the
        // properties. See https://www.iana.org/assignments/media-types/application/mpeg4-generic
        let ptime = s
            .get::<u32>("ptime")
            .ok()
            .map(u64::from)
            .map(gst::ClockTime::from_mseconds);

        let max_ptime = s
            .get::<u32>("maxptime")
            .ok()
            .map(u64::from)
            .map(gst::ClockTime::from_mseconds);

        self.parent_negotiate(src_caps);

        let mut state = self.state.borrow_mut();
        state.ptime = ptime;
        state.max_ptime = max_ptime;
        drop(state);
    }

    // Encapsulation of MPEG-4 Generic Elementary Streams:
    // https://www.rfc-editor.org/rfc/rfc3640
    fn handle_buffer(
        &self,
        buffer: &gst::Buffer,
        id: u64,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.borrow_mut();
        let mut settings = self.settings.lock().unwrap();

        gst::trace!(
            CAT,
            imp = self,
            "Handling buffer {id} duration {} pts {} dts {}, len {}",
            buffer.duration().display(),
            buffer.pts().display(),
            buffer.dts().display(),
            buffer.size(),
        );

        let maybe_random_access = if state.mode.random_access_indication {
            Some(!buffer.flags().contains(gst::BufferFlags::DELTA_UNIT))
        } else {
            None
        };

        let dts_delta = ct_delta_to_rtp(buffer.dts(), buffer.pts(), state.clock_rate).and_then(
            |dts_delta_res| {
                if dts_delta_res.is_none() {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "Overflow computing DTS-delta between pts {} & dts {}",
                        buffer.dts().display(),
                        buffer.pts().display(),
                    );
                }

                dts_delta_res
            },
        );

        gst::trace!(CAT, imp = self,
            "Pushing AU from buffer {id} dts_delta {dts_delta:?} random access {maybe_random_access:?}",
        );

        state.pending_aus.push_back(AccessUnit {
            id,
            duration: buffer.duration(),
            pts: buffer.pts(),
            dts_delta,
            buffer: buffer.clone().into_mapped_buffer_readable().map_err(|_| {
                gst::error!(CAT, imp = self, "Can't map incoming buffer readable");
                gst::FlowError::Error
            })?,
            maybe_random_access,
        });

        state.pending_size += buffer.size();
        state.pending_duration.opt_add_assign(buffer.duration());

        // Make sure we have queried upstream liveness if needed
        if settings.aggregate_mode == RtpMpeg4GenericPayAggregateMode::Auto {
            self.ensure_upstream_liveness(&mut settings);
        }

        self.send_packets(&settings, &mut state, SendPacketMode::WhenReady)
    }

    fn drain(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        let settings = self.settings.lock().unwrap().clone();
        let mut state = self.state.borrow_mut();

        self.send_packets(&settings, &mut state, SendPacketMode::ForcePending)
    }

    fn flush(&self) {
        self.state.borrow_mut().flush();
    }

    #[allow(clippy::single_match)]
    fn src_query(&self, query: &mut gst::QueryRef) -> bool {
        let res = self.parent_src_query(query);
        if !res {
            return false;
        }

        match query.view_mut() {
            gst::QueryViewMut::Latency(query) => {
                let settings = self.settings.lock().unwrap();

                let (is_live, mut min, mut max) = query.result();

                {
                    let mut live_guard = self.is_live.lock().unwrap();

                    if Some(is_live) != *live_guard {
                        gst::info!(CAT, imp = self, "Upstream is live: {is_live}");
                        *live_guard = Some(is_live);
                    }
                }

                if self.effective_aggregate_mode(&settings)
                    == RtpMpeg4GenericPayAggregateMode::Aggregate
                {
                    if let Some(max_ptime) = settings.max_ptime {
                        min += max_ptime;
                        max.opt_add_assign(max_ptime);
                    } else if is_live {
                        gst::warning!(
                            CAT,
                            imp = self,
                            "Aggregating packets in live mode, but no max_ptime configured. \
                            Configured latency may be too low!"
                        );
                    }
                    query.set(is_live, min, max);
                }
            }
            _ => (),
        }

        true
    }

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        *self.state.borrow_mut() = State::default();
        *self.is_live.lock().unwrap() = None;

        self.parent_start()
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        *self.state.borrow_mut() = State::default();
        *self.is_live.lock().unwrap() = None;

        self.parent_stop()
    }
}

#[derive(Debug, PartialEq)]
enum SendPacketMode {
    WhenReady,
    ForcePending,
}

impl RtpMpeg4GenericPay {
    fn send_packets(
        &self,
        settings: &Settings,
        state: &mut State,
        send_mode: SendPacketMode,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let agg_mode = self.effective_aggregate_mode(settings);

        if (self.obj().mtu() as usize) < state.min_mtu {
            gst::error!(
                CAT,
                imp = self,
                "Insufficient mtu {} at least {} bytes needed",
                self.obj().mtu(),
                state.min_mtu
            );
            return Err(gst::FlowError::Error);
        }

        let max_payload_size = self.obj().max_payload_size() as usize - HEADERS_LEN_SIZE;

        let mut ctx = AuHeaderContext {
            config: &state.mode,
            prev_index: None,
        };
        let mut headers_buf = SmallVec::<[u8; 10 * HEADER_MAX_LEN]>::new();
        let mut au_data_list = SmallVec::<[gst::MappedBuffer<gst::buffer::Readable>; 10]>::new();

        // https://www.rfc-editor.org/rfc/rfc3640.html#section-3.1
        // The M bit is set to 1 to indicate that the RTP packet payload
        // contains either the final fragment of a fragmented Access Unit
        // or one or more complete Access Units.

        // Send out packets if there's enough data for one (or more), or if forced.
        while let Some(front) = state.pending_aus.front() {
            headers_buf.clear();
            ctx.prev_index = None;

            if front.buffer.len() + state.max_header_bit_len.div_ceil(8) > max_payload_size {
                // AU needs to be fragmented
                let au = state.pending_aus.pop_front().unwrap();
                let mut data = au.buffer.as_slice();
                state.pending_size = state.pending_size.saturating_sub(data.len());
                let mut next_frag_offset = 0;
                let mut is_final = false;

                while !is_final {
                    let header = AuHeader {
                        // The size of the complete AU for all the fragments
                        size: Some(au.buffer.len() as u32),
                        // One AU fragment per packet
                        index: AccessUnitIndex::ZERO,
                        // CTS-delta SHOULD not be set for a fragment, see § 3.2.1.1
                        dts_delta: au.dts_delta,
                        maybe_random_access: au.maybe_random_access,
                        ..Default::default()
                    };

                    headers_buf.clear();
                    let mut w = BitWriter::endian(&mut headers_buf, BigEndian);
                    let mut res = w.build_with(&header, &ctx);
                    if res.is_ok() {
                        // add final padding
                        res = w.write::<7, u8>(0).map_err(Into::into);
                    }
                    if let Err(err) = res {
                        gst::error!(
                            CAT,
                            imp = self,
                            "Failed to write header for AU {} in buffer {}: {err:#}",
                            header.index,
                            au.id
                        );
                        return Err(gst::FlowError::Error);
                    }

                    // Unfortunately BitWriter doesn't return the size written.
                    let mut c = BitsWritten::<u32>::new();
                    c.build_with(&header, &ctx).unwrap();
                    let header_bit_len = c.written() as u16;

                    let left = au.buffer.len() - next_frag_offset;
                    let bytes_in_this_packet = std::cmp::min(
                        left,
                        max_payload_size - (header_bit_len as usize).div_ceil(8),
                    );

                    next_frag_offset += bytes_in_this_packet;
                    is_final = next_frag_offset >= au.buffer.len();

                    self.obj().queue_packet(
                        au.id.into(),
                        rtp_types::RtpPacketBuilder::new()
                            // AU-headers-length: only one 1 AU header here
                            .payload(header_bit_len.to_be_bytes().as_slice())
                            .payload(headers_buf.as_slice())
                            .payload(&data[0..bytes_in_this_packet])
                            .marker_bit(is_final),
                    )?;

                    data = &data[bytes_in_this_packet..];
                }

                continue;
            }

            // Will not fragment this AU

            // We optimistically add average size/duration to send out packets as early as possible
            // if we estimate that the next AU would likely overflow our accumulation limits.
            let n_aus = state.pending_aus.len();
            let avg_size = state.pending_size / n_aus;
            let avg_duration = state.pending_duration.opt_div(n_aus as u64);

            let max_ptime = settings
                .max_ptime
                .opt_min(state.max_ptime)
                .opt_min(state.ptime);

            let is_ready = send_mode == SendPacketMode::ForcePending
                || agg_mode != RtpMpeg4GenericPayAggregateMode::Aggregate
                || state.pending_size + avg_size + n_aus * (state.max_header_bit_len + 7) / 8
                    > max_payload_size
                || state
                    .pending_duration
                    .opt_add(avg_duration)
                    .opt_gt(max_ptime)
                    .unwrap_or(false);

            gst::log!(
                CAT,
                imp = self,
                "Pending: size {}, duration ~{:.3}, mode: {agg_mode:?} + {send_mode:?} => {}",
                state.pending_size,
                state.pending_duration.display(),
                if is_ready {
                    "ready"
                } else {
                    "not ready, waiting for more data"
                },
            );

            if !is_ready {
                break;
            }

            gst::trace!(CAT, imp = self, "Creating packet..");

            let id = front.id;
            let mut end_id = front.id;

            let mut acc_duration = gst::ClockTime::ZERO;
            let mut acc_size = 0;

            let mut headers_len = 0;

            let mut w = BitWriter::endian(&mut headers_buf, BigEndian);
            let mut index = AccessUnitIndex::ZERO;
            let mut previous_pts = None;

            au_data_list.clear();

            while let Some(front) = state.pending_aus.front() {
                gst::trace!(
                    CAT,
                    imp = self,
                    "{front:?}, accumulated size {acc_size} duration ~{acc_duration:.3}"
                );

                // If this AU would overflow the packet, bail out and send out what we have.
                //
                // Don't take into account the max_ptime for the first AU, since it could be
                // lower than the AU duration in which case we would never payload anything.
                //
                // For the size check in bytes we know that the first AU will fit the mtu,
                // because we already checked for the "AU needs to be fragmented" scenario above.

                let cts_delta = if ctx.prev_index.is_none() {
                    // No CTS-delta for the first AU in the packet
                    None
                } else {
                    ct_delta_to_rtp(front.pts, previous_pts, state.clock_rate).and_then(
                        |dts_delta_res| {
                            if dts_delta_res.is_none() {
                                gst::warning!(
                                    CAT,
                                    imp = self,
                                    "Overflow computing CTS-delta between pts {} & previous pts {}",
                                    front.pts.display(),
                                    previous_pts.display(),
                                );
                            }

                            dts_delta_res
                        },
                    )
                };

                previous_pts = front.pts;

                let header = AuHeader {
                    size: Some(front.buffer.len() as u32),
                    index,
                    cts_delta,
                    dts_delta: front.dts_delta,
                    maybe_random_access: front.maybe_random_access,
                    ..Default::default()
                };

                w.build_with(&header, &ctx).map_err(|err| {
                    gst::error!(
                        CAT,
                        imp = self,
                        "Failed to write header for AU {} in buffer {}: {err:#}",
                        header.index,
                        front.id,
                    );
                    gst::FlowError::Error
                })?;

                // Unfortunately BitWriter doesn't return the size written.
                let mut c = BitsWritten::<u32>::new();
                c.build_with(&header, &ctx).unwrap();
                let header_bit_len = c.written() as u16;

                if acc_size
                    + ((headers_len + header_bit_len) as usize).div_ceil(8)
                    + front.buffer.len()
                    > max_payload_size
                    || (ctx.prev_index.is_some()
                        && max_ptime
                            .opt_lt(acc_duration.opt_add(front.duration))
                            .unwrap_or(false))
                {
                    break;
                }

                let au = state.pending_aus.pop_front().unwrap();

                end_id = au.id;
                acc_size += au.buffer.len();
                acc_duration.opt_add_assign(au.duration);

                state.pending_size -= au.buffer.len();
                state.pending_duration.opt_saturating_sub(au.duration);

                headers_len += header_bit_len;
                au_data_list.push(au.buffer);

                ctx.prev_index = Some(index);
                index += 1;
            }

            // add final padding
            if let Err(err) = w.write::<7, u8>(0) {
                gst::error!(
                    CAT,
                    imp = self,
                    "Failed to write padding for final AU {} in buffer {end_id}: {err}",
                    ctx.prev_index.expect("at least one AU"),
                );
                return Err(gst::FlowError::Error);
            }

            let headers_len = headers_len.to_be_bytes();
            debug_assert_eq!(headers_len.len(), 2);

            let mut packet = rtp_types::RtpPacketBuilder::new()
                .marker_bit(true)
                .payload(headers_len.as_slice())
                .payload(headers_buf.as_slice());

            for au_data in &au_data_list {
                packet = packet.payload(au_data.as_slice());
            }

            self.obj()
                .queue_packet(PacketToBufferRelation::Ids(id..=end_id), packet)?;
        }

        gst::log!(
            CAT,
            imp = self,
            "All done for now, {} pending AUs",
            state.pending_aus.len()
        );

        if send_mode == SendPacketMode::ForcePending {
            self.obj().finish_pending_packets()?;
        }

        Ok(gst::FlowSuccess::Ok)
    }

    fn effective_aggregate_mode(&self, settings: &Settings) -> RtpMpeg4GenericPayAggregateMode {
        match settings.aggregate_mode {
            RtpMpeg4GenericPayAggregateMode::Auto => match self.is_live() {
                Some(true) => RtpMpeg4GenericPayAggregateMode::ZeroLatency,
                Some(false) => RtpMpeg4GenericPayAggregateMode::Aggregate,
                None => RtpMpeg4GenericPayAggregateMode::ZeroLatency,
            },
            mode => mode,
        }
    }

    fn is_live(&self) -> Option<bool> {
        *self.is_live.lock().unwrap()
    }

    // Query upstream live-ness if needed, in case of aggregate-mode=auto
    fn ensure_upstream_liveness(&self, settings: &mut Settings) {
        if settings.aggregate_mode != RtpMpeg4GenericPayAggregateMode::Auto
            || self.is_live().is_some()
        {
            return;
        }

        let mut q = gst::query::Latency::new();
        let is_live = if self.obj().sink_pad().peer_query(&mut q) {
            let (is_live, _, _) = q.result();
            is_live
        } else {
            false
        };

        *self.is_live.lock().unwrap() = Some(is_live);

        gst::info!(CAT, imp = self, "Upstream is live: {is_live}");
    }
}
