#!/bin/bash

if [[ $1 = "upgrade" ]]; then

	UPGRADE="$(pip install --upgrade mopidy-iris)"
	echo -e "${UPGRADE}"

elif [[ $1 = "restart" ]]; then

	sleep $2

	RESTART="$(service mopidy restart)"
	echo -e "${RESTART}"
fi

exit 0