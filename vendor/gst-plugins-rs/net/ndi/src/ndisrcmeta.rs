// SPDX-License-Identifier: MPL-2.0

use gst::prelude::*;
use std::fmt;
use std::mem;

use crate::ndi::{AudioFrame, MetadataFrame, VideoFrame};

#[repr(transparent)]
pub struct NdiSrcMeta(imp::NdiSrcMeta);

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Buffer {
    Audio {
        frame: AudioFrame,
        discont: bool,
        receive_time_gst: gst::ClockTime,
        receive_time_real: gst::ClockTime,
    },
    Video {
        frame: VideoFrame,
        discont: bool,
        receive_time_gst: gst::ClockTime,
        receive_time_real: gst::ClockTime,
    },
    Metadata {
        frame: MetadataFrame,
        receive_time_gst: gst::ClockTime,
        receive_time_real: gst::ClockTime,
    },
}

unsafe impl Send for NdiSrcMeta {}
unsafe impl Sync for NdiSrcMeta {}

impl NdiSrcMeta {
    pub fn add(
        buffer: &mut gst::BufferRef,
        ndi_buffer: Buffer,
    ) -> gst::MetaRefMut<'_, Self, gst::meta::Standalone> {
        unsafe {
            // Manually dropping because gst_buffer_add_meta() takes ownership of the
            // content of the struct
            let mut params = mem::ManuallyDrop::new(imp::NdiSrcMetaParams { ndi_buffer });

            let meta = gst::ffi::gst_buffer_add_meta(
                buffer.as_mut_ptr(),
                imp::ndi_src_meta_get_info(),
                &mut *params as *mut imp::NdiSrcMetaParams as glib::ffi::gpointer,
            ) as *mut imp::NdiSrcMeta;

            Self::from_mut_ptr(buffer, meta)
        }
    }

    pub fn take_ndi_buffer(&mut self) -> Buffer {
        self.0.ndi_buffer.take().expect("can only take buffer once")
    }
}

unsafe impl MetaAPI for NdiSrcMeta {
    type GstType = imp::NdiSrcMeta;

    fn meta_api() -> glib::Type {
        imp::ndi_src_meta_api_get_type()
    }
}

impl fmt::Debug for NdiSrcMeta {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("NdiSrcMeta")
            .field("ndi_buffer", &self.0.ndi_buffer)
            .finish()
    }
}

mod imp {
    use super::Buffer;
    use glib::translate::*;
    use std::mem;
    use std::ptr;
    use std::sync::LazyLock;

    pub(super) struct NdiSrcMetaParams {
        pub ndi_buffer: Buffer,
    }

    #[repr(C)]
    pub struct NdiSrcMeta {
        parent: gst::ffi::GstMeta,
        pub(super) ndi_buffer: Option<Buffer>,
    }

    pub(super) fn ndi_src_meta_api_get_type() -> glib::Type {
        static TYPE: LazyLock<glib::Type> = LazyLock::new(|| unsafe {
            let t = from_glib(gst::ffi::gst_meta_api_type_register(
                c"GstNdiSrcMetaAPI".as_ptr() as *const _,
                [ptr::null::<std::os::raw::c_char>()].as_ptr() as *mut *const _,
            ));

            assert_ne!(t, glib::Type::INVALID);

            t
        });

        *TYPE
    }

    unsafe extern "C" fn ndi_src_meta_init(
        meta: *mut gst::ffi::GstMeta,
        params: glib::ffi::gpointer,
        _buffer: *mut gst::ffi::GstBuffer,
    ) -> glib::ffi::gboolean {
        assert!(!params.is_null());

        let meta = &mut *(meta as *mut NdiSrcMeta);
        let params = ptr::read(params as *const NdiSrcMetaParams);

        ptr::write(&mut meta.ndi_buffer, Some(params.ndi_buffer));

        true.into_glib()
    }

    unsafe extern "C" fn ndi_src_meta_free(
        meta: *mut gst::ffi::GstMeta,
        _buffer: *mut gst::ffi::GstBuffer,
    ) {
        let meta = &mut *(meta as *mut NdiSrcMeta);

        ptr::drop_in_place(&mut meta.ndi_buffer);
    }

    unsafe extern "C" fn ndi_src_meta_transform(
        _dest: *mut gst::ffi::GstBuffer,
        _meta: *mut gst::ffi::GstMeta,
        _buffer: *mut gst::ffi::GstBuffer,
        _type_: glib::ffi::GQuark,
        _data: glib::ffi::gpointer,
    ) -> glib::ffi::gboolean {
        false.into_glib()
    }

    pub(super) fn ndi_src_meta_get_info() -> *const gst::ffi::GstMetaInfo {
        struct MetaInfo(ptr::NonNull<gst::ffi::GstMetaInfo>);
        unsafe impl Send for MetaInfo {}
        unsafe impl Sync for MetaInfo {}

        static META_INFO: LazyLock<MetaInfo> = LazyLock::new(|| unsafe {
            MetaInfo(
                ptr::NonNull::new(gst::ffi::gst_meta_register(
                    ndi_src_meta_api_get_type().into_glib(),
                    c"GstNdiSrcMeta".as_ptr() as *const _,
                    mem::size_of::<NdiSrcMeta>(),
                    Some(ndi_src_meta_init),
                    Some(ndi_src_meta_free),
                    Some(ndi_src_meta_transform),
                ) as *mut gst::ffi::GstMetaInfo)
                .expect("Failed to register meta API"),
            )
        });

        META_INFO.0.as_ptr()
    }
}
