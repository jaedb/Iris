#!/bin/bash

# copy assets
echo -e Copying assets $SERVER
rsync -avr src/assets/ mopidy_iris/static/assets/ 2>&1 >/dev/null

# create HTML files
echo -e Copying index.html
cp src/index.html mopidy_iris/static/index.html

# process production html
BUILD=$(date +%s)
if [ $# -eq 0 ]; then
	VERSION="0.0.0"
else
	VERSION=$1
fi

echo -e "Injecting build details to js/css URLs (version=$VERSION, build=$BUILD)"
sed -i 's/VERSION_HERE/'$VERSION'/g' mopidy_iris/static/index.html
sed -i 's/BUILD_HERE/'$BUILD'/g' mopidy_iris/static/index.html

echo -e "Injecting version to static files"
sed -i '/__version__ = "/c\__version__ = "'$VERSION'"' mopidy_iris/__init__.py
sed -i '/	"version": "/c\	"version": "'$VERSION'",' package.json

echo -e "\x1b[32;01m"Done!"\x1b[39;49;00m"