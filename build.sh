#!/bin/bash

# copy assets
echo -e Copying assets $SERVER
rsync -avr src/assets/ mopidy_iris/static/assets/ 2>&1 >/dev/null

# create HTML files
echo -e Copying index.html
cp src/index.html mopidy_iris/static/index.html

# process production html
BUILD_NAME=$(date +%s)
echo -e "Injecting build name to js/css URLs ($BUILD_NAME)"
sed -i 's/BUILD_NAME_HERE/'$BUILD_NAME'/g' mopidy_iris/static/index.html

echo -e "\x1b[32;01m"Done!"\x1b[39;49;00m"