// Copyright (C) 2019 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;

use gtk::prelude::*;

use std::cell::RefCell;

const MAIN_PIPELINE: &str = "videotestsrc is-live=true pattern=ball";
const FALLBACK_PIPELINE: &str = "videotestsrc is-live=true pattern=snow";

//const MAIN_PIPELINE: &str = "videotestsrc is-live=true pattern=ball ! x264enc tune=zerolatency";
//const FALLBACK_PIPELINE: &str = "videotestsrc is-live=true pattern=snow ! x264enc tune=zerolatency";

fn create_pipeline() -> (gst::Pipeline, gst::Pad, gst::Element) {
    let pipeline = gst::Pipeline::default();

    let video_src = gst::parse::bin_from_description(MAIN_PIPELINE, true)
        .unwrap()
        .upcast();
    let fallback_video_src = gst::parse::bin_from_description(FALLBACK_PIPELINE, true)
        .unwrap()
        .upcast();

    let fallbackswitch = gst::ElementFactory::make("fallbackswitch")
        .property("timeout", gst::ClockTime::SECOND)
        .build()
        .unwrap();

    let decodebin = gst::ElementFactory::make("decodebin").build().unwrap();
    let videoconvert = gst::ElementFactory::make("videoconvert").build().unwrap();

    let videoconvert_clone = videoconvert.clone();
    decodebin.connect_pad_added(move |_, pad| {
        let caps = pad.current_caps().unwrap();
        let s = caps.structure(0).unwrap();

        let sinkpad = videoconvert_clone.static_pad("sink").unwrap();

        if s.name() == "video/x-raw" && !sinkpad.is_linked() {
            pad.link(&sinkpad).unwrap();
        }
    });

    let video_sink = gst::ElementFactory::make("gtk4paintablesink")
        .build()
        .unwrap();

    pipeline
        .add_many([
            &video_src,
            &fallback_video_src,
            &fallbackswitch,
            &decodebin,
            &videoconvert,
            &video_sink,
        ])
        .unwrap();

    /* The first pad requested will be automatically preferred */
    video_src
        .link_pads(Some("src"), &fallbackswitch, Some("sink_%u"))
        .unwrap();
    fallback_video_src
        .link_pads(Some("src"), &fallbackswitch, Some("sink_%u"))
        .unwrap();
    fallbackswitch
        .link_pads(Some("src"), &decodebin, Some("sink"))
        .unwrap();
    videoconvert
        .link_pads(Some("src"), &video_sink, Some("sink"))
        .unwrap();

    (pipeline, video_src.static_pad("src").unwrap(), video_sink)
}

fn create_ui(app: &gtk::Application) {
    let (pipeline, video_src_pad, video_sink) = create_pipeline();

    let window = gtk::ApplicationWindow::new(app);
    window.set_default_size(320, 240);
    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let picture = gtk::Picture::new();
    let paintable = video_sink.property::<gtk::gdk::Paintable>("paintable");
    picture.set_paintable(Some(&paintable));
    vbox.append(&picture);

    let position_label = gtk::Label::new(Some("Position: 00:00:00"));
    vbox.append(&position_label);

    let drop_button = gtk::ToggleButton::with_label("Drop Signal");
    vbox.append(&drop_button);

    window.set_child(Some(&vbox));
    window.present();

    app.add_window(&window);

    let video_sink_weak = video_sink.downgrade();
    let timeout_id = glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        let Some(video_sink) = video_sink_weak.upgrade() else {
            return glib::ControlFlow::Break;
        };

        let position = video_sink
            .query_position::<gst::ClockTime>()
            .unwrap_or(gst::ClockTime::ZERO);
        position_label.set_text(&format!("Position: {position:.1}"));

        glib::ControlFlow::Continue
    });

    let video_src_pad_weak = video_src_pad.downgrade();
    let drop_id = RefCell::new(None);
    drop_button.connect_toggled(move |drop_button| {
        let Some(video_src_pad) = video_src_pad_weak.upgrade() else {
            return;
        };

        let drop = drop_button.is_active();
        if drop {
            let mut drop_id = drop_id.borrow_mut();
            if drop_id.is_none() {
                *drop_id = video_src_pad
                    .add_probe(gst::PadProbeType::BUFFER, |_, _| gst::PadProbeReturn::Drop);
            }
        } else if let Some(drop_id) = drop_id.borrow_mut().take() {
            video_src_pad.remove_probe(drop_id);
        }
    });

    let app_weak = app.downgrade();
    window.connect_close_request(move |_| {
        let Some(app) = app_weak.upgrade() else {
            return glib::Propagation::Stop;
        };

        app.quit();
        glib::Propagation::Stop
    });

    let bus = pipeline.bus().unwrap();
    let app_weak = app.downgrade();
    let bus_watch = bus
        .add_watch_local(move |_, msg| {
            use gst::MessageView;

            let Some(app) = app_weak.upgrade() else {
                return glib::ControlFlow::Break;
            };

            match msg.view() {
                MessageView::Eos(..) => app.quit(),
                MessageView::Error(err) => {
                    println!(
                        "Error from {:?}: {} ({:?})",
                        msg.src().map(|s| s.path_string()),
                        err.error(),
                        err.debug()
                    );
                    app.quit();
                }
                _ => (),
            };

            glib::ControlFlow::Continue
        })
        .expect("Failed to add bus watch");

    pipeline.set_state(gst::State::Playing).unwrap();

    // Pipeline reference is owned by the closure below, so will be
    // destroyed once the app is destroyed
    let timeout_id = RefCell::new(Some(timeout_id));
    let bus_watch = RefCell::new(Some(bus_watch));
    app.connect_shutdown(move |_| {
        drop(bus_watch.borrow_mut().take());
        pipeline.set_state(gst::State::Null).unwrap();

        if let Some(timeout_id) = timeout_id.borrow_mut().take() {
            timeout_id.remove();
        }
    });
}

fn main() -> glib::ExitCode {
    gst::init().unwrap();
    gtk::init().unwrap();

    gstfallbackswitch::plugin_register_static().expect("Failed to register fallbackswitch plugin");
    gstgtk4::plugin_register_static().expect("Failed to register gtk4paintablesink plugin");

    let app = gtk::Application::new(None::<&str>, gio::ApplicationFlags::FLAGS_NONE);

    app.connect_activate(create_ui);
    app.run()
}
