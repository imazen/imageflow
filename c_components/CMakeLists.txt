project(ImageFlow_C)
cmake_minimum_required(VERSION 2.8.12)

option(ENABLE_TEST ON )
option(ENABLE_PROFILING OFF)

file(GLOB_RECURSE LIB_SRCS lib/*.c)
file(GLOB LIB_HDRS lib/*.h)
list(APPEND LIB_HDRS imageflow.h)
list(APPEND LIB_HDRS imageflow_advanced.h)

list(APPEND LIB_SRCS tests/helpers.c)
list(APPEND LIB_HDRS tests/helpers.h)

SET_SOURCE_FILES_PROPERTIES(LIB_SRCS PROPERTIES LANGUAGE CXX )
SET_SOURCE_FILES_PROPERTIES(LIB_HDRS PROPERTIES LANGUAGE CXX )

set(ARCH_SANDYBRIDGE "sandybridge")
set(ARCH_HASWELL "haswell")

set(MSVC_OPTIMIZE "")
set(ENABLE_INSTRUCTIONS "")
if ("${TARGET_CPU}" STREQUAL "sandybridge")
    SET(TARGET_CPU "${ARCH_SANDYBRIDGE}")
    set(MSVC_OPTIMIZE " /arch:AVX")
    set(ENABLE_INSTRUCTIONS "-mavx")
endif()

if ("${TARGET_CPU}" STREQUAL "haswell")
    SET(TARGET_CPU "${ARCH_HASWELL}")
    set(MSVC_OPTIMIZE " /arch:AVX2")
    set(ENABLE_INSTRUCTIONS "-mavx2")
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
    set(IMAGEFLOW_C_OPTIMIZE "-O3 ${NON_MSVC_OPTIMIZE} -funroll-loops -ffast-math -mfpmath=sse  -msse2 ${ENABLE_INSTRUCTIONS}")
endif()


if (NOT MSVC)
    #message(FATAL_ERROR "Using optimization flags ${IMAGEFLOW_C_OPTIMIZE}")
    #mpfmath=sse is required for accuracy
    set(SHARED_FLAGS "-pipe -iquotelib -fPIC -mfpmath=sse")
    set(SHARED_WARNINGS "-Wpointer-arith -Wcast-qual -Wpedantic -Wall -Wextra -Wno-unused-parameter -Wuninitialized -Wredundant-decls -Wno-error=unused-function -Wno-parentheses -Werror")
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

    target_include_directories(profile_imageflow PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
    target_include_directories(profile_imageflow PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/lib)
    target_compile_options(profile_imageflow PRIVATE "-flto")
endif()


#message(FATAL_ERROR "Using CXX_FLAGS ${CMAKE_CXX_FLAGS}")
#message(FATAL_ERROR "Using C_FLAGS ${CMAKE_C_FLAGS}")

    include(CTest)
    enable_testing()

    add_library(imageflow_test_helpers_OBJECTS OBJECT tests/helpers.c tests/runner.cpp)
    target_include_directories(imageflow_test_helpers_OBJECTS PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
    target_include_directories(imageflow_test_helpers_OBJECTS PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/lib)

    function(create_test target)
        add_executable(${target} $<TARGET_OBJECTS:imageflow_test_helpers_OBJECTS> ${ARGN})
        add_test(NAME ${target} COMMAND $<TARGET_FILE:${target}>)
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




if(${CMAKE_GENERATOR} MATCHES "Unix Makefiles")
    message(STATUS ${CMAKE_BUILD_TOOL})
    set(CMAKE_MAKE_PROGRAM "${CMAKE_MAKE_PROGRAM} -j10")
    message(STATUS "Added arguments to CMAKE_BUILD_TOOL: ${CMAKE_MAKE_PROGRAM}")
endif()
