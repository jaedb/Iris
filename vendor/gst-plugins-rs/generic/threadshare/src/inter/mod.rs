// Copyright (C) 2025 François Laignel <francois@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0
/// Module for the threadsharing `ts-intersink` & `ts-intersrc` elements
///
/// These threadshare-based elements provide a means to connect an upstream pipeline to
/// multiple downstream pipelines while taking advantage of reduced nunmber of threads &
/// context switches.
///
/// Differences with the `ts-proxy` elements:
///
/// * Link one to many pipelines instead of one to one.
/// * No back pressure: items which can't be handled by a downstream pipeline are
///   lost, wherease they are kept in a pending queue and block the stream for
///   `ts-proxysink`.
use gst::glib;
use gst::prelude::*;

use slab::Slab;

use async_lock::{
    futures::{Read as AsyncLockRead, Write as AsyncLockWrite},
    Mutex, RwLock,
};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Weak};
use std::time::Duration;

use crate::dataqueue::DataQueue;
use crate::runtime::executor::block_on_or_add_subtask;

mod sink;
mod src;

static INTER_CONTEXTS: LazyLock<Mutex<HashMap<String, InterContextWeak>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

const DEFAULT_INTER_CONTEXT: &str = "";
const DEFAULT_CONTEXT: &str = "";
const DEFAULT_CONTEXT_WAIT: Duration = Duration::ZERO;

#[derive(Debug)]
struct InterContextInner {
    name: String,
    dataqueues: Slab<DataQueue>,
    sources: Slab<src::InterSrc>,
    sinkpad: Option<gst::Pad>,
    upstream_latency: Option<gst::ClockTime>,
}

impl InterContextInner {
    fn new(name: &str) -> InterContextInner {
        InterContextInner {
            name: name.into(),
            dataqueues: Slab::new(),
            sources: Slab::new(),
            sinkpad: None,
            upstream_latency: None,
        }
    }
}

impl Drop for InterContextInner {
    fn drop(&mut self) {
        let name = self.name.clone();
        block_on_or_add_subtask(async move {
            let mut inter_ctxs = INTER_CONTEXTS.lock().await;
            inter_ctxs.remove(&name);
        });
    }
}

#[derive(Debug, Clone)]
struct InterContext(Arc<RwLock<InterContextInner>>);

impl InterContext {
    fn new(name: &str) -> InterContext {
        InterContext(Arc::new(RwLock::new(InterContextInner::new(name))))
    }

    fn downgrade(&self) -> InterContextWeak {
        InterContextWeak(Arc::downgrade(&self.0))
    }

    fn read(&self) -> AsyncLockRead<'_, InterContextInner> {
        self.0.read()
    }

    fn write(&self) -> AsyncLockWrite<'_, InterContextInner> {
        self.0.write()
    }
}

#[derive(Debug, Clone)]
struct InterContextWeak(Weak<RwLock<InterContextInner>>);

impl InterContextWeak {
    fn upgrade(&self) -> Option<InterContext> {
        self.0.upgrade().map(InterContext)
    }
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "ts-intersink",
        gst::Rank::NONE,
        sink::InterSink::static_type(),
    )?;
    gst::Element::register(
        Some(plugin),
        "ts-intersrc",
        gst::Rank::NONE,
        src::InterSrc::static_type(),
    )
}
