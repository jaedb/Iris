FROM debian:buster

RUN set -ex \
 && apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y \
       curl \
       dumb-init \
       gcc \
       gnupg \
       python3-pip \
       gstreamer1.0-alsa \
       gstreamer1.0-plugins-bad \
       python3-gst-1.0 \
       git \
       nano \
       sudo \
 && python3 -m pip install --pre mopidy \
 && git clone https://github.com/jaedb/Iris.git /iris \
 && cd /iris \
 && mkdir -p /var/lib/mopidy/.config \
 && ln -s /config /var/lib/mopidy/.config/mopidy

# Start helper script.
COPY docker/entrypoint.sh /entrypoint.sh

# Default configuration.
COPY docker/mopidy.example.conf /config/mopidy.conf

# Copy the pulse-client configuratrion.
COPY docker/pulse-client.conf /etc/pulse/client.conf

# Allows any user to run mopidy, but runs by default as a randomly generated UID/GID.
ENV HOME=/var/lib/mopidy

EXPOSE 6600 6680

CMD ["/bin/bash"]
#ENTRYPOINT ["/usr/bin/dumb-init", "/entrypoint.sh"]
#CMD ["mopidy"]
