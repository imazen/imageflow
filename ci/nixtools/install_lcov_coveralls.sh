#!/bin/bash

set -e #Exit on failure.
set -x

if [[ "$(uname -s)" == 'Darwin' ]]; then
  echo "lcov/coveralls install not supported on OS X"
  exit 1
fi

wget http://ftp.de.debian.org/debian/pool/main/l/lcov/lcov_1.11.orig.tar.gz && \
sudo tar xf lcov_1.11.orig.tar.gz && sudo make -C lcov-1.11/ install

#install lcov to coveralls conversion + upload tool
#crashes on darwin
sudo apt-get install rubygems-integration -y
ls /usr/bin/g*
sudo /usr/bin/gem install coveralls-lcov
