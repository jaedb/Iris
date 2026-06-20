# gst-plugin-quinn

This is a [GStreamer](https://gstreamer.freedesktop.org/) plugin for using [QUIC](https://www.rfc-editor.org/rfc/rfc9000.html) as the transport build using [quinn-rs](https://github.com/quinn-rs/quinn).

## Examples

Build the examples by running
```bash
cargo build -p gst-plugin-quinn --examples
```

QUIC multiplexing example can be tested as follows.
```bash
GST_PLUGIN_PATH=target/debug cargo run -p gst-plugin-quinn --example quic_mux
GST_PLUGIN_PATH=target/debug cargo run -p gst-plugin-quinn --example quic_mux -- --receiver
```

QUIC multiplexing example with WebTransport can be tested as follows.
```bash
GST_PLUGIN_PATH=target/debug cargo run -p gst-plugin-quinn --example quic_mux -- --webtransport
GST_PLUGIN_PATH=target/debug cargo run -p gst-plugin-quinn --example quic_mux -- --receiver --webtransport
```

RoQ example can be tested as follows. This tests H264 by default.
```bash
GST_PLUGIN_PATH=target/debug cargo run -p gst-plugin-quinn --example quic_roq
GST_PLUGIN_PATH=target/debug cargo run -p gst-plugin-quinn --example quic_roq -- --receiver
```

To test RoQ with VP8.
```bash
GST_PLUGIN_PATH=target/debug cargo run -p gst-plugin-quinn --example quic_roq -- --vp8
GST_PLUGIN_PATH=target/debug cargo run -p gst-plugin-quinn --example quic_roq -- --receiver --vp8
```
