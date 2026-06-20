// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use std::sync::{LazyLock, OnceLock};
use std::time::Duration;
use tokio::runtime;
mod config;
mod internal;
mod jitterbuffer;
mod rtprecv;
mod rtpsend;
mod session;
mod source;
mod sync;
mod time;

glib::wrapper! {
    pub struct RtpSend(ObjectSubclass<rtpsend::RtpSend>) @extends gst::Element, gst::Object;
}
glib::wrapper! {
    pub struct RtpRecv(ObjectSubclass<rtprecv::RtpRecv>) @extends gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    {
        crate::rtpbin2::sync::TimestampingMode::static_type()
            .mark_as_plugin_api(gst::PluginAPIFlags::empty());
        crate::rtpbin2::config::Rtp2Session::static_type()
            .mark_as_plugin_api(gst::PluginAPIFlags::empty());
        crate::rtpbin2::rtpsend::Profile::static_type()
            .mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }
    gst::Element::register(
        Some(plugin),
        "rtpsend",
        gst::Rank::NONE,
        RtpSend::static_type(),
    )?;
    gst::Element::register(
        Some(plugin),
        "rtprecv",
        gst::Rank::NONE,
        RtpRecv::static_type(),
    )
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtpbin2",
        gst::DebugColorFlags::empty(),
        Some("RTP bin2 plugin"),
    )
});

/// Number of worker threads the Runtime will use (default: 1)
///
/// 0 => number of cores available on the system
const RUNTIME_WORKER_THREADS_ENV_VAR: &str = "GST_RTPBIN2_RT_WORKER_THREADS";

/// Limit for the number of threads in the blocking pool (default: 512)
const RUNTIME_MAX_BLOCKING_THREADS_ENV_VAR: &str = "GST_RTPBIN2_RT_MAX_BLOCKING_THREADS";

/// Timeout for a thread in the blocking pool in ms (default: 10s)
const RUNTIME_THREAD_KEEP_ALIVE_MS: &str = "GST_RTPBIN2_RT_THREAD_KEEP_ALIVE";

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum RuntimeError {
    #[error("Invalid value for env var {env_var}")]
    InvalidEnvVar { env_var: &'static str },
}

static RUNTIME: OnceLock<Result<runtime::Runtime, RuntimeError>> = OnceLock::new();

pub fn get_or_init_runtime<'a>() -> Result<&'a runtime::Runtime, &'a RuntimeError> {
    RUNTIME
        .get_or_init(|| {
            fn maybe_set_env_var<T, S, D>(
                builder: &mut runtime::Builder,
                env_var: &'static str,
                setter: S,
                set_default: D,
            ) -> Result<(), RuntimeError>
            where
                T: std::str::FromStr + std::fmt::Display,
                S: Fn(&mut runtime::Builder, T) -> &mut runtime::Builder,
                D: Fn(&mut runtime::Builder) -> &mut runtime::Builder,
            {
                match std::env::var(env_var) {
                    Ok(val) => {
                        let Ok(val) = val.parse() else {
                            return Err(RuntimeError::InvalidEnvVar { env_var });
                        };
                        gst::info!(CAT, "Runtime: {env_var} defined => using value {val}");
                        setter(builder, val);
                    }
                    Err(std::env::VarError::NotPresent) => {
                        gst::info!(CAT, "Runtime: {env_var} undefined => using default");
                        set_default(builder);
                    }
                    _ => return Err(RuntimeError::InvalidEnvVar { env_var }),
                }

                Ok(())
            }

            fn init() -> Result<runtime::Runtime, RuntimeError> {
                let mut builder = runtime::Builder::new_multi_thread();
                builder.enable_time();

                maybe_set_env_var(
                    &mut builder,
                    RUNTIME_WORKER_THREADS_ENV_VAR,
                    |builder, val| builder.worker_threads(val),
                    |builder| builder.worker_threads(1),
                )?;

                maybe_set_env_var(
                    &mut builder,
                    RUNTIME_MAX_BLOCKING_THREADS_ENV_VAR,
                    |builder, val| builder.max_blocking_threads(val),
                    |builder| builder.max_blocking_threads(512),
                )?;

                maybe_set_env_var(
                    &mut builder,
                    RUNTIME_THREAD_KEEP_ALIVE_MS,
                    |builder, val| builder.thread_keep_alive(Duration::from_millis(val)),
                    |builder| builder.thread_keep_alive(Duration::from_secs(10)),
                )?;

                Ok(builder.build().unwrap())
            }

            init()
        })
        .as_ref()
}
