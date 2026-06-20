// Copyright (C) 2019 Amazon.com, Inc. or its affiliates <mkolny@amazon.com>
// Copyright (C) 2023 Asymptotic Inc
//      Author: Arun Raghavan <arun@asymptotic.io>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_base::subclass::prelude::*;

use aws_sdk_s3::{
    config::{self, retry::RetryConfig, Credentials, Region},
    operation::put_object::builders::PutObjectFluentBuilder,
    primitives::ByteStream,
    Client,
};

use super::NextFile;
use std::collections::HashMap;
use std::convert::From;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::Duration;

use crate::s3url::*;
use crate::s3utils::{self, duration_from_millis, duration_to_millis, WaitError};

const DEFAULT_RETRY_ATTEMPTS: u32 = 5;
const DEFAULT_FLUSH_INTERVAL_BUFFERS: u64 = 1;
const DEFAULT_FLUSH_INTERVAL_BYTES: u64 = 0;
const DEFAULT_FLUSH_INTERVAL_TIME: gst::ClockTime = gst::ClockTime::from_nseconds(0);
const DEFAULT_FLUSH_ON_ERROR: bool = false;
const DEFAULT_FORCE_PATH_STYLE: bool = false;
const DEFAULT_NEXT_FILE: NextFile = NextFile::Buffer;
const DEFAULT_MIN_KEYFRAME_DISTANCE: gst::ClockTime = gst::ClockTime::from_seconds(10);

// General setting for create / abort requests
const DEFAULT_REQUEST_TIMEOUT_MSEC: u64 = 15_000;

struct Started {
    client: Client,
    buffer: Vec<u8>,
    start_pts: Option<gst::ClockTime>,
    num_buffers: u64,
    need_flush: bool,
    index: u64,
    next_segment: Option<gst::ClockTime>,
    streamheaders: Option<Vec<u8>>,
    streamheaders_size: u64,
    file_start_pts: Option<gst::ClockTime>,
}

impl Started {
    pub fn new(client: Client, buffer: Vec<u8>) -> Started {
        Started {
            client,
            buffer,
            start_pts: gst::ClockTime::NONE,
            num_buffers: 0,
            need_flush: false,
            index: 0,
            next_segment: gst::ClockTime::NONE,
            streamheaders: None,
            streamheaders_size: 0,
            file_start_pts: gst::ClockTime::NONE,
        }
    }
}

#[derive(Default)]
enum State {
    #[default]
    Stopped,
    Started(Started),
}

struct Settings {
    region: Region,
    bucket: Option<String>,
    key: Option<String>,
    cache_control: Option<String>,
    content_type: Option<String>,
    content_disposition: Option<String>,
    content_encoding: Option<String>,
    content_language: Option<String>,
    access_key: Option<String>,
    secret_access_key: Option<String>,
    session_token: Option<String>,
    metadata: Option<gst::Structure>,
    retry_attempts: u32,
    request_timeout: Duration,
    endpoint_uri: Option<String>,
    force_path_style: bool,
    flush_interval_buffers: u64,
    flush_interval_bytes: u64,
    flush_interval_time: Option<gst::ClockTime>,
    flush_on_error: bool,
    next_file: NextFile,
    min_keyframe_distance: gst::ClockTime,
}

impl Settings {
    fn to_uri(&self) -> String {
        GstS3Url {
            region: self.region.clone(),
            bucket: self.bucket.clone().unwrap(),
            object: self.key.clone().unwrap(),
            version: None,
        }
        .to_string()
    }

    fn to_metadata(&self, imp: &S3PutObjectSink) -> Option<HashMap<String, String>> {
        self.metadata.as_ref().map(|structure| {
            let mut hash = HashMap::new();

            for (key, value) in structure.iter() {
                if let Ok(Ok(value_str)) = value.transform::<String>().map(|v| v.get()) {
                    gst::log!(CAT, imp = imp, "metadata '{}' -> '{}'", key, value_str);
                    hash.insert(key.to_string(), value_str);
                } else {
                    gst::warning!(
                        CAT,
                        imp = imp,
                        "Failed to convert metadata '{}' to string ('{:?}')",
                        key,
                        value
                    );
                }
            }

            hash
        })
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            region: Region::new("us-west-2"),
            bucket: None,
            key: None,
            cache_control: None,
            content_type: None,
            content_disposition: None,
            content_encoding: None,
            content_language: None,
            access_key: None,
            secret_access_key: None,
            session_token: None,
            metadata: None,
            retry_attempts: DEFAULT_RETRY_ATTEMPTS,
            request_timeout: Duration::from_millis(DEFAULT_REQUEST_TIMEOUT_MSEC),
            endpoint_uri: None,
            force_path_style: DEFAULT_FORCE_PATH_STYLE,
            flush_interval_buffers: DEFAULT_FLUSH_INTERVAL_BUFFERS,
            flush_interval_bytes: DEFAULT_FLUSH_INTERVAL_BYTES,
            flush_interval_time: Some(DEFAULT_FLUSH_INTERVAL_TIME),
            flush_on_error: DEFAULT_FLUSH_ON_ERROR,
            next_file: DEFAULT_NEXT_FILE,
            min_keyframe_distance: DEFAULT_MIN_KEYFRAME_DISTANCE,
        }
    }
}

#[derive(Default)]
pub struct S3PutObjectSink {
    url: Mutex<Option<GstS3Url>>,
    s3_uri: Mutex<Option<GstS3Uri>>,
    settings: Mutex<Settings>,
    state: Mutex<State>,
    canceller: Mutex<s3utils::Canceller>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "awss3putobjectsink",
        gst::DebugColorFlags::empty(),
        Some("Amazon S3 PutObject Sink"),
    )
});

impl S3PutObjectSink {
    fn check_thresholds(&self, settings: &Settings, state: &Started, buffer: &gst::Buffer) -> bool {
        let pts = buffer.pts();
        let duration = buffer.duration();

        #[allow(clippy::if_same_then_else)]
        #[allow(clippy::needless_bool)]
        // Verbose if/else form for readability
        if settings.flush_interval_buffers > 0
            && (state.num_buffers % settings.flush_interval_buffers) == 0
        {
            true
        } else if settings.flush_interval_bytes > 0
            && (state.buffer.len() as u64 % settings.flush_interval_bytes) == 0
        {
            true
        } else if settings.flush_interval_time.is_some()
            && settings.flush_interval_time.unwrap() != DEFAULT_FLUSH_INTERVAL_TIME
            && state.start_pts.is_some()
            && pts.is_some()
            && duration.is_some()
            && (pts.unwrap() - state.start_pts.unwrap() + duration.unwrap())
                % settings.flush_interval_time.unwrap()
                == gst::ClockTime::from_nseconds(0)
        {
            true
        } else {
            false
        }
    }

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.lock().unwrap();
        let settings = self.settings.lock().unwrap();

        if let State::Started { .. } = *state {
            unreachable!("Element should be started");
        }

        let s3url = self.url.lock().unwrap();
        let s3uri = self.s3_uri.lock().unwrap();
        if s3url.is_none() && s3uri.is_none() {
            return Err(gst::error_msg!(
                gst::ResourceError::Settings,
                ["Cannot start without a URL or S3 compatible URI being set"]
            ));
        }
        let region = s3url
            .as_ref()
            .map(|u| u.region.clone())
            .or_else(|| s3uri.as_ref()?.region.clone().map(Region::new));
        let use_arn_region = s3uri.as_ref().is_some_and(|uri| uri.region.is_some());
        drop(s3url);
        drop(s3uri);

        let timeout_config = s3utils::timeout_config(settings.request_timeout);

        let cred = match (
            settings.access_key.as_ref(),
            settings.secret_access_key.as_ref(),
        ) {
            (Some(access_key), Some(secret_access_key)) => Some(Credentials::new(
                access_key.clone(),
                secret_access_key.clone(),
                settings.session_token.clone(),
                None,
                "aws-s3-putobject-sink",
            )),
            _ => None,
        };

        let sdk_config = s3utils::wait_config(&self.canceller, region, timeout_config, cred)
            .map_err(|err| match err {
                WaitError::FutureError(err) => gst::error_msg!(
                    gst::ResourceError::OpenWrite,
                    ["Failed to create SDK config: {}", err]
                ),
                WaitError::Cancelled => {
                    gst::error_msg!(
                        gst::LibraryError::Failed,
                        ["SDK config request interrupted during start"]
                    )
                }
            })?;

        let config_builder = config::Builder::from(&sdk_config)
            .use_arn_region(use_arn_region)
            // Force path style cannot be used with ARN.
            .force_path_style(settings.force_path_style && !use_arn_region)
            .retry_config(RetryConfig::standard().with_max_attempts(settings.retry_attempts));

        let config = if let Some(ref uri) = settings.endpoint_uri {
            config_builder.endpoint_url(uri).build()
        } else {
            config_builder.build()
        };

        let client = Client::from_conf(config);

        *state = State::Started(Started::new(client, Vec::new()));

        Ok(())
    }

    fn set_uri(self: &S3PutObjectSink, url_str: Option<&str>) -> Result<(), glib::Error> {
        let state = self.state.lock().unwrap();

        if let State::Started { .. } = *state {
            return Err(glib::Error::new(
                gst::URIError::BadState,
                "Cannot set URI on a started s3sink",
            ));
        }

        let mut url = self.url.lock().unwrap();

        if url_str.is_none() {
            *url = None;
            return Ok(());
        }

        gst::debug!(CAT, imp = self, "Setting uri to {:?}", url_str);

        let url_str = url_str.unwrap();
        match parse_s3_url(url_str) {
            Ok(s3url) => {
                *url = Some(s3url);
                Ok(())
            }
            Err(_) => Err(glib::Error::new(
                gst::URIError::BadUri,
                "Could not parse S3 URI",
            )),
        }
    }

    fn set_s3_uri(self: &S3PutObjectSink, uri_str: Option<&str>) -> Result<(), glib::Error> {
        let state = self.state.lock().unwrap();

        if let State::Started { .. } = *state {
            return Err(glib::Error::new(
                gst::URIError::BadState,
                "Cannot set S3 URI on a started s3src",
            ));
        }

        let mut s3_uri = self.s3_uri.lock().unwrap();

        if uri_str.is_none() {
            *s3_uri = None;
            return Ok(());
        }

        let uri_str = uri_str.unwrap();
        match parse_s3_uri(uri_str) {
            Ok(s3uri) => {
                gst::info!(CAT, imp = self, "S3 URI: {s3uri:?}");
                *s3_uri = Some(s3uri);
                Ok(())
            }
            Err(e) => {
                gst::error!(CAT, imp = self, "Failed to parse S3 URI: {e}");
                Err(glib::Error::new(
                    gst::URIError::BadUri,
                    "Could not parse S3 URI",
                ))
            }
        }
    }

    fn accumulate_buffer(
        &self,
        buffer: &gst::Buffer,
        started_state: &mut Started,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        if started_state.start_pts.is_none() {
            started_state.start_pts = buffer.pts();
        }

        started_state.num_buffers += 1;
        started_state.need_flush = true;

        gst::trace!(CAT, imp = self, "Rendering {:?}", buffer);
        let map = buffer.map_readable().map_err(|_| {
            gst::element_imp_error!(self, gst::CoreError::Failed, ["Failed to map buffer"]);
            gst::FlowError::Error
        })?;

        started_state.buffer.extend_from_slice(map.as_slice());

        Ok(gst::FlowSuccess::Ok)
    }

    fn create_body_with_streamheaders(
        &self,
        next_file: NextFile,
        streamheaders: &Option<Vec<u8>>,
        buffer: &[u8],
    ) -> ByteStream {
        match next_file {
            NextFile::KeyFrame | NextFile::MaxSize | NextFile::MaxDuration => {
                if let Some(headers) = streamheaders {
                    let with_sh = [&headers[..], buffer].concat();

                    ByteStream::from(with_sh)
                } else {
                    ByteStream::from(buffer.to_vec())
                }
            }
            _ => ByteStream::from(buffer.to_vec()),
        }
    }

    fn create_put_object_request(
        &self,
        started_state: &mut Started,
    ) -> Result<Option<PutObjectFluentBuilder>, gst::FlowError> {
        let settings = self.settings.lock().unwrap();

        if started_state.buffer.is_empty() {
            return Ok(None);
        }

        let body = Some(self.create_body_with_streamheaders(
            settings.next_file,
            &started_state.streamheaders,
            &started_state.buffer,
        ));

        let (bucket, object, new_file) = self.get_bucket_and_key();

        let key = if new_file {
            match sprintf::sprintf!(&object.unwrap().clone(), started_state.index) {
                Ok(k) => {
                    /* Equivalent to opening a new file */
                    started_state.index += 1;
                    started_state.buffer = Vec::new();

                    Some(k)
                }
                Err(e) => {
                    gst::element_imp_error!(
                        self,
                        gst::CoreError::Failed,
                        ["Failed to format file name: {}", e]
                    );
                    return Err(gst::FlowError::Error);
                }
            }
        } else {
            object
        };
        let metadata = settings.to_metadata(self);
        let client = &started_state.client;

        Ok(Some(
            client
                .put_object()
                .set_body(body)
                .set_bucket(bucket)
                .set_key(key)
                .set_metadata(metadata),
        ))
    }

    fn to_write_next_file(
        &self,
        started_state: &mut Started,
        buffer: &gst::Buffer,
        buffer_size: u64,
    ) -> bool {
        let settings = self.settings.lock().unwrap();
        let next_file = settings.next_file;
        let max_file_size = settings.flush_interval_bytes;
        let max_file_duration = settings.flush_interval_time;
        let min_keyframe_distance = settings.min_keyframe_distance;

        match next_file {
            NextFile::Buffer => self.check_thresholds(&settings, started_state, buffer),
            NextFile::MaxSize => {
                started_state.buffer.len() as u64 + started_state.streamheaders_size + buffer_size
                    > max_file_size
            }
            NextFile::MaxDuration => {
                let mut new_duration = gst::ClockTime::ZERO;

                if buffer.pts().is_some() && started_state.file_start_pts.is_some() {
                    new_duration = buffer.pts().unwrap() - started_state.file_start_pts.unwrap();

                    if buffer.duration().is_some() {
                        new_duration += buffer.duration().unwrap();
                    }
                }

                started_state.file_start_pts = match buffer.pts() {
                    Some(pts) => Some(pts),
                    None => started_state.file_start_pts,
                };

                new_duration > max_file_duration.unwrap()
            }
            NextFile::KeyFrame => {
                if started_state.next_segment == gst::ClockTime::NONE && buffer.pts().is_some() {
                    started_state.next_segment =
                        Some(buffer.pts().unwrap() + min_keyframe_distance);
                }

                if buffer.pts().is_some() {
                    let buffer_ts = buffer.pts().unwrap();
                    let delta_unit = buffer.flags().contains(gst::BufferFlags::DELTA_UNIT);
                    let next_segment = started_state
                        .next_segment
                        .expect("Next segment must be valid here");

                    if buffer_ts >= next_segment && !delta_unit {
                        started_state.next_segment = Some(next_segment + min_keyframe_distance);

                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            NextFile::Discont => buffer.flags().contains(gst::BufferFlags::DISCONT),
            NextFile::KeyUnitEvent => false, // Next file will be opened on KeyUnitEvent
        }
    }

    fn write_put_object_request(
        &self,
        started_state: &mut Started,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let req = self.create_put_object_request(started_state)?;

        if let Some(put_object_req) = req {
            let put_object_req_future = put_object_req.send();

            match s3utils::wait(&self.canceller, put_object_req_future) {
                Ok(_) => Ok(gst::FlowSuccess::Ok),
                Err(err) => match err {
                    WaitError::Cancelled => Ok(gst::FlowSuccess::Ok),
                    WaitError::FutureError(e) => {
                        gst::element_imp_error!(self, gst::CoreError::Failed, ["{e}"]);
                        Err(gst::FlowError::Error)
                    }
                },
            }
        } else {
            Ok(gst::FlowSuccess::Ok)
        }
    }

    fn write_buffer(&self, buffer: &gst::Buffer) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.lock().unwrap();
        let started_state = match *state {
            State::Started(ref mut started_state) => started_state,
            State::Stopped => {
                gst::element_imp_error!(self, gst::CoreError::Failed, ["Not started yet"]);
                return Err(gst::FlowError::Error);
            }
        };

        let map = buffer.map_readable().map_err(|_| {
            gst::element_imp_error!(self, gst::CoreError::Failed, ["Failed to map buffer"]);
            gst::FlowError::Error
        })?;

        if self.to_write_next_file(started_state, buffer, map.size() as u64) {
            self.write_put_object_request(started_state)?;
        }

        self.accumulate_buffer(buffer, started_state)
    }

    fn get_bucket_and_key(&self) -> (Option<String>, Option<String>, bool) {
        let url = self.url.lock().unwrap().clone();
        let s3_uri = self.s3_uri.lock().unwrap().clone();

        if let Some(s3_uri) = s3_uri {
            return (
                Some(s3_uri.bucket.clone()),
                Some(s3_uri.key.clone()),
                s3_uri.key.contains("%0"),
            );
        }

        if let Some(url) = url {
            return (
                Some(url.bucket.clone()),
                Some(url.object.clone()),
                url.object.contains("%0"),
            );
        }

        unreachable!()
    }
}

#[glib::object_subclass]
impl ObjectSubclass for S3PutObjectSink {
    const NAME: &'static str = "GstAwsS3PutObjectSink";
    type Type = super::S3PutObjectSink;
    type ParentType = gst_base::BaseSink;
}

impl ObjectImpl for S3PutObjectSink {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecString::builder("bucket")
                    .nick("S3 Bucket")
                    .blurb("The bucket of the file to write")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("key")
                    .nick("S3 Key")
                    .blurb("The key of the file to write")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("region")
                    .nick("AWS Region")
                    .blurb("An AWS region (e.g. eu-west-2).")
                    .default_value(Some("us-west-2"))
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("uri")
                    .nick("URI")
                    .blurb("The S3 object URI")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("access-key")
                    .nick("Access Key")
                    .blurb("AWS Access Key")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("secret-access-key")
                    .nick("Secret Access Key")
                    .blurb("AWS Secret Access Key")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("session-token")
                    .nick("Session Token")
                    .blurb("AWS temporary Session Token from STS")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoxed::builder::<gst::Structure>("metadata")
                    .nick("Metadata")
                    .blurb("A map of metadata to store with the object in S3; field values need to be convertible to strings.")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder("retry-attempts")
                    .nick("Retry attempts")
                    .blurb("Number of times AWS SDK attempts a request before abandoning the request")
                    .minimum(1)
                    .maximum(10)
                    .default_value(DEFAULT_RETRY_ATTEMPTS)
                    .build(),
                glib::ParamSpecInt64::builder("request-timeout")
                    .nick("Request timeout")
                    .blurb("Timeout for general S3 requests (in ms, set to -1 for infinity)")
                    .minimum(-1)
                    .default_value(DEFAULT_REQUEST_TIMEOUT_MSEC as i64)
                    .build(),
                glib::ParamSpecString::builder("endpoint-uri")
                    .nick("S3 endpoint URI")
                    .blurb("The S3 endpoint URI to use")
                    .build(),
                glib::ParamSpecString::builder("cache-control")
                    .nick("cache-control")
                    .blurb("Cache-Control header to set for uploaded object")
                    .build(),
                glib::ParamSpecString::builder("content-type")
                    .nick("content-type")
                    .blurb("Content-Type header to set for uploaded object")
                    .build(),
                glib::ParamSpecString::builder("content-disposition")
                    .nick("content-disposition")
                    .blurb("Content-Disposition header to set for uploaded object")
                    .build(),
                glib::ParamSpecString::builder("content-encoding")
                    .nick("content-encoding")
                    .blurb("Content-Encoding header to set for uploaded object")
                    .build(),
                glib::ParamSpecString::builder("content-language")
                    .nick("content-language")
                    .blurb("Content-Language header to set for uploaded object")
                    .build(),
                glib::ParamSpecUInt64::builder("flush-interval-buffers")
                    .nick("Flush interval in buffers")
                    .blurb("Number of buffers to accumulate before doing a write (0 => disable)")
                    .default_value(DEFAULT_FLUSH_INTERVAL_BUFFERS)
                    .build(),
                glib::ParamSpecUInt64::builder("flush-interval-bytes")
                    .nick("Flush interval in bytes")
                    .blurb("Number of bytes to accumulate before doing a write (0 => disable)")
                    .default_value(DEFAULT_FLUSH_INTERVAL_BYTES)
                    .build(),
                glib::ParamSpecUInt64::builder("flush-interval-time")
                    .nick("Flush interval in duration")
                    .blurb("Total duration of buffers to accumulate before doing a write (0 => disable)")
                    .default_value(DEFAULT_FLUSH_INTERVAL_TIME.nseconds())
                    .build(),
                glib::ParamSpecBoolean::builder("flush-on-error")
                    .nick("Flush on error")
                    .blurb("Whether to write out the data on error (like stopping without an EOS)")
                    .default_value(DEFAULT_FLUSH_ON_ERROR)
                    .build(),
                glib::ParamSpecBoolean::builder("force-path-style")
                    .nick("Force path style")
                    .blurb("Force client to use path-style addressing for buckets")
                    .default_value(DEFAULT_FORCE_PATH_STYLE)
                    .build(),
                glib::ParamSpecEnum::builder_with_default("next-file", DEFAULT_NEXT_FILE)
                    .nick("Next File")
                    .blurb("When to start new file")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt64::builder("min-keyframe-distance")
                    .nick("Minimum keyframe distance")
                    .blurb("Minimum distance between keyframes to start a new file")
                    .default_value(DEFAULT_MIN_KEYFRAME_DISTANCE.into())
                    .build(),
                glib::ParamSpecString::builder("s3-uri")
                    .nick("S3 URI")
                    .blurb("The S3 compatible URI or S3 Access Point URI to use")
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        let mut settings = self.settings.lock().unwrap();

        gst::debug!(
            CAT,
            imp = self,
            "Setting property '{}' to '{:?}'",
            pspec.name(),
            value
        );

        match pspec.name() {
            "bucket" => {
                settings.bucket = value
                    .get::<Option<String>>()
                    .expect("type checked upstream");
                if settings.key.is_some() {
                    let _ = self.set_uri(Some(&settings.to_uri()));
                }
            }
            "key" => {
                settings.key = value
                    .get::<Option<String>>()
                    .expect("type checked upstream");
                if settings.bucket.is_some() {
                    let _ = self.set_uri(Some(&settings.to_uri()));
                }
            }
            "region" => {
                let region = value.get::<String>().expect("type checked upstream");
                settings.region = Region::new(region);
                if settings.key.is_some() && settings.bucket.is_some() {
                    let _ = self.set_uri(Some(&settings.to_uri()));
                }
            }
            "uri" => {
                let _ = self.set_uri(value.get().expect("type checked upstream"));
            }
            "access-key" => {
                settings.access_key = value.get().expect("type checked upstream");
            }
            "secret-access-key" => {
                settings.secret_access_key = value.get().expect("type checked upstream");
            }
            "session-token" => {
                settings.session_token = value.get().expect("type checked upstream");
            }
            "metadata" => {
                settings.metadata = value.get().expect("type checked upstream");
            }
            "retry-attempts" => {
                settings.retry_attempts = value.get::<u32>().expect("type checked upstream");
            }
            "request-timeout" => {
                settings.request_timeout =
                    duration_from_millis(value.get::<i64>().expect("type checked upstream"));
            }
            "endpoint-uri" => {
                settings.endpoint_uri = value
                    .get::<Option<String>>()
                    .expect("type checked upstream");
                if settings.key.is_some() && settings.bucket.is_some() {
                    let _ = self.set_uri(Some(&settings.to_uri()));
                }
            }
            "cache-control" => {
                settings.cache_control = value
                    .get::<Option<String>>()
                    .expect("type checked upstream");
            }
            "content-type" => {
                settings.content_type = value
                    .get::<Option<String>>()
                    .expect("type checked upstream");
            }
            "content-disposition" => {
                settings.content_disposition = value
                    .get::<Option<String>>()
                    .expect("type checked upstream");
            }
            "content-encoding" => {
                settings.content_encoding = value
                    .get::<Option<String>>()
                    .expect("type checked upstream");
            }
            "content-language" => {
                settings.content_language = value
                    .get::<Option<String>>()
                    .expect("type checked upstream");
            }
            "flush-interval-buffers" => {
                settings.flush_interval_buffers =
                    value.get::<u64>().expect("type checked upstream");
            }
            "flush-interval-bytes" => {
                settings.flush_interval_bytes = value.get::<u64>().expect("type checked upstream");
            }
            "flush-interval-time" => {
                settings.flush_interval_time = value
                    .get::<Option<gst::ClockTime>>()
                    .expect("type checked upstream");
            }
            "flush-on-error" => {
                settings.flush_on_error = value.get::<bool>().expect("type checked upstream");
            }
            "force-path-style" => {
                settings.force_path_style = value.get::<bool>().expect("type checked upstream");
            }
            "next-file" => {
                settings.next_file = value.get::<NextFile>().expect("type checked upstream");
            }
            "min-keyframe-distance" => {
                settings.min_keyframe_distance = value
                    .get::<gst::ClockTime>()
                    .expect("type checked upstream");
            }
            "s3-uri" => {
                let _ = self.set_s3_uri(value.get().expect("type checked upstream"));
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().unwrap();

        match pspec.name() {
            "key" => settings.key.to_value(),
            "bucket" => settings.bucket.to_value(),
            "region" => settings.region.to_string().to_value(),
            "uri" => {
                let url = self.url.lock().unwrap();
                let url = match *url {
                    Some(ref url) => url.to_string(),
                    None => "".to_string(),
                };

                url.to_value()
            }
            "access-key" => settings.access_key.to_value(),
            "secret-access-key" => settings.secret_access_key.to_value(),
            "session-token" => settings.session_token.to_value(),
            "metadata" => settings.metadata.to_value(),
            "retry-attempts" => settings.retry_attempts.to_value(),
            "request-timeout" => duration_to_millis(Some(settings.request_timeout)).to_value(),
            "endpoint-uri" => settings.endpoint_uri.to_value(),
            "cache-control" => settings.cache_control.to_value(),
            "content-type" => settings.content_type.to_value(),
            "content-disposition" => settings.content_disposition.to_value(),
            "content-encoding" => settings.content_encoding.to_value(),
            "content-language" => settings.content_language.to_value(),
            "flush-interval-buffers" => settings.flush_interval_buffers.to_value(),
            "flush-interval-bytes" => settings.flush_interval_bytes.to_value(),
            "flush-interval-time" => settings.flush_interval_time.to_value(),
            "flush-on-error" => settings.flush_on_error.to_value(),
            "force-path-style" => settings.force_path_style.to_value(),
            "min-keyframe-distance" => settings.min_keyframe_distance.to_value(),
            "next-file" => settings.next_file.to_value(),
            "s3-uri" => {
                let s3_uri = self.s3_uri.lock().unwrap();
                let uri = match *s3_uri {
                    Some(ref uri) => uri.to_string(),
                    None => "".to_string(),
                };

                uri.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for S3PutObjectSink {}

impl ElementImpl for S3PutObjectSink {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            #[cfg(feature = "doc")]
            NextFile::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
            gst::subclass::ElementMetadata::new(
                "Amazon S3 PutObject sink",
                "Source/Network",
                "Writes an object to Amazon S3 using PutObject (mostly useful for small files)",
                "Arun Raghavan <arun@asymptotic.io>",
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

            vec![sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl BaseSinkImpl for S3PutObjectSink {
    fn start(&self) -> Result<(), gst::ErrorMessage> {
        self.start()
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.lock().unwrap();
        let settings = self.settings.lock().unwrap();

        if let State::Started(ref mut started_state) = *state {
            if settings.flush_on_error && started_state.need_flush {
                drop(settings);

                if self.write_put_object_request(started_state).is_err() {
                    gst::error!(CAT, imp = self, "Failed to finalize the next-file upload",);
                }
            }
        }

        *state = State::Stopped;

        gst::info!(CAT, imp = self, "Stopped");

        Ok(())
    }

    fn render(&self, buffer: &gst::Buffer) -> Result<gst::FlowSuccess, gst::FlowError> {
        self.write_buffer(buffer)
    }

    fn event(&self, event: gst::Event) -> bool {
        use gst::EventView;

        match event.view() {
            EventView::CustomDownstream(ev) => {
                let settings = self.settings.lock().unwrap();
                let next_file = settings.next_file;
                let is_next_key_unit_event = next_file == NextFile::KeyUnitEvent;
                drop(settings);

                if is_next_key_unit_event && gst_video::ForceKeyUnitEvent::is(ev) {
                    use gst_video::DownstreamForceKeyUnitEvent;

                    match DownstreamForceKeyUnitEvent::parse(ev) {
                        Ok(_key_unit_event) => {
                            let mut state = self.state.lock().unwrap();

                            if let State::Started(ref mut started_state) = *state {
                                if let Err(e) = self.write_put_object_request(started_state) {
                                    gst::element_imp_error!(
                                        self,
                                        gst::CoreError::Failed,
                                        ["Failed to write on KeyUnitEvent, {e}"]
                                    );
                                }
                            }
                        }
                        Err(e) => gst::error!(CAT, "Failed to parse key unit event: {e}"),
                    }
                }
            }
            EventView::Eos(_) => {
                let mut state = self.state.lock().unwrap();

                if let State::Started(ref mut started_state) = *state {
                    started_state.need_flush = false;

                    if self.write_put_object_request(started_state).is_err() {
                        gst::element_imp_error!(
                            self,
                            gst::CoreError::Failed,
                            ["Failed to finalize the upload"]
                        );
                    }
                }
            }
            _ => (),
        }

        BaseSinkImplExt::parent_event(self, event)
    }

    fn unlock(&self) -> Result<(), gst::ErrorMessage> {
        let mut canceller = self.canceller.lock().unwrap();
        canceller.abort();
        Ok(())
    }

    fn unlock_stop(&self) -> Result<(), gst::ErrorMessage> {
        let mut canceller = self.canceller.lock().unwrap();
        *canceller = s3utils::Canceller::None;
        Ok(())
    }

    fn set_caps(&self, caps: &gst::Caps) -> Result<(), gst::LoggableError> {
        let s = caps
            .structure(0)
            .ok_or(gst::loggable_error!(CAT, "Missing caps in set_caps"))?;

        if let Ok(Some(streamheaders)) = s.get_optional::<gst::ArrayRef>("streamheader") {
            if streamheaders.is_empty() {
                return Ok(());
            }

            let streamheaders = streamheaders.as_slice();
            let mut headers: Vec<u8> = Vec::new();

            let mut state = self.state.lock().unwrap();
            let started_state = match *state {
                State::Started(ref mut started_state) => started_state,
                State::Stopped => {
                    return Err(gst::loggable_error!(CAT, "Element should be started"));
                }
            };

            started_state.streamheaders_size = 0;

            for header in streamheaders {
                let buffer = header.get::<Option<gst::Buffer>>();

                if let Ok(Some(buf)) = buffer {
                    let map = buf.map_readable().map_err(|_| {
                        gst::element_imp_error!(
                            self,
                            gst::CoreError::Failed,
                            ["Failed to map streamheader buffer"]
                        );
                        gst::loggable_error!(CAT, "Failed to map streamheader buffer")
                    })?;

                    headers.extend_from_slice(map.as_slice());

                    started_state.streamheaders_size += map.size() as u64;
                }
            }

            if !headers.is_empty() {
                let _ = started_state.streamheaders.take();
                gst::info!(CAT, imp = self, "Got streamheaders");
                started_state.streamheaders = Some(headers);
            }
        }

        self.parent_set_caps(caps)
    }
}
