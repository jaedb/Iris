#!/bin/bash
LINKS=`find -type l`
if test -z $LINKS; then
  echo "No symlinks found."
else
  echo "===> FOUND SYMLINKS!"
  echo
  echo "$LINKS"
  echo
  echo "Please replace these with actual files."
  exit 1;
fi
