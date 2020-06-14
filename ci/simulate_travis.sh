#!/bin/bash
set -e
shopt -s extglob

#FOR THE CLEANEST TEST - 100% ephemeral.
# DISABLE_COMPILATION_CACHES=True

# To remove (the LARGE) caches this writes to your home directory
# rm -rf ~/.docker_imageflow_caches

#Or, to prevent copying of the host's ~/.cargo directory into the instance
# COPY_HOST_CARGO_DIR=False

#REQUIRES
# 1 param of imazen/imageflow_build_ubuntu16:latest (or whichever image you expect)
# Certain paths are expected within the image

# COMMON OPTIONAL PARAMS
# OPEN_DOCKER_BASH_INSTEAD=True
# VALGRIND=True
# SIM_COVERAGE=True
# TEST_C
# CLEAN_RELEASE
# TEST_RELEASE
# BUILD_RELEASE
# CHECK_DEBUG
# TEST_DEBUG
# BUILD_DEBUG
# CLEAN_RUST_TARGETS
# UPLOAD_BUILD, UPLOAD_DOCS
# DISABLE_COMPILATION_CACHES=True

#OPTIONAL
# SIM_TRAVIS_TAG=v0.0.99999
# DOCKER_IMAGE override
# PACKAGE_SUFFIX like x86_64-linux-gcc48-eglibc219
# FETCH_COMMIT_SUFFIX like mac64
export BUILD_QUIETER="$BUILD_QUIETER"

echo_maybe(){
	if [[ "$BUILD_QUIETER" != "True" ]]; then
			echo "$1"
	fi
}

if [[ "$BUILD_QUIETER" != "True" ]]; then
	exec 9>&1
else
	exec 9>/dev/null
fi

if [[ -z "$1" ]]; then
	echo "You must provide a docker image name as the first argument"
	exit 1
fi


echo_maybe "Preparing to build Imageflow"

# We change this default (not like Travis), but for speed.
# Not relevant when DISABLE_COMPILATION_CACHES=True
export CLEAN_RUST_TARGETS="${CLEAN_RUST_TARGETS:-False}"

export IMAGEFLOW_BUILD_OVERRIDE="${IMAGEFLOW_BUILD_OVERRIDE}"

export COPY_HOST_CARGO_DIR="${COPY_HOST_CARGO_DIR:-True}"


# First parameter to script must be the name of the docker image (excluding imazen/)
export SAFE_IMAGE_NAME="$1"
SAFE_IMAGE_NAME="${SAFE_IMAGE_NAME//\//_}"
SAFE_IMAGE_NAME="${SAFE_IMAGE_NAME//:/_}"

export TARGET_CPU="${TARGET_CPU:-x86-64}"

export CARGO_TARGET="${CARGO_TARGET:-}"

if [[ -n "$CARGO_TARGET" ]]; then
		export TARGET_DIR="target/${CARGO_TARGET}/"
else
		export TARGET_DIR="target/"
fi

# Set DOCKER_IMAGE to override entire name
export DOCKER_IMAGE="${DOCKER_IMAGE:-$1}"

############## Paths for caching
export SCRIPT_DIR
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd)"
export TEST_SH_CACHE_DIR="${HOME}/.docker_imageflow_caches"
#export TEST_SH_CACHE_DIR="${SCRIPT_DIR}/../.docker_imageflow_caches"

export WORKING_DIR="${TEST_SH_CACHE_DIR}/.docker_${SAFE_IMAGE_NAME}_${TARGET_CPU}"
export SHARED_CACHE="${TEST_SH_CACHE_DIR}/.shared_cache"


echo_maybe "===================================================================== [test.sh]"
echo_maybe "Rsync imageflow/* into dedicated work folder ${WORKING_DIR}"
echo_maybe

[[ -d "$WORKING_DIR" ]] || mkdir -p "$WORKING_DIR"
rsync -q -av --delete "${SCRIPT_DIR}/.." "$WORKING_DIR" --filter=':- .gitignore'  --exclude="target/"
(
	cd "$WORKING_DIR"

	############## Set up travis env


	## REQUIRED ALWAYS
	export TRAVIS_BUILD_DIR="${WORKING_DIR}"
	export DOCKER_IMAGE="${DOCKER_IMAGE}"
	export CI="true"

	## REQUIRED FOR SIMULATION
	export SIM_CI="True"
	export SIM_OPEN_BASH="${OPEN_DOCKER_BASH_INSTEAD:-False}"
	export SIM_DOCKER_CACHE_VAR



	mkdir -p "${WORKING_DIR}_cache/${TARGET_DIR}debug" || true
	mkdir -p "${WORKING_DIR}_cache/${TARGET_DIR}release" || true
	mkdir -p "${WORKING_DIR}_cache/.cache" || true


	# The first two are only needed in test.sh, since we're rsyncing away the whole /target folder
	export SIM_DOCKER_CACHE_VARS=(
		-v
		"${WORKING_DIR}_cache/${TARGET_DIR}debug:/home/imageflow/imageflow/${TARGET_DIR}debug"
		-v
		"${WORKING_DIR}_cache/${TARGET_DIR}release:/home/imageflow/imageflow/${TARGET_DIR}release"
		-v
		"${WORKING_DIR}_cache/.cache:/home/imageflow/.cache"
	)
	if [[ "$COPY_HOST_CARGO_DIR" == "True" ]]; then
		SIM_DOCKER_CACHE_VARS+=(
			-v
			"${HOME}/.cargo:/home/imageflow/host_cargo"
		)
	fi
	if [[ -n "$IMAGEFLOW_DOCKER_TEST_MAP_EXTRA_DIR" ]]; then
		SIM_DOCKER_CACHE_VARS+=(
				-v
				"${WORKING_DIR}_cache/${IMAGEFLOW_DOCKER_TEST_MAP_EXTRA_DIR}:/home/imageflow/imageflow/${IMAGEFLOW_DOCKER_TEST_MAP_EXTRA_DIR}"
		)
	fi
	# The very last is unique to test.sh (for speed?)
	#Ensure that .cargo is NOT volume mapped; cargo will not work. Also, cargo fetches faster than rsync, it seems?


	if [[ "$DISABLE_COMPILATION_CACHES" == 'True' ]]; then
		echo "DISABLE_COMPILATION_CACHES=${DISABLE_COMPILATION_CACHES:-False}"
		export SIM_DOCKER_CACHE_VARS=()
	fi


	## For artifacts to be created
	export TRAVIS_PULL_REQUEST=false
	export TRAVIS_PULL_REQUEST_SHA=
	export UPLOAD_BUILD="${UPLOAD_BUILD:-True}"
	export PACKAGE_SUFFIX

	if [[ "${TARGET_CPU}" == "x86-64" || "${TARGET_CPU}" == "" ]]; then
		ARCH_SUFFIX="x86_64"
	else
		if [[ "${TARGET_CPU}" == "native" ]]; then
			ARCH_SUFFIX="HOST-NATIVE"
		fi
	fi

	if [[ "$DOCKER_IMAGE" == 'imazen/imageflow_build_ubuntu14' ]]; then
		export PACKAGE_SUFFIX="${PACKAGE_SUFFIX:-${ARCH_SUFFIX}-linux-gcc48-eglibc219}"
	fi
	if [[ "$DOCKER_IMAGE" == 'imazen/imageflow_build_ubuntu16' ]]; then
		export PACKAGE_SUFFIX="${PACKAGE_SUFFIX:-${ARCH_SUFFIX}-linux-gcc54-glibc223}"
	fi
    if [[ "$DOCKER_IMAGE" == 'imazen/imageflow_build_ubuntu18' ]]; then
		export PACKAGE_SUFFIX="${PACKAGE_SUFFIX:-${ARCH_SUFFIX}-linux-gcc73-glibc227}"
	fi


	export TRAVIS_BUILD_NUMBER=99999
	export TRAVIS_JOB_NUMBER=88888
	export TRAVIS_BRANCH=

	## For docs
	export UPLOAD_DOCS="${UPLOAD_DOCS:-True}"

	## For tagged releases
	export TRAVIS_TAG="$SIM_TRAVIS_TAG"
	## For artifact-by-commit
	export FETCH_COMMIT_SUFFIX="${FETCH_COMMIT_SUFFIX}"

	## CONFIGURATION
	# VALGRIND=True or False
	#
	## MOST LIKELY TO GET POLLUTED
	# GIT_* vars
	# BUILD_RELEASE
	# TEST_RELEASE
	# CLEAN_RELEASE
	# TEST_C
	# CLEAN_RUST_TARGETS

	#In some configurations, true
	export COVERAGE=${SIM_COVERAGE:-False}
	export COVERALLS=
	export COVERALLS_TOKEN=


	# For os x convenience
	#if [[ "$(uname -s)" == 'Darwin' ]]; then
	#	eval "$(docker-machine env default)"
	#fi

	TRAVIS_RUN_VARS=(
		"PACKAGE_SUFFIX=${PACKAGE_SUFFIX}"
		"DOCKER_IMAGE=${DOCKER_IMAGE}"
		"TARGET_CPU=${TARGET_CPU}"
		"VALGRIND=${VALGRIND}"
		"UPLOAD_BUILD=${UPLOAD_BUILD}"
		"UPLOAD_DOCS=${UPLOAD_DOCS}"
		"TRAVIS_BUILD_DIR=${TRAVIS_BUILD_DIR}"
		"CARGO_TARGET=${CARGO_TARGET}"
		"CI=${CI}"
		"SIM_CI=${SIM_CI}"
		"SIM_OPEN_BASH=${SIM_OPEN_BASH}"
		"SIM_DOCKER_CACHE_VARS="
		"${SIM_DOCKER_CACHE_VARS[@]}"
		"CHECK_DEBUG=${CHECK_DEBUG}"
		"TEST_DEBUG=${TEST_DEBUG}"
		"BUILD_DEBUG=${BUILD_DEBUG}"
		"CLEAN_RELEASE=${CLEAN_RELEASE}"
		"TEST_RELEASE=${TEST_RELEASE}"
		"BUILD_RELEASE=${BUILD_RELEASE}"
		"BUILD_QUIETER=${BUILD_QUIETER}"
		"IMAGEFLOW_BUILD_OVERRIDE=${IMAGEFLOW_BUILD_OVERRIDE}"
		"CLEAN_RUST_TARGETS=${CLEAN_RUST_TARGETS}"
		"TRAVIS_BUILD_NUMBER=${TRAVIS_BUILD_NUMBER}"
		"TRAVIS_JOB_NUMBER=${TRAVIS_JOB_NUMBER}"
		"TRAVIS_BRANCH=${TRAVIS_BRANCH}"
		"TRAVIS_TAG=${TRAVIS_TAG}"
		"FETCH_COMMIT_SUFFIX=${FETCH_COMMIT_SUFFIX}"
		"TEST_C=${TEST_C}"
		"COVERAGE=${COVERAGE}"
		"COVERALLS=${COVERALLS}"
		"COVERALLS_TOKEN=${COVERALLS_TOKEN}"
	)

	if [[ "$BUILD_QUIETER" -ne "True" ]]; then
		printf "TRAVIS_RUN_VARS: \n%s\n" "${TRAVIS_RUN_VARS[@]}"
	fi

	#echo "SIM_DOCKER_CACHE_VARS ${SIM_DOCKER_CACHE_VARS[*]}"

	echo_maybe ""
	echo_maybe "switching to ./ci/travis_run.sh ================================"

	#echo "SIM_DOCKER_CACHE_VARS ${SIM_DOCKER_CACHE_VARS[*]}"

	SIM_DOCKER_CACHE_VARS="${SIM_DOCKER_CACHE_VARS[*]}" ./ci/travis_run.sh
)
