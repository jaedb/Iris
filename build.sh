#!/bin/bash

if [ ! -d production ]; then
	mkdir production;
fi

cp src/index.html production/index.html
cp src/.htaccess production/.htaccess
cp -R src/fonts production/fonts