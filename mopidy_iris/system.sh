#!/bin/bash

if [ $1 = "upgrade" ]; then
	UPGRADE=$(pip install --upgrade mopidy-iris)
	RESTART=$(service mopidy restart)

elif [ $1 = "restart" ]; then
	RESTART=$(service mopidy restart)

fi

exit 0