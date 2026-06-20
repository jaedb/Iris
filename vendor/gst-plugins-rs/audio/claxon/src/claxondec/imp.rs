// Copyright (C) 2019 Ruben Gonzalez <rgonzalez@fluendo.com>
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
use gst_audio::prelude::*;
use gst_audio::subclass::prelude::*;

use std::io::Cursor;

use atomic_refcell::AtomicRefCell;

use byte_slice_cast::*;

use std::sync::LazyLock;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "claxondec",
        gst::DebugColorFlags::empty(),
        Some("Claxon FLAC decoder"),
    )
});

struct State {
    audio_info: Option<gst_audio::AudioInfo>,
}

#[derive(Default)]
pub struct ClaxonDec {
    state: AtomicRefCell<Option<State>>,
}

#[glib::object_subclass]
impl ObjectSubclass for ClaxonDec {
    const NAME: &'static str = "GstClaxonDec";
    type Type = super::ClaxonDec;
    type ParentType = gst_audio::AudioDecoder;
}

impl ObjectImpl for ClaxonDec {}

impl GstObjectImpl for ClaxonDec {}

impl ElementImpl for ClaxonDec {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Claxon FLAC decoder",
                "Decoder/Audio",
                "Claxon FLAC decoder",
                "Ruben Gonzalez <rgonzalez@fluendo.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let sink_caps = gst::Caps::builder("audio/x-flac")
                .field("framed", true)
                .build();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &sink_caps,
            )
            .unwrap();

            let src_caps = gst_audio::AudioCapsBuilder::new_interleaved()
                .format_list([
                    gst_audio::AudioFormat::S8,
                    gst_audio::AUDIO_FORMAT_S16,
                    gst_audio::AUDIO_FORMAT_S2432,
                    gst_audio::AUDIO_FORMAT_S32,
                ])
                .rate_range(1..655_350)
                .channels_range(1..8)
                .build();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &src_caps,
            )
            .unwrap();

            vec![sink_pad_template, src_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl AudioDecoderImpl for ClaxonDec {
    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        *self.state.borrow_mut() = None;

        Ok(())
    }

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        *self.state.borrow_mut() = Some(State { audio_info: None });

        Ok(())
    }

    fn set_format(&self, caps: &gst::Caps) -> Result<(), gst::LoggableError> {
        gst::debug!(CAT, imp = self, "Setting format {:?}", caps);

        let mut audio_info: Option<gst_audio::AudioInfo> = None;

        let s = caps.structure(0).unwrap();
        if let Ok(Some(streamheaders)) = s.get_optional::<gst::ArrayRef>("streamheader") {
            let streamheaders = streamheaders.as_slice();

            if streamheaders.len() < 2 {
                gst::debug!(CAT, imp = self, "Not enough streamheaders, trying in-band");
            } else {
                let ident_buf = streamheaders[0].get::<Option<gst::Buffer>>();
                if let Ok(Some(ident_buf)) = ident_buf {
                    gst::debug!(CAT, imp = self, "Got streamheader buffers");
                    let inmap = ident_buf.map_readable().unwrap();

                    if inmap[0..7] != [0x7f, b'F', b'L', b'A', b'C', 0x01, 0x00] {
                        gst::debug!(CAT, imp = self, "Unknown streamheader format");
                    } else if let Ok(tstreaminfo) = claxon_streaminfo(&inmap[13..]) {
                        if let Ok(taudio_info) = gstaudioinfo(&tstreaminfo) {
                            // To speed up negotiation
                            let element = self.obj();
                            if element.set_output_format(&taudio_info).is_err()
                                || element.negotiate().is_err()
                            {
                                gst::debug!(
                                    CAT,
                                    imp = self,
                                    "Error to negotiate output from based on in-caps streaminfo"
                                );
                            }

                            audio_info = Some(taudio_info);
                        }
                    }
                }
            }
        }

        let mut state_guard = self.state.borrow_mut();
        *state_guard = Some(State { audio_info });

        Ok(())
    }

    #[allow(clippy::verbose_bit_mask)]
    fn handle_frame(
        &self,
        inbuf: Option<&gst::Buffer>,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::debug!(CAT, imp = self, "Handling buffer {:?}", inbuf);

        let inbuf = match inbuf {
            None => return Ok(gst::FlowSuccess::Ok),
            Some(inbuf) => inbuf,
        };

        let inmap = inbuf.map_readable().map_err(|_| {
            gst::error!(CAT, imp = self, "Failed to buffer readable");
            gst::FlowError::Error
        })?;

        let mut state_guard = self.state.borrow_mut();
        let state = state_guard.as_mut().ok_or(gst::FlowError::NotNegotiated)?;

        if inmap.as_slice() == b"fLaC" {
            gst::debug!(CAT, imp = self, "fLaC buffer received");
        } else if inmap[0] & 0x7F == 0x00 {
            gst::debug!(CAT, imp = self, "Streaminfo header buffer received");
            return self.handle_streaminfo_header(state, inmap.as_ref());
        } else if inmap[0] == 0b1111_1111 && inmap[1] & 0b1111_1100 == 0b1111_1000 {
            gst::debug!(CAT, imp = self, "Data buffer received");
            return self.handle_data(state, inmap.as_ref());
        } else {
            // info about other headers in flacparse and https://xiph.org/flac/format.html
            gst::debug!(
                CAT,
                imp = self,
                "Other header buffer received {:?}",
                inmap[0] & 0x7F
            );
        }

        self.obj().finish_frame(None, 1)
    }
}

impl ClaxonDec {
    fn handle_streaminfo_header(
        &self,
        state: &mut State,
        indata: &[u8],
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let streaminfo = claxon_streaminfo(indata).map_err(|e| {
            gst::element_imp_error!(self, gst::StreamError::Decode, ["{e}"]);
            gst::FlowError::Error
        })?;

        let audio_info = gstaudioinfo(&streaminfo).map_err(|e| {
            gst::element_imp_error!(self, gst::StreamError::Decode, ["{e}"]);
            gst::FlowError::Error
        })?;

        gst::debug!(
            CAT,
            imp = self,
            "Successfully parsed headers: {:?}",
            audio_info
        );

        let element = self.obj();
        element.set_output_format(&audio_info)?;
        element.negotiate()?;

        state.audio_info = Some(audio_info);

        element.finish_frame(None, 1)
    }

    fn handle_data(
        &self,
        state: &mut State,
        indata: &[u8],
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        // TODO It's valid for FLAC to not have any streaminfo header at all, for a small subset
        // of possible FLAC configurations. (claxon does not actually support that)
        let audio_info = state
            .audio_info
            .as_ref()
            .ok_or(gst::FlowError::NotNegotiated)?;
        let depth = AudioDepth::validate(audio_info.depth())?;

        let channels = audio_info.channels() as usize;
        if channels > 8 {
            unreachable!(
                "FLAC only supports from 1 to 8 channels (audio contains {} channels)",
                channels
            );
        }

        let buffer = Vec::new();
        let mut cursor = Cursor::new(indata);
        let mut reader = claxon::frame::FrameReader::new(&mut cursor);
        let result = match reader.read_next_or_eof(buffer) {
            Ok(Some(result)) => result,
            Ok(None) => return self.obj().finish_frame(None, 1),
            Err(err) => {
                return gst_audio::audio_decoder_error!(
                    self.obj(),
                    1,
                    gst::StreamError::Decode,
                    ["Failed to decode packet: {:?}", err]
                );
            }
        };

        assert_eq!(cursor.position(), indata.len() as u64);

        let v = if channels != 1 {
            let mut v: Vec<i32> = vec![0; result.len() as usize];

            for (o, i) in v.chunks_exact_mut(channels).enumerate() {
                for (c, s) in i.iter_mut().enumerate() {
                    *s = result.sample(c as u32, o as u32);
                }
            }
            v
        } else {
            result.into_buffer()
        };

        let depth_adjusted = depth.adjust_samples(v);
        let outbuf = gst::Buffer::from_mut_slice(depth_adjusted);
        self.obj().finish_frame(Some(outbuf), 1)
    }
}

/// Depth of audio samples
enum AudioDepth {
    /// 8bits.
    I8,
    /// 16bits.
    I16,
    /// 24bits.
    I24,
    /// 32bits.
    I32,
}

enum ByteVec {
    I8(Vec<i8>),
    I16(Vec<i16>),
    I32(Vec<i32>),
}

impl AsRef<[u8]> for ByteVec {
    fn as_ref(&self) -> &[u8] {
        match self {
            ByteVec::I8(ref vec) => vec.as_byte_slice(),
            ByteVec::I16(ref vec) => vec.as_byte_slice(),
            ByteVec::I32(ref vec) => vec.as_byte_slice(),
        }
    }
}

impl AsMut<[u8]> for ByteVec {
    fn as_mut(&mut self) -> &mut [u8] {
        match self {
            ByteVec::I8(ref mut vec) => vec.as_mut_byte_slice(),
            ByteVec::I16(ref mut vec) => vec.as_mut_byte_slice(),
            ByteVec::I32(ref mut vec) => vec.as_mut_byte_slice(),
        }
    }
}

impl AudioDepth {
    /// Validate input audio depth.
    fn validate(input: u32) -> Result<Self, gst::FlowError> {
        let depth = match input {
            8 => AudioDepth::I8,
            16 => AudioDepth::I16,
            24 => AudioDepth::I24,
            32 => AudioDepth::I32,
            _ => return Err(gst::FlowError::NotSupported),
        };
        Ok(depth)
    }

    /// Adjust samples depth.
    ///
    /// This takes a vector of 32bits samples, adjusts the depth of each,
    /// and returns the adjusted bytes stream.
    fn adjust_samples(&self, input: Vec<i32>) -> ByteVec {
        match *self {
            AudioDepth::I8 => ByteVec::I8(input.into_iter().map(|x| x as i8).collect::<Vec<_>>()),
            AudioDepth::I16 => {
                ByteVec::I16(input.into_iter().map(|x| x as i16).collect::<Vec<_>>())
            }
            AudioDepth::I24 | AudioDepth::I32 => ByteVec::I32(input),
        }
    }
}

fn claxon_streaminfo(indata: &[u8]) -> Result<claxon::metadata::StreamInfo, &'static str> {
    let mut cursor = Cursor::new(indata);
    let mut metadata_iter = claxon::metadata::MetadataBlockReader::new(&mut cursor);
    let streaminfo = match metadata_iter.next() {
        Some(Ok(claxon::metadata::MetadataBlock::StreamInfo(info))) => info,
        _ => return Err("Failed to decode STREAMINFO"),
    };

    assert_eq!(cursor.position(), indata.len() as u64);

    Ok(streaminfo)
}

fn gstaudioinfo(streaminfo: &claxon::metadata::StreamInfo) -> Result<gst_audio::AudioInfo, String> {
    let format = match streaminfo.bits_per_sample {
        8 => gst_audio::AudioFormat::S8,
        16 => gst_audio::AUDIO_FORMAT_S16,
        24 => gst_audio::AUDIO_FORMAT_S2432,
        32 => gst_audio::AUDIO_FORMAT_S32,
        _ => return Err("format not supported".to_string()),
    };

    let index = match streaminfo.channels as usize {
        0 => return Err("no channels".to_string()),
        n if n > 8 => return Err("more than 8 channels, not supported yet".to_string()),
        n => n,
    };
    let to = &FLAC_CHANNEL_POSITIONS[index - 1][..index];
    let info_builder =
        gst_audio::AudioInfo::builder(format, streaminfo.sample_rate, streaminfo.channels)
            .positions(to);

    let audio_info = info_builder
        .build()
        .map_err(|e| format!("failed to build audio info: {e}"))?;

    Ok(audio_info)
}

// http://www.xiph.org/vorbis/doc/Vorbis_I_spec.html#x1-800004.3.9
// http://flac.sourceforge.net/format.html#frame_header
const FLAC_CHANNEL_POSITIONS: [[gst_audio::AudioChannelPosition; 8]; 8] = [
    [
        gst_audio::AudioChannelPosition::Mono,
        gst_audio::AudioChannelPosition::Invalid,
        gst_audio::AudioChannelPosition::Invalid,
        gst_audio::AudioChannelPosition::Invalid,
        gst_audio::AudioChannelPosition::Invalid,
        gst_audio::AudioChannelPosition::Invalid,
        gst_audio::AudioChannelPosition::Invalid,
        gst_audio::AudioChannelPosition::Invalid,
    ],
    [
        gst_audio::AudioChannelPosition::FrontLeft,
        gst_audio::AudioChannelPosition::FrontRight,
        gst_audio::AudioChannelPosition::Invalid,
        gst_audio::AudioChannelPosition::Invalid,
        gst_audio::AudioChannelPosition::Invalid,
        gst_audio::AudioChannelPosition::Invalid,
        gst_audio::AudioChannelPosition::Invalid,
        gst_audio::AudioChannelPosition::Invalid,
    ],
    [
        gst_audio::AudioChannelPosition::FrontLeft,
        gst_audio::AudioChannelPosition::FrontCenter,
        gst_audio::AudioChannelPosition::FrontRight,
        gst_audio::AudioChannelPosition::Invalid,
        gst_audio::AudioChannelPosition::Invalid,
        gst_audio::AudioChannelPosition::Invalid,
        gst_audio::AudioChannelPosition::Invalid,
        gst_audio::AudioChannelPosition::Invalid,
    ],
    [
        gst_audio::AudioChannelPosition::FrontLeft,
        gst_audio::AudioChannelPosition::FrontRight,
        gst_audio::AudioChannelPosition::RearLeft,
        gst_audio::AudioChannelPosition::RearRight,
        gst_audio::AudioChannelPosition::Invalid,
        gst_audio::AudioChannelPosition::Invalid,
        gst_audio::AudioChannelPosition::Invalid,
        gst_audio::AudioChannelPosition::Invalid,
    ],
    [
        gst_audio::AudioChannelPosition::FrontLeft,
        gst_audio::AudioChannelPosition::FrontCenter,
        gst_audio::AudioChannelPosition::FrontRight,
        gst_audio::AudioChannelPosition::RearLeft,
        gst_audio::AudioChannelPosition::RearRight,
        gst_audio::AudioChannelPosition::Invalid,
        gst_audio::AudioChannelPosition::Invalid,
        gst_audio::AudioChannelPosition::Invalid,
    ],
    [
        gst_audio::AudioChannelPosition::FrontLeft,
        gst_audio::AudioChannelPosition::FrontCenter,
        gst_audio::AudioChannelPosition::FrontRight,
        gst_audio::AudioChannelPosition::RearLeft,
        gst_audio::AudioChannelPosition::RearRight,
        gst_audio::AudioChannelPosition::Lfe1,
        gst_audio::AudioChannelPosition::Invalid,
        gst_audio::AudioChannelPosition::Invalid,
    ],
    // FIXME: 7/8 channel layouts are not defined in the FLAC specs
    [
        gst_audio::AudioChannelPosition::FrontLeft,
        gst_audio::AudioChannelPosition::FrontCenter,
        gst_audio::AudioChannelPosition::FrontRight,
        gst_audio::AudioChannelPosition::SideLeft,
        gst_audio::AudioChannelPosition::SideRight,
        gst_audio::AudioChannelPosition::RearCenter,
        gst_audio::AudioChannelPosition::Lfe1,
        gst_audio::AudioChannelPosition::Invalid,
    ],
    [
        gst_audio::AudioChannelPosition::FrontLeft,
        gst_audio::AudioChannelPosition::FrontCenter,
        gst_audio::AudioChannelPosition::FrontRight,
        gst_audio::AudioChannelPosition::SideLeft,
        gst_audio::AudioChannelPosition::SideRight,
        gst_audio::AudioChannelPosition::RearLeft,
        gst_audio::AudioChannelPosition::RearRight,
        gst_audio::AudioChannelPosition::Lfe1,
    ],
];
