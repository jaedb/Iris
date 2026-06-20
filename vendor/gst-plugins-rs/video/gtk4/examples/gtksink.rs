use gst::prelude::*;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use std::cell::RefCell;

fn create_ui(app: &gtk::Application) {
    let pipeline = gst::Pipeline::new();

    let overlay = gst::ElementFactory::make("clockoverlay")
        .property("font-desc", "Monospace 42")
        .build()
        .unwrap();

    let gtksink = gst::ElementFactory::make("gtk4paintablesink")
        .build()
        .unwrap();

    let paintable = gtksink.property::<gdk::Paintable>("paintable");

    // TODO: future plans to provide a bin-like element that works with less setup
    let (src, sink) = if paintable
        .property::<Option<gdk::GLContext>>("gl-context")
        .is_some()
    {
        let src = gst::ElementFactory::make("gltestsrc").build().unwrap();

        let sink = gst::ElementFactory::make("glsinkbin")
            .property("sink", &gtksink)
            .build()
            .unwrap();
        (src, sink)
    } else {
        let src = gst::ElementFactory::make("videotestsrc").build().unwrap();

        let sink = gst::Bin::default();
        let convert = gst::ElementFactory::make("videoconvert").build().unwrap();

        sink.add(&convert).unwrap();
        sink.add(&gtksink).unwrap();
        convert.link(&gtksink).unwrap();

        sink.add_pad(&gst::GhostPad::with_target(&convert.static_pad("sink").unwrap()).unwrap())
            .unwrap();

        (src, sink.upcast())
    };

    pipeline.add_many([&src, &overlay, &sink]).unwrap();
    let caps = gst_video::VideoCapsBuilder::new()
        .width(640)
        .height(480)
        .any_features()
        .build();

    src.link_filtered(&overlay, &caps).unwrap();
    overlay.link(&sink).unwrap();

    let window = gtk::ApplicationWindow::new(app);
    window.set_default_size(640, 480);

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let gst_widget = gstgtk4::RenderWidget::new(&gtksink);
    vbox.append(&gst_widget);

    let label = gtk::Label::new(Some("Position: 00:00:00"));
    vbox.append(&label);

    window.set_child(Some(&vbox));
    window.present();

    app.add_window(&window);

    let pipeline_weak = pipeline.downgrade();
    let timeout_id = glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
        let Some(pipeline) = pipeline_weak.upgrade() else {
            return glib::ControlFlow::Break;
        };

        let position = pipeline.query_position::<gst::ClockTime>();
        label.set_text(&format!("Position: {:.0}", position.display()));
        glib::ControlFlow::Continue
    });

    let bus = pipeline.bus().unwrap();

    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");

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
                        err.src().map(|s| s.path_string()),
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

    let timeout_id = RefCell::new(Some(timeout_id));
    let pipeline = RefCell::new(Some(pipeline));
    let bus_watch = RefCell::new(Some(bus_watch));
    app.connect_shutdown(move |_| {
        window.close();

        drop(bus_watch.borrow_mut().take());
        if let Some(pipeline) = pipeline.borrow_mut().take() {
            pipeline
                .set_state(gst::State::Null)
                .expect("Unable to set the pipeline to the `Null` state");
        }

        if let Some(timeout_id) = timeout_id.borrow_mut().take() {
            timeout_id.remove();
        }
    });
}

fn main() -> glib::ExitCode {
    gst::init().unwrap();
    gtk::init().unwrap();

    gstgtk4::plugin_register_static().expect("Failed to register gstgtk4 plugin");

    let app = gtk::Application::new(None::<&str>, gio::ApplicationFlags::FLAGS_NONE);

    app.connect_activate(create_ui);
    let res = app.run();

    unsafe {
        gst::deinit();
    }

    res
}
