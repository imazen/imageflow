#!/bin/bash
set -e #Exit on failure.

has_shellcheck() {
	command -v shellcheck >/dev/null 2>&1 
}
if has_shellcheck; then
	shellcheck ./*.sh
	shellcheck ./ci/*.sh
	shellcheck ./imageflow_*/*.sh
	shellcheck ./c_components/*.sh
	shellcheck ./ci/docker/*.sh
	shellcheck ./ci/docker/build_*/*.sh
	shellcheck ./ci/nixtools/*.sh
	# wait until v0.44 for this; global ignores are needed shellcheck ./imageflow_tool/result_testing/*.sh
	
fi

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
export BUILD_RELEASE="${BUILD_RELEASE:-True}"
# Run all tests (both C and Rust) under Valgrind
export VALGRIND="${VALGRIND:-False}"
# Compile and run C tests
export TEST_C="${TEST_C:-True}"
# Rebuild C part of libimageflow
export REBUILD_C="${REBUILD_C:-True}"
# Build C Tests in debug mode for clearer valgrind output
export TEST_C_DEBUG_BUILD="${TEST_C_DEBUG_BUILD:${VALGRIND}}"
# Rebuild final Rust artifacts (not deps)
export CLEAN_RUST_TARGETS="${CLEAN_RUST_TARGETS:-False}"
# Run Rust tests
export TEST_RUST="${TEST_RUST:-True}"
# Enable compilation of imageflow_server, which has a problematic openssl dependency
export IMAGEFLOW_SERVER="${IMAGEFLOW_SERVER:-True}"
# Enables generated coverage information for the C portion of the code. 
# Also forces C tests to build in debug mode
export COVERAGE="${COVERAGE:-False}"

# Chooses values for ARTIFACT_UPLOAD_PATH and DOCS_UPLOAD_DIR if they are empty
export UPLOAD_BY_DEFAULT="${UPLOAD_BY_DEFAULT:-False}"


############ GIT VALUES ##################

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

############ NAMING OF ARTIFACTS (local-only, CI should determint the rest) ##################

if [[ "$(uname -s)" == 'Darwin' ]]; then
	export SHORT_OS_NAME="${SHORT_OS_NAME:-mac}"
else
	export SHORT_OS_NAME="${SHORT_OS_NAME:-linux}"
fi

if [[ "${UPLOAD_BY_DEFAULT}" == "True" ]]; then
	if [[ -n "${GIT_OPTIONAL_BRANCH}" ]]; then
		export ARTIFACT_UPLOAD_PATH="${ARTIFACT_UPLOAD_PATH:-${GIT_OPTIONAL_BRANCH}/imageflow-localbuild-${GIT_COMMIT_SHORT}-${SHORT_OS_NAME}}"
		export DOCS_UPLOAD_DIR="${DOCS_UPLOAD_DIR:-${GIT_OPTIONAL_BRANCH}/doc}"
		export ARTIFACT_UPLOAD_PATH_2="${ARTIFACT_UPLOAD_PATH_2}"
		export DOCS_UPLOAD_DIR_2="${DOCS_UPLOAD_DIR_2}"
	fi
fi

##################################

export RUST_BACKTRACE=1
STAMP="+[%H:%M:%S]"
date "$STAMP"


#Turn off coverage if lcov is missing
command -v lcov >/dev/null 2>&1 || { export COVERAGE=False; }

# TODO: Add CI env vars?
BUILD_VARS=(
	"BUILD_RELEASE=${BUILD_RELEASE}"
	"VALGRIND=${VALGRIND}" 
	"TEST_C=${TEST_C}"
	"REBUILD_C=${REBUILD_C}"
	"CLEAN_RUST_TARGETS=${CLEAN_RUST_TARGETS}"
	"TEST_C_DEBUG_BUILD=${TEST_C_DEBUG_BUILD}"
	"TEST_RUST=${TEST_RUST}"
	"IMAGEFLOW_SERVER=${IMAGEFLOW_SERVER}"
	"COVERAGE=${COVERAGE}" 
	"COVERALLS=${COVERALLS}" 
	"COVERALLS_TOKEN=${COVERALLS_TOKEN}"
	"GIT_COMMIT=${GIT_COMMIT}" 
	"ARTIFACT_UPLOAD_PATH=${ARTIFACT_UPLOAD_PATH}"  
	"ARTIFACT_UPLOAD_PATH_2=${ARTIFACT_UPLOAD_PATH_2}" 
	"ARTIFACT_UPLOAD_PATH_3=${ARTIFACT_UPLOAD_PATH_3}" 
	"DOCS_UPLOAD_DIR=${DOCS_UPLOAD_DIR}" 
	"DOCS_UPLOAD_DIR_2=${DOCS_UPLOAD_DIR_2}" 
	"RUNTIME_REQUIREMENTS_FILE=${RUNTIME_REQUIREMENTS_FILE}" 
	"RUST_BACKTRACE=${RUST_BACKTRACE}" 
)



echo "build.sh sees these relevant variables: ${BUILD_VARS[*]}"

( 
	cd c_components
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
			date "$STAMP"
			conan install --scope build_tests=True --scope "debug_build=${TEST_C_DEBUG_BUILD:-False}" --scope "coverage=${COVERAGE:-False}" --scope "skip_test_run=${VALGRIND:-False}" --build missing -u ../
			date "$STAMP"
			conan build ../

			#Sync to build/CTestTestfile.cmake
			#Also update imageflow_core/build_c.sh
			if [[ "$VALGRIND" == 'True' ]]; then
				(
					cd ../..
					./valgrind_existing.sh ./c_components/build/bin/test_imageflow
					./valgrind_existing.sh ./c_components/build/bin/test_variations
					./valgrind_existing.sh ./c_components/build/bin/test_fastscaling
					#echo "This next test is slow; it's a quickcheck running under valgrind"
					#./valgrind_existing.sh ./c_components/bin/test_theft_render
				)
				./bin/test_theft_render
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
	if [[ "$REBUILD_C" == 'True' ]]; then
	  conan remove imageflow_c/* -f
	fi
	conan export imazen/testing
	(
		cd ../imageflow_core
		date "$STAMP"
		conan install --build missing
		date "$STAMP"
	)
)

echo 
echo "================================== Rust ============================ [build.sh]"

rustc --version
cargo --version
if [[ "$CLEAN_RUST_TARGETS" == 'True' ]]; then
	echo "Removing output imageflow binaries (but not dependencies)"
	if [ -d "./target/debug" ]; then
		find  ./target/debug  -maxdepth 1 -type f  -delete
	fi
	if [ -d "./target/release" ]; then
		find  ./target/release  -maxdepth 1 -type f  -delete
	fi
fi

if [[ "$TEST_RUST" == 'True' ]]; then


	echo "Running all crate tests"
	(
		cd imageflow_core
		date "$STAMP"
		RUST_TEST_TASKS=1 cargo test
	)
	(
		cd imageflow_abi
		date "$STAMP"
		cargo test
	)
	(
		cd imageflow_types
		date "$STAMP"
		cargo test
	)
	(
		cd imageflow_tool
		date "$STAMP"
		RUST_TEST_TASKS=1 cargo test
		date "$STAMP"
	)
	if [[ "$IMAGEFLOW_SERVER" == 'True' ]]; then
		(
			cd imageflow_server
			date "$STAMP"
			cargo test
		)
	fi
	if [[ "$VALGRIND" == 'True' ]]; then
		./valgrind_existing.sh
	fi
fi

if [[ "$BUILD_RELEASE" == 'True' ]]; then
	echo "==================================================================== [build.sh]"
	echo "Building release mode binaries and generating docs"
	echo 
	export RUSTFLAGS=-g
	echo "Building with debugging symbols (adds 20+mb)"
	date "$STAMP"
	echo "Building imageflow_core docs"
	(
		cd imageflow_core
		cargo doc --no-deps
	)
	echo "Building imageflow_types docs"
	(
		cd imageflow_types
		cargo doc --no-deps
	)
	echo "Building imageflow_tool (Release) and docs"
	(
		cd imageflow_tool
		date "$STAMP"
		cargo build --release
		cargo doc --no-deps
		date "$STAMP"
		../target/release/imageflow_tool diagnose --show-compilation-info
	)
	echo "Building libimageflow (Release) and docs"
	(
		cd imageflow_abi
		date "$STAMP"
		cargo build --release
		cargo doc --no-deps
	)
	if [[ "$IMAGEFLOW_SERVER" == 'True' ]]; then
		echo "Building imageflow_server (Release) and docs"

		(
			cd imageflow_server
			date "$STAMP"
			cargo build --release
			cargo doc --no-deps
		)
	fi

	date "$STAMP"
	echo "==================================================================== [build.sh]"
	echo "Copying stuff to artifacts folder"
	echo 
	echo 
	mkdir -p artifacts/staging/doc || true
	mkdir -p artifacts/staging/headers || true

	cp -R target/release/{flow-proto1,imageflow_,libimageflow}*  ./artifacts/staging/
	rm ./artifacts/staging/*.rlib || true
	cp bindings/headers/*.h  ./artifacts/staging/headers/
	cp -a target/doc ./artifacts/staging/
	rm ./artifacts/staging/*.{o,d} || true

	if [[ -n "$RUNTIME_REQUIREMENTS_FILE" ]]; then
		cp "${RUNTIME_REQUIREMENTS_FILE}" ./artifacts/staging/runtime_requirements.txt 
	fi

	(
		cd ./artifacts/staging
		tar czf "archive.tar.gz" ./*
	)
	ARTIFACT_ARCHIVE_NAME="./artifacts/staging/archive.tar.gz"

	if [[ -n "$ARTIFACT_UPLOAD_PATH" ]]; then
		mkdir -p "./artifacts/upload/$(dirname "${ARTIFACT_UPLOAD_PATH}")" || true
		cp "${ARTIFACT_ARCHIVE_NAME}" "./artifacts/upload/${ARTIFACT_UPLOAD_PATH}.tar.gz"
	fi
	if [[ -n "$ARTIFACT_UPLOAD_PATH_2" ]]; then
		mkdir -p "./artifacts/upload/$(dirname "${ARTIFACT_UPLOAD_PATH_2}")" || true
		cp "${ARTIFACT_ARCHIVE_NAME}" "./artifacts/upload/${ARTIFACT_UPLOAD_PATH_2}.tar.gz"
	fi
	if [[ -n "$ARTIFACT_UPLOAD_PATH_3" ]]; then
		mkdir -p "./artifacts/upload/$(dirname "${ARTIFACT_UPLOAD_PATH_3}")" || true
		cp "${ARTIFACT_ARCHIVE_NAME}" "./artifacts/upload/${ARTIFACT_UPLOAD_PATH_3}.tar.gz"
	fi

	if [[ -n "$DOCS_UPLOAD_DIR" ]]; then
		mkdir -p "./artifacts/upload/${DOCS_UPLOAD_DIR}" || true
		cp -a target/doc/* "./artifacts/upload/${DOCS_UPLOAD_DIR}/"
	fi
	if [[ -n "$DOCS_UPLOAD_DIR_2" ]]; then
		mkdir -p "./artifacts/upload/${DOCS_UPLOAD_DIR_2}" || true
		cp -a target/doc/* "./artifacts/upload/${DOCS_UPLOAD_DIR_2}/"
	fi



fi
echo
date "$STAMP"
echo "========================== Build complete :) =================== [build.sh]"


