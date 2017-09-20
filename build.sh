#!/bin/bash

# process production html
BUILD=$(date +%s)
VERSION=$(cat VERSION.md)

echo -e "Building $VERSION ($BUILD)"

# copy assets
echo -e Copying assets $SERVER
rsync -avr src/assets/ mopidy_iris/static/assets/ 2>&1 >/dev/null

# create HTML files
echo -e Copying index.html
cp src/index.html mopidy_iris/static/index.html

echo -e "Setting version in static HTML"
sed -i 's/VERSION_HERE/'$VERSION'/g' mopidy_iris/static/index.html
sed -i 's/BUILD_HERE/'$BUILD'/g' mopidy_iris/static/index.html

echo -e "Setting version in Python"
sed -i "/__version__ = '/c\__version__ = '"$VERSION"'" mopidy_iris/__init__.py

echo -e "Setting version in NPM"
sed -i '/	"version": "/c\	"version": "'$VERSION'",' package.json

echo -e "\x1b[32;01m"Done!"\x1b[39;49;00m"