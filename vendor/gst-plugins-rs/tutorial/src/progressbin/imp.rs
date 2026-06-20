// Copyright (C) 2019 Sebastian Dröge <sebastian@centricular.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use std::sync::Mutex;

use std::sync::LazyLock;

use super::ProgressBinOutput;

// This module contains the private implementation details of our element

const DEFAULT_OUTPUT_TYPE: ProgressBinOutput = ProgressBinOutput::Println;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rsprogressbin",
        gst::DebugColorFlags::empty(),
        Some("Rust Progress Reporter"),
    )
});

// Struct containing all the element data
pub struct ProgressBin {
    progress: gst::Element,
    srcpad: gst::GhostPad,
    sinkpad: gst::GhostPad,
    // We put the output_type property behind a mutex, as we want
    // change it in the set_property function, which can be called
    // from any thread.
    output_type: Mutex<ProgressBinOutput>,
}

// This trait registers our type with the GObject object system and
// provides the entry points for creating a new instance and setting
// up the class data
#[glib::object_subclass]
impl ObjectSubclass for ProgressBin {
    const NAME: &'static str = "GstRsProgressBin";
    type Type = super::ProgressBin;
    type ParentType = gst::Bin;

    // Called when a new instance is to be created. We need to return an instance
    // of our struct here and also get the class struct passed in case it's needed
    fn with_class(klass: &Self::Class) -> Self {
        // Create our two ghostpads from the templates that were registered with
        // the class. We don't provide a target for them yet because we can only
        // do so after the progressreport element was added to the bin.
        //
        // We do that and adding the pads inside glib::Object::constructed() later.
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::GhostPad::from_template(&templ);
        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::GhostPad::from_template(&templ);

        // Create the progressreport element.
        let progress = gst::ElementFactory::make("progressreport")
            .name("progress")
            // Don't let progressreport print to stdout itself
            .property("silent", true)
            .build()
            .unwrap();

        // Return an instance of our struct
        Self {
            progress,
            srcpad,
            sinkpad,
            output_type: Mutex::new(ProgressBinOutput::Println),
        }
    }
}

// Implementation of glib::Object virtual methods
impl ObjectImpl for ProgressBin {
    // Metadata for the element's properties
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecEnum::builder_with_default("output", DEFAULT_OUTPUT_TYPE)
                    .nick("Output")
                    .blurb("Defines the output type of the progressbin")
                    .mutable_playing()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    // Called whenever a value of a property is changed. It can be called
    // at any time from any thread.
    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "output" => {
                let mut output_type = self.output_type.lock().unwrap();
                let new_output_type = value
                    .get::<ProgressBinOutput>()
                    .expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing output from {:?} to {:?}",
                    output_type,
                    new_output_type
                );
                *output_type = new_output_type;
            }
            _ => unimplemented!(),
        }
    }

    // Called whenever a value of a property is read. It can be called
    // at any time from any thread.
    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "output" => {
                let output_type = self.output_type.lock().unwrap();
                output_type.to_value()
            }
            _ => unimplemented!(),
        }
    }

    // Called right after construction of a new instance
    fn constructed(&self) {
        // Call the parent class' ::constructed() implementation first
        self.parent_constructed();

        // Here we actually add the pads we created in ProgressBin::new() to the
        // element so that GStreamer is aware of their existence.

        let obj = self.obj();

        // Add the progressreport element to the bin.
        obj.add(&self.progress).unwrap();

        // Then set the ghost pad targets to the corresponding pads of the progressreport element.
        self.sinkpad
            .set_target(Some(&self.progress.static_pad("sink").unwrap()))
            .unwrap();
        self.srcpad
            .set_target(Some(&self.progress.static_pad("src").unwrap()))
            .unwrap();

        // And finally add the two ghostpads to the bin.
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }
}

impl GstObjectImpl for ProgressBin {}

// Implementation of gst::Element virtual methods
impl ElementImpl for ProgressBin {
    // Set the element specific metadata. This information is what
    // is visible from gst-inspect-1.0 and can also be programmatically
    // retrieved from the gst::Registry after initial registration
    // without having to load the plugin in memory.
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "ProgressBin",
                "Generic",
                "Prints progress information to stdout",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }
    // Create and add pad templates for our sink and source pad. These
    // are later used for actually creating the pads and beforehand
    // already provide information to GStreamer about all possible
    // pads that could exist for this type.
    //
    // Actual instances can create pads based on those pad templates
    // with a subset of the caps given here.
    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            // Our element can accept any possible caps on both pads
            let caps = gst::Caps::new_any();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

// Implementation of gst::Bin virtual methods
impl BinImpl for ProgressBin {
    fn handle_message(&self, msg: gst::Message) {
        use gst::MessageView;

        match msg.view() {
            // If this is the progressreport message, we print the status
            // to stdout. Otherwise we pass through to the default message
            // handling of the parent class, i.e. forwarding to the parent
            // bins and the application.
            MessageView::Element(msg)
                if msg.src() == Some(self.progress.upcast_ref())
                    && msg
                        .structure()
                        .map(|s| s.name() == "progress")
                        .unwrap_or(false) =>
            {
                let s = msg.structure().unwrap();
                if let Ok(percent) = s.get::<f64>("percent-double") {
                    let output_type = *self.output_type.lock().unwrap();
                    match output_type {
                        ProgressBinOutput::Println => println!("progress: {percent:5.1}%"),
                        ProgressBinOutput::DebugCategory => {
                            gst::info!(CAT, imp = self, "progress: {:5.1}%", percent);
                        }
                    };
                }
            }
            _ => self.parent_handle_message(msg),
        }
    }
}
