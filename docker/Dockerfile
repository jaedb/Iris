FROM circleci/python:3.7.5


# Switch to the root user while we do our changes
USER root

# Install GStreamer and other required Debian packages
RUN apt-get update \
  && apt-get install -y \
    graphviz-dev \
    gstreamer1.0-plugins-bad \
    gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-ugly \
    libasound2-dev \
    python-dev \
    python-gst-1.0 \
    python3-gst-1.0 \
  && rm -rf /var/lib/apt/lists/*

# Make python3-gst-1.0 available to non-Debian Python 3.7 installation
RUN ln -s /usr/lib/python3/dist-packages/gi /usr/local/lib/python3.7/site-packages/gi

# Install libspotify-dev from apt.mopidy.com
RUN wget -q -O - https://apt.mopidy.com/mopidy.gpg \
  | APT_KEY_DONT_WARN_ON_DANGEROUS_USAGE=DontWarn apt-key add - \
  && wget -q -O /etc/apt/sources.list.d/mopidy.list https://apt.mopidy.com/buster.list \
  && apt-get update \
  && apt-get install -y libspotify-dev \
  && rm -rf /var/lib/apt/lists/*

# Install additional Python dependencies
RUN cd /iris \
  && python3.7 setup.py develop --no-cache \
  && python3.7 -m pip install --no-cache tox mopidy-spotify

# Add a VERSION file to the image
ADD VERSION /


# Switch back to the circleci user
USER circleci

CMD ["/bin/sh"]
