// Copyright (C) 2022 Thibault Saunier <tsaunier@igalia.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

/**
* tracer-pcap-writer:
* @short_description: Dumps pad dataflow into a `.pcap` file.
*
* This tracer provides an easy way to save data flowing in a specified pad using
* the [`pcap`](https://wiki.wireshark.org/Development/LibpcapFileFormat) format.
*
* It can also wrap the dataflow inside fake IpV4/UDP headers so those pcap can
* then be reused with the #pcapparse element.
*
* It can be used to easily record/replay some parts of a WebRTC session without
* requiring to modify the GStreamer pipeline.
*
* ## Parameters:
*
* - `output-dir` (string, default: "tracer_pcaps"): The directory where to save `.pcap` files
* - `fake-protocol` (['udp', 'none'], default: "udp"): the fake headers to add to packets
*
* ## Example:
*
* ```console
* $ GST_TRACERS=pcap-writer(target-factory=rtph264pay) gst-launch-1.0 videotestsrc num-buffers=15 ! x264enc tune=zerolatency ! video/x-h264,profile=constrained-baseline ! rtph264pay seqnum-offset=0 ! rtph264depay ! fakesink
* ```
*
* Then this can be replayed with:
*
* ```console
* $ gst-launch-1.0 filesrc location="tracer_pcaps/pipeline0>rtph264pay0>src.pcap" ! pcapparse ! \
*   "application/x-rtp, media=(string)video, clock-rate=(int)90000, encoding-name=(string)H264, profile-level-id=(string)42c015, a-framerate=(string)30" ! \
*   rtph264depay ! avdec_h264 ! fakesink silent=false -v -m
* ```
*
* Since: plugins-rs-0.13.0
*/
use pcap_file::pcap::{self, RawPcapPacket};

use etherparse::PacketBuilder;
use gst::glib::Properties;
use std::collections::{HashMap, HashSet};
use std::fs::{create_dir_all, File};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use gst::glib;

use gst::prelude::*;
use gst::subclass::prelude::*;
use std::sync::LazyLock;

static MAX_PACKET_LEN: usize = 65535;
static MAX_FAKE_HEADERS_LEN: usize = 54;
static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "pcap-writer",
        gst::DebugColorFlags::empty(),
        Some("pcap writer tracer"),
    )
});

#[derive(Debug)]
struct Settings {
    output_dir: PathBuf,
    target_factory: Option<String>,
    pad_path: Option<String>,
    fake_protocol: FakeProtocol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, glib::Enum)]
#[enum_type(name = "GstPcapWriterFakeProtocol")]
pub enum FakeProtocol {
    Udp,
    None,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            output_dir: Path::new("tracer_pcaps").to_path_buf(),
            target_factory: None,
            pad_path: None,
            fake_protocol: FakeProtocol::Udp,
        }
    }
}

struct Writer {
    writer: pcap::PcapWriter<File>,
    buf: Vec<u8>,
    protocol: FakeProtocol,
}

impl Writer {
    fn new(file: File, protocol: FakeProtocol) -> Self {
        Self {
            writer: pcap::PcapWriter::new(file).expect("Error writing pcap"),
            buf: Vec::<u8>::with_capacity(MAX_PACKET_LEN + MAX_FAKE_HEADERS_LEN),
            protocol,
        }
    }

    fn write(&mut self, buffer: &gst::BufferRef) -> Result<(), anyhow::Error> {
        if buffer.size() > MAX_PACKET_LEN {
            anyhow::bail!("Maximum size of packet is {MAX_PACKET_LEN}");
        }

        let pts = buffer.pts().unwrap_or(gst::ClockTime::from_seconds(0));

        // Store capture time in microsecond precision because that's what wireshark uses
        // by default and the precision is not signalled in the packet headers.
        let ts_sec = pts.seconds() as u32;
        let ts_frac = (pts.useconds() % 1_000_000) as u32;

        let map = buffer.map_readable()?;
        if matches!(self.protocol, FakeProtocol::None) {
            let packet = RawPcapPacket {
                ts_sec,
                ts_frac,
                incl_len: map.len() as u32,
                orig_len: map.len() as u32,
                data: std::borrow::Cow::Borrowed(map.as_slice()),
            };
            self.writer.write_raw_packet(&packet)?;

            return Ok(());
        }

        /* Add fake Ethernet/IP/UDP encapsulation for this packet */
        /* FIXME: Make Ips configurable? */
        let builder = PacketBuilder::ethernet2([1, 2, 3, 4, 5, 6], [7, 8, 9, 10, 11, 12])
            .ipv4([192, 168, 1, 1], [192, 168, 1, 2], 20)
            .udp(21, 1234);

        let size = builder.size(map.size());

        self.buf.clear();
        builder.write(&mut self.buf, map.as_slice()).unwrap();

        let packet = RawPcapPacket {
            ts_sec,
            ts_frac,
            incl_len: size as u32,
            orig_len: size as u32,
            data: std::borrow::Cow::Borrowed(&self.buf),
        };
        self.writer.write_raw_packet(&packet)?;

        Ok(())
    }
}

#[derive(Properties, Default)]
#[properties(wrapper_type = super::PcapWriter)]
pub struct PcapWriter {
    #[property(
        name="output-dir",
        get,
        set,
        type = String,
        member = output_dir,
        blurb = "Directory where to save pcap files")
    ]
    #[property(
        name="target-factory",
        get,
        set,
        type = Option<String>,
        member = target_factory,
        blurb = "Factory name to target")
    ]
    #[property(
        name="pad-path",
        get,
        set,
        type = Option<String>,
        member = pad_path,
        blurb = "Pad path to target")
    ]
    #[property(
        name="fake-protocol",
        get,
        set,
        type = FakeProtocol,
        member = fake_protocol,
        blurb = "Protocol to fake in pcap files",
        builder(FakeProtocol::Udp))
    ]
    settings: Mutex<Settings>,

    pads: Mutex<HashMap<usize, Arc<Mutex<Writer>>>>,

    ignored: Mutex<HashSet<usize>>,
}

#[glib::object_subclass]
impl ObjectSubclass for PcapWriter {
    const NAME: &'static str = "GstPcapWriter";
    type Type = super::PcapWriter;
    type ParentType = gst::Tracer;
}

#[glib::derived_properties]
impl ObjectImpl for PcapWriter {
    fn constructed(&self) {
        self.parent_constructed();

        let settings = self.settings.lock().unwrap();
        if settings.target_factory.is_some() || settings.pad_path.is_some() {
            if let Err(err) = create_dir_all(&settings.output_dir) {
                gst::error!(
                    CAT,
                    "Could not create output dir, not writing pcaps: {err:?}"
                )
            } else {
                self.register_hook(TracerHook::PadPushPre);
                self.register_hook(TracerHook::PadPushListPre);
            }
        } else {
            gst::warning!(
                CAT,
                imp = self,
                "'pcap-writer' enabled without specifying 'target-factory' or 'pad-path' parameters. Not writing pcaps."
            );
        }
    }
}

impl GstObjectImpl for PcapWriter {}

fn pad_is_wanted(pad: &gst::Pad, settings: &Settings) -> bool {
    if let Some(factory_name) = settings.target_factory.as_ref() {
        return pad.parent().is_some_and(|p| {
            p.downcast::<gst::Element>()
                .is_ok_and(|e| e.factory().is_some_and(|f| f.name() == *factory_name))
        });
    }

    let mut element_name_pad_name = settings.pad_path.as_ref().unwrap().split(':');
    let element_name = element_name_pad_name.next().unwrap();
    let pad_name = element_name_pad_name.next();

    if let Some(parent) = pad.parent() {
        if parent.name() != element_name {
            return false;
        } else if let Some(pad_name) = pad_name {
            return pad_name == pad.name();
        }

        return true;
    }

    false
}

impl TracerImpl for PcapWriter {
    const USE_STRUCTURE_PARAMS: bool = true;

    fn pad_push_list_pre(&self, _ts: u64, pad: &gst::Pad, blist: &gst::BufferList) {
        for buffer in blist.iter() {
            self.maybe_write_buffer(pad, buffer);
        }
    }

    fn pad_push_pre(&self, _ts: u64, pad: &gst::Pad, buffer: &gst::Buffer) {
        self.maybe_write_buffer(pad, buffer.as_ref());
    }
}

impl PcapWriter {
    fn maybe_write_buffer(&self, pad: &gst::Pad, buffer: &gst::BufferRef) {
        if self
            .ignored
            .lock()
            .unwrap()
            .contains(&(pad.as_ptr() as usize))
        {
            return;
        }

        let mut pads = self.pads.lock().unwrap();
        let writer = if let Some(writer) = pads.get(&(pad.as_ptr() as usize)) {
            writer.clone()
        } else if pad_is_wanted(pad, &self.settings.lock().unwrap()) {
            let mut obj = pad.upcast_ref::<gst::Object>().clone();
            let mut fname = obj.name().to_string();
            while let Some(p) = obj.parent() {
                fname = p.name().to_string() + ">" + &fname;
                obj = p.clone();
            }

            fname.push_str(".pcap");
            let outpath = self.settings.lock().unwrap().output_dir.join(fname);
            gst::info!(CAT, obj = pad, "Writing pcap: {outpath:?}");

            let outfile = File::create(outpath).expect("Error creating file");

            let pcap_writer = Arc::new(Mutex::new(Writer::new(
                outfile,
                self.settings.lock().unwrap().fake_protocol,
            )));
            pads.insert(pad.as_ptr() as usize, pcap_writer.clone());

            pcap_writer
        } else {
            self.ignored.lock().unwrap().insert(pad.as_ptr() as usize);
            return;
        };

        drop(pads);

        let mut writer = writer.lock().unwrap();
        if let Err(err) = writer.write(buffer) {
            gst::error!(CAT, "Error writing buffer: {err:?}");
        }
    }
}
