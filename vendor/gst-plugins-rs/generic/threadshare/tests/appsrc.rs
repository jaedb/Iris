// Copyright (C) 2018 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::prelude::*;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstthreadshare::plugin_register_static().expect("gstthreadshare appsrc test");
    });
}

#[test]
fn push() {
    init();

    let mut h = gst_check::Harness::new("ts-appsrc");

    let caps = gst::Caps::builder("foo/bar").build();
    {
        let appsrc = h.element().unwrap();
        appsrc.set_property("caps", &caps);
        appsrc.set_property("do-timestamp", true);
        appsrc.set_property("context", "appsrc-push");
    }

    h.play();

    {
        let appsrc = h.element().unwrap();
        for _ in 0..3 {
            assert!(appsrc.emit_by_name::<bool>("push-buffer", &[&gst::Buffer::new()]));
        }

        assert!(appsrc.emit_by_name::<bool>("end-of-stream", &[]));
    }

    for _ in 0..3 {
        let _buffer = h.pull().unwrap();
    }

    let mut n_events = 0;
    loop {
        use gst::EventView;

        let event = h.pull_event().unwrap();
        match event.view() {
            EventView::StreamStart(..) => {
                assert_eq!(n_events, 0);
            }
            EventView::Caps(ev) => {
                assert_eq!(n_events, 1);
                let event_caps = ev.caps();
                assert_eq!(caps.as_ref(), event_caps);
            }
            EventView::Segment(..) => {
                assert_eq!(n_events, 2);
            }
            EventView::Eos(..) => {
                break;
            }
            _ => (),
        }
        n_events += 1;
    }
    assert!(n_events >= 2);
}

#[test]
fn pause_regular() {
    init();

    let mut h = gst_check::Harness::new("ts-appsrc");

    let caps = gst::Caps::builder("foo/bar").build();
    {
        let appsrc = h.element().unwrap();
        appsrc.set_property("caps", &caps);
        appsrc.set_property("do-timestamp", true);
        appsrc.set_property("context", "appsrc-pause");
    }

    h.play();

    let appsrc = h.element().unwrap();

    // Initial buffer
    assert!(
        appsrc.emit_by_name::<bool>("push-buffer", &[&gst::Buffer::from_slice(vec![1, 2, 3, 4])])
    );

    let _ = h.pull().unwrap();

    // Pre-pause buffer
    assert!(appsrc.emit_by_name::<bool>("push-buffer", &[&gst::Buffer::from_slice(vec![5, 6, 7])]));

    appsrc
        .change_state(gst::StateChange::PlayingToPaused)
        .unwrap();

    // Buffer is queued during Paused
    assert!(appsrc.emit_by_name::<bool>("push-buffer", &[&gst::Buffer::from_slice(vec![8, 9])]));

    appsrc
        .change_state(gst::StateChange::PausedToPlaying)
        .unwrap();

    // Pull Pre-pause buffer
    let _ = h.pull().unwrap();

    // Pull buffer queued while Paused
    let _ = h.pull().unwrap();

    // Can push again
    assert!(appsrc.emit_by_name::<bool>("push-buffer", &[&gst::Buffer::new()]));

    let _ = h.pull().unwrap();
    assert!(h.try_pull().is_none());
}

#[test]
fn flush_regular() {
    init();

    let mut h = gst_check::Harness::new("ts-appsrc");

    let caps = gst::Caps::builder("foo/bar").build();
    {
        let appsrc = h.element().unwrap();
        appsrc.set_property("caps", &caps);
        appsrc.set_property("do-timestamp", true);
        appsrc.set_property("context", "appsrc-flush");
    }

    h.play();

    let appsrc = h.element().unwrap();

    // Initial buffer
    assert!(
        appsrc.emit_by_name::<bool>("push-buffer", &[&gst::Buffer::from_slice(vec![1, 2, 3, 4])])
    );

    let _ = h.pull().unwrap();

    // FlushStart
    assert!(h.push_upstream_event(gst::event::FlushStart::new()));

    // Can't push buffer while flushing
    assert!(!appsrc.emit_by_name::<bool>("push-buffer", &[&gst::Buffer::new()]));

    assert!(h.try_pull().is_none());

    // FlushStop
    assert!(h.push_upstream_event(gst::event::FlushStop::new(true)));

    // No buffer available due to flush
    assert!(h.try_pull().is_none());

    // Can push again
    assert!(appsrc.emit_by_name::<bool>("push-buffer", &[&gst::Buffer::new()]));

    let _ = h.pull().unwrap();
    assert!(h.try_pull().is_none());
}

#[test]
fn pause_flush() {
    init();

    let mut h = gst_check::Harness::new("ts-appsrc");

    let caps = gst::Caps::builder("foo/bar").build();
    {
        let appsrc = h.element().unwrap();
        appsrc.set_property("caps", &caps);
        appsrc.set_property("do-timestamp", true);
        appsrc.set_property("context", "appsrc-pause_flush");
    }

    h.play();

    let appsrc = h.element().unwrap();

    // Initial buffer
    assert!(
        appsrc.emit_by_name::<bool>("push-buffer", &[&gst::Buffer::from_slice(vec![1, 2, 3, 4])])
    );

    let _ = h.pull().unwrap();

    appsrc
        .change_state(gst::StateChange::PlayingToPaused)
        .unwrap();

    // FlushStart
    assert!(h.push_upstream_event(gst::event::FlushStart::new()));

    // Can't push buffers while flushing
    assert!(!appsrc.emit_by_name::<bool>("push-buffer", &[&gst::Buffer::new()]));

    assert!(h.try_pull().is_none());

    // FlushStop
    assert!(h.push_upstream_event(gst::event::FlushStop::new(true)));

    appsrc
        .change_state(gst::StateChange::PausedToPlaying)
        .unwrap();

    // No buffer available due to flush
    assert!(h.try_pull().is_none());

    // Can push again
    assert!(appsrc.emit_by_name::<bool>("push-buffer", &[&gst::Buffer::new()]));

    let _ = h.pull().unwrap();
    assert!(h.try_pull().is_none());
}
