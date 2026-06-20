# Mopidy + Mopidy-Spotify for arm64 (Raspberry Pi 5 / k3s in namespace "snapserver")
#
# Mopidy-Spotify 5.x dropped libspotify/pyspotify in favour of the
# "spotifyaudiosrc" GStreamer element from gst-plugins-rs (Rust, built on
# librespot-core/-metadata/-playback). We vendor a fork of gst-plugins-rs
# at vendor/gst-plugins-rs (kingosticks' "spotify-logging" branch, the
# basis the Mopidy project itself currently builds from) so its
# audio/spotify plugin can be modified locally.
#
# That plugin is compiled in a dedicated cross-compilation stage that
# runs NATIVELY on the amd64 build host (no QEMU) using the
# gcc-aarch64-linux-gnu cross toolchain + a Rust aarch64-unknown-linux-gnu
# target. Compiling Rust under QEMU emulation works but is roughly an
# order of magnitude slower per core (instruction-by-instruction
# translation), so true cross-compilation is used instead - this mirrors
# what the upstream build images at github.com/mopidy/gst-plugins-rs-build
# do.
#
# Only the final stage needs to actually be arm64 (it just unpacks .deb
# packages and pip wheels, which QEMU emulates cheaply), so the overall
# build is:
#
#   docker buildx build --builder multi-arch --platform linux/arm64 \
#     -t k8s1:31320/mopidy_stable:latest --push .

FROM --platform=$BUILDPLATFORM debian:trixie-slim AS rust-cross-builder
ARG TARGETARCH
RUN dpkg --add-architecture ${TARGETARCH} && apt-get update && apt-get install -y --no-install-recommends \
        build-essential pkg-config curl ca-certificates clang git \
        gcc-aarch64-linux-gnu \
        libgstreamer1.0-dev:${TARGETARCH} \
        libgstreamer-plugins-base1.0-dev:${TARGETARCH} \
    && rm -rf /var/lib/apt/lists/*
RUN curl -fsSL https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.86.0 --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"
RUN rustup target add aarch64-unknown-linux-gnu
ENV PKG_CONFIG_ALLOW_CROSS=1 \
    PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig \
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc

COPY vendor/librespot /librespot
COPY vendor/gst-plugins-rs /src
WORKDIR /src
RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --target aarch64-unknown-linux-gnu --package gst-plugin-spotify \
    && cp target/aarch64-unknown-linux-gnu/release/libgstspotify.so /tmp/libgstspotify.so

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
        gstreamer1.0-plugins-base \
        gstreamer1.0-plugins-good \
        gstreamer1.0-plugins-bad \
        gstreamer1.0-tools

# Mopidy core + extensions used by mopidy.conf (m3u/local/file/http are
# separate extensions again as of Mopidy 4.x, not bundled in core).
RUN --mount=type=cache,target=/root/.cache/pip \
    python3 -m pip install --break-system-packages \
        Mopidy \
        Mopidy-Spotify \
        Mopidy-Local \
        Mopidy-Iris \
        Mopidy-InternetArchive \
        Mopidy-YouTube

COPY --from=rust-cross-builder /tmp/libgstspotify.so /usr/lib/aarch64-linux-gnu/gstreamer-1.0/libgstspotify.so
RUN gst-inspect-1.0 spotifyaudiosrc >/dev/null

EXPOSE 6600 6680
VOLUME ["/var/lib/mopidy"]

COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
CMD ["mopidy"]
