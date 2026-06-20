# custom-mopidy

Mopidy + Mopidy-Spotify build chain for the Raspberry Pi 5 / k3s cluster
(namespace `snapserver`), with the actual Spotify implementation kept as
locally editable, vendored source rather than a black-box dependency.

## Why this looks the way it does

Mopidy-Spotify 5.x no longer uses libspotify/pyspotify. Spotify playback
goes through a GStreamer element, `spotifyaudiosrc`, implemented in Rust
in [gst-plugins-rs](https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs)
(`audio/spotify`), which is itself built on top of
[librespot](https://github.com/librespot-org/librespot)
(`librespot-core`/`-metadata`/`-audio`/`-playback`/`-oauth`).

Both are vendored here so they can be modified directly:

- `vendor/gst-plugins-rs` — the `spotify-logging` fork/branch from
  `gitlab.freedesktop.org/kingosticks/gst-plugins-rs`, pinned at commit
  `3aab0473`. This is the same baseline the Mopidy project's own
  [gst-plugins-rs-build](https://github.com/mopidy/gst-plugins-rs-build)
  publishes prebuilt `.deb`s from — "better logging" than mainline.
- `vendor/librespot` — upstream `librespot` at tag `v0.8.0` (the version
  `audio/spotify`'s `Cargo.toml` depends on).

`vendor/gst-plugins-rs/Cargo.toml` has a `[patch.crates-io]` section
redirecting `librespot-core`/`-oauth`/`-protocol`/`-metadata`/`-audio`/
`-playback` to `../librespot/...`, so editing anything under
`vendor/librespot` takes effect the next time the plugin is built — no
need to touch version strings or republish a crate.

Mopidy itself and Mopidy-Spotify (the Python backend that drives
`spotifyaudiosrc` and handles config/search/browsing) are **not**
vendored — they're installed from PyPI in the final image. Only the
Rust/librespot side was in scope for local development.

## Build chain

```
┌─────────────────────────┐
│ rust-cross-builder stage│  debian:trixie-slim, native amd64 (NO QEMU)
│  - gcc-aarch64-linux-gnu │  cargo build --release \
│  - rustup target add     │    --target aarch64-unknown-linux-gnu \
│    aarch64-unknown-linux-gnu  --package gst-plugin-spotify
└───────────┬──────────────┘
            │ libgstspotify.so (AArch64 ELF)
            ▼
┌─────────────────────────┐
│ final stage              │  debian:trixie-slim, linux/arm64 (QEMU is fine
│  - gstreamer runtime      │  here - just unpacking apt/pip packages)
│  - pip install Mopidy,    │
│    Mopidy-Spotify, ...    │
└───────────────────────────┘
```

The Rust build deliberately runs **natively on the amd64 build host**
via cross-compilation (`gcc-aarch64-linux-gnu` linker + Rust's
`aarch64-unknown-linux-gnu` target), not under QEMU emulation. QEMU user-mode
emulation can use multiple host cores (each parallel `cargo` job is its own
emulated process, and MTTCG lets one multi-threaded process span cores too),
but every core pays a per-instruction translation penalty - measured at
roughly 2.5 minutes natively for this plugin's full dependency tree
(librespot + gstreamer-rs + tokio stack), where emulated compilation would be
substantially slower per core. Only the final stage (apt/dpkg/pip, no
compilation) runs under QEMU, which is cheap.

`debian:trixie-slim` is required for both stages' *output* — `trixie` is
the first Debian release with the 64-bit-time_t glib ABI
(`libglib2.0-0t64`), which is what current `gst-plugins-rs` builds link
against. This is unrelated to the Pi5 nodes' host OS (Debian 12/bookworm
on the kernel side) — only the container's own userland matters here, not
the host.

## Build & push

```sh
# one-off, if the "multi-arch" buildx builder doesn't exist yet:
make bootstrap-builder

make push            # builds for linux/arm64 and pushes to k8s1:31320/mopidy_stable:latest
make deploy           # optional: force-restart instead of waiting for keel's poll interval
```

`REGISTRY`, `IMAGE`, `TAG` and `BUILDER` are all overridable, e.g.
`make push TAG=dev`.

The `mopidy*` Deployments in namespace `snapserver` already run with
`keel.sh/policy: force` + `keel.sh/trigger: poll`, so pushing `:latest`
gets picked up and rolled out automatically without `make deploy`.

## Updating the vendored sources

```sh
# re-vendor gst-plugins-rs at a newer commit on the same fork/branch
rm -rf vendor/gst-plugins-rs
git clone --branch spotify-logging https://gitlab.freedesktop.org/kingosticks/gst-plugins-rs.git vendor/gst-plugins-rs
rm -rf vendor/gst-plugins-rs/.git
# then re-apply the [patch.crates-io] block at the bottom of vendor/gst-plugins-rs/Cargo.toml

# re-vendor librespot at a newer tag
rm -rf vendor/librespot
git clone --depth 1 --branch vX.Y.Z https://github.com/librespot-org/librespot.git vendor/librespot
rm -rf vendor/librespot/.git
```

## Known gaps / not yet addressed

- The deployed `mopidy.conf` (see `mopidy1-config` ConfigMap) has `[m3u]`
  and `[file]` sections, but `Mopidy-M3U` is abandoned upstream
  (Python 2-only on PyPI) and `Mopidy-File` was never published as a
  separate package — neither is installed in the final image. Needs a
  decision on whether to drop those sections, find a maintained fork, or
  reimplement minimally.
- `k8s/mopidy1-deployment.yaml` is a trimmed mirror of the live
  Deployment as a starting point; `mopidy2`-`mopidy5` aren't captured yet.
