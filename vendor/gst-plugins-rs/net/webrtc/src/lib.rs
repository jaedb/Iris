// SPDX-License-Identifier: MPL-2.0
#![allow(clippy::non_send_fields_in_send_ty, unused_doc_comments)]

/**
 * plugin-rswebrtc:
 * @title: Rust WebRTC elements
 * @short_description: A collection of high level WebRTC elements wrapping webrtcbin
 *
 * {{ net/webrtc/README.md[2:233] }}
 *
 * Since: plugins-rs-0.9
 */
use gst::glib;
use std::sync::LazyLock;
use tokio::runtime;

#[cfg(feature = "aws")]
mod aws_kvs_signaller;
#[cfg(feature = "janus")]
mod janusvr_signaller;
#[cfg(feature = "livekit")]
mod livekit_signaller;
pub mod signaller;
pub mod utils;
pub mod webrtcsink;
pub mod webrtcsrc;
#[cfg(feature = "whep")]
mod whep_signaller;
#[cfg(feature = "whip")]
mod whip_signaller;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    webrtcsink::register(plugin)?;
    webrtcsrc::register(Some(plugin))?;

    Ok(())
}

gst::plugin_define!(
    rswebrtc,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "MPL-2.0",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);

pub static RUNTIME: LazyLock<runtime::Runtime> = LazyLock::new(|| {
    runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(1)
        .build()
        .unwrap()
});
