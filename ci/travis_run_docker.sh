#!/bin/bash

set -e #Exit on failure.
set -x

if [[ "$RUST_CHANNEL" == 'nightly' ]]; then
  ./ci/install_nightly_rust.sh
fi 


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

#We can also build imageflow_server (linux only)
cd imageflow_server
cargo test
cargo build --release
cp target/release/imageflow_server  ../artifacts/staging/
cd ..

if [[ "$COVERALLS" == 'true' ]]; then
  pwd
  echo "*******  Cleaning cov **************"
  sudo chmod -R a+rw .
  lcov --directory ./build --capture --output-file coverage.info # capture coverage info
  lcov --remove coverage.info 'tests/*' '.conan/*' '/usr/*' --output-file coverage.info # filter out system and test code
  lcov --list coverage.info # debug before upload

  echo "******* Uploading to coveralls **************"
  coveralls-lcov --repo-token=${COVERALLS_TOKEN} coverage.info # uploads to coveralls
fi
