# Oxidized-Cell PS3 Emulator - TODO

This document outlines the development roadmap for the Oxidized-Cell PS3 emulator. The project is organized into multiple phases, with each crate handling a specific aspect of PS3 emulation.

---

## Project Overview

**Current Status:** Active Development (v0.1.0)

**Architecture:**
- Written in Rust with C++ components for LLVM JIT
- Modular crate-based architecture (15 workspace crates)
- Cross-platform support (Linux, macOS, Windows)
- GPL-3.0 licensed

---

## ✅ Completed Features

### Core Infrastructure (`oc-core`)
- [x] Configuration system with TOML support
- [x] Emulator state management
- [x] Logging infrastructure with tracing
- [x] Thread scheduler
- [x] Error handling framework

### Memory Management (`oc-memory`)
- [x] 4KB page-based memory management
- [x] 32-bit address space (4GB) emulation
- [x] Memory protection flags (Read/Write/Execute)
- [x] 128-byte atomic reservation system
- [x] PS3 memory region mapping:
  - [x] Main Memory (256 MB @ 0x00000000)
  - [x] User Memory (256 MB @ 0x20000000)
  - [x] RSX I/O (1 MB @ 0x40000000)
  - [x] RSX Memory (256 MB @ 0xC0000000)
  - [x] Stack Area (256 MB @ 0xD0000000)
  - [x] SPU Local Storage regions
- [x] Big-endian memory operations
- [x] Comprehensive test suite (unit, integration, and stress tests)

### PPU Emulation (`oc-ppu`)
- [x] PowerPC 64-bit interpreter
- [x] 32 GPRs (64-bit)
- [x] 32 FPRs (64-bit)
- [x] 32 Vector Registers (128-bit AltiVec/VMX)
- [x] Special Purpose Registers (PC, LR, CTR, XER, CR, FPSCR)
- [x] Instruction decoder (all PowerPC formats)
- [x] Branch instructions (b, bl, bc, bclr, bcctr)
- [x] Integer arithmetic (add, sub, mul, div, shifts, rotates)
- [x] Logical operations (and, or, xor, nand, nor)
- [x] Load/Store instructions (byte, halfword, word, doubleword)
- [x] Atomic operations (lwarx/stwcx, ldarx/stdcx)
- [x] Floating-point arithmetic (add, sub, mul, div, sqrt, FMA)
- [x] VMX/AltiVec SIMD operations
- [x] FPSCR exception tracking
- [x] Rounding mode support
- [x] Breakpoint support (conditional and unconditional)
- [x] Comprehensive unit tests

### SPU Emulation (`oc-spu`)
- [x] SPU interpreter with 128 × 128-bit registers
- [x] 256 KB local storage per SPU
- [x] Instruction decoder (all SPU formats)
- [x] Branch instructions (14 types)
- [x] Logical instructions (18 types)
- [x] Arithmetic operations (multiply, add, subtract, shift, rotate)
- [x] Memory instructions (load/store quadword)
- [x] Compare instructions
- [x] Floating-point instructions
- [x] Channel instructions (rdch, wrch, rchcnt)
- [x] Shuffle/permute operations (shufb, selb)
- [x] Memory Flow Controller (MFC) with DMA
- [x] Atomic operations (GETLLAR/PUTLLC)
- [x] Mailbox communication (SPU ↔ PPU)
- [x] Signal notification channels
- [x] DMA tag management
- [x] 66+ tests (52 unit + 14 integration)

### Loader (`oc-loader`)
- [x] ELF file parsing
- [x] SELF (Signed ELF) file parsing
- [x] PRX (Loadable module) support
- [x] Basic decryption framework

### LV2 Kernel (`oc-lv2`)
- [x] Syscall dispatcher
- [x] Process management structures
- [x] Thread management
- [x] Memory syscalls
- [x] File system syscalls (fs.rs)
- [x] PRX module loading
- [x] SPU thread management
- [x] Synchronization primitives (mutex, semaphore, cond, rwlock, event_flag, lwmutex)
- [x] Timer infrastructure

### HLE Modules (`oc-hle`)
- [x] Module framework
- [x] cellAudio - Audio output
- [x] cellFs - File system access
- [x] cellGame - Game data access
- [x] cellGcmSys - RSX control
- [x] cellKb - Keyboard input
- [x] cellMouse - Mouse input
- [x] cellPad - Controller input
- [x] cellPngDec - PNG decoder
- [x] cellJpgDec - JPEG decoder
- [x] cellGifDec - GIF decoder
- [x] cellSaveData - Save data management
- [x] cellSysutil - System utilities
- [x] cellSpurs - SPU task scheduler
- [x] cellSpursJq - SPURS job queue
- [x] cellAdec - Audio decoder
- [x] cellDmux - Demuxer
- [x] cellVdec - Video decoder
- [x] cellVpost - Video post-processing
- [x] cellHttp - HTTP client
- [x] cellSsl - SSL support
- [x] cellNetCtl - Network control
- [x] cellMic - Microphone
- [x] cellFont/cellFontFT - Font rendering
- [x] cellResc - Resolution scaler
- [x] libsre - SPU runtime environment

### Virtual File System (`oc-vfs`)
- [x] Mount point management
- [x] Device abstraction
- [x] Disc image support
- [x] Save data handling
- [x] Trophy system structures
- [x] User profile management
- [x] Format support infrastructure

### Audio (`oc-audio`)
- [x] Backend abstraction
- [x] cellAudio interface
- [x] Audio mixer
- [x] Audio thread management

### Input (`oc-input`)
- [x] Keyboard input
- [x] Mouse input
- [x] Pad (controller) input
- [x] Input mapping system

### RSX Graphics (`oc-rsx`)
- [x] RSX state management
- [x] FIFO command queue
- [x] Buffer management
- [x] Texture handling
- [x] Shader infrastructure
- [x] Vertex processing
- [x] Backend abstraction (for Vulkan)
- [x] RSX thread

### UI (`oc-ui`)
- [x] Application framework (eframe/egui)
- [x] Game list view
- [x] Settings panel
- [x] Controller configuration
- [x] Log viewer
- [x] Memory viewer
- [x] Debugger panel
- [x] Shader debugger
- [x] Theme support

### Debugging (`oc-debug`)
- [x] PPU debugger
- [x] SPU debugger
- [x] RSX debugger
- [x] Breakpoint system
- [x] Disassembler
- [x] Profiler infrastructure

### FFI/JIT (`oc-ffi`)
- [x] Rust-C++ interop layer
- [x] Atomic operations via C++
- [x] SIMD intrinsics support
- [x] JIT infrastructure
- [x] Type definitions for FFI

### C++ Components (`cpp/`)
- [x] PPU JIT framework (LLVM)
- [x] SPU JIT framework (LLVM)
- [x] DMA operations
- [x] RSX shader compilation
- [x] SIMD AVX implementations
- [x] Atomic operations
- [x] CMake build system
- [x] FFI header (oc_ffi.h)

### Integration (`oc-integration`)
- [x] Component loader
- [x] Pipeline management
- [x] Runner framework

---

## 🔨 In Progress / Partial Implementation

### PPU JIT Compilation
- [ ] Complete all PowerPC instruction coverage in LLVM IR
- [ ] VMX/AltiVec full JIT support
- [ ] All floating-point edge cases
- [ ] Cross-block optimization
- [ ] Profile-guided optimization

### SPU JIT Compilation
- [ ] Complete SPU instruction coverage
- [ ] Full floating-point instruction set
- [ ] All permute/shuffle variants
- [ ] Extended arithmetic operations
- [ ] Hint instructions

### RSX Graphics
- [x] Vulkan backend implementation (enhanced with MSAA, MRT support)
- [ ] Complete shader translation (SPIR-V infrastructure ready)
- [x] Full texture format support (DXT, ETC, ASTC formats)
- [x] Render target management (MRT, RTT)
- [ ] Display output

### HLE Modules
- [ ] Complete cellAudio implementation
- [ ] Full cellVdec support (all codecs)
- [ ] cellSpurs full implementation
- [ ] Network modules (actual network I/O)

---

## 📋 Future Work / Planned Features

### Phase 1: Stability & Compatibility

#### CPU Emulation
- [x] Cycle-accurate timing option
- [x] Pipeline simulation
- [x] Power management emulation
- [x] Full exception model
- [x] Trap instruction implementation

#### Memory System
- [x] Memory watchpoints
- [x] Self-modifying code detection
- [x] Cache simulation mode
- [x] Memory access profiling

#### Loader
- [ ] Complete SELF decryption (partial - infrastructure in place)
- [x] Firmware file support (PUP parsing)
- [x] PSN package extraction (PKG parsing)
- [x] Update package support (PUP parsing)

### Phase 2: Graphics

#### RSX/GPU Emulation
- [x] Complete Vulkan backend (enhanced with MSAA, MRT, RTT support)
- [ ] Full NV4x shader translation (infrastructure in place)
- [x] All texture formats (DXT, ETC, ASTC, and more)
- [x] Render to texture (RTT framebuffer infrastructure)
- [x] Multiple render targets (MRT support up to 4 attachments)
- [x] Anti-aliasing support (MSAA and post-process AA)
- [x] Anisotropic filtering (configurable 1x-16x)
- [x] Frame buffer operations (blit, copy, clear)
- [x] MSAA support (1x, 2x, 4x, 8x, 16x, 32x, 64x)
- [x] Post-processing effects (FXAA, SMAA, TAA, Bloom, DOF, etc.)
- [x] Resolution scaling (integer, aspect-preserved, FSR, xBRZ, etc.)
- [x] Frame pacing (VSync modes, frame limiting, timing stats)

### Phase 3: Audio

#### Audio Emulation
- [x] Complete SPDIF output
- [x] Multi-channel audio (5.1, 7.1)
- [x] Audio resampling
- [x] Time stretching
- [x] All audio codecs (AAC, AT3, etc.)

### Phase 4: Input & Peripherals

#### Controller Support
- [ ] DualShock 3 full emulation
- [ ] Sixaxis motion sensors
- [ ] Vibration feedback
- [ ] USB controller support
- [ ] Bluetooth controller pairing
- [ ] PlayStation Move support
- [ ] Guitar/Drum controllers

#### Other Peripherals
- [ ] Camera support
- [ ] Microphone input
- [ ] USB devices
- [ ] Memory card (if applicable)

### Phase 5: Networking

#### Network Emulation
- [ ] PSN authentication (mock)
- [ ] Online multiplayer support
- [ ] LAN play
- [ ] UPNP support
- [ ] HTTP/HTTPS client
- [ ] SSL/TLS implementation

### Phase 6: System Features

#### OS Emulation
- [ ] XMB (Cross Media Bar) simulation
- [ ] Trophy system
- [ ] Friend list
- [ ] Message system
- [ ] Screenshot capture
- [ ] Video recording

#### Firmware
- [ ] Firmware decryption
- [ ] System software emulation
- [ ] VSH modules

### Phase 7: Performance Optimization

#### CPU Performance
- [ ] Advanced JIT optimizations
- [ ] SIMD host optimizations (AVX-512)
- [ ] Branch prediction optimization
- [ ] Hot path detection
- [ ] Adaptive compilation levels
- [ ] Multi-threaded PPU execution

#### GPU Performance
- [ ] Shader caching
- [ ] Texture caching
- [ ] Command buffer optimization
- [ ] Async compute
- [ ] Multi-threaded rendering

#### General
- [ ] Performance profiler
- [ ] Frame time analysis
- [ ] Bottleneck detection

### Phase 8: User Experience

#### UI Improvements
- [ ] Modern UI redesign
- [ ] Game cover art
- [ ] Game metadata display
- [ ] Save state management
- [ ] Configuration per-game
- [ ] Recent games list
- [ ] Search/filter games
- [ ] Accessibility features

#### Debugging
- [ ] Trace recording/replay
- [ ] Memory dump/load
- [ ] Register state save/load
- [ ] Call stack visualization
- [ ] Performance graphs
- [ ] GPU command stream viewer

### Phase 9: Platform Support

#### Cross-Platform
- [ ] ARM64 support (Apple Silicon, Raspberry Pi)
- [ ] Android port
- [ ] iOS port (jailbroken)
- [ ] Web/WASM port

#### Build System
- [ ] CI/CD pipeline
- [ ] Automated testing
- [ ] Binary releases
- [ ] Package managers (brew, apt, etc.)

### Phase 10: Documentation & Community

#### Documentation
- [ ] User manual
- [ ] Game compatibility list
- [ ] FAQ section
- [ ] Troubleshooting guide
- [ ] Developer documentation
- [ ] API reference
- [ ] Architecture overview
- [ ] Contributing guide

#### Community
- [ ] Discord server
- [ ] Bug tracker integration
- [ ] Feature request system
- [ ] Progress blog/updates

---

## Known Issues

- [ ] JIT compilation incomplete - some instructions fall back to interpreter
- [ ] RSX graphics backend not fully implemented
- [ ] Some HLE modules are stubs
- [ ] Limited game compatibility
- [ ] No firmware decryption (requires legal firmware dump)

---

## Priority Items (Short-term)

1. **Complete RSX Vulkan backend** - Required for any graphical output
2. **Improve PPU JIT coverage** - Performance critical
3. **Implement more HLE functions** - Game compatibility
4. **Add game compatibility database** - Track what works
5. **CI/CD setup** - Automated builds and testing

---

## Testing Checklist

- [ ] Run all existing tests: `cargo test --workspace`
- [ ] Memory tests: `cargo test -p oc-memory`
- [ ] PPU tests: `cargo test -p oc-ppu`
- [ ] SPU tests: `cargo test -p oc-spu`
- [ ] Integration tests: `cargo test -p oc-integration`
- [ ] Build C++ components: `cmake --build build`
- [ ] Benchmark performance: `cargo bench -p oc-memory`

---

## Building

```bash
# Rust components
cargo build --release

# C++ JIT components (requires LLVM 15+)
mkdir build && cd build
cmake ..
cmake --build .

# Run the emulator
cargo run --release
```

---

## Contributing

Key areas needing help:

1. **Graphics** - Vulkan/RSX implementation
2. **HLE** - Game-specific module implementations  
3. **Testing** - Game compatibility testing
4. **Documentation** - User and developer docs
5. **JIT** - LLVM IR generation for more instructions

---

*Last updated: December 2024*
