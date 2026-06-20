/*
 * Taken from
 * https://gitlab.freedesktop.org/gstreamer/gstreamer-rs/-/blob/main/examples/src/bin/custom_meta.rs
 */
use gst::prelude::*;
use std::{fmt, mem};

#[repr(transparent)]
pub struct QuinnQuicMeta(imp::QuinnQuicMeta);

unsafe impl Send for QuinnQuicMeta {}
unsafe impl Sync for QuinnQuicMeta {}

impl QuinnQuicMeta {
    pub fn add(
        buffer: &mut gst::BufferRef,
        stream_id: u64,
        is_datagram: bool,
    ) -> gst::MetaRefMut<'_, Self, gst::meta::Standalone> {
        unsafe {
            let mut params = mem::ManuallyDrop::new(imp::QuinnQuicMetaParams {
                stream_id,
                is_datagram,
            });

            let meta = gst::ffi::gst_buffer_add_meta(
                buffer.as_mut_ptr(),
                imp::custom_meta_get_info(),
                &mut *params as *mut imp::QuinnQuicMetaParams as glib::ffi::gpointer,
            ) as *mut imp::QuinnQuicMeta;

            Self::from_mut_ptr(buffer, meta)
        }
    }

    pub fn stream_id(&self) -> u64 {
        self.0.stream_id
    }

    pub fn is_datagram(&self) -> bool {
        self.0.is_datagram
    }
}

unsafe impl MetaAPI for QuinnQuicMeta {
    type GstType = imp::QuinnQuicMeta;

    fn meta_api() -> glib::Type {
        imp::custom_meta_api_get_type()
    }
}

impl fmt::Debug for QuinnQuicMeta {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("QuinnQuicMeta")
            .field("stream_id", &self.0.stream_id)
            .field("is_datagram", &self.0.is_datagram)
            .finish()
    }
}

mod imp {
    use std::{mem, ptr};

    use glib::translate::*;

    pub(super) struct QuinnQuicMetaParams {
        pub stream_id: u64,
        pub is_datagram: bool,
    }

    #[repr(C)]
    pub struct QuinnQuicMeta {
        parent: gst::ffi::GstMeta,
        pub(super) stream_id: u64,
        pub(super) is_datagram: bool,
    }

    pub(super) fn custom_meta_api_get_type() -> glib::Type {
        static TYPE: std::sync::OnceLock<glib::Type> = std::sync::OnceLock::new();

        *TYPE.get_or_init(|| unsafe {
            let t = glib::Type::from_glib(gst::ffi::gst_meta_api_type_register(
                c"QuinnQuicMetaAPI".as_ptr() as *const _,
                [ptr::null::<std::os::raw::c_int>()].as_ptr() as *mut *const _,
            ));

            assert_ne!(t, glib::Type::INVALID);

            t
        })
    }

    unsafe extern "C" fn custom_meta_init(
        meta: *mut gst::ffi::GstMeta,
        params: glib::ffi::gpointer,
        _buffer: *mut gst::ffi::GstBuffer,
    ) -> glib::ffi::gboolean {
        assert!(!params.is_null());

        let meta = &mut *(meta as *mut QuinnQuicMeta);
        let params = ptr::read(params as *const QuinnQuicMetaParams);

        ptr::write(&mut meta.stream_id, params.stream_id);
        ptr::write(&mut meta.is_datagram, params.is_datagram);

        true.into_glib()
    }

    unsafe extern "C" fn custom_meta_free(
        meta: *mut gst::ffi::GstMeta,
        _buffer: *mut gst::ffi::GstBuffer,
    ) {
        let meta = &mut *(meta as *mut QuinnQuicMeta);

        // Need to free/drop all our fields here.
        ptr::drop_in_place(&mut meta.stream_id);
    }

    unsafe extern "C" fn custom_meta_transform(
        dest: *mut gst::ffi::GstBuffer,
        meta: *mut gst::ffi::GstMeta,
        _buffer: *mut gst::ffi::GstBuffer,
        _type_: glib::ffi::GQuark,
        _data: glib::ffi::gpointer,
    ) -> glib::ffi::gboolean {
        let meta = &*(meta as *mut QuinnQuicMeta);

        super::QuinnQuicMeta::add(
            gst::BufferRef::from_mut_ptr(dest),
            meta.stream_id,
            meta.is_datagram,
        );

        true.into_glib()
    }

    pub(super) fn custom_meta_get_info() -> *const gst::ffi::GstMetaInfo {
        struct MetaInfo(ptr::NonNull<gst::ffi::GstMetaInfo>);
        unsafe impl Send for MetaInfo {}
        unsafe impl Sync for MetaInfo {}

        static META_INFO: std::sync::OnceLock<MetaInfo> = std::sync::OnceLock::new();

        META_INFO
            .get_or_init(|| unsafe {
                MetaInfo(
                    ptr::NonNull::new(gst::ffi::gst_meta_register(
                        custom_meta_api_get_type().into_glib(),
                        c"QuinnQuicMeta".as_ptr() as *const _,
                        mem::size_of::<QuinnQuicMeta>(),
                        Some(custom_meta_init),
                        Some(custom_meta_free),
                        Some(custom_meta_transform),
                    ) as *mut gst::ffi::GstMetaInfo)
                    .expect("Failed to register meta API"),
                )
            })
            .0
            .as_ptr()
    }
}
