# Mopidy + Mopidy-Spotify for arm64 (Raspberry Pi 5 / k3s in namespace "snapserver")
#
# Mopidy-Spotify 5.x dropped libspotify/pyspotify in favour of the
# "spotifyaudiosrc" GStreamer element from gst-plugins-rs (Rust, built on
# librespot-core/-metadata/-playback). We vendor a fork of gst-plugins-rs
# at vendor/gst-plugins-rs (kingosticks' "spotify-logging" branch, the
# basis the Mopidy project itself currently builds from) so its
# audio/spotify plugin can be modified locally.
#
# That plugin is compiled in a dedicated Rust build stage that always
# runs NATIVELY, never under QEMU emulation - compiling Rust under QEMU
# works but is roughly an order of magnitude slower per core
# (instruction-by-instruction translation). What "native" means depends
# on where the build actually runs, detected at RUN-time (dpkg
# --print-architecture vs TARGETARCH), not assumed in advance:
#
# - On this cluster's CI (GitLab Runner's Kubernetes executor, arm64
#   nodes only): a plain native arm64 build, no cross toolchain at all.
# - Building from an amd64 dev machine via `docker buildx` instead: the
#   original cross-compilation path - gcc-aarch64-linux-gnu + a Rust
#   aarch64-unknown-linux-gnu target, mirroring what the upstream build
#   images at github.com/mopidy/gst-plugins-rs-build do.
#
# Only the final stage needs to actually be arm64 (it just unpacks .deb
# packages and pip wheels, which QEMU emulates cheaply if needed at all).
# Manual build from an amd64 machine:
#
#   docker buildx build --builder multi-arch --platform linux/arm64 \
#     -t k8s1:31320/mopidy_stable:latest --push .
#
# CI (this cluster) builds natively via Kaniko instead - see .gitlab-ci.yml.

# Stage 1 of 2 for Rust compilation: install the toolchain.
# Separated from the cargo build stage so that apt/rustup layers are only
# re-run when the toolchain definition itself changes, not when sources change.
# Cache mounts keep apt package downloads across builds even on cold layer cache.
#
# --platform=$BUILDPLATFORM only actually does anything under `docker
# buildx` - Kaniko (what this cluster's GitLab CI runner uses, since the
# Kubernetes executor has no privileged/DinD access for a real Docker
# daemon) ignores it entirely and always builds as its own pod's real
# host architecture. That's fine and not a bug to route around: every
# node in this cluster is arm64, so a Kaniko build here already *is* a
# native arm64 build with no BUILDPLATFORM pin needed. The distinction
# that actually matters is computed at RUN-time below (dpkg
# --print-architecture, i.e. "what am I actually running as right now"),
# which gives the right answer regardless of whether it's Kaniko (always
# reports the pod's real arch) or buildx from an amd64 dev machine
# (reports amd64 in this pinned stage, thanks to --platform=$BUILDPLATFORM
# still being honored there).
FROM --platform=$BUILDPLATFORM debian:trixie-slim AS rust-toolchain
# Defaults to arm64 (this project's only real target) so a Kaniko
# build - which, unlike buildx, does not auto-populate this from a
# --platform flag - still gets a correct, non-empty value even
# without an explicit --build-arg. CI passes one explicitly anyway
# (see .gitlab-ci.yml) - this default is a backstop, not the primary
# mechanism.
ARG TARGETARCH=arm64
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt/lists,sharing=locked \
    set -eu; \
    HOST_ARCH="$(dpkg --print-architecture)"; \
    apt-get update; \
    apt-get install -y --no-install-recommends \
        build-essential pkg-config curl ca-certificates clang git; \
    if [ "${HOST_ARCH}" = "${TARGETARCH}" ]; then \
        echo "Host (${HOST_ARCH}) already matches target (${TARGETARCH}) - native build, no cross toolchain"; \
        apt-get install -y --no-install-recommends \
            libgstreamer1.0-dev \
            libgstreamer-plugins-base1.0-dev; \
    else \
        echo "Cross-compiling for ${TARGETARCH} from ${HOST_ARCH}"; \
        dpkg --add-architecture "${TARGETARCH}"; \
        apt-get update; \
        apt-get install -y --no-install-recommends \
            gcc-aarch64-linux-gnu \
            "libgstreamer1.0-dev:${TARGETARCH}" \
            "libgstreamer-plugins-base1.0-dev:${TARGETARCH}"; \
    fi
RUN curl -fsSL https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.86.0 --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"
RUN set -eu; \
    if [ "$(dpkg --print-architecture)" != "${TARGETARCH}" ]; then \
        rustup target add aarch64-unknown-linux-gnu; \
    fi

# Stage 2 of 2: compile the Spotify GStreamer plugin against the vendored sources.
FROM rust-toolchain AS rust-cross-builder
ARG TARGETARCH=arm64

COPY vendor/librespot /librespot
COPY vendor/gst-plugins-rs /src
WORKDIR /src
RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/src/target \
    set -eu; \
    if [ "$(dpkg --print-architecture)" = "${TARGETARCH}" ]; then \
        cargo build --release --package gst-plugin-spotify; \
        cp target/release/libgstspotify.so /tmp/libgstspotify.so; \
    else \
        export PKG_CONFIG_ALLOW_CROSS=1; \
        export PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig; \
        export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc; \
        export CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc; \
        cargo build --release --target aarch64-unknown-linux-gnu --package gst-plugin-spotify; \
        cp target/aarch64-unknown-linux-gnu/release/libgstspotify.so /tmp/libgstspotify.so; \
    fi

# Build our Iris fork (vendor/iris, github.com/FlatterAtMainz/Iris) instead
# of pulling the stock Mopidy-Iris package from PyPI. The wheel it produces
# is architecture-independent (frontend assets + pure-Python package), so
# this runs natively on the amd64 build host, same rationale as the Rust
# toolchain stage above.
FROM --platform=$BUILDPLATFORM node:18-slim AS iris-builder
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt/lists,sharing=locked \
    apt-get update && apt-get install -y --no-install-recommends \
        python3 python3-pip python3-venv
COPY vendor/iris /iris
WORKDIR /iris
RUN --mount=type=cache,target=/root/.npm \
    npm ci && npm run prod
RUN python3 -m pip install --break-system-packages build \
    && python3 -m build --wheel --no-isolation -o /dist

FROM debian:trixie-slim AS final
ARG TZ=Europe/Berlin
ENV TZ=${TZ} \
    DEBIAN_FRONTEND=noninteractive \
    PYTHONUNBUFFERED=1

# apt/pip layers are deliberately ordered *before* the COPY of the
# freshly-built libgstspotify.so below: that .so changes on every build
# (it's recompiled from vendor/), so any layer after its COPY cache-busts
# on every build too. Keeping these expensive, rarely-changing installs
# upstream of it means they stay cached across rebuilds that only touch
# the Rust sources. The cache mounts additionally avoid re-downloading
# packages even when the install list itself changes.
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt/lists,sharing=locked \
    apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        tzdata \
        python3 \
        python3-pip \
        python3-gi \
        gir1.2-glib-2.0 \
        gir1.2-gstreamer-1.0 \
        gir1.2-gst-plugins-base-1.0 \
        gstreamer1.0-plugins-base \
        gstreamer1.0-plugins-good \
        gstreamer1.0-plugins-bad \
        gstreamer1.0-tools

# Mopidy core + extensions used by mopidy.conf.
#
# Mopidy itself is built from vendor/mopidy instead of pip-installed: it's
# our FlatterAtMainz/mopidy fork (branch stream-error-fix-v4.0.1, based on
# the v4.0.1 release tag) with a stream_error auto-recovery patch - when a
# GStreamer stream error stops audio (e.g. a broken tcpclientsink to
# snapserver), core.playback now retries the current track a few times
# instead of leaving Mopidy's state stuck on "playing" while nothing is
# actually being sent downstream.
#
# 4.0.1 is used rather than bleeding-edge main because Mopidy-Spotify and
# Mopidy-InternetArchive's own version pins below were only verified
# against it; jumping to a newer Mopidy without re-checking would drop
# that guarantee for no benefit here.
#
# This is what unblocked the move off the old 3.4.2 pin: Mopidy 4 rewrote
# its models around pydantic and deleted the old
# mopidy.models.serialize.ModelJSONEncoder class entirely, which stock
# Mopidy-Iris (3.70.0) still imports directly and which broke Iris on
# Mopidy 4. Our vendor/iris fork already carries the upstream jaedb/Iris
# fix for this (branching serialization on a MOPIDY_V4 feature check), so
# that blocker no longer applies. Mopidy-Spotify and Mopidy-InternetArchive
# are bumped to their current stable releases (5.0.0 / 4.0.1), the first
# versions that require Mopidy>=4.0.0 - diffed against the previously
# pinned 5.0.0a3 to confirm the spotifyaudiosrc GStreamer property
# interface our librespot/gst-plugins-rs patches rely on is unchanged.
#
# setuptools-scm can't infer a version from vendor/mopidy (COPY'd in
# without its .git history), so SETUPTOOLS_SCM_PRETEND_VERSION_FOR_MOPIDY
# pins it explicitly to match the vendored tag.
#
# setuptools<81 is required: Mopidy-Iris still does
# `from pkg_resources import ...`, which setuptools 81+ no longer bundles.
# Without this pin, Iris fails to load at startup. This applies equally to
# our Iris fork below, which hasn't touched that import.
#
# Mopidy-Local and Mopidy-YouTube are intentionally absent:
# - Mopidy-Local 3.2.1 imports `imghdr`, removed from Python 3.13.
# - Mopidy-YouTube has the same compat issue and is unused here.
# Both would fail at startup on debian:trixie-slim (Python 3.13).
COPY vendor/mopidy /tmp/mopidy-src
RUN --mount=type=cache,target=/root/.cache/pip \
    SETUPTOOLS_SCM_PRETEND_VERSION_FOR_MOPIDY=4.0.1 \
    python3 -m pip install --break-system-packages \
        "setuptools<81" \
        /tmp/mopidy-src \
        "Mopidy-Spotify==5.0.0" \
        "Mopidy-InternetArchive==4.0.1" \
    && rm -rf /tmp/mopidy-src

# Our Iris fork (vendor/iris), built by the iris-builder stage above, in
# place of the stock Mopidy-Iris PyPI package.
COPY --from=iris-builder /dist/*.whl /tmp/
RUN --mount=type=cache,target=/root/.cache/pip \
    python3 -m pip install --break-system-packages /tmp/*.whl \
    && rm -f /tmp/*.whl

COPY --from=rust-cross-builder /tmp/libgstspotify.so /usr/lib/aarch64-linux-gnu/gstreamer-1.0/libgstspotify.so
RUN gst-inspect-1.0 spotifyaudiosrc >/dev/null

EXPOSE 6600 6680
VOLUME ["/var/lib/mopidy"]

COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
CMD ["mopidy"]
