use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::File,
    io::{BufReader, Cursor},
    ops::Deref,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, LazyLock,
    },
};
use tokio_rustls::{rustls, TlsAcceptor, TlsConnector};

use anyhow::{anyhow, Context, Error};
use gst::{glib, prelude::*};

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "webrtcutils",
        gst::DebugColorFlags::empty(),
        Some("WebRTC Utils"),
    )
});

pub fn gvalue_to_json(val: &gst::glib::Value) -> Option<serde_json::Value> {
    match val.type_() {
        glib::Type::STRING => Some(val.get::<String>().unwrap().into()),
        glib::Type::BOOL => Some(val.get::<bool>().unwrap().into()),
        glib::Type::I32 => Some(val.get::<i32>().unwrap().into()),
        glib::Type::U32 => Some(val.get::<u32>().unwrap().into()),
        glib::Type::I_LONG | glib::Type::I64 => Some(val.get::<i64>().unwrap().into()),
        glib::Type::U_LONG | glib::Type::U64 => Some(val.get::<u64>().unwrap().into()),
        glib::Type::F32 => Some(val.get::<f32>().unwrap().into()),
        glib::Type::F64 => Some(val.get::<f64>().unwrap().into()),
        _ => {
            if let Ok(s) = val.get::<gst::Structure>() {
                serde_json::to_value(
                    s.iter()
                        .filter_map(|(name, value)| {
                            gvalue_to_json(value).map(|value| (name.to_string(), value))
                        })
                        .collect::<HashMap<String, serde_json::Value>>(),
                )
                .ok()
            } else if let Ok(a) = val.get::<gst::Array>() {
                serde_json::to_value(
                    a.iter()
                        .filter_map(|value| gvalue_to_json(value))
                        .collect::<Vec<serde_json::Value>>(),
                )
                .ok()
            } else if let Some((_klass, values)) = gst::glib::FlagsValue::from_value(val) {
                Some(
                    values
                        .iter()
                        .map(|value| value.nick())
                        .collect::<Vec<&str>>()
                        .join("+")
                        .into(),
                )
            } else if let Ok(value) = val.serialize() {
                Some(value.as_str().into())
            } else {
                None
            }
        }
    }
}

fn json_to_gststructure(val: &serde_json::Value) -> Option<glib::SendValue> {
    match val {
        serde_json::Value::Bool(v) => Some(v.to_send_value()),
        serde_json::Value::Number(n) => {
            if n.is_u64() {
                Some(n.as_u64().unwrap().to_send_value())
            } else if n.is_i64() {
                Some(n.as_i64().unwrap().to_send_value())
            } else if n.is_f64() {
                Some(n.as_f64().unwrap().to_send_value())
            } else {
                todo!("Unhandled case {n:?}");
            }
        }
        serde_json::Value::String(v) => Some(v.to_send_value()),
        serde_json::Value::Array(v) => {
            let array = v
                .iter()
                .filter_map(json_to_gststructure)
                .collect::<Vec<glib::SendValue>>();
            Some(gst::Array::from_values(array).to_send_value())
        }
        serde_json::Value::Object(v) => Some(serialize_json_object(v).to_send_value()),
        _ => None,
    }
}

pub fn serialize_json_object(val: &serde_json::Map<String, serde_json::Value>) -> gst::Structure {
    let mut res = gst::Structure::new_empty("v");

    val.iter().for_each(|(k, v)| {
        if let Some(gvalue) = json_to_gststructure(v) {
            res.set_value(k, gvalue);
        }
    });

    res
}

use crate::RUNTIME;
use futures::future;
use futures::prelude::*;
use gst::ErrorMessage;
#[cfg(any(feature = "whip", feature = "whep"))]
use reqwest::header::HeaderMap;
#[cfg(any(feature = "whip", feature = "whep"))]
use reqwest::redirect::Policy;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Debug)]
pub enum WaitError {
    FutureAborted,
    FutureError(ErrorMessage),
}

pub async fn wait_async<F, T>(
    canceller: &Mutex<Option<future::AbortHandle>>,
    future: F,
    timeout: u32,
) -> Result<T, WaitError>
where
    F: Send + Future<Output = T>,
    T: Send + 'static,
{
    let (abort_handle, abort_registration) = future::AbortHandle::new_pair();
    {
        let mut canceller_guard = canceller.lock().unwrap();
        if canceller_guard.is_some() {
            return Err(WaitError::FutureError(gst::error_msg!(
                gst::ResourceError::Failed,
                ["Old Canceller should not exist"]
            )));
        }

        canceller_guard.replace(abort_handle);
        drop(canceller_guard);
    }

    let future = async {
        if timeout == 0 {
            Ok(future.await)
        } else {
            let res = tokio::time::timeout(Duration::from_secs(timeout.into()), future).await;

            match res {
                Ok(r) => Ok(r),
                Err(e) => Err(WaitError::FutureError(gst::error_msg!(
                    gst::ResourceError::Read,
                    ["Request timeout, elapsed: {}", e]
                ))),
            }
        }
    };

    let future = async {
        match future::Abortable::new(future, abort_registration).await {
            Ok(Ok(r)) => Ok(r),

            Ok(Err(err)) => Err(WaitError::FutureError(gst::error_msg!(
                gst::ResourceError::Failed,
                ["Future resolved with an error {:?}", err]
            ))),

            Err(future::Aborted) => Err(WaitError::FutureAborted),
        }
    };

    let res = future.await;

    let mut canceller_guard = canceller.lock().unwrap();
    *canceller_guard = None;

    res
}

pub fn wait<F, T>(
    canceller: &Mutex<Option<future::AbortHandle>>,
    future: F,
    timeout: u32,
) -> Result<T, WaitError>
where
    F: Send + Future<Output = Result<T, ErrorMessage>>,
    T: Send + 'static,
{
    let mut canceller_guard = canceller.lock().unwrap();
    let (abort_handle, abort_registration) = future::AbortHandle::new_pair();

    if canceller_guard.is_some() {
        return Err(WaitError::FutureError(gst::error_msg!(
            gst::ResourceError::Failed,
            ["Old Canceller should not exist"]
        )));
    }

    canceller_guard.replace(abort_handle);
    drop(canceller_guard);

    let future = async {
        if timeout == 0 {
            future.await
        } else {
            let res = tokio::time::timeout(Duration::from_secs(timeout.into()), future).await;

            match res {
                Ok(r) => r,
                Err(e) => Err(gst::error_msg!(
                    gst::ResourceError::Read,
                    ["Request timeout, elapsed: {}", e.to_string()]
                )),
            }
        }
    };

    let future = async {
        match future::Abortable::new(future, abort_registration).await {
            Ok(Ok(res)) => Ok(res),

            Ok(Err(err)) => Err(WaitError::FutureError(gst::error_msg!(
                gst::ResourceError::Failed,
                ["Future resolved with an error {:?}", err]
            ))),

            Err(future::Aborted) => Err(WaitError::FutureAborted),
        }
    };

    let res = {
        let _enter = RUNTIME.enter();
        futures::executor::block_on(future)
    };

    canceller_guard = canceller.lock().unwrap();
    *canceller_guard = None;

    res
}

#[cfg(any(feature = "whip", feature = "whep"))]
pub fn parse_redirect_location(
    headermap: &HeaderMap,
    old_url: &reqwest::Url,
) -> Result<reqwest::Url, ErrorMessage> {
    let location = match headermap.get(reqwest::header::LOCATION) {
        Some(location) => location,
        None => {
            return Err(gst::error_msg!(
                gst::ResourceError::Failed,
                ["Location header field should be present for WHIP/WHEP resource URL"]
            ));
        }
    };

    let location = match location.to_str() {
        Ok(loc) => loc,
        Err(e) => {
            return Err(gst::error_msg!(
                gst::ResourceError::Failed,
                ["Failed to convert location to string {}", e]
            ));
        }
    };

    match reqwest::Url::parse(location) {
        Ok(url) => Ok(url), // Location URL is an absolute path
        Err(_) => {
            // Location URL is a relative path
            let new_url = old_url.clone().join(location).map_err(|err| {
                gst::error_msg!(
                    gst::ResourceError::Failed,
                    ["URL join operation failed: {:?}", err]
                )
            })?;

            Ok(new_url)
        }
    }
}

#[cfg(any(feature = "whip", feature = "whep"))]
pub fn build_reqwest_client(pol: Policy) -> reqwest::Client {
    let client_builder = reqwest::Client::builder();
    client_builder.redirect(pol).build().unwrap()
}

#[cfg(any(feature = "whip", feature = "whep"))]
pub fn set_ice_servers(
    webrtcbin: &gst::Element,
    headermap: &HeaderMap,
) -> Result<(), ErrorMessage> {
    for link in headermap.get_all("link").iter() {
        let link = link.to_str().map_err(|err| {
            gst::error_msg!(
                gst::ResourceError::Failed,
                [
                    "Header value should contain only visible ASCII strings: {}",
                    err
                ]
            )
        })?;

        let item_map = match parse_link_header::parse_with_rel(link) {
            Ok(map) => map,
            Err(_) => continue,
        };

        let link = match item_map.contains_key("ice-server") {
            true => item_map.get("ice-server").unwrap(),
            false => continue, // Not a link header we care about
        };

        // Note: webrtcbin needs ice servers to be in the below format
        // <scheme>://<user:pass>@<url>
        // and the ice-servers (link headers) received from the whip server might be
        // in the format <scheme>:<host> with username and password as separate params.
        // Constructing these with 'url' crate also require a format/parse
        // for changing <scheme>:<host> to <scheme>://<user>:<password>@<host>.
        // So preferred to use the String rather

        // check if uri has ://
        let ice_server_url = if link.uri.has_authority() {
            // use raw_uri as is
            // username and password in the link.uri.params ignored
            link.uri.clone()
        } else {
            // No builder pattern is provided by reqwest::Url. Use string operation.
            // construct url as '<scheme>://<user:pass>@<url>'
            let url = format!("{}://{}", link.uri.scheme(), link.uri.path());

            let mut new_url = match reqwest::Url::parse(url.as_str()) {
                Ok(url) => url,
                Err(_) => continue,
            };

            if let Some(user) = link.params.get("username") {
                new_url.set_username(user.as_str()).unwrap();
                if let Some(pass) = link.params.get("credential") {
                    new_url.set_password(Some(pass.as_str())).unwrap();
                }
            }

            new_url
        };

        // It's nicer to not collapse the `else if` and its inner `if`
        #[allow(clippy::collapsible_if)]
        if link.uri.scheme() == "stun" {
            webrtcbin.set_property_from_str("stun-server", ice_server_url.as_str());
        } else if link.uri.scheme().starts_with("turn") {
            if !webrtcbin.emit_by_name::<bool>("add-turn-server", &[&ice_server_url.as_str()]) {
                return Err(gst::error_msg!(
                    gst::ResourceError::Failed,
                    ["Failed to set turn server {}", ice_server_url]
                ));
            }
        }
    }

    Ok(())
}

pub fn build_link_header(url_str: &str) -> Result<String, url::ParseError> {
    let url = url::Url::parse(url_str)?;

    let mut link_str: String = "<".to_owned() + url.scheme();
    if let Some(host) = url.host_str() {
        link_str = link_str + ":" + host;
    }

    if let Some(port) = url.port() {
        link_str = link_str + ":" + port.to_string().as_str();
    }

    link_str += url.path();

    if let Some(query) = url.query() {
        link_str = link_str + "?" + query;
    }

    link_str += ">";

    if let Some(password) = url.password() {
        link_str = link_str
            + "; "
            + "rel=\"ice-server\""
            + "; "
            + "username=\""
            + url.username()
            + "\"; "
            + "credential:\""
            + password
            + "\"; "
            + "credential-type:\"password\";";
    }

    Ok(link_str)
}

/// Wrapper around `gst::ElementFactory::make` with a better error
/// message
pub fn make_element(element: &str, name: Option<&str>) -> Result<gst::Element, Error> {
    gst::ElementFactory::make(element)
        .name_if_some(name)
        .build()
        .with_context(|| format!("Failed to make element {element}"))
}

#[derive(Debug)]
struct DecodingInfo {
    has_decoder: AtomicBool,
}

impl Clone for DecodingInfo {
    fn clone(&self) -> Self {
        Self {
            has_decoder: AtomicBool::new(self.has_decoder.load(Ordering::SeqCst)),
        }
    }
}

#[derive(Clone, Debug)]
struct EncodingInfo {
    encoder: Option<gst::ElementFactory>,
    payloader: gst::ElementFactory,
    output_filter: Option<gst::Caps>,
}

#[derive(Clone, Debug)]
pub struct Codec {
    pub name: String,
    pub caps: gst::Caps,
    pub stream_type: gst::StreamType,
    pub is_raw: bool,

    payload_type: Option<i32>,
    clock_rate: Option<i32>,
    decoding_info: Option<DecodingInfo>,
    encoding_info: Option<EncodingInfo>,
}

impl Codec {
    pub fn new(
        name: &str,
        stream_type: gst::StreamType,
        caps: &gst::Caps,
        decoders: &glib::List<gst::ElementFactory>,
        depayloaders: &glib::List<gst::ElementFactory>,
        encoders: &glib::List<gst::ElementFactory>,
        payloaders: &glib::List<gst::ElementFactory>,
    ) -> Self {
        let mut clock_rate: Option<i32> = None;
        let has_decoder = Self::has_decoder_for_caps(caps, decoders);
        let has_depayloader = Self::has_depayloader_for_codec(name, depayloaders, &mut clock_rate);

        let decoding_info = if has_depayloader && has_decoder {
            Some(DecodingInfo {
                has_decoder: AtomicBool::new(has_decoder),
            })
        } else {
            None
        };

        let encoder = Self::get_encoder_for_caps(caps, encoders);
        let payloader = Self::get_payloader_for_codec(name, payloaders);

        let encoding_info = if let (encoder, Some(payloader)) = (encoder, payloader) {
            Some(EncodingInfo {
                encoder,
                payloader,
                output_filter: None,
            })
        } else {
            None
        };

        Self {
            caps: caps.clone(),
            stream_type,
            name: name.into(),
            is_raw: false,
            payload_type: None,
            clock_rate,
            decoding_info,
            encoding_info,
        }
    }

    pub fn new_raw(
        name: &str,
        stream_type: gst::StreamType,
        depayloaders: &glib::List<gst::ElementFactory>,
        payloaders: &glib::List<gst::ElementFactory>,
    ) -> Self {
        let mut clock_rate: Option<i32> = None;
        let decoding_info = if Self::has_depayloader_for_codec(name, depayloaders, &mut clock_rate)
        {
            Some(DecodingInfo {
                has_decoder: AtomicBool::new(false),
            })
        } else {
            None
        };

        let payloader = Self::get_payloader_for_codec(name, payloaders);
        let encoding_info = payloader.map(|payloader| EncodingInfo {
            encoder: None,
            payloader,
            output_filter: None,
        });

        let mut caps = None;
        if let Some(elem) = Codec::get_payloader_for_codec(name, payloaders) {
            if let Some(tmpl) = elem
                .static_pad_templates()
                .iter()
                .find(|template| template.direction() == gst::PadDirection::Sink)
            {
                caps = Some(tmpl.caps());
            }
        }

        Self {
            caps: caps.unwrap_or_else(gst::Caps::new_empty),
            stream_type,
            name: name.into(),
            is_raw: true,
            payload_type: None,
            clock_rate,
            decoding_info,
            encoding_info,
        }
    }

    pub fn can_encode(&self) -> bool {
        self.encoding_info.is_some()
    }

    pub fn set_pt(&mut self, pt: i32) {
        self.payload_type = Some(pt);
    }

    pub fn can_be_received(&self) -> bool {
        if self.decoding_info.is_none() {
            return false;
        }

        if self.is_raw {
            return true;
        }

        let decoder_info = self.decoding_info.as_ref().unwrap();
        if decoder_info.has_decoder.load(Ordering::SeqCst) {
            true
        } else if Self::has_decoder_for_caps(
            &self.caps,
            // Replicating decodebin logic
            &gst::ElementFactory::factories_with_type(
                gst::ElementFactoryType::DECODER,
                gst::Rank::MARGINAL,
            ),
        ) {
            // Check if new decoders have been installed meanwhile
            decoder_info.has_decoder.store(true, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    fn get_encoder_for_caps(
        caps: &gst::Caps,
        encoders: &glib::List<gst::ElementFactory>,
    ) -> Option<gst::ElementFactory> {
        encoders
            .iter()
            .find(|factory| {
                factory.static_pad_templates().iter().any(|template| {
                    let template_caps = template.caps();
                    template.direction() == gst::PadDirection::Src
                        && !template_caps.is_any()
                        && caps.can_intersect(&template_caps)
                })
            })
            .cloned()
    }

    fn get_payloader_for_codec(
        codec: &str,
        payloaders: &glib::List<gst::ElementFactory>,
    ) -> Option<gst::ElementFactory> {
        payloaders
            .iter()
            .find(|factory| {
                factory.static_pad_templates().iter().any(|template| {
                    let template_caps = template.caps();

                    if template.direction() != gst::PadDirection::Src || template_caps.is_any() {
                        return false;
                    }

                    template_caps.iter().any(|s| {
                        s.has_field("encoding-name")
                            && s.get::<gst::List>("encoding-name").map_or_else(
                                |_| {
                                    if let Ok(encoding_name) = s.get::<&str>("encoding-name") {
                                        encoding_name == codec
                                    } else {
                                        false
                                    }
                                },
                                |encoding_names| {
                                    encoding_names.iter().any(|v| {
                                        v.get::<&str>()
                                            .is_ok_and(|encoding_name| encoding_name == codec)
                                    })
                                },
                            )
                    })
                })
            })
            .cloned()
    }

    fn has_decoder_for_caps(caps: &gst::Caps, decoders: &glib::List<gst::ElementFactory>) -> bool {
        decoders.iter().any(|factory| {
            factory.static_pad_templates().iter().any(|template| {
                let template_caps = template.caps();
                template.direction() == gst::PadDirection::Sink
                    && !template_caps.is_any()
                    && caps.can_intersect(&template_caps)
            })
        })
    }

    fn has_depayloader_for_codec(
        codec: &str,
        depayloaders: &glib::List<gst::ElementFactory>,
        clock_rate: &mut Option<i32>,
    ) -> bool {
        depayloaders.iter().any(|factory| {
            factory.static_pad_templates().iter().any(|template| {
                let template_caps = template.caps();

                if template.direction() != gst::PadDirection::Sink {
                    return false;
                }

                template_caps.iter().any(|s| {
                    s.has_field("encoding-name")
                        && s.get::<gst::List>("encoding-name").map_or_else(
                            |_| {
                                if let Ok(encoding_name) = s.get::<&str>("encoding-name") {
                                    if encoding_name == codec {
                                        if s.has_field("clock-rate") {
                                            match s.get_optional::<i32>("clock-rate") {
                                                Ok(Some(rate)) => {
                                                    *clock_rate = Some(rate);
                                                }
                                                _ => {
                                                    // if None or Err or IntRange
                                                    *clock_rate = None;
                                                }
                                            };
                                        }
                                        true
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                }
                            },
                            |encoding_names| {
                                encoding_names.iter().any(|v| v.get::<&str>() == Ok(codec))
                            },
                        )
                })
            })
        })
    }

    pub fn is_video(&self) -> bool {
        matches!(self.stream_type, gst::StreamType::VIDEO)
    }

    pub fn payload(&self) -> Option<i32> {
        self.payload_type
    }

    pub fn clock_rate(&self) -> Option<i32> {
        self.clock_rate
    }

    pub fn build_encoder(&self) -> Option<Result<gst::Element, Error>> {
        self.encoding_info.as_ref().and_then(|info| {
            info.encoder.as_ref().map(|encoder| {
                encoder
                    .create()
                    .build()
                    .with_context(|| format!("Creating encoder {}", encoder.name()))
            })
        })
    }

    pub fn create_payloader(&self) -> Option<gst::Element> {
        self.encoding_info
            .as_ref()
            .map(|info| info.payloader.create().build().unwrap())
    }

    pub fn raw_converter_filter(&self) -> Result<gst::Element, Error> {
        let caps = if self.is_video() {
            let mut structure_builder = gst::Structure::builder("video/x-raw")
                .field("pixel-aspect-ratio", gst::Fraction::new(1, 1));

            if self
                .encoder_name()
                .map(|e| e.as_str() == "nvh264enc")
                .unwrap_or(false)
            {
                // Quirk: nvh264enc can perform conversion from RGB formats, but
                // doesn't advertise / negotiate colorimetry correctly, leading
                // to incorrect color display in Chrome (but interestingly not in
                // Firefox). In any case, restrict to exclude RGB formats altogether,
                // and let videoconvert do the conversion properly if needed.
                structure_builder =
                    structure_builder.field("format", gst::List::new(["NV12", "YV12", "I420"]));
            }

            gst::Caps::builder_full_with_any_features()
                .structure(structure_builder.build())
                .build()
        } else {
            gst::Caps::builder("audio/x-raw").build()
        };

        gst::ElementFactory::make("capsfilter")
            .property("caps", &caps)
            .property_from_str("caps-change-mode", "delayed")
            .build()
            .with_context(|| "Creating capsfilter caps")
    }

    pub fn encoder_factory(&self) -> Option<gst::ElementFactory> {
        self.encoding_info
            .as_ref()
            .and_then(|info| info.encoder.clone())
    }

    pub fn encoder_name(&self) -> Option<String> {
        self.encoding_info.as_ref().and_then(|info| {
            info.encoder
                .as_ref()
                .map(|encoder| encoder.name().to_string())
        })
    }

    pub fn set_output_filter(&mut self, caps: gst::Caps) {
        if let Some(info) = self.encoding_info.as_mut() {
            info.output_filter = Some(caps);
        }
    }

    pub fn output_filter(&self) -> Option<gst::Caps> {
        self.encoding_info
            .as_ref()
            .and_then(|info| info.output_filter.clone())
    }

    pub fn build_parser(&self) -> Result<Option<gst::Element>, Error> {
        match self.name.as_str() {
            "VP9" => make_element("vp9parse", None),
            "H264" => make_element("h264parse", None),
            "H265" => make_element("h265parse", None),
            "AV1" => make_element("av1parse", None),
            _ => return Ok(None),
        }
        .map(Some)
    }

    pub fn parser_caps(&self, force_profile: bool) -> gst::Caps {
        let codec_caps_name = self.caps.structure(0).unwrap().name();
        match self.name.as_str() {
            "H264" => {
                if force_profile {
                    gst::debug!(
                        CAT,
                        "No H264 profile requested, selecting constrained-baseline"
                    );

                    gst::Caps::builder(codec_caps_name)
                        .field("stream-format", "avc")
                        .field("profile", "constrained-baseline")
                        .build()
                } else {
                    gst::Caps::builder(codec_caps_name)
                        .field("stream-format", "avc")
                        .build()
                }
            }
            "H265" => gst::Caps::new_empty_simple("video/x-h265"),
            _ => gst::Caps::new_any(),
        }
    }
}

pub static AUDIO_CAPS: LazyLock<gst::Caps> =
    LazyLock::new(|| gst::Caps::new_empty_simple("audio/x-raw"));
pub static OPUS_CAPS: LazyLock<gst::Caps> =
    LazyLock::new(|| gst::Caps::new_empty_simple("audio/x-opus"));

pub static VIDEO_CAPS: LazyLock<gst::Caps> = LazyLock::new(|| {
    gst::Caps::builder_full_with_any_features()
        .structure(gst::Structure::new_empty("video/x-raw"))
        .build()
});
pub static VP8_CAPS: LazyLock<gst::Caps> =
    LazyLock::new(|| gst::Caps::new_empty_simple("video/x-vp8"));
pub static VP9_CAPS: LazyLock<gst::Caps> =
    LazyLock::new(|| gst::Caps::new_empty_simple("video/x-vp9"));
pub static H264_CAPS: LazyLock<gst::Caps> =
    LazyLock::new(|| gst::Caps::new_empty_simple("video/x-h264"));
pub static H265_CAPS: LazyLock<gst::Caps> =
    LazyLock::new(|| gst::Caps::new_empty_simple("video/x-h265"));
pub static AV1_CAPS: LazyLock<gst::Caps> =
    LazyLock::new(|| gst::Caps::new_empty_simple("video/x-av1"));

pub static RTP_CAPS: LazyLock<gst::Caps> =
    LazyLock::new(|| gst::Caps::new_empty_simple("application/x-rtp"));

#[derive(Debug, Clone)]
pub struct Codecs(Vec<Codec>);

impl Deref for Codecs {
    type Target = Vec<Codec>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Codecs {
    pub fn to_map(&self) -> BTreeMap<i32, Codec> {
        self.0
            .iter()
            .map(|codec| (codec.payload().unwrap(), codec.clone()))
            .collect()
    }

    pub fn from_map(codecs: &BTreeMap<i32, Codec>) -> Self {
        Self(codecs.values().cloned().collect())
    }

    pub fn list_encoders(&self) -> Codecs {
        Codecs(
            self.iter()
                .filter(|codec| {
                    codec
                        .encoding_info
                        .as_ref()
                        .is_some_and(|info| info.encoder.is_some())
                })
                .cloned()
                .collect(),
        )
    }

    pub fn find_for_payloadable_caps(&self, caps: &gst::Caps) -> Option<Codec> {
        self.iter()
            .find(|codec| codec.caps.can_intersect(caps) && codec.encoding_info.is_some())
            .cloned()
    }
}

static CODECS: LazyLock<Codecs> = LazyLock::new(|| {
    let decoders = gst::ElementFactory::factories_with_type(
        gst::ElementFactoryType::DECODER,
        gst::Rank::MARGINAL,
    );

    let depayloaders = gst::ElementFactory::factories_with_type(
        gst::ElementFactoryType::DEPAYLOADER,
        gst::Rank::MARGINAL,
    );

    let encoders = gst::ElementFactory::factories_with_type(
        gst::ElementFactoryType::ENCODER,
        gst::Rank::MARGINAL,
    );

    let payloaders = gst::ElementFactory::factories_with_type(
        gst::ElementFactoryType::PAYLOADER,
        gst::Rank::MARGINAL,
    );

    Codecs(vec![
        Codec::new(
            "OPUS",
            gst::StreamType::AUDIO,
            &OPUS_CAPS,
            &decoders,
            &depayloaders,
            &encoders,
            &payloaders,
        ),
        Codec::new_raw("L24", gst::StreamType::AUDIO, &depayloaders, &payloaders),
        Codec::new_raw("L16", gst::StreamType::AUDIO, &depayloaders, &payloaders),
        Codec::new_raw("L8", gst::StreamType::AUDIO, &depayloaders, &payloaders),
        Codec::new(
            "VP8",
            gst::StreamType::VIDEO,
            &VP8_CAPS,
            &decoders,
            &depayloaders,
            &encoders,
            &payloaders,
        ),
        Codec::new(
            "H264",
            gst::StreamType::VIDEO,
            &H264_CAPS,
            &decoders,
            &depayloaders,
            &encoders,
            &payloaders,
        ),
        Codec::new(
            "VP9",
            gst::StreamType::VIDEO,
            &VP9_CAPS,
            &decoders,
            &depayloaders,
            &encoders,
            &payloaders,
        ),
        Codec::new(
            "H265",
            gst::StreamType::VIDEO,
            &H265_CAPS,
            &decoders,
            &depayloaders,
            &encoders,
            &payloaders,
        ),
        Codec::new(
            "AV1",
            gst::StreamType::VIDEO,
            &AV1_CAPS,
            &decoders,
            &depayloaders,
            &encoders,
            &payloaders,
        ),
        Codec::new_raw("RAW", gst::StreamType::VIDEO, &depayloaders, &payloaders),
    ])
});

impl Codecs {
    pub fn find(encoding_name: &str) -> Option<Codec> {
        CODECS
            .iter()
            .find(|codec| codec.name == encoding_name)
            .cloned()
    }

    pub fn video_codecs<'a>() -> impl Iterator<Item = &'a Codec> {
        CODECS
            .iter()
            .filter(|codec| codec.stream_type == gst::StreamType::VIDEO)
    }

    pub fn audio_codecs<'a>() -> impl Iterator<Item = &'a Codec> {
        CODECS
            .iter()
            .filter(|codec| codec.stream_type == gst::StreamType::AUDIO)
    }

    /// List all codecs that can be used for encoding and/or payloading the given
    /// caps and assign a payload type to each of them. This is useful to initiate
    /// SDP negotiation.
    pub fn list_encoders_and_payloaders<'a>(
        caps: impl IntoIterator<Item = &'a gst::StructureRef>,
    ) -> Codecs {
        let mut payload = 96..128;

        Codecs(
            caps.into_iter()
                .filter_map(move |s| {
                    let caps = gst::Caps::builder_full().structure(s.to_owned()).build();

                    CODECS
                        .iter()
                        .find(|codec| {
                            codec
                                .encoding_info
                                .as_ref()
                                .is_some_and(|_| codec.caps.can_intersect(&caps))
                        })
                        .and_then(|codec| {
                            /* Assign a payload type to the codec */
                            if let Some(pt) = payload.next() {
                                let mut codec = codec.clone();

                                codec.payload_type = Some(pt);

                                Some(codec)
                            } else {
                                gst::warning!(
                                    CAT,
                                    "Too many formats for available payload type range, ignoring {}",
                                    s
                                );
                                None
                            }
                        })
                })
                .collect()
        )
    }
}

pub fn has_raw_caps(caps: &gst::Caps) -> bool {
    caps.iter()
        .any(|s| ["video/x-raw", "audio/x-raw"].contains(&s.name().as_str()))
}

pub fn cleanup_codec_caps(mut caps: gst::Caps) -> gst::Caps {
    assert!(caps.is_fixed());

    if let Some(s) = caps.make_mut().structure_mut(0) {
        if ["video/x-h264", "video/x-h265"].contains(&s.name().as_str()) {
            s.remove_fields(["codec_data"]);
        } else if ["video/x-vp8", "video/x-vp9"].contains(&s.name().as_str()) {
            s.remove_fields(["profile"]);
        } else if s.name() == "audio/x-opus" {
            s.remove_fields(["streamheader"]);
        }
    }

    caps
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct NavigationEvent {
    pub mid: Option<String>,
    #[serde(flatten)]
    pub event: gst_video::NavigationEvent,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
pub enum ControlRequest {
    NavigationEvent {
        event: gst_video::NavigationEvent,
    },
    #[serde(rename_all = "camelCase")]
    CustomUpstreamEvent {
        structure_name: String,
        structure: serde_json::Value,
    },
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum StringOrRequest {
    String(String),
    Request(ControlRequest),
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ControlRequestMessage {
    pub id: u64,
    pub mid: Option<String>,
    pub request: StringOrRequest,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub struct ControlResponseMessage {
    pub id: u64,
    pub error: Option<String>,
}

pub fn find_smallest_available_ext_id(ids: impl IntoIterator<Item = u32>) -> u32 {
    let used_numbers: HashSet<_> = ids.into_iter().collect();
    (1..).find(|&num| !used_numbers.contains(&num)).unwrap()
}

#[derive(Clone, Debug)]
enum CoerceTarget {
    Undefined,
    U64,
    I64,
    F64,
    Other(serde_json::Value),
}

fn pick_coerce_target(
    arr: &Vec<serde_json::Value>,
    mut target: CoerceTarget,
) -> Result<CoerceTarget, Error> {
    for val in arr {
        match val {
            serde_json::Value::Null => {
                return Err(anyhow!("Untyped null values are not handled"));
            }
            serde_json::Value::Bool(_) => match &target {
                CoerceTarget::Undefined => {
                    target = CoerceTarget::Other(val.clone());
                }
                CoerceTarget::Other(other) => {
                    if !other.is_boolean() {
                        return Err(anyhow!("Mixed types in arrays are not supported"));
                    }
                }
                _ => {
                    return Err(anyhow!("Mixed types in arrays are not supported"));
                }
            },
            serde_json::Value::Number(v) => {
                let v_target = if v.as_u64().is_some() {
                    CoerceTarget::U64
                } else if v.as_i64().is_some() {
                    CoerceTarget::I64
                } else {
                    CoerceTarget::F64
                };
                match &target {
                    CoerceTarget::Undefined => {
                        target = v_target;
                    }
                    CoerceTarget::Other(_) => {
                        return Err(anyhow!("Mixed types in arrays are not supported"));
                    }
                    CoerceTarget::U64 => {
                        target = v_target;
                    }
                    CoerceTarget::I64 => {
                        if matches!(v_target, CoerceTarget::F64) {
                            target = CoerceTarget::F64;
                        }
                    }
                    _ => (),
                }
            }
            serde_json::Value::Array(a) => {
                target = pick_coerce_target(a, target)?;
            }
            serde_json::Value::Object(_) => match &target {
                CoerceTarget::Undefined => {
                    target = CoerceTarget::Other(val.clone());
                }
                CoerceTarget::Other(other) => {
                    if !other.is_object() {
                        return Err(anyhow!("Mixed types in arrays are not supported"));
                    }
                }
                _ => {
                    return Err(anyhow!("Mixed types in arrays are not supported"));
                }
            },
            serde_json::Value::String(_) => match &target {
                CoerceTarget::Undefined => {
                    target = CoerceTarget::Other(val.clone());
                }
                CoerceTarget::Other(other) => {
                    if !other.is_object() {
                        return Err(anyhow!("Mixed types in arrays are not supported"));
                    }
                }
                _ => {
                    return Err(anyhow!("Mixed types in arrays are not supported"));
                }
            },
        }
    }

    Ok(target)
}

fn deserialize_serde_value(
    val: &serde_json::Value,
    mut target: CoerceTarget,
) -> Result<gst::glib::SendValue, Error> {
    match val {
        serde_json::Value::Null => Err(anyhow!("Untyped null values are not handled")),
        serde_json::Value::Bool(v) => Ok(v.to_send_value()),
        serde_json::Value::Number(v) => match target {
            CoerceTarget::U64 => Ok(v
                .as_u64()
                .ok_or(anyhow!("Mixed types in arrays are not supported"))?
                .to_send_value()),
            CoerceTarget::I64 => Ok(v
                .as_i64()
                .ok_or(anyhow!("Mixed types in arrays are not supported"))?
                .to_send_value()),
            CoerceTarget::F64 => Ok(v
                .as_f64()
                .expect("all numbers coerce to f64")
                .to_send_value()),
            CoerceTarget::Undefined => {
                if let Some(u) = v.as_u64() {
                    Ok(u.to_send_value())
                } else if let Some(i) = v.as_i64() {
                    Ok(i.to_send_value())
                } else if let Some(f) = v.as_f64() {
                    Ok(f.to_send_value())
                } else {
                    unreachable!()
                }
            }
            _ => unreachable!(),
        },
        serde_json::Value::String(v) => Ok(v.to_send_value()),
        serde_json::Value::Array(a) => {
            let mut gst_array = gst::Array::default();

            target = pick_coerce_target(a, target)?;

            for val in a {
                gst_array.append_value(deserialize_serde_value(val, target.to_owned())?);
            }

            Ok(gst_array.to_send_value())
        }
        serde_json::Value::Object(_) => {
            Ok(deserialize_serde_object(val, "webrtcsink-deserialized")?.to_send_value())
        }
    }
}

pub fn deserialize_serde_object(
    obj: &serde_json::Value,
    name: &str,
) -> Result<gst::Structure, Error> {
    let serde_json::Value::Object(map) = obj else {
        return Err(anyhow!("not a serde object"));
    };

    let mut ret = gst::Structure::builder(name);

    for (key, value) in map {
        ret = ret.field(
            key,
            deserialize_serde_value(value, CoerceTarget::Undefined)?,
        );
    }

    Ok(ret.build())
}

#[derive(Debug, PartialEq, Eq)]
pub struct VideoTimeCodeFlags(gst_video::VideoTimeCodeFlags);

impl serde::Serialize for VideoTimeCodeFlags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if self.0.bits() == 0 {
            serializer.serialize_str("none")
        } else {
            let mut ret = String::new();
            if self.0.contains(gst_video::VideoTimeCodeFlags::DROP_FRAME) {
                ret.push_str("drop-frame");
                if self.0.contains(gst_video::VideoTimeCodeFlags::INTERLACED) {
                    ret.push_str("+interlaced");
                }
            } else if self.0.contains(gst_video::VideoTimeCodeFlags::INTERLACED) {
                ret.push_str("interlaced");
            }
            serializer.serialize_str(&ret)
        }
    }
}

#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum Meta {
    TimeCode {
        hours: u32,
        minutes: u32,
        seconds: u32,
        frames: u32,
        field_count: u32,
        fps: gst::Fraction,
        flags: VideoTimeCodeFlags,
        latest_daily_jam: Option<String>,
    },
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Info {
    Meta(Meta),
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub struct InfoMessage {
    pub mid: String,
    pub info: Info,
}

pub fn serialize_meta(buffer: &gst::BufferRef, forward_metas: &HashSet<String>) -> Vec<Meta> {
    let mut ret = vec![];

    buffer.foreach_meta(|meta| {
        if forward_metas.contains("timecode") {
            if let Some(tc_meta) = meta.downcast_ref::<gst_video::VideoTimeCodeMeta>() {
                let tc = tc_meta.tc();
                ret.push(Meta::TimeCode {
                    hours: tc.hours(),
                    minutes: tc.minutes(),
                    seconds: tc.seconds(),
                    frames: tc.frames(),
                    field_count: tc.field_count(),
                    fps: tc.fps(),
                    flags: VideoTimeCodeFlags(tc.flags()),
                    latest_daily_jam: tc
                        .latest_daily_jam()
                        .and_then(|dt| {
                            let gst_dt: gst::DateTime = dt.into();
                            gst_dt.to_iso8601_string().ok()
                        })
                        .map(|s| s.to_string()),
                });
            }
        }
        std::ops::ControlFlow::Continue(())
    });

    ret
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_array() -> Result<(), String> {
        let arr = serde_json::from_str::<serde_json::Value>("[1, -1, 1.0]").unwrap();
        let gst_arr = deserialize_serde_value(&arr, CoerceTarget::Undefined)
            .unwrap()
            .get::<gst::Array>()
            .unwrap();
        let gst_type = gst_arr.first().unwrap().value_type();
        assert_eq!(gst_type, f64::static_type());

        let arr = serde_json::from_str::<serde_json::Value>("[1, -1]").unwrap();
        let gst_arr = deserialize_serde_value(&arr, CoerceTarget::Undefined)
            .unwrap()
            .get::<gst::Array>()
            .unwrap();
        let gst_type = gst_arr.first().unwrap().value_type();
        assert_eq!(gst_type, i64::static_type());

        let arr = serde_json::from_str::<serde_json::Value>("[1]").unwrap();
        let gst_arr = deserialize_serde_value(&arr, CoerceTarget::Undefined)
            .unwrap()
            .get::<gst::Array>()
            .unwrap();
        let gst_type = gst_arr.first().unwrap().value_type();
        assert_eq!(gst_type, u64::static_type());

        // u64::MAX can't be represented as i64, mixed types
        let arr = serde_json::from_str::<serde_json::Value>("[18446744073709551615, -1]").unwrap();
        assert!(deserialize_serde_value(&arr, CoerceTarget::Undefined).is_err());

        // we won't coerce bool to i64, mixed types
        let arr = serde_json::from_str::<serde_json::Value>("[true, -1]").unwrap();
        assert!(deserialize_serde_value(&arr, CoerceTarget::Undefined).is_err());

        let arr = serde_json::from_str::<serde_json::Value>("[[0.2, 0], [0, 0]]").unwrap();
        let gst_arr = deserialize_serde_value(&arr, CoerceTarget::Undefined)
            .unwrap()
            .get::<gst::Array>()
            .unwrap();
        let gst_type = gst_arr
            .first()
            .unwrap()
            .get::<gst::Array>()
            .unwrap()
            .first()
            .unwrap()
            .value_type();
        assert_eq!(gst_type, f64::static_type());
        let gst_type = gst_arr
            .last()
            .unwrap()
            .get::<gst::Array>()
            .unwrap()
            .first()
            .unwrap()
            .value_type();
        assert_eq!(gst_type, f64::static_type());

        Ok(())
    }

    #[test]
    fn test_serialize_meta() -> Result<(), String> {
        gst::init().unwrap();

        let mut buffer = gst::Buffer::new();
        let time_code = gst_video::VideoTimeCode::new(
            gst::Fraction::new(30, 1),
            None,
            gst_video::VideoTimeCodeFlags::empty(),
            10,
            53,
            17,
            0,
            0,
        );
        gst_video::VideoTimeCodeMeta::add(
            buffer.get_mut().unwrap(),
            &time_code.try_into().unwrap(),
        );

        assert_eq!(
            serialize_meta(&buffer, &[String::from("timecode")].into()),
            vec![Meta::TimeCode {
                hours: 10,
                minutes: 53,
                seconds: 17,
                frames: 0,
                field_count: 0,
                fps: gst::Fraction::new(30, 1),
                flags: VideoTimeCodeFlags(gst_video::VideoTimeCodeFlags::empty()),
                latest_daily_jam: None,
            }]
        );

        Ok(())
    }

    fn test_find_smallest_available_ext_id_case(
        ids: impl IntoIterator<Item = u32>,
        expected: u32,
    ) -> Result<(), String> {
        let actual = find_smallest_available_ext_id(ids);

        if actual != expected {
            return Err(format!("Expected {expected}, got {actual}"));
        }

        Ok(())
    }

    #[test]
    fn test_find_smallest_available_ext_id() -> Result<(), String> {
        [
            (vec![], 1u32),
            (vec![2u32, 3u32, 4u32], 1u32),
            (vec![1u32, 3u32, 4u32], 2u32),
            (vec![4u32, 1u32, 3u32], 2u32),
            (vec![1u32, 2u32, 3u32], 4u32),
        ]
        .into_iter()
        .try_for_each(|(input, expected)| test_find_smallest_available_ext_id_case(input, expected))
    }
}

fn read_certs_from_file(
    certificate_file: PathBuf,
) -> Result<Vec<rustls_pki_types::CertificateDer<'static>>, Box<dyn std::error::Error>> {
    let cert_file = File::open(&certificate_file)?;
    let mut cert_file_rdr = BufReader::new(cert_file);

    let certs_result = rustls_pemfile::certs(&mut cert_file_rdr);
    let mut certs = Vec::new();

    for cert_result in certs_result {
        match cert_result {
            Ok(cert) => certs.push(cert),
            Err(e) => {
                return Err(format!("Failed to parse certificate: {e}").into());
            }
        }
    }

    if certs.is_empty() {
        return Err(format!(
            "No valid certificates found in {}",
            certificate_file.display()
        )
        .into());
    }

    Ok(certs)
}

fn read_private_key_from_file(
    private_key_file: PathBuf,
) -> Result<rustls_pki_types::PrivateKeyDer<'static>, Box<dyn std::error::Error>> {
    let key_file = File::open(&private_key_file)?;
    let mut key_file_rdr = BufReader::new(key_file);
    let items_result = rustls_pemfile::read_all(&mut key_file_rdr);

    for item_result in items_result {
        let item = item_result.map_err(|e| {
            format!(
                "Failed to parse PEM item in {}: {e}",
                private_key_file.display(),
            )
        })?;

        match item {
            rustls_pemfile::Item::Pkcs1Key(key) => {
                return Ok(rustls_pki_types::PrivateKeyDer::from(key));
            }
            rustls_pemfile::Item::Pkcs8Key(key) => {
                return Ok(rustls_pki_types::PrivateKeyDer::from(key));
            }
            rustls_pemfile::Item::Sec1Key(key) => {
                return Ok(rustls_pki_types::PrivateKeyDer::from(key));
            }
            _ => continue,
        }
    }

    Err(format!(
        "No valid private key found in {}",
        private_key_file.display()
    )
    .into())
}

#[derive(Debug)]
struct SkipServerVerification(Arc<rustls::crypto::CryptoProvider>);

impl SkipServerVerification {
    pub fn new() -> Arc<Self> {
        Arc::new(Self(Arc::new(rustls::crypto::ring::default_provider())))
    }
}

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls_pki_types::CertificateDer,
        _intermediates: &[rustls_pki_types::CertificateDer],
        _server_name: &rustls::pki_types::ServerName,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls_pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls_pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

async fn get_root_certstore<P: AsRef<Path>>(
    cafile_path: P,
) -> Result<rustls::RootCertStore, std::io::Error> {
    let mut root_cert_store = rustls::RootCertStore::empty();

    let certs = {
        let cert_file = tokio::fs::read(&cafile_path).await?;
        let mut cert_file_rdr = BufReader::new(Cursor::new(cert_file));
        let cert_vec = rustls_pemfile::certs(&mut cert_file_rdr);
        cert_vec.into_iter().map(|c| c.unwrap()).collect::<Vec<_>>()
    };

    let _ = root_cert_store.add_parsable_certificates(certs);

    Ok(root_cert_store)
}

pub async fn create_tls_acceptor(
    certificate_file: &str,
    private_key_file: &str,
) -> Result<TlsAcceptor, Box<dyn std::error::Error>> {
    let ring_provider = rustls::crypto::ring::default_provider();
    let certs = read_certs_from_file(certificate_file.into())?;
    let key = read_private_key_from_file(private_key_file.into())?;

    let config = rustls::ServerConfig::builder_with_provider(ring_provider.into())
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;

    Ok(TlsAcceptor::from(Arc::new(config)))
}

pub async fn create_tls_connector<P: AsRef<Path>>(
    certificate_file: Option<P>,
    insecure_tls: bool,
) -> Result<TlsConnector, std::io::Error> {
    let ring_provider = rustls::crypto::ring::default_provider();

    if insecure_tls || certificate_file.is_none() {
        let config = rustls::ClientConfig::builder_with_provider(ring_provider.into())
            .with_safe_default_protocol_versions()
            .unwrap()
            .dangerous()
            .with_custom_certificate_verifier(SkipServerVerification::new())
            .with_no_client_auth();

        Ok(TlsConnector::from(Arc::new(config)))
    } else {
        let root_cert_store = get_root_certstore(certificate_file.unwrap()).await?;
        let config = rustls::ClientConfig::builder_with_provider(ring_provider.into())
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(root_cert_store)
            .with_no_client_auth();

        Ok(TlsConnector::from(Arc::new(config)))
    }
}
