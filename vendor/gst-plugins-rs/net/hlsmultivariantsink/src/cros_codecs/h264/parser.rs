// Copyright 2022 The ChromiumOS Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::collections::BTreeMap;
use std::io::Cursor;
use std::rc::Rc;

use anyhow::anyhow;
use anyhow::Context;
use bytes::Buf;
use enumn::N;

use crate::cros_codecs::h264::nalu;
use crate::cros_codecs::h264::nalu::Header;
use crate::cros_codecs::h264::nalu_reader::NaluReader;

pub type Nalu<'a> = nalu::Nalu<'a, NaluHeader>;

pub(super) const DEFAULT_4X4_INTRA: [u8; 16] = [
    6, 13, 13, 20, 20, 20, 28, 28, 28, 28, 32, 32, 32, 37, 37, 42,
];

pub(super) const DEFAULT_4X4_INTER: [u8; 16] = [
    10, 14, 14, 20, 20, 20, 24, 24, 24, 24, 27, 27, 27, 30, 30, 34,
];

pub(super) const DEFAULT_8X8_INTRA: [u8; 64] = [
    6, 10, 10, 13, 11, 13, 16, 16, 16, 16, 18, 18, 18, 18, 18, 23, 23, 23, 23, 23, 23, 25, 25, 25,
    25, 25, 25, 25, 27, 27, 27, 27, 27, 27, 27, 27, 29, 29, 29, 29, 29, 29, 29, 31, 31, 31, 31, 31,
    31, 33, 33, 33, 33, 33, 36, 36, 36, 36, 38, 38, 38, 40, 40, 42,
];

pub(super) const DEFAULT_8X8_INTER: [u8; 64] = [
    9, 13, 13, 15, 13, 15, 17, 17, 17, 17, 19, 19, 19, 19, 19, 21, 21, 21, 21, 21, 21, 22, 22, 22,
    22, 22, 22, 22, 24, 24, 24, 24, 24, 24, 24, 24, 25, 25, 25, 25, 25, 25, 25, 27, 27, 27, 27, 27,
    27, 28, 28, 28, 28, 28, 30, 30, 30, 30, 32, 32, 32, 33, 33, 35,
];

const MAX_SPS_COUNT: u8 = 32;

/// The maximum number of pictures in the DPB, as per A.3.1, clause h)
const DPB_MAX_SIZE: usize = 16;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Point<T> {
    pub x: T,
    pub y: T,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Rect<T> {
    pub min: Point<T>,
    pub max: Point<T>,
}

#[derive(N, Debug, PartialEq, Eq, Clone, Copy)]
pub enum NaluType {
    Unknown = 0,
    Slice = 1,
    SliceDpa = 2,
    SliceDpb = 3,
    SliceDpc = 4,
    SliceIdr = 5,
    Sei = 6,
    Sps = 7,
    Pps = 8,
    AuDelimiter = 9,
    SeqEnd = 10,
    StreamEnd = 11,
    FillerData = 12,
    SpsExt = 13,
    PrefixUnit = 14,
    SubsetSps = 15,
    DepthSps = 16,
    SliceAux = 19,
    SliceExt = 20,
    SliceDepth = 21,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RefPicListModification {
    pub modification_of_pic_nums_idc: u8,
    /* if modification_of_pic_nums_idc == 0 || 1 */
    pub abs_diff_pic_num_minus1: u32,
    /* if modification_of_pic_nums_idc == 2 */
    pub long_term_pic_num: u32,
    /* if modification_of_pic_nums_idc == 4 || 5 */
    pub abs_diff_view_idx_minus1: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PredWeightTable {
    pub luma_log2_weight_denom: u8,
    pub chroma_log2_weight_denom: u8,

    pub luma_weight_l0: [i16; 32],
    pub luma_offset_l0: [i8; 32],

    /* if seq->ChromaArrayType != 0 */
    pub chroma_weight_l0: [[i16; 2]; 32],
    pub chroma_offset_l0: [[i8; 2]; 32],

    /* if slice->slice_type % 5 == 1 */
    pub luma_weight_l1: [i16; 32],
    pub luma_offset_l1: [i16; 32],

    /* and if seq->ChromaArrayType != 0 */
    pub chroma_weight_l1: [[i16; 2]; 32],
    pub chroma_offset_l1: [[i8; 2]; 32],
}

#[derive(N, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    #[default]
    L1 = 10,
    L1B = 9,
    L1_1 = 11,
    L1_2 = 12,
    L1_3 = 13,
    L2_0 = 20,
    L2_1 = 21,
    L2_2 = 22,
    L3 = 30,
    L3_1 = 31,
    L3_2 = 32,
    L4 = 40,
    L4_1 = 41,
    L4_2 = 42,
    L5 = 50,
    L5_1 = 51,
    L5_2 = 52,
    L6 = 60,
    L6_1 = 61,
    L6_2 = 62,
}

/// A H264 Sequence Parameter Set. A syntax structure containing syntax elements
/// that apply to zero or more entire coded video sequences as determined by the
/// content of a seq_parameter_set_id syntax element found in the picture
/// parameter set referred to by the pic_parameter_set_id syntax element found
/// in each slice header.
#[derive(Debug, PartialEq, Eq)]
pub struct Sps {
    /// Identifies the sequence parameter set that is referred to by the picture
    /// parameter set
    pub seq_parameter_set_id: u8,

    /// Profile to which the coded video sequence conforms
    pub profile_idc: u8,

    /// Retains the same meaning as in the specification. See 7.4.2.1.1
    pub constraint_set0_flag: bool,
    /// Retains the same meaning as in the specification. See 7.4.2.1.1
    pub constraint_set1_flag: bool,
    /// Retains the same meaning as in the specification. See 7.4.2.1.1
    pub constraint_set2_flag: bool,
    /// Retains the same meaning as in the specification. See 7.4.2.1.1
    pub constraint_set3_flag: bool,
    /// Retains the same meaning as in the specification. See 7.4.2.1.1
    pub constraint_set4_flag: bool,
    /// Retains the same meaning as in the specification. See 7.4.2.1.1
    pub constraint_set5_flag: bool,

    /// Level to which the coded video sequence conforms
    pub level_idc: Level,

    /// Specifies the chroma sampling relative to the luma sampling as specified
    /// in clause 6.2.
    pub chroma_format_idc: u8,

    /// Specifies whether the three colour components of the 4:4:4 chroma format
    /// are coded separately.
    pub separate_colour_plane_flag: bool,

    /// Specifies the bit depth of the samples of the luma array and the value
    /// of the luma quantization parameter range offset QpBdOffsetY. See 7-3 and
    /// 7-4.
    pub bit_depth_luma_minus8: u8,

    /// Specifies the bit depth of the samples of the chroma arrays and the
    /// value of the chroma quantization parameter range offset QpBdOffsetC. See
    /// 7-5 and 7-6.
    pub bit_depth_chroma_minus8: u8,

    /// qpprime_y_zero_transform_bypass_flag equal to 1 specifies that, when
    /// QP′Y is equal to 0, a transform bypass operation for the transform
    /// coefficient decoding process and picture construction process prior to
    /// deblocking filter process as specified in clause 8.5 shall be applied.
    /// qpprime_y_zero_transform_bypass_flag equal to 0 specifies that the
    /// transform coefficient decoding process and picture construction process
    /// prior to deblocking filter process shall not use the transform bypass
    /// operation
    /// QP′Y is defined in 7-38 as QP′Y = QPY + QpBdOffsetY
    pub qpprime_y_zero_transform_bypass_flag: bool,

    /// Whether `seq_scaling_list_present_flag[i]` for i = 0..7 or i = 0..11 is
    /// present or whether the sequence level scaling list shall be specified by
    /// Flat_4x4_16 for i = 0..5 and flat_8x8_16 for i = 6..11
    pub seq_scaling_matrix_present_flag: bool,

    /// 4x4 Scaling list as read with 7.3.2.1.1.1
    pub scaling_lists_4x4: [[u8; 16]; 6],
    /// 8x8 Scaling list as read with 7.3.2.1.1.1
    pub scaling_lists_8x8: [[u8; 64]; 6],

    /// Specifies the value of the variable MaxFrameNum that is used in
    /// frame_num related derivations as follows: MaxFrameNum = 2 ^
    /// (log2_max_frame_num_minus4 + 4 )
    pub log2_max_frame_num_minus4: u8,

    /// Specifies the method to decode picture order count (as specified in
    /// clause 8.2.1)
    pub pic_order_cnt_type: u8,

    /// Specifies the value of the variable MaxPicOrderCntLsb that is used in
    /// the decoding process for picture order count as specified in clause
    /// 8.2.1 as follows: MaxPicOrderCntLsb = 2 ^ (
    /// log2_max_pic_order_cnt_lsb_minus4 + 4 ).
    pub log2_max_pic_order_cnt_lsb_minus4: u8,

    /// If true, specifies that `delta_pic_order_cnt[0]` and
    /// `delta_pic_order_cnt[1]` are not present in the slice headers of the
    /// sequence and shall be inferred to be equal to 0.
    /// If false, specifies that `delta_pic_order_cnt[0]` is present in the
    /// slice headers of the sequence and `delta_pic_order_cnt[1]` may be
    /// present in the slice headers of the sequence.
    pub delta_pic_order_always_zero_flag: bool,

    /// Used to calculate the picture order count of a non-reference picture as
    /// specified in clause 8.2.1.
    pub offset_for_non_ref_pic: i32,

    /// Used to calculate the picture order count of a bottom field as specified
    /// in clause 8.2.1.
    pub offset_for_top_to_bottom_field: i32,

    /// Used in the decoding process for picture order count as specified in
    /// clause 8.2.1
    pub num_ref_frames_in_pic_order_cnt_cycle: u8,

    /// An element of a list of num_ref_frames_in_pic_order_cnt_cycle values
    /// used in the decoding process for picture order count as specified in
    /// clause 8.2.
    pub offset_for_ref_frame: [i32; 255],

    /// Specifies the maximum number of short-term and long-term reference
    /// frames, complementary reference field pairs, and non-paired reference
    /// fields that may be used by the decoding process for inter prediction of
    /// any picture in the coded video sequence. Also
    /// determines the size of the sliding window operation as specified in
    /// clause 8.2.5.3.
    pub max_num_ref_frames: u8,

    /// Specifies the allowed values of frame_num as specified in clause 7.4.3
    /// and the decoding process in case of an inferred gap between values of
    /// frame_num as specified in clause 8.2.5.2
    pub gaps_in_frame_num_value_allowed_flag: bool,

    /// Plus 1 specifies the width of each decoded picture in units of
    /// macroblocks.
    pub pic_width_in_mbs_minus1: u16,
    /// Plus 1 specifies the height in slice group map units of a decoded frame
    /// or field.
    pub pic_height_in_map_units_minus1: u16,

    /// If true,  specifies that every coded picture of the coded video sequence
    /// is a coded frame containing only frame macroblocks, else specifies that
    /// coded pictures of the coded video sequence may either be coded fields or
    /// coded frames.
    pub frame_mbs_only_flag: bool,

    /// If true, specifies the possible use of switching between frame and field
    /// macroblocks within frames, else, specifies no switching between frame
    /// and field macroblocks within a picture.
    pub mb_adaptive_frame_field_flag: bool,

    /// Specifies the method used in the derivation process for luma motion
    /// vectors for B_Skip, B_Direct_16x16 and B_Direct_8x8 as specified in
    /// clause 8.4.1.2.
    pub direct_8x8_inference_flag: bool,

    /// If true, specifies that the frame cropping offset parameters follow next
    /// in the sequence parameter, else specifies that the frame cropping offset
    /// parameters are not present
    pub frame_cropping_flag: bool,

    /// Specify the samples of the pictures in the coded video sequence that are
    /// output from the decoding process, in terms of a rectangular region
    /// specified in frame coordinates for output.
    pub frame_crop_left_offset: u32,
    /// Specify the samples of the pictures in the coded video sequence that are
    /// output from the decoding process, in terms of a rectangular region
    /// specified in frame coordinates for output.
    pub frame_crop_right_offset: u32,
    /// Specify the samples of the pictures in the coded video sequence that are
    /// output from the decoding process, in terms of a rectangular region
    /// specified in frame coordinates for output.
    pub frame_crop_top_offset: u32,
    /// Specify the samples of the pictures in the coded video sequence that are
    /// output from the decoding process, in terms of a rectangular region
    /// specified in frame coordinates for output.
    pub frame_crop_bottom_offset: u32,

    // Calculated
    /// Same as ExpectedDeltaPerPicOrderCntCycle, see 7-12 in the specification.
    pub expected_delta_per_pic_order_cnt_cycle: i32,

    pub vui_parameters_present_flag: bool,
    pub vui_parameters: VuiParams,
}

impl Sps {
    /// Returns the coded width of the stream.
    ///
    /// See 7-13 through 7-17 in the specification.
    pub const fn width(&self) -> u32 {
        (self.pic_width_in_mbs_minus1 as u32 + 1) * 16
    }

    /// Returns the coded height of the stream.
    ///
    /// See 7-13 through 7-17 in the specification.
    pub const fn height(&self) -> u32 {
        (self.pic_height_in_map_units_minus1 as u32 + 1)
            * 16
            * (2 - self.frame_mbs_only_flag as u32)
    }

    /// Returns `ChromaArrayType`, as computed in the specification.
    pub const fn chroma_array_type(&self) -> u8 {
        match self.separate_colour_plane_flag {
            false => self.chroma_format_idc,
            true => 0,
        }
    }

    /// Returns `SubWidthC` and `SubHeightC`.
    ///
    /// See table 6-1 in the specification.
    fn sub_width_height_c(&self) -> (u32, u32) {
        match (self.chroma_format_idc, self.separate_colour_plane_flag) {
            (1, false) => (2, 2),
            (2, false) => (2, 1),
            (3, false) => (1, 1),
            // undefined.
            _ => (1, 1),
        }
    }

    /// Returns `CropUnitX` and `CropUnitY`.
    ///
    /// See 7-19 through 7-22 in the specification.
    fn crop_unit_x_y(&self) -> (u32, u32) {
        match self.chroma_array_type() {
            0 => (1, 2 - u32::from(self.frame_mbs_only_flag)),
            _ => {
                let (sub_width_c, sub_height_c) = self.sub_width_height_c();
                (
                    sub_width_c,
                    sub_height_c * (2 - u32::from(self.frame_mbs_only_flag)),
                )
            }
        }
    }
}

// TODO: Replace with builder
impl Default for Sps {
    fn default() -> Self {
        Self {
            scaling_lists_4x4: [[0; 16]; 6],
            scaling_lists_8x8: [[0; 64]; 6],
            offset_for_ref_frame: [0; 255],
            seq_parameter_set_id: Default::default(),
            profile_idc: Default::default(),
            constraint_set0_flag: Default::default(),
            constraint_set1_flag: Default::default(),
            constraint_set2_flag: Default::default(),
            constraint_set3_flag: Default::default(),
            constraint_set4_flag: Default::default(),
            constraint_set5_flag: Default::default(),
            level_idc: Default::default(),
            chroma_format_idc: Default::default(),
            separate_colour_plane_flag: Default::default(),
            bit_depth_luma_minus8: Default::default(),
            bit_depth_chroma_minus8: Default::default(),
            qpprime_y_zero_transform_bypass_flag: Default::default(),
            seq_scaling_matrix_present_flag: Default::default(),
            log2_max_frame_num_minus4: Default::default(),
            pic_order_cnt_type: Default::default(),
            log2_max_pic_order_cnt_lsb_minus4: Default::default(),
            delta_pic_order_always_zero_flag: Default::default(),
            offset_for_non_ref_pic: Default::default(),
            offset_for_top_to_bottom_field: Default::default(),
            num_ref_frames_in_pic_order_cnt_cycle: Default::default(),
            max_num_ref_frames: Default::default(),
            gaps_in_frame_num_value_allowed_flag: Default::default(),
            pic_width_in_mbs_minus1: Default::default(),
            pic_height_in_map_units_minus1: Default::default(),
            frame_mbs_only_flag: Default::default(),
            mb_adaptive_frame_field_flag: Default::default(),
            direct_8x8_inference_flag: Default::default(),
            frame_cropping_flag: Default::default(),
            frame_crop_left_offset: Default::default(),
            frame_crop_right_offset: Default::default(),
            frame_crop_top_offset: Default::default(),
            frame_crop_bottom_offset: Default::default(),
            expected_delta_per_pic_order_cnt_cycle: Default::default(),
            vui_parameters_present_flag: Default::default(),
            vui_parameters: Default::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HrdParams {
    /// Plus 1 specifies the number of alternative CPB specifications in the
    /// bitstream. The value of `cpb_cnt_minus1` shall be in the range of 0 to 31,
    /// inclusive
    pub cpb_cnt_minus1: u8,
    /// Together with `bit_rate_value_minus1[ SchedSelIdx ]` specifies the
    /// maximum input bit rate of the `SchedSelIdx`-th CPB.
    pub bit_rate_scale: u8,
    /// Together with `cpb_size_value_minus1[ SchedSelIdx ]` specifies the CPB
    /// size of the SchedSelIdx-th CPB.
    pub cpb_size_scale: u8,

    /// `[ SchedSelIdx ]` (together with bit_rate_scale) specifies the maximum
    /// input bit rate for the SchedSelIdx-th CPB.
    pub bit_rate_value_minus1: [u32; 32],
    /// `[ SchedSelIdx ]` is used together with cpb_size_scale to specify the
    /// SchedSelIdx-th CPB size.
    pub cpb_size_value_minus1: [u32; 32],
    /// `[ SchedSelIdx ]` equal to 0 specifies that to decode this bitstream by
    /// the HRD using the `SchedSelIdx`-th CPB specification, the hypothetical
    /// stream delivery scheduler (HSS) operates in an intermittent bit rate
    /// mode. `cbr_flag[ SchedSelIdx ]` equal to 1 specifies that the HSS operates
    /// in a constant bit rate (CBR) mode
    pub cbr_flag: [bool; 32],

    /// Specifies the length in bits of the `initial_cpb_removal_delay[
    /// SchedSelIdx ]` and `initial_cpb_removal_delay_offset[ SchedSelIdx ]` syntax
    /// elements of the buffering period SEI message.
    pub initial_cpb_removal_delay_length_minus1: u8,
    /// Specifies the length in bits of the `cpb_removal_delay` syntax element.
    pub cpb_removal_delay_length_minus1: u8,
    /// Specifies the length in bits of the `dpb_output_delay` syntax element.
    pub dpb_output_delay_length_minus1: u8,
    /// If greater than 0, specifies the length in bits of the `time_offset`
    /// syntax element. `time_offset_length` equal to 0 specifies that the
    /// `time_offset` syntax element is not present
    pub time_offset_length: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VuiParams {
    /// Specifies whether `aspect_ratio_idc` is present.
    pub aspect_ratio_info_present_flag: bool,
    /// Specifies the value of the sample aspect ratio of the luma samples.
    /// Table E-1 shows the meaning of the code. When aspect_ratio_idc indicates
    /// Extended_SAR, the sample aspect ratio is represented by sar_width :
    /// sar_height. When the aspect_ratio_idc syntax element is not present,
    /// aspect_ratio_idc value shall be inferred to be equal to 0
    pub aspect_ratio_idc: u8,

    /* if aspect_ratio_idc == 255 */
    /// Indicates the horizontal size of the sample aspect ratio (in arbitrary
    /// units)
    pub sar_width: u16,
    /// Indicates the vertical size of the sample aspect ratio (in the same
    /// arbitrary units as sar_width).
    pub sar_height: u16,

    /// If true specifies that the overscan_appropriate_flag is present. Else,
    /// the preferred display method for the video signal is unspecified
    pub overscan_info_present_flag: bool,
    /* if overscan_info_present_flag */
    /// If true, indicates that the cropped decoded pictures output are suitable
    /// for display using overscan. Else, indicates that the cropped decoded
    /// pictures output contain visually important information in the entire
    /// region out to the edges of the cropping rectangle of the picture, such
    /// that the cropped decoded pictures output should not be displayed using
    /// overscan.
    pub overscan_appropriate_flag: bool,

    /// Specifies that video_format, video_full_range_flag and
    /// colour_description_present_flag are present
    pub video_signal_type_present_flag: bool,
    /// Indicates the representation of the pictures as specified in Table E-2,
    /// before being coded in accordance with this Recommendation |
    /// International Standard. When the video_format syntax element is not
    /// present, video_format value shall be inferred to be equal to 5.
    pub video_format: u8,
    /// Indicates the black level and range of the luma and chroma signals as
    /// derived from E′Y, E′PB, and E′PR or E′ R, E′G, and E′B real-valued
    /// component signals.
    pub video_full_range_flag: bool,
    /// Specifies that colour_primaries, transfer_characteristics and
    /// matrix_coefficients are present.
    pub colour_description_present_flag: bool,
    /// Indicates the chromaticity coordinates of the source primaries as
    /// specified in Table E-3 in terms of the CIE 1931 definition of x and y as
    /// specified by ISO 11664-1.
    pub colour_primaries: u8,
    /// Retains same meaning as in the specification.
    pub transfer_characteristics: u8,
    /// Describes the matrix coefficients used in deriving luma and chroma
    /// signals from the green, blue, and red, or Y, Z, and X primaries, as
    /// specified in Table E-5.
    pub matrix_coefficients: u8,

    /// Specifies that chroma_sample_loc_type_top_field and
    /// chroma_sample_loc_type_bottom_field are present
    pub chroma_loc_info_present_flag: bool,
    /// Specify the location of chroma samples. See the spec for more details.
    pub chroma_sample_loc_type_top_field: u8,
    /// Specify the location of chroma samples. See the spec for more details.
    pub chroma_sample_loc_type_bottom_field: u8,

    /// Specifies that num_units_in_tick, time_scale and fixed_frame_rate_flag
    /// are present in the bitstream
    pub timing_info_present_flag: bool,
    /* if timing_info_present_flag */
    /// The number of time units of a clock operating at the frequency
    /// time_scale Hz that corresponds to one increment (called a clock tick) of
    /// a clock tick counter
    pub num_units_in_tick: u32,
    /// The number of time units that pass in one second. For example, a time
    /// coordinate system that measures time using a 27 MHz clock has a
    /// time_scale of 27 000 000. time_scale shall be greater than 0.
    pub time_scale: u32,
    /// Retains the same meaning as the specification.
    pub fixed_frame_rate_flag: bool,

    /// Specifies that NAL HRD parameters (pertaining to Type II bitstream
    /// conformance) are present.
    pub nal_hrd_parameters_present_flag: bool,
    /* if nal_hrd_parameters_present_flag */
    /// The NAL HDR parameters
    pub nal_hrd_parameters: HrdParams,
    /// Specifies that VCL HRD parameters (pertaining to all bitstream
    /// conformance) are present.
    pub vcl_hrd_parameters_present_flag: bool,
    /* if vcl_hrd_parameters_present_flag */
    /// The VCL HRD parameters
    pub vcl_hrd_parameters: HrdParams,

    /// Specifies the HRD operational mode as specified in Annex C.
    pub low_delay_hrd_flag: bool,

    /// Specifies that picture timing SEI messages (clause D.2.3) are present
    /// that include the pic_struct syntax element.
    pub pic_struct_present_flag: bool,

    /// Specifies that the following coded video sequence bitstream restriction
    /// parameters are present
    pub bitstream_restriction_flag: bool,
    /*  if bitstream_restriction_flag */
    /// If false, indicates that no sample outside the picture boundaries and no
    /// sample at a fractional sample position for which the sample value is
    /// derived using one or more samples outside the picture boundaries is used
    /// for inter prediction of any sample. If true, indicates that one or more
    /// samples outside picture boundaries may be used in inter prediction. When
    /// the motion_vectors_over_pic_boundaries_flag syntax element is not
    /// present, motion_vectors_over_pic_boundaries_flag value shall be inferred
    /// to be true.
    pub motion_vectors_over_pic_boundaries_flag: bool,
    /// Indicates a number of bytes not exceeded by the sum of the sizes of the
    /// VCL NAL units associated with any coded picture in the coded video
    /// sequence.
    pub max_bytes_per_pic_denom: u32,
    /// Indicates an upper bound for the number of coded bits of
    /// macroblock_layer( ) data for any macroblock in any picture of the coded
    /// video sequence
    pub max_bits_per_mb_denom: u32,
    /// Retains the same meaning as the specification.
    pub log2_max_mv_length_horizontal: u32,
    /// Retains the same meaning as the specification.
    pub log2_max_mv_length_vertical: u32,
    /// Indicates an upper bound for the number of frames buffers, in the
    /// decoded picture buffer (DPB), that are required for storing frames,
    /// complementary field pairs, and non-paired fields before output. It is a
    /// requirement of bitstream conformance that the maximum number of frames,
    /// complementary field pairs, or non-paired fields that precede any frame,
    /// complementary field pair, or non-paired field in the coded video
    /// sequence in decoding order and follow it in output order shall be less
    /// than or equal to max_num_reorder_frames. The value of
    /// max_num_reorder_frames shall be in the range of 0 to
    /// max_dec_frame_buffering, inclusive.
    ///
    /// When the max_num_reorder_frames syntax element is not present, the value
    /// of max_num_reorder_frames value shall be inferred as follows:
    /// If profile_idc is equal to 44, 86, 100, 110, 122, or 244 and
    /// constraint_set3_flag is equal to 1, the value of max_num_reorder_frames
    /// shall be inferred to be equal to 0.
    ///
    /// Otherwise (profile_idc is not equal to 44, 86, 100, 110, 122, or 244 or
    /// constraint_set3_flag is equal to 0), the value of max_num_reorder_frames
    /// shall be inferred to be equal to MaxDpbFrames.
    pub max_num_reorder_frames: u32,
    /// Specifies the required size of the HRD decoded picture buffer (DPB) in
    /// units of frame buffers. It is a requirement of bitstream conformance
    /// that the coded video sequence shall not require a decoded picture buffer
    /// with size of more than Max( 1, max_dec_frame_buffering ) frame buffers
    /// to enable the output of decoded pictures at the output times specified
    /// by dpb_output_delay of the picture timing SEI messages. The value of
    /// max_dec_frame_buffering shall be greater than or equal to
    /// max_num_ref_frames. An upper bound for the value of
    /// max_dec_frame_buffering is specified by the level limits in clauses
    /// A.3.1, A.3.2, G.10.2.1, and H.10.2.
    ///
    /// When the max_dec_frame_buffering syntax element is not present, the
    /// value of max_dec_frame_buffering shall be inferred as follows:
    ///
    /// If profile_idc is equal to 44, 86, 100, 110, 122, or 244 and
    /// constraint_set3_flag is equal to 1, the value of max_dec_frame_buffering
    /// shall be inferred to be equal to 0.
    ///
    /// Otherwise (profile_idc is not equal to 44, 86, 100, 110, 122, or 244 or
    /// constraint_set3_flag is equal to 0), the value of
    /// max_dec_frame_buffering shall be inferred to be equal to MaxDpbFrames.
    pub max_dec_frame_buffering: u32,
}

impl Default for VuiParams {
    fn default() -> Self {
        Self {
            aspect_ratio_info_present_flag: Default::default(),
            aspect_ratio_idc: Default::default(),
            sar_width: Default::default(),
            sar_height: Default::default(),
            overscan_info_present_flag: Default::default(),
            overscan_appropriate_flag: Default::default(),
            video_signal_type_present_flag: Default::default(),
            video_format: 5,
            video_full_range_flag: Default::default(),
            colour_description_present_flag: Default::default(),
            colour_primaries: 2,
            transfer_characteristics: 2,
            matrix_coefficients: 2,
            chroma_loc_info_present_flag: Default::default(),
            chroma_sample_loc_type_top_field: Default::default(),
            chroma_sample_loc_type_bottom_field: Default::default(),
            timing_info_present_flag: Default::default(),
            num_units_in_tick: Default::default(),
            time_scale: Default::default(),
            fixed_frame_rate_flag: Default::default(),
            nal_hrd_parameters_present_flag: Default::default(),
            nal_hrd_parameters: Default::default(),
            vcl_hrd_parameters_present_flag: Default::default(),
            vcl_hrd_parameters: Default::default(),
            low_delay_hrd_flag: Default::default(),
            pic_struct_present_flag: Default::default(),
            bitstream_restriction_flag: Default::default(),
            motion_vectors_over_pic_boundaries_flag: Default::default(),
            max_bytes_per_pic_denom: Default::default(),
            max_bits_per_mb_denom: Default::default(),
            log2_max_mv_length_horizontal: Default::default(),
            log2_max_mv_length_vertical: Default::default(),
            max_num_reorder_frames: Default::default(),
            max_dec_frame_buffering: Default::default(),
        }
    }
}

/// A H264 Picture Parameter Set. A syntax structure containing syntax elements
/// that apply to zero or more entire coded pictures as determined by the
/// `pic_parameter_set_id` syntax element found in each slice header.
#[derive(Debug, PartialEq, Eq)]
pub struct Pps {
    /// Identifies the picture parameter set that is referred to in the slice header.
    pub pic_parameter_set_id: u8,

    /// Refers to the active sequence parameter set.
    pub seq_parameter_set_id: u8,

    /// Selects the entropy decoding method to be applied for the syntax
    /// elements for which two descriptors appear in the syntax tables as
    /// follows: If `entropy_coding_mode_flag` is false, the method specified by
    /// the left descriptor in the syntax table is applied (Exp-Golomb coded,
    /// see clause 9.1 or CAVLC, see clause 9.2). Otherwise
    /// (`entropy_coding_mode_flag` is true), the method specified by the right
    /// descriptor in the syntax table is applied (CABAC, see clause 9.3).
    pub entropy_coding_mode_flag: bool,

    /// If true, specifies that the syntax elements delta_pic_order_cnt_bottom
    /// (when `pic_order_cnt_type` is equal to 0) or `delta_pic_order_cnt[1]`
    /// (when `pic_order_cnt_type` is equal to 1), which are related to picture
    /// order counts for the bottom field of a coded frame, are present in the
    /// slice headers for coded frames as specified in clause 7.3.3. Otherwise,
    /// specifies that the syntax elements `delta_pic_order_cnt_bottom` and
    /// `delta_pic_order_cnt[1]` are not present in the slice headers.
    pub bottom_field_pic_order_in_frame_present_flag: bool,

    /// Plus 1 specifies the number of slice groups for a picture. When
    /// `num_slice_groups_minus1` is equal to 0, all slices of the picture
    /// belong to the same slice group. The allowed range of
    /// `num_slice_groups_minus1` is specified in Annex A.
    pub num_slice_groups_minus1: u32,

    /// Specifies how `num_ref_idx_l0_active_minus1` is inferred for P, SP, and
    /// B slices with `num_ref_idx_active_override_flag` not set.
    pub num_ref_idx_l0_default_active_minus1: u8,

    /// Specifies how `num_ref_idx_l1_active_minus1` is inferred for B slices
    /// with `num_ref_idx_active_override_flag` not set.
    pub num_ref_idx_l1_default_active_minus1: u8,

    /// If not set, specifies that the default weighted prediction shall be
    /// applied to P and SP slices. If set, specifies that explicit weighted
    /// prediction shall be applied to P and SP slices.
    pub weighted_pred_flag: bool,

    /// `weighted_bipred_idc` equal to 0 specifies that the default weighted
    /// prediction shall be applied to B slices. `weighted_bipred_idc` equal to
    /// 1 specifies that explicit weighted prediction shall be applied to B
    /// slices. `weighted_bipred_idc` equal to 2 specifies that implicit
    /// weighted prediction shall be applied to B slices
    pub weighted_bipred_idc: u8,

    /// Specifies the initial value minus 26 of SliceQPY for each slice. The
    /// initial value is modified at the slice layer when a non-zero value of
    /// `slice_qp_delta` is decoded, and is modified further when a non-zero
    /// value of `mb_qp_delta` is decoded at the macroblock layer.
    pub pic_init_qp_minus26: i8,

    /// Specifies the initial value minus 26 of SliceQSY for all macroblocks in
    /// SP or SI slices. The initial value is modified at the slice layer when a
    /// non-zero value of `slice_qs_delta` is decoded.
    pub pic_init_qs_minus26: i8,

    /// Specifies the offset that shall be added to QP Y and QSY for addressing
    /// the table of QPC values for the Cb chroma component.
    pub chroma_qp_index_offset: i8,

    /// If set, specifies that a set of syntax elements controlling the
    /// characteristics of the deblocking filter is present in the slice header.
    /// If not set, specifies that the set of syntax elements controlling the
    /// characteristics of the deblocking filter is not present in the slice
    /// headers and their inferred values are in effect.
    pub deblocking_filter_control_present_flag: bool,

    /// If not set, specifies that intra prediction allows usage of residual
    /// data and decoded samples of neighbouring macroblocks coded using Inter
    /// macroblock prediction modes for the prediction of macroblocks coded
    /// using Intra macroblock prediction modes. If set, specifies constrained
    /// intra prediction, in which case prediction of macroblocks coded using
    /// Intra macroblock prediction modes only uses residual data and decoded
    /// samples from I or SI macroblock types.
    pub constrained_intra_pred_flag: bool,

    /// If not set, specifies that the `redundant_pic_cnt` syntax element is not
    /// present in slice headers, coded slice data partition B NAL units, and
    /// coded slice data partition C NAL units that refer (either directly or by
    /// association with a corresponding coded slice data partition A NAL unit)
    /// to the picture parameter set. If set, specifies that the
    /// `redundant_pic_cnt` syntax element is present in all slice headers,
    /// coded slice data partition B NAL units, and coded slice data partition C
    /// NAL units that refer (either directly or by association with a
    /// corresponding coded slice data partition A NAL unit) to the picture
    /// parameter set.
    pub redundant_pic_cnt_present_flag: bool,

    /// If set, specifies that the 8x8 transform decoding process may be in use
    /// (see clause 8.5). If not set, specifies that the 8x8 transform decoding
    /// process is not in use.
    pub transform_8x8_mode_flag: bool,

    ///  If set, specifies that parameters are present to modify the scaling
    ///  lists specified in the sequence parameter set. If not set, specifies
    ///  that the scaling lists used for the picture shall be inferred to be
    ///  equal to those specified by the sequence parameter set.
    pub pic_scaling_matrix_present_flag: bool,

    /// 4x4 Scaling list as read with 7.3.2.1.1.1
    pub scaling_lists_4x4: [[u8; 16]; 6],
    /// 8x8 Scaling list as read with 7.3.2.1.1.1
    pub scaling_lists_8x8: [[u8; 64]; 6],

    /// Specifies the offset that shall be added to QPY and QSY for addressing
    /// the table of QPC values for the Cr chroma component. When
    /// `second_chroma_qp_index_offset` is not present, it shall be inferred to be
    /// equal to `chroma_qp_index_offset`.
    pub second_chroma_qp_index_offset: i8,

    /// The SPS referenced by this PPS.
    pub sps: Rc<Sps>,
}

#[derive(Debug, Default)]
pub struct Parser {
    active_spses: BTreeMap<u8, Rc<Sps>>,
}

impl Parser {
    fn fill_default_scaling_list_4x4(scaling_list4x4: &mut [u8; 16], i: usize) {
        // See table 7.2 in the spec.
        assert!(i < 6);
        if i < 3 {
            *scaling_list4x4 = DEFAULT_4X4_INTRA;
        } else if i < 6 {
            *scaling_list4x4 = DEFAULT_4X4_INTER;
        }
    }

    fn fill_default_scaling_list_8x8(scaling_list8x8: &mut [u8; 64], i: usize) {
        assert!(i < 6);
        if i % 2 == 0 {
            *scaling_list8x8 = DEFAULT_8X8_INTRA;
        } else {
            *scaling_list8x8 = DEFAULT_8X8_INTER;
        }
    }

    fn fill_fallback_scaling_list_4x4(
        scaling_list4x4: &mut [[u8; 16]; 6],
        i: usize,
        default_scaling_list_intra: &[u8; 16],
        default_scaling_list_inter: &[u8; 16],
    ) {
        // See table 7.2 in the spec.
        scaling_list4x4[i] = match i {
            0 => *default_scaling_list_intra,
            1 => scaling_list4x4[0],
            2 => scaling_list4x4[1],
            3 => *default_scaling_list_inter,
            4 => scaling_list4x4[3],
            5 => scaling_list4x4[4],
            _ => panic!("Unexpected value {i}"),
        }
    }

    fn fill_fallback_scaling_list_8x8(
        scaling_list8x8: &mut [[u8; 64]; 6],
        i: usize,
        default_scaling_list_intra: &[u8; 64],
        default_scaling_list_inter: &[u8; 64],
    ) {
        // See table 7.2 in the spec.
        scaling_list8x8[i] = match i {
            0 => *default_scaling_list_intra,
            1 => *default_scaling_list_inter,
            2 => scaling_list8x8[0],
            3 => scaling_list8x8[1],
            4 => scaling_list8x8[2],
            5 => scaling_list8x8[3],
            _ => panic!("Unexpected value {i}"),
        }
    }

    fn fill_scaling_list_flat(
        scaling_list4x4: &mut [[u8; 16]; 6],
        scaling_list8x8: &mut [[u8; 64]; 6],
    ) {
        // (7-8) in the spec.
        for outer in scaling_list4x4 {
            for inner in outer {
                *inner = 16;
            }
        }

        // (7-9) in the spec.
        for outer in scaling_list8x8 {
            for inner in outer {
                *inner = 16;
            }
        }
    }

    fn parse_scaling_list<U: AsMut<[u8]>>(
        r: &mut NaluReader,
        scaling_list: &mut U,
        use_default: &mut bool,
    ) -> anyhow::Result<()> {
        // 7.3.2.1.1.1
        let mut last_scale = 8u8;
        let mut next_scale = 8u8;

        for j in 0..scaling_list.as_mut().len() {
            if next_scale != 0 {
                let delta_scale = r.read_se::<i32>()?;
                next_scale = ((last_scale as i32 + delta_scale + 256) % 256) as u8;
                *use_default = j == 0 && next_scale == 0;
                if *use_default {
                    return Ok(());
                }
            }

            scaling_list.as_mut()[j] = if next_scale == 0 {
                last_scale
            } else {
                next_scale
            };

            last_scale = scaling_list.as_mut()[j];
        }

        Ok(())
    }

    fn parse_sps_scaling_lists(r: &mut NaluReader, sps: &mut Sps) -> anyhow::Result<()> {
        let scaling_lists4x4 = &mut sps.scaling_lists_4x4;
        let scaling_lisst8x8 = &mut sps.scaling_lists_8x8;

        // Parse scaling_list4x4
        for i in 0..6 {
            let seq_scaling_list_present_flag = r.read_bit()?;
            if seq_scaling_list_present_flag {
                let mut use_default = false;

                Parser::parse_scaling_list(r, &mut scaling_lists4x4[i], &mut use_default)?;

                if use_default {
                    Parser::fill_default_scaling_list_4x4(&mut scaling_lists4x4[i], i);
                }
            } else {
                Parser::fill_fallback_scaling_list_4x4(
                    scaling_lists4x4,
                    i,
                    &DEFAULT_4X4_INTRA,
                    &DEFAULT_4X4_INTER,
                );
            }
        }

        // Parse scaling_list8x8
        let num_8x8 = if sps.chroma_format_idc != 3 { 2 } else { 6 };
        for i in 0..num_8x8 {
            let seq_scaling_list_present_flag = r.read_bit()?;
            if seq_scaling_list_present_flag {
                let mut use_default = false;
                Parser::parse_scaling_list(r, &mut scaling_lisst8x8[i], &mut use_default)?;

                if use_default {
                    Parser::fill_default_scaling_list_8x8(&mut scaling_lisst8x8[i], i);
                }
            } else {
                Parser::fill_fallback_scaling_list_8x8(
                    scaling_lisst8x8,
                    i,
                    &DEFAULT_8X8_INTRA,
                    &DEFAULT_8X8_INTER,
                );
            }
        }
        Ok(())
    }

    fn parse_hrd(r: &mut NaluReader, hrd: &mut HrdParams) -> anyhow::Result<()> {
        hrd.cpb_cnt_minus1 = r.read_ue_max(31)?;
        hrd.bit_rate_scale = r.read_bits(4)?;
        hrd.cpb_size_scale = r.read_bits(4)?;

        for sched_sel_idx in 0..=usize::from(hrd.cpb_cnt_minus1) {
            hrd.bit_rate_value_minus1[sched_sel_idx] = r.read_ue()?;
            hrd.cpb_size_value_minus1[sched_sel_idx] = r.read_ue()?;
            hrd.cbr_flag[sched_sel_idx] = r.read_bit()?;
        }

        hrd.initial_cpb_removal_delay_length_minus1 = r.read_bits(5)?;
        hrd.cpb_removal_delay_length_minus1 = r.read_bits(5)?;
        hrd.dpb_output_delay_length_minus1 = r.read_bits(5)?;
        hrd.time_offset_length = r.read_bits(5)?;
        Ok(())
    }

    fn parse_vui(r: &mut NaluReader, sps: &mut Sps) -> anyhow::Result<()> {
        let vui = &mut sps.vui_parameters;

        vui.aspect_ratio_info_present_flag = r.read_bit()?;
        if vui.aspect_ratio_info_present_flag {
            vui.aspect_ratio_idc = r.read_bits(8)?;
            if vui.aspect_ratio_idc == 255 {
                vui.sar_width = r.read_bits(16)?;
                vui.sar_height = r.read_bits(16)?;
            }
        }

        vui.overscan_info_present_flag = r.read_bit()?;
        if vui.overscan_info_present_flag {
            vui.overscan_appropriate_flag = r.read_bit()?;
        }

        vui.video_signal_type_present_flag = r.read_bit()?;
        if vui.video_signal_type_present_flag {
            vui.video_format = r.read_bits(3)?;
            vui.video_full_range_flag = r.read_bit()?;
            vui.colour_description_present_flag = r.read_bit()?;
            if vui.colour_description_present_flag {
                vui.colour_primaries = r.read_bits(8)?;
                vui.transfer_characteristics = r.read_bits(8)?;
                vui.matrix_coefficients = r.read_bits(8)?;
            }
        }

        vui.chroma_loc_info_present_flag = r.read_bit()?;
        if vui.chroma_loc_info_present_flag {
            vui.chroma_sample_loc_type_top_field = r.read_ue_max(5)?;
            vui.chroma_sample_loc_type_bottom_field = r.read_ue_max(5)?;
        }

        vui.timing_info_present_flag = r.read_bit()?;
        if vui.timing_info_present_flag {
            vui.num_units_in_tick = r.read_bits::<u32>(31)? << 1;
            vui.num_units_in_tick |= r.read_bit()? as u32;
            if vui.num_units_in_tick == 0 {
                return Err(anyhow!(
                    "num_units_in_tick == 0, which is not allowed by E.2.1"
                ));
            }

            vui.time_scale = r.read_bits::<u32>(31)? << 1;
            vui.time_scale |= r.read_bit()? as u32;
            if vui.time_scale == 0 {
                return Err(anyhow!("time_scale == 0, which is not allowed by E.2.1"));
            }

            vui.fixed_frame_rate_flag = r.read_bit()?;
        }

        vui.nal_hrd_parameters_present_flag = r.read_bit()?;
        if vui.nal_hrd_parameters_present_flag {
            Parser::parse_hrd(r, &mut vui.nal_hrd_parameters)?;
        }

        vui.vcl_hrd_parameters_present_flag = r.read_bit()?;
        if vui.vcl_hrd_parameters_present_flag {
            Parser::parse_hrd(r, &mut vui.vcl_hrd_parameters)?;
        }

        if vui.nal_hrd_parameters_present_flag || vui.vcl_hrd_parameters_present_flag {
            vui.low_delay_hrd_flag = r.read_bit()?;
        }

        vui.pic_struct_present_flag = r.read_bit()?;
        vui.bitstream_restriction_flag = r.read_bit()?;

        if vui.bitstream_restriction_flag {
            vui.motion_vectors_over_pic_boundaries_flag = r.read_bit()?;
            vui.max_bytes_per_pic_denom = r.read_ue()?;
            vui.max_bits_per_mb_denom = r.read_ue_max(16)?;
            vui.log2_max_mv_length_horizontal = r.read_ue_max(16)?;
            vui.log2_max_mv_length_vertical = r.read_ue_max(16)?;
            vui.max_num_reorder_frames = r.read_ue()?;
            vui.max_dec_frame_buffering = r.read_ue()?;
        }

        Ok(())
    }

    /// Parse a SPS and add it to the list of active SPSes.
    ///
    /// Returns a reference to the new SPS.
    pub fn parse_sps(&mut self, nalu: &Nalu) -> anyhow::Result<&Rc<Sps>> {
        if !matches!(nalu.header.type_, NaluType::Sps) {
            return Err(anyhow!(
                "Invalid NALU type, expected {:?}, got {:?}",
                NaluType::Sps,
                nalu.header.type_
            ));
        }

        let data = nalu.as_ref();
        // Skip the header
        let mut r = NaluReader::new(&data[nalu.header.len()..]);
        let mut sps = Sps {
            profile_idc: r.read_bits(8)?,
            constraint_set0_flag: r.read_bit()?,
            constraint_set1_flag: r.read_bit()?,
            constraint_set2_flag: r.read_bit()?,
            constraint_set3_flag: r.read_bit()?,
            constraint_set4_flag: r.read_bit()?,
            constraint_set5_flag: r.read_bit()?,
            ..Default::default()
        };

        // skip reserved_zero_2bits
        r.skip_bits(2)?;

        let level: u8 = r.read_bits(8)?;
        sps.level_idc = Level::n(level).with_context(|| format!("Unsupported level {level}"))?;
        sps.seq_parameter_set_id = r.read_ue_max(31)?;

        if sps.profile_idc == 100
            || sps.profile_idc == 110
            || sps.profile_idc == 122
            || sps.profile_idc == 244
            || sps.profile_idc == 44
            || sps.profile_idc == 83
            || sps.profile_idc == 86
            || sps.profile_idc == 118
            || sps.profile_idc == 128
            || sps.profile_idc == 138
            || sps.profile_idc == 139
            || sps.profile_idc == 134
            || sps.profile_idc == 135
        {
            sps.chroma_format_idc = r.read_ue_max(3)?;
            if sps.chroma_format_idc == 3 {
                sps.separate_colour_plane_flag = r.read_bit()?;
            }

            sps.bit_depth_luma_minus8 = r.read_ue_max(6)?;
            sps.bit_depth_chroma_minus8 = r.read_ue_max(6)?;
            sps.qpprime_y_zero_transform_bypass_flag = r.read_bit()?;
            sps.seq_scaling_matrix_present_flag = r.read_bit()?;

            if sps.seq_scaling_matrix_present_flag {
                Parser::parse_sps_scaling_lists(&mut r, &mut sps)?;
            } else {
                Parser::fill_scaling_list_flat(
                    &mut sps.scaling_lists_4x4,
                    &mut sps.scaling_lists_8x8,
                );
            }
        } else {
            sps.chroma_format_idc = 1;
            Parser::fill_scaling_list_flat(&mut sps.scaling_lists_4x4, &mut sps.scaling_lists_8x8);
        }

        sps.log2_max_frame_num_minus4 = r.read_ue_max(12)?;

        sps.pic_order_cnt_type = r.read_ue_max(2)?;

        if sps.pic_order_cnt_type == 0 {
            sps.log2_max_pic_order_cnt_lsb_minus4 = r.read_ue_max(12)?;
            sps.expected_delta_per_pic_order_cnt_cycle = 0;
        } else if sps.pic_order_cnt_type == 1 {
            sps.delta_pic_order_always_zero_flag = r.read_bit()?;
            sps.offset_for_non_ref_pic = r.read_se()?;
            sps.offset_for_top_to_bottom_field = r.read_se()?;
            sps.num_ref_frames_in_pic_order_cnt_cycle = r.read_ue_max(254)?;

            let mut offset_acc = 0;
            for i in 0..usize::from(sps.num_ref_frames_in_pic_order_cnt_cycle) {
                sps.offset_for_ref_frame[i] = r.read_se()?;

                // (7-12) in the spec.
                offset_acc += sps.offset_for_ref_frame[i];
            }

            sps.expected_delta_per_pic_order_cnt_cycle = offset_acc;
        }

        sps.max_num_ref_frames = r.read_ue_max(DPB_MAX_SIZE as u32)?;
        sps.gaps_in_frame_num_value_allowed_flag = r.read_bit()?;
        sps.pic_width_in_mbs_minus1 = r.read_ue()?;
        sps.pic_height_in_map_units_minus1 = r.read_ue()?;
        sps.frame_mbs_only_flag = r.read_bit()?;

        if !sps.frame_mbs_only_flag {
            sps.mb_adaptive_frame_field_flag = r.read_bit()?;
        }

        sps.direct_8x8_inference_flag = r.read_bit()?;
        sps.frame_cropping_flag = r.read_bit()?;

        if sps.frame_cropping_flag {
            sps.frame_crop_left_offset = r.read_ue()?;
            sps.frame_crop_right_offset = r.read_ue()?;
            sps.frame_crop_top_offset = r.read_ue()?;
            sps.frame_crop_bottom_offset = r.read_ue()?;

            // Validate that cropping info is valid.
            let (crop_unit_x, crop_unit_y) = sps.crop_unit_x_y();

            let _ = sps
                .frame_crop_left_offset
                .checked_add(sps.frame_crop_right_offset)
                .and_then(|r| r.checked_mul(crop_unit_x))
                .and_then(|r| sps.width().checked_sub(r))
                .ok_or(anyhow!("Invalid frame crop width"))?;

            let _ = sps
                .frame_crop_top_offset
                .checked_add(sps.frame_crop_bottom_offset)
                .and_then(|r| r.checked_mul(crop_unit_y))
                .and_then(|r| sps.height().checked_sub(r))
                .ok_or(anyhow!("invalid frame crop height"))?;
        }

        sps.vui_parameters_present_flag = r.read_bit()?;
        if sps.vui_parameters_present_flag {
            Parser::parse_vui(&mut r, &mut sps)?;
        }

        let key = sps.seq_parameter_set_id;

        if self.active_spses.keys().len() >= MAX_SPS_COUNT as usize {
            return Err(anyhow!(
                "Broken data: Number of active SPSs > MAX_SPS_COUNT"
            ));
        }

        let sps = Rc::new(sps);
        self.active_spses.remove(&key);
        Ok(self.active_spses.entry(key).or_insert(sps))
    }
}

#[derive(Debug)]
pub struct NaluHeader {
    pub type_: NaluType,
}

impl Header for NaluHeader {
    fn parse<T: AsRef<[u8]>>(cursor: &Cursor<T>) -> anyhow::Result<Self> {
        if !cursor.has_remaining() {
            return Err(anyhow!("Broken Data"));
        }

        let byte = cursor.chunk()[0];

        let type_ = NaluType::n(byte & 0x1f).ok_or(anyhow!("Broken Data"))?;

        if let NaluType::SliceExt = type_ {
            return Err(anyhow!("Stream contain unsupported/unimplemented NALs"));
        }

        Ok(NaluHeader { type_ })
    }

    fn is_end(&self) -> bool {
        matches!(self.type_, NaluType::SeqEnd | NaluType::StreamEnd)
    }

    fn len(&self) -> usize {
        1
    }
}
