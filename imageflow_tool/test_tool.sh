#!/bin/bash
set -e
set -x

export RUST_BACKTRACE=1
cargo build --bin imageflow_tool

TOOL="../target/debug/imageflow_tool"


$TOOL --help

$TOOL diagnose --show-compilation-info

$TOOL --version
$TOOL -V

$TOOL diagnose --self-test

(
    mkdir _test || true
    cd _test
    $TOOL examples --generate
)