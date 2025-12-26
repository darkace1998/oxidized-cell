# 🎮 oxidized-cell Development Roadmap

This document outlines the development tasks, improvements, and features planned for the oxidized-cell PS3 emulator.

---

## 📊 Project Status Overview

| Component | Status | Completion | Priority |
|-----------|--------|------------|----------|
| Memory Management | ✅ Complete | 100% | - |
| PPU Interpreter | ✅ Complete | 95% | Low |
| SPU Interpreter | ✅ Complete | 95% | Low |
| RSX/Vulkan Backend | ✅ Complete | 95% | Low |
| JIT Compilation | ✅ Complete | 100% | - |
| LV2 Kernel | 🔨 Mostly Complete | 75% | Medium |
| Audio System | ✅ Complete | 85% | Low |
| Input System | ✅ Complete | 80% | Low |
| VFS | ✅ Complete | 80% | Low |
| ELF/Game Loader | ✅ Complete | 90% | Low |
| HLE Modules | 🚧 In Progress | 15% | **High** |
| User Interface | 🚧 In Progress | 15% | Medium |
| Game Loading Pipeline | ❌ Not Started | 0% | **High** |

---

## 🔴 High Priority Tasks

### 1. HLE Modules Implementation (Critical for Game Execution)

The HLE (High-Level Emulation) modules are essential for games to run. Current implementation provides stubs but needs actual functionality.

#### cellGcmSys (Graphics Command Management)
- [ ] Complete RSX backend integration in `oc-hle/src/cell_gcm_sys.rs`
- [ ] Implement memory read/write for texture descriptors
- [ ] Connect command buffer submission to `oc-rsx`
- [ ] Implement proper flip/vsync synchronization
- [ ] Add missing GCM functions:
  - [ ] `cellGcmSetViewport`
  - [ ] `cellGcmSetScissor`
  - [ ] `cellGcmSetBlendFunc`
  - [ ] `cellGcmSetVertexProgram`
  - [ ] `cellGcmSetFragmentProgram`
  - [ ] `cellGcmDrawArrays`
  - [ ] `cellGcmDrawIndexArray`

#### cellSysutil (System Utilities)
- [ ] Implement system callback registration in `oc-hle/src/cell_sysutil.rs`
- [ ] Add message dialog functionality
- [ ] Implement OSK (on-screen keyboard)
- [ ] Add disc ejection/insertion notifications
- [ ] Implement save data dialog integration
- [ ] Add trophy unlock notifications

#### cellSpurs (SPU Runtime System)
- [ ] Complete task queue execution in `oc-hle/src/cell_spurs.rs`
- [ ] Connect to actual SPU thread execution
- [ ] Implement job chain execution on SPUs
- [ ] Add proper synchronization with PPU
- [ ] Implement taskset completion callbacks

#### cellPad (Controller Input)
- [ ] Connect to `oc-input` system in `oc-hle/src/cell_pad.rs`
- [ ] Implement pressure-sensitive button support
- [ ] Add SIXAXIS motion sensor data
- [ ] Implement vibration/rumble feedback
- [ ] Add multi-controller support (up to 7 controllers)

#### cellFs (File System)
- [ ] Connect to `oc-vfs` in `oc-hle/src/cell_fs.rs`
- [ ] Implement all file operations:
  - [ ] `cellFsOpen`, `cellFsClose`
  - [ ] `cellFsRead`, `cellFsWrite`
  - [ ] `cellFsLseek`, `cellFsStat`
  - [ ] `cellFsMkdir`, `cellFsRmdir`
  - [ ] `cellFsReaddir`, `cellFsUnlink`
- [ ] Add proper error handling and PS3 error codes
- [ ] Implement file attribute handling

#### cellAudio (Audio Output)
- [ ] Connect to `oc-audio` backend in `oc-hle/src/cell_audio.rs`
- [ ] Implement audio port management
- [ ] Add proper sample rate conversion
- [ ] Implement multi-channel audio mixing

### 2. Game Loading Pipeline

Complete end-to-end game loading in `oc-integration/src/pipeline.rs`:

- [ ] Implement complete game initialization sequence:
  - [ ] Parse PARAM.SFO for game metadata
  - [ ] Load and decrypt EBOOT.BIN (SELF format)
  - [ ] Initialize system modules in correct order
  - [ ] Set up memory layout for game execution
  - [ ] Initialize main PPU thread with proper register state
- [ ] Add game compatibility database
- [ ] Implement PRX module loading and linking
- [ ] Add NID (Native ID) resolution for imports
- [ ] Create save data initialization
- [ ] Implement trophy data loading

### 3. PRX Shared Library Support

Enhance `oc-loader/src/prx.rs`:

- [ ] Implement PRX file parsing and loading
- [ ] Add proper symbol resolution using NIDs
- [ ] Implement module linking and relocation
- [ ] Add module start/stop entry point execution
- [ ] Create module dependency resolution
- [ ] Implement module versioning support

---

## 🟡 Medium Priority Tasks

### 4. LV2 Kernel Improvements

Complete remaining syscalls in `oc-lv2/`:

#### Thread Management (`thread.rs`)
- [ ] Implement thread priorities and scheduling
- [ ] Add thread local storage (TLS) support
- [ ] Implement thread cleanup handlers
- [ ] Add thread-specific data (pthread_key)

#### Synchronization (`sync/`)
- [ ] Complete mutex implementation with proper blocking
- [ ] Implement condition variable wait with timeout
- [ ] Add reader-writer lock functionality
- [ ] Implement event flag operations
- [ ] Add lightweight mutex (lwmutex) support
- [ ] Implement semaphore with proper counting

#### Memory (`memory.rs`)
- [ ] Implement memory mapping syscalls
- [ ] Add memory protection changes
- [ ] Implement memory container management
- [ ] Add physical memory allocation

#### Timer (`timer.rs`)
- [ ] Implement high-resolution timer
- [ ] Add periodic timer support
- [ ] Implement timer interrupts

### 5. RSX Graphics Improvements

Enhance `oc-rsx/`:

#### Shader Support
- [ ] Implement vertex shader translation in `shader.rs`
- [ ] Implement fragment shader translation
- [ ] Add shader caching mechanism
- [ ] Support shader microcode formats

#### Texture Handling (`texture.rs`)
- [ ] Implement all texture formats:
  - [ ] ARGB8, RGB565, DXT1/3/5
  - [ ] Depth textures (D16, D24S8)
  - [ ] Swizzled texture formats
- [ ] Add texture filtering modes
- [ ] Implement mipmapping support
- [ ] Add cubemap texture support

#### Rendering (`backend/vulkan.rs`)
- [ ] Implement all draw commands
- [ ] Add multi-render target (MRT) support
- [ ] Implement occlusion queries
- [ ] Add render-to-texture support
- [ ] Implement anti-aliasing modes

### 6. User Interface Improvements

Enhance `oc-ui/`:

#### Game Management (`game_list.rs`)
- [ ] Implement game grid view with icons
- [ ] Add game sorting and filtering
- [ ] Implement game search functionality
- [ ] Add favorites/recently played lists
- [ ] Display game compatibility status

#### Debugger (`debugger.rs`)
- [ ] Implement PPU register viewer
- [ ] Add SPU register viewer
- [ ] Implement memory search/edit
- [ ] Add disassembly view
- [ ] Implement breakpoint management UI
- [ ] Add call stack display

#### Settings (`settings.rs`)
- [ ] Add graphics settings panel
- [ ] Implement audio settings
- [ ] Add input configuration UI
- [ ] Implement path configuration
- [ ] Add emulator behavior settings

### 7. Audio System Improvements

Enhance `oc-audio/`:

- [ ] Implement proper audio mixing in `mixer.rs`
- [ ] Add surround sound support (5.1, 7.1)
- [ ] Implement audio effects
- [ ] Add audio buffer synchronization
- [ ] Implement audio streaming for large files

### 8. Input System Improvements

Enhance `oc-input/`:

- [ ] Add SDL2 or winit backend for real controller support
- [ ] Implement keyboard-to-controller mapping
- [ ] Add mouse support for pointer games
- [ ] Implement pressure sensitivity simulation
- [ ] Add motion controls emulation

---

## 🟢 Low Priority Tasks

### 9. PPU Interpreter Completion

Complete remaining instructions in `oc-ppu/src/instructions/`:

#### VMX/AltiVec Instructions (`vector.rs`)
- [ ] Implement remaining vector operations
- [ ] Add vector permute instructions
- [ ] Implement vector floating-point

#### System Instructions (`system.rs`)
- [ ] Implement all SPR (Special Purpose Register) access
- [ ] Add hypervisor call support
- [ ] Implement cache control instructions

### 10. SPU Interpreter Completion

Complete remaining features in `oc-spu/`:

#### MFC (Memory Flow Controller) (`mfc.rs`)
- [ ] Implement all DMA commands
- [ ] Add tag group management
- [ ] Implement MFC status queries
- [ ] Add proper atomic operations

#### Channel Operations (`channels.rs`)
- [ ] Implement all channel read/write operations
- [ ] Add interrupt handling
- [ ] Implement decrementer support

### 11. VFS Improvements

Enhance `oc-vfs/`:

#### Disc Support (`disc.rs`)
- [ ] Implement ISO 9660 filesystem parsing
- [ ] Add UDF filesystem support
- [ ] Implement disc sector reading

#### Save Data (`savedata.rs`)
- [ ] Implement save data encryption/decryption
- [ ] Add PFD (Protected File Descriptor) support
- [ ] Implement save data copying

#### Trophy System (`trophy.rs`)
- [ ] Implement trophy data parsing
- [ ] Add trophy unlock tracking
- [ ] Implement trophy synchronization

### 12. Loader Improvements

Enhance `oc-loader/`:

#### SELF Decryption (`self_file.rs`)
- [ ] Implement proper SELF file decryption
- [ ] Add key management
- [ ] Support different encryption types

#### ELF Loading (`elf.rs`)
- [ ] Add dynamic linking support
- [ ] Implement lazy symbol resolution
- [ ] Add support for large ELF files

### 13. Debug System

Enhance `oc-debug/`:

- [ ] Implement GDB remote protocol support in `ppu_debugger.rs`
- [ ] Add trace logging with configurable levels
- [ ] Implement performance counters
- [ ] Add memory access tracking
- [ ] Implement instruction trace recording

---

## 🔧 Technical Debt & Improvements

### Code Quality
- [ ] Add comprehensive documentation to all public APIs
- [ ] Implement proper error handling throughout
- [ ] Add logging with tracing levels
- [ ] Remove TODO/FIXME comments by implementing features
- [ ] Add clippy lint compliance

### Testing
- [ ] Add unit tests for all HLE modules
- [ ] Create integration tests for game loading
- [ ] Add PPU instruction tests (expand from current 75+)
- [ ] Create SPU instruction tests (expand from current 14+)
- [ ] Add RSX command tests
- [ ] Implement fuzzing for critical components

### Performance
- [ ] Profile and optimize hot paths
- [ ] Implement JIT compilation fallback
- [ ] Add SIMD optimizations where possible
- [ ] Implement memory access caching
- [ ] Add frame pacing improvements

### Build System
- [ ] Add CI/CD pipeline for automated builds
- [ ] Create cross-platform build scripts
- [ ] Add automated testing in CI
- [ ] Implement release automation

---

## 📝 Documentation Tasks

- [ ] Write architecture overview document
- [ ] Create contribution guide
- [ ] Document HLE module implementation guide
- [ ] Add debugging guide for developers
- [ ] Create user manual for emulator usage
- [ ] Document save state format (when implemented)
- [ ] Add API documentation generation

---

## 🎯 Milestone Goals

### Milestone 1: Basic Game Loading
**Target:** Load and display game title screens
- Complete essential HLE modules (cellGcmSys, cellSysutil)
- Implement basic game loading pipeline
- Get simple homebrew applications running

### Milestone 2: Interactive Games
**Target:** Run simple 2D games with input
- Complete input system (cellPad)
- Implement audio output (cellAudio)
- Support basic file system operations

### Milestone 3: 3D Graphics
**Target:** Render 3D graphics correctly
- Complete RSX shader support
- Implement all texture formats
- Support common rendering techniques

### Milestone 4: Commercial Games
**Target:** Run retail PS3 games
- Complete HLE module coverage
- Implement save data support
- Add trophy system support

### Milestone 5: Optimization
**Target:** Achieve playable performance
- Optimize JIT compilation
- Implement frame skipping
- Add async shader compilation

---

## 🤝 Contribution Areas

### Good First Issues
- Add missing error codes to HLE modules
- Implement stub functions for unimplemented HLE calls
- Add unit tests for existing functionality
- Improve documentation and comments

### Intermediate Tasks
- Implement specific HLE function calls
- Add new PPU/SPU instruction implementations
- Extend VFS format support
- Improve UI components

### Advanced Tasks
- Implement RSX shader translation
- Add SPURS task scheduling
- Optimize JIT compilation
- Implement network emulation

---

## 📚 Reference Resources

- [PS3 Developer Wiki](https://www.psdevwiki.com/)
- [Cell BE Programming Handbook](https://www.ibm.com/support/pages/cell-be-programming-handbook)
- [RPCS3 Source Code](https://github.com/RPCS3/rpcs3) - Reference implementation
- [LibPSF](https://github.com/RPCS3/rpcs3/tree/master/rpcs3/Loader) - PARAM.SFO parsing reference

---

*Last updated: December 2024*
