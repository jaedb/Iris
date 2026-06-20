# gst-plugins-spotify

This is a [GStreamer](https://gstreamer.freedesktop.org/) plugin to read content from
[Spotify](https://www.spotify.com/).

Make sure that your application follows [Spotify's design guidelines](https://developer.spotify.com/documentation/general/design-and-branding/)
to respect their legal/licensing restrictions.

## Spotify Credentials

This plugin requires a [Spotify Premium](https://www.spotify.com/premium/) account.

Provide a Spotify access token with 'streaming' scope using the `access-token` property. Such a token can be obtained by using either:
- [this webpage](https://open.spotify.com/get_access_token) for easy testing;
- [Spotify's OAuth flow](https://developer.spotify.com/documentation/web-api/concepts/authorization);
- the facility on their [Web SDK getting started guide](https://developer.spotify.com/documentation/web-playback-sdk/tutorials/getting-started);
- [librespot-oauth](https://github.com/librespot-org/librespot/blob/dev/oauth/examples/oauth.rs):

```console
cargo install librespot-oauth --example oauth && oauth
```

Note, Spotify access tokens are only valid for 1 hour and must be [refreshed](https://developer.spotify.com/documentation/web-api/tutorials/refreshing-tokens)
for usage beyond that.

It is therefore advisable to also use the `cache-credentials` property. On first usage, your access token is exchanged for a reusable credentials blob and
stored at the location specified by this property. Once obtained, that credentials blob is used for login and any provided `access-token` is ignored.
Unlike Spotify access tokens, the user's credentials blob does not expire. Avoiding handling token refresh greatly simplifies plugin usage.
If you do not set `cache-credentials`, you must manage refreshing your Spotify access token so it's valid for login when the element starts.

You may also want to cache downloaded files, see the `cache-files` property.

## spotifyaudiosrc

The `spotifyaudiosrc` element can be used to play a song from Spotify using its [Spotify URI](https://community.spotify.com/t5/FAQs/What-s-a-Spotify-URI/ta-p/919201).

```
gst-launch-1.0 spotifyaudiosrc access-token=$ACCESS_TOKEN track=spotify:track:3i3P1mGpV9eRlfKccjDjwi ! oggdemux ! vorbisdec ! audioconvert ! autoaudiosink
```

The element also implements an URI handler which accepts credentials and cache settings as URI parameters:

```console
gst-launch-1.0 playbin3 uri=spotify:track:3i3P1mGpV9eRlfKccjDjwi?access-token=$ACCESS_TOKEN\&cache-credentials=cache\&cache-files=cache
```

## spotifylyricssrc

The `spotifylyricssrc` element can be used to retrieve the lyrics of a song from Spotify.

```
gst-launch-1.0 spotifylyricssrc access-token=$ACCESS_TOKEN track=spotify:track:3yMFBuIdPBdJkkzaPBDjKY ! txt. videotestsrc pattern=black ! video/x-raw,width=1920,height=1080 ! textoverlay name=txt shaded-background=yes valignment=center halignment=center ! autovideosink
```