// SPDX-License-Identifier: MPL-2.0

use crate::signaller::Signallable;
use gst::{glib, prelude::ObjectExt, subclass::prelude::ObjectSubclassIsExt};

mod client;

glib::wrapper! {
    pub struct WhepClientSignaller(ObjectSubclass<client::WhepClient>) @implements Signallable;
}

unsafe impl Send for WhepClientSignaller {}
unsafe impl Sync for WhepClientSignaller {}

impl Default for WhepClientSignaller {
    fn default() -> Self {
        let sig: WhepClientSignaller = glib::Object::new();
        sig.connect_closure("webrtcbin-ready", false, sig.imp().on_webrtcbin_ready());
        sig
    }
}
