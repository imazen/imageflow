#!/bin/bash

#Installs python2

set -e #Exit on failure.
set -x

if [[ "$(uname -s)" == 'Darwin' ]]; then
  echo "kcov install not supported on OS X"
  exit 1
fi

#if python2 is missing, install it
command -v python2 >/dev/null 2>&1 || { sudo apt-get install --no-install-recommends -y \
   python2.7-minimal && sudo ln -sf /usr/bin/python2.7 /usr/bin/python2; }


sudo apt-get install --no-install-recommends -y \
  libcurl4-openssl-dev libelf-dev libdw-dev cmake


wget -O kcov.tar.gz https://github.com/SimonKagstrom/kcov/archive/master.tar.gz \
    && tar xvzf kcov.tar.gz && rm kcov.tar.gz && mv kcov-master kcov \
    && mkdir kcov/build \
    && cd kcov/build \
    && cmake .. \
    && make \
    && sudo make install \
    && cd ../.. \
    && rm -rf kcov
