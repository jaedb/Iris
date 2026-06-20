use anyhow::Error;
use gst::prelude::*;
use std::sync::{Arc, Mutex};

fn toplevel(obj: &gst::Object) -> gst::Object {
    if let Some(parent) = obj.parent() {
        toplevel(&parent)
    } else {
        obj.clone()
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    gst::init()?;

    let src_pipeline = gst::parse::launch("gltestsrc is-live=true ! intersink")?;
    let sink_pipeline = gst::parse::launch("intersrc ! glimagesink")?;

    let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);

    let shared_gl_context: Arc<Mutex<Option<gst::Context>>> = Arc::new(Mutex::new(None));
    let msg_func = glib::clone!(
        #[strong]
        tx,
        move |_bus: &gst::Bus, msg: &gst::Message| {
            // NOTE: this code is verbose with logging only for clarity purposes
            match msg.view() {
                gst::MessageView::NeedContext(msg) => {
                    if msg.context_type() == gst_gl::GL_DISPLAY_CONTEXT_TYPE {
                        // Other contexts, ignore
                        return gst::BusSyncReply::Pass;
                    }
                    let Some(o) = msg.src() else {
                        eprintln!("Ignoring need-context message without a source object");
                        return gst::BusSyncReply::Pass;
                    };
                    let Some(e) = o.downcast_ref::<gst::Element>() else {
                        eprintln!("Ignoring need-context message without an element");
                        return gst::BusSyncReply::Pass;
                    };
                    let pipeline_name = match toplevel(o).downcast::<gst::Pipeline>() {
                        Ok(p) => p.name().to_string(),
                        Err(_) => "unknown".to_string(),
                    };

                    let shared_ctx = shared_gl_context.lock().unwrap();
                    if let Some(existing_ctx) = &*shared_ctx {
                        eprintln!(
                            "Setting pre-existing GstGLContext on {} in pipeline {}",
                            e.name(),
                            pipeline_name
                        );
                        e.set_context(existing_ctx);
                    } else {
                        eprintln!(
                            "Need GstGLContext for {} in pipeline {} but don't have any yet",
                            e.name(),
                            pipeline_name
                        );
                    }
                }
                gst::MessageView::HaveContext(msg) => {
                    let ctx = msg.context();
                    if !ctx.has_context_type(gst_gl::GL_DISPLAY_CONTEXT_TYPE) {
                        // Other contexts, ignore
                        return gst::BusSyncReply::Pass;
                    }
                    let Some(o) = msg.src() else {
                        eprintln!("Ignoring need-context message without a source object");
                        return gst::BusSyncReply::Pass;
                    };
                    let Some(e) = o.downcast_ref::<gst::Element>() else {
                        eprintln!("Ignoring need-context message without an element");
                        return gst::BusSyncReply::Pass;
                    };
                    let pipeline_name = match toplevel(o).downcast::<gst::Pipeline>() {
                        Ok(p) => p.name().to_string(),
                        Err(_) => "unknown".to_string(),
                    };

                    let mut shared_ctx = shared_gl_context.lock().unwrap();
                    if shared_ctx.is_some() {
                        eprintln!("Already have a GstGLContext, not overwriting it with a new context from {} in pipeline {}", e.name(), pipeline_name);
                        return gst::BusSyncReply::Pass;
                    }
                    eprintln!(
                        "Storing GstGLContext from {} in pipeline {}",
                        e.name(),
                        pipeline_name
                    );
                    let _ = shared_ctx.insert(ctx);
                }
                gst::MessageView::Latency(..) => {
                    if let Some(o) = msg.src() {
                        if let Ok(pipeline) = toplevel(o).downcast::<gst::Pipeline>() {
                            // This message handler is a sync handler
                            pipeline.call_async(|pipeline: &gst::Pipeline| {
                                eprintln!("Recalculating latency {pipeline:?}");
                                let _ = pipeline.recalculate_latency();
                            });
                        }
                    }
                }
                gst::MessageView::Eos(..) => {
                    eprintln!("Unexpected EOS");
                    tx.try_send(()).unwrap();
                }
                gst::MessageView::Error(err) => {
                    eprintln!(
                        "Got error from {}: {} ({})",
                        msg.src()
                            .map(|s| String::from(s.path_string()))
                            .unwrap_or_else(|| "None".into()),
                        err.error(),
                        err.debug().unwrap_or_else(|| "".into()),
                    );
                    tx.try_send(()).unwrap();
                }
                _ => (),
            }
            return gst::BusSyncReply::Pass;
        }
    );

    // Comment-out any of these two lines to see that GL texture sharing doesn't work:
    // you will get black frames at best.
    src_pipeline
        .bus()
        .unwrap()
        .set_sync_handler(msg_func.clone());
    sink_pipeline.bus().unwrap().set_sync_handler(msg_func);
    // ... any number of src or sink pipelines can share the GL context

    let base_time = gst::SystemClock::obtain().time();

    src_pipeline.set_clock(Some(&gst::SystemClock::obtain()))?;
    src_pipeline.set_start_time(gst::ClockTime::NONE);
    src_pipeline.set_base_time(base_time);

    sink_pipeline.set_clock(Some(&gst::SystemClock::obtain()))?;
    sink_pipeline.set_start_time(gst::ClockTime::NONE);
    sink_pipeline.set_base_time(base_time);

    // Set sink to playing first, because that's the one that will talk to the display and will
    // have the best context to use for the rest of the elements/pipelines. glimagesink and most
    // sinks do special things for GL context creation.
    sink_pipeline.set_state(gst::State::Playing)?;
    src_pipeline.set_state(gst::State::Playing)?;

    // Wait for EOS or ERROR message
    let _ = rx.recv().await;

    src_pipeline.set_state(gst::State::Null)?;
    sink_pipeline.set_state(gst::State::Null)?;

    Ok(())
}
