// Copyright (C) 2018 LEE Dongjun <redongjun@gmail.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use std::io::Write;
use std::sync::{Arc, Mutex};
use std::{net, thread, time};

fn main() {
    let sockets: Arc<Mutex<Vec<net::TcpStream>>> = Arc::new(Mutex::new(vec![]));

    let streams = sockets.clone();
    let _handler = thread::spawn(move || {
        let listener = net::TcpListener::bind("0.0.0.0:40000").unwrap();
        for stream in listener.incoming() {
            streams.lock().unwrap().push(stream.unwrap());
        }
    });

    let buffer = [0; 160];
    let wait = time::Duration::from_millis(20);

    loop {
        let now = time::Instant::now();

        let sockets = sockets.lock().unwrap();
        for mut socket in sockets.iter() {
            let _ = socket.write(&buffer);
        }

        let elapsed = now.elapsed();
        if elapsed < wait {
            thread::sleep(wait - elapsed);
        }
    }
}
