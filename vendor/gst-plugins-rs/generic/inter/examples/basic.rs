use anyhow::Error;
use futures::prelude::*;
use futures::stream::select_all;
use gst::prelude::*;

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

    let src_pipeline = gst::parse::launch("videotestsrc is-live=true ! intersink")?;
    let sink_pipeline = gst::parse::launch("intersrc ! videoconvert ! autovideosink")?;

    let mut stream = select_all([
        src_pipeline.bus().unwrap().stream(),
        sink_pipeline.bus().unwrap().stream(),
    ]);

    let base_time = gst::SystemClock::obtain().time();

    src_pipeline.set_clock(Some(&gst::SystemClock::obtain()))?;
    src_pipeline.set_start_time(gst::ClockTime::NONE);
    src_pipeline.set_base_time(base_time);

    sink_pipeline.set_clock(Some(&gst::SystemClock::obtain()))?;
    sink_pipeline.set_start_time(gst::ClockTime::NONE);
    sink_pipeline.set_base_time(base_time);

    src_pipeline.set_state(gst::State::Playing)?;
    sink_pipeline.set_state(gst::State::Playing)?;

    while let Some(msg) = stream.next().await {
        use gst::MessageView;

        match msg.view() {
            MessageView::Latency(..) => {
                if let Some(o) = msg.src() {
                    if let Ok(pipeline) = toplevel(o).downcast::<gst::Pipeline>() {
                        pipeline
                            .call_async_future(|pipeline: &gst::Pipeline| {
                                eprintln!("Recalculating latency {pipeline:?}");
                                let _ = pipeline.recalculate_latency();
                            })
                            .await;
                    }
                }
            }
            MessageView::Eos(..) => {
                eprintln!("Unexpected EOS");
                break;
            }
            MessageView::Error(err) => {
                eprintln!(
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
        }
    }

    src_pipeline.set_state(gst::State::Null)?;
    sink_pipeline.set_state(gst::State::Null)?;

    Ok(())
}
