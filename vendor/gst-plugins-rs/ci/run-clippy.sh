#! /bin/bash

set -ex

rustc --version
cargo --version
cargo clippy --version

cpus=$(nproc || sysctl -n hw.ncpu)
CARGO_FLAGS="--color=always -j${FDO_CI_CONCURRENT:-$cpus}"

for cfg in "" "--all-features --exclude gst-plugin-gtk4" "--no-default-features"; do
    cargo clippy $CARGO_FLAGS --locked --all --all-targets $cfg -- $CLIPPY_LINTS
done
