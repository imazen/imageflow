#!/bin/bash

set -e #Exit on failure.
set -x

mkdir -p artifacts || true
mkdir -p build || true
cd build
conan install --scope build_tests=True --build missing -u ../
conan build ../

mkdir -p artifacts || true

cd ..
conan export lasote/testing

cd imageflow_core

conan install --build missing # Will build imageflow package with your current settings
cargo build --release
cargo test
cd ..
cd imageflow_tool
cargo build --release
cp target/release/flow-proto1  ../artifacts/
cd ..
