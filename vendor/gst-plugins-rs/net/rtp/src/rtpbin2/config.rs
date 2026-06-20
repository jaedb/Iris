// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use std::sync::LazyLock;
use std::sync::{Mutex, Weak};

use crate::rtpbin2::internal::SharedSessionInner;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rtp2-config",
        gst::DebugColorFlags::empty(),
        Some("Rtp2 config"),
    )
});

glib::wrapper! {
    pub struct Rtp2Session(ObjectSubclass<imp::Rtp2Session>);
}

impl Rtp2Session {
    pub(crate) fn new(weak_session: Weak<Mutex<SharedSessionInner>>) -> Self {
        let ret = glib::Object::new::<Self>();
        let imp = ret.imp();
        imp.set_session(weak_session);
        ret
    }
}

mod imp {
    use std::sync::Arc;

    use super::*;

    #[derive(Debug, Default)]
    struct State {
        pub(super) weak_session: Option<Weak<Mutex<SharedSessionInner>>>,
    }

    #[derive(Debug, Default)]
    pub struct Rtp2Session {
        state: Mutex<State>,
    }

    impl Rtp2Session {
        pub(super) fn set_session(&self, weak_session: Weak<Mutex<SharedSessionInner>>) {
            let mut state = self.state.lock().unwrap();
            state.weak_session = Some(weak_session);
        }

        fn session(&self) -> Option<Arc<Mutex<SharedSessionInner>>> {
            self.state
                .lock()
                .unwrap()
                .weak_session
                .as_ref()
                .and_then(|sess| sess.upgrade())
        }

        pub fn set_pt_map(&self, pt_map: Option<gst::Structure>) {
            let Some(session) = self.session() else {
                return;
            };
            let mut session = session.lock().unwrap();
            session.clear_pt_map();
            let Some(pt_map) = pt_map else {
                return;
            };

            for (key, value) in pt_map.iter() {
                let Ok(pt) = key.parse::<u8>() else {
                    gst::warning!(CAT, "failed to parse key as a pt");
                    continue;
                };
                if let Ok(caps) = value.get::<gst::Caps>() {
                    session.add_caps(caps);
                } else {
                    gst::warning!(CAT, "{pt} does not contain a caps value");
                    continue;
                }
            }
        }

        pub fn pt_map(&self) -> gst::Structure {
            let mut ret = gst::Structure::builder("application/x-rtp2-pt-map");
            let Some(session) = self.session() else {
                return ret.build();
            };
            let session = session.lock().unwrap();

            for (pt, caps) in session.pt_map() {
                ret = ret.field(pt.to_string(), caps);
            }

            ret.build()
        }

        pub fn stats(&self) -> Option<gst::Structure> {
            let session = self.session()?;
            let session = session.lock().unwrap();
            Some(session.stats())
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Rtp2Session {
        const NAME: &'static str = "GstRtp2Session";
        type Type = super::Rtp2Session;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for Rtp2Session {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
                vec![glib::ParamSpecBoxed::builder::<gst::Structure>("pt-map")
                    .nick("RTP Payload Type Map")
                    .blurb("Mapping of RTP payload type to caps")
                    .build()]
            });

            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "pt-map" => self.pt_map().to_value(),
                "stats" => self.stats().to_value(),
                _ => unreachable!(),
            }
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "pt-map" => self.set_pt_map(
                    value
                        .get::<Option<gst::Structure>>()
                        .expect("Type checked upstream"),
                ),
                _ => unreachable!(),
            }
        }

        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: LazyLock<Vec<glib::subclass::Signal>> = LazyLock::new(|| {
                vec![
                    glib::subclass::Signal::builder("new-ssrc")
                        .param_types([u32::static_type()])
                        .build(),
                    glib::subclass::Signal::builder("bye-ssrc")
                        .param_types([u32::static_type()])
                        .build(),
                ]
            });

            SIGNALS.as_ref()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize},
        Arc,
    };

    use crate::{rtpbin2::session::tests::generate_rtp_packet, test_init};

    use super::*;

    static ELEMENT_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn next_element_counter() -> usize {
        ELEMENT_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    #[test]
    fn pt_map_get_empty() {
        test_init();
        let id = next_element_counter();
        let rtpbin2 = gst::ElementFactory::make("rtpsend")
            .property("rtp-id", id.to_string())
            .build()
            .unwrap();
        let _pad = rtpbin2.request_pad_simple("rtp_sink_0").unwrap();
        let session = rtpbin2.emit_by_name::<gst::glib::Object>("get-session", &[&0u32]);
        let pt_map = session.property::<gst::Structure>("pt-map");
        assert!(pt_map.has_name("application/x-rtp2-pt-map"));
        assert_eq!(pt_map.fields().len(), 0);
    }

    #[test]
    fn pt_map_set() {
        test_init();
        let id = next_element_counter();
        let rtpbin2 = gst::ElementFactory::make("rtpsend")
            .property("rtp-id", id.to_string())
            .build()
            .unwrap();
        let _pad = rtpbin2.request_pad_simple("rtp_sink_0").unwrap();
        let session = rtpbin2.emit_by_name::<gst::glib::Object>("get-session", &[&0u32]);
        let pt = 96i32;
        let pt_caps = gst::Caps::builder("application/x-rtp")
            .field("payload", pt)
            .field("clock-rate", 90000i32)
            .build();
        let pt_map = gst::Structure::builder("application/x-rtp2-pt-map")
            .field(pt.to_string(), pt_caps.clone())
            .build();
        session.set_property("pt-map", pt_map);
        let prop = session.property::<gst::Structure>("pt-map");
        assert!(prop.has_name("application/x-rtp2-pt-map"));
        assert_eq!(prop.fields().len(), 1);
        let caps = prop.get::<gst::Caps>(pt.to_string()).unwrap();
        assert_eq!(pt_caps, caps);
    }

    #[test]
    fn pt_map_set_none() {
        test_init();
        let id = next_element_counter();
        let rtpbin2 = gst::ElementFactory::make("rtpsend")
            .property("rtp-id", id.to_string())
            .build()
            .unwrap();
        let _pad = rtpbin2.request_pad_simple("rtp_sink_0").unwrap();
        let session = rtpbin2.emit_by_name::<gst::glib::Object>("get-session", &[&0u32]);
        session.set_property("pt-map", None::<gst::Structure>);
        let prop = session.property::<gst::Structure>("pt-map");
        assert!(prop.has_name("application/x-rtp2-pt-map"));
    }

    #[test]
    fn new_send_ssrc() {
        test_init();
        let ssrc = 0x12345678;
        let new_ssrc_hit = Arc::new(AtomicBool::new(false));
        let id = next_element_counter();
        let rtpbin2 = gst::ElementFactory::make("rtpsend")
            .property("rtp-id", id.to_string())
            .build()
            .unwrap();
        let mut h =
            gst_check::Harness::with_element(&rtpbin2, Some("rtp_sink_0"), Some("rtp_src_0"));
        let session = h
            .element()
            .unwrap()
            .emit_by_name::<gst::glib::Object>("get-session", &[&0u32]);
        let ssrc_hit = new_ssrc_hit.clone();
        session.connect("new-ssrc", false, move |args| {
            let new_ssrc = args[1].get::<u32>().unwrap();
            ssrc_hit.store(true, std::sync::atomic::Ordering::SeqCst);
            assert_eq!(new_ssrc, ssrc);
            None
        });
        h.set_src_caps_str("application/x-rtp,payload=96,clock-rate=90000");
        let mut segment = gst::Segment::new();
        segment.set_format(gst::Format::Time);
        h.push_event(gst::event::Segment::builder(&segment).build());
        let buf1 = gst::Buffer::from_mut_slice(generate_rtp_packet(ssrc, 0x34, 0x10, 16));
        h.push(buf1.clone()).unwrap();
        assert!(new_ssrc_hit.load(std::sync::atomic::Ordering::SeqCst));
        let buf2 = gst::Buffer::from_mut_slice(generate_rtp_packet(ssrc, 0x35, 0x10, 16));
        h.push(buf2.clone()).unwrap();

        let buf3 = h.pull().unwrap();
        assert_eq!(buf3, buf1);
        let buf4 = h.pull().unwrap();
        assert_eq!(buf4, buf2);
    }

    #[test]
    fn bye_send_ssrc() {
        test_init();
        let ssrc = 0x12345678;
        let (bye_ssrc_sender, bye_ssrc_receiver) = std::sync::mpsc::sync_channel(16);
        let id = next_element_counter();
        let rtpbin2 = gst::ElementFactory::make("rtpsend")
            .property("rtp-id", id.to_string())
            .build()
            .unwrap();
        let mut h =
            gst_check::Harness::with_element(&rtpbin2, Some("rtp_sink_0"), Some("rtp_src_0"));
        let mut h_rtcp = gst_check::Harness::with_element(&rtpbin2, None, Some("rtcp_src_0"));
        let session = h
            .element()
            .unwrap()
            .emit_by_name::<gst::glib::Object>("get-session", &[&0u32]);
        session.connect("bye-ssrc", false, move |args| {
            let bye_ssrc = args[1].get::<u32>().unwrap();
            assert_eq!(bye_ssrc, ssrc);
            bye_ssrc_sender.send(ssrc).unwrap();
            None
        });
        h.set_src_caps_str("application/x-rtp,payload=96,clock-rate=90000");
        let mut segment = gst::Segment::new();
        segment.set_format(gst::Format::Time);
        h.push_event(gst::event::Segment::builder(&segment).build());
        let buf1 = gst::Buffer::from_mut_slice(generate_rtp_packet(ssrc, 0x34, 0x10, 16));
        h.push(buf1.clone()).unwrap();
        let buf2 = gst::Buffer::from_mut_slice(generate_rtp_packet(ssrc, 0x35, 0x10, 16));
        h.push(buf2.clone()).unwrap();

        let buf3 = h.pull().unwrap();
        assert_eq!(buf3, buf1);
        let buf4 = h.pull().unwrap();
        assert_eq!(buf4, buf2);

        h.push_event(gst::event::Eos::builder().build());
        let _rtcp = h_rtcp.pull().unwrap();
        assert_eq!(bye_ssrc_receiver.recv().unwrap(), ssrc);
    }
}
