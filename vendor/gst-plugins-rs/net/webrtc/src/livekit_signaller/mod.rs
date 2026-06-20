// SPDX-License-Identifier: MPL-2.0

use crate::signaller::{Signallable, WebRTCSignallerRole};
use gst::glib;

mod imp;

glib::wrapper! {
    pub struct LiveKitSignaller(ObjectSubclass<imp::Signaller>) @implements Signallable;
}

impl LiveKitSignaller {
    fn new(role: WebRTCSignallerRole) -> Self {
        glib::Object::builder().property("role", role).build()
    }

    pub fn new_consumer() -> Self {
        Self::new(WebRTCSignallerRole::Consumer)
    }

    pub fn new_producer() -> Self {
        Self::new(WebRTCSignallerRole::Producer)
    }
}

impl Default for LiveKitSignaller {
    fn default() -> Self {
        glib::Object::new()
    }
}
