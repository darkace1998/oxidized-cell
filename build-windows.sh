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

# Install MinGW-w64 cross-compiler
echo "Installing MinGW-w64 cross-compiler..."
if command -v apt-get &> /dev/null; then
    sudo apt-get update
    sudo apt-get install -y mingw-w64
elif command -v dnf &> /dev/null; then
    sudo dnf install -y mingw64-gcc mingw64-winpthreads-static
elif command -v pacman &> /dev/null; then
    sudo pacman -S mingw-w64-gcc
else
    echo "Warning: Could not detect package manager. Please install mingw-w64 manually."
fi

echo "Building for Windows..."
# Build the main oxidized-cell binary
cargo build --release --target x86_64-pc-windows-gnu

echo ""
echo "=== Build Complete ==="
echo "Windows executable is located at:"
echo "  target/x86_64-pc-windows-gnu/release/oxidized-cell.exe"
