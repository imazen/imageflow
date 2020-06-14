#!/bin/bash
set -e
#echo "SIM_DOCKER_CACHE_VARS ${SIM_DOCKER_CACHE_VARS[*]}"
# shellcheck disable=SC2116
# shellcheck disable=SC2086
SIM_DOCKER_CACHE_VARS=($(echo $SIM_DOCKER_CACHE_VARS))

printf "travis_run.sh:  "

## REQUIRED ALWAYS
# TRAVIS_BUILD_DIR (root copy of repo, with .git folder present)
# DOCKER_IMAGE Ex. imazen/imageflow_build_ubuntu14:1 imazen/imageflow_build_ubuntu16:latest
# CI=true

## REQUIRED FOR SIMULATION
# SIM_CI=True
# SIM_OPEN_BASH=False
# SIM_DOCKER_CACHE_VARS=("-v" "mapping:to")


## For artifacts to be created
# TRAVIS_PULL_REQUEST=false
# TRAVIS_PULL_REQUEST_SHA=
# UPLOAD_BUILD=True
# PACKAGE_SUFFIX=  Ex. x86_64-linux-gcc48-eglibc219 x86_64-linux-gcc54-glibc223 x86_64-mac-osx10_11
# TRAVIS_BUILD_NUMBER=[integer]
# TRAVIS_BRANCH= (closest relevant - optional if  `git symbolic-ref --short HEAD` works

## For docs
# UPLOAD_DOCS=True or False

## For tagged releases
# TRAVIS_TAG= (optional)

## For artifact-by-commit
# FETCH_COMMIT_SUFFIX=mac64, linux64

## CONFIGURATION
# VALGRIND=True or False
#
## MOST LIKELY TO GET POLLUTED
# GIT_* vars
# BUILD_RELEASE
# TEST_C
# TEST_C_DEBUG_BUILD
# TEST_RUST
# CLEAN_RUST_TARGETS
# IMAGEFLOW_SERVER
# COVERAGE
# COVERALLS
# COVERALLS_TOKEN

if [[ "$BUILD_QUIETER" != "True" ]]; then
	exec 9>&1
else
	exec 9>/dev/null
fi

echo_maybe(){
	echo "$1" 1>&9
}

if [ -n "${TRAVIS_BUILD_DIR}" ]; then
	cd "${TRAVIS_BUILD_DIR}"
fi

STAMP="+[%H:%M:%S]"
date "$STAMP" 1>&9

#Export CI stuff
export CI_SEQUENTIAL_BUILD_NUMBER="${TRAVIS_BUILD_NUMBER}"
export CI_BUILD_URL="https://travis-ci.org/${TRAVIS_REPO_SLUG}/builds/${TRAVIS_BUILD_ID}"
export CI_JOB_URL="https://travis-ci.org/${TRAVIS_REPO_SLUG}/jobs/${TRAVIS_JOB_ID}"
export CI_JOB_TITLE="Travis ${TRAVIS_JOB_NUMBER} ${TRAVIS_OS_NAME}"
export CI_STRING="name:Travis job_id:${TRAVIS_JOB_ID} build_id:${TRAVIS_BUILD_ID} travis_commit:${TRAVIS_COMMIT} build_number:${TRAVIS_BUILD_NUMBER} job_number: ${TRAVIS_JOB_NUMBER} repo_slug:${TRAVIS_REPO_SLUG} tag:${TRAVIS_TAG} branch:${TRAVIS_BRANCH} is_pull_request:${TRAVIS_PULL_REQUEST}"
export CI_PULL_REQUEST_INFO="${TRAVIS_PULL_REQUEST_SHA}"
export CI_TAG="${TRAVIS_TAG}"
export CI_REPO="${TRAVIS_REPO_SLUG}"
export CI_RELATED_BRANCH="${TRAVIS_BRANCH}"
if [[ -z "$CI_PULL_REQUEST_INFO" ]]; then
	export GIT_OPTIONAL_BRANCH="${CI_RELATED_BRANCH}"
fi
export UPLOAD_URL="https://s3-us-west-1.amazonaws.com/imageflow-nightlies"

############ GIT VALUES ##################

echo_maybe "Querying git for version and branch information"
export GIT_COMMIT
GIT_COMMIT="${GIT_COMMIT:-$(git rev-parse HEAD)}"
GIT_COMMIT="${GIT_COMMIT:-unknown-commit}"
export GIT_COMMIT_SHORT
GIT_COMMIT_SHORT="${GIT_COMMIT_SHORT:-$(git rev-parse --short HEAD)}"
GIT_COMMIT_SHORT="${GIT_COMMIT_SHORT:-unknown-commit}"
export GIT_OPTIONAL_TAG
if git describe --exact-match --tags 1>&9 2>&9 ; then
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
if git symbolic-ref --short HEAD 1>&9 2>&9 ; then
	GIT_OPTIONAL_BRANCH="${GIT_OPTIONAL_BRANCH:-$(git symbolic-ref --short HEAD)}"
fi
echo_maybe "Naming things... (using TRAVIS_TAG=${TRAVIS_TAG}, GIT_OPTIONAL_BRANCH=${GIT_OPTIONAL_BRANCH}, PACKAGE_SUFFIX=${PACKAGE_SUFFIX}, GIT_DESCRIBE_ALWAYS_LONG=${GIT_DESCRIBE_ALWAYS_LONG}, CI_SEQUENTIAL_BUILD_NUMBER=${CI_SEQUENTIAL_BUILD_NUMBER}, GIT_COMMIT_SHORT=$GIT_COMMIT_SHORT, GIT_COMMIT=$GIT_COMMIT, FETCH_COMMIT_SUFFIX=${FETCH_COMMIT_SUFFIX})"
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
			if [[ "$ESTIMATED_DOCS_URL_2" == "$ESTIMATED_DOCS_URL" ]]; then
				export ESTIMATED_DOCS_URL_2=
			fi
		fi

		if [ -n "${FETCH_COMMIT_SUFFIX}" ]; then
			#Always upload by commit ID
			export ARTIFACT_UPLOAD_PATH_3="commits/${GIT_COMMIT}/${FETCH_COMMIT_SUFFIX}"
			export ESTIMATED_ARTIFACT_URL_3="${UPLOAD_URL}/${ARTIFACT_UPLOAD_PATH_3}.tar.gz"
		fi

		export DELETE_UPLOAD_FOLDER="False"

		export RUNTIME_REQUIREMENTS_FILE="./ci/packaging_extras/requirements/${PACKAGE_SUFFIX}.txt"
		if [ -f "$RUNTIME_REQUIREMENTS_FILE" ]; then
			echo_maybe "Using runtime requirements file ${RUNTIME_REQUIREMENTS_FILE}"
		else
			echo "Failed to locate a runtime requirements file for build variation ${PACKAGE_SUFFIX}" >&2
			exit 1
		fi
	fi
	if [ "${UPLOAD_DOCS}" != "True" ]; then
		export ESTIMATED_DOCS_URL_2=
		export DOCS_UPLOAD_DIR_2=
		export DOCS_UPLOAD_DIR=
		export ESTIMATED_DOCS_URL=
	fi


fi
if [ "${DELETE_UPLOAD_FOLDER}" == "True" ]; then
	printf "SKIPPING UPLOAD\n"
else
	printf "UPLOAD_BUILD=%s, UPLOAD_DOCS=%s\n" "${UPLOAD_BUILD}" "${UPLOAD_DOCS}"

	export URL_LIST
	URL_LIST="$(printf "\n%s\n\n%s\n\n%s\n\n%s\n\n%s\n" "${ESTIMATED_ARTIFACT_URL}" "${ESTIMATED_ARTIFACT_URL_2}" "${ESTIMATED_ARTIFACT_URL_3}" "${ESTIMATED_DOCS_URL}" "${ESTIMATED_DOCS_URL_2}" | tr -s '\n')"

fi

if [[ "$(echo "$URL_LIST" | tr -d '\r\n')" != "" ]]; then
	printf "\n=================================================\n\n" 1>&9
	printf "Estimated upload URLs:\n%s\n" "${URL_LIST}"
	printf "\n=================================================\n" 1>&9
fi



########## Travis defaults ###################
export COVERAGE="${COVERAGE:-False}"
export VALGRIND="${VALGRIND:-False}"

## Force rebuild of the final binaries (not even the shared libraries of imageflow) when TRAVIS_TAG=true
if [[ -n "$TRAVIS_TAG" ]]; then
	export CLEAN_RUST_TARGETS="${CLEAN_RUST_TARGETS:-True}"
	export CLEAN_RELEASE=True
	export CLEAN_DEBUG=True
else
	export CLEAN_RUST_TARGETS="${CLEAN_RUST_TARGETS:-False}"
fi

######################################################
#### Parameters passed through docker to build.sh (or used by travis_*.sh) ####

# Build docs; build release mode binaries (separate pass from testing); populate ./artifacts folder

# Run all tests (both C and Rust) under Valgrind
export VALGRIND="${VALGRIND:-False}"

export CHECK_DEBUG="${CHECK_DEBUG:-False}"
export CLEAN_DEBUG="${CLEAN_DEBUG:-False}"
export BUILD_DEBUG="${BUILD_DEBUG:-False}"
export TEST_DEBUG="${TEST_DEBUG:-False}"

if [[ "$VALGRIND" == "True" ]]; then
	export BUILD_RELEASE="${BUILD_RELEASE:-False}"
	export TEST_RELEASE="${TEST_RELEASE:-False}"

	export TEST_DEBUG=True
	export TEST_C=True
else
	export BUILD_RELEASE="${BUILD_RELEASE:-True}"
	export TEST_RELEASE="${TEST_RELEASE:-True}"
fi

# Compile and run C tests
export TEST_C="${TEST_C:-True}"
# Enables generated coverage information for the C portion of the code.
# Also forces C tests to build in debug mode
export COVERAGE="${COVERAGE:-False}"
# travis_run_docker.sh uploads Coverage information when true
export COVERALLS="${COVERALLS}"
export COVERALLS_TOKEN="${COVERALLS_TOKEN}"
export CODECOV="${CODECOV}"

#Overrides everything
export IMAGEFLOW_BUILD_OVERRIDE="${IMAGEFLOW_BUILD_OVERRIDE}"

export TARGET_CPU="${TARGET_CPU:-x86-64}"
export TUNE_CPU="${TUNE_CPU}"

if [ -n "${TRAVIS_BUILD_DIR}" ]; then
	cd "${TRAVIS_BUILD_DIR}"
fi


DOCKER_ENV_VARS=(
	"-e"
	 "CI=${CI}"
	 "-e"
	 "TARGET_CPU=${TARGET_CPU}"
	 "-e"
	 "IMAGEFLOW_BUILD_OVERRIDE=${IMAGEFLOW_BUILD_OVERRIDE}"
		"-e"
	 "CLEAN_DEBUG=${CLEAN_DEBUG}"
	"-e"
	 "CHECK_DEBUG=${CHECK_DEBUG}"
		"-e"
	 "TEST_DEBUG=${TEST_DEBUG}"
	"-e"
	 "BUILD_DEBUG=${BUILD_DEBUG}"
	"-e"
	 "CLEAN_RELEASE=${CLEAN_RELEASE}"
	"-e"
	 "TEST_RELEASE=${TEST_RELEASE}"
	 "-e"
	 "SKIP_HOST_CARGO_EXPORT=${SIM_CI}"
	"-e"
	 "BUILD_RELEASE=${BUILD_RELEASE}"
	"-e"
	 "VALGRIND=${VALGRIND}"
	"-e"
	 "TEST_C=${TEST_C}"
	"-e"
	 "CLEAN_RUST_TARGETS=${CLEAN_RUST_TARGETS}"
	"-e"
	 "TUNE_CPU=${TUNE_CPU}"
	"-e"
	 "COVERAGE=${COVERAGE}"
	"-e"
	"CARGO_TARGET=${CARGO_TARGET}"
	"-e"
	 "COVERALLS=${COVERALLS}"
	 "-e"
	 "CODECOV=${CODECOV}"
	"-e"
	 "COVERALLS_TOKEN=${COVERALLS_TOKEN}"
	"-e"
	 "DOCS_UPLOAD_DIR=${DOCS_UPLOAD_DIR}"
	"-e"
	"DEPLOY_DOCS=${DEPLOY_DOCS}"
	"-e"
	 "DOCS_UPLOAD_DIR_2=${DOCS_UPLOAD_DIR}"
	"-e"
	 "ARTIFACT_UPLOAD_PATH=${ARTIFACT_UPLOAD_PATH}"
	"-e"
	 "ARTIFACT_UPLOAD_PATH_2=${ARTIFACT_UPLOAD_PATH_2}"
	"-e"
	 "ARTIFACT_UPLOAD_PATH_3=${ARTIFACT_UPLOAD_PATH_3}"
	"-e"
	 "SCCACHE_BUCKET=${SCCACHE_BUCKET}"
	"-e"
	 "AWS_ACCESS_KEY_ID=${AWS_ACCESS_KEY_ID}"
	"-e"
	 "AWS_SECRET_ACCESS_KEY=${AWS_SECRET_ACCESS_KEY}"
	"-e"
	 "GIT_COMMIT=${GIT_COMMIT}"
	"-e"
	 "PACKAGE_SUFFIX=${PACKAGE_SUFFIX}"
	"-e"
	"NUGET_RUNTIME=${NUGET_RUNTIME}"
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
	 "CI_REPO=${CI_REPO}"
	"-e"
	 "CI_RELATED_BRANCH=${CI_RELATED_BRANCH}"
	 "-e"
	 "BUILD_QUIETER=${BUILD_QUIETER}"
)


echo_maybe
echo_maybe "========================================================="
echo_maybe "Relevant dockered ENV VARS for build.sh: ${DOCKER_ENV_VARS[*]}"
echo_maybe "========================================================="
echo_maybe
##############################


if [[ "$(uname -s)" == 'Darwin' && -z "$SIM_CI" ]]; then
	./build.sh
else
	echo_maybe "===================================================================== [travis_run.sh]"
	echo "Launching docker SIM_CI=${SIM_CI}"
	echo

	DOCKER_COMMAND=(
			/bin/bash -c "./ci/travis_run_docker.sh"
			)
	export DOCKER_CACHE_VARS=(
			-v
			"${HOME}/.cargo:/home/imageflow/host_cargo"
	)
	DOCKER_INVOCATION=(docker run "--rm")

	if [[ "$SIM_CI" == 'True' ]]; then
		if [[ "$SIM_OPEN_BASH" == 'True' ]]; then
			DOCKER_COMMAND=(
			/bin/bash
			)
		fi
		DOCKER_CACHE_VARS=("${SIM_DOCKER_CACHE_VARS[@]}")

		DOCKER_INVOCATION=(docker run "--interactive" "--rm")

		export DOCKER_TTY_FLAG=
		if [[ -t 1 ]]; then
			export DOCKER_TTY_FLAG="--tty"
			DOCKER_INVOCATION=(docker run "--interactive" "$DOCKER_TTY_FLAG" "--rm")
		fi


	fi
	#echo "SIM_DOCKER_CACHE_VARS ${SIM_DOCKER_CACHE_VARS[*]}"
	echo "PWD=${PWD}"
	ls "$TRAVIS_BUILD_DIR/ci/travis_run_docker.sh"
	set -x
	"${DOCKER_INVOCATION[@]}" -w "/home/imageflow/imageflow" -v "${TRAVIS_BUILD_DIR}:/home/imageflow/imageflow" "${DOCKER_CACHE_VARS[@]}" "${DOCKER_ENV_VARS[@]}" "${DOCKER_IMAGE}" "${DOCKER_COMMAND[@]}"
	set +x
fi
if [[ "$SIM_CI" != 'True' ]]; then
	if [[ -n "$CI_TAG" ]]; then
		# We always cleanup after a tagged release; no point in wasting cache space
		sudo rm -rf ./target || sudo rm -rf ./target || true
		sudo rm -rf ./c_components/build || sudo rm -rf ./c_components/build || true
		sudo rm -rf ~/.cargo || sudo rm -rf ~/.cargo || true
	fi

	# Don't let the cache become polluted by a build profile we aren't doing
	if [[ "$BUILD_RELEASE" == "False" ]]; then
		sudo rm -rf ./target/release || sudo rm -rf ./target/release || true
	fi
	if [[ "$BUILD_DEBUG" == "False" ]]; then
		sudo rm -rf ./target/debug || sudo rm -rf ./target/debug || true
	fi
fi

if [[ "$DELETE_UPLOAD_FOLDER" == 'True' ]]; then
	echo_maybe -e "\nRemoving all files scheduled for upload to s3\n\n"
	sudo rm -rf ./artifacts/upload || sudo rm -rf ./artifacts/upload || true
	mkdir -p ./artifacts/upload || true
else

	if [ -d "./artifacts/nuget" ]; then

      (cd ./artifacts/nuget
        for i in *.nupkg; do
          [ -f "$i" ] || break

          # Upload each package
          #dotnet nuget push "$i" --api-key "${NUGET_API_KEY}" -s "nuget.org"
          #dotnet nuget push "$NUGET_TEST_PACKAGE" --api-key "${NUGET_API_KEY}" -s "nuget.org"

          #curl -L "https://www.nuget.org/api/v2/package" -H "X-NuGet-ApiKey: ${NUGET_API_KEY}" -H "X-NuGet-Client-Version: 4.1.0" -A "NuGet Command Line/3.4.4.1321 (Unix 4.4.0.92)" --upload-file "$NUGET_TEST_PACKAGE"
          if [[ -n "$NUGET_API_KEY" ]]; then
            echo -e "\nUploading $i to NuGet.org\n"Rn
            curl -L "https://www.nuget.org/api/v2/package" -H "X-NuGet-ApiKey: ${NUGET_API_KEY}" -H "X-NuGet-Client-Version: 4.1.0" -A "NuGet Command Line/3.4.4.1321 (Unix 4.4.0.92)" --upload-file "$i" --fail
          else
		        echo "NUGET_API_KEY not defined ... skipping nuget upload"
		      fi
        done
		  )


	fi
fi
