// SPDX-License-Identifier: MPL-2.0

use gst::prelude::*;
use std::fmt;
use std::mem;

#[repr(transparent)]
pub struct NdiSinkAudioMeta(imp::NdiSinkAudioMeta);

unsafe impl Send for NdiSinkAudioMeta {}
unsafe impl Sync for NdiSinkAudioMeta {}

impl NdiSinkAudioMeta {
    pub fn add(
        buffer: &mut gst::BufferRef,
        buffers: Vec<(gst::Buffer, gst_audio::AudioInfo, i64)>,
    ) -> gst::MetaRefMut<'_, Self, gst::meta::Standalone> {
        unsafe {
            // Manually dropping because gst_buffer_add_meta() takes ownership of the
            // content of the struct
            let mut params = mem::ManuallyDrop::new(imp::NdiSinkAudioMetaParams { buffers });

            let meta = gst::ffi::gst_buffer_add_meta(
                buffer.as_mut_ptr(),
                imp::ndi_sink_audio_meta_get_info(),
                &mut *params as *mut imp::NdiSinkAudioMetaParams as glib::ffi::gpointer,
            ) as *mut imp::NdiSinkAudioMeta;

            Self::from_mut_ptr(buffer, meta)
        }
    }

    pub fn buffers(&self) -> &[(gst::Buffer, gst_audio::AudioInfo, i64)] {
        &self.0.buffers
    }
}

unsafe impl MetaAPI for NdiSinkAudioMeta {
    type GstType = imp::NdiSinkAudioMeta;

    fn meta_api() -> glib::Type {
        imp::ndi_sink_audio_meta_api_get_type()
    }
}

impl fmt::Debug for NdiSinkAudioMeta {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("NdiSinkAudioMeta")
            .field("buffers", &self.buffers())
            .finish()
    }
}

mod imp {
    use glib::translate::*;
    use std::mem;
    use std::ptr;
    use std::sync::LazyLock;

    pub(super) struct NdiSinkAudioMetaParams {
        pub buffers: Vec<(gst::Buffer, gst_audio::AudioInfo, i64)>,
    }

    #[repr(C)]
    pub struct NdiSinkAudioMeta {
        parent: gst::ffi::GstMeta,
        pub(super) buffers: Vec<(gst::Buffer, gst_audio::AudioInfo, i64)>,
    }

    pub(super) fn ndi_sink_audio_meta_api_get_type() -> glib::Type {
        static TYPE: LazyLock<glib::Type> = LazyLock::new(|| unsafe {
            let t = from_glib(gst::ffi::gst_meta_api_type_register(
                c"GstNdiSinkAudioMetaAPI".as_ptr() as *const _,
                [ptr::null::<std::os::raw::c_char>()].as_ptr() as *mut *const _,
            ));

            assert_ne!(t, glib::Type::INVALID);

            t
        });

        *TYPE
    }

    unsafe extern "C" fn ndi_sink_audio_meta_init(
        meta: *mut gst::ffi::GstMeta,
        params: glib::ffi::gpointer,
        _buffer: *mut gst::ffi::GstBuffer,
    ) -> glib::ffi::gboolean {
        assert!(!params.is_null());

        let meta = &mut *(meta as *mut NdiSinkAudioMeta);
        let params = ptr::read(params as *const NdiSinkAudioMetaParams);

        ptr::write(&mut meta.buffers, params.buffers);

        true.into_glib()
    }

    unsafe extern "C" fn ndi_sink_audio_meta_free(
        meta: *mut gst::ffi::GstMeta,
        _buffer: *mut gst::ffi::GstBuffer,
    ) {
        let meta = &mut *(meta as *mut NdiSinkAudioMeta);

        ptr::drop_in_place(&mut meta.buffers);
    }

    unsafe extern "C" fn ndi_sink_audio_meta_transform(
        dest: *mut gst::ffi::GstBuffer,
        meta: *mut gst::ffi::GstMeta,
        _buffer: *mut gst::ffi::GstBuffer,
        _type_: glib::ffi::GQuark,
        _data: glib::ffi::gpointer,
    ) -> glib::ffi::gboolean {
        let meta = &*(meta as *mut NdiSinkAudioMeta);

        super::NdiSinkAudioMeta::add(gst::BufferRef::from_mut_ptr(dest), meta.buffers.clone());

        true.into_glib()
    }

    pub(super) fn ndi_sink_audio_meta_get_info() -> *const gst::ffi::GstMetaInfo {
        struct MetaInfo(ptr::NonNull<gst::ffi::GstMetaInfo>);
        unsafe impl Send for MetaInfo {}
        unsafe impl Sync for MetaInfo {}

        static META_INFO: LazyLock<MetaInfo> = LazyLock::new(|| unsafe {
            MetaInfo(
                ptr::NonNull::new(gst::ffi::gst_meta_register(
                    ndi_sink_audio_meta_api_get_type().into_glib(),
                    c"GstNdiSinkAudioMeta".as_ptr() as *const _,
                    mem::size_of::<NdiSinkAudioMeta>(),
                    Some(ndi_sink_audio_meta_init),
                    Some(ndi_sink_audio_meta_free),
                    Some(ndi_sink_audio_meta_transform),
                ) as *mut gst::ffi::GstMetaInfo)
                .expect("Failed to register meta API"),
            )
        });

        META_INFO.0.as_ptr()
    }
}
