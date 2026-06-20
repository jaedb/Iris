use gst::glib;
use gst::glib::subclass::*;
use gst::prelude::*;
use gst::subclass::prelude::*;
use std::sync::LazyLock;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct SignallableInterface {
    _parent: glib::gobject_ffi::GTypeInterface,
    pub start: fn(&super::Signallable),
    pub stop: fn(&super::Signallable),
    pub send_sdp: fn(&super::Signallable, &str, &gst_webrtc::WebRTCSessionDescription),
    pub add_ice: fn(&super::Signallable, &str, &str, u32, Option<String>),
    pub end_session: fn(&super::Signallable, &str),
}

unsafe impl InterfaceStruct for SignallableInterface {
    type Type = Signallable;
}

pub enum Signallable {}

impl Signallable {
    fn request_meta(_iface: &super::Signallable) -> Option<gst::Structure> {
        None
    }
    fn start(_iface: &super::Signallable) {}
    fn stop(_iface: &super::Signallable) {}
    fn send_sdp(
        _iface: &super::Signallable,
        _session_id: &str,
        _sdp: &gst_webrtc::WebRTCSessionDescription,
    ) {
    }
    fn add_ice(
        _iface: &super::Signallable,
        _session_id: &str,
        _candidate: &str,
        _sdp_m_line_index: u32,
        _sdp_mid: Option<String>,
    ) {
    }
    fn end_session(_iface: &super::Signallable, _session_id: &str) {}
}

#[glib::object_interface]
impl prelude::ObjectInterface for Signallable {
    const NAME: &'static ::std::primitive::str = "GstRSWebRTCSignallableIface";
    type Interface = SignallableInterface;
    type Prerequisites = (glib::Object,);

    fn interface_init(iface: &mut SignallableInterface) {
        iface.start = Signallable::start;
        iface.stop = Signallable::stop;
        iface.send_sdp = Signallable::send_sdp;
        iface.add_ice = Signallable::add_ice;
        iface.end_session = Signallable::end_session;
    }

    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![glib::ParamSpecBoolean::builder("manual-sdp-munging")
                .nick("Manual SDP munging")
                .blurb("Whether the signaller manages SDP munging itself")
                .default_value(false)
                .read_only()
                .build()]
        });

        PROPERTIES.as_ref()
    }

    fn signals() -> &'static [Signal] {
        static SIGNALS: LazyLock<Vec<Signal>> = LazyLock::new(|| {
            vec![
                /**
                 * GstRSWebRTCSignallableIface::session-ended:
                 * @self: The object implementing #GstRSWebRTCSignallableIface
                 * @session-id: The ID of the session that ended
                 *
                 * Notify the underlying webrtc object that a session has ended.
                 */
                Signal::builder("session-ended")
                    .param_types([str::static_type()])
                    .return_type::<bool>()
                    .build(),
                /**
                 * GstRSWebRTCSignallableIface::producer-added:
                 * @self: The object implementing #GstRSWebRTCSignallableIface
                 * @producer_id: The ID of the available producer
                 * @meta: The metadata structure of the producer
                 * @new_connection: true if the producer has just connected to
                 *                  the signalling server or false if it was
                 *                  already running before starting the client
                 *
                 * Some producing peer is available to produce a WebRTC stream.
                 */
                Signal::builder("producer-added")
                    .param_types([
                        str::static_type(),
                        <Option<gst::Structure>>::static_type(),
                        bool::static_type(),
                    ])
                    .build(),
                /**
                 * GstRSWebRTCSignallableIface::producer-removed:
                 * @self: The object implementing #GstRSWebRTCSignallableIface
                 * @producer_id: The ID of the producer that is not available
                 *               anymore
                 * @meta: The metadata structure of the producer
                 *
                 * Some producing peer has stopped producing streams.
                 */
                Signal::builder("producer-removed")
                    .param_types([str::static_type(), <Option<gst::Structure>>::static_type()])
                    .build(),
                /**
                 * GstRSWebRTCSignallableIface::session-started:
                 * @self: The object implementing #GstRSWebRTCSignallableIface
                 *
                 * Notify the underlying webrtc object that a session has started.
                 */
                Signal::builder("session-started")
                    .param_types([str::static_type(), str::static_type()])
                    .build(),
                /**
                 * GstRSWebRTCSignallableIface::session-requested:
                 * @self: The object implementing #GstRSWebRTCSignallableIface
                 * @session_id: The ID of the producer that was added
                 * @peer_id: The ID of the consumer peer who wants to initiate a
                 *           session
                 *
                 * Notify the underlying webrtc object that a session has been requested from the
                 * peer.
                 */
                Signal::builder("session-requested")
                    .param_types([
                        str::static_type(),
                        str::static_type(),
                        gst_webrtc::WebRTCSessionDescription::static_type(),
                    ])
                    .build(),
                /**
                 * GstRSWebRTCSignallableIface::error:
                 * @self: The object implementing #GstRSWebRTCSignallableIface
                 * @error: The error message as a string
                 */
                Signal::builder("error")
                    .param_types([str::static_type()])
                    .build(),
                /**
                 * GstRSWebRTCSignallableIface::request-meta:
                 * @self: The object implementing #GstRSWebRTCSignallableIface
                 *
                 * The signaller requests a meta about the peer using it
                 *
                 * Returns: The metadata about the peer represented by the signaller
                 */
                Signal::builder("request-meta")
                    .return_type::<Option<gst::Structure>>()
                    .class_handler(|args| {
                        let this = args[0usize]
                            .get::<&super::Signallable>()
                            .unwrap_or_else(|e| {
                                panic!("Wrong type for argument {}: {:?}", 0usize, e)
                            });
                        Some(Signallable::request_meta(this).to_value())
                    })
                    .accumulator(move |_hint, _acc, value| {
                        // First signal handler wins
                        std::ops::ControlFlow::Break(value.clone())
                    })
                    .build(),
                /**
                 * GstRSWebRTCSignallableIface::handle-ice:
                 * @self: The object implementing #GstRSWebRTCSignallableIface
                 * @session_id: Id of the session the ice information is about
                 * @sdp_m_line_index: The mlineindex of the ice candidate
                 * @sdp_mid: Media ID of the ice candidate
                 * @candidate: Information about the candidate
                 *
                 * Notify the underlying webrtc object of an ICE candidate.
                 */
                Signal::builder("handle-ice")
                    .flags(glib::SignalFlags::ACTION)
                    .param_types([
                        str::static_type(),
                        u32::static_type(),
                        <Option<String>>::static_type(),
                        str::static_type(),
                    ])
                    .build(),
                /**
                 * GstRSWebRTCSignallableIface::session-description:
                 * @self: The object implementing #GstRSWebRTCSignallableIface
                 * @session_id: Id of the session being described
                 * @description: The WebRTC session description
                 *
                 * Notify the underlying webrtc object of a received session description
                 */
                Signal::builder("session-description")
                    .flags(glib::SignalFlags::ACTION)
                    .param_types([
                        str::static_type(),
                        gst_webrtc::WebRTCSessionDescription::static_type(),
                    ])
                    .build(),
                /**
                 * GstRSWebRTCSignallableIface::start:
                 * @self: The object implementing #GstRSWebRTCSignallableIface
                 *
                 * Starts the signaller, connecting it to the signalling server.
                 */
                Signal::builder("start")
                    .run_last()
                    .return_type::<bool>()
                    .class_handler(|args| {
                        let this = args[0usize]
                            .get::<&super::Signallable>()
                            .unwrap_or_else(|e| {
                                panic!("Wrong type for argument {}: {:?}", 0usize, e)
                            });
                        let vtable = this.interface::<super::Signallable>().unwrap();
                        let vtable = vtable.as_ref();
                        (vtable.start)(this);

                        Some(false.into())
                    })
                    .accumulator(move |_hint, _acc, value| {
                        // First signal handler wins
                        std::ops::ControlFlow::Break(value.clone())
                    })
                    .build(),
                /**
                 * GstRSWebRTCSignallableIface::stop:
                 * @self: The object implementing #GstRSWebRTCSignallableIface
                 *
                 * Stops the signaller, disconnecting it to the signalling server.
                 */
                Signal::builder("stop")
                    .run_last()
                    .return_type::<bool>()
                    .class_handler(|args| {
                        let this = args[0usize]
                            .get::<&super::Signallable>()
                            .unwrap_or_else(|e| {
                                panic!("Wrong type for argument {}: {:?}", 0usize, e)
                            });
                        let vtable = this.interface::<super::Signallable>().unwrap();
                        let vtable = vtable.as_ref();
                        (vtable.stop)(this);

                        Some(false.into())
                    })
                    .accumulator(move |_hint, _acc, value| {
                        // First signal handler wins
                        std::ops::ControlFlow::Break(value.clone())
                    })
                    .build(),
                /**
                 * GstRSWebRTCSignallableIface::shutdown:
                 * @self: The object implementing #GstRSWebRTCSignallableIface
                 */
                Signal::builder("shutdown")
                    .flags(glib::SignalFlags::ACTION)
                    .build(),
                /**
                 * GstRSWebRTCSignallableIface::end-session:
                 * @self: The object implementing #GstRSWebRTCSignallableIface
                 * @session-id: The ID of the session that should be ended
                 *
                 * Notify the signaller that a session should be ended.
                 */
                Signal::builder("end-session")
                    .run_last()
                    .param_types([str::static_type()])
                    .return_type::<bool>()
                    .class_handler(|args| {
                        let this = args[0usize]
                            .get::<&super::Signallable>()
                            .unwrap_or_else(|e| {
                                panic!("Wrong type for argument {}: {:?}", 0usize, e)
                            });
                        let session_id = args[1usize].get::<&str>().unwrap_or_else(|e| {
                            panic!("Wrong type for argument {}: {:?}", 1usize, e)
                        });
                        let vtable = this.interface::<super::Signallable>().unwrap();
                        let vtable = vtable.as_ref();
                        (vtable.end_session)(this, session_id);

                        Some(false.into())
                    })
                    .accumulator(move |_hint, _acc, value| {
                        // First signal handler wins
                        std::ops::ControlFlow::Break(value.clone())
                    })
                    .build(),
                /**
                 * GstRSWebRTCSignallableIface::consumer-added:
                 * @self: The object implementing #GstRSWebRTCSignallableIface
                 * @peer_id: Id of the consumer
                 * @webrtcbin: The internal WebRTCBin element
                 *
                 * This signal can be used to tweak @webrtcbin, creating a data
                 * channel for example.
                 *
                 * Deprecated: 1.24: Use `webrtcbin-ready` instead
                 */
                Signal::builder("consumer-added")
                    .param_types([String::static_type(), gst::Element::static_type()])
                    .deprecated()
                    .build(),
                /**
                 * GstRSWebRTCSignallableIface::consumer-removed:
                 * @consumer_id: Identifier of the consumer that was removed
                 * @webrtcbin: The webrtcbin connected to the newly removed consumer
                 *
                 * This signal is emitted right after the connection with a consumer
                 * has been dropped.
                 */
                Signal::builder("consumer-removed")
                    .param_types([String::static_type(), gst::Element::static_type()])
                    .build(),
                /**
                 * GstRSWebRTCSignallableIface::munge-session-description:
                 * @self: The object implementing #GstRSWebRTCSignallableIface
                 * @session_id: Id of the session being described
                 * @description: The WebRTC session description to modify
                 *
                 * For special-case handling, a callback can be registered to modify the session
                 * description before the signaller sends it to the peer. Only the first connected
                 * handler has any effect.
                 *
                 * Returns: A modified session description
                 */
                Signal::builder("munge-session-description")
                    .run_last()
                    .param_types([
                        str::static_type(),
                        gst_webrtc::WebRTCSessionDescription::static_type(),
                    ])
                    .return_type::<gst_webrtc::WebRTCSessionDescription>()
                    .class_handler(|args| {
                        let _ = args[0usize]
                            .get::<&super::Signallable>()
                            .unwrap_or_else(|e| {
                                panic!("Wrong type for argument {}: {:?}", 0usize, e)
                            });
                        let _ = args[1usize].get::<&str>().unwrap_or_else(|e| {
                            panic!("Wrong type for argument {}: {:?}", 1usize, e)
                        });
                        let desc = args[2usize]
                            .get::<&gst_webrtc::WebRTCSessionDescription>()
                            .unwrap_or_else(|e| {
                                panic!("Wrong type for argument {}: {:?}", 2usize, e)
                            });

                        Some(desc.clone().into())
                    })
                    .accumulator(move |_hint, _acc, value| {
                        // First signal handler wins
                        std::ops::ControlFlow::Break(value.clone())
                    })
                    .build(),
                /**
                 * GstRSWebRTCSignallableIface::send-session-description:
                 * @self: The object implementing #GstRSWebRTCSignallableIface
                 * @session_id: Id of the session being described
                 * @description: The WebRTC session description to send to the peer
                 *
                 * Send @description to the peer.
                 */
                Signal::builder("send-session-description")
                    .run_last()
                    .param_types([
                        str::static_type(),
                        gst_webrtc::WebRTCSessionDescription::static_type(),
                    ])
                    .return_type::<bool>()
                    .class_handler(|args| {
                        let this = args[0usize]
                            .get::<&super::Signallable>()
                            .unwrap_or_else(|e| {
                                panic!("Wrong type for argument {}: {:?}", 0usize, e)
                            });
                        let session_id = args[1usize].get::<&str>().unwrap_or_else(|e| {
                            panic!("Wrong type for argument {}: {:?}", 1usize, e)
                        });
                        let desc = args[2usize]
                            .get::<&gst_webrtc::WebRTCSessionDescription>()
                            .unwrap_or_else(|e| {
                                panic!("Wrong type for argument {}: {:?}", 2usize, e)
                            });
                        let vtable = this.interface::<super::Signallable>().unwrap();
                        let vtable = vtable.as_ref();
                        (vtable.send_sdp)(this, session_id, desc);

                        Some(false.into())
                    })
                    .accumulator(move |_hint, _acc, value| {
                        // First signal handler wins
                        std::ops::ControlFlow::Break(value.clone())
                    })
                    .build(),
                /**
                 * GstRSWebRTCSignallableIface::send-ice:
                 * @self: The object implementing #GstRSWebRTCSignallableIface
                 * @session_id: Id of the session being described
                 * @candidate: The ICE candidate description to send to the peer
                 * @sdp_m_line_index: The M-line of the session description this candidate applies to
                 * @sdp_mid: The MID this candidate applies to
                 *
                 * Send @candidate to the peer.
                 */
                Signal::builder("send-ice")
                    .param_types([
                        str::static_type(),
                        str::static_type(),
                        u32::static_type(),
                        String::static_type(),
                    ])
                    .return_type::<bool>()
                    .class_handler(|args| {
                        let this = args[0usize]
                            .get::<&super::Signallable>()
                            .unwrap_or_else(|e| {
                                panic!("Wrong type for argument {}: {:?}", 0usize, e)
                            });
                        let session_id = args[1usize].get::<&str>().unwrap_or_else(|e| {
                            panic!("Wrong type for argument {}: {:?}", 1usize, e)
                        });
                        let candidate = args[2usize].get::<&str>().unwrap_or_else(|e| {
                            panic!("Wrong type for argument {}: {:?}", 2usize, e)
                        });
                        let sdp_m_line_index = args[3usize].get::<u32>().unwrap_or_else(|e| {
                            panic!("Wrong type for argument {}: {:?}", 2usize, e)
                        });
                        let sdp_mid = args[4usize].get::<Option<String>>().unwrap_or_else(|e| {
                            panic!("Wrong type for argument {}: {:?}", 2usize, e)
                        });
                        let vtable = this.interface::<super::Signallable>().unwrap();
                        let vtable = vtable.as_ref();
                        (vtable.add_ice)(this, session_id, candidate, sdp_m_line_index, sdp_mid);

                        Some(false.into())
                    })
                    .accumulator(move |_hint, _acc, value| {
                        // First signal handler wins
                        std::ops::ControlFlow::Break(value.clone())
                    })
                    .build(),
                /**
                 * GstRSWebRTCSignallableIface::webrtcbin-ready:
                 * @self: The object implementing #GstRSWebRTCSignallableIface
                 * @peer_id: Id of the consumer/producer
                 * @webrtcbin: The internal WebRTCBin element
                 *
                 * This signal can be used to tweak @webrtcbin, creating a data
                 * channel for example.
                 */
                Signal::builder("webrtcbin-ready")
                    .param_types([String::static_type(), gst::Element::static_type()])
                    .build(),
            ]
        });
        SIGNALS.as_ref()
    }
}

unsafe impl<Obj: SignallableImpl> types::IsImplementable<Obj> for super::Signallable
where
    <Obj as types::ObjectSubclass>::Type: IsA<glib::Object>,
{
    fn interface_init(iface: &mut glib::Interface<Self>) {
        let iface = ::std::convert::AsMut::as_mut(iface);

        fn vstart_trampoline<Obj: types::ObjectSubclass + SignallableImpl>(
            obj: &super::Signallable,
        ) {
            let this = obj
                .dynamic_cast_ref::<<Obj as types::ObjectSubclass>::Type>()
                .unwrap()
                .imp();
            SignallableImpl::start(this)
        }
        iface.start = vstart_trampoline::<Obj>;

        fn vstop_trampoline<Obj: types::ObjectSubclass + SignallableImpl>(
            this: &super::Signallable,
        ) {
            let this = this
                .dynamic_cast_ref::<<Obj as types::ObjectSubclass>::Type>()
                .unwrap();
            SignallableImpl::stop(this.imp())
        }
        iface.stop = vstop_trampoline::<Obj>;

        fn send_sdp_trampoline<Obj: types::ObjectSubclass + SignallableImpl>(
            this: &super::Signallable,
            session_id: &str,
            sdp: &gst_webrtc::WebRTCSessionDescription,
        ) {
            let this = this
                .dynamic_cast_ref::<<Obj as types::ObjectSubclass>::Type>()
                .unwrap();
            SignallableImpl::send_sdp(this.imp(), session_id, sdp)
        }
        iface.send_sdp = send_sdp_trampoline::<Obj>;

        fn add_ice_trampoline<Obj: types::ObjectSubclass + SignallableImpl>(
            this: &super::Signallable,
            session_id: &str,
            candidate: &str,
            sdp_m_line_index: u32,
            sdp_mid: Option<String>,
        ) {
            let this = this
                .dynamic_cast_ref::<<Obj as types::ObjectSubclass>::Type>()
                .unwrap();
            SignallableImpl::add_ice(this.imp(), session_id, candidate, sdp_m_line_index, sdp_mid)
        }
        iface.add_ice = add_ice_trampoline::<Obj>;

        fn end_session_trampoline<Obj: types::ObjectSubclass + SignallableImpl>(
            this: &super::Signallable,
            session_id: &str,
        ) {
            let this = this
                .dynamic_cast_ref::<<Obj as types::ObjectSubclass>::Type>()
                .unwrap();
            SignallableImpl::end_session(this.imp(), session_id)
        }
        iface.end_session = end_session_trampoline::<Obj>;
    }
}

pub trait SignallableImpl:
    ObjectImpl + ObjectSubclass<Type: IsA<super::Signallable>> + Send + Sync + 'static
{
    fn start(&self) {}
    fn stop(&self) {}
    fn send_sdp(&self, _session_id: &str, _sdp: &gst_webrtc::WebRTCSessionDescription) {}
    fn add_ice(
        &self,
        _session_id: &str,
        _candidate: &str,
        _sdp_m_line_index: u32,
        _sdp_mid: Option<String>,
    ) {
    }
    fn end_session(&self, _session_id: &str) {}
}

pub trait SignallableExt: IsA<super::Signallable> + 'static {
    fn start(&self);
    fn stop(&self);
    fn munge_sdp(
        &self,
        session_id: &str,
        sdp: &gst_webrtc::WebRTCSessionDescription,
    ) -> gst_webrtc::WebRTCSessionDescription;
    fn send_sdp(&self, session_id: &str, sdp: &gst_webrtc::WebRTCSessionDescription);
    fn add_ice(
        &self,
        session_id: &str,
        candidate: &str,
        sdp_m_line_index: u32,
        sdp_mid: Option<String>,
    );
    fn end_session(&self, session_id: &str);
}

impl<Obj: IsA<super::Signallable>> SignallableExt for Obj {
    fn start(&self) {
        self.emit_by_name::<bool>("start", &[]);
    }

    fn stop(&self) {
        self.emit_by_name::<bool>("stop", &[]);
    }

    fn munge_sdp(
        &self,
        session_id: &str,
        sdp: &gst_webrtc::WebRTCSessionDescription,
    ) -> gst_webrtc::WebRTCSessionDescription {
        self.emit_by_name::<gst_webrtc::WebRTCSessionDescription>(
            "munge-session-description",
            &[&session_id, sdp],
        )
    }

    fn send_sdp(&self, session_id: &str, sdp: &gst_webrtc::WebRTCSessionDescription) {
        self.emit_by_name::<bool>("send-session-description", &[&session_id, sdp]);
    }

    fn add_ice(
        &self,
        session_id: &str,
        candidate: &str,
        sdp_m_line_index: u32,
        sdp_mid: Option<String>,
    ) {
        self.emit_by_name::<bool>(
            "send-ice",
            &[&session_id, &candidate, &sdp_m_line_index, &sdp_mid],
        );
    }

    fn end_session(&self, session_id: &str) {
        self.emit_by_name::<bool>("end-session", &[&session_id]);
    }
}
