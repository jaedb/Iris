// Copyright (C) 2024 Sebastian Dröge <sebastian@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
 * SECTION:element-st2038anctocc
 *
 * Extracts closed captions (CEA-608 and/or CEA-708) from SMPTE ST-2038 ancillary streams
 * outputs them on the respective sometimes source pad (`src_cea608` or `src_cea708`). The
 * data is output as a closed caption stream with caps `closedcaption/x-cea-608,format=s334-1a`
 * or `closedcaption/x-cea-708,format=cdp` for further processing by other GStreamer closed
 * caption processing elements.
 *
 * Since: plugins-rs-0.14.0
 */
use std::{mem, sync::LazyLock};

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;

use atomic_refcell::AtomicRefCell;

use crate::st2038anc_utils::AncDataHeader;

#[derive(Default)]
struct State {
    cea608_srcpad: Option<gst::Pad>,
    cea708_srcpad: Option<gst::Pad>,
    flow_combiner: gst_base::UniqueFlowCombiner,
    framerate: Option<gst::Fraction>,
}

pub struct St2038AncToCc {
    sinkpad: gst::Pad,

    state: AtomicRefCell<State>,
}

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "st2038anctocc",
        gst::DebugColorFlags::empty(),
        Some("ST-2038 ANC to Closed Caption Element"),
    )
});

impl St2038AncToCc {
    fn sink_chain(
        &self,
        pad: &gst::Pad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::trace!(CAT, obj = pad, "Handling buffer {:?}", buffer);

        let mut state = self.state.borrow_mut();

        let map = buffer.map_readable().map_err(|_| {
            gst::error!(CAT, obj = pad, "Can't map buffer readable");

            gst::FlowError::Error
        })?;

        let mut slice = map.as_slice();
        while !slice.is_empty() {
            // Stop on stuffing bytes
            if slice[0] == 0b1111_1111 {
                break;
            }

            let header = match AncDataHeader::from_slice(slice) {
                Ok(anc_hdr) => anc_hdr,
                Err(err) => {
                    gst::debug!(
                        CAT,
                        imp = self,
                        "Failed to parse ancillary data header: {err:?}"
                    );
                    break;
                }
            };

            gst::trace!(CAT, imp = self, "Parsed ST2038 header {header:?}");

            match (header.did, header.sdid) {
                // S334 EIA 708 | S334 EIA 608
                (0x61, 0x01) | (0x61, 0x02) => {
                    if (header.sdid == 0x01 && state.cea708_srcpad.is_none())
                        || (header.sdid == 0x02 && state.cea608_srcpad.is_none())
                    {
                        gst::debug!(
                            CAT,
                            imp = self,
                            "Adding {} source pad",
                            if header.sdid == 0x01 {
                                "CEA-708"
                            } else {
                                "CEA-608"
                            },
                        );

                        let templ = self
                            .obj()
                            .pad_template(if header.sdid == 0x01 {
                                "src_cea708"
                            } else {
                                "src_cea608"
                            })
                            .unwrap();
                        let srcpad = gst::Pad::builder_from_template(&templ).build();
                        let _ = srcpad.set_active(true);

                        self.sinkpad.sticky_events_foreach(|event| {
                            if event.type_() != gst::EventType::Caps {
                                let _ = srcpad.store_sticky_event(event);
                            } else {
                                let src_caps = {
                                    let mut caps = srcpad.pad_template_caps();
                                    let caps_ref = caps.make_mut();
                                    if let Some(framerate) = state.framerate {
                                        caps_ref.set("framerate", framerate);
                                    }
                                    caps
                                };

                                let event = gst::event::Caps::builder(&src_caps)
                                    .seqnum(event.seqnum())
                                    .build();
                                let _ = srcpad.store_sticky_event(&event);
                            }
                            std::ops::ControlFlow::Continue(gst::EventForeachAction::Keep)
                        });

                        drop(state);
                        self.obj().add_pad(&srcpad).unwrap();
                        state = self.state.borrow_mut();

                        state.flow_combiner.add_pad(&srcpad);

                        if header.sdid == 0x01 {
                            state.cea708_srcpad = Some(srcpad);
                        } else {
                            state.cea608_srcpad = Some(srcpad);
                        }
                    }

                    let srcpad = if header.sdid == 0x01 {
                        state.cea708_srcpad.as_ref().unwrap().clone()
                    } else {
                        state.cea608_srcpad.as_ref().unwrap().clone()
                    };

                    // Create sub-buffer of the payload of the packet
                    let mut sub_buffer =
                        gst::Buffer::with_size(header.data_count as usize).unwrap();

                    gst::trace!(
                        CAT,
                        imp = self,
                        "Outputting {} buffer of size {}",
                        if header.sdid == 0x01 {
                            "CEA-708"
                        } else {
                            "CEA-608"
                        },
                        header.data_count,
                    );

                    {
                        let sub_buffer = sub_buffer.get_mut().unwrap();
                        let _ = buffer.copy_into(sub_buffer, gst::BUFFER_COPY_METADATA, ..);

                        let mut sub_map = sub_buffer.map_writable().unwrap();
                        let mut sub_slice = sub_map.as_mut_slice();

                        use bitstream_io::{BigEndian, BitRead, BitReader};
                        use std::io::Cursor;

                        let mut r = BitReader::endian(Cursor::new(slice), BigEndian);

                        // Skip header portion
                        r.skip(6 + 1 + 11 + 12 + 10 + 10 + 10).unwrap();

                        // Convert data from 10 bits to 8 bits
                        for _ in 0..header.data_count {
                            let b = (r.read::<10, u16>().unwrap() & 0xff) as u8;
                            sub_slice[0] = b;
                            sub_slice = &mut sub_slice[1..];
                        }
                    }

                    drop(state);
                    let res = srcpad.push(sub_buffer);
                    state = self.state.borrow_mut();
                    state.flow_combiner.update_pad_flow(&srcpad, res)?;
                }
                _ => {
                    gst::trace!(
                        CAT,
                        imp = self,
                        "Skipping ANC with DID {} and SDID {}",
                        header.did,
                        header.sdid
                    );
                }
            }

            slice = &slice[header.len..];
        }

        Ok(gst::FlowSuccess::Ok)
    }

    #[allow(clippy::single_match)]
    fn sink_event(&self, pad: &gst::Pad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::log!(CAT, obj = pad, "Handling event {:?}", event);
        match event.view() {
            EventView::Caps(ev) => {
                let mut state = self.state.borrow_mut();
                let s = ev.caps().structure(0).unwrap();
                state.framerate = s.get::<gst::Fraction>("framerate").ok();

                for srcpad in [state.cea608_srcpad.as_ref(), state.cea708_srcpad.as_ref()]
                    .iter()
                    .flatten()
                {
                    let mut caps = srcpad.pad_template_caps();
                    if let Some(framerate) = state.framerate {
                        caps.make_mut().set("framerate", framerate);
                    }

                    let event = gst::event::Caps::builder(&caps)
                        .seqnum(event.seqnum())
                        .build();
                    let _ = srcpad.store_sticky_event(&event);
                }

                // Drop caps events
                return true;
            }
            _ => (),
        }

        gst::Pad::event_default(pad, Some(&*self.obj()), event)
    }
}

#[glib::object_subclass]
impl ObjectSubclass for St2038AncToCc {
    const NAME: &'static str = "GstSt2038AncToCc";
    type Type = super::St2038AncToCc;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_from_template(&templ)
            .chain_function(|pad, parent, buffer| {
                St2038AncToCc::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |this| this.sink_chain(pad, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                St2038AncToCc::catch_panic_pad_function(
                    parent,
                    || false,
                    |this| this.sink_event(pad, event),
                )
            })
            .flags(gst::PadFlags::FIXED_CAPS)
            .build();

        Self {
            sinkpad,
            state: AtomicRefCell::new(State::default()),
        }
    }
}

impl ObjectImpl for St2038AncToCc {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.sinkpad).unwrap();
    }
}

impl GstObjectImpl for St2038AncToCc {}

impl ElementImpl for St2038AncToCc {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "ST-2038 ANC to CC",
                "Generic",
                "Converts ST-2038 ANC to Closed Captions",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let src_cea608_pad_template = gst::PadTemplate::new(
                "src_cea608",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &gst::Caps::builder("closedcaption/x-cea-608")
                    .field("format", "s334-1a")
                    .build(),
            )
            .unwrap();

            let src_cea708_pad_template = gst::PadTemplate::new(
                "src_cea708",
                gst::PadDirection::Src,
                gst::PadPresence::Sometimes,
                &gst::Caps::builder("closedcaption/x-cea-708")
                    .field("format", "cdp")
                    .build(),
            )
            .unwrap();

            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &gst::Caps::builder("meta/x-st-2038").build(),
            )
            .unwrap();

            vec![
                src_cea608_pad_template,
                src_cea708_pad_template,
                sink_pad_template,
            ]
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
                *self.state.borrow_mut() = State::default();
            }
            _ => (),
        }

        let ret = self.parent_change_state(transition)?;

        match transition {
            gst::StateChange::PausedToReady => {
                let old_state = mem::take(&mut *self.state.borrow_mut());
                for pad in [
                    old_state.cea608_srcpad.as_ref(),
                    old_state.cea708_srcpad.as_ref(),
                ]
                .into_iter()
                .flatten()
                {
                    let _ = self.obj().remove_pad(pad);
                }
            }
            _ => (),
        }

        Ok(ret)
    }
}
