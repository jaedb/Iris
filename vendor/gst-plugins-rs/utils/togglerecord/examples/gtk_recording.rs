// Copyright (C) 2017 Sebastian Dröge <sebastian@centricular.com>
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

fn create_pipeline() -> (
    gst::Pipeline,
    gst::Pad,
    gst::Pad,
    gst::Element,
    gst::Element,
) {
    let pipeline = gst::Pipeline::default();

    let video_src = gst::ElementFactory::make("videotestsrc")
        .property("is-live", true)
        .property_from_str("pattern", "ball")
        .build()
        .unwrap();

    let timeoverlay = gst::ElementFactory::make("timeoverlay")
        .property("font-desc", "Monospace 20")
        .build()
        .unwrap();

    let video_tee = gst::ElementFactory::make("tee").build().unwrap();
    let video_queue1 = gst::ElementFactory::make("queue").build().unwrap();
    let video_queue2 = gst::ElementFactory::make("queue").build().unwrap();

    let video_convert1 = gst::ElementFactory::make("videoconvert").build().unwrap();
    let video_convert2 = gst::ElementFactory::make("videoconvert").build().unwrap();

    let video_sink = gst::ElementFactory::make("gtk4paintablesink")
        .build()
        .unwrap();

    let video_enc = gst::ElementFactory::make("x264enc")
        .property("rc-lookahead", 10i32)
        .property("key-int-max", 30u32)
        .build()
        .unwrap();
    let video_parse = gst::ElementFactory::make("h264parse").build().unwrap();

    let audio_src = gst::ElementFactory::make("audiotestsrc")
        .property("is-live", true)
        .property_from_str("wave", "ticks")
        .build()
        .unwrap();

    let audio_tee = gst::ElementFactory::make("tee").build().unwrap();
    let audio_queue1 = gst::ElementFactory::make("queue").build().unwrap();
    let audio_queue2 = gst::ElementFactory::make("queue").build().unwrap();

    let audio_convert1 = gst::ElementFactory::make("audioconvert").build().unwrap();
    let audio_convert2 = gst::ElementFactory::make("audioconvert").build().unwrap();

    let audio_sink = gst::ElementFactory::make("autoaudiosink").build().unwrap();

    let audio_enc = gst::ElementFactory::make("lamemp3enc").build().unwrap();
    let audio_parse = gst::ElementFactory::make("mpegaudioparse").build().unwrap();

    let togglerecord = gst::ElementFactory::make("togglerecord").build().unwrap();

    let mux_queue1 = gst::ElementFactory::make("queue").build().unwrap();
    let mux_queue2 = gst::ElementFactory::make("queue").build().unwrap();

    let mux = gst::ElementFactory::make("mp4mux").build().unwrap();

    let file_sink = gst::ElementFactory::make("filesink")
        .property("location", "recording.mp4")
        .property("async", false)
        .property("sync", false)
        .build()
        .unwrap();

    pipeline
        .add_many([
            &video_src,
            &timeoverlay,
            &video_tee,
            &video_queue1,
            &video_queue2,
            &video_convert1,
            &video_convert2,
            &video_sink,
            &video_enc,
            &video_parse,
            &audio_src,
            &audio_tee,
            &audio_queue1,
            &audio_queue2,
            &audio_convert1,
            &audio_convert2,
            &audio_sink,
            &audio_enc,
            &audio_parse,
            &togglerecord,
            &mux_queue1,
            &mux_queue2,
            &mux,
            &file_sink,
        ])
        .unwrap();

    gst::Element::link_many([
        &video_src,
        &timeoverlay,
        &video_tee,
        &video_queue1,
        &video_convert1,
        &video_sink,
    ])
    .unwrap();

    gst::Element::link_many([
        &video_tee,
        &video_queue2,
        &video_convert2,
        &video_enc,
        &video_parse,
    ])
    .unwrap();

    video_parse
        .link_pads(Some("src"), &togglerecord, Some("sink"))
        .unwrap();
    togglerecord
        .link_pads(Some("src"), &mux_queue1, Some("sink"))
        .unwrap();
    mux_queue1
        .link_pads(Some("src"), &mux, Some("video_%u"))
        .unwrap();

    gst::Element::link_many([
        &audio_src,
        &audio_tee,
        &audio_queue1,
        &audio_convert1,
        &audio_sink,
    ])
    .unwrap();

    gst::Element::link_many([
        &audio_tee,
        &audio_queue2,
        &audio_convert2,
        &audio_enc,
        &audio_parse,
    ])
    .unwrap();

    audio_parse
        .link_pads(Some("src"), &togglerecord, Some("sink_0"))
        .unwrap();
    togglerecord
        .link_pads(Some("src_0"), &mux_queue2, Some("sink"))
        .unwrap();
    mux_queue2
        .link_pads(Some("src"), &mux, Some("audio_%u"))
        .unwrap();

    gst::Element::link_many([&mux, &file_sink]).unwrap();

    (
        pipeline,
        video_queue2.static_pad("sink").unwrap(),
        audio_queue2.static_pad("sink").unwrap(),
        togglerecord,
        video_sink,
    )
}

fn create_ui(app: &gtk::Application) {
    let (pipeline, video_pad, audio_pad, togglerecord, video_sink) = create_pipeline();

    let window = gtk::ApplicationWindow::new(app);
    window.set_default_size(320, 240);

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let picture = gtk::Picture::new();
    let paintable = video_sink.property::<gtk::gdk::Paintable>("paintable");
    picture.set_paintable(Some(&paintable));
    vbox.append(&picture);

    let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    hbox.set_hexpand(true);
    hbox.set_homogeneous(true);
    let position_label = gtk::Label::new(Some("Position: 00:00:00"));
    hbox.append(&position_label);
    let recorded_duration_label = gtk::Label::new(Some("Recorded: 00:00:00"));
    hbox.append(&recorded_duration_label);
    vbox.append(&hbox);

    let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    hbox.set_hexpand(true);
    hbox.set_homogeneous(true);
    let record_button = gtk::Button::with_label("Record");
    hbox.append(&record_button);
    let finish_button = gtk::Button::with_label("Finish");
    hbox.append(&finish_button);
    vbox.append(&hbox);

    window.set_child(Some(&vbox));
    window.present();

    app.add_window(&window);

    let video_sink_weak = video_sink.downgrade();
    let togglerecord_weak = togglerecord.downgrade();
    let timeout_id = glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        let Some(video_sink) = video_sink_weak.upgrade() else {
            return glib::ControlFlow::Break;
        };

        let Some(togglerecord) = togglerecord_weak.upgrade() else {
            return glib::ControlFlow::Break;
        };

        let position = video_sink
            .query_position::<gst::ClockTime>()
            .unwrap_or(gst::ClockTime::ZERO);
        position_label.set_text(&format!("Position: {position:.1}"));

        let recording_duration = togglerecord
            .static_pad("src")
            .unwrap()
            .query_position::<gst::ClockTime>()
            .unwrap_or(gst::ClockTime::ZERO);
        recorded_duration_label.set_text(&format!("Recorded: {recording_duration:.1}"));

        glib::ControlFlow::Continue
    });

    let togglerecord_weak = togglerecord.downgrade();
    record_button.connect_clicked(move |button| {
        let Some(togglerecord) = togglerecord_weak.upgrade() else {
            return;
        };

        let recording = !togglerecord.property::<bool>("record");
        togglerecord.set_property("record", recording);

        button.set_label(if recording { "Stop" } else { "Record" });
    });

    let record_button_weak = record_button.downgrade();
    finish_button.connect_clicked(move |button| {
        let Some(record_button) = record_button_weak.upgrade() else {
            return;
        };

        record_button.set_sensitive(false);
        button.set_sensitive(false);

        video_pad.send_event(gst::event::Eos::new());
        audio_pad.send_event(gst::event::Eos::new());
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

    gsttogglerecord::plugin_register_static().expect("Failed to register togglerecord plugin");
    gstgtk4::plugin_register_static().expect("Failed to register gtk4paintablesink plugin");

    let app = gtk::Application::new(None::<&str>, gio::ApplicationFlags::FLAGS_NONE);

    app.connect_activate(create_ui);
    app.run()
}
