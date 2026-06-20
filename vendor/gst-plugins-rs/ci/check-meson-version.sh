#!/bin/bash

MESON_VERSION=`head -n5 meson.build | grep ' version\s*:' | sed -e "s/.*version\s*:\s*'//" -e "s/',.*//"`
CARGO_VERSION=`cat Cargo.toml | grep -A1 workspace.package | grep ^version | sed -e 's/^version = "\(.*\)"/\1/'`

echo "gst-plugins-rs version (meson.build) : $MESON_VERSION"
echo "gst-plugins-rs version (Cargo.toml)  : $CARGO_VERSION"

if test "x$MESON_VERSION" != "x$CARGO_VERSION"; then
  echo
  echo "===> Version mismatch between meson.build and Cargo.toml! <==="
  echo
  exit 1;
fi
