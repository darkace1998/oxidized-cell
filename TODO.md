# Oxidized-Cell — Alpha/Beta Testing Roadmap

**Goal:** Get games loading to a menu screen (XMB-style or in-game) for alpha/beta testing.

> **Current state:** Core emulation (PPU/SPU interpreters, memory, threads, scheduler) is solid.
> Game loading pipeline (ELF/SELF/ISO, PRX/NID resolution) works end-to-end.
> The egui UI can browse games and start emulation.
> **Blockers:** HLE callbacks never execute on PPU, RSX has no present/flip path,
> and most HLE modules are stubs (~15 of ~135 functions implemented).

---

## Phase 0 — Critical-Path Blockers

These issues prevent *any* game from reaching a menu screen.

- [x] **HLE callback execution loop**
  - `cell_sysutil::check_callback()` queues callbacks but the runner never invokes them on the PPU thread.
  - Add a callback pump in `EmulatorRunner::run_frame()` that pops pending callbacks and executes them as PPU function calls (set PC, LR, R3–R5 from callback info, run until return).
  - *Files:* `crates/oc-integration/src/runner.rs`, `crates/oc-hle/src/cell_sysutil.rs`

- [x] **RSX present / flip path**
  - Vulkan backend has no `present()` or `flip()` — rendered framebuffer never reaches the screen.
  - Wire `end_frame()` in the Vulkan backend to copy the active display buffer into a staging buffer and expose it via `get_framebuffer()`.
  - Ensure `EmulatorRunner::get_framebuffer()` returns real RGBA pixel data so the egui emulation view can display it.
  - *Files:* `crates/oc-rsx/src/backend/vulkan.rs`, `crates/oc-rsx/src/backend/mod.rs`, `crates/oc-integration/src/runner.rs`

- [x] **GCM display-buffer flip handling**
  - `cellGcmSetFlip` / `cellGcmSetWaitFlip` must signal the RSX backend to swap display buffers and unblock the PPU.
  - Implement flip-queue logic in `cell_gcm_sys.rs` and bridge it to `oc-rsx`.
  - *Files:* `crates/oc-hle/src/cell_gcm_sys.rs`, `crates/oc-rsx/src/lib.rs`

---

## Phase 1 — Minimum HLE for Boot

The smallest set of HLE functions games call between `main()` and their first rendered frame.

### cellGcmSys (graphics init)
- [x] `cellGcmSetDisplayBuffer` — store framebuffer address/pitch/dimensions; dispatcher now forwards to GcmManager which sends BridgeDisplayBuffer to RSX
- [x] `cellGcmGetConfiguration` — returns local memory size (256 MB), address, I/O size, frequencies
- [x] `cellGcmGetTiledPitchSize` — returns 256-byte aligned pitch
- [x] `cellGcmSetFlipMode` — sets V-sync / immediate mode via GcmManager
- [x] `cellGcmAddressToOffset` / `cellGcmMapMainMemory` — fully implemented memory mapping for RSX access

### cellSysutil (system callbacks & events)
- [x] Verify `cellSysutilRegisterCallback` stores handler correctly — stores func/userdata in up to 4 callback slots
- [x] Verify `cellSysutilCheckCallback` triggers callbacks — Phase 0 pump_hle_callbacks() executes pending callbacks on PPU
- [x] `cellSysutilGetSystemParamInt` — returns language (English), enter button (Cross), date/time format, timezone, game rating
- [x] `cellVideoOutGetState` / `cellVideoOutConfigure` — registered in dispatcher, implementation returns 720p state and applies resolution configuration

### cellGame (boot check)
- [x] `cellGameBootCheck` — initializes GameManager, writes game type (disc), attributes, content size, directory name ("GAME00000")
- [x] `cellGameContentPermit` — registered in dispatcher, writes contentInfoPath and usrdirPath to guest memory
- [x] `cellGameGetParamSfo` — `cellGameGetParamInt`/`cellGameGetParamString` now write values to guest memory from GameManager

### cellPad (controller input)
- [x] `cellPadInit` / `cellPadGetData` — PadManager returns button + analog stick data from oc-input backend
- [x] `cellPadGetInfo2` — now writes full CellPadInfo2 structure (max_connect, now_connect, port_status, device_capability, device_type)
- [x] Ensure input polling runs at frame start in `run_frame()` — poll_input() called at frame start

### cellFs (file I/O)
- [x] `cellFsOpen` / `cellFsRead` / `cellFsClose` — dispatcher functions now use FsManager with VFS backend for real file I/O
- [x] `cellFsStat` / `cellFsFstat` — dispatcher functions now use FsManager to return real file metadata
- [x] `cellFsOpendir` / `cellFsReaddir` — registered in dispatcher, use FsManager to enumerate directories via VFS

---

## Phase 2 — RSX Rendering Pipeline

Get basic 2D/3D graphics on screen so menus are visible.

- [x] **NV4097 command processing** — 100+ method handlers process state updates; `execute_command()` in thread.rs now forwards viewport/scissor/texture state to the backend on `SET_BEGIN_END`, and handles clear/draw_arrays/draw_indexed directly
- [x] **Vertex shader passthrough** — VpSpirVGen generates passthrough SPIR-V (position input → position output) for empty programs; vertex attribute flush reads data from RSX memory and submits to backend
- [x] **Fragment shader passthrough** — FpSpirVGen generates passthrough SPIR-V (outputs white 1,1,1,1) for empty programs; texture sampling supported for non-empty programs
- [x] **Texture upload** — DXT1/DXT3/DXT5 block decompression and RGBA8 support in texture.rs; TextureCache with LRU eviction; async texture loader; detiling for tiled RSX surfaces
- [x] **Render-target → display-buffer blit** — `perform_flip()` calls `end_frame()`/`begin_frame()` and signals flip complete via bridge; Vulkan `get_framebuffer()` does image→staging buffer readback for egui display
- [x] **Null-backend fallback** — null backend now fills framebuffer with the game's clear color (dark blue default) plus an animated white stripe to show the emulator is alive; tracks draw calls per frame

---

## Phase 3 — SPU / SPURS (needed by most commercial titles)

- [x] `cellSpursInitialize` / `cellSpursFinalize` — dispatcher wired to SpursManager; creates SPURS instance with SPU thread group, validates parameters, tracks initialization state
- [x] `cellSpursCreateTaskset` / `cellSpursTasksetAttributeSetName` — new dispatcher registrations; creates tasksets via SpursManager, attribute names accepted for debugging
- [x] `cellSpursCreateTask` — new dispatcher registration; creates task queue + enqueues SPU task with ELF address and context; writes task ID to guest memory
- [x] `cellSpursAttachLv2EventQueue` — dispatcher now forwards queue/port/dynamic args to SpursManager.attach_lv2_event_queue()
- [x] Workload scheduling — `cellSpursSetMaxContention` + `cellSpursSetPriorities` + `cellSpursGetSpuThreadId` registered; SpursManager has get_next_workload(), process_workloads(), schedule_pending_workloads()
- [x] Basic DMA: LV2 syscalls `sys_spu_thread_transfer_data_get` (MFC_GET), `sys_spu_thread_transfer_data_put` (MFC_PUT), `sys_spu_thread_atomic_get` (MFC_GETLLAR), `sys_spu_thread_atomic_put` (MFC_PUTLLC) — all with bounds checking and alignment validation; oc-spu MFC has full DMA engine with timing model

---

## Phase 4 — Audio Playback

- [x] `cellAudioInit` / `cellAudioPortOpen` — dispatcher stubs wired to real AudioManager implementations that handle port lifecycle, channel layout selection, and mixer source creation
- [x] `cellAudioPortStart` — dispatcher now calls cell_audio::cell_audio_port_start() to transition port to Started state, enabling audio sample submission
- [x] Wire `oc-audio` cpal backend to the HLE audio port — EmulatorRunner::new() creates HleAudioMixer, connects it to AudioManager via set_audio_backend(), initializes CpalBackend with mixer callback, audio flows: game → AudioManager::submit_audio() → HleAudioMixer → CpalBackend → speakers
- [x] `cellAudioGetPortTimestamp` — returns timestamp in microseconds from block tag (256 samples at 48 kHz per block ≈ 5333 µs); registered in dispatcher and wired to AudioManager::get_port_timestamp()

---

## Phase 5 — Image & Font Decoding (menus need these)

### Image decoders
- [x] `cellPngDecOpen` / `cellPngDecReadHeader` / `cellPngDecDecodeData` / `cellPngDecClose`
- [x] `cellJpgDecOpen` / `cellJpgDecReadHeader` / `cellJpgDecDecodeData` / `cellJpgDecClose`
- [x] `cellGifDecOpen` / `cellGifDecReadHeader` / `cellGifDecDecodeData` / `cellGifDecClose`
- [x] These are used for icons, splash screens, and in-game menus

### Font rendering
- [x] `cellFontInit` / `cellFontOpenFontMemory` — load TrueType/Type1 font data
- [x] `cellFontCreateRenderer` / `cellFontRenderCharGlyphImage` — rasterize glyphs to a surface
- [x] `cellFontFTInit` / `cellFontFTLoadGlyph` / `cellFontFTSetCharSize` — FreeType path

---

## Phase 6 — Save Data & Misc Utilities ✅

- [x] `cellSaveDataListLoad2` / `cellSaveDataListSave2` — load/save game data (many menus check for existing saves)
- [x] `cellSaveDataDelete2` — delete save entries (+ AutoLoad2/AutoSave2/FixedLoad2/FixedSave2)
- [x] `cellMsgDialogOpen2` — display in-game message dialog overlay (+ Close, ProgressBar)
- [x] `cellRescInit` / `cellRescSetDisplayMode` — resolution scaling for non-native resolutions (wired to real RescManager + SetConvertAndFlip)
- [x] `cellSysutilGetBgmPlaybackStatus` — background music status (+ Enable/Disable BGM playback)

---

## Phase 7 — Testing & Validation

- [x] **Homebrew test ROM** — GCM init/display buffer/flip lifecycle test validates Phase 0–2 pipeline without external ELF
- [x] **HLE integration tests** — `test_null_backend_produces_non_black_framebuffer` and `test_rsx_clear_produces_colored_framebuffer` verify framebuffer output
- [x] **Callback round-trip test** — `test_callback_roundtrip` registers callback, triggers event, pops and verifies func/status/userdata
- [x] **CI gating** — `.github/workflows/ci.yml` runs build + `cargo test` across 6 crates (oc-audio, oc-lv2, oc-rsx, oc-hle, oc-ffi, oc-loader)
- [x] **Screenshot comparison** — `test_screenshot_capture_and_compare` captures two framebuffers and compares pixel-by-pixel (>99.9% match required)

---

## Phase 8 — Polish for Alpha Release

- [ ] **Error overlay** — show HLE stub hits and unimplemented-syscall warnings in the emulation view so testers can report them
- [ ] **Log filtering** — allow testers to filter log output by module (PPU / SPU / RSX / HLE)
- [ ] **Game compatibility database** — add a simple status field (Nothing / Intro / Menu / In-Game / Playable) per tested title
- [ ] **User documentation** — update `docs/USER_MANUAL.md` with alpha-tester instructions (firmware installation, game folder structure, known limitations)
- [ ] **Performance baseline** — measure and log FPS on reference hardware; set minimum target (30 FPS menu navigation)

---

## Quick Reference — Files to Touch per Phase

| Phase | Key Files |
|-------|-----------|
| 0 | `runner.rs`, `cell_sysutil.rs`, `vulkan.rs`, `cell_gcm_sys.rs` |
| 1 | `cell_gcm_sys.rs`, `cell_sysutil.rs`, `cell_game.rs`, `cell_pad.rs`, `cell_fs.rs`, `module.rs` |
| 2 | `vulkan.rs`, `spirv_gen.rs`, `texture.rs`, `backend/mod.rs` |
| 3 | `cell_spurs.rs`, `spu.rs` (lv2), `dma.rs` |
| 4 | `cell_audio.rs`, `oc-audio/src/lib.rs` |
| 5 | `cell_png_dec.rs`, `cell_jpg_dec.rs`, `cell_gif_dec.rs`, `cell_font.rs`, `cell_font_ft.rs` |
| 6 | `cell_save_data.rs`, `cell_resc.rs`, `cell_sysutil.rs` |
| 7 | `oc-integration/tests/`, CI config |
| 8 | `app.rs`, `docs/USER_MANUAL.md` |

---

*Last updated: 2026-02-12*
