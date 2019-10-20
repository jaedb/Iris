#!/bin/bash

CURR_DIR="$(pwd)"

if [[ $1 = "upgrade" ]]; then
	if [[ $CURR_DIR = "/iris/mopidy_iris/system.sh" ]]; then
		echo "cd /iris && git checkout master && git pull origin master"
		UPGRADE="$(cd /iris && git checkout master && git pull origin master)"
	else
		echo "pip install --upgrade mopidy-iris"
		UPGRADE="$(pip install --upgrade mopidy-iris)"
	fi
	echo -e "${UPGRADE}"

elif [[ $1 = "restart" ]]; then

	RESTART="$(service mopidy restart)"
	echo -e "${RESTART}"

elif [[ $1 = "local_scan" ]]; then

	SCAN="$(mopidyctl local scan)"
	echo -e "${SCAN}"

elif [[ $1 = "test" ]]; then

	sleep 3

	TEST="$(echo 'Hello, this is your bash speaking. I was sleeping for 3 seconds.')"
	echo -e "${TEST}"

else
	echo -e "Unsupported system task"
fi

exit 0