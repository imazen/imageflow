#!/bin/bash
set -e #Exit on failure.
echo "travis_run_osx.sh:"

set -x OPENSSL_INCLUDE_DIR /usr/local/opt/openssl/include
set -x OPENSSL_ROOT_DIR /usr/local/opt/openssl
set -x OPENSSL_LIB_DIR /usr/local/opt/openssl/lib

./build.sh

