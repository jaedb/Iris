/*
 *  Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
 *  SPDX-License-Identifier: Apache-2.0
 */

/*
 * FIXME: When the below situation changes, drop this.
 * aws-sdk-rust does not expose any ARN parsing utilities.
 * Issue: github.com/smithy-lang/smithy-rs/issues/4090
 *
 * Taken from
 * https://github.com/smithy-lang/smithy-rs/blob/main/rust-runtime/inlineable/src/endpoint_lib/arn.rs
*/

use std::borrow::Cow;
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Eq, PartialEq)]
pub struct Arn<'a> {
    partition: &'a str,
    service: &'a str,
    region: &'a str,
    account_id: &'a str,
    resource_id: Vec<&'a str>,
}

#[allow(unused)]
impl<'a> Arn<'a> {
    pub fn partition(&self) -> &'a str {
        self.partition
    }

    pub fn service(&self) -> &'a str {
        self.service
    }

    pub fn region(&self) -> &'a str {
        self.region
    }

    pub fn account_id(&self) -> &'a str {
        self.account_id
    }

    pub fn resource_id(&self) -> &Vec<&'a str> {
        &self.resource_id
    }
}

#[derive(Debug, PartialEq)]
pub struct InvalidArn {
    message: Cow<'static, str>,
}

impl InvalidArn {
    fn from_static(message: &'static str) -> InvalidArn {
        Self {
            message: Cow::Borrowed(message),
        }
    }
}

impl Display for InvalidArn {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for InvalidArn {}

impl<'a> Arn<'a> {
    fn parse(arn: &'a str) -> Result<Self, InvalidArn> {
        let mut split = arn.splitn(6, ':');
        let invalid_format =
            || InvalidArn::from_static("ARN must have 6 components delimited by `:`");
        let arn = split.next().ok_or_else(invalid_format)?;
        let partition = split.next().ok_or_else(invalid_format)?;
        let service = split.next().ok_or_else(invalid_format)?;
        let region = split.next().ok_or_else(invalid_format)?;
        let account_id = split.next().ok_or_else(invalid_format)?;
        let resource_id = split.next().ok_or_else(invalid_format)?;

        if arn != "arn" {
            return Err(InvalidArn::from_static(
                "first component of the ARN must be `arn`",
            ));
        }

        if partition.is_empty() || service.is_empty() || resource_id.is_empty() {
            return Err(InvalidArn::from_static(
                "partition, service, and resource id must all be non-empty",
            ));
        }

        let resource_id = resource_id.split([':', '/']).collect::<Vec<_>>();

        Ok(Self {
            partition,
            service,
            region,
            account_id,
            resource_id,
        })
    }
}

pub fn parse_arn<'a>(input: &'a str) -> Result<Arn<'a>, InvalidArn> {
    Arn::parse(input)
}
