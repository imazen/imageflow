#!/bin/bash

set -e
set -x

if [[ "$(uname -s)" == 'Darwin' ]]; then
    .travis/run_tests.sh
else
    sudo docker run --rm -v $(pwd):/home/conan lasote/conangcc${GCC_VERSION} /bin/bash -c "sudo pip install conan --upgrade && .travis/run_tests.sh"	
fi

