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
- [ ] `cellGcmSetDisplayBuffer` — store framebuffer address/pitch/dimensions (partially done; verify RSX bridge message delivery)
- [ ] `cellGcmGetConfiguration` — return local memory size and address (currently stubbed, returns 0)
- [ ] `cellGcmGetTiledPitchSize` — return valid pitch (currently stubbed)
- [ ] `cellGcmSetFlipMode` — set V-sync / immediate mode (registered, verify implementation)
- [ ] `cellGcmAddressToOffset` / `cellGcmMapMainMemory` — memory mapping for RSX access

### cellSysutil (system callbacks & events)
- [ ] Verify `cellSysutilRegisterCallback` stores handler correctly
- [ ] Verify `cellSysutilCheckCallback` triggers `CELL_SYSUTIL_REQUEST_EXITGAME` on window close
- [ ] `cellSysutilGetSystemParamInt` — return correct language, region, resolution, game-parental-level
- [ ] `cellVideoOutGetState` / `cellVideoOutConfigure` — video output configuration

### cellGame (boot check)
- [ ] `cellGameBootCheck` — validate and return correct content-type / directory path so games proceed past boot
- [ ] `cellGameContentPermit` — permit game-data access
- [ ] `cellGameGetParamSfo` — supply PARAM.SFO values to the game

### cellPad (controller input)
- [ ] `cellPadInit` / `cellPadGetData` — return button + analog stick data from `oc-input`
- [ ] `cellPadGetInfo2` — report connected pad count and capabilities
- [ ] Ensure input polling runs at frame start in `run_frame()`

### cellFs (file I/O)
- [ ] Verify `cellFsOpen` / `cellFsRead` / `cellFsClose` work for `/dev_bdvd` and `/dev_hdd0` paths
- [ ] `cellFsStat` / `cellFsFstat` — return correct file size and type (already implemented; integration-test)
- [ ] `cellFsOpendir` / `cellFsReaddir` — directory listing for game-data enumeration

---

## Phase 2 — RSX Rendering Pipeline

Get basic 2D/3D graphics on screen so menus are visible.

- [ ] **NV4097 command processing** — process minimum command set: clear, draw arrays, draw indexed, set render target, set viewport/scissor, texture bind
- [ ] **Vertex shader passthrough** — translate a simple VP (position + texcoord) through SPIR-V pipeline
- [ ] **Fragment shader passthrough** — translate a simple FP (texture sample → output) through SPIR-V pipeline
- [ ] **Texture upload** — DXT1/DXT5 and RGBA8 textures from guest memory to Vulkan images
- [ ] **Render-target → display-buffer blit** — copy rendered image to the active display buffer for presentation
- [ ] **Null-backend fallback** — ensure the null graphics backend returns a solid-color framebuffer so the UI still shows something during development

---

## Phase 3 — SPU / SPURS (needed by most commercial titles)

- [ ] `cellSpursInitialize` / `cellSpursFinalize` — create SPURS instance with SPU thread group
- [ ] `cellSpursCreateTaskset` / `cellSpursTasksetAttributeSetName` — task set management
- [ ] `cellSpursCreateTask` — enqueue SPU tasks
- [ ] `cellSpursAttachLv2EventQueue` — event notification between PPU and SPU
- [ ] Workload scheduling — pick highest-priority ready workload and dispatch to SPU thread
- [ ] Basic DMA: `MFC_PUT` / `MFC_GET` / `MFC_GETLLAR` / `MFC_PUTLLC` working between SPU LS ↔ main memory

---

## Phase 4 — Audio Playback

- [ ] `cellAudioInit` / `cellAudioPortOpen` — initialize audio output port
- [ ] `cellAudioPortStart` — begin pulling PCM samples from game-supplied ring buffer
- [ ] Wire `oc-audio` cpal backend to the HLE audio port so sound reaches speakers
- [ ] `cellAudioGetPortTimestamp` — return timing info for A/V sync

---

## Phase 5 — Image & Font Decoding (menus need these)

### Image decoders
- [ ] `cellPngDecOpen` / `cellPngDecReadHeader` / `cellPngDecDecodeData` / `cellPngDecClose`
- [ ] `cellJpgDecOpen` / `cellJpgDecReadHeader` / `cellJpgDecDecodeData` / `cellJpgDecClose`
- [ ] `cellGifDecOpen` / `cellGifDecReadHeader` / `cellGifDecDecodeData` / `cellGifDecClose`
- [ ] These are used for icons, splash screens, and in-game menus

### Font rendering
- [ ] `cellFontInit` / `cellFontOpenFontMemory` — load TrueType/Type1 font data
- [ ] `cellFontCreateRenderer` / `cellFontRenderCharGlyphImage` — rasterize glyphs to a surface
- [ ] `cellFontFTInit` / `cellFontFTLoadGlyph` / `cellFontFTSetCharSize` — FreeType path

---

## Phase 6 — Save Data & Misc Utilities

- [ ] `cellSaveDataListLoad2` / `cellSaveDataListSave2` — load/save game data (many menus check for existing saves)
- [ ] `cellSaveDataDelete2` — delete save entries
- [ ] `cellMsgDialogOpen2` — display in-game message dialog overlay
- [ ] `cellRescInit` / `cellRescSetDisplayMode` — resolution scaling for non-native resolutions
- [ ] `cellSysutilGetBgmPlaybackStatus` — background music status (some games query this at boot)

---

## Phase 7 — Testing & Validation

- [ ] **Homebrew test ROM** — create or source a minimal PS3 homebrew ELF that exercises: GCM init → clear screen → draw textured quad → flip (validates phases 0–2)
- [ ] **HLE integration tests** — add Rust tests in `oc-integration` that load an ELF, run N frames, and assert framebuffer is non-black
- [ ] **Callback round-trip test** — register a sysutil callback, trigger an event, assert the callback executes
- [ ] **CI gating** — run `cargo test -p oc-audio -p oc-lv2 -p oc-rsx -p oc-hle -p oc-ffi -p oc-loader` in CI and require pass for merge
- [ ] **Screenshot comparison** — capture reference screenshots from known-good homebrew and compare in CI

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
