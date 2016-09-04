#!/bin/bash

set -e
set -x

cd ${TRAVIS_BUILD_DIR}

#Just ask git for the short commit hash; ignore travis
export GIT_COMMIT=$(git rev-parse --short HEAD)

#Use a travis build number so artifacts sort nicely
if [ -z ${TRAVIS_JOB_NUMBER+x} ]; then
  echo "TRAVIS_JOB_NUMBER is missing"
else
	export JOB_BADGE="travisjob-${TRAVIS_JOB_NUMBER}"
fi 

#Put tagged commits in their own folder instead of using the branch name
if [ -n "${TRAVIS_TAG}" ]; then
  export GIT_BRANCH_NAME=${TRAVIS_TAG}
  export UPLOAD_AS_LATEST=False
else
	export GIT_BRANCH_NAME=${TRAVIS_BRANCH}
	export UPLOAD_AS_LATEST=True
else

#Don't upload pull requests
if [ "${TRAVIS_PULL_REQUEST}" == "false" ]; then
	export UPLOAD_BUILD=${UPLOAD_BUILD:-true}
else
	export UPLOAD_BUILD=false
	export UPLOAD_AS_LATEST=false
fi

#
if [[ "$(uname -s)" == 'Darwin' ]]; then
    ${TRAVIS_BUILD_DIR}/ci/travis_run_osx.sh
else
    docker run --rm -v $HOME/.ccache:/home/conan/.ccache -v $HOME/.conan/data:/home/conan/.conan/data -v ${TRAVIS_BUILD_DIR}:/home/conan/imageflow -e "COVERALLS_TOKEN=${COVERALLS_TOKEN}" -e "JOB_NAME=${JOB_NAME}"  -e "UPLOAD_BUILD=${UPLOAD_BUILD}" -e "RUST_CHANNEL=${RUST_CHANNEL}" -e "COVERALLS=${COVERALLS}" -e "VALGRIND=${VALGRIND}" -e "GIT_BRANCH_NAME=${GIT_BRANCH_NAME}" -e "GIT_COMMIT=${GIT_COMMIT}" -e "JOB_BADGE=${JOB_BADGE}" -e "PACKAGE_PREFIX=${PACKAGE_PREFIX}"  -e "PACKAGE_SUFFIX=${PACKAGE_SUFFIX}" -e "UPLOAD_AS_LATEST=${UPLOAD_AS_LATEST}"  ${DOCKER_IMAGE} /bin/bash -c "./ci/travis_run_docker.sh"
fi

if [[ "$UPLOAD_BUILD" != 'true' ]]; then
	echo -e "\nRemvoing all files scheduled for upload to s3\n\n"
	rm -rf ./artifacts/upload
	mkdir -p ./artifacts/upload
else
	echo -e "\nListing files scheduled for upload to s3\n\n"
	ls -R ./artifacts/upload/*
fi

