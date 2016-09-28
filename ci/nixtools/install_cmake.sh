#!/bin/bash

#Installs python2 

set -e #Exit on failure.
set -x

wget https://s3.amazonaws.com/public-unit-test-resources/cmake-3.4.1-Linux-x86_64.tar.gz \
    && tar -xzf cmake-3.4.1-Linux-x86_64.tar.gz \
    && cp cmake-3.4.1-Linux-x86_64/bin/cmake /usr/bin/cmake \
    && cp cmake-3.4.1-Linux-x86_64/bin/ctest /usr/bin/ctest \
    && cp -fR cmake-3.4.1-Linux-x86_64/share/* /usr/share \
    && rm -rf cmake-3.4.1-Linux-x86_64 && rm cmake-3.4.1-Linux-x86_64.tar.gz