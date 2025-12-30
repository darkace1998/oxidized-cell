# 📋 Oxidized-Cell TODO List

This document tracks the development progress and remaining tasks for the oxidized-cell PS3 emulator.

> **Note**: This document reflects the current state of the codebase as analyzed from the source files and TODO comments.

---

## 📊 Project Overview

**Oxidized-Cell** is a hybrid Rust/C++ PlayStation 3 emulator that aims to accurately emulate the Cell Broadband Engine and RSX graphics processor.

### Architecture
```
oxidized-cell/
├── crates/                    # 15 Rust crates
│   ├── oc-core/              # Core emulator, config, logging, bridges
│   ├── oc-memory/            # Memory management (4GB virtual, 4KB pages)
│   ├── oc-ppu/               # PPU interpreter & decoder + JIT support
│   ├── oc-spu/               # SPU interpreter & decoder  
│   ├── oc-rsx/               # RSX graphics & Vulkan backend
│   ├── oc-lv2/               # LV2 kernel syscalls
│   ├── oc-audio/             # Audio system (cpal backend)
│   ├── oc-input/             # Input handling (controller, keyboard, mouse)
│   ├── oc-vfs/               # Virtual file system (ISO, PKG, saves)
│   ├── oc-hle/               # HLE modules (25+ modules)
│   ├── oc-loader/            # ELF/SELF/PRX/PKG loader
│   ├── oc-ffi/               # Rust/C++ FFI bridge
│   ├── oc-ui/                # egui user interface
│   ├── oc-integration/       # EmulatorRunner integration
│   └── oc-debug/             # Debugging tools
├── cpp/                       # C++ performance components
│   ├── ppu_jit.cpp           # PPU JIT (LLVM)
│   ├── spu_jit.cpp           # SPU JIT (LLVM)
│   ├── rsx_shaders.cpp       # SPIR-V shader compilation
│   ├── atomics.cpp           # 128-bit atomics
│   └── simd_avx.cpp          # AVX SIMD helpers
└── docs/                      # Documentation
```

---

## 🎯 High Priority Tasks

### Memory Operations Integration
The HLE modules have their logic implemented but many still need proper memory integration:
- [ ] Write actual values to PS3 memory addresses in HLE function calls
- [ ] Read parameters from PS3 memory in HLE functions
- [ ] Implement proper global manager instances for HLE modules

### Game Loading Pipeline
- [ ] Complete ELF segment loading to memory
- [ ] Finish PRX module dependency resolution and linking
- [ ] Implement SELF file decryption using firmware keys
- [ ] Complete NID (Native ID) resolution for HLE function hooking

### Core Emulation Integration
- [ ] Use dedicated thread ID counter instead of thread count (runner.rs:300)
- [ ] Complete RSX FIFO command processing from GCM command buffer
- [ ] Implement frame pacing and VSync synchronization
- [ ] Implement actual framebuffer readback using staging buffer (vulkan.rs:2193)

---

## 🔧 Medium Priority Tasks

### PPU Emulation (`oc-ppu`)
- [ ] Track actual rounding during FP operations instead of checking fractional part (float.rs:274)
- [ ] Complete VMX/AltiVec instruction edge cases
- [ ] Implement FPSCR exception handling
- [ ] Implement performance counters

### SPU Emulation (`oc-spu`)
- [ ] Complete atomic operations (GETLLAR, PUTLLC, PUTLLUC)
- [ ] Implement all MFC DMA operations
- [ ] Complete channel communication with PPU
- [ ] Add SPU interrupt handling

### RSX Graphics (`oc-rsx`)
- [ ] Implement vertex buffer submission to backend (thread.rs:360)
- [ ] Implement actual Vulkan rendering pipeline integration (postprocess.rs:209)
- [ ] Complete texture format conversions (DXT1, DXT3, DXT5)
- [ ] Implement render target format handling
- [ ] Add multi-sample anti-aliasing support

### Memory Management (`oc-memory`)
- [ ] Implement memory reservation for atomic operations
- [ ] Add page protection tracking
- [ ] Implement memory-mapped I/O regions
- [ ] Add main memory to RSX DMA support

### LV2 Kernel (`oc-lv2`)
- [ ] Complete thread priority inheritance
- [ ] Implement event queue multiplexing
- [ ] Add timer interrupt handling
- [ ] Implement PPU/SPU synchronization primitives

### Audio (`oc-audio`)
- [ ] Implement AAC decoding (codec.rs:228)
- [ ] Implement AT3 (ATRAC3+) decoding (codec.rs:275)
- [ ] Reset AAC/AT3 decoder state properly

---

## 📦 HLE Module Status

> **Legend**: 🟢 Complete | 🟡 Needs Memory Integration | 🔴 Stub

### Graphics Modules
| Module | Status | Remaining Work |
|--------|--------|----------------|
| cellGcmSys | 🟢 | RSX bridge connected |
| cellGifDec | 🟡 | Need actual GIF header parsing for real dimensions |
| cellPngDec | 🟢 | Fully implemented with zlib decompression |
| cellJpgDec | 🟢 | JPEG marker parsing complete |
| cellResc | 🟡 | Perform actual scaling/flip through RSX backend |

### System Modules
| Module | Status | Remaining Work |
|--------|--------|----------------|
| cellSysutil | 🟡 | Write trophy info, percentages, configs to memory |
| cellGame | 🟡 | Check game data existence, calculate content size |
| cellSaveData | 🟡 | Create/delete directories in VFS |

### Input Modules
| Module | Status | Remaining Work |
|--------|--------|----------------|
| cellPad | 🟡 | Connect to oc-input, write info/data to memory |
| cellKb | 🟡 | Write info/data to memory, clear input buffer |
| cellMouse | 🟡 | Get actual mouse data from oc-input |
| cellMic | 🟡 | Start actual audio capture, read captured data |

### Audio/Video Modules
| Module | Status | Remaining Work |
|--------|--------|----------------|
| cellAudio | 🟡 | Read params from memory, create event queue via kernel |
| cellDmux | 🟡 | Actual PAMF/MPEG-PS/MPEG-TS parsing |
| cellVdec | 🟡 | Actual H.264/MPEG-2 decoding |
| cellAdec | 🟡 | Actual AAC/MP3/ATRAC3+ decoding |
| cellVpost | 🟡 | Actual YUV/RGB conversion |

### Network Modules
| Module | Status | Remaining Work |
|--------|--------|----------------|
| cellNetCtl | 🟡 | Write state/info to memory |
| cellHttp | 🟡 | Integrate with actual HTTP networking |
| cellSsl | 🟢 | Certificate management complete |

### Utility Modules
| Module | Status | Remaining Work |
|--------|--------|----------------|
| cellSpurs | 🟡 | Actually attach/detach event queues |
| cellSpursJq | 🟡 | Actually wait for job completion |
| cellFont | 🟡 | Font cache allocation, glyph rendering |
| cellFontFt | 🟡 | Write face handle to memory |
| libsre | 🟢 | Regex via Rust regex crate |

### File System
| Module | Status | Remaining Work |
|--------|--------|----------------|
| cellFs | 🟡 | Read path/fd from memory, queue actual async I/O |

### VFS
| Module | Status | Remaining Work |
|--------|--------|----------------|
| savedata | 🟡 | Proper PARAM.SFO format generation/parsing |
| pkg | 🟡 | PKG extraction logic |
| disc | 🟡 | Parse PARAM.SFO for title and game ID |

---

## 🧪 Testing Tasks

### Current Test Coverage
- Memory: 128+ tests
- PPU: 75+ tests  
- SPU: 14+ tests
- RSX: 36+ tests
- HLE: 483+ tests
- Integration: 4+ tests

### Additional Tests Needed
- [ ] Add more PPU instruction edge case tests
- [ ] Add SPU channel communication tests
- [ ] Add RSX state management tests
- [ ] Add HLE memory integration tests

### Integration Tests
- [ ] Test game loading from ISO/folder
- [ ] Test SELF decryption with firmware
- [ ] Test PRX module loading chain
- [ ] Test graphics output (simple rendering tests)

### Homebrew Testing
- [ ] Test with PSL1GHT SDK examples
- [ ] Test with open-source PS3 homebrew
- [ ] Create test suite with simple graphics demos

---

## 🖥️ UI Tasks (`oc-ui`)

Current UI features implemented:
- ✅ Game list view
- ✅ Emulation view with framebuffer display
- ✅ Debugger view
- ✅ Log viewer
- ✅ Memory viewer
- ✅ Shader debugger
- ✅ Controller configuration
- ✅ Settings panel
- ✅ Theme support

Remaining tasks:
- [ ] Implement game library scanning and caching
- [ ] Add game cover art display
- [ ] Add performance overlay (FPS, CPU usage)
- [ ] Add memory viewer hex editing

---

## 🔌 JIT Compilation (C++)

### PPU JIT (`cpp/src/ppu_jit.cpp`)
Current features:
- ✅ Basic block compilation with LLVM IR
- ✅ Code cache (64MB)
- ✅ Multi-threaded compilation support

Remaining:
- [ ] Implement remaining integer instructions
- [ ] Add floating-point JIT compilation
- [ ] Implement branch and link optimization
- [ ] Add VMX instruction JIT support

### SPU JIT (`cpp/src/spu_jit.cpp`)
Current features:
- ✅ Basic block compilation with LLVM IR
- ✅ Code cache (64MB)

Remaining:
- [ ] Implement remaining SIMD instructions
- [ ] Add shuffle/permute instruction JIT
- [ ] Implement channel operation JIT
- [ ] Add DMA operation JIT support

### RSX Shaders (`cpp/src/rsx_shaders.cpp`)
Current features:
- ✅ Vertex/fragment program opcode definitions
- ✅ Pipeline state caching

Remaining:
- [ ] Complete vertex program to SPIR-V translation
- [ ] Complete fragment program to SPIR-V translation
- [ ] Implement shader caching
- [ ] Add shader hot-reloading for debugging

---

## 📚 Documentation

Current documentation:
- ✅ README.md - Project overview and building instructions
- ✅ docs/ppu_instructions.md - PPU instruction reference
- ✅ docs/spu_instructions.md - SPU instruction reference
- ✅ docs/HLE_STATUS.md - HLE module status
- ✅ docs/USER_MANUAL.md - User manual
- ✅ docs/jit-compilation.md - JIT documentation
- ✅ docs/phase2-memory-management.md - Memory management docs
- ✅ docs/advanced-ppu-instructions.md - Advanced PPU docs

Additional documentation needed:
- [ ] Architecture overview document
- [ ] Game compatibility list
- [ ] Debugging guide
- [ ] Contributor guidelines

---

## 🔒 Security & Quality

- [ ] Audit memory access bounds checking
- [ ] Review SELF decryption key handling
- [ ] Add fuzzing tests for loader components
- [ ] Implement safe file path handling in VFS

---

## 🚀 Performance Optimizations

- [ ] Profile PPU interpreter hot paths
- [ ] Optimize SPU DMA transfers
- [ ] Implement RSX command batching
- [ ] Add multi-threaded PPU scheduling
- [ ] Optimize memory page fault handling

---

## 📅 Milestones

### v0.1.0 - Foundation ✅
- [x] Core architecture in place
- [x] PPU interpreter functional
- [x] SPU interpreter functional
- [x] Basic HLE framework (25+ modules)
- [x] egui-based UI
- [x] RSX Vulkan backend basics
- [x] Audio system with cpal
- [x] Input system with controller/keyboard/mouse support
- [ ] First homebrew running

### v0.2.0 - Graphics
- [ ] Basic RSX rendering working
- [ ] Simple 2D games running
- [ ] Audio output functional
- [ ] Complete HLE memory integration

### v0.3.0 - Compatibility
- [ ] Commercial game loading
- [ ] SELF decryption working
- [ ] Multiple games partially running

### v1.0.0 - Playable
- [ ] Multiple commercial games fully playable
- [ ] JIT compilation for acceptable performance
- [ ] Stable and well-tested

---

## 📝 Development Tips

### Building
```bash
# Build release
cargo build --release

# Run tests
cargo test

# Run specific crate tests
cargo test -p oc-memory
cargo test -p oc-ppu
cargo test -p oc-hle
```

### Debugging
```bash
# Enable detailed logging
RUST_LOG=debug cargo run --release

# Run with a specific game
cargo run --release -- /path/to/game.elf
```

### Code Style
- Rust: Follow `rustfmt` and `clippy` conventions
- C++: Use clang-format with project settings
- Write tests for new functionality
- Document public APIs

### Key Resources
- [PS3 Developer Wiki](https://www.psdevwiki.com/)
- [RPCS3 Source](https://github.com/RPCS3/rpcs3) - Reference implementation
- [Cell BE Programming Handbook](https://www.ibm.com/support/pages/cell-be-programming-handbook)

---

*Last updated: December 2024*
