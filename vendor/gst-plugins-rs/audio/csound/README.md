# gst-plugin-csound

This is a [GStreamer](https://gstreamer.freedesktop.org/) plugin to interact
with the [Csound](https://csound.com/) sound computing system.

Currently, there is only a filter element, called, csoundfilter. Two more elements a source and sink would be implemented
later on.

For more information about dependencies and installation process, please refer to the [csound-rs](https://crates.io/crates/csound) 
documentation

## simple example
The included example constructs the follow pipeline
```
$ gst-launch-1.0 \
    audiotestsrc ! \
    audioconvert ! \
    csoundfilter location=effect.csd ! \
    audioconvert ! \
    autoaudiosink
```
