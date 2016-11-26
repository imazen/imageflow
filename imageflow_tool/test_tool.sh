#!/bin/bash
set -e
set -x

cargo build --bin imageflow_tool

TOOL="../target/debug/imageflow_tool"


$TOOL --help

$TOOL diagnose --show-compilation-info

$TOOL --version
$TOOL -V

$TOOL diagnose --self-test