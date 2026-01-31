# 📋 Oxidized-Cell TODO List

This document tracks pending tasks, improvements, and future features for the oxidized-cell PS3 emulator.

---

## 🔴 High Priority

### CPU & Execution Core

#### PPU Interpreter Improvements

- [x] **Complete 64-bit Instruction Set**: Add missing doubleword operations
  - `mulld`, `divd`, `divdu` - 64-bit multiply/divide ✅
  - `rldic`, `rldicl`, `rldicr` - 64-bit rotate operations ✅
  - `rldimi`, `rldcl`, `rldcr` - 64-bit rotate and mask insert ✅
  - `srad`, `sradi` - Shift right algebraic doubleword ✅
  - `srawi` - Shift right algebraic word immediate ✅
  - Location: `crates/oc-ppu/src/instructions/integer.rs`, `crates/oc-ppu/src/decoder.rs`

- [x] **VMX/AltiVec Completion**: Implement remaining vector instructions
  - **Byte/Halfword Operations**: `vaddubm`, `vadduhm`, `vsububm`, `vsubuhm` (modulo variants) ✅
  - **Pack Operations**: `vpkswss`, `vpkshss`, `vpkshus` (signed to smaller with saturation) ✅
  - **Unpack Operations**: `vupkhsb`, `vupklsb`, `vupkhsh`, `vupklsh` (sign-extend expand) ✅
  - **Multiply High**: `vmulhuw`, `vmulhsw` (high 32-bits of 64-bit product) ✅
  - **Sum Across**: `vsum4ubs`, `vsum4sbs`, `vsum4shs`, `vsum2sws`, `vsumsws` ✅
  - **Average**: `vavgub`, `vavguh`, `vavguw`, `vavgsb`, `vavgsh`, `vavgsw` ✅
  - **Min/Max Integer**: `vminub`, `vminuh`, `vminuw`, `vmaxub`, `vmaxuh`, `vmaxuw`, `vminsb`, `vminsh`, `vmaxsb`, `vmaxsh` ✅
  - **Reciprocal/RSQRT**: `vrsqrtefp` (reciprocal square root estimate) ✅
  - Location: `crates/oc-ppu/src/instructions/vector.rs`, `crates/oc-ppu/src/vmx.rs`

- [x] **FPSCR Full Accuracy**: Complete floating-point exception handling
  - Enable exception bits (`VE`, `OE`, `UE`, `ZE`, `XE`) for trapping ✅
  - Implement `mcrfs` (Move to CR from FPSCR) ✅
  - Full FPRF (Floating-Point Result Flags) update for all FP ops ✅
  - Denormalized number handling per IEEE 754 ✅
  - FEX (Enabled Exception Summary) automatic update ✅
  - VX (Invalid Operation Summary) automatic update ✅
  - NI (Non-IEEE Mode) for denormals-are-zero behavior ✅
  - Location: `crates/oc-ppu/src/instructions/float.rs`

- [ ] **System Instruction Stubs**: Implement missing SPR handling
- [x] **System Instruction Stubs**: Implement missing SPR handling
  - `mftb`, `mftbu` - Move from Time Base ✅
  - Accurate decrementer (`DEC`) handling for timed operations ✅
  - `mtmsr`, `mfmsr` - Machine State Register (for privilege level) ✅
  - `mtmsrd` - Move To Machine State Register Doubleword ✅
  - MSR bit constants (SF, HV, VEC, EE, PR, FP, ME, etc.) ✅
  - Time base frequency constant (79.8 MHz for Cell BE) ✅
  - Decrementer update and interrupt checking ✅
  - Location: `crates/oc-ppu/src/instructions/system.rs`

#### PPU JIT Compilation

- [x] **PPU JIT Instruction Coverage**: Extend LLVM IR generation for remaining PowerPC instructions
  - Branch instructions with link register handling ✅
  - VMX/AltiVec SIMD instructions (128-bit vectors) ✅ (stub for interpreter fallback)
  - All floating-point edge cases and FPSCR flag handling ✅
  - Location: `cpp/src/ppu_jit.cpp`, `crates/oc-ppu/src/`

- [x] **JIT Integer Instructions**: Add LLVM IR generation
  - `mullw`, `mulhw`, `mulhwu` - Multiply word ✅
  - `divw`, `divwu` - Divide word ✅
  - `rlwinm`, `rlwimi`, `rlwnm` - Rotate and mask ✅
  - `cntlzw`, `cntlzd` - Count leading zeros ✅
  - `extsb`, `extsh`, `extsw` - Sign extension ✅
  - Location: `cpp/src/ppu_jit.cpp`

- [x] **JIT Branch Instructions**: Complete branch compilation
  - `bc`, `bca`, `bcl`, `bcla` - Conditional branch with CTR ✅
  - `bclr`, `bclrl` - Branch to LR ✅
  - `bcctr`, `bcctrl` - Branch to CTR ✅
  - Link register save/restore for function calls ✅
  - Location: `cpp/src/ppu_jit.cpp`

- [x] **JIT Load/Store Instructions**: Implement memory access IR
  - `lhz`, `lha`, `sth` - Halfword operations ✅
  - `ld`, `std` - Doubleword operations ✅
  - `lmw`, `stmw` - Multiple word operations ✅
  - Update forms (`lwzu`, `stwu`, etc.) ✅
  - Location: `cpp/src/ppu_jit.cpp`

- [x] **JIT VMX Instructions**: Add vector operation compilation ✅
  - `vaddfp`, `vsubfp`, `vmaddfp` - Vector float arithmetic ✅
  - `vand`, `vor`, `vxor`, `vnor` - Vector logical ✅
  - `vperm`, `vsel` - Vector permute/select ✅
  - `vcmpequw`, `vcmpgtsw` - Vector compare ✅
  - `vadduwm`, `vsubuwm` - Vector integer add/subtract ✅
  - `vnmsubfp` - Vector negative multiply-subtract ✅
  - 32 vector registers (VRs) with VSCR support ✅
  - Location: `cpp/src/ppu_jit.cpp`

- [x] **SPU JIT Instruction Coverage**: Complete SPU SIMD instruction compilation ✅
  - Memory Flow Controller (MFC) DMA operations (infrastructure exists via MfcDmaManager) ✅
  - Channel communication instructions (rdch, wrch, rchcnt) ✅
  - Quadword shift/rotate immediate forms (shlqbyi, rotqbyi, shlqbii, rotqbii) ✅
  - Float to integer conversions (cflts, cfltu, csflt, cuflt) ✅
  - Compare immediate halfword/byte (ceqhi, ceqbi, cgthi, cgtbi, clgthi, clgtbi) ✅
  - Sign extension (xsbh, xshw, xswd) ✅
  - Gather bits (gbb, gbh, gb) ✅
  - Average bytes, sum bytes ✅
  - Location: `cpp/src/spu_jit.cpp`, `crates/oc-spu/src/`

#### SPU Interpreter Improvements

- [x] **Double-Precision Floating-Point**: Complete f64 instruction coverage
  - `dfa`, `dfs`, `dfm` - Double-precision add/subtract/multiply
  - `dfma`, `dfms`, `dfnma`, `dfnms` - Double-precision FMA variants
  - `dfceq`, `dfcgt`, `dfcmeq`, `dfcmgt` - Double-precision comparisons
  - `fesd`, `frds` - Float to double / double to float conversion
  - Location: `crates/oc-spu/src/instructions/float.rs`

- [x] **Byte/Halfword Operations Completion**: Implement remaining element-wise ops
  - **Carry/Borrow**: `cg`, `bg`, `cgx`, `bgx` - Carry/borrow generation with extended ✅
  - **Extended Arithmetic**: `addx`, `sfx` - Add/subtract extended ✅
  - **Absolute Difference**: `absdb` - Absolute difference of bytes ✅
  - **Byte Sum**: `sumb` - Sum bytes into halfwords ✅
  - Location: `crates/oc-spu/src/instructions/arithmetic.rs`

- [x] **Hint and Scheduling Instructions**: Implement branch hints ✅
  - `hbra`, `hbrr` - Hint for branch (absolute/relative) ✅
  - `hbrp` - Hint for branch predict ✅
  - `nop`, `lnop` - No operation ✅
  - `sync`, `dsync` - Synchronize (data) ✅
  - `mfspr` - Move from special purpose register ✅
  - Location: `crates/oc-spu/src/instructions/hints.rs`

- [x] **Channel Blocking Behavior**: Implement proper stalling semantics ✅
  - `rdch` stalls when channel is empty (returns WouldBlock, not 0) ✅
  - `wrch` stalls when channel is full ✅
  - `BlockingBehavior` enum for tracking blocking state ✅
  - `ChannelContext` for save/restore of blocked operations ✅
  - `is_channel_stalled()` / `is_channel_write_stalled()` helpers ✅
  - `rdch_blocking()` / `wrch_blocking()` with automatic state management ✅
  - `save_channel_context()` / `restore_and_resume()` for context switching ✅
  - Location: `crates/oc-spu/src/instructions/channel.rs`, `crates/oc-spu/src/channels.rs`

- [x] **MFC List DMA Operations**: Complete DMA list transfer support ✅
  - `GETL`, `PUTL`, `GETLB`, `PUTLB`, `GETLF`, `PUTLF` - DMA list transfer commands ✅
  - `MfcListElement` with stall-and-notify flag parsing ✅
  - `ListTransferState` for tracking in-progress list transfers ✅
  - List stall handling via `MFC_RD_LIST_STALL` and `MFC_WR_LIST_STALL_ACK` channels ✅
  - `resume_list_transfer()` for resuming after stall acknowledgment ✅
  - Location: `crates/oc-spu/src/mfc.rs`, `crates/oc-spu/src/channels.rs`

#### SPU JIT Compilation

- [x] **JIT Arithmetic Instructions**: Add LLVM IR generation ✅
  - `a`, `ah`, `ai`, `ahi` - Word/halfword add ✅
  - `sf`, `sfh`, `sfi`, `sfhi` - Word/halfword subtract from ✅
  - `mpy`, `mpyu`, `mpyh` - Multiply variants ✅
  - `addx`, `sfx` - Extended add/subtract with carry/borrow ✅
  - `cg`, `bg`, `cgx`, `bgx` - Carry/borrow generate ✅
  - `absdb` - Absolute difference of bytes ✅
  - Location: `cpp/src/spu_jit.cpp`

- [x] **JIT Shift/Rotate Instructions**: Complete shift compilation ✅
  - `shl`, `shli` - Shift left word (register and immediate) ✅
  - `shlh`, `shlhi` - Shift left halfword (register and immediate) ✅
  - `rot`, `roti` - Rotate word (register and immediate) ✅
  - `roth`, `rothi` - Rotate halfword (register and immediate) ✅
  - `rotm`, `rotmi` - Rotate and mask word (right shift logical) ✅
  - `rothm`, `rotmhi` - Rotate and mask halfword (right shift logical) ✅
  - `rotma`, `rotmai` - Rotate and mask algebraic word (right shift arithmetic) ✅
  - `rotmah`, `rotmahi` - Rotate and mask algebraic halfword (right shift arithmetic) ✅
  - Fixed incorrect opcodes for rotm/rotma (0x59/0x5a per RPCS3)
  - Location: `cpp/src/spu_jit.cpp`

- [x] **JIT Quadword Operations**: Compile 128-bit operations
  - `shlqby`, `shlqbyi`, `shlqbi`, `shlqbii` - Quadword shift left
  - `rotqby`, `rotqbyi`, `rotqbi`, `rotqbii` - Quadword rotate
  - `rotqmby`, `rotqmbyi`, `rotqmbi`, `rotqmbii`, `rotqmbybi` - Quadword rotate and mask (right shift)
  - Location: `cpp/src/spu_jit.cpp`

- [x] **JIT Memory Operations**: Implement load/store IR ✅
  - `lqd`, `lqa`, `lqr`, `lqx` - Load quadword variants ✅
  - `stqd`, `stqa`, `stqr`, `stqx` - Store quadword variants ✅
  - Proper 16-byte alignment handling ✅
  - Location: `cpp/src/spu_jit.cpp`

- [x] **JIT Channel Operations**: Compile channel I/O ✅
  - `rdch`, `wrch`, `rchcnt` - Channel read/write/count with runtime callbacks ✅
  - Blocking behavior with fallback via rchcnt check before operations ✅
  - MFC command channel (channel 25) handling via write callback ✅
  - Added `ChannelCountFunc` type and `count_callback` to ChannelManager ✅
  - Location: `cpp/src/spu_jit.cpp`

- [x] **JIT Compare Instructions**: Add comparison IR ✅
  - `ceq`, `ceqb`, `ceqh`, `ceqi`, `ceqbi`, `ceqhi` - Compare equal ✅
  - `cgt`, `cgtb`, `cgth`, `cgti`, `cgtbi`, `cgthi` - Compare greater than ✅
  - `clgt`, `clgtb`, `clgth`, `clgti`, `clgtbi`, `clgthi` - Compare logical greater than ✅
  - Location: `cpp/src/spu_jit.cpp`

- [ ] **JIT Floating-Point Instructions**: Complete FP compilation
  - `fa`, `fs`, `fm` - Basic float arithmetic
  - `fma`, `fms`, `fnms` - Fused multiply-add variants
  - `frest`, `frsqest` - Reciprocal estimates
  - `fi` - Floating interpolate
  - Location: `cpp/src/spu_jit.cpp`

- [ ] **Cross-Block Optimization**: Implement interprocedural JIT optimization
  - Currently each basic block is compiled independently
  - Add function-level optimization
  - Location: `cpp/src/ppu_jit.cpp`, `cpp/src/spu_jit.cpp`

#### JIT Infrastructure

- [ ] **LLVM Backend Improvements**: Complete LLVM integration
  - **ORC JIT v2**: Migrate from MCJIT to ORC for better performance
  - **Target Machine Configuration**: Optimize for host CPU features (AVX2, AVX-512)
  - **Module Management**: Proper module ownership and memory management
  - **Error Handling**: Comprehensive LLVM error propagation
  - Location: `cpp/src/ppu_jit.cpp`, `cpp/src/spu_jit.cpp`

- [ ] **Code Cache Management**: Improve compiled code storage
  - **LRU Eviction**: Implement least-recently-used cache eviction
  - **Size Limits**: Configurable cache size limits per processor type
  - **Invalidation**: Proper code invalidation on self-modifying code
  - **Statistics**: Cache hit/miss tracking for profiling
  - Location: `cpp/src/ppu_jit.cpp` (CodeCache struct)

- [ ] **Basic Block Detection**: Improve block boundary identification
  - **PPU**: Handle complex branch patterns (indirect, conditional)
  - **SPU**: Detect all branch types (br, bra, brsl, bi, bisl, brnz/brz)
  - **Block Merging**: Merge consecutive blocks for better optimization
  - Location: `cpp/src/ppu_jit.cpp`, `cpp/src/spu_jit.cpp`

#### Branch Prediction

- [ ] **Branch Prediction Enhancement**: Complete prediction infrastructure
  - **Hint Types**: Likely, Unlikely, Static (backward=taken)
  - **Runtime Updates**: Update predictions based on execution history
  - **Threshold Tuning**: Configurable taken/not-taken thresholds
  - **Prediction Stats**: Track prediction accuracy per branch
  - Location: `cpp/src/ppu_jit.cpp` (BranchPredictor struct)

- [ ] **Branch Target Cache**: Implement indirect branch optimization
  - **BTB (Branch Target Buffer)**: Cache indirect branch targets
  - **Polymorphic Inline Cache**: Multiple targets per call site
  - **Target Validation**: Validate cached targets before use
  - Location: `cpp/src/ppu_jit.cpp`

#### Inline Caching

- [ ] **Call Site Inline Caching**: Complete IC implementation
  - **Monomorphic IC**: Single target call site caching
  - **Polymorphic IC**: Multiple target call site handling
  - **Megamorphic Fallback**: Fallback for highly polymorphic sites
  - **IC Invalidation**: Invalidate on code modification
  - Location: `cpp/src/ppu_jit.cpp` (InlineCacheManager struct)

- [ ] **Constant Propagation Cache**: Cache constant values
  - **Immediate Values**: Cache frequently used immediate values
  - **Memory Load Caching**: Cache repeated memory loads
  - **Register Value Tracking**: Track known register values
  - Location: `cpp/src/ppu_jit.cpp`

#### Register Allocation

- [ ] **Register Allocation Optimization**: Complete register allocator
  - **Liveness Analysis**: Track live GPR/FPR/VR across blocks
  - **Spill/Fill Optimization**: Minimize stack spills
  - **Register Hints**: Caller-saved vs callee-saved preferences
  - **Cross-Block Allocation**: Preserve registers across basic blocks
  - Location: `cpp/src/ppu_jit.cpp` (RegisterAllocator struct)

- [ ] **Register Coalescing**: Reduce move instructions
  - **Copy Elimination**: Eliminate unnecessary register copies
  - **Phi Elimination**: Handle SSA phi nodes efficiently
  - **Argument Passing**: Optimize function call register usage
  - Location: `cpp/src/ppu_jit.cpp`

#### Lazy Compilation

- [ ] **Lazy Compilation Manager**: Complete lazy JIT implementation
  - **Threshold Tuning**: Configurable execution count threshold (default: 10)
  - **State Machine**: NotCompiled → Pending → Compiling → Compiled/Failed
  - **Stub Generation**: Generate interpreter stubs for uncompiled code
  - **Hot Path Detection**: Identify and prioritize hot code paths
  - Location: `cpp/src/ppu_jit.cpp` (LazyCompilationManager struct)

- [ ] **Tiered Compilation**: Implement multi-tier compilation
  - **Tier 0**: Interpreter (immediate execution)
  - **Tier 1**: Baseline JIT (fast compilation, low optimization)
  - **Tier 2**: Optimizing JIT (slow compilation, high optimization)
  - **Tier Transition**: Automatic tier promotion based on execution count
  - Location: `cpp/src/ppu_jit.cpp`

#### Multi-threaded Compilation

- [ ] **Compilation Thread Pool**: Complete parallel compilation
  - **Worker Threads**: Configurable thread count
  - **Priority Queue**: Priority-based task scheduling
  - **Task Completion**: Track pending/completed compilation tasks
  - **Thread Synchronization**: Proper mutex/condition variable usage
  - Location: `cpp/src/ppu_jit.cpp` (CompilationThreadPool struct)

- [ ] **Background Compilation**: Compile ahead of execution
  - **Speculative Compilation**: Compile likely-to-execute blocks
  - **Branch Target Precompilation**: Compile branch targets in advance
  - **Idle Compilation**: Compile during idle time
  - Location: `cpp/src/ppu_jit.cpp`

#### SPU-Specific JIT Features

- [ ] **Loop Optimization**: Complete SPU loop handling
  - **Loop Detection**: Identify loop headers and back edges
  - **Iteration Count**: Determine compile-time iteration count
  - **Vectorization Check**: Mark loops as vectorizable
  - **Loop Unrolling**: Unroll small loops for performance
  - Location: `cpp/src/spu_jit.cpp` (LoopOptimizer struct)

- [ ] **Channel Operation JIT**: Compile channel I/O
  - **Channel Read/Write**: JIT `rdch`, `wrch`, `rchcnt` instructions
  - **Blocking Semantics**: Handle blocking channel operations
  - **Callback Integration**: Channel callbacks for interpreter fallback
  - **All 32 Channels**: Support all SPU/MFC channels
  - Location: `cpp/src/spu_jit.cpp` (ChannelManager struct)

- [ ] **MFC DMA JIT**: Compile DMA operations
  - **GET/PUT Commands**: All DMA command variants (GET, PUT, GETB, PUTB, GETF, PUTF)
  - **Atomic Operations**: GETLLAR, PUTLLC, PUTLLUC
  - **Tag Management**: DMA tag tracking and completion
  - **Transfer Callbacks**: DMA transfer callbacks
  - Location: `cpp/src/spu_jit.cpp` (MfcDmaManager struct)

- [ ] **SIMD Intrinsics**: Native SIMD code generation
  - **Integer Ops**: VecAddI8/16/32, VecSubI8/16/32, VecMulI16
  - **Float Ops**: VecAddF32, VecSubF32, VecMulF32, VecMaddF32
  - **Logical Ops**: VecAndV, VecOrV, VecXorV, VecNotV
  - **Shuffle Ops**: VecShuffle, VecRotateBytes, VecShiftBytes, VecSelect
  - **Instruction Mapping**: Map SPU opcodes to native SIMD intrinsics
  - Location: `cpp/src/spu_jit.cpp` (SimdIntrinsicManager struct)

#### JIT Execution & Debugging

- [ ] **Execution Context**: Complete context management
  - **PPU Context**: All 32 GPR, 32 FPR, 32 VR, CR, LR, CTR, XER, FPSCR, VSCR
  - **SPU Context**: 128 128-bit registers, local storage pointer
  - **Exit Reason Codes**: Normal, Branch, Syscall, Breakpoint, Error
  - **Memory Base**: Memory pointer for load/store operations
  - Location: `crates/oc-ffi/src/jit.rs` (PpuContext, SpuContext)

- [ ] **Breakpoint Integration**: Complete debugger support
  - **Software Breakpoints**: Insert breakpoints in compiled code
  - **Breakpoint Tracking**: Per-address breakpoint management
  - **Code Patching**: Patch compiled code for breakpoints
  - **Breakpoint Exit**: Exit JIT execution on breakpoint hit
  - Location: `cpp/src/ppu_jit.cpp` (BreakpointManager struct)

- [ ] **JIT Profiling**: Add performance profiling
  - **Execution Counting**: Count block executions
  - **Time Measurement**: Measure compilation and execution time
  - **Hot Block Detection**: Identify performance-critical blocks
  - **IR Dump**: Dump LLVM IR for debugging
  - Location: `cpp/src/ppu_jit.cpp`, `cpp/src/spu_jit.cpp`

### Graphics (RSX)

#### NV4097 Method Handlers

- [ ] **Complete NV4097 Method Handlers**: Implement remaining RSX draw commands
  - Handle unknown/unimplemented methods (see `crates/oc-rsx/src/methods.rs:590`)
  - Add more texture format support
  - Location: `crates/oc-rsx/src/methods.rs`

- [ ] **Draw Command Methods**: Complete primitive rendering
  - `NV4097_DRAW_ARRAYS` - Indexed draw calls with proper primitive restart
  - `NV4097_DRAW_INDEX_ARRAY` - Vertex index buffer handling
  - `NV4097_CLEAR_SURFACE` - Multi-render target clearing
  - `NV4097_SET_PRIMITIVE_TYPE` - All primitive types (fans, strips, quads)
  - Location: `crates/oc-rsx/src/methods.rs`

- [ ] **Render Target Methods**: Complete surface and framebuffer handling
  - `NV4097_SET_SURFACE_COLOR_TARGET` - MRT (Multiple Render Targets) support
  - `NV4097_SET_SURFACE_FORMAT` - All depth/color format combinations
  - `NV4097_SET_SURFACE_PITCH_*` - Pitch calculation for non-linear surfaces
  - Tile/swizzle surface layouts
  - Location: `crates/oc-rsx/src/methods.rs`, `crates/oc-rsx/src/state.rs`

- [ ] **Blend State Methods**: Complete blend mode support
  - `NV4097_SET_BLEND_ENABLE_MRT` - Per-render target blend enable
  - `NV4097_SET_BLEND_EQUATION_RGB/ALPHA` - Separate RGB/Alpha equations
  - `NV4097_SET_BLEND_COLOR` - Constant blend color
  - All blend factor combinations
  - Location: `crates/oc-rsx/src/methods.rs`

- [ ] **Stencil Methods**: Complete two-sided stencil
  - `NV4097_SET_TWO_SIDED_STENCIL_TEST_ENABLE` - Two-sided stencil
  - `NV4097_SET_BACK_STENCIL_*` - All back face stencil operations
  - Stencil write mask per face
  - Location: `crates/oc-rsx/src/methods.rs`

- [ ] **Texture Sampling Methods**: Complete texture unit configuration
  - `NV4097_SET_TEXTURE_CONTROL3` - Anisotropic filtering levels
  - `NV4097_SET_TEXTURE_BORDER_COLOR` - Border color sampling
  - `NV4097_SET_TEXTURE_CONTROL0` - LOD bias and clamping
  - Cube map and 3D texture addressing
  - Location: `crates/oc-rsx/src/methods.rs`

- [ ] **Transform Feedback Methods**: Implement stream output
  - `NV4097_SET_TRANSFORM_FEEDBACK_ENABLE` - Enable/disable
  - Buffer binding and offset handling
  - Primitive counting
  - Location: `crates/oc-rsx/src/methods.rs`

- [ ] **Occlusion Query Methods**: Complete query support
  - `NV4097_SET_ZPASS_PIXEL_COUNT_ENABLE` - Z-pass counting
  - `NV4097_SET_REPORT_SEMAPHORE_OFFSET` - Query result writing
  - Conditional rendering based on query results
  - Location: `crates/oc-rsx/src/methods.rs`

#### Shader System

- [ ] **Shader Compilation Improvements**: Enhance RSX shader handling
  - Complete fragment program decoder
  - Handle all vertex program instructions
  - Improve SPIR-V generation for edge cases
  - Location: `crates/oc-rsx/src/shader/`

- [ ] **Vertex Program Opcodes**: Complete VP instruction coverage
  - **Vector Ops**: `TXL` (texture lookup with LOD), `SSG` (sign of source)
  - **Scalar Ops**: `BRA`, `BRI`, `CAL`, `CLI`, `RET` (flow control)
  - **Push/Pop**: `PSH`, `POP` (address stack operations)
  - Indexed constant/input access with ARL
  - Location: `crates/oc-rsx/src/shader/vp_decode.rs`, `crates/oc-rsx/src/shader/types.rs`

- [ ] **Fragment Program Opcodes**: Complete FP instruction coverage
  - **Texture Ops**: `TEX`, `TXP`, `TXD`, `TXB`, `TXL` with all addressing modes
  - **Flow Control**: `BRK`, `LOOP`, `REP`, `RET`, `IF`, `ELSE`, `ENDIF`
  - **Special Ops**: `DDX`, `DDY` (derivatives), `KIL` (pixel kill)
  - Half-precision operations
  - Location: `crates/oc-rsx/src/shader/fp_decode.rs`

- [ ] **SPIR-V Generation**: Complete shader translation
  - All VP/FP opcodes to SPIR-V mapping
  - Proper handling of RSX-specific semantics
  - Texture coordinate projection
  - Fragment program fog integration
  - Location: `crates/oc-rsx/src/shader/spirv_gen.rs`

- [ ] **Shader Cache**: Implement persistent shader caching
  - Hash-based shader lookup
  - Disk cache for compiled SPIR-V
  - Cache invalidation on driver updates
  - Location: `crates/oc-rsx/src/shader/cache.rs`

#### Texture System

- [ ] **Texture Format Support**: Complete format handling
  - **Standard Formats**: All ARGB/RGBA/BGR variants
  - **Compressed Formats**: DXT1/3/5 decompression fallback
  - **HDR Formats**: `W16_Z16_Y16_X16_FLOAT`, `W32_Z32_Y32_X32_FLOAT`
  - **Depth Formats**: `DEPTH24_D8`, `DEPTH16`, `DEPTH24_D8_FLOAT`
  - Location: `crates/oc-rsx/src/texture.rs`

- [ ] **Texture Swizzle/Tile**: Implement memory layout conversion
  - Linear to tiled conversion
  - Morton/Z-order swizzling
  - Pitch calculation for arbitrary widths
  - Location: `crates/oc-rsx/src/texture.rs`

- [ ] **Mipmap Generation**: Complete mipmap handling
  - Automatic mipmap generation
  - Proper LOD selection
  - Trilinear filtering
  - Location: `crates/oc-rsx/src/texture.rs`

#### Vulkan Backend

- [ ] **Vulkan Backend Enhancements**: Complete Vulkan graphics implementation
  - Multi-sample anti-aliasing (MSAA)
  - More texture compression formats
  - Compute shader support for RSX emulation
  - Location: `crates/oc-rsx/src/backend/vulkan.rs`

- [ ] **Pipeline State Management**: Optimize pipeline creation
  - Pipeline caching and reuse
  - Dynamic state for viewport/scissor
  - Separate blend state per attachment
  - Location: `crates/oc-rsx/src/backend/vulkan.rs`

- [ ] **Memory Management**: Improve GPU memory handling
  - Suballocation for small buffers
  - Staging buffer pooling
  - Memory type selection optimization
  - Location: `crates/oc-rsx/src/backend/vulkan.rs`

- [ ] **Synchronization**: Complete sync primitive handling
  - Fence management for frame pacing
  - Semaphore-based GPU/CPU sync
  - Timeline semaphores for RSX semaphores
  - Location: `crates/oc-rsx/src/backend/vulkan.rs`

- [ ] **MSAA Support**: Implement multi-sample anti-aliasing
  - Sample count selection (2x, 4x, 8x)
  - MSAA resolve to non-MSAA targets
  - Sample mask handling
  - Location: `crates/oc-rsx/src/backend/vulkan.rs`

#### Rendering Features

- [ ] **Post-Processing**: Complete post-process effects
  - Gamma correction
  - Color space conversion
  - FXAA/SMAA anti-aliasing
  - Location: `crates/oc-rsx/src/postprocess.rs`

- [ ] **Upscaling/Downscaling**: Improve scaling quality
  - Bilinear/bicubic scaling
  - FSR/DLSS support (future)
  - Aspect ratio handling
  - Location: `crates/oc-rsx/src/scaling.rs`

- [ ] **Frame Timing**: Improve frame pacing
  - VSync modes (off, on, adaptive)
  - Frame limiter
  - GPU profiling
  - Location: `crates/oc-rsx/src/timing.rs`

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

#### DualShock 3 Controller

- [ ] **Native DualShock 3 Support**: Add real PS3 controller support
  - USB connection handling
  - Bluetooth pairing
  - Sixaxis motion sensor passthrough
  - Location: `crates/oc-input/src/dualshock3.rs`

- [ ] **DualShock 3 USB Mode**: Complete wired controller support
  - **USB HID Reports**: Parse DS3 USB HID input reports
  - **Feature Reports**: Handle DS3-specific feature reports (LED, rumble)
  - **Report Rate**: Support full 1000Hz polling rate
  - **Hot-plugging**: Detect USB connect/disconnect events
  - Location: `crates/oc-input/src/dualshock3.rs`, `crates/oc-input/src/usb.rs`

- [ ] **DualShock 3 Bluetooth Mode**: Complete wireless controller support
  - **Pairing Process**: Implement DS3 Bluetooth pairing (BD_ADDR swap)
  - **HID Protocol**: Parse Bluetooth HID reports
  - **Connection State**: Handle reconnection and sleep modes
  - **Multiple Controllers**: Support up to 7 wireless controllers
  - Location: `crates/oc-input/src/dualshock3.rs`, `crates/oc-input/src/bluetooth.rs`

- [ ] **Sixaxis Motion Sensors**: Complete motion sensor emulation
  - **Accelerometer**: 3-axis accelerometer (-512 to 511 range)
  - **Gyroscope**: 3-axis gyroscope for rotation detection
  - **Calibration**: Proper sensor calibration and offset handling
  - **Pitch/Roll Calculation**: Accurate angle estimation from sensor data
  - Location: `crates/oc-input/src/dualshock3.rs` (SixaxisData struct)

- [ ] **Vibration/Rumble**: Complete vibration motor control
  - **Dual Motor**: Small (high-freq) and large (low-freq) motor control
  - **Intensity Control**: Full 0-255 intensity for large motor
  - **Duration Timing**: Proper vibration timing and cutoff
  - **Host Passthrough**: Pass vibration to connected hardware
  - Location: `crates/oc-input/src/dualshock3.rs` (VibrationState struct)

- [ ] **Pressure-Sensitive Buttons**: Complete analog button support
  - **12 Analog Buttons**: D-pad, face buttons, L1/R1, L2/R2
  - **Pressure Range**: Full 0-255 pressure sensitivity
  - **Threshold Tuning**: Configurable press/release thresholds
  - Location: `crates/oc-input/src/pad.rs` (PadState.pressure)

#### PlayStation Move Controller

- [ ] **Move Controller Support**: Complete PlayStation Move implementation
  - Camera tracking
  - Position calculation
  - Location: `crates/oc-input/src/move_controller.rs`

- [ ] **Move Motion Sensors**: Complete motion sensor support
  - **Accelerometer**: 3-axis accelerometer with high precision
  - **Gyroscope**: 3-axis gyroscope for orientation tracking
  - **Magnetometer**: 3-axis magnetometer for heading correction
  - **Sensor Fusion**: Combine sensors for accurate orientation
  - Location: `crates/oc-input/src/move_controller.rs` (MoveMotionData struct)

- [ ] **Tracking Sphere**: Complete sphere tracking
  - **LED Color Control**: RGB color control for tracking sphere
  - **Position Tracking**: 3D position estimation from camera
  - **Occlusion Handling**: Handle sphere visibility loss
  - **Multi-Move Tracking**: Support multiple Move controllers
  - Location: `crates/oc-input/src/move_controller.rs` (SphereColor struct)

- [ ] **Move Buttons**: Complete button handling
  - **All Buttons**: Select, T (Trigger), Move, Start, Triangle, Circle, Cross, Square, PS
  - **Analog Trigger**: Full analog trigger support (0-255)
  - **Button Callbacks**: Event-based button press handling
  - Location: `crates/oc-input/src/move_controller.rs` (MoveButtons)

#### Instruments (Guitar Hero / Rock Band)

- [ ] **Instruments Support**: Complete special controller support
  - Guitar Hero controllers
  - Rock Band drum kits
  - Location: `crates/oc-input/src/instruments.rs`

- [ ] **Guitar Controller**: Complete guitar support
  - **Fret Buttons**: 5-fret (GH/RB) and 6-fret (GH Live) support
  - **Strum Bar**: Up/down strum detection
  - **Whammy Bar**: Full analog whammy (0-255)
  - **Tilt Sensor**: Star power activation via tilt
  - **Touch Strip**: Slider support for World Tour+ guitars
  - Location: `crates/oc-input/src/instruments.rs` (GuitarController, GuitarFrets)

- [ ] **Drum Controller**: Complete drum kit support
  - **4-Pad Drums**: Red, Yellow, Blue, Green pads
  - **Pro Drums**: Cymbal detection (3 cymbals)
  - **Kick Pedal**: Bass drum pedal support
  - **Velocity Sensitivity**: Per-pad hit velocity (0-255)
  - **Double Pedal**: Dual bass pedal support
  - Location: `crates/oc-input/src/instruments.rs` (DrumController, DrumPads)

- [ ] **Turntable Controller**: DJ Hero support
  - **Turntable Rotation**: Scratch detection
  - **Crossfader**: Full analog crossfader
  - **Effects Dial**: Effects knob input
  - **Euphoria Button**: Star power equivalent
  - Location: `crates/oc-input/src/instruments.rs` (TurntableController)

#### Camera (PlayStation Eye)

- [ ] **PlayStation Eye Support**: Complete camera emulation
  - **Resolution Modes**: QVGA (320x240@120fps), VGA (640x480@60fps)
  - **Pixel Formats**: RGB24, BGR24, YUV420P, YUYV, Bayer
  - **Frame Rate Control**: Variable frame rate up to 120fps
  - **Exposure/Gain**: Auto and manual exposure control
  - Location: `crates/oc-input/src/camera.rs` (CameraResolution, CameraSettings)

- [ ] **Camera Image Processing**: Complete image handling
  - **Bayer Demosaic**: Convert raw Bayer to RGB
  - **Color Correction**: White balance and color temperature
  - **Brightness/Contrast**: Image adjustment controls
  - **Flip/Mirror**: Horizontal and vertical flipping
  - Location: `crates/oc-input/src/camera.rs`

- [ ] **Move Tracking Integration**: Camera-based Move tracking
  - **Sphere Detection**: Detect Move controller sphere in camera feed
  - **Position Calculation**: 3D position from sphere size/location
  - **Latency Compensation**: Low-latency tracking pipeline
  - **Multiple Spheres**: Track up to 4 Move controllers
  - Location: `crates/oc-input/src/camera.rs`, `crates/oc-input/src/move_controller.rs`

#### Microphone

- [ ] **Microphone Support**: Complete audio input
  - **Sample Rates**: 8kHz, 16kHz, 22.05kHz, 44.1kHz, 48kHz
  - **Formats**: U8, S16LE, F32 sample formats
  - **Mono/Stereo**: Single and dual channel support
  - **Gain Control**: Input gain adjustment (0.0-2.0)
  - Location: `crates/oc-input/src/microphone.rs` (MicrophoneConfig)

- [ ] **Audio Processing**: Microphone audio processing
  - **Noise Gate**: Configurable noise threshold
  - **Echo Cancellation**: AEC for voice chat
  - **Noise Reduction**: Background noise suppression
  - **Audio Buffer**: Ring buffer for sample storage
  - Location: `crates/oc-input/src/microphone.rs`

- [ ] **Multi-Microphone**: SingStar and karaoke support
  - **Dual Microphone**: Two-player microphone input
  - **USB Audio Devices**: Support various USB microphones
  - **Pitch Detection**: Note/pitch detection for karaoke games
  - **Volume Metering**: Real-time volume levels
  - Location: `crates/oc-input/src/microphone.rs` (MicrophoneManager)

#### Keyboard & Mouse

- [ ] **Keyboard Support (cellKb)**: Complete keyboard emulation
  - **USB HID Codes**: Full USB HID key code support
  - **Modifier Keys**: Ctrl, Shift, Alt, Win modifiers
  - **N-Key Rollover**: Multiple simultaneous key presses
  - **Key Events**: KeyDown/KeyUp event handling
  - Location: `crates/oc-input/src/keyboard.rs` (KeyCode, KeyModifiers)

- [ ] **Mouse Support (cellMouse)**: Complete mouse emulation
  - **Position Tracking**: Absolute and relative mouse position
  - **Button State**: Left, Right, Middle, X1, X2 buttons
  - **Scroll Wheel**: Vertical and horizontal scroll
  - **Sensitivity**: Configurable mouse sensitivity
  - Location: `crates/oc-input/src/mouse.rs`

#### USB & Bluetooth

- [ ] **USB Controller Detection**: Improve USB gamepad support
  - **Device Enumeration**: List connected USB HID devices
  - **Known Devices**: DS3, DS4, DualSense, Xbox 360/One, Switch Pro, 8BitDo
  - **HID Report Parsing**: Generic HID report descriptor parsing
  - **Button Mapping**: Automatic button mapping for known controllers
  - Location: `crates/oc-input/src/usb.rs` (UsbControllerManager, known_devices)

- [ ] **Bluetooth Adapter**: Improve Bluetooth controller support
  - **Device Discovery**: Scan for Bluetooth controllers
  - **Pairing**: Controller pairing process
  - **Connection State**: Connected, Pairing, Reconnecting states
  - **HID Protocol**: Bluetooth HID data parsing
  - Location: `crates/oc-input/src/bluetooth.rs` (BluetoothManager, BluetoothState)

#### Input Mapping

- [ ] **Input Mapping System**: Complete controller remapping
  - **Keyboard to Controller**: Map keyboard keys to PS3 buttons
  - **Mouse to Controller**: Map mouse buttons/axes to controller
  - **Gamepad Remapping**: Remap gamepad buttons/axes
  - **Profile System**: Save/load mapping profiles
  - Location: `crates/oc-input/src/mapping.rs` (InputMapping)

- [ ] **Default Mappings**: Provide sensible default mappings
  - **Keyboard Layout**: Default WASD/arrow key mappings
  - **Xbox to PS3**: Map Xbox controller to PS3 layout
  - **Switch to PS3**: Map Switch Pro controller to PS3 layout
  - **Custom Profiles**: User-defined mapping profiles
  - Location: `crates/oc-input/src/mapping.rs`

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

- [x] **SPU Instruction Tests**: Expand test coverage for SPU instructions
  - **Double-Precision**: Tests for `dfa`, `dfm`, `dfma`, `fesd`, `frds`, `dfceq`, `dfcgt` ✅
  - **Quadword Operations**: Tests for `shlqby`, `rotqby`, `rotqmby` edge cases
  - **Channel Blocking**: Multi-threaded channel stall/resume tests
  - **MFC Timing**: Verify DMA completion timing is accurate
  - **Atomic Operations**: GETLLAR/PUTLLC reservation stress tests
  - Location: `crates/oc-spu/src/`, `crates/oc-spu/src/atomics.rs`

- [ ] **RSX/Graphics Tests**: Expand test coverage for graphics
  - **Method Handlers**: Tests for all NV4097 method categories
  - **Shader Decoding**: VP/FP instruction decode/encode round-trip tests
  - **SPIR-V Generation**: Validate generated shaders against reference
  - **Texture Formats**: Format conversion accuracy tests
  - **Vulkan Backend**: Render output comparison tests
  - Location: `crates/oc-rsx/src/`, `crates/oc-rsx/src/shader/`

- [ ] **JIT Compilation Tests**: Expand test coverage for JIT compilers
  - **PPU JIT**: Compiled code correctness vs interpreter
  - **SPU JIT**: Compiled code correctness vs interpreter
  - **Branch Prediction**: Prediction accuracy tests
  - **Inline Caching**: Cache hit/miss behavior tests
  - **Lazy Compilation**: Threshold and state transition tests
  - **Multi-threaded**: Thread pool stress tests
  - **Loop Optimization**: Loop detection accuracy (SPU)
  - **SIMD Intrinsics**: Intrinsic mapping correctness
  - Location: `cpp/src/`, `crates/oc-ffi/src/jit.rs`

- [ ] **Input Device Tests**: Expand test coverage for input handling
  - **DualShock 3**: Sixaxis sensor data accuracy
  - **Pressure Sensitivity**: Analog button value accuracy
  - **Vibration**: Motor timing and intensity tests
  - **Move Controller**: Motion sensor fusion accuracy
  - **Instruments**: Fret/drum pad detection correctness
  - **Camera**: Frame capture and format conversion
  - **Microphone**: Sample rate and format handling
  - **Input Mapping**: Mapping accuracy and profile save/load
  - Location: `crates/oc-input/src/`

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

### SPU Instruction Coverage Details

| Instruction Category | Status | Notes |
|----------------------|--------|-------|
| Integer Add/Subtract | ✅ Complete | `a`, `ah`, `ai`, `ahi`, `sf`, `sfh`, `sfi`, `sfhi` |
| Integer Multiply | ✅ Complete | `mpy`, `mpyu`, `mpyh`, `mpys`, `mpyi`, `mpyui` |
| Shift Word | ✅ Complete | `shl`, `shlh`, `shlhi`, `shli` |
| Rotate Word | ✅ Complete | `rot`, `roth`, `rothi`, `roti` |
| Quadword Shift | ✅ Complete | `shlqby`, `shlqbyi`, `shlqbi`, `shlqbii` |
| Quadword Rotate | ✅ Complete | `rotqby`, `rotqbyi`, `rotqbi`, `rotqbii` |
| Quadword Rotate/Mask | ✅ Complete | `rotqmby`, `rotqmbyi`, `rotqmbi` |
| Logical Operations | ✅ Complete | `and`, `or`, `xor`, `nand`, `nor`, `eqv`, `andc`, `orc` |
| Logical Immediate | ✅ Complete | `andi`, `ori`, `xori`, `andbi`, `orbi`, `xorbi` |
| Select Bits | ✅ Complete | `selb` - conditional bit selection |
| Branch Relative | ✅ Complete | `br`, `bra`, `brsl`, `brasl` |
| Branch Indirect | ✅ Complete | `bi`, `bisl`, `biz`, `binz`, `bihz`, `bihnz` |
| Branch Conditional | ✅ Complete | `brz`, `brnz`, `brhz`, `brhnz` |
| Compare Equal | ✅ Complete | `ceq`, `ceqb`, `ceqh`, `ceqi`, `ceqbi`, `ceqhi` |
| Compare Greater Than | ✅ Complete | `cgt`, `cgtb`, `cgth`, `cgti`, `cgtbi`, `cgthi` |
| Compare Logical GT | ✅ Complete | `clgt`, `clgtb`, `clgth`, `clgti`, `clgtbi`, `clgthi` |
| Float Add/Sub/Mul | ✅ Complete | `fa`, `fs`, `fm` - 4-way SIMD float |
| Float FMA | ✅ Complete | `fma`, `fms`, `fnms` - fused multiply-add |
| Float Estimates | ✅ Complete | `frest`, `frsqest` - reciprocal estimates |
| Float Conversion | 🟡 Partial | `csflt`, `cuflt`, `cflts`, `cfltu` done; `fi` incomplete |
| Double-Precision | ✅ Complete | `dfa`, `dfs`, `dfm`, `dfma`, `dfms`, `dfnms`, `dfnma`, `dfceq`, `dfcgt`, `dfcmeq`, `dfcmgt`, `fesd`, `frds` |
| Load Quadword | ✅ Complete | `lqd`, `lqa`, `lqr`, `lqx` |
| Store Quadword | ✅ Complete | `stqd`, `stqa`, `stqr`, `stqx` |
| Immediate Load | ✅ Complete | `il`, `ilh`, `ilhu`, `ila`, `iohl` |
| Channel Read/Write | ✅ Complete | `rdch`, `wrch`, `rchcnt` |
| Channel Blocking | 🟡 Partial | Basic ops done; proper stalling incomplete |
| Shuffle Bytes | ✅ Complete | `shufb` - arbitrary byte permutation |
| Copy-to-Insert | ✅ Complete | `cbd`, `chd`, `cwd`, `cdd`, `cbx`, `chx`, `cwx`, `cdx` |
| Carry/Borrow | ✅ Complete | `cg`, `bg`, `cgx`, `bgx`, `addx`, `sfx` |
| Control/Hints | ✅ Complete | `nop`, `lnop`, `sync`, `dsync`, `hbra`, `hbrr`, `hbrp`, `mfspr` |
| MFC DMA | ✅ Complete | GET, PUT, GETB, PUTB, GETF, PUTF with timing |
| MFC Atomic | ✅ Complete | GETLLAR, PUTLLC, PUTLLUC with reservation |
| MFC List DMA | ✅ Complete | GETL, PUTL with stall-and-notify, resume support |
| JIT Arithmetic | ✅ Complete | a, ah, ai, ahi, sf, sfh, sfi, sfhi, mpy, mpyu, mpyh, addx, sfx, cg, bg, cgx, bgx, absdb |
| JIT Quadword | 🔴 Minimal | Not implemented |
| JIT Load/Store | 🔴 Minimal | Not implemented |
| JIT Channel | 🟡 Partial | Channel framework in C++; incomplete coverage |
| JIT Float | 🔴 Minimal | Not implemented |

### JIT Compilation Coverage Details

| Component Category | Status | Notes |
|--------------------|--------|-------|
| LLVM Context | ✅ Complete | Context and module creation |
| LLVM ORC JIT | 🟡 Partial | Basic ORC v2 setup; optimization passes incomplete |
| Target Machine | ✅ Complete | Native target selection |
| IR Builder | ✅ Complete | Basic LLVM IR generation |
| Code Cache (PPU) | ✅ Complete | Block caching with 64MB limit |
| Code Cache (SPU) | ✅ Complete | Block caching with 64MB limit |
| Cache Eviction | 🔴 Minimal | Size-based only; LRU not implemented |
| Cache Statistics | 🔴 Minimal | Not implemented |
| Basic Block Detection (PPU) | ✅ Complete | Branch-based block boundaries |
| Basic Block Detection (SPU) | ✅ Complete | All branch types detected |
| Block Merging | 🔴 Minimal | Not implemented |
| Branch Prediction | ✅ Complete | Likely/Unlikely/Static hints |
| Branch History | ✅ Complete | Taken/not-taken counters |
| Prediction Updates | ✅ Complete | Runtime prediction updates |
| Branch Target Cache | 🔴 Minimal | Not implemented |
| Inline Cache (PPU) | ✅ Complete | Call site caching with eviction |
| IC Lookup | ✅ Complete | Hit counting, validation |
| IC Invalidation | ✅ Complete | Target-based invalidation |
| Polymorphic IC | 🔴 Minimal | Single-target only |
| Register Liveness | ✅ Complete | GPR/FPR/VR liveness analysis |
| Register Hints | ✅ Complete | Caller/callee preference hints |
| Spill/Fill | 🔴 Minimal | Not optimized |
| Register Coalescing | 🔴 Minimal | Not implemented |
| Lazy Compilation | ✅ Complete | Threshold-based triggering |
| Lazy State Machine | ✅ Complete | NotCompiled → Pending → Compiling → Compiled |
| Lazy Threshold | ✅ Complete | Configurable threshold (default: 10) |
| Tiered Compilation | 🔴 Minimal | Single tier only |
| Compilation Thread Pool | ✅ Complete | Multi-threaded worker pool |
| Priority Queue | ✅ Complete | Priority-based task scheduling |
| Task Tracking | ✅ Complete | Pending/completed counters |
| Background Compilation | 🔴 Minimal | Not implemented |
| Loop Detection (SPU) | ✅ Complete | Header/back-edge/exit detection |
| Loop Iteration Count | ✅ Complete | Compile-time count tracking |
| Loop Vectorization Flag | ✅ Complete | Vectorizable marking |
| Loop Unrolling | 🔴 Minimal | Not implemented |
| Channel Manager (SPU) | ✅ Complete | All 32 channels supported |
| Channel Callbacks | ✅ Complete | Read/write callback registration |
| Channel Blocking JIT | 🟡 Partial | Basic operations, blocking incomplete |
| MFC DMA Manager | ✅ Complete | DMA operation queuing |
| MFC Tag Groups | ✅ Complete | Tag-based operation tracking |
| MFC GET/PUT | ✅ Complete | All command variants |
| MFC Atomic | ✅ Complete | GETLLAR, PUTLLC, PUTLLUC |
| DMA Callbacks | ✅ Complete | Transfer callback registration |
| SIMD Intrinsic Map | ✅ Complete | SPU opcode to intrinsic mapping |
| SIMD Integer Ops | 🟡 Partial | Add/Sub/And/Or/Xor mapped |
| SIMD Float Ops | 🟡 Partial | Add/Sub/Mul mapped |
| SIMD Shuffle | 🔴 Minimal | Not implemented |
| PPU Context | ✅ Complete | 32 GPR, 32 FPR, 32 VR, CR, LR, CTR, XER, FPSCR, VSCR |
| SPU Context | 🟡 Partial | 128 registers, LS pointer; some fields incomplete |
| Exit Reason Codes | ✅ Complete | Normal, Branch, Syscall, Breakpoint, Error |
| Breakpoint Manager | ✅ Complete | Add/remove/check breakpoints |
| Breakpoint Code Patch | 🔴 Minimal | Not implemented |
| JIT Profiling | 🔴 Minimal | Not implemented |
| IR Dump | 🔴 Minimal | Not implemented |

### RSX Method & Shader Coverage Details

| Component Category | Status | Notes |
|--------------------|--------|-------|
| Surface/Render Target | ✅ Complete | `SET_SURFACE_FORMAT`, `SET_SURFACE_PITCH`, color/depth targets |
| Viewport/Scissor | ✅ Complete | `SET_VIEWPORT_*`, `SET_SCISSOR_*`, clip ranges |
| Clear Operations | ✅ Complete | `CLEAR_SURFACE`, color/depth/stencil clear values |
| Blend State | ✅ Complete | Enable, src/dst factors, equation, color |
| Blend MRT | 🟡 Partial | Per-target blend enable, separate RGB/Alpha equations incomplete |
| Depth Test | ✅ Complete | Enable, function, mask |
| Stencil Test (Front) | ✅ Complete | Func, ref, mask, ops |
| Stencil Test (Back) | 🟡 Partial | Two-sided stencil, back face ops incomplete |
| Cull Face | ✅ Complete | Enable, mode, front face |
| Alpha Test | ✅ Complete | Enable, function, reference |
| Polygon Offset | ✅ Complete | Fill/line/point enable, factor, bias |
| Line/Point Size | ✅ Complete | Width, size, point sprite |
| Color Mask | ✅ Complete | RGBA masks, MRT masks |
| Logic Op | ✅ Complete | Enable, operation |
| Fog | ✅ Complete | Mode, params |
| Dither | ✅ Complete | Enable |
| Anti-Aliasing | 🟡 Partial | Sample count, alpha-to-coverage incomplete |
| Primitive Restart | ✅ Complete | Enable, restart index |
| Occlusion Query | 🟡 Partial | Z-pass enable, semaphore offset |
| Vertex Attrib Format | ✅ Complete | 16 attributes, format/offset |
| Vertex Constants | ✅ Complete | 512 vec4 constants, load slot |
| Transform Feedback | 🔴 Minimal | Enable only, buffer binding incomplete |
| Texture Offset/Format | ✅ Complete | 16 textures, offset/format/rect |
| Texture Filter | ✅ Complete | Min/mag filter, LOD |
| Texture Address | ✅ Complete | Wrap modes |
| Texture Control | 🟡 Partial | Control0 done, anisotropic incomplete |
| Texture Border | 🟡 Partial | Border color, cube maps incomplete |
| Semaphore Methods | ✅ Complete | Offset, release |
| Draw Arrays/Index | 🟡 Partial | Basic draw, primitive restart incomplete |
| VP Vector Opcodes | ✅ Complete | MOV, MUL, ADD, MAD, DP3, DP4, MIN, MAX, etc. |
| VP Scalar Opcodes | 🟡 Partial | MOV, RCP, RSQ, EXP, LOG done; flow control incomplete |
| VP Flow Control | 🔴 Minimal | BRA, CAL, RET not implemented |
| VP Texture Lookup | 🟡 Partial | TXL incomplete |
| FP Arithmetic Opcodes | ✅ Complete | ADD, MUL, MAD, DP3, DP4, etc. |
| FP Texture Opcodes | 🟡 Partial | TEX, TXP done; TXD, TXB, TXL incomplete |
| FP Flow Control | 🔴 Minimal | BRK, LOOP, IF/ELSE not implemented |
| FP Derivative Opcodes | 🔴 Minimal | DDX, DDY not implemented |
| FP Kill | 🟡 Partial | Basic KIL done |
| SPIR-V Arithmetic | ✅ Complete | FADD, FSUB, FMUL, FDIV, DOT |
| SPIR-V Texture | 🟡 Partial | Basic sampling, projection incomplete |
| SPIR-V Flow Control | 🔴 Minimal | Not implemented |
| Shader Cache | 🟡 Partial | Runtime cache, disk cache incomplete |
| Texture DXT | ✅ Complete | DXT1/3/5 via Vulkan |
| Texture ARGB | ✅ Complete | All ARGB variants |
| Texture HDR | 🟡 Partial | Float16 done, Float32 incomplete |
| Texture Depth | 🟡 Partial | DEPTH24_D8, DEPTH16 done; float depth incomplete |
| Texture Swizzle | 🔴 Minimal | Linear only, tiled incomplete |
| Vulkan Pipeline | ✅ Complete | Basic pipeline creation, layout |
| Vulkan Descriptor | ✅ Complete | Set layout, pool, sets |
| Vulkan Sync | 🟡 Partial | Fences, semaphores; timeline incomplete |
| Vulkan MSAA | 🔴 Minimal | Sample count only, resolve incomplete |
| Vulkan Memory | 🟡 Partial | Allocator, suballocation incomplete |
| Post-Processing | 🟡 Partial | Basic present, gamma incomplete |
| Upscaling | 🟡 Partial | Basic resize, bicubic incomplete |
| Frame Timing | 🟡 Partial | Basic VSync, limiter incomplete |

### Input Device Coverage Details

| Component Category | Status | Notes |
|--------------------|--------|-------|
| Pad State (cellPad) | ✅ Complete | Button state, analog sticks, pressure sensitivity |
| Pad Buttons | ✅ Complete | All 16 buttons: D-pad, face, shoulder, L3/R3, Start/Select |
| Analog Sticks | ✅ Complete | Left/right stick X/Y (0-255, 128 center) |
| Pressure Sensitivity | ✅ Complete | 12 analog button pressure values (0-255) |
| DS3 Sixaxis Accel | ✅ Complete | 3-axis accelerometer (-512 to 511) |
| DS3 Sixaxis Gyro | ✅ Complete | 3-axis gyroscope rotation rates |
| DS3 Pitch/Roll | ✅ Complete | Angle calculation from accelerometer |
| DS3 Vibration | ✅ Complete | Dual motor (small on/off, large 0-255) |
| DS3 USB Mode | 🟡 Partial | Basic HID reports, feature reports incomplete |
| DS3 Bluetooth | 🔴 Minimal | Framework only, pairing incomplete |
| DS3 Hot-Plug | 🔴 Minimal | Not implemented |
| Move Buttons | ✅ Complete | All 9 buttons: Select, T, Move, Start, face, PS |
| Move Sphere Color | ✅ Complete | RGB LED control (0-255 per channel) |
| Move Motion Accel | 🟡 Partial | Accelerometer done, magnetometer incomplete |
| Move Motion Gyro | 🟡 Partial | Gyroscope done, sensor fusion incomplete |
| Move Tracking | 🔴 Minimal | Framework only, camera tracking incomplete |
| Move Vibration | 🟡 Partial | Basic rumble, intensity incomplete |
| Guitar Frets | ✅ Complete | 5-fret and 6-fret (GH Live) support |
| Guitar Strum | ✅ Complete | Up/down strum detection |
| Guitar Whammy | ✅ Complete | Full analog whammy (0-255) |
| Guitar Tilt | ✅ Complete | Tilt sensor for star power |
| Guitar Touch Strip | 🟡 Partial | Touch position, activation incomplete |
| Drum Pads | ✅ Complete | 4-pad drum (R/Y/B/G) support |
| Drum Cymbals | 🟡 Partial | Pro drums, cymbal detection incomplete |
| Drum Velocity | 🟡 Partial | Hit velocity, threshold incomplete |
| Drum Kick Pedal | ✅ Complete | Single pedal, double pedal incomplete |
| Turntable | 🔴 Minimal | Framework only, rotation incomplete |
| Camera Resolution | ✅ Complete | QVGA, VGA, HD720, HD1080 |
| Camera Pixel Format | ✅ Complete | RGB24, BGR24, YUV420P, YUYV, Bayer |
| Camera Settings | ✅ Complete | Brightness, contrast, saturation, exposure, gain |
| Camera Frame Capture | 🟡 Partial | Basic capture, Bayer demosaic incomplete |
| Camera Move Tracking | 🔴 Minimal | Sphere detection not implemented |
| Mic Sample Rates | ✅ Complete | 8k, 16k, 22.05k, 44.1k, 48k Hz |
| Mic Formats | ✅ Complete | U8, S16LE, F32 formats |
| Mic Gain/Buffer | ✅ Complete | Gain control, audio buffer |
| Mic Noise Gate | ✅ Complete | Configurable threshold |
| Mic Echo Cancel | 🔴 Minimal | Not implemented |
| Mic Noise Reduction | 🔴 Minimal | Not implemented |
| Multi-Microphone | 🟡 Partial | Dual mic, pitch detection incomplete |
| Keyboard Keys (cellKb) | ✅ Complete | Full USB HID key codes |
| Keyboard Modifiers | ✅ Complete | Ctrl, Shift, Alt, Win modifiers |
| Keyboard Events | ✅ Complete | KeyDown/KeyUp events |
| Mouse Position | 🟡 Partial | Tracking done, sensitivity incomplete |
| Mouse Buttons | ✅ Complete | Left, Right, Middle, X1, X2 |
| Mouse Scroll | 🟡 Partial | Vertical done, horizontal incomplete |
| USB Device Enum | ✅ Complete | Device listing, known device detection |
| USB Known Devices | ✅ Complete | DS3, DS4, DualSense, Xbox, Switch Pro |
| USB HID Parsing | 🟡 Partial | Basic reports, descriptor parsing incomplete |
| USB Button Mapping | ✅ Complete | HashMap-based button mapping |
| Bluetooth Address | ✅ Complete | BD_ADDR parsing, string conversion |
| Bluetooth State | ✅ Complete | Disconnected, Scanning, Pairing, Connected |
| Bluetooth Pairing | 🔴 Minimal | Framework only, DS3 pairing incomplete |
| Bluetooth HID | 🔴 Minimal | Not implemented |
| Input Mapping | ✅ Complete | HostInput to Ps3Input mapping |
| Default Keyboard Map | ✅ Complete | WASD, arrows, ZXCV face buttons |
| Gamepad Remapping | 🟡 Partial | Button mapping done, axis incomplete |
| Profile Save/Load | 🔴 Minimal | Not implemented |

---

## 📝 Notes

- Refer to `docs/HLE_STATUS.md` for detailed HLE module status
- Check `docs/ppu_instructions.md` and `docs/spu_instructions.md` for instruction coverage
- Build with `cargo build --release` and test with `cargo test`
- Run `RUST_LOG=debug cargo run` for detailed logging

---

*Last updated: 2026-01-21*
