// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-ts-blocking-adapter
 * @title: ts-blocking-adapter
 *
 * Thread-sharing downstream blocking branch async adapter.
 *
 * When a threadshare element streams buffers to an element which synchronizes on
 * the clock or which uses a limited blocking queue, the threadshare context is
 * perdiodically blocked, preventing other async tasks from progressing.
 *
 * One workaround is to use a regular `queue` with enough buffering to cope with
 * early buffers. This is suboptimal though because this `queue` doesn't propagate
 * backpressure upstream.
 *
 * The `ts-blocking-adapter` applies an async backpressure when downstream blocks,
 * allowing the threadshare context to process other tasks. This is achieved by
 * having the 'sink' Pad forward serialized items (buffers, ...) to the 'src' Pad
 * Task (thread) via a rendezvous channel.
 *
 * Since: plugins-rs-0.14.2
 */
use std::sync::{LazyLock, Mutex};

use flume::{Receiver, Sender};
use futures::future;
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use crate::runtime::prelude::*;
use crate::runtime::PadSink;

#[derive(Debug)]
enum Item {
    Buffer(gst::Buffer),
    List(gst::BufferList),
    Event(gst::Event),
    Stop,
}

#[derive(Clone)]
struct BlockingAdapterPadSinkHandler;

impl PadSinkHandler for BlockingAdapterPadSinkHandler {
    type ElementImpl = BlockingAdapter;

    async fn sink_chain(
        self,
        pad: gst::Pad,
        elem: super::BlockingAdapter,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(CAT, obj = pad, "Handling {buffer:?}");
        elem.imp().hand_item_to_task(Item::Buffer(buffer)).await
    }

    async fn sink_chain_list(
        self,
        pad: gst::Pad,
        elem: super::BlockingAdapter,
        list: gst::BufferList,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::log!(CAT, obj = pad, "Handling {list:?}");
        elem.imp().hand_item_to_task(Item::List(list)).await
    }

    fn sink_event(self, pad: &gst::Pad, imp: &BlockingAdapter, event: gst::Event) -> bool {
        gst::log!(CAT, obj = pad, "Handling non-serialized {event:?}");

        if let gst::EventView::FlushStart(..) = event.view() {
            imp.flush_start();
        }

        imp.srcpad.push_event(event)
    }

    async fn sink_event_serialized(
        self,
        pad: gst::Pad,
        elem: super::BlockingAdapter,
        event: gst::Event,
    ) -> bool {
        gst::log!(CAT, obj = pad, "Handling serialized {event:?}");

        let imp = elem.imp();

        if let gst::EventView::FlushStop(..) = event.view() {
            imp.flush_stop();
        }

        imp.hand_item_to_task(Item::Event(event)).await.is_ok()
    }

    fn sink_query(self, pad: &gst::Pad, imp: &BlockingAdapter, query: &mut gst::QueryRef) -> bool {
        gst::log!(CAT, obj = pad, "Handling {query:?}");

        // Note about serialized queries:
        // if upstream is able to pass a serialized query,
        // then buffer backpressure is not active,
        // which means downstream is not blocked and is ready to handle the query.

        imp.srcpad.peer_query(query)
    }
}

#[derive(Debug)]
struct ItemHandler {
    is_flushing: bool,
    item_tx: flume::Sender<Item>,
    res_rx: Receiver<Result<gst::FlowSuccess, gst::FlowError>>,
    hand_item_abort_handle: Option<future::AbortHandle>,
}

#[derive(Debug)]
pub struct BlockingAdapter {
    sinkpad: PadSink,
    srcpad: gst::Pad,
    item_handler: Mutex<ItemHandler>,
    item_rx: Receiver<Item>,
    res_tx: Sender<Result<gst::FlowSuccess, gst::FlowError>>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "ts-blocking-adapter",
        gst::DebugColorFlags::empty(),
        Some("Thread-sharing blocking adapter"),
    )
});

impl BlockingAdapter {
    async fn hand_item_to_task(&self, item: Item) -> Result<gst::FlowSuccess, gst::FlowError> {
        let hand_item_fut = {
            let mut handler = self.item_handler.lock().unwrap();
            if handler.is_flushing {
                return Err(gst::FlowError::Flushing);
            }

            let (hand_item_fut, abort_handle) = future::abortable({
                let item_tx = handler.item_tx.clone();
                let res_rx = handler.res_rx.clone();
                async move {
                    item_tx
                        .send_async(item)
                        .await
                        .expect("channel always valid");
                    res_rx.recv_async().await.expect("channel always valid")
                }
            });

            handler.hand_item_abort_handle = Some(abort_handle);

            hand_item_fut
        };

        match hand_item_fut.await {
            Ok(res) => {
                if res.is_err() {
                    gst::error!(CAT, imp = self, "Error handing item to task {res:?}");
                    self.flush_start();
                }

                res
            }
            Err(future::Aborted) => {
                gst::debug!(CAT, imp = self, "Handing item to task aborted");
                self.flush_start();
                Err(gst::FlowError::Flushing)
            }
        }
    }

    fn flush_start(&self) {
        let mut sender = self.item_handler.lock().unwrap();

        sender.is_flushing = true;

        if let Some(abort_handle) = sender.hand_item_abort_handle.take() {
            abort_handle.abort();
        }
    }

    fn flush_stop(&self) {
        self.item_handler.lock().unwrap().is_flushing = false;
    }

    fn src_activatemode(
        &self,
        pad: &gst::Pad,
        imp: &BlockingAdapter,
        mode: gst::PadMode,
        active: bool,
    ) -> Result<(), gst::LoggableError> {
        if mode != gst::PadMode::Push {
            return Err(gst::LoggableError::new(
                *CAT,
                glib::bool_error!("Unsupported pad mode {mode:?}"),
            ));
        }

        if !active {
            gst::trace!(CAT, obj = pad, "Stopping task");
            imp.flush_start();
            // Unlock the task loop
            let _ = imp
                .item_handler
                .lock()
                .unwrap()
                .item_tx
                .try_send(Item::Stop);
            let _ = imp.srcpad.send_event(gst::event::FlushStart::new());
            pad.stop_task()?;
            gst::trace!(CAT, obj = pad, "Task stopped");

            return Ok(());
        }

        gst::trace!(CAT, obj = pad, "Starting task");
        pad.start_task({
            let weak_elem = imp.obj().downgrade();
            let item_rx = imp.item_rx.clone();
            let res_tx = imp.res_tx.clone();
            move || {
                let res = item_rx.recv();

                let Some(elem) = weak_elem.upgrade() else {
                    return;
                };
                let imp = elem.imp();
                let pad = &imp.srcpad;

                gst::trace!(CAT, obj = pad, "Recv returned: {res:?}");

                let pause = || {
                    imp.flush_start();
                    let _ = pad.pause_task();
                };

                match res {
                    Ok(Item::Buffer(buffer)) => {
                        gst::log!(CAT, obj = pad, "Handling {buffer:?}");
                        let res = pad.push(buffer);

                        if let Err(err) = res {
                            gst::info!(CAT, obj = pad, "Error forwarding buffer: {err}");
                        }
                        if res_tx.send(res).is_err() || res.is_err() {
                            pause();
                        }
                    }
                    Ok(Item::List(list)) => {
                        gst::log!(CAT, obj = pad, "Handling {list:?}");
                        let res = pad.push_list(list);

                        if let Err(err) = res {
                            gst::info!(CAT, obj = pad, "Error forwarding list: {err}");
                        }
                        if res_tx.send(res).is_err() || res.is_err() {
                            pause();
                        }
                    }
                    Ok(Item::Event(event)) => {
                        gst::log!(CAT, obj = pad, "Handling {event:?}");

                        if let gst::EventView::FlushStart(_) = event.view() {
                            imp.flush_start();
                            let _ = pad.push_event(event);
                            return;
                        }

                        let res = if pad.push_event(event) {
                            Ok(gst::FlowSuccess::Ok)
                        } else {
                            gst::info!(CAT, obj = pad, "Error forwarding event");
                            Err(gst::FlowError::Error)
                        };

                        if res_tx.send(res).is_err() || res.is_err() {
                            pause();
                        }
                    }
                    Ok(Item::Stop) => {
                        gst::log!(CAT, obj = pad, "Stopping task");
                        // Make sure the task doesn't iterate again before it's actually stopped
                        pause();
                    }
                    Err(_) => pause(),
                }
            }
        })
        .map_err(|err| gst::loggable_error!(CAT, "Failed to start pad task: {err}"))?;

        imp.flush_stop();

        Ok(())
    }
}

#[glib::object_subclass]
impl ObjectSubclass for BlockingAdapter {
    const NAME: &'static str = "GstTsBlockingAdapter";
    type Type = super::BlockingAdapter;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let (item_tx, item_rx) = flume::bounded(0);
        let (res_tx, res_rx) = flume::bounded(0);

        let srcpad = gst::Pad::builder_from_template(&klass.pad_template("src").unwrap())
            .event_function(|pad, parent, event| {
                BlockingAdapter::catch_panic_pad_function(
                    parent,
                    || false,
                    |this| {
                        gst::log!(CAT, obj = pad, "Handling upstrem {event:?}");

                        use gst::EventView;
                        match event.view() {
                            EventView::FlushStart(..) => this.flush_start(),
                            EventView::FlushStop(..) => this.flush_stop(),
                            _ => (),
                        }

                        this.sinkpad.gst_pad().push_event(event)
                    },
                )
            })
            .query_function(|pad, parent, query| {
                BlockingAdapter::catch_panic_pad_function(
                    parent,
                    || false,
                    |this| {
                        gst::log!(CAT, obj = pad, "Forwarding {query:?}");
                        this.sinkpad.gst_pad().peer_query(query)
                    },
                )
            })
            .activatemode_function(|pad, parent, mode, active| {
                BlockingAdapter::catch_panic_pad_function(
                    parent,
                    || {
                        Err(gst::loggable_error!(
                            CAT,
                            "Panic activating src pad with mode"
                        ))
                    },
                    |this| this.src_activatemode(pad, this, mode, active),
                )
            })
            .build();

        Self {
            sinkpad: PadSink::new(
                gst::Pad::from_template(&klass.pad_template("sink").unwrap()),
                BlockingAdapterPadSinkHandler,
            ),
            srcpad,
            item_handler: Mutex::new(ItemHandler {
                is_flushing: false,
                item_tx,
                res_rx,
                hand_item_abort_handle: None,
            }),
            item_rx,
            res_tx,
        }
    }
}

impl ObjectImpl for BlockingAdapter {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(self.sinkpad.gst_pad()).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }
}

impl GstObjectImpl for BlockingAdapter {}

impl ElementImpl for BlockingAdapter {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Thread-sharing blocking adapter",
                "Generic",
                "Converts a blocking downstream branch into an async backpressure",
                "François Laignel <francois@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::new_any();

            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            vec![sink_pad_template, src_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}
