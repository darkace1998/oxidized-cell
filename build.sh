#!/bin/bash
# Build script for oxidized-cell
# This script automates the build process for different configurations

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default values
BUILD_TYPE="debug"
WITH_LLVM=false
CLEAN=false
RUN_TESTS=false
TARGET=""

# Function to print colored messages
print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to show usage
show_usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Build oxidized-cell emulator

OPTIONS:
    -r, --release       Build in release mode (default: debug)
    -l, --llvm          Build with LLVM JIT support
    -c, --clean         Clean before building
    -t, --test          Run tests after building
    -T, --target TARGET Cross-compile for target triple
    -h, --help          Show this help message

EXAMPLES:
    $0                              # Debug build without LLVM
    $0 --release --llvm             # Release build with LLVM
    $0 --clean --test               # Clean build and run tests
    $0 --target aarch64-apple-darwin --release  # Cross-compile for ARM64 macOS

EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -r|--release)
            BUILD_TYPE="release"
            shift
            ;;
        -l|--llvm)
            WITH_LLVM=true
            shift
            ;;
        -c|--clean)
            CLEAN=true
            shift
            ;;
        -t|--test)
            RUN_TESTS=true
            shift
            ;;
        -T|--target)
            TARGET="$2"
            shift 2
            ;;
        -h|--help)
            show_usage
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            show_usage
            exit 1
            ;;
    esac
done

# Print build configuration
print_info "Build configuration:"
echo "  Build type: $BUILD_TYPE"
echo "  With LLVM: $WITH_LLVM"
echo "  Clean build: $CLEAN"
echo "  Run tests: $RUN_TESTS"
if [ -n "$TARGET" ]; then
    echo "  Target: $TARGET"
fi
echo ""

# Check for required tools
print_info "Checking for required tools..."

if ! command -v cargo &> /dev/null; then
    print_error "cargo not found. Please install Rust from https://rustup.rs/"
    exit 1
fi

if ! command -v cmake &> /dev/null; then
    print_warning "CMake not found. C++ components will not be built."
    print_warning "Install CMake from https://cmake.org/download/"
fi

# Check for LLVM if requested
if [ "$WITH_LLVM" = true ]; then
    if [ -z "$LLVM_DIR" ]; then
        print_warning "LLVM_DIR not set. Attempting to auto-detect..."
        
        # Try to find LLVM
        for version in 19 18 17; do
            if [ -d "/usr/lib/llvm-$version" ]; then
                export LLVM_DIR="/usr/lib/llvm-$version/lib/cmake/llvm"
                print_info "Found LLVM $version at $LLVM_DIR"
                break
            elif command -v llvm-config-$version &> /dev/null; then
                LLVM_PREFIX=$(llvm-config-$version --prefix)
                export LLVM_DIR="$LLVM_PREFIX/lib/cmake/llvm"
                print_info "Found LLVM $version at $LLVM_DIR"
                break
            fi
        done
        
        if [ -z "$LLVM_DIR" ]; then
            print_error "Could not find LLVM 17+. Please set LLVM_DIR or install LLVM."
            exit 1
        fi
    else
        print_info "Using LLVM from: $LLVM_DIR"
    fi
fi

# Clean if requested
if [ "$CLEAN" = true ]; then
    print_info "Cleaning build artifacts..."
    cargo clean
    rm -rf cpp/build
fi

# Build cargo arguments
CARGO_ARGS="--workspace"

if [ "$BUILD_TYPE" = "release" ]; then
    CARGO_ARGS="$CARGO_ARGS --release"
fi

if [ -n "$TARGET" ]; then
    CARGO_ARGS="$CARGO_ARGS --target $TARGET"
    
    # Add target if not already installed
    print_info "Adding Rust target: $TARGET"
    rustup target add "$TARGET" 2>/dev/null || true
fi

# Build
print_info "Building oxidized-cell..."
if cargo build $CARGO_ARGS; then
    print_info "Build completed successfully!"
else
    print_error "Build failed!"
    exit 1
fi

# Run tests if requested
if [ "$RUN_TESTS" = true ]; then
    print_info "Running tests..."
    if cargo test $CARGO_ARGS; then
        print_info "All tests passed!"
    else
        print_error "Some tests failed!"
        exit 1
    fi
fi

# Print build output location
if [ -n "$TARGET" ]; then
    BUILD_DIR="target/$TARGET/$BUILD_TYPE"
else
    BUILD_DIR="target/$BUILD_TYPE"
fi

print_info "Build artifacts are in: $BUILD_DIR"

# List executables
if [ -d "$BUILD_DIR" ]; then
    EXECUTABLES=$(find "$BUILD_DIR" -maxdepth 1 -type f -executable 2>/dev/null | head -5)
    if [ -n "$EXECUTABLES" ]; then
        print_info "Executables:"
        echo "$EXECUTABLES" | while read -r exe; do
            echo "  - $(basename "$exe")"
        done
    fi
fi

print_info "Done!"
