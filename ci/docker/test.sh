#!/bin/bash

rust_channels=(stable nightly)

set -e
set -x
shopt -s extglob

conan user

export SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )/$1"

export IMAGE_NAME=$1

export DOCKER_IMAGE=imazen/$IMAGE_NAME

export RUST_CHANNEL=$2

export JOB_NAME=${IMAGE_NAME}_rust_$RUST_CHANNEL
export WORKING_DIR=${SCRIPT_DIR}/.docker_$JOB_NAME
export SHARED_CACHE=${SCRIPT_DIR}/../.shared_cache

echo $JOB_NAME
mkdir $WORKING_DIR | true
rsync -av --delete "${SCRIPT_DIR}/../../.." "$WORKING_DIR" --filter=':- .gitignore' --exclude=".git/" #--exclude-from "${SCRIPT_DIR}/../exclude_paths.txt" 

cd $WORKING_DIR

export UPLOAD_BUILD=false
export UPLOAD_AS_LATEST=false

export GIT_COMMIT=$(git rev-parse --short HEAD)
export GIT_BRANCH_NAME=$(git symbolic-ref HEAD | sed -e 's,.*/\(.*\),\1,')

export VALGRIND=false
# if [[ "${RUST_CHANNEL}" == 'nightly' ]]; then
#   #export VALGRIND=true
# fi

if [[ "$(uname -s)" == 'Darwin' ]]; then
	eval "$(docker-machine env default)"
fi



export DOCKER_TTY_FLAG=
if [[ -t 1 ]]; then
  export DOCKER_TTY_FLAG="--tty"
fi

#Ensure that .cargo is NOT volume mapped; cargo will not work. Also, cargo fetches faster than rsync, it seems?

docker run --interactive $DOCKER_TTY_FLAG --rm -v ${WORKING_DIR}:/home/conan/imageflow -v ${WORKING_DIR}_cache/wrappers_server_target:/home/conan/imageflow/wrappers/server/target -v ${WORKING_DIR}:/home/conan/imageflow -v ${SHARED_CACHE}/conan_data:/home/conan/.conan/data -v ${WORKING_DIR}_cache/build:/home/conan/imageflow/build  -v ${WORKING_DIR}_cache/ccache:/home/conan/.ccache -e "JOB_NAME=${JOB_NAME}"  -e "UPLOAD_BUILD=false" -e "RUST_CHANNEL=${RUST_CHANNEL}" -e "VALGRIND=${VALGRIND}" -e "GIT_BRANCH_NAME=${GIT_BRANCH_NAME}" -e "GIT_COMMIT=${GIT_COMMIT}" ${DOCKER_IMAGE} /bin/bash -c "./ci/travis_run_docker.sh"  

# uncomment for interactive
#docker run --interactive $DOCKER_TTY_FLAG --rm -v ${WORKING_DIR}:/home/conan/imageflow -v ${WORKING_DIR}_cache/wrappers_server_target:/home/conan/imageflow/wrappers/server/target -v ${WORKING_DIR}:/home/conan/imageflow -v ${SHARED_CACHE}/conan_data:/home/conan/.conan/data -v ${WORKING_DIR}_cache/build:/home/conan/imageflow/build  -v ${WORKING_DIR}_cache/ccache:/home/conan/.ccache -e "JOB_NAME=${JOB_NAME}"  -e "UPLOAD_BUILD=false" -e "RUST_CHANNEL=${RUST_CHANNEL}" -e "VALGRIND=${VALGRIND}" -e "GIT_BRANCH_NAME=${GIT_BRANCH_NAME}" -e "GIT_COMMIT=${GIT_COMMIT}" ${DOCKER_IMAGE} /bin/bash 
