// Copyright 2019 Jordan Petridis <jordan@centricular.com>
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to
// deal in the Software without restriction, including without limitation the
// rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
// sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
// IN THE SOFTWARE.
//
// SPDX-License-Identifier: MIT
#![allow(clippy::non_send_fields_in_send_ty, unused_doc_comments)]

/**
 * plugin-sodium:
 *
 * Since: plugins-rs-0.5.0
 */
use gst::glib;

const TYPEFIND_HEADER: &[u8; 12] = b"gst-sodium10";
// `core::slice::<impl [T]>::len` is not yet stable as a const fn
// const TYPEFIND_HEADER_SIZE: usize = TYPEFIND_HEADER.len();
const TYPEFIND_HEADER_SIZE: usize = 12;
/// Encryted steams use an offset at the start to store the block_size
/// and the nonce that was used.
const HEADERS_SIZE: usize =
    TYPEFIND_HEADER_SIZE + sodiumoxide::crypto::box_::NONCEBYTES + std::mem::size_of::<u32>();

mod decrypter;
mod encrypter;

fn typefind_register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    use gst::{Caps, TypeFind, TypeFindProbability};

    TypeFind::register(
        Some(plugin),
        "sodium_encrypted_typefind",
        gst::Rank::NONE,
        None,
        Some(&Caps::builder("application/x-sodium-encrypted").build()),
        |typefind| {
            if let Some(data) = typefind.peek(0, TYPEFIND_HEADER_SIZE as u32) {
                if data == TYPEFIND_HEADER {
                    typefind.suggest(
                        TypeFindProbability::Maximum,
                        &Caps::builder("application/x-sodium-encrypted").build(),
                    );
                }
            }
        },
    )
}

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    encrypter::register(plugin)?;
    decrypter::register(plugin)?;
    typefind_register(plugin)?;
    Ok(())
}

gst::plugin_define!(
    sodium,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "MIT/X11",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);
