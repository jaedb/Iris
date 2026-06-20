# rtspsrc2

Rust rewrite of rtspsrc, with the purpose of fixing the fundamentally broken
architecture of rtspsrc. There are some major problems with rtspsrc:

1. Element states are linked to RTSP states, which causes unfixable glitching
   and issues, especially in shared RTSP media
2. The command loop is fundamentally broken and buggy, which can cause RTSP
   commands such as `SET_PARAMETER` and `GET_PARAMETER` to be lost
3. The combination of the above two causes unfixable deadlocks when doing state
   changes due to external factors such as server state, or when seeking
4. Parsing of untrusted RTSP messages from the network was done in C with the
   `GstRTSPMessage` API.
5. Parsing of untrusted SDP from the network was done in C with the
   `GstSDPMessage` API

## Implemented features

* RTSP 1.0 support
* Lower transports: TCP, UDP, UDP-Multicast
* RTCP SR and RTCP RR
* RTCP-based A/V sync
* Lower transport selection and priority (NEW!)
  - Also supports different lower transports for each SETUP

## Missing features

Roughly in order of priority:

* Credentials support
* TLS/TCP support
* NAT hole punching
* Allow ignoring specific streams (SDP medias)
  - Currently all available source pads must be linked
* SRTP support
* HTTP tunnelling
* Proxy support
* `GET_PARAMETER` / `SET_PARAMETER`
* Make TCP connection optional when using UDP transport
  - Or TCP reconnection if UDP has not timed out
* Parse more SDP attributes
  - extmap
  - key-mgmt
  - rid
  - rtcp-fb
  - source-filter
  - ssrc
* Clock sync support, such as RFC7273
* PAUSE support with VOD
* Seeking support with VOD
* ONVIF backchannel support
* ONVIF trick mode support
* RTSP 2 support (no servers exist at present)

## Missing configuration properties

These are some misc rtspsrc props that haven't been implemented in rtspsrc2
yet:

* latency
* do-rtx
* do-rtcp
* iface
* user-agent

## Maintenance and future cleanup

* Test with market RTSP cameras
  - Currently, only live555 and gst-rtsp-server have been tested
* Add tokio-console and tokio tracing support
