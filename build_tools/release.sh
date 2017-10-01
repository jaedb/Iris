#!/bin/bash

VERSION=$(cat VERSION.md)

echo -e "Releasing $VERSION"

echo -e "git add -A"
ADD=$(git add -A)
echo -e $ADD

echo -e "git commit -m 'Releasing $VERSION'"
COMMIT=$(git commit -m "Releasing $VERSION")
echo -e $COMMIT

echo -e "git tag -a '$VERSION' -m 'Releasing $VERSION'"
TAG=$(git tag -a "$VERSION" -m "Releasing $VERSION")
echo -e $TAG

echo -e "git push origin $VERSION"
PUSH=$(git push origin "$VERSION")
echo -e $PUSH

echo -e "\x1b[32;01m"Done!"\x1b[39;49;00m"