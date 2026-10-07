cmake_minimum_required(VERSION 3.20)
project(zinc-cpp VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Header-only library
add_library(zinc INTERFACE)
target_include_directories(zinc INTERFACE
    ${CMAKE_CURRENT_SOURCE_DIR}/include
    ${CMAKE_CURRENT_LIST_DIR}/../../include
)

# Link the Rust core shared library
# Point ZINC_CORE_LIB to the built libzinc_core.{so,dylib}
if(NOT DEFINED ZINC_CORE_LIB)
    if(APPLE)
        set(ZINC_CORE_LIB_DEFAULT "${CMAKE_CURRENT_LIST_DIR}/../../target/release/libzinc_core.dylib")
    else()
        set(ZINC_CORE_LIB_DEFAULT "${CMAKE_CURRENT_LIST_DIR}/../../target/release/libzinc_core.so")
    endif()
    set(ZINC_CORE_LIB ${ZINC_CORE_LIB_DEFAULT} CACHE FILEPATH "Path to libzinc_core")
endif()

target_link_libraries(zinc INTERFACE ${ZINC_CORE_LIB})

# Optional: build examples
option(ZINC_BUILD_EXAMPLES "Build example programs" OFF)
if(ZINC_BUILD_EXAMPLES)
    file(GLOB EXAMPLES "${CMAKE_CURRENT_SOURCE_DIR}/examples/*.cpp")
    foreach(EXAMPLE IN LISTS EXAMPLES)
        get_filename_component(NAME ${EXAMPLE} NAME_WE)
        add_executable(${NAME} ${EXAMPLE})
        target_link_libraries(${NAME} PRIVATE zinc)
    endforeach()
endif()

include(CTest)
if(BUILD_TESTING)
    add_executable(zinc_cpp_test tests/region.cpp)
    target_link_libraries(zinc_cpp_test PRIVATE zinc)
    add_test(NAME zinc_cpp_test COMMAND zinc_cpp_test)
endif()
