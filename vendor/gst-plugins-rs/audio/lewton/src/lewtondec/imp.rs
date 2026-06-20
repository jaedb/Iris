// Copyright (C) 2019 Sebastian Dröge <sebastian@centricular.com>
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
use gst_audio::audio_decoder_error;
use gst_audio::prelude::*;
use gst_audio::subclass::prelude::*;

use atomic_refcell::AtomicRefCell;

use byte_slice_cast::*;

use std::sync::LazyLock;

struct State {
    header_bufs: (
        Option<gst::Buffer>,
        Option<gst::Buffer>,
        Option<gst::Buffer>,
    ),
    headerset: Option<lewton::header::HeaderSet>,
    pwr: lewton::audio::PreviousWindowRight,
    audio_info: Option<gst_audio::AudioInfo>,
    reorder_map: Option<[usize; 8]>,
}

#[derive(Default)]
pub struct LewtonDec {
    state: AtomicRefCell<Option<State>>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "lewtondec",
        gst::DebugColorFlags::empty(),
        Some("lewton Vorbis decoder"),
    )
});

#[glib::object_subclass]
impl ObjectSubclass for LewtonDec {
    const NAME: &'static str = "GstLewtonDec";
    type Type = super::LewtonDec;
    type ParentType = gst_audio::AudioDecoder;
}

impl ObjectImpl for LewtonDec {}

impl GstObjectImpl for LewtonDec {}

impl ElementImpl for LewtonDec {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "lewton Vorbis decoder",
                "Decoder/Audio",
                "lewton Vorbis decoder",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let sink_caps = gst::Caps::builder("audio/x-vorbis").build();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &sink_caps,
            )
            .unwrap();

            let src_caps = gst_audio::AudioCapsBuilder::new_interleaved()
                .format(gst_audio::AUDIO_FORMAT_F32)
                .channels_range(1..=255)
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

impl AudioDecoderImpl for LewtonDec {
    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        *self.state.borrow_mut() = None;

        Ok(())
    }

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        *self.state.borrow_mut() = Some(State {
            header_bufs: (None, None, None),
            headerset: None,
            pwr: lewton::audio::PreviousWindowRight::new(),
            audio_info: None,
            reorder_map: None,
        });

        Ok(())
    }

    fn set_format(&self, caps: &gst::Caps) -> Result<(), gst::LoggableError> {
        gst::debug!(CAT, imp = self, "Setting format {:?}", caps);

        // When the caps are changing we require new headers
        let mut state_guard = self.state.borrow_mut();
        *state_guard = Some(State {
            header_bufs: (None, None, None),
            headerset: None,
            pwr: lewton::audio::PreviousWindowRight::new(),
            audio_info: None,
            reorder_map: None,
        });

        let state = state_guard.as_mut().unwrap();

        let s = caps.structure(0).unwrap();
        if let Ok(Some(streamheaders)) = s.get_optional::<gst::ArrayRef>("streamheader") {
            let streamheaders = streamheaders.as_slice();
            if streamheaders.len() < 3 {
                gst::debug!(CAT, imp = self, "Not enough streamheaders, trying in-band");
                return Ok(());
            }

            let ident_buf = streamheaders[0].get::<Option<gst::Buffer>>();
            let comment_buf = streamheaders[1].get::<Option<gst::Buffer>>();
            let setup_buf = streamheaders[2].get::<Option<gst::Buffer>>();
            if let (Ok(Some(ident_buf)), Ok(Some(comment_buf)), Ok(Some(setup_buf))) =
                (ident_buf, comment_buf, setup_buf)
            {
                gst::debug!(CAT, imp = self, "Got streamheader buffers");
                state.header_bufs = (Some(ident_buf), Some(comment_buf), Some(setup_buf));
            }
        }

        Ok(())
    }

    fn flush(&self, _hard: bool) {
        gst::debug!(CAT, imp = self, "Flushing");

        let mut state_guard = self.state.borrow_mut();
        if let Some(ref mut state) = *state_guard {
            state.pwr = lewton::audio::PreviousWindowRight::new();
        }
    }

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

        // Ignore empty packets unless we have no headers yet
        if inmap.is_empty() {
            self.obj().finish_frame(None, 1)?;

            if state.headerset.is_some() {
                return Ok(gst::FlowSuccess::Ok);
            } else {
                gst::error!(CAT, imp = self, "Got empty packet before all headers");
                return Err(gst::FlowError::Error);
            }
        }

        // If this is a header packet then handle it
        if inmap[0] & 0x01 == 0x01 {
            return self.handle_header(state, inbuf, inmap.as_ref());
        }

        // If it's a data packet then try to initialize the headerset now if we didn't yet
        if state.headerset.is_none() {
            self.initialize(state)?;
        }

        self.handle_data(state, inmap.as_ref())
    }
}

impl LewtonDec {
    fn handle_header(
        &self,
        state: &mut State,
        inbuf: &gst::Buffer,
        indata: &[u8],
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        // ident header
        if indata[0] == 0x01 {
            gst::debug!(CAT, imp = self, "Got ident header buffer");
            state.header_bufs = (Some(inbuf.clone()), None, None);
        } else if indata[0] == 0x03 {
            // comment header
            if state.header_bufs.0.is_none() {
                gst::warning!(CAT, imp = self, "Got comment header before ident header");
            } else {
                gst::debug!(CAT, imp = self, "Got comment header buffer");
                state.header_bufs.1 = Some(inbuf.clone());
            }
        } else if indata[0] == 0x05 {
            // setup header
            if state.header_bufs.0.is_none() || state.header_bufs.1.is_none() {
                gst::warning!(
                    CAT,
                    imp = self,
                    "Got setup header before ident/comment header"
                );
            } else {
                gst::debug!(CAT, imp = self, "Got setup header buffer");
                state.header_bufs.2 = Some(inbuf.clone());
            }
        }

        self.obj().finish_frame(None, 1)
    }

    fn initialize(&self, state: &mut State) -> Result<(), gst::FlowError> {
        let (ident_buf, comment_buf, setup_buf) = match state.header_bufs {
            (Some(ref ident_buf), Some(ref comment_buf), Some(ref setup_buf)) => {
                (ident_buf, comment_buf, setup_buf)
            }
            _ => {
                gst::element_imp_error!(
                    self,
                    gst::StreamError::Decode,
                    ["Got no headers before data packets"]
                );
                return Err(gst::FlowError::NotNegotiated);
            }
        };

        // First try to parse the headers
        let ident_map = ident_buf.map_readable().map_err(|_| {
            gst::error!(CAT, imp = self, "Failed to map ident buffer readable");
            gst::FlowError::Error
        })?;
        let ident = lewton::header::read_header_ident(ident_map.as_ref()).map_err(|err| {
            gst::element_imp_error!(
                self,
                gst::StreamError::Decode,
                ["Failed to parse ident header: {:?}", err]
            );
            gst::FlowError::Error
        })?;

        let comment_map = comment_buf.map_readable().map_err(|_| {
            gst::error!(CAT, imp = self, "Failed to map comment buffer readable");
            gst::FlowError::Error
        })?;
        let comment = lewton::header::read_header_comment(comment_map.as_ref()).map_err(|err| {
            gst::element_imp_error!(
                self,
                gst::StreamError::Decode,
                ["Failed to parse comment header: {:?}", err]
            );
            gst::FlowError::Error
        })?;

        let setup_map = setup_buf.map_readable().map_err(|_| {
            gst::error!(CAT, imp = self, "Failed to map setup buffer readable");
            gst::FlowError::Error
        })?;
        let setup = lewton::header::read_header_setup(
            setup_map.as_ref(),
            ident.audio_channels,
            (ident.blocksize_0, ident.blocksize_1),
        )
        .map_err(|err| {
            gst::element_imp_error!(
                self,
                gst::StreamError::Decode,
                ["Failed to parse setup header: {:?}", err]
            );
            gst::FlowError::Error
        })?;

        let mut audio_info = gst_audio::AudioInfo::builder(
            gst_audio::AUDIO_FORMAT_F32,
            ident.audio_sample_rate,
            ident.audio_channels as u32,
        );

        // For 1-8 channels there are defined channel positions, so initialize
        // everything accordingly and set the channel positions in the output
        // format
        let mut reorder_map = None;
        if ident.audio_channels > 1 && ident.audio_channels < 9 {
            let channels = ident.audio_channels as usize;
            let from = &VORBIS_CHANNEL_POSITIONS[channels - 1][..channels];
            let to = &GST_VORBIS_CHANNEL_POSITIONS[channels - 1][..channels];

            audio_info = audio_info.positions(to);

            let mut map = [0; 8];
            if gst_audio::channel_reorder_map(from, to, &mut map[..channels]).is_err() {
                gst::error!(
                    CAT,
                    imp = self,
                    "Failed to generate channel reorder map from {:?} to {:?}",
                    from,
                    to,
                );
            } else {
                // If the reorder map is not the identity matrix
                if !map[..channels].iter().enumerate().all(|(c1, c2)| c1 == *c2) {
                    reorder_map = Some(map);
                }
            }
        }
        let audio_info = audio_info.build().unwrap();

        gst::debug!(
            CAT,
            imp = self,
            "Successfully parsed headers: {:?}",
            audio_info
        );
        state.headerset = Some((ident, comment, setup));
        state.audio_info = Some(audio_info.clone());
        state.reorder_map = reorder_map;

        self.obj().set_output_format(&audio_info)?;
        self.obj().negotiate()?;

        Ok(())
    }

    fn handle_data(
        &self,
        state: &mut State,
        indata: &[u8],
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        // We ensured above that we have headers here now
        let headerset = state.headerset.as_ref().unwrap();
        let audio_info = state.audio_info.as_ref().unwrap();

        // Decode the input packet
        let decoded = match lewton::audio::read_audio_packet_generic::<
            lewton::samples::InterleavedSamples<f32>,
        >(&headerset.0, &headerset.2, indata, &mut state.pwr)
        {
            Ok(decoded) => decoded,
            Err(err) => {
                return audio_decoder_error!(
                    self.obj(),
                    1,
                    gst::StreamError::Decode,
                    ["Failed to decode packet: {:?}", err]
                );
            }
        };

        if decoded.channel_count != audio_info.channels() as usize {
            return audio_decoder_error!(
                self.obj(),
                1,
                gst::StreamError::Decode,
                [
                    "Channel count mismatch (got {}, expected {})",
                    decoded.channel_count,
                    audio_info.channels()
                ]
            );
        }

        let sample_count = decoded.samples.len() / audio_info.channels() as usize;
        gst::debug!(CAT, imp = self, "Got {} decoded samples", sample_count);

        if sample_count == 0 {
            return self.obj().finish_frame(None, 1);
        }

        let outbuf = if let Some(ref reorder_map) = state.reorder_map {
            let mut outbuf = self
                .obj()
                .allocate_output_buffer(sample_count * audio_info.bpf() as usize);
            {
                // And copy the decoded data into our output buffer while reordering the channels to the
                // GStreamer channel order
                let outbuf = outbuf.get_mut().unwrap();
                let mut outmap = outbuf.map_writable().map_err(|_| {
                    gst::element_imp_error!(
                        self,
                        gst::StreamError::Decode,
                        ["Failed to map output buffer writable"]
                    );
                    gst::FlowError::Error
                })?;

                let outdata = outmap.as_mut_slice_of::<f32>().unwrap();
                let channels = audio_info.channels() as usize;
                assert!(reorder_map.len() >= channels);
                assert!(reorder_map[..channels].iter().all(|c| *c < channels));

                for (output, input) in outdata
                    .chunks_exact_mut(channels)
                    .zip(decoded.samples.chunks_exact(channels))
                {
                    for (c, s) in input.iter().enumerate() {
                        output[reorder_map[c]] = *s;
                    }
                }
            }

            outbuf
        } else {
            struct CastVec(Vec<f32>);
            impl AsRef<[u8]> for CastVec {
                fn as_ref(&self) -> &[u8] {
                    self.0.as_byte_slice()
                }
            }
            impl AsMut<[u8]> for CastVec {
                fn as_mut(&mut self) -> &mut [u8] {
                    self.0.as_mut_byte_slice()
                }
            }

            gst::Buffer::from_mut_slice(CastVec(decoded.samples))
        };

        self.obj().finish_frame(Some(outbuf), 1)
    }
}

// http://www.xiph.org/vorbis/doc/Vorbis_I_spec.html#x1-800004.3.9
const VORBIS_CHANNEL_POSITIONS: [[gst_audio::AudioChannelPosition; 8]; 8] = [
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

const GST_VORBIS_CHANNEL_POSITIONS: [[gst_audio::AudioChannelPosition; 8]; 8] = [
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
        gst_audio::AudioChannelPosition::FrontRight,
        gst_audio::AudioChannelPosition::FrontCenter,
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
        gst_audio::AudioChannelPosition::FrontRight,
        gst_audio::AudioChannelPosition::FrontCenter,
        gst_audio::AudioChannelPosition::RearLeft,
        gst_audio::AudioChannelPosition::RearRight,
        gst_audio::AudioChannelPosition::Invalid,
        gst_audio::AudioChannelPosition::Invalid,
        gst_audio::AudioChannelPosition::Invalid,
    ],
    [
        gst_audio::AudioChannelPosition::FrontLeft,
        gst_audio::AudioChannelPosition::FrontRight,
        gst_audio::AudioChannelPosition::FrontCenter,
        gst_audio::AudioChannelPosition::Lfe1,
        gst_audio::AudioChannelPosition::RearLeft,
        gst_audio::AudioChannelPosition::RearRight,
        gst_audio::AudioChannelPosition::Invalid,
        gst_audio::AudioChannelPosition::Invalid,
    ],
    [
        gst_audio::AudioChannelPosition::FrontLeft,
        gst_audio::AudioChannelPosition::FrontRight,
        gst_audio::AudioChannelPosition::FrontCenter,
        gst_audio::AudioChannelPosition::Lfe1,
        gst_audio::AudioChannelPosition::RearCenter,
        gst_audio::AudioChannelPosition::SideLeft,
        gst_audio::AudioChannelPosition::SideRight,
        gst_audio::AudioChannelPosition::Invalid,
    ],
    [
        gst_audio::AudioChannelPosition::FrontLeft,
        gst_audio::AudioChannelPosition::FrontRight,
        gst_audio::AudioChannelPosition::FrontCenter,
        gst_audio::AudioChannelPosition::Lfe1,
        gst_audio::AudioChannelPosition::RearLeft,
        gst_audio::AudioChannelPosition::RearRight,
        gst_audio::AudioChannelPosition::SideLeft,
        gst_audio::AudioChannelPosition::SideRight,
    ],
];
