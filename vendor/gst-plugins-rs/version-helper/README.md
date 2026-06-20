# gst-plugin-version-helper [![crates.io](https://img.shields.io/crates/v/gst-plugin-version-helper.svg)](https://crates.io/crates/gst-plugin-version-helper) [![docs.rs](https://docs.rs/gst-plugin-version-helper/badge.svg)](https://docs.rs/gst-plugin-version-helper)

Extracts release for [GStreamer](https://gstreamer.freedesktop.org) plugin metadata

See the [documentation](https://docs.rs/gst-plugin-version-helper) for details.

This function is supposed to be used as follows in the `build.rs` of a crate that implements a
plugin:

```rust,ignore
gst_plugin_version_helper::info();
```

Inside `lib.rs` of the plugin, the information provided by `info` are usable as follows:

```rust,ignore
gst::plugin_define!(
    the_plugin_name,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "The Plugin's License",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);
```

## LICENSE

`gst-plugin-version-helper` is licensed under the MIT license ([LICENSE](LICENSE) or
http://opensource.org/licenses/MIT).

## Contribution

Any kinds of contributions are welcome as a pull request.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in `gst-plugin-version-helper` by you shall be licensed
under the MIT license as above, without any additional terms or conditions.
