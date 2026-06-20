use gst::glib;
use gst::subclass::prelude::*;
use gst_rtp::prelude::*;
use gst_rtp::subclass::prelude::*;
use std::sync::LazyLock;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtponvifmetadatapay",
        gst::DebugColorFlags::empty(),
        Some("RTP ONVIF metadata payloader"),
    )
});

#[derive(Default)]
pub struct OnvifMetadataPay {}

#[glib::object_subclass]
impl ObjectSubclass for OnvifMetadataPay {
    const NAME: &'static str = "GstOnvifMetadataPay";
    type Type = super::OnvifMetadataPay;
    type ParentType = gst_rtp::RTPBasePayload;
}

impl ObjectImpl for OnvifMetadataPay {}

impl GstObjectImpl for OnvifMetadataPay {}

impl ElementImpl for OnvifMetadataPay {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "ONVIF metadata RTP payloader",
                "Payloader/Network/RTP",
                "ONVIF metadata RTP payloader",
                "Mathieu Duponchelle <mathieu@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let sink_caps = gst::Caps::builder("application/x-onvif-metadata").build();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &sink_caps,
            )
            .unwrap();

            let src_caps = gst::Caps::builder("application/x-rtp")
                .field("media", "application")
                .field("payload", gst::IntRange::new(96, 127))
                .field("clock-rate", 90000)
                .field("encoding-name", "VND.ONVIF.METADATA")
                .build();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &src_caps,
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl RTPBasePayloadImpl for OnvifMetadataPay {
    fn handle_buffer(&self, buffer: gst::Buffer) -> Result<gst::FlowSuccess, gst::FlowError> {
        let pts = buffer.pts();
        let dts = buffer.dts();

        // Input buffer must be readable
        let buffer = buffer.into_mapped_buffer_readable().map_err(|_| {
            gst::element_imp_error!(
                self,
                gst::ResourceError::Read,
                ["Failed to map buffer readable"]
            );

            gst::FlowError::Error
        })?;

        // Input buffer must be valid UTF-8
        let utf8 = std::str::from_utf8(buffer.as_ref()).map_err(|err| {
            gst::element_imp_error!(
                self,
                gst::StreamError::Format,
                ["Failed to decode buffer as UTF-8: {}", err]
            );

            gst::FlowError::Error
        })?;

        // Input buffer must start with a tt:MetadataStream node
        let process = {
            let mut process = false;

            for token in xmlparser::Tokenizer::from(utf8) {
                match token {
                    Ok(token) => match token {
                        xmlparser::Token::Comment { .. } => {
                            continue;
                        }
                        xmlparser::Token::Declaration { .. } => {
                            continue;
                        }
                        xmlparser::Token::ElementStart { local, .. } => {
                            if local.as_str() == "MetadataStream" {
                                process = true;
                            }
                            break;
                        }
                        _ => {
                            process = false;
                            break;
                        }
                    },
                    Err(err) => {
                        gst::element_imp_error!(
                            self,
                            gst::StreamError::Format,
                            ["Invalid XML: {}", err]
                        );

                        return Err(gst::FlowError::Error);
                    }
                }
            }

            process
        };

        if !process {
            gst::element_imp_error!(
                self,
                gst::StreamError::Format,
                ["document must start with tt:MetadataStream element"]
            );

            return Err(gst::FlowError::Error);
        }

        let mtu = self.obj().mtu();
        let payload_size = gst_rtp::calc_payload_len(mtu, 0, 0) as usize;

        let mut chunks = utf8.as_bytes().chunks(payload_size).peekable();
        let mut chunk_offset: usize = 0;
        let mut buflist = gst::BufferList::new_sized((utf8.len() / payload_size) + 1);

        {
            let buflist_mut = buflist.get_mut().unwrap();

            while let Some(chunk) = chunks.next() {
                let chunk_end = chunk_offset + chunk.len();
                let mut outbuf = gst::Buffer::new_rtp_with_sizes(chunk.len() as u32, 0, 0)
                    .map_err(|err| {
                        gst::element_imp_error!(
                            self,
                            gst::ResourceError::Write,
                            ["Failed to allocate output buffer: {}", err]
                        );

                        gst::FlowError::Error
                    })?;

                {
                    let outbuf_mut = outbuf.get_mut().unwrap();
                    buffer
                        .buffer()
                        .copy_into(
                            outbuf_mut,
                            gst::BUFFER_COPY_METADATA,
                            chunk_offset..chunk_end,
                        )
                        .map_err(|err| {
                            gst::element_imp_error!(
                                self,
                                gst::ResourceError::Write,
                                ["Failed to copy metadata into output buffer: {}", err]
                            );

                            gst::FlowError::Error
                        })?;

                    outbuf_mut.set_pts(pts);
                    outbuf_mut.set_dts(dts);

                    let mut outrtp = gst_rtp::RTPBuffer::from_buffer_writable(outbuf_mut).unwrap();
                    let payload = outrtp.payload_mut().unwrap();
                    payload.copy_from_slice(chunk);

                    // Last chunk, set marker bit
                    if chunks.peek().is_none() {
                        outrtp.set_marker(true);
                    }
                }

                buflist_mut.add(outbuf);
                chunk_offset = chunk_end;
            }
        }

        self.obj().push_list(buflist)
    }

    fn set_caps(&self, _caps: &gst::Caps) -> Result<(), gst::LoggableError> {
        self.obj()
            .set_options("application", true, "VND.ONVIF.METADATA", 90000);
        self.obj()
            .set_outcaps(None)
            .map_err(|_| gst::loggable_error!(CAT, "Failed to set output caps"))
    }
}
