# --- Build Node ---
FROM rust:slim-bookworm AS Builder
LABEL org.opencontainers.image.authors="https://github.com/seppi91"
ARG TARGETPLATFORM
ARG TARGETARCH
ARG TARGETVARIANT

# Print Info about current build Target
RUN printf "I'm building for TARGETPLATFORM=${TARGETPLATFORM}" \
    && printf ", TARGETARCH=${TARGETARCH}" \
    && printf ", TARGETVARIANT=${TARGETVARIANT} \n" \
    && printf "With uname -s : " && uname -s \
    && printf "and  uname -m : " && uname -mm

# Switch to the root user while we do our changes
USER root

# Install all libraries and needs
RUN apt update \
 && apt install -yq --no-install-recommends \
    git \
	patch \
	libgstreamer-plugins-base1.0-dev \
	libgstreamer1.0-dev \
    libcsound64-dev \
	libclang-14-dev \
 	libpango1.0-dev  \
	libdav1d-dev \
	libgtk-4-dev \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/gst-plugins-rs

# Clone source of gst-plugins-rs to workdir
ARG GST_PLUGINS_RS_TAG=0.12.2
RUN git clone -c advice.detachedHead=false \
	--single-branch --depth 1 \
	--branch ${GST_PLUGINS_RS_TAG} \
	https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs.git ./

# Build GStreamer plugins written in Rust (optional with --no-default-features)
ENV DEST_DIR /target/gst-plugins-rs
ENV CARGO_PROFILE_RELEASE_DEBUG false
RUN export CSOUND_LIB_DIR="/usr/lib/$(uname -m)-linux-gnu" \
 && export PLUGINS_DIR=$(pkg-config --variable=pluginsdir gstreamer-1.0) \
 && export SO_SUFFIX=so \
 && cargo build --release --no-default-features \
 # List of packages to build
    --package gst-plugin-spotify \
 # Use install command to create directory (-d), copy and print filenames (-v), and set attributes/permissions (-m)
 && install -v -d ${DEST_DIR}/${PLUGINS_DIR} \
 && install -v -m 755 target/release/*.${SO_SUFFIX} ${DEST_DIR}/${PLUGINS_DIR}


# --- Release Node ---
FROM debian:bookworm-slim as Release

# Switch to the root user while we do our changes
USER root
WORKDIR /

# Install GStreamer and other required Debian packages
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
    sudo \
    build-essential \
    curl \
    git \
    wget \
    gnupg2 \
    dumb-init \
    graphviz-dev \
    pulseaudio \
    libasound2-dev \
    libdbus-glib-1-dev \
    libgirepository1.0-dev \
    # Install Python
    python3-dev \
    python3-gst-1.0 \
    python3-setuptools \
    python3-pip \
    python3-venv \
    # GStreamer (Plugins)
    gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-bad \
    gstreamer1.0-plugins-ugly \
    gstreamer1.0-libav \
    gstreamer1.0-pulseaudio \
 && rm -rf /var/lib/apt/lists/*

# Allow pip to install over system packages
ENV PIP_BREAK_SYSTEM_PACKAGES 1

# Copy builded target data from Builder DEST_DIR to root
# Note: target directory tree links directly to $GST_PLUGIN_PATH
COPY --from=Builder /target/gst-plugins-rs/ /

# Install Node, to build Iris JS application
RUN curl -fsSL https://deb.nodesource.com/setup_18.x | bash - && \
    apt-get install -y nodejs

# Install mopidy and (optional) DLNA-server dleyna from apt.mopidy.com
# see https://docs.mopidy.com/en/latest/installation/debian/
RUN mkdir -p /etc/apt/keyrings \
 && wget -q -O /etc/apt/keyrings/mopidy-archive-keyring.gpg https://apt.mopidy.com/mopidy.gpg \
 && wget -q -O /etc/apt/sources.list.d/mopidy.list https://apt.mopidy.com/bullseye.list \
 && apt-get update \
 && apt-get install -y mopidy \
 && rm -rf /var/lib/apt/lists/*

# Clone Iris from the repository and install in development mode.
# This allows a binding at "/iris" to map to your local folder for development, rather than
# installing using pip.
# Note: ADD helps prevent RUN caching issues. When HEAD changes in repo, our cache will be invalidated!
ADD https://api.github.com/repos/jaedb/Iris/git/refs/heads/master version.json
ENV IRIS_VERSION=develop
RUN git clone --depth 1 --single-branch -b ${IRIS_VERSION} https://github.com/jaedb/Iris.git /iris \
 && cd /iris \
 && npm install \
 && npm run prod \
 && python3 setup.py develop \
 && mkdir -p /var/lib/mopidy/.config \
 && ln -s /config /var/lib/mopidy/.config/mopidy \
 # Allow mopidy user to run system commands (restart, local scan, etc)
 && echo "mopidy ALL=NOPASSWD: /iris/mopidy_iris/system.sh" >> /etc/sudoers \
 # Enable container mode (disable restart option, etc.)
 && echo "1" >> /IS_CONTAINER \
 # Copy Version file
 && cp /iris/VERSION /

# Install Mopidy Spotify
ARG MOPIDY_SPOTIFY_TAG=v5.0.0a1
RUN git clone --depth 1 --single-branch -b ${MOPIDY_SPOTIFY_TAG} https://github.com/mopidy/mopidy-spotify.git mopidy-spotify \
 && cd mopidy-spotify \
 && python3 setup.py install \
 && cd .. \
 && rm -rf mopidy-spotify

# Install additional mopidy extensions and Python dependencies via pip
COPY docker/requirements.txt .
RUN python3 -m pip install -r requirements.txt

# Cleanup
RUN apt-get clean all \
 && rm -rf /var/lib/apt/lists/* \
 && rm -rf /root/.cache \
 && rm -rf /iris/node_modules

# Start helper script.
COPY docker/entrypoint.sh /entrypoint.sh

# Copy Default configuration for mopidy
COPY docker/mopidy/mopidy.example.conf /config/mopidy.conf

# Copy the pulse-client configuratrion
COPY docker/mopidy/pulse-client.conf /etc/pulse/client.conf

# Allows any user to run mopidy, but runs by default as a randomly generated UID/GID.
# RUN useradd -ms /bin/bash mopidy
ENV HOME=/var/lib/mopidy
RUN set -ex \
 && usermod -G audio,sudo,pulse-access mopidy \
 && mkdir /var/lib/mopidy/local \
 && chown mopidy:audio -R $HOME /entrypoint.sh /iris \
 && chmod go+rwx -R $HOME /entrypoint.sh /iris

# Runs as mopidy user by default.
USER mopidy:audio

VOLUME ["/var/lib/mopidy/local"]

EXPOSE 6600 6680 1704 1705 5555/udp

ENTRYPOINT ["/usr/bin/dumb-init", "/entrypoint.sh"]
CMD ["mopidy"]
