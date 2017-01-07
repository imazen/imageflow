#!/bin/bash
set -e #Exit on failure.

# Change directory to root (call this in a subshell if you have a problem with that)
cd "$( dirname "${BASH_SOURCE[0]}" )"

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

export IMAGEFLOW_BUILD_OVERRIDE="${IMAGEFLOW_BUILD_OVERRIDE:-$1}"

if [[ -n "$IMAGEFLOW_BUILD_OVERRIDE" ]]; then
	#Change the defaults when we're invoking an override
	export BUILD_QUIETER="${BUILD_QUIETER:-True}"
	export REBUILD_C="False" 
	export TEST_C="False"
	export BUILD_RELEASE="False"
	export BUILD_DEBUG="False"
	export CLEAN_RUST_TARGETS="False"
	export TEST_RUST="False"
fi 

if [[ "$IMAGEFLOW_BUILD_OVERRIDE" == *'clean'* ]]; then
	export CLEAN_RUST_TARGETS=True
	export REBUILD_C=True
	IMAGEFLOW_BUILD_OVERRIDE="${IMAGEFLOW_BUILD_OVERRIDE/clean/}"
fi 
if [[ "$IMAGEFLOW_BUILD_OVERRIDE" == *'debug'* ]]; then
	export BUILD_DEBUG=True
	IMAGEFLOW_BUILD_OVERRIDE="${IMAGEFLOW_BUILD_OVERRIDE/debug/}"
fi 
if [[ "$IMAGEFLOW_BUILD_OVERRIDE" == *'release'* ]]; then
	export BUILD_RELEASE=True
	IMAGEFLOW_BUILD_OVERRIDE="${IMAGEFLOW_BUILD_OVERRIDE/release/}"
fi 
if [[ "$IMAGEFLOW_BUILD_OVERRIDE" == *'rusttest'* ]]; then
	export TEST_RUST=True
	IMAGEFLOW_BUILD_OVERRIDE="${IMAGEFLOW_BUILD_OVERRIDE/rusttest/}"
fi 
if [[ "$IMAGEFLOW_BUILD_OVERRIDE" == *'test'* ]]; then
	export TEST_C=True
	export TEST_RUST=True
	IMAGEFLOW_BUILD_OVERRIDE="${IMAGEFLOW_BUILD_OVERRIDE/test/}"
fi 
if [[ "$IMAGEFLOW_BUILD_OVERRIDE" == *'valgrind'* ]]; then
	export TEST_C=True
	export TEST_RUST=True

	export VALGRIND=True
	export COVERAGE=True
fi 
if [[ "$IMAGEFLOW_BUILD_OVERRIDE" == *'quiet1'* ]]; then
	export BUILD_QUIETER=True
fi 
if [[ "$IMAGEFLOW_BUILD_OVERRIDE" == *'quiet2'* ]]; then
	export BUILD_QUIETER=True
	export SILENCE_CARGO=True
fi 
if [[ "$IMAGEFLOW_BUILD_OVERRIDE" == *'quiet3'* ]]; then
	export BUILD_QUIETER=True
	export SILENCE_CARGO=True
	export SILENCE_VALGRIND=True
fi 

######################################################
#### Parameters used by build.sh 
export SILENCE_CARGO="${SILENCE_CARGO:-False}"
export SILENCE_VALGRIND="${SILENCE_VALGRIND:-False}"
export BUILD_QUIETER="${BUILD_QUIETER:-False}"
if [[ "$BUILD_QUIETER" -ne "True" ]]; then
	export INFO_STDOUT="&1"
else
	export INFO_STDOUT=/dev/null
fi
echo_maybe(){
	if [[ "$BUILD_QUIETER" -ne "True" ]]; then
	    echo "$1"
	fi
}
echo_maybe "============================= [build.sh] ======================================"



export BUILD_DEBUG="${BUILD_DEBUG:-False}"
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
if git describe --exact-match --tags &>$INFO_STDOUT ; then
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
if git symbolic-ref --short HEAD &>$INFO_STDOUT ; then 
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
date_stamp(){
	if [[ "$BUILD_QUIETER" -ne "True" ]]; then
	    date "$STAMP"
	fi
}
date_stamp



#Turn off coverage if lcov is missing
command -v lcov >/dev/null 2>&1 || { export COVERAGE=False; }

# TODO: Add CI env vars?
BUILD_VARS=(
	"BUILD_DEBUG=${BUILD_DEBUG}"
	"BUILD_RELEASE=${BUILD_RELEASE}"
	"IMAGEFLOW_BUILD_OVERRIDE=${IMAGEFLOW_BUILD_OVERRIDE}"
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



sep_bar(){
    printf "\n=================== %s ======================\n" "$1"
}


export CONAN_STDOUT="$INFO_STDOUT"
if [[ "$SILENCE_CARGO" != "True" ]]; then
	export CARGO_STDOUT="&1"
else
	export CARGO_STDOUT=/dev/null
fi
if [[ "$SILENCE_VALGRIND" != "True" ]]; then
	export VALGRIND_STDOUT="&1"
else
	export VALGRIND_STDOUT=/dev/null
fi

echo_maybe "build.sh sees these relevant variables: ${BUILD_VARS[*]}"

( 
	cd c_components
	[[ -d build ]] || mkdir build

	echo_maybe "================================== C/C++ =========================== [build.sh]"

	if [[ "$TEST_C" == 'True' ]]; then
		echo_maybe "Testing C/C++ components of Imageflow "
		echo_maybe "(and fetching and compiling dependencies)"
		echo_maybe 
		echo_maybe

		(
			cd build
			eval "$COPY_VALGRINDRC"
			conan install --scope build_tests=True --scope "debug_build=${TEST_C_DEBUG_BUILD:-False}" --scope "coverage=${COVERAGE:-False}" --scope "skip_test_run=${VALGRIND:-False}" --build missing -u ../ 1>$CONAN_STDOUT
			date_stamp
			conan build ../ 1>$CONAN_STDOUT

			#Sync to build/CTestTestfile.cmake
			#Also update imageflow_core/build_c.sh
			if [[ "$VALGRIND" == 'True' ]]; then
				(
					cd ../..
					./valgrind_existing.sh ./c_components/build/bin/test_imageflow  1>$VALGRIND_STDOUT
					./valgrind_existing.sh ./c_components/build/bin/test_variations  1>$VALGRIND_STDOUT
					./valgrind_existing.sh ./c_components/build/bin/test_fastscaling  1>$VALGRIND_STDOUT
					#echo "This next test is slow; it's a quickcheck running under valgrind"
					#./valgrind_existing.sh ./c_components/bin/test_theft_render
				)
				./bin/test_theft_render
			fi 
		)
		if [[ "$COVERAGE" == 'True' ]]; then

			echo_maybe "==================================================================== [build.sh]"
			echo_maybe "Process coverage information with lcov"
			lcov -q --directory ./build --capture --output-file coverage.info 1>$INFO_STDOUT
			lcov -q --remove coverage.info 'tests/*' '.conan/*' '/usr/*' --output-file coverage.info 1>$INFO_STDOUT
		fi
	fi


	echo_maybe "==================================================================== [build.sh]"
	echo_maybe "Build C/C++ parts of Imageflow & dependencies as needed"
	echo_maybe 
	if [[ "$REBUILD_C" == 'True' ]]; then
	  conan remove imageflow_c/* -f
	fi 
	conan export imazen/testing 1>$CONAN_STDOUT
	
	(
		cd ../imageflow_core
		date_stamp
		#Conan regens every time. Let's avoid triggering rebuilds
		BACKUP_FILE=./old_build_rs.bak 
		CHANGING_FILE=./conan_cargo_build.rs
		cp -p "$CHANGING_FILE" "$BACKUP_FILE"
		#Conan modifies it
		conan install --build missing 1>$CONAN_STDOUT
		#We restore it if identical
		if cmp -s "$CHANGING_FILE" "$BACKUP_FILE" ; then
		   rm -f "$CHANGING_FILE"
		   mv "$BACKUP_FILE" "$CHANGING_FILE"
		else
		   rm -f "$BACKUP_FILE"
		fi

		date_stamp
	)
)

echo_maybe 
echo_maybe "================================== Rust ============================ [build.sh]"

rustc --version 
cargo --version 1>$INFO_STDOUT

if [[ "$SILENCE_CARGO" != "True" ]]; then
	export RUST_LOG=cargo::ops::cargo_rustc::fingerprint=info
fi

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
		cd imageflow_helpers
		date_stamp
		cargo test 1>$CARGO_STDOUT
	)
	(
		cd imageflow_riapi
		date_stamp
		cargo test 1>$CARGO_STDOUT
	)
	(
		cd imageflow_core
		date_stamp
		RUST_TEST_TASKS=1 cargo test 1>$CARGO_STDOUT
	)
	(
		cd imageflow_abi
		date_stamp
		cargo test 1>$CARGO_STDOUT
	)
	(
		cd imageflow_types
		date_stamp
		cargo test 1>$CARGO_STDOUT
	)
	(
		cd imageflow_tool
		date_stamp
		RUST_TEST_TASKS=1 cargo test 1>$CARGO_STDOUT
		date_stamp
	)
	if [[ "$IMAGEFLOW_SERVER" == 'True' ]]; then
		(
			cd imageflow_server
			date_stamp
			cargo test 1>$CARGO_STDOUT
		)
	fi
	if [[ "$VALGRIND" == 'True' ]]; then
		./valgrind_existing.sh   1>$VALGRIND_STDOUT
	fi
fi
if [[ "$BUILD_DEBUG" == 'True' ]]; then

	echo "Building debug binaries"
	(
		cd imageflow_abi
		date_stamp
		cargo build 1>$CARGO_STDOUT
	)
	(
		cd imageflow_tool
		date_stamp
		cargo build 1>$CARGO_STDOUT
		date_stamp
		../target/debug/imageflow_tool diagnose --show-compilation-info 1>$INFO_STDOUT
	)
	if [[ "$IMAGEFLOW_SERVER" == 'True' ]]; then
		(
			cd imageflow_server
			date_stamp
			cargo build 1>$INFO_STDOUT
		)
	fi
fi 


if [[ "$BUILD_RELEASE" == 'True' ]]; then
	echo_maybe "==================================================================== [build.sh]"
	echo "Building release mode binaries and generating docs"
	echo_maybe 
	date_stamp
	echo_maybe "Building imageflow_core docs"
	(
		cd imageflow_core
		cargo doc --no-deps 1>$CARGO_STDOUT
	)
	echo_maybe "Building imageflow_types docs"
	(
		cd imageflow_types
		cargo doc --no-deps 1>$CARGO_STDOUT
	)
	echo_maybe "Building imageflow_tool (Release) and docs"
	(
		cd imageflow_tool
		date_stamp
		cargo build --release 1>$CARGO_STDOUT
		cargo doc --no-deps 1>$CARGO_STDOUT
		date_stamp
		../target/release/imageflow_tool diagnose --show-compilation-info 1>$INFO_STDOUT
	)
	echo_maybe "Building libimageflow (Release) and docs"
	(
		cd imageflow_abi
		date_stamp
		cargo build --release 1>$CARGO_STDOUT
		cargo doc --no-deps 1>$CARGO_STDOUT
	)
	if [[ "$IMAGEFLOW_SERVER" == 'True' ]]; then
		echo_maybe "Building imageflow_server (Release) and docs"

		(
			cd imageflow_server
			date_stamp
			cargo build --release 1>$CARGO_STDOUT
			cargo doc --no-deps 1>$CARGO_STDOUT
		)
	fi

	date_stamp
	echo_maybe "==================================================================== [build.sh]"
	echo "Copying stuff to artifacts folder"
	date_stamp 
	date_stamp 
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
echo_maybe
date_stamp
echo_maybe "========================== Build complete :) =================== [build.sh]"


