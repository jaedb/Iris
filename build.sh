#!/bin/bash

if [ ! -d public_html ]; then
	ln -s mopidy_iris/static public_html
fi

cp src/index.html public_html/index.html
cp src/.htaccess public_html/.htaccess
cp -R src/fonts public_html/fonts