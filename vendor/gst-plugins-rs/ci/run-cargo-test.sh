#! /bin/bash

set -ex

rustc --version
cargo --version

build_cfg="--locked --all --all-targets"

cpus=$(nproc || sysctl -n hw.ncpu)
CARGO_FLAGS="--color=always -j${FDO_CI_CONCURRENT:-$cpus}"
CARGO_NEXTEST_FLAGS="--profile=ci --no-tests=pass"

parent="${CI_PROJECT_DIR:-$(pwd)}"

new_report_dir="$parent/junit_reports"
mkdir -p "$new_report_dir"

for cfg in "",default "--all-features --exclude gst-plugin-gtk4",all "--no-default-features",no-default; do
    IFS="," read cfg junit <<< "${cfg}"

    echo "Building and testing with $cfg"

    cargo build $CARGO_FLAGS $build_cfg $cfg
    RUST_BACKTRACE=1 G_DEBUG=fatal_warnings cargo nextest run $CARGO_NEXTEST_FLAGS $CARGO_FLAGS $build_cfg $cfg

    mv "$parent/target/nextest/ci/junit.xml" "$new_report_dir/junit-$junit.xml"
done
