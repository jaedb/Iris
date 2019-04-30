FROM debian:stretch-slim

# Copy the current codebase
COPY . /iris

RUN set -ex \
    # Official Mopidy install for Debian/Ubuntu along with some extensions
    # (see https://docs.mopidy.com/en/latest/installation/debian/ )
 && apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y \
        curl \
        dumb-init \
        gcc \
        gnupg \
        gstreamer1.0-alsa \
        gstreamer1.0-plugins-bad \
        python-crypto \
 && curl -L https://apt.mopidy.com/mopidy.gpg | apt-key add - \
 && curl -L https://apt.mopidy.com/mopidy.list -o /etc/apt/sources.list.d/mopidy.list \
 && apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y \
        mopidy \
        mopidy-soundcloud \
        mopidy-spotify \
 && curl -L https://bootstrap.pypa.io/get-pip.py | python - \
 && pip install -U six pyasn1 requests[security] cryptography \
 && pip install \
        Mopidy-Local-Images \
        Mopidy-Local-SQLite \
        Mopidy-GMusic \
        pyopenssl \
        youtube-dl \
 # Clone Iris from current directory and install in development mode.
 # This allows a binding at "/iris" to map to your local folder for development.
 && cd /iris \
 && python setup.py develop \
 && cd / \
 && mkdir -p /var/lib/mopidy/.config \
 && ln -s /config /var/lib/mopidy/.config/mopidy \
# Install Snapcast
# This is the most reliable way to get audio out of a Docker instance.
 && curl -L https://github.com/badaix/snapcast/releases/download/v0.15.0/snapserver_0.15.0_amd64.deb -o snapserver.deb \
 && dpkg -i snapserver.deb \
 && touch /etc/default/snapserver \
 && apt-get -f install \
# Clean-up
 && apt-get purge --auto-remove -y \
        curl \
        gcc \
        git \
 && apt-get clean \
 && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/* ~/.cache

# Start helper script.
COPY docker/entrypoint.sh /entrypoint.sh

# Default configuration.
COPY docker/mopidy.conf /config/mopidy.conf

# Copy the pulse-client configuratrion.
COPY docker/pulse-client.conf /etc/pulse/client.conf

# Allows any user to run mopidy, but runs by default as a randomly generated UID/GID.
ENV HOME=/var/lib/mopidy
RUN set -ex \
 && usermod -G audio,sudo mopidy \
 && chown mopidy:audio -R $HOME /entrypoint.sh \
 && chmod go+rwx -R $HOME /entrypoint.sh

# Runs as mopidy user by default.
USER mopidy

VOLUME ["/var/lib/mopidy/local", "/var/lib/mopidy/media"]

EXPOSE 6600 6680 1705 5555/udp

ENTRYPOINT ["/usr/bin/dumb-init", "/entrypoint.sh"]
CMD ["/usr/bin/mopidy"]
