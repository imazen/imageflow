#!/bin/bash

set -e
set -x

cd ${TRAVIS_BUILD_DIR}

if [[ "$(uname -s)" == 'Darwin' ]]; then
    ./ci/travis_run_osx.sh
else
    sudo docker run --rm -e "COVERALLS_TOKEN=${COVERALLS_TOKEN}" -e "JOB_NAME=${JOB_NAME}"  -e "UPLOAD_BUILD=${UPLOAD_BUILD}" -e "RUST_CHANNEL=${RUST_CHANNEL}" -e "COVERALLS=${COVERALLS}" -e "COVERALLS=${COVERALLS}" -e "VALGRIND=${VALGRIND}" -v $(pwd):/home/conan ${DOCKER_IMAGE} /bin/bash -c "./ci/travis_run_docker.sh"
fi
