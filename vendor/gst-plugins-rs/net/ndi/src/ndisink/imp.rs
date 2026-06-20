// SPDX-License-Identifier: MPL-2.0

use glib::subclass::prelude::*;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_base::prelude::*;
use gst_base::subclass::prelude::*;

use std::sync::Mutex;

use std::sync::LazyLock;

use crate::ndi::SendInstance;
use crate::ndi_cc_meta::NDICCMetaEncoder;

static DEFAULT_SENDER_NDI_NAME: LazyLock<String> = LazyLock::new(|| {
    format!(
        "GStreamer NewTek NDI Sink {}-{}",
        env!("CARGO_PKG_VERSION"),
        env!("COMMIT_ID")
    )
});

#[derive(Debug)]
struct Settings {
    ndi_name: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            ndi_name: DEFAULT_SENDER_NDI_NAME.clone(),
        }
    }
}

struct State {
    send: SendInstance,
    video_info: Option<gst_video::VideoInfo>,
    ndi_cc_encoder: Option<NDICCMetaEncoder>,
    audio_info: Option<gst_audio::AudioInfo>,
}

pub struct NdiSink {
    settings: Mutex<Settings>,
    state: Mutex<Option<State>>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new("ndisink", gst::DebugColorFlags::empty(), Some("NDI Sink"))
});

#[glib::object_subclass]
impl ObjectSubclass for NdiSink {
    const NAME: &'static str = "GstNdiSink";
    type Type = super::NdiSink;
    type ParentType = gst_base::BaseSink;

    fn new() -> Self {
        Self {
            settings: Mutex::new(Default::default()),
            state: Mutex::new(Default::default()),
        }
    }
}

impl ObjectImpl for NdiSink {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![glib::ParamSpecString::builder("ndi-name")
                .nick("NDI Name")
                .blurb("NDI Name to use")
                .doc_show_default()
                .build()]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "ndi-name" => {
                let mut settings = self.settings.lock().unwrap();
                settings.ndi_name = value
                    .get::<String>()
                    .unwrap_or_else(|_| DEFAULT_SENDER_NDI_NAME.clone());
            }
            _ => unimplemented!(),
        };
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "ndi-name" => {
                let settings = self.settings.lock().unwrap();
                settings.ndi_name.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for NdiSink {}

impl ElementImpl for NdiSink {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "NewTek NDI Sink",
                "Sink/Audio/Video",
                "NewTek NDI Sink",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::builder_full()
                .structure(
                    gst::Structure::builder("video/x-raw")
                        .field(
                            "format",
                            gst::List::new([
                                gst_video::VideoFormat::Uyvy.to_str(),
                                gst_video::VideoFormat::I420.to_str(),
                                gst_video::VideoFormat::Nv12.to_str(),
                                gst_video::VideoFormat::Nv21.to_str(),
                                gst_video::VideoFormat::Yv12.to_str(),
                                gst_video::VideoFormat::Bgra.to_str(),
                                gst_video::VideoFormat::Bgrx.to_str(),
                                gst_video::VideoFormat::Rgba.to_str(),
                                gst_video::VideoFormat::Rgbx.to_str(),
                            ]),
                        )
                        .field("width", gst::IntRange::<i32>::new(1, i32::MAX))
                        .field("height", gst::IntRange::<i32>::new(1, i32::MAX))
                        .field(
                            "framerate",
                            gst::FractionRange::new(
                                gst::Fraction::new(0, 1),
                                gst::Fraction::new(i32::MAX, 1),
                            ),
                        )
                        .build(),
                )
                .structure(
                    gst::Structure::builder("audio/x-raw")
                        .field("format", gst_audio::AUDIO_FORMAT_F32.to_str())
                        .field("rate", gst::IntRange::<i32>::new(1, i32::MAX))
                        .field("channels", gst::IntRange::<i32>::new(1, i32::MAX))
                        .field("layout", "interleaved")
                        .build(),
                )
                .build();

            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();
            vec![sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    #[allow(clippy::single_match)]
    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        match transition {
            gst::StateChange::NullToReady => {
                if let Err(err) = crate::ndi::load() {
                    gst::element_imp_error!(self, gst::LibraryError::Init, ("{}", err));
                    return Err(gst::StateChangeError);
                }
            }
            _ => (),
        }

        self.parent_change_state(transition)
    }
}

impl BaseSinkImpl for NdiSink {
    fn start(&self) -> Result<(), gst::ErrorMessage> {
        let mut state_storage = self.state.lock().unwrap();
        let settings = self.settings.lock().unwrap();

        let send = SendInstance::builder(&settings.ndi_name)
            .build()
            .ok_or_else(|| {
                gst::error_msg!(
                    gst::ResourceError::OpenWrite,
                    ["Could not create send instance"]
                )
            })?;

        let state = State {
            send,
            video_info: None,
            ndi_cc_encoder: None,
            audio_info: None,
        };
        *state_storage = Some(state);
        gst::info!(CAT, imp = self, "Started");

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        let mut state_storage = self.state.lock().unwrap();

        *state_storage = None;
        gst::info!(CAT, imp = self, "Stopped");

        Ok(())
    }

    fn unlock(&self) -> Result<(), gst::ErrorMessage> {
        Ok(())
    }

    fn unlock_stop(&self) -> Result<(), gst::ErrorMessage> {
        Ok(())
    }

    fn set_caps(&self, caps: &gst::Caps) -> Result<(), gst::LoggableError> {
        gst::debug!(CAT, imp = self, "Setting caps {}", caps);

        let mut state_storage = self.state.lock().unwrap();
        let state = match &mut *state_storage {
            None => return Err(gst::loggable_error!(CAT, "Sink not started yet")),
            Some(ref mut state) => state,
        };

        let s = caps.structure(0).unwrap();
        if s.name() == "video/x-raw" {
            let info = gst_video::VideoInfo::from_caps(caps)
                .map_err(|_| gst::loggable_error!(CAT, "Couldn't parse caps {}", caps))?;

            state.ndi_cc_encoder = Some(NDICCMetaEncoder::new(info.width()));
            state.video_info = Some(info);
            state.audio_info = None;
        } else {
            let info = gst_audio::AudioInfo::from_caps(caps)
                .map_err(|_| gst::loggable_error!(CAT, "Couldn't parse caps {}", caps))?;

            state.audio_info = Some(info);
            state.video_info = None;
            state.ndi_cc_encoder = None;
        }

        Ok(())
    }

    fn render(&self, buffer: &gst::Buffer) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state_storage = self.state.lock().unwrap();
        let state = match &mut *state_storage {
            None => return Err(gst::FlowError::Error),
            Some(ref mut state) => state,
        };

        if let Some(ref info) = state.video_info {
            if let Some(audio_meta) = buffer.meta::<crate::ndisinkmeta::NdiSinkAudioMeta>() {
                for (buffer, info, timecode) in audio_meta.buffers() {
                    let frame = crate::ndi::AudioFrame::try_from_buffer(info, buffer, *timecode)
                        .map_err(|_| {
                            gst::error!(CAT, imp = self, "Unsupported audio frame");
                            gst::FlowError::NotNegotiated
                        })?;

                    gst::trace!(
                        CAT,
                        imp = self,
                        "Sending audio buffer {:?} with timecode {} and format {:?}",
                        buffer,
                        if *timecode < 0 {
                            gst::ClockTime::NONE.display()
                        } else {
                            Some((*timecode as u64 * 100).nseconds()).display()
                        },
                        info,
                    );
                    state.send.send_audio(&frame);
                }
            }

            // Skip empty/gap buffers from ndisinkcombiner
            if buffer.size() != 0 {
                let timecode = self
                    .obj()
                    .segment()
                    .downcast::<gst::ClockTime>()
                    .ok()
                    .and_then(|segment| {
                        segment
                            .to_running_time(buffer.pts())
                            .zip(self.obj().base_time())
                    })
                    .and_then(|(running_time, base_time)| running_time.checked_add(base_time))
                    .map(|time| (time.nseconds() / 100) as i64)
                    .unwrap_or(crate::ndisys::NDIlib_send_timecode_synthesize);

                let mut ndi_meta = None;
                if let Some(ref mut ndi_cc_encoder) = state.ndi_cc_encoder {
                    // handle potential width change
                    ndi_cc_encoder.set_width(info.width());
                    ndi_meta = ndi_cc_encoder.encode(buffer);
                }

                let frame = gst_video::VideoFrame::from_buffer_readable(buffer.clone(), info)
                    .map_err(|_| {
                        gst::error!(CAT, imp = self, "Failed to map buffer");
                        gst::FlowError::Error
                    })?;

                let frame = crate::ndi::VideoFrame::try_from_video_frame(frame, ndi_meta, timecode)
                    .map_err(|_| {
                        gst::error!(CAT, imp = self, "Unsupported video frame");
                        gst::FlowError::NotNegotiated
                    })?;

                gst::trace!(
                    CAT,
                    imp = self,
                    "Sending video buffer {:?} with timecode {} and format {:?}",
                    buffer,
                    if timecode < 0 {
                        gst::ClockTime::NONE.display()
                    } else {
                        Some((timecode as u64 * 100).nseconds()).display()
                    },
                    info
                );
                state.send.send_video(&frame);
            }
        } else if let Some(ref info) = state.audio_info {
            let timecode = self
                .obj()
                .segment()
                .downcast::<gst::ClockTime>()
                .ok()
                .and_then(|segment| {
                    segment
                        .to_running_time(buffer.pts())
                        .zip(self.obj().base_time())
                })
                .and_then(|(running_time, base_time)| running_time.checked_add(base_time))
                .map(|time| (time.nseconds() / 100) as i64)
                .unwrap_or(crate::ndisys::NDIlib_send_timecode_synthesize);

            let frame =
                crate::ndi::AudioFrame::try_from_buffer(info, buffer, timecode).map_err(|_| {
                    gst::error!(CAT, imp = self, "Unsupported audio frame");
                    gst::FlowError::NotNegotiated
                })?;

            gst::trace!(
                CAT,
                imp = self,
                "Sending audio buffer {:?} with timecode {} and format {:?}",
                buffer,
                if timecode < 0 {
                    gst::ClockTime::NONE.display()
                } else {
                    Some((timecode as u64 * 100).nseconds()).display()
                },
                info,
            );
            state.send.send_audio(&frame);
        } else {
            return Err(gst::FlowError::Error);
        }

        Ok(gst::FlowSuccess::Ok)
    }
}
