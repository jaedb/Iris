// Copyright (C) 2025 Matthew Waters <matthew@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::prelude::*;
use pretty_assertions::assert_eq;

use cdp_types::cea708_types::tables::*;
use cdp_types::cea708_types::*;
use cdp_types::*;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstrsclosedcaption::plugin_register_static().unwrap();
    });
}

fn gen_cdp_data(seq: u8, service: u8, codes: &[Code]) -> gst::Buffer {
    assert!(seq < 4);
    assert!(service < 64);

    let fps = cdp_types::Framerate::from_id(0x8).unwrap();
    let mut writer = CDPWriter::default();
    let mut packet = DTVCCPacket::new(seq);
    let mut service = Service::new(service);
    for c in codes {
        service.push_code(c).unwrap();
    }
    packet.push_service(service).unwrap();
    writer.push_packet(packet);
    let mut data = vec![];
    writer.write(fps, &mut data).unwrap();
    let mut buf = gst::Buffer::from_mut_slice(data);
    {
        let buf = buf.get_mut().unwrap();
        buf.set_pts(0.nseconds());
    }
    buf
}

#[test]
fn test_cdpserviceinject_override() {
    init();

    let mut h = gst_check::Harness::with_padnames("cdpserviceinject", Some("sink"), Some("src"));
    h.set_src_caps_str("closedcaption/x-cea-708,format=cdp,framerate=60/1");
    let services = gst::Array::from_iter([gst::Structure::builder("service")
        .field("service", 2)
        .field("language", "eng")
        .field("easy-reader", true)
        .field("wide-aspect-ratio", true)
        .build()
        .to_send_value()]);
    h.element().unwrap().set_property("services", services);

    let buf = gen_cdp_data(0, 2, &[Code::LatinCapitalA]);
    let out = h.push_and_pull(buf).unwrap();
    let readable = out.map_readable().unwrap();

    let mut parser = CDPParser::default();
    parser.parse(&readable).unwrap();
    let parsed = parser.pop_packet().unwrap();
    assert_eq!(parsed.sequence_no(), 0);
    let services = parsed.services();
    assert_eq!(services.len(), 1);
    assert_eq!(services[0].number(), 2);
    let codes = services[0].codes();
    assert_eq!(codes.len(), 1);
    assert_eq!(codes[0], Code::LatinCapitalA);

    let service = parser.service_info().unwrap();
    assert!(service.is_start());
    assert!(service.is_complete());
    let services = service.services();
    assert_eq!(services.len(), 1);
    assert_eq!(services[0].language(), [b'e', b'n', b'g']);
    let FieldOrService::Service(digital) = services[0].service() else {
        unreachable!();
    };
    assert_eq!(digital.service_no(), 2);
    assert!(digital.easy_reader());
    assert!(digital.wide_aspect_ratio());
}
