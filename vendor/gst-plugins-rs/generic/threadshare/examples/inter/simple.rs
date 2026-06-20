// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use futures::prelude::*;
use gst::prelude::*;

fn main() {
    let g_ctx = gst::glib::MainContext::default();
    gst::init().unwrap();

    let pipe_up = gst::parse::launch(
        "pipeline. (name=upstream
         ts-audiotestsrc num-buffers=2000 volume=0.02 context=ts-group-1 context-wait=20
         ! opusenc
         ! ts-intersink inter-context=my-inter-ctx
         )",
    )
    .unwrap()
    .downcast::<gst::Pipeline>()
    .unwrap();

    // A downstream pipeline which will receive the Opus encoded audio stream
    // and render it locally.
    let pipe_down = gst::parse::launch(
        "pipeline. (name=downstream
         ts-intersrc inter-context=my-inter-ctx context=ts-group-1 context-wait=20
         ! opusdec
         ! audioconvert
         ! audioresample
         ! ts-queue context=ts-group-1 context-wait=20 max-size-buffers=1 max-size-bytes=0 max-size-time=0
         ! ts-blocking-adapter
         ! autoaudiosink
         )",
    )
    .unwrap()
    .downcast::<gst::Pipeline>()
    .unwrap();

    // Both pipelines must agree on the timing information or we'll get glitches
    // or overruns/underruns. Ideally, we should tell pipe_up to use the same clock
    // as pipe_down, but since that will be set asynchronously to the audio clock, it
    // is simpler and likely accurate enough to use the system clock for both
    // pipelines. If no element in either pipeline will provide a clock, this
    // is not needed.
    let clock = gst::SystemClock::obtain();
    pipe_up.set_clock(Some(&clock)).unwrap();
    pipe_down.set_clock(Some(&clock)).unwrap();

    // This is not really needed in this case since the pipelines are created and
    // started at the same time. However, an application that dynamically
    // generates pipelines must ensure that all the pipelines that will be
    // connected together share the same base time.
    let base_time = clock.time();
    pipe_up.set_start_time(gst::ClockTime::NONE);
    pipe_up.set_base_time(base_time);
    pipe_down.set_start_time(gst::ClockTime::NONE);
    pipe_down.set_base_time(base_time);

    pipe_up.set_state(gst::State::Playing).unwrap();
    pipe_down.set_state(gst::State::Playing).unwrap();

    g_ctx.block_on(async {
        use gst::MessageView::*;

        let mut bus_up_stream = pipe_up.bus().unwrap().stream();
        let mut bus_down_stream = pipe_down.bus().unwrap().stream();

        loop {
            futures::select! {
                msg_up = bus_up_stream.next() => {
                    let Some(msg) = msg_up else { continue };
                    match msg.view() {
                        Latency(_) => {
                            let _ = pipe_up.recalculate_latency();
                        }
                        Error(err) => {
                            eprintln!("Error with downstream pipeline {err:?}");
                            break;
                        }
                        _ => (),
                    }
                }
                msg_down = bus_down_stream.next() => {
                    let Some(msg) = msg_down else { continue };
                    match msg.view() {
                        Latency(_) => {
                            let _ = pipe_down.recalculate_latency();
                        }
                        Eos(_) => {
                            println!("Got EoS");
                            break;
                        }
                        Error(err) => {
                            eprintln!("Error with downstream pipeline {err:?}");
                            break;
                        }
                        _ => (),
                    }
                }
            };
        }
    });

    pipe_up.set_state(gst::State::Null).unwrap();
    pipe_down.set_state(gst::State::Null).unwrap();

    // This is needed by some tracers to write their log file
    unsafe {
        gst::deinit();
    }
}
