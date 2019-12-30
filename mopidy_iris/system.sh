#!/bin/bash

echo -e "Running $1 yeah nah"

if [[ "$(pwd)" = "/iris/mopidy_iris/system.sh" ]]; then
	IS_CONTAINER=true
else
	IS_CONTAINER=false
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

	RESTART="$(sudo service mopidy restart)"
	echo -e "${RESTART}"

elif [[ $1 = "local_scan" ]]; then

	if [[ $IS_CONTAINER ]]; then
		SCAN="$(sudo -u mopidy mopidy local scan)"
	else
		SCAN="$(sudo mopidyctl local scan)"
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