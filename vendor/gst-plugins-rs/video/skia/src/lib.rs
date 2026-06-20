// SPDX-License-Identifier: MPL-2.0

#![allow(clippy::non_send_fields_in_send_ty, unused_doc_comments)]

/**
 * plugin-skia:
 *
 * Since: plugins-rs-0.14.0
 */
use gst::glib;

mod compositor;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    compositor::register(plugin)?;
    #[cfg(feature = "doc")]
    {
        use gst::prelude::*;

        compositor::Background::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
        compositor::SkiaCompositorPad::static_type()
            .mark_as_plugin_api(gst::PluginAPIFlags::empty());
        compositor::Operator::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }
    Ok(())
}

gst::plugin_define!(
    skia,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "MIT/X11",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);
