// Copyright 2022 The ChromiumOS Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! An Annex B h.265 parser.
//!
//! Parses SPSs from NALUs.

use std::collections::BTreeMap;

use anyhow::anyhow;
use anyhow::Context;
use bitreader::BitReader;
use bytes::Buf;
use enumn::N;

use crate::cros_codecs::h264::nalu;
use crate::cros_codecs::h264::nalu::Header;
use crate::cros_codecs::h264::nalu_reader::NaluReader;

// Given the max SPS id.
const MAX_SPS_COUNT: usize = 16;

// 7.4.3.2.1:
// num_short_term_ref_pic_sets specifies the number of st_ref_pic_set( ) syntax
// structures included in the SPS. The value of num_short_term_ref_pic_sets
// shall be in the range of 0 to 64, inclusive.
// NOTE 5 – A decoder should allocate memory for a total number of
// num_short_term_ref_pic_sets + 1 st_ref_pic_set( ) syntax structures since
// there may be a st_ref_pic_set( ) syntax structure directly signalled in the
// slice headers of a current picture. A st_ref_pic_set( ) syntax structure
// directly signalled in the slice headers of a current picture has an index
// equal to num_short_term_ref_pic_sets.
const MAX_SHORT_TERM_REF_PIC_SETS: usize = 65;

// 7.4.3.2.1:
const MAX_LONG_TERM_REF_PIC_SETS: usize = 32;

// From table 7-5.
const DEFAULT_SCALING_LIST_0: [u8; 16] = [16; 16];

// From Table 7-6.
const DEFAULT_SCALING_LIST_1: [u8; 64] = [
    16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 17, 16, 17, 16, 17, 18, 17, 18, 18, 17, 18, 21, 19, 20,
    21, 20, 19, 21, 24, 22, 22, 24, 24, 22, 22, 24, 25, 25, 27, 30, 27, 25, 25, 29, 31, 35, 35, 31,
    29, 36, 41, 44, 41, 36, 47, 54, 54, 47, 65, 70, 65, 88, 88, 115,
];

// From Table 7-6.
const DEFAULT_SCALING_LIST_2: [u8; 64] = [
    16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 17, 17, 17, 17, 17, 18, 18, 18, 18, 18, 18, 20, 20, 20,
    20, 20, 20, 20, 24, 24, 24, 24, 24, 24, 24, 24, 25, 25, 25, 25, 25, 25, 25, 28, 28, 28, 28, 28,
    28, 33, 33, 33, 33, 33, 41, 41, 41, 41, 54, 54, 54, 71, 71, 91,
];

/// Table 7-1 – NAL unit type codes and NAL unit type classes
#[derive(N, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum NaluType {
    #[default]
    TrailN = 0,
    TrailR = 1,
    TsaN = 2,
    TsaR = 3,
    StsaN = 4,
    StsaR = 5,
    RadlN = 6,
    RadlR = 7,
    RaslN = 8,
    RaslR = 9,
    RsvVclN10 = 10,
    RsvVclR11 = 11,
    RsvVclN12 = 12,
    RsvVclR13 = 13,
    RsvVclN14 = 14,
    RsvVclR15 = 15,
    BlaWLp = 16,
    BlaWRadl = 17,
    BlaNLp = 18,
    IdrWRadl = 19,
    IdrNLp = 20,
    CraNut = 21,
    RsvIrapVcl22 = 22,
    RsvIrapVcl23 = 23,
    RsvVcl24 = 24,
    RsvVcl25 = 25,
    RsvVcl26 = 26,
    RsvVcl27 = 27,
    RsvVcl28 = 28,
    RsvVcl29 = 29,
    RsvVcl30 = 30,
    RsvVcl31 = 31,
    VpsNut = 32,
    SpsNut = 33,
    PpsNut = 34,
    AudNut = 35,
    EosNut = 36,
    EobNut = 37,
    FdNut = 38,
    PrefixSeiNut = 39,
    SuffixSeiNut = 40,
    RsvNvcl41 = 41,
    RsvNvcl42 = 42,
    RsvNvcl43 = 43,
    RsvNvcl44 = 44,
    RsvNvcl45 = 45,
    RsvNvcl46 = 46,
    RsvNvcl47 = 47,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NaluHeader {
    /// The NALU type.
    pub type_: NaluType,
    /// Specifies the identifier of the layer to which a VCL NAL unit belongs or
    /// the identifier of a layer to which a non-VCL NAL unit applies.
    pub nuh_layer_id: u8,
    /// Minus 1 specifies a temporal identifier for the NAL unit. The value of
    /// nuh_temporal_id_plus1 shall not be equal to 0.
    pub nuh_temporal_id_plus1: u8,
}

impl Header for NaluHeader {
    fn parse<T: AsRef<[u8]>>(cursor: &std::io::Cursor<T>) -> anyhow::Result<Self> {
        let data = &cursor.chunk()[0..2];
        let mut r = BitReader::new(data);

        // Skip forbidden_zero_bit
        r.skip(1)?;

        Ok(Self {
            type_: NaluType::n(r.read_u32(6)?).ok_or(anyhow!("Invalid NALU type"))?,
            nuh_layer_id: r.read_u8(6)?,
            nuh_temporal_id_plus1: r.read_u8(3)?,
        })
    }

    fn is_end(&self) -> bool {
        matches!(self.type_, NaluType::EosNut | NaluType::EobNut)
    }

    fn len(&self) -> usize {
        // 7.3.1.2
        2
    }
}

pub type Nalu<'a> = nalu::Nalu<'a, NaluHeader>;

/// H265 levels as defined by table A.8.
/// `general_level_idc` and `sub_layer_level_idc[ OpTid ]` shall be set equal to a
/// value of 30 times the level number specified in Table A.8
#[derive(N, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    #[default]
    L1 = 30,
    L2 = 60,
    L2_1 = 63,
    L3 = 90,
    L3_1 = 93,
    L4 = 120,
    L4_1 = 123,
    L5 = 150,
    L5_1 = 153,
    L5_2 = 156,
    L6 = 180,
    L6_1 = 183,
    L6_2 = 186,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProfileTierLevel {
    /// Specifies the context for the interpretation of general_profile_idc and
    /// `general_profile_compatibility_flag[ j ]` for all values of j in the range
    /// of 0 to 31, inclusive.
    pub general_profile_space: u8,
    /// Specifies the tier context for the interpretation of general_level_idc
    /// as specified in Annex A.
    pub general_tier_flag: bool,
    /// When general_profile_space is equal to 0, indicates a profile to which
    /// the CVS conforms as specified in Annex A. Bitstreams shall not contain
    /// values of general_profile_idc other than those specified in Annex A.
    /// Other values of general_profile_idc are reserved for future use by ITU-T
    /// | ISO/IEC.
    pub general_profile_idc: u8,
    /// `general_profile_compatibility_flag[ j ]` equal to true, when
    /// general_profile_space is false, indicates that the CVS conforms to the
    /// profile indicated by general_profile_idc equal to j as specified in
    /// Annex A.
    pub general_profile_compatibility_flag: [bool; 32],
    /// general_progressive_source_flag and general_interlaced_source_flag are
    /// interpreted as follows:
    ///
    /// –If general_progressive_source_flag is true and
    /// general_interlaced_source_flag is false, the source scan type of the
    /// pictures in the CVS should be interpreted as progressive only.
    ///
    /// –Otherwise, if general_progressive_source_flag is false and
    /// general_interlaced_source_flag is true, the source scan type of the
    /// pictures in the CVS should be interpreted as interlaced only.
    ///
    /// –Otherwise, if general_progressive_source_flag is false and
    /// general_interlaced_source_flag is false, the source scan type of the
    /// pictures in the CVS should be interpreted as unknown or unspecified.
    ///
    /// –Otherwise (general_progressive_source_flag is true and
    /// general_interlaced_source_flag is true), the source scan type of each
    /// picture in the CVS is indicated at the picture level using the syntax
    /// element source_scan_type in a picture timing SEI message.
    pub general_progressive_source_flag: bool,
    /// See `general_progressive_source_flag`.
    pub general_interlaced_source_flag: bool,
    /// If true, specifies that there are no frame packing arrangement SEI
    /// messages, segmented rectangular frame packing arrangement SEI messages,
    /// equirectangular projection SEI messages, or cubemap projection SEI
    /// messages present in the CVS. If false, indicates that there may or may
    /// not be one or more frame packing arrangement SEI messages, segmented
    /// rectangular frame packing arrangement SEI messages, equirectangular
    /// projection SEI messages, or cubemap projection SEI messages present in
    /// the CVS.
    pub general_non_packed_constraint_flag: bool,
    /// When true, specifies that field_seq_flag is false. When false, indicates
    /// that field_seq_flag may or may not be false.
    pub general_frame_only_constraint_flag: bool,
    /// See Annex A.
    pub general_max_12bit_constraint_flag: bool,
    /// See Annex A.
    pub general_max_10bit_constraint_flag: bool,
    /// See Annex A.
    pub general_max_8bit_constraint_flag: bool,
    /// See Annex A.
    pub general_max_422chroma_constraint_flag: bool,
    /// See Annex A.
    pub general_max_420chroma_constraint_flag: bool,
    /// See Annex A.
    pub general_max_monochrome_constraint_flag: bool,
    /// See Annex A.
    pub general_intra_constraint_flag: bool,
    /// See Annex A.
    pub general_lower_bit_rate_constraint_flag: bool,
    /// See Annex A.
    pub general_max_14bit_constraint_flag: bool,
    /// See Annex A.
    pub general_one_picture_only_constraint_flag: bool,
    /// When true, specifies that the INBLD capability as specified in Annex F
    /// is required for decoding of the layer to which the profile_tier_level( )
    /// syntax structure applies. When false, specifies that the INBLD
    /// capability as specified in Annex F is not required for decoding of the
    /// layer to which the profile_tier_level( ) syntax structure applies.
    pub general_inbld_flag: bool,
    /// Indicates a level to which the CVS conforms as specified in Annex A.
    pub general_level_idc: Level,
    /// Sub-layer syntax element.
    pub sub_layer_profile_present_flag: [bool; 6],
    /// Sub-layer syntax element.
    pub sub_layer_level_present_flag: [bool; 6],
    /// Sub-layer syntax element.
    pub sub_layer_profile_space: [u8; 6],
    /// Sub-layer syntax element.
    pub sub_layer_tier_flag: [bool; 6],
    /// Sub-layer syntax element.
    pub sub_layer_profile_idc: [u8; 6],
    /// Sub-layer syntax element.
    pub sub_layer_profile_compatibility_flag: [[bool; 32]; 6],
    /// Sub-layer syntax element.
    pub sub_layer_progressive_source_flag: [bool; 6],
    /// Sub-layer syntax element.
    pub sub_layer_interlaced_source_flag: [bool; 6],
    /// Sub-layer syntax element.
    pub sub_layer_non_packed_constraint_flag: [bool; 6],
    /// Sub-layer syntax element.
    pub sub_layer_frame_only_constraint_flag: [bool; 6],
    /// Sub-layer syntax element.
    pub sub_layer_max_12bit_constraint_flag: [bool; 6],
    /// Sub-layer syntax element.
    pub sub_layer_max_10bit_constraint_flag: [bool; 6],
    /// Sub-layer syntax element.
    pub sub_layer_max_8bit_constraint_flag: [bool; 6],
    /// Sub-layer syntax element.
    pub sub_layer_max_422chroma_constraint_flag: [bool; 6],
    /// Sub-layer syntax element.
    pub sub_layer_max_420chroma_constraint_flag: [bool; 6],
    /// Sub-layer syntax element.
    pub sub_layer_max_monochrome_constraint_flag: [bool; 6],
    /// Sub-layer syntax element.
    pub sub_layer_intra_constraint_flag: [bool; 6],
    /// Sub-layer syntax element.
    pub sub_layer_one_picture_only_constraint_flag: [bool; 6],
    /// Sub-layer syntax element.
    pub sub_layer_lower_bit_rate_constraint_flag: [bool; 6],
    /// Sub-layer syntax element.
    pub sub_layer_max_14bit_constraint_flag: [bool; 6],
    /// Sub-layer syntax element.
    pub sub_layer_inbld_flag: [bool; 6],
    /// Sub-layer syntax element.
    pub sub_layer_level_idc: [Level; 6],
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpsRangeExtension {
    pub transform_skip_rotation_enabled_flag: bool,
    pub transform_skip_context_enabled_flag: bool,
    pub implicit_rdpcm_enabled_flag: bool,
    pub explicit_rdpcm_enabled_flag: bool,
    pub extended_precision_processing_flag: bool,
    pub intra_smoothing_disabled_flag: bool,
    pub high_precision_offsets_enabled_flag: bool,
    pub persistent_rice_adaptation_enabled_flag: bool,
    pub cabac_bypass_alignment_enabled_flag: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpsSccExtension {
    /// When set, specifies that a picture in the CVS may be included in a
    /// reference picture list of a slice of the picture itself.  When not set,
    /// specifies that a picture in the CVS is never included in a reference
    /// picture list of a slice of the picture itself.
    pub curr_pic_ref_enabled_flag: bool,
    /// When set, specifies that the decoding process for palette mode may be
    /// used for intra blocks. When not set, specifies that the decoding process
    /// for palette mode is not applied.
    pub palette_mode_enabled_flag: bool,
    /// Specifies the maximum allowed palette size.
    pub palette_max_size: u8,
    /// Specifies the difference between the maximum allowed palette predictor
    /// size and the maximum allowed palette size.
    pub delta_palette_max_predictor_size: u8,
    /// When set, specifies that the sequence palette predictors are initialized
    /// using the sps_palette_predictor_initializers. When not set, specifies
    /// that the entries in the sequence palette predictor are initialized to 0.
    pub palette_predictor_initializers_present_flag: bool,
    /// num_palette_predictor_initializers_minus1 plus 1 specifies the number of
    /// entries in the sequence palette predictor initializer.
    pub num_palette_predictor_initializer_minus1: u8,
    /// `palette_predictor_initializer[ comp ][ i ]` specifies the value of the
    /// comp-th component of the i-th palette entry in the SPS that is used to
    /// initialize the array PredictorPaletteEntries.
    pub palette_predictor_initializer: [[u32; 128]; 3],
    /// Controls the presence and inference of the use_integer_mv_flag that
    /// specifies the resolution of motion vectors for inter prediction.
    pub motion_vector_resolution_control_idc: u8,
    /// When set, specifies that the intra boundary filtering process is
    /// unconditionally disabled for intra prediction.  If not set, specifies
    /// that the intra boundary filtering process may be used.
    pub intra_boundary_filtering_disabled_flag: bool,
}

impl Default for SpsSccExtension {
    fn default() -> Self {
        Self {
            curr_pic_ref_enabled_flag: Default::default(),
            palette_mode_enabled_flag: Default::default(),
            palette_max_size: Default::default(),
            delta_palette_max_predictor_size: Default::default(),
            palette_predictor_initializers_present_flag: Default::default(),
            num_palette_predictor_initializer_minus1: Default::default(),
            palette_predictor_initializer: [[0; 128]; 3],
            motion_vector_resolution_control_idc: Default::default(),
            intra_boundary_filtering_disabled_flag: Default::default(),
        }
    }
}

/// A H.265 Sequence Parameter Set.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sps {
    /// Specifies the value of the vps_video_parameter_set_id of the active VPS.
    pub video_parameter_set_id: u8,
    /// `max_sub_layers_minus1` plus 1 specifies the maximum number of temporal
    /// sub-layers that may be present in each CVS referring to the SPS.
    pub max_sub_layers_minus1: u8,
    /// When sps_max_sub_layers_minus1 is greater than 0, specifies whether
    /// inter prediction is additionally restricted for CVSs referring to the
    /// SPS.
    pub temporal_id_nesting_flag: bool,
    /// profile_tier_level() data.
    pub profile_tier_level: ProfileTierLevel,
    /// Provides an identifier for the SPS for reference by other syntax
    /// elements.
    pub seq_parameter_set_id: u8,
    /// Specifies the chroma sampling relative to the luma sampling as specified
    /// in clause 6.2.
    pub chroma_format_idc: u8,
    /// When true, specifies that the three colour components of the 4:4:4
    /// chroma format are coded separately. When false, specifies that the
    /// colour components are not coded separately.
    pub separate_colour_plane_flag: bool,
    /// Specifies the width of each decoded picture in units of luma samples.
    pub pic_width_in_luma_samples: u16,
    /// Specifies the height of each decoded picture in units of luma samples.
    pub pic_height_in_luma_samples: u16,
    /// When true, indicates that the conformance cropping window offset
    /// parameters follow next in the SPS. When false, indicates that the
    /// conformance cropping window offset parameters are not present.
    pub conformance_window_flag: bool,
    /* if conformance_window_flag */
    /// Specify the samples of the pictures in the CVS that are output from the
    /// decoding process, in terms of a rectangular region specified in picture
    /// coordinates for output.
    pub conf_win_left_offset: u32,
    pub conf_win_right_offset: u32,
    pub conf_win_top_offset: u32,
    pub conf_win_bottom_offset: u32,

    /// Specifies the bit depth of the samples of the luma array BitDepthY and
    /// the value of the luma quantization parameter range offset QpBdOffsetY.
    pub bit_depth_luma_minus8: u8,
    /// Specifies the bit depth of the samples of the chroma arrays BitDepthC
    /// and the value of the chroma quantization parameter range offset
    /// QpBdOffsetC.
    pub bit_depth_chroma_minus8: u8,
    /// Specifies the value of the variable MaxPicOrderCntLsb that is used in
    /// the decoding process for picture order count.
    pub log2_max_pic_order_cnt_lsb_minus4: u8,
    /// When true, specifies that `max_dec_pic_buffering_minus1[ i ]`,
    /// `max_num_reorder_pics[ i ]` and `max_latency_increase_plus1[ i ]` are
    /// present for max_sub_layers_minus1 + 1 sub- layers. When false, specifies
    /// that the values of `max_dec_pic_ buffering_minus1[ max_sub_layers_minus1
    /// ]`, `max_num_reorder_pics[ max_sub_layers_minus1 ]` and max_
    /// `latency_increase_plus1[ max_sub_layers_minus1 ]` apply to all sub-layers.
    pub sub_layer_ordering_info_present_flag: bool,
    /// `max_dec_pic_buffering_minus1[ i ]` plus 1 specifies the maximum required
    /// size of the decoded picture buffer for the CVS in units of picture
    /// storage buffers when HighestTid is equal to i.
    pub max_dec_pic_buffering_minus1: [u8; 7],
    /// `max_num_reorder_pics[ i ]` indicates the maximum allowed number of
    /// pictures with PicOutputFlag equal to 1 that can precede any picture with
    /// PicOutputFlag equal to 1 in the CVS in decoding order and follow that
    /// picture with PicOutputFlag equal to 1 in output order when HighestTid is
    /// equal to i.
    pub max_num_reorder_pics: [u8; 7],
    /// `max_latency_increase_plus1[ i ]` not equal to 0 is used to compute the
    /// value of `SpsMaxLatencyPictures[ i ]`, which specifies the maximum number
    /// of pictures with PicOutputFlag equal to 1 that can precede any picture
    /// with PicOutputFlag equal to 1 in the CVS in output order and follow that
    /// picture with PicOutputFlag equal to 1 in decoding order when HighestTid
    /// is equal to i.
    pub max_latency_increase_plus1: [u8; 7],
    /// min_luma_coding_block_size_minus3 plus 3 specifies the minimum luma
    /// coding block size.
    pub log2_min_luma_coding_block_size_minus3: u8,
    /// Specifies the difference between the maximum and minimum luma coding
    /// block size.
    pub log2_diff_max_min_luma_coding_block_size: u8,
    /// min_luma_transform_block_size_minus2 plus 2 specifies the minimum luma
    /// transform block size.
    pub log2_min_luma_transform_block_size_minus2: u8,
    /// Specifies the difference between the maximum and minimum luma transform
    /// block size.
    pub log2_diff_max_min_luma_transform_block_size: u8,
    /// Specifies the maximum hierarchy depth for transform units of coding
    /// units coded in inter prediction mode.
    pub max_transform_hierarchy_depth_inter: u8,
    /// Specifies the maximum hierarchy depth for transform units of coding
    /// units coded in intra prediction mode.
    pub max_transform_hierarchy_depth_intra: u8,
    /// When true, specifies that a scaling list is used for the scaling process
    /// for transform coefficients. When false, specifies that scaling list is
    /// not used for the scaling process for transform coefficients.
    pub scaling_list_enabled_flag: bool,
    /* if scaling_list_enabled_flag */
    /// When true, specifies that the scaling_list_data( ) syntax structure is
    /// present in the SPS. When false, specifies that the scaling_list_data( )
    /// syntax structure is not present in the SPS.
    pub scaling_list_data_present_flag: bool,
    /// The scaling_list_data() syntax data.
    pub scaling_list: ScalingLists,
    /// When true, specifies that asymmetric motion partitions, i.e., PartMode
    /// equal to PART_2NxnU, PART_2NxnD, PART_nLx2N or PART_nRx2N, may be used
    /// in CTBs. When false, specifies that asymmetric motion partitions cannot
    /// be used in CTBs.
    pub amp_enabled_flag: bool,
    /// When true, specifies that the sample adaptive offset process is applied
    /// to the reconstructed picture after the deblocking filter process.  When
    /// false, specifies that the sample adaptive offset process is not applied
    /// to the reconstructed picture after the deblocking filter process.
    pub sample_adaptive_offset_enabled_flag: bool,
    /// When false, specifies that PCM-related syntax
    /// (pcm_sample_bit_depth_luma_minus1, pcm_sample_ bit_depth_chroma_minus1,
    /// log2_min_pcm_luma_coding_block_size_minus3, log2_diff_max_min_pcm_luma_
    /// coding_block_size, pcm_loop_filter_disabled_flag, pcm_flag,
    /// pcm_alignment_zero_bit syntax elements and pcm_sample( ) syntax
    /// structure) is not present in the CVS.
    pub pcm_enabled_flag: bool,

    /* if pcm_enabled_flag */
    pub pcm_sample_bit_depth_luma_minus1: u8,
    /// Specifies the number of bits used to represent each of PCM sample values
    /// of the luma component.
    pub pcm_sample_bit_depth_chroma_minus1: u8,
    /// Specifies the number of bits used to represent each of PCM sample values
    /// of the chroma components.
    pub log2_min_pcm_luma_coding_block_size_minus3: u8,
    /// Specifies the difference between the maximum and minimum size of coding
    /// blocks with pcm_flag equal to true.
    pub log2_diff_max_min_pcm_luma_coding_block_size: u8,
    /// Specifies whether the loop filter process is disabled on reconstructed
    /// samples in a coding unit with pcm_flag equal to true as follows:
    ///
    /// – If pcm_loop_filter_disabled_flag is set, the deblocking filter and
    /// sample adaptive offset filter processes on the reconstructed samples in
    /// a coding unit with pcm_flag set are disabled.
    ///
    /// – Otherwise (pcm_loop_filter_disabled_flag value is not set), the
    /// deblocking filter and sample adaptive offset filter processes on the
    /// reconstructed samples in a coding unit with pcm_flag set are not
    /// disabled.
    pub pcm_loop_filter_disabled_flag: bool,
    /// Specifies the number of st_ref_pic_set( ) syntax structures included in
    /// the SPS.
    pub num_short_term_ref_pic_sets: u8,
    /// the st_ref_pic_set() data.
    pub short_term_ref_pic_set: Vec<ShortTermRefPicSet>,
    /// If unset, specifies that no long-term reference picture is used for
    /// inter prediction of any coded picture in the CVS.
    /// If set, specifies that long-term reference pictures may be used for
    /// inter prediction of one or more coded pictures in the CVS.
    pub long_term_ref_pics_present_flag: bool,

    /* if long_term_ref_pics_present_flag */
    /// Specifies the number of candidate long-term reference pictures that are
    /// specified in the SPS.
    pub num_long_term_ref_pics_sps: u8,
    /// `lt_ref_pic_poc_lsb_sps[ i ]` specifies the picture order count modulo
    /// MaxPicOrderCntLsb of the i-th candidate long-term reference picture
    /// specified in the SPS.
    pub lt_ref_pic_poc_lsb_sps: [u32; MAX_LONG_TERM_REF_PIC_SETS],
    /// `used_by_curr_pic_lt_sps_flag[ i ]` equal to false specifies that the i-th
    /// candidate long-term reference picture specified in the SPS is not used
    /// for reference by a picture that includes in its long-term reference
    /// picture set (RPS) the i-th candidate long-term reference picture
    /// specified in the SPS.
    pub used_by_curr_pic_lt_sps_flag: [bool; MAX_LONG_TERM_REF_PIC_SETS],
    /// When set, specifies that slice_temporal_mvp_enabled_flag is present in
    /// the slice headers of non-IDR pictures in the CVS. When not set,
    /// specifies that slice_temporal_mvp_enabled_flag is not present in slice
    /// headers and that temporal motion vector predictors are not used in the
    /// CVS.
    pub temporal_mvp_enabled_flag: bool,
    /// When set, specifies that bi-linear interpolation is conditionally used
    /// in the intraprediction filtering process in the CVS as specified in
    /// clause 8.4.4.2.3.
    pub strong_intra_smoothing_enabled_flag: bool,
    /// When set, specifies that the vui_parameters( ) syntax structure as
    /// specified in Annex E is present. When not set, specifies that the
    /// vui_parameters( ) syntax structure as specified in Annex E is not
    /// present.
    pub vui_parameters_present_flag: bool,
    /// The vui_parameters() data.
    pub vui_parameters: VuiParams,
    /// When set, specifies that the syntax elements sps_range_extension_flag,
    /// sps_multilayer_extension_flag, sps_3d_extension_flag,
    /// sps_scc_extension_flag, and sps_extension_4bits are present in the SPS
    /// RBSP syntax structure. When not set, specifies that these syntax
    /// elements are not present.
    pub extension_present_flag: bool,

    pub range_extension_flag: bool,
    /// The sps_range_extension() data.
    pub range_extension: SpsRangeExtension,
    /// When set, specifies that the sps_scc_extension( ) syntax structure is
    /// present in the SPS RBSP syntax structure. When not set, specifies that
    /// this syntax structure is not present
    pub scc_extension_flag: bool,
    /// The sps_scc_extension() data.
    pub scc_extension: SpsSccExtension,

    // Internal H265 variables. Computed from the bitstream.
    /// Equivalent to MinCbLog2SizeY in the specification.
    pub min_cb_log2_size_y: u32,
    /// Equivalent to CtbLog2SizeY in the specification.
    pub ctb_log2_size_y: u32,
    /// Equivalent to CtbSizeY in the specification.
    pub ctb_size_y: u32,
    /// Equivalent to PicHeightInCtbsY in the specification.
    pub pic_height_in_ctbs_y: u32,
    /// Equivalent to PicWidthInCtbsY in the specification.
    pub pic_width_in_ctbs_y: u32,
    /// Equivalent to PicSizeInCtbsY in the specification.
    pub pic_size_in_ctbs_y: u32,
    /// Equivalent to ChromaArrayType in the specification.
    pub chroma_array_type: u8,
    /// Equivalent to WpOffsetHalfRangeY in the specification.
    pub wp_offset_half_range_y: u32,
    /// Equivalent to WpOffsetHalfRangeC in the specification.
    pub wp_offset_half_range_c: u32,
    /// Equivalent to MaxTbLog2SizeY in the specification.
    pub max_tb_log2_size_y: u32,
    /// Equivalent to PicSizeInSamplesY in the specification.
    pub pic_size_in_samples_y: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PpsSccExtension {
    /// When set, specifies that a picture referring to the PPS may be included
    /// in a reference picture list of a slice of the picture itself.  If not
    /// set, specifies that a picture referring to the PPS is never included in
    /// a reference picture list of a slice of the picture itself.
    pub curr_pic_ref_enabled_flag: bool,
    /// When set, specifies that an adaptive colour transform may be applied to
    /// the residual in the decoding process. When not set, specifies that
    /// adaptive colour transform is not applied to the residual.
    pub residual_adaptive_colour_transform_enabled_flag: bool,
    /// When set, specifies that slice_act_y_qp_offset, slice_act_cb_qp_offset,
    /// slice_act_cr_qp_offset are present in the slice header.  When not set,
    /// specifies that slice_act_y_qp_offset, slice_act_cb_qp_offset,
    /// slice_act_cr_qp_offset are not present in the slice header.
    pub slice_act_qp_offsets_present_flag: bool,
    /// See the specificartion for more details.
    pub act_y_qp_offset_plus5: i8,
    /// See the specificartion for more details.
    pub act_cb_qp_offset_plus5: i8,
    /// See the specificartion for more details.
    pub act_cr_qp_offset_plus3: i8,
    /// When set, specifies that the palette predictor initializers used for the
    /// pictures referring to the PPS are derived based on the palette predictor
    /// initializers specified by the PPS. If not set, specifies that the
    /// palette predictor initializers used for the pictures referring to the
    /// PPS are inferred to be equal to those specified by the active SPS.
    pub palette_predictor_initializers_present_flag: bool,
    /// Specifies the number of entries in the picture palette predictor
    /// initializer.
    pub num_palette_predictor_initializers: u8,
    /// When set, specifies that the pictures that refer to this PPS are
    /// monochrome. If not set, specifies that the pictures that refer to this
    /// PPS have multiple components.
    pub monochrome_palette_flag: bool,
    /// luma_bit_depth_entry_minus8 plus 8 specifies the bit depth of the luma
    /// component of the entries of the palette predictor initializer.
    pub luma_bit_depth_entry_minus8: u8,
    /// chroma_bit_depth_entry_minus8 plus 8 specifies the bit depth of the
    /// chroma components of the entries of the palette predictor initializer.
    pub chroma_bit_depth_entry_minus8: u8,
    /// `pps_palette_predictor_initializer[ comp ][ i ]` specifies the value of
    /// the comp-th component of the i-th palette entry in the PPS that is used
    /// to initialize the array PredictorPaletteEntries.
    pub palette_predictor_initializer: [[u8; 128]; 3],
}

impl Default for PpsSccExtension {
    fn default() -> Self {
        Self {
            curr_pic_ref_enabled_flag: Default::default(),
            residual_adaptive_colour_transform_enabled_flag: Default::default(),
            slice_act_qp_offsets_present_flag: Default::default(),
            act_y_qp_offset_plus5: Default::default(),
            act_cb_qp_offset_plus5: Default::default(),
            act_cr_qp_offset_plus3: Default::default(),
            palette_predictor_initializers_present_flag: Default::default(),
            num_palette_predictor_initializers: Default::default(),
            monochrome_palette_flag: Default::default(),
            luma_bit_depth_entry_minus8: Default::default(),
            chroma_bit_depth_entry_minus8: Default::default(),
            palette_predictor_initializer: [[0; 128]; 3],
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PpsRangeExtension {
    /// log2_max_transform_skip_block_size_minus2 plus 2 specifies the maximum
    /// transform block size for which transform_skip_flag may be present in
    /// coded pictures referring to the PPS. When not present, the value of
    /// log2_max_transform_skip_block_size_minus2 is inferred to be equal to 0.
    /// When present, the value of log2_max_transform_skip_block_size_minus2
    /// shall be less than or equal to MaxTbLog2SizeY − 2.
    pub log2_max_transform_skip_block_size_minus2: u32,
    /// When set, specifies that log2_res_scale_abs_plus1 and
    /// res_scale_sign_flag may be present in the transform unit syntax for
    /// pictures referring to the PPS. When not set, specifies that
    /// log2_res_scale_abs_plus1 and res_scale_sign_flag are not present for
    /// pictures referring to the PPS.
    pub cross_component_prediction_enabled_flag: bool,
    /// When set, specifies that the cu_chroma_qp_offset_flag may be present in
    /// the transform unit syntax. When not set, specifies that the
    /// cu_chroma_qp_offset_flag is not present in the transform unit syntax.
    pub chroma_qp_offset_list_enabled_flag: bool,
    /// Specifies the difference between the luma CTB size and the minimum luma
    /// coding block size of coding units that convey cu_chroma_qp_offset_flag.
    pub diff_cu_chroma_qp_offset_depth: u32,
    /// chroma_qp_offset_list_len_minus1 plus 1 specifies the number of
    /// `cb_qp_offset_list[ i ]` and `cr_qp_offset_list[ i ]` syntax elements that
    /// are present in the PPS.
    pub chroma_qp_offset_list_len_minus1: u32,
    /// Specify offsets used in the derivation of Qp′Cb and Qp′Cr, respectively.
    pub cb_qp_offset_list: [i32; 6],
    /// Specify offsets used in the derivation of Qp′Cb and Qp′Cr, respectively.
    pub cr_qp_offset_list: [i32; 6],
    /// The base 2 logarithm of the scaling parameter that is used to scale
    /// sample adaptive offset (SAO) offset values for luma samples.
    pub log2_sao_offset_scale_luma: u32,
    /// The base 2 logarithm of the scaling parameter that is used to scale SAO
    /// offset values for chroma samples.
    pub log2_sao_offset_scale_chroma: u32,
}

/// A H.265 Picture Parameter Set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pps {
    /// Identifies the PPS for reference by other syntax elements.
    pub pic_parameter_set_id: u8,
    /// Specifies the value of sps_seq_parameter_set_id for the active SPS.
    pub seq_parameter_set_id: u8,
    /// When set, specifies the presence of the syntax element
    /// dependent_slice_segment_flag in the slice segment headers for coded
    /// pictures referring to the PPS. When not set, specifies the absence of
    /// the syntax element dependent_slice_segment_flag in the slice segment
    /// headers for coded pictures referring to the PPS.
    pub dependent_slice_segments_enabled_flag: bool,
    /// When set, indicates that the pic_output_flag syntax element is present
    /// in the associated slice headers. When not set, indicates that the
    /// pic_output_flag syntax element is not present in the associated slice
    /// headers.
    pub output_flag_present_flag: bool,
    /// Specifies the number of extra slice header bits that are present in the
    /// slice header RBSP for coded pictures referring to the PPS.
    pub num_extra_slice_header_bits: u8,
    /// When not set, specifies that sign bit hiding is disabled. Whens set,
    /// specifies that sign bit hiding is enabled.
    pub sign_data_hiding_enabled_flag: bool,
    /// When set, specifies that cabac_init_flag is present in slice headers
    /// referring to the PPS. When not set, specifies that cabac_init_flag is
    /// not present in slice headers referring to the PPS.
    pub cabac_init_present_flag: bool,
    /// Specifies the inferred value of num_ref_idx_l0_active_minus1 for P and B
    /// slices with num_ref_idx_active_override_flag not set.
    pub num_ref_idx_l0_default_active_minus1: u8,
    /// Specifies the inferred value of num_ref_idx_l1_active_minus1 for B
    /// slices with num_ref_idx_active_override_flag not set.
    pub num_ref_idx_l1_default_active_minus1: u8,
    /// init_qp_minus26 plus 26 specifies the initial value of SliceQpY for each
    /// slice referring to the PPS. The initial value of SliceQpY is modified at
    /// the slice segment layer when a non-zero value of slice_qp_delta is
    /// decoded.
    pub init_qp_minus26: i8,
    /// When not set, specifies that intra prediction allows usage of residual
    /// data and decoded samples of neighbouring coding blocks coded using
    /// either intra or inter prediction modes. When set, specifies constrained
    /// intra prediction, in which case intra prediction only uses residual data
    /// and decoded samples from neighbouring coding blocks coded using intra
    /// prediction modes.
    pub constrained_intra_pred_flag: bool,
    /// When set, specifies that transform_skip_flag may be present in the
    /// residual coding syntax. When not set, specifies that transform_skip_flag
    /// is not present in the residual coding syntax.
    pub transform_skip_enabled_flag: bool,
    /// When set, specifies that the diff_cu_qp_delta_depth syntax element is
    /// present in the PPS and that cu_qp_delta_abs may be present in the
    /// transform unit syntax and the palette syntax. When not set, specifies
    /// that the diff_cu_qp_delta_depth syntax element is not present in the PPS
    /// and that cu_qp_delta_abs is not present in the transform unit syntax and
    /// the palette syntax.
    pub cu_qp_delta_enabled_flag: bool,

    /*if cu_qp_delta_enabled_flag */
    /// Specifies the difference between the luma CTB size and the minimum luma
    /// coding block size of coding units that convey cu_qp_delta_abs and
    /// cu_qp_delta_sign_flag.
    pub diff_cu_qp_delta_depth: u8,
    /// Specifies the offsets to the luma quantization parameter Qp′Y used for
    /// deriving Qp′Cb and Qp′Cr, respectively.
    pub cb_qp_offset: i8,
    /// Specifies the offsets to the luma quantization parameter Qp′Y used for
    /// deriving Qp′Cb and Qp′Cr, respectively.
    pub cr_qp_offset: i8,
    /// When set, indicates that the slice_cb_qp_offset and slice_cr_qp_offset
    /// syntax elements are present in the associated slice headers.  When not
    /// set, indicates that these syntax elements are not present in the
    /// associated slice headers. When ChromaArrayType is equal to 0,
    /// pps_slice_chroma_qp_offsets_present_flag shall be equal to 0
    pub slice_chroma_qp_offsets_present_flag: bool,
    /// When not set, specifies that weighted prediction is not applied to P
    /// slices. When set, specifies that weighted prediction is applied to P
    /// slices.
    pub weighted_pred_flag: bool,
    /// When not set, specifies that the default weighted prediction is applied
    /// to B slices. When set, specifies that weighted prediction is applied to
    /// B slices.
    pub weighted_bipred_flag: bool,
    /// When set, specifies that `cu_transquant_bypass_flag` is present, When
    /// not set, specifies that `cu_transquant_bypass_flag` is not present.
    pub transquant_bypass_enabled_flag: bool,
    /// When set, specifies that there is more than one tile in each picture
    /// referring to the PPS. When not set, specifies that there is only one
    /// tile in each picture referring to the PPS.
    pub tiles_enabled_flag: bool,
    /// When set, specifies that a specific synchronization process for context
    /// variables, and when applicable, Rice parameter initialization states and
    /// palette predictor variables, is invoked before decoding the CTU which
    /// includes the first CTB of a row of CTBs in each tile in each picture
    /// referring to the PPS, and a specific storage process for context
    /// variables, and when applicable, Rice parameter initialization states and
    /// palette predictor variables, is invoked after decoding the CTU which
    /// includes the second CTB of a row of CTBs in each tile in each picture
    /// referring to the PPS. When not set, specifies that no specific
    /// synchronization process for context variables, and when applicable, Rice
    /// parameter initialization states and palette predictor variables, is
    /// required to be invoked before decoding the CTU which includes the first
    /// CTB of a row of CTBs in each tile in each picture referring to the PPS,
    /// and no specific storage process for context variables, and when
    /// applicable, Rice parameter initialization states and palette predictor
    /// variables, is required to be invoked after decoding the CTU which
    /// includes the second CTB of a row of CTBs in each tile in each picture
    /// referring to the PPS.
    pub entropy_coding_sync_enabled_flag: bool,
    /// num_tile_columns_minus1 plus 1 specifies the number of tile columns
    /// partitioning the picture.
    pub num_tile_columns_minus1: u8,
    /// num_tile_rows_minus1 plus 1 specifies the number of tile rows
    /// partitioning the picture.
    pub num_tile_rows_minus1: u8,
    /// When set, specifies that tile column boundaries and likewise tile row
    /// boundaries are distributed uniformly across the picture.  When not set,
    /// specifies that tile column boundaries and likewise tile row boundaries
    /// are not distributed uniformly across the picture but signalled
    /// explicitly using the syntax elements `column_width_minus1[ i ]` and
    /// `row_height_minus1[ i ]`.
    pub uniform_spacing_flag: bool,
    /// `column_width_minus1[ i ]` plus 1 specifies the width of the i-th tile
    /// column in units of CTBs.
    pub column_width_minus1: [u32; 19],
    /// `row_height_minus1[ i ]` plus 1 specifies the height of the i-th tile row
    /// in units of CTBs.
    pub row_height_minus1: [u32; 21],
    /// When set, specifies that in-loop filtering operations may be performed
    /// across tile boundaries in pictures referring to the PPS.  When not set,
    /// specifies that in-loop filtering operations are not performed across
    /// tile boundaries in pictures referring to the PPS. The in-loop filtering
    /// operations include the deblocking filter and sample adaptive offset
    /// filter operations.
    pub loop_filter_across_tiles_enabled_flag: bool,
    /// When set, specifies that in-loop filtering operations may be performed
    /// across left and upper boundaries of slices referring to the PPS.  When
    /// not set, specifies that in-loop filtering operations are not performed
    /// across left and upper boundaries of slices referring to the PPS. The in-
    /// loop filtering operations include the deblocking filter and sample
    /// adaptive offset filter operations.
    pub loop_filter_across_slices_enabled_flag: bool,
    /// When set, specifies the presence of deblocking filter control syntax
    /// elements in the PPS. When not set, specifies the absence of deblocking
    /// filter control syntax elements in the PPS.
    pub deblocking_filter_control_present_flag: bool,
    /// When set, specifies the presence of deblocking_filter_override_flag in
    /// the slice headers for pictures referring to the PPS.  When not set,
    /// specifies the absence of deblocking_filter_override_flag in the slice
    /// headers for pictures referring to the PPS.
    pub deblocking_filter_override_enabled_flag: bool,
    /// When set, specifies that the operation of deblocking filter is not
    /// applied for slices referring to the PPS in which
    /// slice_deblocking_filter_disabled_flag is not present.  When not set,
    /// specifies that the operation of the deblocking filter is applied for
    /// slices referring to the PPS in which
    /// slice_deblocking_filter_disabled_flag is not present.
    pub deblocking_filter_disabled_flag: bool,
    /// Specify the default deblocking parameter offsets for β and tC (divided
    /// by 2) that are applied for slices referring to the PPS, unless the
    /// default deblocking parameter offsets are overridden by the deblocking
    /// parameter offsets present in the slice headers of the slices referring
    /// to the PPS.
    pub beta_offset_div2: i8,
    /// Specify the default deblocking parameter offsets for β and tC (divided
    /// by 2) that are applied for slices referring to the PPS, unless the
    /// default deblocking parameter offsets are overridden by the deblocking
    /// parameter offsets present in the slice headers of the slices referring
    /// to the PPS.
    pub tc_offset_div2: i8,
    /// When set, specifies that the scaling list data used for the pictures
    /// referring to the PPS are derived based on the scaling lists specified by
    /// the active SPS and the scaling lists specified by the PPS.
    /// pps_scaling_list_data_present_flag equal to 0 specifies that the scaling
    /// list data used for the pictures referring to the PPS are inferred to be
    /// equal to those specified by the active SPS.
    pub scaling_list_data_present_flag: bool,
    /// The scaling list data.
    pub scaling_list: ScalingLists,
    /// When set, specifies that the syntax structure
    /// ref_pic_lists_modification( ) is present in the slice segment header.
    /// When not set, specifies that the syntax structure
    /// ref_pic_lists_modification( ) is not present in the slice segment header
    pub lists_modification_present_flag: bool,
    /// log2_parallel_merge_level_minus2 plus 2 specifies the value of the
    /// variable Log2ParMrgLevel, which is used in the derivation process for
    /// luma motion vectors for merge mode as specified in clause 8.5.3.2.2 and
    /// the derivation process for spatial merging candidates as specified in
    /// clause 8.5.3.2.3.
    pub log2_parallel_merge_level_minus2: u8,
    /// When not set, specifies that no slice segment header extension syntax
    /// elements are present in the slice segment headers for coded pictures
    /// referring to the PPS. When set, specifies that slice segment header
    /// extension syntax elements are present in the slice segment headers for
    /// coded pictures referring to the PPS.
    pub slice_segment_header_extension_present_flag: bool,
    /// When set, specifies that the syntax elements pps_range_extension_flag,
    /// pps_multilayer_extension_flag, pps_3d_extension_flag,
    /// pps_scc_extension_flag, and pps_extension_4bits are present in the
    /// picture parameter set RBSP syntax structure. When not set, specifies
    /// that these syntax elements are not present.
    pub extension_present_flag: bool,
    /// When setspecifies that the pps_range_extension( ) syntax structure is
    /// present in the PPS RBSP syntax structure. When not set, specifies that
    /// this syntax structure is not present.
    pub range_extension_flag: bool,
    /// The range extension data.
    pub range_extension: PpsRangeExtension,

    pub scc_extension_flag: bool,
    /// The SCC extension data.
    pub scc_extension: PpsSccExtension,

    // Internal variables.
    /// Equivalent to QpBdOffsetY in the specification.
    pub qp_bd_offset_y: u32,

    /// The nuh_temporal_id_plus1 - 1 of the associated NALU.
    pub temporal_id: u8,
}

impl Default for Pps {
    fn default() -> Self {
        Self {
            pic_parameter_set_id: Default::default(),
            seq_parameter_set_id: Default::default(),
            dependent_slice_segments_enabled_flag: Default::default(),
            output_flag_present_flag: Default::default(),
            num_extra_slice_header_bits: Default::default(),
            sign_data_hiding_enabled_flag: Default::default(),
            cabac_init_present_flag: Default::default(),
            num_ref_idx_l0_default_active_minus1: Default::default(),
            num_ref_idx_l1_default_active_minus1: Default::default(),
            init_qp_minus26: Default::default(),
            constrained_intra_pred_flag: Default::default(),
            transform_skip_enabled_flag: Default::default(),
            cu_qp_delta_enabled_flag: Default::default(),
            diff_cu_qp_delta_depth: Default::default(),
            cb_qp_offset: Default::default(),
            cr_qp_offset: Default::default(),
            slice_chroma_qp_offsets_present_flag: Default::default(),
            weighted_pred_flag: Default::default(),
            weighted_bipred_flag: Default::default(),
            transquant_bypass_enabled_flag: Default::default(),
            tiles_enabled_flag: Default::default(),
            entropy_coding_sync_enabled_flag: Default::default(),
            num_tile_columns_minus1: Default::default(),
            num_tile_rows_minus1: Default::default(),
            uniform_spacing_flag: true,
            column_width_minus1: Default::default(),
            row_height_minus1: Default::default(),
            loop_filter_across_tiles_enabled_flag: true,
            loop_filter_across_slices_enabled_flag: Default::default(),
            deblocking_filter_control_present_flag: Default::default(),
            deblocking_filter_override_enabled_flag: Default::default(),
            deblocking_filter_disabled_flag: Default::default(),
            beta_offset_div2: Default::default(),
            tc_offset_div2: Default::default(),
            scaling_list_data_present_flag: Default::default(),
            scaling_list: Default::default(),
            lists_modification_present_flag: Default::default(),
            log2_parallel_merge_level_minus2: Default::default(),
            slice_segment_header_extension_present_flag: Default::default(),
            extension_present_flag: Default::default(),
            range_extension_flag: Default::default(),
            range_extension: Default::default(),
            qp_bd_offset_y: Default::default(),
            scc_extension: Default::default(),
            scc_extension_flag: Default::default(),
            temporal_id: Default::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScalingLists {
    /// plus 8 specifies the value of the variable `ScalingFactor[ 2 ][ matrixId
    /// ] [ 0 ][ 0 ]` for the scaling list for the 16x16 size.
    pub scaling_list_dc_coef_minus8_16x16: [i16; 6],
    /// plus 8 specifies the value of the variable `ScalingFactor[ 3 ][ matrixId
    /// ][ 0 ][ 0 ]` for the scaling list for the 32x32 size.
    pub scaling_list_dc_coef_minus8_32x32: [i16; 6],
    /// The 4x4 scaling list.
    pub scaling_list_4x4: [[u8; 16]; 6],
    /// The 8x8 scaling list.
    pub scaling_list_8x8: [[u8; 64]; 6],
    /// The 16x16 scaling list.
    pub scaling_list_16x16: [[u8; 64]; 6],
    /// The 32x32 scaling list.
    pub scaling_list_32x32: [[u8; 64]; 6],
}

impl Default for ScalingLists {
    fn default() -> Self {
        Self {
            scaling_list_dc_coef_minus8_16x16: Default::default(),
            scaling_list_dc_coef_minus8_32x32: Default::default(),
            scaling_list_4x4: Default::default(),
            scaling_list_8x8: [[0; 64]; 6],
            scaling_list_16x16: [[0; 64]; 6],
            scaling_list_32x32: [[0; 64]; 6],
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RefPicListModification {
    /// Whenset, indicates that reference picture list 0 is specified explicitly
    /// by a list of `list_entry_l0[ i ]` values.  When not set, indicates that
    /// reference picture list 0 is determined implicitly.
    pub ref_pic_list_modification_flag_l0: bool,
    /// `list_entry_l0[ i ]` specifies the index of the reference picture in
    /// RefPicListTemp0 to be placed at the current position of reference
    /// picture list 0.
    pub list_entry_l0: Vec<u32>,
    /// Whenset, indicates that reference picture list 1 is specified explicitly
    /// by a list of `list_entry_l1[ i ]` values.  When not set, indicates that
    /// reference picture list 1 is determined implicitly.
    pub ref_pic_list_modification_flag_l1: bool,
    /// `list_entry_l1[ i ]` specifies the index of the reference picture in
    /// RefPicListTemp1 to be placed at the current position of reference
    /// picture list 1.
    pub list_entry_l1: Vec<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PredWeightTable {
    /// The base 2 logarithm of the denominator for all luma weighting factors.
    pub luma_log2_weight_denom: u8,
    /// The difference of the base 2 logarithm of the denominator for all chroma
    /// weighting factors.
    pub delta_chroma_log2_weight_denom: i8,
    /// `luma_weight_l0_flag[ i ]` set specifies that weighting factors for the
    /// luma component of list 0 prediction using `RefPicList0[ i ]` are present.
    /// `luma_weight_l0_flag[ i ]` not set specifies that these weighting factors
    /// are not present.
    pub luma_weight_l0_flag: [bool; 15],
    /// `chroma_weight_l0_flag[ i ]` set specifies that weighting factors for the
    /// chroma prediction values of list 0 prediction using `RefPicList0[ i ]` are
    /// present. `chroma_weight_l0_flag[ i ]` not set specifies that these
    /// weighting factors are not present.
    pub chroma_weight_l0_flag: [bool; 15],
    /// `delta_luma_weight_l0[ i ]` is the difference of the weighting factor
    /// applied to the luma prediction value for list 0 prediction using
    /// `RefPicList0[ i ]`.
    pub delta_luma_weight_l0: [i8; 15],
    /// `luma_offset_l0[ i ]` is the additive offset applied to the luma
    /// prediction value for list 0 prediction using `RefPicList0[ i ]`.
    pub luma_offset_l0: [i8; 15],
    /// `delta_chroma_weight_l0[ i ][ j ]` is the difference of the weighting
    /// factor applied to the chroma prediction values for list 0 prediction
    /// using `RefPicList0[ i ]` with j equal to 0 for Cb and j equal to 1 for Cr.
    pub delta_chroma_weight_l0: [[i8; 2]; 15],
    /// `delta_chroma_offset_l0[ i ][ j ]` is the difference of the additive
    /// offset applied to the chroma prediction values for list 0 prediction
    /// using `RefPicList0[ i ]` with j equal to 0 for Cb and j equal to 1 for Cr.
    pub delta_chroma_offset_l0: [[i16; 2]; 15],

    // `luma_weight_l1_flag[ i ]`, `chroma_weight_l1_flag[ i ]`,
    // `delta_luma_weight_l1[ i ]`, `luma_offset_l1[ i ]`, delta_chroma_weight_l1[ i
    // `][ j ]` and `delta_chroma_offset_l1[ i ]`[ j ] have the same
    // `semanticsasluma_weight_l0_flag[ i ]`, `chroma_weight_l0_flag[ i ]`,
    // `delta_luma_weight_l0[ i ]`, `luma_offset_l0[ i ]`, `delta_chroma_weight_l0[ i
    // ][ j ]` and `delta_chroma_offset_l0[ i ][ j ]`, respectively, with `l0`, `L0`,
    // `list 0` and `List0` replaced by `l1`, `L1`, `list 1` and `List1`, respectively.
    pub luma_weight_l1_flag: [bool; 15],
    pub chroma_weight_l1_flag: [bool; 15],
    pub delta_luma_weight_l1: [i8; 15],
    pub luma_offset_l1: [i8; 15],

    pub delta_chroma_weight_l1: [[i8; 2]; 15],
    pub delta_chroma_offset_l1: [[i16; 2]; 15],

    // Calculated.
    /// Same as ChromaLog2WeightDenom in the specification.
    pub chroma_log2_weight_denom: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShortTermRefPicSet {
    /// When set, specifies that the stRpsIdx-th candidate short-term RPS is
    /// predicted from another candidate short-term RPS, which is referred to as
    /// the source candidate short-term RPS.
    pub inter_ref_pic_set_prediction_flag: bool,
    /// delta_idx_minus1 plus 1 specifies the difference between the value of
    /// stRpsIdx and the index, into the list of the candidate short-term RPSs
    /// specified in the SPS, of the source candidate short-term RPS.
    pub delta_idx_minus1: u8,
    /// delta_rps_sign and abs_delta_rps_minus1 together specify the value of
    /// the variable deltaRps.
    pub delta_rps_sign: bool,
    /// delta_rps_sign and abs_delta_rps_minus1 together specify the value of
    /// the variable deltaRps.
    pub abs_delta_rps_minus1: u16,
    /// specifies the number of entries in the stRpsIdx-th candidate short-term
    /// RPS that have picture order count values less than the picture order
    /// count value of the current picture.
    pub num_negative_pics: u8,
    /// specifies the number of entries in the stRpsIdx-th candidate short-term
    /// RPS that have picture order count values greater than the picture order
    /// count value of the current picture.
    pub num_positive_pics: u8,
    /// Same as UsedByCurrPicS0 in the specification.
    pub used_by_curr_pic_s0: [bool; MAX_SHORT_TERM_REF_PIC_SETS],
    /// Same as UsedByCurrPicS1 in the specification.
    pub used_by_curr_pic_s1: [bool; MAX_SHORT_TERM_REF_PIC_SETS],
    /// Same as DeltaPocS0 in the specification.
    pub delta_poc_s0: [i32; MAX_SHORT_TERM_REF_PIC_SETS],
    /// Same as DeltaPocS1 in the specification.
    pub delta_poc_s1: [i32; MAX_SHORT_TERM_REF_PIC_SETS],
    /// Same as NumDeltaPocs in the specification.
    pub num_delta_pocs: u32,
}

impl Default for ShortTermRefPicSet {
    fn default() -> Self {
        Self {
            inter_ref_pic_set_prediction_flag: Default::default(),
            delta_idx_minus1: Default::default(),
            delta_rps_sign: Default::default(),
            abs_delta_rps_minus1: Default::default(),
            num_negative_pics: Default::default(),
            num_positive_pics: Default::default(),
            used_by_curr_pic_s0: [false; MAX_SHORT_TERM_REF_PIC_SETS],
            used_by_curr_pic_s1: [false; MAX_SHORT_TERM_REF_PIC_SETS],
            delta_poc_s0: [0; MAX_SHORT_TERM_REF_PIC_SETS],
            delta_poc_s1: [0; MAX_SHORT_TERM_REF_PIC_SETS],
            num_delta_pocs: Default::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SublayerHrdParameters {
    // NOTE: The value of CpbCnt is cpb_cnt_minus1[i] + 1, and cpb_cnt_minus1
    // ranges from 0..=31
    /// `bit_rate_value_minus1[ i ]` (together with bit_rate_scale) specifies the
    /// maximum input bit rate for the i-th CPB when the CPB operates at the
    /// access unit level
    pub bit_rate_value_minus1: [u32; 32],
    /// `cpb_size_value_minus1[ i ]` is used together with cpb_size_scale to
    /// specify the i-th CPB size when the CPB operates at the access unit
    /// level.
    pub cpb_size_value_minus1: [u32; 32],
    /// `cpb_size_du_value_minus1[ i ]` is used together with cpb_size_du_scale to
    /// specify the i-th CPB size when the CPB operates at sub-picture level.
    pub cpb_size_du_value_minus1: [u32; 32],
    /// `bit_rate_du_value_minus1[ i ]` (together with bit_rate_scale) specifies
    /// the maximum input bit rate for the i-th CPB when the CPB operates at the
    /// sub-picture level.
    pub bit_rate_du_value_minus1: [u32; 32],
    /// `cbr_flag[ i ]` not set specifies that to decode this CVS by the HRD using
    /// the i-th CPB specification.
    pub cbr_flag: [bool; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HrdParams {
    /// When set, specifies that NAL HRD parameters (pertaining to the Type II
    /// bitstream conformance point) are present in the hrd_parameters( ) syntax
    /// structure. When not set, specifies that NAL HRD parameters are not
    /// present in the hrd_parameters( ) syntax structure.
    pub nal_hrd_parameters_present_flag: bool,
    /// When set, specifies that VCL HRD parameters (pertaining to the Type I
    /// bitstream conformance point) are present in the hrd_parameters( ) syntax
    /// structure. When not set, specifies that VCL HRD parameters are not
    /// present in the hrd_parameters( ) syntax structure.
    pub vcl_hrd_parameters_present_flag: bool,
    /// When set, specifies that sub-picture level HRD parameters are present
    /// and the HRD may operate at access unit level or sub-picture level. When
    /// not set, specifies that sub-picture level HRD parameters are not present
    /// and the HRD operates at access unit level.
    pub sub_pic_hrd_params_present_flag: bool,
    /// Used to specify the clock sub-tick. A clock sub-tick is the minimum
    /// interval of time that can be represented in the coded data when
    /// sub_pic_hrd_params_present_flag is set.
    pub tick_divisor_minus2: u8,
    /// du_cpb_removal_delay_increment_length_minus1 plus 1 specifies the
    /// length, in bits, of the `du_cpb_removal_delay_increment_minus1[ i ]` and
    /// du_common_cpb_removal_delay_increment_minus1 syntax elements of the
    /// picture timing SEI message and the du_spt_cpb_removal_delay_increment
    /// syntax element in the decoding unit information SEI message.
    pub du_cpb_removal_delay_increment_length_minus1: u8,
    /// When set, specifies that sub-picture level CPB removal delay parameters
    /// are present in picture timing SEI messages and no decoding unit
    /// information SEI message is available (in the CVS or provided through
    /// external means not specified in this Specification).  When not set,
    /// specifies that sub-picture level CPB removal delay parameters are
    /// present in decoding unit information SEI messages and picture timing SEI
    /// messages do not include sub-picture level CPB removal delay parameters.
    pub sub_pic_cpb_params_in_pic_timing_sei_flag: bool,
    /// dpb_output_delay_du_length_minus1 plus 1 specifies the length, in bits,
    /// of the pic_dpb_output_du_delay syntax element in the picture timing SEI
    /// message and the pic_spt_dpb_output_du_delay syntax element in the
    /// decoding unit information SEI message.
    pub dpb_output_delay_du_length_minus1: u8,
    /// Together with `bit_rate_value_minus1[ i ]`, specifies the maximum input
    /// bit rate of the i-th CPB.
    pub bit_rate_scale: u8,
    /// Together with `cpb_size_du_value_minus1[ i ]`, specifies the CPB size of
    /// the i-th CPB when the CPB operates at sub-picture level.
    pub cpb_size_scale: u8,
    /// Together with `cpb_size_du_value_minus1[ i ]`, specifies the CPB size of
    /// the i-th CPB when the CPB operates at sub-picture level.
    pub cpb_size_du_scale: u8,
    /// initial_cpb_removal_delay_length_minus1 plus 1 specifies the length, in
    /// bits, of the `nal_initial_cpb_removal_delay[ i ]`,
    /// `nal_initial_cpb_removal_offset[ i ]`, `vcl_initial_cpb_removal_delay[ i ]`
    /// and `vcl_initial_cpb_removal_offset[ i ]` syntax elements of the buffering
    /// period SEI message.
    pub initial_cpb_removal_delay_length_minus1: u8,
    /// au_cpb_removal_delay_length_minus1 plus 1 specifies the length, in bits,
    /// of the cpb_delay_offset syntax element in the buffering period SEI
    /// message and the au_cpb_removal_delay_minus1 syntax element in the
    /// picture timing SEI message.
    pub au_cpb_removal_delay_length_minus1: u8,
    /// dpb_output_delay_length_minus1 plus 1 specifies the length, in bits, of
    /// the dpb_delay_offset syntax element in the buffering period SEI message
    /// and the pic_dpb_output_delay syntax element in the picture timing SEI
    /// message.
    pub dpb_output_delay_length_minus1: u8,
    /// `fixed_pic_rate_general_flag[ i ]` set indicates that, when HighestTid is
    /// equal to i, the temporal distance between the HRD output times of
    /// consecutive pictures in output order is constrained as specified in the
    /// specification. `fixed_pic_rate_general_flag[ i ]` not set indicates that
    /// this constraint may not apply.
    pub fixed_pic_rate_general_flag: [bool; 7],
    /// `fixed_pic_rate_within_cvs_flag[ i ]` set indicates that, when HighestTid
    /// is equal to i, the temporal distance between the HRD output times of
    /// consecutive pictures in output order is constrained as specified in the
    /// specification. `fixed_pic_rate_within_cvs_flag[ i ]` not set indicates
    /// that this constraint may not apply.
    pub fixed_pic_rate_within_cvs_flag: [bool; 7],
    /// `elemental_duration_in_tc_minus1[ i ]` plus 1 (when present) specifies,
    /// when HighestTid is equal to i, the temporal distance, in clock ticks,
    /// between the elemental units that specify the HRD output times of
    /// consecutive pictures in output order as specified in the specification.
    pub elemental_duration_in_tc_minus1: [u32; 7],
    /// `low_delay_hrd_flag[ i ]` specifies the HRD operational mode, when
    /// HighestTid is equal to i, as specified in Annex C or clause F.13.
    pub low_delay_hrd_flag: [bool; 7],
    /// `cpb_cnt_minus1[ i ]` plus 1 specifies the number of alternative CPB
    /// specifications in the bitstream of the CVS when HighestTid is equal to
    /// i.
    pub cpb_cnt_minus1: [u32; 7],
    /// The NAL HRD data.
    pub nal_hrd: [SublayerHrdParameters; 7],
    /// The VCL HRD data.
    pub vcl_hrd: [SublayerHrdParameters; 7],
}

impl Default for HrdParams {
    fn default() -> Self {
        Self {
            initial_cpb_removal_delay_length_minus1: 23,
            au_cpb_removal_delay_length_minus1: 23,
            dpb_output_delay_du_length_minus1: 23,
            nal_hrd_parameters_present_flag: Default::default(),
            vcl_hrd_parameters_present_flag: Default::default(),
            sub_pic_hrd_params_present_flag: Default::default(),
            tick_divisor_minus2: Default::default(),
            du_cpb_removal_delay_increment_length_minus1: Default::default(),
            sub_pic_cpb_params_in_pic_timing_sei_flag: Default::default(),
            bit_rate_scale: Default::default(),
            cpb_size_scale: Default::default(),
            cpb_size_du_scale: Default::default(),
            dpb_output_delay_length_minus1: Default::default(),
            fixed_pic_rate_general_flag: Default::default(),
            fixed_pic_rate_within_cvs_flag: Default::default(),
            elemental_duration_in_tc_minus1: Default::default(),
            low_delay_hrd_flag: Default::default(),
            cpb_cnt_minus1: Default::default(),
            nal_hrd: Default::default(),
            vcl_hrd: Default::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VuiParams {
    /// When set, specifies that aspect_ratio_idc is present.  When not set,
    /// specifies that aspect_ratio_idc is not present.
    pub aspect_ratio_info_present_flag: bool,
    /// Specifies the value of the sample aspect ratio of the luma samples.
    pub aspect_ratio_idc: u32,
    /// Indicates the horizontal size of the sample aspect ratio (in arbitrary
    /// units).
    pub sar_width: u32,
    /// Indicates the vertical size of the sample aspect ratio (in arbitrary
    /// units).
    pub sar_height: u32,
    /// When set, specifies that the overscan_appropriate_flag is present. When
    /// not set, the preferred display method for the video signal is
    /// unspecified.
    pub overscan_info_present_flag: bool,
    /// When set indicates that the cropped decoded pictures output are suitable
    /// for display using overscan. When not set, indicates that the cropped
    /// decoded pictures output contain visually important information in the
    /// entire region out to the edges of the conformance cropping window of the
    /// picture, such that the cropped decoded pictures output should not be
    /// displayed using overscan.
    pub overscan_appropriate_flag: bool,
    /// When set, specifies that video_format, video_full_range_flag and
    /// colour_description_present_flag are present.  When not set, specify that
    /// video_format, video_full_range_flag and colour_description_present_flag
    /// are not present.
    pub video_signal_type_present_flag: bool,
    /// Indicates the representation of the pictures as specified in Table E.2,
    /// before being coded in accordance with this Specification.
    pub video_format: u8,
    /// Indicates the black level and range of the luma and chroma signals as
    /// derived from E′Y, E′PB, and E′PR or E′R, E′G, and E′B real-valued
    /// component signals.
    pub video_full_range_flag: bool,
    /// When set, specifies that colour_primaries, transfer_characteristics, and
    /// matrix_coeffs are present. When not set, specifies that
    /// colour_primaries, transfer_characteristics, and matrix_coeffs are not
    /// present.
    pub colour_description_present_flag: bool,
    /// Indicates the chromaticity coordinates of the source primaries as
    /// specified in Table E.3 in terms of the CIE 1931 definition of x and y as
    /// specified in ISO 11664-1.
    pub colour_primaries: u32,
    /// See table E.4 in the specification.
    pub transfer_characteristics: u32,
    /// Describes the matrix coefficients used in deriving luma and chroma
    /// signals from the green, blue, and red, or Y, Z, and X primaries, as
    /// specified in Table E.5.
    pub matrix_coeffs: u32,
    /// When true, specifies that chroma_sample_loc_type_top_field and
    /// chroma_sample_loc_type_bottom_field are present. When false, specifies
    /// that chroma_sample_loc_type_top_field and
    /// chroma_sample_loc_type_bottom_field are not present.
    pub chroma_loc_info_present_flag: bool,
    /// See the specification for more details.
    pub chroma_sample_loc_type_top_field: u32,
    /// See the specification for more details.
    pub chroma_sample_loc_type_bottom_field: u32,
    /// When true, indicates that the value of all decoded chroma samples is
    /// equal to 1 << ( BitDepthC − 1 ). When false, provides no indication of
    /// decoded chroma sample values.
    pub neutral_chroma_indication_flag: bool,
    /// When true, indicates that the CVS conveys pictures that represent
    /// fields, and specifies that a picture timing SEI message shall be present
    /// in every access unit of the current CVS. When false, indicates that the
    /// CVS conveys pictures that represent frames and that a picture timing SEI
    /// message may or may not be present in any access unit of the current CVS.
    pub field_seq_flag: bool,
    /// When true, specifies that picture timing SEI messages are present for
    /// every picture and include the pic_struct, source_scan_type and
    /// duplicate_flag syntax elements. When false, specifies that the
    /// pic_struct syntax element is not present in picture timing SEI messages.
    pub frame_field_info_present_flag: bool,
    /// When true, indicates that the default display window parameters follow
    /// next in the VUI. When false, indicates that the default display window
    /// parameters are not present.
    pub default_display_window_flag: bool,
    /// Specifies the samples of the pictures in the CVS that are within the
    /// default display window, in terms of a rectangular region specified in
    /// picture coordinates for display.
    pub def_disp_win_left_offset: u32,
    /// Specifies the samples of the pictures in the CVS that are within the
    /// default display window, in terms of a rectangular region specified in
    /// picture coordinates for display.
    pub def_disp_win_right_offset: u32,
    /// Specifies the samples of the pictures in the CVS that are within the
    /// default display window, in terms of a rectangular region specified in
    /// picture coordinates for display.
    pub def_disp_win_top_offset: u32,
    /// Specifies the samples of the pictures in the CVS that are within the
    /// default display window, in terms of a rectangular region specified in
    /// picture coordinates for display.
    pub def_disp_win_bottom_offset: u32,
    /// When set, specifies that vui_num_units_in_tick, vui_time_scale,
    /// vui_poc_proportional_to_timing_flag and vui_hrd_parameters_present_flag
    /// are present in the vui_parameters( ) syntax structure.  When not set,
    /// specifies that vui_num_units_in_tick, vui_time_scale,
    /// vui_poc_proportional_to_timing_flag and vui_hrd_parameters_present_flag
    /// are not present in the vui_parameters( ) syntax structure
    pub timing_info_present_flag: bool,
    /// The number of time units of a clock operating at the frequency
    /// vui_time_scale Hz that corresponds to one increment (called a clock
    /// tick) of a clock tick counter.
    pub num_units_in_tick: u32,
    /// Is the number of time units that pass in one second. For example, a time
    /// coordinate system that measures time using a 27 MHz clock has a
    /// vui_time_scale of 27 000 000.
    pub time_scale: u32,
    /// When set, indicates that the picture order count value for each picture
    /// in the CVS that is not the first picture in the CVS, in decoding order,
    /// is proportional to the output time of the picture relative to the output
    /// time of the first picture in the CVS.  When not set, indicates that the
    /// picture order count value for each picture in the CVS that is not the
    /// first picture in the CVS, in decoding order, may or may not be
    /// proportional to the output time of the picture relative to the output
    /// time of the first picture in the CVS.
    pub poc_proportional_to_timing_flag: bool,
    /// vui_num_ticks_poc_diff_one_minus1 plus 1 specifies the number of clock
    /// ticks corresponding to a difference of picture order count values equal
    /// to 1.
    pub num_ticks_poc_diff_one_minus1: u32,
    /// When set, specifies that the syntax structure hrd_parameters( ) is
    /// present in the vui_parameters( ) syntax structure.  When not set,
    /// specifies that the syntax structure hrd_parameters( ) is not present in
    /// the vui_parameters( ) syntax structure.
    pub hrd_parameters_present_flag: bool,
    /// The hrd_parameters() data.
    pub hrd: HrdParams,
    /// When set, specifies that the bitstream restriction parameters for the
    /// CVS are present. When not set, specifies that the bitstream restriction
    /// parameters for the CVS are not present.
    pub bitstream_restriction_flag: bool,
    /// When set, indicates that each PPS that is active in the CVS has the same
    /// value of the syntax elements num_tile_columns_minus1,
    /// num_tile_rows_minus1, uniform_spacing_flag, `column_width_minus1[ i ]`,
    /// `row_height_minus1[ i ]` and loop_filter_across_tiles_enabled_flag, when
    /// present. When not set, indicates that tiles syntax elements in different
    /// PPSs may or may not have the same value
    pub tiles_fixed_structure_flag: bool,
    /// When not set, indicates that no sample outside the picture boundaries
    /// and no sample at a fractional sample position for which the sample value
    /// is derived using one or more samples outside the picture boundaries is
    /// used for inter prediction of any sample.  When set, indicates that one
    /// or more samples outside the picture boundaries may be used in inter
    /// prediction.
    pub motion_vectors_over_pic_boundaries_flag: bool,
    /// When set, indicates that all P and B slices (when present) that belong
    /// to the same picture have an identical reference picture list 0 and that
    /// all B slices (when present) that belong to the same picture have an
    /// identical reference picture list 1.
    pub restricted_ref_pic_lists_flag: bool,
    /// When not equal to 0, establishes a bound on the maximum possible size of
    /// distinct coded spatial segmentation regions in the pictures of the CVS.
    pub min_spatial_segmentation_idc: u32,
    /// Indicates a number of bytes not exceeded by the sum of the sizes of the
    /// VCL NAL units associated with any coded picture in the CVS.
    pub max_bytes_per_pic_denom: u32,
    /// Indicates an upper bound for the number of coded bits of coding_unit( )
    /// data for anycoding block in any picture of the CVS.
    pub max_bits_per_min_cu_denom: u32,
    /// Indicate the maximum absolute value of a decoded horizontal and vertical
    /// motion vector component, respectively, in quarter luma sample units, for
    /// all pictures in the CVS.
    pub log2_max_mv_length_horizontal: u32,
    /// Indicate the maximum absolute value of a decoded horizontal and vertical
    /// motion vector component, respectively, in quarter luma sample units, for
    /// all pictures in the CVS.
    pub log2_max_mv_length_vertical: u32,
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
            matrix_coeffs: 2,
            chroma_loc_info_present_flag: Default::default(),
            chroma_sample_loc_type_top_field: Default::default(),
            chroma_sample_loc_type_bottom_field: Default::default(),
            neutral_chroma_indication_flag: Default::default(),
            field_seq_flag: Default::default(),
            frame_field_info_present_flag: Default::default(),
            default_display_window_flag: Default::default(),
            def_disp_win_left_offset: Default::default(),
            def_disp_win_right_offset: Default::default(),
            def_disp_win_top_offset: Default::default(),
            def_disp_win_bottom_offset: Default::default(),
            timing_info_present_flag: Default::default(),
            num_units_in_tick: Default::default(),
            time_scale: Default::default(),
            poc_proportional_to_timing_flag: Default::default(),
            num_ticks_poc_diff_one_minus1: Default::default(),
            hrd_parameters_present_flag: Default::default(),
            hrd: Default::default(),
            bitstream_restriction_flag: Default::default(),
            tiles_fixed_structure_flag: Default::default(),
            motion_vectors_over_pic_boundaries_flag: true,
            restricted_ref_pic_lists_flag: Default::default(),
            min_spatial_segmentation_idc: Default::default(),
            max_bytes_per_pic_denom: 2,
            max_bits_per_min_cu_denom: 1,
            log2_max_mv_length_horizontal: 15,
            log2_max_mv_length_vertical: 15,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Parser {
    active_spses: BTreeMap<u8, Sps>,
}

impl Parser {
    fn parse_profile_tier_level(
        ptl: &mut ProfileTierLevel,
        r: &mut NaluReader,
        profile_present_flag: bool,
        sps_max_sub_layers_minus_1: u8,
    ) -> anyhow::Result<()> {
        if profile_present_flag {
            ptl.general_profile_space = r.read_bits(2)?;
            ptl.general_tier_flag = r.read_bit()?;
            ptl.general_profile_idc = r.read_bits(5)?;

            for i in 0..32 {
                ptl.general_profile_compatibility_flag[i] = r.read_bit()?;
            }

            ptl.general_progressive_source_flag = r.read_bit()?;
            ptl.general_interlaced_source_flag = r.read_bit()?;
            ptl.general_non_packed_constraint_flag = r.read_bit()?;
            ptl.general_frame_only_constraint_flag = r.read_bit()?;

            if ptl.general_profile_idc == 4
                || ptl.general_profile_compatibility_flag[4]
                || ptl.general_profile_idc == 5
                || ptl.general_profile_compatibility_flag[5]
                || ptl.general_profile_idc == 6
                || ptl.general_profile_compatibility_flag[6]
                || ptl.general_profile_idc == 7
                || ptl.general_profile_compatibility_flag[7]
                || ptl.general_profile_idc == 8
                || ptl.general_profile_compatibility_flag[8]
                || ptl.general_profile_idc == 9
                || ptl.general_profile_compatibility_flag[9]
                || ptl.general_profile_idc == 10
                || ptl.general_profile_compatibility_flag[10]
                || ptl.general_profile_idc == 11
                || ptl.general_profile_compatibility_flag[11]
            {
                ptl.general_max_12bit_constraint_flag = r.read_bit()?;
                ptl.general_max_10bit_constraint_flag = r.read_bit()?;
                ptl.general_max_8bit_constraint_flag = r.read_bit()?;
                ptl.general_max_422chroma_constraint_flag = r.read_bit()?;
                ptl.general_max_420chroma_constraint_flag = r.read_bit()?;
                ptl.general_max_monochrome_constraint_flag = r.read_bit()?;
                ptl.general_intra_constraint_flag = r.read_bit()?;
                ptl.general_one_picture_only_constraint_flag = r.read_bit()?;
                ptl.general_lower_bit_rate_constraint_flag = r.read_bit()?;
                if ptl.general_profile_idc == 5
                    || ptl.general_profile_compatibility_flag[5]
                    || ptl.general_profile_idc == 9
                    || ptl.general_profile_compatibility_flag[9]
                    || ptl.general_profile_idc == 10
                    || ptl.general_profile_compatibility_flag[10]
                    || ptl.general_profile_idc == 11
                    || ptl.general_profile_compatibility_flag[11]
                {
                    ptl.general_max_14bit_constraint_flag = r.read_bit()?;
                    // Skip general_reserved_zero_33bits
                    r.skip_bits(31)?;
                    r.skip_bits(2)?;
                } else {
                    // Skip general_reserved_zero_34bits
                    r.skip_bits(31)?;
                    r.skip_bits(3)?;
                }
            } else if ptl.general_profile_idc == 2 || ptl.general_profile_compatibility_flag[2] {
                // Skip general_reserved_zero_7bits
                r.skip_bits(7)?;
                ptl.general_one_picture_only_constraint_flag = r.read_bit()?;
                // Skip general_reserved_zero_35bits
                r.skip_bits(31)?;
                r.skip_bits(4)?;
            } else {
                r.skip_bits(31)?;
                r.skip_bits(12)?;
            }

            if ptl.general_profile_idc == 1
                || ptl.general_profile_compatibility_flag[1]
                || ptl.general_profile_idc == 2
                || ptl.general_profile_compatibility_flag[2]
                || ptl.general_profile_idc == 3
                || ptl.general_profile_compatibility_flag[3]
                || ptl.general_profile_idc == 4
                || ptl.general_profile_compatibility_flag[4]
                || ptl.general_profile_idc == 5
                || ptl.general_profile_compatibility_flag[5]
                || ptl.general_profile_idc == 9
                || ptl.general_profile_compatibility_flag[9]
                || ptl.general_profile_idc == 11
                || ptl.general_profile_compatibility_flag[11]
            {
                ptl.general_inbld_flag = r.read_bit()?;
            } else {
                r.skip_bits(1)?;
            }
        }

        let level: u8 = r.read_bits(8)?;
        ptl.general_level_idc =
            Level::n(level).with_context(|| format!("Unsupported level {level}"))?;

        for i in 0..sps_max_sub_layers_minus_1 as usize {
            ptl.sub_layer_profile_present_flag[i] = r.read_bit()?;
            ptl.sub_layer_level_present_flag[i] = r.read_bit()?;
        }

        if sps_max_sub_layers_minus_1 > 0 {
            for _ in sps_max_sub_layers_minus_1..8 {
                r.skip_bits(2)?;
            }
        }

        for i in 0..sps_max_sub_layers_minus_1 as usize {
            if ptl.sub_layer_level_present_flag[i] {
                ptl.sub_layer_profile_space[i] = r.read_bits(2)?;
                ptl.sub_layer_tier_flag[i] = r.read_bit()?;
                ptl.sub_layer_profile_idc[i] = r.read_bits(5)?;
                for j in 0..32 {
                    ptl.sub_layer_profile_compatibility_flag[i][j] = r.read_bit()?;
                }
                ptl.sub_layer_progressive_source_flag[i] = r.read_bit()?;
                ptl.sub_layer_interlaced_source_flag[i] = r.read_bit()?;
                ptl.sub_layer_non_packed_constraint_flag[i] = r.read_bit()?;
                ptl.sub_layer_frame_only_constraint_flag[i] = r.read_bit()?;

                if ptl.sub_layer_profile_idc[i] == 4
                    || ptl.sub_layer_profile_compatibility_flag[i][4]
                    || ptl.sub_layer_profile_idc[i] == 5
                    || ptl.sub_layer_profile_compatibility_flag[i][5]
                    || ptl.sub_layer_profile_idc[i] == 6
                    || ptl.sub_layer_profile_compatibility_flag[i][6]
                    || ptl.sub_layer_profile_idc[i] == 7
                    || ptl.sub_layer_profile_compatibility_flag[i][7]
                    || ptl.sub_layer_profile_idc[i] == 8
                    || ptl.sub_layer_profile_compatibility_flag[i][8]
                    || ptl.sub_layer_profile_idc[i] == 9
                    || ptl.sub_layer_profile_compatibility_flag[i][9]
                    || ptl.sub_layer_profile_idc[i] == 10
                    || ptl.sub_layer_profile_compatibility_flag[i][10]
                    || ptl.sub_layer_profile_idc[i] == 11
                    || ptl.sub_layer_profile_compatibility_flag[i][11]
                {
                    ptl.sub_layer_max_12bit_constraint_flag[i] = r.read_bit()?;
                    ptl.sub_layer_max_10bit_constraint_flag[i] = r.read_bit()?;
                    ptl.sub_layer_max_8bit_constraint_flag[i] = r.read_bit()?;
                    ptl.sub_layer_max_422chroma_constraint_flag[i] = r.read_bit()?;
                    ptl.sub_layer_max_420chroma_constraint_flag[i] = r.read_bit()?;
                    ptl.sub_layer_max_monochrome_constraint_flag[i] = r.read_bit()?;
                    ptl.sub_layer_intra_constraint_flag[i] = r.read_bit()?;
                    ptl.sub_layer_one_picture_only_constraint_flag[i] = r.read_bit()?;
                    ptl.sub_layer_lower_bit_rate_constraint_flag[i] = r.read_bit()?;

                    if ptl.sub_layer_profile_idc[i] == 5
                        || ptl.sub_layer_profile_compatibility_flag[i][5]
                        || ptl.sub_layer_profile_idc[i] == 9
                        || ptl.sub_layer_profile_compatibility_flag[i][9]
                        || ptl.sub_layer_profile_idc[i] == 10
                        || ptl.sub_layer_profile_compatibility_flag[i][10]
                        || ptl.sub_layer_profile_idc[i] == 11
                        || ptl.sub_layer_profile_compatibility_flag[i][11]
                    {
                        ptl.sub_layer_max_14bit_constraint_flag[i] = r.read_bit()?;
                        r.skip_bits(33)?;
                    } else {
                        r.skip_bits(34)?;
                    }
                } else if ptl.sub_layer_profile_idc[i] == 2
                    || ptl.sub_layer_profile_compatibility_flag[i][2]
                {
                    r.skip_bits(7)?;
                    ptl.sub_layer_one_picture_only_constraint_flag[i] = r.read_bit()?;
                    r.skip_bits(35)?;
                } else {
                    r.skip_bits(43)?;
                }

                if ptl.sub_layer_profile_idc[i] == 1
                    || ptl.sub_layer_profile_compatibility_flag[i][1]
                    || ptl.sub_layer_profile_idc[i] == 2
                    || ptl.sub_layer_profile_compatibility_flag[i][2]
                    || ptl.sub_layer_profile_idc[i] == 3
                    || ptl.sub_layer_profile_compatibility_flag[i][3]
                    || ptl.sub_layer_profile_idc[i] == 4
                    || ptl.sub_layer_profile_compatibility_flag[i][4]
                    || ptl.sub_layer_profile_idc[i] == 5
                    || ptl.sub_layer_profile_compatibility_flag[i][5]
                    || ptl.sub_layer_profile_idc[i] == 9
                    || ptl.sub_layer_profile_compatibility_flag[i][9]
                    || ptl.sub_layer_profile_idc[i] == 11
                    || ptl.sub_layer_profile_compatibility_flag[i][11]
                {
                    ptl.sub_layer_inbld_flag[i] = r.read_bit()?;
                } else {
                    r.skip_bits(1)?;
                }

                if ptl.sub_layer_level_present_flag[i] {
                    let level: u8 = r.read_bits(8)?;
                    ptl.sub_layer_level_idc[i] =
                        Level::n(level).with_context(|| format!("Unsupported level {level}"))?;
                }
            }
        }
        Ok(())
    }

    fn fill_default_scaling_list(sl: &mut ScalingLists, size_id: i32, matrix_id: i32) {
        if size_id == 0 {
            sl.scaling_list_4x4[matrix_id as usize] = DEFAULT_SCALING_LIST_0;
            return;
        }

        let dst = match size_id {
            1 => &mut sl.scaling_list_8x8[matrix_id as usize],
            2 => &mut sl.scaling_list_16x16[matrix_id as usize],
            3 => &mut sl.scaling_list_32x32[matrix_id as usize],
            _ => panic!("Invalid size_id {size_id}"),
        };

        let src = if matrix_id < 3 {
            &DEFAULT_SCALING_LIST_1
        } else if matrix_id <= 5 {
            &DEFAULT_SCALING_LIST_2
        } else {
            panic!("Invalid matrix_id {matrix_id}");
        };

        *dst = *src;

        //  When `scaling_list_pred_mode_flag[ sizeId ]`[ matrixId ] is equal to
        //  0, scaling_list_pred_matrix_id_ `delta[ sizeId ]`[ matrixId ] is equal
        //  to 0 and sizeId is greater than 1, the value of
        //  scaling_list_dc_coef_minus8[ sizeId − 2 `][ matrixId ]` is inferred to
        //  be equal to 8.
        //
        // Since we are using a slightly different layout here, with two
        // different field names (i.e. 16x16, and 32x32), we must differentiate
        // between size_id == 2 or size_id == 3.
        if size_id == 2 {
            sl.scaling_list_dc_coef_minus8_16x16[matrix_id as usize] = 8;
        } else if size_id == 3 {
            sl.scaling_list_dc_coef_minus8_32x32[matrix_id as usize] = 8;
        }
    }

    fn parse_scaling_list_data(sl: &mut ScalingLists, r: &mut NaluReader) -> anyhow::Result<()> {
        // 7.4.5
        for size_id in 0..4 {
            let mut matrix_id = 0;
            while matrix_id < 6 {
                let scaling_list_pred_mode_flag = r.read_bit()?;
                // If `scaling_list_pred_matrix_id_delta[ sizeId ]`[ matrixId ] is
                // equal to 0, the scaling list is inferred from the default
                // scaling list `ScalingList[ sizeId ]`[ matrixId `][ i ]` as specified
                // in Table 7-5 and Table 7-6 for i = 0..Min( 63, ( 1 << ( 4 + (
                // sizeId << 1 ) ) ) − 1 ).
                if !scaling_list_pred_mode_flag {
                    let scaling_list_pred_matrix_id_delta: u32 = r.read_ue()?;
                    if scaling_list_pred_matrix_id_delta == 0 {
                        Self::fill_default_scaling_list(sl, size_id, matrix_id);
                    } else {
                        // Equation 7-42
                        let factor = if size_id == 3 { 3 } else { 1 };
                        let ref_matrix_id =
                            matrix_id as u32 - scaling_list_pred_matrix_id_delta * factor;
                        if size_id == 0 {
                            sl.scaling_list_4x4[matrix_id as usize] =
                                sl.scaling_list_4x4[ref_matrix_id as usize];
                        } else {
                            let src = match size_id {
                                1 => sl.scaling_list_8x8[ref_matrix_id as usize],
                                2 => sl.scaling_list_16x16[ref_matrix_id as usize],
                                3 => sl.scaling_list_32x32[ref_matrix_id as usize],
                                _ => return Err(anyhow!("Invalid size_id {}", size_id)),
                            };

                            let dst = match size_id {
                                1 => &mut sl.scaling_list_8x8[matrix_id as usize],
                                2 => &mut sl.scaling_list_16x16[matrix_id as usize],
                                3 => &mut sl.scaling_list_32x32[matrix_id as usize],
                                _ => return Err(anyhow!("Invalid size_id {}", size_id)),
                            };

                            *dst = src;

                            if size_id == 2 {
                                sl.scaling_list_dc_coef_minus8_16x16[matrix_id as usize] =
                                    sl.scaling_list_dc_coef_minus8_16x16[ref_matrix_id as usize];
                            } else if size_id == 3 {
                                sl.scaling_list_dc_coef_minus8_32x32[matrix_id as usize] =
                                    sl.scaling_list_dc_coef_minus8_32x32[ref_matrix_id as usize];
                            }
                        }
                    }
                } else {
                    let mut next_coef = 8i32;
                    let coef_num = std::cmp::min(64, 1 << (4 + (size_id << 1)));

                    if size_id > 1 {
                        if size_id == 2 {
                            sl.scaling_list_dc_coef_minus8_16x16[matrix_id as usize] =
                                r.read_se_bounded(-7, 247)?;
                            next_coef =
                                i32::from(sl.scaling_list_dc_coef_minus8_16x16[matrix_id as usize])
                                    + 8;
                        } else if size_id == 3 {
                            sl.scaling_list_dc_coef_minus8_32x32[matrix_id as usize] =
                                r.read_se_bounded(-7, 247)?;
                            next_coef =
                                i32::from(sl.scaling_list_dc_coef_minus8_32x32[matrix_id as usize])
                                    + 8;
                        }
                    }

                    for i in 0..coef_num as usize {
                        let scaling_list_delta_coef: i32 = r.read_se_bounded(-128, 127)?;
                        next_coef = (next_coef + scaling_list_delta_coef + 256) % 256;
                        match size_id {
                            0 => sl.scaling_list_4x4[matrix_id as usize][i] = next_coef as _,
                            1 => sl.scaling_list_8x8[matrix_id as usize][i] = next_coef as _,
                            2 => sl.scaling_list_16x16[matrix_id as usize][i] = next_coef as _,
                            3 => sl.scaling_list_32x32[matrix_id as usize][i] = next_coef as _,
                            _ => return Err(anyhow!("Invalid size_id {}", size_id)),
                        }
                    }
                }
                let step = if size_id == 3 { 3 } else { 1 };
                matrix_id += step;
            }
        }
        Ok(())
    }

    fn parse_short_term_ref_pic_set(
        sps: &Sps,
        st: &mut ShortTermRefPicSet,
        r: &mut NaluReader,
        st_rps_idx: u8,
    ) -> anyhow::Result<()> {
        if st_rps_idx != 0 {
            st.inter_ref_pic_set_prediction_flag = r.read_bit()?;
        }

        // (7-59)
        if st.inter_ref_pic_set_prediction_flag {
            if st_rps_idx == sps.num_short_term_ref_pic_sets {
                st.delta_idx_minus1 = r.read_ue_max(st_rps_idx as u32 - 1)?;
            }

            st.delta_rps_sign = r.read_bit()?;
            // The value of abs_delta_rps_minus1 shall be in the range of 0 to
            // 2^15 − 1, inclusive.
            st.abs_delta_rps_minus1 = r.read_ue_max(32767)?;

            let ref_rps_idx = st_rps_idx - (st.delta_idx_minus1 + 1);
            let delta_rps =
                (1 - 2 * st.delta_rps_sign as i32) * (st.abs_delta_rps_minus1 as i32 + 1);

            let ref_st = sps
                .short_term_ref_pic_set
                .get(usize::from(ref_rps_idx))
                .ok_or(anyhow!("Invalid ref_rps_idx"))?;

            let mut used_by_curr_pic_flag = [false; 64];

            // 7.4.8 - defaults to 1 if not present
            let mut use_delta_flag = [true; 64];

            for j in 0..=ref_st.num_delta_pocs as usize {
                used_by_curr_pic_flag[j] = r.read_bit()?;
                if !used_by_curr_pic_flag[j] {
                    use_delta_flag[j] = r.read_bit()?;
                }
            }

            // (7-61)
            let mut i = 0;
            // Ranges are [a,b[, but the real loop is [b, a], i.e.
            // [num_positive_pics - 1, 0]. Use ..= so that b is included when
            // rev() is called.
            for j in (0..=isize::from(ref_st.num_positive_pics) - 1)
                .rev()
                .take_while(|j| *j >= 0)
                .map(|j| j as usize)
            {
                let d_poc = ref_st.delta_poc_s1[j] + delta_rps;
                if d_poc < 0 && use_delta_flag[usize::from(ref_st.num_negative_pics) + j] {
                    st.delta_poc_s0[i] = d_poc;
                    st.used_by_curr_pic_s0[i] =
                        used_by_curr_pic_flag[usize::from(ref_st.num_negative_pics) + j];

                    i += 1;
                }
            }

            if delta_rps < 0 && use_delta_flag[ref_st.num_delta_pocs as usize] {
                st.delta_poc_s0[i] = delta_rps;
                st.used_by_curr_pic_s0[i] = used_by_curr_pic_flag[ref_st.num_delta_pocs as usize];

                i += 1;
            }

            // Let's *not* change the original algorithm in any way.
            #[allow(clippy::needless_range_loop)]
            for j in 0..ref_st.num_negative_pics as usize {
                let d_poc = ref_st.delta_poc_s0[j] + delta_rps;
                if d_poc < 0 && use_delta_flag[j] {
                    st.delta_poc_s0[i] = d_poc;
                    st.used_by_curr_pic_s0[i] = used_by_curr_pic_flag[j];

                    i += 1;
                }
            }

            st.num_negative_pics = i as u8;

            // (7-62)
            let mut i = 0;
            // Ranges are [a,b[, but the real loop is [b, a], i.e.
            // [num_negative_pics - 1, 0]. Use ..= so that b is included when
            // rev() is called.
            for j in (0..=isize::from(ref_st.num_negative_pics) - 1)
                .rev()
                .take_while(|j| *j >= 0)
                .map(|j| j as usize)
            {
                let d_poc = ref_st.delta_poc_s0[j] + delta_rps;
                if d_poc > 0 && use_delta_flag[j] {
                    st.delta_poc_s1[i] = d_poc;
                    st.used_by_curr_pic_s1[i] = used_by_curr_pic_flag[j];

                    i += 1;
                }
            }

            if delta_rps > 0 && use_delta_flag[ref_st.num_delta_pocs as usize] {
                st.delta_poc_s1[i] = delta_rps;
                st.used_by_curr_pic_s1[i] = used_by_curr_pic_flag[ref_st.num_delta_pocs as usize];

                i += 1;
            }

            for j in 0..usize::from(ref_st.num_positive_pics) {
                let d_poc = ref_st.delta_poc_s1[j] + delta_rps;
                if d_poc > 0 && use_delta_flag[ref_st.num_negative_pics as usize + j] {
                    st.delta_poc_s1[i] = d_poc;
                    st.used_by_curr_pic_s1[i] =
                        used_by_curr_pic_flag[ref_st.num_negative_pics as usize + j];

                    i += 1;
                }
            }

            st.num_positive_pics = i as u8;
        } else {
            st.num_negative_pics = r.read_ue_max(u32::from(
                sps.max_dec_pic_buffering_minus1[usize::from(sps.max_sub_layers_minus1)],
            ))?;

            st.num_positive_pics = r.read_ue_max(u32::from(
                sps.max_dec_pic_buffering_minus1[usize::from(sps.max_sub_layers_minus1)]
                    - st.num_negative_pics,
            ))?;

            for i in 0..usize::from(st.num_negative_pics) {
                let delta_poc_s0_minus1: u32 = r.read_ue_max(32767)?;

                if i == 0 {
                    st.delta_poc_s0[i] = -(delta_poc_s0_minus1 as i32 + 1);
                } else {
                    st.delta_poc_s0[i] = st.delta_poc_s0[i - 1] - (delta_poc_s0_minus1 as i32 + 1);
                }

                st.used_by_curr_pic_s0[i] = r.read_bit()?;
            }

            for i in 0..usize::from(st.num_positive_pics) {
                let delta_poc_s1_minus1: u32 = r.read_ue_max(32767)?;

                if i == 0 {
                    st.delta_poc_s1[i] = delta_poc_s1_minus1 as i32 + 1;
                } else {
                    st.delta_poc_s1[i] = st.delta_poc_s1[i - 1] + (delta_poc_s1_minus1 as i32 + 1);
                }

                st.used_by_curr_pic_s1[i] = r.read_bit()?;
            }
        }

        st.num_delta_pocs = u32::from(st.num_negative_pics + st.num_positive_pics);

        Ok(())
    }

    fn parse_sublayer_hrd_parameters(
        h: &mut SublayerHrdParameters,
        cpb_cnt: u32,
        sub_pic_hrd_params_present_flag: bool,
        r: &mut NaluReader,
    ) -> anyhow::Result<()> {
        for i in 0..cpb_cnt as usize {
            h.bit_rate_value_minus1[i] = r.read_ue_max((2u64.pow(32) - 2) as u32)?;
            h.cpb_size_value_minus1[i] = r.read_ue_max((2u64.pow(32) - 2) as u32)?;
            if sub_pic_hrd_params_present_flag {
                h.cpb_size_du_value_minus1[i] = r.read_ue_max((2u64.pow(32) - 2) as u32)?;
                h.bit_rate_du_value_minus1[i] = r.read_ue_max((2u64.pow(32) - 2) as u32)?;
            }

            h.cbr_flag[i] = r.read_bit()?;
        }

        Ok(())
    }

    fn parse_hrd_parameters(
        common_inf_present_flag: bool,
        max_num_sublayers_minus1: u8,
        hrd: &mut HrdParams,
        r: &mut NaluReader,
    ) -> anyhow::Result<()> {
        if common_inf_present_flag {
            hrd.nal_hrd_parameters_present_flag = r.read_bit()?;
            hrd.vcl_hrd_parameters_present_flag = r.read_bit()?;
            if hrd.nal_hrd_parameters_present_flag || hrd.vcl_hrd_parameters_present_flag {
                hrd.sub_pic_hrd_params_present_flag = r.read_bit()?;
                if hrd.sub_pic_hrd_params_present_flag {
                    hrd.tick_divisor_minus2 = r.read_bits(8)?;
                    hrd.du_cpb_removal_delay_increment_length_minus1 = r.read_bits(5)?;
                    hrd.sub_pic_cpb_params_in_pic_timing_sei_flag = r.read_bit()?;
                    hrd.dpb_output_delay_du_length_minus1 = r.read_bits(5)?;
                }
                hrd.bit_rate_scale = r.read_bits(4)?;
                hrd.cpb_size_scale = r.read_bits(4)?;
                if hrd.sub_pic_hrd_params_present_flag {
                    hrd.cpb_size_du_scale = r.read_bits(4)?;
                }
                hrd.initial_cpb_removal_delay_length_minus1 = r.read_bits(5)?;
                hrd.au_cpb_removal_delay_length_minus1 = r.read_bits(5)?;
                hrd.dpb_output_delay_length_minus1 = r.read_bits(5)?;
            }
        }

        for i in 0..=max_num_sublayers_minus1 as usize {
            hrd.fixed_pic_rate_general_flag[i] = r.read_bit()?;
            if !hrd.fixed_pic_rate_general_flag[i] {
                hrd.fixed_pic_rate_within_cvs_flag[i] = r.read_bit()?;
            }
            if hrd.fixed_pic_rate_within_cvs_flag[i] {
                hrd.elemental_duration_in_tc_minus1[i] = r.read_ue_max(2047)?;
            } else {
                hrd.low_delay_hrd_flag[i] = r.read_bit()?;
            }

            if !hrd.low_delay_hrd_flag[i] {
                hrd.cpb_cnt_minus1[i] = r.read_ue_max(31)?;
            }

            if hrd.nal_hrd_parameters_present_flag {
                Self::parse_sublayer_hrd_parameters(
                    &mut hrd.nal_hrd[i],
                    hrd.cpb_cnt_minus1[i] + 1,
                    hrd.sub_pic_hrd_params_present_flag,
                    r,
                )?;
            }

            if hrd.vcl_hrd_parameters_present_flag {
                Self::parse_sublayer_hrd_parameters(
                    &mut hrd.vcl_hrd[i],
                    hrd.cpb_cnt_minus1[i] + 1,
                    hrd.sub_pic_hrd_params_present_flag,
                    r,
                )?;
            }
        }

        Ok(())
    }

    fn parse_vui_parameters(sps: &mut Sps, r: &mut NaluReader) -> anyhow::Result<()> {
        let vui = &mut sps.vui_parameters;

        vui.aspect_ratio_info_present_flag = r.read_bit()?;
        if vui.aspect_ratio_info_present_flag {
            vui.aspect_ratio_idc = r.read_bits(8)?;
            const EXTENDED_SAR: u32 = 255;
            if vui.aspect_ratio_idc == EXTENDED_SAR {
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
                vui.matrix_coeffs = r.read_bits(8)?;
            }
        }

        vui.chroma_loc_info_present_flag = r.read_bit()?;
        if vui.chroma_loc_info_present_flag {
            vui.chroma_sample_loc_type_top_field = r.read_ue_max(5)?;
            vui.chroma_sample_loc_type_bottom_field = r.read_ue_max(5)?;
        }

        vui.neutral_chroma_indication_flag = r.read_bit()?;
        vui.field_seq_flag = r.read_bit()?;
        vui.frame_field_info_present_flag = r.read_bit()?;
        vui.default_display_window_flag = r.read_bit()?;

        if vui.default_display_window_flag {
            vui.def_disp_win_left_offset = r.read_ue()?;
            vui.def_disp_win_right_offset = r.read_ue()?;
            vui.def_disp_win_top_offset = r.read_ue()?;
            vui.def_disp_win_bottom_offset = r.read_ue()?;
        }

        vui.timing_info_present_flag = r.read_bit()?;
        if vui.timing_info_present_flag {
            vui.num_units_in_tick = r.read_bits::<u32>(31)? << 1;
            vui.num_units_in_tick |= r.read_bits::<u32>(1)?;

            // if vui.num_units_in_tick == 0 {
            //     log::warn!(
            //         "Incompliant value for num_units_in_tick {}",
            //         vui.num_units_in_tick
            //     );
            // }

            vui.time_scale = r.read_bits::<u32>(31)? << 1;
            vui.time_scale |= r.read_bits::<u32>(1)?;

            // if vui.time_scale == 0 {
            //     log::warn!("Incompliant value for time_scale {}", vui.time_scale);
            // }

            vui.poc_proportional_to_timing_flag = r.read_bit()?;
            if vui.poc_proportional_to_timing_flag {
                vui.num_ticks_poc_diff_one_minus1 = r.read_ue_max((2u64.pow(32) - 2) as u32)?;
            }

            vui.hrd_parameters_present_flag = r.read_bit()?;
            if vui.hrd_parameters_present_flag {
                let sps_max_sub_layers_minus1 = sps.max_sub_layers_minus1;
                Self::parse_hrd_parameters(true, sps_max_sub_layers_minus1, &mut vui.hrd, r)?;
            }
        }

        vui.bitstream_restriction_flag = r.read_bit()?;
        if vui.bitstream_restriction_flag {
            vui.tiles_fixed_structure_flag = r.read_bit()?;
            vui.motion_vectors_over_pic_boundaries_flag = r.read_bit()?;
            vui.restricted_ref_pic_lists_flag = r.read_bit()?;

            vui.min_spatial_segmentation_idc = r.read_ue_max(4095)?;
            vui.max_bytes_per_pic_denom = r.read_ue()?;
            vui.max_bits_per_min_cu_denom = r.read_ue()?;
            vui.log2_max_mv_length_horizontal = r.read_ue_max(16)?;
            vui.log2_max_mv_length_vertical = r.read_ue_max(15)?;
        }

        Ok(())
    }

    fn parse_sps_scc_extension(sps: &mut Sps, r: &mut NaluReader) -> anyhow::Result<()> {
        let scc = &mut sps.scc_extension;

        scc.curr_pic_ref_enabled_flag = r.read_bit()?;
        scc.palette_mode_enabled_flag = r.read_bit()?;
        if scc.palette_mode_enabled_flag {
            scc.palette_max_size = r.read_ue_max(64)?;
            scc.delta_palette_max_predictor_size =
                r.read_ue_max(128 - u32::from(scc.palette_max_size))?;
            scc.palette_predictor_initializers_present_flag = r.read_bit()?;
            if scc.palette_predictor_initializers_present_flag {
                let max =
                    u32::from(scc.palette_max_size + scc.delta_palette_max_predictor_size - 1);
                scc.num_palette_predictor_initializer_minus1 = r.read_ue_max(max)?;

                let num_comps = if sps.chroma_format_idc == 0 { 1 } else { 3 };
                for comp in 0..num_comps {
                    for i in 0..=usize::from(scc.num_palette_predictor_initializer_minus1) {
                        let num_bits = if comp == 0 {
                            sps.bit_depth_luma_minus8 + 8
                        } else {
                            sps.bit_depth_chroma_minus8 + 8
                        };
                        scc.palette_predictor_initializer[comp][i] =
                            r.read_bits(usize::from(num_bits))?;
                    }
                }
            }
        }

        scc.motion_vector_resolution_control_idc = r.read_bits(2)?;
        scc.intra_boundary_filtering_disabled_flag = r.read_bit()?;

        Ok(())
    }

    fn parse_sps_range_extension(sps: &mut Sps, r: &mut NaluReader) -> anyhow::Result<()> {
        let ext = &mut sps.range_extension;

        ext.transform_skip_rotation_enabled_flag = r.read_bit()?;
        ext.transform_skip_context_enabled_flag = r.read_bit()?;
        ext.implicit_rdpcm_enabled_flag = r.read_bit()?;
        ext.explicit_rdpcm_enabled_flag = r.read_bit()?;
        ext.extended_precision_processing_flag = r.read_bit()?;
        ext.intra_smoothing_disabled_flag = r.read_bit()?;
        ext.high_precision_offsets_enabled_flag = r.read_bit()?;
        ext.persistent_rice_adaptation_enabled_flag = r.read_bit()?;
        ext.cabac_bypass_alignment_enabled_flag = r.read_bit()?;

        Ok(())
    }

    /// Parse a SPS NALU.
    pub fn parse_sps(&mut self, nalu: &Nalu) -> anyhow::Result<&Sps> {
        if !matches!(nalu.header.type_, NaluType::SpsNut) {
            return Err(anyhow!(
                "Invalid NALU type, expected {:?}, got {:?}",
                NaluType::SpsNut,
                nalu.header.type_
            ));
        }

        let data = nalu.as_ref();
        let header = &nalu.header;
        let hdr_len = header.len();
        // Skip the header
        let mut r = NaluReader::new(&data[hdr_len..]);

        let mut sps = Sps {
            video_parameter_set_id: r.read_bits(4)?,
            max_sub_layers_minus1: r.read_bits(3)?,
            temporal_id_nesting_flag: r.read_bit()?,
            ..Default::default()
        };

        Self::parse_profile_tier_level(
            &mut sps.profile_tier_level,
            &mut r,
            true,
            sps.max_sub_layers_minus1,
        )?;

        sps.seq_parameter_set_id = r.read_ue_max(MAX_SPS_COUNT as u32 - 1)?;
        sps.chroma_format_idc = r.read_ue_max(3)?;

        if sps.chroma_format_idc == 3 {
            sps.separate_colour_plane_flag = r.read_bit()?;
        }

        sps.chroma_array_type = if sps.separate_colour_plane_flag {
            0
        } else {
            sps.chroma_format_idc
        };

        sps.pic_width_in_luma_samples = r.read_ue_bounded(1, 16888)?;
        sps.pic_height_in_luma_samples = r.read_ue_bounded(1, 16888)?;

        sps.conformance_window_flag = r.read_bit()?;
        if sps.conformance_window_flag {
            sps.conf_win_left_offset = r.read_ue()?;
            sps.conf_win_right_offset = r.read_ue()?;
            sps.conf_win_top_offset = r.read_ue()?;
            sps.conf_win_bottom_offset = r.read_ue()?;
        }

        sps.bit_depth_luma_minus8 = r.read_ue_max(6)?;
        sps.bit_depth_chroma_minus8 = r.read_ue_max(6)?;
        sps.log2_max_pic_order_cnt_lsb_minus4 = r.read_ue_max(12)?;
        sps.sub_layer_ordering_info_present_flag = r.read_bit()?;

        {
            let i = if sps.sub_layer_ordering_info_present_flag {
                0
            } else {
                sps.max_sub_layers_minus1
            };

            for j in i..=sps.max_sub_layers_minus1 {
                sps.max_dec_pic_buffering_minus1[j as usize] = r.read_ue_max(16)?;
                sps.max_num_reorder_pics[j as usize] =
                    r.read_ue_max(sps.max_dec_pic_buffering_minus1[j as usize] as _)?;
                sps.max_latency_increase_plus1[j as usize] = r.read_ue_max(u32::MAX - 1)?;
            }
        }

        sps.log2_min_luma_coding_block_size_minus3 = r.read_ue_max(3)?;
        sps.log2_diff_max_min_luma_coding_block_size = r.read_ue_max(6)?;
        sps.log2_min_luma_transform_block_size_minus2 = r.read_ue_max(3)?;
        sps.log2_diff_max_min_luma_transform_block_size = r.read_ue_max(3)?;

        // (7-10)
        sps.min_cb_log2_size_y = u32::from(sps.log2_min_luma_coding_block_size_minus3 + 3);
        // (7-11)
        sps.ctb_log2_size_y =
            sps.min_cb_log2_size_y + u32::from(sps.log2_diff_max_min_luma_coding_block_size);
        // (7-12)
        sps.ctb_size_y = 1 << sps.ctb_log2_size_y;
        // (7-17)
        sps.pic_height_in_ctbs_y =
            (sps.pic_height_in_luma_samples as f64 / sps.ctb_size_y as f64).ceil() as u32;
        // (7-15)
        sps.pic_width_in_ctbs_y =
            (sps.pic_width_in_luma_samples as f64 / sps.ctb_size_y as f64).ceil() as u32;

        sps.max_tb_log2_size_y = u32::from(
            sps.log2_min_luma_transform_block_size_minus2
                + 2
                + sps.log2_diff_max_min_luma_transform_block_size,
        );

        sps.pic_size_in_samples_y =
            u32::from(sps.pic_width_in_luma_samples) * u32::from(sps.pic_height_in_luma_samples);

        if sps.max_tb_log2_size_y > std::cmp::min(sps.ctb_log2_size_y, 5) {
            return Err(anyhow!(
                "Invalid value for MaxTbLog2SizeY: {}",
                sps.max_tb_log2_size_y
            ));
        }

        sps.pic_size_in_ctbs_y = sps.pic_width_in_ctbs_y * sps.pic_height_in_ctbs_y;

        sps.max_transform_hierarchy_depth_inter = r.read_ue_max(4)?;
        sps.max_transform_hierarchy_depth_intra = r.read_ue_max(4)?;

        sps.scaling_list_enabled_flag = r.read_bit()?;
        if sps.scaling_list_enabled_flag {
            sps.scaling_list_data_present_flag = r.read_bit()?;
            if sps.scaling_list_data_present_flag {
                Self::parse_scaling_list_data(&mut sps.scaling_list, &mut r)?;
            }
        }

        sps.amp_enabled_flag = r.read_bit()?;
        sps.sample_adaptive_offset_enabled_flag = r.read_bit()?;

        sps.pcm_enabled_flag = r.read_bit()?;
        if sps.pcm_enabled_flag {
            sps.pcm_sample_bit_depth_luma_minus1 = r.read_bits(4)?;
            sps.pcm_sample_bit_depth_chroma_minus1 = r.read_bits(4)?;
            sps.log2_min_pcm_luma_coding_block_size_minus3 = r.read_ue_max(2)?;
            sps.log2_diff_max_min_pcm_luma_coding_block_size = r.read_ue_max(2)?;
            sps.pcm_loop_filter_disabled_flag = r.read_bit()?;
        }

        sps.num_short_term_ref_pic_sets = r.read_ue_max(64)?;

        for i in 0..sps.num_short_term_ref_pic_sets {
            let mut st = ShortTermRefPicSet::default();
            Self::parse_short_term_ref_pic_set(&sps, &mut st, &mut r, i)?;
            sps.short_term_ref_pic_set.push(st);
        }

        sps.long_term_ref_pics_present_flag = r.read_bit()?;
        if sps.long_term_ref_pics_present_flag {
            sps.num_long_term_ref_pics_sps = r.read_ue_max(32)?;
            for i in 0..usize::from(sps.num_long_term_ref_pics_sps) {
                sps.lt_ref_pic_poc_lsb_sps[i] =
                    r.read_bits(usize::from(sps.log2_max_pic_order_cnt_lsb_minus4) + 4)?;
                sps.used_by_curr_pic_lt_sps_flag[i] = r.read_bit()?;
            }
        }

        sps.temporal_mvp_enabled_flag = r.read_bit()?;
        sps.strong_intra_smoothing_enabled_flag = r.read_bit()?;

        sps.vui_parameters_present_flag = r.read_bit()?;
        if sps.vui_parameters_present_flag {
            Self::parse_vui_parameters(&mut sps, &mut r)?;
        }

        sps.extension_present_flag = r.read_bit()?;
        if sps.extension_present_flag {
            sps.range_extension_flag = r.read_bit()?;
            if sps.range_extension_flag {
                Self::parse_sps_range_extension(&mut sps, &mut r)?;
            }

            let multilayer_extension_flag = r.read_bit()?;
            if multilayer_extension_flag {
                return Err(anyhow!("Multilayer extension not supported."));
            }

            let three_d_extension_flag = r.read_bit()?;
            if three_d_extension_flag {
                return Err(anyhow!("3D extension not supported."));
            }

            sps.scc_extension_flag = r.read_bit()?;
            if sps.scc_extension_flag {
                Self::parse_sps_scc_extension(&mut sps, &mut r)?;
            }
        }

        let shift = if sps.range_extension.high_precision_offsets_enabled_flag {
            sps.bit_depth_luma_minus8 + 7
        } else {
            7
        };

        sps.wp_offset_half_range_y = 1 << shift;

        let shift = if sps.range_extension.high_precision_offsets_enabled_flag {
            sps.bit_depth_chroma_minus8 + 7
        } else {
            7
        };

        sps.wp_offset_half_range_c = 1 << shift;

        let key = sps.seq_parameter_set_id;
        self.active_spses.insert(key, sps);

        if self.active_spses.keys().len() > MAX_SPS_COUNT {
            return Err(anyhow!(
                "Broken data: Number of active SPSs > MAX_SPS_COUNT"
            ));
        }

        Ok(self.get_sps(key).unwrap())
    }

    /// Returns a previously parsed sps given `sps_id`, if any.
    pub fn get_sps(&self, sps_id: u8) -> Option<&Sps> {
        self.active_spses.get(&sps_id)
    }
}
