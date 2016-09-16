#!/bin/bash
set -e #Exit on failure.

echo "============================= [build.sh] ======================================"
# You're going to need:
# Conan
# clang or gcc 4.8, 4.9, or 5.4
# Rust nightly
# nasm
# Cmake
# OpenSSL (if IMAGEFLOW_SERVER=True)
# DSSIM
# lcov (if coverage is used)
# valgrind (if valgrind is used)



######################################################
#### Parameters used by build.sh 

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
# Affects how /artifacts folder is structured by build.sh
# UPLOAD_AS_LATEST overrides the nightly build for the branch if UPLOAD_BUILD=True and run on travis with s3 uploading on
export UPLOAD_AS_LATEST=${UPLOAD_AS_LATEST:-False}
# Used by build.sh to determine the package archive name in ./artifacts
export JOB_BADGE=${JOB_BADGE:-local-build}
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
export PACKAGE_PREFIX=${PACKAGE_PREFIX:-imageflow}
if [[ "$(uname -s)" == 'Darwin' ]]; then
	export PACKAGE_SUFFIX=${PACKAGE_SUFFIX:-unknown-mac}
else
	export PACKAGE_SUFFIX=${PACKAGE_SUFFIX:-unknown-linux}
fi


export SCRIPT_DIR
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
export COPY_VALGRINDRC="cp ${SCRIPT_DIR}/.valgrindrc ./.valgrindrc; cp ${SCRIPT_DIR}/valgrind_suppressions.txt ./valgrind_suppressions.txt"

# If we're running as 'conan' (we assume this indicates we are in a docker container)
# Then we need to also change permissions so that .valgrindrc is respected
# It cannot be world-writable, and should be owned by the current user (according to valgrind)
USERNAME_WHEN_DOCKERIZED=conan
if [[ "$(id -u -n)" == "${USERNAME_WHEN_DOCKERIZED}" ]]; then
	COPY_VALGRINDRC="${COPY_VALGRINDRC}; sudo chown ${USERNAME_WHEN_DOCKERIZED}: ./.valgrindrc; sudo chown ${USERNAME_WHEN_DOCKERIZED}: ./valgrind_suppressions.txt; sudo chmod o-w ./.valgrindrc; sudo chmod o-w ./valgrind_suppressions.txt"
fi

export VALGRIND_COMMAND="valgrind -q --error-exitcode=9 --gen-suppressions=all"
export VALGRIND_RUST_COMMAND="$VALGRIND_COMMAND cargo test"
export RUST_BACKTRACE=1


export PACKAGE_DIR=${GIT_BRANCH_NAME}
export PACKAGE_ARCHIVE_NAME=${PACKAGE_PREFIX}-${JOB_BADGE}-${GIT_COMMIT}-${PACKAGE_SUFFIX}
export PACKAGE_LATEST_NAME=${PACKAGE_PREFIX}-nightly-${PACKAGE_SUFFIX}


#Turn off coverage if lcov is missing
command -v lcov >/dev/null 2>&1 || { export COVERAGE=False; }


BUILD_VARS=(
	"BUILD_RELEASE=${BUILD_RELEASE}"
	"VALGRIND=${VALGRIND}" 
	"TEST_C=${TEST_C}"
	"TEST_C_DEBUG_BUILD=${TEST_C_DEBUG_BUILD}"
	"TEST_RUST=${TEST_RUST}"
	"IMAGEFLOW_SERVER=${IMAGEFLOW_SERVER}"
	"COVERAGE=${COVERAGE}" 
	"UPLOAD_AS_LATEST=${UPLOAD_AS_LATEST}"
	"COVERALLS=${COVERALLS}" 
	"COVERALLS_TOKEN=${COVERALLS_TOKEN}"
	"JOB_BADGE=${JOB_BADGE}" 
	"GIT_COMMIT=${GIT_COMMIT}" 
	"GIT_BRANCH_NAME=${GIT_BRANCH_NAME}" 
	"PACKAGE_PREFIX=${PACKAGE_PREFIX}"  
	"PACKAGE_SUFFIX=${PACKAGE_SUFFIX}" 
	"PACKAGE_DIR=${PACKAGE_DIR}" 
	"PACKAGE_ARCHIVE_NAME=${PACKAGE_ARCHIVE_NAME}" 
	"PACKAGE_LATEST_NAME=${PACKAGE_LATEST_NAME}" 
	"VALGRIND_COMMAND=${VALGRIND_COMMAND}" 
	"VALGRIND_RUST_COMMAND=${VALGRIND_RUST_COMMAND}" 
	"COPY_VALGRINDRC=${COPY_VALGRINDRC}" 
	"RUST_BACKTRACE=${RUST_BACKTRACE}" 
)



echo "build.sh sees these relevant variables: ${BUILD_VARS[*]}"

[[ -d build ]] || mkdir build

echo "================================== C/C++ =========================== [build.sh]"

if [[ "$TEST_C" == 'True' ]]; then
	echo "Testing C/C++ components of Imageflow "
	echo "(and fetching and compiling dependencies)"
	echo 
	echo

	(
		cd build
		eval "$COPY_VALGRINDRC"
		conan install --scope build_tests=True --scope "debug_build=${TEST_C_DEBUG_BUILD:-False}" --scope "coverage=${COVERAGE:-False}" --scope "skip_test_run=${VALGRIND:-False}" --build missing -u ../
		conan build ../
		if [[ "$VALGRIND" == 'True' ]]; then
			#Sync to build/CTestTestfile.cmake
			$VALGRIND_COMMAND ./bin/test_imageflow
			$VALGRIND_COMMAND ./bin/test_variations
			$VALGRIND_COMMAND ./bin/test_fastscaling
			#echo "This next test is slow; it's a quickcheck running under valgrind"
			#$VALGRIND_COMMAND ./bin/test_theft_render
		fi 
	)
	if [[ "$COVERAGE" == 'True' ]]; then

		echo "==================================================================== [build.sh]"
		echo "Process coverage information with lcov"
		lcov -q --directory ./build --capture --output-file coverage.info
		lcov -q --remove coverage.info 'tests/*' '.conan/*' '/usr/*' --output-file coverage.info
	fi
fi

echo "==================================================================== [build.sh]"
echo "Build C/C++ parts of Imageflow & dependencies as needed"
echo 
conan export imazen/testing
(
	cd imageflow_core
	conan install --build missing
)

echo 
echo "================================== Rust ============================ [build.sh]"


if [[ "$TEST_RUST" == 'True' ]]; then

	if [[ "$VALGRIND" == 'True' ]]; then
		echo "Running all crate tests under Valgrind"
		(
			cd imageflow_core
			eval "$COPY_VALGRINDRC"
			eval "$VALGRIND_CARGO_COMMAND"
		)
		(
			cd imageflow_cdylib
			eval "$COPY_VALGRINDRC"
			eval "$VALGRIND_CARGO_COMMAND"
		)
		(
			cd imageflow_serde
			eval "$COPY_VALGRINDRC"
			eval "$VALGRIND_CARGO_COMMAND"
		)
		(
			cd imageflow_tool
			eval "$COPY_VALGRINDRC"
			eval "$VALGRIND_CARGO_COMMAND"
		)
		if [[ "$IMAGEFLOW_SERVER" == 'True' ]]; then
			(
				cd imageflow_server
				eval "$COPY_VALGRINDRC"
				eval "$VALGRIND_CARGO_COMMAND"
			)
		fi
	else 
		echo "Running all crate tests"
		(
			cd imageflow_core
			cargo test
		)
		(
			cd imageflow_cdylib
			cargo test
		)
		(
			cd imageflow_serde
			cargo test
		)
		(
			cd imageflow_tool
			cargo test
		)
		if [[ "$IMAGEFLOW_SERVER" == 'True' ]]; then
			(
				cd imageflow_server
				cargo test
			)
		fi
	fi 
fi

if [[ "$BUILD_RELEASE" == 'True' ]]; then
	echo "==================================================================== [build.sh]"
	echo "Building release mode binaries and generating docs"
	echo 

	echo "Building imageflow_core docs"

	(
		cd imageflow_core
		cargo doc --no-deps
	)
	echo "Building imageflow_serde docs"
	(
		cd imageflow_serde
		cargo doc --no-deps
	)

	echo "Building imageflow_tool (Release) and docs"

	(
		cd imageflow_tool
		cargo build --release
		cargo doc --no-deps
	)

	echo "Building libimageflow (Release) and docs"
	(
		cd imageflow_cdylib
		cargo build --release
		cargo doc --no-deps
	)

	if [[ "$IMAGEFLOW_SERVER" == 'True' ]]; then
		echo "Building imageflow_server (Release) and docs"

		(
			cd imageflow_server
			cargo build --release
			cargo doc --no-deps
		)
	fi


	echo "==================================================================== [build.sh]"
	echo "Copying stuff to artifacts folder"
	echo 
	echo 
	mkdir -p artifacts/staging/doc || true
	mkdir -p "artifacts/upload/${PACKAGE_DIR}/doc" || true


	cp target/release/{flow-,imageflow_,libimageflow}*  ./artifacts/staging/
	cp -a target/doc ./artifacts/staging/
	rm ./artifacts/staging/*.{o,d} || true

	#Remove these lines when these binaries actually do something
	rm ./artifacts/staging/flow-client || true
	rm ./artifacts/staging/imageflow_tool || true

	cd ./artifacts/staging
	tar czf "../upload/${PACKAGE_DIR}/${PACKAGE_ARCHIVE_NAME}.tar.gz" ./*
	cd ../..
	cp -a target/doc/* "./artifacts/upload/${PACKAGE_DIR}/doc/"

	if [[ "$UPLOAD_AS_LATEST" == 'True' ]]; then
		cp "./artifacts/upload/${PACKAGE_DIR}/${PACKAGE_ARCHIVE_NAME}.tar.gz" "./artifacts/upload/${PACKAGE_DIR}/${PACKAGE_LATEST_NAME}.tar.gz"
	fi

fi
echo
echo "========================== Build complete :) =================== [build.sh]"


