project(ImageFlow)
cmake_minimum_required(VERSION 2.8.12)

if(EXISTS ${CMAKE_CURRENT_SOURCE_DIR}/build/conanbuildinfo.cmake) #Clion, with conanbuildinfo.cmake in root
    include(${CMAKE_CURRENT_SOURCE_DIR}/build/conanbuildinfo.cmake)
else()
    include(${CMAKE_BINARY_DIR}/conanbuildinfo.cmake) # Not CLion
endif()

conan_basic_setup()

option(ENABLE_TEST OFF)

list(REMOVE_ITEM CONAN_LIBS efence) # FIXME: Maybe efence should be a private dep of openssls

file(GLOB LIB_SRCS lib/*.c)
file(GLOB LIB_HDRS lib/*.h)


add_library(imageflow ${LIB_SRCS} ${LIB_HDRS})
target_link_libraries(imageflow ${CONAN_LIBS})
target_include_directories(imageflow PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})


set(FASTSCALING_FLAGS "") # "-iquotelib -fPIC -O2 -g -Wpointer-arith -Wcast-qual -Wpedantic -Wall -Wextra -Wno-unused-parameter -Wuninitialized -Wredundant-decls -Werror")
set(CMAKE_C_FLAGS  "${CMAKE_C_FLAGS} ${FASTSCALING_FLAGS} -std=gnu11 -Wstrict-prototypes -Wmissing-prototypes -Wc++-compat -Wshadow" )
set(CMAKE_CXX_FLAGS  "${CMAKE_CXX_FLAGS} ${FASTSCALING_FLAGS} -std=gnu++11" )

if(ENABLE_TEST)
    include(CTest)
    enable_testing()
	function(create_test target)
		add_executable(${target} ${ARGN})
		add_test(NAME ${target} COMMAND $<TARGET_FILE:${target}>)
	    target_link_libraries(${target} imageflow ${CONAN_LIBS})
	    target_include_directories(${target} PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
		target_include_directories(${target} PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/lib)	    
	endfunction()

	create_test(fastscaling tests/fastscaling.c)
	create_test(test_integration_program tests/test_integration.cpp tests/test_integration_program.cpp)
	create_test(test_graph_program tests/test_graph.cpp tests/test_graph_program.cpp tests/helpers.c)
	create_test(test_error_handling tests/test_error_handling.cpp tests/test_graph_program.cpp  tests/helpers.c)
	create_test(test_test tests/test.cpp tests/test_graph_program.cpp tests/weighting_test_helpers.cpp tests/helpers.c)
	
	# Theft library don't compile in windows
	if(UNIX)
		create_test(theft_test_main theft_tests/theft_test_main.cpp theft_tests/test_rendering.cpp)
	endif()

endif()
