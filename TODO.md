# 📋 oxidized-cell Development TODO

This document tracks development tasks, improvements, and known issues for the oxidized-cell PS3 emulator.

---

## 🔥 High Priority

### Core Emulation

- [x] **PPU JIT - Complete LLVM IR Generation**
  - Location: `cpp/src/ppu_jit.cpp:691`
  - Full LLVM IR generation for all PowerPC instructions
  - Implemented comprehensive instruction support including:
    - All integer arithmetic (add, sub, mul, div, logical, shift, rotate)
    - All load/store variants (byte, halfword, word, doubleword)
    - All floating-point operations (arithmetic, FMA, conversions, comparisons)
    - M-form rotate/mask instructions
    - DS-form 64-bit load/store
    - SPR access and CR operations
    - Single-precision floating-point (opcode 59)
    - Comparison immediate instructions

- [ ] **SPU JIT - Complete LLVM IR Generation**
  - Location: `cpp/src/spu_jit.cpp:564`
  - Full LLVM IR generation for all SPU instructions
  - Currently only supports ~15 SIMD instruction patterns

- [ ] **Thread ID Management**
  - Location: `crates/oc-integration/src/runner.rs:300`
  - Use a dedicated thread ID counter instead of thread count
  - Ensures unique IDs even after thread removal

### RSX Graphics

- [ ] **Vulkan Framebuffer Readback**
  - Location: `crates/oc-rsx/src/backend/vulkan.rs:2193`
  - Implement actual framebuffer readback using staging buffer and `vkCmdCopyImageToBuffer`
  - Required for screenshot functionality and frame output

- [ ] **Vertex Buffer Submission**
  - Location: `crates/oc-rsx/src/thread.rs:360`
  - Implement vertex buffer submission to Vulkan backend
  - Critical for actual game rendering

- [ ] **Post-Processing Pipeline Integration**
  - Location: `crates/oc-rsx/src/postprocess.rs:209`
  - Implement actual Vulkan rendering pipeline integration for post-processing effects

---

## 🎮 HLE Module Improvements

### Audio Codecs

- [ ] **AAC Decoder Implementation**
  - Locations: `crates/oc-audio/src/codec.rs:228`, `crates/oc-hle/src/cell_adec.rs:155`
  - Integrate with ffmpeg, symphonia, or similar library for actual AAC decoding
  - Currently returns stub/silence data

- [ ] **ATRAC3+ Decoder Implementation**
  - Locations: `crates/oc-audio/src/codec.rs:275`, `crates/oc-hle/src/cell_adec.rs:210`
  - Implement actual ATRAC3+ decoding (Sony proprietary format)
  - Required for many PS3 games' audio

- [ ] **MP3 Decoder Implementation**
  - Location: `crates/oc-hle/src/cell_adec.rs:182`
  - Integrate with minimp3 or symphonia library

### Video Codecs

- [ ] **H.264/AVC Decoder**
  - Location: `crates/oc-hle/src/cell_vdec.rs:176`
  - Implement actual H.264/AVC video decoding
  - Required for FMV/cutscene playback

- [ ] **MPEG-2 Decoder**
  - Location: `crates/oc-hle/src/cell_vdec.rs:206`
  - Implement actual MPEG-2 video decoding

### Demuxer

- [ ] **PAMF Container Parsing**
  - Location: `crates/oc-hle/src/cell_dmux.rs:131`
  - Implement actual PAMF (PlayStation Audiovisual Media Format) parsing

- [ ] **MPEG-2 PS/TS Parsing**
  - Locations: `crates/oc-hle/src/cell_dmux.rs:163`, `crates/oc-hle/src/cell_dmux.rs:221`
  - Implement actual MPEG-2 Program Stream and Transport Stream parsing

### Image Decoders

- [ ] **GIF Decoder Enhancement**
  - Locations: `crates/oc-hle/src/cell_gif_dec.rs:876`, `915`, `952`
  - Parse actual GIF header for real dimensions
  - Complete GIF decoding implementation

### Video Post-Processing

- [ ] **YUV ↔ RGB Color Conversion**
  - Locations: `crates/oc-hle/src/cell_vpost.rs:396`, `434`
  - Implement actual YUV to RGB and RGB to YUV color conversions
  - Required for video playback and capture

---

## 📁 File System & Storage

### Save Data

- [ ] **Save Data Directory Operations**
  - Locations: `crates/oc-hle/src/cell_save_data.rs:283`, `292`
  - Create and delete directories through VFS

- [ ] **PARAM.SFO Format Generation/Parsing**
  - Locations: `crates/oc-vfs/src/savedata.rs:190`, `219`
  - Implement proper PARAM.SFO format generation using ParamSfo struct
  - Required for save data metadata

### Disc & Package Handling

- [ ] **Disc Info Parsing**
  - Location: `crates/oc-vfs/src/disc.rs:139`
  - Parse PARAM.SFO for title and game ID from disc images

- [ ] **PKG Extraction**
  - Locations: `crates/oc-vfs/src/formats/pkg.rs:101`, `157`
  - Parse title ID and content ID from PKG metadata
  - Implement complete PKG extraction logic

### Async File I/O

- [ ] **Async I/O Operations**
  - Locations: `crates/oc-hle/src/cell_fs.rs:874`, `923`, `944`
  - Queue and handle actual async I/O operations
  - Implement request completion waiting

---

## 🎛️ Input & Peripherals

### Controller

- [ ] **oc-input Subsystem Integration**
  - Location: `crates/oc-hle/src/cell_pad.rs:349`
  - Complete connection to oc-input subsystem for controller data

### Keyboard

- [ ] **Input Buffer Operations**
  - Location: `crates/oc-hle/src/cell_kb.rs:495`
  - Implement actual input buffer clearing

- [ ] **LED Status Setting**
  - Location: `crates/oc-hle/src/cell_kb.rs:854`
  - Set actual LED status (Num/Caps/Scroll Lock)

### Mouse

- [ ] **Mouse Data Retrieval**
  - Locations: `crates/oc-hle/src/cell_mouse.rs:285`, `310`
  - Get actual mouse data and buffered data from oc-input subsystem

### Microphone

- [ ] **Audio Capture Implementation**
  - Locations: `crates/oc-hle/src/cell_mic.rs:305`, `334`, `360`
  - Start/stop actual audio capture
  - Read actual captured audio data
  
- [ ] **Device Info Memory Write**
  - Locations: `crates/oc-hle/src/cell_mic.rs:662`, `682`, `762`
  - Write device count and info to memory
  - Read captured data to buffer

---

## 🎵 Audio System

- [ ] **Notification Event Queue**
  - Locations: `crates/oc-hle/src/cell_audio.rs:841`, `856`
  - Set and remove notification event queue for audio manager

---

## 🌐 Network

### HTTP Client

- [ ] **Request Body Handling**
  - Location: `crates/oc-hle/src/cell_http.rs:358`, `367`
  - Get actual request body for HTTP methods

- [ ] **HTTP Networking Integration**
  - Location: `crates/oc-hle/src/cell_http.rs:398`
  - Integrate with actual HTTP networking library

- [ ] **Client Handle Memory Write**
  - Location: `crates/oc-hle/src/cell_http.rs:516`
  - Write client handle to memory

---

## ⚙️ System Utilities

### System Information

- [ ] **Disc Info Memory Write**
  - Location: `crates/oc-hle/src/cell_sysutil.rs:1426`
  - Write disc info to memory

- [ ] **Video/Audio Configuration**
  - Locations: `crates/oc-hle/src/cell_sysutil.rs:1591`, `1611`, `1671`, `1691`
  - Read and write video/audio configuration to/from memory

- [ ] **Trophy Info Memory Write**
  - Locations: `crates/oc-hle/src/cell_sysutil.rs:1822`, `1849`
  - Write trophy info and progress percentage to memory

### Game Content

- [ ] **Game Data Checks**
  - Location: `crates/oc-hle/src/cell_game.rs:345-346`
  - Check if game data exists
  - Calculate actual content size

---

## 🔀 SPURS & Job Queues

### Event Queues

- [ ] **SPURS Event Queue Attachment**
  - Locations: `crates/oc-hle/src/cell_spurs.rs:503`, `520`
  - Actually attach and detach event queues to SPURS

### Job Queue

- [ ] **Job Completion Waiting**
  - Locations: `crates/oc-hle/src/cell_spurs_jq.rs:328`, `350`
  - Actually wait for individual job or all jobs to complete

- [ ] **Queue Handle Memory Write**
  - Location: `crates/oc-hle/src/cell_spurs_jq.rs:539`
  - Write queue ID to memory

---

## 🔤 Font Rendering

### Font Library

- [ ] **Font Cache Management**
  - Locations: `crates/oc-hle/src/cell_font.rs:242-243`, `260`
  - Allocate and free font cache
  - Set up default system fonts

- [ ] **Font Loading and Parsing**
  - Locations: `crates/oc-hle/src/cell_font.rs:584-617`
  - Parse font data from memory
  - Load font from file
  - Create and write font handles to memory

- [ ] **Font Rendering Operations**
  - Locations: `crates/oc-hle/src/cell_font.rs:655`, `694-696`, `712-713`
  - Write renderer handle to memory
  - Render character glyph to surface
  - Get horizontal layout metrics

### FreeType Integration

- [ ] **Face Handle Memory Write**
  - Locations: `crates/oc-hle/src/cell_font_ft.rs:420`, `444`
  - Write face handle to memory after loading

---

## 📐 Resolution & Scaling

- [ ] **RSX Backend Scaling**
  - Location: `crates/oc-hle/src/cell_resc.rs:634`
  - Perform actual scaling and flip through RSX backend

- [ ] **Flip Completion Wait**
  - Location: `crates/oc-hle/src/cell_resc.rs:651`
  - Wait for flip operation to complete

- [ ] **RESC Info Memory Writes**
  - Locations: `crates/oc-hle/src/cell_resc.rs:668`, `687`, `706`
  - Write buffer number, size, and time to memory

---

## 🧮 PPU Floating Point

- [ ] **FPSCR Rounding Tracking**
  - Location: `crates/oc-ppu/src/instructions/float.rs:274`
  - Track actual rounding during operations instead of checking fractional part

---

## 📝 Global Manager Refactoring

The following HLE modules use temporary local instances where global managers are needed:

- [ ] **cell_dmux** - `crates/oc-hle/src/cell_dmux.rs:1031`, `1061`
- [ ] **cell_vpost** - `crates/oc-hle/src/cell_vpost.rs:935`
- [ ] **libsre** - `crates/oc-hle/src/libsre.rs:530`
- [ ] **cell_adec** - `crates/oc-hle/src/cell_adec.rs:526`
- [ ] **cell_vdec** - `crates/oc-hle/src/cell_vdec.rs:532`

---

## 📊 Dispatcher Memory Operations

- [ ] **Memory Parameter Writing**
  - Locations: `crates/oc-hle/src/dispatcher.rs:238`, `266`, `282`, `322`, `480`, `524`
  - Write various values (params, configs, port numbers) to memory

---

## 🔧 Technical Debt

### Code Quality

- [ ] Refactor HLE modules to use global context instances consistently
- [ ] Add error handling for all stub implementations
- [ ] Improve logging granularity in HLE functions
- [ ] Add unit tests for stub implementations

### Documentation

- [ ] Document HLE function calling conventions
- [ ] Add inline documentation for complex algorithms
- [ ] Create developer guide for adding new HLE modules

### Performance

- [ ] Profile and optimize hot paths in interpreters
- [ ] Implement instruction caching for PPU/SPU
- [ ] Optimize memory access patterns

---

## 🧪 Testing

### Test Coverage

- [ ] Add integration tests for game loading pipeline
- [ ] Create test fixtures for common PS3 file formats
- [ ] Add fuzzing tests for parser code (ELF, SELF, PKG, etc.)
- [ ] Benchmark tests for PPU/SPU interpreters vs JIT

### Compatibility Testing

- [ ] Test with PS3 homebrew applications
- [ ] Create compatibility database
- [ ] Automated regression testing

---

## 📚 Documentation Tasks

- [ ] Complete PPU instruction documentation (`docs/ppu_instructions.md`)
- [ ] Complete SPU instruction documentation (`docs/spu_instructions.md`)
- [ ] Create architecture overview document
- [ ] Document save state format
- [ ] Create troubleshooting guide

---

## 💡 Future Enhancements

### JIT Compiler

- [ ] Extended instruction coverage for all PPU/SPU instructions
- [ ] Cross-block optimization
- [ ] Profile-guided optimization
- [ ] Custom PowerPC64 backend for better code quality
- [ ] Custom SPU backend for dual-issue pipeline optimization
- [ ] Runtime profiling and hot path detection

### Graphics

- [ ] Complete RSX method handler coverage
- [ ] Shader caching and precompilation
- [ ] Resolution upscaling options
- [ ] Anti-aliasing improvements
- [ ] HDR support

### Audio

- [ ] Low-latency audio mode
- [ ] Audio recording/streaming
- [ ] Surround sound improvements

### User Interface

- [ ] Gamepad configuration UI
- [ ] Memory viewer improvements
- [ ] Shader debugger enhancements
- [ ] Performance overlay

### Networking

- [ ] PSN authentication stubs
- [ ] Network game support
- [ ] UPNP/NAT traversal

---

## 📌 Notes

- **Priority Legend**: 🔥 High | 🔧 Medium | 💡 Low
- **Status**: Most HLE modules are complete (see `docs/HLE_STATUS.md`), but many have stub implementations for edge cases
- **Testing**: Run `cargo test` to verify all tests pass before contributing

---

*Last updated: December 2024*
