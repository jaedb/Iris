#!/bin/sh

if [ -z "$PULSE_COOKIE_DATA" ]
then
    printf '%s' "$(echo $PULSE_COOKIE_DATA | sed -e 's/../\\x&/g')" >$HOME/pulse.cookie
    export PULSE_COOKIE=$HOME/pulse.cookie
fi

if [ ${PIP_PACKAGES:+x} ]; then
    echo "-- INSTALLING PIP PACKAGES $PIP_PACKAGES --"
    python3 -m pip install --no-cache --upgrade $PIP_PACKAGES
fi

exec "$@"
