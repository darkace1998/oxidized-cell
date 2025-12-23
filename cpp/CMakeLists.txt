cmake_minimum_required(VERSION 3.20)
project(oxidized-cell-cpp LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# Platform-specific compiler flags
if(CMAKE_SYSTEM_PROCESSOR MATCHES "x86_64|AMD64|amd64")
    set(ARCH_X64 TRUE)
    add_compile_definitions(ARCH_X64)
    if(MSVC)
        add_compile_options(/arch:AVX2)
    else()
        add_compile_options(-mavx2 -mbmi2)
    endif()
elseif(CMAKE_SYSTEM_PROCESSOR MATCHES "aarch64|ARM64|arm64")
    set(ARCH_ARM64 TRUE)
    add_compile_definitions(ARCH_ARM64)
    if(NOT MSVC)
        add_compile_options(-march=armv8-a)
    endif()
endif()

# Try to find LLVM 17 or higher
find_package(LLVM 17 CONFIG)

if(LLVM_FOUND)
    message(STATUS "Found LLVM ${LLVM_PACKAGE_VERSION}")
    message(STATUS "Using LLVMConfig.cmake in: ${LLVM_DIR}")
    
    # Add LLVM include directories
    include_directories(${LLVM_INCLUDE_DIRS})
    separate_arguments(LLVM_DEFINITIONS_LIST NATIVE_COMMAND ${LLVM_DEFINITIONS})
    add_definitions(${LLVM_DEFINITIONS_LIST})
    
    # Define that LLVM is available
    add_compile_definitions(HAVE_LLVM)
    
    # Map LLVM components to libraries
    llvm_map_components_to_libnames(LLVM_LIBS
        core
        executionengine
        mcjit
        native
        orcjit
        passes
    )
    
    # Add architecture-specific components
    if(ARCH_X64)
        llvm_map_components_to_libnames(LLVM_X64_LIBS
            x86asmparser
            x86codegen
            x86desc
            x86info
        )
        list(APPEND LLVM_LIBS ${LLVM_X64_LIBS})
    elseif(ARCH_ARM64)
        llvm_map_components_to_libnames(LLVM_ARM64_LIBS
            aarch64asmparser
            aarch64codegen
            aarch64desc
            aarch64info
        )
        list(APPEND LLVM_LIBS ${LLVM_ARM64_LIBS})
    endif()
    
    set(HAVE_LLVM_SUPPORT TRUE)
else()
    message(WARNING "LLVM 17+ not found. JIT compilation will be disabled.")
    message(WARNING "To enable JIT, install LLVM 17 or higher and set LLVM_DIR")
    set(HAVE_LLVM_SUPPORT FALSE)
endif()

# Library
add_library(oc_cpp STATIC
    src/ffi.cpp
    src/ppu_jit.cpp
    src/spu_jit.cpp
    src/rsx_shaders.cpp
    src/atomics.cpp
    src/dma.cpp
)

if(ARCH_X64)
    target_sources(oc_cpp PRIVATE
        src/simd_avx.cpp
    )
endif()

target_include_directories(oc_cpp PUBLIC include)

# Link LLVM if available
if(HAVE_LLVM_SUPPORT)
    target_link_libraries(oc_cpp PUBLIC ${LLVM_LIBS})
    target_compile_definitions(oc_cpp PUBLIC HAVE_LLVM)
endif()

# Install for Rust linking
install(TARGETS oc_cpp
    ARCHIVE DESTINATION lib
    LIBRARY DESTINATION lib
)

# Install headers
install(DIRECTORY include/
    DESTINATION include
    FILES_MATCHING PATTERN "*.h" PATTERN "*.hpp"
)
