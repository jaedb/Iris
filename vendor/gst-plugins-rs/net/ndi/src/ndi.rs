// SPDX-License-Identifier: MPL-2.0

use crate::ndisys;
use crate::ndisys::*;
use std::ffi;
use std::fmt;
use std::mem;
use std::ptr;
use std::sync::Arc;

use byte_slice_cast::*;

use gst_video::prelude::*;

pub fn load() -> Result<(), glib::BoolError> {
    ndisys::load()
}

#[derive(Debug)]
pub struct FindBuilder<'a> {
    show_local_sources: bool,
    groups: Option<&'a str>,
    extra_ips: Option<&'a str>,
}

impl Default for FindBuilder<'_> {
    fn default() -> Self {
        Self {
            show_local_sources: true,
            groups: None,
            extra_ips: None,
        }
    }
}

impl<'a> FindBuilder<'a> {
    pub fn show_local_sources(self, show_local_sources: bool) -> Self {
        Self {
            show_local_sources,
            ..self
        }
    }

    pub fn groups(self, groups: &'a str) -> Self {
        Self {
            groups: Some(groups),
            ..self
        }
    }

    pub fn extra_ips(self, extra_ips: &'a str) -> Self {
        Self {
            extra_ips: Some(extra_ips),
            ..self
        }
    }

    pub fn build(self) -> Option<FindInstance> {
        let groups = self.groups.map(|s| ffi::CString::new(s).unwrap());
        let extra_ips = self.extra_ips.map(|s| ffi::CString::new(s).unwrap());

        unsafe {
            let ptr = NDIlib_find_create_v2(&NDIlib_find_create_t {
                show_local_sources: self.show_local_sources,
                p_groups: groups.as_ref().map(|s| s.as_ptr()).unwrap_or(ptr::null()),
                p_extra_ips: extra_ips
                    .as_ref()
                    .map(|s| s.as_ptr())
                    .unwrap_or(ptr::null()),
            });
            if ptr.is_null() {
                None
            } else {
                Some(FindInstance(ptr::NonNull::new_unchecked(ptr)))
            }
        }
    }
}

#[derive(Debug)]
pub struct FindInstance(ptr::NonNull<::std::os::raw::c_void>);

unsafe impl Send for FindInstance {}

impl FindInstance {
    pub fn builder<'a>() -> FindBuilder<'a> {
        FindBuilder::default()
    }

    pub fn wait_for_sources(&mut self, timeout_in_ms: u32) -> bool {
        unsafe { NDIlib_find_wait_for_sources(self.0.as_ptr(), timeout_in_ms) }
    }

    pub fn get_current_sources(&mut self) -> Vec<Source<'_>> {
        unsafe {
            let mut no_sources = mem::MaybeUninit::uninit();
            let sources_ptr =
                NDIlib_find_get_current_sources(self.0.as_ptr(), no_sources.as_mut_ptr());
            let no_sources = no_sources.assume_init();

            if sources_ptr.is_null() || no_sources == 0 {
                return vec![];
            }

            let mut sources = vec![];
            for i in 0..no_sources {
                sources.push(Source::Borrowed(
                    ptr::NonNull::new_unchecked(sources_ptr.add(i as usize) as *mut _),
                    self,
                ));
            }

            sources
        }
    }
}

impl Drop for FindInstance {
    fn drop(&mut self) {
        unsafe {
            NDIlib_find_destroy(self.0.as_mut());
        }
    }
}

#[derive(Debug)]
pub enum Source<'a> {
    Borrowed(ptr::NonNull<NDIlib_source_t>, &'a FindInstance),
    Owned(NDIlib_source_t, ffi::CString, ffi::CString),
}

unsafe impl Send for Source<'_> {}
unsafe impl Sync for Source<'_> {}

impl Source<'_> {
    pub fn ndi_name(&self) -> &str {
        unsafe {
            let ptr = match *self {
                Source::Borrowed(ptr, _) => &*ptr.as_ptr(),
                Source::Owned(ref source, _, _) => source,
            };

            assert!(!ptr.p_ndi_name.is_null());
            ffi::CStr::from_ptr(ptr.p_ndi_name).to_str().unwrap()
        }
    }

    pub fn url_address(&self) -> &str {
        unsafe {
            let ptr = match *self {
                Source::Borrowed(ptr, _) => &*ptr.as_ptr(),
                Source::Owned(ref source, _, _) => source,
            };

            assert!(!ptr.p_url_address.is_null());
            ffi::CStr::from_ptr(ptr.p_url_address).to_str().unwrap()
        }
    }

    pub fn to_owned<'b>(&self) -> Source<'b> {
        unsafe {
            let (ndi_name, url_address) = match *self {
                Source::Borrowed(ptr, _) => (ptr.as_ref().p_ndi_name, ptr.as_ref().p_url_address),
                Source::Owned(_, ref ndi_name, ref url_address) => {
                    (ndi_name.as_ptr(), url_address.as_ptr())
                }
            };

            let ndi_name = ffi::CString::new(ffi::CStr::from_ptr(ndi_name).to_bytes()).unwrap();
            let url_address =
                ffi::CString::new(ffi::CStr::from_ptr(url_address).to_bytes()).unwrap();

            Source::Owned(
                NDIlib_source_t {
                    p_ndi_name: ndi_name.as_ptr(),
                    p_url_address: url_address.as_ptr(),
                },
                ndi_name,
                url_address,
            )
        }
    }
}

impl<'a> PartialEq for Source<'a> {
    fn eq(&self, other: &Source<'a>) -> bool {
        self.ndi_name() == other.ndi_name() && self.url_address() == other.url_address()
    }
}

#[derive(Debug)]
pub struct RecvBuilder<'a> {
    ndi_name: Option<&'a str>,
    url_address: Option<&'a str>,
    allow_video_fields: bool,
    bandwidth: NDIlib_recv_bandwidth_e,
    color_format: NDIlib_recv_color_format_e,
    ndi_recv_name: &'a str,
}

impl RecvBuilder<'_> {
    pub fn allow_video_fields(self, allow_video_fields: bool) -> Self {
        Self {
            allow_video_fields,
            ..self
        }
    }

    pub fn bandwidth(self, bandwidth: NDIlib_recv_bandwidth_e) -> Self {
        Self { bandwidth, ..self }
    }

    pub fn color_format(self, color_format: NDIlib_recv_color_format_e) -> Self {
        Self {
            color_format,
            ..self
        }
    }

    pub fn build(self) -> Option<RecvInstance> {
        unsafe {
            let ndi_recv_name = ffi::CString::new(self.ndi_recv_name).unwrap();
            let ndi_name = self
                .ndi_name
                .as_ref()
                .map(|s| ffi::CString::new(*s).unwrap());
            let url_address = self
                .url_address
                .as_ref()
                .map(|s| ffi::CString::new(*s).unwrap());
            let ptr = NDIlib_recv_create_v3(&NDIlib_recv_create_v3_t {
                source_to_connect_to: NDIlib_source_t {
                    p_ndi_name: ndi_name
                        .as_ref()
                        .map(|s| s.as_ptr())
                        .unwrap_or(ptr::null_mut()),
                    p_url_address: url_address
                        .as_ref()
                        .map(|s| s.as_ptr())
                        .unwrap_or(ptr::null_mut()),
                },
                allow_video_fields: self.allow_video_fields,
                bandwidth: self.bandwidth,
                color_format: self.color_format,
                p_ndi_recv_name: ndi_recv_name.as_ptr(),
            });

            if ptr.is_null() {
                None
            } else {
                #[allow(clippy::arc_with_non_send_sync)]
                Some(RecvInstance(Arc::new(RecvInstancePtr(
                    ptr::NonNull::new_unchecked(ptr),
                ))))
            }
        }
    }
}

#[derive(Debug)]
struct RecvInstancePtr(ptr::NonNull<::std::os::raw::c_void>);

impl Drop for RecvInstancePtr {
    fn drop(&mut self) {
        unsafe { NDIlib_recv_destroy(self.0.as_ptr() as *mut _) }
    }
}

impl RecvInstancePtr {
    fn as_ptr(&self) -> *mut ::std::os::raw::c_void {
        self.0.as_ptr()
    }
}

#[derive(Debug)]
pub struct RecvInstance(Arc<RecvInstancePtr>);

unsafe impl Send for RecvInstance {}

#[derive(Debug, Copy, Clone)]
pub struct ReceiveError;

impl fmt::Display for ReceiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Receive error")
    }
}

impl std::error::Error for ReceiveError {}

impl RecvInstance {
    pub fn builder<'a>(
        ndi_name: Option<&'a str>,
        url_address: Option<&'a str>,
        ndi_recv_name: &'a str,
    ) -> RecvBuilder<'a> {
        RecvBuilder {
            ndi_name,
            url_address,
            allow_video_fields: true,
            bandwidth: NDIlib_recv_bandwidth_highest,
            color_format: NDIlib_recv_color_format_UYVY_BGRA,
            ndi_recv_name,
        }
    }

    pub fn set_tally(&self, tally: &Tally) -> bool {
        unsafe { NDIlib_recv_set_tally(self.0.as_ptr(), &tally.0) }
    }

    pub fn send_metadata(&self, metadata: &MetadataFrame) -> bool {
        unsafe { NDIlib_recv_send_metadata(self.0.as_ptr(), metadata.as_ptr()) }
    }

    pub fn get_queue(&self) -> Queue {
        unsafe {
            let mut queue = mem::MaybeUninit::uninit();
            NDIlib_recv_get_queue(self.0.as_ptr(), queue.as_mut_ptr());
            Queue(queue.assume_init())
        }
    }

    pub fn capture(&self, timeout_in_ms: u32) -> Result<Option<Frame>, ReceiveError> {
        unsafe {
            let ptr = self.0.as_ptr();

            let mut video_frame = mem::zeroed();
            let mut audio_frame = mem::zeroed();
            let mut metadata_frame = mem::zeroed();

            let res = NDIlib_recv_capture_v3(
                ptr,
                &mut video_frame,
                &mut audio_frame,
                &mut metadata_frame,
                timeout_in_ms,
            );

            match res {
                NDIlib_frame_type_e::NDIlib_frame_type_audio => Ok(Some(Frame::Audio(AudioFrame(
                    AudioFrameInner::BorrowedRecv(audio_frame, Arc::clone(&self.0)),
                )))),
                NDIlib_frame_type_e::NDIlib_frame_type_video => Ok(Some(Frame::Video(VideoFrame(
                    VideoFrameInner::BorrowedRecv(video_frame, Arc::clone(&self.0)),
                )))),
                NDIlib_frame_type_e::NDIlib_frame_type_metadata => {
                    Ok(Some(Frame::Metadata(MetadataFrame(
                        MetadataFrameInner::Borrowed(metadata_frame, Arc::clone(&self.0)),
                    ))))
                }
                NDIlib_frame_type_e::NDIlib_frame_type_error => Err(ReceiveError),
                _ => Ok(None),
            }
        }
    }
}

#[derive(Debug)]
pub struct SendBuilder<'a> {
    ndi_name: &'a str,
    clock_audio: bool,
    clock_video: bool,
}

impl SendBuilder<'_> {
    pub fn clock_audio(self) -> Self {
        Self {
            clock_audio: true,
            ..self
        }
    }

    pub fn clock_video(self) -> Self {
        Self {
            clock_video: true,
            ..self
        }
    }

    pub fn build(self) -> Option<SendInstance> {
        unsafe {
            let ndi_name = ffi::CString::new(self.ndi_name).unwrap();
            let ptr = NDIlib_send_create(&NDIlib_send_create_t {
                p_ndi_name: ndi_name.as_ptr(),
                clock_video: self.clock_video,
                clock_audio: self.clock_audio,
                p_groups: ptr::null(),
            });

            if ptr.is_null() {
                None
            } else {
                Some(SendInstance(ptr::NonNull::new_unchecked(ptr)))
            }
        }
    }
}

#[derive(Debug)]
pub struct SendInstance(ptr::NonNull<::std::os::raw::c_void>);

unsafe impl Send for SendInstance {}

impl SendInstance {
    pub fn builder(ndi_name: &str) -> SendBuilder<'_> {
        SendBuilder {
            ndi_name,
            clock_video: false,
            clock_audio: false,
        }
    }

    pub fn send_video(&mut self, frame: &VideoFrame) {
        unsafe {
            NDIlib_send_send_video_v2(self.0.as_ptr(), frame.as_ptr());
        }
    }

    pub fn send_audio(&mut self, frame: &AudioFrame) {
        unsafe {
            NDIlib_send_send_audio_v3(self.0.as_ptr(), frame.as_ptr());
        }
    }

    pub fn send_metadata(&self, metadata: &MetadataFrame) {
        unsafe { NDIlib_send_send_metadata(self.0.as_ptr(), metadata.as_ptr()) }
    }
}

impl Drop for SendInstance {
    fn drop(&mut self) {
        unsafe { NDIlib_send_destroy(self.0.as_ptr() as *mut _) }
    }
}

#[derive(Debug)]
pub struct Tally(NDIlib_tally_t);

impl Default for Tally {
    fn default() -> Self {
        Self(NDIlib_tally_t {
            on_program: true,
            on_preview: false,
        })
    }
}

impl Tally {
    pub fn new(on_program: bool, on_preview: bool) -> Self {
        Self(NDIlib_tally_t {
            on_program,
            on_preview,
        })
    }

    pub fn on_program(&self) -> bool {
        self.0.on_program
    }

    pub fn on_preview(&self) -> bool {
        self.0.on_preview
    }
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Frame {
    Video(VideoFrame),
    Audio(AudioFrame),
    Metadata(MetadataFrame),
}

#[derive(Debug)]
pub struct VideoFrame(VideoFrameInner);

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
enum VideoFrameInner {
    //Owned(NDIlib_video_frame_v2_t, Option<ffi::CString>, Option<Vec<u8>>),
    BorrowedRecv(NDIlib_video_frame_v2_t, Arc<RecvInstancePtr>),
    BorrowedGst(
        NDIlib_video_frame_v2_t,
        gst_video::VideoFrame<gst_video::video_frame::Readable>,
        Option<std::ffi::CString>,
    ),
}

impl Drop for VideoFrameInner {
    #[allow(irrefutable_let_patterns)]
    fn drop(&mut self) {
        if let VideoFrameInner::BorrowedRecv(ref mut frame, ref recv) = *self {
            unsafe {
                NDIlib_recv_free_video_v2(recv.0.as_ptr() as *mut _, frame);
            }
        }
    }
}

unsafe impl Send for VideoFrameInner {}
unsafe impl Sync for VideoFrameInner {}

#[derive(Debug, Copy, Clone)]
pub struct TryFromVideoFrameError;

impl fmt::Display for TryFromVideoFrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Can't handle video frame")
    }
}

impl std::error::Error for TryFromVideoFrameError {}

impl VideoFrame {
    pub fn xres(&self) -> i32 {
        match self.0 {
            VideoFrameInner::BorrowedRecv(ref frame, _)
            | VideoFrameInner::BorrowedGst(ref frame, ..) => frame.xres,
        }
    }

    pub fn yres(&self) -> i32 {
        match self.0 {
            VideoFrameInner::BorrowedRecv(ref frame, _)
            | VideoFrameInner::BorrowedGst(ref frame, ..) => frame.yres,
        }
    }

    pub fn fourcc(&self) -> NDIlib_FourCC_video_type_e {
        match self.0 {
            VideoFrameInner::BorrowedRecv(ref frame, _)
            | VideoFrameInner::BorrowedGst(ref frame, ..) => frame.FourCC,
        }
    }

    pub fn frame_rate(&self) -> (i32, i32) {
        match self.0 {
            VideoFrameInner::BorrowedRecv(ref frame, _)
            | VideoFrameInner::BorrowedGst(ref frame, ..) => {
                (frame.frame_rate_N, frame.frame_rate_D)
            }
        }
    }

    pub fn picture_aspect_ratio(&self) -> f32 {
        match self.0 {
            VideoFrameInner::BorrowedRecv(ref frame, _)
            | VideoFrameInner::BorrowedGst(ref frame, ..) => frame.picture_aspect_ratio,
        }
    }

    pub fn frame_format_type(&self) -> NDIlib_frame_format_type_e {
        match self.0 {
            VideoFrameInner::BorrowedRecv(ref frame, _)
            | VideoFrameInner::BorrowedGst(ref frame, ..) => frame.frame_format_type,
        }
    }

    pub fn timecode(&self) -> i64 {
        match self.0 {
            VideoFrameInner::BorrowedRecv(ref frame, _)
            | VideoFrameInner::BorrowedGst(ref frame, ..) => frame.timecode,
        }
    }

    pub fn data(&self) -> Option<&[u8]> {
        let fourcc = self.fourcc();

        if [
            NDIlib_FourCC_video_type_UYVY,
            NDIlib_FourCC_video_type_UYVA,
            NDIlib_FourCC_video_type_P216,
            NDIlib_FourCC_video_type_PA16,
            NDIlib_FourCC_video_type_YV12,
            NDIlib_FourCC_video_type_I420,
            NDIlib_FourCC_video_type_NV12,
            NDIlib_FourCC_video_type_BGRA,
            NDIlib_FourCC_video_type_BGRX,
            NDIlib_FourCC_video_type_RGBA,
            NDIlib_FourCC_video_type_RGBX,
        ]
        .contains(&fourcc)
        {
            // FIXME: Unclear if this is correct. Needs to be validated against an actual
            // interlaced stream
            let frame_size = if self.frame_format_type()
                == NDIlib_frame_format_type_e::NDIlib_frame_format_type_field_0
                || self.frame_format_type()
                    == NDIlib_frame_format_type_e::NDIlib_frame_format_type_field_1
            {
                self.yres() * self.line_stride_or_data_size_in_bytes() / 2
            } else {
                self.yres() * self.line_stride_or_data_size_in_bytes()
            };

            return unsafe {
                use std::slice;
                match self.0 {
                    VideoFrameInner::BorrowedRecv(ref frame, ..)
                    | VideoFrameInner::BorrowedGst(ref frame, ..) => Some(slice::from_raw_parts(
                        frame.p_data as *const u8,
                        frame_size as usize,
                    )),
                }
            };
        }

        #[cfg(feature = "advanced-sdk")]
        if [
            NDIlib_FourCC_video_type_ex_SHQ0_highest_bandwidth,
            NDIlib_FourCC_video_type_ex_SHQ2_highest_bandwidth,
            NDIlib_FourCC_video_type_ex_SHQ7_highest_bandwidth,
            NDIlib_FourCC_video_type_ex_SHQ0_lowest_bandwidth,
            NDIlib_FourCC_video_type_ex_SHQ2_lowest_bandwidth,
            NDIlib_FourCC_video_type_ex_SHQ7_lowest_bandwidth,
        ]
        .contains(&fourcc)
        {
            return unsafe {
                use std::slice;
                match self.0 {
                    VideoFrameInner::BorrowedRecv(ref frame, ..)
                    | VideoFrameInner::BorrowedGst(ref frame, ..) => Some(slice::from_raw_parts(
                        frame.p_data as *const u8,
                        frame.line_stride_or_data_size_in_bytes as usize,
                    )),
                }
            };
        }

        None
    }

    #[cfg(feature = "advanced-sdk")]
    pub fn compressed_packet(&self) -> Option<CompressedPacket<'_>> {
        use byteorder::{LittleEndian, ReadBytesExt};
        use std::io::Cursor;
        use std::slice;

        unsafe {
            let fourcc = self.fourcc();

            if ![
                NDIlib_FourCC_video_type_ex_H264_highest_bandwidth,
                NDIlib_FourCC_video_type_ex_H264_lowest_bandwidth,
                NDIlib_FourCC_video_type_ex_HEVC_highest_bandwidth,
                NDIlib_FourCC_video_type_ex_HEVC_lowest_bandwidth,
                NDIlib_FourCC_video_type_ex_H264_alpha_highest_bandwidth,
                NDIlib_FourCC_video_type_ex_H264_alpha_lowest_bandwidth,
                NDIlib_FourCC_video_type_ex_HEVC_alpha_highest_bandwidth,
                NDIlib_FourCC_video_type_ex_HEVC_alpha_lowest_bandwidth,
            ]
            .contains(&fourcc)
            {
                return None;
            }

            let data = match self.0 {
                VideoFrameInner::BorrowedRecv(ref frame, ..)
                | VideoFrameInner::BorrowedGst(ref frame, ..) => slice::from_raw_parts(
                    frame.p_data as *const u8,
                    frame.line_stride_or_data_size_in_bytes as usize,
                ),
            };

            let mut cursor = Cursor::new(data);
            let version = cursor.read_u32::<LittleEndian>().ok()?;
            if version != ndisys::NDIlib_compressed_packet_version_0 {
                return None;
            }

            let fourcc = cursor.read_u32::<LittleEndian>().ok()?;
            let pts = cursor.read_i64::<LittleEndian>().ok()?;
            let dts = cursor.read_i64::<LittleEndian>().ok()?;
            let _reserved = cursor.read_u64::<LittleEndian>().ok()?;
            let flags = cursor.read_u32::<LittleEndian>().ok()?;
            let data_size = cursor.read_u32::<LittleEndian>().ok()?;
            let extra_data_size = cursor.read_u32::<LittleEndian>().ok()?;

            let expected_size = (ndisys::NDIlib_compressed_packet_version_0 as usize)
                .checked_add(data_size as usize)?
                .checked_add(extra_data_size as usize)?;
            if data.len() < expected_size {
                return None;
            }

            Some(CompressedPacket {
                fourcc,
                pts,
                dts,
                key_frame: flags & ndisys::NDIlib_compressed_packet_flags_keyframe != 0,
                data: &data[ndisys::NDIlib_compressed_packet_version_0 as usize..]
                    [..data_size as usize],
                extra_data: if extra_data_size > 0 {
                    Some(
                        &data[ndisys::NDIlib_compressed_packet_version_0 as usize
                            + data_size as usize..][..extra_data_size as usize],
                    )
                } else {
                    None
                },
            })
        }
    }

    pub fn line_stride_or_data_size_in_bytes(&self) -> i32 {
        match self.0 {
            VideoFrameInner::BorrowedRecv(ref frame, _)
            | VideoFrameInner::BorrowedGst(ref frame, ..) => {
                let stride = frame.line_stride_or_data_size_in_bytes;

                if stride != 0 {
                    return stride;
                }

                let xres = frame.xres;

                match frame.FourCC {
                    ndisys::NDIlib_FourCC_video_type_UYVY
                    | ndisys::NDIlib_FourCC_video_type_UYVA
                    | ndisys::NDIlib_FourCC_video_type_YV12
                    | ndisys::NDIlib_FourCC_video_type_NV12
                    | ndisys::NDIlib_FourCC_video_type_I420
                    | ndisys::NDIlib_FourCC_video_type_BGRA
                    | ndisys::NDIlib_FourCC_video_type_BGRX
                    | ndisys::NDIlib_FourCC_video_type_RGBA
                    | ndisys::NDIlib_FourCC_video_type_RGBX => xres,
                    ndisys::NDIlib_FourCC_video_type_P216
                    | ndisys::NDIlib_FourCC_video_type_PA16 => 2 * xres,
                    _ => 0,
                }
            }
        }
    }

    pub fn metadata(&self) -> Option<&str> {
        unsafe {
            match self.0 {
                VideoFrameInner::BorrowedRecv(ref frame, _)
                | VideoFrameInner::BorrowedGst(ref frame, ..) => {
                    if frame.p_metadata.is_null() {
                        None
                    } else {
                        Some(ffi::CStr::from_ptr(frame.p_metadata).to_str().unwrap())
                    }
                }
            }
        }
    }

    pub fn timestamp(&self) -> i64 {
        match self.0 {
            VideoFrameInner::BorrowedRecv(ref frame, _)
            | VideoFrameInner::BorrowedGst(ref frame, ..) => frame.timestamp,
        }
    }

    pub fn as_ptr(&self) -> *const NDIlib_video_frame_v2_t {
        match self.0 {
            VideoFrameInner::BorrowedRecv(ref frame, _)
            | VideoFrameInner::BorrowedGst(ref frame, ..) => frame,
        }
    }

    pub fn try_from_video_frame(
        frame: gst_video::VideoFrame<gst_video::video_frame::Readable>,
        metadata: Option<std::ffi::CString>,
        timecode: i64,
    ) -> Result<Self, TryFromVideoFrameError> {
        // Planar formats must be in contiguous memory
        let format = match frame.format() {
            gst_video::VideoFormat::Uyvy => ndisys::NDIlib_FourCC_video_type_UYVY,
            gst_video::VideoFormat::I420 => {
                if (frame.plane_data(1).unwrap().as_ptr() as usize)
                    .checked_sub(frame.plane_data(0).unwrap().as_ptr() as usize)
                    != Some(frame.height() as usize * frame.plane_stride()[0] as usize)
                {
                    return Err(TryFromVideoFrameError);
                }

                if (frame.plane_data(2).unwrap().as_ptr() as usize)
                    .checked_sub(frame.plane_data(1).unwrap().as_ptr() as usize)
                    != Some(
                        (frame.height() as usize).div_ceil(2) * frame.plane_stride()[1] as usize,
                    )
                {
                    return Err(TryFromVideoFrameError);
                }

                ndisys::NDIlib_FourCC_video_type_I420
            }
            gst_video::VideoFormat::Nv12 => {
                if (frame.plane_data(1).unwrap().as_ptr() as usize)
                    .checked_sub(frame.plane_data(0).unwrap().as_ptr() as usize)
                    != Some(frame.height() as usize * frame.plane_stride()[0] as usize)
                {
                    return Err(TryFromVideoFrameError);
                }

                ndisys::NDIlib_FourCC_video_type_NV12
            }
            gst_video::VideoFormat::Nv21 => {
                if (frame.plane_data(1).unwrap().as_ptr() as usize)
                    .checked_sub(frame.plane_data(0).unwrap().as_ptr() as usize)
                    != Some(frame.height() as usize * frame.plane_stride()[0] as usize)
                {
                    return Err(TryFromVideoFrameError);
                }

                ndisys::NDIlib_FourCC_video_type_NV12
            }
            gst_video::VideoFormat::Yv12 => {
                if (frame.plane_data(1).unwrap().as_ptr() as usize)
                    .checked_sub(frame.plane_data(0).unwrap().as_ptr() as usize)
                    != Some(frame.height() as usize * frame.plane_stride()[0] as usize)
                {
                    return Err(TryFromVideoFrameError);
                }

                if (frame.plane_data(2).unwrap().as_ptr() as usize)
                    .checked_sub(frame.plane_data(1).unwrap().as_ptr() as usize)
                    != Some(
                        (frame.height() as usize).div_ceil(2) * frame.plane_stride()[1] as usize,
                    )
                {
                    return Err(TryFromVideoFrameError);
                }

                ndisys::NDIlib_FourCC_video_type_YV12
            }
            gst_video::VideoFormat::Bgra => ndisys::NDIlib_FourCC_video_type_BGRA,
            gst_video::VideoFormat::Bgrx => ndisys::NDIlib_FourCC_video_type_BGRX,
            gst_video::VideoFormat::Rgba => ndisys::NDIlib_FourCC_video_type_RGBA,
            gst_video::VideoFormat::Rgbx => ndisys::NDIlib_FourCC_video_type_RGBX,
            _ => return Err(TryFromVideoFrameError),
        };

        let frame_format_type = match frame.info().interlace_mode() {
            gst_video::VideoInterlaceMode::Progressive => {
                NDIlib_frame_format_type_e::NDIlib_frame_format_type_progressive
            }
            gst_video::VideoInterlaceMode::Interleaved => {
                NDIlib_frame_format_type_e::NDIlib_frame_format_type_interleaved
            }
            // FIXME: Is this correct?
            gst_video::VideoInterlaceMode::Alternate
                if frame.flags().contains(gst_video::VideoFrameFlags::TFF) =>
            {
                NDIlib_frame_format_type_e::NDIlib_frame_format_type_field_0
            }
            gst_video::VideoInterlaceMode::Alternate
                if !frame.flags().contains(gst_video::VideoFrameFlags::TFF) =>
            {
                NDIlib_frame_format_type_e::NDIlib_frame_format_type_field_1
            }
            _ => return Err(TryFromVideoFrameError),
        };

        let picture_aspect_ratio =
            frame.info().par() * gst::Fraction::new(frame.width() as i32, frame.height() as i32);
        let picture_aspect_ratio =
            picture_aspect_ratio.numer() as f32 / picture_aspect_ratio.denom() as f32;

        let ndi_frame = NDIlib_video_frame_v2_t {
            xres: frame.width() as i32,
            yres: frame.height() as i32,
            FourCC: format,
            frame_rate_N: frame.info().fps().numer(),
            frame_rate_D: frame.info().fps().denom(),
            picture_aspect_ratio,
            frame_format_type,
            timecode,
            p_data: frame.plane_data(0).unwrap().as_ptr() as *const ::std::os::raw::c_char,
            line_stride_or_data_size_in_bytes: frame.plane_stride()[0],
            p_metadata: metadata.as_ref().map_or(ptr::null(), |meta| meta.as_ptr()),
            timestamp: 0,
        };

        Ok(VideoFrame(VideoFrameInner::BorrowedGst(
            ndi_frame, frame, metadata,
        )))
    }
}

#[derive(Debug)]
pub struct AudioFrame(AudioFrameInner);

#[derive(Debug)]
enum AudioFrameInner {
    Owned(
        NDIlib_audio_frame_v3_t,
        Option<ffi::CString>,
        Option<Vec<f32>>,
    ),
    BorrowedRecv(NDIlib_audio_frame_v3_t, Arc<RecvInstancePtr>),
}

impl Drop for AudioFrameInner {
    #[allow(irrefutable_let_patterns)]
    fn drop(&mut self) {
        if let AudioFrameInner::BorrowedRecv(ref mut frame, ref recv) = *self {
            unsafe {
                NDIlib_recv_free_audio_v3(recv.0.as_ptr() as *mut _, frame);
            }
        }
    }
}

unsafe impl Send for AudioFrameInner {}
unsafe impl Sync for AudioFrameInner {}

#[derive(Debug, Copy, Clone)]
pub struct TryFromAudioBufferError;

impl fmt::Display for TryFromAudioBufferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Can't handle audio buffer")
    }
}

impl std::error::Error for TryFromAudioBufferError {}

impl AudioFrame {
    pub fn sample_rate(&self) -> i32 {
        match self.0 {
            AudioFrameInner::BorrowedRecv(ref frame, _)
            | AudioFrameInner::Owned(ref frame, _, _) => frame.sample_rate,
        }
    }

    pub fn no_channels(&self) -> i32 {
        match self.0 {
            AudioFrameInner::BorrowedRecv(ref frame, _)
            | AudioFrameInner::Owned(ref frame, _, _) => frame.no_channels,
        }
    }

    pub fn no_samples(&self) -> i32 {
        match self.0 {
            AudioFrameInner::BorrowedRecv(ref frame, _)
            | AudioFrameInner::Owned(ref frame, _, _) => frame.no_samples,
        }
    }

    pub fn timecode(&self) -> i64 {
        match self.0 {
            AudioFrameInner::BorrowedRecv(ref frame, _)
            | AudioFrameInner::Owned(ref frame, _, _) => frame.timecode,
        }
    }

    pub fn fourcc(&self) -> NDIlib_FourCC_audio_type_e {
        match self.0 {
            AudioFrameInner::BorrowedRecv(ref frame, _)
            | AudioFrameInner::Owned(ref frame, _, _) => frame.FourCC,
        }
    }

    pub fn data(&self) -> Option<&[u8]> {
        unsafe {
            use std::slice;

            let fourcc = self.fourcc();

            if [NDIlib_FourCC_audio_type_FLTp].contains(&fourcc) {
                return match self.0 {
                    AudioFrameInner::BorrowedRecv(ref frame, _)
                    | AudioFrameInner::Owned(ref frame, _, _) => Some(slice::from_raw_parts(
                        frame.p_data as *const u8,
                        (frame.no_channels * frame.channel_stride_or_data_size_in_bytes) as usize,
                    )),
                };
            }

            #[cfg(feature = "advanced-sdk")]
            if [NDIlib_FourCC_audio_type_Opus].contains(&fourcc) {
                return match self.0 {
                    AudioFrameInner::BorrowedRecv(ref frame, _)
                    | AudioFrameInner::Owned(ref frame, _, _) => Some(slice::from_raw_parts(
                        frame.p_data as *const u8,
                        frame.channel_stride_or_data_size_in_bytes as usize,
                    )),
                };
            }

            None
        }
    }

    #[cfg(feature = "advanced-sdk")]
    pub fn compressed_packet(&self) -> Option<CompressedPacket<'_>> {
        use byteorder::{LittleEndian, ReadBytesExt};
        use std::io::Cursor;
        use std::slice;

        unsafe {
            let fourcc = self.fourcc();

            if ![NDIlib_FourCC_audio_type_AAC].contains(&fourcc) {
                return None;
            }

            let data = match self.0 {
                AudioFrameInner::BorrowedRecv(ref frame, _)
                | AudioFrameInner::Owned(ref frame, _, _) => slice::from_raw_parts(
                    frame.p_data as *const u8,
                    frame.channel_stride_or_data_size_in_bytes as usize,
                ),
            };

            let mut cursor = Cursor::new(data);
            let version = cursor.read_u32::<LittleEndian>().ok()?;
            if version != ndisys::NDIlib_compressed_packet_version_0 {
                return None;
            }

            let fourcc = cursor.read_u32::<LittleEndian>().ok()?;
            let pts = cursor.read_i64::<LittleEndian>().ok()?;
            let dts = cursor.read_i64::<LittleEndian>().ok()?;
            let _reserved = cursor.read_u64::<LittleEndian>().ok()?;
            let flags = cursor.read_u32::<LittleEndian>().ok()?;
            let data_size = cursor.read_u32::<LittleEndian>().ok()?;
            let extra_data_size = cursor.read_u32::<LittleEndian>().ok()?;

            let expected_size = (ndisys::NDIlib_compressed_packet_version_0 as usize)
                .checked_add(data_size as usize)?
                .checked_add(extra_data_size as usize)?;
            if data.len() < expected_size {
                return None;
            }

            Some(CompressedPacket {
                fourcc,
                pts,
                dts,
                key_frame: flags & ndisys::NDIlib_compressed_packet_flags_keyframe != 0,
                data: &data[ndisys::NDIlib_compressed_packet_version_0 as usize..]
                    [..data_size as usize],
                extra_data: if extra_data_size > 0 {
                    Some(
                        &data[ndisys::NDIlib_compressed_packet_version_0 as usize
                            + data_size as usize..][..extra_data_size as usize],
                    )
                } else {
                    None
                },
            })
        }
    }

    pub fn channel_stride_or_data_size_in_bytes(&self) -> i32 {
        match self.0 {
            AudioFrameInner::BorrowedRecv(ref frame, _)
            | AudioFrameInner::Owned(ref frame, _, _) => frame.channel_stride_or_data_size_in_bytes,
        }
    }

    pub fn metadata(&self) -> Option<&str> {
        unsafe {
            match self.0 {
                AudioFrameInner::BorrowedRecv(ref frame, _)
                | AudioFrameInner::Owned(ref frame, _, _) => {
                    if frame.p_metadata.is_null() {
                        None
                    } else {
                        Some(ffi::CStr::from_ptr(frame.p_metadata).to_str().unwrap())
                    }
                }
            }
        }
    }

    pub fn timestamp(&self) -> i64 {
        match self.0 {
            AudioFrameInner::BorrowedRecv(ref frame, _)
            | AudioFrameInner::Owned(ref frame, _, _) => frame.timestamp,
        }
    }

    pub fn as_ptr(&self) -> *const NDIlib_audio_frame_v3_t {
        match self.0 {
            AudioFrameInner::BorrowedRecv(ref frame, _)
            | AudioFrameInner::Owned(ref frame, _, _) => frame,
        }
    }

    pub fn try_from_buffer(
        info: &gst_audio::AudioInfo,
        buffer: &gst::BufferRef,
        timecode: i64,
    ) -> Result<Self, TryFromAudioBufferError> {
        if info.format() != gst_audio::AUDIO_FORMAT_F32 {
            return Err(TryFromAudioBufferError);
        }

        let map = buffer.map_readable().map_err(|_| TryFromAudioBufferError)?;
        let src_data = map
            .as_slice_of::<f32>()
            .map_err(|_| TryFromAudioBufferError)?;

        let no_samples = src_data.len() as i32 / info.channels() as i32;
        let channel_stride_or_data_size_in_bytes = no_samples * mem::size_of::<f32>() as i32;
        let mut dest_data =
            Vec::<f32>::with_capacity(no_samples as usize * info.channels() as usize);

        assert_eq!(dest_data.capacity(), src_data.len());

        unsafe {
            let dest_ptr = dest_data.as_mut_ptr();

            for (i, samples) in src_data.chunks_exact(info.channels() as usize).enumerate() {
                for (c, sample) in samples.iter().enumerate() {
                    ptr::write(dest_ptr.add(c * no_samples as usize + i), *sample);
                }
            }

            dest_data.set_len(no_samples as usize * info.channels() as usize);
        }

        let dest = NDIlib_audio_frame_v3_t {
            sample_rate: info.rate() as i32,
            no_channels: info.channels() as i32,
            no_samples,
            timecode,
            FourCC: NDIlib_FourCC_audio_type_FLTp,
            p_data: dest_data.as_mut_ptr(),
            channel_stride_or_data_size_in_bytes,
            p_metadata: ptr::null(),
            timestamp: 0,
        };

        Ok(AudioFrame(AudioFrameInner::Owned(
            dest,
            None,
            Some(dest_data),
        )))
    }
}

#[cfg(feature = "advanced-sdk")]
pub struct CompressedPacket<'a> {
    pub fourcc: ndisys::NDIlib_compressed_FourCC_type_e,
    pub pts: i64,
    pub dts: i64,
    pub key_frame: bool,
    pub data: &'a [u8],
    pub extra_data: Option<&'a [u8]>,
}

#[derive(Debug)]
pub struct MetadataFrame(MetadataFrameInner);

#[derive(Debug)]
enum MetadataFrameInner {
    Owned(NDIlib_metadata_frame_t, Option<ffi::CString>),
    Borrowed(NDIlib_metadata_frame_t, Arc<RecvInstancePtr>),
}

impl Drop for MetadataFrameInner {
    fn drop(&mut self) {
        if let MetadataFrameInner::Borrowed(ref mut frame, ref recv) = *self {
            unsafe {
                NDIlib_recv_free_metadata(recv.0.as_ptr() as *mut _, frame);
            }
        }
    }
}

unsafe impl Send for MetadataFrameInner {}
unsafe impl Sync for MetadataFrameInner {}

impl MetadataFrame {
    pub fn new(timecode: i64, data: Option<&str>) -> Self {
        let data = data.map(|s| ffi::CString::new(s).unwrap());

        MetadataFrame(MetadataFrameInner::Owned(
            NDIlib_metadata_frame_t {
                length: data
                    .as_ref()
                    .map(|s| s.to_str().unwrap().len())
                    .unwrap_or(0) as i32,
                timecode,
                p_data: data
                    .as_ref()
                    .map(|s| s.as_ptr() as *mut _)
                    .unwrap_or(ptr::null_mut()),
            },
            data,
        ))
    }

    pub fn timecode(&self) -> i64 {
        match self.0 {
            MetadataFrameInner::Owned(ref frame, _) => frame.timecode,
            MetadataFrameInner::Borrowed(ref frame, _) => frame.timecode,
        }
    }

    pub fn metadata(&self) -> Option<&str> {
        unsafe {
            match self.0 {
                MetadataFrameInner::Owned(_, ref metadata) => {
                    metadata.as_ref().map(|s| s.to_str().unwrap())
                }
                MetadataFrameInner::Borrowed(ref frame, _) => {
                    if frame.p_data.is_null() || frame.length == 0 {
                        None
                    } else if frame.length != 0 {
                        use std::slice;

                        Some(
                            ffi::CStr::from_bytes_with_nul_unchecked(slice::from_raw_parts(
                                frame.p_data as *const u8,
                                frame.length as usize,
                            ))
                            .to_str()
                            .unwrap(),
                        )
                    } else {
                        Some(ffi::CStr::from_ptr(frame.p_data).to_str().unwrap())
                    }
                }
            }
        }
    }

    pub fn as_ptr(&self) -> *const NDIlib_metadata_frame_t {
        match self.0 {
            MetadataFrameInner::Owned(ref frame, _) => frame,
            MetadataFrameInner::Borrowed(ref frame, _) => frame,
        }
    }
}

impl Default for MetadataFrame {
    fn default() -> Self {
        MetadataFrame(MetadataFrameInner::Owned(
            NDIlib_metadata_frame_t {
                length: 0,
                timecode: 0, //NDIlib_send_timecode_synthesize,
                p_data: ptr::null(),
            },
            None,
        ))
    }
}

#[derive(Debug, Clone)]
pub struct Queue(NDIlib_recv_queue_t);

impl Queue {
    pub fn audio_frames(&self) -> i32 {
        self.0.audio_frames
    }
    pub fn video_frames(&self) -> i32 {
        self.0.video_frames
    }
    pub fn metadata_frames(&self) -> i32 {
        self.0.metadata_frames
    }
}
