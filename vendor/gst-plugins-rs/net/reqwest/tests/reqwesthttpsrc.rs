// Copyright (C) 2019 Sebastian Dröge <sebastian@centricular.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::single_match)]

use gst::{glib, prelude::*};
use http_body_util::combinators::BoxBody;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

use std::sync::mpsc;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        // clear this environment because it affects the default settings
        std::env::remove_var("http_proxy");
        gst::init().unwrap();
        gstreqwest::plugin_register_static().expect("reqwesthttpsrc tests");
    });
}

/// Our custom test harness around the HTTP source
#[derive(Debug)]
struct Harness {
    src: gst::Element,
    pad: gst::Pad,
    receiver: Option<mpsc::Receiver<Message>>,
    rt: tokio::runtime::Runtime,
}

/// Messages sent from our test harness
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone)]
enum Message {
    Buffer(gst::Buffer),
    Event(gst::Event),
    Message(gst::Message),
    ServerError(String),
}

fn full_body(s: impl Into<bytes::Bytes>) -> BoxBody<bytes::Bytes, hyper::Error> {
    use http_body_util::{BodyExt, Full};
    Full::new(s.into()).map_err(|never| match never {}).boxed()
}

fn empty_body() -> BoxBody<bytes::Bytes, hyper::Error> {
    use http_body_util::{BodyExt, Empty};
    Empty::new().map_err(|never| match never {}).boxed()
}

impl Harness {
    /// Creates a new HTTP source and test harness around it
    ///
    /// `http_func`: Function to generate HTTP responses based on a request
    /// `setup_func`: Setup function for the HTTP source, should only set properties and similar
    fn new<
        F: FnMut(
                hyper::Request<hyper::body::Incoming>,
            ) -> hyper::Response<BoxBody<bytes::Bytes, hyper::Error>>
            + Send
            + 'static,
        G: FnOnce(&gst::Element),
    >(
        http_func: F,
        setup_func: G,
    ) -> Harness {
        use hyper::server::conn::http1;
        use hyper::service::service_fn;
        use std::sync::{Arc, Mutex};

        // Create the HTTP source
        let src = gst::ElementFactory::make("reqwesthttpsrc").build().unwrap();

        // Sender/receiver for the messages we generate from various places for the tests
        //
        // Sending to this sender will block until the corresponding item was received from the
        // receiver, which allows us to handle everything as if it is running in a single thread
        let (sender, receiver) = mpsc::sync_channel(0);

        // Sink pad that receives everything the source is generating
        let pad = gst::Pad::builder(gst::PadDirection::Sink)
            .name("sink")
            .chain_function({
                let sender_clone = sender.clone();
                move |_pad, _parent, buffer| {
                    let _ = sender_clone.send(Message::Buffer(buffer));
                    Ok(gst::FlowSuccess::Ok)
                }
            })
            .event_function({
                let sender_clone = sender.clone();
                move |_pad, _parent, event| {
                    let _ = sender_clone.send(Message::Event(event));
                    true
                }
            })
            .build();

        let srcpad = src.static_pad("src").unwrap();
        srcpad.link(&pad).unwrap();

        let bus = gst::Bus::new();
        bus.set_flushing(false);
        src.set_bus(Some(&bus));
        let sender_clone = sender.clone();
        bus.set_sync_handler(move |_bus, msg| {
            let _ = sender_clone.send(Message::Message(msg.clone()));
            gst::BusSyncReply::Drop
        });

        // Activate the pad so that it can be used now
        pad.set_active(true).unwrap();

        // Create the tokio runtime used for the HTTP server in this test
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        // Create an HTTP sever that listens on localhost on some random, free port
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 0));

        // Whenever a new client is connecting, a new service function is requested. For each
        // client we use the same service function, which simply calls the function used by the
        // test
        let http_func = Arc::new(Mutex::new(http_func));
        let service = service_fn(move |req: hyper::Request<hyper::body::Incoming>| {
            let http_func = http_func.clone();
            async move { Ok::<_, hyper::Error>((*http_func.lock().unwrap())(req)) }
        });

        let (local_addr_sender, local_addr_receiver) = tokio::sync::oneshot::channel();

        // Spawn the server in the background so that it can handle requests
        rt.spawn(async move {
            // Bind the server, retrieve the local port that was selected in the end and set this as
            // the location property on the source
            let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
            let local_addr = listener.local_addr().unwrap();

            local_addr_sender.send(local_addr).unwrap();

            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let io = tokio_io::TokioIo::new(stream);
                let service = service.clone();
                let sender = sender.clone();
                tokio::task::spawn(async move {
                    let http = http1::Builder::new().serve_connection(io, service);
                    if let Err(e) = http.await {
                        let _ = sender.send(Message::ServerError(format!("{e}")));
                    }
                });
            }
        });

        let local_addr = futures::executor::block_on(local_addr_receiver).unwrap();
        src.set_property("location", format!("http://{local_addr}/"));

        // Let the test setup anything needed on the HTTP source now
        setup_func(&src);

        Harness {
            src,
            pad,
            receiver: Some(receiver),
            rt,
        }
    }

    fn wait_for_error(&mut self) -> glib::Error {
        loop {
            match self.receiver.as_mut().unwrap().recv().unwrap() {
                Message::ServerError(err) => {
                    panic!("Got server error: {err}");
                }
                Message::Event(ev) => {
                    use gst::EventView;

                    match ev.view() {
                        EventView::Eos(_) => {
                            panic!("Got EOS but expected error");
                        }
                        _ => (),
                    }
                }
                Message::Message(msg) => {
                    use gst::MessageView;

                    match msg.view() {
                        MessageView::Error(err) => {
                            return err.error();
                        }
                        _ => (),
                    }
                }
                Message::Buffer(_buffer) => {
                    panic!("Got buffer but expected error");
                }
            }
        }
    }

    fn wait_for_state_change(&mut self) -> gst::State {
        loop {
            match self.receiver.as_mut().unwrap().recv().unwrap() {
                Message::ServerError(err) => {
                    panic!("Got server error: {err}");
                }
                Message::Event(ev) => {
                    use gst::EventView;

                    match ev.view() {
                        EventView::Eos(_) => {
                            panic!("Got EOS but expected state change");
                        }
                        _ => (),
                    }
                }
                Message::Message(msg) => {
                    use gst::MessageView;

                    match msg.view() {
                        MessageView::StateChanged(state) => {
                            return state.current();
                        }
                        MessageView::Error(err) => {
                            panic!(
                                "Got error: {} ({})",
                                err.error(),
                                err.debug()
                                    .unwrap_or_else(|| glib::GString::from("UNKNOWN"))
                            );
                        }
                        _ => (),
                    }
                }
                Message::Buffer(_buffer) => {
                    panic!("Got buffer but expected state change");
                }
            }
        }
    }

    fn wait_for_segment(
        &mut self,
        allow_buffer: bool,
        mut allow_server_error: bool,
    ) -> gst::FormattedSegment<gst::format::Bytes> {
        loop {
            match self.receiver.as_mut().unwrap().recv().unwrap() {
                Message::ServerError(err) => {
                    if allow_server_error {
                        allow_server_error = false;
                    } else {
                        panic!("Got server error: {err}");
                    }
                }
                Message::Event(ev) => {
                    use gst::EventView;

                    match ev.view() {
                        EventView::Segment(seg) => {
                            return seg
                                .segment()
                                .clone()
                                .downcast::<gst::format::Bytes>()
                                .unwrap();
                        }
                        _ => (),
                    }
                }
                Message::Message(msg) => {
                    use gst::MessageView;

                    match msg.view() {
                        MessageView::Error(err) => {
                            panic!(
                                "Got error: {} ({})",
                                err.error(),
                                err.debug()
                                    .unwrap_or_else(|| glib::GString::from("UNKNOWN"))
                            );
                        }
                        _ => (),
                    }
                }
                Message::Buffer(_buffer) => {
                    if !allow_buffer {
                        panic!("Got buffer but expected segment");
                    }
                }
            }
        }
    }

    /// Wait until a buffer is available or EOS was reached
    ///
    /// This function will panic on errors.
    fn wait_buffer_or_eos(&mut self) -> Option<gst::Buffer> {
        loop {
            match self.receiver.as_mut().unwrap().recv().unwrap() {
                Message::ServerError(err) => {
                    panic!("Got server error: {err}");
                }
                Message::Event(ev) => {
                    use gst::EventView;

                    match ev.view() {
                        EventView::Eos(_) => return None,
                        _ => (),
                    }
                }
                Message::Message(msg) => {
                    use gst::MessageView;

                    match msg.view() {
                        MessageView::Error(err) => {
                            panic!(
                                "Got error: {} ({})",
                                err.error(),
                                err.debug()
                                    .unwrap_or_else(|| glib::GString::from("UNKNOWN"))
                            );
                        }
                        _ => (),
                    }
                }
                Message::Buffer(buffer) => return Some(buffer),
            }
        }
    }

    /// Run some code asynchronously on another thread with the HTTP source
    fn run<F: FnOnce(&gst::Element) + Send + 'static>(&self, func: F) {
        self.src.call_async(move |src| func(src));
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        // Shut down everything that was set up for this test harness
        // and wait until the tokio runtime exited
        let bus = self.src.bus().unwrap();
        bus.set_flushing(true);

        // Drop the receiver first before setting the state so that
        // any threads that might still be blocked on the sender
        // are immediately unblocked
        self.receiver.take().unwrap();

        self.pad.set_active(false).unwrap();
        self.src.set_state(gst::State::Null).unwrap();
    }
}

#[test]
fn test_basic_request() {
    use std::io::{Cursor, Read};

    init();

    // Set up a harness that returns "Hello World" for any HTTP request and checks if the
    // default headers are all sent
    let mut h = Harness::new(
        |req| {
            let headers = req.headers();
            assert_eq!(headers.get("connection").unwrap(), "keep-alive");
            assert_eq!(headers.get("accept-encoding").unwrap(), "identity");
            assert_eq!(headers.get("icy-metadata").unwrap(), "1");

            hyper::Response::new(full_body("Hello World"))
        },
        |_src| {
            // No additional setup needed here
        },
    );

    // Set the HTTP source to Playing so that everything can start
    h.run(|src| {
        src.set_state(gst::State::Playing).unwrap();
    });

    // And now check if the data we receive is exactly what we expect it to be
    let expected_output = "Hello World";
    let mut cursor = Cursor::new(expected_output);

    while let Some(buffer) = h.wait_buffer_or_eos() {
        // On the first buffer also check if the duration reported by the HTTP source is what we
        // would expect it to be
        if cursor.position() == 0 {
            assert_eq!(
                h.src.query_duration::<gst::format::Bytes>(),
                Some(gst::format::Bytes::from_usize(expected_output.len()))
            );
        }

        // Map the buffer readable and check if it contains exactly the data we would expect at
        // this point after reading everything else we read in previous runs
        let map = buffer.map_readable().unwrap();
        let mut read_buf = vec![0; map.size()];
        assert_eq!(cursor.read(&mut read_buf).unwrap(), map.size());
        assert_eq!(&*map, &*read_buf);
    }

    // Check if everything was read
    assert_eq!(cursor.position(), 11);
}

#[test]
fn test_basic_request_inverted_defaults() {
    use std::io::{Cursor, Read};

    init();

    // Set up a harness that returns "Hello World" for any HTTP request and override various
    // default properties to check if the corresponding headers are set correctly
    let mut h = Harness::new(
        |req| {
            let headers = req.headers();
            assert_eq!(headers.get("connection").unwrap(), "close");
            assert_eq!(headers.get("accept-encoding").unwrap(), "gzip");
            assert_eq!(headers.get("icy-metadata"), None);
            assert_eq!(headers.get("user-agent").unwrap(), "test user-agent");

            hyper::Response::new(full_body("Hello World"))
        },
        |src| {
            src.set_property("keep-alive", false);
            src.set_property("compress", true);
            src.set_property("iradio-mode", false);
            src.set_property("user-agent", "test user-agent");
        },
    );

    // Set the HTTP source to Playing so that everything can start
    h.run(|src| {
        src.set_state(gst::State::Playing).unwrap();
    });

    // And now check if the data we receive is exactly what we expect it to be
    let expected_output = "Hello World";
    let mut cursor = Cursor::new(expected_output);

    while let Some(buffer) = h.wait_buffer_or_eos() {
        // On the first buffer also check if the duration reported by the HTTP source is what we
        // would expect it to be
        if cursor.position() == 0 {
            assert_eq!(
                h.src.query_duration::<gst::format::Bytes>(),
                Some(gst::format::Bytes::from_usize(expected_output.len()))
            );
        }

        // Map the buffer readable and check if it contains exactly the data we would expect at
        // this point after reading everything else we read in previous runs
        let map = buffer.map_readable().unwrap();
        let mut read_buf = vec![0; map.size()];
        assert_eq!(cursor.read(&mut read_buf).unwrap(), map.size());
        assert_eq!(&*map, &*read_buf);
    }

    // Check if everything was read
    assert_eq!(cursor.position(), 11);
}

#[test]
fn test_extra_headers() {
    use std::io::{Cursor, Read};

    init();

    // Set up a harness that returns "Hello World" for any HTTP request and check if the
    // extra-headers property works correctly for setting additional headers
    let mut h = Harness::new(
        |req| {
            let headers = req.headers();
            assert_eq!(headers.get("foo").unwrap(), "bar");
            assert_eq!(headers.get("baz").unwrap(), "1");
            assert_eq!(
                headers
                    .get_all("list")
                    .iter()
                    .map(|v| v.to_str().unwrap())
                    .collect::<Vec<&str>>(),
                vec!["1", "2"]
            );
            assert_eq!(
                headers
                    .get_all("array")
                    .iter()
                    .map(|v| v.to_str().unwrap())
                    .collect::<Vec<&str>>(),
                vec!["1", "2"]
            );

            hyper::Response::new(full_body("Hello World"))
        },
        |src| {
            src.set_property(
                "extra-headers",
                gst::Structure::builder("headers")
                    .field("foo", "bar")
                    .field("baz", 1i32)
                    .field("list", gst::List::new([1i32, 2i32]))
                    .field("array", gst::Array::new([1i32, 2i32]))
                    .build(),
            );
        },
    );

    // Set the HTTP source to Playing so that everything can start
    h.run(|src| {
        src.set_state(gst::State::Playing).unwrap();
    });

    // And now check if the data we receive is exactly what we expect it to be
    let expected_output = "Hello World";
    let mut cursor = Cursor::new(expected_output);

    while let Some(buffer) = h.wait_buffer_or_eos() {
        // On the first buffer also check if the duration reported by the HTTP source is what we
        // would expect it to be
        if cursor.position() == 0 {
            assert_eq!(
                h.src.query_duration::<gst::format::Bytes>(),
                Some(gst::format::Bytes::from_usize(expected_output.len()))
            );
        }

        // Map the buffer readable and check if it contains exactly the data we would expect at
        // this point after reading everything else we read in previous runs
        let map = buffer.map_readable().unwrap();
        let mut read_buf = vec![0; map.size()];
        assert_eq!(cursor.read(&mut read_buf).unwrap(), map.size());
        assert_eq!(&*map, &*read_buf);
    }

    // Check if everything was read
    assert_eq!(cursor.position(), 11);
}

#[test]
fn test_cookies_property() {
    use std::io::{Cursor, Read};

    init();

    // Set up a harness that returns "Hello World" for any HTTP request and check if the
    // cookies property can be used to set cookies correctly
    let mut h = Harness::new(
        |req| {
            let headers = req.headers();
            assert_eq!(headers.get("cookie").unwrap(), "foo=1; bar=2; baz=3");

            hyper::Response::new(full_body("Hello World"))
        },
        |src| {
            src.set_property(
                "cookies",
                vec![
                    String::from("foo=1"),
                    String::from("bar=2"),
                    String::from("baz=3"),
                ],
            );
        },
    );

    // Set the HTTP source to Playing so that everything can start
    h.run(|src| {
        src.set_state(gst::State::Playing).unwrap();
    });

    // And now check if the data we receive is exactly what we expect it to be
    let expected_output = "Hello World";
    let mut cursor = Cursor::new(expected_output);

    while let Some(buffer) = h.wait_buffer_or_eos() {
        // On the first buffer also check if the duration reported by the HTTP source is what we
        // would expect it to be
        if cursor.position() == 0 {
            assert_eq!(
                h.src.query_duration::<gst::format::Bytes>(),
                Some(gst::format::Bytes::from_usize(expected_output.len()))
            );
        }

        // Map the buffer readable and check if it contains exactly the data we would expect at
        // this point after reading everything else we read in previous runs
        let map = buffer.map_readable().unwrap();
        let mut read_buf = vec![0; map.size()];
        assert_eq!(cursor.read(&mut read_buf).unwrap(), map.size());
        assert_eq!(&*map, &*read_buf);
    }

    // Check if everything was read
    assert_eq!(cursor.position(), 11);
}

#[test]
fn test_iradio_mode() {
    use std::io::{Cursor, Read};

    init();

    // Set up a harness that returns "Hello World" for any HTTP request and check if the
    // iradio-mode property works correctly, and especially the icy- headers are parsed correctly
    // and put into caps/tags
    let mut h = Harness::new(
        |req| {
            let headers = req.headers();
            assert_eq!(headers.get("icy-metadata").unwrap(), "1");

            hyper::Response::builder()
                .header("icy-metaint", "8192")
                .header("icy-name", "Name")
                .header("icy-genre", "Genre")
                .header("icy-url", "http://www.example.com")
                .header("Content-Type", "audio/mpeg; rate=44100")
                .body(full_body("Hello World"))
                .unwrap()
        },
        |_src| {
            // No additional setup needed here
        },
    );

    // Set the HTTP source to Playing so that everything can start
    h.run(|src| {
        src.set_state(gst::State::Playing).unwrap();
    });

    // And now check if the data we receive is exactly what we expect it to be
    let expected_output = "Hello World";
    let mut cursor = Cursor::new(expected_output);

    while let Some(buffer) = h.wait_buffer_or_eos() {
        // On the first buffer also check if the duration reported by the HTTP source is what we
        // would expect it to be
        if cursor.position() == 0 {
            assert_eq!(
                h.src.query_duration::<gst::format::Bytes>(),
                Some(gst::format::Bytes::from_usize(expected_output.len()))
            );
        }

        // Map the buffer readable and check if it contains exactly the data we would expect at
        // this point after reading everything else we read in previous runs
        let map = buffer.map_readable().unwrap();
        let mut read_buf = vec![0; map.size()];
        assert_eq!(cursor.read(&mut read_buf).unwrap(), map.size());
        assert_eq!(&*map, &*read_buf);
    }

    // Check if everything was read
    assert_eq!(cursor.position(), 11);

    let srcpad = h.src.static_pad("src").unwrap();
    let caps = srcpad.current_caps().unwrap();
    assert_eq!(
        caps,
        gst::Caps::builder("application/x-icy")
            .field("metadata-interval", 8192i32)
            .field("content-type", "audio/mpeg; rate=44100")
            .build()
    );

    {
        if let Some(tag_event) = srcpad.sticky_event::<gst::event::Tag>(0) {
            let tags = tag_event.tag();
            assert_eq!(tags.get::<gst::tags::Organization>().unwrap().get(), "Name");
            assert_eq!(tags.get::<gst::tags::Genre>().unwrap().get(), "Genre");
            assert_eq!(
                tags.get::<gst::tags::Location>().unwrap().get(),
                "http://www.example.com",
            );
        } else {
            unreachable!();
        }
    }
}

#[test]
fn test_audio_l16() {
    use std::io::{Cursor, Read};

    init();

    // Set up a harness that returns "Hello World" for any HTTP request and check if the
    // audio/L16 content type is parsed correctly and put into the caps
    let mut h = Harness::new(
        |_req| {
            hyper::Response::builder()
                .header("Content-Type", "audio/L16; rate=48000; channels=2")
                .body(full_body("Hello World"))
                .unwrap()
        },
        |_src| {
            // No additional setup needed here
        },
    );

    // Set the HTTP source to Playing so that everything can start
    h.run(|src| {
        src.set_state(gst::State::Playing).unwrap();
    });

    // And now check if the data we receive is exactly what we expect it to be
    let expected_output = "Hello World";
    let mut cursor = Cursor::new(expected_output);

    while let Some(buffer) = h.wait_buffer_or_eos() {
        // On the first buffer also check if the duration reported by the HTTP source is what we
        // would expect it to be
        if cursor.position() == 0 {
            assert_eq!(
                h.src.query_duration::<gst::format::Bytes>(),
                Some(gst::format::Bytes::from_usize(expected_output.len()))
            );
        }

        // Map the buffer readable and check if it contains exactly the data we would expect at
        // this point after reading everything else we read in previous runs
        let map = buffer.map_readable().unwrap();
        let mut read_buf = vec![0; map.size()];
        assert_eq!(cursor.read(&mut read_buf).unwrap(), map.size());
        assert_eq!(&*map, &*read_buf);
    }

    // Check if everything was read
    assert_eq!(cursor.position(), 11);

    let srcpad = h.src.static_pad("src").unwrap();
    let caps = srcpad.current_caps().unwrap();
    assert_eq!(
        caps,
        gst::Caps::builder("audio/x-unaligned-raw")
            .field("format", "S16BE")
            .field("layout", "interleaved")
            .field("channels", 2i32)
            .field("rate", 48_000i32)
            .build()
    );
}

#[test]
fn test_authorization() {
    use std::io::{Cursor, Read};

    init();

    // Set up a harness that returns "Hello World" for any HTTP request
    // but requires authentication first
    let mut h = Harness::new(
        |req| {
            let headers = req.headers();

            if let Some(authorization) = headers.get("authorization") {
                assert_eq!(authorization, "Basic dXNlcjpwYXNzd29yZA==");
                hyper::Response::new(full_body("Hello World"))
            } else {
                hyper::Response::builder()
                    .status(reqwest::StatusCode::UNAUTHORIZED.as_u16())
                    .header("WWW-Authenticate", "Basic realm=\"realm\"")
                    .body(empty_body())
                    .unwrap()
            }
        },
        |src| {
            src.set_property("user-id", "user");
            src.set_property("user-pw", "password");
        },
    );

    // Set the HTTP source to Playing so that everything can start
    h.run(|src| {
        src.set_state(gst::State::Playing).unwrap();
    });

    // And now check if the data we receive is exactly what we expect it to be
    let expected_output = "Hello World";
    let mut cursor = Cursor::new(expected_output);

    while let Some(buffer) = h.wait_buffer_or_eos() {
        // On the first buffer also check if the duration reported by the HTTP source is what we
        // would expect it to be
        if cursor.position() == 0 {
            assert_eq!(
                h.src.query_duration::<gst::format::Bytes>(),
                Some(gst::format::Bytes::from_usize(expected_output.len()))
            );
        }

        // Map the buffer readable and check if it contains exactly the data we would expect at
        // this point after reading everything else we read in previous runs
        let map = buffer.map_readable().unwrap();
        let mut read_buf = vec![0; map.size()];
        assert_eq!(cursor.read(&mut read_buf).unwrap(), map.size());
        assert_eq!(&*map, &*read_buf);
    }

    // Check if everything was read
    assert_eq!(cursor.position(), 11);
}

#[test]
fn test_404_error() {
    init();

    // Harness that always returns 404 and we check if that is mapped to the correct error code
    let mut h = Harness::new(
        |_req| {
            hyper::Response::builder()
                .status(reqwest::StatusCode::NOT_FOUND.as_u16())
                .body(empty_body())
                .unwrap()
        },
        |_src| {},
    );

    h.run(|src| {
        let _ = src.set_state(gst::State::Playing);
    });

    let err_code = h.wait_for_error();
    if let Some(err) = err_code.kind::<gst::ResourceError>() {
        assert_eq!(err, gst::ResourceError::NotFound);
    }
}

#[test]
fn test_403_error() {
    init();

    // Harness that always returns 403 and we check if that is mapped to the correct error code
    let mut h = Harness::new(
        |_req| {
            hyper::Response::builder()
                .status(reqwest::StatusCode::FORBIDDEN.as_u16())
                .body(empty_body())
                .unwrap()
        },
        |_src| {},
    );

    h.run(|src| {
        let _ = src.set_state(gst::State::Playing);
    });

    let err_code = h.wait_for_error();
    if let Some(err) = err_code.kind::<gst::ResourceError>() {
        assert_eq!(err, gst::ResourceError::NotAuthorized);
    }
}

#[test]
fn test_network_error() {
    init();

    // Harness that always fails with a network error
    let mut h = Harness::new(
        |_req| unreachable!(),
        |src| {
            src.set_property("location", "http://0.0.0.0:0");
        },
    );

    h.run(|src| {
        let _ = src.set_state(gst::State::Playing);
    });

    let err_code = h.wait_for_error();
    if let Some(err) = err_code.kind::<gst::ResourceError>() {
        assert_eq!(err, gst::ResourceError::OpenRead);
    }
}

#[test]
fn test_seek_after_ready() {
    use std::io::{Cursor, Read};

    init();

    // Harness that checks if seeking in Ready state works correctly
    let mut h = Harness::new(
        |req| {
            let headers = req.headers();
            if let Some(range) = headers.get("Range") {
                if range == "bytes=123-" {
                    let mut data_seek = vec![0; 8192 - 123];
                    for (i, d) in data_seek.iter_mut().enumerate() {
                        *d = ((i + 123) % 256) as u8;
                    }

                    hyper::Response::builder()
                        .header("content-length", 8192 - 123)
                        .header("accept-ranges", "bytes")
                        .header("content-range", "bytes 123-8192/8192")
                        .body(full_body(data_seek))
                        .unwrap()
                } else {
                    panic!("Received an unexpected Range header")
                }
            } else {
                // `panic!("Received no Range header")` should be called here but due to a bug
                // in `basesrc` we cant do that here. If we do a seek in READY state, basesrc
                // will do a `start()` call without seek. Once we get seek forwarded, the call
                // with seek is made. This issue has to be solved.
                // issue link: https://gitlab.freedesktop.org/gstreamer/gstreamer/issues/413
                let mut data_full = vec![0; 8192];
                for (i, d) in data_full.iter_mut().enumerate() {
                    *d = (i % 256) as u8;
                }

                hyper::Response::builder()
                    .header("content-length", 8192)
                    .header("accept-ranges", "bytes")
                    .body(full_body(data_full))
                    .unwrap()
            }
        },
        |_src| {},
    );

    h.run(|src| {
        src.set_state(gst::State::Ready).unwrap();
    });

    let current_state = h.wait_for_state_change();
    assert_eq!(current_state, gst::State::Ready);

    h.run(|src| {
        src.seek_simple(gst::SeekFlags::FLUSH, 123.bytes()).unwrap();
        src.set_state(gst::State::Playing).unwrap();
    });

    let segment = h.wait_for_segment(false, true);
    assert_eq!(segment.start(), Some(123.bytes()));

    let mut expected_output = vec![0; 8192 - 123];
    for (i, d) in expected_output.iter_mut().enumerate() {
        *d = ((123 + i) % 256) as u8;
    }
    let mut cursor = Cursor::new(expected_output);

    while let Some(buffer) = h.wait_buffer_or_eos() {
        assert_eq!(buffer.offset(), 123 + cursor.position());

        let map = buffer.map_readable().unwrap();
        let mut read_buf = vec![0; map.size()];

        assert_eq!(cursor.read(&mut read_buf).unwrap(), map.size());
        assert_eq!(&*map, &*read_buf);
    }
}

#[test]
fn test_seek_after_buffer_received() {
    use std::io::{Cursor, Read};

    init();

    // Harness that checks if seeking in Playing state after having received a buffer works
    // correctly
    let mut h = Harness::new(
        |req| {
            let headers = req.headers();
            if let Some(range) = headers.get("Range") {
                if range == "bytes=123-" {
                    let mut data_seek = vec![0; 8192 - 123];
                    for (i, d) in data_seek.iter_mut().enumerate() {
                        *d = ((i + 123) % 256) as u8;
                    }

                    hyper::Response::builder()
                        .header("content-length", 8192 - 123)
                        .header("accept-ranges", "bytes")
                        .header("content-range", "bytes 123-8192/8192")
                        .body(full_body(data_seek))
                        .unwrap()
                } else {
                    panic!("Received an unexpected Range header")
                }
            } else {
                let mut data_full = vec![0; 8192];
                for (i, d) in data_full.iter_mut().enumerate() {
                    *d = (i % 256) as u8;
                }

                hyper::Response::builder()
                    .header("content-length", 8192)
                    .header("accept-ranges", "bytes")
                    .body(full_body(data_full))
                    .unwrap()
            }
        },
        |_src| {},
    );

    h.run(|src| {
        src.set_state(gst::State::Playing).unwrap();
    });

    //wait for a buffer
    let buffer = h.wait_buffer_or_eos().unwrap();
    assert_eq!(buffer.offset(), 0);

    //seek to a position after a buffer is Received
    h.run(|src| {
        src.seek_simple(gst::SeekFlags::FLUSH, 123.bytes()).unwrap();
    });

    let segment = h.wait_for_segment(true, true);
    assert_eq!(segment.start(), Some(123.bytes()));

    let mut expected_output = vec![0; 8192 - 123];
    for (i, d) in expected_output.iter_mut().enumerate() {
        *d = ((123 + i) % 256) as u8;
    }
    let mut cursor = Cursor::new(expected_output);

    while let Some(buffer) = h.wait_buffer_or_eos() {
        assert_eq!(buffer.offset(), 123 + cursor.position());

        let map = buffer.map_readable().unwrap();
        let mut read_buf = vec![0; map.size()];

        assert_eq!(cursor.read(&mut read_buf).unwrap(), map.size());
        assert_eq!(&*map, &*read_buf);
    }
}

#[test]
fn test_seek_with_stop_position() {
    use std::io::{Cursor, Read};

    init();

    // Harness that checks if seeking in Playing state after having received a buffer works
    // correctly
    let mut h = Harness::new(
        |req| {
            let headers = req.headers();
            if let Some(range) = headers.get("Range") {
                if range == "bytes=123-130" {
                    let mut data_seek = vec![0; 8];
                    for (i, d) in data_seek.iter_mut().enumerate() {
                        *d = ((i + 123) % 256) as u8;
                    }

                    hyper::Response::builder()
                        .header("content-length", 8)
                        .header("accept-ranges", "bytes")
                        .header("content-range", "bytes 123-130/8192")
                        .body(full_body(data_seek))
                        .unwrap()
                } else {
                    panic!("Received an unexpected Range header")
                }
            } else {
                let mut data_full = vec![0; 8192];
                for (i, d) in data_full.iter_mut().enumerate() {
                    *d = (i % 256) as u8;
                }

                hyper::Response::builder()
                    .header("content-length", 8192)
                    .header("accept-ranges", "bytes")
                    .body(full_body(data_full))
                    .unwrap()
            }
        },
        |_src| {},
    );

    h.run(|src| {
        src.set_state(gst::State::Playing).unwrap();
    });

    //wait for a buffer
    let buffer = h.wait_buffer_or_eos().unwrap();
    assert_eq!(buffer.offset(), 0);

    //seek to a position after a buffer is Received
    let start = 123.bytes();
    let stop = 131.bytes();
    h.run(move |src| {
        src.seek(
            1.0,
            gst::SeekFlags::FLUSH,
            gst::SeekType::Set,
            start,
            gst::SeekType::Set,
            stop,
        )
        .unwrap();
    });

    let segment = h.wait_for_segment(true, true);
    assert_eq!(segment.start(), Some(start));
    assert_eq!(segment.stop(), Some(stop));

    let mut expected_output = vec![0; 8];
    for (i, d) in expected_output.iter_mut().enumerate() {
        *d = ((123 + i) % 256) as u8;
    }
    let mut cursor = Cursor::new(expected_output);

    while let Some(buffer) = h.wait_buffer_or_eos() {
        assert_eq!(buffer.offset(), 123 + cursor.position());

        let map = buffer.map_readable().unwrap();
        let mut read_buf = vec![0; map.size()];

        assert_eq!(cursor.read(&mut read_buf).unwrap(), map.size());
        assert_eq!(&*map, &*read_buf);
    }
}

#[test]
fn test_cookies() {
    init();

    // Set up a harness that returns "Hello World" for any HTTP request and sets a cookie in our
    // client
    let mut h = Harness::new(
        |_req| {
            hyper::Response::builder()
                .header("Set-Cookie", "foo=bar")
                .body(full_body("Hello World"))
                .unwrap()
        },
        |_src| {
            // No additional setup needed here
        },
    );

    // Set the HTTP source to Playing so that everything can start
    h.run(|src| {
        src.set_state(gst::State::Playing).unwrap();
    });

    let mut num_bytes = 0;
    while let Some(buffer) = h.wait_buffer_or_eos() {
        num_bytes += buffer.size();
    }
    assert_eq!(num_bytes, 11);

    // Set up a second harness that returns "Hello World" for any HTTP request that checks if our
    // client provides the cookie that was set in the previous request
    let mut h2 = Harness::new(
        |req| {
            let headers = req.headers();
            let cookies = headers
                .get("Cookie")
                .expect("No cookies set")
                .to_str()
                .unwrap();
            assert!(cookies.split(';').any(|c| c == "foo=bar"));
            hyper::Response::builder()
                .body(full_body("Hello again!"))
                .unwrap()
        },
        |_src| {
            // No additional setup needed here
        },
    );

    let context = h.src.context("gst.reqwest.client").expect("No context");
    h2.src.set_context(&context);

    // Set the HTTP source to Playing so that everything can start
    h2.run(|src| {
        src.set_state(gst::State::Playing).unwrap();
    });

    let mut num_bytes = 0;
    while let Some(buffer) = h2.wait_buffer_or_eos() {
        num_bytes += buffer.size();
    }
    assert_eq!(num_bytes, 12);
}

#[test]
fn test_proxy_prop_souphttpsrc_compatibility() {
    init();

    fn assert_proxy_set(set_to: Option<&str>, expected: Option<&str>) {
        // The same assertions should hold for "souphttpsrc".
        let src = gst::ElementFactory::make("reqwesthttpsrc")
            .property("proxy", set_to)
            .build()
            .unwrap();
        assert_eq!(src.property::<Option<String>>("proxy").as_deref(), expected);
    }

    // Test env var proxy.
    assert_proxy_set(Some("http://mydomain/"), Some("http://mydomain/"));

    // It should prepend http if no protocol specified and add /.
    assert_proxy_set(Some("myotherdomain"), Some("http://myotherdomain/"));

    // Empty env var should result in "" proxy (meaning None) for compatibility.
    assert_proxy_set(Some(""), Some(""));

    // It should allow setting this value for proxy for compatibility.
    assert_proxy_set(Some("&$"), Some("http://&$/"));

    // No env var should result in "" proxy (meaning None) for compatibility.
    assert_proxy_set(None, Some(""));
}

#[test]
fn test_proxy() {
    init();

    let mut h = Harness::new(
        |_req| {
            hyper::Response::builder()
                .body(full_body("Hello Proxy World"))
                .unwrap()
        },
        |_src| {},
    );

    // Simplest possible implementation of naive oneshot proxy server?
    // Listen on socket before spawning thread (we won't error out with connection refused).
    let (proxy_handle, proxy_addr) = {
        let (proxy_addr_sender, proxy_addr_receiver) = tokio::sync::oneshot::channel();

        let proxy_handle = h.rt.spawn(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let proxy_addr = listener.local_addr().unwrap();
            println!("listening on {proxy_addr}, starting proxy server");

            proxy_addr_sender.send(proxy_addr).unwrap();

            println!("awaiting connection to proxy server");
            let (conn, _addr) = listener.accept().await.unwrap();

            let (conn_reader, mut conn_writer) = tokio::io::split(conn);
            println!("client connected, reading request line");
            let mut reader = tokio::io::BufReader::new(conn_reader);
            let mut buf = String::new();
            reader.read_line(&mut buf).await.unwrap();
            let parts: Vec<&str> = buf.split(' ').collect();
            let url = reqwest::Url::parse(parts[1]).unwrap();
            let host = format!(
                "{}:{}",
                url.host_str().unwrap(),
                url.port_or_known_default().unwrap()
            );

            println!("connecting to target server {host}");
            let mut server_connection = tokio::net::TcpStream::connect(host).await.unwrap();

            println!("connected to target server, sending modified request line");
            server_connection
                .write_all(format!("{} {} {}", parts[0], url.path(), parts[2]).as_bytes())
                .await
                .unwrap();

            let (mut server_reader, mut server_writer) = tokio::io::split(server_connection);

            println!("sent modified request line, forwarding data in both directions");
            let send_join_handle = tokio::task::spawn(async move {
                tokio::io::copy(&mut reader, &mut server_writer)
                    .await
                    .unwrap();
            });
            tokio::io::copy(&mut server_reader, &mut conn_writer)
                .await
                .unwrap();
            send_join_handle.await.unwrap();
            println!("shutting down proxy server");
        });

        (
            proxy_handle,
            futures::executor::block_on(proxy_addr_receiver).unwrap(),
        )
    };

    // Set the HTTP source to Playing so that everything can start.
    h.run(move |src| {
        src.set_property("proxy", proxy_addr.to_string());
        src.set_state(gst::State::Playing).unwrap();
    });

    // Wait for a buffer.
    let mut num_bytes = 0;
    while let Some(buffer) = h.wait_buffer_or_eos() {
        num_bytes += buffer.size();
    }
    assert_eq!(num_bytes, "Hello Proxy World".len());

    // Don't leave threads hanging around.
    proxy_handle.abort();
    let _ = futures::executor::block_on(proxy_handle);
}

/// Adapter from tokio IO traits to hyper IO traits.
mod tokio_io {
    use pin_project_lite::pin_project;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    pin_project! {
        #[derive(Debug)]
        pub struct TokioIo<T> {
            #[pin]
            inner: T,
        }
    }

    impl<T> TokioIo<T> {
        pub fn new(inner: T) -> Self {
            Self { inner }
        }
    }

    impl<T> hyper::rt::Read for TokioIo<T>
    where
        T: tokio::io::AsyncRead,
    {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            mut buf: hyper::rt::ReadBufCursor<'_>,
        ) -> Poll<Result<(), std::io::Error>> {
            let n = unsafe {
                let mut tbuf = tokio::io::ReadBuf::uninit(buf.as_mut());
                match tokio::io::AsyncRead::poll_read(self.project().inner, cx, &mut tbuf) {
                    Poll::Ready(Ok(())) => tbuf.filled().len(),
                    other => return other,
                }
            };

            unsafe {
                buf.advance(n);
            }
            Poll::Ready(Ok(()))
        }
    }

    impl<T> hyper::rt::Write for TokioIo<T>
    where
        T: tokio::io::AsyncWrite,
    {
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<Result<usize, std::io::Error>> {
            tokio::io::AsyncWrite::poll_write(self.project().inner, cx, buf)
        }

        fn poll_flush(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), std::io::Error>> {
            tokio::io::AsyncWrite::poll_flush(self.project().inner, cx)
        }

        fn poll_shutdown(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Result<(), std::io::Error>> {
            tokio::io::AsyncWrite::poll_shutdown(self.project().inner, cx)
        }

        fn is_write_vectored(&self) -> bool {
            tokio::io::AsyncWrite::is_write_vectored(&self.inner)
        }

        fn poll_write_vectored(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            bufs: &[std::io::IoSlice<'_>],
        ) -> Poll<Result<usize, std::io::Error>> {
            tokio::io::AsyncWrite::poll_write_vectored(self.project().inner, cx, bufs)
        }
    }
}
