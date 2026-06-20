// Copyright (C) 2022 LTN Global Communications, Inc.
// Contact: Jan Alexander Steffens (heftig) <jan.steffens@ltnglobal.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gio::prelude::*;
use gst::{glib, prelude::*};
use gtk::prelude::*;
use std::cell::{Cell, RefCell};

struct DroppingProbe(glib::WeakRef<gst::Pad>, Option<gst::PadProbeId>);

impl DroppingProbe {
    fn install(pad: &gst::Pad) -> Self {
        let probe_id = pad
            .add_probe(gst::PadProbeType::BUFFER, |_, _| gst::PadProbeReturn::Drop)
            .unwrap();
        Self(pad.downgrade(), Some(probe_id))
    }
}

impl Drop for DroppingProbe {
    fn drop(&mut self) {
        if let Some((pad, probe_id)) = self.0.upgrade().zip(self.1.take()) {
            pad.remove_probe(probe_id);
        }
    }
}

fn create_pipeline() -> gst::Pipeline {
    gst::parse::launch(
        r#"videotestsrc name=vsrc is-live=1
            ! video/x-raw,framerate=60/1,width=800,height=600
            ! identity single-segment=1
            ! timeoverlay text="Pre:"
            ! queue
            ! livesync latency=50000000
            ! videorate
            ! timeoverlay text="Post:" halignment=right
            ! queue
            ! gtk4paintablesink name=vsink
          audiotestsrc name=asrc is-live=1
            ! audio/x-raw,channels=2
            ! identity single-segment=1
            ! queue
            ! livesync latency=50000000
            ! audiorate
            ! queue
            ! autoaudiosink
        "#,
    )
    .expect("Failed to create pipeline")
    .downcast()
    .unwrap()
}

fn create_window(app: &gtk::Application) {
    let pipeline = create_pipeline();
    let video_src_pad = pipeline.by_name("vsrc").unwrap().static_pad("src").unwrap();
    let audio_src_pad = pipeline.by_name("asrc").unwrap().static_pad("src").unwrap();

    let window = gtk::ApplicationWindow::new(app);
    window.set_default_size(800, 684);

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let picture = gtk::Picture::new();
    let paintable = pipeline
        .by_name("vsink")
        .unwrap()
        .property::<gtk::gdk::Paintable>("paintable");
    picture.set_paintable(Some(&paintable));
    vbox.append(&picture);

    let action_bar = gtk::ActionBar::new();
    vbox.append(&action_bar);

    let offset_spin = gtk::SpinButton::with_range(0.0, 500.0, 100.0);
    action_bar.pack_start(&offset_spin);

    {
        let video_src_pad = video_src_pad.clone();
        let audio_src_pad = audio_src_pad.clone();
        offset_spin.connect_value_notify(move |offset_spin| {
            const MSECOND: f64 = gst::ClockTime::MSECOND.nseconds() as _;

            let offset = (offset_spin.value() * -MSECOND) as i64;
            video_src_pad.set_offset(offset);
            audio_src_pad.set_offset(offset);
        });
    }

    let drop_button = gtk::ToggleButton::with_label("Drop Signal");
    action_bar.pack_end(&drop_button);

    let drop_ids = Cell::new(None);
    drop_button.connect_toggled(move |drop_button| {
        if drop_button.is_active() {
            let video_probe = DroppingProbe::install(&video_src_pad);
            let audio_probe = DroppingProbe::install(&audio_src_pad);
            drop_ids.set(Some((video_probe, audio_probe)));
        } else {
            drop_ids.set(None);
        }
    });

    let bus_watch = {
        let bus = pipeline.bus().unwrap();
        let window = window.downgrade();
        bus.add_watch_local(move |_, msg| {
            use gst::MessageView;

            match msg.view() {
                MessageView::Eos(..) => {
                    if let Some(window) = window.upgrade() {
                        window.close();
                    }
                }

                MessageView::Error(err) => {
                    eprintln!(
                        "Error from {}: {} ({:?})",
                        msg.src().map(|s| s.path_string()).as_deref().unwrap_or(""),
                        err.error(),
                        err.debug(),
                    );
                    if let Some(window) = window.upgrade() {
                        window.close();
                    }
                }

                _ => (),
            };

            glib::ControlFlow::Continue
        })
        .unwrap()
    };

    {
        let pipeline = pipeline.clone();
        window.connect_realize(move |_| {
            pipeline
                .set_state(gst::State::Playing)
                .expect("Failed to start pipeline");
        });
    }

    let bus_watch = RefCell::new(Some(bus_watch));
    window.connect_unrealize(move |_| {
        drop(bus_watch.borrow_mut().take());
        pipeline
            .set_state(gst::State::Null)
            .expect("Failed to stop pipeline");
    });

    window.set_child(Some(&vbox));
    window.present();
}

fn main() -> glib::ExitCode {
    let app = gtk::Application::new(
        Some("gtk-plugins-rs.gtk-livesync"),
        gio::ApplicationFlags::FLAGS_NONE,
    );

    app.connect_startup(move |_app| {
        gst::init().expect("Failed to initialize GStreamer");
        gstlivesync::plugin_register_static().expect("Failed to register livesync plugin");
        gstgtk4::plugin_register_static().expect("Failed to register gstgtk4 plugin");
    });

    app.connect_activate(create_window);
    app.run()
}
