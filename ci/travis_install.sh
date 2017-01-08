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
    brew install cmake || true
    brew install --force openssl || true
    brew link openssl --force || true
    brew install conan nasm
    ./ci/nixtools/install_dssim.sh
    set +x
else
    cat /proc/cpuinfo
  
	set -x
    docker pull "${DOCKER_IMAGE}"
    set +x
fi

