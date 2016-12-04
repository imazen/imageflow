#!/bin/bash
set -e

echo "travis_run.sh:"

if [ -n "${TRAVIS_BUILD_DIR}" ]; then
  cd "${TRAVIS_BUILD_DIR}"
fi

STAMP="+[%H:%M:%S]"
date "$STAMP"

#Export CI stuff
export GIT_COMMIT="${TRAVIS_COMMIT}"
export CI_SEQUENTIAL_BUILD_NUMBER="${TRAVIS_BUILD_NUMBER}"
export CI_BUILD_URL="https://travis-ci.org/${TRAVIS_REPO_SLUG}/builds/${TRAVIS_BUILD_ID}"
export CI_JOB_URL="https://travis-ci.org/${TRAVIS_REPO_SLUG}/jobs/${TRAVIS_JOB_ID}"
export CI_JOB_TITLE="Travis ${TRAVIS_JOB_NUMBER} ${TRAVIS_OS_NAME}"
export CI_STRING="name:Travis job_id:${TRAVIS_JOB_ID} build_id:${TRAVIS_BUILD_ID} build_number:${TRAVIS_BUILD_NUMBER} job_number: ${TRAVIS_JOB_NUMBER} repo_slug:${TRAVIS_REPO_SLUG} tag:${TRAVIS_TAG} branch:${TRAVIS_BRANCH} is_pull_request:${TRAVIS_PULL_REQUEST}"
export CI_PULL_REQUEST_INFO="${TRAVIS_PULL_REQUEST_SHA}"
export CI_TAG="${TRAVIS_TAG}"
export CI_RELATED_BRANCH="${TRAVIS_BRANCH}"
if [[ -z "$CI_PULL_REQUEST_INFO" ]]; then
	export GIT_OPTIONAL_BRANCH="${CI_RELATED_BRANCH}"
fi
export UPLOAD_URL="https://s3-us-west-1.amazonaws.com/imageflow-nightlies"

############ GIT VALUES ##################

echo "Querying git for version and branch information"
export GIT_COMMIT
GIT_COMMIT="${GIT_COMMIT:-$(git rev-parse HEAD)}"
GIT_COMMIT="${GIT_COMMIT:-unknown-commit}"
export GIT_COMMIT_SHORT
GIT_COMMIT_SHORT="${GIT_COMMIT_SHORT:-$(git rev-parse --short HEAD)}"
GIT_COMMIT_SHORT="${GIT_COMMIT_SHORT:-unknown-commit}"
export GIT_OPTIONAL_TAG
if git describe --exact-match --tags; then
	GIT_OPTIONAL_TAG="${GIT_OPTIONAL_TAG:-$(git describe --exact-match --tags)}"
fi
export GIT_DESCRIBE_ALWAYS
GIT_DESCRIBE_ALWAYS="${GIT_DESCRIBE_ALWAYS:-$(git describe --always --tags)}"
export GIT_DESCRIBE_ALWAYS_LONG
GIT_DESCRIBE_ALWAYS_LONG="${GIT_DESCRIBE_ALWAYS_LONG:-$(git describe --always --tags --long)}"
export GIT_DESCRIBE_AAL
GIT_DESCRIBE_AAL="${GIT_DESCRIBE_AAL:-$(git describe --always --all --long)}"

# But let others override GIT_OPTIONAL_BRANCH, as HEAD might not have a symbolic ref, and it could crash
# I.e, provide GIT_OPTIONAL_BRANCH to this script in Travis - but NOT For 
export GIT_OPTIONAL_BRANCH
if git symbolic-ref --short HEAD; then 
	GIT_OPTIONAL_BRANCH="${GIT_OPTIONAL_BRANCH:-$(git symbolic-ref --short HEAD)}"
fi 
echo "Naming things... (using TRAVIS_TAG=${TRAVIS_TAG}, GIT_OPTIONAL_BRANCH=${GIT_OPTIONAL_BRANCH}, "
################## NAMING THINGS ####################

export DELETE_UPLOAD_FOLDER="${DELETE_UPLOAD_FOLDER:-True}"

if [ "${TRAVIS_PULL_REQUEST}" == "false" ]; then

	if [ "${UPLOAD_BUILD}" == "True" ]; then


		#Put tagged commits in their own folder instead of using the branch name
		if [ -n "${TRAVIS_TAG}" ]; then
			export UPLOAD_DIR="releases/${TRAVIS_TAG}"
			export ARTIFACT_UPLOAD_PATH="${UPLOAD_DIR}/imageflow-${TRAVIS_TAG}-${GIT_COMMIT_SHORT}-${PACKAGE_SUFFIX}"
			export DOCS_UPLOAD_DIR="${UPLOAD_DIR}/doc"
			export ESTIMATED_DOCS_URL="${UPLOAD_URL}/${DOCS_UPLOAD_DIR}"
		else
			if [ -n "${GIT_OPTIONAL_BRANCH}" ]; then
				export ARTIFACT_UPLOAD_PATH="${GIT_OPTIONAL_BRANCH}/imageflow-nightly-${CI_SEQUENTIAL_BUILD_NUMBER}-${GIT_DESCRIBE_ALWAYS_LONG}-${PACKAGE_SUFFIX}"
			fi
		fi

		export ESTIMATED_ARTIFACT_URL="${UPLOAD_URL}/${ARTIFACT_UPLOAD_PATH}.tar.gz"

		if [ -n "${GIT_OPTIONAL_BRANCH}" ]; then
			export ARTIFACT_UPLOAD_PATH_2="${GIT_OPTIONAL_BRANCH}/imageflow-nightly-${PACKAGE_SUFFIX}"
			export ESTIMATED_ARTIFACT_URL_2="${UPLOAD_URL}/${ARTIFACT_UPLOAD_PATH_2}.tar.gz"

			export DOCS_UPLOAD_DIR_2="${GIT_OPTIONAL_BRANCH}/doc"
			export ESTIMATED_DOCS_URL_2="${UPLOAD_URL}/${DOCS_UPLOAD_DIR_2}"
			export ESTIMATED_DOCS_URL="${ESTIMATED_DOCS_URL:-${ESTIMATED_DOCS_URL_2}}"
		fi

		if [ -n "${FETCH_COMMIT_SUFFIX}" ]; then
			#Always upload by commit ID
			export ARTIFACT_UPLOAD_PATH_3="commits/${GIT_COMMIT}/${FETCH_COMMIT_SUFFIX}"
			export ESTIMATED_ARTIFACT_URL_3="${UPLOAD_URL}/${ARTIFACT_UPLOAD_PATH_3}.tar.gz"
		fi

		export DELETE_UPLOAD_FOLDER="False"

		export RUNTIME_REQUIREMENTS_FILE="./ci/packaging_extras/requirements/${PACKAGE_SUFFIX}.txt"
		if [ -f "$RUNTIME_REQUIREMENTS_FILE" ]; then
			echo "Using runtime requirements file ${RUNTIME_REQUIREMENTS_FILE}"
		else
			echo "Failed to locate a runtime requirements file for build variation ${PACKAGE_SUFFIX}"
			exit 1
		fi
	fi
	if [ "${UPLOAD_DOCS}" != "True" ]; then
		export ESTIMATED_DOCS_URL_2=
		export ESTIMATED_DOCS_URL=
	fi


fi
if [ "${DELETE_UPLOAD_FOLDER}" == "True" ]; then
	printf "\nSKIPPING UPLOAD\n"
else
	printf "\nUPLOAD_BUILD=%s, UPLOAD_DOCS=%s" "${UPLOAD_BUILD}" "${UPLOAD_DOCS}"
fi


printf "\n=================================================\n"
printf "\nEstimated upload URLs:\n\n%s\n\n%s\n\n" "${ESTIMATED_ARTIFACT_URL}" "${ESTIMATED_ARTIFACT_URL_2}" "${ESTIMATED_ARTIFACT_URL_3}"
printf "\nEstimated docs URLs:\n\n%s\n\n%s\n\n" "${ESTIMATED_DOCS_URL}" "${DOCS_UPLOAD_DIR_2}"
printf "\n=================================================\n"





########## Travis defaults ###################
export IMAGEFLOW_SERVER="${IMAGEFLOW_SERVER:-True}"
export COVERAGE="${COVERAGE:-False}"
export VALGRIND="${VALGRIND:-False}"

######################################################
#### Parameters passed through docker to build.sh (or used by travis_*.sh) ####

# Not actually used as of 2016-09-16
# Likely to be used by travis_run_docker.sh if we can ever support 'stable'
export RUST_CHANNEL="${RUST_CHANNEL:-nightly}"
# Build docs; build release mode binaries (separate pass from testing); populate ./artifacts folder
export BUILD_RELEASE="${BUILD_RELEASE:-True}"
# Run all tests (both C and Rust) under Valgrind
export VALGRIND="${VALGRIND:-False}"
# Compile and run C tests
export TEST_C="${TEST_C:-True}"
# Build C Tests in debug mode for clearer valgrind output
export TEST_C_DEBUG_BUILD="${TEST_C_DEBUG_BUILD:${VALGRIND}}"
# Run Rust tests
export TEST_RUST="${TEST_RUST:-True}"
# Enable compilation of imageflow_server, which has a problematic openssl dependency
export IMAGEFLOW_SERVER="${IMAGEFLOW_SERVER:-True}"
# Enables generated coverage information for the C portion of the code. 
# Also forces C tests to build in debug mode
export COVERAGE="${COVERAGE:-False}"
# Affects how /artifacts folder is structured by build.sh
export UPLOAD_AS_LATEST="${UPLOAD_AS_LATEST:-False}"
# travis_run_docker.sh uploads Coverage information when true
export COVERALLS="${COVERALLS}"
export COVERALLS_TOKEN="${COVERALLS_TOKEN}"

if [ -n "${TRAVIS_BUILD_DIR}" ]; then
  cd "${TRAVIS_BUILD_DIR}"
fi


DOCKER_ENV_VARS=(
  "-e"
	 "CI=${CI}"
	"-e"
	 "RUST_CHANNEL=${RUST_CHANNEL}" 
	"-e"
	 "BUILD_RELEASE=${BUILD_RELEASE}"
	"-e"
	 "VALGRIND=${VALGRIND}" 
	"-e"
	 "TEST_C=${TEST_C}"
	"-e"
	 "TEST_C_DEBUG_BUILD=${TEST_C_DEBUG_BUILD}"
	"-e"
	 "TEST_RUST=${TEST_RUST}"
	"-e"
	 "IMAGEFLOW_SERVER=${IMAGEFLOW_SERVER}"
	"-e"
	 "COVERAGE=${COVERAGE}" 
	"-e"
	 "COVERALLS=${COVERALLS}" 
	"-e"
	 "COVERALLS_TOKEN=${COVERALLS_TOKEN}"
	"-e"
	 "DOCS_UPLOAD_DIR=${DOCS_UPLOAD_DIR}" 
	"-e"
	 "DOCS_UPLOAD_DIR_2=${DOCS_UPLOAD_DIR}" 
	"-e"
	 "ARTIFACT_UPLOAD_PATH=${ARTIFACT_UPLOAD_PATH}"  
	"-e"
	 "ARTIFACT_UPLOAD_PATH_2=${ARTIFACT_UPLOAD_PATH_2}" 
	"-e"
	 "ARTIFACT_UPLOAD_PATH_3=${ARTIFACT_UPLOAD_PATH_3}" 
    "-e"
	 "GIT_COMMIT=${GIT_COMMIT}" 
	"-e"
	 "GIT_COMMIT_SHORT=${GIT_COMMIT_SHORT}" 
	"-e"
	 "GIT_OPTIONAL_TAG=${GIT_OPTIONAL_TAG}" 
	"-e"
	 "GIT_DESCRIBE_ALWAYS=${GIT_DESCRIBE_ALWAYS}" 
	"-e"
	 "GIT_DESCRIBE_ALWAYS_LONG=${GIT_DESCRIBE_ALWAYS_LONG}" 
	 "-e"
	 "RUNTIME_REQUIREMENTS_FILE=${RUNTIME_REQUIREMENTS_FILE}"
	"-e"
	 "GIT_DESCRIBE_AAL=${GIT_DESCRIBE_AAL}" 
	"-e"
	 "GIT_OPTIONAL_BRANCH=${GIT_OPTIONAL_BRANCH}" 
	"-e"
	 "ESTIMATED_ARTIFACT_URL=${ESTIMATED_ARTIFACT_URL}" 
	"-e"
	 "ESTIMATED_DOCS_URL=${ESTIMATED_DOCS_URL}" 
	"-e"
	 "CI_SEQUENTIAL_BUILD_NUMBER=${CI_SEQUENTIAL_BUILD_NUMBER}" 
	"-e"
	 "CI_BUILD_URL=${CI_BUILD_URL}" 
	"-e"
	 "CI_JOB_URL=${CI_JOB_URL}" 
	"-e"
	 "CI_JOB_TITLE=${CI_JOB_TITLE}" 
	"-e"
	 "CI_STRING=${CI_STRING}" 
	"-e"
	 "CI_PULL_REQUEST_INFO=${CI_PULL_REQUEST_INFO}" 
	"-e"
	 "CI_TAG=${CI_TAG}" 
	"-e"
	 "CI_RELATED_BRANCH=${CI_RELATED_BRANCH}" 
)


echo 
echo "========================================================="
echo "Relevant ENV VARS for build.sh: ${DOCKER_ENV_VARS[*]}"
echo "========================================================="
echo 
##############################


if [[ "$(uname -s)" == 'Darwin' ]]; then
	./ci/travis_run_osx.sh
else
	set -x
	docker run --rm -v "$HOME/.ccache:/home/conan/.ccache" -v "$HOME/.conan/data:/home/conan/.conan/data" -v "${TRAVIS_BUILD_DIR}:/home/conan/imageflow" "${DOCKER_ENV_VARS[@]}" "${DOCKER_IMAGE}" /bin/bash -c "./ci/travis_run_docker.sh"
	set +x
fi

if [[ "$DELETE_UPLOAD_FOLDER" != 'True' ]]; then
	echo -e "\nRemvoing all files scheduled for upload to s3\n\n"
	rm -rf ./artifacts/upload
	mkdir -p ./artifacts/upload
else
	echo -e "\nListing files scheduled for upload to s3\n\n"
	ls -R ./artifacts/upload/*
fi

