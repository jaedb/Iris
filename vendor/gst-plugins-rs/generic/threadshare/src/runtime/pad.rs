// Copyright (C) 2019-2022 François Laignel <fengalin@free.fr>
// Copyright (C) 2020 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

//! An implementation of `Pad`s to run asynchronous processings.
//!
//! [`PadSink`] & [`PadSrc`] provide an asynchronous API to ease the development of `Element`s in
//! the `threadshare` GStreamer plugins framework.
//!
//! The diagram below shows how the [`PadSrc`] & [`PadSink`] and the related `struct`s integrate in
//! `ts` `Element`s.
//!
//! Note: [`PadSrc`] & [`PadSink`] only support `gst::PadMode::Push` at the moment.
//!
//! ```text
//!    ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓          ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//!                    Element A               ┃          ┃          Element B
//!                                            ┃          ┃
//!                 ╭─────────────────╮        ┃          ┃       ╭──────────────────╮
//!                 │     PadSrc      │        ┃          ┃       │      PadSink     │
//!                 │     Handler     │        ┃          ┃       │      Handler     │
//!                 │─────────────────│        ┃          ┃       │──────────────────│
//!                 │ - src_activate* │     ╭──┸──╮    ╭──┸──╮    │ - sink_activate* │
//!                 │ - src_event*    │<────│     │<╌╌╌│     │───>│ - sink_chain*    │
//!                 │ - src_query     │<────│ gst │    │ gst │───>│ - sink_event*    │
//!                 ╰─────────────────╯     │     │    │     │───>│ - sink_query     │
//!                          │              │ Pad │    │ Pad │    ╰──────────────────╯
//!                          │           ╭─>│     │╌╌╌>│     │─╮            │
//!                          │           │  ╰──┰──╯    ╰──┰──╯ ╰───────╮    │
//!      ╭─────────╮    ╭────────╮ push* │     ┃          ┃          ╭─────────╮
//!      │ Task ↺  │───>│ PadSrc │───────╯     ┃          ┃          │ PadSink │
//!      ╰─────────╯    ╰────────╯             ┃          ┃          ╰─────────╯
//!    ━━━━━━━│━━━━━━━━━━━━━━│━━━━━━━━━━━━━━━━━┛          ┗━━━━━━━━━━━━━━━│━━━━━━━━━━━━
//!           ╰──────────────┴───────────────────╮      ╭─────────────────╯
//!                                           ╭────────────╮
//!                                           │  Context ↺ │
//!                                           ╰────────────╯
//! ```
//!
//! Asynchronous operations for both [`PadSrc`] in `Element A` and [`PadSink`] in `Element B` run on
//!  the same [`Context`], which can also be shared by other `Element`s or instances of the same
//! `Element`s in multiple `Pipeline`s.
//!
//! `Element A` & `Element B` can also be linked to non-threadshare `Element`s in which case, they
//! operate in a regular synchronous way.
//!
//! Note that only operations on the streaming thread (serialized events, buffers, serialized
//! queries) are handled from the `PadContext` and asynchronously, everything else operates
//! blocking.
//!
//! [`PadSink`]: struct.PadSink.html
//! [`PadSrc`]: struct.PadSrc.html
//! [`Context`]: ../executor/struct.Context.html

use futures::future;
use futures::prelude::*;

use gst::prelude::*;
use gst::subclass::prelude::*;
use gst::{FlowError, FlowSuccess};

use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::{Arc, Weak};

use super::executor::{self, Context};
use super::RUNTIME_CAT;

#[inline]
fn event_ret_to_event_full_res(
    ret: bool,
    event_type: gst::EventType,
) -> Result<FlowSuccess, FlowError> {
    if ret {
        Ok(FlowSuccess::Ok)
    } else if event_type == gst::EventType::Caps {
        Err(FlowError::NotNegotiated)
    } else {
        Err(FlowError::Error)
    }
}

#[inline]
fn event_to_event_full(ret: bool, event_type: gst::EventType) -> Result<FlowSuccess, FlowError> {
    event_ret_to_event_full_res(ret, event_type)
}

#[inline]
fn event_to_event_full_serialized(
    ret: impl Future<Output = bool> + Send,
    event_type: gst::EventType,
) -> impl Future<Output = Result<FlowSuccess, FlowError>> + Send {
    ret.map(move |ret| event_ret_to_event_full_res(ret, event_type))
}

/// A trait to define `handler`s for [`PadSrc`] callbacks.
///
/// *See the [`pad` module] documentation for a description of the model.*
///
/// [`PadSrc`]: struct.PadSrc.html
/// [`pad` module]: index.html
pub trait PadSrcHandler: Clone + Send + Sync + 'static {
    type ElementImpl: ElementImpl<Type: Send>;

    fn src_activate(
        self,
        pad: &gst::Pad,
        _imp: &Self::ElementImpl,
    ) -> Result<(), gst::LoggableError> {
        if pad.is_active() {
            gst::debug!(
                RUNTIME_CAT,
                obj = pad,
                "Already activated in {:?} mode ",
                pad.mode()
            );
            return Ok(());
        }

        pad.activate_mode(gst::PadMode::Push, true).map_err(|err| {
            gst::error!(
                RUNTIME_CAT,
                obj = pad,
                "Error in PadSrc activate: {:?}",
                err
            );
            gst::loggable_error!(RUNTIME_CAT, "Error in PadSrc activate: {:?}", err)
        })
    }

    fn src_activatemode(
        self,
        _pad: &gst::Pad,
        _imp: &Self::ElementImpl,
        _mode: gst::PadMode,
        _active: bool,
    ) -> Result<(), gst::LoggableError> {
        Ok(())
    }

    fn src_event(self, pad: &gst::Pad, imp: &Self::ElementImpl, event: gst::Event) -> bool {
        gst::log!(RUNTIME_CAT, obj = pad, "Handling {:?}", event);
        gst::Pad::event_default(pad, Some(imp.obj().as_ref()), event)
    }

    fn src_event_full(
        self,
        pad: &gst::Pad,
        imp: &Self::ElementImpl,
        event: gst::Event,
    ) -> Result<FlowSuccess, FlowError> {
        // default is to dispatch to `src_event`
        // (as implemented in `gst_pad_send_event_unchecked`)
        let event_type = event.type_();
        event_to_event_full(self.src_event(pad, imp, event), event_type)
    }

    fn src_query(self, pad: &gst::Pad, imp: &Self::ElementImpl, query: &mut gst::QueryRef) -> bool {
        gst::log!(RUNTIME_CAT, obj = pad, "Handling {:?}", query);
        if query.is_serialized() {
            // FIXME serialized queries should be handled with the dataflow
            // but we can't return a `Future` because we couldn't honor QueryRef's lifetime
            false
        } else {
            gst::log!(RUNTIME_CAT, obj = pad, "Handling {:?}", query);
            gst::Pad::query_default(pad, Some(imp.obj().as_ref()), query)
        }
    }
}

#[derive(Debug)]
pub struct PadSrcInner {
    gst_pad: gst::Pad,
}

impl PadSrcInner {
    fn new(gst_pad: gst::Pad) -> Self {
        if gst_pad.direction() != gst::PadDirection::Src {
            panic!("Wrong pad direction for PadSrc");
        }

        PadSrcInner { gst_pad }
    }

    pub fn gst_pad(&self) -> &gst::Pad {
        &self.gst_pad
    }

    pub async fn push(&self, buffer: gst::Buffer) -> Result<FlowSuccess, FlowError> {
        gst::log!(RUNTIME_CAT, obj = self.gst_pad, "Pushing {buffer:?}");

        let success = self.gst_pad.push(buffer).inspect_err(|&err| {
            if err == gst::FlowError::Flushing {
                gst::debug!(
                    RUNTIME_CAT,
                    obj = self.gst_pad,
                    "Failed to push Buffer to PadSrc: Flushing"
                );
            } else {
                gst::error!(
                    RUNTIME_CAT,
                    obj = self.gst_pad,
                    "Failed to push Buffer to PadSrc: {err:?}"
                );
            }
        })?;

        gst::log!(
            RUNTIME_CAT,
            obj = self.gst_pad,
            "Processing any pending sub tasks"
        );
        Context::drain_sub_tasks().await?;

        Ok(success)
    }

    pub async fn push_list(&self, list: gst::BufferList) -> Result<FlowSuccess, FlowError> {
        gst::log!(RUNTIME_CAT, obj = self.gst_pad, "Pushing {:?}", list);

        let success = self.gst_pad.push_list(list).inspect_err(|&err| {
            if err == gst::FlowError::Flushing {
                gst::debug!(
                    RUNTIME_CAT,
                    obj = self.gst_pad,
                    "Failed to push BufferList to PadSrc: Flushing"
                );
            } else {
                gst::error!(
                    RUNTIME_CAT,
                    obj = self.gst_pad,
                    "Failed to push BufferList to PadSrc: {err:?}"
                );
            }
        })?;

        gst::log!(
            RUNTIME_CAT,
            obj = self.gst_pad,
            "Processing any pending sub tasks"
        );
        Context::drain_sub_tasks().await?;

        Ok(success)
    }

    pub async fn push_event(&self, event: gst::Event) -> bool {
        gst::log!(RUNTIME_CAT, obj = self.gst_pad, "Pushing {:?}", event);

        let was_handled = self.gst_pad.push_event(event);

        gst::log!(
            RUNTIME_CAT,
            obj = self.gst_pad,
            "Processing any pending sub tasks"
        );
        if Context::drain_sub_tasks().await.is_err() {
            return false;
        }

        was_handled
    }
}

/// A [`PadSrc`] which can be moved in [`handler`]s functions and `Future`s.
///
/// Call [`upgrade`] to use the [`PadSrc`].
///
/// *See the [`pad` module] documentation for a description of the model.*
///
/// [`PadSrc`]: struct.PadSrc.html
/// [`handler`]: trait.PadSrcHandler.html
/// [`upgrade`]: struct.PadSrcWeak.html#method.upgrade
/// [`pad` module]: index.html
#[derive(Clone, Debug)]
pub struct PadSrcWeak(Weak<PadSrcInner>);

impl PadSrcWeak {
    pub fn upgrade(&self) -> Option<PadSrcRef<'_>> {
        self.0.upgrade().map(PadSrcRef::new)
    }
}

/// A [`PadSrc`] to be used in `Handler`s functions and `Future`s.
///
/// Call [`downgrade`] if you need to `clone` the [`PadSrc`].
///
/// *See the [`pad` module] documentation for a description of the model.*
///
/// [`PadSrc`]: struct.PadSrc.html
/// [`PadSrcWeak`]: struct.PadSrcWeak.html
/// [`downgrade`]: struct.PadSrcRef.html#method.downgrade
/// [`pad` module]: index.html
#[derive(Debug)]
pub struct PadSrcRef<'a> {
    strong: Arc<PadSrcInner>,
    phantom: PhantomData<&'a Self>,
}

impl PadSrcRef<'_> {
    fn new(inner_arc: Arc<PadSrcInner>) -> Self {
        PadSrcRef {
            strong: inner_arc,
            phantom: PhantomData,
        }
    }

    pub fn downgrade(&self) -> PadSrcWeak {
        PadSrcWeak(Arc::downgrade(&self.strong))
    }
}

impl Deref for PadSrcRef<'_> {
    type Target = PadSrcInner;

    fn deref(&self) -> &Self::Target {
        &self.strong
    }
}

/// The `PadSrc` which `Element`s must own.
///
/// Call [`downgrade`] if you need to `clone` the `PadSrc`.
///
/// *See the [`pad` module] documentation for a description of the model.*
///
/// [`downgrade`]: struct.PadSrc.html#method.downgrade
/// [`pad` module]: index.html
#[derive(Debug)]
pub struct PadSrc(Arc<PadSrcInner>);

impl PadSrc {
    pub fn new(gst_pad: gst::Pad, handler: impl PadSrcHandler) -> Self {
        let this = PadSrc(Arc::new(PadSrcInner::new(gst_pad)));
        this.init_pad_functions(handler);

        this
    }

    pub fn downgrade(&self) -> PadSrcWeak {
        PadSrcWeak(Arc::downgrade(&self.0))
    }

    pub fn as_ref(&self) -> PadSrcRef<'_> {
        PadSrcRef::new(Arc::clone(&self.0))
    }

    pub fn check_reconfigure(&self) -> bool {
        self.0.gst_pad.check_reconfigure()
    }

    fn init_pad_functions<H: PadSrcHandler>(&self, handler: H) {
        // FIXME: Do this better
        unsafe {
            let handler_clone = handler.clone();
            self.0
                .gst_pad
                .set_activate_function(move |gst_pad, parent| {
                    let handler = handler_clone.clone();
                    H::ElementImpl::catch_panic_pad_function(
                        parent,
                        || {
                            gst::error!(RUNTIME_CAT, obj = gst_pad, "Panic in PadSrc activate");
                            Err(gst::loggable_error!(
                                RUNTIME_CAT,
                                "Panic in PadSrc activate"
                            ))
                        },
                        move |imp| H::src_activate(handler, gst_pad, imp),
                    )
                });

            let handler_clone = handler.clone();
            self.0
                .gst_pad
                .set_activatemode_function(move |gst_pad, parent, mode, active| {
                    let handler = handler_clone.clone();
                    H::ElementImpl::catch_panic_pad_function(
                        parent,
                        || {
                            gst::error!(RUNTIME_CAT, obj = gst_pad, "Panic in PadSrc activatemode");
                            Err(gst::loggable_error!(
                                RUNTIME_CAT,
                                "Panic in PadSrc activatemode"
                            ))
                        },
                        move |imp| {
                            gst::log!(
                                RUNTIME_CAT,
                                obj = gst_pad,
                                "ActivateMode {:?}, {}",
                                mode,
                                active
                            );

                            if mode == gst::PadMode::Pull {
                                gst::error!(
                                    RUNTIME_CAT,
                                    obj = gst_pad,
                                    "Pull mode not supported by PadSrc"
                                );
                                return Err(gst::loggable_error!(
                                    RUNTIME_CAT,
                                    "Pull mode not supported by PadSrc"
                                ));
                            }

                            H::src_activatemode(handler, gst_pad, imp, mode, active)
                        },
                    )
                });

            // No need to `set_event_function` since `set_event_full_function`
            // overrides it and dispatches to `src_event` when necessary
            let handler_clone = handler.clone();
            self.0
                .gst_pad
                .set_event_full_function(move |gst_pad, parent, event| {
                    let handler = handler_clone.clone();
                    H::ElementImpl::catch_panic_pad_function(
                        parent,
                        || Err(FlowError::Error),
                        move |imp| H::src_event_full(handler, gst_pad, imp, event),
                    )
                });

            self.0
                .gst_pad
                .set_query_function(move |gst_pad, parent, query| {
                    let handler = handler.clone();
                    H::ElementImpl::catch_panic_pad_function(
                        parent,
                        || false,
                        move |imp| {
                            if !query.is_serialized() {
                                H::src_query(handler, gst_pad, imp, query)
                            } else {
                                gst::fixme!(
                                    RUNTIME_CAT,
                                    obj = gst_pad,
                                    "Serialized Query not supported"
                                );
                                false
                            }
                        },
                    )
                });
        }
    }
}

impl Drop for PadSrc {
    fn drop(&mut self) {
        // FIXME: Do this better
        unsafe {
            self.0
                .gst_pad
                .set_activate_function(move |_gst_pad, _parent| {
                    Err(gst::loggable_error!(RUNTIME_CAT, "PadSrc no longer exists"))
                });
            self.0
                .gst_pad
                .set_activatemode_function(move |_gst_pad, _parent, _mode, _active| {
                    Err(gst::loggable_error!(RUNTIME_CAT, "PadSrc no longer exists"))
                });
            self.0
                .gst_pad
                .set_event_function(move |_gst_pad, _parent, _event| false);
            self.0
                .gst_pad
                .set_event_full_function(move |_gst_pad, _parent, _event| Err(FlowError::Flushing));
            self.0
                .gst_pad
                .set_query_function(move |_gst_pad, _parent, _query| false);
        }
    }
}

impl Deref for PadSrc {
    type Target = PadSrcInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A trait to define `handler`s for [`PadSink`] callbacks.
///
/// *See the [`pad` module] documentation for a description of the model.*
///
/// [`PadSink`]: struct.PadSink.html
/// [`pad` module]: index.html
pub trait PadSinkHandler: Clone + Send + Sync + 'static {
    type ElementImpl: ElementImpl<Type: Send>;

    fn sink_activate(
        self,
        pad: &gst::Pad,
        _imp: &Self::ElementImpl,
    ) -> Result<(), gst::LoggableError> {
        if pad.is_active() {
            gst::debug!(
                RUNTIME_CAT,
                obj = pad,
                "Already activated in {:?} mode ",
                pad.mode()
            );
            return Ok(());
        }

        pad.activate_mode(gst::PadMode::Push, true).map_err(|err| {
            gst::error!(
                RUNTIME_CAT,
                obj = pad,
                "Error in PadSink activate: {:?}",
                err
            );
            gst::loggable_error!(RUNTIME_CAT, "Error in PadSink activate: {:?}", err)
        })
    }

    fn sink_activatemode(
        self,
        _pad: &gst::Pad,
        _imp: &Self::ElementImpl,
        _mode: gst::PadMode,
        _active: bool,
    ) -> Result<(), gst::LoggableError> {
        Ok(())
    }

    fn sink_chain(
        self,
        _pad: gst::Pad,
        _elem: <Self::ElementImpl as ObjectSubclass>::Type,
        _buffer: gst::Buffer,
    ) -> impl Future<Output = Result<FlowSuccess, FlowError>> + Send {
        future::err(FlowError::NotSupported)
    }

    fn sink_chain_list(
        self,
        _pad: gst::Pad,
        _elem: <Self::ElementImpl as ObjectSubclass>::Type,
        _buffer_list: gst::BufferList,
    ) -> impl Future<Output = Result<FlowSuccess, FlowError>> + Send {
        future::err(FlowError::NotSupported)
    }

    fn sink_event(self, pad: &gst::Pad, imp: &Self::ElementImpl, event: gst::Event) -> bool {
        assert!(!event.is_serialized());
        gst::log!(RUNTIME_CAT, obj = pad, "Handling {:?}", event);
        gst::Pad::event_default(pad, Some(imp.obj().as_ref()), event)
    }

    fn sink_event_serialized(
        self,
        pad: gst::Pad,
        elem: <Self::ElementImpl as ObjectSubclass>::Type,
        event: gst::Event,
    ) -> impl Future<Output = bool> + Send {
        assert!(event.is_serialized());

        async move {
            gst::log!(RUNTIME_CAT, obj = pad, "Handling {:?}", event);
            gst::Pad::event_default(&pad, Some(&elem), event)
        }
    }

    fn sink_event_full(
        self,
        pad: &gst::Pad,
        imp: &Self::ElementImpl,
        event: gst::Event,
    ) -> Result<FlowSuccess, FlowError> {
        assert!(!event.is_serialized());
        // default is to dispatch to `sink_event`
        // (as implemented in `gst_pad_send_event_unchecked`)
        let event_type = event.type_();
        event_to_event_full(self.sink_event(pad, imp, event), event_type)
    }

    fn sink_event_full_serialized(
        self,
        pad: gst::Pad,
        elem: <Self::ElementImpl as ObjectSubclass>::Type,
        event: gst::Event,
    ) -> impl Future<Output = Result<FlowSuccess, FlowError>> + Send {
        assert!(event.is_serialized());
        // default is to dispatch to `sink_event`
        // (as implemented in `gst_pad_send_event_unchecked`)
        let event_type = event.type_();
        event_to_event_full_serialized(
            Self::sink_event_serialized(self, pad, elem, event),
            event_type,
        )
    }

    fn sink_query(
        self,
        pad: &gst::Pad,
        imp: &Self::ElementImpl,
        query: &mut gst::QueryRef,
    ) -> bool {
        if query.is_serialized() {
            gst::log!(RUNTIME_CAT, obj = pad, "Dropping {:?}", query);
            // FIXME serialized queries should be handled with the dataflow
            // but we can't return a `Future` because we couldn't honor QueryRef's lifetime
            false
        } else {
            gst::log!(RUNTIME_CAT, obj = pad, "Handling {:?}", query);
            gst::Pad::query_default(pad, Some(imp.obj().as_ref()), query)
        }
    }
}

#[derive(Debug)]
pub struct PadSinkInner {
    gst_pad: gst::Pad,
}

impl PadSinkInner {
    fn new(gst_pad: gst::Pad) -> Self {
        if gst_pad.direction() != gst::PadDirection::Sink {
            panic!("Wrong pad direction for PadSink");
        }

        PadSinkInner { gst_pad }
    }

    pub fn gst_pad(&self) -> &gst::Pad {
        &self.gst_pad
    }
}

/// A [`PadSink`] which can be moved in `Handler`s functions and `Future`s.
///
/// Call [`upgrade`] to use the [`PadSink`].
///
/// *See the [`pad` module] documentation for a description of the model.*
///
/// [`PadSink`]: struct.PadSink.html
/// [`upgrade`]: struct.PadSinkWeak.html#method.upgrade
/// [`pad` module]: index.html
#[derive(Clone, Debug)]
pub struct PadSinkWeak(Weak<PadSinkInner>);

impl PadSinkWeak {
    pub fn upgrade(&self) -> Option<PadSinkRef<'_>> {
        self.0.upgrade().map(PadSinkRef::new)
    }
}

/// A [`PadSink`] to be used in [`handler`]s functions and `Future`s.
///
/// Call [`downgrade`] if you need to `clone` the [`PadSink`].
///
/// *See the [`pad` module] documentation for a description of the model.*
///
/// [`PadSink`]: struct.PadSink.html
/// [`handler`]: trait.PadSinkHandler.html
/// [`downgrade`]: struct.PadSinkRef.html#method.downgrade
/// [`pad` module]: index.html
#[derive(Debug)]
pub struct PadSinkRef<'a> {
    strong: Arc<PadSinkInner>,
    phantom: PhantomData<&'a Self>,
}

impl PadSinkRef<'_> {
    fn new(inner_arc: Arc<PadSinkInner>) -> Self {
        PadSinkRef {
            strong: inner_arc,
            phantom: PhantomData,
        }
    }

    pub fn downgrade(&self) -> PadSinkWeak {
        PadSinkWeak(Arc::downgrade(&self.strong))
    }
}

impl Deref for PadSinkRef<'_> {
    type Target = PadSinkInner;

    fn deref(&self) -> &Self::Target {
        &self.strong
    }
}

/// The `PadSink` which `Element`s must own.
///
/// Call [`downgrade`] if you need to `clone` the `PadSink`.
///
/// *See the [`pad` module] documentation for a description of the model.*
///
/// [`downgrade`]: struct.PadSink.html#method.downgrade
/// [`pad` module]: index.html
#[derive(Debug)]
pub struct PadSink(Arc<PadSinkInner>);

impl PadSink {
    pub fn downgrade(&self) -> PadSinkWeak {
        PadSinkWeak(Arc::downgrade(&self.0))
    }

    pub fn as_ref(&self) -> PadSinkRef<'_> {
        PadSinkRef::new(Arc::clone(&self.0))
    }
}

impl PadSink {
    pub fn new<H>(gst_pad: gst::Pad, handler: H) -> Self
    where
        H: PadSinkHandler,
        <H::ElementImpl as ObjectSubclass>::Type: IsA<gst::Element> + Send,
    {
        let this = PadSink(Arc::new(PadSinkInner::new(gst_pad)));
        this.init_pad_functions(handler);

        this
    }

    fn init_pad_functions<H>(&self, handler: H)
    where
        H: PadSinkHandler,
        <H::ElementImpl as ObjectSubclass>::Type: IsA<gst::Element> + Send,
    {
        unsafe {
            let handler_clone = handler.clone();
            self.0
                .gst_pad
                .set_activate_function(move |gst_pad, parent| {
                    let handler = handler_clone.clone();
                    H::ElementImpl::catch_panic_pad_function(
                        parent,
                        || {
                            gst::error!(RUNTIME_CAT, obj = gst_pad, "Panic in PadSink activate");
                            Err(gst::loggable_error!(
                                RUNTIME_CAT,
                                "Panic in PadSink activate"
                            ))
                        },
                        move |imp| H::sink_activate(handler, gst_pad, imp),
                    )
                });

            let handler_clone = handler.clone();
            self.0
                .gst_pad
                .set_activatemode_function(move |gst_pad, parent, mode, active| {
                    let handler = handler_clone.clone();
                    H::ElementImpl::catch_panic_pad_function(
                        parent,
                        || {
                            gst::error!(
                                RUNTIME_CAT,
                                obj = gst_pad,
                                "Panic in PadSink activatemode"
                            );
                            Err(gst::loggable_error!(
                                RUNTIME_CAT,
                                "Panic in PadSink activatemode"
                            ))
                        },
                        move |imp| {
                            gst::log!(
                                RUNTIME_CAT,
                                obj = gst_pad,
                                "ActivateMode {:?}, {}",
                                mode,
                                active
                            );

                            if mode == gst::PadMode::Pull {
                                gst::error!(
                                    RUNTIME_CAT,
                                    obj = gst_pad,
                                    "Pull mode not supported by PadSink"
                                );
                                return Err(gst::loggable_error!(
                                    RUNTIME_CAT,
                                    "Pull mode not supported by PadSink"
                                ));
                            }

                            H::sink_activatemode(handler, gst_pad, imp, mode, active)
                        },
                    )
                });

            let handler_clone = handler.clone();
            self.0
                .gst_pad
                .set_chain_function(move |gst_pad, parent, buffer| {
                    let handler = handler_clone.clone();
                    H::ElementImpl::catch_panic_pad_function(
                        parent,
                        || Err(FlowError::Error),
                        move |imp| {
                            let elem = imp.obj().clone();
                            let gst_pad = gst_pad.clone();

                            if let Some((ctx, task_id)) = Context::current_task() {
                                let delayed_fut = async move {
                                    H::sink_chain(handler, gst_pad, elem, buffer).await
                                };
                                let _ =
                                    ctx.add_sub_task(task_id, delayed_fut.map(|res| res.map(drop)));

                                Ok(gst::FlowSuccess::Ok)
                            } else {
                                let chain_fut = H::sink_chain(handler, gst_pad, elem, buffer);
                                executor::block_on(chain_fut)
                            }
                        },
                    )
                });

            let handler_clone = handler.clone();
            self.0
                .gst_pad
                .set_chain_list_function(move |gst_pad, parent, list| {
                    let handler = handler_clone.clone();
                    H::ElementImpl::catch_panic_pad_function(
                        parent,
                        || Err(FlowError::Error),
                        move |imp| {
                            let elem = imp.obj().clone();
                            let gst_pad = gst_pad.clone();

                            if let Some((ctx, task_id)) = Context::current_task() {
                                let delayed_fut = async move {
                                    H::sink_chain_list(handler, gst_pad, elem, list).await
                                };
                                let _ =
                                    ctx.add_sub_task(task_id, delayed_fut.map(|res| res.map(drop)));

                                Ok(gst::FlowSuccess::Ok)
                            } else {
                                let chain_list_fut =
                                    H::sink_chain_list(handler, gst_pad, elem, list);
                                executor::block_on(chain_list_fut)
                            }
                        },
                    )
                });

            // No need to `set_event_function` since `set_event_full_function`
            // overrides it and dispatches to `sink_event` when necessary
            let handler_clone = handler.clone();
            self.0
                .gst_pad
                .set_event_full_function(move |gst_pad, parent, event| {
                    let handler = handler_clone.clone();
                    H::ElementImpl::catch_panic_pad_function(
                        parent,
                        || Err(FlowError::Error),
                        move |imp| {
                            if event.is_serialized() {
                                let elem = imp.obj().clone();
                                let gst_pad = gst_pad.clone();

                                if let Some((ctx, task_id)) = Context::current_task() {
                                    let delayed_fut = async move {
                                        H::sink_event_full_serialized(handler, gst_pad, elem, event)
                                            .await
                                    };
                                    let _ = ctx.add_sub_task(
                                        task_id,
                                        delayed_fut.map(|res| res.map(drop)),
                                    );

                                    Ok(gst::FlowSuccess::Ok)
                                } else {
                                    let event_fut = H::sink_event_full_serialized(
                                        handler, gst_pad, elem, event,
                                    );
                                    executor::block_on(event_fut)
                                }
                            } else {
                                handler.sink_event_full(gst_pad, imp, event)
                            }
                        },
                    )
                });

            self.0
                .gst_pad
                .set_query_function(move |gst_pad, parent, query| {
                    let handler = handler.clone();
                    H::ElementImpl::catch_panic_pad_function(
                        parent,
                        || false,
                        move |imp| {
                            if !query.is_serialized() {
                                H::sink_query(handler, gst_pad, imp, query)
                            } else {
                                gst::fixme!(
                                    RUNTIME_CAT,
                                    obj = gst_pad,
                                    "Serialized Query not supported {query:?}"
                                );
                                false
                            }
                        },
                    )
                });
        }
    }
}

impl Drop for PadSink {
    fn drop(&mut self) {
        // FIXME: Do this better
        unsafe {
            self.0
                .gst_pad
                .set_activate_function(move |_gst_pad, _parent| {
                    Err(gst::loggable_error!(
                        RUNTIME_CAT,
                        "PadSink no longer exists"
                    ))
                });
            self.0
                .gst_pad
                .set_activatemode_function(move |_gst_pad, _parent, _mode, _active| {
                    Err(gst::loggable_error!(
                        RUNTIME_CAT,
                        "PadSink no longer exists"
                    ))
                });
            self.0
                .gst_pad
                .set_chain_function(move |_gst_pad, _parent, _buffer| Err(FlowError::Flushing));
            self.0
                .gst_pad
                .set_chain_list_function(move |_gst_pad, _parent, _list| Err(FlowError::Flushing));
            self.0
                .gst_pad
                .set_event_function(move |_gst_pad, _parent, _event| false);
            self.0
                .gst_pad
                .set_event_full_function(move |_gst_pad, _parent, _event| Err(FlowError::Flushing));
            self.0
                .gst_pad
                .set_query_function(move |_gst_pad, _parent, _query| false);
        }
    }
}

impl Deref for PadSink {
    type Target = PadSinkInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
