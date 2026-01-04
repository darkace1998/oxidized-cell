# 📋 oxidized-cell Development TODO

A comprehensive task list for the oxidized-cell PlayStation 3 emulator project.

---

## 🔴 High Priority

### JIT Compiler Improvements

- [x] **PPU JIT: Add CR0 update for arithmetic instructions** - `cpp/src/ppu_jit.cpp:822` ✅ Implemented
- [x] **PPU JIT: Implement CA flag handling in XER for adde/addze/addme instructions** - `cpp/src/ppu_jit.cpp:872-880` ✅ Implemented
- [x] **PPU JIT: Set CA flag for carry-based arithmetic operations** ✅ Implemented
  - subfic instruction - `cpp/src/ppu_jit.cpp:1323`
  - subfc instruction - `cpp/src/ppu_jit.cpp:1331`
  - adde instruction - `cpp/src/ppu_jit.cpp:1344`
  - addze instruction - `cpp/src/ppu_jit.cpp:1353`
  - addme instruction - `cpp/src/ppu_jit.cpp:1359`
  - subfe instruction - `cpp/src/ppu_jit.cpp:1366`
  - subfze instruction - `cpp/src/ppu_jit.cpp:1374`
  - subfme instruction - `cpp/src/ppu_jit.cpp:1383`
- [x] **PPU JIT: Add SO (Summary Overflow) bit from XER** - `cpp/src/ppu_jit.cpp:1662` ✅ Implemented
- [ ] **Complete PPU JIT instruction coverage** - Currently supports ~20+ instructions, need full PowerPC ISA
- [ ] **Complete SPU JIT instruction coverage** - Currently supports ~15+ instructions, need full SPU ISA

### HLE Module Global Context

- [x] **cellAdec: Implement PCM item retrieval through global context** - `crates/oc-hle/src/cell_adec.rs:545` ✅ Implemented
- [x] **cellDmux: Implement global manager instance** - `crates/oc-hle/src/cell_dmux.rs:1711` ✅ Already implemented
- [x] **cellDmux: Implement global manager instance for ES handling** - `crates/oc-hle/src/cell_dmux.rs:1741` ✅ Already implemented
- [x] **cellVpost: Use global manager instead of temporary instance** - `crates/oc-hle/src/cell_vpost.rs:1069` ✅ Already implemented
- [x] **cellVdec: Store frame rate configuration** - `crates/oc-hle/src/cell_vdec.rs:2523` ✅ Implemented
- [x] **cellVdec: Implement picture item retrieval through global context** - `crates/oc-hle/src/cell_vdec.rs:2662` ✅ Implemented
- [x] **libsre: Implement global regex manager** - `crates/oc-hle/src/libsre.rs:530` ✅ Already implemented

---

## 🟡 Medium Priority

### Game Loading & Execution

- [ ] Complete game loading pipeline end-to-end
- [ ] Test with PS3 homebrew applications
- [ ] Improve ELF/SELF loading error messages
- [ ] Add PKG installation progress UI
- [ ] Implement game update detection and patching

### RSX Graphics

- [ ] Complete NV4097 method handlers for all draw commands
- [ ] Implement additional texture formats
- [ ] Add shader caching to disk
- [ ] Improve RSX timing accuracy
- [ ] Implement anti-aliasing modes

### PPU Interpreter

- [ ] Profile and optimize hot instruction paths
- [ ] Improve FPSCR edge case handling
- [ ] Add more comprehensive VMX/AltiVec instruction coverage

### SPU Subsystem

- [ ] Improve MFC DMA timing accuracy
- [ ] Implement full channel communication
- [ ] Add SPU profiling tools
- [ ] Optimize SIMD operations for host AVX/NEON

### LV2 Kernel

- [ ] Implement additional syscalls for game compatibility
- [ ] Improve PRX module loading
- [ ] Add event queue debugging
- [ ] Implement remaining sync primitives edge cases

---

## 🟢 Lower Priority

### Audio System

- [ ] Add 7.1 surround sound output support
- [ ] Implement audio resampling quality options
- [ ] Add audio latency configuration
- [ ] Support DTS/Dolby passthrough

### Input System

- [ ] Improve DualShock3 sixaxis calibration
- [ ] Add rumble intensity configuration
- [ ] Support DualSense controller
- [ ] Improve keyboard layout support

### Virtual File System

- [ ] Add SFB container support
- [ ] Improve ISO 9660 UDF handling
- [ ] Add compressed save data support
- [ ] Implement file watching for hot-reload

### User Interface

- [ ] Add game cover art display
- [ ] Implement per-game settings
- [ ] Add shader debugger visualization
- [ ] Improve memory viewer hex editing
- [ ] Add controller configuration presets
- [ ] Implement save state support UI

### Performance

- [ ] Add multi-threaded PPU execution
- [ ] Implement GPU-assisted texture decoding
- [ ] Add frame limiting options
- [ ] Profile and optimize memory allocation
- [ ] Implement async shader compilation

---

## 🧪 Testing

- [ ] Increase PPU instruction test coverage (currently 75+ tests)
- [ ] Increase SPU instruction test coverage (currently 14+ tests)
- [ ] Add integration tests for game loading
- [ ] Create homebrew test ROM suite
- [ ] Add regression testing CI workflow
- [ ] Benchmark JIT vs Interpreter performance

---

## 📚 Documentation

- [ ] Document RSX method handlers
- [ ] Add syscall reference documentation
- [ ] Create debugging guide
- [ ] Write game compatibility reporting guide
- [ ] Document build system for all platforms
- [ ] Add architecture diagrams

---

## 🛠️ Infrastructure

- [ ] Set up Windows CI builds
- [ ] Add macOS ARM (Apple Silicon) CI
- [ ] Implement auto-updater
- [ ] Add crash reporting
- [ ] Create release automation

---

## 🔬 Research & Investigation

- [ ] Investigate cross-block JIT optimization
- [ ] Research profile-guided JIT optimization
- [ ] Explore GPU compute for SPU emulation
- [ ] Investigate alternative graphics backends (Metal, D3D12)
- [ ] Research self-modifying code handling strategies

---

## 📊 Current Status Summary

| Component | Status | Notes |
|-----------|--------|-------|
| PPU Interpreter | 🟢 Complete | 2,700+ lines, all core instructions |
| PPU JIT | 🟡 Partial | ~20 instructions, needs expansion |
| SPU Interpreter | 🟢 Complete | Full 128-bit SIMD support |
| SPU JIT | 🟡 Partial | ~15 instructions, needs expansion |
| RSX Graphics | 🟢 Complete | Vulkan backend, core rendering |
| HLE Modules | 🟢 Complete | All major modules implemented |
| LV2 Kernel | 🟢 Complete | Syscalls, sync primitives |
| Audio | 🟢 Complete | 8 ports, multi-channel |
| Input | 🟢 Complete | Controller, keyboard, mouse |
| VFS | 🟢 Complete | ISO, PKG, save data |
| UI | 🟢 Complete | egui-based, debugger |

---

## ✅ Recently Completed

- [x] All HLE modules (cellGcmSys, cellSysutil, cellFs, cellPad, etc.)
- [x] Full PPU interpreter implementation
- [x] Full SPU interpreter implementation
- [x] Vulkan RSX backend
- [x] Audio system with cpal
- [x] Input handling with sixaxis support
- [x] VFS with ISO/PKG/PARAM.SFO support
- [x] ELF/SELF/PRX loader
- [x] LV2 syscall framework
- [x] egui-based UI with debugger

---

*Last updated: 2026-01-04*
