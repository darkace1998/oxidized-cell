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

- [x] **SPU JIT - Complete LLVM IR Generation**
  - Location: `cpp/src/spu_jit.cpp:564`
  - Full LLVM IR generation for all SPU instructions
  - Implemented comprehensive SIMD instruction support including:
    - RI10-form: ai, ahi, sfi, sfhi, andi, ori, xori, lqd, stqd, ceqi, cgti, clgti
    - RI7-form: shli, roti, rotmi, rotmai (shift/rotate immediates)
    - RI16-form: il, ilh, ilhu, iohl (immediate loads), lqa, stqa
    - RR-form: a, ah, sf, sfh, mpy, mpyu, mpyh (arithmetic)
    - RR-form: and, or, xor, nor, nand, andc, orc, eqv (logical)
    - RR-form: shl, rot, rotm, rotma (shift/rotate)
    - RR-form: ceq, ceqb, cgt, clgt (compare)
    - RR-form: lqx, stqx (indexed load/store)
    - RR-form: fa, fs, fm, fceq, fcgt (floating-point)
    - RRR-form: selb, fma, fms, fnms, mpya, shufb

- [x] **Thread ID Management**
  - Location: `crates/oc-integration/src/runner.rs:300`
  - Use a dedicated thread ID counter instead of thread count
  - Ensures unique IDs even after thread removal
  - Implemented using AtomicU32 counters for both PPU and SPU threads

### RSX Graphics

- [x] **Vulkan Framebuffer Readback**
  - Location: `crates/oc-rsx/src/backend/vulkan.rs:2301`
  - Implemented actual framebuffer readback using staging buffer and `vkCmdCopyImageToBuffer`
  - Added `copy_image_to_buffer` helper function
  - Extended `transition_image_layout` to support TRANSFER_SRC transitions
  - Required for screenshot functionality and frame output

- [x] **Vertex Buffer Submission**
  - Location: `crates/oc-rsx/src/thread.rs:356`
  - Implemented vertex buffer submission to Vulkan backend
  - Added `submit_vertex_buffer` and `submit_index_buffer` methods to GraphicsBackend trait
  - Vulkan backend creates GPU buffers with CPU-visible memory for direct upload
  - Thread flushes vertex data from RSX memory using attribute format/offset state
  - Critical for actual game rendering

- [x] **Post-Processing Pipeline Integration**
  - Location: `crates/oc-rsx/src/postprocess.rs:209`
  - Implemented full Vulkan rendering pipeline integration for post-processing effects:
    - Added `VulkanPostProcessResources` struct to manage Vulkan resources
    - Created `init_vulkan()` method to initialize post-process render pass, descriptor sets, pipeline layout
    - Implemented intermediate render target creation with ping-pong rendering support
    - Added `process_with_cmd_buffer()` for recording post-process passes into command buffers
    - Created `PostProcessPushConstants` struct for effect parameters via push constants
    - Implemented `shutdown()` method for proper Vulkan resource cleanup
    - Added pipeline registration for effect-specific shader pipelines
    - Full-screen triangle rendering (no vertex buffer needed)

---

## 🎮 HLE Module Improvements

### Audio Codecs

- [x] **AAC Decoder Implementation**
  - Locations: `crates/oc-audio/src/codec.rs:228`, `crates/oc-hle/src/cell_adec.rs:155`
  - Integrated with symphonia library for actual AAC decoding
  - Supports ADTS-framed AAC and raw AAC data with automatic header generation
  - Decodes AAC LC profile at various sample rates and channel configurations

- [x] **ATRAC3+ Decoder Implementation**
  - Locations: `crates/oc-audio/src/codec.rs:275`, `crates/oc-hle/src/cell_adec.rs:210`
  - Implemented full ATRAC3+ decoding (Sony proprietary format)
  - Features:
    - 16 subbands with IMDCT (Modified Discrete Cosine Transform)
    - Gain control processing with interpolation
    - Joint stereo (M/S) processing
    - QMF synthesis filter bank
    - Overlap-add reconstruction
    - Support for up to 8 channels
    - Frame parsing with variable bitrates
  - Required for many PS3 games' audio

- [x] **MP3 Decoder Implementation**
  - Locations: `crates/oc-audio/src/codec.rs:450`, `crates/oc-hle/src/cell_adec.rs:182`
  - Integrated with symphonia library for actual MP3 decoding
  - Supports MPEG-1/2 Layer III at various bitrates and sample rates
  - Features:
    - Frame sync detection and parsing
    - Uses symphonia's built-in MP3 decoder for:
      - Huffman decoding of quantized spectral data
      - Inverse quantization and dequantization
      - IMDCT transform
      - Polyphase synthesis filter bank
      - Joint stereo processing
    - 1152 samples per channel per frame output

### Video Codecs

- [x] **H.264/AVC Decoder**
  - Location: `crates/oc-hle/src/cell_vdec.rs:176`
  - Implemented full H.264/AVC video decoding
  - Features:
    - NAL unit parsing with Annex B byte stream support
    - RBSP extraction (emulation prevention byte removal)
    - Sequence Parameter Set (SPS) parsing for video dimensions
    - Picture Parameter Set (PPS) parsing
    - Slice header parsing (I, P, B slice types)
    - I-frame decoding with DC intra prediction
    - P-frame decoding with motion compensation
    - Deblocking filter for artifact reduction
    - Reference frame management (up to 16 frames)
    - YUV420 output format
  - Supported profiles: Baseline (66), Main (77), High (100)
  - Required for FMV/cutscene playback

- [x] **MPEG-2 Decoder**
  - Location: `crates/oc-hle/src/cell_vdec.rs:1146`
  - Implemented full MPEG-2 video decoding
  - Features:
    - Start code parsing (sequence header, GOP, picture header, extensions)
    - Sequence header parsing for video dimensions and aspect ratio
    - Sequence extension parsing for progressive/interlaced modes
    - Picture header parsing (I, P, B frame types)
    - Picture coding extension parsing
    - I-frame decoding with DC intra prediction
    - P-frame decoding with forward motion compensation
    - B-frame decoding with bidirectional prediction
    - 8x8 IDCT implementation
    - Reference frame management (forward and backward)
    - YUV420 output format
  - Supported profiles: Simple (5), Main (4), High (1)
  - Required for DVD-quality video playback

### Demuxer

- [x] **PAMF Container Parsing**
  - Location: `crates/oc-hle/src/cell_dmux.rs:128`
  - Implemented full PAMF (PlayStation Audiovisual Media Format) parsing
  - Features:
    - PAMF header parsing (magic, version, data offset, stream count)
    - Stream info table parsing (stream type, coding type, stream ID, offset)
    - PES packet extraction with PTS/DTS timestamp parsing
    - Fallback scanning for PES packets if header is invalid
    - Support for video (0xE0-0xEF) and audio (0xC0-0xDF, 0xBD) streams

- [x] **MPEG-2 PS/TS Parsing**
  - Locations: `crates/oc-hle/src/cell_dmux.rs:348`, `crates/oc-hle/src/cell_dmux.rs:481`
  - Implemented full MPEG-2 Program Stream and Transport Stream parsing
  - MPEG-2 PS Features:
    - Pack header parsing (0x000001BA) with SCR extraction
    - System header parsing (0x000001BB)
    - PES packet parsing with PTS/DTS extraction
    - Support for video (0xE0-0xEF), audio (0xC0-0xDF), and private streams (0xBD)
    - MPEG-1/2 format detection and handling
  - MPEG-2 TS Features:
    - TS packet parsing (188-byte packets with 0x47 sync)
    - PAT (Program Association Table) parsing for PMT PID discovery
    - PMT (Program Map Table) parsing for elementary stream PID discovery
    - Stream type classification (H.264, MPEG-2, AAC, AC-3, etc.)
    - PES packet assembly from fragmented TS packets
    - PTS/DTS timestamp extraction from PES headers

### Image Decoders

- [x] **GIF Decoder Enhancement**
  - Locations: `crates/oc-hle/src/cell_gif_dec.rs:920`, `960`, `1005`
  - Implemented full GIF decoding using GifDecoder backend
  - Features:
    - Full GIF89a header parsing (Logical Screen Descriptor, Global Color Table)
    - Graphics Control Extension parsing for animation and transparency
    - Image Descriptor parsing with local color tables
    - LZW decompression of image data
    - Palette to RGBA color conversion
    - Animation frame handling (multiple frames, timing, disposal methods)
    - Added `parse_header_from_data` and `decode_data` methods to GifDecManager
    - HLE functions updated to use actual decoder backend

### Video Post-Processing

- [x] **YUV ↔ RGB Color Conversion**
  - Locations: `crates/oc-hle/src/cell_vpost.rs:383`, `456`
  - Implemented full YUV to RGB and RGB to YUV color conversions
  - Features:
    - YUV420 to RGBA conversion with proper chroma upsampling
    - RGBA to YUV420 conversion with 2x2 chroma subsampling
    - YUV422 to RGBA conversion with horizontal-only chroma upsampling
    - BT.601 (SDTV) and BT.709 (HDTV) color matrix support
    - Proper coefficient calculations for both standards
  - Required for video playback and capture

---

## 📁 File System & Storage

### Save Data

- [x] **Save Data Directory Operations**
  - Locations: `crates/oc-hle/src/cell_save_data.rs:271`, `288`
  - Create and delete directories through VFS/host filesystem
  - Implemented `create_directory_on_disk` and `delete_directory_from_disk` methods
  - Uses configurable `OXIDIZED_CELL_SAVEDATA` environment variable or fallback path
  - Non-blocking disk operations with graceful error handling

- [x] **PARAM.SFO Format Generation/Parsing**
  - Locations: `crates/oc-vfs/src/formats/sfo.rs`, `crates/oc-vfs/src/savedata.rs:189`, `218`
  - Implemented proper PARAM.SFO binary format generation using `SfoBuilder`
  - Full SFO format support: header, index table, key table, data table
  - Parsing using `Sfo::parse()` method
  - Builder pattern for creating save data SFOs with all required fields
  - Category, Title, Title ID, Save Directory, Detail, Subtitle, Version support
  - Round-trip tested (generate then parse)

### Disc & Package Handling

- [x] **Disc Info Parsing**
  - Location: `crates/oc-vfs/src/disc.rs:139`
  - Parse PARAM.SFO for title and game ID from disc images
  - Implemented `parse_param_sfo_file`, `parse_param_sfo_data`, and `parse_param_sfo_reader` methods
  - Supports both folder-based discs and ISO images
  - Uses existing SFO parser for proper PARAM.SFO parsing

- [x] **PKG Extraction**
  - Locations: `crates/oc-vfs/src/formats/pkg.rs:101`, `157`
  - Parse title ID and content ID from PKG metadata (content ID at offset 48)
  - Extract title ID from content ID format (XX00000-TITLE_ID_00000)
  - Implemented complete PKG extraction logic via `extract_to()` method
  - Added `PkgFileEntry` struct for file table entries
  - Added file table parsing from PKG structure
  - Added `read_file()` method for reading individual files from PKG
  - Added `list_files()` and `info_summary()` utility methods

### Async File I/O

- [x] **Async I/O Operations**
  - Locations: `crates/oc-hle/src/cell_fs.rs:868`, `883`, `1015`, `1096`
  - Implemented thread-based async I/O with completion channel
  - Features:
    - `aio_read`: Spawns background thread to read from file asynchronously
    - `aio_write`: Spawns background thread to write to file asynchronously
    - `aio_wait`: Blocks until request completes with timeout support
    - `aio_poll`: Non-blocking check if request has completed
    - `aio_cancel`: Cancel pending async I/O request
    - `aio_get_result`: Get bytes transferred after completion
  - Completion results communicated via mpsc channel
  - Proper data transfer to guest memory for read operations
  - Positioned I/O support (read/write at specific offset)

---

## 🎛️ Input & Peripherals

### Controller

- [x] **oc-input Subsystem Integration**
  - Location: `crates/oc-hle/src/cell_pad.rs:349`
  - Complete connection to oc-input subsystem for controller data
  - Implemented via `set_input_backend()` and `poll_input()` methods
  - Full button mapping, analog sticks, pressure sensitivity, and sixaxis sensors
  - Rumble/vibration support forwarded to oc-input backend

### Keyboard

- [x] **Input Buffer Operations**
  - Location: `crates/oc-hle/src/cell_kb.rs:495`
  - Implement actual input buffer clearing
  - Clears keyboard data buffer and forwards to oc-input backend if connected

- [x] **LED Status Setting**
  - Location: `crates/oc-hle/src/cell_kb.rs:854`
  - Set actual LED status (Num/Caps/Scroll Lock)
  - Uses `set_led()` method which updates internal state and forwards to backend

### Mouse

- [x] **Mouse Data Retrieval**
  - Locations: `crates/oc-hle/src/cell_mouse.rs:285`, `310`
  - Get actual mouse data and buffered data from oc-input subsystem
  - Returns cached mouse state updated by `poll_input()` when backend is connected
  - Supports both single data retrieval and buffered data list

### Microphone

- [x] **Audio Capture Implementation**
  - Locations: `crates/oc-hle/src/cell_mic.rs:305`, `334`, `360`
  - Start/stop actual audio capture via oc-input MicrophoneManager backend
  - Read actual captured audio data via backend_read_data() method
  
- [x] **Device Info Memory Write**
  - Locations: `crates/oc-hle/src/cell_mic.rs:673`, `693`, `781`
  - Write device count to memory in cell_mic_get_device_count()
  - Write device info struct to memory in cell_mic_get_device_info()
  - Read captured data to buffer and write bytes read in cell_mic_read()

---

## 🎵 Audio System

- [x] **Notification Event Queue**
  - Locations: `crates/oc-hle/src/cell_audio.rs:916`, `931`
  - Set and remove notification event queue for audio manager
  - Implemented via `set_notify_event_queue()` and `remove_notify_event_queue()` methods
  - Event queue keys stored in AudioManager and cleared on quit
  - Added unit tests for event queue management

---

## 🌐 Network

### HTTP Client

- [x] **Request Body Handling**
  - Location: `crates/oc-hle/src/cell_http.rs`
  - Added `request_body` field to TransactionEntry
  - Added `set_request_body()` and `get_request_body()` methods
  - Request body is now passed to HttpBackend::send_request()

- [x] **HTTP Networking Integration**
  - Location: `crates/oc-hle/src/cell_http.rs`
  - Implemented simulated HTTP responses based on method type
  - Response bodies now stored in transaction and returned via recv_response()
  - Support for GET, POST, PUT, DELETE, HEAD, OPTIONS methods
  - Proper content-length tracking and response body retrieval

- [x] **Client Handle Memory Write**
  - Location: `crates/oc-hle/src/cell_http.rs:607`
  - cell_http_create_client() now writes client ID to memory via write_be32()
  - Added 6 new unit tests for HTTP networking

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
