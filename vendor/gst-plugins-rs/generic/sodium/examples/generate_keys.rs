// generate_keys.rs
//
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

use clap::Parser;
use serde::{Deserialize, Serialize};
use sodiumoxide::crypto::box_;
use std::fs::File;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[clap(
    version,
    author,
    about = "Generate a pair of Sodium's crypto_box_curve25519xsalsa20poly1305 keys."
)]
struct Args {
    /// Path to write the Keys
    #[clap(short, long)]
    path: PathBuf,

    /// Write a JSON file instead of a key.prv/key.pub pair
    #[clap(short, long)]
    json: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Keys {
    public: box_::PublicKey,
    private: box_::SecretKey,
}

impl Keys {
    fn new() -> Self {
        let (public, private) = box_::gen_keypair();
        Keys { public, private }
    }

    fn write_to_file(&self, mut path: PathBuf, json: bool) {
        if json {
            if !path.ends_with(".json") {
                path.set_extension("json");
            }

            let file = File::create(&path)
                .unwrap_or_else(|_| panic!("Failed to create file at {}", path.display()));
            serde_json::to_writer(file, &self)
                .unwrap_or_else(|_| panic!("Failed to write to file at {}", path.display()));
        } else {
            use std::io::Write;

            let mut private = path.clone();
            private.set_extension("prv");
            let mut file = File::create(&private)
                .unwrap_or_else(|_| panic!("Failed to create file at {}", private.display()));
            file.write_all(&self.private.0)
                .unwrap_or_else(|_| panic!("Failed to write to file at {}", private.display()));

            let mut public = path.clone();
            public.set_extension("pub");
            let mut file = File::create(&public)
                .unwrap_or_else(|_| panic!("Failed to create file at {}", public.display()));
            file.write_all(self.public.as_ref())
                .unwrap_or_else(|_| panic!("Failed to write to file at {}", public.display()));
        }
    }
}

fn main() {
    let args = Args::parse();

    let keys = Keys::new();

    keys.write_to_file(args.path, args.json);
}
