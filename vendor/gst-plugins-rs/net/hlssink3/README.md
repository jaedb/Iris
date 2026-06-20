# GStreamer HTTP Live Streaming Plugin
A GStreamer HLS sink plugin. Based on the
["hlssink2"](https://gstreamer.freedesktop.org/documentation/hls/hlssink2.html?gi-language=c)
element.

The `hlssink3` plugin consists of `hlssink3` and `hlscmafsink` elements.
`hlssink3` element generates traditional MPEG-TS based HLS segments
and playlist while `hlscmafsink` generates `CMAF` (fragmented mp4)
segments.

**NOTE**: `hlssink3` element is feature-equivalent to the `hlssink2` element.
Any pipeline that uses `hlssink2` can use the `hlssink3` element
and the result should be the same.

Both elements have a `playlist-type` property used to control the behavior of the HLS playlist file. The
available values for this property are:
- `null` (default): The tag `#EXT-X-PLAYLIST-TYPE` won't be present in the playlist during the pipeline processing. The
  playlist will be updated in sync as new segments are available, old segments are removed, keeping N segments as
  defined in the property `playlist-length`. This is the default behavior, and is compatible with how "hlssink2" works;
- `"event"`: The playlist is updated as new segments are available, and the tag `#EXT-X-PLAYLIST-TYPE:EVENT` is present
  during processing. No segments will be removed from the playlist.
- `"vod"`: The playlist behaves like the `event` option (a live event), but at the end of the processing, the playlist
  will be set to `#EXT-X-PLAYLIST-TYPE:VOD`.

At the end of the processing, `#EXT-X-ENDLIST` is added to the playlist
if a `enable-endlist` property is enabled (default is `true`).

## Live playlist generation

In case of live recording with multiple playlists,
the `#EXT-X-PROGRAM-DATE-TIME` tags can be useful hint for clients
when mapping each stream time to the wall-clock.

The `#EXT-X-PROGRAM-DATE-TIME` tags will be written to the playlist
if `enable-program-date-time` property is enabled.

