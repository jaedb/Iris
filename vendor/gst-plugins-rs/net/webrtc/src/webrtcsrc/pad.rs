// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::LazyLock;
use std::sync::Mutex;

#[derive(Default)]
pub struct WebRTCSrcPad {
    needs_raw: AtomicBool,
    stream_id: Mutex<Option<String>>,
    webrtcbin_pad: Mutex<Option<gst::glib::WeakRef<gst::Pad>>>,
}

impl WebRTCSrcPad {
    pub fn set_needs_decoding(&self, raw_wanted: bool) {
        self.needs_raw.store(raw_wanted, Ordering::SeqCst);
    }

    pub fn needs_decoding(&self) -> bool {
        self.needs_raw.load(Ordering::SeqCst)
    }

    pub fn set_stream_id(&self, stream_id: &str) {
        *self.stream_id.lock().unwrap() = Some(stream_id.to_string());
    }

    pub fn stream_id(&self) -> String {
        let stream_id = self.stream_id.lock().unwrap();
        stream_id.as_ref().unwrap().clone()
    }

    pub fn set_webrtc_pad(&self, pad: glib::object::WeakRef<gst::Pad>) {
        *self.webrtcbin_pad.lock().unwrap() = Some(pad);
    }
}

#[glib::object_subclass]
impl ObjectSubclass for WebRTCSrcPad {
    const NAME: &'static str = "GstWebRTCSrcPad";
    type Type = super::WebRTCSrcPad;
    type ParentType = gst::GhostPad;
}

impl ObjectImpl for WebRTCSrcPad {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPS: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![glib::ParamSpecString::builder("msid")
                .flags(glib::ParamFlags::READABLE)
                .blurb("Remote MediaStream ID in use for this pad")
                .build()]
        });
        PROPS.as_ref()
    }
    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "msid" => self
                .webrtcbin_pad
                .lock()
                .unwrap()
                .as_ref()
                .and_then(|p| p.upgrade())
                .and_then(|p| p.property::<Option<String>>("msid"))
                .to_value(),
            name => panic!("no readable property {name:?}"),
        }
    }
}
impl GstObjectImpl for WebRTCSrcPad {}
impl PadImpl for WebRTCSrcPad {}
impl ProxyPadImpl for WebRTCSrcPad {}
impl GhostPadImpl for WebRTCSrcPad {}

unsafe impl<T: WebRTCSrcPadImpl> IsSubclassable<T> for super::WebRTCSrcPad {}

pub trait WebRTCSrcPadImpl: GhostPadImpl + ObjectSubclass<Type: IsA<super::WebRTCSrcPad>> {}
