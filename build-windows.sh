#!/bin/bash
# Build script for Windows cross-compilation
# This script builds oxidized-cell for Windows from Linux

set -e

echo "=== Oxidized-Cell Windows Build Script ==="

# Check for rustup
if ! command -v rustup &> /dev/null; then
    echo "Installing rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

# Add Windows target
echo "Adding Windows target..."
rustup target add x86_64-pc-windows-gnu

# Install MinGW-w64 cross-compiler (includes C++ support)
echo "Installing MinGW-w64 cross-compiler..."
if command -v apt-get &> /dev/null; then
    sudo apt-get update
    # Install posix threading variant for full C++11 std::thread/mutex support
    sudo apt-get install -y mingw-w64 g++-mingw-w64-x86-64-posix
    # Set posix as default (required for std::mutex in C++ code)
    sudo update-alternatives --set x86_64-w64-mingw32-gcc /usr/bin/x86_64-w64-mingw32-gcc-posix 2>/dev/null || true
    sudo update-alternatives --set x86_64-w64-mingw32-g++ /usr/bin/x86_64-w64-mingw32-g++-posix 2>/dev/null || true
elif command -v dnf &> /dev/null; then
    sudo dnf install -y mingw64-gcc mingw64-gcc-c++ mingw64-winpthreads-static
elif command -v pacman &> /dev/null; then
    sudo pacman -S mingw-w64-gcc
else
    echo "Warning: Could not detect package manager. Please install mingw-w64 manually."
fi

# Set up cross-compilation environment for C++
# Either threading model works now since we use Windows native threading primitives
if [ -f /usr/bin/x86_64-w64-mingw32-g++-posix ]; then
    export CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc-posix
    export CXX_x86_64_pc_windows_gnu=x86_64-w64-mingw32-g++-posix
    echo "Using posix threading model compiler"
else
    export CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc
    export CXX_x86_64_pc_windows_gnu=x86_64-w64-mingw32-g++
fi
export AR_x86_64_pc_windows_gnu=x86_64-w64-mingw32-ar

# Get the GCC library paths
GCC_LIB_PATH=$($CXX_x86_64_pc_windows_gnu -print-file-name=libgcc.a 2>/dev/null | xargs dirname || echo "")
MINGW_LIB_PATH="/usr/x86_64-w64-mingw32/lib"
# Get the path to libstdc++ which contains the threading implementation
STDCXX_PATH=$($CXX_x86_64_pc_windows_gnu -print-file-name=libstdc++.a 2>/dev/null | xargs dirname || echo "")

echo "GCC lib path: $GCC_LIB_PATH"
echo "libstdc++ path: $STDCXX_PATH"

# Build RUSTFLAGS with proper library paths and linking
# We use Windows native threading (CRITICAL_SECTION, CreateThread) in C++ code
# so no pthread dependency is needed
# Use static linking for libgcc and libstdc++ to create a standalone executable
LINK_ARGS="-C link-arg=-Wl,--allow-multiple-definition"
LINK_ARGS="$LINK_ARGS -C link-arg=-static-libgcc"
LINK_ARGS="$LINK_ARGS -C link-arg=-static-libstdc++"

# Add library search paths
if [ -n "$GCC_LIB_PATH" ] && [ -d "$GCC_LIB_PATH" ]; then
    LINK_ARGS="$LINK_ARGS -C link-arg=-L$GCC_LIB_PATH"
fi
if [ -n "$STDCXX_PATH" ] && [ -d "$STDCXX_PATH" ]; then
    LINK_ARGS="$LINK_ARGS -C link-arg=-L$STDCXX_PATH"
fi
if [ -d "$MINGW_LIB_PATH" ]; then
    LINK_ARGS="$LINK_ARGS -C link-arg=-L$MINGW_LIB_PATH"
fi

export RUSTFLAGS="$LINK_ARGS"

echo "Building for Windows (including C++ JIT components)..."
echo "RUSTFLAGS: $RUSTFLAGS"
# Build the main oxidized-cell binary
cargo build --release --target x86_64-pc-windows-gnu

echo ""
echo "=== Build Complete ==="
echo "Windows executable is located at:"
echo "  target/x86_64-pc-windows-gnu/release/oxidized-cell.exe"
