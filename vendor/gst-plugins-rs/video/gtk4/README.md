# GTK 4 Sink & Paintable

GTK 4 provides `gtk::Video` & `gtk::Picture` for rendering media such as videos. As the default `gtk::Video` widget doesn't
offer the possibility to use a custom `gst::Pipeline`. The plugin provides a `gst_video::VideoSink` along with a `gdk::Paintable` that's capable of rendering the sink's frames.

The sink can generate GL Textures if the system is capable of it, but it needs
to be compiled with either `waylandegl`, `x11glx` or `x11egl` cargo features. On
Windows and macOS this is enabled by default.

Additionally, the sink can render DMABufs directly on Linux if GTK 4.14 or
newer is used. For this the `dmabuf` feature needs to be enabled.

Depending on the GTK version that is used and should be supported as minimum,
new features or more efficient processing can be opted in with the `gtk_v4_10`,
`gtk_v4_12` and `gtk_v4_14` features. The minimum GTK version required by the
sink is GTK 4.4 on Linux without GL support, and 4.6 on Windows and macOS, and
on Linux with GL support.

The sink will provides a simple test window when launched via `gst-launch-1.0`
or `gst-play-1.0` or if the environment variable `GST_GTK4_WINDOW=1` is set.
Setting `GST_GTK4_WINDOW_FULLSCREEN=1` will make the window launch in fullscreen
mode.

# Flatpak Integration

To build and include the plugin in a Flatpak manifest, you can add the following snippet to your json manifest:

```json
{
    "sdk-extensions": [
        "org.freedesktop.Sdk.Extension.rust-stable"
    ],
    "build-options": {
        "append-path": "/usr/lib/sdk/rust-stable/bin",
    },
    "modules": [
        {
            "name": "gst-plugins-rs",
            "buildsystem": "simple",
            "sources": [
                {
                    "type": "archive",
                    "url": "https://crates.io/api/v1/crates/gst-plugin-gtk4/0.12.5/download",
                    "dest-filename": "gst-plugin-gtk4-0.12.5.tar.gz",
                    "sha256": "56e483cb1452f056ae94ccd5f63bdec697e04c87b30d89eb30c3f934042e1022"
                },
                "gst-plugin-gtk4-sources.json"
            ],
            "build-options": {
                "env": {
                    "CARGO_HOME": "$FLATPAK_BUILDER_BUILDDIR/cargo"
                }
            },
            "build-commands": [
                "cargo cinstall --offline --release --features=waylandegl,x11glx,x11egl,dmabuf --library-type=cdylib --prefix=/app"
            ]
        }
    ]
}
```

To generate the additional file `gst-plugin-gtk4-sources.json` which will contain links to all the Cargo dependencies for the plugin to avoid making network requests while building, you need to use the `flatpak-cargo-generator` tool from [flatpak-builder-tools](https://github.com/flatpak/flatpak-builder-tools):

```sh
wget https://crates.io/api/v1/crates/gst-plugin-gtk4/0.12.5/download
tar -xf download
sha256sum download # update the sha256 in the Flatpak manifest
cd gst-plugin-gtk4-0.12.5/
/path/to/flatpak-cargo-generator Cargo.lock -o gst-plugin-gtk4-sources.json
```
