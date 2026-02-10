# 📋 oxidized-cell — Project TODO

> Generated from codebase analysis. Tracks work needed to bring the PS3 emulator from its current early-development state toward game compatibility.

---

## 🔴 High Priority

### HLE Module Completion — Core System

#### cellGcmSys (`cell_gcm_sys.rs`) — Mostly Implemented
The GcmManager handles display buffers, flip mode, and command buffer basics via `RsxBridgeSender`. Remaining work:
- [x] Implement deep command FIFO parsing for inline RSX commands
- [x] Add 3D rendering parameter validation in `cellGcmSetSurface`
- [x] Implement memory mapping cache for RSX local memory ↔ main memory
- [x] Add tile/zcull region management (`cellGcmSetTileInfo`, `cellGcmBindTile`)
- [x] Handle cursor management methods (`cellGcmSetCursorEnable`, `cellGcmSetCursorPosition`)

#### cellSpurs (`cell_spurs.rs`) — Mostly Implemented, Needs SPU Execution
The SpursManager tracks tasksets and task states (Ready/Running/Waiting/Completed). Remaining work:
- [x] Wire up actual SPU instruction execution for SPURS tasks (currently tracks state only)
- [x] Implement workload contention handling when tasksets exceed available SPUs
- [x] Add SPURS trace/profiling support for debugging task scheduling
- [x] Implement SPURS kernel-mode tasklet execution
- [x] Support SPURS policy module loading (`cellSpursAddPolicyModule`)

#### cellSysutil (`cell_sysutil.rs`) — Mostly Implemented
The SysutilManager has 4 callback slots, system params, and dialog support. Remaining work:
- [x] Implement date/time formatting algorithms for `cellSysutilGetSystemParamString`
- [x] Add on-screen keyboard (OSK) overlay support
- [x] Implement `cellSysutilGetBgmPlaybackStatus` for background music
- [x] Add game update checking stubs (`cellSysutilCheckUpdateStatus`)
- [x] Implement trophy system integration stubs (`cellSysutilRegisterTrophyCallback`)

#### cellSaveData (`cell_save_data.rs`) — Mostly Implemented
The SaveDataManager supports load/save with callback-based selection. Remaining work:
- [x] Implement actual AES-128 file encryption/decryption (currently placeholder)
- [x] Add save data corruption detection and recovery
- [x] Implement save data icon rendering for the selection UI
- [x] Add `cellSaveDataUserGetListItem` for per-user enumeration
- [x] Handle auto-save overwrite confirmation dialogs

#### cellFont / cellFontFt (`cell_font.rs`, `cell_font_ft.rs`) — Partial
The FontManager has glyph rendering surfaces but uses placeholder font data. Remaining work:
- [x] Implement actual TrueType font file parsing (currently returns placeholder glyphs)
- [x] Add FreeType library integration for `cellFontFt` rasterization
- [x] Implement kerning table parsing in `cellFontFtGetKerning`
- [x] Support system font loading from firmware (`/dev_flash/data/font/`)
- [x] Add Unicode code-point to glyph-index mapping

#### module.rs — Registry Only
The module system registers 50+ function NIDs but all map to dummy return values. Remaining work:
- [x] Wire NID registry to actual HLE handler functions in each module
- [x] Implement dynamic PRX import resolution through module registry
- [x] Add function-level logging for unimplemented NID calls

### HLE Module Completion — File System & Game Data

#### cellFs (`cell_fs.rs`) — Fully Implemented
Core file I/O, async I/O, and directory operations work. Minor gaps:
- [x] Add encrypted content reading for MSELF-flagged files
- [x] Implement `cellFsTruncate` and `cellFsFtruncate`
- [x] Add `cellFsGetFreeSize` for device storage queries
- [x] Support `/dev_bdvd` and `/dev_usb` device path prefixes

#### cellGame (`cell_game.rs`) — Fully Implemented
Game data detection and PARAM.SFO parsing work. Minor gaps:
- [x] Add game patch detection and merge logic
- [x] Implement `cellGameGetLocalWebContentPath` for web-content titles
- [x] Support DLC enumeration (`cellGameGetContentInfoList`)

### Game Loading Pipeline
- [x] Complete end-to-end game boot sequence in `crates/oc-integration/`
- [x] Verify ELF/SELF decryption with real firmware keys (`crates/oc-loader/`)
- [x] Implement PRX dependency resolution and loading order
- [x] Handle PLT stub patching for HLE dispatch

### JIT Compiler — PPU (`cpp/src/ppu_jit.cpp`)

The PPU JIT has a sophisticated LLVM-based framework with 170+ instruction stubs, a 64 MB LRU code cache, O2 optimization passes, branch prediction, inline caching, register coalescing, lazy/tiered compilation, and a background compilation thread pool. However, **actual LLVM IR emission requires the `HAVE_LLVM` backend to be enabled** — without it, all code falls through to the Rust interpreter. Specific work needed:

#### Enable and Verify LLVM Backend
- [x] Ensure `HAVE_LLVM` is defined and LLVM 14+ links correctly on all platforms
- [x] Add fallback-to-interpreter error path when JIT compilation fails (currently no error handling)
- [x] Test JIT code emission produces valid machine code for the host ISA

#### Missing PPU Instruction Categories in JIT
The JIT covers integer, load/store, float, branch, rotate, and some AltiVec. Still missing:
- [x] **Remaining AltiVec/VMX** — Only ~10 vector ops (VADDFP, VSUBFP, VAND, VOR, VXOR, VNOR, VPERM, VSEL, VMADDFP, VNMSUBFP) are JIT-compiled; the interpreter handles 100+ VMX instructions. Add JIT paths for high-frequency vector ops: `VADDSWS`, `VADDUWS`, `VMAXSW`, `VMINSW`, `VAVGSW`, `VCMPEQUW`, `VCMPGTSW`, `VMULESH`, `VMULESW`, `VMRGHW`, `VMRGLW`, `VSPLTW`, `VSPLTH`, `VSPLTB`, `VSPLTISW`
- [x] **Vector load/store** — Add `LVX`, `STVX`, `LVLX`, `LVRX`, `STVLX`, `STVRX` to JIT
- [x] **Vector permute/shuffle** — Add `VPKUHUM`, `VPKUWUM`, `VUPKHSH`, `VUPKLSH` to JIT
- [x] **Atomic load/store** — JIT `LWARX`/`STWCX.` and `LDARX`/`STDCX.` pairs for lock-free code hot paths
- [x] **Performance monitor SPRs** — JIT `MFSPR`/`MTSPR` for PMR access (currently interpreted)
- [x] **Supervisor-mode** — Add `RFI`, `RFID`, `MTMSR`, `MFMSR` if needed for kernel-mode emulation

#### PPU JIT Optimization
- [x] Implement block linking — compiled blocks currently return to dispatcher; link direct jumps to avoid dispatch overhead
- [x] Add constant propagation for `LI`/`LIS` → immediate-folding in subsequent arithmetic
- [x] Tune branch predictor thresholds (currently uses default taken/not-taken counters)
- [x] Profile-guided recompilation — use Tier0 execution counts to identify hot blocks for Tier1 O2 recompilation
- [x] Implement trace compilation for hot loops (merge basic blocks along loop back-edges)

### JIT Compiler — SPU (`cpp/src/spu_jit.cpp`)

The SPU JIT has 130+ instruction stubs covering arithmetic, logic, memory, float, branch, channel I/O, quadword shifts, extended arithmetic, and sign-extension ops. It also handles 16 DMA command types and 9 MFC channels. Like the PPU JIT, **emission requires the LLVM backend**.

#### Missing SPU Instruction Categories in JIT
- [ ] **Double-precision float** — Add `DFA` (double float add), `DFS` (double float subtract), `DFM` (double float multiply), `DFMA`/`DFMS`/`DFNMA`/`DFNMS` (double FMA variants)
- [ ] **Missing channel ops** — Expand beyond the 9 implemented MFC channels; add SPU event channels (`SPU_RdEventStat`, `SPU_WrEventMask`, `SPU_WrEventAck`), signal notification channels (`SPU_RdSigNotify1`, `SPU_RdSigNotify2`), and decrementer channel (`SPU_RdDec`)
- [ ] **SPU interrupt handling** — Add `IRET` (interrupt return) instruction JIT path and interrupt enable/disable channel ops
- [ ] **Missing compare ops** — Verify `CLGTHI`, `CLGTBI` (logical compare halfword/byte immediate) are fully emitting IR

#### SPU JIT Optimization
- [ ] Implement loop-aware block merging — `SpuBlockMerger` detects branch types but doesn't merge across loop iterations
- [ ] Add SIMD intrinsic mapping — map SPU 128-bit vector ops to host AVX2/NEON intrinsics instead of generic LLVM IR vectors
- [ ] Optimize channel read/write — inline common channel operations (tag status polling) instead of calling through FFI
- [ ] Implement SPU-to-SPU mailbox fast path for inter-SPU communication without kernel traps

### JIT Compiler — Shared Infrastructure

#### FFI Bridge (`cpp/include/oc_ffi.h`, `crates/oc-ffi/`)
- [ ] **Fix unsafe 128-bit atomic fallback** — `oc_atomic_cas128` falls back to non-atomic `memcpy` on non-x86_64; replace with a mutex-guarded CAS or `__atomic_compare_exchange` intrinsic
- [ ] Add error propagation from LLVM compilation failures back to Rust (currently silent)
- [ ] Add JIT-to-interpreter fallback callback so failed blocks gracefully degrade
- [ ] Wrap opaque C handles (`PpuJit*`, `SpuJit*`, `RsxShader*`) in Rust RAII types for automatic cleanup

#### DMA Acceleration (`cpp/src/dma.cpp`)
- [ ] Implement actual DMA transfer acceleration (currently a placeholder with no logic)
- [ ] Add DMA list command support for scatter-gather transfers
- [ ] Implement DMA fence/barrier synchronization between SPU and PPU

#### SIMD Helpers (`cpp/src/simd_avx.cpp`)
- [ ] Implement AVX2 fast paths for SPU 128-bit vector operations (currently placeholder)
- [ ] Add runtime CPU feature detection to select AVX2 vs SSE4.2 vs scalar fallback
- [ ] Map SPU `SHUFB` (shuffle bytes) to `_mm_shuffle_epi8` / `vpshufb`

---

## 🟡 Medium Priority

### HLE Modules — Multimedia Codecs

#### cellVdec (`cell_vdec.rs`) — Partial Stub
Defines H.264, MPEG-2, and DivX codec types with decoder instance management (up to 16). All decode operations return dummy frames.
- [ ] Implement NAL unit parsing for H.264 bitstreams
- [ ] Implement IDCT transform and motion compensation for frame reconstruction
- [ ] Wire callback notification queue to signal decoded frames to the game
- [ ] Support Baseline, Main, and High H.264 profiles
- [ ] Implement MPEG-2 Simple/Main/High profile decoding

#### cellAdec (`cell_adec.rs`) — Partial Stub
Defines LPCM, AC-3, ATRAC3/3+, MP3, AAC, and WMA codec types. All return silence buffers.
- [ ] Implement AAC-LC frame decoding (most common audio codec in PS3 games)
- [ ] Implement ATRAC3+ decoding (Sony proprietary, used in many first-party titles)
- [ ] Implement MP3 frame decoding via Symphonia integration
- [ ] Wire PCM output format conversion (float ↔ int16 ↔ int32)

#### cellDmux (`cell_dmux.rs`) — Partial Stub
Defines MP4/TS/AVI container support with elementary stream extraction.
- [ ] Implement MP4 container demuxing (moov/moof atom parsing)
- [ ] Implement MPEG-TS packet parsing and PID filtering
- [ ] Support stream-type detection for audio/video elementary streams
- [ ] Wire demuxed elementary streams to cellVdec/cellAdec decoders

#### cellVpost (`cell_vpost.rs`) — Minimal
Defines video post-processing with CSC matrix setup.
- [ ] Implement YCbCr → RGB color space conversion math
- [ ] Add deinterlacing algorithms (bob, weave, motion-adaptive)
- [ ] Implement resolution scaling (bilinear, bicubic)
- [ ] Support picture-in-picture compositing

### HLE Modules — Image Decoders

#### cellPngDec (`cell_png_dec.rs`) — Partial
Parses PNG headers and extracts dimensions but returns dummy pixel data.
- [ ] Implement DEFLATE decompression for IDAT chunks
- [ ] Handle PNG interlacing (Adam7) and color type conversions (palette, grayscale, RGBA)
- [ ] Support streaming decode for large images

#### cellJpgDec (`cell_jpg_dec.rs`) — Partial
Returns placeholder dimensions; no actual JPEG decompression.
- [ ] Implement JPEG baseline DCT decoding (Huffman + IDCT)
- [ ] Handle EXIF orientation and color profile metadata
- [ ] Support progressive JPEG decode

#### cellGifDec (`cell_gif_dec.rs`) — Partial
GIF header parsing present but no frame decompression.
- [ ] Implement LZW decompression for GIF image data
- [ ] Support animated GIF frame extraction with delay timing
- [ ] Handle transparent color index

### HLE Modules — Networking

#### cellNetCtl (`cell_net_ctl.rs`) — Fully Implemented (Simulated)
Complete network state machine, IP info, NAT detection — all simulated without real sockets.
- [ ] Optionally bridge to host network stack for real connectivity
- [ ] Implement DNS resolution passthrough

#### cellHttp (`cell_http.rs`) — Fully Implemented (Simulated)
Full HTTP/1.1 transaction model with headers, callbacks, and pooling — all returning dummy responses.
- [ ] Optionally bridge HTTP transactions to host `reqwest`/`hyper` for real requests
- [ ] Implement response body streaming for large downloads

#### cellSsl (`cell_ssl.rs`) — Basic Stub
Client management and cipher enumeration only; no actual TLS.
- [ ] Implement TLS handshake via `rustls` or `native-tls`
- [ ] Add certificate validation and CA store loading
- [ ] Wire SSL sessions to cellHttp for HTTPS support

### HLE Modules — Input (Fully Implemented, Minor Gaps)

#### cellPad (`cell_pad.rs`) — ✅ Complete
Full DualShock 3 emulation: 7 ports, 64 button codes, analog sticks, motion sensors, rumble.

#### cellKb (`cell_kb.rs`) — ✅ Complete
10 keyboard layouts, raw HID + character mode, LED control, modifier tracking.

#### cellMouse (`cell_mouse.rs`) — ✅ Complete
2 mice, button/position/wheel/tilt tracking, tablet mode.

#### cellMic (`cell_mic.rs`) — ✅ Complete
4 microphones, USB/Bluetooth/EyeToy types, 16K–48K sample rates.

### HLE Modules — Other

#### cellResc (`cell_resc.rs`) — Basic Implementation
Resolution scaling and color buffer conversion present.
- [ ] Implement advanced upscaling filter algorithms (bilinear, lanczos)
- [ ] Add PAL/NTSC framerate conversion support

#### libsre (`libsre.rs`) — ✅ Complete
Regex compilation and matching via Rust `regex` crate. Fully functional.

#### spu_runtime (`spu_runtime.rs`) — Partial
SPU module tracking (max 64) with state machine, but no ELF loading.
- [ ] Implement SPU ELF binary parsing and segment loading into SPU local store
- [ ] Add module relocation handling for position-independent SPU code
- [ ] Implement symbol resolution for SPU-side imports

#### cellAudio (`cell_audio.rs`) — ✅ Complete
8 audio ports, multi-channel (mono/stereo/5.1/7.1), event notification, HleAudioMixer.

### RSX Graphics
- [ ] Complete SPIR-V vertex/fragment shader translation from RSX programs
- [ ] Implement remaining NV4097 command methods
- [ ] Add texture format conversions (swizzled, tiled, compressed)
- [ ] Implement render-to-texture and framebuffer copies
- [ ] Add multi-sample anti-aliasing (MSAA) support
- [ ] Optimize shader caching and pipeline state management

### Audio System
- [ ] Implement full AC3 decoding with IMDCT and bit allocation (`crates/oc-audio/src/codec.rs` — see `TODO` in code)
- [ ] Add audio resampling for non-48kHz sources
- [ ] Implement audio mixing across multiple ports
- [ ] Add audio time-stretching for frame-rate independence

### LV2 Kernel
- [ ] Complete SPU thread group management and scheduling
- [ ] Implement event queue and event flag syscalls
- [ ] Add lightweight mutex support
- [ ] Implement memory-mapped I/O for device emulation
- [ ] Add PRX module management syscalls (load, start, stop, unload)

---

## 🟢 Low Priority

### Testing & Quality
- [ ] Add integration tests that boot a minimal ELF through the full pipeline
- [ ] Expand SPU test coverage (currently 14 tests vs 128+ for memory)
- [ ] Add RSX rendering correctness tests with reference images
- [ ] Add HLE module unit tests for each implemented function
- [ ] Set up benchmark suite for interpreter and JIT performance comparison
- [ ] Track and test game compatibility (homebrew first, then commercial titles)

### CI/CD & Tooling
- [ ] Add GitHub Actions CI workflow (build + test on Linux, Windows, macOS)
- [ ] Add clippy lint checks to CI
- [ ] Add rustfmt formatting checks to CI
- [ ] Set up code coverage reporting
- [ ] Add pre-commit hooks for formatting and linting

### Documentation
- [ ] Create `docs/ppu_instructions.md` (referenced in README but missing)
- [ ] Create `docs/spu_instructions.md` (referenced in README but missing)
- [ ] Create `docs/HLE_STATUS.md` (referenced in README but missing)
- [ ] Write architecture deep-dive documentation
- [ ] Generate and host API documentation (`cargo doc`)
- [ ] Add a CONTRIBUTING.md with detailed contribution guidelines
- [ ] Document the FFI boundary between Rust and C++ components

### UI & Usability
- [ ] Add controller profile management and per-game input configs
- [ ] Implement save state support (snapshot/restore emulator state)
- [ ] Add game compatibility database / status indicators in the UI
- [ ] Improve shader debugger with live SPIR-V disassembly
- [ ] Add performance overlay (FPS, CPU/SPU/RSX usage)
- [ ] Add ROM/ISO validation and integrity checking

### Performance Optimization
- [ ] Profile and optimize PPU interpreter hot paths
- [ ] Use arena allocators for high-frequency allocations in the memory manager
- [ ] Optimize SPU ↔ PPU synchronization and context switching
- [ ] Add SIMD acceleration for SPU interpreter (AVX2 on x86_64)
- [ ] Optimize Vulkan command buffer recording and submission

### Security & Robustness
- [ ] Validate guest memory pointers in all HLE module handlers
- [ ] Audit firmware key management and AES decryption paths
- [ ] Add bounds checking for RSX command buffer parsing
- [ ] Harden FFI boundary with safe wrapper types

---

## ✅ Completed

### Interpreters
- [x] PPU interpreter — ~190 PowerPC instructions across integer, float, VMX/AltiVec (100+), load/store, branch, and system categories
- [x] SPU interpreter — ~206 SPU instructions across arithmetic, memory, logic, compare, float, branch, control, quadword, hints, and channel I/O

### JIT Framework (Architecture Complete, Backend Activation Needed)
- [x] PPU JIT C++/LLVM framework — 170+ instruction stubs, 64 MB LRU code cache, O2 passes, branch predictor, inline cache, register coalescer, tiered lazy compilation, background thread pool
- [x] SPU JIT C++/LLVM framework — 130+ instruction stubs, MFC channel handling, block merging across all SPU branch types, DMA command compilation (16 command types)
- [x] RSX shader pipeline — SPIR-V code generation, VP/FP opcode handling, shader & pipeline caching with LRU eviction

### HLE Modules (Fully Implemented)
- [x] HLE dispatcher (`dispatcher.rs`) — stub registration, call routing, per-function statistics
- [x] HLE context (`context.rs`) — global state holding 28+ manager instances, RSX/SPU bridge integration
- [x] HLE memory bridge (`memory.rs`) — big-endian read/write, guest memory traits, pointer validation
- [x] cellAudio — 8 ports, mono/stereo/5.1/7.1, event notification, HleAudioMixer
- [x] cellPad — 7 ports, 64 button codes, analog sticks, motion sensors, rumble/actuator
- [x] cellKb — 10 keyboard layouts, raw HID + character mode, LED control
- [x] cellMouse — 2 mice, position/wheel/tilt, tablet mode
- [x] cellMic — 4 microphones, USB/Bluetooth/EyeToy, 16K–48K sample rates
- [x] cellFs — 1024 FDs, async I/O with background threads, device path resolution, oc-vfs integration
- [x] cellGame — game type detection, PARAM.SFO parsing, content attributes
- [x] cellSysutil — 4 callback slots, system params, dialog support, language/timezone
- [x] cellNetCtl — full state machine, IP/DNS/NAT info, event handlers (simulated)
- [x] cellHttp — HTTP/1.1 transactions, headers, status/data callbacks, pooling (simulated)
- [x] cellSpursJq — job queues (max 16), 256 jobs/queue, priority scheduling, sync/abort (11 test cases)
- [x] libsre — regex compilation and matching via Rust `regex` crate

### Infrastructure
- [x] Memory manager — 4GB virtual address space, 4KB pages, reservation system
- [x] LV2 kernel — threads, mutex, condvar, semaphore, rwlock, timers
- [x] Vulkan graphics backend with NV4097 method handling
- [x] ELF/SELF/PRX loader with AES decryption
- [x] Audio system with cpal backend and codec support (AAC, MP3, MP4)
- [x] Input system — controller, keyboard, mouse with customizable mappings
- [x] Virtual file system — ISO 9660, PKG, PARAM.SFO
- [x] egui-based UI with game list, debugger, settings, and log viewer
- [x] Configuration system (`config.toml`)
- [x] Cross-platform build support (Linux, Windows, macOS)
- [x] Memory subsystem test suite (128+ tests with benchmarks)
- [x] User manual (`docs/USER_MANUAL.md`)
