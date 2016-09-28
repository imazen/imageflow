project(ImageFlow)
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

SET_SOURCE_FILES_PROPERTIES(LIB_SRCS PROPERTIES LANGUAGE CXX )
SET_SOURCE_FILES_PROPERTIES(LIB_HDRS PROPERTIES LANGUAGE CXX )

if (NOT SKIP_LIBRARY)
	add_library(imageflow ${LIB_SRCS} ${LIB_HDRS})
	set_target_properties(imageflow PROPERTIES LIBRARY_OUTPUT_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/lib)
	set_target_properties(imageflow PROPERTIES ARCHIVE_OUTPUT_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/lib)

	target_link_libraries(imageflow ${CONAN_LIBS})
	target_include_directories(imageflow PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
	# target_compile_options(imageflow PRIVATE "-flto")
	target_compile_options(imageflow PRIVATE "-fverbose-asm")
endif()

if (BUILD_SHARED_LIBS)
add_definitions(-Dimageflow_BUILD_SHARED)
else()
add_definitions(-Dimageflow_BUILD_STATIC)
endif()

# -ffast-math changes results
# -no-parenthesis is unfortunate, but gcc 4.8.5 broke all unit test macros.

if (UNIX)
	set(FASTSCALING_FLAGS "-O3 -iquotelib -fPIC -ffast-math -mfpmath=sse  -msse2 -funroll-loops -mtune=native -march=native -Wpointer-arith -Wcast-qual -Wpedantic -Wall -Wextra -Wno-unused-parameter -Wuninitialized -Wredundant-decls -Wno-error=unused-function -Wno-parentheses -Werror")
	set(CMAKE_C_FLAGS  "${CMAKE_C_FLAGS} ${FASTSCALING_FLAGS} -std=gnu11 -Wstrict-prototypes -Wmissing-prototypes -Wc++-compat -Wshadow" )
	set(CMAKE_CXX_FLAGS  "${CMAKE_CXX_FLAGS} ${FASTSCALING_FLAGS} -std=gnu++11" )
else()

	set(FASTSCALING_FLAGS " /fp:fast")
	set(CMAKE_C_FLAGS  "${CMAKE_C_FLAGS} ${FASTSCALING_FLAGS}" )
	set(CMAKE_CXX_FLAGS  "${CMAKE_CXX_FLAGS} ${FASTSCALING_FLAGS}" )
endif()

option(COVERAGE "Generate coverage data" OFF)

option(DEBUG_BUILD "Generate debug build data" OFF)

if (COVERAGE)
	set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -g -O0") # debug, no optimisation
	set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} --coverage")#--fprofile-arcs --ftest-coverage") # enabling coverage
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

	add_executable(profile_imageflow tests/profile_imageflow.c tests/helpers_visual.c tests/helpers.c ${LIB_SRCS} ${LIB_HDRS})
	target_link_libraries(profile_imageflow ${CONAN_LIBS})
	target_include_directories(profile_imageflow PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
	target_include_directories(profile_imageflow PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/lib)
	target_compile_options(profile_imageflow PRIVATE "-flto")
endif()


if(ENABLE_TEST)
    include(CTest)
    enable_testing()

	add_library(imageflow_test_helpers_OBJECTS OBJECT tests/helpers.c tests/helpers_visual.c tests/runner.cpp)
	target_include_directories(imageflow_test_helpers_OBJECTS PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
	target_include_directories(imageflow_test_helpers_OBJECTS PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/lib)

	function(create_test target)
		add_executable(${target} $<TARGET_OBJECTS:imageflow_test_helpers_OBJECTS> ${ARGN})
		add_test(NAME ${target} COMMAND $<TARGET_FILE:${target}>)
	    target_link_libraries(${target} imageflow ${CONAN_LIBS})
	    target_include_directories(${target} PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
		target_include_directories(${target} PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/lib)
	endfunction()

	create_test(test_imageflow tests/test_operations.cpp tests/test_context.cpp tests/test_io.cpp tests/test_graph.cpp tests/test_error_handling.cpp tests/test_visuals.cpp)

	create_test(test_variations tests/test_variations.cpp)
	create_test(test_fastscaling tests/test_simple_fastscaling.cpp  tests/test_integration.cpp tests/test.cpp tests/test_weighting.cpp tests/test_weighting_helpers.cpp  )

	add_custom_target(do_nothing)
	# Theft library doesn't compile in windows
	if(UNIX)
		create_test(test_theft_render tests/test_theft_render.cpp)
	endif()

endif()



if(${CMAKE_GENERATOR} MATCHES "Unix Makefiles")
	message(STATUS ${CMAKE_BUILD_TOOL})
	set(CMAKE_MAKE_PROGRAM "${CMAKE_MAKE_PROGRAM} -j10")
	message(STATUS "Added arguments to CMAKE_BUILD_TOOL: ${CMAKE_MAKE_PROGRAM}")
endif()


# VALGRIND INTEGRATION
find_program( MEMORYCHECK_COMMAND valgrind )
set( MEMORYCHECK_COMMAND_OPTIONS "--trace-children=yes --leak-check=full" )
# set( MEMORYCHECK_SUPPRESSIONS_FILE "${PROJECT_SOURCE_DIR}/valgrind_suppress.txt" )
