#!/bin/bash

if [ ! -d public_html ]; then
	ln -s mopidy_iris/static public_html
fi

# copy assets
cp src/.htaccess mopidy_iris/static/.htaccess
rsync -avr src/assets/ mopidy_iris/static/assets/

# create HTML files
cp src/index.html mopidy_iris/static/index.html
cp src/index.html mopidy_iris/static/test.html

# process production html
CACHEBUSTER=$(cat /dev/urandom | tr -dc 'a-zA-Z0-9' | fold -w 16 | head -n 1)
sed -i 's/app.js/app.min.js?'$CACHEBUSTER'/g' mopidy_iris/static/index.html
sed -i 's/app.css/app.min.css?'$CACHEBUSTER'/g' mopidy_iris/static/index.html