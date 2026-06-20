// Copyright (C) 2017 Author: Arun Raghavan <arun@arunraghavan.net>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::{
    config::{timeout::TimeoutConfig, Credentials, Region},
    error::{DisplayErrorContext, ProvideErrorMetadata},
    primitives::{ByteStream, ByteStreamError},
};
use aws_types::sdk_config::SdkConfig;

use bytes::{buf::BufMut, Bytes, BytesMut};
use futures::{future, Future};
use std::fmt;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::Duration;
use tokio::runtime;

pub const DEFAULT_S3_REGION: &str = "us-west-2";

#[allow(deprecated)]
pub static AWS_BEHAVIOR_VERSION: LazyLock<aws_config::BehaviorVersion> =
    LazyLock::new(aws_config::BehaviorVersion::v2023_11_09);

pub static RUNTIME: LazyLock<runtime::Runtime> = LazyLock::new(|| {
    runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .thread_name("gst-aws-runtime")
        .build()
        .unwrap()
});

#[derive(Debug)]
pub enum WaitError<E> {
    Cancelled,
    FutureError(E),
}

impl<E: ProvideErrorMetadata + std::error::Error> fmt::Display for WaitError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WaitError::Cancelled => f.write_str("Cancelled"),
            WaitError::FutureError(err) => {
                write!(f, "{}: {}", DisplayErrorContext(&err), err.meta())
            }
        }
    }
}

#[derive(Default)]
pub enum Canceller {
    #[default]
    None,
    Handle(future::AbortHandle),
    Cancelled,
}

impl Canceller {
    pub fn abort(&mut self) {
        if let Canceller::Handle(ref canceller) = *self {
            canceller.abort();
        }

        *self = Canceller::Cancelled;
    }
}

pub fn wait<F, T, E>(canceller_mutex: &Mutex<Canceller>, future: F) -> Result<T, WaitError<E>>
where
    F: Send + Future<Output = Result<T, E>>,
    F::Output: Send,
    T: Send,
    E: Send,
{
    let mut canceller = canceller_mutex.lock().unwrap();
    if matches!(*canceller, Canceller::Cancelled) {
        return Err(WaitError::Cancelled);
    }
    let (abort_handle, abort_registration) = future::AbortHandle::new_pair();
    *canceller = Canceller::Handle(abort_handle);
    drop(canceller);

    let abortable_future = future::Abortable::new(future, abort_registration);

    // FIXME: add a timeout as well

    let res = {
        let _enter = RUNTIME.enter();
        futures::executor::block_on(async {
            match abortable_future.await {
                // Future resolved successfully
                Ok(Ok(res)) => Ok(res),
                // Future resolved with an error
                Ok(Err(err)) => Err(WaitError::FutureError(err)),
                // Canceller called before future resolved
                Err(future::Aborted) => Err(WaitError::Cancelled),
            }
        })
    };

    /* Clear out the canceller */
    let mut canceller = canceller_mutex.lock().unwrap();
    if matches!(*canceller, Canceller::Cancelled) {
        return Err(WaitError::Cancelled);
    }
    *canceller = Canceller::None;
    drop(canceller);

    res
}

pub fn wait_stream(
    canceller_mutex: &Mutex<Canceller>,
    stream: &mut ByteStream,
) -> Result<Bytes, WaitError<ByteStreamError>> {
    wait(canceller_mutex, async move {
        let mut collect = BytesMut::new();

        // Loop over the stream and collect till we're done
        while let Some(item) = stream.try_next().await? {
            collect.put(item)
        }

        Ok::<Bytes, ByteStreamError>(collect.freeze())
    })
}

// See setting-timeouts example in aws-sdk-rust.
pub fn timeout_config(request_timeout: Duration) -> TimeoutConfig {
    TimeoutConfig::builder()
        .operation_attempt_timeout(request_timeout)
        .build()
}

pub fn wait_config(
    canceller_mutex: &Mutex<Canceller>,
    region: Option<Region>,
    timeout_config: TimeoutConfig,
    credentials: Option<Credentials>,
) -> Result<SdkConfig, WaitError<ByteStreamError>> {
    let region_provider = if let Some(region) = region {
        RegionProviderChain::first_try(region)
            .or_default_provider()
            .or_else(Region::new(DEFAULT_S3_REGION))
    } else {
        RegionProviderChain::default_provider()
    };
    let config_future = match credentials {
        Some(cred) => aws_config::defaults(*AWS_BEHAVIOR_VERSION)
            .timeout_config(timeout_config)
            .region(region_provider)
            .credentials_provider(cred)
            .load(),
        None => aws_config::defaults(*AWS_BEHAVIOR_VERSION)
            .timeout_config(timeout_config)
            .region(region_provider)
            .load(),
    };

    let mut canceller = canceller_mutex.lock().unwrap();
    if matches!(*canceller, Canceller::Cancelled) {
        return Err(WaitError::Cancelled);
    }
    let (abort_handle, abort_registration) = future::AbortHandle::new_pair();
    *canceller = Canceller::Handle(abort_handle);
    drop(canceller);

    let abortable_future = future::Abortable::new(config_future, abort_registration);

    let res = {
        let _enter = RUNTIME.enter();
        futures::executor::block_on(async {
            match abortable_future.await {
                // Future resolved successfully
                Ok(config) => Ok(config),
                // Canceller called before future resolved
                Err(future::Aborted) => Err(WaitError::Cancelled),
            }
        })
    };

    /* Clear out the canceller */
    let mut canceller = canceller_mutex.lock().unwrap();
    if matches!(*canceller, Canceller::Cancelled) {
        return Err(WaitError::Cancelled);
    }
    *canceller = Canceller::None;
    drop(canceller);

    res
}

pub fn duration_from_millis(millis: i64) -> Duration {
    match millis {
        -1 => Duration::MAX,
        v => Duration::from_millis(v as u64),
    }
}

pub fn duration_to_millis(dur: Option<Duration>) -> i64 {
    match dur {
        None => Duration::MAX.as_millis() as i64,
        Some(d) => d.as_millis() as i64,
    }
}
