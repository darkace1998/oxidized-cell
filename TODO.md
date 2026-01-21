# 📋 Oxidized-Cell TODO List

This document tracks pending tasks, improvements, and future features for the oxidized-cell PS3 emulator.

---

## 🔴 High Priority

### CPU & Execution Core

#### PPU Interpreter Improvements

- [ ] **Complete 64-bit Instruction Set**: Add missing doubleword operations
  - `mulld`, `divd`, `divdu` - 64-bit multiply/divide (partial)
  - `rldic`, `rldicl`, `rldicr` - 64-bit rotate operations
  - `rldimi`, `rldcl`, `rldcr` - 64-bit rotate and mask insert
  - `srad`, `sradi` - Shift right algebraic doubleword
  - Location: `crates/oc-ppu/src/instructions/integer.rs`, `crates/oc-ppu/src/decoder.rs`

- [ ] **VMX/AltiVec Completion**: Implement remaining vector instructions
  - **Byte/Halfword Operations**: `vaddubm`, `vadduhm`, `vsububm`, `vsubuhm` (modulo variants)
  - **Pack Operations**: `vpkswss`, `vpkshss`, `vpkshus` (signed to smaller with saturation)
  - **Unpack Operations**: `vupkhsb`, `vupklsb`, `vupkhsh`, `vupklsh` (sign-extend expand)
  - **Multiply High**: `vmulhuw`, `vmulhsw` (high 32-bits of 64-bit product)
  - **Sum Across**: `vsum4ubs`, `vsum4sbs`, `vsum4shs`, `vsum2sws`, `vsumsws`
  - **Average**: `vavgub`, `vavguh`, `vavguw`, `vavgsb`, `vavgsh`, `vavgsw`
  - **Min/Max Integer**: `vminub`, `vminuh`, `vminuw`, `vmaxub`, `vmaxuh`, `vmaxuw`
  - **Reciprocal/RSQRT**: `vrsqrtefp` (reciprocal square root estimate)
  - Location: `crates/oc-ppu/src/instructions/vector.rs`, `crates/oc-ppu/src/vmx.rs`

- [ ] **FPSCR Full Accuracy**: Complete floating-point exception handling
  - Enable exception bits (`VE`, `OE`, `UE`, `ZE`, `XE`) for trapping
  - Implement `mcrfs` (Move to CR from FPSCR)
  - Full FPRF (Floating-Point Result Flags) update for all FP ops
  - Denormalized number handling per IEEE 754
  - Location: `crates/oc-ppu/src/instructions/float.rs`

- [ ] **System Instruction Stubs**: Implement missing SPR handling
  - `mftb`, `mftbu` - Move from Time Base (currently approximate)
  - Accurate decrementer (`DEC`) handling for timed operations
  - `mtmsr`, `mfmsr` - Machine State Register (for privilege level)
  - Location: `crates/oc-ppu/src/instructions/system.rs`

#### PPU JIT Compilation

- [ ] **PPU JIT Instruction Coverage**: Extend LLVM IR generation for remaining PowerPC instructions
  - Branch instructions with link register handling
  - VMX/AltiVec SIMD instructions (128-bit vectors)
  - All floating-point edge cases and FPSCR flag handling
  - Location: `cpp/src/ppu_jit.cpp`, `crates/oc-ppu/src/`

- [ ] **JIT Integer Instructions**: Add LLVM IR generation
  - `mullw`, `mulhw`, `mulhwu` - Multiply word
  - `divw`, `divwu` - Divide word
  - `rlwinm`, `rlwimi`, `rlwnm` - Rotate and mask
  - `cntlzw`, `cntlzd` - Count leading zeros
  - `extsb`, `extsh`, `extsw` - Sign extension
  - Location: `cpp/src/ppu_jit.cpp`

- [ ] **JIT Branch Instructions**: Complete branch compilation
  - `bc`, `bca`, `bcl`, `bcla` - Conditional branch with CTR
  - `bclr`, `bclrl` - Branch to LR
  - `bcctr`, `bcctrl` - Branch to CTR
  - Link register save/restore for function calls
  - Location: `cpp/src/ppu_jit.cpp`

- [ ] **JIT Load/Store Instructions**: Implement memory access IR
  - `lhz`, `lha`, `sth` - Halfword operations
  - `ld`, `std` - Doubleword operations
  - `lmw`, `stmw` - Multiple word operations
  - Update forms (`lwzu`, `stwu`, etc.)
  - Location: `cpp/src/ppu_jit.cpp`

- [ ] **JIT VMX Instructions**: Add vector operation compilation
  - `vaddfp`, `vsubfp`, `vmaddfp` - Vector float arithmetic
  - `vand`, `vor`, `vxor`, `vnor` - Vector logical
  - `vperm`, `vsel` - Vector permute/select
  - `vcmpequw`, `vcmpgtsw` - Vector compare
  - Location: `cpp/src/ppu_jit.cpp`

- [ ] **SPU JIT Instruction Coverage**: Complete SPU SIMD instruction compilation
  - Memory Flow Controller (MFC) DMA operations
  - Channel communication instructions
  - All vector operation variants
  - Location: `cpp/src/spu_jit.cpp`, `crates/oc-spu/src/`

- [ ] **Cross-Block Optimization**: Implement interprocedural JIT optimization
  - Currently each basic block is compiled independently
  - Add function-level optimization
  - Location: `cpp/src/ppu_jit.cpp`, `cpp/src/spu_jit.cpp`

### Graphics (RSX)

- [ ] **Complete NV4097 Method Handlers**: Implement remaining RSX draw commands
  - Handle unknown/unimplemented methods (see `crates/oc-rsx/src/methods.rs:590`)
  - Add more texture format support
  - Location: `crates/oc-rsx/src/methods.rs`

- [ ] **Shader Compilation Improvements**: Enhance RSX shader handling
  - Complete fragment program decoder
  - Handle all vertex program instructions
  - Improve SPIR-V generation for edge cases
  - Location: `crates/oc-rsx/src/shader/`

- [ ] **Vulkan Backend Enhancements**: Complete Vulkan graphics implementation
  - Multi-sample anti-aliasing (MSAA)
  - More texture compression formats
  - Compute shader support for RSX emulation
  - Location: `crates/oc-rsx/src/backend/vulkan.rs`

### Game Loading & Compatibility

- [ ] **Game Loading Pipeline**: Complete the game loading workflow
  - Improve ELF/SELF loading reliability
  - Better PRX shared library handling
  - Enhanced NID symbol resolution
  - Location: `crates/oc-loader/src/`, `crates/oc-integration/src/loader.rs`

- [ ] **Firmware Installation**: Improve firmware extraction and key handling
  - Better error messages for missing firmware
  - Automatic key extraction from PS3UPDAT.PUP
  - Location: `crates/oc-loader/src/firmware.rs`

---

## 🟡 Medium Priority

### HLE Module Improvements

- [ ] **Global Manager Instances**: Fix TODO markers for HLE module managers
  - `cell_dmux.rs`: Implement global demuxer manager instance (see `cellDmuxOpen` function)
  - `cell_vpost.rs`: Use global video post-processor manager (see `cell_vpost_close` function)
  - `libsre.rs`: Use global regex manager instance (see regex tests)
  - Location: `crates/oc-hle/src/`

- [ ] **HLE Edge Cases**: Handle remaining edge cases in HLE modules
  - Look for TODO comments in source code
  - Add unit tests for edge cases
  - Location: `crates/oc-hle/src/`

### Debugging & Development Tools

- [ ] **PPU Debugger Enhancements**: Improve debugging experience
  - Step-over and step-out functionality
  - Watch expressions
  - Call stack visualization
  - Location: `crates/oc-debug/src/ppu_debugger.rs`

- [ ] **SPU Debugger**: Add SPU-specific debugging features
  - Local storage viewer
  - Channel state inspection
  - DMA queue visualization
  - Location: `crates/oc-debug/src/spu_debugger.rs`

- [ ] **RSX Debugger**: Improve graphics debugging
  - Render target inspection
  - Command buffer visualization
  - Shader debugging with step-through
  - Location: `crates/oc-debug/src/rsx_debugger.rs`

- [ ] **Profiler Integration**: Expand performance profiling
  - Per-frame timing breakdown
  - Hot path identification
  - Memory access patterns
  - Location: `crates/oc-debug/src/profiler.rs`

### Audio System

- [ ] **Audio Timing Accuracy**: Improve audio synchronization
  - Better sample rate conversion
  - Time stretching for speed variations
  - S/PDIF passthrough handling
  - Location: `crates/oc-audio/src/`

- [ ] **Audio Codec Accuracy**: Improve decoder implementations
  - Complete ATRAC3+ implementation
  - WMA codec improvements
  - AC3 surround sound support
  - Location: `crates/oc-audio/src/codec.rs`

### Input System

- [ ] **Native DualShock 3 Support**: Add real PS3 controller support
  - USB connection handling
  - Bluetooth pairing
  - Sixaxis motion sensor passthrough
  - Location: `crates/oc-input/src/dualshock3.rs`

- [ ] **Move Controller Support**: Complete PlayStation Move implementation
  - Camera tracking
  - Position calculation
  - Location: `crates/oc-input/src/move_controller.rs`

- [ ] **Instruments Support**: Complete special controller support
  - Guitar Hero controllers
  - Rock Band drum kits
  - Location: `crates/oc-input/src/instruments.rs`

---

## 🟢 Lower Priority

### User Interface

- [ ] **Theme Customization**: Add user-selectable UI themes
  - Dark/light mode toggle
  - Custom color schemes
  - Location: `crates/oc-ui/src/themes.rs`

- [ ] **Controller Configuration UI**: Improve controller mapping interface
  - Visual controller diagram
  - Profile save/load
  - Location: `crates/oc-ui/src/controller_config.rs`

- [ ] **Game Icons**: Display game icons in game list
  - Extract ICON0.PNG from PARAM.SFO
  - Cache icons for performance
  - Location: `crates/oc-ui/src/game_list.rs`

- [ ] **Localization**: Add multi-language support
  - UI string externalization
  - Translation files
  - Location: `crates/oc-ui/src/`

### Virtual File System

- [ ] **Network Mounts**: Add network file system support
  - SMB/CIFS shares
  - FTP access
  - Location: `crates/oc-vfs/src/`

- [ ] **File Caching**: Improve VFS performance
  - Read-ahead caching
  - Lazy loading for large files
  - Location: `crates/oc-vfs/src/`

### Testing & CI

- [ ] **PPU Instruction Tests**: Expand test coverage for CPU instructions
  - **64-bit Operations**: Tests for `mulld`, `divd`, `rldic`, `srad`
  - **VMX Edge Cases**: NaN handling, denormal numbers, saturation boundaries
  - **Atomic Operations**: Multi-threaded `lwarx`/`stwcx.` stress tests
  - **FPSCR Flags**: Verify all exception bits set correctly
  - Location: `crates/oc-ppu/src/tests/`, `crates/oc-ppu/src/interpreter.rs`

- [ ] **Integration Tests**: Add game-level integration tests
  - Homebrew test suite
  - Known-working game tests
  - Location: `crates/oc-integration/`

- [ ] **Performance Benchmarks**: Create performance regression tests
  - Instruction execution speed
  - Graphics rendering benchmarks
  - Memory throughput tests
  - Location: `benches/`

- [ ] **CI/CD Pipeline**: Enhance automated testing
  - Cross-platform builds (Linux, Windows, macOS)
  - Automated release builds
  - Location: `.github/workflows/` (new directory and files to be created)

### Documentation

- [ ] **API Documentation**: Improve inline documentation
  - Complete rustdoc for all public APIs
  - Example code snippets
  - Location: All `src/` files

- [ ] **Architecture Guide**: Create developer architecture documentation
  - System interaction diagrams
  - Data flow documentation
  - Location: `docs/` (new file to be created)

- [ ] **Contributing Guide**: Create CONTRIBUTING.md
  - Development setup instructions
  - Code style guidelines
  - Pull request process
  - Location: Root directory (new file to be created)

---

## 🔧 Technical Debt

### Code Quality

- [ ] **Error Handling Consistency**: Standardize error types across crates
  - Migrate to consistent error enums
  - Improve error messages
  - Location: All `error.rs` files

- [ ] **Logging Standardization**: Ensure consistent logging practices
  - Use appropriate log levels
  - Add structured logging where needed
  - Location: All source files

- [ ] **Code Deduplication**: Reduce duplicate code
  - Shared utilities extraction
  - Common pattern abstractions
  - Location: Various

### Performance

- [ ] **Memory Allocation Optimization**: Reduce heap allocations
  - Use arena allocators where appropriate
  - Pool frequently allocated objects
  - Location: `crates/oc-memory/src/`

- [ ] **Lock Contention Reduction**: Optimize multi-threaded access
  - Fine-grained locking strategies
  - Lock-free data structures where possible
  - Location: Various threading code

- [ ] **SIMD Optimization**: Expand SIMD usage in hot paths
  - AVX2/AVX-512 for x86-64
  - NEON for ARM
  - Location: `cpp/src/simd_avx.cpp`

### FFI Layer

- [ ] **FFI Safety Audit**: Review FFI boundary safety
  - Ensure proper null pointer handling
  - Validate all C++ <-> Rust data transfers
  - Location: `crates/oc-ffi/src/`

- [ ] **C++ Code Modernization**: Update C++ code standards
  - Use C++20 features consistently
  - Smart pointer usage
  - Location: `cpp/src/`

---

## 💡 Future Enhancements

### Advanced Features

- [ ] **Save State Support**: Implement full save state functionality
  - Serialize complete emulator state
  - Fast resume capability

- [ ] **Rewind Feature**: Add rewind-time functionality
  - Ring buffer for state snapshots
  - Configurable rewind length

- [ ] **Netplay Support**: Implement network multiplayer
  - P2P connection handling
  - Input synchronization
  - Rollback netcode

- [ ] **Record & Replay**: Add TAS (Tool-Assisted) features
  - Input recording
  - Frame-perfect replay
  - Movie file format

### Platform Support

- [ ] **ARM64 Support**: Optimize for Apple Silicon and ARM servers
  - ARM-specific JIT backend
  - NEON SIMD optimizations

- [ ] **Steamdeck Optimization**: Optimize for handheld PC gaming
  - Power-efficient modes
  - Controller integration

- [ ] **Android Port**: Mobile platform support
  - Touch controls
  - Android-specific backends

---

## 📊 Progress Tracking

| Category | Complete | In Progress | Not Started |
|----------|----------|-------------|-------------|
| HLE Modules | ✅ All 25 | - | - |
| PPU Instructions | ~80% | ~15% | ~5% |
| SPU Instructions | ~70% | ~20% | ~10% |
| RSX Methods | ~60% | ~30% | ~10% |
| JIT Compilation | ~30% | ~20% | ~50% |
| Input Devices | ~50% | ~30% | ~20% |

### PPU Instruction Coverage Details

| Instruction Category | Status | Notes |
|----------------------|--------|-------|
| Integer Arithmetic (32-bit) | ✅ Complete | `add`, `subf`, `mullw`, `divw`, etc. |
| Integer Arithmetic (64-bit) | 🟡 Partial | Basic ops done, rotate/mask need work |
| Integer Logical | ✅ Complete | `and`, `or`, `xor`, `nand`, `nor`, `eqv` |
| Shift/Rotate (32-bit) | ✅ Complete | `slw`, `srw`, `sraw`, `rlwinm`, `rlwimi` |
| Shift/Rotate (64-bit) | 🟡 Partial | `sld`, `srd` done; `rldic`, `rldimi` needed |
| Branch Instructions | ✅ Complete | All branch forms implemented |
| Load/Store (Basic) | ✅ Complete | All sizes, indexed, update forms |
| Load/Store (Atomic) | ✅ Complete | `lwarx`, `stwcx.`, `ldarx`, `stdcx.` |
| Floating-Point Arithmetic | ✅ Complete | All basic ops with single/double |
| Floating-Point FMA | ✅ Complete | `fmadd`, `fmsub`, `fnmadd`, `fnmsub` |
| Floating-Point Convert | ✅ Complete | All integer <-> float conversions |
| FPSCR Handling | 🟡 Partial | Basic flags done, exception trapping incomplete |
| VMX Integer Add/Sub | 🟡 Partial | Saturating done, modulo variants partial |
| VMX Logical | ✅ Complete | `vand`, `vor`, `vxor`, `vnor`, `vsel` |
| VMX Float | 🟡 Partial | Basic ops done, estimates incomplete |
| VMX Pack/Unpack | 🟡 Partial | Basic pack done, signed variants needed |
| VMX Compare | 🟡 Partial | Basic compare done, Rc forms incomplete |
| VMX Permute | ✅ Complete | `vperm`, `vsplt*`, `vmrgh*`, `vmrgl*` |
| System Instructions | ✅ Complete | SPR access, sync, cache hints |
| JIT Integer | 🟡 Partial | Basic arithmetic in LLVM IR |
| JIT Branch | 🔴 Minimal | Only unconditional branches |
| JIT Load/Store | 🟡 Partial | `lwz`, `stw` done; others needed |
| JIT Floating-Point | 🟡 Partial | Basic ops; FMA needs completion |
| JIT VMX | 🔴 Minimal | Framework exists, few instructions |

---

## 📝 Notes

- Refer to `docs/HLE_STATUS.md` for detailed HLE module status
- Check `docs/ppu_instructions.md` and `docs/spu_instructions.md` for instruction coverage
- Build with `cargo build --release` and test with `cargo test`
- Run `RUST_LOG=debug cargo run` for detailed logging

---

*Last updated: 2026-01-21*
