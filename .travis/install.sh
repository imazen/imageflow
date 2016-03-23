#!/bin/bash

set -e
set -x

if [[ "$(uname -s)" == 'Darwin' ]]; then
    brew update || brew update
    brew install cmake || true
    brew install conan
else
    sudo docker pull lasote/conangcc${GCC_VERSION}
fi


