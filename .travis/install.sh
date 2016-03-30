#!/bin/bash

set -e
set -x

if [[ "$(uname -s)" == 'Darwin' ]]; then
    brew update || brew update
    brew install cmake || true
    brew install lcov || true
    brew install conan
else
    cd ${TRAVIS_BUILD_DIR}
    # install latest LCOV
    wget http://ftp.de.debian.org/debian/pool/main/l/lcov/lcov_1.11.orig.tar.gz
    tar xf lcov_1.11.orig.tar.gz
    sudo make -C lcov-1.11/ install
    sudo docker pull lasote/conangcc${GCC_VERSION}
fi

#install lcov to coveralls conversion + upload tool
gem install coveralls-lcov

