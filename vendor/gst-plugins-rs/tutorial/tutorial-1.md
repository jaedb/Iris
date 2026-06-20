# How to write GStreamer Elements in Rust Part 1: A Video Filter for converting RGB to grayscale

In this first part we’re going to write a plugin that contains a video filter element. The video filter can convert from RGB to grayscale, either output as 8-bit per pixel grayscale or 32-bit per pixel RGB. In addition there’s a property to invert all grayscale values, or to shift them by up to 255 values. In the end this will allow you to watch [Big Bucky Bunny](https://peach.blender.org/), or anything else really that can somehow go into a GStreamer pipeline, in grayscale. Or encode the output to a new video file, send it over the network via [WebRTC](https://gstconf.ubicast.tv/videos/gstreamer-webrtc/) or something else, or basically do anything you want with it.

![alt text](img/bbb.jpg "Big Bucky Bunny – Grayscale")

This will show the basics of how to write a GStreamer plugin and element in Rust: the basic setup for registering a type and implementing it in Rust, and how to use the various GStreamer API and APIs from the Rust standard library to do the processing.

The final code for this plugin can be found [here](https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/tree/main/tutorial), and it is based on latest git version of the [gstreamer](https://gitlab.freedesktop.org/gstreamer/gstreamer-rs). At least Rust 1.32 is required for all this. You also need to have GStreamer (at least version 1.8) installed for your platform, see e.g. the GStreamer bindings [installation instructions](https://gitlab.freedesktop.org/gstreamer/gstreamer-rs/blob/main/README.md).

# Table of contents

 1.  [Project Structure](#project-structure)
 2.  [Plugin Initialization](#plugin-initialization)
 3.  [Type Registration](#type-registration)
 4.  [Type Class & Instance Initialization](#type-class-instance-initialization)
 5.  [Debug Category](#debug-category)
 6.  [Caps & Pad Templates](#caps-pad-templates)
 7.  [Caps Handling](#caps-handling)
 8.  [Conversion of BGRx Video Frames to Grayscale](#conversion-of-bgrx-video-frames-to-grayscale)
 9.  [Testing the new element](#testing-the-new-element)
 10.  [Properties](#properties)
 11.  [What next](#what-next)

## Project Structure

We’ll create a new `cargo` project with `cargo init --lib --name gst-plugin-tutorial`. This will create a basically empty `Cargo.toml` and a corresponding `src/lib.rs`. We will use this structure: `lib.rs` will contain all the plugin related code, separate modules will contain any GStreamer plugins that are added.

The empty `Cargo.toml` has to be updated to list all the dependencies that we need, and to define that the crate should result in a `cdylib`, i.e. a C library that does not contain any Rust-specific metadata. The final `Cargo.toml` looks as follows.


```toml
[package]
name = "gst-plugin-tutorial"
version = "0.1.0"
authors = ["Sebastian Dröge <sebastian@centricular.com>"]
repository = "https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs"
license = "MIT OR Apache-2.0"
edition = "2018"
description = "Rust Tutorial Plugin"

[dependencies]
gst = { package = "gstreamer", git = "https://gitlab.freedesktop.org/gstreamer/gstreamer-rs" }
gst-base = { package = "gstreamer-base", git = "https://gitlab.freedesktop.org/gstreamer/gstreamer-rs" }
gst-video = { package = "gstreamer-video", git = "https://gitlab.freedesktop.org/gstreamer/gstreamer-rs" }

[lib]
name = "gstrstutorial"
crate-type = ["cdylib"]
path = "src/lib.rs"

[build-dependencies]
gst-plugin-version-helper = {  git = "https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs" }
```

We depend on the `gstreamer`, `gstreamer-base` and `gstreamer-video` crates for various GStreamer APIs that will be used later. GStreamer is building upon GLib, and this leaks through in various places. We also have one build dependency on the `gst-plugin-version-helper` crate, which helps to get some information about the plugin for the `gst::plugin_define!` macro automatically.

With the basic project structure being set-up, we should be able to compile the project with `cargo build` now, which will download and build all dependencies and then creates a file called `target/debug/libgstrstutorial.so` (or .dll on Windows, .dylib on macOS). This is going to be our GStreamer plugin.

To allow GStreamer to find our new plugin and make it available in every GStreamer-based application, we could install it into the system or user-wide GStreamer plugin path or simply point the `GST_PLUGIN_PATH` environment variable to the directory containing it:

```bash
export GST_PLUGIN_PATH=`pwd`/target/debug
```

If you now run the `gst-inspect-1.0` tool on the `libgstrstutorial.so`, it will not yet print all information it can extract from the plugin but for now just complains that this is not a valid GStreamer plugin. Which is true, we didn’t write any code for it yet.

## Plugin Initialization

Let’s start editing `src/lib.rs` to make this an actual GStreamer plugin.

Next we make use of the `gst::plugin_define!` `macro` from the `gstreamer` crate to set-up the static metadata of the plugin (and make the shared library recognizable by GStreamer to be a valid plugin), and to define the name of our entry point function (`plugin_init`) where we will register all the elements that this plugin provides.

```rust
gst::plugin_define!(
    rstutorial,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "MIT/X11",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);
```

GStreamer requires this information to be statically available in the shared library, not returned by a function.

The static plugin metadata that we provide here is

 1. name of the plugin
 1. short description for the plugin
 1. name of the plugin entry point function
 1. version number of the plugin
 1. license of the plugin (only a fixed set of licenses is allowed here, [see](https://gstreamer.freedesktop.org/data/doc/gstreamer/head/gstreamer/html/GstPlugin.html#GstPluginDesc))
 1. source package name
 1. binary package name (only really makes sense for e.g. Linux distributions)
 1. origin of the plugin
 1. release date of this version


Next we create `build.rs` in the project main directory.

```rust
fn main() {
    gst_plugin_version_helper::info()
}
```

`build.rs` compiles and runs before anything else, [see](https://doc.rust-lang.org/cargo/reference/build-scripts.html).

Therefore, `gst_plugin_version_helper::info()` will provide various information via `cargo` environment variables such as `COMMIT_ID` and `BUILD_REL_DATE` and these environment variables are then later accessed by our invocation of the `gst::plugin_define!` macro during compilation.

In addition we’re defining an empty plugin entry point function that just returns `Ok(())`

```rust
fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    Ok(())
}
```

With all that given, `gst-inspect-1.0` should print exactly this information when running on the `libgstrstutorial.so` file (or .dll on Windows, or .dylib on macOS)

```sh
gst-inspect-1.0 target/debug/libgstrstutorial.so
```

## Type Registration

As a next step, we’re going to add another module `rgb2gray` to our project, and call a function called `register` from our `plugin_init` function.

```rust
use gst::glib;

mod rgb2gray;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    rgb2gray::register(plugin)?;
    Ok(())
}
```

With that our `src/lib.rs` is complete, and all following code is only in `src/rgb2gray/imp.rs`. At the top of the new file we first need to add various `use-directives` to import various types, macros and functions we’re going to use into the current module’s scope.

```rust
use gst::glib;
use gst::prelude::*;
use gst_video::subclass::prelude::*;

use std::sync::Mutex;

use std::sync::LazyLock;
```

GStreamer is based on the GLib object system ([GObject](https://developer.gnome.org/gobject/stable/)). C (just like Rust) does not have built-in support for object orientated programming, inheritance, virtual methods and related concepts, and GObject makes these features available in C as a library. Without language support this is a quite verbose endeavour in C, and the `glib` crate tries to expose all this in a (as much as possible) Rust-style API while hiding all the details that do not really matter.

So, as a next step we need to register a new type for our RGB to Grayscale converter GStreamer element with the GObject type system, and then register that type with GStreamer to be able to create new instances of it. We do this with the following code

```rust
use gst::glib;

#[derive(Default)]
pub struct Rgb2Gray {}

impl Rgb2Gray {}

#[glib::object_subclass]
impl ObjectSubclass for Rgb2Gray {
    const NAME: &'static str = "GstRsRgb2Gray";
    type Type = super::Rgb2Gray;
    type ParentType = gst_video::VideoFilter;
}
```

This defines a struct `Rgb2Gray` which is empty for now and an empty implementation of the struct which will later be used. The `ObjectSubclass` trait is implemented on the struct `Rgb2Gray` for providing static information about the type to the type system. By implementing `ObjectSubclass` we allow registering our struct with the GObject object system.

`ObjectSubclass` has an associated constant which contains the name of the type and some associated types.

Additionally we need to implement various traits on the Rgb2Gray struct, which will later be used to override virtual methods of the various parent classes of our element. For now we can keep the trait implementations empty. There is one trait implementation required per parent class.

```rust
impl ObjectImpl for Rgb2Gray {}
impl GstObjectImpl for Rgb2Gray {}
impl ElementImpl for Rgb2Gray {}
impl BaseTransformImpl for Rgb2Gray {}
impl VideoFilterImpl for Rgb2Gray {}
```

We also add the following code is in `src/rgb2gray/mod.rs`:

```rust
use gst::glib;
use gst::prelude::*;

mod imp;

glib::wrapper! {
    pub struct Rgb2Gray(ObjectSubclass<imp::Rgb2Gray>) @extends gst_base::BaseTransform, gst::Element, gst::Object;
}

// Registers the type for our element, and then registers in GStreamer under
// the name "rsrgb2gray" for being able to instantiate it via e.g.
// gst::ElementFactory::make().
pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "rsrgb2gray",
        gst::Rank::NONE,
        Rgb2Gray::static_type(),
    )
}
```

This defines a Rust wrapper type for our element subclass. This is similar to `gst::Element` and others and provides the public interface to our element.

In addition, we also define a `register` function (the one that is already called from our `plugin_init` function). When `register` function is called it registers the element factory with GStreamer based on the type ID, to be able to create new instances of it with the name “rsrgb2gray” (e.g. when using [`gst::ElementFactory::make`](https://gstreamer.pages.freedesktop.org/gstreamer-rs/stable/latest/docs/gstreamer/struct.ElementFactory.html#method.make)). The `static_type` function will register the type with the GObject type system on the first call and the next time it's called (or on all the following calls) it will return the type ID.


## Type Class & Instance Initialization

As the next step we implement the `metadata` function and configure the `BaseTransform` class.

```rust
use gst::glib;

#[derive(Default)]
pub struct Rgb2Gray {}

impl Rgb2Gray {}

#[glib::object_subclass]
impl ObjectSubclass for Rgb2Gray {
    [...]
}

impl ObjectImpl for Rgb2Gray {}

impl GstObjectImpl for Rgb2Gray {}

impl ElementImpl for Rgb2Gray {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "RGB-GRAY Converter",
                "Filter/Effect/Converter/Video",
                "Converts RGB to GRAY or grayscale RGB",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }
}

impl BaseTransformImpl for Rgb2Gray {
    const MODE: gst_base::subclass::BaseTransformMode =
        gst_base::subclass::BaseTransformMode::NeverInPlace;
    const PASSTHROUGH_ON_SAME_CAPS: bool = false;
    const TRANSFORM_IP_ON_PASSTHROUGH: bool = false;
}
```

In the `metadata` function we set up `ElementMetadata` structure for our new element. In this case it contains a description, a classification of our element, a longer description and the author. The metadata can later be retrieved and made use of via the [`Registry`](https://gstreamer.pages.freedesktop.org/gstreamer-rs/stable/latest/docs/gstreamer/struct.Registry.html) and [`PluginFeature`](https://gstreamer.pages.freedesktop.org/gstreamer-rs/stable/latest/docs/gstreamer/struct.PluginFeature.html)/[`ElementFactory`](https://gstreamer.pages.freedesktop.org/gstreamer-rs/stable/latest/docs/gstreamer/struct.ElementFactory.html) API. We employ the `std::sync::LazyLock` to declare lazily initialized global variables, i.e. on the very first use the provided code will be executed and the result will be stored for all later uses.

We also implement the required `BaseTransformImpl` trait items, which define that we will never operate in-place (producing our output in the input buffer), and that we don’t want to work in passthrough mode if the input/output formats are the same.

With all this defined, `gst-inspect-1.0` should be able to show some more information about our element already but will still complain that it’s not complete yet.

**Side note:** This is the basic code that should be in `src/rgb2gray/imp.rs` and `src/rgb2gray/mod.rs` to successfully build the plugin. You can fill up the code while going through the later part of the tutorial.

```rust
// all imports...

#[derive(Default)]
pub struct Rgb2Gray {}

impl Rgb2Gray {}

#[glib::object_subclass]
impl ObjectSubclass for Rgb2Gray {
    const NAME: &'static str = "GstRsRgb2Gray";
    type Type = super::Rgb2Gray;
    type ParentType = gst_base::VideoFilter;
}

impl ObjectImpl for Rgb2Gray {}

impl GstObjectImpl for Rgb2Gray {}

impl ElementImpl for Rgb2Gray {}

impl BaseTransformImpl for Rgb2Gray {
    const MODE: gst_base::subclass::BaseTransformMode =
        gst_base::subclass::BaseTransformMode::NeverInPlace;
    const PASSTHROUGH_ON_SAME_CAPS: bool = false;
    const TRANSFORM_IP_ON_PASSTHROUGH: bool = false;
}

impl VideoFilterImpl for Rgb2Gray {}
```

```rust
use gst::glib;
use gst::prelude::*;

mod imp;

// The public Rust wrapper type for our element
glib::wrapper! {
    pub struct Rgb2Gray(ObjectSubclass<imp::Rgb2Gray>) @extends gst_base::BaseTransform, gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "rsrgb2gray",
        gst::Rank::NONE,
        Rgb2Gray::static_type(),
    )
}
```

## Debug Category

To be able to later have a debug category available for our new element that
can be used for logging, we again make use of the `use std::sync::LazyLock` type.

```rust
static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rsrgb2gray",
        gst::DebugColorFlags::empty(),
        Some("Rust RGB-GRAY converter"),
    )
});
```

We give the debug category the same name as our element, which generally is
the convention with GStreamer elements.

## Caps & Pad Templates

Data flow of GStreamer elements is happening via pads, which are the input(s) and output(s) (or sinks and sources in GStreamer terminology) of an element. Via the pads, buffers containing actual media data, events or queries are transferred. An element can have any number of sink and source pads, but our new element will only have one of each.

To be able to declare what kinds of pads an element can create (they are not necessarily all static but could be created at runtime by the element or the application), it is necessary to install so-called pad templates during the class initialization. These pad templates contain the name (or rather “name template”, it could be something like `src_%u` for e.g. pad templates that declare multiple possible pads), the direction of the pad (sink or source), the availability of the pad (is it always there, sometimes added/removed by the element or to be requested by the application) and all the possible media types (called caps) that the pad can consume (sink pads) or produce (src pads).

In our case we only have always pads, one sink pad called “sink”, on which we can only accept RGB (BGRx to be exact) data with any width/height/framerate and one source pad called “src”, on which we will produce either RGB (BGRx) data or GRAY8 (8-bit grayscale) data. We do this by implementing `pad_templates` method from `ElementImpl` trait.

```rust
    impl ElementImpl for Rgb2Gray {
        [...]

        fn pad_templates() -> &'static [gst::PadTemplate] {
            static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
                let caps = gst_video::VideoCapsBuilder::new()
                .format_list([gst_video::VideoFormat::Bgrx, gst_video::VideoFormat::Gray8])
                .build();
                let src_pad_template = gst::PadTemplate::new(
                    "src",
                    gst::PadDirection::Src,
                    gst::PadPresence::Always,
                    &caps,
                )
                .unwrap();

                let caps = gst_video::VideoCapsBuilder::new()
                    .format(gst_video::VideoFormat::Bgrx)
                    .build();
                let sink_pad_template = gst::PadTemplate::new(
                    "sink",
                    gst::PadDirection::Sink,
                    gst::PadPresence::Always,
                    &caps,
                )
                .unwrap();

                vec![src_pad_template, sink_pad_template]
            });

            PAD_TEMPLATES.as_ref()
        }
    }
```

The names “src” and “sink” are pre-defined by the `BaseTransform` class and this base-class will also create the actual pads with those names from the templates for us whenever a new element instance is created. Otherwise we would have to do that in our `new` function but here this is not needed.

If you now run gst-inspect-1.0 on the rsrgb2gray element, these pad templates with their caps should also show up.

## Caps Handling

As the next step we will add caps handling to our new element. It is required that we implement a function that is converting caps into the corresponding caps in the other direction. That is, we should convert BGRx to BGRx or GRAY8. Similarly, if the element downstream of ours can accept GRAY8 with a specific width/height from our source pad, we have to convert this to BGRx with that very same width/height. For example, if we receive BGRx caps with some width/height on the sinkpad, we should convert this into new caps with the same width/height but BGRx or GRAY8.

This has to be implemented in the `transform_caps` virtual method from the `BaseTransformImpl` trait, and looks as follows

```rust
impl BaseTransformImpl for Rgb2Gray {
    fn transform_caps(
        &self,
        direction: gst::PadDirection,
        caps: &gst::Caps,
        filter: Option<&gst::Caps>,
    ) -> Option<gst::Caps> {
        let other_caps = if direction == gst::PadDirection::Src {
            let mut caps = caps.clone();

            for s in caps.make_mut().iter_mut() {
                s.set("format", &gst_video::VideoFormat::Bgrx.to_str());
            }

            caps
        } else {
            let mut gray_caps = gst::Caps::new_empty();

            {
                let gray_caps = gray_caps.get_mut().unwrap();

                for s in caps.iter() {
                    let mut s_gray = s.to_owned();
                    s_gray.set("format", &gst_video::VideoFormat::Gray8.to_str());
                    gray_caps.append_structure(s_gray);
                }
                gray_caps.append(caps.clone());
            }

            gray_caps
        };

        gst::debug!(
            CAT,
            imp = self,
            "Transformed caps from {} to {} in direction {:?}",
            caps,
            other_caps,
            direction
        );

        if let Some(filter) = filter {
            Some(filter.intersect_with_mode(&other_caps, gst::CapsIntersectMode::First))
        } else {
            Some(other_caps)
        }
    }
}
```

This caps conversion happens in 3 steps. First we check if we got caps for the source pad. In that case, the caps on the other pad (the sink pad) are going to be exactly the same caps but no matter if the caps contained BGRx or GRAY8 they must become BGRx as that’s the only format that our sink pad can accept. We do this by creating a clone of the input caps, then making sure that those caps are actually writable (i.e. we’re having the only reference to them, or a copy is going to be created) and then iterate over all the structures inside the caps and then set the “format” field to BGRx. After this, all structures in the new caps will be with the format field set to BGRx.

Similarly, if we get caps for the sink pad and are supposed to convert it to caps for the source pad, we create new caps and in there append a copy of each structure of the input caps (which are BGRx) with the format field set to GRAY8. In the end we append the original caps, giving us first all caps as GRAY8 and then the same caps as BGRx. With this ordering we signal to GStreamer that we would prefer to output GRAY8 over BGRx.

In the end the caps we created for the other pad are filtered against optional filter caps to reduce the potential size of the caps. This is done by intersecting the caps with that filter, while keeping the order (and thus preferences) of the filter caps (`gst::CapsIntersectMode::First`).

## Conversion of BGRx Video Frames to Grayscale

Now that all the caps handling is implemented, we can finally get to the implementation of the actual video frame conversion. For this we start with defining a helper function `bgrx_to_gray` that converts one BGRx pixel to a grayscale value. The BGRx pixel is passed as a `&[u8]` slice with 4 elements and the function returns another `u8` for the grayscale value.

```rust
impl Rgb2Gray {
    #[inline]
    fn bgrx_to_gray(in_p: &[u8]) -> u8 {
        // See https://en.wikipedia.org/wiki/YUV#SDTV_with_BT.601
        const R_Y: u32 = 19595; // 0.299 * 65536
        const G_Y: u32 = 38470; // 0.587 * 65536
        const B_Y: u32 = 7471; // 0.114 * 65536

        assert_eq!(in_p.len(), 4);

        let b = u32::from(in_p[0]);
        let g = u32::from(in_p[1]);
        let r = u32::from(in_p[2]);

        let gray = ((r * R_Y) + (g * G_Y) + (b * B_Y)) / 65536;

        gray as u8
    }
}
```

This function works by extracting the blue, green and red components from each pixel (remember: we work on BGRx, so the first value will be blue, the second green, the third red and the fourth unused), extending it from 8 to 32 bits for a wider value-range and then converts it to the Y component of the YUV colorspace (basically what your grandparents’ black & white TV would’ve displayed). The coefficients come from the Wikipedia page about YUV and are normalized to unsigned 16 bit integers so we can keep some accuracy, don’t have to work with floating point arithmetic and stay inside the range of 32 bit integers for all our calculations. As you can see, the green component is weighted more than the others, which comes from our eyes being more sensitive to green than to other colors.

Note: This is only doing the actual conversion from linear RGB to grayscale (and in [BT.601](https://en.wikipedia.org/wiki/YUV#SDTV_with_BT.601) colorspace). To do this conversion correctly you need to know your colorspaces and use the correct coefficients for conversion, and also do [gamma correction](https://en.wikipedia.org/wiki/Gamma_correction). See [this](https://web.archive.org/web/20161024090830/http://www.4p8.com/eric.brasseur/gamma.html) about why it is important.

Afterwards we have to actually call this function on every pixel. For this the `transform_frame` virtual method from the `VideoFilterImpl` trait is implemented, which gets a input and output video frame passed and we’re supposed to read the input frame and fill the output frame. The implementation looks as follows, and is going to be our biggest function for this element

```rust
impl VideoFilterImpl for Rgb2Gray {
    fn transform_frame(
        &self,
        in_frame: &gst_video::VideoFrameRef<&gst::BufferRef>,
        out_frame: &mut gst_video::VideoFrameRef<&mut gst::BufferRef>,
    ) -> Result <gst::FlowSuccess, gst::FlowError> {
        let width = in_frame.width() as usize;
        let in_stride = in_frame.plane_stride()[0] as usize;
        let in_data = in_frame.plane_data(0).unwrap();
        let out_stride = out_frame.plane_stride()[0] as usize;
        let out_format = out_frame.format();
        let out_data = out_frame.plane_data_mut(0).unwrap();

        if out_format == gst_video::VideoFormat::Bgrx {
            assert_eq!(in_data.len() % 4, 0);
            assert_eq!(out_data.len() % 4, 0);
            assert_eq!(out_data.len() / out_stride, in_data.len() / in_stride);

            let in_line_bytes = width * 4;
            let out_line_bytes = width * 4;

            assert!(in_line_bytes <= in_stride);
            assert!(out_line_bytes <= out_stride);

            for (in_line, out_line) in in_data
                .chunks_exact(in_stride)
                .zip(out_data.chunks_exact_mut(out_stride))
            {
                for (in_p, out_p) in in_line[..in_line_bytes]
                    .chunks_exact(4)
                    .zip(out_line[..out_line_bytes].chunks_exact_mut(4))
                {
                    assert_eq!(out_p.len(), 4);

                    let gray = Rgb2Gray::bgrx_to_gray(in_p);
                    out_p[0] = gray;
                    out_p[1] = gray;
                    out_p[2] = gray;
                }
            }
        } else if out_format == gst_video::VideoFormat::Gray8 {
            assert_eq!(in_data.len() % 4, 0);
            assert_eq!(out_data.len() / out_stride, in_data.len() / in_stride);

            let in_line_bytes = width * 4;
            let out_line_bytes = width;

            assert!(in_line_bytes <= in_stride);
            assert!(out_line_bytes <= out_stride);

            for (in_line, out_line) in in_data
                .chunks_exact(in_stride)
                .zip(out_data.chunks_exact_mut(out_stride))
            {
                for (in_p, out_p) in in_line[..in_line_bytes]
                    .chunks_exact(4)
                    .zip(out_line[..out_line_bytes].iter_mut())
                {
                    let gray = Rgb2Gray::bgrx_to_gray(in_p);
                    *out_p = gray;
                }
            }
        } else {
            unimplemented!();
        }

        Ok(gst::FlowSuccess::Ok)
    }
}
```

First we store various information we’re going to need later in local variables to save some typing later. This is the width (same for input and output as we never changed the width in `transform_caps`), the input and out (row-) stride (the number of bytes per row/line, which possibly includes some padding at the end of each line for alignment reasons), the output format (which can be BGRx or GRAY8 because of how we implemented `transform_caps`) and the pointers to the first plane of the input and output (which in this case also is the only plane, BGRx and GRAY8 both have only a single plane containing all the RGB/gray components).

Then based on whether the output suppose to be BGRx or GRAY8, we iterate over all pixels. The code is basically the same in both cases, so we're only going to go through the case where BGRx is output.

We start by iterating over each line of the input and output, and do so by using the [`chunks_exact`](https://doc.rust-lang.org/std/primitive.slice.html#method.chunks_exact) iterator and [`chunks_exact_mut`](https://doc.rust-lang.org/std/primitive.slice.html#method.chunks_exact_mut) to give us chunks of as many bytes as the (row-) stride of the video frame is, do the same for the other frame and then zip both iterators together. This means that on each iteration we get exactly one line as a slice from each of the frames and can then start accessing the actual pixels in each line. The only difference of `chunks_exact_mut` from `chunks_exact` is that it gives the mutable sub-slices so that their content can be changed.

To access the individual pixels in each line, we again use the chunks iterator the same way, but this time to always give us chunks of 4 bytes from each line. As BGRx uses 4 bytes for each pixel, this gives us exactly one pixel. Instead of iterating over the whole line, we only take the actual sub-slice that contains the pixels, not the whole line with stride number of bytes containing potential padding at the end. Now for each of these pixels we call our previously defined `bgrx_to_gray` function and then fill the B, G and R components of the output buffer with that value to get grayscale output. And that’s all.

Using Rust high-level abstractions like the chunks iterators and bounds-checking slice accesses might seem like it’s going to cause quite some performance penalty, but if you look at the generated assembly most of the bounds checks are completely optimized away and the resulting assembly code is close to what one would’ve written manually. Here you’re getting safe and high-level looking code with low-level performance!

You might’ve also noticed the various assertions in the processing function. These are there to document the assumptions in the code and to give further hints to the compiler about properties of the code, and thus potentially being able to optimize the code better and moving e.g. bounds checks out of the inner loop and just having the assertion outside the loop check for the same. In Rust adding assertions can often improve performance by allowing further optimizations to be applied, but in the end always check the resulting assembly to see if what you did made any difference.

## Testing the new element

Now we implemented almost all functionality of our new element and could run it on actual video data. This can be done now with the `gst-launch-1.0` tool, or any application using GStreamer and allowing us to insert our new element somewhere in the video part of the pipeline. With `gst-launch-1.0` you could run for example the following pipelines

```bash
# Run on a test pattern
gst-launch-1.0 videotestsrc ! rsrgb2gray ! videoconvert ! autovideosink

# Run on some video file, also playing the audio
gst-launch-1.0 playbin uri=file:///path/to/some/file video-filter=rsrgb2gray
```

Note that you will likely want to compile with `cargo build --release` and add the `target/release` directory to `GST_PLUGIN_PATH` instead. The debug build might be too slow, and generally the release builds are multiple orders of magnitude (!) faster.

## Properties

The only feature missing now are the properties mentioned in the opening paragraph: one boolean property to invert the grayscale value and one integer property to shift the value by up to 255. Implementing this on top of the previous code is not a lot of work. Let’s start with defining a struct for holding the property values and defining the property metadata.

```rust
const DEFAULT_INVERT: bool = false;
const DEFAULT_SHIFT: u32 = 0;

#[derive(Debug, Clone, Copy)]
struct Settings {
    invert: bool,
    shift: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            invert: DEFAULT_INVERT,
            shift: DEFAULT_SHIFT,
        }
    }
}

#[derive(Default)]
pub struct Rgb2Gray {
    settings: Mutex<Settings>,
    state: Mutex<Option<State>>,
}

impl Rgb2Gray{...}

impl ObjectImpl for Rgb2Gray {
    fn properties() -> &'static [glib::ParamSpec] {
        // Metadata for the properties
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecBoolean::builder("invert")
                    .nick("Invert")
                    .blurb("Invert grayscale output")
                    .default_value(DEFAULT_INVERT)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecUInt::builder("shift")
                    .nick("Shift")
                    .blurb("Shift grayscale output (wrapping around)")
                    .maximum(255)
                    .default_value(DEFAULT_SHIFT)
                    .mutable_playing()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }
}
```

This should all be rather straightforward: we define a `Settings` struct that stores the two values, implement the [`Default`](https://doc.rust-lang.org/nightly/std/default/trait.Default.html) trait for it, then define a two-element array with property metadata (names, description, ranges, default value, writability), and then store the default value of our `Settings` struct inside another `Mutex` inside the element struct.

In the next step we have to implement functions that are called whenever a property value is set or get.

```rust
impl ObjectImpl for Rgb2Gray {
    [...]

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "invert" => {
                let mut settings = self.settings.lock().unwrap();
                let invert = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing invert from {} to {}",
                    settings.invert,
                    invert
                );
                settings.invert = invert;
            }
            "shift" => {
                let mut settings = self.settings.lock().unwrap();
                let shift = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing shift from {} to {}",
                    settings.shift,
                    shift
                );
                settings.shift = shift;
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "invert" => {
                let settings = self.settings.lock().unwrap();
                settings.invert.to_value()
            }
            "shift" => {
                let settings = self.settings.lock().unwrap();
                settings.shift.to_value()
            }
            _ => unimplemented!(),
        }
    }
}
```

Property values can be changed from any thread at any time, that’s why the `Mutex` is needed here to protect our struct. And we’re using a new mutex to be able to have it locked only for the shorted possible amount of time: we don’t want to keep it locked for the whole time of the `transform` function, otherwise applications trying to set/get values would block for up to the processing time of one frame.

In the property setter/getter functions we are working with a `glib::Value`. This is a dynamically typed value type that can contain values of any type, together with the type information of the contained value. Here we’re using it to handle an unsigned integer (`u32`) and a boolean for our two properties. To know which property is currently set/get, we get an identifier passed which is the index into our `PROPERTIES` array. We then simply match on the name of that to decide which property was meant

With this implemented, we can already compile everything, see the properties and their metadata in `gst-inspect-1.0` and can also set them on `gst-launch-1.0` like this

```bash
# Set invert to true and shift to 128
gst-launch-1.0 videotestsrc ! rsrgb2gray invert=true shift=128 ! videoconvert ! autovideosink
```

If we set `GST_DEBUG=rsrgb2gray:6` in the environment before running that, we can also see the corresponding debug output when the values are changing. The only thing missing now is to actually make use of the property values for the processing. For this we add the following changes to `bgrx_to_gray` and the transform function

```rust
impl Rgb2Gray {
    #[inline]
    fn bgrx_to_gray(in_p: &[u8], shift: u8, invert: bool) -> u8 {
        [...]

        let gray = ((r * R_Y) + (g * G_Y) + (b * B_Y)) / 65536;
        let gray = (gray as u8).wrapping_add(shift);

        if invert {
            255 - gray
        } else {
            gray
        }
    }
}

impl BaseTransformImpl for Rgb2Gray {
    fn transform(
        &self,
        inbuf: &gst::Buffer,
        outbuf: &mut gst::BufferRef,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let settings = *self.settings.lock().unwrap();
        [...]
                    let gray = Rgb2Gray::bgrx_to_gray(in_p, settings.shift as u8, settings.invert);
        [...]
    }
}
```


And that’s all. If you run the element in `gst-launch-1.0` and change the values of the properties you should also see the corresponding changes in the video output.

Note that we always take a copy of the `Settings` struct at the beginning of the transform function. This ensures that we take the mutex only the shortest possible amount of time and then have a local snapshot of the settings for each frame.

Also keep in mind that the usage of the property values in the `bgrx_to_gray` function is far from optimal. It means the addition of another condition to the calculation of each pixel, thus potentially slowing it down a lot. Ideally this condition would be moved outside the inner loops and the `bgrx_to_gray` function would made generic over that. See for example [this blog post](https://bluejekyll.github.io/blog/rust/2018/01/10/branchless-rust.html) about “branchless Rust” for ideas how to do that, the actual implementation is left as an exercise for the reader.

## What next?

We hope the code walkthrough above was useful to understand how to implement GStreamer plugins and elements in Rust. If you have any questions, feel free to ask them in the IRC channel (#gstreamer on freenode) and on our [mailing list](https://lists.freedesktop.org/mailman/listinfo/gstreamer-devel).

The same approach also works for audio filters or anything that can be handled in some way with the API of the `BaseTransform` base class. You can find another filter, an audio echo filter, using the same approach [here](https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/blob/main/gst-plugin-audiofx/src/audioecho.rs).

In the [next tutorial](tutorial-2.md) in this series we’ll discuss how to use another base class to implement another kind of element, but for the time being you can also check the [git repository](https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs) for various other element implementations.


