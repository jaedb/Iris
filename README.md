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
vendored — they're installed from PyPI in the final image, pinned to
`Mopidy==3.4.2` / `Mopidy-Spotify==5.0.0a3` (see "Why Mopidy 3.4.2, not
4.x" below). Only the Rust/librespot side was in scope for local
development.

`vendor/librespot/audio/src/fetch/mod.rs` carries one backported patch:
librespot's CDN fetch accepted whatever URL `storage-resolve` returned
first and gave up entirely on anything other than a 206 response, instead
of trying the next candidate URL. Spotify's CDN intermittently returns
500 for the first URL in the list - on the production account this
reliably broke 100% of track playback (login, search, and metadata all
worked fine; the audio fetch itself didn't). Fixed by backporting
[librespot-org/librespot#1722](https://github.com/librespot-org/librespot/pull/1722)
(open, unmerged upstream as of 2026-06-19).

### Why Mopidy 3.4.2, not 4.x

Mopidy 4 rewrote its models around pydantic and deleted
`mopidy.models.serialize.ModelJSONEncoder` entirely. The current
Mopidy-Iris (3.70.0) still imports that class directly, so Iris fails to
even load its extension on Mopidy 4 - confirmed by testing, not just
reading changelogs. `Mopidy-Spotify` correspondingly has to stay on
`5.0.0a3` (the last alpha supporting `Mopidy>=3.4.0`; the final 5.0.0
requires `Mopidy>=4.0.0`), and `Mopidy-Local`/`Mopidy-InternetArchive` are
pinned to their last Mopidy-3-compatible releases (`3.2.1` / `3.1.2`).
This whole combination matches what's already running in production
(verified via `pip show` on the live pods) - we're not introducing a new
stack, just replacing `libgstspotify.so` with one built from current,
editable sources.

## Build chain

```
┌──────────────────────────┐   ┌──────────────────────────┐
│ rust-toolchain/           │   │ iris-builder stage        │
│ rust-cross-builder stages │   │  - node:18-slim            │
│  - native on this arch, or│   │  - npm ci && npm run prod   │
│    cross-compiled to arm64│   │  - python -m build --wheel  │
│    from an amd64 host      │   └───────────┬──────────────┘
│  cargo build --release \   │               │ Iris wheel (arch-independent)
│    [--target aarch64-...]  │               │
│    --package gst-plugin-   │               │
│    spotify                 │               │
└───────────┬────────────────┘               │
            │ libgstspotify.so (AArch64 ELF)   │
            ▼                                 ▼
┌─────────────────────────────────────────────────┐
│ final stage                                       │  debian:trixie-slim,
│  - gstreamer runtime                               │  linux/arm64 (QEMU is
│  - pip install Mopidy, Mopidy-Spotify, ...          │  fine here - just
│  - pip install the Iris wheel from iris-builder      │  unpacking apt/pip
└───────────────────────────────────────────────────┘  packages)
```

The Rust build only ever compiles natively - never under QEMU emulation,
which works but is roughly an order of magnitude slower per core
(instruction-by-instruction translation) than true native compilation,
measured at roughly 2.5 minutes natively for this plugin's full dependency
tree (librespot + gstreamer-rs + tokio stack). What "native" means
depends on where the build actually runs - see the Dockerfile's top
comment and "Build & push" below: this cluster's CI builds it natively on
arm64 (every node here is arm64), a manual build from an amd64 dev
machine instead cross-compiles (`gcc-aarch64-linux-gnu` + Rust's
`aarch64-unknown-linux-gnu` target) rather than running under QEMU. Only
the final stage (apt/dpkg/pip, no compilation) ever runs under QEMU
emulation, if at all, which is cheap regardless.

`iris-builder` (`npm run prod` + a Python wheel build) produces an
architecture-independent artifact either way - a frontend JS bundle plus
pure-Python source has no native code to get "wrong" the way the Rust
plugin does, so it doesn't need the same native-vs-cross distinction.

`debian:trixie-slim` is required for both stages' *output* — `trixie` is
the first Debian release with the 64-bit-time_t glib ABI
(`libglib2.0-0t64`), which is what current `gst-plugins-rs` builds link
against. This is unrelated to the Pi5 nodes' host OS (Debian 12/bookworm
on the kernel side) — only the container's own userland matters here, not
the host.

## Build & push

`.gitlab-ci.yml` builds both images (`mopidy_stable` and `spotify-auth`)
and force-restarts `mopidy1`-`4` and `spotify-auth` on every push to
`main`, via
[cluster/ci-templates](https://gitlab.lerchenberg-net.de/cluster/ci-templates).
Uses [Kaniko](https://gitlab.lerchenberg-net.de/cluster/ci-templates), not
`docker buildx` — the GitLab Runner's Kubernetes executor has no
privileged/DinD access. Since every node in this cluster is arm64, that
build is already native (no QEMU) with no cross-compilation needed at
all — see the Dockerfile's own top comment for how the Rust build stage
detects this at run-time rather than assuming it.

Manual equivalent (from an amd64 machine, still using the original
cross-compilation path via `docker buildx`):

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
gets picked up and rolled out automatically without `make deploy` (or
CI's `deploy-*` jobs) — those exist for an immediate rollout instead of
waiting out keel's poll interval. `spotify-auth` has no keel annotation
at all, so its CI deploy job is the only thing that rolls it out.

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

- `Mopidy-Local==3.2.1` (pinned per above) imports the stdlib `imghdr`
  module to sniff embedded cover art, which Python 3.13 removed (PEP
  594). The `local` extension currently fails to load
  (`ModuleNotFoundError: No module named 'imghdr'`), so local file
  browsing/playback is broken on this image even though it works in the
  currently-deployed production image (which runs Python 3.11). Needs
  either a small `imghdr` shim module or patching `mopidy_local/storage.py`
  to not need it.
- `Mopidy==3.4.2` is bundled `m3u`/`file`/`stream`/`http`, so unlike an
  earlier version of this README claimed, no separate `Mopidy-M3U`/
  `Mopidy-File` packages are needed - that requirement was an artifact of
  an earlier (abandoned) attempt to build on Mopidy 4 instead.
- `Mopidy-Podcast`, `Mopidy-Mobile`, `Mopidy-Scrobbler`, `Mopidy-MPD` are
  present in production but not yet in this image.
- `k8s/mopidy{1,2,3,4}-deployment.yaml` mirror the live Deployments;
  `mopidy5` isn't captured (it has no Deployment in the cluster, service
  only). `mopidy2`-`4` had drifted - their `conf`/`podcast`
  `volumeMounts` were missing even though `volumes` still referenced
  them, so they were silently running on a blank default config with
  Spotify disabled (not visible until something tried to use Spotify on
  them). Restored to match `mopidy1`.
- All four (`mopidy1`-`4`) are now verified running
  `k8s1:31320/mopidy_stable:latest` with the librespot CDN fix and
  playing real Spotify audio.
