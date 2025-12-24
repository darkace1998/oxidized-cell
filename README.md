# 🎮 oxidized-cell

<p align="center">
  <strong>A PlayStation 3 emulator written in Rust and C++</strong>
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#installation">Installation</a> •
  <a href="#building">Building</a> •
  <a href="#usage">Usage</a> •
  <a href="#project-status">Status</a> •
  <a href="#contributing">Contributing</a>
</p>

---

## 📖 Overview

**oxidized-cell** is a hybrid Rust/C++ PlayStation 3 emulator aiming to accurately emulate the Cell Broadband Engine and RSX graphics processor. The project leverages Rust's memory safety for core emulation logic while utilizing C++ with LLVM for high-performance JIT compilation.

> ⚠️ **Early Development**: This project is under active development. Game compatibility is limited and many features are still being implemented.

## ✨ Features

### Core Emulation
- **PPU (PowerPC Processing Unit)** - Full interpreter with 2,700+ lines of instruction handling
  - Integer, floating-point, branch, load/store instructions
  - VMX/AltiVec SIMD support (128-bit vectors)
  - Comprehensive FPSCR flag handling
  - Breakpoint debugging support

- **SPU (Synergistic Processing Unit)** - Complete interpreter implementation
  - 128x 128-bit vector registers
  - 256KB Local Storage per SPU
  - Memory Flow Controller (MFC) basics
  - Channel communication

- **RSX Graphics** - Vulkan-based rendering backend
  - NV4097 method handlers for draw commands
  - 16 vertex attributes, 16 texture units
  - Blend, depth, and stencil state management
  - Multi-frame synchronization

### JIT Compilation (C++ with LLVM)
- PPU JIT with LLVM IR generation for 20+ PowerPC instructions
- SPU JIT with LLVM IR generation for 15+ SIMD instructions
- Basic block compilation with code caching
- O2 optimization passes

### System Emulation
- **LV2 Kernel** - 75% complete syscall implementation
  - Process and thread management
  - Synchronization primitives (mutex, cond, semaphore, rwlock)
  - Memory allocation syscalls
  - Time management

- **Audio System** - cpal backend for cross-platform output
  - 8 audio ports, multi-channel support
  - 48kHz sample rate

- **Input System** - Controller, keyboard, and mouse emulation
  - Customizable key mappings

- **Virtual File System** - ISO 9660, PKG, PARAM.SFO support

### Game Loading
- ELF/SELF file parsing
- PRX shared library loading
- Symbol resolution with NID system


**Current Focus**: Implementing HLE modules (cellGcmSys, cellSysutil, cellSpurs) to enable game execution.

## 🔧 Building

### Prerequisites

- **Rust** 1.70+ (via rustup)
- **C++ Compiler** with C++17 support (GCC 9+, Clang 10+, MSVC 2019+)
- **CMake** 3.16+
- **LLVM** 14+ (for JIT compilation)
- **Vulkan SDK** 1.2+

### Linux (Ubuntu/Debian)

```bash
# Install dependencies
sudo apt update
sudo apt install -y build-essential cmake llvm-dev libvulkan-dev libasound2-dev

# Clone and build
git clone https://github.com/darkace1998/oxidized-cell.git
cd oxidized-cell
cargo build --release
```

### Windows

```powershell
# Install Rust from https://rustup.rs
# Install Visual Studio 2019+ with C++ workload
# Install Vulkan SDK from https://vulkan.lunarg.com

git clone https://github.com/darkace1998/oxidized-cell.git
cd oxidized-cell
cargo build --release
```

### macOS

```bash
# Install dependencies
brew install llvm cmake

# Clone and build
git clone https://github.com/darkace1998/oxidized-cell.git
cd oxidized-cell
cargo build --release
```

## 🚀 Usage

```bash
# Run the emulator (UI mode)
cargo run --release

# Run with a specific game (future)
cargo run --release -- /path/to/game.elf
```

### Configuration

Configuration is stored in `config.toml`:

```toml
[cpu]
ppu_decoder = "interpreter"  # or "jit"
spu_decoder = "interpreter"  # or "jit"

[graphics]
backend = "vulkan"
resolution_scale = 1

[audio]
backend = "cpal"
volume = 100
```

## 📁 Project Structure

```
oxidized-cell/
├── crates/                    # Rust crates
│   ├── oc-core/              # Core emulator, config, logging
│   ├── oc-memory/            # Memory management (4GB virtual, 4KB pages)
│   ├── oc-ppu/               # PPU interpreter & decoder
│   ├── oc-spu/               # SPU interpreter & decoder  
│   ├── oc-rsx/               # RSX graphics & Vulkan backend
│   ├── oc-lv2/               # LV2 kernel syscalls
│   ├── oc-audio/             # Audio system (cpal)
│   ├── oc-input/             # Input handling
│   ├── oc-vfs/               # Virtual file system
│   ├── oc-hle/               # HLE modules (cellGcmSys, etc.)
│   ├── oc-loader/            # ELF/SELF/PRX loader
│   ├── oc-ffi/               # Rust/C++ FFI bridge
│   ├── oc-ui/                # egui user interface
│   └── oc-integration/       # Integration & EmulatorRunner
├── cpp/                       # C++ performance components
│   ├── src/
│   │   ├── ppu_jit.cpp       # PPU JIT (LLVM)
│   │   ├── spu_jit.cpp       # SPU JIT (LLVM)
│   │   ├── rsx_shaders.cpp   # SPIR-V compilation
│   │   ├── atomics.cpp       # 128-byte atomics
│   │   └── simd_avx.cpp      # AVX helpers
│   └── include/
│       └── oc_ffi.h          # FFI header
└── docs/                      # Documentation
```

## 🧪 Testing

```bash
# Run all tests
cargo test

# Run specific crate tests
cargo test -p oc-memory
cargo test -p oc-ppu
cargo test -p oc-spu

# Run with verbose output
cargo test -- --nocapture
```

### Test Coverage
- Memory: 128+ tests
- PPU: 75+ tests  
- SPU: 14+ tests
- RSX: 36+ tests
- Integration: 4+ tests

## 🤝 Contributing

Contributions are welcome! Here's how you can help:

### High Priority Tasks
1. **HLE Modules** - Implement cellGcmSys, cellSysutil, cellSpurs, cellPad, cellFs
2. **Game Loading** - Complete the game loading pipeline
3. **Testing** - Test with PS3 homebrew applications

### Getting Started
1. Fork the repository
2. Create a feature branch (`git checkout -b feature/my-feature`)
3. Make your changes
4. Run tests (`cargo test`)
5. Submit a pull request

### Code Style
- Rust: Follow `rustfmt` and `clippy` conventions
- C++: Use clang-format with project settings
- Write tests for new functionality
- Document public APIs

## 📚 Documentation

- [docs/ppu_instructions.md](docs/ppu_instructions.md) - PPU instruction reference
- [docs/spu_instructions.md](docs/spu_instructions.md) - SPU instruction reference

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         oxidized-cell                            │
├─────────────────────────────────────────────────────────────────┤
│  ┌────────────────────────────────────────────────────────────┐ │
│  │                    Rust Core (70%)                          │ │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │ │
│  │  │  Memory  │ │  Kernel  │ │  Thread  │ │   VFS    │       │ │
│  │  │  Manager │ │  (LV2)   │ │  Manager │ │          │       │ │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘       │ │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │ │
│  │  │  Audio   │ │  Input   │ │  Config  │ │    UI    │       │ │
│  │  │  Backend │ │  Handler │ │  System  │ │  (egui)  │       │ │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘       │ │
│  └────────────────────────────────────────────────────────────┘ │
│                              │ FFI                               │
│                              ▼                                   │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │                 C++ Performance Core (30%)                  │ │
│  │  ┌───────────────┐ ┌───────────────┐ ┌───────────────┐     │ │
│  │  │   PPU JIT     │ │   SPU JIT     │ │  RSX Shaders  │     │ │
│  │  │   (LLVM)      │ │   (LLVM)      │ │  (SPIRV)      │     │ │
│  │  └───────────────┘ └───────────────┘ └───────────────┘     │ │
│  └────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## 📜 License

This project is licensed under the **GPL-3.0 License** - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [RPCS3](https://github.com/RPCS3/rpcs3) - Reference PS3 emulator
- [PS3 Developer Wiki](https://www.psdevwiki.com/) - Documentation resource
- [Cell BE Programming Handbook](https://www.ibm.com/support/pages/cell-be-programming-handbook) - IBM documentation

---

<p align="center">
  <sub>Made with ❤️ by darkace1998</sub>
</p>
