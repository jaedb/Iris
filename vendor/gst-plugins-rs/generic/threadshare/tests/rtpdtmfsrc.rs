// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

// use futures::channel::oneshot;
use futures::prelude::*;
use gst::prelude::*;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use gstthreadshare::runtime;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstthreadshare::plugin_register_static().expect("gstthreadshare inter test");
    });
}

#[test]
fn nominal() {
    init();

    let pipe = gst::Pipeline::with_name("rtpdtmfsrc::nominal");
    // let src = gst::ElementFactory::make("rtpdtmfsrc")
    let src = gst::ElementFactory::make("ts-rtpdtmfsrc")
        .property("context", "rtpdtmfsrc::nominal")
        .property("context-wait", 20u32)
        .name("src::rtpdtmfsrc::nominal")
        .build()
        .unwrap();
    let mux = gst::ElementFactory::make("rtpdtmfmux").build().unwrap();
    let appsink = gst_app::AppSink::builder()
        .name("appsink::rtpdtmfsrc::nominal")
        .caps(
            &gst::Caps::builder("application/x-rtp")
                .field("media", "audio")
                .field("clock-rate", gst::List::new([8_000, 16_000]))
                .field("encoding-name", "TELEPHONE-EVENT")
                .build(),
        )
        .build();

    let elems = [&src, &mux, appsink.upcast_ref::<gst::Element>()];
    pipe.add_many(elems).unwrap();
    gst::Element::link_many(elems).unwrap();

    // Steps
    const INIT: u32 = 0;
    const SEND_EARLY_END: u32 = 1;
    const SEND_START_4: u32 = 2;
    const START_4_SENT: u32 = 3;
    const START_4_MSG_RECEIVED: u32 = 4;
    const START_4_BUFFER_RECEIVED: u32 = 5;
    const START_4_MSG_AND_BUFFER_RECEIVED: u32 = 6;
    // time interval before end
    const SEND_4_END: u32 = 9;
    const END_4_SENT: u32 = 10;
    const END_4_MSG_RECEIVED: u32 = 11;
    const END_4_BUFFER_RECEIVED: u32 = 12;
    const END_4_MSG_AND_BUFFER_RECEIVED: u32 = 13;

    const SEND_START_2: u32 = 20;
    const START_2_SENT: u32 = 21;
    const START_2_MSG_RECEIVED: u32 = 22;
    const START_2_BUFFER_RECEIVED: u32 = 23;
    const START_2_MSG_AND_BUFFER_RECEIVED: u32 = 24;
    // time interval before end
    const SEND_2_END: u32 = 27;
    const END_2_SENT: u32 = 28;
    const END_2_MSG_RECEIVED: u32 = 29;
    const END_2_BUFFER_RECEIVED: u32 = 30;
    const END_2_MSG_AND_BUFFER_RECEIVED: u32 = 31;

    const VALID_EVENT_4_DIGIT: u8 = 4;
    const VALID_EVENT_4_VOLUME: u8 = 12;
    const VALID_EVENT_2_DIGIT: u8 = 2;
    const VALID_EVENT_2_VOLUME: u8 = 18;

    const PAYLOAD_DIGIT_BYTE_INDEX: usize = 12;
    const PAYLOAD_END_VOLUME_BYTE_INDEX: usize = 13;
    const PAYLOAD_END_MASK: u8 = 0x80;
    const PAYLOAD_DURATION_BIG_BYTE_INDEX: usize = 14;
    const PAYLOAD_DURATION_SMALL_BYTE_INDEX: usize = 15;
    const PAYLOAD_DURATION_INCREMENT: u16 = 40 * 8; // 40ms @ 8kHZ

    const BUFFER_DURATION: gst::ClockTime = gst::ClockTime::from_mseconds(40);
    const BUFFER_DURATION_END: gst::ClockTime = gst::ClockTime::from_mseconds(40 * 4);

    let step = Arc::new(Mutex::new(INIT));
    appsink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample({
                let step = step.clone();
                let mut buffer_idx = 0;
                move |appsink| {
                    let sample = appsink.pull_sample().unwrap();
                    let buffer = sample.buffer().unwrap();
                    let buf = buffer.map_readable().unwrap();
                    let duration = (buf[PAYLOAD_DURATION_BIG_BYTE_INDEX] as u16) << 8
                        | buf[PAYLOAD_DURATION_SMALL_BYTE_INDEX] as u16;
                    let mut step = step.lock().unwrap();

                    let details = |buffer_idx| {
                        format!(
                            "idx {buffer_idx} ts {} @ step {step:02} DTMF payload: {:02x} {:02x} {:02x} {:02x}",
                            buffer.pts().display(),
                            buf[PAYLOAD_DIGIT_BYTE_INDEX],
                            buf[PAYLOAD_END_VOLUME_BYTE_INDEX],
                            buf[PAYLOAD_DURATION_BIG_BYTE_INDEX],
                            buf[PAYLOAD_DURATION_SMALL_BYTE_INDEX],
                        )
                    };

                    match *step {
                        START_4_SENT | START_4_MSG_RECEIVED => {
                            assert_eq!(buf[PAYLOAD_DIGIT_BYTE_INDEX], VALID_EVENT_4_DIGIT);
                            assert_eq!(buf[PAYLOAD_END_VOLUME_BYTE_INDEX], VALID_EVENT_4_VOLUME);
                            assert_eq!(buffer.duration().unwrap(), BUFFER_DURATION);
                            assert_eq!(duration, (buffer_idx + 1) * PAYLOAD_DURATION_INCREMENT);
                            println!(
                                "rtpdtmfsrc::nominal start buffer received {}",
                                details(buffer_idx)
                            );

                            buffer_idx += 1;
                            *step = if *step == START_4_SENT {
                                START_4_BUFFER_RECEIVED
                            } else {
                                START_4_MSG_AND_BUFFER_RECEIVED
                            };
                        }
                        START_4_BUFFER_RECEIVED..END_4_SENT => {
                            assert_eq!(buf[PAYLOAD_DIGIT_BYTE_INDEX], VALID_EVENT_4_DIGIT);
                            assert_eq!(buf[PAYLOAD_END_VOLUME_BYTE_INDEX], VALID_EVENT_4_VOLUME);
                            assert_eq!(buffer.duration().unwrap(), BUFFER_DURATION);
                            assert_eq!(duration, (buffer_idx + 1) * PAYLOAD_DURATION_INCREMENT);
                            println!(
                                "rtpdtmfsrc::nominal intermediate buffer received {}",
                                details(buffer_idx)
                            );

                            buffer_idx += 1;
                        }
                        END_4_SENT | END_4_MSG_RECEIVED => {
                            if buf[PAYLOAD_END_VOLUME_BYTE_INDEX] & PAYLOAD_END_MASK != 0 {
                                println!(
                                    "rtpdtmfsrc::nominal end buffer received {}",
                                    details(buffer_idx)
                                );
                                assert_eq!(buffer.duration().unwrap(), BUFFER_DURATION_END);
                                assert_eq!(duration, (buffer_idx + 1) * PAYLOAD_DURATION_INCREMENT);

                                buffer_idx += 1;
                                *step = if *step == END_4_SENT {
                                    END_4_BUFFER_RECEIVED
                                } else {
                                    END_4_MSG_AND_BUFFER_RECEIVED
                                };
                            } else {
                                println!(
                                    "rtpdtmfsrc::nominal late intermediate buffer received {}",
                                    details(buffer_idx)
                                );
                                assert_eq!(buffer.duration().unwrap(), BUFFER_DURATION);
                                assert_eq!(duration, (buffer_idx + 1) * PAYLOAD_DURATION_INCREMENT);

                                buffer_idx += 1;
                            }
                        }
                        START_2_SENT | START_2_MSG_RECEIVED => {
                            // New digit
                            buffer_idx = 0;

                            assert_eq!(buf[PAYLOAD_DIGIT_BYTE_INDEX], VALID_EVENT_2_DIGIT);
                            assert_eq!(buf[PAYLOAD_END_VOLUME_BYTE_INDEX], VALID_EVENT_2_VOLUME);
                            assert_eq!(buffer.duration().unwrap(), BUFFER_DURATION);
                            assert_eq!(duration, (buffer_idx + 1) * PAYLOAD_DURATION_INCREMENT);
                            println!(
                                "rtpdtmfsrc::nominal start buffer received {}",
                                details(buffer_idx)
                            );

                            buffer_idx += 1;
                            *step = if *step == START_2_SENT {
                                START_2_BUFFER_RECEIVED
                            } else {
                                START_2_MSG_AND_BUFFER_RECEIVED
                            };
                        }
                        START_2_SENT..END_2_SENT => {
                            assert_eq!(buf[PAYLOAD_DIGIT_BYTE_INDEX], VALID_EVENT_2_DIGIT);
                            assert_eq!(buf[PAYLOAD_END_VOLUME_BYTE_INDEX], VALID_EVENT_2_VOLUME);
                            assert_eq!(buffer.duration().unwrap(), BUFFER_DURATION);
                            assert_eq!(duration, (buffer_idx + 1) * PAYLOAD_DURATION_INCREMENT);
                            println!(
                                "rtpdtmfsrc::nominal intermediate buffer received {}",
                                details(buffer_idx)
                            );

                            buffer_idx += 1;
                        }
                        END_2_SENT | END_2_MSG_RECEIVED => {
                            if buf[PAYLOAD_END_VOLUME_BYTE_INDEX] & PAYLOAD_END_MASK != 0 {
                                println!(
                                    "rtpdtmfsrc::nominal end buffer received {}",
                                    details(buffer_idx)
                                );
                                assert_eq!(buffer.duration().unwrap(), BUFFER_DURATION_END);
                                assert_eq!(duration, (buffer_idx + 1) * PAYLOAD_DURATION_INCREMENT);

                                buffer_idx += 1;
                                *step = if *step == END_2_SENT {
                                    END_2_BUFFER_RECEIVED
                                } else {
                                    END_2_MSG_AND_BUFFER_RECEIVED
                                };
                            } else {
                                println!(
                                    "rtpdtmfsrc::nominal late intermediate buffer received {}",
                                    details(buffer_idx)
                                );
                                assert_eq!(buffer.duration().unwrap(), BUFFER_DURATION);
                                assert_eq!(duration, (buffer_idx + 1) * PAYLOAD_DURATION_INCREMENT);

                                buffer_idx += 1;
                            }
                        }
                        _ => panic!(
                            "rtpdtmfsrc::nominal unexpected received buffer {}",
                            details(buffer_idx)
                        ),
                    }

                    Ok(gst::FlowSuccess::Ok)
                }
            })
            .build(),
    );

    pipe.set_state(gst::State::Playing).unwrap();

    runtime::executor::block_on({
        fn new_dtmf_start_event(number: u8, volume: u8) -> gst::Event {
            gst::event::CustomUpstream::builder(
                gst::Structure::builder("dtmf-event")
                    .field("type", 1i32)
                    .field("method", 1i32)
                    .field("start", true)
                    .field("number", number as i32)
                    .field("volume", volume as i32)
                    .build(),
            )
            .build()
        }

        fn new_dtmf_end_event() -> gst::Event {
            gst::event::CustomUpstream::builder(
                gst::Structure::builder("dtmf-event")
                    .field("type", 1i32)
                    .field("method", 1i32)
                    .field("start", false)
                    .build(),
            )
            .build()
        }

        let pipe = pipe.clone();
        let src = src.clone();
        let appsink = appsink.upcast::<gst::Element>();
        async move {
            use gst::MessageView::*;

            let mut timer = runtime::timer::interval(Duration::from_millis(30)).unwrap();
            let mut bus_stream = pipe.bus().unwrap().stream();

            loop {
                futures::select! {
                    _ = timer.next() => {
                        let mut step = step.lock().unwrap();
                        match *step {
                            INIT => {
                                *step = SEND_EARLY_END;
                            }
                            SEND_EARLY_END => {
                                println!("rtpdtmfsrc::nominal sending early end");
                                if appsink.send_event(new_dtmf_end_event()) {
                                    panic!("Shouldn't be able to send initial end dtmf-event");
                                }
                                *step = SEND_START_4;
                            }
                            SEND_START_4 => {
                                println!("rtpdtmfsrc::nominal sending start 4");
                                if !appsink.send_event(new_dtmf_start_event(VALID_EVENT_4_DIGIT, VALID_EVENT_4_VOLUME)) {
                                    panic!("Failed to send start dtmf-event {step}");
                                }
                                *step = START_4_SENT;
                            }
                            START_4_MSG_AND_BUFFER_RECEIVED..SEND_4_END => {
                                // give a little time interval
                                *step += 1;
                            }
                            SEND_4_END => {
                                println!("rtpdtmfsrc::nominal sending end 4");
                                if !appsink.send_event(new_dtmf_end_event()) {
                                    panic!("Failed to send start dtmf-event {step}");
                                }
                                *step = END_4_SENT;
                            }
                            END_4_MSG_AND_BUFFER_RECEIVED..SEND_START_2 => {
                                // give a little time interval
                                *step += 1;
                            }
                            SEND_START_2 => {
                                println!("rtpdtmfsrc::nominal sending start 2");
                                if !appsink.send_event(new_dtmf_start_event(VALID_EVENT_2_DIGIT, VALID_EVENT_2_VOLUME)) {
                                    panic!("Failed to send start dtmf-event {step}");
                                }
                                *step = START_2_SENT;
                            }
                            START_2_MSG_AND_BUFFER_RECEIVED..SEND_2_END => {
                                // give a little time interval
                                *step += 1;
                            }
                            SEND_2_END => {
                                println!("rtpdtmfsrc::nominal sending end 2");
                                if !appsink.send_event(new_dtmf_end_event()) {
                                    panic!("Failed to send start dtmf-event {step}");
                                }
                                *step = END_2_SENT;
                            }
                            END_2_MSG_AND_BUFFER_RECEIVED => {
                                break;
                            }
                            _ => (),
                        }
                    }
                    // _ = eos_rx => {
                    //     println!("rtpdtmfsrc::nominal");
                    // }
                    msg = bus_stream.next() => {
                        let Some(msg) = msg else { continue };
                        match msg.view() {
                            Element(_) => {
                                if msg.src().is_some_and(|obj| *obj == src) {
                                    let mut step = step.lock().unwrap();
                                    match *step {
                                        START_4_SENT | START_4_BUFFER_RECEIVED => {
                                            let s = msg.structure().unwrap();
                                            assert_eq!(s.name(), "dtmf-event-processed");
                                            assert!(s.get::<bool>("start").unwrap());
                                            assert_eq!(s.get::<i32>("number").unwrap(), VALID_EVENT_4_DIGIT as i32);
                                            assert_eq!(s.get::<i32>("volume").unwrap(), VALID_EVENT_4_VOLUME as i32);
                                            println!("rtpdtmfsrc::nominal start 4 msg received");
                                            *step = if *step == START_4_SENT {
                                                START_4_MSG_RECEIVED
                                            } else {
                                                START_4_MSG_AND_BUFFER_RECEIVED
                                            };
                                        }
                                        END_4_SENT | END_4_BUFFER_RECEIVED => {
                                            let s = msg.structure().unwrap();
                                            assert_eq!(s.name(), "dtmf-event-processed");
                                            assert!(!s.get::<bool>("start").unwrap());
                                            println!("rtpdtmfsrc::nominal end 4 msg received");
                                            *step = if *step == END_4_SENT {
                                                END_4_MSG_RECEIVED
                                            } else {
                                                END_4_MSG_AND_BUFFER_RECEIVED
                                            };
                                        }
                                        START_2_SENT | START_2_BUFFER_RECEIVED => {
                                            let s = msg.structure().unwrap();
                                            assert_eq!(s.name(), "dtmf-event-processed");
                                            assert!(s.get::<bool>("start").unwrap());
                                            assert_eq!(s.get::<i32>("number").unwrap(), VALID_EVENT_2_DIGIT as i32);
                                            assert_eq!(s.get::<i32>("volume").unwrap(), VALID_EVENT_2_VOLUME as i32);
                                            println!("rtpdtmfsrc::nominal start 2 msg received");
                                            *step = if *step == START_2_SENT {
                                                START_2_MSG_RECEIVED
                                            } else {
                                                START_2_MSG_AND_BUFFER_RECEIVED
                                            };
                                        }
                                        END_2_SENT | END_2_BUFFER_RECEIVED => {
                                            let s = msg.structure().unwrap();
                                            assert_eq!(s.name(), "dtmf-event-processed");
                                            assert!(!s.get::<bool>("start").unwrap());
                                            println!("rtpdtmfsrc::nominal end 2 msg received");
                                            *step = if *step == END_2_SENT {
                                                END_2_MSG_RECEIVED
                                            } else {
                                                END_2_MSG_AND_BUFFER_RECEIVED
                                            };
                                        }
                                        _ => panic!("Unexpected ts-rtpdtmfsrc msg {msg:?}"),
                                    }
                                }
                            }
                            Latency(_) => {
                                let _ = pipe.recalculate_latency();
                            }
                            Error(err) => unreachable!("rtpdtmfsrc::nominal {err}"),
                            _ => (),
                        }
                    }
                };
            }
        }
    });

    pipe.set_state(gst::State::Null).unwrap();
}
