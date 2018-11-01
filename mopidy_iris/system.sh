#!/bin/bash

if [[ $1 = "upgrade" ]]; then

	UPGRADE="$(pip install --upgrade mopidy-iris)"
	echo -e "${UPGRADE}"

elif [[ $1 = "restart" ]]; then

	sleep $2

	RESTART="$(service mopidy restart)"
	echo -e "${RESTART}"

elif [[ $1 = "scan" ]]; then

	SCAN="$(mopidyctl local scan)"
	echo -e "${SCAN}"

elif [[ $1 = "test" ]]; then

	sleep 10

	SCAN="$(echo 'Hello, this is your bash speaking')"
	echo -e "${SCAN}"
fi

exit 0