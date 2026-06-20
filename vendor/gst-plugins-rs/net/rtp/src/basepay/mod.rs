//
// Copyright (C) 2023 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::ops::{RangeBounds, RangeInclusive};

use gst::{glib, prelude::*, subclass::prelude::*};

pub mod imp;

glib::wrapper! {
    pub struct RtpBasePay2(ObjectSubclass<imp::RtpBasePay2>)
        @extends gst::Element, gst::Object;
}

/// Trait containing extension methods for `RtpBasePay2`.
pub trait RtpBasePay2Ext: IsA<RtpBasePay2> + 'static {
    /// Sends a caps event with the given caps downstream before the next output buffer.
    ///
    /// The caps must be `application/x-rtp` and contain the `clock-rate` field with a suitable
    /// clock-rate for this stream.
    ///
    /// The caps can be unfixed and will be passed through `RtpBasePay2Impl::negotiate()` to
    /// negotiate caps with downstream, and finally fixate them.
    fn set_src_caps(&self, src_caps: &gst::Caps) {
        self.upcast_ref::<RtpBasePay2>()
            .imp()
            .set_src_caps(src_caps);
    }

    /// Drop the buffers from the given buffer range.
    ///
    /// This should be called when input buffers are dropped because they are not included in any
    /// output packet.
    ///
    /// All pending buffers up to the end of the range are dropped, i.e. the start of the range is
    /// irrelevant.
    fn drop_buffers(&self, ids: impl RangeBounds<u64>) {
        self.upcast_ref::<RtpBasePay2>().imp().drop_buffers(ids)
    }

    /// Queue an RTP packet made from a given range of input buffer ids and timestamp offset.
    ///
    /// All packets that are queued during one call of `handle_buffer()` are collected in a
    /// single buffer list and forwarded once `handle_buffer()` has returned with a successful flow
    /// return or `finish_pending_packets()` was called.
    ///
    /// All pending buffers for which packets were queued are released once `handle_buffer()`
    /// returned except for the last one. This means that it is possible for subclasses to queue a
    /// buffer and output a remaining chunk of that buffer together with data from the next buffer.
    ///
    /// If passing `OutOfBand` then the packet is assumed to be produced using some other data,
    /// e.g. from the caps, and not associated with any packets. In that case it will be pushed
    /// right before the next packet with the timestamp of that packet, or at EOS with the
    /// timestamp of the previous packet.
    ///
    /// In all other cases a buffer id range is provided and needs to be valid.
    ///
    /// If the id range doesn't start with the first pending buffer then all buffers up to the
    /// first one given in the range are considered dropped.
    ///
    /// Together with the buffer ids it is possible to provide a timestamp offset relative to which
    /// the outgoing RTP timestamp and GStreamer PTS should be set.
    ///
    /// The timestamp offset can be provided in two ways:
    ///
    ///   * Nanoseconds relative to the PTS of the buffer that the first id refers to. This mode is
    ///     mostly useful for subclasses that consume a buffer with multiple frames and send out
    ///     one packet per frame.
    ///
    ///   * RTP clock-rate units (without wrap-arounds) relative to the last buffer that had no
    ///     timestamp offset given in RTP clock-rate units. In this mode the subclass needs to be
    ///     careful to handle discontinuities of any sort correctly. Also, the subclass needs to
    ///     provide an explicit timestamp (either by setting no offset or by setting a PTS-based
    ///     offset) for the first packet ever and after every `drain()` or `flush()`.
    fn queue_packet(
        &self,
        packet_to_buffer_relation: PacketToBufferRelation,
        packet: rtp_types::RtpPacketBuilder<&[u8], &[u8]>,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        self.upcast_ref::<RtpBasePay2>()
            .imp()
            .queue_packet(packet_to_buffer_relation, packet)
    }

    /// Returns the sequence number of the next packet.
    fn next_seqnum(&self) -> u16 {
        self.upcast_ref::<RtpBasePay2>().imp().next_seqnum()
    }

    /// Finish currently pending packets and push them downstream in a single buffer list.
    fn finish_pending_packets(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        self.upcast_ref::<RtpBasePay2>()
            .imp()
            .finish_pending_packets()
    }

    /// Returns a reference to the sink pad.
    fn sink_pad(&self) -> &gst::Pad {
        self.upcast_ref::<RtpBasePay2>().imp().sink_pad()
    }

    /// Returns a reference to the src pad.
    #[allow(unused)]
    fn src_pad(&self) -> &gst::Pad {
        self.upcast_ref::<RtpBasePay2>().imp().src_pad()
    }

    /// Returns the currently configured MTU.
    fn mtu(&self) -> u32 {
        self.upcast_ref::<RtpBasePay2>().imp().mtu()
    }

    /// Returns the maximum available payload size.
    fn max_payload_size(&self) -> u32 {
        self.upcast_ref::<RtpBasePay2>().imp().max_payload_size()
    }
}

impl<O: IsA<RtpBasePay2>> RtpBasePay2Ext for O {}

/// Trait to implement in `RtpBasePay2` subclasses.
pub trait RtpBasePay2Impl: ElementImpl + ObjectSubclass<Type: IsA<RtpBasePay2>> {
    /// Drop buffers with `HEADER` flag.
    const DROP_HEADER_BUFFERS: bool = false;

    /// By default only metas without any tags are copied. Adding tags here will also copy the
    /// metas that *only* have exactly one of these tags.
    ///
    /// If more complex copying of metas is needed then [`RtpBasePay2Impl::transform_meta`] has
    /// to be implemented.
    const ALLOWED_META_TAGS: &'static [&'static str] = &[];

    /// Default payload type for this subclass.
    const DEFAULT_PT: u8 = 96;

    /// Called when streaming starts (READY -> PAUSED state change)
    ///
    /// Optional, can be used to initialise streaming state.
    fn start(&self) -> Result<(), gst::ErrorMessage> {
        self.parent_start()
    }

    /// Called after streaming has stopped (PAUSED -> READY state change)
    ///
    /// Optional, can be used to clean up streaming state.
    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        self.parent_stop()
    }

    /// Called when new caps are received on the sink pad.
    ///
    /// Can be used to configure the caps on the src pad or to configure caps-specific state.
    ///
    /// Optional, by default does nothing.
    fn set_sink_caps(&self, caps: &gst::Caps) -> bool {
        self.parent_set_sink_caps(caps)
    }

    /// Called when new caps are configured on the source pad and whenever renegotiation has to happen.
    ///
    /// The `src_caps` are the caps passed into `set_src_caps()` before, intersected with the
    /// supported caps by the peer, and will have to be fixated.
    ///
    /// Optional, by default sets the `payload` (pt) and `ssrc` fields, and negotiates RTP header
    /// extensions with downstream, and finally fixates the caps and configures them on the source
    /// pad.
    fn negotiate(&self, src_caps: gst::Caps) {
        assert!(src_caps.is_writable());
        self.parent_negotiate(src_caps);
    }

    /// Called whenever a new buffer is available.
    fn handle_buffer(
        &self,
        buffer: &gst::Buffer,
        id: u64,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        self.parent_handle_buffer(buffer, id)
    }

    /// Called whenever a discontinuity or EOS is observed.
    ///
    /// The subclass should output any pending buffers it can output at this point.
    ///
    /// This will be followed by a call to [`Self::flush`].
    ///
    /// Optional, by default drops all still pending buffers and forwards all still pending packets
    /// with the last known timestamp.
    fn drain(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        self.parent_drain()
    }

    /// Called on `FlushStop` or whenever all pending data should simply be discarded.
    ///
    /// The subclass should reset its internal state as necessary.
    ///
    /// Optional.
    fn flush(&self) {
        self.parent_flush()
    }

    /// Called whenever a new event arrives on the sink pad.
    ///
    /// Optional, by default does the standard event handling of the base class.
    fn sink_event(&self, event: gst::Event) -> Result<gst::FlowSuccess, gst::FlowError> {
        self.parent_sink_event(event)
    }

    /// Called whenever a new event arrives on the src pad.
    ///
    /// Optional, by default does the standard event handling of the base class.
    fn src_event(&self, event: gst::Event) -> Result<gst::FlowSuccess, gst::FlowError> {
        self.parent_src_event(event)
    }

    /// Called whenever a new query arrives on the sink pad.
    ///
    /// Optional, by default does the standard query handling of the base class.
    fn sink_query(&self, query: &mut gst::QueryRef) -> bool {
        self.parent_sink_query(query)
    }

    /// Called whenever a new query arrives on the src pad.
    ///
    /// Optional, by default does the standard query handling of the base class.
    fn src_query(&self, query: &mut gst::QueryRef) -> bool {
        self.parent_src_query(query)
    }

    /// Called whenever a meta from an input buffer has to be copied to the output buffer.
    ///
    /// Optional, by default simply copies over all metas.
    fn transform_meta(
        &self,
        in_buf: &gst::BufferRef,
        meta: &gst::MetaRef<gst::Meta>,
        out_buf: &mut gst::BufferRef,
    ) {
        self.parent_transform_meta(in_buf, meta, out_buf);
    }
}

/// Trait containing extension methods for `RtpBasePay2Impl`, specifically methods for chaining
/// up to the parent implementation of virtual methods.
pub trait RtpBasePay2ImplExt: RtpBasePay2Impl {
    fn parent_set_sink_caps(&self, caps: &gst::Caps) -> bool {
        unsafe {
            let data = Self::type_data();
            let parent_class = &*(data.as_ref().parent_class() as *mut Class);
            (parent_class.set_sink_caps)(self.obj().unsafe_cast_ref(), caps)
        }
    }

    fn parent_negotiate(&self, src_caps: gst::Caps) {
        assert!(src_caps.is_writable());
        unsafe {
            let data = Self::type_data();
            let parent_class = &*(data.as_ref().parent_class() as *mut Class);
            (parent_class.negotiate)(self.obj().unsafe_cast_ref(), src_caps);
        }
    }

    fn parent_start(&self) -> Result<(), gst::ErrorMessage> {
        unsafe {
            let data = Self::type_data();
            let parent_class = &*(data.as_ref().parent_class() as *mut Class);
            (parent_class.start)(self.obj().unsafe_cast_ref())
        }
    }

    fn parent_stop(&self) -> Result<(), gst::ErrorMessage> {
        unsafe {
            let data = Self::type_data();
            let parent_class = &*(data.as_ref().parent_class() as *mut Class);
            (parent_class.stop)(self.obj().unsafe_cast_ref())
        }
    }

    fn parent_handle_buffer(
        &self,
        buffer: &gst::Buffer,
        id: u64,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        unsafe {
            let data = Self::type_data();
            let parent_class = &*(data.as_ref().parent_class() as *mut Class);
            (parent_class.handle_buffer)(self.obj().unsafe_cast_ref(), buffer, id)
        }
    }

    fn parent_drain(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        unsafe {
            let data = Self::type_data();
            let parent_class = &*(data.as_ref().parent_class() as *mut Class);
            (parent_class.drain)(self.obj().unsafe_cast_ref())
        }
    }

    fn parent_flush(&self) {
        unsafe {
            let data = Self::type_data();
            let parent_class = &*(data.as_ref().parent_class() as *mut Class);
            (parent_class.flush)(self.obj().unsafe_cast_ref());
        }
    }

    fn parent_sink_event(&self, event: gst::Event) -> Result<gst::FlowSuccess, gst::FlowError> {
        unsafe {
            let data = Self::type_data();
            let parent_class = &*(data.as_ref().parent_class() as *mut Class);
            (parent_class.sink_event)(self.obj().unsafe_cast_ref(), event)
        }
    }

    fn parent_src_event(&self, event: gst::Event) -> Result<gst::FlowSuccess, gst::FlowError> {
        unsafe {
            let data = Self::type_data();
            let parent_class = &*(data.as_ref().parent_class() as *mut Class);
            (parent_class.src_event)(self.obj().unsafe_cast_ref(), event)
        }
    }

    fn parent_sink_query(&self, query: &mut gst::QueryRef) -> bool {
        unsafe {
            let data = Self::type_data();
            let parent_class = &*(data.as_ref().parent_class() as *mut Class);
            (parent_class.sink_query)(self.obj().unsafe_cast_ref(), query)
        }
    }

    fn parent_src_query(&self, query: &mut gst::QueryRef) -> bool {
        unsafe {
            let data = Self::type_data();
            let parent_class = &*(data.as_ref().parent_class() as *mut Class);
            (parent_class.src_query)(self.obj().unsafe_cast_ref(), query)
        }
    }

    fn parent_transform_meta(
        &self,
        in_buf: &gst::BufferRef,
        meta: &gst::MetaRef<gst::Meta>,
        out_buf: &mut gst::BufferRef,
    ) {
        unsafe {
            let data = Self::type_data();
            let parent_class = &*(data.as_ref().parent_class() as *mut Class);
            (parent_class.transform_meta)(self.obj().unsafe_cast_ref(), in_buf, meta, out_buf)
        }
    }
}

impl<T: RtpBasePay2Impl> RtpBasePay2ImplExt for T {}

/// Class struct for `RtpBasePay2`.
#[repr(C)]
pub struct Class {
    parent: gst::ffi::GstElementClass,

    start: fn(&RtpBasePay2) -> Result<(), gst::ErrorMessage>,
    stop: fn(&RtpBasePay2) -> Result<(), gst::ErrorMessage>,

    set_sink_caps: fn(&RtpBasePay2, caps: &gst::Caps) -> bool,
    negotiate: fn(&RtpBasePay2, src_caps: gst::Caps),
    handle_buffer:
        fn(&RtpBasePay2, buffer: &gst::Buffer, id: u64) -> Result<gst::FlowSuccess, gst::FlowError>,
    drain: fn(&RtpBasePay2) -> Result<gst::FlowSuccess, gst::FlowError>,
    flush: fn(&RtpBasePay2),

    sink_event: fn(&RtpBasePay2, event: gst::Event) -> Result<gst::FlowSuccess, gst::FlowError>,
    src_event: fn(&RtpBasePay2, event: gst::Event) -> Result<gst::FlowSuccess, gst::FlowError>,

    sink_query: fn(&RtpBasePay2, query: &mut gst::QueryRef) -> bool,
    src_query: fn(&RtpBasePay2, query: &mut gst::QueryRef) -> bool,

    transform_meta: fn(
        &RtpBasePay2,
        in_buf: &gst::BufferRef,
        meta: &gst::MetaRef<gst::Meta>,
        out_buf: &mut gst::BufferRef,
    ),

    allowed_meta_tags: &'static [&'static str],
    drop_header_buffers: bool,
    default_pt: u8,
}

unsafe impl ClassStruct for Class {
    type Type = imp::RtpBasePay2;
}

impl std::ops::Deref for Class {
    type Target = glib::Class<<<Self as ClassStruct>::Type as ObjectSubclass>::ParentType>;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(&self.parent as *const _ as *const _) }
    }
}

unsafe impl<T: RtpBasePay2Impl> IsSubclassable<T> for RtpBasePay2 {
    fn class_init(class: &mut glib::Class<Self>) {
        Self::parent_class_init::<T>(class);

        let class = class.as_mut();

        class.start = |obj| unsafe {
            let imp = obj.unsafe_cast_ref::<T::Type>().imp();
            imp.start()
        };

        class.stop = |obj| unsafe {
            let imp = obj.unsafe_cast_ref::<T::Type>().imp();
            imp.stop()
        };

        class.set_sink_caps = |obj, caps| unsafe {
            let imp = obj.unsafe_cast_ref::<T::Type>().imp();
            imp.set_sink_caps(caps)
        };

        class.negotiate = |obj, src_caps| unsafe {
            let imp = obj.unsafe_cast_ref::<T::Type>().imp();
            imp.negotiate(src_caps)
        };

        class.handle_buffer = |obj, buffer, id| unsafe {
            let imp = obj.unsafe_cast_ref::<T::Type>().imp();
            imp.handle_buffer(buffer, id)
        };

        class.drain = |obj| unsafe {
            let imp = obj.unsafe_cast_ref::<T::Type>().imp();
            imp.drain()
        };

        class.flush = |obj| unsafe {
            let imp = obj.unsafe_cast_ref::<T::Type>().imp();
            imp.flush()
        };

        class.sink_event = |obj, event| unsafe {
            let imp = obj.unsafe_cast_ref::<T::Type>().imp();
            imp.sink_event(event)
        };

        class.src_event = |obj, event| unsafe {
            let imp = obj.unsafe_cast_ref::<T::Type>().imp();
            imp.src_event(event)
        };

        class.sink_query = |obj, query| unsafe {
            let imp = obj.unsafe_cast_ref::<T::Type>().imp();
            imp.sink_query(query)
        };

        class.src_query = |obj, query| unsafe {
            let imp = obj.unsafe_cast_ref::<T::Type>().imp();
            imp.src_query(query)
        };

        class.transform_meta = |obj, in_buf, meta, out_buf| unsafe {
            let imp = obj.unsafe_cast_ref::<T::Type>().imp();
            imp.transform_meta(in_buf, meta, out_buf)
        };

        class.allowed_meta_tags = T::ALLOWED_META_TAGS;
        class.drop_header_buffers = T::DROP_HEADER_BUFFERS;
        class.default_pt = T::DEFAULT_PT;
    }
}

/// Timestamp offset between this packet and the reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum TimestampOffset {
    /// Offset in nanoseconds relative to the first buffer id this packet belongs to.
    Pts(gst::ClockTime),
    /// Offset in RTP clock-time units relative to the last packet that had offset given in RTP
    /// clock-rate units.
    Rtp(u64),
}

/// Relation between queued packet and input buffer ids.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum PacketToBufferRelation {
    Ids(RangeInclusive<u64>),
    IdsWithOffset {
        ids: RangeInclusive<u64>,
        timestamp_offset: TimestampOffset,
    },
    OutOfBand,
}

impl From<u64> for PacketToBufferRelation {
    fn from(id: u64) -> Self {
        PacketToBufferRelation::Ids(id..=id)
    }
}
