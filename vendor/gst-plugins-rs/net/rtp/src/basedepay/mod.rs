//
// Copyright (C) 2023 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::ops::{Range, RangeBounds, RangeInclusive};

use gst::{glib, prelude::*, subclass::prelude::*};

pub mod imp;

glib::wrapper! {
    pub struct RtpBaseDepay2(ObjectSubclass<imp::RtpBaseDepay2>)
        @extends gst::Element, gst::Object;
}

/// Trait containing extension methods for `RtpBaseDepay2`.
pub trait RtpBaseDepay2Ext: IsA<RtpBaseDepay2> + 'static {
    /// Sends a caps event with the given caps downstream before the next output buffer.
    fn set_src_caps(&self, src_caps: &gst::Caps) {
        assert!(src_caps.is_fixed());
        self.upcast_ref::<RtpBaseDepay2>()
            .imp()
            .set_src_caps(src_caps);
    }

    /// Drop the packets of the given packet range.
    ///
    /// This should be called when packets are dropped because they either don't make up any output
    /// buffer or because they're corrupted in some way.
    ///
    /// All pending packets up to the end of the range are dropped, i.e. the start of the range is
    /// irrelevant.
    ///
    /// It is not necessary to call this as part of `drain()` or `flush()` as all still pending packets are
    /// considered dropped afterwards.
    ///
    /// The next buffer that is finished will automatically get the `DISCONT` flag set.
    // FIXME: Allow subclasses to drop explicit packet ranges while keeping older packets around to
    // allow for continuing to reconstruct a frame despite broken packets in the middle.
    fn drop_packets(&self, ext_seqnum: impl RangeBounds<u64>) {
        self.upcast_ref::<RtpBaseDepay2>()
            .imp()
            .drop_packets(ext_seqnum)
    }

    /// Drop a single packet
    ///
    /// Convenience wrapper for calling drop_packets() for a single packet.
    fn drop_packet(&self, packet: &Packet) {
        self.drop_packets(packet.ext_seqnum()..=packet.ext_seqnum())
    }

    /// Queue a buffer made from a given range of packet seqnums and timestamp offsets.
    ///
    /// All buffers that are queued during one call of `handle_packet()` are collected in a
    /// single buffer list and forwarded once `handle_packet()` has returned with a successful flow
    /// return or `finish_pending_buffers()` was called.
    ///
    /// All pending packets for which buffers were queued are released once `handle_packets()`
    /// returned except for the last one. This means that it is possible for subclasses to queue a
    /// buffer and output a remaining chunk of that buffer together with data from the next buffer.
    ///
    /// If passing `OutOfBand` then the buffer is assumed to be produced using some other data,
    /// e.g. from the caps, and not associated with any packets. In that case it will be pushed
    /// right before the next buffer with the timestamp of that buffer, or at EOS with the
    /// timestamp of the previous buffer.
    ///
    /// In all other cases a seqnum range is provided and needs to be valid.
    ///
    /// If the seqnum range doesn't start with the first pending packet then all packets up to the
    /// first one given in the range are considered dropped.
    ///
    /// Together with the seqnum range it is possible to provide a timestamp offset relative to
    /// which the outgoing buffers's timestamp should be set.
    ///
    /// The timestamp offset is provided in nanoseconds relative to the PTS of the packet that the
    /// first seqnum refers to. This mode is mostly useful for subclasses that consume a packet
    /// with multiple frames and send out one buffer per frame.
    ///
    /// Both a PTS and DTS offset can be provided. If no DTS offset is provided then no DTS will be
    /// set at all. If no PTS offset is provided then the first buffer for a given start seqnum
    /// will get the PTS of the corresponding packet and all following buffers that start with the
    /// same seqnum will get no PTS set.
    ///
    /// Note that the DTS offset is relative to the final PTS and as such should not include the
    /// PTS offset.
    fn queue_buffer(
        &self,
        packet_to_buffer_relation: PacketToBufferRelation,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        self.upcast_ref::<RtpBaseDepay2>()
            .imp()
            .queue_buffer(packet_to_buffer_relation, buffer)
    }

    /// Finish currently pending buffers and push them downstream in a single buffer list.
    #[allow(unused)]
    fn finish_pending_buffers(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        self.upcast_ref::<RtpBaseDepay2>()
            .imp()
            .finish_pending_buffers()
    }

    /// Returns a reference to the sink pad.
    fn sink_pad(&self) -> &gst::Pad {
        self.upcast_ref::<RtpBaseDepay2>().imp().sink_pad()
    }

    /// Returns a reference to the src pad.
    fn src_pad(&self) -> &gst::Pad {
        self.upcast_ref::<RtpBaseDepay2>().imp().src_pad()
    }
}

impl<O: IsA<RtpBaseDepay2>> RtpBaseDepay2Ext for O {}

/// Trait to implement in `RtpBaseDepay2` subclasses.
pub trait RtpBaseDepay2Impl: ElementImpl + ObjectSubclass<Type: IsA<RtpBaseDepay2>> {
    /// By default only metas without any tags are copied. Adding tags here will also copy the
    /// metas that *only* have exactly one of these tags.
    ///
    /// If more complex copying of metas is needed then [`RtpBaseDepay2Impl::transform_meta`] has
    /// to be implemented.
    const ALLOWED_META_TAGS: &'static [&'static str] = &[];

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
    /// If draining is necessary because of the caps change then the subclass will have to do that.
    ///
    /// Optional, by default does nothing.
    fn set_sink_caps(&self, caps: &gst::Caps) -> bool {
        self.parent_set_sink_caps(caps)
    }

    /// Called whenever a new packet is available.
    fn handle_packet(&self, packet: &Packet) -> Result<gst::FlowSuccess, gst::FlowError> {
        self.parent_handle_packet(packet)
    }

    /// Called whenever a discontinuity or EOS is observed.
    ///
    /// The subclass should output any pending buffers it can output at this point.
    ///
    /// This will be followed by a call to [`Self::flush`].
    ///
    /// Optional, by default drops all still pending packets and forwards all still pending buffers
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

/// Trait containing extension methods for `RtpBaseDepay2Impl`, specifically methods for chaining
/// up to the parent implementation of virtual methods.
pub trait RtpBaseDepay2ImplExt: RtpBaseDepay2Impl {
    fn parent_set_sink_caps(&self, caps: &gst::Caps) -> bool {
        unsafe {
            let data = Self::type_data();
            let parent_class = &*(data.as_ref().parent_class() as *mut Class);
            (parent_class.set_sink_caps)(self.obj().unsafe_cast_ref(), caps)
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

    fn parent_handle_packet(&self, packet: &Packet) -> Result<gst::FlowSuccess, gst::FlowError> {
        unsafe {
            let data = Self::type_data();
            let parent_class = &*(data.as_ref().parent_class() as *mut Class);
            (parent_class.handle_packet)(self.obj().unsafe_cast_ref(), packet)
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

impl<T: RtpBaseDepay2Impl> RtpBaseDepay2ImplExt for T {}

#[derive(Debug)]
pub struct Packet {
    buffer: gst::MappedBuffer<gst::buffer::Readable>,

    discont: bool,
    ext_seqnum: u64,
    ext_timestamp: u64,
    marker: bool,
    payload_range: Range<usize>,
}

impl Packet {
    pub fn ext_seqnum(&self) -> u64 {
        self.ext_seqnum
    }

    pub fn ext_timestamp(&self) -> u64 {
        self.ext_timestamp
    }

    pub fn marker_bit(&self) -> bool {
        self.marker
    }

    pub fn discont(&self) -> bool {
        self.discont
    }

    pub fn pts(&self) -> Option<gst::ClockTime> {
        self.buffer.buffer().pts()
    }

    pub fn payload(&self) -> &[u8] {
        &self.buffer[self.payload_range.clone()]
    }

    pub fn payload_buffer(&self) -> gst::Buffer {
        self.buffer
            .buffer()
            .copy_region(
                gst::BufferCopyFlags::MEMORY,
                self.payload_range.start..self.payload_range.end,
            )
            .expect("Failed copying buffer")
    }

    /// Note: This function will panic if the range is out of bounds.
    pub fn payload_subbuffer(&self, range: impl RangeBounds<usize>) -> gst::Buffer {
        let range_start = match range.start_bound() {
            std::ops::Bound::Included(&start) => start,
            std::ops::Bound::Excluded(&start) => start.checked_add(1).unwrap(),
            std::ops::Bound::Unbounded => 0,
        }
        .checked_add(self.payload_range.start)
        .unwrap();

        let range_end = match range.end_bound() {
            std::ops::Bound::Included(&range_end) => range_end
                .checked_add(self.payload_range.start)
                .and_then(|v| v.checked_add(1))
                .unwrap(),
            std::ops::Bound::Excluded(&range_end) => {
                range_end.checked_add(self.payload_range.start).unwrap()
            }
            std::ops::Bound::Unbounded => self.payload_range.end,
        };

        self.buffer
            .buffer()
            .copy_region(gst::BufferCopyFlags::MEMORY, range_start..range_end)
            .expect("Failed to create subbuffer")
    }

    /// Note: For offset with unspecified length just use `payload_subbuffer(off..)`.
    /// Note: This function will panic if the offset or length are out of bounds.
    pub fn payload_subbuffer_from_offset_with_length(
        &self,
        start: usize,
        length: usize,
    ) -> gst::Buffer {
        self.payload_subbuffer(start..start + length)
    }
}

/// Class struct for `RtpBaseDepay2`.
#[repr(C)]
pub struct Class {
    parent: gst::ffi::GstElementClass,

    start: fn(&RtpBaseDepay2) -> Result<(), gst::ErrorMessage>,
    stop: fn(&RtpBaseDepay2) -> Result<(), gst::ErrorMessage>,

    set_sink_caps: fn(&RtpBaseDepay2, caps: &gst::Caps) -> bool,
    handle_packet: fn(&RtpBaseDepay2, packet: &Packet) -> Result<gst::FlowSuccess, gst::FlowError>,
    drain: fn(&RtpBaseDepay2) -> Result<gst::FlowSuccess, gst::FlowError>,
    flush: fn(&RtpBaseDepay2),

    sink_event: fn(&RtpBaseDepay2, event: gst::Event) -> Result<gst::FlowSuccess, gst::FlowError>,
    src_event: fn(&RtpBaseDepay2, event: gst::Event) -> Result<gst::FlowSuccess, gst::FlowError>,

    sink_query: fn(&RtpBaseDepay2, query: &mut gst::QueryRef) -> bool,
    src_query: fn(&RtpBaseDepay2, query: &mut gst::QueryRef) -> bool,

    transform_meta: fn(
        &RtpBaseDepay2,
        in_buf: &gst::BufferRef,
        meta: &gst::MetaRef<gst::Meta>,
        out_buf: &mut gst::BufferRef,
    ),

    allowed_meta_tags: &'static [&'static str],
}

unsafe impl ClassStruct for Class {
    type Type = imp::RtpBaseDepay2;
}

impl std::ops::Deref for Class {
    type Target = glib::Class<<<Self as ClassStruct>::Type as ObjectSubclass>::ParentType>;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(&self.parent as *const _ as *const _) }
    }
}

unsafe impl<T: RtpBaseDepay2Impl> IsSubclassable<T> for RtpBaseDepay2 {
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

        class.handle_packet = |obj, packet| unsafe {
            let imp = obj.unsafe_cast_ref::<T::Type>().imp();
            imp.handle_packet(packet)
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
    }
}

/// Timestamp offset between this buffer and the reference packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum TimestampOffset {
    /// Offset in nanoseconds relative to the first seqnum this buffer belongs to.
    Pts(gst::Signed<gst::ClockTime>),
    /// Offset in nanoseconds relative to the first seqnum this buffer belongs to.
    PtsAndDts(gst::Signed<gst::ClockTime>, gst::Signed<gst::ClockTime>),
}

/// Relation between queued buffer and input packet seqnums.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum PacketToBufferRelation {
    Seqnums(RangeInclusive<u64>),
    SeqnumsWithOffset {
        seqnums: RangeInclusive<u64>,
        timestamp_offset: TimestampOffset,
    },
    OutOfBand,
}

impl<'a> From<&'a Packet> for PacketToBufferRelation {
    fn from(packet: &'a Packet) -> Self {
        PacketToBufferRelation::Seqnums(packet.ext_seqnum()..=packet.ext_seqnum())
    }
}

impl From<u64> for PacketToBufferRelation {
    fn from(ext_seqnum: u64) -> Self {
        PacketToBufferRelation::Seqnums(ext_seqnum..=ext_seqnum)
    }
}
