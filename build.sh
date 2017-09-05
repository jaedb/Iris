#!/bin/bash

# copy assets
echo -e Copying assets $SERVER
rsync -avrs src/assets/ mopidy_iris/static/assets/ 2>&1 >/dev/null

# create HTML files
echo -e Copying index.html
cp src/index.html mopidy_iris/static/index.html

# process production html
BUILD_VERSION=$(date | md5sum | cut -f1 -d' ')
echo -e "Injecting build version to js/css URLs ($BUILD_VERSION)"
sed -i 's/BUILD_VERSION_HERE/'$BUILD_VERSION'/g' mopidy_iris/static/index.html

echo -e "\x1b[32;01m"Done!"\x1b[39;49;00m"