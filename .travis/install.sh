#!/bin/bash

set -e
set -x

if [[ "$(uname -s)" == 'Darwin' ]]; then
    brew update || brew update
    brew install cmake || true
    brew install lcov || true
    brew install conan
    brew install nasm
else
    cd ${TRAVIS_BUILD_DIR}
    sudo docker pull lasote/conangcc${GCC_VERSION}
fi


