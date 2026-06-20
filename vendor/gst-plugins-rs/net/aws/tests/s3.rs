// Copyright (C) 2022, Asymptotic Inc.
//      Author: Arun Raghavan <arun@asymptotic.io>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0
//

// Note: these tests need valid AWS credentials to run. To avoid failures on CI, we test for the
// existence of AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY environment variables and skip the test
// if those don't exist. That means that in local testing, other methods of providing credentials
// (such as ~/.aws/credentials) will not work.

// The test times out on Windows for some reason, skip until we figure out why
#[cfg(not(target_os = "windows"))]
#[cfg(test)]
mod tests {
    use gst::prelude::*;

    use gstaws::s3utils::{AWS_BEHAVIOR_VERSION, DEFAULT_S3_REGION};

    fn init() {
        use std::sync::Once;
        static INIT: Once = Once::new();

        INIT.call_once(|| {
            gst::init().unwrap();
            gstaws::plugin_register_static().unwrap();
            // Makes it easier to get AWS SDK logs if needed
            env_logger::init();
        });
    }

    fn make_buffer(content: &[u8]) -> gst::Buffer {
        let mut buf = gst::Buffer::from_slice(content.to_owned());
        buf.make_mut().set_pts(gst::ClockTime::from_mseconds(200));
        buf
    }

    async fn delete_object(region: String, bucket: &str, key: &str) {
        let region_provider = aws_config::meta::region::RegionProviderChain::first_try(
            aws_sdk_s3::config::Region::new(region),
        )
        .or_default_provider();

        let config = aws_config::defaults(*AWS_BEHAVIOR_VERSION)
            .region(region_provider)
            .load()
            .await;
        let client = aws_sdk_s3::Client::new(&config);

        client
            .delete_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .unwrap();
    }

    // Common helper
    async fn do_s3_multipart_test(key_prefix: &str) {
        init();

        let region = std::env::var("AWS_REGION").unwrap_or_else(|_| DEFAULT_S3_REGION.to_string());
        let bucket =
            std::env::var("AWS_S3_BUCKET").unwrap_or_else(|_| "gst-plugins-rs-tests".to_string());
        let key = format!("{key_prefix}-{:?}.txt", chrono::Utc::now());
        let uri = format!("s3://{region}/{bucket}/{key}");
        let content = "Hello, world!\n".as_bytes();

        // Manually add the element so we can configure it before it goes to PLAYING
        let mut h1 = gst_check::Harness::new_empty();
        // Need to add_parse() because the Harness API / Rust bindings aren't conducive to creating and
        // adding an element manually

        h1.add_parse(format!("awss3sink uri=\"{uri}\"").as_str());

        h1.set_src_caps(gst::Caps::builder("text/plain").build());
        h1.play();

        h1.push(make_buffer(content)).unwrap();
        h1.push(make_buffer(content)).unwrap();
        h1.push(make_buffer(content)).unwrap();
        h1.push(make_buffer(content)).unwrap();
        h1.push(make_buffer(content)).unwrap();
        h1.push_event(gst::event::Eos::new());

        let mut h2 = gst_check::Harness::new("awss3src");
        h2.element().unwrap().set_property("uri", uri.clone());
        h2.play();

        let buf = h2.pull_until_eos().unwrap().unwrap();
        assert_eq!(
            content.repeat(5),
            buf.into_mapped_buffer_readable().unwrap().as_slice()
        );

        delete_object(region.clone(), &bucket, &key).await;
    }

    // Common helper
    async fn do_s3_putobject_test(
        key_prefix: &str,
        buffers: Option<u64>,
        bytes: Option<u64>,
        time: Option<gst::ClockTime>,
        do_eos: bool,
    ) {
        init();

        let region = std::env::var("AWS_REGION").unwrap_or_else(|_| DEFAULT_S3_REGION.to_string());
        let bucket =
            std::env::var("AWS_S3_BUCKET").unwrap_or_else(|_| "gst-plugins-rs-tests".to_string());
        let key = format!("{key_prefix}-{:?}.txt", chrono::Utc::now());
        let uri = format!("s3://{region}/{bucket}/{key}");
        let content = "Hello, world!\n".as_bytes();

        // Manually add the element so we can configure it before it goes to PLAYING
        let mut h1 = gst_check::Harness::new_empty();

        // Need to add_parse() because the Harness API / Rust bindings aren't conducive to creating and
        // adding an element manually

        h1.add_parse(
            format!("awss3putobjectsink key=\"{key}\" region=\"{region}\" bucket=\"{bucket}\" name=\"sink\"")
                .as_str(),
        );

        let h1el = h1
            .element()
            .unwrap()
            .dynamic_cast::<gst::Bin>()
            .unwrap()
            .by_name("sink")
            .unwrap();
        if let Some(b) = buffers {
            h1el.set_property("flush-interval-buffers", b)
        };
        if let Some(b) = bytes {
            h1el.set_property("flush-interval-bytes", b)
        };
        if time.is_some() {
            h1el.set_property("flush-interval-time", time)
        };
        if !do_eos {
            h1el.set_property("flush-on-error", true)
        }

        h1.set_src_caps(gst::Caps::builder("text/plain").build());
        h1.play();

        h1.push(make_buffer(content)).unwrap();
        h1.push(make_buffer(content)).unwrap();
        h1.push(make_buffer(content)).unwrap();
        h1.push(make_buffer(content)).unwrap();
        h1.push(make_buffer(content)).unwrap();

        if do_eos {
            h1.push_event(gst::event::Eos::new());
        } else {
            // teardown to trigger end
            drop(h1);
        }

        let mut h2 = gst_check::Harness::new("awss3src");
        h2.element().unwrap().set_property("uri", uri.clone());
        h2.play();

        let buf = h2.pull_until_eos().unwrap().unwrap();
        assert_eq!(
            content.repeat(5),
            buf.into_mapped_buffer_readable().unwrap().as_slice()
        );

        delete_object(region.clone(), &bucket, &key).await;
    }

    #[test_with::env(AWS_ACCESS_KEY_ID)]
    #[test_with::env(AWS_SECRET_ACCESS_KEY)]
    #[tokio::test]
    async fn test_s3_multipart_simple() {
        do_s3_multipart_test("s3-test").await;
    }

    #[test_with::env(AWS_ACCESS_KEY_ID)]
    #[test_with::env(AWS_SECRET_ACCESS_KEY)]
    #[tokio::test]
    async fn test_s3_multipart_whitespace() {
        do_s3_multipart_test("s3 test").await;
    }

    #[test_with::env(AWS_ACCESS_KEY_ID)]
    #[test_with::env(AWS_SECRET_ACCESS_KEY)]
    #[tokio::test]
    async fn test_s3_multipart_unicode() {
        do_s3_multipart_test("s3 🧪 😱").await;
    }

    #[test_with::env(AWS_ACCESS_KEY_ID)]
    #[test_with::env(AWS_SECRET_ACCESS_KEY)]
    #[tokio::test]
    async fn test_s3_put_object_simple() {
        do_s3_putobject_test("s3-put-object-test", None, None, None, true).await;
    }

    #[test_with::env(AWS_ACCESS_KEY_ID)]
    #[test_with::env(AWS_SECRET_ACCESS_KEY)]
    #[tokio::test]
    async fn test_s3_put_object_whitespace() {
        do_s3_putobject_test("s3 put object test", None, None, None, true).await;
    }

    #[test_with::env(AWS_ACCESS_KEY_ID)]
    #[test_with::env(AWS_SECRET_ACCESS_KEY)]
    #[tokio::test]
    async fn test_s3_put_object_unicode() {
        do_s3_putobject_test("s3 put object 🧪 😱", None, None, None, true).await;
    }

    #[test_with::env(AWS_ACCESS_KEY_ID)]
    #[test_with::env(AWS_SECRET_ACCESS_KEY)]
    #[tokio::test]
    async fn test_s3_put_object_flush_buffers() {
        // Awkward threshold as we push 5 buffers
        do_s3_putobject_test("s3-put-object-test fbuf", Some(2), None, None, true).await;
    }

    #[test_with::env(AWS_ACCESS_KEY_ID)]
    #[test_with::env(AWS_SECRET_ACCESS_KEY)]
    #[tokio::test]
    async fn test_s3_put_object_flush_bytes() {
        // Awkward threshold as we push 14 bytes per buffer
        do_s3_putobject_test("s3-put-object-test fbytes", None, Some(30), None, true).await;
    }

    #[test_with::env(AWS_ACCESS_KEY_ID)]
    #[test_with::env(AWS_SECRET_ACCESS_KEY)]
    #[tokio::test]
    async fn test_s3_put_object_flush_time() {
        do_s3_putobject_test(
            "s3-put-object-test ftime",
            None,
            None,
            // Awkward threshold as we push each buffer with 200ms
            Some(gst::ClockTime::from_mseconds(300)),
            true,
        )
        .await;
    }

    #[test_with::env(AWS_ACCESS_KEY_ID)]
    #[test_with::env(AWS_SECRET_ACCESS_KEY)]
    #[tokio::test]
    async fn test_s3_put_object_on_eos() {
        // Disable all flush thresholds, so only EOS causes a flush
        do_s3_putobject_test(
            "s3-put-object-test eos",
            Some(0),
            Some(0),
            Some(gst::ClockTime::from_nseconds(0)),
            true,
        )
        .await;
    }

    #[test_with::env(AWS_ACCESS_KEY_ID)]
    #[test_with::env(AWS_SECRET_ACCESS_KEY)]
    #[tokio::test]
    async fn test_s3_put_object_without_eos() {
        // Disable all flush thresholds, skip EOS, and cause a flush on error
        do_s3_putobject_test(
            "s3-put-object-test !eos",
            Some(0),
            Some(0),
            Some(gst::ClockTime::from_nseconds(0)),
            false,
        )
        .await;
    }
}
