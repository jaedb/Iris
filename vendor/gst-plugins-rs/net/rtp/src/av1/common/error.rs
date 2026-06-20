//
// Copyright (C) 2022 Vivienne Watermeier <vwatermeier@igalia.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

macro_rules! err_flow {
    ($imp:ident, read, $msg:literal) => {
        |err| {
            gst::warning!(CAT, imp = $imp, $msg, err);
            gst::element_imp_warning!($imp, gst::ResourceError::Read, [$msg, err]);
            gst::FlowError::Error
        }
    };
    ($imp:ident, write, $msg:literal) => {
        |err| {
            gst::warning!(CAT, imp = $imp, $msg, err);
            gst::element_imp_warning!($imp, gst::ResourceError::Write, [$msg, err]);
            gst::FlowError::Error
        }
    };

    ($imp:ident, buf_read) => {
        err_flow!($imp, read, "Failed to read buffer: {}")
    };
    ($imp:ident, aggr_header_write) => {
        err_flow!(
            $imp,
            write,
            "Failed to write aggregation header to the payload: {}"
        )
    };
    ($imp:ident, leb_write) => {
        err_flow!(
            $imp,
            write,
            "Failed to write leb128 size field to the payload: {}"
        )
    };
    ($imp:ident, obu_write) => {
        err_flow!($imp, write, "Failed to write OBU bytes to the payload: {}")
    };
    ($imp:ident, outbuf_alloc) => {
        err_flow!($imp, write, "Failed to allocate output buffer: {}")
    };
    ($imp:ident, payload_buf) => {
        err_flow!($imp, read, "Failed to get RTP payload buffer: {}")
    };
    ($imp:ident, aggr_header_read) => {
        err_flow!($imp, read, "Failed to read aggregation header: {}")
    };
    ($imp:ident, find_element) => {
        err_flow!($imp, read, "Failed to find OBU element in packet: {}")
    };
    ($imp:ident, leb_read) => {
        err_flow!($imp, read, "Failed to read leb128 size field: {}")
    };
    ($imp:ident, leb_write) => {
        err_flow!($imp, read, "Failed to write leb128 size field: {}")
    };
    ($imp:ident, obu_read) => {
        err_flow!($imp, read, "Failed to read OBU header: {}")
    };
}

pub(crate) use err_flow;
