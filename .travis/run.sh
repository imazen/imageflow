#!/bin/bash

set -e
set -x

if [[ "$(uname -s)" == 'Darwin' ]]; then
    .travis/run_tests.sh
else
    if [ -n "$COVERALLS" ]; then
        # sed -i s/%COVERALLS_TOKEN%/${COVERALLS_TOKEN}/g .travis/proc_coveralls.sh
        sudo docker run --rm -e "COVERALLS_TOKEN=${COVERALLS_TOKEN}" -e "VALGRIND=${VALGRIND}" -v $(pwd):/home/conan lasote/conangcc${GCC_VERSION} /bin/bash -c "sudo pip install conan --upgrade && .travis/run_tests.sh && .travis/proc_coveralls.sh"
    else
        sudo docker run --rm -e "VALGRIND=${VALGRIND}" -v $(pwd):/home/conan lasote/conangcc${GCC_VERSION} /bin/bash -c "sudo pip install conan --upgrade && .travis/run_tests.sh"
    fi
fi
