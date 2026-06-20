// Copyright (C) 2016-2017 Sebastian Dröge <sebastian@centricular.com>
//               2016 Luis de Bethencourt <luisbg@osg.samsung.com>
//               2018 François Laignel <fengalin@free.fr>
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
use gst_base::prelude::*;
use gst_base::subclass::prelude::*;

use std::fs::File;
use std::io::Write;
use std::sync::Mutex;

use url::Url;

use crate::file_location::FileLocation;

const DEFAULT_LOCATION: Option<FileLocation> = None;

#[derive(Debug)]
struct Settings {
    location: Option<FileLocation>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            location: DEFAULT_LOCATION,
        }
    }
}

#[derive(Default)]
enum State {
    #[default]
    Stopped,
    Started {
        file: File,
        position: u64,
    },
}

#[derive(Default)]
pub struct FileSink {
    settings: Mutex<Settings>,
    state: Mutex<State>,
}

use std::sync::LazyLock;
static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rsfilesink",
        gst::DebugColorFlags::empty(),
        Some("File Sink"),
    )
});

impl FileSink {
    fn set_location(&self, location: Option<FileLocation>) -> Result<(), glib::Error> {
        let state = self.state.lock().unwrap();
        if let State::Started { .. } = *state {
            return Err(glib::Error::new(
                gst::URIError::BadState,
                "Changing the `location` property on a started `filesink` is not supported",
            ));
        }

        let mut settings = self.settings.lock().unwrap();
        settings.location = match location {
            Some(location) => {
                match settings.location {
                    Some(ref location_cur) => {
                        gst::info!(
                            CAT,
                            imp = self,
                            "Changing `location` from {:?} to {}",
                            location_cur,
                            location,
                        );
                    }
                    None => {
                        gst::info!(CAT, imp = self, "Setting `location` to {}", location,);
                    }
                }
                Some(location)
            }
            None => {
                gst::info!(CAT, imp = self, "Resetting `location` to None",);
                None
            }
        };

        Ok(())
    }
}

#[glib::object_subclass]
impl ObjectSubclass for FileSink {
    const NAME: &'static str = "GstRsFileSink";
    type Type = super::FileSink;
    type ParentType = gst_base::BaseSink;
    type Interfaces = (gst::URIHandler,);
}

impl ObjectImpl for FileSink {
    fn constructed(&self) {
        self.parent_constructed();

        self.obj().set_sync(false);
    }

    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![glib::ParamSpecString::builder("location")
                .nick("File Location")
                .blurb("Location of the file to write")
                .build()]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "location" => {
                let res = match value.get::<Option<String>>() {
                    Ok(Some(location)) => FileLocation::try_from_path_str(location)
                        .and_then(|file_location| self.set_location(Some(file_location))),
                    Ok(None) => self.set_location(None),
                    Err(_) => unreachable!("type checked upstream"),
                };

                if let Err(err) = res {
                    gst::error!(
                        CAT,
                        imp = self,
                        "Failed to set property `location`: {}",
                        err
                    );
                }
            }
            _ => unimplemented!(),
        };
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "location" => {
                let settings = self.settings.lock().unwrap();
                let location = settings
                    .location
                    .as_ref()
                    .map(|location| location.to_string());

                location.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for FileSink {}

impl ElementImpl for FileSink {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "File Sink",
                "Sink/File",
                "Write stream to a file",
                "François Laignel <fengalin@free.fr>, Luis de Bethencourt <luisbg@osg.samsung.com>",
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

impl BaseSinkImpl for FileSink {
    fn start(&self) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.lock().unwrap();
        if let State::Started { .. } = *state {
            unreachable!("FileSink already started");
        }

        let settings = self.settings.lock().unwrap();
        let location = settings.location.as_ref().ok_or_else(|| {
            gst::error_msg!(
                gst::ResourceError::Settings,
                ["File location is not defined"]
            )
        })?;

        let file = File::create(location).map_err(|err| {
            gst::error_msg!(
                gst::ResourceError::OpenWrite,
                [
                    "Could not open file {} for writing: {}",
                    location,
                    err.to_string(),
                ]
            )
        })?;
        gst::debug!(CAT, imp = self, "Opened file {:?}", file);

        *state = State::Started { file, position: 0 };
        gst::info!(CAT, imp = self, "Started");

        Ok(())
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.lock().unwrap();
        if let State::Stopped = *state {
            return Err(gst::error_msg!(
                gst::ResourceError::Settings,
                ["FileSink not started"]
            ));
        }

        *state = State::Stopped;
        gst::info!(CAT, imp = self, "Stopped");

        Ok(())
    }

    // TODO: implement seek in BYTES format

    fn render(&self, buffer: &gst::Buffer) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.lock().unwrap();
        let (file, position) = match *state {
            State::Started {
                ref mut file,
                ref mut position,
            } => (file, position),
            State::Stopped => {
                gst::element_imp_error!(self, gst::CoreError::Failed, ["Not started yet"]);
                return Err(gst::FlowError::Error);
            }
        };

        gst::trace!(CAT, imp = self, "Rendering {:?}", buffer);
        let map = buffer.map_readable().map_err(|_| {
            gst::element_imp_error!(self, gst::CoreError::Failed, ["Failed to map buffer"]);
            gst::FlowError::Error
        })?;

        file.write_all(map.as_ref()).map_err(|err| {
            gst::element_imp_error!(
                self,
                gst::ResourceError::Write,
                ["Failed to write buffer: {}", err]
            );
            gst::FlowError::Error
        })?;

        *position += map.len() as u64;

        Ok(gst::FlowSuccess::Ok)
    }
}

impl URIHandlerImpl for FileSink {
    const URI_TYPE: gst::URIType = gst::URIType::Sink;

    fn protocols() -> &'static [&'static str] {
        &["file"]
    }

    fn uri(&self) -> Option<String> {
        let settings = self.settings.lock().unwrap();

        // Conversion to Url already checked while building the `FileLocation`
        settings.location.as_ref().map(|location| {
            Url::from_file_path(location)
                .expect("FileSink::get_uri couldn't build `Url` from `location`")
                .into()
        })
    }

    fn set_uri(&self, uri: &str) -> Result<(), glib::Error> {
        // Special case for "file://" as this is used by some applications to test
        // with `gst_element_make_from_uri` if there's an element that supports the URI protocol

        if uri != "file://" {
            let file_location = FileLocation::try_from_uri_str(uri)?;
            self.set_location(Some(file_location))
        } else {
            Ok(())
        }
    }
}
