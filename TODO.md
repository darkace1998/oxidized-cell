# oxidized-cell TODO

This document outlines the development roadmap and remaining work for the oxidized-cell PS3 emulator.

## 🎯 Project Status: Late Development

The emulator has all critical subsystems implemented and wired together. RSX bridge, SPU bridge, VFS integration, and input backend are all connected. The primary focus now should be on **integration testing** with homebrew samples and addressing any remaining edge cases to enable game execution.

---

## 🔥 High Priority (Critical for Game Execution)

### HLE Module Completion
The HLE (High Level Emulation) modules are essential for games to run. Memory subsystem integration is mostly complete.

- [x] **cellGcmSys** - Memory read/write operations implemented (~95%) ✅
  - ✅ `cell_gcm_set_vertex_program()` - Reads program descriptor, validates address
  - ✅ `cell_gcm_set_fragment_program()` - Reads program descriptor, validates address
  - ✅ `cell_gcm_get_configuration()` - Writes config to memory
  - ✅ `cell_gcm_address_to_offset()` - Writes offset to memory
  - ✅ `cell_gcm_map_main_memory()` - Maps memory, returns offset
  - ✅ **GcmManager wired to RSX thread via RsxBridge**
    - Bridge created in `EmulatorRunner::new()` with `create_rsx_bridge()`
    - Commands sent via `flush_commands()` to RSX thread
    - Flips and display buffer config forwarded to RSX

- [x] **cellSpurs** - SPURS task scheduler (~80% complete) ✅
  - ✅ `cell_spurs_set_priorities()` - Sets workload priorities for 8 SPUs
  - ✅ `cell_spurs_get_spu_thread_id()` - Returns simulated SPU thread ID
  - ✅ SPU thread group creation through SPU bridge
  - ✅ **SPU bridge connects SPURS to SPU interpreter**
    - Bridge created in `EmulatorRunner::new()` with `create_spu_bridge()`
    - Workloads submitted via `SpuBridgeMessage::SubmitWorkload`
    - Thread groups managed via CreateGroup/StartGroup/StopGroup messages
    - DMA transfers, signals, and mailbox operations supported

- [x] **cellSysutil** - System utilities (~85% complete) ✅
  - ✅ `cell_sysutil_get_system_param_int()` - Writes value to memory
  - ✅ `cell_sysutil_get_system_param_string()` - Writes string to memory
  - ✅ `cell_sysutil_get_ps_id()` - Writes PSID to memory
  - ✅ Dialog callbacks now invoke registered handlers
    - Events queue pending callbacks for all registered slots
    - `pop_sysutil_callback()` returns callbacks to invoke on PPU
    - Dialog close automatically queues MenuClose event

- [x] **cellFs** - File system operations (~85% complete) ✅
  - ✅ File descriptor management, path validation
  - ✅ **VFS integration complete** - `set_vfs()` connects to oc-vfs layer
  - ✅ File read/write uses real I/O through VFS path resolution
  - ✅ Directory operations (opendir/readdir/closedir) work with VFS
  - ✅ mkdir, rmdir, unlink, truncate use VFS
  - ✅ stat/fstat return real metadata when VFS connected

- [x] **cellPad** - Controller input (~95% complete) ✅
  - ✅ Full button/analog/sensor data structures
  - ✅ Manual state updates work
  - ✅ **oc-input connected for actual controller polling**
    - DualShock3Manager created in EmulatorRunner
    - Input backend wired via `set_input_backend()`
    - `poll_input()` called each frame to update pad state
    - Conversion from oc-input format to PS3 pad data

### Game Loading Pipeline

- [x] **SELF Decryption** - ~95% Complete ✅
  - ✅ SELF header parsing (SCE header, extended header, app info)
  - ✅ AES-128/256-CBC and AES-128-CTR decryption
  - ✅ Key lookup by type/revision (matches RPCS3 approach)
  - ✅ Metadata decryption and section decryption
  - ✅ Zlib decompression for compressed sections
  - ✅ Firmware key loading from files

- [x] **PRX Loading** - ~85% Complete ✅
  - ✅ NID-based symbol resolution with database (~11 known NIDs)
  - ✅ Symbol cache for resolved NIDs
  - ✅ Stub library creation for unresolved imports
  - ✅ Import resolution with fallback to stubs

- [x] **Memory Mapping Integration** - Complete ✅
  - ✅ HLE modules connected to MemoryManager
  - ✅ Segment loading uses memory manager
  - ✅ Relocation processing implemented

### RSX/Graphics Integration

- [x] **Connect cellGcmSys to oc-rsx** - ✅ Complete
  - ✅ RSX bridge module in oc-core for decoupled communication
  - ✅ Command buffer submissions routed to RSX thread via bridge
  - ✅ Display buffer configuration sent to RSX
  - ✅ Flip synchronization with status feedback to GCM
  - ✅ Bridge wired in EmulatorRunner initialization

- [x] **RSX Shader Compilation** - ~95% Complete ✅
  - ✅ VP/FP instruction decoders (128-bit format, half-word swap for FP)
  - ✅ SPIR-V code generator with proper section ordering
  - ✅ Most vector/scalar VP opcodes: MOV, MUL, ADD, MAD, DP3/DP4, MIN/MAX, FRC/FLR
  - ✅ Scalar ops: RCP, RSQ, EXP, LOG, SIN, COS, EX2, LG2
  - ✅ FP opcodes: MOV, MUL, ADD, MAD, DP3/DP4, MIN/MAX, FRC/FLR, RCP, RSQ, SIN/COS, POW, LRP, NRM
  - ✅ Texture sampling: TEX, TXP (projective), TXL (explicit LOD), TXB (bias)
  - ✅ Vertex program constants (512 vec4 registers) connected to RSX state
  - ✅ Shader translator with caching
  - ✅ VulkanBackend: compile_vertex_program(), compile_fragment_program()
  - ✅ Graphics pipeline creation with compiled shaders
  - ✅ NV4097 method handlers for transform constants and texture state

- [x] **Vulkan Backend Completion** - ~90% Complete ✅
  - ✅ Device creation, command pools, synchronization primitives
  - ✅ Render targets, depth buffers, MSAA (1-64x), MRT up to 4 targets
  - ✅ 40+ NV4097 method handlers implemented
  - ✅ 35+ texture formats supported
  - ✅ Shader module creation and graphics pipeline
  - ✅ Descriptor set layout for 16 texture samplers
  - ✅ Descriptor pool and per-frame descriptor sets
  - ✅ Texture upload with staging buffer and layout transitions
  - ✅ Combined image sampler binding for fragment shaders

---

## 📌 Medium Priority (Feature Completeness)

### PPU Enhancements

- [x] **JIT Compiler Integration** - ~80% Complete ✅
  - ✅ C++ JIT exists (1300+ lines with LLVM IR generation)
  - ✅ FFI bridge declared in oc-ffi
  - ✅ Rust interpreter connected to JIT via `PpuJitCompiler`
  - ✅ JIT/Interpreter hybrid mode with lazy compilation
  - ✅ Hot block detection and automatic compilation
  - ✅ Branch prediction recording for JIT optimization
  - ✅ **JIT execution bridge implemented** - PpuContext struct, execute() FFI, context conversion

- [x] **Instruction Set** - ~98% Complete ✅
  - ✅ All major instruction forms: D, DS, I, B, X, XO, XL, M, MD, MDS, A, VA, SC
  - ✅ Integer, load/store, branch, rotate/mask, CR, floating-point
  - ✅ VMX/AltiVec comprehensive (70+ vector instructions)
    - VA-form: vperm, vmaddfp, vnmsubfp, vsel, vsldoi
    - VX-form: add/sub (byte/half/word, signed/unsigned, modulo/saturate)
    - Logical: vand, vandc, vor, vnor, vxor
    - Shift/rotate: vslw, vsrw, vsraw, vrlw
    - Compare: vcmpequw, vcmpgtsw, vcmpgtuw, vcmpeqfp, vcmpgtfp, vcmpgefp, vcmpbfp
    - FP: vaddfp, vsubfp, vmaddfp, vnmsubfp, vrefp, vrsqrtefp, vlogefp, vexptefp, vmaxfp, vminfp
    - Convert: vctsxs, vcfsx, vctuxs, vcfux
    - Splat: vspltisb/h/w, vspltb/h/w
    - Merge: vmrghb/h/w, vmrglb/h/w
    - Pack/unpack: vpkuwus, vpkshss, vpkswss, vupkhsb, vupklsb
    - Multiply: vmuleuw, vmulouw, vmulhuw, vmulesb/ub/sh/uh, vmulosb/ob/sh/oh
    - Average: vavgub, vavguh, vavguw, vavgsb, vavgsh, vavgsw
    - Load/store: lvsl, lvsr, lvx, stvx
  - ✅ SPR handling improved - SPRG0-3, SRR0/1, DEC, HID0-6 supported

- [x] **Debugging Features** - Mostly Complete ✅
  - ✅ Breakpoints (unconditional + conditional with hit counts)
  - ✅ Instruction tracing and step execution
  - ✅ Register state inspection
  - ❌ Memory watchpoints not implemented

### SPU Enhancements

- [x] **DMA Operations** - Complete ✅
  - ✅ All DMA commands (Put/Get/PutB/GetB/PutF/GetF variants)
  - ✅ Atomic reservations (GetLLAR/PutLLC/PutLLUC)
  - ✅ Tag completion tracking and timing simulation
  - ✅ 650+ lines of MFC implementation

- [x] **Channel Operations** - Complete ✅
  - ✅ 32 channels implemented (670+ lines)
  - ✅ SPU-PPU mailbox communication
  - ✅ Event mask/status, decrementer, timeouts
  - ✅ Signal notification channels

- [ ] **JIT Integration**
  - ✅ C++ JIT exists (1000+ lines with LLVM, SIMD intrinsics)
  - ✅ FFI bridge declared with channel ops and MFC DMA APIs
  - ❌ **TODO: Connect C++ SPU JIT to Rust interpreter**

### Audio System

- [x] **cellAudio Full Implementation** - Complete ✅
  - ✅ Multi-port audio mixing (8 ports, stereo/5.1/7.1)
  - ✅ Sample rate conversion (3 quality levels)
  - ✅ Audio buffer management, per-source volume, clipping prevention

- [ ] **Codec Support** - Framework Only
  - ⚠️ cellAdec: Manager exists, decode functions simulate but don't decode
  - ❌ AAC decoder returns silence (needs symphonia/ffmpeg)
  - ❌ AT3/AT3+ decoder returns silence (Sony proprietary)
  - ❌ cellVdec not implemented
  - ⚠️ cellDmux: Container parser framework, returns simulated AU info

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

| Component | Status | Test Coverage | Notes |
|-----------|--------|---------------|-------|
| Memory Manager | ✅ Complete | 128+ tests | Fully working |
| PPU Interpreter | ✅ Complete | 75+ tests | ~98% instructions, VMX comprehensive |
| SPU Interpreter | ✅ Complete | 14+ tests | DMA + channels complete |
| RSX State | ✅ Complete | 36+ tests | 40+ NV4097 methods |
| Vulkan Backend | ✅ 90% | - | Shaders, textures, pipelines working |
| LV2 Kernel | ⚡ Partial | Basic tests | - |
| HLE Modules | ✅ 85% | Function stubs | All critical modules wired to backends |
| Audio | ✅ 85% | Basic tests | Core complete, codecs stubbed |
| Input | ✅ 95% | Structure only | DualShock3Manager connected |
| VFS | ⚡ Partial | ISO/PKG parsing | - |
| Loader | ✅ 90% | ELF parsing | SELF decrypt + PRX loading work |
| Integration | ✅ Complete | 4+ tests | All bridges wired |
| UI | ✅ Complete | Manual testing | - |

**Legend:** ✅ Complete | ⚡ Partial | ❌ Not Started

---

## 🎮 Testing Milestones

1. **Milestone 1**: Load and display PARAM.SFO game info ✅ (loader + VFS parsing implemented)
2. **Milestone 2**: Execute homebrew ELF to first syscall ✅ (SELF decrypt + PPU interpreter + all bridges wired)
3. **Milestone 3**: Run PSL1GHT samples with graphics ⚡ (ready for testing - all subsystems connected)
4. **Milestone 4**: Boot commercial game to menu ❌ (pending integration testing)
5. **Milestone 5**: Playable commercial game ❌

### 🚧 Critical Blockers for Game Execution

1. ~~**cellGcmSys → RSX Connection**~~ ✅ RESOLVED - Bridge module wires GCM to RSX
2. ~~**RSX Shader Compilation**~~ ✅ RESOLVED - VP/FP decoders, SPIR-V generator (~95% complete)
3. ~~**cellFs → VFS Connection**~~ ✅ RESOLVED - FsManager wired to VFS via `set_vfs()`
4. ~~**cellSpurs → SPU Connection**~~ ✅ RESOLVED - SPU bridge connects SPURS to interpreter
5. ~~**cellPad → Input Connection**~~ ✅ RESOLVED - DualShock3Manager wired via `set_input_backend()`

**All critical blockers resolved!** Ready for integration testing.

---

## 🤝 Contributing

When working on tasks:

1. Follow existing code patterns in each crate
2. Add tests for new functionality
3. Update documentation as needed
4. Run `cargo test` and `cargo clippy` before submitting
5. Use `rustfmt` for Rust code and `clang-format` for C++

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.
