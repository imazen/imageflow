project(ImageFlow)
cmake_minimum_required(VERSION 2.8.12)

if(EXISTS ${CMAKE_CURRENT_SOURCE_DIR}/build/conanbuildinfo.cmake) #Clion, with conanbuildinfo.cmake in root
    include(${CMAKE_CURRENT_SOURCE_DIR}/build/conanbuildinfo.cmake)
else()
    include(${CMAKE_BINARY_DIR}/conanbuildinfo.cmake) # Not CLion
endif()

conan_basic_setup()

option(ENABLE_TEST OFF)
option(ENABLE_PROFILING OFF)
option(SKIP_LIBRARY OFF)

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

if (NOT SKIP_LIBRARY)
	add_library(imageflow SHARED ${LIB_SRCS} ${LIB_HDRS})
	target_link_libraries(imageflow ${CONAN_LIBS})
	target_include_directories(imageflow PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
	target_compile_options(imageflow PRIVATE "-flto")
	target_compile_options(imageflow PRIVATE "-fverbose-asm")
endif()

# -ffast-math changes results
if (UNIX)
	set(FASTSCALING_FLAGS "-iquotelib -fPIC -ffast-math -funroll-loops -mtune=native -march=native -Wpointer-arith -Wcast-qual -Wpedantic -Wall -Wextra -Wno-unused-parameter -Wuninitialized -Wredundant-decls -Werror")
	set(CMAKE_C_FLAGS  "${CMAKE_C_FLAGS} ${FASTSCALING_FLAGS} -std=gnu11 -Wstrict-prototypes -Wmissing-prototypes -Wc++-compat -Wshadow" )
	set(CMAKE_CXX_FLAGS  "${CMAKE_CXX_FLAGS} ${FASTSCALING_FLAGS} -std=gnu++11" )
else()
	set(FASTSCALING_FLAGS "")
	set(CMAKE_C_FLAGS  "${CMAKE_C_FLAGS} ${FASTSCALING_FLAGS}" )
	set(CMAKE_CXX_FLAGS  "${CMAKE_CXX_FLAGS} ${FASTSCALING_FLAGS}" )
endif()

option(COVERAGE "Generate coverage data" OFF)
if (COVERAGE)
	set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -g -O0") # debug, no optimisation
	set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} --coverage")#--fprofile-arcs --ftest-coverage") # enabling coverage
	set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -g -O0") # debug, no optimisation
	set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} --coverage ") # enabling coverage
else()

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
	#create_test(test_context tests/test_context.cpp)
	#create_test(test_io tests/test_io.cpp)
	#create_test(test_visuals tests/test_visuals.cpp)
	#create_test(test_graph_program tests/test_graph.cpp )
	#create_test(test_error_handling  tests/test_error_handling.cpp)
	create_test(test_fastscaling tests/test_simple_fastscaling.cpp  tests/test_integration.cpp tests/test.cpp tests/test_weighting.cpp tests/test_weighting_helpers.cpp  )
#	create_test(test_integration_program  tests/test_integration.cpp )
#	create_test(test_test  tests/test.cpp  )
#	create_test(test_weighting  tests/test_weighting.cpp tests/test_weighting_helpers.cpp )

	add_custom_target(do_nothing)
	# Theft library don't compile in windows
	if(UNIX)
		create_test(test_theft_render tests/test_theft_render.cpp)
	endif()

endif()



if(${CMAKE_GENERATOR} MATCHES "Unix Makefiles")
	message(STATUS ${CMAKE_BUILD_TOOL})
	set(CMAKE_MAKE_PROGRAM "${CMAKE_MAKE_PROGRAM} -j10")
	message(STATUS "Added arguments to CMAKE_BUILD_TOOL: ${CMAKE_MAKE_PROGRAM}")
endif()
