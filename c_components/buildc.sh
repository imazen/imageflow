#!/bin/bash

set -e
set -x

export VALGRIND="${VALGRIND:-False}"
export TEST_C_DEBUG_BUILD="${TEST_C_DEBUG_BUILD:-${VALGRIND}}"

[[ -d build ]] || mkdir build

echo "Testing C/C++ components of Imageflow "
echo "(and fetching and compiling dependencies)"
echo 
echo

(
	cd build
	conan install --scope build_tests=True --scope "debug_build=${TEST_C_DEBUG_BUILD:-False}" --scope "skip_test_run=${VALGRIND:-False}" --build missing -u ../
	conan build ../

	#Sync to build/CTestTestfile.cmake
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

echo Reexport package
conan remove imageflow_c/* -f
conan export imazen/testing

