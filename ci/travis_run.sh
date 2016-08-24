#!/bin/bash

set -e
set -x

cd ${TRAVIS_BUILD_DIR}

if [[ "$(uname -s)" == 'Darwin' ]]; then
    ${TRAVIS_BUILD_DIR}/ci/travis_run_osx.sh
else
    docker run --rm -v $HOME/.ccache:/home/conan/.ccache -v $HOME/.conan/data:/home/conan/.conan/data -v ${TRAVIS_BUILD_DIR}:/home/conan/imageflow -e "COVERALLS_TOKEN=${COVERALLS_TOKEN}" -e "JOB_NAME=${JOB_NAME}"  -e "UPLOAD_BUILD=${UPLOAD_BUILD}" -e "RUST_CHANNEL=${RUST_CHANNEL}" -e "COVERALLS=${COVERALLS}" -e "COVERALLS=${COVERALLS}" -e "VALGRIND=${VALGRIND}" ${DOCKER_IMAGE} /bin/bash -c "./ci/travis_run_docker.sh"
fi

bash ./travis_tar_artifacts.sh
