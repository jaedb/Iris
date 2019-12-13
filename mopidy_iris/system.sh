#!/bin/bash

if [[ "$(pwd)" = "/iris/mopidy_iris/system.sh" ]]; then
	IS_CONTAINER = true
else
	IS_CONTAINER = false
fi

if [[ $1 = "upgrade" ]]; then
	if [[ $IS_CONTAINER ]]; then
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

	if [[ $IS_CONTAINER ]]; then
		SCAN="$(-u mopidy mopidy local scan)"
	else
		SCAN="$(mopidyctl local scan)"
	fi
	echo -e "${SCAN}"

elif [[ $1 = "test" ]]; then

	sleep 3

	TEST="$(echo 'Hello, this is your bash speaking. I was sleeping for 3 seconds.')"
	echo -e "${TEST}"

else
	echo -e "Unsupported system task"
fi

exit 0