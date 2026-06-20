# gst-plugins-textahead

This is [GStreamer](https://gstreamer.freedesktop.org/) plugin displays upcoming
text buffers ahead with the current one. This is mainly useful for Karaoke
applications where singers need to know beforehand the next lines of the song.

```
gst-launch-1.0 videotestsrc pattern=black ! video/x-raw,width=1920,height=1080 ! textoverlay name=txt ! autovideosink filesrc location=subtitles.srt ! subparse ! textahead n-ahead=2 ! txt.
```
