#!/bin/bash

set -e #Exit on failure.
set -x

mkdir -p artifacts/staging
mkdir -p build
cd build
conan install --scope build_tests=True --scope coverage=True --scope valgrind=${VALGRIND} --build missing -u ../
conan build ../

cd ..
conan export lasote/testing

cd imageflow_core

conan install --build missing # Will build imageflow package with your current settings
cargo build --release
cargo test
cd ..
cd imageflow_tool
cargo test
cargo build --release
cp target/release/flow-proto1  ../artifacts/staging/
cd ..

ls -R ./artifacts/staging/*


