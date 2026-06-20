use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_webrtc::WebRTCSessionDescription;

use gstrswebrtc::signaller::{Signallable, SignallableImpl};

use std::sync::LazyLock;

#[derive(Default)]
pub struct Signaller {}

impl Signaller {}

impl SignallableImpl for Signaller {
    fn start(&self) {
        unimplemented!()
    }

    fn stop(&self) {
        unimplemented!()
    }

    fn send_sdp(&self, _session_id: &str, _sdp: &WebRTCSessionDescription) {
        unimplemented!()
    }

    fn add_ice(
        &self,
        _session_id: &str,
        _candidate: &str,
        _sdp_m_line_index: u32,
        _sdp_mid: Option<String>,
    ) {
        unimplemented!()
    }

    fn end_session(&self, _session_id: &str) {
        unimplemented!()
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Signaller {
    const NAME: &'static str = "MyCustomWebRTCSinkSignaller";
    type Type = super::MyCustomSignaller;
    type ParentType = glib::Object;
    type Interfaces = (Signallable,);
}

impl ObjectImpl for Signaller {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPS: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![glib::ParamSpecBoolean::builder("manual-sdp-munging")
                .nick("Manual SDP munging")
                .blurb("Whether the signaller manages SDP munging itself")
                .default_value(false)
                .read_only()
                .build()]
        });

        PROPS.as_ref()
    }
    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "manual-sdp-munging" => false.to_value(),
            _ => unimplemented!(),
        }
    }
}
