#!/bin/bash

if [[ "$(pwd)" = "/iris" ]]; then
	IS_CONTAINER=true
	echo -e "Detected as running in a Docker container"
else
	IS_CONTAINER=false
	echo -e "Not running in a Docker container"
fi

if [[ $1 = "upgrade" ]]; then
	if [[ $IS_CONTAINER ]]; then
		echo "cd /iris && git checkout master && git pull origin master"
		UPGRADE="$(cd /iris && git checkout master && git pull origin master)"
	else
		echo "sudo pip install --upgrade mopidy-iris"
		UPGRADE="$(sudo pip install --upgrade mopidy-iris)"
	fi
	echo -e "${UPGRADE}"

elif [[ $1 = "restart" ]]; then
	if [[ $IS_CONTAINER ]]; then
		echo -e "Cannot restart Mopidy when running in a Docker container"
		exit 1
	else
		RESTART="$(sudo service mopidy restart)"
		echo -e "${RESTART}"
	fi

elif [[ $1 = "local_scan" ]]; then

	if [[ $IS_CONTAINER ]]; then
		SCAN="$(mopidy --config /config/mopidy.conf local scan)"
	else
		SCAN="$(sudo mopidyctl local scan)"
	fi
	echo -e "${SCAN}"

elif [[ $1 = "check" ]]; then
	echo -e "Access permitted"

elif [[ $1 = "test" ]]; then

	sleep 3

	TEST="$(echo 'Hello, this is your bash speaking. I was sleeping for 3 seconds.')"
	echo -e "${TEST}"

else
	echo -e "Unsupported system task"
	exit 1
fi

exit 0