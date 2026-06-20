# WebRTCSink custom signaller

A simple application that consist of two parts:

* main executable, which demonstrates how to instantiate WebRTCSink with a custom signaller
* `signaller` module, which provides all the required boilerplate code
  and stub implementations needed to create a custom signaller

Run with:

``` shell
cargo run --example webrtcsink-custom-signaller
```

The expected output is a not-implemented panic (from `imp::Signaller::start` function):

```text
thread 'tokio-runtime-worker' panicked at 'not implemented', net/webrtc/examples/webrtcsink-custom-signaller/signaller/imp.rs:14:9
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
```

Simply implement the methods in [imp.rs](signaller/imp.rs) and you should be good
to go!
