# PS3 Emulator Project Specification

## Hybrid Rust/C++ Architecture

**Project Codename:** `oxidized-cell`
**Target Platforms:** Windows, Linux, macOS, (future: Android/iOS)
**License:** GPL-3.0 (to allow referencing RPCS3)

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Project Structure](#project-structure)
3. [Build System](#build-system)
4. [Phase 1: Foundation](#phase-1-foundation)
5. [Phase 2: Memory Management](#phase-2-memory-management)
6. [Phase 3: PPU Emulation](#phase-3-ppu-emulation)
7. [Phase 4: SPU Emulation](#phase-4-spu-emulation)
8. [Phase 5: RSX Graphics](#phase-5-rsx-graphics)
9. [Phase 6: LV2 Kernel](#phase-6-lv2-kernel)
10. [Phase 7: Audio System](#phase-7-audio-system)
11. [Phase 8: Input System](#phase-8-input-system)
12. [Phase 9: File Systems](#phase-9-file-systems)
13. [Phase 10: JIT Compilation](#phase-10-jit-compilation)
14. [Phase 11: HLE Modules](#phase-11-hle-modules)
15. [Phase 12: Optimization](#phase-12-optimization)
16. [Testing Strategy](#testing-strategy)
17. [FFI Interface Specification](#ffi-interface-specification)
18. [Error Handling](#error-handling)
19. [Logging System](#logging-system)
20. [Configuration System](#configuration-system)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              oxidized-cell                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                         Rust Core (70%)                                 │ │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐      │ │
│  │  │   Memory    │ │   Kernel    │ │   Thread    │ │    VFS      │      │ │
│  │  │   Manager   │ │   (LV2)     │ │   Manager   │ │             │      │ │
│  │  └─────────────┘ └─────────────┘ └─────────────┘ └─────────────┘      │ │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐      │ │
│  │  │    Audio    │ │    Input    │ │   Config    │ │     UI      │      │ │
│  │  │   Backend   │ │   Handler   │ │   System    │ │   (egui)    │      │ │
│  │  └─────────────┘ └─────────────┘ └─────────────┘ └─────────────┘      │ │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐      │ │
│  │  │  Decoder    │ │  RSX State  │ │  Scheduler  │ │   Logging   │      │ │
│  │  │  (PPU/SPU)  │ │  Manager    │ │             │ │             │      │ │
│  │  └─────────────┘ └─────────────┘ └─────────────┘ └─────────────┘      │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                      │                                       │
│                                      │ FFI Boundary                          │
│                                      ▼                                       │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                       C++ Performance Core (30%)                        │ │
│  │  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐       │ │
│  │  │    PPU JIT       │ │    SPU JIT       │ │   RSX Shaders    │       │ │
│  │  │    (LLVM)        │ │    (LLVM)        │ │   (SPIRV-Cross)  │       │ │
│  │  └──────────────────┘ └──────────────────┘ └──────────────────┘       │ │
│  │  ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐       │ │
│  │  │  128-byte        │ │    DMA Engine    │ │   SIMD Helpers   │       │ │
│  │  │  Atomics         │ │                  │ │                  │       │ │
│  │  └──────────────────┘ └──────────────────┘ └──────────────────┘       │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Technology Stack

| Component | Technology | Justification |
|-----------|------------|---------------|
| Core Logic | Rust | Memory safety, concurrency |
| JIT Compiler | C++ + LLVM | Direct API access |
| Graphics | Vulkan (ash) | Cross-platform, modern |
| UI | egui | Pure Rust, immediate mode |
| Audio | cpal | Cross-platform Rust |
| Build | Cargo + CMake | Best of both worlds |
| Testing | cargo test + catch2 | Native tooling |

---

## Project Structure

```
oxidized-cell/
├── Cargo.toml                    # Workspace root
├── CMakeLists.txt                # C++ build configuration
├── rust-toolchain.toml           # Rust version pinning
├── . cargo/
│   └── config.toml               # Cargo configuration
│
├── crates/                       # Rust crates
│   ├── oc-core/                  # Core emulator logic
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── emulator.rs       # Main emulator struct
│   │       ├── error.rs          # Error types
│   │       └── config.rs         # Configuration
│   │
│   ├── oc-memory/                # Memory management
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── manager.rs        # Memory manager
│   │       ├── reservation.rs    # Atomic reservations
│   │       ├── mapping.rs        # Memory mapping
│   │       └── pages.rs          # Page management
│   │
│   ├── oc-ppu/                   # PPU emulation
│   │   ├── Cargo. toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── thread.rs         # PPU thread state
│   │       ├── decoder.rs        # Instruction decoder
│   │       ├── interpreter.rs    # PPU interpreter
│   │       ├── instructions/     # Instruction implementations
│   │       │   ├── mod.rs
│   │       │   ├── branch.rs
│   │       │   ├── integer.rs
│   │       │   ├── float.rs
│   │       │   ├── vector.rs
│   │       │   ├── load_store.rs
│   │       │   └── system.rs
│   │       └── vmx.rs            # AltiVec/VMX
│   │
│   ├── oc-spu/                   # SPU emulation
│   │   ├── Cargo. toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── thread.rs         # SPU thread state
│   │       ├── decoder.rs        # Instruction decoder
│   │       ├── interpreter. rs    # SPU interpreter
│   │       ├── mfc.rs            # Memory Flow Controller
│   │       ├── channels.rs       # SPU channels
│   │       ├── instructions/
│   │       │   ├── mod.rs
│   │       │   ├── memory.rs
│   │       │   ├── arithmetic.rs
│   │       │   ├── logical.rs
│   │       │   ├── compare.rs
│   │       │   ├── branch.rs
│   │       │   ├── float.rs
│   │       │   └── channel.rs
│   │       └── atomics.rs        # GETLLAR/PUTLLC handling
│   │
│   ├── oc-rsx/                   # RSX graphics
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── thread. rs         # RSX command processor
│   │       ├── fifo.rs           # Command FIFO
│   │       ├── state.rs          # Graphics state
│   │       ├── methods.rs        # NV4097 method handlers
│   │       ├── vertex.rs         # Vertex processing
│   │       ├── texture.rs        # Texture handling
│   │       └── backend/
│   │           ├── mod.rs
│   │           ├── vulkan.rs     # Vulkan backend
│   │           └── null.rs       # Null backend (testing)
│   │
│   ├── oc-lv2/                   # LV2 kernel (HLE)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── syscall.rs        # Syscall dispatcher
│   │       ├── process.rs        # sys_process_*
│   │       ├── thread.rs         # sys_ppu_thread_*
│   │       ├── spu.rs            # sys_spu_*
│   │       ├── memory.rs         # sys_memory_*
│   │       ├── sync/
│   │       │   ├── mod.rs
│   │       │   ├── mutex.rs      # sys_mutex_*
│   │       │   ├── cond.rs       # sys_cond_*
│   │       │   ├── rwlock.rs     # sys_rwlock_*
│   │       │   ├── semaphore.rs  # sys_semaphore_*
│   │       │   └── event.rs      # sys_event_*
│   │       ├── fs. rs             # sys_fs_*
│   │       ├── time.rs           # sys_time_*
│   │       └── prx.rs            # PRX loading
│   │
│   ├── oc-audio/                 # Audio system
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── thread.rs         # Audio thread
│   │       ├── cell_audio.rs     # cellAudio HLE
│   │       ├── mixer.rs          # Audio mixing
│   │       └── backend/
│   │           ├── mod.rs
│   │           ├── cpal.rs       # cpal backend
│   │           └── null.rs       # Null backend
│   │
│   ├── oc-input/                 # Input handling
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── pad.rs            # cellPad
│   │       ├── keyboard.rs       # cellKb
│   │       ├── mouse. rs          # cellMouse
│   │       └── mapping.rs        # Input mapping
│   │
│   ├── oc-vfs/                   # Virtual file system
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── mount.rs          # Mount points
│   │       ├── devices/
│   │       │   ├── mod.rs
│   │       │   ├── hdd.rs        # /dev_hdd0
│   │       │   ├── bdvd.rs       # /dev_bdvd
│   │       │   ├── usb.rs        # /dev_usb*
│   │       │   └── flash.rs      # /dev_flash
│   │       └── formats/
│   │           ├── mod.rs
│   │           ├── iso.rs        # ISO 9660
│   │           ├── pkg.rs        # PKG files
│   │           └── sfo.rs        # PARAM. SFO
│   │
│   ├── oc-hle/                   # HLE modules
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib. rs
│   │       ├── module.rs         # Module registry
│   │       ├── cellGcmSys.rs     # RSX management
│   │       ├── cellSpurs.rs      # SPURS
│   │       ├── cellFs.rs         # File system
│   │       ├── cellPad.rs        # Controller
│   │       ├── cellAudio.rs      # Audio
│   │       ├── cellSysutil.rs    # System utilities
│   │       ├── cellGame.rs       # Game data
│   │       ├── cellSaveData.rs   # Save data
│   │       ├── cellNetCtl.rs     # Network
│   │       ├── cellSsl.rs        # SSL
│   │       ├── cellHttp.rs       # HTTP
│   │       ├── cellFont.rs       # Font rendering
│   │       ├── cellPngDec.rs     # PNG decoder
│   │       ├── cellJpgDec.rs     # JPEG decoder
│   │       ├── cellGifDec.rs     # GIF decoder
│   │       ├── cellVpost.rs      # Video post-processing
│   │       ├── cellDmux.rs       # Demuxer
│   │       ├── cellVdec.rs       # Video decoder
│   │       ├── cellAdec.rs       # Audio decoder
│   │       └── libsre.rs         # Regular expressions
│   │
│   ├── oc-loader/                # Game/ELF loader
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── elf.rs            # ELF parser
│   │       ├── self.rs           # SELF decryption
│   │       ├── prx.rs            # PRX loader
│   │       └── crypto.rs         # Decryption keys
│   │
│   ├── oc-ffi/                   # FFI bridge to C++
│   │   ├── Cargo.toml
│   │   ├── build.rs              # Build script for C++ linking
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── jit.rs            # JIT interface
│   │       ├── atomics.rs        # Atomic operations
│   │       ├── simd.rs           # SIMD helpers
│   │       └── types.rs          # Shared types
│   │
│   └── oc-ui/                    # User interface
│       ├── Cargo. toml
│       └── src/
│           ├── lib.rs
│           ├── app.rs            # Main application
│           ├── game_list.rs      # Game browser
│           ├── settings.rs       # Settings UI
│           ├── debugger.rs       # Debugger UI
│           └── themes.rs         # UI themes
│
├── cpp/                          # C++ components
│   ├── CMakeLists.txt
│   ├── include/
│   │   ├── oc_ffi.h              # FFI header (shared with Rust)
│   │   ├── ppu_jit.hpp           # PPU JIT compiler
│   │   ├── spu_jit.hpp           # SPU JIT compiler
│   │   ├── rsx_shaders.hpp       # RSX shader compiler
│   │   ├── atomics.hpp           # 128-byte atomics
│   │   ├── simd.hpp              # SIMD helpers
│   │   └── dma.hpp               # DMA engine
│   │
│   └── src/
│       ├── ffi.cpp               # FFI implementation
│       ├── ppu_jit. cpp           # PPU JIT
│       ├── ppu_jit_ops.cpp       # PPU operation emitters
│       ├── spu_jit.cpp           # SPU JIT
│       ├── spu_jit_ops.cpp       # SPU operation emitters
│       ├── rsx_shaders.cpp       # Shader compilation
│       ├── atomics.cpp           # Atomic implementations
│       ├── simd_avx.cpp          # AVX implementations
│       ├── simd_avx512.cpp       # AVX-512 implementations
│       └── dma.cpp               # DMA engine
│
├── tests/                        # Integration tests
│   ├── roms/                     # Test ROMs (gitignored)
│   ├── ppu_tests.rs
│   ├── spu_tests. rs
│   ├── memory_tests.rs
│   └── integration_tests.rs
│
├── benches/                      # Benchmarks
│   ├── ppu_bench.rs
│   ├── spu_bench.rs
│   ├── memory_bench.rs
│   └── jit_bench.rs
│
├── docs/                         # Documentation
│   ├── architecture.md
│   ├── ppu_instructions.md
│   ├── spu_instructions.md
│   ├── rsx_methods.md
│   ├── syscalls.md
│   └── hle_modules.md
│
└── tools/                        # Development tools
    ├── elf_dump/                 # ELF dumper
    ├── shader_dump/              # Shader dumper
    └── trace_analyzer/           # Execution trace analyzer
```

---

## Build System

### Cargo.toml (Workspace Root)

```toml
[workspace]
resolver = "2"
members = [
    "crates/oc-core",
    "crates/oc-memory",
    "crates/oc-ppu",
    "crates/oc-spu",
    "crates/oc-rsx",
    "crates/oc-lv2",
    "crates/oc-audio",
    "crates/oc-input",
    "crates/oc-vfs",
    "crates/oc-hle",
    "crates/oc-loader",
    "crates/oc-ffi",
    "crates/oc-ui",
]

[workspace. package]
version = "0.1.0"
edition = "2021"
rust-version = "1.75"
license = "GPL-3.0"
repository = "https://github.com/user/oxidized-cell"

[workspace.dependencies]
# Logging
tracing = "0.1"
tracing-subscriber = "0.3"

# Error handling
thiserror = "1.0"
anyhow = "1.0"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"

# Async
tokio = { version = "1.0", features = ["full"] }

# Graphics
ash = "0.37"
gpu-allocator = "0.25"

# Audio
cpal = "0.15"

# UI
eframe = "0.27"
egui = "0.27"

# Numerics
bytemuck = { version = "1.14", features = ["derive"] }
bitflags = "2.4"

# Concurrency
parking_lot = "0.12"
crossbeam = "0.8"

# FFI
libc = "0.2"

# Testing
criterion = "0.5"

[profile.release]
lto = "thin"
codegen-units = 1
panic = "abort"

[profile.dev]
opt-level = 1  # Some optimization for usable debug builds

[profile.bench]
inherits = "release"
debug = true
```

### CMakeLists.txt (C++ Root)

```cmake
cmake_minimum_required(VERSION 3.20)
project(oxidized-cell-cpp LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# LLVM
find_package(LLVM 17 REQUIRED CONFIG)
message(STATUS "Found LLVM ${LLVM_PACKAGE_VERSION}")
include_directories(${LLVM_INCLUDE_DIRS})
add_definitions(${LLVM_DEFINITIONS})

# SPIRV-Cross
find_package(spirv_cross_core REQUIRED)
find_package(spirv_cross_glsl REQUIRED)

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
    src/ppu_jit_ops.cpp
    src/spu_jit.cpp
    src/spu_jit_ops.cpp
    src/rsx_shaders.cpp
    src/atomics. cpp
    src/dma.cpp
)

if(ARCH_X64)
    target_sources(oc_cpp PRIVATE
        src/simd_avx. cpp
        src/simd_avx512.cpp
    )
endif()

target_include_directories(oc_cpp PUBLIC include)

# Link LLVM
llvm_map_components_to_libnames(LLVM_LIBS
    core
    executionengine
    mcjit
    native
    orcjit
    passes
    x86asmparser
    x86codegen
    aarch64asmparser
    aarch64codegen
)
target_link_libraries(oc_cpp PUBLIC ${LLVM_LIBS})
target_link_libraries(oc_cpp PUBLIC spirv-cross-core spirv-cross-glsl)

# Install for Rust linking
install(TARGETS oc_cpp
    ARCHIVE DESTINATION lib
    LIBRARY DESTINATION lib
)
```

---

## Phase 1: Foundation

### Goals
- Project structure setup
- Basic error handling
- Logging infrastructure
- Configuration system

### Tasks

#### 1.1 Create Workspace Structure

```bash
# Initialize workspace
cargo new --lib crates/oc-core
cargo new --lib crates/oc-memory
# ...  etc
```

#### 1.2 Error Handling (oc-core/src/error.rs)

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EmulatorError {
    #[error("Memory error: {0}")]
    Memory(#[from] MemoryError),
    
    #[error("PPU error: {0}")]
    Ppu(#[from] PpuError),
    
    #[error("SPU error: {0}")]
    Spu(#[from] SpuError),
    
    #[error("RSX error: {0}")]
    Rsx(#[from] RsxError),
    
    #[error("Kernel error: {0}")]
    Kernel(#[from] KernelError),
    
    #[error("Loader error: {0}")]
    Loader(#[from] LoaderError),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Game not found: {0}")]
    GameNotFound(String),
    
    #[error("Unsupported feature: {0}")]
    Unsupported(String),
}

#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("Out of memory")]
    OutOfMemory,
    
    #[error("Invalid address: 0x{0:08x}")]
    InvalidAddress(u32),
    
    #[error("Access violation at 0x{addr:08x}:  {kind}")]
    AccessViolation { addr: u32, kind: AccessKind },
    
    #[error("Reservation conflict at 0x{0:08x}")]
    ReservationConflict(u32),
}

#[derive(Error, Debug)]
pub enum PpuError {
    #[error("Invalid instruction at 0x{addr:08x}:  0x{opcode:08x}")]
    InvalidInstruction { addr: u32, opcode: u32 },
    
    #[error("Syscall failed: {0}")]
    SyscallFailed(i32),
}

#[derive(Error, Debug)]
pub enum SpuError {
    #[error("Invalid instruction at 0x{addr:05x}: 0x{opcode:08x}")]
    InvalidInstruction { addr: u32, opcode: u32 },
    
    #[error("MFC error: {0}")]
    MfcError(String),
    
    #[error("Channel timeout: {0}")]
    ChannelTimeout(u32),
}

#[derive(Error, Debug)]
pub enum KernelError {
    #[error("Unknown syscall: {0}")]
    UnknownSyscall(u64),
    
    #[error("Invalid ID: {0}")]
    InvalidId(u32),
    
    #[error("Resource limit exceeded")]
    ResourceLimit,
    
    #[error("Permission denied")]
    PermissionDenied,
}

#[derive(Debug, Clone, Copy)]
pub enum AccessKind {
    Read,
    Write,
    Execute,
}

impl std::fmt::Display for AccessKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read => write!(f, "read"),
            Self:: Write => write!(f, "write"),
            Self::Execute => write!(f, "execute"),
        }
    }
}

pub type Result<T> = std::result::Result<T, EmulatorError>;
```

#### 1.3 Configuration (oc-core/src/config.rs)

```rust
use serde::{Deserialize, Serialize};
use std::path:: PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub general: GeneralConfig,
    pub cpu: CpuConfig,
    pub gpu: GpuConfig,
    pub audio: AudioConfig,
    pub input: InputConfig,
    pub paths: PathConfig,
    pub debug: DebugConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub start_paused: bool,
    pub confirm_exit: bool,
    pub auto_save_state: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CpuConfig {
    pub ppu_decoder: PpuDecoder,
    pub spu_decoder: SpuDecoder,
    pub ppu_threads: u32,
    pub spu_threads:  u32,
    pub accurate_dfma: bool,
    pub accurate_rsx_reservation: bool,
    pub spu_loop_detection: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum PpuDecoder {
    Interpreter,
    #[default]
    Recompiler,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum SpuDecoder {
    Interpreter,
    #[default]
    Recompiler,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GpuConfig {
    pub backend: GpuBackend,
    pub resolution_scale: u32,
    pub anisotropic_filter: u32,
    pub vsync: bool,
    pub frame_limit: u32,
    pub shader_cache:  bool,
    pub write_color_buffers: bool,
    pub write_depth_buffer: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum GpuBackend {
    #[default]
    Vulkan,
    Null,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    pub backend: AudioBackend,
    pub enable:  bool,
    pub volume: f32,
    pub buffer_duration_ms: u32,
    pub time_stretching: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum AudioBackend {
    #[default]
    Auto,
    Null,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InputConfig {
    pub controller:  ControllerConfig,
    pub keyboard_mapping: KeyboardMapping,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ControllerConfig {
    pub player1: Option<String>,
    pub player2: Option<String>,
    pub player3: Option<String>,
    pub player4: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeyboardMapping {
    // Maps keyboard keys to PS3 buttons
    pub cross: String,
    pub circle: String,
    pub square: String,
    pub triangle:  String,
    pub l1: String,
    pub l2: String,
    pub l3: String,
    pub r1: String,
    pub r2: String,
    pub r3: String,
    pub start: String,
    pub select: String,
    pub dpad_up: String,
    pub dpad_down: String,
    pub dpad_left: String,
    pub dpad_right: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PathConfig {
    pub games: PathBuf,
    pub dev_hdd0: PathBuf,
    pub dev_hdd1: PathBuf,
    pub dev_flash:  PathBuf,
    pub save_data: PathBuf,
    pub shader_cache: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DebugConfig {
    pub log_level: LogLevel,
    pub log_to_file: bool,
    pub log_path: PathBuf,
    pub dump_shaders: bool,
    pub trace_ppu: bool,
    pub trace_spu: bool,
    pub trace_rsx: bool,
    pub breakpoints: Vec<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum LogLevel {
    Off,
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            cpu: CpuConfig::default(),
            gpu: GpuConfig::default(),
            audio: AudioConfig::default(),
            input: InputConfig::default(),
            paths: PathConfig:: default(),
            debug: DebugConfig::default(),
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            start_paused: false,
            confirm_exit: true,
            auto_save_state: false,
        }
    }
}

impl Default for CpuConfig {
    fn default() -> Self {
        Self {
            ppu_decoder: PpuDecoder::default(),
            spu_decoder: SpuDecoder::default(),
            ppu_threads: 2,
            spu_threads: 6,
            accurate_dfma: false,
            accurate_rsx_reservation:  false,
            spu_loop_detection: true,
        }
    }
}

impl Default for GpuConfig {
    fn default() -> Self {
        Self {
            backend: GpuBackend:: default(),
            resolution_scale: 100,
            anisotropic_filter:  8,
            vsync: true,
            frame_limit: 60,
            shader_cache: true,
            write_color_buffers: false,
            write_depth_buffer: false,
        }
    }
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            backend: AudioBackend::default(),
            enable:  true,
            volume:  1.0,
            buffer_duration_ms: 100,
            time_stretching: true,
        }
    }
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            controller: ControllerConfig::default(),
            keyboard_mapping:  KeyboardMapping {
                cross: "X".to_string(),
                circle: "C".to_string(),
                square: "Z".to_string(),
                triangle: "V".to_string(),
                l1: "Q".to_string(),
                l2: "1".to_string(),
                l3: "F".to_string(),
                r1: "E".to_string(),
                r2: "3".to_string(),
                r3: "G".to_string(),
                start: "Return".to_string(),
                select: "Backspace".to_string(),
                dpad_up: "Up".to_string(),
                dpad_down: "Down". to_string(),
                dpad_left: "Left".to_string(),
                dpad_right:  "Right".to_string(),
            },
        }
    }
}

impl Default for PathConfig {
    fn default() -> Self {
        let base = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from(". "))
            .join("oxidized-cell");
        
        Self {
            games: base.join("games"),
            dev_hdd0: base.join("dev_hdd0"),
            dev_hdd1: base. join("dev_hdd1"),
            dev_flash: base.join("dev_flash"),
            save_data: base. join("savedata"),
            shader_cache: base. join("cache/shaders"),
        }
    }
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            log_level: LogLevel::default(),
            log_to_file: false,
            log_path: PathBuf::from("oxidized-cell.log"),
            dump_shaders: false,
            trace_ppu:  false,
            trace_spu: false,
            trace_rsx: false,
            breakpoints:  Vec::new(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std:: error::Error>> {
        let path = Self::config_path();
        
        if path.exists() {
            let content = std::fs:: read_to_string(&path)?;
            Ok(toml::from_str(&content)?)
        } else {
            let config = Self::default();
            config.save()?;
            Ok(config)
        }
    }
    
    pub fn save(&self) -> Result<(), Box<dyn std:: error::Error>> {
        let path = Self::config_path();
        
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
    
    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf:: from(". "))
            .join("oxidized-cell")
            .join("config.toml")
    }
}
```

#### 1.4 Logging (oc-core/src/logging.rs)

```rust
use tracing:: Level;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use crate::config: :{Config, LogLevel};

pub fn init(config: &Config) {
    let level = match config.debug.log_level {
        LogLevel:: Off => return,
        LogLevel::Error => Level:: ERROR,
        LogLevel::Warn => Level::WARN,
        LogLevel:: Info => Level::INFO,
        LogLevel::Debug => Level:: DEBUG,
        LogLevel::Trace => Level::TRACE,
    };
    
    let filter = EnvFilter:: from_default_env()
        .add_directive(level.into());
    
    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer()
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true));
    
    if config.debug.log_to_file {
        let file = std::fs:: File::create(&config.debug.log_path)
            .expect("Failed to create log file");
        
        let file_layer = fmt::layer()
            .with_writer(file)
            .with_ansi(false);
        
        subscriber.with(file_layer).init();
    } else {
        subscriber.init();
    }
}

// Convenience macros for component-specific logging
#[macro_export]
macro_rules!  ppu_trace {
    ($($arg:tt)*) => {
        tracing::trace!(target: "ppu", $($arg)*)
    };
}

#[macro_export]
macro_rules! spu_trace {
    ($($arg:tt)*) => {
        tracing::trace!(target: "spu", $($arg)*)
    };
}

#[macro_export]
macro_rules! rsx_trace {
    ($($arg: tt)*) => {
        tracing:: trace!(target: "rsx", $($arg)*)
    };
}

#[macro_export]
macro_rules! kernel_trace {
    ($($arg:tt)*) => {
        tracing::trace!(target:  "kernel", $($arg)*)
    };
}
```

---

## Phase 2: Memory Management

### Goals
- Virtual memory system mimicking PS3 address space
- Reservation system for SPU atomics
- Memory-mapped I/O regions

### PS3 Memory Map

```
┌──────────────────────────────────────────────────────────────┐
│                    PS3 Memory Map (32-bit EA)                │
├──────────────────────────────────────────────────────────────┤
│ 0x00000000 - 0x0FFFFFFF │ Main Memory (256 MB)              │
│ 0x10000000 - 0x1FFFFFFF │ Main Memory Mirror                │
│ 0x20000000 - 0x2FFFFFFF │ User Memory (applications)        │
│ 0x30000000 - 0x3FFFFFFF │ RSX Mapped Memory                 │
│ 0x40000000 - 0x4FFFFFFF │ RSX I/O (control registers)       │
│ 0xC0000000 - 0xCFFFFFFF │ RSX Local Memory (256 MB VRAM)    │
│ 0xD0000000 - 0xDFFFFFFF │ Stack area                        │
│ 0xE0000000 - 0xEFFFFFFF │ SPU Local Storage mappings        │
│ 0xF0000000 - 0xFFFFFFFF │ Hypervisor / System               │
└──────────────────────────────────────────────────────────────┘
```

### Tasks

#### 2.1 Memory Manager (oc-memory/src/manager.rs)

```rust
use std::sync:: atomic::{AtomicU64, Ordering};
use std::sync:: Arc;
use parking_lot::RwLock;
use crate::error: :{MemoryError, Result};

/// PS3 memory constants
pub mod constants {
    pub const MAIN_MEM_BASE: u32 = 0x0000_0000;
    pub const MAIN_MEM_SIZE: u32 = 0x1000_0000; // 256 MB
    
    pub const USER_MEM_BASE: u32 = 0x2000_0000;
    pub const USER_MEM_SIZE:  u32 = 0x1000_0000; // 256 MB
    
    pub const RSX_MAP_BASE: u32 = 0x3000_0000;
    pub const RSX_MAP_SIZE: u32 = 0x1000_0000;
    
    pub const RSX_IO_BASE: u32 = 0x4000_0000;
    pub const RSX_IO_SIZE: u32 = 0x0010_0000;
    
    pub const RSX_MEM_BASE: u32 = 0xC000_0000;
    pub const RSX_MEM_SIZE: u32 = 0x1000_0000; // 256 MB
    
    pub const STACK_BASE: u32 = 0xD000_0000;
    pub const STACK_SIZE:  u32 = 0x1000_0000;
    
    pub const SPU_BASE: u32 = 0xE000_0000;
    pub const SPU_LS_SIZE: u32 = 0x0004_0000; // 256 KB per SPU
    
    pub const PAGE_SIZE: u32 = 0x1000; // 4 KB
    pub const LARGE_PAGE_SIZE:  u32 = 0x10_0000; // 1 MB
    
    pub const RESERVATION_GRANULARITY: u32 = 128;
}

use constants::*;

bitflags:: bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct PageFlags: u32 {
        const READ    = 0b0001;
        const WRITE   = 0b0010;
        const EXECUTE = 0b0100;
        const MMIO    = 0b1000;
        
        const RW  = Self::READ.bits() | Self::WRITE.bits();
        const RWX = Self::READ.bits() | Self::WRITE.bits() | Self::EXECUTE.bits();
        const RX  = Self::READ.bits() | Self::EXECUTE.bits();
    }
}

/// Reservation for SPU atomic operations
#[repr(C, align(64))]
pub struct Reservation {
    /// Timestamp (version counter) with lock bit in LSB
    pub timestamp: AtomicU64,
}

impl Reservation {
    pub const LOCK_BIT: u64 = 1;
    
    pub fn new() -> Self {
        Self {
            timestamp: AtomicU64:: new(0),
        }
    }
    
    pub fn acquire(&self) -> u64 {
        self.timestamp.load(Ordering:: Acquire) & ! Self::LOCK_BIT
    }
    
    pub fn try_lock(&self, expected_time: u64) -> bool {
        self.timestamp
            .compare_exchange(
                expected_time,
                expected_time | Self::LOCK_BIT,
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .is_ok()
    }
    
    pub fn unlock_and_increment(&self) {
        let current = self.timestamp. load(Ordering:: Relaxed);
        let new_time = (current & !Self::LOCK_BIT) + 128; // Increment by cache line size
        self.timestamp.store(new_time, Ordering::Release);
    }
}

/// Memory region descriptor
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    pub base: u32,
    pub size:  u32,
    pub flags: PageFlags,
    pub name: &'static str,
}

/// Main memory manager
pub struct MemoryManager {
    /// Base pointer for the entire address space
    base:  *mut u8,
    
    /// Allocation bitmap (one bit per page)
    allocation_map: RwLock<Vec<u64>>,
    
    /// Page flags
    page_flags: RwLock<Vec<PageFlags>>,
    
    /// Reservation array (one per 128-byte cache line)
    reservations: Box<[Reservation]>,
    
    /// Memory regions
    regions: Vec<MemoryRegion>,
    
    /// RSX memory (separate allocation for VRAM)
    rsx_mem:  *mut u8,
}

// Safety: Memory is accessed through atomic operations and proper synchronization
unsafe impl Send for MemoryManager {}
unsafe impl Sync for MemoryManager {}

impl MemoryManager {
    pub fn new() -> Result<Self> {
        // Allocate 4 GB address space (32-bit)
        let base = Self::allocate_address_space(0x1_0000_0000)?;
        
        // Allocate RSX memory separately
        let rsx_mem = Self:: allocate_address_space(RSX_MEM_SIZE as usize)?;
        
        // Create page tracking (4 GB / 4 KB pages = 1M pages, 64 bits per u64 = 16K u64s)
        let num_pages = 0x1_0000_0000usize / PAGE_SIZE as usize;
        let allocation_map = RwLock::new(vec![0u64; num_pages / 64]);
        let page_flags = RwLock::new(vec![PageFlags::empty(); num_pages]);
        
        // Create reservations (4 GB / 128 bytes = 32M reservations)
        let num_reservations = 0x1_0000_0000usize / RESERVATION_GRANULARITY as usize;
        let reservations = (0.. num_reservations)
            .map(|_| Reservation::new())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        
        let regions = vec![
            MemoryRegion {
                base:  MAIN_MEM_BASE,
                size: MAIN_MEM_SIZE,
                flags: PageFlags::RWX,
                name: "Main Memory",
            },
            MemoryRegion {
                base:  USER_MEM_BASE,
                size: USER_MEM_SIZE,
                flags: PageFlags:: RWX,
                name: "User Memory",
            },
            MemoryRegion {
                base: RSX_IO_BASE,
                size: RSX_IO_SIZE,
                flags: PageFlags::RW | PageFlags::MMIO,
                name: "RSX I/O",
            },
            MemoryRegion {
                base:  STACK_BASE,
                size: STACK_SIZE,
                flags: PageFlags:: RW,
                name: "Stack",
            },
        ];
        
        let mut manager = Self {
            base,
            allocation_map,
            page_flags,
            reservations,
            regions,
            rsx_mem,
        };
        
        // Initialize standard regions
        manager.init_regions()?;
        
        Ok(manager)
    }
    
    fn allocate_address_space(size: usize) -> Result<*mut u8> {
        #[cfg(unix)]
        {
            use libc: :{mmap, MAP_ANONYMOUS, MAP_PRIVATE, PROT_READ, PROT_WRITE};
            
            let ptr = unsafe {
                mmap(
                    std::ptr::null_mut(),
                    size,
                    PROT_READ | PROT_WRITE,
                    MAP_PRIVATE | MAP_ANONYMOUS,
                    -1,
                    0,
                )
            };
            
            if ptr == libc::MAP_FAILED {
                return Err(MemoryError::OutOfMemory. into());
            }
            
            Ok(ptr as *mut u8)
        }
        
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Memory::*;
            
            let ptr = unsafe {
                VirtualAlloc(
                    std::ptr::null(),
                    size,
                    MEM_RESERVE | MEM_COMMIT,
                    PAGE_READWRITE,
                )
            };
            
            if ptr. is_null() {
                return Err(MemoryError::OutOfMemory.into());
            }
            
            Ok(ptr as *mut u8)
        }
    }
    
    fn init_regions(&mut self) -> Result<()> {
        // Commit main memory
        self. commit_region(MAIN_MEM_BASE, MAIN_MEM_SIZE, PageFlags::RWX)?;
        
        // Commit user memory
        self. commit_region(USER_MEM_BASE, USER_MEM_SIZE, PageFlags::RWX)?;
        
        // Commit stack
        self.commit_region(STACK_BASE, STACK_SIZE, PageFlags::RW)?;
        
        Ok(())
    }
    
    fn commit_region(&mut self, addr: u32, size: u32, flags: PageFlags) -> Result<()> {
        let start_page = (addr / PAGE_SIZE) as usize;
        let num_pages = (size / PAGE_SIZE) as usize;
        
        let mut page_flags = self. page_flags.write();
        
        for i in start_page..start_page + num_pages {
            page_flags[i] = flags;
        }
        
        Ok(())
    }
    
    /// Get raw pointer for address (unchecked, for hot paths)
    #[inline(always)]
    pub unsafe fn ptr(&self, addr: u32) -> *mut u8 {
        self.base.add(addr as usize)
    }
    
    /// Get pointer with bounds and permission checking
    pub fn get_ptr(&self, addr: u32, size: u32, flags: PageFlags) -> Result<*mut u8> {
        self.check_access(addr, size, flags)?;
        Ok(unsafe { self.ptr(addr) })
    }
    
    /// Check if memory access is valid
    pub fn check_access(&self, addr: u32, size: u32, required:  PageFlags) -> Result<()> {
        let start_page = (addr / PAGE_SIZE) as usize;
        let end_page = ((addr + size - 1) / PAGE_SIZE) as usize;
        
        let page_flags = self. page_flags.read();
        
        for page in start_page..=end_page {
            if page >= page_flags.len() {
                return Err(MemoryError::InvalidAddress(addr).into());
            }
            
            if ! page_flags[page]. contains(required) {
                return Err(MemoryError::AccessViolation {
                    addr,
                    kind: if required. contains(PageFlags:: WRITE) {
                        crate::error::AccessKind::Write
                    } else if required.contains(PageFlags:: EXECUTE) {
                        crate::error::AccessKind:: Execute
                    } else {
                        crate::error:: AccessKind::Read
                    },
                }.into());
            }
        }
        
        Ok(())
    }
    
    /// Read from memory
    #[inline]
    pub fn read<T:  Copy>(&self, addr: u32) -> Result<T> {
        self. check_access(addr, std::mem::size_of::<T>() as u32, PageFlags::READ)?;
        Ok(unsafe { self.read_unchecked(addr) })
    }
    
    /// Read without checking (for hot paths after validation)
    #[inline(always)]
    pub unsafe fn read_unchecked<T: Copy>(&self, addr: u32) -> T {
        std::ptr::read_unaligned(self.ptr(addr) as *const T)
    }
    
    /// Write to memory
    #[inline]
    pub fn write<T:  Copy>(&self, addr: u32, value: T) -> Result<()> {
        self. check_access(addr, std::mem:: size_of:: <T>() as u32, PageFlags:: WRITE)?;
        unsafe { self.write_unchecked(addr, value) };
        Ok(())
    }
    
    /// Write without checking (for hot paths after validation)
    #[inline(always)]
    pub unsafe fn write_unchecked<T: Copy>(&self, addr: u32, value: T) {
        std::ptr::write_unaligned(self.ptr(addr) as *mut T, value);
    }
    
    /// Get reservation for address
    #[inline(always)]
    pub fn reservation(&self, addr:  u32) -> &Reservation {
        let index = (addr / RESERVATION_GRANULARITY) as usize;
        &self.reservations[index]
    }
    
    /// Allocate memory
    pub fn allocate(&self, size:  u32, align: u32, flags: PageFlags) -> Result<u32> {
        let aligned_size = (size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        let num_pages = aligned_size / PAGE_SIZE;
        
        let mut allocation_map = self. allocation_map.write();
        let mut page_flags = self.page_flags.write();
        
        // Find contiguous free pages in user memory region
        let start_page = (USER_MEM_BASE / PAGE_SIZE) as usize;
        let end_page = ((USER_MEM_BASE + USER_MEM_SIZE) / PAGE_SIZE) as usize;
        
        let mut found_start = None;
        let mut consecutive = 0u32;
        
        for page in start_page..end_page {
            let word_idx = page / 64;
            let bit_idx = page % 64;
            
            if allocation_map[word_idx] & (1u64 << bit_idx) == 0 {
                if consecutive == 0 {
                    found_start = Some(page);
                }
                consecutive += 1;
                
                if consecutive >= num_pages {
                    break;
                }
            } else {
                consecutive = 0;
                found_start = None;
            }
        }
        
        let start_page = found_start.ok_or(MemoryError::OutOfMemory)?;
        
        if consecutive < num_pages {
            return Err(MemoryError::OutOfMemory. into());
        }
        
        // Mark pages as allocated
        for page in start_page..start_page + num_pages as usize {
            let word_idx = page / 64;
            let bit_idx = page % 64;
            allocation_map[word_idx] |= 1u64 << bit_idx;
            page_flags[page] = flags;
        }
        
        Ok((start_page as u32) * PAGE_SIZE)
    }
    
    /// Free memory
    pub fn free(&self, addr:  u32, size: u32) -> Result<()> {
        let start_page = (addr / PAGE_SIZE) as usize;
        let num_pages = ((size + PAGE_SIZE - 1) / PAGE_SIZE) as usize;
        
        let mut allocation_map = self. allocation_map.write();
        let mut page_flags = self.page_flags. write();
        
        for page in start_page..start_page + num_pages {
            let word_idx = page / 64;
            let bit_idx = page % 64;
            allocation_map[word_idx] &= !(1u64 << bit_idx);
            page_flags[page] = PageFlags::empty();
        }
        
        Ok(())
    }
    
    /// Get RSX memory pointer
    pub fn rsx_ptr(&self, offset: u32) -> *mut u8 {
        unsafe { self.rsx_mem.add(offset as usize) }
    }
    
    /// Copy data to memory
    pub fn write_bytes(&self, addr: u32, data: &[u8]) -> Result<()> {
        self.check_access(addr, data. len() as u32, PageFlags::WRITE)?;
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), self.ptr(addr), data.len());
        }
        Ok(())
    }
    
    /// Copy data from memory
    pub fn read_bytes(&self, addr: u32, size: u32) -> Result<Vec<u8>> {
        self.check_access(addr, size, PageFlags::READ)?;
        let mut data = vec![0u8; size as usize];
        unsafe {
            std::ptr:: copy_nonoverlapping(self.ptr(addr), data.as_mut_ptr(), size as usize);
        }
        Ok(data)
    }
}

impl Drop for MemoryManager {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            libc::munmap(self.base as *mut libc::c_void, 0x1_0000_0000);
            libc::munmap(self.rsx_mem as *mut libc::c_void, RSX_MEM_SIZE as usize);
        }
        
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::System::Memory::*;
            VirtualFree(self.base as *mut _, 0, MEM_RELEASE);
            VirtualFree(self.rsx_mem as *mut _, 0, MEM_RELEASE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_memory_allocation() {
        let mem = MemoryManager::new().unwrap();
        
        let addr = mem.allocate(0x10000, 0x1000, PageFlags:: RW).unwrap();
        assert!(addr >= USER_MEM_BASE);
        assert!(addr < USER_MEM_BASE + USER_MEM_SIZE);
        
        mem.free(addr, 0x10000).unwrap();
    }
    
    #[test]
    fn test_read_write() {
        let mem = MemoryManager::new().unwrap();
        
        let addr = mem.allocate(0x1000, 0x1000, PageFlags:: RW).unwrap();
        
        mem.write::<u32>(addr, 0x12345678).unwrap();
        assert_eq!(mem.read::<u32>(addr).unwrap(), 0x12345678);
        
        mem.write::<u64>(addr + 4, 0xDEADBEEFCAFEBABE).unwrap();
        assert_eq!(mem.read:: <u64>(addr + 4).unwrap(), 0xDEADBEEFCAFEBABE);
    }
    
    #[test]
    fn test_reservation() {
        let mem = MemoryManager::new().unwrap();
        
        let addr = 0x1000u32;
        let res = mem.reservation(addr);
        
        let time = res.acquire();
        assert!(res.try_lock(time));
        res.unlock_and_increment();
        
        let new_time = res.acquire();
        assert_eq!(new_time, time + 128);
    }
}
```

---

## Phase 3: PPU Emulation

### Goals
- Complete PowerPC 970 instruction set
- VMX/AltiVec SIMD support
- Interpreter for debugging
- JIT via C++ (later phase)

### PPU Registers

| Register Set | Count | Size | Description |
|--------------|-------|------|-------------|
| GPR | 32 | 64-bit | General Purpose Registers |
| FPR | 32 | 64-bit | Floating Point Registers |
| VR | 32 | 128-bit | Vector Registers (VMX) |
| CR | 1 | 32-bit | Condition Register (8 x 4-bit fields) |
| LR | 1 | 64-bit | Link Register |
| CTR | 1 | 64-bit | Count Register |
| XER | 1 | 64-bit | Fixed-Point Exception Register |
| FPSCR | 1 | 64-bit | FP Status and Control Register |
| VSCR | 1 | 32-bit | Vector Status and Control Register |

### Tasks

#### 3.1 PPU Thread State (
