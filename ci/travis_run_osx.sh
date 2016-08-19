#!/bin/bash

set -e #Exit on failure.
set -x

mkdir -p build
cd build

conan install --scope build_tests=True --scope coverage=True --scope valgrind=${VALGRIND} --build missing -u ../
conan build ../
