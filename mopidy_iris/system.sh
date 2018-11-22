#!/bin/bash

if [[ $1 = "upgrade" ]]; then

	UPGRADE="$(pip install --upgrade mopidy-iris)"
	echo -e "${UPGRADE}"

elif [[ $1 = "restart" ]]; then

	RESTART="$(service mopidy restart)"
	echo -e "${RESTART}"

elif [[ $1 = "local_scan" ]]; then

	SCAN="$(mopidyctl local scan)"
	echo -e "${SCAN}"

elif [[ $1 = "test" ]]; then

	sleep 10

	TEST="$(echo 'Hello, this is your bash speaking. I was sleeping for 10 seconds.')"
	echo -e "${TEST}"
fi

exit 0