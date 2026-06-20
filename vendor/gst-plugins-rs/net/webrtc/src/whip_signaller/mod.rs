// SPDX-License-Identifier: MPL-2.0

use crate::signaller::Signallable;
use gst::{glib, prelude::ObjectExt, subclass::prelude::ObjectSubclassIsExt};

mod imp;

glib::wrapper! {
    pub struct WhipClientSignaller(ObjectSubclass<imp::WhipClient>) @implements Signallable;
}

glib::wrapper! {
    pub struct WhipServerSignaller(ObjectSubclass<imp::WhipServer>) @implements Signallable;
}

impl Default for WhipClientSignaller {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl Default for WhipServerSignaller {
    fn default() -> Self {
        let sig: WhipServerSignaller = glib::Object::new();
        sig.connect_closure("webrtcbin-ready", false, sig.imp().on_webrtcbin_ready());
        sig
    }
}
