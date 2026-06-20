GStreamer NDI Plugin
====================

*Compatible with NDI SDK 6.x and 5.x*

This is a plugin for the [GStreamer](https://gstreamer.freedesktop.org/)
multimedia framework that allows GStreamer to receive or send an
[NDI](https://www.newtek.com/ndi/) stream.

This plugin has been initially developed by [Teltek](http://teltek.es/) and
was funded by the [University of the Arts London](https://www.arts.ac.uk/) and
[The University of Manchester](https://www.manchester.ac.uk/).

Currently the plugin has a source element for receiving from NDI sources, a
sink element to provide an NDI source and a device provider for discovering
NDI sources on the network.

The plugin is loading the NDI SDK at runtime, either from the default library
path or, if set, from the directory given by the `NDI_RUNTIME_DIR_V6` or
`NDI_RUNTIME_DIR_V5` environment variables.

Some examples of how to use these elements from the command line:

```console
# Information about the elements
$ gst-inspect-1.0 ndi
$ gst-inspect-1.0 ndisrc
$ gst-inspect-1.0 ndisink

# Discover all NDI sources on the network
$ gst-device-monitor-1.0 -f Source/Network:application/x-ndi

# Audio/Video source pipeline
$ gst-launch-1.0 ndisrc ndi-name="GC-DEV2 (OBS)" ! ndisrcdemux name=demux   demux.video ! queue ! videoconvert ! autovideosink  demux.audio ! queue ! audioconvert ! autoaudiosink

# Audio/Video sink pipeline
$ gst-launch-1.0 videotestsrc is-live=true ! video/x-raw,format=UYVY ! ndisinkcombiner name=combiner ! ndisink ndi-name="My NDI source"  audiotestsrc is-live=true ! combiner.audio
```

Feel free to contribute to this project. Some ways you can contribute are:

* Testing with more hardware and software and reporting bugs
* Doing pull requests.

Closed Captions Support
-----------------------

Closed captions support is based on [1] & [2].

This pipelines streams a test video with test subtitles from
gst-plugins-rs/video/closedcaption. Run from the gst-plugins-rs root directory.

```console
# Audio/Video sink pipeline with closed captions (cc start around 0:00:14)

$ gst-launch-1.0 \
    ndisinkcombiner name=ndicombiner ! ndisink ndi-name="My NDI source" \
    cccombiner name=cccombiner ! videoconvert ! video/x-raw,format=UYVY ! ndicombiner.video \
    videotestsrc is-live=true ! cccombiner. \
    filesrc location=video/closedcaption/tests/dn2018-1217.scc ! sccparse ! cccombiner.caption \
    audiotestsrc is-live=true volume=0.1 ! ndicombiner.audio

# Discover all NDI sources on the network
$ gst-device-monitor-1.0 -f Source/Network:application/x-ndi

# Audio/Video source pipeline with closed caption overlay
$ gst-launch-1.0 \
    ndisrc ndi-name="_REPLACE_WITH_SOURCE_NAME_" ! ndisrcdemux name=demux \
    demux.video ! queue ! cea608overlay ! videoconvert ! autovideosink \
    demux.audio ! queue ! audioconvert ! autoaudiosink

# Variant 1: sink pipeline with c708 closed captions

$ gst-launch-1.0 \
    ndisinkcombiner name=ndicombiner ! ndisink ndi-name="My NDI source" \
    cccombiner name=cccombiner ! videoconvert ! video/x-raw,format=UYVY ! ndicombiner.video \
    videotestsrc is-live=true ! cccombiner. \
    filesrc location=video/closedcaption/tests/dn2018-1217.scc ! sccparse ! ccconverter ! closedcaption/x-cea-708,format=cdp ! cccombiner.caption \
    audiotestsrc is-live=true volume=0.1 ! ndicombiner.audio

# Variant 2: sink pipeline with c608 and c708 closed captions

$ gst-launch-1.0 \
    ndisinkcombiner name=ndicombiner ! ndisink ndi-name="My NDI source" \
    cccombiner name=cccombiner_1 ! cccombiner name=cccombiner_2 ! videoconvert ! video/x-raw,format=UYVY ! ndicombiner.video \
    videotestsrc is-live=true ! cccombiner_1. \
    filesrc location=video/closedcaption/tests/dn2018-1217.scc ! sccparse ! tee name=cctee \
    cctee. ! ccconverter ! closedcaption/x-cea-608,format=raw ! cccombiner_1.caption \
    cctee. ! ccconverter ! closedcaption/x-cea-708,format=cdp ! cccombiner_2.caption \
    audiotestsrc is-live=true volume=0.1 ! ndicombiner.audio
```

[1]: http://www.sienna-tv.com/ndi/ndiclosedcaptions.html
[2]: http://www.sienna-tv.com/ndi/ndiclosedcaptions608.html

License
-------
This plugin is licensed under the MPL-2 - see the [LICENSE](LICENSE-MPL-2.0) file for details

Acknowledgments
-------
* University of the Arts London and The University of Manchester.
* Sebastian Dröge (@sdroege).
