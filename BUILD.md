# Building oxidized-cell

This guide covers building oxidized-cell on different platforms with optional LLVM JIT support.

## Prerequisites

### All Platforms
- **Rust**: 1.80 or later (install from [rustup.rs](https://rustup.rs/))
- **CMake**: 3.20 or later
- **C++ compiler**: Supporting C++20

### Platform-Specific

#### Linux
```bash
# Ubuntu/Debian
sudo apt-get install -y \
  cmake \
  build-essential \
  libasound2-dev \
  libudev-dev \
  libxcb-render0-dev \
  libxcb-shape0-dev \
  libxcb-xfixes0-dev \
  libxkbcommon-dev \
  libssl-dev \
  pkg-config

# Fedora/RHEL
sudo dnf install -y \
  cmake \
  gcc-c++ \
  alsa-lib-devel \
  systemd-devel \
  libxcb-devel \
  libxkbcommon-devel \
  openssl-devel
```

#### macOS
```bash
brew install cmake
```

Xcode Command Line Tools are required:
```bash
xcode-select --install
```

#### Windows
- Install [Visual Studio 2022](https://visualstudio.microsoft.com/) with C++ desktop development workload
- Install [CMake](https://cmake.org/download/)
- Ensure CMake is in your PATH

## Building

### Standard Build (Without LLVM JIT)

The simplest way to build oxidized-cell is without LLVM. This will use interpreter mode for PPU/SPU emulation:

```bash
# Clone the repository
git clone https://github.com/darkace1998/oxidized-cell.git
cd oxidized-cell

# Build all crates
cargo build --workspace

# Build optimized release version
cargo build --workspace --release
```

### Build with LLVM JIT (Recommended for Performance)

For best performance, build with LLVM 17 or later for JIT compilation support:

#### Installing LLVM

**Linux (Ubuntu/Debian):**
```bash
# Install LLVM 17
wget https://apt.llvm.org/llvm.sh
chmod +x llvm.sh
sudo ./llvm.sh 17

sudo apt-get install -y llvm-17-dev libclang-17-dev

# Set LLVM_DIR environment variable
export LLVM_DIR=/usr/lib/llvm-17/lib/cmake/llvm
```

**macOS:**
```bash
# Install LLVM 17
brew install llvm@17

# Set LLVM_DIR environment variable
export LLVM_DIR=$(brew --prefix llvm@17)/lib/cmake/llvm
```

**Windows:**
- Download LLVM from [releases page](https://github.com/llvm/llvm-project/releases)
- Install LLVM and add to PATH
- Set `LLVM_DIR` environment variable to `C:\Program Files\LLVM\lib\cmake\llvm`

#### Building with LLVM

```bash
# With LLVM_DIR set, build normally
cargo build --workspace --release
```

The build system will automatically detect LLVM and enable JIT compilation.

## Cross-Compilation

### Linux to ARM64
```bash
# Install cross-compilation tools
sudo apt-get install -y \
  gcc-aarch64-linux-gnu \
  g++-aarch64-linux-gnu

# Add ARM64 target
rustup target add aarch64-unknown-linux-gnu

# Build for ARM64
cargo build --target aarch64-unknown-linux-gnu --release
```

### macOS Universal Binary
```bash
# Build for both architectures
rustup target add x86_64-apple-darwin aarch64-apple-darwin

cargo build --target x86_64-apple-darwin --release
cargo build --target aarch64-apple-darwin --release

# Create universal binary
lipo -create \
  target/x86_64-apple-darwin/release/oxidized-cell \
  target/aarch64-apple-darwin/release/oxidized-cell \
  -output oxidized-cell-universal
```

### Windows MinGW
```bash
# Add MinGW target
rustup target add x86_64-pc-windows-gnu

# Build with MinGW
cargo build --target x86_64-pc-windows-gnu --release
```

## Build Profiles

### Debug Build (Default)
```bash
cargo build --workspace
```
- Fast compilation
- Debug symbols included
- Some optimizations enabled (opt-level = 1)
- Useful for development

### Release Build
```bash
cargo build --workspace --release
```
- Full optimizations
- Thin LTO enabled
- No debug symbols
- Panic = abort
- Best for production use

### Benchmark Build
```bash
cargo bench --workspace
```
- Inherits from release profile
- Includes debug symbols for profiling

## Testing

### Run all tests
```bash
cargo test --workspace
```

### Run specific crate tests
```bash
cargo test -p oc-core
cargo test -p oc-memory
cargo test -p oc-ppu
```

### Run with release optimizations
```bash
cargo test --workspace --release
```

## Code Quality

### Format code
```bash
cargo fmt --all
```

### Check formatting without modifying
```bash
cargo fmt --all -- --check
```

### Run linter
```bash
cargo clippy --workspace --all-targets --all-features
```

### Run linter with strict warnings
```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Common Build Issues

### ALSA not found (Linux)
**Error:** `Package alsa was not found`

**Solution:**
```bash
sudo apt-get install libasound2-dev  # Ubuntu/Debian
sudo dnf install alsa-lib-devel       # Fedora/RHEL
```

### LLVM not found
**Error:** `Could not find LLVM`

**Solution:** Set the `LLVM_DIR` environment variable as described above.

### C++ compiler not found (Windows)
**Error:** `MSVC not found`

**Solution:** Install Visual Studio 2022 with C++ desktop development workload.

### CMake version too old
**Error:** `CMake 3.20 or a later version is required`

**Solution:** Update CMake from [cmake.org/download](https://cmake.org/download/)

## Environment Variables

- `LLVM_DIR` - Path to LLVM CMake files (for JIT support)
- `CARGO_BUILD_JOBS` - Number of parallel build jobs
- `RUST_BACKTRACE` - Set to `1` for detailed error backtraces
- `CARGO_TARGET_DIR` - Custom build output directory

## Build System Architecture

The project uses a hybrid Rust/C++ build system:

1. **Cargo** - Main build orchestrator
   - Builds all Rust crates
   - Invokes CMake for C++ components via `build.rs`

2. **CMake** - C++ component builder
   - Builds C++ library (`liboc_cpp.a`)
   - Handles LLVM integration
   - Platform-specific optimizations

3. **FFI Bridge** - Rust-C++ integration
   - `oc-ffi` crate links C++ library
   - Provides safe Rust wrappers
   - Manages cross-language data types

## CI/CD

The project includes GitHub Actions workflows for:

- **CI** - Automated testing on Linux, Windows, macOS
- **LLVM Build** - Testing with LLVM 17, 18, 19
- **Cross-Compilation** - Building for multiple platforms
- **Release** - Automated binary releases on version tags

See `.github/workflows/` for workflow definitions.
