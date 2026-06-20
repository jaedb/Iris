#!/usr/bin/env python3
#
# Copyright (C) 2018 Matthew Waters <matthew@centricular.com>
#               2022 Nirbheek Chauhan <nirbheek@centricular.com>
#               2024 Sebastian Dröge <sebastian@centricular.com>
#
# Demo application that shows how to implement a custom signaller around
# webrtcsink from Python

from websockets.version import version as wsv
import random
import ssl
import websockets
import asyncio
import os
import sys
import json
import argparse

import gi
gi.require_version('Gst', '1.0')
from gi.repository import Gst  # NOQA
gi.require_version('GstSdp', '1.0')
from gi.repository import GstSdp  # NOQA
gi.require_version('GstWebRTC', '1.0')
from gi.repository import GstWebRTC  # NOQA

# Ensure that gst-python is installed
try:
    from gi.overrides import Gst as _
except ImportError:
    print('gstreamer-python binding overrides aren\'t available, please install them')
    raise

class WebRTCClient:
    def __init__(self, loop, server):
        self.conn = None
        self.pipe = None
        self.webrtc = None
        self.event_loop = loop
        self.server = server

        self.pipe = Gst.parse_launch('webrtcsink name=sink do-fec=false  videotestsrc is-live=true ! video/x-raw,width=800,height=600 ! sink.  audiotestsrc is-live=true ! sink.')
        bus = self.pipe.get_bus()
        self.event_loop.add_reader(bus.get_pollfd().fd, self.on_bus_poll_cb, bus)
        self.webrtcsink = self.pipe.get_by_name('sink')
        self.signaller = self.webrtcsink.get_property('signaller')
        self.signaller.connect('start', self.signaller_on_start)
        self.signaller.connect('stop', self.signaller_on_stop)
        self.signaller.connect('send-session-description', self.signaller_on_send_session_description)
        self.signaller.connect('send-ice', self.signaller_on_send_ice)
        self.signaller.connect('end-session', self.signaller_on_end_session)
        self.signaller.connect('consumer-added', self.signaller_on_consumer_added)
        self.signaller.connect('consumer-removed', self.signaller_on_consumer_removed)
        self.signaller.connect('webrtcbin-ready', self.signaller_on_webrtcbin_ready)
        self.pipe.set_state(Gst.State.PLAYING)

    async def send(self, msg):
        assert self.conn
        print(f'>>> {msg}')
        await self.conn.send(json.dumps(msg))

    async def connect(self):
        print(f'connecting to {self.server}')
        self.conn = await websockets.connect(self.server)
        assert self.conn
        async for message in self.conn:
            await self.handle_json(message)
        self.close_pipeline()

    def send_soon(self, msg):
        asyncio.run_coroutine_threadsafe(self.send(msg), self.event_loop)

    def on_bus_poll_cb(self, bus):
        def remove_bus_poll():
            self.event_loop.remove_reader(bus.get_pollfd().fd)
            self.event_loop.stop()
        while bus.peek():
            msg = bus.pop()
            if msg.type == Gst.MessageType.ERROR:
                err = msg.parse_error()
                print("ERROR:", err.gerror, err.debug)
                remove_bus_poll()
                break
            elif msg.type == Gst.MessageType.EOS:
                remove_bus_poll()
                break
            elif msg.type == Gst.MessageType.LATENCY:
                self.pipe.recalculate_latency()

    async def handle_json(self, message):
        try:
            msg = json.loads(message)
        except json.decoder.JSONDecoderError:
            print('Failed to parse JSON message, this might be a bug')
            raise
        print(f'<<< {msg}')

        if msg['type'] == 'welcome':
            self.peer_id = msg['peerId']
            print(f'Got peer ID {self.peer_id} assigned')
            meta = self.signaller_emit_request_meta()
            await self.send({'type': 'setPeerStatus', 'roles': ['producer'], 'meta': meta})
        elif msg['type'] == 'sessionStarted':
            pass
        elif msg['type'] == 'startSession':
            offer = None
            if msg['offer']:
                _, sdpmsg = GstSdp.SDPMessage.new()
                GstSdp.sdp_message_parse_buffer(bytes(msg['offer'].encode()), sdpmsg)
                offer = GstWebRTC.WebRTCSessionDescription.new(GstWebRTC.WebRTCSDPType.OFFER, sdpmsg)
            self.signaller_emit_session_requested(msg['sessionId'], msg['peerId'], offer)
        elif msg['type'] == 'endSession':
            self.signaller_emit_session_ended(msg['sessionId'])
        elif msg['type'] == 'peer':
            if 'sdp' in msg:
                sdp = msg['sdp']
                assert sdp['type'] == 'answer'
                self.signaller_emit_session_description(msg['sessionId'], sdp['sdp'])
            elif 'ice' in msg:
                ice = msg['ice']
                self.signaller_emit_handle_ice(msg['sessionId'], ice['sdpMLineIndex'], None, ice['candidate'])
            else:
                print('unknown peer message')
            pass
        elif msg['type'] == 'error':
            self.signaller_emit_error(f'Error message from server {msg['details']}')
        else:
            print('unknown message type')

    def close_pipeline(self):
        if self.pipe:
            self.pipe.set_state(Gst.State.NULL)
            self.pipe = None
        self.webrtcsink = None
        self.signaller = None

    async def stop(self):
        if self.conn:
            await self.conn.close()
        self.conn = None

    # Signal handlers that are called from webrtcsink
    def signaller_on_start(self, _):
        print('starting')
        asyncio.run_coroutine_threadsafe(self.connect(), self.event_loop)
        return True

    def signaller_on_stop(self, _):
        print('stopping')
        self.event_loop.stop()
        return True

    def signaller_on_send_session_description(self, _, session_id, offer):
        typ = 'offer'
        if offer.type == GstWebRTC.WebRTCSDPType.ANSWER:
            typ = 'answer'
        sdp = offer.sdp.as_text()
        self.send_soon({'type': 'peer', 'sessionId': session_id, 'sdp': { 'type': typ, 'sdp': sdp }})
        return True

    def signaller_on_send_ice(self, _, session_id, candidate, sdp_m_line_index, sdp_mid):
        self.send_soon({'type': 'peer', 'sessionId': session_id, 'ice': {'candidate': candidate, 'sdpMLineIndex': sdp_m_line_index}})
        return True

    def signaller_on_end_session(self, _, session_id):
        self.send_soon({'type': 'endSession', 'sessionId': session_id})
        return True

    def signaller_on_consumer_added(self, _, peer_id, webrtcbin):
        pass

    def signaller_on_consumer_removed(self, _, peer_id, webrtcbin):
        pass

    def signaller_on_webrtcbin_ready(self, _, peer_id, webrtcbin):
        pass

    # Signals we have to emit to notify webrtcsink
    def signaller_emit_error(self, error):
        self.signaller.emit('error', error)

    def signaller_emit_request_meta(self):
        meta = self.signaller.emit('request-meta')
        return meta

    def signaller_emit_session_requested(self, session_id, peer_id, offer):
        self.signaller.emit('session-requested', session_id, peer_id, offer)

    def signaller_emit_session_description(self, session_id, sdp):
        res, sdp = GstSdp.SDPMessage.new_from_text(sdp)
        answer = GstWebRTC.WebRTCSessionDescription.new(GstWebRTC.WebRTCSDPType.ANSWER, sdp)
        self.signaller.emit('session-description', session_id, answer)

    def signaller_emit_handle_ice(self, session_id, sdp_m_line_index, sdp_mid, candidate):
        self.signaller.emit('handle-ice', session_id, sdp_m_line_index, sdp_mid, candidate)

    def signaller_emit_session_ended(self, session_id):
        return self.signaller.emit('session-ended', session_id)

    def signaller_emit_shutdown(self):
        self.signaller.emit('shutdown')

if __name__ == '__main__':
    Gst.init(None)
    parser = argparse.ArgumentParser()
    parser.add_argument('--server', default='ws://127.0.0.1:8443',
                        help='Signalling server to connect to, eg "ws://127.0.0.1:8443"')
    args = parser.parse_args()
    loop = asyncio.new_event_loop()
    c = WebRTCClient(loop, args.server)
    res = loop.run_forever()
    sys.exit(res)
