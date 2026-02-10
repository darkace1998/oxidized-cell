# Oxidized-Cell TODO List

This document tracks development tasks and progress for the oxidized-cell PS3 emulator.

---

## 🔥 High Priority - Game Execution

### HLE Module Implementation
The HLE (High Level Emulation) modules are critical for running games.

- [ ] **cellGcmSys** - RSX graphics command management
  - [ ] Complete command buffer submission
  - [ ] Display buffer management
  - [ ] Flip queue synchronization
  - [ ] Label/semaphore handling

- [ ] **cellSysutil** - System utilities
  - [ ] Callback system for system events
  - [ ] Dialog handling (save, message, etc.)
  - [ ] Game disc detection
  - [ ] System param settings

- [ ] **cellSpurs** - SPU task scheduling
  - [ ] Task workload management
  - [ ] Event flag synchronization
  - [ ] Job queue integration
  - [ ] Taskset management

- [ ] **cellPad** - Controller input
  - [ ] DualShock 3 button/axis mapping
  - [ ] Vibration support
  - [ ] Multiple controller support

- [ ] **cellFs** - File system operations
  - [ ] Virtual file system integration
  - [ ] dev_hdd0 emulation
  - [ ] dev_flash emulation

### Game Loading Pipeline
- [ ] Complete SELF file decryption chain
- [ ] PRX module loading and linking
- [ ] Symbol resolution via NID table
- [ ] Main thread entry point execution
- [ ] Initial system module loading (libc, etc.)

---

## 🖥️ CPU Emulation

### PPU (PowerPC Processing Unit)
- [ ] **Interpreter**
  - [x] Core integer instructions
  - [x] Floating-point instructions
  - [x] VMX/AltiVec SIMD instructions
  - [x] Branch instructions
  - [x] Load/store instructions
  - [ ] Complete system call interface
  - [ ] TLB/MMU emulation (if needed)

- [ ] **JIT Compiler** (C++ with LLVM)
  - [x] Basic block compilation
  - [x] Code caching with LRU eviction
  - [ ] Expand instruction coverage (currently ~20 instructions)
  - [ ] Register allocation optimization
  - [ ] Branch prediction hints
  - [ ] Hot path detection and optimization

### SPU (Synergistic Processing Unit)
- [ ] **Interpreter**
  - [x] Core SIMD instructions
  - [x] Channel communication
  - [x] MFC (Memory Flow Controller) basics
  - [ ] Complete DMA operations
  - [ ] Atomic operations for synchronization
  - [ ] Event/interrupt handling

- [ ] **JIT Compiler** (C++ with LLVM)
  - [x] Basic block compilation
  - [ ] Expand instruction coverage (currently ~15 instructions)
  - [ ] Channel operations in JIT
  - [ ] MFC DMA optimization
  - [ ] Loop optimization for hot SPU code

### SPU Thread Management
- [ ] Thread group management
- [ ] SPU context switching
- [ ] Mailbox communication
- [ ] Signal notification

---

## 🎮 Graphics (RSX)

### Vulkan Backend
- [x] Basic initialization
- [x] Display buffer presentation
- [ ] Complete NV4097 method handling
- [ ] Vertex shader translation
- [ ] Fragment shader translation
- [ ] Texture sampling
- [ ] Multi-render target support
- [ ] Depth/stencil operations
- [ ] Blending modes

### Shader System
- [ ] **Vertex Shaders**
  - [ ] VP instruction decoding
  - [ ] SPIR-V compilation
  - [ ] Constant buffer handling

- [ ] **Fragment Shaders**
  - [ ] FP instruction decoding
  - [ ] SPIR-V compilation
  - [ ] Texture coordinate handling
  - [ ] Alpha testing

### Post-Processing
- [ ] Resolution scaling
- [ ] Anti-aliasing (MSAA/FXAA)
- [ ] Anisotropic filtering
- [ ] Gamma correction

---

## 🔊 Audio

### Audio Backend
- [x] cpal integration for cross-platform output
- [x] 48kHz sample rate support
- [x] Multi-channel output (8 ports)
- [ ] Audio resampling
- [ ] Time stretching for frame rate changes
- [ ] Audio mixing improvements

### Audio Codecs
- [ ] ATRAC3/ATRAC3plus decoding
- [ ] AC3 decoding (partially implemented)
- [ ] AAC decoding
- [ ] MP3 decoding

---

## 💾 System

### LV2 Kernel
- [x] Process management basics
- [x] Thread creation/termination
- [x] Mutex/cond/semaphore primitives
- [ ] Read-write locks
- [ ] Event flags
- [ ] Timer management
- [ ] Memory mapping syscalls
- [ ] File system syscalls

### Memory Management
- [x] 4GB virtual address space
- [x] 4KB page management
- [x] Memory protection
- [ ] Memory-mapped I/O regions
- [ ] RSX memory mapping
- [ ] Shared memory between PPU/SPU

### Virtual File System
- [x] ISO 9660 disc reading
- [x] PKG file extraction
- [x] PARAM.SFO parsing
- [ ] Save data encryption/decryption
- [ ] Trophy system
- [ ] User profiles

---

## 🖼️ User Interface

### Main Window
- [x] egui-based UI
- [x] Game list view
- [ ] Game metadata display (icons, descriptions)
- [ ] Recent games list
- [ ] Search/filter functionality

### Settings
- [x] Configuration file (config.toml)
- [ ] Graphics settings panel
- [ ] Audio settings panel
- [ ] Input mapping configuration
- [ ] Per-game configuration

### Debugger
- [x] Memory viewer basics
- [ ] PPU register display
- [ ] SPU register display
- [ ] Breakpoint management
- [ ] Step execution
- [ ] Disassembly view
- [ ] RSX command trace

---

## 🧪 Testing

### Unit Tests
- [x] Memory manager tests (128+ tests)
- [x] PPU instruction tests (75+ tests)
- [x] SPU instruction tests (14+ tests)
- [x] RSX state tests (36+ tests)
- [ ] HLE module tests
- [ ] Syscall tests

### Integration Tests
- [ ] Boot sequence testing
- [ ] Homebrew application tests
- [ ] Commercial game compatibility tests
- [ ] Performance benchmarks

### Test Infrastructure
- [ ] Automated test suite in CI
- [ ] Performance regression tracking
- [ ] Compatibility database

---

## 📚 Documentation

### User Documentation
- [x] README with installation instructions
- [x] Building instructions for Linux/Windows/macOS
- [ ] Configuration guide
- [ ] Troubleshooting guide
- [ ] FAQ

### Developer Documentation
- [x] PPU instruction reference
- [x] SPU instruction reference
- [x] HLE status document
- [ ] Architecture overview
- [ ] Contributing guide
- [ ] Code style guide
- [ ] API documentation (rustdoc)

---

## 🔧 Build & Infrastructure

### Build System
- [x] Cargo workspace setup
- [x] CMake for C++ components
- [ ] Cross-compilation support
- [ ] Static linking option
- [ ] Release packaging scripts

### CI/CD
- [ ] GitHub Actions workflow
- [ ] Automated builds for all platforms
- [ ] Test suite automation
- [ ] Release artifact generation

---

## 🌟 Future Enhancements

### Performance
- [ ] Parallel compilation threads for JIT
- [ ] SIMD-accelerated interpreters
- [ ] GPU compute for SPU emulation
- [ ] Memory access optimization

### Compatibility
- [ ] Network play support (cellNet modules)
- [ ] PlayStation Network stubs
- [ ] Move controller support
- [ ] 3D stereoscopic rendering

### Debugging
- [ ] GDB remote protocol support
- [ ] Memory watchpoints
- [ ] Performance profiler
- [ ] Trace logging

---

## 📋 Known Issues

1. **Build**: Edition 2024 in root Cargo.toml should be 2021 (matches workspace)
2. **JIT**: LLVM headers may not be available on all systems
3. **Graphics**: Vulkan backend requires SDK 1.2+
4. **Audio**: AC3 codec needs full IMDCT implementation

---

## 📅 Milestones

### Milestone 1: Boot Homebrew
- [ ] Load and execute simple ELF files
- [ ] Basic syscall support
- [ ] Simple graphics output

### Milestone 2: Boot Commercial Games
- [ ] SELF decryption working
- [ ] HLE modules functional
- [ ] Game menu rendering

### Milestone 3: Playable Games
- [ ] Input working correctly
- [ ] Audio output
- [ ] Stable frame rate

---

*Last updated: 2026-02-10*
