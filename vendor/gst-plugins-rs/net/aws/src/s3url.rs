// Copyright (C) 2017 Author: Arun Raghavan <arun@arunraghavan.net>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use crate::s3arn::parse_arn;
use aws_sdk_s3::config::Region;
use percent_encoding::{percent_decode, percent_encode, AsciiSet, CONTROLS};
use url::Url;

#[derive(Clone)]
pub struct GstS3Url {
    pub region: Region,
    pub bucket: String,
    pub object: String,
    pub version: Option<String>,
}

// FIXME: Copied from the url crate, see https://github.com/servo/rust-url/issues/529
// https://url.spec.whatwg.org/#fragment-percent-encode-set
const FRAGMENT: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'<').add(b'>').add(b'`');
// https://url.spec.whatwg.org/#path-percent-encode-set
const PATH: &AsciiSet = &FRAGMENT.add(b'#').add(b'?').add(b'{').add(b'}');
const PATH_SEGMENT: &AsciiSet = &PATH.add(b'/').add(b'%');

impl std::fmt::Display for GstS3Url {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "s3://{}/{}/{}{}",
            self.region,
            self.bucket,
            percent_encode(self.object.as_bytes(), PATH_SEGMENT),
            if self.version.is_some() {
                format!("?version={}", self.version.clone().unwrap())
            } else {
                "".to_string()
            }
        )
    }
}

pub fn parse_s3_url(url_str: &str) -> Result<GstS3Url, String> {
    let url = Url::parse(url_str).map_err(|err| format!("Parse error: {err}"))?;

    if url.scheme() != "s3" {
        return Err(format!("Unsupported URI '{}'", url.scheme()));
    }

    if !url.has_host() {
        return Err(format!("Invalid host in uri '{url}'"));
    }

    let host = url.host_str().unwrap();

    let region_str = host
        .parse()
        .or_else(|_| {
            let (name, endpoint) = host.split_once('+').ok_or(())?;
            let name =
                base32::decode(base32::Alphabet::Rfc4648 { padding: true }, name).ok_or(())?;
            let endpoint =
                base32::decode(base32::Alphabet::Rfc4648 { padding: true }, endpoint).ok_or(())?;
            let name = String::from_utf8(name).map_err(|_| ())?;
            let endpoint = String::from_utf8(endpoint).map_err(|_| ())?;
            Ok(format!("{name}{endpoint}"))
        })
        .map_err(|_: ()| format!("Invalid region '{host}'"))?;

    // Note that aws_sdk_s3::Region does not provide any error/validation
    // methods to check the region argument being passed to it.
    // See https://docs.rs/aws-sdk-s3/latest/aws_sdk_s3/struct.Region.html
    let region = Region::new(region_str);

    let mut path = url
        .path_segments()
        .ok_or_else(|| format!("Invalid uri '{url}'"))?;

    let bucket = path.next().unwrap().to_string();

    let o = path
        .next()
        .ok_or_else(|| format!("Invalid empty object/bucket '{url}'"))?;

    if o.is_empty() {
        return Err(format!("Invalid empty object/bucket '{url}'"));
    }

    let mut object = percent_decode(o.as_bytes())
        .decode_utf8()
        .unwrap()
        .to_string();

    object = path.fold(object, |o, p| {
        format!(
            "{o}/{}",
            percent_decode(p.as_bytes()).decode_utf8().unwrap()
        )
    });

    let mut q = url.query_pairs();
    let v = q.next();

    let version = match v {
        Some((ref k, ref v)) if k == "version" => Some((*v).to_string()),
        None => None,
        Some(_) => return Err("Bad query, only 'version' is supported".to_owned()),
    };

    if q.next().is_some() {
        return Err("Extra query terms, only 'version' is supported".to_owned());
    }

    Ok(GstS3Url {
        region,
        bucket,
        object,
        version,
    })
}

// S3 compliant URI or access point/Amazon Resource Name.
#[derive(Clone, Debug)]
pub struct GstS3Uri {
    pub bucket: String,
    pub key: String,
    pub region: Option<String>,
}

impl std::fmt::Display for GstS3Uri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "s3://{}/{}", self.bucket, self.key)
    }
}

pub fn parse_s3_uri(uri_str: &str) -> Result<GstS3Uri, String> {
    /*
     * S3 URI can have multiple forms.
     *
     * When not using an access point, these can be follows
     * - s3://s3gstreamer/upload.mp4
     *
     * When using an access point, these can be follows
     * - s3://arn:aws:s3:us-west-2:123456789012:accesspoint/myaccesspoint/mykey
     *
     * Both of the above are also how AWS Console displays the URI.
     *
     * To summarise, based on the below link which lists different resource types,
     * https://docs.aws.amazon.com/service-authorization/latest/reference/list_amazons3.html#amazons3-resources-for-iam-policies
     *
     * we only support the `accesspoint` resource type here. This is also
     * determined by what `set_bucket` in AWS SDK can actually accept in
     * the context of S3 and when considering an URI.
     */
    if uri_str.starts_with("s3://arn:") {
        let uri_str = uri_str.strip_prefix("s3://").unwrap();

        match parse_arn(uri_str) {
            Ok(arn) => {
                if arn.partition() != "aws" || arn.service() != "s3" {
                    return Err("Invalid partition or service".to_string());
                }

                if arn.resource_id().is_empty() {
                    return Err("No resource found".to_string());
                }

                if arn.region().is_empty() || arn.account_id().is_empty() {
                    return Err("Missing region or account id".to_string());
                }

                // `set_bucket` in AWS SDK needs the bucket to be the
                // complete ARN without key and not just resource_id
                // as parsed above.
                let bucket = uri_str
                    .rfind('/')
                    .map(|pos| &uri_str[..pos])
                    .map(|arn| arn.to_string());
                let key = arn.resource_id().last();

                if let (Some(bucket), Some(key)) = (bucket, key) {
                    let region = (!arn.region().is_empty()).then(|| arn.region().to_string());

                    return Ok(GstS3Uri {
                        bucket: bucket.to_string(),
                        key: key.to_string(),
                        region,
                    });
                }

                return Err("No bucket or key found in resource".to_string());
            }
            Err(e) => return Err(e.to_string()),
        }
    }

    // S3 Uri represents the location of a S3 object, prefix, or bucket.
    if uri_str.starts_with("s3://") {
        let without_prefix = uri_str.strip_prefix("s3://").unwrap();

        if let Some(pos) = without_prefix.find('/') {
            let bucket = without_prefix[..pos].to_string();
            let key = without_prefix[pos + 1..].to_string();

            if bucket.is_empty() {
                return Err("Bucket name cannot be empty".into());
            }

            return Ok(GstS3Uri {
                bucket,
                key,
                region: None,
            });
        }
    }

    Err("Failed to parse S3 URI".to_owned())
}
