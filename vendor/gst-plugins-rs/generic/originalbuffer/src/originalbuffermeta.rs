// Copyright (C) 2024 Collabora Ltd
//   @author: Olivier Crête <olivier.crete@collabora.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::prelude::*;
use std::fmt;
use std::mem;

#[repr(transparent)]
pub struct OriginalBufferMeta(imp::OriginalBufferMeta);

unsafe impl Send for OriginalBufferMeta {}
unsafe impl Sync for OriginalBufferMeta {}

impl OriginalBufferMeta {
    pub fn add(
        buffer: &mut gst::BufferRef,
        original: gst::Buffer,
        caps: Option<gst::Caps>,
    ) -> gst::MetaRefMut<'_, Self, gst::meta::Standalone> {
        unsafe {
            // Manually dropping because gst_buffer_add_meta() takes ownership of the
            // content of the struct
            let mut params =
                mem::ManuallyDrop::new(imp::OriginalBufferMetaParams { original, caps });

            let meta = gst::ffi::gst_buffer_add_meta(
                buffer.as_mut_ptr(),
                imp::original_buffer_meta_get_info(),
                &mut *params as *mut imp::OriginalBufferMetaParams as gst::glib::ffi::gpointer,
            ) as *mut imp::OriginalBufferMeta;

            Self::from_mut_ptr(buffer, meta)
        }
    }

    pub fn replace(&mut self, original: gst::Buffer, caps: Option<gst::Caps>) {
        self.0.original = Some(original);
        self.0.caps = caps;
    }

    pub fn original(&self) -> &gst::Buffer {
        self.0.original.as_ref().unwrap()
    }

    pub fn caps(&self) -> &gst::Caps {
        self.0.caps.as_ref().unwrap()
    }
}

unsafe impl MetaAPI for OriginalBufferMeta {
    type GstType = imp::OriginalBufferMeta;

    fn meta_api() -> gst::glib::Type {
        imp::original_buffer_meta_api_get_type()
    }
}

impl fmt::Debug for OriginalBufferMeta {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("OriginalBufferMeta")
            .field("buffer", &self.original())
            .finish()
    }
}

mod imp {
    use gst::glib::translate::*;
    use std::mem;
    use std::ptr;
    use std::sync::LazyLock;

    pub(super) struct OriginalBufferMetaParams {
        pub original: gst::Buffer,
        pub caps: Option<gst::Caps>,
    }

    #[repr(C)]
    pub struct OriginalBufferMeta {
        parent: gst::ffi::GstMeta,
        pub(super) original: Option<gst::Buffer>,
        pub(super) caps: Option<gst::Caps>,
    }

    pub(super) fn original_buffer_meta_api_get_type() -> glib::Type {
        static TYPE: LazyLock<glib::Type> = LazyLock::new(|| unsafe {
            let t = from_glib(gst::ffi::gst_meta_api_type_register(
                c"GstOriginalBufferMetaAPI".as_ptr() as *const _,
                [ptr::null::<std::os::raw::c_char>()].as_ptr() as *mut *const _,
            ));

            assert_ne!(t, glib::Type::INVALID);

            t
        });

        *TYPE
    }

    unsafe extern "C" fn original_buffer_meta_init(
        meta: *mut gst::ffi::GstMeta,
        params: glib::ffi::gpointer,
        _buffer: *mut gst::ffi::GstBuffer,
    ) -> glib::ffi::gboolean {
        assert!(!params.is_null());
        let meta = &mut *(meta as *mut OriginalBufferMeta);
        let params = ptr::read(params as *const OriginalBufferMetaParams);

        let OriginalBufferMetaParams { original, caps } = params;

        ptr::write(&mut meta.original, Some(original));
        ptr::write(&mut meta.caps, caps);

        true.into_glib()
    }

    unsafe extern "C" fn original_buffer_meta_free(
        meta: *mut gst::ffi::GstMeta,
        _buffer: *mut gst::ffi::GstBuffer,
    ) {
        let meta = &mut *(meta as *mut OriginalBufferMeta);
        meta.original = None;
        meta.caps = None;
    }

    unsafe extern "C" fn original_buffer_meta_transform(
        dest: *mut gst::ffi::GstBuffer,
        meta: *mut gst::ffi::GstMeta,
        _buffer: *mut gst::ffi::GstBuffer,
        _type_: glib::ffi::GQuark,
        _data: glib::ffi::gpointer,
    ) -> glib::ffi::gboolean {
        let dest = gst::BufferRef::from_mut_ptr(dest);
        let meta = &*(meta as *const OriginalBufferMeta);

        if dest.meta::<super::OriginalBufferMeta>().is_some() {
            return true.into_glib();
        }
        // We don't store a ref in the meta if it's self-refencing, but we add it
        // when copying the meta to another buffer.
        super::OriginalBufferMeta::add(
            dest,
            meta.original.as_ref().unwrap().clone(),
            meta.caps.clone(),
        );

        true.into_glib()
    }

    pub(super) fn original_buffer_meta_get_info() -> *const gst::ffi::GstMetaInfo {
        struct MetaInfo(ptr::NonNull<gst::ffi::GstMetaInfo>);
        unsafe impl Send for MetaInfo {}
        unsafe impl Sync for MetaInfo {}

        static META_INFO: LazyLock<MetaInfo> = LazyLock::new(|| unsafe {
            MetaInfo(
                ptr::NonNull::new(gst::ffi::gst_meta_register(
                    original_buffer_meta_api_get_type().into_glib(),
                    c"OriginalBufferMeta".as_ptr() as *const _,
                    mem::size_of::<OriginalBufferMeta>(),
                    Some(original_buffer_meta_init),
                    Some(original_buffer_meta_free),
                    Some(original_buffer_meta_transform),
                ) as *mut gst::ffi::GstMetaInfo)
                .expect("Failed to register meta API"),
            )
        });

        META_INFO.0.as_ptr()
    }
}

#[test]
fn test() {
    gst::init().unwrap();
    let mut b = gst::Buffer::with_size(10).unwrap();
    let caps = gst::Caps::new_empty_simple("video/x-raw");
    let copy = b.copy();
    let m = OriginalBufferMeta::add(b.make_mut(), copy, Some(caps.clone()));
    assert_eq!(m.caps(), caps.as_ref());
    assert_eq!(m.original().clone(), b);
    let b2: gst::Buffer = b.copy_deep().unwrap();
    let m = b.meta::<OriginalBufferMeta>().unwrap();
    assert_eq!(m.caps(), caps.as_ref());
    assert_eq!(m.original(), &b);
    let m = b2.meta::<OriginalBufferMeta>().unwrap();
    assert_eq!(m.caps(), caps.as_ref());
    assert_eq!(m.original(), &b);
    let b3: gst::Buffer = b2.copy_deep().unwrap();
    drop(b2);
    let m = b3.meta::<OriginalBufferMeta>().unwrap();
    assert_eq!(m.caps(), caps.as_ref());
    assert_eq!(m.original(), &b);
}
