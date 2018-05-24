#!/bin/bash
set -e #Exit on failure.

STAMP="+[%H:%M:%S]"
date "$STAMP"

cd "${TRAVIS_BUILD_DIR}"

if [[ "$(uname -s)" == 'Darwin' ]]; then
    sysctl -n machdep.cpu.brand_string
    sysctl machdep.cpu.family
    sysctl -n machdep.cpu.features
    sysctl -n machdep.cpu.leaf7_features
    sysctl -n machdep.cpu.extfeatures

    set -x
    brew update || brew update
    date "$STAMP"
    brew install nasm dssim
    set +x
fi

