#!/bin/bash

VERSION=$(cat VERSION.md)

echo -e "Releasing $VERSION ($BUILD)"

echo -e "git add -A"
ADD=$(git add -A)
echo -e $ADD

echo -e "git commit -m 'Releasing $VERSION ($BUILD)'"
COMMIT=$(git commit -m "Releasing $VERSION ($BUILD)")
echo -e $COMMIT

echo -e "git tag -a '$VERSION' -m 'Releasing $VERSION ($BUILD)'"
TAG=$(git tag -a "$VERSION" -m "Releasing $VERSION ($BUILD)")
echo -e $TAG

echo -e "git push"
PUSH=$(git push)
echo -e $PUSH

echo -e "\x1b[32;01m"Done!"\x1b[39;49;00m"