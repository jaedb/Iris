#!/bin/bash

if [ ! -d public_html ]; then
	ln -s mopidy_iris/static public_html
fi

cp src/index.html mopidy_iris/static/index.html
cp src/.htaccess mopidy_iris/static/.htaccess
rsync -avr src/assets/ mopidy_iris/static/assets/