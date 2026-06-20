// Copyright (C) 2024, Asymptotic Inc.
//      Author: Sanchayan Maity <sanchayan@asymptotic.io>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/*
 * `hlsmultivariantsink` for supporting multi-variant playlist with alternate
 * renditions and variant streams. Builds on top of and requires `hlscmafsink`
 * and `hlssink3`.
 *
 * TODO:
 *
 * - Support for closed captions
 * - Support for WebVTT subtitles
 *
 * NOT SUPPORTED:
 *
 * - Muxed audio and video with alternate renditions
 * - Simple Media playlist. Use `hlssink3` for the same
 */

use crate::{
    HlsMultivariantSinkAlternativeMediaType, HlsMultivariantSinkMuxerType,
    HlsMultivariantSinkPlaylistType,
};
use gio::prelude::*;
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use m3u8_rs::{
    AlternativeMedia, AlternativeMediaType, MasterPlaylist, MediaPlaylistType, VariantStream,
};
use std::collections::HashMap;
use std::convert::From;
use std::fmt;
use std::fmt::Display;
use std::fs;
use std::fs::File;
use std::path;
use std::str::FromStr;
use std::sync::{LazyLock, Mutex};

const DEFAULT_AUTO_SELECT: bool = false;
const DEFAULT_FORCED: bool = false;
const DEFAULT_I_FRAMES_ONLY_PLAYLIST: bool = false;
const DEFAULT_IS_DEFAULT: bool = false;
const DEFAULT_MAX_NUM_SEGMENT_FILES: u32 = 10;
const DEFAULT_MUXER_TYPE: HlsMultivariantSinkMuxerType = HlsMultivariantSinkMuxerType::Cmaf;
const DEFAULT_PLAYLIST_LENGTH: u32 = 5;
const DEFAULT_PLAYLIST_TYPE: HlsMultivariantSinkPlaylistType =
    HlsMultivariantSinkPlaylistType::Unspecified;
const DEFAULT_SEND_KEYFRAME_REQUESTS: bool = true;
const DEFAULT_TARGET_DURATION: u32 = 15;
const DEFAULT_INIT_LOCATION: &str = "init%05d.mp4";
const DEFAULT_CMAF_LOCATION: &str = "segment%05d.m4s";
const DEFAULT_TS_LOCATION: &str = "segment%05d.ts";
const DEFAULT_MULTIVARIANT_PLAYLIST_LOCATION: &str = "multivariant.m3u8";

const SIGNAL_DELETE_FRAGMENT: &str = "delete-fragment";
const SIGNAL_GET_FRAGMENT_STREAM: &str = "get-fragment-stream";
const SIGNAL_GET_INIT_STREAM: &str = "get-init-stream";
const SIGNAL_GET_MULTIVARIANT_PLAYLIST_STREAM: &str = "get-multivariant-playlist-stream";
const SIGNAL_GET_PLAYLIST_STREAM: &str = "get-playlist-stream";

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "hlsmultivariantsink",
        gst::DebugColorFlags::empty(),
        Some("HLS sink"),
    )
});

impl From<HlsMultivariantSinkAlternativeMediaType> for AlternativeMediaType {
    fn from(media_type: HlsMultivariantSinkAlternativeMediaType) -> Self {
        match media_type {
            HlsMultivariantSinkAlternativeMediaType::Audio => AlternativeMediaType::Audio,
            HlsMultivariantSinkAlternativeMediaType::Video => AlternativeMediaType::Video,
        }
    }
}

impl From<AlternativeMediaType> for HlsMultivariantSinkAlternativeMediaType {
    fn from(value: AlternativeMediaType) -> Self {
        match value {
            AlternativeMediaType::Audio => HlsMultivariantSinkAlternativeMediaType::Audio,
            AlternativeMediaType::Video => HlsMultivariantSinkAlternativeMediaType::Video,
            AlternativeMediaType::ClosedCaptions => unimplemented!(),
            AlternativeMediaType::Subtitles => unimplemented!(),
            AlternativeMediaType::Other(_) => unimplemented!(),
        }
    }
}

impl FromStr for HlsMultivariantSinkAlternativeMediaType {
    type Err = String;

    fn from_str(s: &str) -> Result<HlsMultivariantSinkAlternativeMediaType, String> {
        match s {
            "AUDIO" => Ok(HlsMultivariantSinkAlternativeMediaType::Audio),
            "VIDEO" => Ok(HlsMultivariantSinkAlternativeMediaType::Video),
            "audio" => Ok(HlsMultivariantSinkAlternativeMediaType::Audio),
            "video" => Ok(HlsMultivariantSinkAlternativeMediaType::Video),
            _ => unimplemented!(),
        }
    }
}

impl Display for HlsMultivariantSinkAlternativeMediaType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                HlsMultivariantSinkAlternativeMediaType::Audio => "AUDIO",
                HlsMultivariantSinkAlternativeMediaType::Video => "VIDEO",
            }
        )
    }
}

impl Display for HlsMultivariantSinkPlaylistType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                HlsMultivariantSinkPlaylistType::Unspecified => "unspecified",
                HlsMultivariantSinkPlaylistType::Event => "event",
                HlsMultivariantSinkPlaylistType::Vod => "vod",
            }
        )
    }
}

impl From<HlsMultivariantSinkPlaylistType> for Option<MediaPlaylistType> {
    fn from(pl_type: HlsMultivariantSinkPlaylistType) -> Self {
        use HlsMultivariantSinkPlaylistType::*;
        match pl_type {
            Unspecified => None,
            Event => Some(MediaPlaylistType::Event),
            Vod => Some(MediaPlaylistType::Vod),
        }
    }
}

impl From<Option<&MediaPlaylistType>> for HlsMultivariantSinkPlaylistType {
    fn from(inner_pl_type: Option<&MediaPlaylistType>) -> Self {
        use HlsMultivariantSinkPlaylistType::*;
        match inner_pl_type {
            Some(MediaPlaylistType::Event) => Event,
            Some(MediaPlaylistType::Vod) => Vod,
            None | Some(MediaPlaylistType::Other(_)) => Unspecified,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct AlternateRendition {
    media_type: HlsMultivariantSinkAlternativeMediaType,
    /*
     * While the URI is optional for an alternate rendition when
     * the media type is audio or video, we keep it required here
     * because of the way we handle media as non-muxed and each
     * media having it's own HLS sink element downstream.
     *
     * We do not support muxed audio video for renditions.
     */
    uri: String,
    group_id: String,
    language: Option<String>,
    name: String,
    default: bool,
    autoselect: bool,
    forced: bool,
}

impl From<&AlternateRendition> for AlternativeMedia {
    fn from(rendition: &AlternateRendition) -> Self {
        Self {
            media_type: AlternativeMediaType::from(rendition.media_type),
            uri: Some(rendition.uri.clone()),
            group_id: rendition.group_id.clone(),
            language: rendition.language.clone(),
            name: rendition.name.clone(),
            default: rendition.default,
            autoselect: rendition.autoselect,
            forced: rendition.forced,
            ..Default::default()
        }
    }
}

impl From<gst::Structure> for AlternateRendition {
    fn from(s: gst::Structure) -> Self {
        AlternateRendition {
            media_type: s.get::<&str>("media_type").map_or(
                HlsMultivariantSinkAlternativeMediaType::Audio,
                |media| {
                    HlsMultivariantSinkAlternativeMediaType::from_str(media)
                        .expect("Failed to get media type")
                },
            ),
            uri: s.get("uri").expect("uri missing in alternate rendition"),
            group_id: s
                .get("group_id")
                .expect("group_id missing in alternate rendition"),
            language: s.get("language").unwrap_or(None),
            name: s.get("name").expect("name missing in alternate rendition"),
            default: s.get("default").unwrap_or(DEFAULT_IS_DEFAULT),
            autoselect: s.get("autoselect").unwrap_or(DEFAULT_AUTO_SELECT),
            forced: s.get("forced").unwrap_or(DEFAULT_FORCED),
        }
    }
}

impl From<AlternateRendition> for gst::Structure {
    fn from(obj: AlternateRendition) -> Self {
        gst::Structure::builder("pad-settings")
            .field("media_type", obj.media_type)
            .field("uri", obj.uri)
            .field("group_id", obj.group_id)
            .field("language", obj.language)
            .field("name", obj.name)
            .field("default", obj.default)
            .field("autoselect", obj.autoselect)
            .field("forced", obj.forced)
            .build()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Variant {
    /* No effect when using hlscmafsink which is the default */
    is_i_frame: bool,
    /*
     * For a variant to have muxed audio and video, set the URI on the
     * variant pad property of the audio and video pads to be the same.
     */
    uri: String,
    bandwidth: u64,
    codecs: Option<String>,
    audio: Option<String>,
    video: Option<String>,
}

impl From<gst::Structure> for Variant {
    fn from(s: gst::Structure) -> Self {
        Variant {
            is_i_frame: s
                .get("is-i-frame")
                .unwrap_or(DEFAULT_I_FRAMES_ONLY_PLAYLIST),
            uri: s.get("uri").expect("uri missing in variant stream"),
            bandwidth: s
                .get::<i32>("bandwidth")
                .expect("bandwidth missing in variant stream") as u64,
            audio: s.get("audio").unwrap_or(None),
            video: s.get("video").unwrap_or(None),
            codecs: s.get("codecs").unwrap_or(None),
        }
    }
}

impl From<Variant> for gst::Structure {
    fn from(obj: Variant) -> Self {
        gst::Structure::builder("variant-stream")
            .field("is-i-frame", obj.is_i_frame)
            .field("uri", obj.uri)
            .field("bandwidth", obj.bandwidth)
            .field("codecs", obj.codecs)
            .field("audio", obj.audio)
            .field("video", obj.video)
            .build()
    }
}

impl From<&Variant> for VariantStream {
    fn from(variant: &Variant) -> Self {
        Self {
            is_i_frame: variant.is_i_frame,
            uri: variant.uri.clone(),
            bandwidth: variant.bandwidth,
            codecs: variant.codecs.clone(),
            audio: variant.audio.clone(),
            video: variant.video.clone(),
            ..Default::default()
        }
    }
}

impl From<VariantStream> for Variant {
    fn from(value: VariantStream) -> Self {
        Self {
            is_i_frame: value.is_i_frame,
            uri: value.uri,
            bandwidth: value.bandwidth,
            codecs: value.codecs,
            audio: value.audio,
            video: value.video,
        }
    }
}

/* Helper functions */
fn accumulate_codec_caps(codecs: &mut HashMap<String, Vec<String>>, caps: String, id: String) {
    match codecs.get_mut(id.as_str()) {
        Some(ref mut v) => {
            v.push(caps);
            v.sort();
            /*
             * TODO: Should we move to itertools unique?
             *
             * It is possible to get multiple CAPS event on the pad. We
             * rely on writing multivariant playlist only after CAPS for
             * all the pads are known so that the codec string for variant
             * can be generated correctly before writing the playlist. In
             * case of multiple events, the count can be higher. If one
             * cap is a subset of the other drop the subset cap to prevent
             * this.
             */
            v.dedup();
        }
        None => {
            let vec = vec![caps];
            codecs.insert(id, vec);
        }
    }
}

fn build_codec_string_for_variant(
    variant: &Variant,
    codecs: &HashMap<String, Vec<String>>,
) -> Result<Option<String>, glib::BoolError> {
    /*
     * mpegtsmux only accepts stream-format as byte-stream for H264/H265.
     * The pbutils helper used relies on codec_data for figuring out the
     * profile and level information, however codec_data is absent in the
     * case of stream-format being byte-stream.
     *
     * If the profile and level information are missing from the codecs
     * field, clients like hls.js or Video.js which rely on browsers
     * built-in HLS support fail to load the media source. This can be
     * checked by running something like below in the browser console.
     *
     * MediaSource.isTypeSupported('video/mp4;codecs=avc1')
     * MediaSource.isTypeSupported('video/mp4;codecs=avc1.42000c')
     *
     * The first one will return a false.
     *
     * The use of `hlscmafsink` with the muxer type being CMAF by default
     * uses the `avc` stream-format where `codec_data` is included and
     * the get mime codec helper retrieves the codec string.
     *
     * If the user opts for MPEGTS, for H264/H265 we parse the in-band
     * SPS from buffer to retrieve the profile, constraint flags and
     * level information. See `sink_chain`.
     */
    let mut codecs_str: Vec<String> = vec![];

    if let Some(audio_group_id) = &variant.audio {
        if let Some(caps) = codecs.get(audio_group_id.as_str()) {
            for cap in caps.iter() {
                codecs_str.push(cap.to_string());
            }
        }
    }

    if let Some(video_group_id) = &variant.video {
        if let Some(caps) = codecs.get(video_group_id.as_str()) {
            for cap in caps.iter() {
                codecs_str.push(cap.to_string());
            }
        }
    }

    if let Some(caps) = codecs.get(variant.uri.as_str()) {
        for cap in caps.iter() {
            codecs_str.push(cap.to_string());
        }
    }

    match codecs_str.is_empty() {
        false => {
            codecs_str.sort();
            codecs_str.dedup();
            /*
             * Remove codec string without profile and level information
             * which will get added in the sink_event path.
             */
            codecs_str.retain(|codec_str| *codec_str != "avc1" && *codec_str != "avc3");
            codecs_str.retain(|codec_str| *codec_str != "hev1" && *codec_str != "hvc1");
            Ok(Some(codecs_str.join(",")))
        }
        true => Ok(None),
    }
}

fn build_codec_string_for_variants(state: &mut State) -> Result<(), glib::BoolError> {
    state.old_variants.clone_from(&state.variants);

    for variant in state.variants.iter_mut() {
        match build_codec_string_for_variant(variant, &state.codecs) {
            Ok(codec_str) => {
                variant.codecs = codec_str;
            }
            Err(e) => return Err(e),
        }
    }

    Ok(())
}

fn get_existing_hlssink_for_variant(
    elem: &HlsMultivariantSink,
    uri: String,
    muxer_type: HlsMultivariantSinkMuxerType,
) -> (bool, String, gst::Element) {
    let sink_name = hlssink_name(uri, muxer_type);

    match muxer_type {
        HlsMultivariantSinkMuxerType::Cmaf => {
            let hlssink = hlssink_element(muxer_type, sink_name.clone());
            (false, sink_name, hlssink)
        }
        HlsMultivariantSinkMuxerType::MpegTs => {
            if let Some(hlssink) = elem.obj().by_name(&sink_name) {
                (true, sink_name, hlssink)
            } else {
                let hlssink = hlssink_element(muxer_type, sink_name.clone());
                (false, sink_name, hlssink)
            }
        }
    }
}

fn hlssink_element(muxer_type: HlsMultivariantSinkMuxerType, sink_name: String) -> gst::Element {
    match muxer_type {
        HlsMultivariantSinkMuxerType::Cmaf => gst::ElementFactory::make("hlscmafsink")
            .name(sink_name)
            .build()
            .expect("hlscmafsink must be available"),
        HlsMultivariantSinkMuxerType::MpegTs => gst::ElementFactory::make("hlssink3")
            .name(sink_name)
            .build()
            .expect("hlssink3 must be available"),
    }
}

fn hlssink_name(uri: String, muxer_type: HlsMultivariantSinkMuxerType) -> String {
    match muxer_type {
        HlsMultivariantSinkMuxerType::Cmaf => format!("hlscmafsink-{uri}").to_string(),
        HlsMultivariantSinkMuxerType::MpegTs => format!("hlssink3-{uri}").to_string(),
    }
}

fn hlssink_pad(
    hlssink: &gst::Element,
    muxer_type: HlsMultivariantSinkMuxerType,
    is_video: bool,
) -> gst::Pad {
    match muxer_type {
        HlsMultivariantSinkMuxerType::Cmaf => hlssink
            .static_pad("sink")
            .expect("hlscmafsink always has a sink pad"),
        HlsMultivariantSinkMuxerType::MpegTs => match is_video {
            true => hlssink
                .request_pad_simple("video")
                .expect("hlssink3 always has a video pad"),
            false => hlssink
                .request_pad_simple("audio")
                .expect("hlssink3 always has a video pad"),
        },
    }
}

fn hlssink_setup_paths_absolute(
    pad: &HlsMultivariantSinkPad,
    hlssink: &gst::Element,
    muxer_type: HlsMultivariantSinkMuxerType,
) -> Result<(), gst::ErrorMessage> {
    let pad_settings = pad.settings.lock().unwrap();

    let playlist_location = pad_settings
        .playlist_location
        .clone()
        .ok_or(gst::error_msg!(
            gst::ResourceError::Failed,
            ["Media playlist location not provided for absolute path"]
        ))?;
    let segment_location = pad_settings
        .segment_location
        .clone()
        .ok_or(gst::error_msg!(
            gst::ResourceError::Failed,
            ["Media segment location not provided for absolute path"]
        ))?;

    gst::info!(
        CAT,
        imp = pad,
        "Segment playlist: {playlist_location}, segment: {segment_location}"
    );

    hlssink.set_property("playlist-location", playlist_location);
    hlssink.set_property("location", segment_location);

    if muxer_type == HlsMultivariantSinkMuxerType::Cmaf {
        let init_location = pad_settings
            .init_segment_location
            .clone()
            .ok_or(gst::error_msg!(
                gst::ResourceError::Failed,
                ["Init segment location not provided for absolute path"]
            ))?;
        gst::info!(CAT, imp = pad, "Init location: {init_location}");
        hlssink.set_property("init-location", init_location);
    }

    Ok(())
}

fn hlssink_setup_paths_relative(
    pad: &HlsMultivariantSinkPad,
    hlssink: &gst::Element,
    muxer_type: HlsMultivariantSinkMuxerType,
    multivariant_playlist_location: String,
    uri: String,
) -> Result<(), gst::ErrorMessage> {
    /*
     * This function is for the convenience path where segment and media
     * playlist paths are always considered to be relative to the
     * multivariant playlist along with the `uri`.
     *
     * If multivariant playlist has the path `/tmp/hlssink/multivariant.m3u8`,
     * in this case, a playlist path for a rendition or variant with an `uri`
     * like `hi-audio/audio.m3u8` will map to `/tmp/hlssink/hi-audio/audio.m3u8`.
     */
    let multivariant_playlist_parts: Vec<&str> =
        multivariant_playlist_location.split('/').collect();
    let segment_location = if multivariant_playlist_parts.len() == 1 {
        // Multi variant playlist will be written to the current directory
        uri
    } else {
        let multivariant_playlist_root =
            &multivariant_playlist_parts[..multivariant_playlist_parts.len() - 1].join("/");
        format!("{multivariant_playlist_root}/{uri}")
    };

    let parts: Vec<&str> = segment_location.split('/').collect();
    if parts.len() == 1 {
        gst::error!(
            CAT,
            imp = pad,
            "URI must be relative to multivariant playlist"
        );
        return Err(gst::error_msg!(
            gst::ResourceError::Failed,
            ["URI must be relative to multivariant playlist"]
        ));
    }

    let segment_playlist_root = &parts[..parts.len() - 1].join("/");

    hlssink.set_property("playlist-location", segment_location);

    match muxer_type {
        HlsMultivariantSinkMuxerType::Cmaf => {
            hlssink.set_property(
                "init-location",
                format!("{segment_playlist_root}/{DEFAULT_INIT_LOCATION}"),
            );
            hlssink.set_property(
                "location",
                format!("{segment_playlist_root}/{DEFAULT_CMAF_LOCATION}"),
            );
        }
        HlsMultivariantSinkMuxerType::MpegTs => {
            hlssink.set_property(
                "location",
                format!("{segment_playlist_root}/{DEFAULT_TS_LOCATION}"),
            );
        }
    }

    Ok(())
}

fn hlssink_setup_paths(
    pad: &HlsMultivariantSinkPad,
    hlssink: &gst::Element,
    muxer_type: HlsMultivariantSinkMuxerType,
    multivariant_playlist_location: String,
    uri: String,
) -> Result<(), gst::ErrorMessage> {
    let pad_settings = pad.settings.lock().unwrap();
    /*
     * If any of these pad properties are set, we assume that the user
     * wants to manually specify the absolute paths where the segment
     * and media playlists are to be written instead of using a relative
     * path to the multivariant playlist. Cannot have a mix of relative
     * and absolute path for any of these properties.
     *
     * An element error will be triggered if only one of these are set.
     */
    if pad_settings.playlist_location.is_some()
        || pad_settings.init_segment_location.is_some()
        || pad_settings.segment_location.is_some()
    {
        gst::info!(
            CAT,
            imp = pad,
            "Using provided absolute paths for media playlist and segments playlist:{:?} init_segment:{:?} segment: {:?}",
            pad_settings.playlist_location,
            pad_settings.init_segment_location,
            pad_settings.segment_location
        );

        drop(pad_settings);
        hlssink_setup_paths_absolute(pad, hlssink, muxer_type)
    } else {
        gst::info!(
            CAT,
            imp = pad,
            "Using relative paths for media playlist and segments"
        );

        drop(pad_settings);
        hlssink_setup_paths_relative(
            pad,
            hlssink,
            muxer_type,
            multivariant_playlist_location,
            uri,
        )
    }
}

/*
 * The EXT-X-MEDIA tag is used to relate Media Playlists that contain
 * alternative Renditions. An EXT-X-MEDIA tag must have TYPE of media.
 * We use the existence of the field to decide whether the user meant
 * a requested media to be an alternate rendition or a variant stream
 * by setting the corresponding property.
 */
fn is_alternate_rendition(s: &gst::Structure) -> bool {
    match s.get::<&str>("media_type") {
        Ok(s) => s == "AUDIO" || s == "VIDEO" || s == "audio" || s == "video",
        Err(_) => false,
    }
}
/* Helper functions end */

/*
 * A pad/media requested represents either an alternate rendition or
 * a variant stream.
 */
#[derive(Clone)]
enum HlsMultivariantSinkPadType {
    PadAlternative(AlternateRendition),
    PadVariant(Variant),
}

impl Default for HlsMultivariantSinkPadType {
    fn default() -> Self {
        HlsMultivariantSinkPadType::PadVariant(Variant::default())
    }
}

impl From<gst::Structure> for HlsMultivariantSinkPadType {
    fn from(s: gst::Structure) -> Self {
        match is_alternate_rendition(&s) {
            true => HlsMultivariantSinkPadType::PadAlternative(AlternateRendition::from(s)),
            false => HlsMultivariantSinkPadType::PadVariant(Variant::from(s)),
        }
    }
}

impl From<HlsMultivariantSinkPadType> for gst::Structure {
    fn from(obj: HlsMultivariantSinkPadType) -> Self {
        match obj {
            HlsMultivariantSinkPadType::PadAlternative(a) => Into::<gst::Structure>::into(a),
            HlsMultivariantSinkPadType::PadVariant(v) => Into::<gst::Structure>::into(v),
        }
    }
}

#[derive(Clone, Default)]
struct HlsMultivariantSinkPadSettings {
    sink: Option<gst::Element>,
    pad_type: HlsMultivariantSinkPadType,
    playlist_location: Option<String>,
    init_segment_location: Option<String>,
    segment_location: Option<String>,
}

#[derive(Default)]
pub(crate) struct HlsMultivariantSinkPad {
    settings: Mutex<HlsMultivariantSinkPadSettings>,
}

#[glib::object_subclass]
impl ObjectSubclass for HlsMultivariantSinkPad {
    const NAME: &'static str = "HlsMultivariantSinkPad";
    type Type = super::HlsMultivariantSinkPad;
    type ParentType = gst::GhostPad;
}

impl ObjectImpl for HlsMultivariantSinkPad {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecBoxed::builder::<gst::Structure>("alternate-rendition")
                    .nick("Rendition")
                    .blurb("Alternate Rendition")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Structure>("variant")
                    .nick("Variant")
                    .blurb("Variant Stream")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("playlist-location")
                    .nick("Playlist location")
                    .blurb("Location of the media playlist to write")
                    .build(),
                glib::ParamSpecString::builder("init-segment-location")
                    .nick("Init segment location")
                    .blurb("Location of the init segment file to write for CMAF")
                    .build(),
                glib::ParamSpecString::builder("segment-location")
                    .nick("Segment location")
                    .blurb("Location of the media segment file to write")
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "alternate-rendition" => {
                let s = value
                    .get::<gst::Structure>()
                    .expect("Must be a valid AlternateRendition");
                let rendition = AlternateRendition::from(s);
                let alternative_media = AlternativeMedia::from(&rendition);

                gst::info!(
                    CAT,
                    imp = self,
                    "Setting alternate rendition: {rendition:?}"
                );

                let parent = self.parent();
                let elem = parent.imp();
                let elem_settings = elem.settings.lock().unwrap();
                let muxer_type = elem_settings.muxer_type;

                let mut state = elem.state.lock().unwrap();

                let obj = self.obj();
                let pad_name = obj.name();

                let is_video = pad_name.contains("video");
                let sink_name = hlssink_name(rendition.uri.clone(), muxer_type);

                let hlssink = hlssink_element(muxer_type, sink_name.clone());
                let peer_pad = hlssink_pad(&hlssink, muxer_type, is_video);

                elem.setup_hlssink(&hlssink, &elem_settings);

                if let Err(e) = hlssink_setup_paths(
                    self,
                    &hlssink,
                    muxer_type,
                    elem_settings.multivariant_playlist_location.clone(),
                    rendition.uri.clone(),
                ) {
                    gst::element_error!(
                        parent,
                        gst::ResourceError::Settings,
                        ["Failed to setup HLS paths {e:?}"]
                    );
                    return;
                }

                parent
                    .add(&hlssink)
                    .expect("Failed to add hlssink for rendition");

                self.obj()
                    .set_target(Some(&peer_pad))
                    .expect("Failed to set target for rendition");

                self.obj()
                    .set_active(true)
                    .expect("Failed to activate rendition pad");

                state.pads.insert(pad_name.to_string(), sink_name);
                state.alternatives.push(alternative_media);

                drop(elem_settings);
                drop(state);

                let mut settings = self.settings.lock().unwrap();
                settings.pad_type = HlsMultivariantSinkPadType::PadAlternative(rendition);
                settings.sink = Some(hlssink);
            }
            "variant" => {
                let s = value
                    .get::<gst::Structure>()
                    .expect("Must be a valid Variant");
                let variant = Variant::from(s);

                gst::info!(CAT, imp = self, "Setting variant: {variant:?}");

                let parent = self.parent();
                let elem = parent.imp();
                let elem_settings = elem.settings.lock().unwrap();
                let muxer_type = elem_settings.muxer_type;

                let mut state = elem.state.lock().unwrap();

                let obj = self.obj();
                let pad_name = obj.name();

                let is_video = pad_name.contains("video");

                /*
                 * If the variant is to have muxed audio and video, look for
                 * a hlssink with the same URI.
                 */
                let (muxed, sink_name, hlssink) = get_existing_hlssink_for_variant(
                    elem,
                    variant.uri.clone(),
                    elem_settings.muxer_type,
                );
                let peer_pad = hlssink_pad(&hlssink, muxer_type, is_video);

                if !muxed {
                    elem.setup_hlssink(&hlssink, &elem_settings);

                    if let Err(e) = hlssink_setup_paths(
                        self,
                        &hlssink,
                        muxer_type,
                        elem_settings.multivariant_playlist_location.clone(),
                        variant.uri.clone(),
                    ) {
                        gst::element_error!(
                            parent,
                            gst::ResourceError::Settings,
                            ["Failed to setup HLS paths {e:?}"]
                        );
                        return;
                    }

                    parent
                        .add(&hlssink)
                        .expect("Failed to add hlssink for variant");

                    state.variants.push(variant.clone());
                }

                if muxer_type == HlsMultivariantSinkMuxerType::MpegTs
                    && is_video
                    && variant.is_i_frame
                {
                    hlssink.set_property("i-frames-only", true);
                }

                self.obj()
                    .set_target(Some(&peer_pad))
                    .expect("Failed to set target for variant");

                self.obj()
                    .set_active(true)
                    .expect("Failed to activate variant pad");

                state.pads.insert(pad_name.to_string(), sink_name);

                drop(elem_settings);
                drop(state);

                let mut settings = self.settings.lock().unwrap();
                settings.pad_type = HlsMultivariantSinkPadType::PadVariant(variant);
                settings.sink = Some(hlssink);
            }
            "playlist-location" => {
                let mut settings = self.settings.lock().unwrap();
                settings.playlist_location = value
                    .get::<Option<String>>()
                    .expect("type checked upstream");

                if let Some(ref sink) = settings.sink {
                    sink.set_property("playlist-location", settings.playlist_location.clone());
                }
            }
            "init-segment-location" => {
                let mut settings = self.settings.lock().unwrap();
                settings.init_segment_location = value
                    .get::<Option<String>>()
                    .expect("type checked upstream");

                if let Some(ref sink) = settings.sink {
                    /* See hlssink_name() */
                    if sink.name().contains("hlscmafsink-") {
                        sink.set_property("init-location", settings.init_segment_location.clone());
                    }
                }
            }
            "segment-location" => {
                let mut settings = self.settings.lock().unwrap();
                settings.segment_location = value
                    .get::<Option<String>>()
                    .expect("type checked upstream");

                if let Some(ref sink) = settings.sink {
                    sink.set_property("location", settings.segment_location.clone());
                }
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().unwrap();

        match pspec.name() {
            "alternate-rendition" | "variant" => {
                Into::<gst::Structure>::into(settings.pad_type.clone()).to_value()
            }
            "playlist-location" => settings.playlist_location.to_value(),
            "init-segment-location" => settings.init_segment_location.to_value(),
            "segment-location" => settings.segment_location.to_value(),
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for HlsMultivariantSinkPad {}

impl PadImpl for HlsMultivariantSinkPad {}

impl ProxyPadImpl for HlsMultivariantSinkPad {}

impl GhostPadImpl for HlsMultivariantSinkPad {}

impl HlsMultivariantSinkPad {
    fn parent(&self) -> super::HlsMultivariantSink {
        self.obj()
            .parent()
            .map(|elem_obj| {
                elem_obj
                    .downcast::<super::HlsMultivariantSink>()
                    .expect("Wrong Element type")
            })
            .expect("Pad should have a parent at this stage")
    }
}

#[derive(Default)]
struct State {
    audio_pad_serial: u32,
    video_pad_serial: u32,
    pads: HashMap<String, String>,
    alternatives: Vec<AlternativeMedia>,
    variants: Vec<Variant>,
    old_variants: Vec<Variant>,
    codecs: HashMap<String, Vec<String>>,
    wrote_manifest: bool,
}

#[derive(Debug)]
struct Settings {
    multivariant_playlist_location: String,
    muxer_type: HlsMultivariantSinkMuxerType,
    /* Below settings will be applied to all underlying hlscmafsink/hlssink3 */
    playlist_length: u32,
    playlist_type: Option<HlsMultivariantSinkPlaylistType>,
    max_num_segment_files: usize,
    send_keyframe_requests: bool,
    target_duration: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            multivariant_playlist_location: DEFAULT_MULTIVARIANT_PLAYLIST_LOCATION.to_string(),
            playlist_length: DEFAULT_PLAYLIST_LENGTH,
            playlist_type: Some(DEFAULT_PLAYLIST_TYPE),
            max_num_segment_files: DEFAULT_MAX_NUM_SEGMENT_FILES as usize,
            send_keyframe_requests: DEFAULT_SEND_KEYFRAME_REQUESTS,
            target_duration: DEFAULT_TARGET_DURATION,
            muxer_type: DEFAULT_MUXER_TYPE,
        }
    }
}

#[derive(Default)]
pub struct HlsMultivariantSink {
    settings: Mutex<Settings>,
    state: Mutex<State>,
}

#[glib::object_subclass]
impl ObjectSubclass for HlsMultivariantSink {
    const NAME: &'static str = "GstHlsMultivariantSink";
    type Type = super::HlsMultivariantSink;
    type ParentType = gst::Bin;
    type Interfaces = (gst::ChildProxy,);
}

impl ObjectImpl for HlsMultivariantSink {
    fn constructed(&self) {
        self.parent_constructed();
    }

    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecString::builder("multivariant-playlist-location")
                    .nick("multivariant Playlist file location")
                    .blurb("Location of the multivariant playlist file to write")
                    .default_value(Some(DEFAULT_MULTIVARIANT_PLAYLIST_LOCATION))
                    .build(),
                glib::ParamSpecUInt::builder("max-files")
                    .nick("Max files")
                    .blurb("Maximum number of files to keep on disk. Once the maximum is reached, old files start to be deleted to make room for new ones.")
                    .build(),
                glib::ParamSpecEnum::builder_with_default("muxer-type", DEFAULT_MUXER_TYPE)
                    .nick("Muxer Type")
                    .blurb("The muxer to use, cmafmux or mpegtsmux, accordingly selects hlssink3 or hlscmafsink")
                    .build(),
                glib::ParamSpecUInt::builder("playlist-length")
                    .nick("Playlist length")
                    .blurb("Length of HLS playlist. To allow players to conform to section 6.3.3 of the HLS specification, this should be at least 3. If set to 0, the playlist will be infinite.")
                    .default_value(DEFAULT_PLAYLIST_LENGTH)
                    .build(),
                glib::ParamSpecEnum::builder_with_default("playlist-type", DEFAULT_PLAYLIST_TYPE)
                    .nick("Playlist Type")
                    .blurb("The type of the playlist to use. When VOD type is set, the playlist will be live until the pipeline ends execution.")
                    .build(),
                glib::ParamSpecBoolean::builder("send-keyframe-requests")
                    .nick("Send Keyframe Requests")
                    .blurb("Send keyframe requests to ensure correct fragmentation. If this is disabled then the input must have keyframes in regular intervals.")
                    .default_value(DEFAULT_SEND_KEYFRAME_REQUESTS)
                    .build(),
                glib::ParamSpecUInt::builder("target-duration")
                    .nick("Target duration")
                    .blurb("The target duration in seconds of a segment/file. (0 - disabled, useful for management of segment duration by the streaming server)")
                    .default_value(DEFAULT_TARGET_DURATION)
                    .build(),
            ]
        });

        &PROPERTIES
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().expect("Failed to get settings lock");

        gst::debug!(
            CAT,
            imp = self,
            "Setting property '{}' to '{:?}'",
            pspec.name(),
            value
        );

        match pspec.name() {
            "multivariant-playlist-location" => {
                settings.multivariant_playlist_location =
                    value.get::<String>().expect("type checked upstream");
            }
            "max-files" => {
                let max_files: u32 = value.get().expect("type checked upstream");
                settings.max_num_segment_files = max_files as usize;
            }
            "muxer-type" => {
                settings.muxer_type = value
                    .get::<HlsMultivariantSinkMuxerType>()
                    .expect("type checked upstream");
            }
            "playlist-length" => {
                settings.playlist_length = value.get().expect("type checked upstream");
            }
            "playlist-type" => {
                settings.playlist_type = value
                    .get::<HlsMultivariantSinkPlaylistType>()
                    .expect("type checked upstream")
                    .into();
            }
            "target-duration" => {
                settings.target_duration = value.get().expect("type checked upstream");
            }

            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().expect("Failed to get settings lock");

        match pspec.name() {
            "multivariant-playlist-location" => settings.multivariant_playlist_location.to_value(),
            "max-files" => {
                let max_files = settings.max_num_segment_files as u32;
                max_files.to_value()
            }
            "muxer-type" => settings.muxer_type.to_value(),
            "playlist-length" => settings.playlist_length.to_value(),
            "playlist-type" => settings
                .playlist_type
                .unwrap_or(DEFAULT_PLAYLIST_TYPE)
                .to_value(),
            "send-keyframe-requests" => settings.send_keyframe_requests.to_value(),
            "target-duration" => settings.target_duration.to_value(),
            _ => unimplemented!(),
        }
    }

    fn signals() -> &'static [glib::subclass::Signal] {
        static SIGNALS: LazyLock<Vec<glib::subclass::Signal>> = LazyLock::new(|| {
            vec![
                glib::subclass::Signal::builder(SIGNAL_GET_MULTIVARIANT_PLAYLIST_STREAM)
                    .param_types([String::static_type()])
                    .return_type::<Option<gio::OutputStream>>()
                    .class_handler(|args| {
                        let multivariant_playlist_location =
                            args[1].get::<String>().expect("signal arg");
                        let elem = args[0]
                            .get::<super::HlsMultivariantSink>()
                            .expect("signal arg");
                        let imp = elem.imp();

                        Some(
                            imp.new_file_stream(&multivariant_playlist_location)
                                .ok()
                                .to_value(),
                        )
                    })
                    .accumulator(|_hint, _ret, value| {
                        /* First signal handler wins */
                        std::ops::ControlFlow::Break(value.clone())
                    })
                    .build(),
                /* We will proxy the below signals from the underlying hlssink3/hlscmafsink */
                glib::subclass::Signal::builder(SIGNAL_DELETE_FRAGMENT)
                    .param_types([String::static_type()])
                    .return_type::<bool>()
                    .class_handler(|args| {
                        let fragment_location = args[1].get::<String>().expect("signal arg");
                        let elem = args[0]
                            .get::<super::HlsMultivariantSink>()
                            .expect("signal arg");
                        let imp = elem.imp();

                        imp.delete_fragment(&fragment_location);
                        Some(true.to_value())
                    })
                    .accumulator(|_hint, _ret, value| {
                        // First signal handler wins
                        std::ops::ControlFlow::Break(value.clone())
                    })
                    .build(),
                glib::subclass::Signal::builder(SIGNAL_GET_FRAGMENT_STREAM)
                    .param_types([String::static_type()])
                    .return_type::<Option<gio::OutputStream>>()
                    .class_handler(|args| {
                        let fragment_location = args[1].get::<String>().expect("signal arg");
                        let elem = args[0]
                            .get::<super::HlsMultivariantSink>()
                            .expect("signal arg");
                        let imp = elem.imp();

                        Some(imp.new_file_stream(&fragment_location).ok().to_value())
                    })
                    .accumulator(|_hint, _ret, value| {
                        // First signal handler wins
                        std::ops::ControlFlow::Break(value.clone())
                    })
                    .build(),
                glib::subclass::Signal::builder(SIGNAL_GET_INIT_STREAM)
                    .param_types([String::static_type()])
                    .return_type::<Option<gio::OutputStream>>()
                    .class_handler(|args| {
                        let elem = args[0]
                            .get::<super::HlsMultivariantSink>()
                            .expect("signal arg");
                        let init_location = args[1].get::<String>().expect("signal arg");
                        let imp = elem.imp();

                        Some(imp.new_file_stream(&init_location).ok().to_value())
                    })
                    .accumulator(|_hint, _ret, value| {
                        // First signal handler wins
                        std::ops::ControlFlow::Break(value.clone())
                    })
                    .build(),
                glib::subclass::Signal::builder(SIGNAL_GET_PLAYLIST_STREAM)
                    .param_types([String::static_type()])
                    .return_type::<Option<gio::OutputStream>>()
                    .class_handler(|args| {
                        let playlist_location = args[1].get::<String>().expect("signal arg");
                        let elem = args[0]
                            .get::<super::HlsMultivariantSink>()
                            .expect("signal arg");
                        let imp = elem.imp();

                        Some(imp.new_file_stream(&playlist_location).ok().to_value())
                    })
                    .accumulator(|_hint, _ret, value| {
                        // First signal handler wins
                        std::ops::ControlFlow::Break(value.clone())
                    })
                    .build(),
            ]
        });

        SIGNALS.as_ref()
    }
}

impl GstObjectImpl for HlsMultivariantSink {}

impl ElementImpl for HlsMultivariantSink {
    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        if transition == gst::StateChange::ReadyToPaused {
            let state = self.state.lock().unwrap();

            gst::debug!(
                CAT,
                imp = self,
                "Validating alternate rendition and variants"
            );

            if !self.validate_alternate_rendition_and_variants(&state.alternatives, &state.variants)
            {
                gst::element_error!(
                    self.obj(),
                    gst::ResourceError::Settings,
                    ["Validation of alternate rendition and variants failed"]
                );
                return Err(gst::StateChangeError);
            }

            drop(state);
        }

        self.parent_change_state(transition)
    }

    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "HTTP Live Streaming sink",
                "Sink/Muxer",
                "HTTP Live Streaming sink",
                "Sanchayan Maity <sanchayan@asymptotic.io>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps_any = gst::Caps::new_any();

            let audio_pad_template = gst::PadTemplate::with_gtype(
                "audio_%u",
                gst::PadDirection::Sink,
                gst::PadPresence::Request,
                &caps_any,
                super::HlsMultivariantSinkPad::static_type(),
            )
            .unwrap();

            let video_pad_template = gst::PadTemplate::with_gtype(
                "video_%u",
                gst::PadDirection::Sink,
                gst::PadPresence::Request,
                &caps_any,
                super::HlsMultivariantSinkPad::static_type(),
            )
            .unwrap();

            vec![audio_pad_template, video_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn request_new_pad(
        &self,
        templ: &gst::PadTemplate,
        _name: Option<&str>,
        _caps: Option<&gst::Caps>,
    ) -> Option<gst::Pad> {
        match templ.name_template() {
            "audio_%u" => {
                let mut state = self.state.lock().unwrap();

                let audio_pad_name = format!("audio_{}", state.audio_pad_serial);
                let sink_pad =
                    gst::PadBuilder::<super::HlsMultivariantSinkPad>::from_template(templ)
                        .name(audio_pad_name.clone())
                        .event_function(|pad, parent, event| {
                            HlsMultivariantSink::catch_panic_pad_function(
                                parent,
                                || false,
                                |this| this.sink_event(pad, event),
                            )
                        })
                        .flags(gst::PadFlags::FIXED_CAPS)
                        .build();

                state.audio_pad_serial += 1;

                self.obj()
                    .add_pad(&sink_pad)
                    .expect("Failed to add audio pad");

                drop(state);

                self.obj()
                    .child_added(sink_pad.upcast_ref::<gst::Object>(), &sink_pad.name());

                Some(sink_pad.upcast())
            }
            "video_%u" => {
                let mut state = self.state.lock().unwrap();

                let video_pad_name = format!("video_{}", state.video_pad_serial);
                let sink_pad =
                    gst::PadBuilder::<super::HlsMultivariantSinkPad>::from_template(templ)
                        .name(video_pad_name.clone())
                        .chain_function(|pad, parent, buffer| {
                            HlsMultivariantSink::catch_panic_pad_function(
                                parent,
                                || Err(gst::FlowError::Error),
                                |this| this.sink_chain(pad, buffer),
                            )
                        })
                        .event_function(|pad, parent, event| {
                            HlsMultivariantSink::catch_panic_pad_function(
                                parent,
                                || false,
                                |this| this.sink_event(pad, event),
                            )
                        })
                        .flags(gst::PadFlags::FIXED_CAPS)
                        .build();

                state.video_pad_serial += 1;

                self.obj()
                    .add_pad(&sink_pad)
                    .expect("Failed to add video pad");

                drop(state);

                self.obj()
                    .child_added(sink_pad.upcast_ref::<gst::Object>(), &sink_pad.name());

                Some(sink_pad.upcast())
            }
            other_name => {
                gst::warning!(
                    CAT,
                    imp = self,
                    "requested_new_pad: name \"{}\" is not one of audio, video",
                    other_name
                );
                None
            }
        }
    }

    fn release_pad(&self, pad: &gst::Pad) {
        pad.set_active(false).unwrap();

        let ghost_pad = pad.downcast_ref::<gst::GhostPad>().unwrap();
        let pad_name = ghost_pad.name().to_string();

        let mut state = self.state.lock().unwrap();
        if let Some(hlssink_name) = state.pads.get(&pad_name.to_string()) {
            if let Some(hlssink) = self.obj().by_name(hlssink_name) {
                if let Err(err) = self.obj().remove(&hlssink) {
                    gst::error!(
                        CAT,
                        imp = self,
                        "Failed to remove hlssink for pad: {} with error: {}",
                        pad_name,
                        err
                    );
                }
                state.pads.remove(&pad_name);
            }
        }

        self.obj().remove_pad(pad).expect("Failed to remove pad");
    }
}

impl BinImpl for HlsMultivariantSink {
    fn handle_message(&self, message: gst::Message) {
        use gst::MessageView;
        match message.view() {
            MessageView::Eos(eos) => {
                gst::debug!(CAT, imp = self, "Got EOS from {:?}", eos.src());
                self.parent_handle_message(message)
            }
            MessageView::Error(err) => {
                gst::error!(
                    CAT,
                    imp = self,
                    "Got error: {} {:?}",
                    err.error(),
                    err.debug()
                );
                self.parent_handle_message(message)
            }
            _ => self.parent_handle_message(message),
        }
    }
}

impl ChildProxyImpl for HlsMultivariantSink {
    fn children_count(&self) -> u32 {
        let object = self.obj();
        object.num_pads() as u32
    }

    fn child_by_name(&self, name: &str) -> Option<glib::Object> {
        let object = self.obj();
        object
            .pads()
            .into_iter()
            .find(|p| p.name() == name)
            .map(|p| p.upcast())
    }

    fn child_by_index(&self, index: u32) -> Option<glib::Object> {
        let object = self.obj();
        object
            .pads()
            .into_iter()
            .nth(index as usize)
            .map(|p| p.upcast())
    }
}

impl HlsMultivariantSink {
    fn build_codec_str_and_write_multivariant_playlist(&self, codec_str: String, group_id: String) {
        let mut state = self.state.lock().unwrap();

        accumulate_codec_caps(&mut state.codecs, codec_str.clone(), group_id.clone());

        if let Err(e) = build_codec_string_for_variants(&mut state) {
            gst::error!(
                CAT,
                imp = self,
                "Failed to build codec string with error: {}",
                e
            );
            gst::element_error!(
                self.obj(),
                gst::ResourceError::Failed,
                ["Failed to build codec string with error: {}", e]
            );
        }

        if state.old_variants != state.variants {
            self.write_multivariant_playlist(&mut state);
        }

        drop(state);
    }

    fn parse_h264_sps(&self, buffer: &[u8], group_id: String) {
        use crate::cros_codecs::h264::parser as H264Parser;
        use std::io::Cursor;

        let mut cursor = Cursor::new(buffer);
        let mut parser = H264Parser::Parser::default();

        while let Ok(nalu) = H264Parser::Nalu::next(&mut cursor) {
            if let H264Parser::NaluType::Sps = nalu.header.type_ {
                let sps = parser.parse_sps(&nalu).unwrap();

                let profile = sps.profile_idc;
                let level = sps.level_idc as u8;
                let flags = [
                    sps.constraint_set0_flag,
                    sps.constraint_set1_flag,
                    sps.constraint_set2_flag,
                    sps.constraint_set3_flag,
                    sps.constraint_set4_flag,
                    sps.constraint_set5_flag,
                    // Reserve zero two bits
                    false,
                    false,
                ];

                let mut constraint_flags = 0;
                flags.iter().enumerate().for_each(|(index, bit)| {
                    constraint_flags += 2u32.pow(7 - index as u32) * (*bit) as u32;
                });

                let codec_str = format!("avc1.{profile:02X}{constraint_flags:02X}{level:02X}");

                self.build_codec_str_and_write_multivariant_playlist(codec_str, group_id);

                break;
            }
        }
    }

    fn parse_h265_sps(&self, buffer: &[u8], group_id: String) {
        use crate::cros_codecs::h265::parser as H265Parser;
        use std::io::Cursor;

        let mut cursor = Cursor::new(buffer);
        let mut parser = H265Parser::Parser::default();

        while let Ok(nalu) = H265Parser::Nalu::next(&mut cursor) {
            if let H265Parser::NaluType::SpsNut = nalu.header.type_ {
                let sps = parser.parse_sps(&nalu).unwrap();
                let profile_tier_level = &sps.profile_tier_level;

                /* Adapted from hevc_get_mime_codec() in codec-utils.c */
                let mut codec_str = "hvc1".to_owned();
                let profile_space = profile_tier_level.general_profile_space;
                if profile_space != 0 {
                    codec_str.push_str(&(65 + profile_space - 1).to_string());
                }

                let tier_flag = if profile_tier_level.general_tier_flag {
                    'H'
                } else {
                    'L'
                };
                let profile_idc = profile_tier_level.general_profile_idc;
                let level_idc = profile_tier_level.general_level_idc as u16;
                let compatibility_flag = profile_tier_level.general_profile_compatibility_flag;

                let mut compat_flags = 0;
                compatibility_flag
                    .iter()
                    .enumerate()
                    .for_each(|(index, bit)| {
                        compat_flags += 2u32.pow(31 - index as u32) * (*bit) as u32;
                    });

                compat_flags =
                    ((compat_flags & 0xaaaaaaaa) >> 1) | ((compat_flags & 0x55555555) << 1);
                compat_flags =
                    ((compat_flags & 0xcccccccc) >> 2) | ((compat_flags & 0x33333333) << 2);
                compat_flags =
                    ((compat_flags & 0xf0f0f0f0) >> 4) | ((compat_flags & 0x0f0f0f0f) << 4);
                compat_flags =
                    ((compat_flags & 0xff00ff00) >> 8) | ((compat_flags & 0x00ff00ff) << 8);
                let compat_flag_parameter = compat_flags.rotate_left(16);

                let constraint_flags = [
                    profile_tier_level.general_progressive_source_flag,
                    profile_tier_level.general_interlaced_source_flag,
                    profile_tier_level.general_non_packed_constraint_flag,
                    profile_tier_level.general_frame_only_constraint_flag,
                    profile_tier_level.general_max_12bit_constraint_flag,
                    profile_tier_level.general_max_10bit_constraint_flag,
                    profile_tier_level.general_max_8bit_constraint_flag,
                    profile_tier_level.general_max_422chroma_constraint_flag,
                ];

                let mut constraint_indicator_flag = 0;
                constraint_flags
                    .iter()
                    .enumerate()
                    .for_each(|(index, bit)| {
                        constraint_indicator_flag += 2u32.pow(7 - index as u32) * (*bit) as u32;
                    });

                let codec_str = format!("{codec_str}.{profile_idc:X}.{compat_flag_parameter}.{tier_flag}{level_idc}.{constraint_indicator_flag:02X}");

                self.build_codec_str_and_write_multivariant_playlist(codec_str, group_id);

                break;
            }
        }
    }

    fn parse_sps(
        &self,
        caps: &gst::Caps,
        buffer: &gst::Buffer,
        group_id: String,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let s = caps.structure(0).ok_or(gst::FlowError::Error)?;

        if s.name().as_str() == "video/x-h264" {
            let map = buffer.map_readable().map_err(|_| {
                gst::element_imp_error!(
                    self,
                    gst::ResourceError::Read,
                    ["Failed to map buffer readable"]
                );

                gst::FlowError::Error
            })?;

            self.parse_h264_sps(map.as_slice(), group_id.clone());
        }

        if s.name().as_str() == "video/x-h265" {
            let map = buffer.map_readable().map_err(|_| {
                gst::element_imp_error!(
                    self,
                    gst::ResourceError::Read,
                    ["Failed to map buffer readable"]
                );

                gst::FlowError::Error
            })?;

            self.parse_h265_sps(map.as_slice(), group_id);
        }

        Ok(gst::FlowSuccess::Ok)
    }

    fn sink_chain(
        &self,
        hlspad: &super::HlsMultivariantSinkPad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        /*
         * MPEG-TS does byte-stream for H264/H265 which does not have
         * codec_data as the relevant information is carried in-band
         * with SPS. codec-utils helpers cannot give us codec string
         * without the codec_data. For MPEG-TS, parse SPS and figure
         * out the relevant information to generate codec string.
         */
        {
            let settings = self.settings.lock().unwrap();
            let is_mpegts = settings.muxer_type == HlsMultivariantSinkMuxerType::MpegTs;
            drop(settings);

            if is_mpegts && buffer.flags().contains(gst::BufferFlags::HEADER) {
                let pad_settings = hlspad.imp().settings.lock().unwrap().to_owned();
                let group_id = match pad_settings.pad_type {
                    HlsMultivariantSinkPadType::PadAlternative(ref a) => a.group_id.clone(),
                    HlsMultivariantSinkPadType::PadVariant(ref v) => {
                        if let Some(group_id) = &v.video {
                            group_id.clone()
                        } else if let Some(group_id) = &v.audio {
                            group_id.clone()
                        } else {
                            v.uri.clone()
                        }
                    }
                };

                if let Some(caps) = hlspad.current_caps() {
                    self.parse_sps(&caps, &buffer, group_id)?;
                }
            }
        }

        hlspad
            .target()
            .expect("HlsMultivariantSinkPad must have a target")
            .chain(buffer)
    }

    fn sink_event(&self, hlspad: &super::HlsMultivariantSinkPad, event: gst::Event) -> bool {
        let pad = hlspad.upcast_ref::<gst::Pad>();

        gst::log!(CAT, obj = pad, "Handling event {event:?}");

        if let gst::EventView::Caps(ev) = event.view() {
            let caps = ev.caps();
            let codec_str = match gst_pbutils::codec_utils_caps_get_mime_codec(caps) {
                Ok(codec_str) => codec_str.to_string(),
                Err(e) => {
                    gst::element_error!(
                        self.obj(),
                        gst::ResourceError::Failed,
                        [
                            "Failed to build codec string for caps {:?} with error: {}",
                            caps,
                            e
                        ]
                    );

                    return false;
                }
            };

            /*
             * Keep track of caps for every pad. Depending on whether a
             * requested pad/media is an alternate rendition or variant
             * stream, track the caps as per group id.
             */
            let pad_settings = hlspad.imp().settings.lock().unwrap().to_owned();
            let group_id = match pad_settings.pad_type {
                HlsMultivariantSinkPadType::PadAlternative(ref a) => a.group_id.clone(),
                HlsMultivariantSinkPadType::PadVariant(ref v) => {
                    if let Some(group_id) = &v.video {
                        group_id.clone()
                    } else if let Some(group_id) = &v.audio {
                        group_id.clone()
                    } else {
                        /*
                         * Variant streams which do not have AUDIO or VIDEO
                         * set and thus are not associated with any rendition
                         * groups, are tracked via their URI.
                         */
                        v.uri.clone()
                    }
                }
            };
            drop(pad_settings);

            self.build_codec_str_and_write_multivariant_playlist(codec_str, group_id);

            gst::debug!(
                CAT,
                imp = self,
                "Received caps {:?} on pad: {}",
                caps,
                pad.name()
            );
        }

        gst::Pad::event_default(pad, Some(&*self.obj()), event)
    }

    fn setup_hlssink(&self, hlssink: &gst::Element, settings: &Settings) {
        /* Propagate some settings to the underlying hlscmafsink/hlssink3 */
        hlssink.set_property("max-files", settings.max_num_segment_files as u32);
        hlssink.set_property("playlist-length", settings.playlist_length);
        hlssink.set_property_from_str(
            "playlist-type",
            &settings
                .playlist_type
                .unwrap_or(DEFAULT_PLAYLIST_TYPE)
                .to_string(),
        );
        if settings.muxer_type == HlsMultivariantSinkMuxerType::MpegTs {
            hlssink.set_property("send-keyframe-requests", settings.send_keyframe_requests);
        }
        hlssink.set_property("target-duration", settings.target_duration);

        let mut signals = vec![
            SIGNAL_DELETE_FRAGMENT,
            SIGNAL_GET_FRAGMENT_STREAM,
            SIGNAL_GET_PLAYLIST_STREAM,
        ];

        if settings.muxer_type == HlsMultivariantSinkMuxerType::Cmaf {
            signals.push(SIGNAL_GET_INIT_STREAM);
        }

        for signal in signals {
            hlssink.connect(signal, false, {
                let self_weak = self.downgrade();
                move |args| -> Option<glib::Value> {
                    let self_ = self_weak.upgrade()?;
                    let location = args[1].get::<&str>().unwrap();

                    if signal == SIGNAL_DELETE_FRAGMENT {
                        Some(
                            self_
                                .obj()
                                .emit_by_name::<bool>(signal, &[&location])
                                .to_value(),
                        )
                    } else {
                        Some(
                            self_
                                .obj()
                                .emit_by_name::<Option<gio::OutputStream>>(signal, &[&location])
                                .to_value(),
                        )
                    }
                }
            });
        }
    }

    fn validate_alternate_rendition_and_variants(
        &self,
        alternatives: &[AlternativeMedia],
        variants: &[Variant],
    ) -> bool {
        if variants.is_empty() {
            gst::error!(CAT, imp = self, "Empty variant stream");
            return false;
        }

        let variants_audio_group_ids = variants
            .iter()
            .filter_map(|variant| variant.audio.clone())
            .collect::<Vec<_>>();
        let variants_video_group_ids = variants
            .iter()
            .filter_map(|variant| variant.video.clone())
            .collect::<Vec<_>>();

        for alternate in alternatives.iter() {
            let groupid = &alternate.group_id;

            let res = if alternate.media_type == AlternativeMediaType::Audio {
                variants_audio_group_ids
                    .clone()
                    .into_iter()
                    .find(|x| *x == *groupid)
            } else {
                variants_video_group_ids
                    .clone()
                    .into_iter()
                    .find(|x| *x == *groupid)
            };

            if res.is_none() {
                gst::error!(
                    CAT,
                    imp = self,
                    "No matching GROUP-ID for alternate rendition in variant stream"
                );
                return false;
            }
        }

        // NAME in alternate renditions must be unique
        let mut names = alternatives
            .iter()
            .map(|alt| Some(alt.name.clone()))
            .collect::<Vec<_>>();
        let names_len = names.len();
        names.dedup();
        if names.len() < names_len {
            gst::error!(
                CAT,
                imp = self,
                "Duplicate NAME not allowed in alternate rendition"
            );
            return false;
        }

        true
    }

    fn write_multivariant_playlist(&self, state: &mut State) {
        let variant_streams = state.variants.iter().map(VariantStream::from).collect();
        let alternatives = state.alternatives.clone();
        state.wrote_manifest = true;

        let settings = self.settings.lock().unwrap();
        let multivariant_playlist_location = settings.multivariant_playlist_location.clone();
        let multivariant_playlist_filename = path::Path::new(&multivariant_playlist_location)
            .to_str()
            .expect("multivariant playlist path to string conversion failed");
        let muxer_type = settings.muxer_type;
        drop(settings);

        let version = if muxer_type == HlsMultivariantSinkMuxerType::Cmaf {
            Some(6)
        } else {
            Some(4)
        };

        let playlist = MasterPlaylist {
            version,
            variants: variant_streams,
            alternatives,
            ..Default::default()
        };

        match self.obj().emit_by_name::<Option<gio::OutputStream>>(
            SIGNAL_GET_MULTIVARIANT_PLAYLIST_STREAM,
            &[&multivariant_playlist_filename],
        ) {
            Some(s) => {
                let mut stream = s.into_write();

                if let Err(err) = playlist.write_to(&mut stream) {
                    gst::error!(
                        CAT,
                        imp = self,
                        "Failed to write multivariant playlist with error: {}",
                        err
                    );
                    gst::element_error!(
                        self.obj(),
                        gst::ResourceError::Settings,
                        ["Failed to write multivariant playlist with error: {}", err]
                    );
                } else {
                    gst::info!(CAT, imp = self, "Writing multivariant playlist");
                }
            }
            None => {
                gst::error!(
                    CAT,
                    imp = self,
                    "Could not get stream to write multivariant playlist"
                );
                gst::element_error!(
                    self.obj(),
                    gst::ResourceError::Settings,
                    ["Could not get stream to write multivariant playlist"]
                );
            }
        }
    }

    fn new_file_stream<P>(&self, location: &P) -> Result<gio::OutputStream, String>
    where
        P: AsRef<path::Path>,
    {
        let file = File::create(location).map_err(move |err| {
            let error_msg = gst::error_msg!(
                gst::ResourceError::OpenWrite,
                [
                    "Could not open file {} for writing: {}",
                    location.as_ref().to_str().unwrap(),
                    err.to_string(),
                ]
            );

            self.post_error_message(error_msg);

            err.to_string()
        })?;

        Ok(gio::WriteOutputStream::new(file).upcast())
    }

    fn delete_fragment<P>(&self, location: &P)
    where
        P: AsRef<path::Path>,
    {
        let _ = fs::remove_file(location).map_err(|err| {
            gst::warning!(
                CAT,
                imp = self,
                "Could not delete segment file: {}",
                err.to_string()
            );
        });
    }
}
