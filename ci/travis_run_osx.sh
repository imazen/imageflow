#!/bin/bash

set -e #Exit on failure.
set -x

export TEST_RUST=True
export TEST_C=True
export BUILD_RELEASE=True
export VALGRIND=${VALGRIND:-False}
export COVERAGE=True
export IMAGEFLOW_SERVER=False

./build.sh
