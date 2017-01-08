#!/bin/bash
set -e #Exit on failure.
echo "travis_run_osx.sh:"

export OPENSSL_INCLUDE_DIR=/usr/local/opt/openssl/include
export OPENSSL_ROOT_DIR=/usr/local/opt/openssl
export OPENSSL_LIB_DIR=/usr/local/opt/openssl/lib

#Copy conan settings - always
cp "./ci/updated_conan_settings.yml" "${HOME}/.conan/settings.yml"



./build.sh

