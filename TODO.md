# 📋 Oxidized-Cell TODO List

This document tracks the development progress and remaining tasks for the oxidized-cell PS3 emulator.

---

## 🎯 High Priority Tasks

### HLE Modules
- [ ] **cellGcmSys** - Complete RSX command translation for actual rendering
- [ ] **cellSysutil** - Finish callback invocation on PPU thread
- [ ] **cellSpurs** - Implement actual SPU workload dispatching to SPU interpreter
- [ ] **cellPad** - Complete integration with oc-input backend for real controller input
- [ ] **cellFs** - Wire up remaining file operations to oc-vfs

### Game Loading Pipeline
- [ ] Complete ELF segment loading to memory
- [ ] Finish PRX module dependency resolution and linking
- [ ] Implement SELF file decryption using firmware keys
- [ ] Complete NID (Native ID) resolution for HLE function hooking

### Core Emulation
- [ ] Wire up PPU interpreter execution loop to main emulator runner
- [ ] Connect SPU interpreter to SPURS workload dispatcher
- [ ] Complete RSX FIFO command processing from GCM command buffer
- [ ] Implement frame pacing and VSync synchronization

---

## 🔧 Medium Priority Tasks

### PPU Emulation (`oc-ppu`)
- [ ] Complete VMX/AltiVec instruction edge cases
- [ ] Implement FPSCR exception handling
- [ ] Add missing branch prediction hints
- [ ] Implement performance counters
- [ ] Complete condition register operations

### SPU Emulation (`oc-spu`)
- [ ] Implement all MFC DMA operations
- [ ] Complete channel communication with PPU
- [ ] Add SPU interrupt handling
- [ ] Implement SPU mailbox operations
- [ ] Complete atomic operations (GETLLAR, PUTLLC, PUTLLUC)

### RSX Graphics (`oc-rsx`)
- [ ] Implement vertex shader translation to SPIR-V
- [ ] Implement fragment shader translation to SPIR-V
- [ ] Complete texture format conversions (DXT1, DXT3, DXT5)
- [ ] Implement render target format handling
- [ ] Add multi-sample anti-aliasing support
- [ ] Complete depth/stencil buffer operations
- [ ] Implement tile/z-cull optimizations

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
- [ ] Complete SPU thread group lifecycle management

---

## 📦 HLE Module Implementation Status

### Graphics Modules
| Module | Status | Notes |
|--------|--------|-------|
| cellGcmSys | 🟡 Partial | Command buffer, display buffers, textures implemented; needs RSX bridge |
| cellGifDec | 🟡 Stub | Basic structure only |
| cellPngDec | 🟡 Stub | Basic structure only |
| cellJpgDec | 🟡 Stub | Basic structure only |
| cellResc | 🟡 Stub | Resolution scaling stub |

### System Modules
| Module | Status | Notes |
|--------|--------|-------|
| cellSysutil | 🟡 Partial | Callbacks, params, dialogs implemented |
| cellGame | 🟡 Partial | Game content path handling |
| cellSaveData | 🟡 Stub | Save data management stub |

### Input Modules
| Module | Status | Notes |
|--------|--------|-------|
| cellPad | 🟡 Partial | DualShock3 support, needs full oc-input integration |
| cellKb | 🟡 Stub | Keyboard input stub |
| cellMouse | 🟡 Stub | Mouse input stub |
| cellMic | 🟡 Stub | Microphone stub |

### Audio/Video Modules
| Module | Status | Notes |
|--------|--------|-------|
| cellAudio | 🟡 Partial | Audio ports, mixing implemented |
| cellDmux | 🟡 Stub | Demux stub |
| cellVdec | 🟡 Stub | Video decoder stub |
| cellAdec | 🟡 Stub | Audio decoder stub |
| cellVpost | 🟡 Stub | Video post-processing stub |

### Network Modules
| Module | Status | Notes |
|--------|--------|-------|
| cellNetCtl | 🟡 Stub | Network control stub |
| cellHttp | 🟡 Stub | HTTP client stub |
| cellSsl | 🟡 Stub | SSL/TLS stub |

### Utility Modules
| Module | Status | Notes |
|--------|--------|-------|
| cellSpurs | 🟡 Partial | Task scheduling, SPU bridge connected |
| cellSpursJq | 🟡 Stub | Job queue stub |
| cellFont | 🟡 Stub | Font rendering stub |
| cellFontFt | 🟡 Stub | FreeType font stub |
| libsre | 🟡 Stub | SPU Runtime Extension stub |

### File System
| Module | Status | Notes |
|--------|--------|-------|
| cellFs | 🟡 Partial | File ops implemented, needs VFS integration |

---

## 🧪 Testing Tasks

### Unit Tests
- [ ] Add more PPU instruction edge case tests
- [ ] Add SPU channel communication tests
- [ ] Add RSX state management tests
- [ ] Add HLE callback invocation tests

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

- [ ] Implement game library scanning and caching
- [ ] Add game cover art display
- [ ] Complete settings persistence
- [ ] Add controller configuration UI
- [ ] Implement log filtering and search
- [ ] Add memory viewer hex editing
- [ ] Complete shader debugger visualization
- [ ] Add performance overlay (FPS, CPU usage)

---

## 🔌 JIT Compilation (C++)

### PPU JIT (`cpp/src/ppu_jit.cpp`)
- [ ] Implement remaining integer instructions
- [ ] Add floating-point JIT compilation
- [ ] Implement branch and link optimization
- [ ] Add VMX instruction JIT support
- [ ] Implement basic block caching improvements

### SPU JIT (`cpp/src/spu_jit.cpp`)
- [ ] Implement remaining SIMD instructions
- [ ] Add shuffle/permute instruction JIT
- [ ] Implement channel operation JIT
- [ ] Add DMA operation JIT support

### RSX Shaders (`cpp/src/rsx_shaders.cpp`)
- [ ] Complete vertex program to SPIR-V translation
- [ ] Complete fragment program to SPIR-V translation
- [ ] Implement shader caching
- [ ] Add shader hot-reloading for debugging

---

## 📚 Documentation Tasks

- [ ] Write architecture overview document
- [ ] Document HLE module implementation guide
- [ ] Add JIT compilation documentation
- [ ] Create game compatibility list
- [ ] Write debugging guide
- [ ] Add contributor guidelines

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

## 📅 Future Milestones

### v0.1.0 - Foundation
- [x] Core architecture in place
- [x] PPU interpreter functional
- [x] SPU interpreter functional
- [x] Basic HLE framework
- [ ] First homebrew running

### v0.2.0 - Graphics
- [ ] Basic RSX rendering working
- [ ] Simple 2D games running
- [ ] Audio output functional

### v0.3.0 - Compatibility
- [ ] Commercial game loading
- [ ] SELF decryption working
- [ ] Multiple games partially running

### v1.0.0 - Playable
- [ ] Multiple commercial games fully playable
- [ ] JIT compilation for acceptable performance
- [ ] Stable and well-tested

---

## 📝 Notes

### Development Tips
- Use `cargo test -p oc-<crate>` to test individual components
- Enable `RUST_LOG=debug` for detailed logging
- The `examples/` folder contains useful testing code
- Check `docs/` for instruction references

### Key Resources
- [PS3 Developer Wiki](https://www.psdevwiki.com/)
- [RPCS3 Source](https://github.com/RPCS3/rpcs3) - Reference implementation
- [Cell BE Programming Handbook](https://www.ibm.com/support/pages/cell-be-programming-handbook)

---

*Last updated: December 2024*
