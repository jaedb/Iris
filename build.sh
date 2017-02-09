#!/bin/bash

if [ ! -d public_html ]; then
	echo -e Creating mopidy_iris/static folder
	ln -s mopidy_iris/static public_html
fi

# copy assets
echo -e Copying assets $SERVER
cp src/.htaccess mopidy_iris/static/.htaccess
rsync -avrs src/assets/ mopidy_iris/static/assets/ 2>&1 >/dev/null

# create HTML files
echo -e Copying index.html and test.html
cp src/index.html mopidy_iris/static/index.html
cp src/index.html mopidy_iris/static/test.html

# process production html
CACHEBUSTER=$(date | md5sum | cut -f1 -d' ')
echo -e Cachebusting js/css URLs $CACHEBUSTER
sed -i 's/app.js/app.min.js?'$CACHEBUSTER'/g' mopidy_iris/static/index.html
sed -i 's/app.css/app.min.css?'$CACHEBUSTER'/g' mopidy_iris/static/index.html
sed -i 's/app.js/app.js?'$CACHEBUSTER'/g' mopidy_iris/static/test.html
sed -i 's/app.css/app.css?'$CACHEBUSTER'/g' mopidy_iris/static/test.html
sed -i 's/app.css/app.css?'$CACHEBUSTER'/g' mopidy_iris/static/test.html
sed -i 's/<\/head>/\t<script type="text\/javascript">window._testMode = true;<\/script>\n\n<\/head>/g' mopidy_iris/static/test.html

echo -e "\x1b[32;01m"Done!"\x1b[39;49;00m"