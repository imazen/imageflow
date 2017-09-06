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
	shellcheck ./ci/cloud/*.sh
	shellcheck ./ci/cloud/*/*.sh
	shellcheck ./ci/nixtools/*.sh
	# wait until v0.44 for this; global ignores are needed shellcheck ./imageflow_tool/result_testing/*.sh
	
fi

# You're going to need:
# Conan
# clang or gcc 4.8, 4.9, or 5.4
# Rust nightly
# nasm
# Cmake
# OpenSSL (on linux)
# DSSIM
# lcov (if coverage is used)
# valgrind (if valgrind is used)

export IMAGEFLOW_BUILD_OVERRIDE="${IMAGEFLOW_BUILD_OVERRIDE:-$1}"

if [[ -n "$IMAGEFLOW_BUILD_OVERRIDE" ]]; then
	printf "Applying IMAGEFLOW_BUILD_OVERRIDE %s\n" "$IMAGEFLOW_BUILD_OVERRIDE"
	#Change the defaults when we're invoking an override
	export BUILD_QUIETER="${BUILD_QUIETER:-True}"
	export REBUILD_C="False" 
	export TEST_C="False"
	export BUILD_RELEASE="False"
	export BUILD_DEBUG="False"
	export CLEAN_RUST_TARGETS="False"
fi 

if [[ "$IMAGEFLOW_BUILD_OVERRIDE" == 'cleanup' ]]; then
	echo "Cleaning up temporary files created by running tests"
	## Remove dotfiles
	find . -type f -name '*.dot' -exec rm {} +
	find . -type f -name '*.dot.png' -exec rm {} +
	## Remove frames
	find . -type d -name node_frames -exec rm -rf {} \;

	## Remove frames
	find . -type d -name self_tests -exec rm -rf {} \;

	# Remove cargo fmt tempfiles
	find . -type f -name '*.rs.bk' -exec rm {} +

	# Remove disassembly files in c_components
	find . -type f -name '*.c.s' -exec rm {} +
	exit 0
fi 

if [[ "$IMAGEFLOW_BUILD_OVERRIDE" == 'purge' ]]; then
	echo "Purging artifacts and temporary files"
	rm -rf artifacts
	rm -rf build
	rm -rf c_components/build
	rm -rf target
	rm libimageflow.so
	rm c_components/conaninfo.txt
	rm c_components/conanbuildinfo.cmake
	rm c_components/conanfile.pyc
	rm -rf node_frames
	rm c_components/tests/visuals/compare*.png
	rm c_components/tests/visuals/*.html
	rm c_components/tests/visuals/*~
	rm c_components/cacert.pem
	rm -rf bin
	rm ./*.{png,jpg,jpeg,gif,user}
	rm ./*~
	conan remove imageflow_c/* -f

	exit 0
fi 

if [[ "$IMAGEFLOW_BUILD_OVERRIDE" == 'codestats' ]]; then
	echo "Check on unsafe code statistics"
	(	  
		(cd imageflow_core && cargo count --unsafe-statistics)
		(cd imageflow_abi && cargo count --unsafe-statistics)
		(cd imageflow_tool && cargo count --unsafe-statistics)
		(cd imageflow_riapi && cargo count --unsafe-statistics)
		(cd imageflow_helpers && cargo count --unsafe-statistics)
		(cd imageflow_types && cargo count --unsafe-statistics)
		(cd imageflow_server && cargo count --unsafe-statistics)
		(cd c_components/lib && cargo count --unsafe-statistics)
	)
	exit 0
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
	export TEST_DEBUG=True
	IMAGEFLOW_BUILD_OVERRIDE="${IMAGEFLOW_BUILD_OVERRIDE/rusttest/}"
fi 
if [[ "$IMAGEFLOW_BUILD_OVERRIDE" == *'test'* ]]; then
	export TEST_C=True
	export TEST_DEBUG=True
	IMAGEFLOW_BUILD_OVERRIDE="${IMAGEFLOW_BUILD_OVERRIDE/test/}"
fi 

if [[ "$IMAGEFLOW_BUILD_OVERRIDE" == *'kcov'* ]]; then
	export TEST_C=True
	export TEST_DEBUG=True
	export BUILD_RELEASE=False
	export BUILD_DEBUG=False
	export REBUILD_C=False
	export CLEAN_RUST_TARGETS=False
	export COVERAGE=True
	IMAGEFLOW_BUILD_OVERRIDE="${IMAGEFLOW_BUILD_OVERRIDE/kcov/}"
fi 

if [[ "$IMAGEFLOW_BUILD_OVERRIDE" == *'codecov'* ]]; then
	export BUILD_QUIETER=False
	export SILENCE_CARGO=True
	export CODECOV=True
	IMAGEFLOW_BUILD_OVERRIDE="${IMAGEFLOW_BUILD_OVERRIDE/codecov/}"
fi 


if [[ "$IMAGEFLOW_BUILD_OVERRIDE" == *'valgrind'* ]]; then
	export TEST_C=True
	export TEST_DEBUG=True
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
if [[ "$IMAGEFLOW_BUILD_OVERRIDE" == *'quiet0'* ]]; then
	export BUILD_QUIETER=False
	export SILENCE_CARGO=False
	export SILENCE_VALGRIND=False
fi 
if [[ "$IMAGEFLOW_BUILD_OVERRIDE" == *'target64linux'* ]]; then
	export CARGO_TARGET="x86_64-unknown-linux-gnu"
fi 

############# SILENCE STUFF #######################
export SILENCE_CARGO="${SILENCE_CARGO:-False}"
export SILENCE_VALGRIND="${SILENCE_VALGRIND:-False}"
export BUILD_QUIETER="${BUILD_QUIETER:-False}"
if [[ "$BUILD_QUIETER" != "True" ]]; then
	exec 9>&1
else
	exec 9>/dev/null
fi
exec 8>&9
if [[ "$SILENCE_CARGO" != "True" ]]; then
	exec 7>&1
else
	exec 7>/dev/null
fi
if [[ "$SILENCE_VALGRIND" != "True" ]]; then
	exec 6>&1
else
	exec 6>/dev/null
fi



######################################################
#### Parameters used by build.sh 

#echo "$BUILD_QUIETER $SILENCE_CARGO $SILENCE_VALGRIND"

echo_maybe(){
	if [[ "$BUILD_QUIETER" != "True" ]]; then
			echo "$1" 
	fi
}
echo_maybe "============================= [build.sh] ======================================"


export TARGET_CPU="${TARGET_CPU:-native}"
export TUNE_CPU="${TUNE_CPU:-}"
export CARGO_TARGET="${CARGO_TARGET:-}"

## all incremental 
# check debug, test debug  BUILD_DEBUG + 
# check debug, valgrind debug tests w/coverage 
# test release, build release, doc release
# build release
# build debug

# clean release build (not incremental)

# clean conan dependencies
# clean target/release
# 

# Compile and run C tests
export TEST_C="${TEST_C:-True}"
# Run debug tests (both C and Rust) under Valgrind. Forces C tests to run under debug
export VALGRIND="${VALGRIND:-False}"
# Enables generated coverage information for C/Rust. 
export COVERAGE="${COVERAGE:-False}"
export LCOV="${LCOV:-False}"
# Rebuild C part of libimageflow (release mode only)
export REBUILD_C="${REBUILD_C:-True}"
# TODO: CLEAN_C
export CODECOV_TOKEN="${CODECOV_TOKEN:-8dc28cae-eb29-4d85-b0be-d20c34ac2c30}"

export CLEAN_DEBUG="${CLEAN_DEBUG:-False}"
export BUILD_DEBUG="${BUILD_DEBUG:-False}"
export CHECK_DEBUG="${CHECK_DEBUG:-$BUILD_DEBUG}"
export TEST_DEBUG="${TEST_DEBUG:-$BUILD_DEBUG}" #(involve VALGRIND/COVERAGE)


export CLEAN_RELEASE="${CLEAN_RELEASE:-False}"
export BUILD_RELEASE="${BUILD_RELEASE:-True}"
export TEST_RELEASE="${TEST_RELEASE:-$BUILD_RELEASE}"
# Build docs; build release mode binaries (separate pass from testing); populate ./artifacts folder

# Rebuild final Rust artifacts (not deps)
export CLEAN_RUST_TARGETS="${CLEAN_RUST_TARGETS:-False}"

# Ignored
export IMAGEFLOW_SERVER=True


# Chooses values for ARTIFACT_UPLOAD_PATH and DOCS_UPLOAD_DIR if they are empty
export UPLOAD_BY_DEFAULT="${UPLOAD_BY_DEFAULT:-False}"

if [[ "$CLEAN_RUST_TARGETS" == "False" ]]; then 
	export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-1}"
else
	export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-0}"
fi 


############ GIT VALUES ##################

export GIT_COMMIT
GIT_COMMIT="${GIT_COMMIT:-$(git rev-parse HEAD)}"
GIT_COMMIT="${GIT_COMMIT:-unknown-commit}"
export GIT_COMMIT_SHORT
GIT_COMMIT_SHORT="${GIT_COMMIT_SHORT:-$(git rev-parse --short HEAD)}"
GIT_COMMIT_SHORT="${GIT_COMMIT_SHORT:-unknown-commit}"
export GIT_OPTIONAL_TAG
if git describe --exact-match --tags 2>&9 1>&9 ; then
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
if git symbolic-ref --short HEAD 2>&9 1>&9 ; then 
	GIT_OPTIONAL_BRANCH="${GIT_OPTIONAL_BRANCH:-$(git symbolic-ref --short HEAD)}"
fi 

############ NAMING OF ARTIFACTS (local-only, CI should determint the rest) ##################

if [[ "$(uname -s)" == 'Darwin' ]]; then
	export SHORT_OS_NAME="${SHORT_OS_NAME:-mac}"
else
	export SHORT_OS_NAME="${SHORT_OS_NAME:-linux}"
fi

if [[ "$(uname -s)" == 'Darwin' ]]; then
	export NUGET_RUNTIME="${NUGET_RUNTIME:-osx-x64}"
else
	export NUGET_RUNTIME="${NUGET_RUNTIME:-linux-x64}"
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
	if [[ "$BUILD_QUIETER" != "True" ]]; then
			date "$STAMP"
	fi
}
date_stamp


#Turn off coverage if lcov is missing
command -v lcov >/dev/null 2>&1 || { export LCOV=False; }

# TODO: Add CI env vars?
BUILD_VARS=(
	"CLEAN_DEBUG=${CLEAN_DEBUG}"
	"CHECK_DEBUG=${CHECK_DEBUG}"
	"TEST_DEBUG=${TEST_DEBUG}"
	"BUILD_DEBUG=${BUILD_DEBUG}"
	"CLEAN_RELEASE=${CLEAN_RELEASE}"
	"TEST_RELEASE=${TEST_RELEASE}"
	"BUILD_RELEASE=${BUILD_RELEASE}"
	"TARGET_CPU=${TARGET_CPU}"
	"TUNE_CPU=${TUNE_CPU}"
	"BUILD_QUIETER=${BUILD_QUIETER}"
	"CARGO_TARGET=${CARGO_TARGET}"
	"SILENCE_CARGO=${SILENCE_CARGO}"
	"SILENCE_VALGRIND=${SILENCE_VALGRIND}"
	"IMAGEFLOW_BUILD_OVERRIDE=${IMAGEFLOW_BUILD_OVERRIDE}"
	"VALGRIND=${VALGRIND}" 
	"TEST_C=${TEST_C}"
	"REBUILD_C=${REBUILD_C}"
	"CLEAN_RUST_TARGETS=${CLEAN_RUST_TARGETS}"
	"COVERAGE=${COVERAGE}" 
	"COVERALLS=${COVERALLS}" 
	"CODECOV=${CODECOV}" 
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



if [[ -n "$TARGET_CPU" ]]; then 
	export RUST_FLAGS="$RUST_FLAGS -C target-cpu=$TARGET_CPU"

	export FIXUP_CPU="$TARGET_CPU"
	if [[ "$CC" == *"gcc"* ]]; then 
		if [[ "$($CC --version)" == *"4.8."* ]]; then 
			if [[ "$FIXUP_CPU" == "sandybridge" ]]; then
				FIXUP_CPU="corei7-avx"
			fi
			if [[ "$FIXUP_CPU" == "haswell" ]]; then
				FIXUP_CPU="core-avx2"
			fi
		fi 
	fi 
	export CFLAGS="${CFLAGS} -march=$FIXUP_CPU -O3"
	export CXXFLAGS="${CXXFLAGS} -march=$FIXUP_CPU -O3"
fi 

if [[ -n "$TUNE_CPU" ]]; then
	export CFLAGS="${CFLAGS} -mtune=$TUNE_CPU"
	export CXXFLAGS="${CXXFLAGS} -mtune=$TUNE_CPU"
fi

export REL_RUST_FLAGS="$RUST_FLAGS"
if [[ "$COVERAGE" == 'True' ]]; then
	export TEST_RUST_FLAGS="$RUST_FLAGS -C link-dead-code"
fi

export RUST_FLAGS="$TEST_RUST_FLAGS"

if [[ -n "$CARGO_TARGET" ]]; then
	export CARGO_ARGS=("--target" "$CARGO_TARGET")
	export TARGET_DIR="target/$CARGO_TARGET/"
else 
	export CARGO_ARGS=()
	export TARGET_DIR="target/"
fi

printf "TARGET_CPU=%s  RUST_FLAGS=%s CFLAGS=%s TARGET_DIR=%s\n" "$TARGET_CPU" "$RUST_FLAGS" "$CFLAGS" "$TARGET_DIR"
	

echo_maybe "build.sh sees these relevant variables: ${BUILD_VARS[*]}"


( 
	cd c_components  
	if [[ "$CLEAN_DEBUG" == 'True' ]]; then
		rm -rf ./build
	fi 
	if [[ "$CLEAN_RELEASE" == 'True' ]]; then
		rm -rf ./build
	fi 

	[[ -d build ]] || mkdir build

	echo_maybe "================================== C/C++ =========================== [build.sh]"
    conan remote add imageflow https://api.bintray.com/conan/imazen/imageflow || true

	if [[ "$TEST_C" == 'True' ]]; then
		echo_maybe "Testing C/C++ components of Imageflow "
		echo_maybe "(and fetching and compiling dependencies)"
		echo_maybe 
		echo_maybe

		(
			cd build
			eval "$COPY_VALGRINDRC"
			conan install --scope build_tests=True -s "target_cpu=$TARGET_CPU" --scope "debug_build=${VALGRIND:-False}" --scope "coverage=${COVERAGE:-False}" --scope "skip_test_run=${VALGRIND:-False}" --build missing -u ../ 1>&8
			date_stamp
			conan build ../ 1>&8

			#Sync to build/CTestTestfile.cmake
			#Also update imageflow_core/build_c.sh
			if [[ "$VALGRIND" == 'True' ]]; then
				(
					cd ../..
					./valgrind_existing.sh ./c_components/build/bin/test_imageflow  1>&6
					./valgrind_existing.sh ./c_components/build/bin/test_variations  1>&6
					./valgrind_existing.sh ./c_components/build/bin/test_fastscaling  1>&6
					#echo "This next test is slow; it's a quickcheck running under valgrind"
					#./valgrind_existing.sh ./c_components/bin/test_theft_render
				)
				#./bin/test_theft_render
			fi 
		)
		if [[ "$LCOV" == 'True' ]]; then

			echo_maybe "==================================================================== [build.sh]"
			echo_maybe "Process coverage information with lcov"
			lcov -q --directory ./build --capture --output-file coverage.info 1>&9
			lcov -q --remove coverage.info 'tests/*' '.conan/*' '/usr/*' --output-file coverage.info 1>&9
		fi
	fi


	echo_maybe "==================================================================== [build.sh]"
	echo_maybe "Build C/C++ parts of Imageflow & dependencies as needed"
	echo_maybe 
	if [[ "$REBUILD_C" == 'True' ]]; then
		conan remove imageflow_c/* -f
	fi 
	conan export imazen/testing   1>&8
	
	(
		cd ../imageflow_core
		date_stamp

		# Conan regens every time. Let's avoid triggering rebuilds
		mkdir -p ../${TARGET_DIR}debug || true 
		BACKUP_FILE=../${TARGET_DIR}debug/conan_cargo_build.rs
		CHANGING_FILE=./conan_cargo_build.rs
		if [[ -f "$CHANGING_FILE" ]]; then 
			# Prefer in-tree copy
			cp -p "$CHANGING_FILE" "$BACKUP_FILE"
		fi 

		#Conan modifies it
		conan install --build missing -s build_type=Release  -s "target_cpu=$TARGET_CPU" 1>&8

		#We restore metadata if identical
		if [[ -f "$BACKUP_FILE" ]]; then 
			if cmp -s "$CHANGING_FILE" "$BACKUP_FILE" ; then
				 cp -p "$BACKUP_FILE" "$CHANGING_FILE"
			fi
		else
			cp -p "$CHANGING_FILE" "$BACKUP_FILE"
		fi 

		date_stamp
	)
)

echo_maybe 
echo_maybe "================================== Rust ============================ [build.sh]"


rustc --version 
cargo --version 1>&9
date_stamp

if [[ "$SILENCE_CARGO" != "True" ]]; then
	export RUST_LOG=cargo::ops::cargo_rustc::fingerprint=info
fi

if [[ "$CLEAN_RUST_TARGETS" == 'True' ]]; then
	echo "Removing output imageflow binaries (but not dependencies)"
	if [ -d "./${TARGET_DIR}debug" ]; then
		find  ./${TARGET_DIR}debug  -maxdepth 1 -type f  -delete
	fi
	if [ -d "./${TARGET_DIR}release" ]; then
		find  ./${TARGET_DIR}release  -maxdepth 1 -type f  -delete
	fi
fi
if [[ "$CLEAN_DEBUG" == 'True' ]]; then
	export CARGO_INCREMENTAL=0
	rm -rf ./target/debug
else
	export CARGO_INCREMENTAL=1
fi 

if [[ "$CHECK_DEBUG" == 'True' ]]; then
	echo_maybe Running debug cargo check 
	date_stamp
	cargo check --all "${CARGO_ARGS[@]}" 1>&7
fi 
if [[ "$TEST_DEBUG" == 'True' ]]; then
	echo_maybe Running debug cargo test
	date_stamp
	RUST_TEST_THREADS=1 cargo test --all "${CARGO_ARGS[@]}" 1>&7
	if [[ "$VALGRIND" == 'True' ]]; then
		date_stamp
		./valgrind_existing.sh   1>&6
	fi
fi
if [[ "$COVERAGE" == 'True' ]]; then
	date_stamp
	./cov.sh   1>&9
fi

export RUST_FLAGS="$REL_RUST_FLAGS"
printf "TARGET_CPU=%s  RUST_FLAGS=%s CFLAGS=%s TARGET_DIR=%s CARGO_ARGS=" "$TARGET_CPU" "$RUST_FLAGS" "$CFLAGS" "$TARGET_DIR"
printf "%s " "${CARGO_ARGS[@]}"
printf "\n"

if [[ "$BUILD_DEBUG" == 'True' ]]; then
	echo_maybe "Building debug binaries"
	date_stamp
	cargo build --all "${CARGO_ARGS[@]}" 1>&7
	./${TARGET_DIR}debug/imageflow_tool diagnose --show-compilation-info 1>&9
fi 

if [[ "$CLEAN_RELEASE" == 'True' ]]; then
	export CARGO_INCREMENTAL=0
	rm -rf ./target/release
else
	export CARGO_INCREMENTAL=1
fi 

if [[ "$TEST_RELEASE" == 'True' ]]; then
	echo_maybe "==================================================================== [build.sh]"
	echo "Running release mode tests"
	echo_maybe 
	date_stamp
	cargo test --all --release "${CARGO_ARGS[@]}" 1>&7
	date_stamp
fi 

if [[ "$BUILD_RELEASE" == 'True' ]]; then
	echo_maybe "==================================================================== [build.sh]"
	echo "Building release mode binaries"
	echo_maybe 
	date_stamp
	cargo build --all --release "${CARGO_ARGS[@]}"  1>&7
	echo_maybe "Generating docs"
	date_stamp
	cargo doc --all --release "${CARGO_ARGS[@]}" --no-deps 1>&7
	date_stamp
	./${TARGET_DIR}release/imageflow_tool diagnose --show-compilation-info 1>&9
	date_stamp
	echo_maybe "==================================================================== [build.sh]"
	echo "Populating artifacts folder"
	date_stamp 
	date_stamp 
	## Artifacts folder should exist - and be empty - at the beginning
	if [[ -d "./artifacts/upload" ]]; then 
		rm -rf ./artifacts/upload
	fi 
	if [[ -d "./artifacts/staging" ]]; then 
		rm -rf ./artifacts/staging
	fi 
	mkdir -p ./artifacts/upload || true
	mkdir -p ./artifacts/staging/headers || true

	(
		cd ./${TARGET_DIR}doc
		tar czf "../docs.tar.gz" ./*
	)
	mv ./${TARGET_DIR}docs.tar.gz ./artifacts/staging/


	cp -R ${TARGET_DIR}release/{imageflow_,libimageflow}*  ./artifacts/staging/
	cp bindings/headers/*.h  ./artifacts/staging/headers/
	rm ./artifacts/staging/*.{o,d,rlib} || true
	rm ./artifacts/staging/*-* || true
	rm -rf ./artifacts/staging/doc || true
	rm -rf ./artifacts/staging/release || true

	if [[ -n "$RUNTIME_REQUIREMENTS_FILE" ]]; then
		cp "${RUNTIME_REQUIREMENTS_FILE}" ./artifacts/staging/runtime_requirements.txt 
	fi

	(
		cd ./artifacts/staging
		tar czf "./archive.tar.gz" ./*
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
		cp -a ${TARGET_DIR}doc/* "./artifacts/upload/${DOCS_UPLOAD_DIR}/"
	fi
	if [[ -n "$DOCS_UPLOAD_DIR_2" ]]; then
		mkdir -p "./artifacts/upload/${DOCS_UPLOAD_DIR_2}" || true
		cp -a ${TARGET_DIR}doc/* "./artifacts/upload/${DOCS_UPLOAD_DIR_2}/"
	fi

	# Create the nuget artifact
	./ci/pack_nuget/pack.sh

fi
echo_maybe
date_stamp
echo_maybe "========================== Build complete :) =================== [build.sh]"


