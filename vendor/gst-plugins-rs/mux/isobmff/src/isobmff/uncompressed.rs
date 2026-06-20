use crate::isobmff::boxes::write_box;
use crate::isobmff::boxes::write_full_box;
use anyhow::Error;
use gst_video::VideoFormat;
use std::collections::BTreeMap;
use std::sync::LazyLock;

static CAT_23001: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "mp4mux-23001-17",
        gst::DebugColorFlags::empty(),
        Some("MP4Mux Element - ISO/IEC 23001-17"),
    )
});

pub(crate) fn write_uncompressed_sample_entries(
    v: &mut Vec<u8>,
    video_info: gst_video::VideoInfo,
) -> Result<(), Error> {
    let profile = *get_profile_for_uncc_format(&video_info);
    if matches!(
        video_info.format(),
        VideoFormat::Rgba | VideoFormat::Abgr | VideoFormat::Rgb
    ) && (get_row_align_size_for_uncc_format(&video_info) == 0)
    {
        write_full_box(v, b"uncC", 1, 0, move |v| {
            v.extend(profile);
            Ok(())
        })?;
    } else {
        let component_types = get_components_for_uncc_format(&video_info);
        let num_components = component_types.len() as u32;
        let mut uncc_component_bytes: Vec<u8> = Vec::new();
        for i in 0..num_components {
            uncc_component_bytes.extend((i as u16).to_be_bytes());
            let component = component_types[i as usize];
            uncc_component_bytes
                .extend(get_bit_depth_for_uncc_format(&video_info, component).to_be_bytes());
            uncc_component_bytes.extend((0_u8).to_be_bytes()); // component_format
            uncc_component_bytes.extend((0_u8).to_be_bytes()); // component_align_size
        }
        write_box(v, b"cmpd", move |v| {
            v.extend(num_components.to_be_bytes());
            for c in component_types {
                v.extend(c.to_be_bytes());
            }
            Ok(())
        })?;
        write_full_box(v, b"uncC", 0, 0, move |v| {
            v.extend(profile);
            v.extend(num_components.to_be_bytes());
            v.extend(uncc_component_bytes);
            let sampling_type = get_sampling_type_for_uncc_format(&video_info);
            gst::debug!(CAT_23001, "sampling_type: {sampling_type:?}");
            v.extend(sampling_type.to_be_bytes());
            let interleave_type = get_interleave_type_for_uncc_format(&video_info);
            gst::debug!(CAT_23001, "interleave_type: {interleave_type:?}");
            v.extend(interleave_type.to_be_bytes());
            v.extend(get_block_size_for_uncc_format(&video_info).to_be_bytes());
            v.extend(get_flag_bits_for_uncc_format(&video_info).to_be_bytes());
            v.extend(get_pixel_size_for_uncc_format(&video_info).to_be_bytes());
            let row_align_size = get_row_align_size_for_uncc_format(&video_info);
            gst::debug!(CAT_23001, "row_align_size: {row_align_size:?}");
            v.extend(row_align_size.to_be_bytes());
            v.extend((0_u32).to_be_bytes()); // tile align size
            v.extend((0_u32).to_be_bytes()); // num tile columns minus 1
            v.extend((0_u32).to_be_bytes()); // num tile rows minus 1
            Ok(())
        })?;
    }
    Ok(())
}

// See ISO/IEC 23001-17:2024 Table 1
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComponentType {
    Monochrome,
    Luma,
    Cb,
    Cr,
    Red,
    Green,
    Blue,
    Alpha,
    // There are more here, but we don't support them yet
}

impl ComponentType {
    fn to_be_bytes(self) -> [u8; 2] {
        match self {
            Self::Monochrome => 0u16.to_be_bytes(),
            Self::Luma => 1u16.to_be_bytes(),
            Self::Cb => 2u16.to_be_bytes(),
            Self::Cr => 3u16.to_be_bytes(),
            Self::Red => 4u16.to_be_bytes(),
            Self::Green => 5u16.to_be_bytes(),
            Self::Blue => 6u16.to_be_bytes(),
            Self::Alpha => 7u16.to_be_bytes(),
        }
    }
}

fn component_index_to_component_type(
    format_info: gst_video::VideoFormatInfo,
    index: u32,
) -> ComponentType {
    // See https://gstreamer.freedesktop.org/documentation/video/video-format.html?gi-language=c#GstVideoFormatFlags
    if format_info.is_rgb() {
        gst::debug!(CAT_23001, "format for {}: rgb", format_info.name());
        return match index {
            0 => ComponentType::Red,
            1 => ComponentType::Green,
            2 => ComponentType::Blue,
            3 => ComponentType::Alpha,
            _ => unreachable!(),
        };
    }
    if format_info.is_yuv() {
        gst::debug!(CAT_23001, "format for {}: yuv", format_info.name());
        return match index {
            0 => ComponentType::Luma,
            1 => ComponentType::Cb,
            2 => ComponentType::Cr,
            3 => ComponentType::Alpha,
            _ => unreachable!(),
        };
    }
    if format_info.is_gray() {
        gst::debug!(CAT_23001, "format for {}: gray", format_info.name());
        return match index {
            0 => ComponentType::Monochrome,
            _ => unreachable!(),
        };
    }
    unreachable!("format for {}: unknown", format_info.name())
}

fn component_type_to_component_index(
    format_info: gst_video::VideoFormatInfo,
    component_type: ComponentType,
) -> u8 {
    // See https://gstreamer.freedesktop.org/documentation/video/video-format.html?gi-language=c#GstVideoFormatFlags
    if format_info.is_rgb() {
        gst::debug!(CAT_23001, "format for {}: rgb", format_info.name());
        return match component_type {
            ComponentType::Red => 0,
            ComponentType::Green => 1,
            ComponentType::Blue => 2,
            ComponentType::Alpha => 3,
            _ => unreachable!(),
        };
    }
    if format_info.is_yuv() {
        gst::debug!(CAT_23001, "format for {}: yuv", format_info.name());
        return match component_type {
            ComponentType::Luma => 0,
            ComponentType::Cb => 1,
            ComponentType::Cr => 2,
            ComponentType::Alpha => 3,
            _ => unreachable!(),
        };
    }
    if format_info.is_gray() {
        gst::debug!(CAT_23001, "format for {}: gray", format_info.name());
        return match component_type {
            ComponentType::Monochrome => 0,
            _ => unreachable!(),
        };
    }
    unreachable!("format for {}: unknown", format_info.name())
}

fn get_components_for_uncc_format(video_info: &gst_video::VideoInfo) -> Vec<ComponentType> {
    let format_info = video_info.format_info();
    let mut component_offsets: BTreeMap<u32, ComponentType> = BTreeMap::new();
    let interleave_type = get_interleave_type_for_uncc_format(video_info);
    if interleave_type == 5 {
        // TODO: it would be nice to pull this out of the info, but too much for now
        return match video_info.format() {
            VideoFormat::Yuy2 => [
                ComponentType::Luma,
                ComponentType::Cb,
                ComponentType::Luma,
                ComponentType::Cr,
            ]
            .to_vec(),
            VideoFormat::Yvyu => [
                ComponentType::Luma,
                ComponentType::Cr,
                ComponentType::Luma,
                ComponentType::Cb,
            ]
            .to_vec(),
            VideoFormat::Uyvy => [
                ComponentType::Cb,
                ComponentType::Luma,
                ComponentType::Cr,
                ComponentType::Luma,
            ]
            .to_vec(),
            VideoFormat::Vyuy => [
                ComponentType::Cr,
                ComponentType::Luma,
                ComponentType::Cb,
                ComponentType::Luma,
            ]
            .to_vec(),
            _ => unreachable!(),
        };
    }
    if interleave_type == 0 {
        for comp_index in 0..video_info.n_components() {
            let comp_offset = video_info.comp_offset(comp_index as u8);
            gst::debug!(
                CAT_23001,
                "planar comp_offsets for {}, index {comp_index:?}, offset: {comp_offset:?}",
                format_info.name()
            );
            let component_type = component_index_to_component_type(format_info, comp_index);
            gst::debug!(
                CAT_23001,
                "planar component_type for {}, index {comp_index:?}: {component_type:?}",
                format_info.name()
            );
            component_offsets.insert(comp_offset as u32, component_type);
        }
        let components = component_offsets.into_values().collect();
        gst::debug!(
            CAT_23001,
            "planar components for video format {}: {components:?}",
            format_info.name()
        );
        return components;
    }
    if interleave_type == 2 {
        // mixed interleave, like NV12 or NV21
        // Assume Y channel (which comp_poffset does not include) is first
        component_offsets.insert(0, ComponentType::Luma);
    }
    let components = match video_info.format() {
        VideoFormat::R210 => [
            ComponentType::Red,
            ComponentType::Green,
            ComponentType::Blue,
        ]
        .to_vec(),
        _ => {
            for comp_index in 0..video_info.n_components() {
                let offset = video_info.comp_poffset(comp_index as u8);
                let component_type = component_index_to_component_type(format_info, comp_index);
                gst::debug!(
                    CAT_23001,
                    "component_type for {}, index {comp_index:?}: {component_type:?}",
                    format_info.name()
                );
                // plus 1 ensures that there is room for the Y value if pushed above
                component_offsets.insert(offset + 1, component_type);
            }
            component_offsets.into_values().collect()
        }
    };
    gst::debug!(
        CAT_23001,
        "components for video format {}: {components:?}",
        format_info.name()
    );
    components
}

fn get_profile_for_uncc_format(video_info: &gst_video::VideoInfo) -> &[u8; 4] {
    // See ISO/IEC 23001-17:2024 Table 5
    // Appears that no VideoFormat value matches "yuv1", "v408", "v410" or "yv22"
    match video_info.format() {
        VideoFormat::Uyvy => b"2vuy",
        VideoFormat::Yuy2 => b"yuv2",
        VideoFormat::Yvyu => b"yvyu",
        VideoFormat::Vyuy => b"vyuy",
        VideoFormat::V308 => b"v308",
        VideoFormat::Y210 => b"y210",
        VideoFormat::V210 => b"v210",
        VideoFormat::Rgb => b"rgb3",
        VideoFormat::I420 => b"i420",
        VideoFormat::Nv12 => b"nv12",
        VideoFormat::Nv21 => b"nv21",
        VideoFormat::Rgba => b"rgba",
        VideoFormat::Abgr => b"abgr",
        VideoFormat::Y42b => b"yu22",
        VideoFormat::Yv12 => b"yv20",
        _ => &[0u8, 0u8, 0u8, 0u8],
    }
}

fn get_bit_depth_for_uncc_format(
    video_info: &gst_video::VideoInfo,
    component_type: ComponentType,
) -> u8 {
    let component_index =
        component_type_to_component_index(video_info.format_info(), component_type);
    let component_bit_depth_minus_one = video_info.comp_depth(component_index) - 1;
    gst::debug!(CAT_23001, "component_bit_depth_minus_one for video format {} index {component_index:?}: {component_bit_depth_minus_one:?}", video_info.format_info().name());
    component_bit_depth_minus_one.try_into().unwrap()
}

fn get_sampling_type_for_uncc_format(video_info: &gst_video::VideoInfo) -> u8 {
    let format_info = video_info.format_info();
    let mut horiz_subsampling = 0;
    let mut vert_subsampling = 0;
    for i in 0..video_info.n_components() {
        if format_info.w_sub()[i as usize] != 0 {
            horiz_subsampling = format_info.w_sub()[i as usize];
        }
        if format_info.h_sub()[i as usize] != 0 {
            vert_subsampling = format_info.h_sub()[i as usize];
        }
    }
    if horiz_subsampling == 0 {
        // No subsampling - 4:4:4 or similar
        return 0;
    }
    if horiz_subsampling == 1 {
        if vert_subsampling == 0 {
            // 4:2:2 or similar
            return 1;
        } else if vert_subsampling == 1 {
            // 4:2:0 or similar
            if video_info.height() % 2 != 0 {
                unreachable!("4:2:0 images must have an even number of rows in 23001-17, should have failed caps negotiation");
            }
            return 2;
        } else {
            unreachable!("Unsupported vertical subsampling");
        }
    }
    if horiz_subsampling == 2 {
        // 4:1:1
        return 3;
    }
    unreachable!("unsupported horizontal subsampling");
}

pub(crate) fn get_interleave_type_for_uncc_format(video_info: &gst_video::VideoInfo) -> u8 {
    let n_components = video_info.n_components();
    let n_planes = video_info.n_planes();
    match video_info.format() {
        VideoFormat::Nv12
        | VideoFormat::Nv21
        | VideoFormat::Nv16
        | VideoFormat::Nv61
        | VideoFormat::Nv24
        | VideoFormat::Nv1264z32
        | VideoFormat::P01010be
        | VideoFormat::P01010le
        | VideoFormat::Nv1210le32
        | VideoFormat::Nv1610le32
        | VideoFormat::Nv1210le40
        | VideoFormat::P016Be
        | VideoFormat::P016Le
        | VideoFormat::P012Be
        | VideoFormat::P012Le
        | VideoFormat::Nv124l4
        | VideoFormat::Nv1232l32
        | VideoFormat::Av12 => 2,
        VideoFormat::Yuy2 | VideoFormat::Yvyu | VideoFormat::Uyvy | VideoFormat::Vyuy => 5,
        _ => {
            if (n_components == 1) || (n_components == n_planes) {
                // component interleave
                0
            } else if n_planes == 1 {
                // pixel interleave
                1
            } else {
                unreachable!()
            }
        }
    }
}

fn get_block_size_for_uncc_format(video_info: &gst_video::VideoInfo) -> u8 {
    match video_info.format() {
        VideoFormat::Iyu2
        | VideoFormat::Rgb
        | VideoFormat::Bgr
        | VideoFormat::Rgba
        | VideoFormat::Argb
        | VideoFormat::Bgra
        | VideoFormat::Abgr
        | VideoFormat::Rgbx
        | VideoFormat::Bgrx
        | VideoFormat::Nv12
        | VideoFormat::Nv21
        | VideoFormat::Y444
        | VideoFormat::I420
        | VideoFormat::Yv12
        | VideoFormat::Yuy2
        | VideoFormat::Yvyu
        | VideoFormat::Uyvy
        | VideoFormat::Vyuy
        | VideoFormat::Ayuv
        | VideoFormat::Y41b
        | VideoFormat::Y42b
        | VideoFormat::V308
        | VideoFormat::Gray8
        | VideoFormat::Gray16Be
        | VideoFormat::Nv16
        | VideoFormat::Nv61
        | VideoFormat::Gbr
        | VideoFormat::Bgrp
        | VideoFormat::Rgbp => 0,
        VideoFormat::R210 => 4,
        _ => unreachable!(),
    }
}

fn get_flag_bits_for_uncc_format(video_info: &gst_video::VideoInfo) -> u8 {
    match video_info.format() {
        VideoFormat::Iyu2
        | VideoFormat::Rgb
        | VideoFormat::Bgr
        | VideoFormat::Rgba
        | VideoFormat::Argb
        | VideoFormat::Bgra
        | VideoFormat::Abgr
        | VideoFormat::Rgbx
        | VideoFormat::Bgrx
        | VideoFormat::Nv12
        | VideoFormat::Nv21
        | VideoFormat::Y444
        | VideoFormat::I420
        | VideoFormat::Yv12
        | VideoFormat::Yuy2
        | VideoFormat::Yvyu
        | VideoFormat::Uyvy
        | VideoFormat::Vyuy
        | VideoFormat::Ayuv
        | VideoFormat::Y41b
        | VideoFormat::Y42b
        | VideoFormat::V308
        | VideoFormat::Gray8
        | VideoFormat::Gray16Be
        | VideoFormat::R210
        | VideoFormat::Nv16
        | VideoFormat::Nv61
        | VideoFormat::Gbr
        | VideoFormat::Bgrp
        | VideoFormat::Rgbp => 0,
        _ => unreachable!(),
    }
}

fn get_pixel_size_for_uncc_format(video_info: &gst_video::VideoInfo) -> u32 {
    let interleave = get_interleave_type_for_uncc_format(video_info);
    if (interleave != 1) && (interleave != 5) {
        // See ISO/IEC 23001-17:2024 Section 5.2.1.7
        return 0;
    }
    if interleave == 1 {
        // Pixel interleave
        // Assume all components use the same pixel stride
        return video_info.comp_pstride(0u8) as u32;
    }
    if interleave == 5 {
        // Multi-Y
        let pixel_size = match video_info.format() {
            VideoFormat::Yuy2 | VideoFormat::Yvyu | VideoFormat::Uyvy | VideoFormat::Vyuy => 4,
            _ => unreachable!(),
        };
        gst::debug!(
            CAT_23001,
            "Multi-Y stride for video format {}: {pixel_size:?}",
            video_info.format_info().name()
        );
        return pixel_size;
    }
    unreachable!()
}

fn get_row_align_size_for_uncc_format(video_info: &gst_video::VideoInfo) -> u32 {
    if ((get_interleave_type_for_uncc_format(video_info) == 0)
        || (get_interleave_type_for_uncc_format(video_info) == 5))
        && (get_sampling_type_for_uncc_format(video_info) != 0)
    {
        // ISO/IEC 23001-17 5.2.1.5 requires alignment to be be done with halved row alignment for 4:2:2, 4:2:0, 4:1:1
        // GStreamer uses the same row stride for subsampling (not halved). So we don't support that in general.
        if video_info.width() % 4 == 0 {
            // However we can handle it if everything is a multiple of 4, which requires no alignment.
            return 0;
        } else {
            unreachable!("23001-17 Sub-sampled images must have image width that is a multiple of 4, should have failed caps negotiation");
        }
    }
    let first_plane_stride = video_info.stride()[0] as u32;
    if (first_plane_stride == video_info.width() * video_info.n_components())
        && (video_info.comp_depth(0) == 8)
    {
        // Then there is no padding at the end of the row
        return 0;
    }
    first_plane_stride
}

#[cfg(test)]
mod tests {
    fn init() {
        use std::sync::Once;
        static INIT: Once = Once::new();

        INIT.call_once(|| {
            gst::init().unwrap();
        });
    }

    mod interleave {
        use super::*;
        use crate::isobmff::uncompressed::get_interleave_type_for_uncc_format;
        use gst_video::VideoFormat;

        const PLANAR: u8 = 0; // aka component
        const PACKED: u8 = 1; // aka pixel
        const SEMI_PLANAR: u8 = 2; // aka mixed
        const MULTI_Y: u8 = 5;

        fn check_interleave(format: VideoFormat, expected_uncompressed_interleave: u8) {
            let video_info = gst_video::VideoInfo::builder(format, 320, 240).build();
            assert!(video_info.is_ok());
            let interleave = get_interleave_type_for_uncc_format(&video_info.unwrap());
            assert_eq!(interleave, expected_uncompressed_interleave);
        }

        #[test]
        fn interleave_i420() {
            init();
            check_interleave(VideoFormat::I420, PLANAR);
        }

        #[test]
        fn interleave_yv12() {
            init();
            check_interleave(VideoFormat::Yv12, PLANAR);
        }

        #[test]
        fn interleave_yuy2() {
            init();
            check_interleave(VideoFormat::Yuy2, MULTI_Y);
        }

        #[test]
        fn interleave_uyvy() {
            init();
            check_interleave(VideoFormat::Uyvy, MULTI_Y);
        }

        #[test]
        fn interleave_ayuv() {
            init();
            check_interleave(VideoFormat::Ayuv, PACKED);
        }

        #[test]
        fn interleave_rgbx() {
            init();
            check_interleave(VideoFormat::Rgbx, PACKED);
        }

        #[test]
        fn interleave_bgrx() {
            init();
            check_interleave(VideoFormat::Bgrx, PACKED);
        }

        #[test]
        fn interleave_xrgb() {
            init();
            check_interleave(VideoFormat::Xrgb, PACKED);
        }

        #[test]
        fn interleave_xbgr() {
            init();
            check_interleave(VideoFormat::Xbgr, PACKED);
        }

        #[test]
        fn interleave_argb() {
            init();
            check_interleave(VideoFormat::Argb, PACKED);
        }

        #[test]
        fn interleave_abgr() {
            init();
            check_interleave(VideoFormat::Abgr, PACKED);
        }

        #[test]
        fn interleave_rgb() {
            init();
            check_interleave(VideoFormat::Rgb, PACKED);
        }

        #[test]
        fn interleave_bgr() {
            init();
            check_interleave(VideoFormat::Bgr, PACKED);
        }

        #[test]
        fn interleave_y41b() {
            init();
            check_interleave(VideoFormat::Y41b, PLANAR);
        }

        #[test]
        fn interleave_y42b() {
            init();
            check_interleave(VideoFormat::Y42b, PLANAR);
        }

        #[test]
        fn interleave_yvyu() {
            init();
            check_interleave(VideoFormat::Yvyu, MULTI_Y);
        }

        #[test]
        fn interleave_y444() {
            init();
            check_interleave(VideoFormat::Y444, PLANAR);
        }

        #[test]
        fn interleave_v210() {
            init();
            check_interleave(VideoFormat::V210, PACKED);
        }

        #[test]
        fn interleave_v216() {
            init();
            check_interleave(VideoFormat::V216, PACKED);
        }

        #[test]
        fn interleave_nv12() {
            init();
            check_interleave(VideoFormat::Nv12, SEMI_PLANAR);
        }

        #[test]
        fn interleave_nv21() {
            init();
            check_interleave(VideoFormat::Nv21, SEMI_PLANAR);
        }

        #[test]
        fn interleave_gray8() {
            init();
            check_interleave(VideoFormat::Gray8, PLANAR);
        }

        #[test]
        fn interleave_gray16be() {
            init();
            check_interleave(VideoFormat::Gray16Be, PLANAR);
        }

        #[test]
        fn interleave_gray16le() {
            init();
            check_interleave(VideoFormat::Gray16Le, PLANAR);
        }

        #[test]
        fn interleave_v308() {
            init();
            check_interleave(VideoFormat::V308, PACKED);
        }

        #[test]
        fn interleave_rgb16() {
            init();
            check_interleave(VideoFormat::Rgb16, PACKED);
        }

        #[test]
        fn interleave_bgr16() {
            init();
            check_interleave(VideoFormat::Bgr16, PACKED);
        }

        #[test]
        fn interleave_rgb15() {
            init();
            check_interleave(VideoFormat::Rgb15, PACKED);
        }

        #[test]
        fn interleave_bgr15() {
            init();
            check_interleave(VideoFormat::Bgr15, PACKED);
        }

        #[test]
        fn interleave_uyvp() {
            init();
            check_interleave(VideoFormat::Uyvp, PACKED);
        }

        #[test]
        fn interleave_a420() {
            init();
            check_interleave(VideoFormat::A420, PLANAR);
        }

        // TODO: RGB8P (35)

        #[test]
        fn interleave_yuv9() {
            init();
            check_interleave(VideoFormat::Yuv9, PLANAR);
        }

        #[test]
        fn interleave_yvu9() {
            init();
            check_interleave(VideoFormat::Yvu9, PLANAR);
        }

        #[test]
        fn interleave_iyu1() {
            init();
            check_interleave(VideoFormat::Iyu1, PACKED);
        }

        #[test]
        fn interleave_argb64() {
            init();
            check_interleave(VideoFormat::Argb64, PACKED);
        }

        #[test]
        fn interleave_ayuv64() {
            init();
            check_interleave(VideoFormat::Ayuv64, PACKED);
        }

        #[test]
        fn interleave_r210() {
            init();
            check_interleave(VideoFormat::R210, PACKED);
        }

        #[test]
        fn interleave_i420_10be() {
            init();
            check_interleave(VideoFormat::I42010be, PLANAR);
        }

        #[test]
        fn interleave_i420_10le() {
            init();
            check_interleave(VideoFormat::I42010le, PLANAR);
        }

        #[test]
        fn interleave_i422_10be() {
            init();
            check_interleave(VideoFormat::I42210be, PLANAR);
        }

        #[test]
        fn interleave_i422_10le() {
            init();
            check_interleave(VideoFormat::I42210le, PLANAR);
        }

        #[test]
        fn interleave_y444_10be() {
            init();
            check_interleave(VideoFormat::Y44410be, PLANAR);
        }

        #[test]
        fn interleave_y444_10le() {
            init();
            check_interleave(VideoFormat::Y44410le, PLANAR);
        }

        #[test]
        fn interleave_gbr() {
            init();
            check_interleave(VideoFormat::Gbr, PLANAR);
        }

        #[test]
        fn interleave_gbr_10be() {
            init();
            check_interleave(VideoFormat::Gbr10be, PLANAR);
        }

        #[test]
        fn interleave_gbr_10le() {
            init();
            check_interleave(VideoFormat::Gbr10le, PLANAR);
        }

        #[test]
        fn interleave_nv16() {
            init();
            check_interleave(VideoFormat::Nv16, SEMI_PLANAR);
        }

        #[test]
        fn interleave_nv24() {
            init();
            check_interleave(VideoFormat::Nv24, SEMI_PLANAR);
        }

        #[test]
        fn interleave_nv12_64z32() {
            init();
            check_interleave(VideoFormat::Nv1264z32, SEMI_PLANAR);
        }

        #[test]
        fn interleave_a420_10be() {
            init();
            check_interleave(VideoFormat::A42010be, PLANAR);
        }

        #[test]
        fn interleave_a420_10le() {
            init();
            check_interleave(VideoFormat::A42010le, PLANAR);
        }

        #[test]
        fn interleave_a422_10be() {
            init();
            check_interleave(VideoFormat::A42210be, PLANAR);
        }

        #[test]
        fn interleave_a422_10le() {
            init();
            check_interleave(VideoFormat::A42210le, PLANAR);
        }

        #[test]
        fn interleave_a444_10be() {
            init();
            check_interleave(VideoFormat::A44410be, PLANAR);
        }

        #[test]
        fn interleave_a444_10le() {
            init();
            check_interleave(VideoFormat::A44410le, PLANAR);
        }

        #[test]
        fn interleave_nv61() {
            init();
            check_interleave(VideoFormat::Nv61, SEMI_PLANAR);
        }

        #[test]
        fn interleave_p010_10be() {
            init();
            check_interleave(VideoFormat::P01010be, SEMI_PLANAR);
        }

        #[test]
        fn interleave_p010_10le() {
            init();
            check_interleave(VideoFormat::P01010le, SEMI_PLANAR);
        }

        #[test]
        fn interleave_iyu2() {
            init();
            check_interleave(VideoFormat::Iyu2, PACKED);
        }

        #[test]
        fn interleave_vyuy() {
            init();
            check_interleave(VideoFormat::Vyuy, MULTI_Y);
        }

        #[test]
        fn interleave_gbra() {
            init();
            check_interleave(VideoFormat::Gbra, PLANAR);
        }

        #[test]
        fn interleave_gbra_10be() {
            init();
            check_interleave(VideoFormat::Gbra10be, PLANAR);
        }

        #[test]
        fn interleave_gbra_10le() {
            init();
            check_interleave(VideoFormat::Gbra10le, PLANAR);
        }

        #[test]
        fn interleave_gbr_12be() {
            init();
            check_interleave(VideoFormat::Gbr12be, PLANAR);
        }

        #[test]
        fn interleave_gbr_12le() {
            init();
            check_interleave(VideoFormat::Gbr12le, PLANAR);
        }

        #[test]
        fn interleave_gbra_12be() {
            init();
            check_interleave(VideoFormat::Gbra12be, PLANAR);
        }

        #[test]
        fn interleave_gbra_12le() {
            init();
            check_interleave(VideoFormat::Gbra12le, PLANAR);
        }

        #[test]
        fn interleave_i420_12be() {
            init();
            check_interleave(VideoFormat::I42012be, PLANAR);
        }

        #[test]
        fn interleave_i420_12le() {
            init();
            check_interleave(VideoFormat::I42012le, PLANAR);
        }

        #[test]
        fn interleave_i422_12be() {
            init();
            check_interleave(VideoFormat::I42212be, PLANAR);
        }

        #[test]
        fn interleave_i422_12le() {
            init();
            check_interleave(VideoFormat::I42212le, PLANAR);
        }

        #[test]
        fn interleave_y444_12be() {
            init();
            check_interleave(VideoFormat::Y44412be, PLANAR);
        }

        #[test]
        fn interleave_y444_12le() {
            init();
            check_interleave(VideoFormat::Y44412le, PLANAR);
        }

        // TODO: GRAY10_LE32

        #[test]
        fn interleave_nv12_10le32() {
            init();
            check_interleave(VideoFormat::Nv1210le32, SEMI_PLANAR);
        }

        #[test]
        fn interleave_nv16_10le32() {
            init();
            check_interleave(VideoFormat::Nv1610le32, SEMI_PLANAR);
        }

        #[test]
        fn interleave_nv12_10le40() {
            init();
            check_interleave(VideoFormat::Nv1210le40, SEMI_PLANAR);
        }

        #[test]
        fn interleave_y210() {
            init();
            check_interleave(VideoFormat::Y210, PACKED);
        }

        #[test]
        fn interleave_y410() {
            init();
            check_interleave(VideoFormat::Y410, PACKED);
        }

        #[test]
        fn interleave_vuya() {
            init();
            check_interleave(VideoFormat::Vuya, PACKED);
        }

        #[test]
        fn interleave_bgra10a2_le() {
            init();
            check_interleave(VideoFormat::Bgr10a2Le, PACKED);
        }

        #[test]
        fn interleave_rgb10a2_le() {
            init();
            check_interleave(VideoFormat::Rgb10a2Le, PACKED);
        }

        #[test]
        fn interleave_y444_16be() {
            init();
            check_interleave(VideoFormat::Y44416be, PLANAR);
        }

        #[test]
        fn interleave_y444_16le() {
            init();
            check_interleave(VideoFormat::Y44416le, PLANAR);
        }

        #[test]
        fn interleave_p016be() {
            init();
            check_interleave(VideoFormat::P016Be, SEMI_PLANAR);
        }

        #[test]
        fn interleave_p016le() {
            init();
            check_interleave(VideoFormat::P016Le, SEMI_PLANAR);
        }

        #[test]
        fn interleave_p012be() {
            init();
            check_interleave(VideoFormat::P012Be, SEMI_PLANAR);
        }

        #[test]
        fn interleave_p012le() {
            init();
            check_interleave(VideoFormat::P012Le, SEMI_PLANAR);
        }

        #[test]
        fn interleave_y212be() {
            init();
            check_interleave(VideoFormat::Y212Be, PACKED);
        }

        #[test]
        fn interleave_y212le() {
            init();
            check_interleave(VideoFormat::Y212Le, PACKED);
        }

        #[test]
        fn interleave_y412be() {
            init();
            check_interleave(VideoFormat::Y412Be, PACKED);
        }

        #[test]
        fn interleave_y412le() {
            init();
            check_interleave(VideoFormat::Y412Le, PACKED);
        }

        #[test]
        fn interleave_nv12_4l4() {
            init();
            check_interleave(VideoFormat::Nv124l4, SEMI_PLANAR);
        }

        #[test]
        fn interleave_nv12_32l32() {
            init();
            check_interleave(VideoFormat::Nv1232l32, SEMI_PLANAR);
        }

        #[test]
        fn interleave_rgbp() {
            init();
            check_interleave(VideoFormat::Rgbp, PLANAR);
        }

        #[test]
        fn interleave_bgrp() {
            init();
            check_interleave(VideoFormat::Bgrp, PLANAR);
        }

        #[test]
        fn interleave_av12() {
            init();
            check_interleave(VideoFormat::Av12, SEMI_PLANAR);
        }

        #[test]
        fn interleave_argb64_le() {
            init();
            check_interleave(VideoFormat::Argb64Le, PACKED);
        }

        #[test]
        fn interleave_argb64_be() {
            init();
            check_interleave(VideoFormat::Argb64Be, PACKED);
        }

        #[test]
        fn interleave_rgba64_le() {
            init();
            check_interleave(VideoFormat::Rgba64Le, PACKED);
        }

        #[test]
        fn interleave_rgba64_be() {
            init();
            check_interleave(VideoFormat::Rgba64Be, PACKED);
        }

        #[test]
        fn interleave_bgra64_le() {
            init();
            check_interleave(VideoFormat::Bgra64Le, PACKED);
        }

        #[test]
        fn interleave_bgra64_be() {
            init();
            check_interleave(VideoFormat::Bgra64Be, PACKED);
        }

        #[test]
        fn interleave_abgr64_le() {
            init();
            check_interleave(VideoFormat::Abgr64Le, PACKED);
        }

        #[test]
        fn interleave_abgr64_be() {
            init();
            check_interleave(VideoFormat::Abgr64Be, PACKED);
        }

        // TODO: NV12_16L32S (110) and after - requires gst-video 1.22
    }

    mod subsampling {
        use super::*;
        use crate::isobmff::uncompressed::get_sampling_type_for_uncc_format;
        use gst_video::VideoFormat;

        const NONE: u8 = 0; // aka 4:4:4
        const Y422: u8 = 1; // 4:2:2
        const Y420: u8 = 2; // 4:2:0
        const Y411: u8 = 3; // 4:1:1

        fn check_subsampling(format: VideoFormat, expected_subsampling: u8) {
            let video_info = gst_video::VideoInfo::builder(format, 320, 240).build();
            assert!(video_info.is_ok());
            let sampling_type = get_sampling_type_for_uncc_format(&video_info.unwrap());
            assert_eq!(sampling_type, expected_subsampling);
        }

        #[test]
        fn subsampling_i420() {
            init();
            check_subsampling(VideoFormat::I420, Y420);
        }

        #[test]
        fn subsampling_yv12() {
            init();
            check_subsampling(VideoFormat::Yv12, Y420);
        }

        #[test]
        fn subsampling_yuy2() {
            init();
            check_subsampling(VideoFormat::Yuy2, Y422);
        }

        #[test]
        fn subsampling_uyvy() {
            init();
            check_subsampling(VideoFormat::Uyvy, Y422);
        }

        #[test]
        fn subsampling_ayuv() {
            init();
            check_subsampling(VideoFormat::Ayuv, NONE);
        }

        #[test]
        fn subsampling_rgbx() {
            init();
            check_subsampling(VideoFormat::Rgbx, NONE);
        }

        #[test]
        fn subsampling_bgrx() {
            init();
            check_subsampling(VideoFormat::Bgrx, NONE);
        }

        #[test]
        fn subsampling_xrgb() {
            init();
            check_subsampling(VideoFormat::Xrgb, NONE);
        }

        #[test]
        fn subsampling_xbgr() {
            init();
            check_subsampling(VideoFormat::Xbgr, NONE);
        }

        #[test]
        fn subsampling_argb() {
            init();
            check_subsampling(VideoFormat::Argb, NONE);
        }

        #[test]
        fn subsampling_abgr() {
            init();
            check_subsampling(VideoFormat::Abgr, NONE);
        }

        #[test]
        fn subsampling_rgb() {
            init();
            check_subsampling(VideoFormat::Rgb, NONE);
        }

        #[test]
        fn subsampling_bgr() {
            init();
            check_subsampling(VideoFormat::Bgr, NONE);
        }

        #[test]
        fn subsampling_y41b() {
            init();
            check_subsampling(VideoFormat::Y41b, Y411);
        }

        #[test]
        fn subsampling_y42b() {
            init();
            check_subsampling(VideoFormat::Y42b, Y422);
        }

        #[test]
        fn subsampling_yvyu() {
            init();
            check_subsampling(VideoFormat::Yvyu, Y422);
        }

        #[test]
        fn subsampling_y444() {
            init();
            check_subsampling(VideoFormat::Y444, NONE);
        }

        #[test]
        fn subsampling_v210() {
            init();
            check_subsampling(VideoFormat::V210, Y422);
        }

        #[test]
        fn subsampling_v216() {
            init();
            check_subsampling(VideoFormat::V216, Y422);
        }

        #[test]
        fn subsampling_nv12() {
            init();
            check_subsampling(VideoFormat::Nv12, Y420);
        }

        #[test]
        fn subsampling_nv21() {
            init();
            check_subsampling(VideoFormat::Nv21, Y420);
        }

        #[test]
        fn subsampling_gray8() {
            init();
            check_subsampling(VideoFormat::Gray8, NONE);
        }

        #[test]
        fn subsampling_gray16be() {
            init();
            check_subsampling(VideoFormat::Gray16Be, NONE);
        }

        #[test]
        fn subsampling_gray16le() {
            init();
            check_subsampling(VideoFormat::Gray16Le, NONE);
        }

        #[test]
        fn subsampling_v308() {
            init();
            check_subsampling(VideoFormat::V308, NONE);
        }

        #[test]
        fn subsampling_rgb16() {
            init();
            check_subsampling(VideoFormat::Rgb16, NONE);
        }

        #[test]
        fn subsampling_bgr16() {
            init();
            check_subsampling(VideoFormat::Bgr16, NONE);
        }

        #[test]
        fn subsampling_rgb15() {
            init();
            check_subsampling(VideoFormat::Rgb15, NONE);
        }

        #[test]
        fn subsampling_bgr15() {
            init();
            check_subsampling(VideoFormat::Bgr15, NONE);
        }

        #[test]
        fn subsampling_uyvp() {
            init();
            check_subsampling(VideoFormat::Uyvp, Y422);
        }

        #[test]
        fn subsampling_a420() {
            init();
            check_subsampling(VideoFormat::A420, Y420);
        }

        // TODO: RGB8P (35)

        #[test]
        fn subsampling_yuv9() {
            init();
            check_subsampling(VideoFormat::Yuv9, Y411);
        }

        #[test]
        fn subsampling_yvu9() {
            init();
            check_subsampling(VideoFormat::Yvu9, Y411);
        }

        #[test]
        fn subsampling_iyu1() {
            init();
            check_subsampling(VideoFormat::Iyu1, Y411);
        }

        #[test]
        fn subsampling_argb64() {
            init();
            check_subsampling(VideoFormat::Argb64, NONE);
        }

        #[test]
        fn subsampling_ayuv64() {
            init();
            check_subsampling(VideoFormat::Ayuv64, NONE);
        }

        #[test]
        fn subsampling_r210() {
            init();
            check_subsampling(VideoFormat::R210, NONE);
        }

        #[test]
        fn subsampling_i420_10be() {
            init();
            check_subsampling(VideoFormat::I42010be, Y420);
        }

        #[test]
        fn subsampling_i420_10le() {
            init();
            check_subsampling(VideoFormat::I42010le, Y420);
        }

        #[test]
        fn subsampling_i422_10be() {
            init();
            check_subsampling(VideoFormat::I42210be, Y422);
        }

        #[test]
        fn subsampling_i422_10le() {
            init();
            check_subsampling(VideoFormat::I42210le, Y422);
        }

        #[test]
        fn subsampling_y444_10be() {
            init();
            check_subsampling(VideoFormat::Y44410be, NONE);
        }

        #[test]
        fn subsampling_y444_10le() {
            init();
            check_subsampling(VideoFormat::Y44410le, NONE);
        }

        #[test]
        fn subsampling_gbr() {
            init();
            check_subsampling(VideoFormat::Gbr, NONE);
        }

        #[test]
        fn subsampling_gbr_10be() {
            init();
            check_subsampling(VideoFormat::Gbr10be, NONE);
        }

        #[test]
        fn subsampling_gbr_10le() {
            init();
            check_subsampling(VideoFormat::Gbr10le, NONE);
        }

        #[test]
        fn subsampling_nv16() {
            init();
            check_subsampling(VideoFormat::Nv16, Y422);
        }

        #[test]
        fn subsampling_nv24() {
            init();
            check_subsampling(VideoFormat::Nv24, NONE);
        }

        #[test]
        fn subsampling_nv12_64z32() {
            init();
            check_subsampling(VideoFormat::Nv1264z32, Y420);
        }

        #[test]
        fn subsampling_a420_10be() {
            init();
            check_subsampling(VideoFormat::A42010be, Y420);
        }

        #[test]
        fn subsampling_a420_10le() {
            init();
            check_subsampling(VideoFormat::A42010le, Y420);
        }

        #[test]
        fn subsampling_a422_10be() {
            init();
            check_subsampling(VideoFormat::A42210be, Y422);
        }

        #[test]
        fn subsampling_a422_10le() {
            init();
            check_subsampling(VideoFormat::A42210le, Y422);
        }

        #[test]
        fn subsampling_a444_10be() {
            init();
            check_subsampling(VideoFormat::A44410be, NONE);
        }

        #[test]
        fn subsampling_a444_10le() {
            init();
            check_subsampling(VideoFormat::A44410le, NONE);
        }

        #[test]
        fn subsampling_nv61() {
            init();
            check_subsampling(VideoFormat::Nv61, Y422);
        }

        #[test]
        fn subsampling_p010_10be() {
            init();
            check_subsampling(VideoFormat::P01010be, Y420);
        }

        #[test]
        fn subsampling_p010_10le() {
            init();
            check_subsampling(VideoFormat::P01010le, Y420);
        }

        #[test]
        fn subsampling_iyu2() {
            init();
            check_subsampling(VideoFormat::Iyu2, NONE);
        }

        #[test]
        fn subsampling_vyuy() {
            init();
            check_subsampling(VideoFormat::Vyuy, Y422);
        }

        #[test]
        fn subsampling_gbra() {
            init();
            check_subsampling(VideoFormat::Gbra, NONE);
        }

        #[test]
        fn subsampling_gbra_10be() {
            init();
            check_subsampling(VideoFormat::Gbra10be, NONE);
        }

        #[test]
        fn subsampling_gbra_10le() {
            init();
            check_subsampling(VideoFormat::Gbra10le, NONE);
        }

        #[test]
        fn subsampling_gbr_12be() {
            init();
            check_subsampling(VideoFormat::Gbr12be, NONE);
        }

        #[test]
        fn subsampling_gbr_12le() {
            init();
            check_subsampling(VideoFormat::Gbr12le, NONE);
        }

        #[test]
        fn subsampling_gbra_12be() {
            init();
            check_subsampling(VideoFormat::Gbra12be, NONE);
        }

        #[test]
        fn subsampling_gbra_12le() {
            init();
            check_subsampling(VideoFormat::Gbra12le, NONE);
        }

        #[test]
        fn subsampling_i420_12be() {
            init();
            check_subsampling(VideoFormat::I42012be, Y420);
        }

        #[test]
        fn subsampling_i420_12le() {
            init();
            check_subsampling(VideoFormat::I42012le, Y420);
        }

        #[test]
        fn subsampling_i422_12be() {
            init();
            check_subsampling(VideoFormat::I42212be, Y422);
        }

        #[test]
        fn subsampling_i422_12le() {
            init();
            check_subsampling(VideoFormat::I42212le, Y422);
        }

        #[test]
        fn subsampling_y444_12be() {
            init();
            check_subsampling(VideoFormat::Y44412be, NONE);
        }

        #[test]
        fn subsampling_y444_12le() {
            init();
            check_subsampling(VideoFormat::Y44412le, NONE);
        }

        // TODO: GRAY10_LE32

        #[test]
        fn subsampling_nv12_10le32() {
            init();
            check_subsampling(VideoFormat::Nv1210le32, Y420);
        }

        #[test]
        fn subsampling_nv16_10le32() {
            init();
            check_subsampling(VideoFormat::Nv1610le32, Y422);
        }

        #[test]
        fn subsampling_nv12_10le40() {
            init();
            check_subsampling(VideoFormat::Nv1210le40, Y420);
        }

        #[test]
        fn subsampling_y210() {
            init();
            check_subsampling(VideoFormat::Y210, Y422);
        }

        #[test]
        fn subsampling_y410() {
            init();
            check_subsampling(VideoFormat::Y410, NONE);
        }

        #[test]
        fn subsampling_vuya() {
            init();
            check_subsampling(VideoFormat::Vuya, NONE);
        }

        #[test]
        fn subsampling_bgra10a2_le() {
            init();
            check_subsampling(VideoFormat::Bgr10a2Le, NONE);
        }

        #[test]
        fn subsampling_rgb10a2_le() {
            init();
            check_subsampling(VideoFormat::Rgb10a2Le, NONE);
        }

        #[test]
        fn subsampling_y444_16be() {
            init();
            check_subsampling(VideoFormat::Y44416be, NONE);
        }

        #[test]
        fn subsampling_y444_16le() {
            init();
            check_subsampling(VideoFormat::Y44416le, NONE);
        }

        #[test]
        fn subsampling_p016be() {
            init();
            check_subsampling(VideoFormat::P016Be, Y420);
        }

        #[test]
        fn subsampling_p016le() {
            init();
            check_subsampling(VideoFormat::P016Le, Y420);
        }

        #[test]
        fn subsampling_p012be() {
            init();
            check_subsampling(VideoFormat::P012Be, Y420);
        }

        #[test]
        fn subsampling_p012le() {
            init();
            check_subsampling(VideoFormat::P012Le, Y420);
        }

        #[test]
        fn subsampling_y212be() {
            init();
            check_subsampling(VideoFormat::Y212Be, Y422);
        }

        #[test]
        fn subsampling_y212le() {
            init();
            check_subsampling(VideoFormat::Y212Le, Y422);
        }

        #[test]
        fn subsampling_y412be() {
            init();
            check_subsampling(VideoFormat::Y412Be, NONE);
        }

        #[test]
        fn subsampling_y412le() {
            init();
            check_subsampling(VideoFormat::Y412Le, NONE);
        }

        #[test]
        fn subsampling_nv12_4l4() {
            init();
            check_subsampling(VideoFormat::Nv124l4, Y420);
        }

        #[test]
        fn subsampling_nv12_32l32() {
            init();
            check_subsampling(VideoFormat::Nv1232l32, Y420);
        }

        #[test]
        fn subsampling_rgbp() {
            init();
            check_subsampling(VideoFormat::Rgbp, NONE);
        }

        #[test]
        fn subsampling_bgrp() {
            init();
            check_subsampling(VideoFormat::Bgrp, NONE);
        }

        #[test]
        fn subsampling_av12() {
            init();
            check_subsampling(VideoFormat::Av12, Y420);
        }

        #[test]
        fn subsampling_argb64_le() {
            init();
            check_subsampling(VideoFormat::Argb64Le, NONE);
        }

        #[test]
        fn subsampling_argb64_be() {
            init();
            check_subsampling(VideoFormat::Argb64Be, NONE);
        }

        #[test]
        fn subsampling_rgba64_le() {
            init();
            check_subsampling(VideoFormat::Rgba64Le, NONE);
        }

        #[test]
        fn subsampling_rgba64_be() {
            init();
            check_subsampling(VideoFormat::Rgba64Be, NONE);
        }

        #[test]
        fn subsampling_bgra64_le() {
            init();
            check_subsampling(VideoFormat::Bgra64Le, NONE);
        }

        #[test]
        fn subsampling_bgra64_be() {
            init();
            check_subsampling(VideoFormat::Bgra64Be, NONE);
        }

        #[test]
        fn subsampling_abgr64_le() {
            init();
            check_subsampling(VideoFormat::Abgr64Le, NONE);
        }

        #[test]
        fn subsampling_abgr64_be() {
            init();
            check_subsampling(VideoFormat::Abgr64Be, NONE);
        }

        // TODO: NV12_16L32S (110) and after - requires gst-video 1.22
    }

    mod pixelsize {
        use super::*;
        use crate::isobmff::uncompressed::get_pixel_size_for_uncc_format;
        use gst_video::VideoFormat;

        fn check_pixel_size(format: VideoFormat, expected_size: u32) {
            let video_info = gst_video::VideoInfo::builder(format, 320, 240).build();
            assert!(video_info.is_ok());
            let pixel_size = get_pixel_size_for_uncc_format(&video_info.unwrap());
            assert_eq!(pixel_size, expected_size);
        }

        #[test]
        fn pixel_size_i420() {
            init();
            check_pixel_size(VideoFormat::I420, 0);
        }

        #[test]
        fn pixel_size_yv12() {
            init();
            check_pixel_size(VideoFormat::Yv12, 0);
        }

        #[test]
        fn pixel_size_yuy2() {
            init();
            check_pixel_size(VideoFormat::Yuy2, 4);
        }

        #[test]
        fn pixel_size_uyvy() {
            init();
            check_pixel_size(VideoFormat::Uyvy, 4);
        }

        #[test]
        fn pixel_size_ayuv() {
            init();
            check_pixel_size(VideoFormat::Ayuv, 4);
        }

        #[test]
        fn pixel_size_rgbx() {
            init();
            check_pixel_size(VideoFormat::Rgbx, 4);
        }

        #[test]
        fn pixel_size_bgrx() {
            init();
            check_pixel_size(VideoFormat::Bgrx, 4);
        }

        #[test]
        fn pixel_size_xrgb() {
            init();
            check_pixel_size(VideoFormat::Xrgb, 4);
        }

        #[test]
        fn pixel_size_xbgr() {
            init();
            check_pixel_size(VideoFormat::Xbgr, 4);
        }

        #[test]
        fn pixel_size_argb() {
            init();
            check_pixel_size(VideoFormat::Argb, 4);
        }

        #[test]
        fn pixel_size_abgr() {
            init();
            check_pixel_size(VideoFormat::Abgr, 4);
        }

        #[test]
        fn pixel_size_rgb() {
            init();
            check_pixel_size(VideoFormat::Rgb, 3);
        }

        #[test]
        fn pixel_size_bgr() {
            init();
            check_pixel_size(VideoFormat::Bgr, 3);
        }

        #[test]
        fn pixel_size_y41b() {
            init();
            check_pixel_size(VideoFormat::Y41b, 0);
        }

        #[test]
        fn pixel_size_y42b() {
            init();
            check_pixel_size(VideoFormat::Y42b, 0);
        }

        #[test]
        fn pixel_size_yvyu() {
            init();
            check_pixel_size(VideoFormat::Yvyu, 4);
        }

        #[test]
        fn pixel_size_y444() {
            init();
            check_pixel_size(VideoFormat::Y444, 0);
        }

        #[test]
        fn pixel_size_v210() {
            init();
            check_pixel_size(VideoFormat::V210, 0);
        }

        #[test]
        fn pixel_size_v216() {
            init();
            check_pixel_size(VideoFormat::V216, 4);
        }

        #[test]
        fn pixel_size_nv12() {
            init();
            check_pixel_size(VideoFormat::Nv12, 0);
        }

        #[test]
        fn pixel_size_nv21() {
            init();
            check_pixel_size(VideoFormat::Nv21, 0);
        }

        #[test]
        fn pixel_size_gray8() {
            init();
            check_pixel_size(VideoFormat::Gray8, 0);
        }

        #[test]
        fn pixel_size_gray16be() {
            init();
            check_pixel_size(VideoFormat::Gray16Be, 0);
        }

        #[test]
        fn pixel_size_gray16le() {
            init();
            check_pixel_size(VideoFormat::Gray16Le, 0);
        }

        #[test]
        fn pixel_size_v308() {
            init();
            check_pixel_size(VideoFormat::V308, 3);
        }

        #[test]
        fn pixel_size_rgb16() {
            init();
            check_pixel_size(VideoFormat::Rgb16, 2);
        }

        #[test]
        fn pixel_size_bgr16() {
            init();
            check_pixel_size(VideoFormat::Bgr16, 2);
        }

        #[test]
        fn pixel_size_rgb15() {
            init();
            check_pixel_size(VideoFormat::Rgb15, 2);
        }

        #[test]
        fn pixel_size_bgr15() {
            init();
            check_pixel_size(VideoFormat::Bgr15, 2);
        }

        #[test]
        fn pixel_size_uyvp() {
            init();
            // TODO: this might need more work
            check_pixel_size(VideoFormat::Uyvp, 0);
        }

        #[test]
        fn pixel_size_a420() {
            init();
            check_pixel_size(VideoFormat::A420, 0);
        }

        // TODO: RGB8P (35)

        #[test]
        fn pixel_size_yuv9() {
            init();
            check_pixel_size(VideoFormat::Yuv9, 0);
        }

        #[test]
        fn pixel_size_yvu9() {
            init();
            check_pixel_size(VideoFormat::Yvu9, 0);
        }

        #[test]
        fn pixel_size_iyu1() {
            init();
            check_pixel_size(VideoFormat::Iyu1, 0);
        }

        #[test]
        fn pixel_size_argb64() {
            init();
            check_pixel_size(VideoFormat::Argb64, 8);
        }

        #[test]
        fn pixel_size_ayuv64() {
            init();
            check_pixel_size(VideoFormat::Ayuv64, 8);
        }

        #[test]
        fn pixel_size_r210() {
            init();
            check_pixel_size(VideoFormat::R210, 4);
        }

        #[test]
        fn pixel_size_i420_10be() {
            init();
            check_pixel_size(VideoFormat::I42010be, 0);
        }

        #[test]
        fn pixel_size_i420_10le() {
            init();
            check_pixel_size(VideoFormat::I42010le, 0);
        }

        #[test]
        fn pixel_size_i422_10be() {
            init();
            check_pixel_size(VideoFormat::I42210be, 0);
        }

        #[test]
        fn pixel_size_i422_10le() {
            init();
            check_pixel_size(VideoFormat::I42210le, 0);
        }

        #[test]
        fn pixel_size_y444_10be() {
            init();
            check_pixel_size(VideoFormat::Y44410be, 0);
        }

        #[test]
        fn pixel_size_y444_10le() {
            init();
            check_pixel_size(VideoFormat::Y44410le, 0);
        }

        #[test]
        fn pixel_size_gbr() {
            init();
            check_pixel_size(VideoFormat::Gbr, 0);
        }

        #[test]
        fn pixel_size_gbr_10be() {
            init();
            check_pixel_size(VideoFormat::Gbr10be, 0);
        }

        #[test]
        fn pixel_size_gbr_10le() {
            init();
            check_pixel_size(VideoFormat::Gbr10le, 0);
        }

        #[test]
        fn pixel_size_nv16() {
            init();
            check_pixel_size(VideoFormat::Nv16, 0);
        }

        #[test]
        fn pixel_size_nv24() {
            init();
            check_pixel_size(VideoFormat::Nv24, 0);
        }

        #[test]
        fn pixel_size_nv12_64z32() {
            init();
            check_pixel_size(VideoFormat::Nv1264z32, 0);
        }

        #[test]
        fn pixel_size_a420_10be() {
            init();
            check_pixel_size(VideoFormat::A42010be, 0);
        }

        #[test]
        fn pixel_size_a420_10le() {
            init();
            check_pixel_size(VideoFormat::A42010le, 0);
        }

        #[test]
        fn pixel_size_a422_10be() {
            init();
            check_pixel_size(VideoFormat::A42210be, 0);
        }

        #[test]
        fn pixel_size_a422_10le() {
            init();
            check_pixel_size(VideoFormat::A42210le, 0);
        }

        #[test]
        fn pixel_size_a444_10be() {
            init();
            check_pixel_size(VideoFormat::A44410be, 0);
        }

        #[test]
        fn pixel_size_a444_10le() {
            init();
            check_pixel_size(VideoFormat::A44410le, 0);
        }

        #[test]
        fn pixel_size_nv61() {
            init();
            check_pixel_size(VideoFormat::Nv61, 0);
        }

        #[test]
        fn pixel_size_p010_10be() {
            init();
            check_pixel_size(VideoFormat::P01010be, 0);
        }

        #[test]
        fn pixel_size_p010_10le() {
            init();
            check_pixel_size(VideoFormat::P01010le, 0);
        }

        #[test]
        fn pixel_size_iyu2() {
            init();
            check_pixel_size(VideoFormat::Iyu2, 3);
        }

        #[test]
        fn pixel_size_vyuy() {
            init();
            check_pixel_size(VideoFormat::Vyuy, 4);
        }

        #[test]
        fn pixel_size_gbra() {
            init();
            check_pixel_size(VideoFormat::Gbra, 0);
        }

        #[test]
        fn pixel_size_gbra_10be() {
            init();
            check_pixel_size(VideoFormat::Gbra10be, 0);
        }

        #[test]
        fn pixel_size_gbra_10le() {
            init();
            check_pixel_size(VideoFormat::Gbra10le, 0);
        }

        #[test]
        fn pixel_size_gbr_12be() {
            init();
            check_pixel_size(VideoFormat::Gbr12be, 0);
        }

        #[test]
        fn pixel_size_gbr_12le() {
            init();
            check_pixel_size(VideoFormat::Gbr12le, 0);
        }

        #[test]
        fn pixel_size_gbra_12be() {
            init();
            check_pixel_size(VideoFormat::Gbra12be, 0);
        }

        #[test]
        fn pixel_size_gbra_12le() {
            init();
            check_pixel_size(VideoFormat::Gbra12le, 0);
        }

        #[test]
        fn pixel_size_i420_12be() {
            init();
            check_pixel_size(VideoFormat::I42012be, 0);
        }

        #[test]
        fn pixel_size_i420_12le() {
            init();
            check_pixel_size(VideoFormat::I42012le, 0);
        }

        #[test]
        fn pixel_size_i422_12be() {
            init();
            check_pixel_size(VideoFormat::I42212be, 0);
        }

        #[test]
        fn pixel_size_i422_12le() {
            init();
            check_pixel_size(VideoFormat::I42212le, 0);
        }

        #[test]
        fn pixel_size_y444_12be() {
            init();
            check_pixel_size(VideoFormat::Y44412be, 0);
        }

        #[test]
        fn pixel_size_y444_12le() {
            init();
            check_pixel_size(VideoFormat::Y44412le, 0);
        }

        // TODO: GRAY10_LE32

        #[test]
        fn pixel_size_nv12_10le32() {
            init();
            check_pixel_size(VideoFormat::Nv1210le32, 0);
        }

        #[test]
        fn pixel_size_nv16_10le32() {
            init();
            check_pixel_size(VideoFormat::Nv1610le32, 0);
        }

        #[test]
        fn pixel_size_nv12_10le40() {
            init();
            check_pixel_size(VideoFormat::Nv1210le40, 0);
        }

        #[test]
        fn pixel_size_y210() {
            init();
            check_pixel_size(VideoFormat::Y210, 4);
        }

        #[test]
        fn pixel_size_y410() {
            init();
            check_pixel_size(VideoFormat::Y410, 4);
        }

        #[test]
        fn pixel_size_vuya() {
            init();
            check_pixel_size(VideoFormat::Vuya, 4);
        }

        #[test]
        fn pixel_size_bgra10a2_le() {
            init();
            check_pixel_size(VideoFormat::Bgr10a2Le, 4);
        }

        #[test]
        fn pixel_size_rgb10a2_le() {
            init();
            check_pixel_size(VideoFormat::Rgb10a2Le, 4);
        }

        #[test]
        fn pixel_size_y444_16be() {
            init();
            check_pixel_size(VideoFormat::Y44416be, 0);
        }

        #[test]
        fn pixel_size_y444_16le() {
            init();
            check_pixel_size(VideoFormat::Y44416le, 0);
        }

        #[test]
        fn pixel_size_p016be() {
            init();
            check_pixel_size(VideoFormat::P016Be, 0);
        }

        #[test]
        fn pixel_size_p016le() {
            init();
            check_pixel_size(VideoFormat::P016Le, 0);
        }

        #[test]
        fn pixel_size_p012be() {
            init();
            check_pixel_size(VideoFormat::P012Be, 0);
        }

        #[test]
        fn pixel_size_p012le() {
            init();
            check_pixel_size(VideoFormat::P012Le, 0);
        }

        #[test]
        fn pixel_size_y212be() {
            init();
            check_pixel_size(VideoFormat::Y212Be, 4);
        }

        #[test]
        fn pixel_size_y212le() {
            init();
            check_pixel_size(VideoFormat::Y212Le, 4);
        }

        #[test]
        fn pixel_size_y412be() {
            init();
            // TODO: check how this is actually packed
            check_pixel_size(VideoFormat::Y412Be, 8);
        }

        #[test]
        fn pixel_size_y412le() {
            init();
            // TODO: check how this is actually packed
            check_pixel_size(VideoFormat::Y412Le, 8);
        }

        #[test]
        fn pixel_size_nv12_4l4() {
            init();
            check_pixel_size(VideoFormat::Nv124l4, 0);
        }

        #[test]
        fn pixel_size_nv12_32l32() {
            init();
            check_pixel_size(VideoFormat::Nv1232l32, 0);
        }

        #[test]
        fn pixel_size_rgbp() {
            init();
            check_pixel_size(VideoFormat::Rgbp, 0);
        }

        #[test]
        fn pixel_size_bgrp() {
            init();
            check_pixel_size(VideoFormat::Bgrp, 0);
        }

        #[test]
        fn pixel_size_av12() {
            init();
            check_pixel_size(VideoFormat::Av12, 0);
        }

        #[test]
        fn pixel_size_argb64_le() {
            init();
            check_pixel_size(VideoFormat::Argb64Le, 8);
        }

        #[test]
        fn pixel_size_argb64_be() {
            init();
            check_pixel_size(VideoFormat::Argb64Be, 8);
        }

        #[test]
        fn pixel_size_rgba64_le() {
            init();
            check_pixel_size(VideoFormat::Rgba64Le, 8);
        }

        #[test]
        fn pixel_size_rgba64_be() {
            init();
            check_pixel_size(VideoFormat::Rgba64Be, 8);
        }

        #[test]
        fn pixel_size_bgra64_le() {
            init();
            check_pixel_size(VideoFormat::Bgra64Le, 8);
        }

        #[test]
        fn pixel_size_bgra64_be() {
            init();
            check_pixel_size(VideoFormat::Bgra64Be, 8);
        }

        #[test]
        fn pixel_size_abgr64_le() {
            init();
            check_pixel_size(VideoFormat::Abgr64Le, 8);
        }

        #[test]
        fn pixel_size_abgr64_be() {
            init();
            check_pixel_size(VideoFormat::Abgr64Be, 8);
        }

        // TODO: NV12_16L32S (110) and after - requires gst-video 1.22
    }

    mod profile_4cc {
        use super::*;
        use crate::isobmff::uncompressed::get_profile_for_uncc_format;
        use gst_video::VideoFormat;

        fn check_profile(video_format: VideoFormat, fourcc: &[u8]) {
            assert_eq!(fourcc.len(), 4);
            let video_info = gst_video::VideoInfo::builder(video_format, 320, 240).build();
            assert!(video_info.is_ok());
            let format = video_info.unwrap();
            let pixel_size = get_profile_for_uncc_format(&format);
            assert_eq!(pixel_size, fourcc);
        }

        #[test]
        fn profile_4cc_i420() {
            init();
            check_profile(VideoFormat::I420, "i420".as_bytes());
        }

        #[test]
        fn profile_4cc_yv12() {
            init();
            // YUV 420 8 bits planar YCrCb
            check_profile(VideoFormat::Yv12, "yv20".as_bytes());
        }

        #[test]
        fn profile_4cc_yuy2() {
            init();
            // 8 bits YUV 422 packed Y0 Cb Y1 Cr
            check_profile(VideoFormat::Yuy2, "yuv2".as_bytes());
        }

        #[test]
        fn profile_4cc_uyvy() {
            init();
            // 8 bits YUV 422 packed Cb Y0 Cr Y1
            check_profile(VideoFormat::Uyvy, "2vuy".as_bytes());
        }

        #[test]
        fn profile_4cc_ayuv() {
            init();
            check_profile(VideoFormat::Ayuv, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_rgbx() {
            init();
            check_profile(VideoFormat::Rgbx, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_bgrx() {
            init();
            check_profile(VideoFormat::Bgrx, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_xrgb() {
            init();
            check_profile(VideoFormat::Xrgb, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_xbgr() {
            init();
            check_profile(VideoFormat::Xbgr, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_argb() {
            init();
            check_profile(VideoFormat::Argb, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_abgr() {
            init();
            check_profile(VideoFormat::Abgr, "abgr".as_bytes());
        }

        #[test]
        fn profile_4cc_rgb() {
            init();
            check_profile(VideoFormat::Rgb, "rgb3".as_bytes());
        }

        #[test]
        fn profile_4cc_bgr() {
            init();
            check_profile(VideoFormat::Bgr, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_y41b() {
            init();
            check_profile(VideoFormat::Y41b, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_y42b() {
            init();
            // YUV 422 8 bits planar YCbCr
            check_profile(VideoFormat::Y42b, "yu22".as_bytes());
        }

        #[test]
        fn profile_4cc_yvyu() {
            init();
            check_profile(VideoFormat::Yvyu, "yvyu".as_bytes());
        }

        #[test]
        fn profile_4cc_y444() {
            init();
            check_profile(VideoFormat::Y444, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_v210() {
            init();
            check_profile(VideoFormat::V210, "v210".as_bytes());
        }

        #[test]
        fn profile_4cc_v216() {
            init();
            check_profile(VideoFormat::V216, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_nv12() {
            init();
            check_profile(VideoFormat::Nv12, "nv12".as_bytes());
        }

        #[test]
        fn profile_4cc_nv21() {
            init();
            check_profile(VideoFormat::Nv21, "nv21".as_bytes());
        }

        #[test]
        fn profile_4cc_gray8() {
            init();
            check_profile(VideoFormat::Gray8, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_gray16be() {
            init();
            check_profile(VideoFormat::Gray16Be, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_gray16le() {
            init();
            check_profile(VideoFormat::Gray16Le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_v308() {
            init();
            check_profile(VideoFormat::V308, "v308".as_bytes());
        }

        #[test]
        fn profile_4cc_rgb16() {
            init();
            check_profile(VideoFormat::Rgb16, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_bgr16() {
            init();
            check_profile(VideoFormat::Bgr16, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_rgb15() {
            init();
            check_profile(VideoFormat::Rgb15, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_bgr15() {
            init();
            check_profile(VideoFormat::Bgr15, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_uyvp() {
            init();
            check_profile(VideoFormat::Uyvp, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_a420() {
            init();
            check_profile(VideoFormat::A420, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_rgb8p() {
            init();
            check_profile(VideoFormat::Rgb8p, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_yuv9() {
            init();
            check_profile(VideoFormat::Yuv9, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_yvu9() {
            init();
            check_profile(VideoFormat::Yvu9, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_iyu1() {
            init();
            check_profile(VideoFormat::Iyu1, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_argb64() {
            init();
            check_profile(VideoFormat::Argb64, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_ayuv64() {
            init();
            check_profile(VideoFormat::Ayuv64, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_r210() {
            init();
            check_profile(VideoFormat::R210, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_i420_10be() {
            init();
            check_profile(VideoFormat::I42010be, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_i420_10le() {
            init();
            check_profile(VideoFormat::I42010le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_i422_10be() {
            init();
            check_profile(VideoFormat::I42210be, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_i422_10le() {
            init();
            check_profile(VideoFormat::I42210le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_y444_10be() {
            init();
            check_profile(VideoFormat::Y44410be, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_y444_10le() {
            init();
            check_profile(VideoFormat::Y44410le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_gbr() {
            init();
            check_profile(VideoFormat::Gbr, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_gbr_10be() {
            init();
            check_profile(VideoFormat::Gbr10be, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_gbr_10le() {
            init();
            check_profile(VideoFormat::Gbr10le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_nv16() {
            init();
            check_profile(VideoFormat::Nv16, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_nv24() {
            init();
            check_profile(VideoFormat::Nv24, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_nv12_64z32() {
            init();
            check_profile(VideoFormat::Nv1264z32, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_a420_10be() {
            init();
            check_profile(VideoFormat::A42010be, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_a420_10le() {
            init();
            check_profile(VideoFormat::A42010le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_a422_10be() {
            init();
            check_profile(VideoFormat::A42210be, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_a422_10le() {
            init();
            check_profile(VideoFormat::A42210le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_a444_10be() {
            init();
            check_profile(VideoFormat::A44410be, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_a444_10le() {
            init();
            check_profile(VideoFormat::A44410le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_nv61() {
            init();
            check_profile(VideoFormat::Nv61, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_p010_10be() {
            init();
            check_profile(VideoFormat::P01010be, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_p010_10le() {
            init();
            check_profile(VideoFormat::P01010le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_iyu2() {
            init();
            check_profile(VideoFormat::Iyu2, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_vyuy() {
            init();
            check_profile(VideoFormat::Vyuy, "vyuy".as_bytes());
        }

        #[test]
        fn profile_4cc_gbra() {
            init();
            check_profile(VideoFormat::Gbra, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_gbra_10be() {
            init();
            check_profile(VideoFormat::Gbra10be, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_gbra_10le() {
            init();
            check_profile(VideoFormat::Gbra10le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_gbr_12be() {
            init();
            check_profile(VideoFormat::Gbr12be, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_gbr_12le() {
            init();
            check_profile(VideoFormat::Gbr12le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_gbra_12be() {
            init();
            check_profile(VideoFormat::Gbra12be, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_gbra_12le() {
            init();
            check_profile(VideoFormat::Gbra12le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_i420_12be() {
            init();
            check_profile(VideoFormat::I42012be, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_i420_12le() {
            init();
            check_profile(VideoFormat::I42012le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_i422_12be() {
            init();
            check_profile(VideoFormat::I42212be, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_i422_12le() {
            init();
            check_profile(VideoFormat::I42212le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_y444_12be() {
            init();
            check_profile(VideoFormat::Y44412be, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_y444_12le() {
            init();
            check_profile(VideoFormat::Y44412le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_gray10_le32() {
            init();
            check_profile(VideoFormat::Gray10Le32, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_nv12_10le32() {
            init();
            check_profile(VideoFormat::Nv1210le32, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_nv16_10le32() {
            init();
            check_profile(VideoFormat::Nv1610le32, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_nv12_10le40() {
            init();
            check_profile(VideoFormat::Nv1210le40, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_y210() {
            init();
            check_profile(VideoFormat::Y210, "y210".as_bytes());
        }

        #[test]
        fn profile_4cc_y410() {
            init();
            check_profile(VideoFormat::Y410, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_vuya() {
            init();
            check_profile(VideoFormat::Vuya, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_bgra10a2_le() {
            init();
            check_profile(VideoFormat::Bgr10a2Le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_rgb10a2_le() {
            init();
            check_profile(VideoFormat::Rgb10a2Le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_y444_16be() {
            init();
            check_profile(VideoFormat::Y44416be, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_y444_16le() {
            init();
            check_profile(VideoFormat::Y44416le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_p016be() {
            init();
            check_profile(VideoFormat::P016Be, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_p016le() {
            init();
            check_profile(VideoFormat::P016Le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_p012be() {
            init();
            check_profile(VideoFormat::P012Be, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_p012le() {
            init();
            check_profile(VideoFormat::P012Le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_y212be() {
            init();
            check_profile(VideoFormat::Y212Be, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_y212le() {
            init();
            check_profile(VideoFormat::Y212Le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_y412be() {
            init();
            check_profile(VideoFormat::Y412Be, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_y412le() {
            init();
            check_profile(VideoFormat::Y412Le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_nv12_4l4() {
            init();
            check_profile(VideoFormat::Nv124l4, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_nv12_32l32() {
            init();
            check_profile(VideoFormat::Nv1232l32, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_rgbp() {
            init();
            check_profile(VideoFormat::Rgbp, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_bgrp() {
            init();
            check_profile(VideoFormat::Bgrp, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_av12() {
            init();
            check_profile(VideoFormat::Av12, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_argb64_le() {
            init();
            check_profile(VideoFormat::Argb64Le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_argb64_be() {
            init();
            check_profile(VideoFormat::Argb64Be, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_rgba64_le() {
            init();
            check_profile(VideoFormat::Rgba64Le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_rgba64_be() {
            init();
            check_profile(VideoFormat::Rgba64Be, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_bgra64_le() {
            init();
            check_profile(VideoFormat::Bgra64Le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_bgra64_be() {
            init();
            check_profile(VideoFormat::Bgra64Be, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_abgr64_le() {
            init();
            check_profile(VideoFormat::Abgr64Le, &[0u8, 0u8, 0u8, 0u8]);
        }

        #[test]
        fn profile_4cc_abgr64_be() {
            init();
            check_profile(VideoFormat::Abgr64Be, &[0u8, 0u8, 0u8, 0u8]);
        }

        // TODO: NV12_16L32S (110) and after - requires gst-video 1.22
    }
}
