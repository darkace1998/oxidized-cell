cmake_minimum_required(VERSION 3.20)
project(oxidized-cell-cpp LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# LLVM Configuration
find_package(LLVM CONFIG)
if(LLVM_FOUND)
    message(STATUS "Found LLVM ${LLVM_PACKAGE_VERSION}")
    message(STATUS "Using LLVMConfig.cmake in: ${LLVM_DIR}")
    
    include_directories(${LLVM_INCLUDE_DIRS})
    separate_arguments(LLVM_DEFINITIONS_LIST NATIVE_COMMAND ${LLVM_DEFINITIONS})
    add_definitions(${LLVM_DEFINITIONS_LIST})
    add_compile_definitions(HAVE_LLVM)
    
    # Required LLVM components for JIT compilation
    llvm_map_components_to_libnames(LLVM_LIBS
        Core
        ExecutionEngine
        MCJIT
        OrcJIT
        Support
        Target
        PowerPC
        X86
        AArch64
        TransformUtils
        Analysis
        Passes
    )
    
    message(STATUS "LLVM libraries: ${LLVM_LIBS}")
else()
    message(WARNING "LLVM not found. JIT will use placeholder implementation.")
    set(LLVM_LIBS "")
endif()

# Platform-specific
if(CMAKE_SYSTEM_PROCESSOR MATCHES "x86_64|AMD64")
    set(ARCH_X64 TRUE)
    add_compile_definitions(ARCH_X64)
    if(MSVC)
        add_compile_options(/arch:AVX2)
    else()
        add_compile_options(-mavx2 -mbmi2)
    endif()
elseif(CMAKE_SYSTEM_PROCESSOR MATCHES "aarch64|ARM64")
    set(ARCH_ARM64 TRUE)
    add_compile_definitions(ARCH_ARM64)
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
if(LLVM_FOUND)
    target_link_libraries(oc_cpp PUBLIC ${LLVM_LIBS})
endif()

# Install for Rust linking
install(TARGETS oc_cpp
    ARCHIVE DESTINATION lib
    LIBRARY DESTINATION lib
)
