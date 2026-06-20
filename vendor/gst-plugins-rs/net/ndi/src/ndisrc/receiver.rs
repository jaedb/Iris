// SPDX-License-Identifier: MPL-2.0

use glib::prelude::*;
use gst::prelude::*;

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::thread;
use std::time;

use std::sync::LazyLock;

use crate::ndi::*;
use crate::ndisrcmeta::Buffer;
use crate::ndisys::*;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "ndireceiver",
        gst::DebugColorFlags::empty(),
        Some("NewTek NDI receiver"),
    )
});

pub struct Receiver(Arc<ReceiverInner>);

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum ReceiverItem {
    Buffer(Buffer),
    Flushing,
    Timeout,
    Error(gst::FlowError),
}

struct ReceiverInner {
    queue: ReceiverQueue,
    max_queue_length: usize,

    element: glib::WeakRef<gst::Element>,

    timeout: u32,
    connect_timeout: u32,

    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

#[derive(Clone)]
struct ReceiverQueue(Arc<(Mutex<ReceiverQueueInner>, Condvar)>);

struct ReceiverQueueInner {
    // Set to true when the capture thread should be stopped
    shutdown: bool,

    // If we're flushing right now and all buffers should simply be discarded
    // and capture() directly returns Flushing
    flushing: bool,

    // If we're playing right now or not: if not we simply discard everything captured
    playing: bool,
    // Queue containing our buffers. This holds at most 5 buffers at a time.
    //
    // On timeout/error will contain a single item and then never be filled again
    buffer_queue: VecDeque<Buffer>,

    error: Option<gst::FlowError>,
    timeout: bool,
}

#[derive(Clone)]
pub struct ReceiverControlHandle {
    queue: ReceiverQueue,
}

impl ReceiverControlHandle {
    pub fn set_flushing(&self, flushing: bool) {
        let mut queue = (self.queue.0).0.lock().unwrap();
        queue.flushing = flushing;
        (self.queue.0).1.notify_all();
    }

    pub fn set_playing(&self, playing: bool) {
        let mut queue = (self.queue.0).0.lock().unwrap();
        queue.playing = playing;
    }

    pub fn shutdown(&self) {
        let mut queue = (self.queue.0).0.lock().unwrap();
        queue.shutdown = true;
        (self.queue.0).1.notify_all();
    }
}

impl Drop for ReceiverInner {
    fn drop(&mut self) {
        // Will shut down the receiver thread on the next iteration
        let mut queue = (self.queue.0).0.lock().unwrap();
        queue.shutdown = true;
        drop(queue);

        let element = self.element.upgrade();

        if let Some(ref element) = element {
            gst::debug!(CAT, obj = element, "Closed NDI connection");
        }
    }
}

impl Receiver {
    fn new(
        recv: RecvInstance,
        timeout: u32,
        connect_timeout: u32,
        max_queue_length: usize,
        element: &gst::Element,
    ) -> Self {
        let receiver = Receiver(Arc::new(ReceiverInner {
            queue: ReceiverQueue(Arc::new((
                Mutex::new(ReceiverQueueInner {
                    shutdown: false,
                    playing: false,
                    flushing: false,
                    buffer_queue: VecDeque::with_capacity(max_queue_length),
                    error: None,
                    timeout: false,
                }),
                Condvar::new(),
            ))),
            max_queue_length,
            element: element.downgrade(),
            timeout,
            connect_timeout,
            thread: Mutex::new(None),
        }));

        let weak = Arc::downgrade(&receiver.0);
        let thread = thread::spawn(move || {
            use std::panic;

            let weak_clone = weak.clone();
            match panic::catch_unwind(panic::AssertUnwindSafe(move || {
                Self::receive_thread(&weak_clone, recv)
            })) {
                Ok(_) => (),
                Err(_) => {
                    if let Some(receiver) = weak.upgrade().map(Receiver) {
                        if let Some(element) = receiver.0.element.upgrade() {
                            gst::element_error!(
                                element,
                                gst::LibraryError::Failed,
                                ["Panic while connecting to NDI source"]
                            );
                        }

                        let mut queue = (receiver.0.queue.0).0.lock().unwrap();
                        queue.error = Some(gst::FlowError::Error);
                        (receiver.0.queue.0).1.notify_one();
                    }
                }
            }
        });

        *receiver.0.thread.lock().unwrap() = Some(thread);

        receiver
    }

    pub fn receiver_control_handle(&self) -> ReceiverControlHandle {
        ReceiverControlHandle {
            queue: self.0.queue.clone(),
        }
    }

    #[allow(dead_code)]
    pub fn set_flushing(&self, flushing: bool) {
        let mut queue = (self.0.queue.0).0.lock().unwrap();
        queue.flushing = flushing;
        (self.0.queue.0).1.notify_all();
    }

    #[allow(dead_code)]
    pub fn set_playing(&self, playing: bool) {
        let mut queue = (self.0.queue.0).0.lock().unwrap();
        queue.playing = playing;
    }

    #[allow(dead_code)]
    pub fn shutdown(&self) {
        let mut queue = (self.0.queue.0).0.lock().unwrap();
        queue.shutdown = true;
        (self.0.queue.0).1.notify_all();
    }

    pub fn capture(&self) -> ReceiverItem {
        let mut queue = (self.0.queue.0).0.lock().unwrap();
        loop {
            if let Some(err) = queue.error {
                return ReceiverItem::Error(err);
            } else if queue.buffer_queue.is_empty() && queue.timeout {
                return ReceiverItem::Timeout;
            } else if queue.flushing || queue.shutdown {
                return ReceiverItem::Flushing;
            } else if let Some(buffer) = queue.buffer_queue.pop_front() {
                return ReceiverItem::Buffer(buffer);
            }

            queue = (self.0.queue.0).1.wait(queue).unwrap();
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn connect(
        element: &gst::Element,
        ndi_name: Option<&str>,
        url_address: Option<&str>,
        receiver_ndi_name: &str,
        connect_timeout: u32,
        bandwidth: NDIlib_recv_bandwidth_e,
        color_format: NDIlib_recv_color_format_e,
        timeout: u32,
        max_queue_length: usize,
    ) -> Option<Self> {
        gst::debug!(CAT, obj = element, "Starting NDI connection...");

        assert!(ndi_name.is_some() || url_address.is_some());

        gst::debug!(
            CAT,
            obj = element,
            "Connecting to NDI source with NDI name '{:?}' and URL/Address {:?}",
            ndi_name,
            url_address,
        );

        // FIXME: Ideally we would use NDIlib_recv_color_format_fastest here but that seems to be
        // broken with interlaced content currently
        let recv = RecvInstance::builder(ndi_name, url_address, receiver_ndi_name)
            .bandwidth(bandwidth)
            .color_format(color_format)
            .allow_video_fields(true)
            .build();
        let recv = match recv {
            None => {
                gst::element_error!(
                    element,
                    gst::CoreError::Negotiation,
                    ["Failed to connect to source"]
                );
                return None;
            }
            Some(recv) => recv,
        };

        recv.set_tally(&Tally::default());

        let enable_hw_accel = MetadataFrame::new(0, Some("<ndi_hwaccel enabled=\"true\"/>"));
        recv.send_metadata(&enable_hw_accel);

        // This will set info.audio/video accordingly
        let receiver = Receiver::new(recv, timeout, connect_timeout, max_queue_length, element);

        Some(receiver)
    }

    fn receive_thread(receiver: &Weak<ReceiverInner>, recv: RecvInstance) {
        let mut first_video_frame = true;
        let mut first_audio_frame = true;
        let mut first_frame = true;
        let mut timer = time::Instant::now();

        // Capture until error or shutdown
        loop {
            let Some(receiver) = receiver.upgrade().map(Receiver) else {
                break;
            };

            let Some(element) = receiver.0.element.upgrade() else {
                break;
            };

            let flushing = {
                let queue = (receiver.0.queue.0).0.lock().unwrap();
                if queue.shutdown {
                    gst::debug!(CAT, obj = element, "Shutting down");
                    break;
                }

                // If an error happened in the meantime, just go out of here
                if queue.error.is_some() {
                    gst::error!(CAT, obj = element, "Error while waiting for connection");
                    return;
                }

                queue.flushing
            };

            let timeout = if first_frame {
                receiver.0.connect_timeout
            } else {
                receiver.0.timeout
            };

            let res = match recv.capture(50) {
                _ if flushing => {
                    gst::debug!(CAT, obj = element, "Flushing");
                    Err(gst::FlowError::Flushing)
                }
                Err(_) => {
                    gst::element_error!(
                        element,
                        gst::ResourceError::Read,
                        ["Error receiving frame"]
                    );
                    Err(gst::FlowError::Error)
                }
                Ok(None) if timeout > 0 && timer.elapsed().as_millis() >= timeout as u128 => {
                    gst::debug!(CAT, obj = element, "Timed out -- assuming EOS",);
                    Err(gst::FlowError::Eos)
                }
                Ok(None) => {
                    gst::debug!(CAT, obj = element, "No frame received yet, retry");
                    continue;
                }
                Ok(Some(frame)) => {
                    // If TimestampMode::Clocked is used then directly use the clock time here,
                    // otherwise work with the running time.
                    let receive_time_gst = if let Some(clock) = element.provide_clock() {
                        Some(clock.internal_time())
                    } else if let Some((clock, base_time)) =
                        Option::zip(element.clock(), element.base_time())
                    {
                        Some(clock.time().saturating_sub(base_time))
                    } else {
                        None
                    };

                    if let Some(receive_time_gst) = receive_time_gst {
                        let receive_time_real = (glib::real_time() as u64 * 1000).nseconds();

                        if matches!(frame, Frame::Video(_) | Frame::Audio(_)) {
                            first_frame = false;
                        }

                        match frame {
                            Frame::Video(frame) => {
                                let discont = first_video_frame;
                                first_video_frame = false;

                                gst::debug!(
                                    CAT,
                                    obj = element,
                                    "Received video frame with timecode {} at {} (wallclock: {}): {:?}",
                                    (frame.timecode() as u64 * 100).nseconds(),
                                    receive_time_gst,
                                    receive_time_real,
                                    frame,
                                );

                                Ok(Buffer::Video {
                                    frame,
                                    discont,
                                    receive_time_gst,
                                    receive_time_real,
                                })
                            }
                            Frame::Audio(frame) => {
                                let discont = first_audio_frame;
                                first_audio_frame = false;

                                gst::debug!(
                                    CAT,
                                    obj = element,
                                    "Received audio frame with timecode {} at {} (wallclock: {}): {:?}",
                                    (frame.timecode() as u64 * 100).nseconds(),
                                    receive_time_gst,
                                    receive_time_real,
                                    frame,
                                );

                                Ok(Buffer::Audio {
                                    frame,
                                    discont,
                                    receive_time_gst,
                                    receive_time_real,
                                })
                            }
                            Frame::Metadata(frame) => {
                                gst::debug!(
                                    CAT,
                                    obj = element,
                                    "Received metadata frame with timecode {} at {} (wallclock: {}): {:?}",
                                    (frame.timecode() as u64 * 100).nseconds(),
                                    receive_time_gst,
                                    receive_time_real,
                                    frame,
                                );
                                Ok(Buffer::Metadata {
                                    frame,
                                    receive_time_gst,
                                    receive_time_real,
                                })
                            }
                        }
                    } else {
                        Err(gst::FlowError::Flushing)
                    }
                }
            };

            match res {
                Ok(item) => {
                    let mut queue = (receiver.0.queue.0).0.lock().unwrap();
                    while queue.buffer_queue.len() > receiver.0.max_queue_length {
                        gst::warning!(
                            CAT,
                            obj = element,
                            "Dropping old buffer -- queue has {} items",
                            queue.buffer_queue.len()
                        );
                        queue.buffer_queue.pop_front();
                    }
                    queue.buffer_queue.push_back(item);
                    (receiver.0.queue.0).1.notify_one();
                    timer = time::Instant::now();
                }
                Err(gst::FlowError::Eos) => {
                    gst::debug!(CAT, obj = element, "Signalling EOS");
                    let mut queue = (receiver.0.queue.0).0.lock().unwrap();
                    queue.timeout = true;
                    (receiver.0.queue.0).1.notify_one();
                    break;
                }
                Err(gst::FlowError::Flushing) => {
                    // Flushing, nothing to be done here except for emptying our queue
                    let mut queue = (receiver.0.queue.0).0.lock().unwrap();
                    queue.buffer_queue.clear();
                    (receiver.0.queue.0).1.notify_one();
                    timer = time::Instant::now();
                    first_frame = true;
                    first_audio_frame = true;
                    first_video_frame = true;
                }
                Err(err) => {
                    gst::error!(CAT, obj = element, "Signalling error");
                    let mut queue = (receiver.0.queue.0).0.lock().unwrap();
                    if queue.error.is_none() {
                        queue.error = Some(err);
                    }
                    (receiver.0.queue.0).1.notify_one();
                    break;
                }
            }
        }
    }
}
