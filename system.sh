#!/bin/bash

ACTION=$1

if [ $ACTION = "upgrade" ]; then
	UPGRADE=$(pip install --upgrade mopidy-iris)
	echo -e "$UPGRADE"

	RESTART=$(service mopidy restart)
	echo -e "$RESTART"

elif [ $ACTION = "restart" ]; then

	RESTART=$(service mopidy restart)
	echo -e "$RESTART"

fi

exit 0