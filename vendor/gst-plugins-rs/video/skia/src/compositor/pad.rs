// SPDX-License-Identifier: MPL-2.0
use gst::glib::Properties;
use gst_base::subclass::prelude::*;
use gst_video::{prelude::*, subclass::prelude::*};
use std::sync::Mutex;

use super::*;

#[derive(Clone, Debug)]
pub struct Settings {
    alpha: f64,
    xpos: f32,
    ypos: f32,
    width: f32,
    height: f32,
    anti_alias: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            alpha: 1.0,
            xpos: 0.0,
            ypos: 0.0,
            width: -1.0,
            height: -1.0,
            anti_alias: true,
        }
    }
}

#[derive(glib::Enum, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Default)]
#[enum_type(name = "GstSkiaCompositorPadOperator")]
#[repr(u32)]
// Trying to exactly match names and value of compositor:operator
pub enum Operator {
    Source,
    #[default]
    Over,
    Add,
    Dest,
    Clear,
    DestOver,
    SourceIn,
    DestIn,
    SourceOut,
    DestOut,
    SourceATop,
    DestATop,
    Xor,
    Modulate,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Multiply,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl From<Operator> for skia::BlendMode {
    fn from(val: Operator) -> Self {
        match val {
            Operator::Clear => skia::BlendMode::Clear,
            Operator::Source => skia::BlendMode::Src,
            Operator::Dest => skia::BlendMode::Dst,
            Operator::Over => skia::BlendMode::SrcOver,
            Operator::DestOver => skia::BlendMode::DstOver,
            Operator::SourceIn => skia::BlendMode::SrcIn,
            Operator::DestIn => skia::BlendMode::DstIn,
            Operator::SourceOut => skia::BlendMode::SrcOut,
            Operator::DestOut => skia::BlendMode::DstOut,
            Operator::SourceATop => skia::BlendMode::SrcATop,
            Operator::DestATop => skia::BlendMode::DstATop,
            Operator::Xor => skia::BlendMode::Xor,
            Operator::Add => skia::BlendMode::Plus,
            Operator::Modulate => skia::BlendMode::Modulate,
            Operator::Screen => skia::BlendMode::Screen,
            Operator::Overlay => skia::BlendMode::Overlay,
            Operator::Darken => skia::BlendMode::Darken,
            Operator::Lighten => skia::BlendMode::Lighten,
            Operator::ColorDodge => skia::BlendMode::ColorDodge,
            Operator::ColorBurn => skia::BlendMode::ColorBurn,
            Operator::HardLight => skia::BlendMode::HardLight,
            Operator::SoftLight => skia::BlendMode::SoftLight,
            Operator::Difference => skia::BlendMode::Difference,
            Operator::Exclusion => skia::BlendMode::Exclusion,
            Operator::Multiply => skia::BlendMode::Multiply,
            Operator::Hue => skia::BlendMode::Hue,
            Operator::Saturation => skia::BlendMode::Saturation,
            Operator::Color => skia::BlendMode::Color,
            Operator::Luminosity => skia::BlendMode::Luminosity,
        }
    }
}

#[derive(Properties, Debug, Default)]
#[properties(wrapper_type = super::SkiaCompositorPad)]
pub struct SkiaCompositorPad {
    #[property(
        get,
        set,
        name = "alpha",
        nick = "Alpha",
        blurb = "Alpha value of the input",
        minimum = 0.0,
        maximum = 1.0,
        type = f64,
        member = alpha,
    )]
    #[property(get, set, type = f32, name= "xpos", nick = "X Position", blurb = "Horizontal position of the input", minimum = f32::MIN, maximum = f32::MAX, member = xpos)]
    #[property(get, set, type = f32, name = "ypos", nick = "Y Position", blurb = "Vertical position of the input", minimum = f32::MIN, maximum = f32::MAX, member = ypos)]
    #[property(get, set, type = f32, name = "width", nick = "Width", blurb = "Width of the picture", minimum = -1.0, maximum = f32::MAX, member = width, default = -1.0)]
    #[property(get, set, type = f32, name = "height", nick = "Height", blurb = "Height of the picture", minimum = -1.0, maximum = f32::MAX, member = height, default = -1.0)]
    #[property(get, set, type = bool, name = "anti-alias", nick = "Anti-alias", blurb = "Whether to use anti-aliasing", member = anti_alias, default = true)]
    pub settings: Mutex<Settings>,
    #[property(
        get,
        set,
        name = "operator",
        nick = "Operator",
        blurb = "Blending operator to use for blending this pad over the previous ones",
        builder(Operator::Over)
    )]
    operator: Mutex<Operator>,
}

// This trait registers our type with the GObject object system and
// provides the entry points for creating a new instance and setting
// up the class data.
#[glib::object_subclass]
impl ObjectSubclass for SkiaCompositorPad {
    const NAME: &'static str = "GstSkiaCompositorPad";
    type Type = super::SkiaCompositorPad;
    type ParentType = gst_video::VideoAggregatorConvertPad;
}

#[glib::derived_properties]
impl ObjectImpl for SkiaCompositorPad {}
impl GstObjectImpl for SkiaCompositorPad {}
impl PadImpl for SkiaCompositorPad {}
impl AggregatorPadImpl for SkiaCompositorPad {}
impl VideoAggregatorPadImpl for SkiaCompositorPad {}
impl VideoAggregatorConvertPadImpl for SkiaCompositorPad {}
