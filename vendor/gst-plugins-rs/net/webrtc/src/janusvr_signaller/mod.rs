// SPDX-License-Identifier: MPL-2.0

use crate::signaller::{Signallable, WebRTCSignallerRole};
use gst::{glib, glib::prelude::*, glib::subclass::prelude::*};

mod imp;

// base class
glib::wrapper! {
    pub struct JanusVRSignaller(ObjectSubclass<imp::Signaller>) @implements Signallable;
}

trait JanusVRSignallerImpl: ObjectImpl + ObjectSubclass<Type: IsA<JanusVRSignaller>> {
    fn emit_talking(&self, talking: bool, id: imp::JanusId, audio_level: f32);
}

#[repr(C)]
pub struct JanusVRSignallerClass {
    parent: glib::object::ObjectClass,

    // virtual methods
    emit_talking: fn(&JanusVRSignaller, talking: bool, id: imp::JanusId, audio_level: f32),
}

unsafe impl ClassStruct for JanusVRSignallerClass {
    type Type = imp::Signaller;
}

impl std::ops::Deref for JanusVRSignallerClass {
    type Target = glib::Class<<<Self as ClassStruct>::Type as ObjectSubclass>::ParentType>;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(&self.parent as *const _ as *const _) }
    }
}

unsafe impl<T: JanusVRSignallerImpl> IsSubclassable<T> for JanusVRSignaller {
    fn class_init(class: &mut glib::Class<Self>) {
        Self::parent_class_init::<T>(class);

        let class = class.as_mut();

        class.emit_talking = |obj, talking, id, audio_level| unsafe {
            let imp = obj.unsafe_cast_ref::<T::Type>().imp();
            imp.emit_talking(talking, id, audio_level)
        };
    }
}

impl Default for JanusVRSignaller {
    fn default() -> Self {
        glib::Object::new()
    }
}

// default signaller using `u64` ids
glib::wrapper! {
    pub struct JanusVRSignallerU64(ObjectSubclass<imp::signaller_u64::SignallerU64>) @extends JanusVRSignaller, @implements Signallable;
}

impl JanusVRSignallerU64 {
    pub fn new(role: WebRTCSignallerRole) -> Self {
        glib::Object::builder().property("role", role).build()
    }
}

// signaller using strings ids, used when `use-string-ids=true` is set on `janusvrwebrtcsink`
glib::wrapper! {
    pub struct JanusVRSignallerStr(ObjectSubclass<imp::signaller_str::SignallerStr>) @extends JanusVRSignaller, @implements Signallable;
}

impl JanusVRSignallerStr {
    pub fn new(role: WebRTCSignallerRole) -> Self {
        glib::Object::builder().property("role", role).build()
    }
}
