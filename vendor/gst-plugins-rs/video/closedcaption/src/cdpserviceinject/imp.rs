// Copyright (C) 2025 Matthew Waters <matthew@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-cdpserviceinject
 *
 * Adds or updates CDP Service Information Descriptor as specified in SMPTE 334-2.  This can be used
 * to e.g. signal the language of a particular caption stream.
 *
 * ```sh
 * gst-launch-1.0 ... ! cdpserviceinject services='<(GstStructure)"a,service=1,language=eng,easy-reader=false,wide-aspect-ratio=false;", (GstStructure)"a,service=-1,language=eng;">' ! ...
 * ```
 *
 * Since: plugins-rs-0.14.0
 */
use std::collections::VecDeque;
use std::sync::{LazyLock, Mutex};

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_base::subclass::prelude::*;

fn service_descriptor_from_array(s: &gst::Array) -> Vec<cdp_types::ServiceEntry> {
    let mut service_entries = vec![];
    for entry in s.iter() {
        let Ok(desc) = entry.get::<gst::Structure>() else {
            gst::warning!(CAT, "List does not contain a structure");
            continue;
        };
        gst::debug!(CAT, "parsing {desc:?}");
        let Ok(service_id) = desc.get::<i32>("service") else {
            gst::warning!(CAT, "Failed to parse service number in service descriptor");
            continue;
        };
        if service_id == 0 || !(-4..=63).contains(&service_id) {
            gst::warning!(CAT, "Invalid service/channel ({service_id}) id provided");
            continue;
        }
        let Ok(language) = desc.get::<&str>("language") else {
            gst::warning!(CAT, "Missing language for service ({service_id}). Ignoring");
            continue;
        };

        // TODO: validate exact values from ISO 639.2/B.
        if language.len() != 3 {
            gst::error!(CAT, "Language descriptor for service {service_id} is not ISO-639.2/B compatible. Ignoring");
            continue;
        }
        let field_or_service = if service_id > 0 {
            let easy_reader = desc.get::<bool>("easy-reader").unwrap_or_default();
            let wide_aspect_ratio = desc.get::<bool>("wide-aspect-ratio").unwrap_or_default();
            cdp_types::FieldOrService::Service(cdp_types::DigitalServiceEntry::new(
                service_id as u8,
                easy_reader,
                wide_aspect_ratio,
            ))
        } else {
            let cea608_channel = -service_id;
            let field1 = [1, 2].contains(&cea608_channel);
            cdp_types::FieldOrService::Field(field1)
        };
        // XXX: convert between utf-8 and ISO 8859-1 (latin-1) when a language code is seen that is
        // outside of the ASCII range.
        let lang: [u8; 3] = language.as_bytes()[..3].try_into().unwrap();
        service_entries.push(cdp_types::ServiceEntry::new(lang, field_or_service));
    }

    gst::log!(CAT, "new service entries {service_entries:?}");
    service_entries
}

#[derive(Default)]
struct State {
    service_descriptor: Vec<cdp_types::ServiceEntry>,
    cdp_parser: cdp_types::CDPParser,
    cdp_writer: cdp_types::CDPWriter,

    pending_service_descriptor: VecDeque<cdp_types::ServiceEntry>,
}

#[derive(Clone, Default)]
struct Settings {
    service_descriptor: gst::Array,
}

#[derive(Default)]
pub struct CdpServiceInject {
    state: Mutex<State>,
    settings: Mutex<Settings>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "cdpserviceinject",
        gst::DebugColorFlags::empty(),
        Some("Add or update a CDP Service Information Descriptor"),
    )
});

impl CdpServiceInject {}

#[glib::object_subclass]
impl ObjectSubclass for CdpServiceInject {
    const NAME: &'static str = "GstCdpServiceInject";
    type Type = super::CdpServiceInject;
    type ParentType = gst_base::BaseTransform;
}

impl ObjectImpl for CdpServiceInject {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                /**
                 * GstCdpServiceInject:services:
                 *
                 * Configure a list of services for injection into the CDP bitstream.
                 *
                 * Each value in the array is a GstStructure with the following fields:
                 *
                 * - "service"          G_TYPE_INT      The service number being identified.
                 *                                      '0' is not a valid service.
                 *                                      Negative values in the range [-4, -1]
                 *                                      correspond to CEA-608 caption service
                 *                                      numbers.
                 *                                      Positive values in the range [1, 63]
                 *                                      correspond to CEA-708 caption service
                 *                                      numbers.
                 * - "language"         G_TYPE_STRING   A 3-letter character language code as
                 *                                      specified in ISO 639.2/B. This value is
                 *                                      encoded as UTF-8.
                 * - "easy-reader"      G_TYPE_BOOLEAN  CEA-708 only. Optional (defaults to false).
                 *                                      Whether the caption service is a format
                 *                                      that is for easy reading.
                 * - "wide-aspect-ratio" G_TYPE_BOOLEAN CEA-708 only. Optional (defaults to false).
                 *                                      Whether the caption service is authored for
                 *                                      a wide aspect ratio (>=16:9).
                 */
                gst::ParamSpecArray::builder("services")
                .nick("Services")
                .blurb("List of service information to add or update in the CDP")
                .element_spec(&glib::ParamSpecBoxed::builder::<gst::Structure>("service").build())
                .mutable_playing()
                .build()]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "services" => {
                let s = value.get().expect("type checked upstream");
                let service_descriptor = service_descriptor_from_array(&s);
                let mut state = self.state.lock().unwrap();
                let mut settings = self.settings.lock().unwrap();
                settings.service_descriptor = s;
                state.service_descriptor = service_descriptor;
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "services" => {
                let settings = self.settings.lock().unwrap();
                settings.service_descriptor.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for CdpServiceInject {}

impl ElementImpl for CdpServiceInject {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "CDP Service Injection",
                "Filter/ClosedCaption",
                "Adds or updates CDP Service Description Information",
                "Matthew Waters <matthew@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("closedcaption/x-cea-708")
                    .field("format", "cdp")
                    .build(),
            )
            .unwrap();

            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &gst::Caps::builder("closedcaption/x-cea-708")
                    .field("format", "cdp")
                    .build(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    #[allow(clippy::single_match)]
    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst::trace!(CAT, imp = self, "Changing state {:?}", transition);

        match transition {
            gst::StateChange::ReadyToPaused => {
                let mut state = self.state.lock().unwrap();
                let settings = self.settings.lock().unwrap();
                *state = State::default();
                state.service_descriptor =
                    service_descriptor_from_array(&settings.service_descriptor);
            }
            _ => (),
        }

        let ret = self.parent_change_state(transition)?;

        match transition {
            gst::StateChange::PausedToReady => {
                let mut state = self.state.lock().unwrap();
                let settings = self.settings.lock().unwrap();
                *state = State::default();
                state.service_descriptor =
                    service_descriptor_from_array(&settings.service_descriptor);
            }
            _ => (),
        }

        Ok(ret)
    }
}

impl BaseTransformImpl for CdpServiceInject {
    const MODE: gst_base::subclass::BaseTransformMode =
        gst_base::subclass::BaseTransformMode::AlwaysInPlace;
    const TRANSFORM_IP_ON_PASSTHROUGH: bool = true;
    const PASSTHROUGH_ON_SAME_CAPS: bool = false;

    fn transform_caps(
        &self,
        _direction: gst::PadDirection,
        caps: &gst::Caps,
        filter: Option<&gst::Caps>,
    ) -> Option<gst::Caps> {
        if let Some(filter) = filter {
            Some(caps.intersect(filter))
        } else {
            Some(caps.clone())
        }
    }

    #[allow(clippy::single_match)]
    fn sink_event(&self, event: gst::Event) -> bool {
        use gst::EventView;

        gst::log!(CAT, imp = self, "Handling sink event {:?}", event);
        match event.view() {
            EventView::FlushStop(_fs) => {
                let mut state = self.state.lock().unwrap();
                state.cdp_parser.flush();
                state.cdp_writer.flush();
                state.pending_service_descriptor.clear();
            }
            _ => (),
        }

        self.parent_sink_event(event)
    }

    fn propose_allocation(
        &self,
        _decide_query: Option<&gst::query::Allocation>,
        _query: &mut gst::query::Allocation,
    ) -> Result<(), gst::LoggableError> {
        Ok(())
    }

    fn transform_ip(&self, buf: &mut gst::BufferRef) -> Result<gst::FlowSuccess, gst::FlowError> {
        let Ok(mapped) = buf.map_writable() else {
            gst::element_imp_error!(
                self,
                gst::ResourceError::Write,
                ["Could not map buffer writable"]
            );
            return Err(gst::FlowError::Error);
        };

        let mut state = self.state.lock().unwrap();
        let State {
            ref mut cdp_parser,
            ref mut cdp_writer,
            ref mut pending_service_descriptor,
            ref mut service_descriptor,
            ..
        } = &mut *state;

        if let Err(e) = cdp_parser.parse(&mapped) {
            gst::element_imp_error!(
                self,
                gst::ResourceError::Write,
                ("Failed to parse incoming CDP"),
                ["err: {e:?}"]
            );
            return Err(gst::FlowError::Error);
        }

        let Some(framerate) = cdp_parser.framerate() else {
            return Ok(gst::FlowSuccess::Ok);
        };

        while let Some(packet) = cdp_parser.pop_packet() {
            cdp_writer.push_packet(packet);
        }

        if let Some(cea608) = cdp_parser.cea608() {
            for pair in cea608 {
                cdp_writer.push_cea608(*pair);
            }
        }

        let time_code = cdp_parser.time_code();
        cdp_writer.set_time_code(time_code);
        let sequence_no = cdp_parser.sequence();
        cdp_writer.set_sequence_count(sequence_no);

        let existing_service_info = cdp_parser.service_info();
        gst::debug!(
            CAT,
            imp = self,
            "Existing service info {existing_service_info:?}"
        );
        let service_info = if !pending_service_descriptor.is_empty() {
            let mut service_info = cdp_types::ServiceInfo::default();
            let mut complete = true;
            while let Some(entry) = pending_service_descriptor.pop_front() {
                if let Err(cdp_types::WriterError::WouldOverflow(_)) =
                    service_info.add_service(entry)
                {
                    pending_service_descriptor.push_front(entry);
                    complete = false;
                    break;
                }
            }
            service_info.set_complete(complete);
            Some(service_info)
        } else if !service_descriptor.is_empty() {
            let mut service_info = cdp_types::ServiceInfo::default();
            service_info.set_start(true);
            let mut overflowed = vec![];
            for entry in service_descriptor.iter() {
                if !overflowed.is_empty() {
                    overflowed.push(*entry);
                } else if let Err(cdp_types::WriterError::WouldOverflow(_)) =
                    service_info.add_service(*entry)
                {
                    overflowed.push(*entry);
                }
            }

            if overflowed.is_empty() {
                service_info.set_complete(true);
            } else {
                pending_service_descriptor.extend(overflowed);
            }
            Some(service_info)
        } else {
            None
        };
        // TODO: handle other service update mechanisms. e.g.
        // replace/replace-existing/replace-missing.

        gst::debug!(CAT, imp = self, "Set new service info {service_info:?}");
        cdp_writer.set_service_info(service_info);

        // TODO: handle other CDP unknown sections.

        let mut output = vec![];
        if let Err(e) = cdp_writer.write(framerate, &mut output) {
            gst::element_imp_error!(
                self,
                gst::ResourceError::Write,
                ("Failed to write outgoing CDP"),
                ["err: {e:?}"]
            );
            return Err(gst::FlowError::Error);
        }
        drop(mapped);

        let mem = gst::Memory::from_mut_slice(output);
        buf.replace_all_memory(mem);

        Ok(gst::FlowSuccess::Ok)
    }
}
