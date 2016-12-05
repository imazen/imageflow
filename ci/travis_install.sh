#!/bin/bash
set -e #Exit on failure.

STAMP="+[%H:%M:%S]"
date "$STAMP"

cd "${TRAVIS_BUILD_DIR}"

if [[ "$(uname -s)" == 'Darwin' ]]; then
	set -x
    brew update || brew update
    brew install cmake || true
    brew install --force openssl || true
    brew link openssl --force || true
    brew install conan nasm
    ./ci/nixtools/install_dssim.sh
    set +x
else
	set -x
    docker pull "${DOCKER_IMAGE}"
    set +x
fi

cp ./ci/updated_conan_settings.yml ~/.conan/settings.yml
