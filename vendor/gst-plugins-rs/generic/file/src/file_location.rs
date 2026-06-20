// Copyright (C) 2016-2017 Sebastian Dröge <sebastian@centricular.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: MIT OR Apache-2.0
use gst::glib;
use url::Url;

use std::fmt;
use std::ops::Deref;
use std::path::{Path, PathBuf};

#[cfg(target_os = "windows")]
const WIN_EXT_PATH_PREFIX: &str = "\\\\?\\";
#[cfg(target_os = "windows")]
const WIN_EXT_PATH_PREFIX_LEN: usize = 4;

#[derive(Debug)]
pub(super) struct FileLocation(PathBuf);

impl FileLocation {
    pub(super) fn try_from_path_str(path_str: String) -> Result<Self, glib::Error> {
        FileLocation::try_from(PathBuf::from(path_str))
    }

    pub(super) fn try_from_uri_str(uri_str: &str) -> Result<Self, glib::Error> {
        match Url::parse(uri_str) {
            Ok(url) => {
                if url.scheme() != "file" {
                    return Err(glib::Error::new(
                        gst::URIError::UnsupportedProtocol,
                        format!("Unsupported URI {uri_str}").as_str(),
                    ));
                }

                let path = url.to_file_path().map_err(|_| {
                    glib::Error::new(
                        gst::URIError::BadUri,
                        format!("Unsupported URI {uri_str}").as_str(),
                    )
                })?;

                FileLocation::try_from(path)
            }
            Err(err) => Err(glib::Error::new(
                gst::URIError::BadUri,
                format!("Couldn't parse URI {uri_str}: {err}").as_str(),
            )),
        }
    }

    fn try_from(location: PathBuf) -> Result<Self, glib::Error> {
        let location_str = location.to_str().ok_or_else(|| {
            glib::Error::new(
                gst::URIError::BadReference,
                format!("Invalid path {location:?}").as_str(),
            )
        })?;

        let file_name = location.file_name().ok_or_else(|| {
            glib::Error::new(
                gst::URIError::BadReference,
                format!("Expected a path with a filename, got {location_str}",).as_str(),
            )
        })?;

        // The filename might not exist yet, so check the parent only.
        // Note: `location` contains a filename, so its parent can't be `None`
        let mut parent_dir = location
            .parent()
            .expect("FileSink::set_location `location` with filename but without a parent")
            .to_owned();
        if parent_dir.is_relative() && parent_dir.components().next().is_none() {
            // `location` only contains the filename
            // need to specify "." for `canonicalize` to resolve the actual path
            parent_dir = PathBuf::from(".");
        }

        let parent_canonical = parent_dir.canonicalize().map_err(|err| {
            glib::Error::new(
                gst::URIError::BadReference,
                format!("Could not resolve path {location_str}: {err}",).as_str(),
            )
        })?;

        #[cfg(target_os = "windows")]
        let parent_canonical = {
            let has_prefix = parent_canonical
                .to_str()
                .unwrap() // already checked above
                .starts_with(WIN_EXT_PATH_PREFIX);
            if has_prefix {
                // Remove the "extended length path" prefix
                // for compatibility with applications which can't deal with it.
                // See https://doc.rust-lang.org/std/fs/fn.canonicalize.html
                let parent_canonical_str = parent_canonical.to_str().unwrap();
                PathBuf::from(&parent_canonical_str[WIN_EXT_PATH_PREFIX_LEN..])
            } else {
                parent_canonical
            }
        };

        let location_canonical = parent_canonical.join(file_name);
        Url::from_file_path(&location_canonical)
            .map_err(|_| {
                glib::Error::new(
                    gst::URIError::BadReference,
                    format!("Could not resolve path to URL {location_str}").as_str(),
                )
            })
            .map(|_| FileLocation(location_canonical))
    }

    fn to_str(&self) -> &str {
        self.0
            .to_str()
            .expect("FileLocation: couldn't get `&str` from internal `PathBuf`")
    }
}

impl AsRef<Path> for FileLocation {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl Deref for FileLocation {
    type Target = Path;

    fn deref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl fmt::Display for FileLocation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_str())
    }
}
