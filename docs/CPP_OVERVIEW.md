# C++ Backend Overview

This document describes the C++ side of **oxidized-cell**, focusing on how the JIT compilers, FFI surface, threading shims, and utility components fit together. It is intended as a single entry point for contributors who need a holistic view of the native code.

## Directory layout

```
cpp/
├── CMakeLists.txt      # Standalone build (optional; primary build is driven from Cargo)
├── include/            # Public headers exposed to Rust
│   ├── oc_ffi.h        # C ABI surface and shared data structures
│   └── oc_threading.h  # Cross-platform threading shims
└── src/
    ├── atomics.cpp     # 128-bit CAS/load/store helpers
    ├── dma.cpp         # DMA accelerator placeholder
    ├── ffi.cpp         # Runtime init/shutdown stubs
    ├── ppu_jit.cpp     # PowerPC PPU JIT compiler
    ├── spu_jit.cpp     # SPU JIT compiler
    ├── rsx_shaders.cpp # RSX shader -> SPIR-V compiler + pipeline cache
    └── simd_avx.cpp    # Placeholder AVX helpers
```

## FFI surface (`oc_ffi.h`, `ffi.cpp`)
- Exposes a C ABI used by the Rust crates to bootstrap and tear down the C++ runtime (`oc_init`, `oc_shutdown`).
- Defines shared types:
  - `oc_v128_t` for 128-bit vector values.
  - `oc_ppu_context_t` which models the full PPU register file and execution state passed into JITed blocks.
- Declares opaque handles for the PPU/SPU JITs and RSX shader compiler plus their management APIs (create, destroy, compile, cache control, breakpoints, etc.).
- Declares 128-bit atomic helpers (`oc_atomic_*`), exposed for interoperability with Rust code that needs wide atomics.

## Threading shims (`oc_threading.h`)
- Wraps platform primitives to avoid MinGW `std::thread`/`std::mutex` pitfalls:
  - On Windows/MinGW uses `CRITICAL_SECTION`, `CONDITION_VARIABLE`, and `CreateThread`.
  - On other platforms aliases to the C++ standard library.
- Provides light RAII wrappers (`oc_lock_guard`, `oc_unique_lock`, `oc_condition_variable`, `oc_thread`) used by the JIT caches and thread pools.

## PPU JIT (`ppu_jit.cpp`)
- Implements basic-block driven JIT for the Cell PPU (PPC64):
  - `identify_basic_block` scans PowerPC instructions (big-endian) to build basic blocks that end on branches, traps, or syscalls.
  - `BasicBlock` objects store decoded instructions, CFG edges, and compiled code pointers.
  - `CodeCache` holds compiled blocks with LRU eviction and statistics (hits, misses, evictions, invalidations).
  - `BlockMerger` can merge fall-through blocks with single-predecessor successors to improve optimization opportunities.
- Execution metadata and optimizations:
  - `BreakpointManager` tracks breakpoints.
  - `BranchPredictor` records hints and runtime behavior (taken/not-taken counters, static/backward hints).
  - `InlineCacheManager` caches call-site targets and compiled pointers with simple eviction by hit count.
  - `RegisterAllocator` tracks GPR/FPR/VR liveness per block and offers allocation hints.
  - `LazyCompilationManager` defers compilation until a block crosses an execution threshold (states: not compiled, pending, compiling, compiled, failed).
  - `CompilationThreadPool` provides prioritized, multi-threaded compilation; uses `oc_thread` and condition variables for work distribution.
- LLVM integration (guarded by `HAVE_LLVM`):
  - `OrcJitManager` wraps LLVM ORC LLJIT with host CPU feature detection (AVX2/AVX-512/SSE4.2), target machine setup, and module management.
  - `generate_llvm_ir` builds LLVM IR for each block (via `emit_ppu_instruction`) and falls back to placeholder x86 `ret` buffers if LLVM/JIT setup is unavailable.
- Entry points exposed through FFI:
  - Creation/destruction, cache invalidation, breakpoints, branch hints, inline cache lookups, register analysis, lazy compilation toggles, and multi-threaded compilation controls.
  - Execution interfaces: `oc_ppu_jit_execute` / `_execute_block` consume an `oc_ppu_context_t` and return instruction counts/exit reasons.

## SPU JIT (`spu_jit.cpp`)
- Mirrors the PPU JIT structure for the Cell SPU:
  - `SpuBasicBlock`, `SpuCodeCache`, and `SpuBlockMerger` understand SPU branch encodings (br/bra/brsl/bi/brnz/brz/brhnz/brhz).
  - Placeholders exist for emitting LLVM IR when `HAVE_LLVM` is available (currently scaffolding).
- Additional SPU-specific features exposed via FFI:
  - Channel ops toggles and callbacks, MFC DMA queuing and tag completion, loop detection/metadata (vectorizable flags, iteration counts), and SIMD intrinsic mapping controls.

## RSX shader pipeline (`rsx_shaders.cpp`)
- Provides a software RSX shader compiler:
  - Decodes vertex (`RsxVpOpcode`) and fragment (`RsxFpOpcode`) programs into `RsxShaderInstruction` sequences and constant pools.
  - Intended to emit SPIR-V for Vulkan; includes linking of vertex/fragment pairs and a pipeline cache keyed by shader hashes and render state.
  - FFI functions cover compilation, linking, pipeline callbacks, cache queries/eviction, and SPIR-V buffer management.

## Atomics and SIMD helpers
- `atomics.cpp` implements 128-bit CAS/load/store using `cmpxchg16b` on x86-64 with GNU inline assembly, falling back to non-atomic memcmp/memcpy elsewhere.
- `simd_avx.cpp` is a placeholder for AVX-optimized routines; guarded by `ARCH_X64`.

## DMA placeholder
- `dma.cpp` currently documents a placeholder for DMA acceleration; real DMA fast-path code can be added here and wired through the SPU JIT APIs that expose DMA hooks.

## Build and configuration notes
- The primary build is driven from Cargo; the C++ code is compiled as part of the Rust crate via `cc`/`cxx` build scripts (see `Cargo.toml` and `crates/oc-ffi`).
- `HAVE_LLVM` enables LLVM/ORC JIT paths; when not defined, compilation falls back to placeholder stubs so the API remains usable.
- Windows/MinGW consumers rely on `oc_threading.h` to avoid `std::thread` ABI issues; no special handling is needed on POSIX toolchains.

## Data flow summary
1. Rust initializes the native side via `oc_init` and creates JIT/shader handles through the FFI.
2. For JIT compilation:
   - Rust registers code buffers and execution contexts.
   - The JIT splits code into blocks, optionally merges them, generates IR/native code (or placeholders), and caches results with LRU.
   - Execution functions run compiled blocks against `oc_ppu_context_t` or SPU context equivalents, updating registers and exit reasons.
3. For graphics:
   - RSX programs are decoded, translated to SPIR-V, linked, and cached; pipelines can be retrieved or evicted via FFI callbacks.

## Current gaps and extension points
- LLVM IR emission for many instructions and AVX helper implementations are stubs; they can be progressively filled in without changing the FFI surface.
- DMA acceleration and full SPIR-V emission are scaffolded; the existing APIs allow incremental feature delivery while keeping Rust integration stable.
