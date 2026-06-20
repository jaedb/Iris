//! NDI Closed Caption encoder and parser
//!
//! See:
//!
//! * http://www.sienna-tv.com/ndi/ndiclosedcaptions.html
//! * http://www.sienna-tv.com/ndi/ndiclosedcaptions608.html

use anyhow::{bail, Result};
use data_encoding::BASE64;
use smallvec::SmallVec;

use gst::glib::translate::IntoGlib;
#[cfg(feature = "sink")]
use gst_video::VideoVBIEncoder;
use gst_video::{VideoAncillary, VideoAncillaryDID16, VideoVBIParser};
use std::sync::LazyLock;

#[cfg(feature = "sink")]
use std::ffi::CString;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "ndiccmeta",
        gst::DebugColorFlags::empty(),
        Some("NewTek NDI CC Meta"),
    )
});

const C608_TAG: &str = "C608";
const C608_TAG_BYTES: &[u8] = C608_TAG.as_bytes();

const C708_TAG: &str = "C708";
const C708_TAG_BYTES: &[u8] = C708_TAG.as_bytes();

#[cfg(feature = "sink")]
const LINE_ATTR: &str = "line";
#[cfg(feature = "sink")]
const DEFAULT_LINE: u8 = 21;
#[cfg(feature = "sink")]
const DEFAULT_LINE_STR: &str = "21";
#[cfg(feature = "sink")]
const DEFAULT_LINE_C708_STR: &str = "10";

// Video anc AFD content:
// ADF + DID/SDID + DATA COUNT + PAYLOAD + checksum:
// 3 + 2 + 1 + 256 max + 1 = 263
// Those are 10bit words, so we need 329 bytes max.
pub const VIDEO_ANC_AFD_CAPACITY: usize = 329;

/// Video anc AFD content padded to 32bit alignment encoded in base64 + padding
const NDI_CC_CONTENT_CAPACITY: usize = (VIDEO_ANC_AFD_CAPACITY + 3) * 3 / 2 + 2;

/// Video anc AFD padded to 32bit alignment encoded in base64
/// + XML tags with brackets and end '/' + attr
const NDI_CC_CAPACITY: usize = NDI_CC_CONTENT_CAPACITY + 13 + 10;

#[derive(thiserror::Error, Debug, Eq, PartialEq)]
/// NDI Video Captions related Errors.
pub enum NDICCError {
    #[cfg(feature = "sink")]
    #[error("Unsupported closed caption type {cc_type:?}")]
    UnsupportedCC {
        cc_type: gst_video::VideoCaptionType,
    },

    #[error("Unexpected AFD data count {found}. Expected: {expected}")]
    UnexpectedAfdDataCount { found: u8, expected: u8 },

    #[error("Unexpected AFD did {found}. Expected: {expected}")]
    UnexpectedAfdDid { found: i32, expected: i32 },
}

impl NDICCError {
    fn new_unexpected_afd_did(found: VideoAncillaryDID16, expected: VideoAncillaryDID16) -> Self {
        NDICCError::UnexpectedAfdDid {
            found: found.into_glib(),
            expected: expected.into_glib(),
        }
    }
}

#[cfg(feature = "sink")]
/// NDI Closed Captions Meta encoder.
pub struct NDICCMetaEncoder {
    v210_encoder: VideoVBIEncoder,
    width: u32,
    line_buf: Vec<u8>,
}

#[cfg(feature = "sink")]
impl NDICCMetaEncoder {
    pub fn new(width: u32) -> Self {
        let v210_encoder = VideoVBIEncoder::try_new(gst_video::VideoFormat::V210, width).unwrap();

        NDICCMetaEncoder {
            line_buf: vec![0; v210_encoder.line_buffer_len()],
            v210_encoder,
            width,
        }
    }

    pub fn set_width(&mut self, width: u32) {
        if width != self.width {
            *self = Self::new(width);
        }
    }

    /// Encodes the VideoCaptionMeta of the provided `gst::Buffer`
    /// in an NDI closed caption metadata suitable to be attached to an NDI video frame.
    pub fn encode(&mut self, video_buf: &gst::BufferRef) -> Option<CString> {
        use quick_xml::events::{BytesEnd, BytesStart, Event};
        use quick_xml::writer::Writer;

        video_buf.meta::<gst_video::VideoCaptionMeta>()?;

        // Start with an initial capacity suitable to store one ndi cc metadata
        let mut xml_writer = Writer::new(Vec::with_capacity(NDI_CC_CAPACITY));

        let cc_meta_iter = video_buf.iter_meta::<gst_video::VideoCaptionMeta>();
        for cc_meta in cc_meta_iter {
            let cc_data = cc_meta.data();
            if cc_data.is_empty() {
                continue;
            }

            use gst_video::VideoCaptionType::*;
            match cc_meta.caption_type() {
                Cea608Raw => {
                    if cc_data.len() != 2 {
                        let err = NDICCError::UnexpectedAfdDataCount {
                            found: cc_data.len() as u8,
                            expected: 2,
                        };
                        gst::error!(CAT, "Failed to encode Cea608Raw metadata: {err}");
                        continue;
                    }

                    let res = self.add_did16_ancillary(
                        VideoAncillaryDID16::S334Eia608,
                        &[DEFAULT_LINE, cc_data[0], cc_data[1]],
                    );
                    if let Err(err) = res {
                        gst::error!(CAT, "Failed to add Cea608Raw metadata: {err}");
                        continue;
                    }

                    let mut elem = BytesStart::new(C608_TAG);
                    elem.push_attribute((LINE_ATTR, DEFAULT_LINE_STR));
                    xml_writer.write_event(Event::Start(elem)).unwrap();

                    self.write_v210_base64(&mut xml_writer);

                    xml_writer
                        .write_event(Event::End(BytesEnd::new(C608_TAG)))
                        .unwrap();
                }
                Cea608S3341a => {
                    if cc_data.len() != 3 {
                        let err = NDICCError::UnexpectedAfdDataCount {
                            found: cc_data.len() as u8,
                            expected: 3,
                        };
                        gst::error!(CAT, "Failed to encode Cea608Raw metadata: {err}");
                        continue;
                    }

                    let res = self.add_did16_ancillary(VideoAncillaryDID16::S334Eia608, cc_data);
                    if let Err(err) = res {
                        gst::error!(CAT, "Failed to add Cea608S3341a metadata: {err}");
                        continue;
                    }

                    let mut elem = BytesStart::new(C608_TAG);
                    elem.push_attribute((LINE_ATTR, format!("{}", cc_meta.data()[0]).as_str()));
                    xml_writer.write_event(Event::Start(elem)).unwrap();

                    self.write_v210_base64(&mut xml_writer);

                    xml_writer
                        .write_event(Event::End(BytesEnd::new(C608_TAG)))
                        .unwrap();
                }
                Cea708Cdp => {
                    let res = self.add_did16_ancillary(VideoAncillaryDID16::S334Eia708, cc_data);
                    if let Err(err) = res {
                        gst::error!(CAT, "Failed to add Cea708Cdp metadata: {err}");
                        continue;
                    }

                    let mut elem = BytesStart::new(C708_TAG);
                    elem.push_attribute((LINE_ATTR, DEFAULT_LINE_C708_STR));
                    xml_writer.write_event(Event::Start(elem)).unwrap();

                    self.write_v210_base64(&mut xml_writer);

                    xml_writer
                        .write_event(Event::End(BytesEnd::new(C708_TAG)))
                        .unwrap();
                }
                other => {
                    gst::info!(CAT, "{}", NDICCError::UnsupportedCC { cc_type: other });
                }
            }
        }

        // # Safety
        // `writer` content is guaranteed to be a C compatible String without interior 0 since:
        // * It contains ASCII XML tags, ASCII XML attributes and base64 encoded content
        // * ASCII & base64 are subsets of UTF-8.
        unsafe {
            let cc_meta = xml_writer.into_inner();
            if cc_meta.is_empty() {
                return None;
            }

            Some(CString::from_vec_unchecked(cc_meta))
        }
    }

    fn add_did16_ancillary(&mut self, did16: VideoAncillaryDID16, data: &[u8]) -> Result<()> {
        self.v210_encoder.add_did16_ancillary(
            gst_video::VideoAFDDescriptionMode::Component,
            did16,
            data,
        )?;

        Ok(())
    }

    /// Encodes previously added data as v210 in base64 and writes it with the XML writer.
    fn write_v210_base64<W>(&mut self, writer: &mut quick_xml::writer::Writer<W>)
    where
        W: std::io::Write,
    {
        use quick_xml::events::{BytesText, Event};

        let anc_len = self.v210_encoder.write_line(&mut self.line_buf).unwrap();
        assert_eq!(anc_len % 4, 0);

        let mut xml_buf = String::with_capacity(NDI_CC_CONTENT_CAPACITY);
        BASE64.encode_append(&self.line_buf[..anc_len], &mut xml_buf);
        writer
            .write_event(Event::Text(BytesText::from_escaped(xml_buf)))
            .unwrap();
    }
}

/// NDI Closed Captions Meta decoder.
pub struct NDICCMetaDecoder {
    v210_parser: VideoVBIParser,
    width: u32,
    line_buf: Vec<u8>,
    xml_content: SmallVec<[u8; NDI_CC_CONTENT_CAPACITY]>,
    xml_buf: Vec<u8>,
}

impl NDICCMetaDecoder {
    pub fn new(width: u32) -> Self {
        let v210_parser = VideoVBIParser::try_new(gst_video::VideoFormat::V210, width).unwrap();

        NDICCMetaDecoder {
            line_buf: vec![0; v210_parser.line_buffer_len()],
            v210_parser,
            width,
            xml_content: SmallVec::<[u8; NDI_CC_CONTENT_CAPACITY]>::new(),
            xml_buf: Vec::with_capacity(NDI_CC_CAPACITY),
        }
    }

    pub fn set_width(&mut self, width: u32) {
        if width != self.width {
            self.v210_parser =
                VideoVBIParser::try_new(gst_video::VideoFormat::V210, width).unwrap();
            self.line_buf = vec![0; self.v210_parser.line_buffer_len()];
            self.width = width;
        }
    }

    /// Decodes the provided NDI metadata string, searching for NDI closed captions
    /// and add them as `VideoCaptionMeta` to the provided `gst::Buffer`.
    pub fn decode(&mut self, input: &str) -> Result<Vec<VideoAncillary>> {
        use quick_xml::events::Event;
        use quick_xml::reader::Reader;

        let mut captions = Vec::new();
        let mut reader = Reader::from_str(input);

        self.xml_buf.clear();
        loop {
            match reader.read_event_into(&mut self.xml_buf)? {
                Event::Eof => break,
                Event::Start(_) => self.xml_content.clear(),
                Event::Text(e) => {
                    self.xml_content.extend(
                        e.iter().copied().filter(|&b| {
                            (b != b' ') && (b != b'\t') && (b != b'\n') && (b != b'\r')
                        }),
                    );
                }
                Event::End(e) => match e.name().as_ref() {
                    C608_TAG_BYTES => match BASE64.decode(self.xml_content.as_slice()) {
                        Ok(v210_buf) => match self.parse_for_cea608(&v210_buf) {
                            Ok(None) => (),
                            Ok(Some(anc)) => {
                                captions.push(anc);
                            }
                            Err(err) => {
                                gst::error!(CAT, "Failed to parse NDI C608 metadata: {err}");
                            }
                        },
                        Err(err) => {
                            gst::error!(CAT, "Failed to decode NDI C608 metadata: {err}");
                        }
                    },
                    C708_TAG_BYTES => match BASE64.decode(self.xml_content.as_slice()) {
                        Ok(v210_buf) => match self.parse_for_cea708(&v210_buf) {
                            Ok(None) => (),
                            Ok(Some(anc)) => {
                                captions.push(anc);
                            }
                            Err(err) => {
                                gst::error!(CAT, "Failed to parse NDI C708 metadata: {err}");
                            }
                        },
                        Err(err) => {
                            gst::error!(CAT, "Failed to decode NDI C708 metadata: {err}");
                        }
                    },
                    _ => (),
                },
                _ => {}
            }

            self.xml_buf.clear();
        }

        Ok(captions)
    }

    fn parse_for_cea608(&mut self, input: &[u8]) -> Result<Option<VideoAncillary>> {
        let Some(anc) = self.parse(input)? else {
            return Ok(None);
        };

        if anc.did16() != VideoAncillaryDID16::S334Eia608 {
            bail!(NDICCError::new_unexpected_afd_did(
                anc.did16(),
                VideoAncillaryDID16::S334Eia608,
            ));
        }

        if anc.len() != 3 {
            bail!(NDICCError::UnexpectedAfdDataCount {
                found: anc.len() as u8,
                expected: 3,
            });
        }

        Ok(Some(anc))
    }

    fn parse_for_cea708(&mut self, input: &[u8]) -> Result<Option<VideoAncillary>> {
        let Some(anc) = self.parse(input)? else {
            return Ok(None);
        };

        if anc.did16() != VideoAncillaryDID16::S334Eia708 {
            bail!(NDICCError::new_unexpected_afd_did(
                anc.did16(),
                VideoAncillaryDID16::S334Eia708,
            ));
        }

        Ok(Some(anc))
    }

    fn parse(&mut self, data: &[u8]) -> Result<Option<VideoAncillary>> {
        if data.is_empty() {
            return Ok(None);
        }

        self.line_buf[0..data.len()].copy_from_slice(data);
        self.line_buf[data.len()..].fill(0);
        self.v210_parser.add_line(self.line_buf.as_slice())?;

        let opt = self.v210_parser.next_ancillary().transpose()?;

        Ok(opt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "sink")]
    use gst_video::VideoCaptionType;

    #[cfg(feature = "sink")]
    #[test]
    fn encode_gst_meta_c608() {
        gst::init().unwrap();

        let mut buf = gst::Buffer::new();
        {
            let buf = buf.get_mut().unwrap();
            gst_video::VideoCaptionMeta::add(
                buf,
                VideoCaptionType::Cea608S3341a,
                &[0x80, 0x94, 0x2c],
            );
        }

        let mut ndi_cc_encoder = NDICCMetaEncoder::new(1920);
        assert_eq!(
            ndi_cc_encoder.encode(&buf).unwrap().as_bytes(),
            b"<C608 line=\"128\">AAAAAP8D8D8AhAUAAgEwIAAABgCUAcASAJgKAAAAAAA=</C608>",
        );
    }

    #[cfg(feature = "sink")]
    #[test]
    fn encode_gst_meta_c708() {
        gst::init().unwrap();

        let mut buf = gst::Buffer::new();
        {
            let buf = buf.get_mut().unwrap();
            gst_video::VideoCaptionMeta::add(
                buf,
                VideoCaptionType::Cea708Cdp,
                &[
                    0x96, 0x69, 0x55, 0x3f, 0x43, 0x00, 0x00, 0x72, 0xf8, 0xfc, 0x94, 0x2c, 0xf9,
                    0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00,
                    0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00,
                    0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa,
                    0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00,
                    0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00,
                    0xfa, 0x00, 0x00, 0x74, 0x00, 0x00, 0x1b,
                ],
            );
        }

        let mut ndi_cc_encoder = NDICCMetaEncoder::new(1920);
        assert_eq!(
            ndi_cc_encoder.encode(&buf).unwrap().as_bytes(),
            b"<C708 line=\"10\">AAAAAP8D8D8AhAUAAQFQJQBYCgBpAlAlAPwIAEMBACAAAAgAcgKAHwDwCwCUAcASAOQLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADQCQAAAgAgAGwIALcCAAAAAAAAAAAAAA==</C708>",
        );
    }

    #[cfg(feature = "sink")]
    #[test]
    fn encode_gst_meta_c608_and_c708() {
        gst::init().unwrap();

        let mut buf = gst::Buffer::new();
        {
            let buf = buf.get_mut().unwrap();
            gst_video::VideoCaptionMeta::add(
                buf,
                VideoCaptionType::Cea608S3341a,
                &[0x80, 0x94, 0x2c],
            );
            gst_video::VideoCaptionMeta::add(
                buf,
                VideoCaptionType::Cea708Cdp,
                &[
                    0x96, 0x69, 0x55, 0x3f, 0x43, 0x00, 0x00, 0x72, 0xf8, 0xfc, 0x94, 0x2c, 0xf9,
                    0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00,
                    0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00,
                    0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa,
                    0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00,
                    0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00,
                    0xfa, 0x00, 0x00, 0x74, 0x00, 0x00, 0x1b,
                ],
            );
        }

        let mut ndi_cc_encoder = NDICCMetaEncoder::new(1920);
        assert_eq!(
            ndi_cc_encoder.encode(&buf).unwrap().as_bytes(),
            b"<C608 line=\"128\">AAAAAP8D8D8AhAUAAgEwIAAABgCUAcASAJgKAAAAAAA=</C608><C708 line=\"10\">AAAAAP8D8D8AhAUAAQFQJQBYCgBpAlAlAPwIAEMBACAAAAgAcgKAHwDwCwCUAcASAOQLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADQCQAAAgAgAGwIALcCAAAAAAAAAAAAAA==</C708>",
        );
    }

    #[cfg(feature = "sink")]
    #[test]
    fn encode_gst_meta_unsupported_cc() {
        gst::init().unwrap();

        let mut buf = gst::Buffer::new();
        {
            let buf = buf.get_mut().unwrap();
            gst_video::VideoCaptionMeta::add(
                buf,
                VideoCaptionType::Cea708Raw,
                // Content doesn't matter here
                &[0x00, 0x01, 0x02, 0x03, 0x04, 0x05],
            );
        }

        let mut ndi_cc_encoder = NDICCMetaEncoder::new(1920);
        assert!(ndi_cc_encoder.encode(&buf).is_none());
    }

    #[cfg(feature = "sink")]
    #[test]
    fn encode_gst_meta_none() {
        gst::init().unwrap();

        let buf = gst::Buffer::new();
        let mut ndi_cc_encoder = NDICCMetaEncoder::new(1920);
        assert!(ndi_cc_encoder.encode(&buf).is_none());
    }

    #[test]
    fn decode_ndi_meta_c608() {
        gst::init().unwrap();

        let mut ndi_cc_decoder = NDICCMetaDecoder::new(1920);
        let captions = ndi_cc_decoder
            .decode("<C608 line=\"128\">AAAAAP8D8D8AhAUAAgEwIAAABgCUAcASAJgKAAAAAAA=</C608>")
            .unwrap();

        assert_eq!(captions.len(), 1);
        assert_eq!(
            captions[0].did16(),
            gst_video::VideoAncillaryDID16::S334Eia608
        );
        assert_eq!(captions[0].data(), [0x80, 0x94, 0x2c]);
    }

    #[test]
    fn decode_ndi_meta_c708() {
        gst::init().unwrap();

        let mut ndi_cc_decoder = NDICCMetaDecoder::new(1920);
        let captions = ndi_cc_decoder.decode(
            "<C708 line=\"10\">AAAAAP8D8D8AhAUAAQFQJQBYCgBpAlAlAPwIAEMBACAAAAgAcgKAHwDwCwCUAcASAOQLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADQCQAAAgAgAGwIALcCAAAAAAAAAAAAAA==</C708>",
        )
        .unwrap();

        assert_eq!(captions.len(), 1);
        assert_eq!(
            captions[0].did16(),
            gst_video::VideoAncillaryDID16::S334Eia708
        );
        assert_eq!(
            captions[0].data(),
            [
                0x96, 0x69, 0x55, 0x3f, 0x43, 0x00, 0x00, 0x72, 0xf8, 0xfc, 0x94, 0x2c, 0xf9, 0x00,
                0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa,
                0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00,
                0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00,
                0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa,
                0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0x74, 0x00, 0x00,
                0x1b,
            ]
        );
    }

    #[test]
    fn decode_ndi_meta_c708_newlines_and_indent() {
        gst::init().unwrap();

        let mut ndi_cc_decoder = NDICCMetaDecoder::new(1920);
        let captions = ndi_cc_decoder
            .decode(
                r#"<C708 line=\"10\">
    AAAAAP8D8D8AhAUAAQFQJQBYCgBpAlAlAPwIAEMBACAAAAgAcgKAHwDwCwCUAcASAOQ
    LAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAA
    ACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACA
    CAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA
    6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADQCQAAAgAgAGwIALcCAAAAAAA
    AAAAAAA==
</C708>"#,
            )
            .unwrap();

        assert_eq!(captions.len(), 1);
        assert_eq!(
            captions[0].did16(),
            gst_video::VideoAncillaryDID16::S334Eia708
        );
        assert_eq!(
            captions[0].data(),
            [
                0x96, 0x69, 0x55, 0x3f, 0x43, 0x00, 0x00, 0x72, 0xf8, 0xfc, 0x94, 0x2c, 0xf9, 0x00,
                0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa,
                0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00,
                0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00,
                0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa,
                0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0x74, 0x00, 0x00,
                0x1b,
            ]
        );
    }

    #[test]
    fn decode_ndi_meta_c608_newlines_spaces_inline() {
        gst::init().unwrap();

        let mut ndi_cc_decoder = NDICCMetaDecoder::new(1920);
        let captions = ndi_cc_decoder.decode(
            "<C608 line=\"128\">\n\tAAAAAP8D8\n\n\r D8AhAUA\r\n\tAgEwIAAABgCUAcASAJgKAAAAAAA=  \n</C608>",
        )
        .unwrap();

        assert_eq!(captions.len(), 1);
        assert_eq!(
            captions[0].did16(),
            gst_video::VideoAncillaryDID16::S334Eia608
        );
        assert_eq!(captions[0].data(), [0x80, 0x94, 0x2c]);
    }

    #[test]
    fn decode_ndi_meta_c608_and_c708() {
        gst::init().unwrap();

        let mut ndi_cc_decoder = NDICCMetaDecoder::new(1920);
        let captions = ndi_cc_decoder.decode(
            "<C608 line=\"128\">AAAAAP8D8D8AhAUAAgEwIAAABgCUAcASAJgKAAAAAAA=</C608><C708 line=\"10\">AAAAAP8D8D8AhAUAAQFQJQBYCgBpAlAlAPwIAEMBACAAAAgAcgKAHwDwCwCUAcASAOQLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADoCwAAAgAgAOgLAAACACAA6AsAAAIAIADQCQAAAgAgAGwIALcCAAAAAAAAAAAAAA==</C708>",
        )
        .unwrap();

        assert_eq!(captions.len(), 2);
        assert_eq!(
            captions[0].did16(),
            gst_video::VideoAncillaryDID16::S334Eia608
        );
        assert_eq!(captions[0].data(), [0x80, 0x94, 0x2c]);

        assert_eq!(
            captions[1].did16(),
            gst_video::VideoAncillaryDID16::S334Eia708
        );
        assert_eq!(
            captions[1].data(),
            [
                0x96, 0x69, 0x55, 0x3f, 0x43, 0x00, 0x00, 0x72, 0xf8, 0xfc, 0x94, 0x2c, 0xf9, 0x00,
                0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa,
                0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00,
                0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00,
                0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa,
                0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0xfa, 0x00, 0x00, 0x74, 0x00, 0x00,
                0x1b,
            ]
        );
    }

    #[test]
    fn decode_ndi_meta_tag_mismatch() {
        gst::init().unwrap();

        // Expecting </C608> found </C708>'
        let mut ndi_cc_decoder = NDICCMetaDecoder::new(1920);
        ndi_cc_decoder
            .decode("<C608 line=\"128\">AAAAAP8D8D8AhAUAAgEwIAAABgCUAcASAJgKAAAAAAA=</C708>")
            .unwrap_err();
    }
}
