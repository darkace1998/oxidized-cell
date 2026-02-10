# 📋 oxidized-cell — Project TODO

> Generated from codebase analysis. Tracks work needed to bring the PS3 emulator from its current early-development state toward game compatibility.

---

## 🔴 High Priority

### HLE Module Completion
The emulator's ability to run games depends on completing the High-Level Emulation modules in `crates/oc-hle/src/`. Most modules are stubs that return `CELL_OK` without real logic.

- [ ] **cellGcmSys** (`cell_gcm_sys.rs`) — Core graphics HLE; needs full GCM command buffer handling
- [ ] **cellSysutil** (`cell_sysutil.rs`) — System utility callbacks, message dialogs, game data access
- [ ] **cellSpurs** (`cell_spurs.rs`) — SPU task/workload scheduler; critical for most commercial titles
- [ ] **cellSpursJq** (`cell_spurs_jq.rs`) — SPURS job queue subsystem
- [ ] **cellFs** (`cell_fs.rs`) — File system operations (open, read, write, stat, directory listing)
- [ ] **cellPad** (`cell_pad.rs`) — Controller input polling and configuration
- [ ] **cellGame** (`cell_game.rs`) — Game content access, boot path, title ID resolution
- [ ] **cellSaveData** (`cell_save_data.rs`) — Save data management (load/save/delete)
- [ ] **cellAudio** (`cell_audio.rs`) — Audio port management and buffer submission
- [ ] **cellFont** / **cellFontFt** (`cell_font.rs`, `cell_font_ft.rs`) — Font rendering support

### Game Loading Pipeline
- [ ] Complete end-to-end game boot sequence in `crates/oc-integration/`
- [ ] Verify ELF/SELF decryption with real firmware keys (`crates/oc-loader/`)
- [ ] Implement PRX dependency resolution and loading order
- [ ] Handle PLT stub patching for HLE dispatch

### JIT Compiler Coverage
- [ ] Expand PPU JIT beyond the current ~20 instructions (`cpp/src/ppu_jit.cpp`)
- [ ] Expand SPU JIT beyond the current ~15 SIMD instructions (`cpp/src/spu_jit.cpp`)
- [ ] Add JIT fast-paths for hot interpreter loops
- [ ] Implement JIT block linking and branch prediction

---

## 🟡 Medium Priority

### HLE Modules — Multimedia & Networking
- [ ] **cellDmux** (`cell_dmux.rs`) — Demultiplexer for media streams
- [ ] **cellVdec** (`cell_vdec.rs`) — Video decoding (H.264, MPEG2)
- [ ] **cellAdec** (`cell_adec.rs`) — Audio decoding (AAC, AT3)
- [ ] **cellVpost** (`cell_vpost.rs`) — Video post-processing
- [ ] **cellPngDec** (`cell_png_dec.rs`) — PNG image decoding
- [ ] **cellJpgDec** (`cell_jpg_dec.rs`) — JPEG image decoding (currently returns placeholder dimensions)
- [ ] **cellGifDec** (`cell_gif_dec.rs`) — GIF image decoding
- [ ] **cellResc** (`cell_resc.rs`) — Resolution/scaling conversion
- [ ] **cellNetCtl** (`cell_net_ctl.rs`) — Network configuration and status
- [ ] **cellHttp** (`cell_http.rs`) — HTTP client support
- [ ] **cellSsl** (`cell_ssl.rs`) — TLS/SSL support
- [ ] **cellMic** (`cell_mic.rs`) — Microphone input (currently stubbed)
- [ ] **cellKb** (`cell_kb.rs`) — Keyboard input
- [ ] **cellMouse** (`cell_mouse.rs`) — Mouse input
- [ ] **libsre** (`libsre.rs`) — SPU runtime environment library
- [ ] **spu_runtime** (`spu_runtime.rs`) — SPU runtime support

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

- [x] PPU interpreter with full instruction set (2,700+ lines)
- [x] SPU interpreter with MFC basics and channel communication
- [x] Memory manager — 4GB virtual address space, 4KB pages, reservation system
- [x] LV2 kernel — threads, mutex, condvar, semaphore, rwlock, timers
- [x] Vulkan graphics backend with NV4097 method handling
- [x] SPIR-V shader compilation infrastructure
- [x] ELF/SELF/PRX loader with AES decryption
- [x] Audio system with cpal backend and codec support (AAC, MP3, MP4)
- [x] Input system — controller, keyboard, mouse with customizable mappings
- [x] Virtual file system — ISO 9660, PKG, PARAM.SFO
- [x] egui-based UI with game list, debugger, settings, and log viewer
- [x] PPU JIT bridge to C++/LLVM (20+ instructions)
- [x] SPU JIT bridge to C++/LLVM (15+ SIMD instructions)
- [x] HLE dispatcher with stub registration and call routing
- [x] Configuration system (`config.toml`)
- [x] Cross-platform build support (Linux, Windows, macOS)
- [x] Memory subsystem test suite (128+ tests with benchmarks)
- [x] User manual (`docs/USER_MANUAL.md`)
