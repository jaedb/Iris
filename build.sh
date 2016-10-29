#!/bin/bash

if [ ! -d public_html ]; then
	mkdir public_html;
fi

cp src/index.html public_html/index.html
cp src/.htaccess public_html/.htaccess
cp -R src/fonts public_html/fonts