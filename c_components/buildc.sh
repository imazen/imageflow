#!/bin/bash

set -e
set -x

export VALGRIND="${VALGRIND:-False}"
export TEST_C_DEBUG_BUILD="${TEST_C_DEBUG_BUILD:-False}"

[[ -d build ]] || mkdir build

echo "Testing C/C++ components of Imageflow "
echo "(and fetching and compiling dependencies)"
echo 
echo

(
	cd build
	eval "$COPY_VALGRINDRC"
	conan install --scope build_tests=True --scope "debug_build=${TEST_C_DEBUG_BUILD:-False}" --scope "skip_test_run=${VALGRIND:-False}" --build missing -u ../
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
	


echo Reexport package
conan remove imageflow_c/* -f
conan export imazen/testing

