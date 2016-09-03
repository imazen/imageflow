#!/bin/bash
set -e #Exit on failure.

# You're going to need:
# Conan
# Rust nightly
# nasm
# Cmake
# DSSIM
# lcov (if coverage is used)
# valgrind (if valgrind is used)

set -x

# All variables are 'True' or "False", case-sensitive!

# default(True) TEST_RUST=True - Runs Rust unit and integration tests
# default(True) TEST_C=True - Runs C unit and integration tests
# default(True) BUILD_RELEASE=True - Builds optimized Rust executables
# default(False) IMAGEFLOW_SERVER=True - Builds the Imageflow server component (openssl-sys breaks things on all platforms, in different ways)
# default(False) COVERAGE=True - if TEST_C is True, then coverage info for C code can be generated
# default(False) VALGRIND=True - if TEST_C is True, Enables the current (crappy) Valgrind testing of the C test suite.

export TEST_RUST=${TEST_RUST:-True}
export TEST_C=${TEST_C:-True}
export BUILD_RELEASE=${BUILD_RELEASE:-True}
export VALGRIND=${VALGRIND:-False}
export COVERAGE=${COVERAGE:-False}
export IMAGEFLOW_SERVER=${IMAGEFLOW_SERVER:-False}

if [[ "$(uname -s)" == 'Darwin' ]]; then
	export PACKAGE_SUFFIX=${PACKAGE_SUFFIX:-unknown-mac}
else
	export PACKAGE_SUFFIX=${PACKAGE_SUFFIX:-unknown-linux}
fi

export PACKAGE_PREFIX=imageflow 
export GIT_BRANCH_NAME=${TRAVIS_BRANCH:-$(git symbolic-ref HEAD | sed -e 's,.*/\(.*\),\1,')}
export GIT_COMMIT=$(git rev-parse --short HEAD)

if [ -z ${TRAVIS_JOB_NUMBER+x} ]; then
	export JOB_BADGE="local-build"
else
	export JOB_BADGE="travisjob-${TRAVIS_JOB_NUMBER}"
fi 

export PACKAGE_ARCHIVE_NAME=${PACKAGE_PREFIX}-${GIT_BRANCH_NAME}-${JOB_BADGE}-${GIT_COMMIT}-${PACKAGE_SUFFIX}

#Turn off coverage if lcov is missing
command -v lcov >/dev/null 2>&1 || { export COVERAGE=False; }

set +x

mkdir -p build || true

if [[ "$TEST_C" == 'True' ]]; then
	echo -e "\nBuilding C/C++ components and dependencies of Imageflow\n\n"

	cd build
	conan install --scope build_tests=True --scope coverage=${COVERAGE:-False} --scope valgrind=${VALGRIND:-False} --build missing -u ../
	conan build ../
	cd ..
	if [[ "$COVERAGE" == 'True' ]]; then
	  lcov --directory ./build --capture --output-file coverage.info # capture coverage info
	  lcov --remove coverage.info 'tests/*' '.conan/*' '/usr/*' --output-file coverage.info  # filter out system and test code
	fi
fi


echo "Build C/C++ parts of Imageflow & dependencies as needed"
conan export imazen/testing
cd imageflow_core
conan install --build missing
cd ..

if [[ "$TEST_RUST" == 'True' ]]; then
	echo "Running crate tests"
	cd imageflow_core
	cargo test
	cd ..
	cd imageflow_cdylib
	cargo test
	cd ..
	cd imageflow_serde
	cargo test
	cd ..
	cd imageflow_tool
	cargo test
	cd ..
	if [[ "$IMAGEFLOW_SERVER" == 'True' ]]; then
		cd imageflow_server
		cargo test
		cd ..
	fi
fi

if [[ "$BUILD_RELEASE" == 'True' ]]; then

	echo "Building imageflow_tool"

	cd imageflow_tool
	cargo build --release
	cargo doc --no-deps
	cd ..

	echo "Building libimageflow"

	cd imageflow_cdylib
	cargo build --release
	cargo doc --no-deps
	cd ..

	if [[ "$IMAGEFLOW_SERVER" == 'True' ]]; then
		cd imageflow_server
		cargo build --release
		cargo doc --no-deps
		cd ..
	fi


	mkdir -p artifacts/staging/doc || true
	mkdir -p artifacts/upload || true


	cp target/release/{flow-,imageflow_,libimageflow}*  ./artifacts/staging/
	cp -a target/doc ./artifacts/staging/
	rm ./artifacts/staging/*.{o,d} || true

	#Remove these lines when these binaries actually do something
	rm ./artifacts/staging/flow-client || true
	rm ./artifacts/staging/imageflow_tool || true

	cd ./artifacts/staging
	tar czf ../upload/${PACKAGE_ARCHIVE_NAME}.tar.gz *
	cd ../..
	cp -a target/doc ./artifacts/upload/${GIT_BRANCH_NAME}/


fi
	
