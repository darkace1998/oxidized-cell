# 📋 Oxidized-Cell TODO List

**Version**: 0.1.0  
**Last Updated**: December 2024

This document outlines the development tasks, planned features, and known issues for the oxidized-cell PS3 emulator project.

---

## Table of Contents

1. [High Priority Tasks](#high-priority-tasks)
2. [Core Emulation](#core-emulation)
3. [HLE Modules](#hle-modules)
4. [Graphics (RSX)](#graphics-rsx)
5. [Audio System](#audio-system)
6. [Input System](#input-system)
7. [Loader & File System](#loader--file-system)
8. [User Interface](#user-interface)
9. [Debugging Tools](#debugging-tools)
10. [JIT Compilation (C++)](#jit-compilation-c)
11. [Testing & Quality Assurance](#testing--quality-assurance)
12. [Documentation](#documentation)
13. [Build & Infrastructure](#build--infrastructure)
14. [Known Issues](#known-issues)
15. [Future Considerations](#future-considerations)

---

## High Priority Tasks

These tasks are critical for getting basic game execution working.

### 🔴 Critical

- [ ] **Complete HLE Module Implementation** - Most games require these modules to run
  - [ ] `cellGcmSys` - Graphics initialization and command buffer submission
  - [ ] `cellSysutil` - System utilities (callbacks, dialogs)
  - [ ] `cellSpurs` - Task scheduler for SPU workloads
  - [ ] `cellPad` - Controller input handling
  - [ ] `cellFs` - File system operations

- [ ] **Game Loading Pipeline** - Complete the end-to-end loading flow
  - [ ] Implement full SELF/EBOOT.BIN decryption with firmware keys
  - [ ] Complete PRX module linking and NID resolution
  - [ ] Handle relocations and dynamic linking properly

- [ ] **RSX-GCM Integration** - Connect HLE GCM calls to RSX backend
  - [ ] Route command buffer submissions to RSX thread
  - [ ] Implement display buffer flipping
  - [ ] Handle texture and render target configuration

---

## Core Emulation

### PPU (PowerPC Processing Unit)

#### Interpreter
- [x] Basic instruction set implementation (~2,700+ lines)
- [x] Integer arithmetic instructions
- [x] Floating-point instructions with FPSCR handling
- [x] Branch and control flow instructions
- [x] Load/store instructions
- [x] VMX/AltiVec SIMD support (128-bit vectors)
- [x] Breakpoint debugging support
- [ ] Complete full instruction coverage verification
- [ ] Optimize hot paths in interpreter loop
- [ ] Add cycle-accurate timing mode

#### Thread Management
- [x] Basic PPU thread structure
- [ ] Thread scheduling improvements
- [ ] Priority-based scheduling
- [ ] Thread affinity support
- [ ] Proper thread synchronization with LV2 primitives

### SPU (Synergistic Processing Unit)

#### Interpreter
- [x] 128x 128-bit vector register support
- [x] 256KB Local Storage per SPU
- [x] Basic instruction implementation
- [x] MFC (Memory Flow Controller) basics
- [x] Channel communication fundamentals
- [ ] Complete MFC DMA command implementation
- [ ] SPU event handling
- [ ] Mailbox communication improvements
- [ ] Signal notification channels
- [ ] SPU isolation mode

#### Thread Management
- [ ] SPU thread group creation/management
- [ ] SPU thread affinity
- [ ] SPU event queue handling
- [ ] SPU thread exception handling

### Memory System

- [x] 4GB virtual address space
- [x] 4KB page management
- [x] Memory reservation system for atomics
- [x] Cache simulation for debugging
- [x] Watchpoint support
- [ ] Memory protection with proper exception handling
- [ ] Self-modifying code (SMC) detection improvements
- [ ] Memory mapping for RSX local memory
- [ ] Shared memory regions between PPU and SPU

---

## HLE Modules

### Graphics Modules

#### cellGcmSys (High Priority)
- [x] `cellGcmInit` - Initialize graphics system
- [x] `cellGcmSetFlipMode` - Set display flip mode
- [x] `cellGcmSetFlip` - Queue buffer swap
- [x] `cellGcmSetDisplayBuffer` - Configure display buffers
- [x] `cellGcmGetConfiguration` - Get GCM configuration
- [x] `cellGcmAddressToOffset` - Address to RSX offset conversion
- [x] `cellGcmFlush` - Flush command buffer
- [x] `cellGcmFinish` - Wait for RSX completion
- [x] `cellGcmSetTexture` - Set texture
- [x] `cellGcmSetSurface` - Set render target
- [ ] `cellGcmSetVertexProgram` - Set vertex shader
- [ ] `cellGcmSetFragmentProgram` - Set fragment shader
- [ ] `cellGcmSetDrawArrays` - Draw call
- [ ] `cellGcmSetDrawIndexArray` - Indexed draw call
- [ ] `cellGcmSetViewport` - Set viewport
- [ ] `cellGcmSetScissor` - Set scissor rect
- [ ] `cellGcmMapMainMemory` - Map main memory for RSX
- [ ] `cellGcmResetFlipStatus` - Reset flip status

#### cellGifDec
- [ ] GIF image decoding
- [ ] Animation support

#### cellPngDec
- [ ] PNG image decoding
- [ ] Alpha channel support

#### cellJpgDec
- [ ] JPEG image decoding
- [ ] Progressive JPEG support

#### cellResc
- [ ] Resolution scaling
- [ ] Frame buffer management

### System Modules

#### cellSysutil (High Priority)
- [ ] System callback registration
- [ ] Dialog display (message, progress, etc.)
- [ ] Trophy notifications
- [ ] System parameter access
- [ ] Background music control
- [ ] Screen saver control
- [ ] Video settings
- [ ] Audio settings

#### cellGame
- [ ] Game data management
- [ ] Title ID retrieval
- [ ] Game content permissions
- [ ] Patch management
- [ ] DLC handling

#### cellSaveData
- [ ] Save data creation
- [ ] Save data loading
- [ ] Auto-save support
- [ ] Save data listing
- [ ] Save data deletion
- [ ] Icon and metadata handling

### Parallel Processing Modules

#### cellSpurs (High Priority)
- [x] SPURS initialization
- [x] SPURS finalization
- [x] LV2 event queue attachment
- [x] Workload priority setting
- [x] Task queue management
- [x] Job chain support
- [x] Taskset operations
- [x] Event flags and barriers
- [ ] Actual SPU workload scheduling
- [ ] SPURS kernel integration
- [ ] SPURS handler implementation
- [ ] Trace buffer support

#### cellSpursJq
- [ ] Job queue creation
- [ ] Job submission
- [ ] Job synchronization

#### libsre
- [ ] SPU Runtime Environment
- [ ] Module loading on SPU

### Input Modules

#### cellPad (High Priority)
- [ ] Controller enumeration
- [ ] Button state reading
- [ ] Analog stick reading
- [ ] Pressure sensitivity
- [ ] Vibration feedback
- [ ] Sensor data (sixaxis)

#### cellKb
- [ ] Keyboard input
- [ ] Key mapping
- [ ] Special keys

#### cellMouse
- [ ] Mouse input
- [ ] Button states
- [ ] Movement delta

#### cellMic
- [ ] Microphone input
- [ ] Audio capture

### Multimedia Modules

#### cellDmux
- [ ] Demultiplexer for video containers
- [ ] MPEG-2 support
- [ ] AVC/H.264 support

#### cellVdec
- [ ] Video decoding
- [ ] H.264 decoder
- [ ] MPEG-2 decoder

#### cellAdec
- [ ] Audio decoding
- [ ] AAC decoder
- [ ] MP3 decoder
- [ ] AT3/AT9 decoder

#### cellVpost
- [ ] Video post-processing
- [ ] Color conversion
- [ ] Scaling

### Network Modules

#### cellNetCtl
- [ ] Network interface control
- [ ] Connection status
- [ ] IP configuration

#### cellHttp
- [ ] HTTP client
- [ ] Request/response handling
- [ ] Cookie management

#### cellSsl
- [ ] SSL/TLS support
- [ ] Certificate handling

### Audio Modules

#### cellAudio
- [x] Audio port management
- [x] Multi-channel support
- [ ] Complete port configuration
- [ ] Audio mixing improvements
- [ ] Surround sound support

### File System Modules

#### cellFs (High Priority)
- [ ] File open/close/read/write
- [ ] Directory operations
- [ ] File attributes
- [ ] Stat operations
- [ ] Truncation/seeking
- [ ] Async I/O

### Font Modules

#### cellFont
- [ ] Font loading
- [ ] Glyph rendering
- [ ] Text layout

#### cellFontFT
- [ ] FreeType integration
- [ ] TrueType/OpenType support

---

## Graphics (RSX)

### Vulkan Backend
- [x] Basic Vulkan initialization
- [x] Render pass creation
- [x] Command buffer management
- [x] Framebuffer management
- [x] 16 vertex attributes
- [x] 16 texture units
- [x] Blend state management
- [x] Depth/stencil state management
- [x] Multi-frame synchronization
- [ ] Complete vertex attribute formats
- [ ] All texture formats (DXT, swizzled, etc.)
- [ ] Render-to-texture
- [ ] Multi-render-target (MRT)
- [ ] Cubemaps
- [ ] 3D textures
- [ ] Shadow mapping
- [ ] Anti-aliasing (MSAA)

### Shader System
- [x] SPIR-V compilation (C++)
- [x] Basic vertex shader translation
- [x] Basic fragment shader translation
- [ ] Complete RSX shader instruction support
- [ ] Shader caching with disk persistence
- [ ] Shader hot-reloading for debugging
- [ ] Texture projection modes
- [ ] All blend modes
- [ ] Fog effects

### FIFO Processing
- [x] Command FIFO structure
- [x] NV4097 method handlers
- [ ] Complete method coverage
- [ ] DMA transfers
- [ ] Semaphore operations
- [ ] Performance counters

### Display Management
- [x] Display buffer configuration
- [x] Flip operations (basic)
- [ ] VSync implementation
- [ ] Resolution scaling
- [ ] Post-processing effects
- [ ] Screenshot capture

---

## Audio System

### Backend (cpal)
- [x] Cross-platform audio output
- [x] 8 audio ports
- [x] 48kHz sample rate
- [x] Multi-channel support
- [ ] Latency optimization
- [ ] Buffer underrun handling
- [ ] SPDIF output

### Features
- [x] Basic audio mixing
- [x] Time stretching
- [x] Resampling
- [ ] 3D audio positioning
- [ ] Reverb effects
- [ ] Surround sound encoding
- [ ] Audio format conversion

---

## Input System

### Controller Support
- [x] DualShock 3 emulation structure
- [x] Customizable key mappings
- [x] Keyboard input
- [x] Mouse input
- [ ] Native DualShock 3 USB support
- [ ] DualShock 4 support
- [ ] DualSense support
- [ ] Generic gamepad support
- [ ] Vibration feedback
- [ ] Motion controls (sixaxis)

### Input Devices
- [x] Keyboard device
- [x] Mouse device
- [ ] Camera device (stub)
- [ ] Move controller
- [ ] Guitar/drum peripherals
- [ ] Racing wheel support

---

## Loader & File System

### File Formats
- [x] ELF file parsing
- [x] SELF file parsing
- [x] PRX module loading
- [x] PUP firmware parsing
- [x] PKG package parsing
- [x] PARAM.SFO parsing
- [ ] SELF decryption with firmware keys
- [ ] Complete PRX relocation handling
- [ ] SFB file support
- [ ] Trophy data (TROPUSR.DAT, etc.)

### Virtual File System
- [x] Mount point system
- [x] ISO 9660 support
- [x] Save data handling
- [x] Trophy support (structure)
- [ ] HDD emulation improvements
- [ ] Flash storage emulation
- [ ] Disc image mounting improvements
- [ ] File watching for hot-reload

### Crypto Engine
- [x] AES encryption/decryption
- [x] SHA-1 hashing
- [x] Key management structure
- [ ] Complete key database
- [ ] NPDRM handling
- [ ] Disc key extraction
- [ ] Debug key support

---

## User Interface

### Main UI (egui)
- [x] Main window layout
- [x] Game list view
- [x] Settings panel
- [x] Log viewer
- [x] Memory viewer
- [x] Shader debugger
- [x] Controller configuration
- [x] Theme support
- [ ] Performance overlay
- [ ] Trophy viewer
- [ ] Save state management UI
- [ ] Cheat code interface
- [ ] Game compatibility database
- [ ] Online update checking

### Usability
- [ ] Drag-and-drop game loading
- [ ] Recent games list
- [ ] Game grid view with cover art
- [ ] Fullscreen mode improvements
- [ ] Localization support

---

## Debugging Tools

### Current Tools
- [x] Log viewer with filtering
- [x] Memory viewer
- [x] Shader debugger
- [x] PPU debugger
- [x] SPU debugger
- [x] RSX debugger
- [x] Breakpoint management
- [x] Disassembler
- [x] Profiler

### Planned Improvements
- [ ] Step-by-step execution improvements
- [ ] Register watch windows
- [ ] Memory search functionality
- [ ] Call stack visualization
- [ ] Performance profiling graphs
- [ ] GPU command stream viewer
- [ ] Texture viewer
- [ ] Shader debugger improvements
- [ ] Network traffic inspector

---

## JIT Compilation (C++)

### PPU JIT (LLVM)
- [x] LLVM IR generation framework
- [x] Basic block compilation
- [x] Code caching
- [x] O2 optimization passes
- [x] 20+ PowerPC instructions
- [ ] Complete instruction coverage
- [ ] Branch prediction hints
- [ ] Inline caching for calls
- [ ] Register allocation optimization
- [ ] Lazy compilation
- [ ] Multi-threaded compilation

### SPU JIT (LLVM)
- [x] LLVM IR generation for SPU
- [x] 15+ SIMD instructions
- [ ] Complete instruction coverage
- [ ] Channel operations in JIT
- [ ] MFC DMA in JIT
- [ ] Loop optimization
- [ ] SIMD intrinsics usage

### RSX Shader Compiler
- [x] SPIR-V generation
- [ ] Complete RSX shader ops
- [ ] Shader linking
- [ ] Pipeline caching

---

## Testing & Quality Assurance

### Current Coverage
- [x] Memory tests (128+ tests)
- [x] PPU tests (75+ tests)
- [x] SPU tests (14+ tests)
- [x] RSX tests (36+ tests)
- [x] Integration tests (4+ tests)

### Needed Tests
- [ ] HLE module unit tests
- [ ] Loader tests with sample files
- [ ] VFS tests
- [ ] Audio system tests
- [ ] Input system tests
- [ ] UI integration tests
- [ ] Performance regression tests
- [ ] Compatibility test suite

### Testing Infrastructure
- [ ] CI/CD pipeline setup (GitHub Actions)
- [ ] Automated build testing
- [ ] Code coverage reporting
- [ ] Performance benchmarks
- [ ] Homebrew test suite

---

## Documentation

### Current Documentation
- [x] README.md with project overview
- [x] User manual (docs/USER_MANUAL.md)
- [x] PPU instruction reference
- [x] SPU instruction reference
- [x] JIT compilation docs
- [x] Memory management docs

### Needed Documentation
- [ ] Architecture overview document
- [ ] Contributing guide (CONTRIBUTING.md)
- [ ] API documentation for each crate
- [ ] HLE module implementation guide
- [ ] Debugging guide
- [ ] Performance tuning guide
- [ ] Homebrew development guide

---

## Build & Infrastructure

### Build System
- [x] Cargo workspace setup
- [x] CMake for C++ components
- [x] Cross-platform support (Linux, Windows, macOS)
- [ ] GitHub Actions CI/CD
- [ ] Release automation
- [ ] Code signing for releases
- [ ] Package distribution (deb, rpm, msi, dmg)

### Code Quality
- [ ] Clippy integration in CI
- [ ] Rustfmt enforcement
- [ ] C++ clang-format rules
- [ ] Pre-commit hooks
- [ ] Dependency security scanning
- [ ] License compliance checking

---

## Known Issues

### Critical
- [ ] Some SELF files fail to decrypt without firmware keys
- [ ] PRX linking not complete for all modules
- [ ] RSX commands not properly routed to Vulkan backend

### Major
- [ ] Memory leaks in long-running sessions
- [ ] SPU thread synchronization issues
- [ ] Audio stuttering under load
- [ ] Shader compilation pauses (stuttering)

### Minor
- [ ] Log viewer performance with many entries
- [ ] Window resizing artifacts
- [ ] Keyboard shortcuts not working in all contexts

---

## Future Considerations

### Long-term Goals
- [ ] PlayStation Network (PSN) stub for single-player games
- [ ] Remote play support
- [ ] Netplay/online multiplayer (game-specific)
- [ ] Save state support
- [ ] Game-specific patches/fixes database
- [ ] Upscaling with AI/ML algorithms
- [ ] HDR support
- [ ] Ray tracing for enhanced graphics
- [ ] Steam Deck/handheld optimization
- [ ] Android/iOS port investigation

### Research Areas
- [ ] Hypervisor emulation improvements
- [ ] SPU-to-native compilation
- [ ] Hardware acceleration on specific GPUs
- [ ] Machine learning for game compatibility prediction

---

## How to Contribute

See the [Contributing](README.md#-contributing) section in the README for guidelines on how to contribute to this project.

### Priority Areas for Contributors

1. **HLE Modules** - Implementing system library functions
2. **Testing** - Writing tests for existing functionality
3. **Documentation** - Improving code comments and docs
4. **Bug Fixes** - Addressing known issues
5. **Performance** - Optimizing hot code paths

---

## Progress Tracking

| Area | Estimated Progress |
|------|-------------------|
| PPU Interpreter | 85% |
| SPU Interpreter | 70% |
| RSX Backend | 60% |
| Memory System | 90% |
| HLE Modules | 30% |
| Audio System | 65% |
| Input System | 40% |
| Loader | 75% |
| VFS | 60% |
| UI | 70% |
| JIT (PPU) | 40% |
| JIT (SPU) | 30% |

---

*This TODO list is subject to change as the project evolves.*
