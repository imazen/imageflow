#!/bin/bash
set -e #Exit on failure.
echo "travis_run_osx.sh:"

#Copy conan settings - always
cp "./ci/updated_conan_settings.yml" "${HOME}/.conan/settings.yml"

./build.sh

