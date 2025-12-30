# oxidized-cell TODO

This document outlines the development roadmap and remaining work for the oxidized-cell PS3 emulator.

## 🎯 Project Status: Early Development

The emulator has a solid foundation with core subsystems implemented. The primary focus now should be on completing HLE modules and the game loading pipeline to enable game execution.

---

## 🔥 High Priority (Critical for Game Execution)

### HLE Module Completion
The HLE (High Level Emulation) modules are essential for games to run. These need memory subsystem integration.

- [ ] **cellGcmSys** - Complete memory read/write operations for shader programs
  - `cell_gcm_set_vertex_program()` - Read CellGcmVertexProgram from memory
  - `cell_gcm_set_fragment_program()` - Read CellGcmFragmentProgram from memory
  - `cell_gcm_get_configuration()` - Write config to memory at provided address
  - `cell_gcm_address_to_offset()` - Write offset to memory
  - `cell_gcm_map_main_memory()` - Write RSX offset to memory

- [ ] **cellSpurs** - SPURS task scheduler enhancements
  - `cell_spurs_set_priorities()` - Read priorities from memory
  - `cell_spurs_get_spu_thread_id()` - Write thread ID to memory
  - Implement actual SPU thread group creation
  - Connect to SPU interpreter for workload execution

- [ ] **cellSysutil** - System utilities completion
  - `cell_sysutil_get_system_param_int()` - Write value to memory
  - `cell_sysutil_get_system_param_string()` - Write string to memory
  - `cell_sysutil_get_ps_id()` - Write PSID to memory
  - Dialog callbacks - Integrate with PPU for callback invocation

- [ ] **cellFs** - File system operations
  - Connect to VFS layer for actual file I/O
  - Implement file read/write with memory subsystem

- [ ] **cellPad** - Controller input
  - Connect to oc-input for actual controller state
  - Implement button state reading to memory

### Game Loading Pipeline

- [ ] **SELF Decryption** - Complete firmware key extraction
  - SELF files are Sony's encrypted executable format (Signed ELF)
  - Extract AES keys from PS3UPDAT.PUP firmware file
  - Implement SELF -> ELF decryption chain
  - Add support for encrypted PRX modules

- [ ] **PRX Loading** - Complete module linking
  - Implement NID-based symbol resolution
  - Load and link PRX dependencies
  - Handle module initialization callbacks

- [ ] **Memory Mapping Integration**
  - Connect HLE modules to MemoryManager
  - Implement PPU/SPU memory access from HLE

### RSX/Graphics Integration

- [ ] **Connect cellGcmSys to oc-rsx**
  - Route command buffer submissions to RSX thread
  - Implement display buffer configuration in RSX
  - Handle flip synchronization

- [ ] **Vulkan Backend Completion**
  - Complete shader compilation pipeline
  - Implement all NV4097 method handlers
  - Add texture upload/sampling support

---

## 📌 Medium Priority (Feature Completeness)

### PPU Enhancements

- [ ] **JIT Compiler Integration**
  - Connect C++ PPU JIT to Rust interpreter
  - Implement JIT block caching
  - Add JIT/Interpreter hybrid mode

- [ ] **Complete Instruction Set**
  - Add remaining VMX/AltiVec instructions
  - Implement privileged instructions for LLE
  - Add branch prediction support

- [ ] **Debugging Features**
  - Implement memory watchpoints
  - Add register state inspection
  - Trace logging for execution flow

### SPU Enhancements

- [ ] **DMA Operations**
  - Implement full MFC DMA commands
  - Add DMA list processing
  - Barrier synchronization

- [ ] **Channel Operations**
  - Complete SPU-PPU mailbox communication
  - Event flag support
  - Signal notification channels

- [ ] **JIT Integration**
  - Connect C++ SPU JIT to Rust interpreter
  - SIMD optimization for common patterns

### Audio System

- [ ] **cellAudio Full Implementation**
  - Multi-port audio mixing
  - Sample rate conversion
  - Audio buffer management

- [ ] **Codec Support**
  - cellAdec audio decoding
  - cellVdec video decoding
  - cellDmux demuxing

### Additional HLE Modules

- [ ] **cellGame** - Game data management
- [ ] **cellSaveData** - Save/load game progress
- [ ] **cellResc** - Resolution scaling
- [ ] **cellHttp** - HTTP networking
- [ ] **cellNetCtl** - Network control
- [ ] **cellSsl** - SSL/TLS support
- [ ] **cellPngDec** / **cellJpgDec** / **cellGifDec** - Image decoding
- [ ] **cellFont** / **cellFontFT** - Font rendering

---

## 🔧 Low Priority (Polish & Optimization)

### Performance Optimization

- [ ] **Memory Manager**
  - Implement page table caching
  - Add memory access coalescing
  - Optimize reservation station

- [ ] **Thread Scheduler**
  - Priority-based preemption
  - Time slice optimization
  - Thread affinity support

- [ ] **RSX Performance**
  - Shader caching
  - Command buffer batching
  - Texture streaming

### UI Improvements

- [ ] **Game List**
  - Icon loading from SFO/PKG
  - Game compatibility database
  - Save state management

- [ ] **Debugger**
  - Disassembly view with symbols
  - Call stack tracing
  - Breakpoint conditions

- [ ] **Memory Viewer**
  - Hex editor mode
  - Structure overlays
  - Search functionality

### Testing & Compatibility

- [ ] **Test Infrastructure**
  - Add more unit tests for PPU instructions
  - SPU instruction tests
  - Integration tests with test ROMs

- [ ] **Homebrew Compatibility**
  - Test with PSL1GHT samples
  - Document known working homebrew
  - Create compatibility list

---

## 🏗️ Architecture Improvements

### Code Organization

- [ ] **Error Handling**
  - Standardize error types across crates
  - Add context to error messages
  - Implement error recovery where possible

- [ ] **Configuration**
  - Per-game settings profiles
  - Import/export configuration
  - Command-line override support

- [ ] **Logging**
  - Structured logging with categories
  - Log file rotation
  - Performance logging

### Build System

- [ ] **C++ Integration**
  - Improve CMake integration
  - Add LLVM version detection
  - Cross-compilation support

- [ ] **CI/CD**
  - Add automated builds for all platforms
  - Implement test coverage reporting
  - Create release automation

---

## 📋 Quick Reference: Crate Dependencies

```
oxidized-cell (main binary)
├── oc-core (config, logging, scheduler)
├── oc-integration (EmulatorRunner, GameLoader)
├── oc-ui (egui interface)
├── oc-memory (4GB virtual memory)
├── oc-ppu (PowerPC interpreter)
├── oc-spu (SPU interpreter)
├── oc-rsx (Vulkan graphics)
├── oc-lv2 (kernel syscalls)
├── oc-hle (system libraries)
├── oc-audio (cpal backend)
├── oc-input (controller handling)
├── oc-vfs (virtual filesystem)
├── oc-loader (ELF/SELF/PRX)
├── oc-ffi (Rust/C++ bridge)
└── oc-debug (debugging tools)

cpp/ (C++ performance components)
├── ppu_jit.cpp (LLVM JIT for PPU)
├── spu_jit.cpp (LLVM JIT for SPU)
├── rsx_shaders.cpp (SPIR-V compilation)
├── atomics.cpp (128-bit atomic operations)
└── simd_avx.cpp (AVX helper functions)
```

---

## 📊 Current Implementation Statistics

| Component | Status | Test Coverage |
|-----------|--------|---------------|
| Memory Manager | ✅ Complete | 128+ tests |
| PPU Interpreter | ✅ Complete | 75+ tests |
| SPU Interpreter | ✅ Complete | 14+ tests |
| RSX State | ✅ Complete | 36+ tests |
| LV2 Kernel | ⚡ Partial | Basic tests |
| HLE Modules | ⚡ Partial | Function stubs |
| Audio | ⚡ Partial | Basic tests |
| Input | ⚡ Partial | Structure only |
| VFS | ⚡ Partial | ISO/PKG parsing |
| Loader | ⚡ Partial | ELF parsing |
| Integration | ✅ Complete | 4+ tests |
| UI | ✅ Complete | Manual testing |

**Legend:** ✅ Complete | ⚡ Partial | ❌ Not Started

---

## 🎮 Testing Milestones

1. **Milestone 1**: Load and display PARAM.SFO game info ⚡
2. **Milestone 2**: Execute homebrew ELF to first syscall ⚡
3. **Milestone 3**: Run PSL1GHT samples with graphics ❌
4. **Milestone 4**: Boot commercial game to menu ❌
5. **Milestone 5**: Playable commercial game ❌

---

## 🤝 Contributing

When working on tasks:

1. Follow existing code patterns in each crate
2. Add tests for new functionality
3. Update documentation as needed
4. Run `cargo test` and `cargo clippy` before submitting
5. Use `rustfmt` for Rust code and `clang-format` for C++

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.
