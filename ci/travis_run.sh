#!/bin/bash
set -e

echo "travis_run.sh:"
########## Travis defaults ###################
export IMAGEFLOW_SERVER=${IMAGEFLOW_SERVER:-False}
export COVERAGE=${COVERAGE:-False}
export VALGRIND=${VALGRIND:-False}

########## Travis Overrides ###################
# JOB_BADGE
# GIT_BRANCH_NAME
# UPLOAD_AS_LATEST
# UPLOAD_BUILD

#Use a travis build number so artifacts sort nicely
if [ -z "${TRAVIS_JOB_NUMBER+x}" ]; then
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
fi

#Don't upload pull requests
if [ "${TRAVIS_PULL_REQUEST}" == "false" ]; then
	export UPLOAD_BUILD=${UPLOAD_BUILD:-True}
else
	export UPLOAD_BUILD=False
	export UPLOAD_AS_LATEST=False
fi

if [ -n "${TRAVIS_BUILD_DIR}" ]; then
  cd "${TRAVIS_BUILD_DIR}"
fi



######################################################
#### Parameters passed through docker to build.sh (or used by travis_*.sh) ####

# Not actually used as of 2016-09-16
# Likely to be used by travis_run_docker.sh if we can ever support 'stable'
export RUST_CHANNEL=${RUST_CHANNEL:-nightly}
# Build docs; build release mode binaries (separate pass from testing); populate ./artifacts folder
export BUILD_RELEASE=${BUILD_RELEASE:-True}
# Run all tests (both C and Rust) under Valgrind
export VALGRIND=${VALGRIND:-False}
# Compile and run C tests
export TEST_C=${TEST_C:-True}
# Build C Tests in debug mode for clearer valgrind output
export TEST_C_DEBUG_BUILD=${TEST_C_DEBUG_BUILD:${VALGRIND}}
# Run Rust tests
export TEST_RUST=${TEST_RUST:-True}
# Enable compilation of imageflow_server, which has a problematic openssl dependency
export IMAGEFLOW_SERVER=${IMAGEFLOW_SERVER:-True}
# Enables generated coverage information for the C portion of the code. 
# Also forces C tests to build in debug mode
export COVERAGE=${COVERAGE:-False}
# travis_run.sh deletes /artifacts folder if False. Only relevant in Travis itself
export UPLOAD_BUILD=${UPLOAD_BUILD:-False}
# Affects how /artifacts folder is structured by build.sh
export UPLOAD_AS_LATEST=${UPLOAD_AS_LATEST:-False}
# travis_run_docker.sh uploads Coverage information when true
export COVERALLS=${COVERALLS}
export COVERALLS_TOKEN=${COVERALLS_TOKEN}
# Used by build.sh to determine the package archive name in ./artifacts
export JOB_BADGE="${JOB_BADGE}"

# Used in build.sh for naming things in ./artifacts; also 
# eventually should be embedded in output binaries
# Always ask Git for the commit ID
export GIT_COMMIT
GIT_COMMIT=${GIT_COMMIT:-$(git rev-parse --short HEAD)}
GIT_COMMIT=${GIT_COMMIT:-unknown-commit}
# But let others override GIT_BRANCH_NAME, as HEAD might not have a symbolic ref, and it could crash
# I.e, provide GIT_BRANCH_NAME to this script in Travis
export GIT_BRANCH_NAME
GIT_BRANCH_NAME=${GIT_BRANCH_NAME:-$(git symbolic-ref HEAD | sed -e 's,.*/\(.*\),\1,')}
GIT_BRANCH_NAME=${GIT_BRANCH_NAME:-unknown-branch}

# Used for naming things in ./artifacts
export PACKAGE_PREFIX=${PACKAGE_PREFIX}
export PACKAGE_SUFFIX=${PACKAGE_SUFFIX}

DOCKER_ENV_VARS=(
	-e "RUST_CHANNEL=${RUST_CHANNEL}" 
	-e "BUILD_RELEASE=${BUILD_RELEASE}"
	-e "VALGRIND=${VALGRIND}" 
	-e "TEST_C=${TEST_C}"
	-e "TEST_C_DEBUG_BUILD=${TEST_C_DEBUG_BUILD}"
	-e "TEST_RUST=${TEST_RUST}"
	-e "IMAGEFLOW_SERVER=${IMAGEFLOW_SERVER}"
	-e "COVERAGE=${COVERAGE}" 
	-e "UPLOAD_BUILD=${UPLOAD_BUILD}" 
	-e "UPLOAD_AS_LATEST=${UPLOAD_AS_LATEST}"
	-e "COVERALLS=${COVERALLS}" 
	-e "COVERALLS_TOKEN=${COVERALLS_TOKEN}"
	-e "JOB_BADGE=${JOB_BADGE}" 
	-e "GIT_COMMIT=${GIT_COMMIT}" 
	-e "GIT_BRANCH_NAME=${GIT_BRANCH_NAME}" 
	-e "PACKAGE_PREFIX=${PACKAGE_PREFIX}"  
	-e "PACKAGE_SUFFIX=${PACKAGE_SUFFIX}" 
)

echo 
echo =========================================================
echo "Relevant ENV VARS for build.sh: ${DOCKER_ENV_VARS[*]}"
echo =========================================================
echo 
##############################


if [[ "$(uname -s)" == 'Darwin' ]]; then
	./ci/travis_run_osx.sh
else
	set -x
	docker run --rm -v "$HOME/.ccache:/home/conan/.ccache" -v "$HOME/.conan/data:/home/conan/.conan/data" -v "${TRAVIS_BUILD_DIR}:/home/conan/imageflow" "${DOCKER_ENV_VARS[*]}" "${DOCKER_IMAGE}" /bin/bash -c "./ci/travis_run_docker.sh"
	set +x
fi

if [[ "$UPLOAD_BUILD" != 'True' ]]; then
	echo -e "\nRemvoing all files scheduled for upload to s3\n\n"
	rm -rf ./artifacts/upload
	mkdir -p ./artifacts/upload
else
	echo -e "\nListing files scheduled for upload to s3\n\n"
	ls -R ./artifacts/upload/*
fi

