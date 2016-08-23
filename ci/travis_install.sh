#!/bin/bash

set -e #Exit on failure.
set -x

cd ${TRAVIS_BUILD_DIR}

if [[ "$(uname -s)" == 'Darwin' ]]; then
    brew update || brew update
    brew install cmake || true
    brew install conan nasm openssl
    brew link --force openssl
    export OPENSSL_INCLUDE_DIR=`brew --prefix openssl`/include
    export OPENSSL_LIB_DIR=`brew --prefix openssl`/lib
    ./ci/install_dssim.sh
else
    docker pull ${DOCKER_IMAGE}
fi

cp ./ci/updated_conan_settings.yml ~/.conan/settings.yml
