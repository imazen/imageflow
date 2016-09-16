#!/bin/bash
set -e
shopt -s extglob

echo "Preparing to build Imageflow"

# First parameter to script must be the name of the docker image (excluding imazen/)
export IMAGE_NAME=$1
# Set DOCKER_IMAGE to override entire name
export DOCKER_IMAGE=${DOCKER_IMAGE:-imazen/$IMAGE_NAME}

# OPEN_DOCKER_BASH_INSTEAD=True to open interactive shell
export OPEN_DOCKER_BASH_INSTEAD=${OPEN_DOCKER_BASH_INSTEAD:-False}

# RUST_CHANNEL doesn't do anything right now, just part of some names
export RUST_CHANNEL=${RUST_CHANNEL:-nightly}

############## Overrides for test.sh


# travis_run.sh deletes /artifacts folder if False. Only relevant in Travis itself
export UPLOAD_BUILD=False
# Affects how /artifacts folder is structured by build.sh
export UPLOAD_AS_LATEST=False

######################################################
#### Parameters passed through docker to build.sh ####

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

echo ===================================================================== [test.sh]
echo "DOCKER_ENV_VARS: ${DOCKER_ENV_VARS[@]}"
echo ===================================================================== [test.sh]
##############################

export SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )/$1"
export WORKING_DIR=${SCRIPT_DIR}/.docker_${IMAGE_NAME}_rust_${RUST_CHANNEL}
export SHARED_CACHE=${SCRIPT_DIR}/../.shared_cache


OTHER_VARS=(
	"OPEN_DOCKER_BASH_INSTEAD=${OPEN_DOCKER_BASH_INSTEAD}"
	"IMAGE_NAME=${IMAGE_NAME}"
	"RUST_CHANNEL=${RUST_CHANNEL}"
	"DOCKER_IMAGE=${DOCKER_IMAGE}"
	"WORKING_DIR=${WORKING_DIR}"
	"SHARED_CACHE=${SHARED_CACHE}"
	"SCRIPT_DIR=${SCRIPT_DIR}"
)

echo "OTHER_VARS: ${OTHER_VARS[@]}"
echo ===================================================================== [test.sh]
echo Initializing Conan
echo

conan user

echo ===================================================================== [test.sh]
echo "Rsync imageflow/* into dedicated work folder"
echo

[[ -d "$WORKING_DIR" ]] || mkdir "$WORKING_DIR"
rsync -q -av --delete "${SCRIPT_DIR}/../../.." "$WORKING_DIR" --filter=':- .gitignore' --exclude=".git/" #--exclude-from "${SCRIPT_DIR}/../exclude_paths.txt" 
cd "$WORKING_DIR"


# For os x convenience
if [[ "$(uname -s)" == 'Darwin' ]]; then
	eval "$(docker-machine env default)"
fi

export DOCKER_TTY_FLAG=
if [[ -t 1 ]]; then
  export DOCKER_TTY_FLAG="--tty"
fi


if [[ "$OPEN_DOCKER_BASH_INSTEAD" == 'True' ]]; then
	DOCKER_COMMAND=(
		/bin/bash
		)
else
	DOCKER_COMMAND=(
		/bin/bash -c "./ci/travis_run_docker.sh"  
		)
fi

echo ===================================================================== [test.sh]
echo "Launching docker "
echo
#Ensure that .cargo is NOT volume mapped; cargo will not work. Also, cargo fetches faster than rsync, it seems?
set -x
docker run --interactive $DOCKER_TTY_FLAG --rm -v ${WORKING_DIR}:/home/conan/imageflow -v ${WORKING_DIR}_cache/wrappers_server_target:/home/conan/imageflow/wrappers/server/target -v ${WORKING_DIR}:/home/conan/imageflow -v ${SHARED_CACHE}/conan_data:/home/conan/.conan/data -v ${WORKING_DIR}_cache/build:/home/conan/imageflow/build  -v ${WORKING_DIR}_cache/ccache:/home/conan/.ccache  "${DOCKER_ENV_VARS[@]}" ${DOCKER_IMAGE} "${DOCKER_COMMAND[@]}" 
set +x