#!/bin/sh
set -e

# Existing deployments mount the config ConfigMap at
# /root/.config/mopidy/mopidy.conf, so we keep running as root/HOME=/root
# to stay a drop-in replacement for the currently deployed image.
if [ -n "$TZ" ] && [ -f "/usr/share/zoneinfo/$TZ" ]; then
    ln -snf "/usr/share/zoneinfo/$TZ" /etc/localtime
    echo "$TZ" > /etc/timezone
fi

exec "$@"
