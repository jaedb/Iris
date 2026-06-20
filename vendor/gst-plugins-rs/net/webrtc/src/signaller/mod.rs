mod iface;
mod imp;
use gst::glib;

/**
 * GstRSWebRTCSignallableIface:
 * @title: Interface for WebRTC signalling protocols
 *
 * Interface that WebRTC elements can implement their own protocol with.
 */
use std::sync::LazyLock;
// Expose traits and objects from the module itself so it exactly looks like
// generated bindings
pub use imp::WebRTCSignallerRole;
pub mod prelude {
    pub use {super::SignallableExt, super::SignallableImpl};
}

pub static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "webrtc-signaller",
        gst::DebugColorFlags::empty(),
        Some("WebRTC signaller"),
    )
});

glib::wrapper! {
    pub struct Signallable(ObjectInterface<iface::Signallable>);
}

glib::wrapper! {
    pub struct Signaller(ObjectSubclass <imp::Signaller>) @implements Signallable;
}

impl Default for Signaller {
    fn default() -> Self {
        glib::Object::builder().build()
    }
}

impl Signaller {
    pub fn new(mode: WebRTCSignallerRole) -> Self {
        glib::Object::builder().property("role", mode).build()
    }
}

pub use iface::SignallableExt;
pub use iface::SignallableImpl;

unsafe impl Send for Signallable {}
unsafe impl Sync for Signallable {}
