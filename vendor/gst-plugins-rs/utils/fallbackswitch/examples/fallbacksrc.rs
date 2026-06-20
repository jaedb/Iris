// Copyright (C) 2025 Sanchayan Maity <sanchayan@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use clap::Parser;
use futures::prelude::*;
use gst::glib;
use gst::prelude::*;
use std::str::FromStr;
use std::sync::LazyLock;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "gst-fallbacksrc-example",
        gst::DebugColorFlags::empty(),
        Some("gst-fallbacksrc-example"),
    )
});

#[derive(Parser, Debug)]
struct Cli {
    #[clap(long, short, action)]
    uri: String,
    #[clap(long, short, action)]
    fallback_uri: String,
    #[clap(long, short, action)]
    audio_caps: Option<String>,
    #[clap(long, short, action)]
    video_caps: Option<String>,
}

fn sink_bin(is_encoded: bool, is_video: bool) -> gst::Bin {
    let bin = gst::Bin::new();

    let queue = gst::ElementFactory::make("queue")
        // Drop old buffers if we are backed up
        .property_from_str("leaky", "downstream")
        .property("max-size-bytes", 0u32)
        .property("max-size-buffers", 0u32)
        // About enough for about 5 seconds of 1080p @ 20fps raw video
        .property("max-size-time", 5.seconds())
        .build()
        .unwrap();
    let decoder = if is_encoded {
        gst::ElementFactory::make("decodebin").build().unwrap()
    } else {
        gst::ElementFactory::make("identity").build().unwrap()
    };
    let decoder_queue = gst::ElementFactory::make("queue")
        // Drop old buffers if we are backed up
        .property_from_str("leaky", "downstream")
        .property("max-size-bytes", 0u32)
        .property("max-size-buffers", 0u32)
        // About enough for about 5 seconds of 1080p @ 20fps raw video
        .property("max-size-time", 5.seconds())
        .build()
        .unwrap();

    let (convert, sink) = if is_video {
        (
            gst::ElementFactory::make("videoconvert").build().unwrap(),
            gst::ElementFactory::make("autovideosink").build().unwrap(),
        )
    } else {
        (
            gst::ElementFactory::make("audioconvert").build().unwrap(),
            gst::ElementFactory::make("autoaudiosink").build().unwrap(),
        )
    };

    bin.add_many([&queue, &decoder, &decoder_queue, &convert, &sink])
        .unwrap();

    if is_encoded {
        let decoder_queue_sink_pad = decoder_queue
            .static_pad("sink")
            .expect("queue must have a sink pad");
        decoder.connect_pad_added(glib::clone!(move |decodebin, pad| {
            pad.link(&decoder_queue_sink_pad).unwrap();
            let _ = decodebin.sync_state_with_parent();
        }));
    }

    queue
        .link(&decoder)
        .expect("Failed to link queue to decoder");

    if !is_encoded {
        decoder
            .link(&decoder_queue)
            .expect("Failed to link decoder to queue");
    }

    decoder_queue
        .link(&convert)
        .expect("Failed to link queue to converter");
    convert
        .link(&sink)
        .expect("Failed to link converter to sink");

    let queue_sink_pad = queue
        .static_pad("sink")
        .expect("queue must have a sink pad");
    let sink_pad_tmpl = queue
        .pad_template("sink")
        .expect("Failed to get sink pad template");

    let sink_pad =
        gst::GhostPad::builder_from_template_with_target(&sink_pad_tmpl, &queue_sink_pad)
            .unwrap()
            .build();

    bin.add_pad(&sink_pad).expect("Failed to add sink pad");

    bin
}

fn handle_caps(pipeline: &gst::Pipeline, src_pad: &gst::Pad) {
    let caps = match src_pad.stream().and_then(|stream| stream.caps()) {
        Some(caps) => caps,
        None => {
            return;
        }
    };
    let s = match caps.structure(0) {
        Some(s) => s,
        None => {
            return;
        }
    };
    let src_pad_name = src_pad.name();
    let codec = s.name().as_str();

    let bin = if codec.starts_with("audio/x-raw") {
        gst::info!(CAT, "Handling raw audio for pad {}", src_pad_name);
        sink_bin(false, false)
    } else if codec.starts_with("video/x-raw") {
        gst::info!(CAT, "Handling raw video for pad {}", src_pad_name);
        sink_bin(false, true)
    } else if codec.starts_with("video/") {
        gst::info!(CAT, "Handling encoded video for pad {}", src_pad_name);
        sink_bin(true, true)
    } else {
        gst::info!(CAT, "Handling encoded audio for pad {}", src_pad_name);
        sink_bin(true, false)
    };

    pipeline.add(&bin).unwrap();
    bin.sync_state_with_parent().unwrap();

    let sink_pad = bin.static_pad("sink").unwrap();
    src_pad.link(&sink_pad).unwrap();

    let src_pad_name = src_pad.name();
    // Capture the pipeline graph a second later so that all pads can
    // be seen with caps negotiated.
    glib::timeout_add_seconds_once(
        1,
        glib::clone!(
            #[weak]
            pipeline,
            move || {
                pipeline.debug_to_dot_file(
                    gst::DebugGraphDetails::all(),
                    format!("caps-handled-on-pad-{}", src_pad_name),
                );
            }
        ),
    );
}

fn main() {
    gst::init().unwrap();

    gstfallbackswitch::plugin_register_static().expect("Failed to register fallbacksrc plugin");

    let cli = Cli::parse();
    let pipeline = gst::Pipeline::new();
    let context = glib::MainContext::default();

    let fallbacksrc = gst::ElementFactory::make("fallbacksrc")
        .property("timeout", gst::ClockTime::from_seconds(5))
        // Main source might have a higher latency than the fallback source
        .property("min-latency", gst::ClockTime::from_mseconds(100))
        .property("uri", cli.uri)
        .property("fallback-uri", cli.fallback_uri)
        .property("restart-on-eos", true)
        .build()
        .unwrap();

    if let Some(caps) = cli.audio_caps {
        if let Ok(c) = gst::Caps::from_str(&caps) {
            fallbacksrc.set_property("audio-caps", c);
        } else {
            gst::error!(CAT, "Invalid audio caps");
            return;
        }
    }

    if let Some(caps) = cli.video_caps {
        if let Ok(c) = gst::Caps::from_str(&caps) {
            fallbacksrc.set_property("video-caps", c);
        } else {
            gst::error!(CAT, "Invalid video caps");
            return;
        }
    }

    fallbacksrc.connect_pad_added(glib::clone!(
        #[weak]
        pipeline,
        move |_, src_pad| {
            let caps = src_pad
                .stream()
                .and_then(|stream| stream.caps())
                .expect("Expect stream caps to be valid here");

            gst::info!(CAT, "{} added with caps {caps:?}", src_pad.name());

            if !src_pad.is_linked() {
                handle_caps(&pipeline, src_pad);
            }

            // `fallbacksrc` has a dummy raw audio and video source
            // along with the primary and fallback source connected
            // to downstream via a `fallbackswitch`. Caps can change
            // between either of primary, fallback and dummy source
            // based on currently `healthy` stream. Handle the caps
            // change and plug in the appropriate sink downstream.
            src_pad.connect_caps_notify(glib::clone!(
                #[weak]
                pipeline,
                move |src_pad| {
                    gst::info!(CAT, "{} caps changed {caps:?}", src_pad.name());

                    if let Some(peer_pad) = src_pad.peer() {
                        if src_pad.unlink(&peer_pad).is_err() {
                            gst::error!(
                                CAT,
                                "Failed to unlink {} and {}",
                                src_pad.name(),
                                peer_pad.name()
                            );
                        }

                        if let Some(el) = peer_pad
                            .parent()
                            .and_then(|p| p.downcast::<gst::Element>().ok())
                        {
                            let _ = pipeline.remove(&el);
                            let _ = el.set_state(gst::State::Null);
                        }
                    }

                    handle_caps(&pipeline, src_pad);
                }
            ));
        }
    ));

    pipeline.add(&fallbacksrc).unwrap();

    pipeline.set_state(gst::State::Playing).unwrap();

    context.block_on(async {
        let bus = pipeline.bus().expect("Pipeline should have a bus");
        let mut messages = bus.stream();

        while let Some(msg) = messages.next().await {
            use gst::MessageView;

            match msg.view() {
                MessageView::Eos(..) => {
                    gst::info!(
                        CAT,
                        "EOS from {}",
                        msg.src()
                            .map(|s| String::from(s.path_string()))
                            .unwrap_or_else(|| "None".into())
                    );
                    break;
                }
                MessageView::StateChanged(sc) => {
                    if msg.src() == Some(pipeline.upcast_ref()) {
                        pipeline.debug_to_dot_file(
                            gst::DebugGraphDetails::all(),
                            format!("{}-{:?}-{:?}", pipeline.name(), sc.old(), sc.current()),
                        );
                    }
                }
                MessageView::Error(err) => {
                    pipeline.debug_to_dot_file(gst::DebugGraphDetails::ALL, "fallbacksrc-error");
                    gst::info!(
                        CAT,
                        "Got error from {}: {} ({})",
                        msg.src()
                            .map(|s| String::from(s.path_string()))
                            .unwrap_or_else(|| "None".into()),
                        err.error(),
                        err.debug().unwrap_or_else(|| "".into()),
                    );
                    break;
                }
                _ => (),
            };
        }
    });

    pipeline.set_state(gst::State::Null).unwrap();
}
