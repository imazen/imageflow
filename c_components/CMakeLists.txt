project(ImageFlow_C)
cmake_minimum_required(VERSION 2.8.12)

if(EXISTS ${CMAKE_CURRENT_SOURCE_DIR}/build/conanbuildinfo.cmake) #Clion, with conanbuildinfo.cmake in root
    include(${CMAKE_CURRENT_SOURCE_DIR}/build/conanbuildinfo.cmake)
elseif(EXISTS ${CMAKE_BINARY_DIR}/../conanbuildinfo.cmake) # Imageflow as a conan package
    include(${CMAKE_BINARY_DIR}/../conanbuildinfo.cmake)
else()
    include(${CMAKE_BINARY_DIR}/conanbuildinfo.cmake) # Not CLion
endif()

conan_basic_setup()

option(ENABLE_TEST OFF)
option(ENABLE_PROFILING OFF)
option(SKIP_LIBRARY OFF)
#option(TARGET_CPU x86_^$)

#Required to be able to run tests from CLion
if (NOT SKIP_LIBRARY)
	if(EXISTS ${CMAKE_CURRENT_SOURCE_DIR}/build/conanbuildinfo.cmake)
		set(ENABLE_TEST ON)
	endif()
endif()

list(REMOVE_ITEM CONAN_LIBS efence) # FIXME: Maybe efence should be a private dep of openssls


file(GLOB_RECURSE LIB_SRCS lib/*.c)
file(GLOB LIB_HDRS lib/*.h)
list(APPEND LIB_HDRS imageflow.h)
list(APPEND LIB_HDRS imageflow_advanced.h)

list(APPEND LIB_SRCS tests/helpers.c)
list(APPEND LIB_HDRS tests/helpers.h)


SET_SOURCE_FILES_PROPERTIES(LIB_SRCS PROPERTIES LANGUAGE CXX )
SET_SOURCE_FILES_PROPERTIES(LIB_HDRS PROPERTIES LANGUAGE CXX )

if (NOT SKIP_LIBRARY)
	add_library(imageflow_c ${LIB_SRCS} ${LIB_HDRS})
	set_target_properties(imageflow_c PROPERTIES LIBRARY_OUTPUT_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/lib)
	set_target_properties(imageflow_c PROPERTIES ARCHIVE_OUTPUT_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/lib)

	target_link_libraries(imageflow_c ${CONAN_LIBS})
	target_include_directories(imageflow_c PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
	# target_compile_options(imageflow_c PRIVATE "-flto")
	target_compile_options(imageflow_c PRIVATE "-fverbose-asm")
endif()

if (BUILD_SHARED_LIBS)
add_definitions(-Dimageflow_c_BUILD_SHARED)
else()
add_definitions(-Dimageflow_c_BUILD_STATIC)
endif()

# -ffast-math changes results
# -no-parenthesis is unfortunate, but gcc 4.8.5 broke all unit test macros.


# target generic, sandybridge, and haswell

# gcc 54: mtune=haswell march=sandybridge
# gcc 48: mtune=native march=corei7-avx
#MMX, SSE, SSE2, SSE3, SSSE3, SSE4.1, SSE4.2, POPCNT, AVX, AES and PCLMUL instruction set support.

#generic64 MMX, SSE, SSE2,

#nehalem -mmmx -msse -msse2 -msse3 -mssse3 -msse4.1 -msse4.2, -mpopcnt

#sandybridge -mmmx -msse -msse2 -msse3 -mssse3 -msse4.1 -msse4.2, -mpopcnt -mavx -maes -mpclmul
#haswell adds -mavx2 -mfsgsbase -mrdrnd -mfma -mbmi -mbmi2 -mf16c

#skylake adds RDSEED, ADCX, PREFETCHW, CLFLUSHOPT, XSAVEC and XSAVES

#FSGSBASE, RDRND, FMA, BMI, BMI2 and F16C

# -mno-avx2 -no-avx512 -mno-fsgsbase -mno-rdrnd -mno-f16c -mno-fma -mno-fma4 -mno-xop -mno-lwp -mno-bmi -mno-bmi2 -mno-lzcnt

#set(IMAGEFLOW_NEHALEM "-mmmx -msse -msse2 -msse3 -mssse3 -msse4.1 -msse4.2, -mpopcnt")
#set(IMAGEFLOW_SANDYBRIDGE "-mmmx -msse -msse2 -msse3 -mssse3 -msse4.1 -msse4.2, -mpopcnt -mavx -maes -mpclmul")
#set(IMAGEFLOW_HASWELL "-mmmx -msse -msse2 -msse3 -mssse3 -msse4.1 -msse4.2, -mpopcnt -mavx -maes -mpclmul -mavx2 -mfsgsbase -mrdrnd -mfma -mbmi -mbmi2 -mf16c")


## let's assume
## generic
## avx (sandybridge)
## avx2 (haswell)
## native - match system
# http://stackoverflow.com/a/25095818

# x86_64
# corei7-avx or sandybridge
# core-avx2 or haswell
# native
# mtune inherits from march


set(ARCH_SANDYBRIDGE "sandybridge")
set(ARCH_HASWELL "haswell")

if (CMAKE_COMPILER_IS_GNUCC)
	execute_process(COMMAND ${CMAKE_C_COMPILER} -dumpversion
					OUTPUT_VARIABLE GCC_VERSION)
	if (GCC_VERSION VERSION_LESS 4.9)
		message(STATUS "GCC < 4.9")
		set(ARCH_SANDYBRIDGE "corei7-avx")
		set(ARCH_HASWELL "core-avx2")
	endif()
endif()

set(MSVC_OPTIMIZE "")

if ("${TARGET_CPU}" STREQUAL "sandybridge")
	SET(TARGET_CPU "${ARCH_SANDYBRIDGE}")
	set(MSVC_OPTIMIZE " /arch:AVX")
endif()

if ("${TARGET_CPU}" STREQUAL "haswell")
	SET(TARGET_CPU "${ARCH_HASWELL}")
	set(MSVC_OPTIMIZE " /arch:AVX2")
endif()

if (MSVC)
	#/arch:AVX which turns on Intel Advanced Vector Extensions
	#/arch:AVX2 which turns on Intel Advanced Vector Extensions 2
	set(IMAGEFLOW_C_OPTIMIZE "${MSVC_OPTIMIZE} /fp:fast")
else()

	SET(NON_MSVC_OPTIMIZE "-march=${TARGET_CPU}")
	if ("${TARGET_CPU}" STREQUAL "OFF")
		SET(NON_MSVC_OPTIMIZE "")
	endif()
	if ("${TARGET_CPU}" STREQUAL "")
		SET(NON_MSVC_OPTIMIZE "")
	endif()
	set(IMAGEFLOW_C_OPTIMIZE "-O3 ${NON_MSVC_OPTIMIZE} -funroll-loops -ffast-math -mfpmath=sse  -msse2")
endif()


if (NOT MSVC)
	#mpfmath=sse is required for accuracy
	set(SHARED_FLAGS "-iquotelib -fPIC -mfpmath=sse")
	set(SHARED_WARNINGS, "-Wpointer-arith -Wcast-qual -Wpedantic -Wall -Wextra -Wno-unused-parameter -Wuninitialized -Wredundant-decls -Wno-error=unused-function -Wno-parentheses -Werror")
	set(CMAKE_C_FLAGS  "${CMAKE_C_FLAGS} ${IMAGEFLOW_C_OPTIMIZE} ${SHARED_FLAGS} ${SHARED_WARNINGS} -std=gnu11 -Wstrict-prototypes -Wmissing-prototypes -Wc++-compat -Wshadow" )
	set(CMAKE_CXX_FLAGS  "${CMAKE_CXX_FLAGS} ${IMAGEFLOW_C_OPTIMIZE} ${SHARED_FLAGS} ${SHARED_WARNINGS} -std=gnu++11" )
else()
	set(CMAKE_C_FLAGS  "${CMAKE_C_FLAGS} ${IMAGEFLOW_C_OPTIMIZE}" )
	set(CMAKE_CXX_FLAGS  "${CMAKE_CXX_FLAGS} ${IMAGEFLOW_C_OPTIMIZE}" )
endif()

option(COVERAGE "Generate coverage data" OFF)

option(DEBUG_BUILD "Generate debug build data" OFF)

if (COVERAGE)
	set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -g -O0") # debug, no optimisation
	set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} --coverage") # --fprofile-arcs --ftest-coverage") # enabling coverage
	set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -g -O0") # debug, no optimisation
	set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} --coverage ") # enabling coverage
else()
	if (DEBUG_BUILD)
		set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -g -O0") # debug, no optimisation
		set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -g -O0") # debug, no optimisation
	else()

	endif()
endif()

if (ENABLE_PROFILING)
	set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -pg ")
	set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -pg ")

	add_executable(profile_imageflow tests/profile_imageflow.c tests/helpers.c ${LIB_SRCS} ${LIB_HDRS})
	target_link_libraries(profile_imageflow ${CONAN_LIBS})
	target_include_directories(profile_imageflow PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
	target_include_directories(profile_imageflow PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/lib)
	target_compile_options(profile_imageflow PRIVATE "-flto")
endif()


if(ENABLE_TEST)
    include(CTest)
    enable_testing()

	add_library(imageflow_test_helpers_OBJECTS OBJECT tests/helpers.c tests/runner.cpp)
	target_include_directories(imageflow_test_helpers_OBJECTS PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
	target_include_directories(imageflow_test_helpers_OBJECTS PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/lib)

	function(create_test target)
		add_executable(${target} $<TARGET_OBJECTS:imageflow_test_helpers_OBJECTS> ${ARGN})
		add_test(NAME ${target} COMMAND $<TARGET_FILE:${target}>)
	    target_link_libraries(${target} imageflow_c ${CONAN_LIBS})
	    target_include_directories(${target} PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
		target_include_directories(${target} PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/lib)
	endfunction()

	create_test(test_imageflow tests/test_operations.cpp tests/test_context.cpp tests/test_io.cpp tests/test_error_handling.cpp)

	create_test(test_variations tests/test_variations.cpp)
	create_test(test_fastscaling tests/test_integration.cpp tests/test.cpp tests/test_weighting.cpp tests/test_weighting_helpers.cpp  )

	add_custom_target(do_nothing)
	# Theft library doesn't compile in windows
	if(UNIX)
		#create_test(test_theft_render tests/test_theft_render.cpp)
	endif()

endif()



if(${CMAKE_GENERATOR} MATCHES "Unix Makefiles")
	message(STATUS ${CMAKE_BUILD_TOOL})
	set(CMAKE_MAKE_PROGRAM "${CMAKE_MAKE_PROGRAM} -j10")
	message(STATUS "Added arguments to CMAKE_BUILD_TOOL: ${CMAKE_MAKE_PROGRAM}")
endif()

