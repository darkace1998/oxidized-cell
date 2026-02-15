# Oxidized-Cell — TODO: Getting Games Running

> A prioritized roadmap to go from the current state to actually playing PS3 games.
> Items are grouped by phase and ordered by dependency/priority within each phase.

---

## Phase 0 — Prerequisites & Setup

- [x] **Obtain PS3 firmware (PUP)**
  - Download official PS3UPDAT.PUP from Sony (`scripts/download-firmware.sh`)
  - Install via `--install-firmware /path/to/PS3UPDAT.PUP`
  - Extract LV2 kernel, VSH, and core OS modules into `./firmware/`
  - Needed for: decryption keys, system PRX libraries, font files

- [x] **Verify SELF/ELF decryption**
  - 27 APP key revisions are embedded — test against retail EBOOT.BIN files
  - Ensure NPDRM (digital store) games can be decrypted if user provides `rap` / `act.dat`
  - Test PRX (dynamic library) loading from firmware modules

- [x] **Firmware PRX module loading**
  - Load system PRX libraries from extracted firmware (liblv2, libsysutil, etc.)
  - Map firmware modules to HLE stubs where native loading is impractical
  - Create a firmware module registry that HLE can fall back to

---

## Phase 1 — RSX Graphics Pipeline (Critical)

The Vulkan backend has infrastructure (device, render passes, pipelines, shaders) but the
draw pipeline is not fully connected. This is the single biggest blocker for visual output.

- [x] **Connect vertex buffer binding to draw calls**
  - `submit_vertex_buffer()` exists but vertex data is not bound before `vkCmdDraw`
  - Wire vertex attribute descriptions from RSX state into Vulkan vertex input
  - Support all 16 vertex attribute slots

- [x] **Connect index buffer binding to indexed draws**
  - `draw_indexed()` records `vkCmdDrawIndexed` but no index buffer is bound
  - Allocate GPU-side index buffers and bind before indexed draw calls

- [x] **Bind graphics pipeline before draw calls**
  - `bind_pipeline()` exists but is not called from the draw path
  - Create/cache pipelines keyed by current RSX state (blend, depth, stencil, shaders)
  - Call `vkCmdBindPipeline` at start of each render pass or on state change

- [x] **Bind descriptor sets for textures**
  - Descriptor sets and texture samplers exist but are not bound during draws
  - Wire `SET_TEXTURE_*` NV4097 commands to descriptor set updates
  - Support at least 4 texture units (most games use 1-4)

- [x] **Implement texture upload**
  - Transfer texture data from PS3 memory to Vulkan images
  - Handle PS3 texture formats: DXT1, DXT3, DXT5, A8R8G8B8, R5G6B5
  - Swizzle/unswizzle PS3 tiled texture layouts

- [x] **Shader translator integration**
  - `ShaderTranslator` with `vp_decode`, `fp_decode`, `spirv_gen` modules exist
  - Connect translated SPIR-V to Vulkan pipeline shader stages
  - Handle RSX vertex programs (VP) and fragment programs (FP) from game memory
  - Test with basic clear + triangle shaders

- [x] **Render target / surface configuration**
  - Wire `SET_SURFACE_FORMAT`, `SET_SURFACE_COLOR_*`, `SET_SURFACE_ZETA` to actual framebuffers
  - Support multiple render target formats (ARGB8, FP16, FP32)
  - Handle render-to-texture scenarios

- [x] **Frame presentation (flip)**
  - Ensure `end_frame()` properly presents to the window swapchain
  - Wire GCM `cellGcmSetFlip` → RSX bridge → Vulkan swapchain present
  - Implement double/triple buffering

- [x] **Clear operations**
  - Wire `CLEAR_SURFACE` NV4097 command to `vkCmdClearAttachments`
  - Support color + depth + stencil clears

---

## Phase 2 — HLE Module Wiring (Critical)

114 functions are registered in the HLE dispatcher, but several important modules have full
implementations that are **not wired** to the dispatcher. Games will crash on unresolved imports.

- [ ] **Register cellVdec (Video Decoder) functions**
  - Full H.264/MPEG-2 decoders exist in `cell_vdec.rs` — just needs dispatcher entries
  - Required for: FMV cutscenes, video playback

- [ ] **Register cellAdec (Audio Decoder) functions**
  - LPCM, AC3, ATRAC3, MP3, AAC, WMA decoders in `cell_adec.rs`
  - Required for: in-game audio, music tracks

- [ ] **Register cellDmux (Demultiplexer) functions**
  - PAMF, MPEG2-PS/TS, MP4 parser in `cell_dmux.rs`
  - Required for: multimedia playback pipeline (feeds VDEC + ADEC)

- [ ] **Register cellVpost (Video Post-Processor) functions**
  - Scaling, color conversion in `cell_vpost.rs`
  - Required for: video output scaling, color space conversion

- [ ] **Register cellNetCtl functions**
  - Network state management in `cell_net_ctl.rs`
  - Many games query network state at boot — stub as "not connected"

- [ ] **Register cellHttp / cellSsl functions**
  - HTTP/SSL exist in `cell_http.rs` / `cell_ssl.rs`
  - Stub as failures for offline play; implement later for online features

- [ ] **Register cellKb / cellMouse functions**
  - Keyboard and mouse input managers exist
  - Some games require these even if unused

- [ ] **Implement cellSaveData callback invocation**
  - `cellSaveDataListLoad2` has a TODO: callbacks are queued but not invoked on PPU
  - Games will hang waiting for save data selection callbacks
  - Need to execute `func_list` / `func_stat` / `func_file` callbacks on PPU thread

- [ ] **Flesh out cellPngDec / cellJpgDec / cellGifDec**
  - Decoder handles created but actual pixel decoding is minimal
  - Games use these to decode icons, textures, UI images
  - Wire to actual image decoding (via `image` crate or built-in decoders)

- [ ] **Improve cellFont / cellFontFT rendering**
  - Font loading works, but glyph rasterization is basic
  - Games rely on fonts for all UI text — needs bitmap glyph output

---

## Phase 3 — SPU / SPURS Framework

The SPU interpreter is complete, but the SPURS (SPU Runtime System) task scheduler is
skeletal. Most commercial games use SPURS for multithreaded workloads.

- [ ] **Complete SPURS task scheduling**
  - 9 functions registered but handlers are minimal
  - Implement actual SPU workload dispatch (tasksets, task execution)
  - Handle SPU thread group creation and management

- [ ] **SPU thread group lifecycle**
  - Create → Start → Join/Destroy flow
  - Priority-based scheduling across 6 SPU slots

- [ ] **SPU-PPU synchronization**
  - Mailbox communication (SPU_WR_OUT_MBOX ↔ PPU reads)
  - Signal notification channels
  - Event queue integration with LV2 event system

- [ ] **DMA transfer accuracy**
  - MFC PUT/GET with proper address translation
  - Atomic operations (GET_LLAR, PUT_LLC) for lock-free synchronization
  - List DMA transfers

- [ ] **libsre (SPU Runtime Extensions)**
  - Manager exists but not registered in dispatcher
  - Provides higher-level SPU task management used by many games

---

## Phase 4 — LV2 Kernel Completeness

158 syscalls are implemented. Key gaps that affect game boot:

- [ ] **File I/O syscalls**
  - Verify `sys_fs_open`, `sys_fs_read`, `sys_fs_write`, `sys_fs_close` are in the syscall dispatcher
  - These may be handled through HLE cellFs, but some games use raw syscalls

- [ ] **Memory management syscalls**
  - `sys_memory_allocate`, `sys_memory_free`, `sys_memory_get_user_memory_size`
  - `sys_mmapper_*` for memory-mapped I/O
  - `sys_vm_*` for virtual memory operations

- [ ] **PRX/module loading syscalls**
  - `sys_prx_load_module`, `sys_prx_start_module`, `sys_prx_stop_module`
  - Games load additional modules dynamically at runtime

- [ ] **Timer syscalls**
  - `sys_timer_create`, `sys_timer_connect_event_queue`
  - Used for periodic callbacks and timing

- [ ] **TLS (Thread Local Storage)**
  - Verify TLS region setup (0x28000000 base, 64KB default)
  - Some games use TLS extensively for per-thread state

- [ ] **Unknown/missing syscall logging**
  - Log every unimplemented syscall number with parameters
  - Critical for diagnosing why a specific game crashes

---

## Phase 5 — Audio Pipeline

Audio infrastructure exists (cpal backend, mixer, 48kHz output) but needs game integration.

- [ ] **Connect cellAudio ports to actual audio output**
  - `AudioManager` creates ports, but verify samples flow to `CpalAudioBackend`
  - Games write PCM samples to shared memory → audio mixer → speakers

- [ ] **Audio timing synchronization**
  - Block size = 256 samples at 48kHz ≈ 5.33ms per block
  - Games poll `cellAudioGetPortTimestamp` for sync — must return accurate values
  - Audio thread must run independently of video frame rate

- [ ] **Background music (BGM) playback**
  - `cellBgmPlaybackEnable` / `cellBgmPlaybackDisable` registered but minimal
  - Some games use BGM APIs for menu music

- [ ] **Audio decoder integration**
  - Wire cellAdec decoded audio to audio mixer
  - Support format conversion (ATRAC3 → PCM, AAC → PCM, etc.)

---

## Phase 6 — Input Integration

- [ ] **Connect real gamepad input to cellPad**
  - Input hardware abstraction exists (`oc-input`)
  - Wire OS gamepad events → `PadManager` → `cellPadGetData` responses
  - Map buttons: Cross, Circle, Square, Triangle, L1/R1/L2/R2, L3/R3, D-pad, Start, Select

- [ ] **Keyboard/mouse mapping**
  - Allow keyboard keys to map to PS3 controller buttons
  - Essential for users without a gamepad

- [ ] **Analog stick handling**
  - Proper dead zone calibration
  - Pressure-sensitive button support (PS3 face buttons are analog)

- [ ] **Sixaxis / motion controls**
  - At minimum return neutral values so games don't crash
  - Optional: map to mouse movement or actual gyro input

---

## Phase 7 — Game Compatibility Testing

- [ ] **Test with PS3 homebrew**
  - Simple homebrew apps (PSL1GHT SDK samples) as first targets
  - Verify: boot → display output → input → exit cleanly

- [ ] **Test with simple commercial games**
  - Start with 2D or simple 3D titles
  - Track per-game status: Nothing → Intro → Menu → In-Game → Playable

- [ ] **Create compatibility database**
  - Track tested games and their status
  - Document which HLE functions each game requires
  - Log common crash points and missing stubs

- [ ] **Automated regression testing**
  - Boot test: game loads without crash for N frames
  - Screenshot comparison: verify visual output hasn't regressed
  - Extend existing integration test suite (currently 48 tests)

---

## Phase 8 — Performance & Optimization

- [ ] **PPU JIT compiler**
  - C++ LLVM backend exists via `oc-ffi`
  - Enable JIT mode: hot basic blocks compiled to native code
  - Target: 10-50x speedup over interpreter for CPU-bound games

- [ ] **SPU JIT compiler**
  - Similar LLVM backend for SPU instruction set
  - SPU workloads (physics, audio decode) are performance-critical

- [ ] **RSX command batching**
  - Batch multiple NV4097 commands into single Vulkan submissions
  - Reduce Vulkan API call overhead

- [ ] **Shader caching**
  - Disk-based shader cache exists — verify it's being used
  - Prevent shader compilation stutter during gameplay

- [ ] **Memory access optimization**
  - Profile memory access patterns
  - Consider page-level caching for hot regions

- [ ] **Thread scheduling tuning**
  - Balance PPU/SPU/RSX execution across host CPU cores
  - Avoid starvation of any subsystem

---

## Phase 9 — Polish & Usability

- [ ] **Save state support**
  - Snapshot entire emulator state (CPU, memory, GPU, audio)
  - Save/load from disk for instant resume

- [ ] **Configuration profiles per game**
  - Some games may need specific settings (CPU mode, GPU hacks, etc.)
  - Store in `~/.config/oxidized-cell/games/{GAME_ID}.toml`

- [ ] **Trophy support**
  - Trophy manager exists — wire unlock events to UI notifications
  - Display trophy list per game

- [ ] **Improved error messages**
  - When a game crashes, show which HLE function or syscall was missing
  - Provide actionable guidance ("Game requires cellVdec — video decoding not yet supported")

- [ ] **Controller configuration UI**
  - UI exists for controller config — verify it saves and loads mappings
  - Support multiple controller profiles

- [ ] **Resolution scaling**
  - cellResc module exists for resolution scaling
  - Allow rendering at higher-than-native resolutions

---

## Current Stats

| Component | Status | Detail |
|-----------|--------|--------|
| PPU Interpreter | ✅ Complete | Full PowerPC 970 instruction set |
| SPU Interpreter | ✅ Complete | All SPU instructions + MFC DMA |
| Memory Manager | ✅ Complete | 4GB address space, atomic reservations |
| ELF/SELF Loader | ✅ Complete | 27 embedded decryption keys |
| LV2 Kernel | ✅ Mostly complete | 158 syscalls, threads, sync primitives |
| VFS | ✅ Complete | ISO 9660, PKG, save data, trophies |
| Audio Backend | ✅ Functional | cpal output, 48kHz, 8 ports |
| Input Framework | ✅ Framework ready | DS3, Move, keyboard, mouse |
| UI | ✅ Functional | egui with game list, debugger, settings |
| Debugger | ✅ Functional | PPU/SPU/RSX debuggers, memory viewer |
| RSX Vulkan Backend | ⚠️ Partial | Infrastructure done, draw pipeline disconnected |
| HLE Dispatcher | ⚠️ Partial | 114/200+ functions registered |
| SPURS | ⚠️ Minimal | 9 stubs, no real task execution |
| PPU/SPU JIT | ⚠️ Framework only | LLVM backend exists, needs testing |
| Shader Translation | ⚠️ Framework only | VP/FP decode + SPIR-V gen exists |
| Video/Audio Decode | ❌ Not wired | Full decoders exist, not in dispatcher |
| Network | ❌ Not wired | HTTP/SSL/NetCtl managers exist |

---

## Quick-Start Priority

**To see first visual output from a game, focus on (in order):**

1. ✦ Phase 0 — Firmware setup (decryption keys)
2. ✦ Phase 1 — RSX draw pipeline (vertex buffers → pipeline bind → present)
3. ✦ Phase 2 — Wire missing HLE modules (especially cellVdec, cellNetCtl stubs)
4. ✦ Phase 6 — Input integration (need controller input to navigate menus)
5. ✦ Phase 7 — Test with homebrew first, then simple commercial games
