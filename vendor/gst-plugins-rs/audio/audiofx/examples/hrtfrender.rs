// Copyright (C) 2021 Tomasz Andrzejak <andreiltd@gmail.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::prelude::*;

use anyhow::{bail, Error};

use std::sync::{Arc, Condvar, Mutex};
use std::{env, thread, time};

// Rotation in radians to apply to object position every 100 ms
const ROTATION: f32 = 2.0 / 180.0 * std::f32::consts::PI;

fn run() -> Result<(), Error> {
    gst::init()?;
    gstrsaudiofx::plugin_register_static()?;

    let args: Vec<String> = env::args().collect();

    // Ircam binaries that the hrtf plugin is using can be downloaded from:
    // https://github.com/mrDIMAS/hrir_sphere_builder/tree/master/hrtf_base/IRCAM
    //
    // Ideally provide a mono input as this example is moving a single object. Otherwise
    // the input will be downmixed to mono with the audioconvert element.
    //
    // e.g.: hrtfrender 'file:///path/to/my/awesome/mono/wav/awesome.wav' IRC_1002_C.bin
    if args.len() != 3 {
        bail!("Usage: {} URI HRIR", args[0].clone());
    }

    let uri = &args[1];
    let hrir = &args[2];

    let pipeline = gst::parse::launch(&format!(
        "uridecodebin uri={uri} ! audioconvert ! audio/x-raw,channels=1 !
            hrtfrender hrir-file={hrir} name=hrtf ! audioresample ! autoaudiosink"
    ))?
    .downcast::<gst::Pipeline>()
    .expect("type error");

    let hrtf = pipeline.by_name("hrtf").expect("hrtf element not found");

    // At the beginning put an object in front of listener
    let objs = [gst::Structure::builder("application/spatial-object")
        .field("x", 0f32)
        .field("y", 0f32)
        .field("z", 1f32)
        .field("distance-gain", 1f32)
        .build()];

    hrtf.set_property("spatial-objects", gst::Array::new(objs));

    let state_cond = Arc::new((Mutex::new(gst::State::Null), Condvar::new()));
    let state_cond_clone = Arc::clone(&state_cond);

    thread::spawn(move || {
        // Wait for the pipeline to start up
        {
            let (lock, cvar) = &*state_cond_clone;
            let mut state = lock.lock().unwrap();

            while *state != gst::State::Playing {
                state = cvar.wait(state).unwrap();
            }
        }

        loop {
            // get current object position and rotate it clockwise
            let s = hrtf.property::<gst::Array>("spatial-objects")[0]
                .get::<gst::Structure>()
                .expect("type error");

            // positive values are on the right side of a listener
            let x = s.get::<f32>("x").expect("type error");
            // elevation, positive value is up
            let y = s.get::<f32>("y").expect("type error");
            // positive values are in front of a listener
            let z = s.get::<f32>("z").expect("type error");
            // gain
            let gain = s.get::<f32>("distance-gain").expect("type error");

            // rotate clockwise: https://en.wikipedia.org/wiki/Rotation_matrix
            let new_x = x * f32::cos(ROTATION) + z * f32::sin(ROTATION);
            let new_z = -x * f32::sin(ROTATION) + z * f32::cos(ROTATION);

            let objs = [gst::Structure::builder("application/spatial-object")
                .field("x", new_x)
                .field("y", y)
                .field("z", new_z)
                .field("distance-gain", gain)
                .build()];

            hrtf.set_property("spatial-objects", gst::Array::new(objs));

            thread::sleep(time::Duration::from_millis(100));
        }
    });

    pipeline.set_state(gst::State::Playing)?;

    let bus = pipeline.bus().unwrap();
    for msg in bus.iter_timed(gst::ClockTime::NONE) {
        use gst::MessageView;

        match msg.view() {
            MessageView::StateChanged(state_changed) => {
                if state_changed.src().map(|s| s == &pipeline).unwrap_or(false)
                    && state_changed.current() == gst::State::Playing
                {
                    let (lock, cvar) = &*state_cond;
                    let mut state = lock.lock().unwrap();

                    *state = gst::State::Playing;
                    cvar.notify_one();
                }
            }
            MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                println!(
                    "Error from {:?}: {} ({:?})",
                    msg.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
                break;
            }
            _ => (),
        }
    }

    pipeline.set_state(gst::State::Null)?;

    Ok(())
}

fn main() {
    match run() {
        Ok(r) => r,
        Err(e) => eprintln!("Error! {e}"),
    }
}
