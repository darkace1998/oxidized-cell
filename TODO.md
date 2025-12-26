# 📋 Oxidized-Cell Development TODO

This document outlines the complete development roadmap for oxidized-cell, a PlayStation 3 emulator written in Rust and C++.

---

## 📊 Project Status Summary

| Component | Status | Completion | Priority |
|-----------|--------|------------|----------|
| Memory Management | ✅ Complete | 100% | - |
| PPU Interpreter | ✅ Complete | 95% | Low |
| SPU Interpreter | ✅ Complete | 95% | Low |
| RSX/Vulkan Backend | ✅ Complete | 95% | Medium |
| JIT Compilation | ✅ Complete | 100% | Low |
| LV2 Kernel | ✅ Complete | 95% | Low |
| Audio System | ✅ Complete | 85% | Medium |
| Input System | ✅ Complete | 80% | Medium |
| VFS | ✅ Complete | 80% | Medium |
| ELF/Game Loader | ✅ Complete | 90% | Low |
| HLE Modules | 🚧 In Progress | 90% | Medium |
| User Interface | 🚧 In Progress | 15% | Medium |
| Game Loading Pipeline | ❌ Not Started | 0% | **HIGH** |
| Debugging Tools | 🔨 Mostly Complete | 70% | Low |

---

## 🎯 High Priority Tasks

### 1. HLE Module Implementation (Critical for Game Execution)

The HLE (High-Level Emulation) modules are essential for running PS3 games. Currently at ~90% completion.

#### HLE Infrastructure
- [x] **Global HLE Context** - Centralized manager instances
  - [x] Create HleContext to hold all manager instances
  - [x] Implement thread-safe access via RwLock
  - [x] Provide get_hle_context() and get_hle_context_mut() accessors
  - [x] Add reset_hle_context() for testing/cleanup
  - [x] Add GcmManager to global context
  - [x] Add SpursManager to global context

#### Graphics Modules
- [x] **cellGcmSys** - RSX Graphics Command Management (Connected to global context)
  - [x] Implement init through global manager
  - [x] Implement set_flip_mode through global manager
  - [x] Implement set_flip through global manager
  - [x] Implement set_display_buffer through global manager
  - [x] Implement get_configuration through global manager
  - [x] Implement address_to_offset through global manager
  - [x] Document backend integration points (RSX command buffer, memory allocation, display settings)
  - [x] Document memory write operations for configuration and offset data
  - [ ] Integrate with actual RSX backend (requires oc-rsx integration)
  - [ ] Implement command buffer submission (requires oc-rsx integration)
  - [ ] Add texture management functions (requires oc-rsx integration)
  - [ ] Implement render target configuration (requires oc-rsx integration)

- [x] **cellResc** - Resolution Scaler
  - [x] Implement RescManager with init/exit
  - [x] Implement set_display_mode through global manager
  - [x] Implement set_src/set_dsts through global manager
  - [x] Implement convert_and_flip through global manager
  - [x] Implement get_num_display_buffers/get_display_buffer_size
  - [x] Document backend integration points for scaling operations
  - [x] Document memory write operations for display buffer info and timing
  - [ ] Integrate with actual RSX backend for scaling (requires oc-rsx integration)

#### System Modules
- [x] **cellSysutil** - System Utilities (Connected to global context)
  - [x] Implement system callbacks
  - [x] Implement check_callback through global manager
  - [x] Get/set system parameters (int/string)
  - [x] Document memory write operations for parameter values
  - [ ] Add dialog support (game data, save data, etc.) (requires implementation)
  - [ ] Implement PSID/account handling (requires implementation)
  - [ ] Add disc detection functions (requires implementation)

- [x] **cellGame** - Game Data Management (Connected to global context)
  - [x] Implement boot_check through global manager
  - [x] Implement data_check through global manager
  - [x] Implement content_permit through global manager
  - [x] Implement content_error_dialog through global manager
  - [x] Implement get_param_int/string through global manager
  - [x] Implement get_local_web_content_path through global manager
  - [x] Document VFS integration points for PARAM.SFO reading
  - [ ] Add actual PARAM.SFO reading/writing (requires oc-vfs integration)
  - [ ] Support game data installation (requires oc-vfs integration)
  - [ ] Handle game updates (requires oc-vfs integration)

- [x] **cellSaveData** - Save Data Management (Connected to global context)
  - [x] Implement list_load2/list_save2 through global manager
  - [x] Implement delete2 through global manager
  - [x] Implement fixed_load2/fixed_save2 through global manager
  - [x] Document VFS integration points for save data operations
  - [ ] Connect to VFS backend (requires oc-vfs integration)
  - [ ] Handle save data encryption (requires oc-vfs integration)

#### SPU/Threading Modules
- [x] **cellSpurs** - SPU Runtime System (Connected to global context)
  - [x] Implement initialize/finalize through global manager
  - [x] Implement attach/detach event queue through global manager
  - [x] Implement set_priorities through global manager
  - [x] Implement get_spu_thread_id through global manager
  - [x] Document backend integration points for SPU operations
  - [x] Document memory read/write operations for priorities and thread IDs
  - [ ] Implement task queue management (requires SPU backend)
  - [ ] Add workload scheduling (requires SPU backend)
  - [ ] Support job chains (requires SPU backend)
  - [ ] Implement taskset operations (requires SPU backend)
  - [ ] Add event flags and barriers (requires SPU backend)

- [x] **cellSpursJq** - SPURS Job Queue
  - [x] Implement SpursJqManager with init/finalize
  - [x] Implement create_queue/destroy_queue through global manager
  - [x] Implement push_job through global manager
  - [x] Implement sync_job/sync_all through global manager
  - [x] Implement abort_job through global manager
  - [x] Document backend integration points for job operations
  - [x] Document memory write operations for queue IDs
  - [ ] Integrate with actual SPU job execution (requires SPU backend)

#### Input Modules
- [x] **cellPad** - Controller Input (Connected to global context)
  - [x] Implement init/end through global manager
  - [x] Implement get_info/get_info2 through global manager
  - [x] Implement get_data through global manager
  - [x] Implement get_capability_info through global manager
  - [x] Document backend integration points for controller data
  - [x] Document memory write operations for info and data structures
  - [ ] Connect to oc-input backend (requires oc-input integration)
  - [ ] Add rumble/vibration support (requires oc-input integration)
  - [ ] Support multiple controllers (requires oc-input integration)

- [x] **cellKb** - Keyboard Input
  - [x] Implement KbManager with init/end
  - [x] Implement get_info through global manager
  - [x] Implement read through global manager
  - [x] Implement set_read_mode/set_code_type through global manager
  - [x] Support multiple keyboard layouts
  - [x] Document backend integration points for keyboard data
  - [x] Document memory write operations for info and data structures
  - [ ] Connect to oc-input backend (requires oc-input integration)

- [x] **cellMouse** - Mouse Input
  - [x] Implement MouseManager with init/end
  - [x] Implement get_info through global manager
  - [x] Implement get_data/get_data_list through global manager
  - [x] Implement get_raw_data through global manager
  - [x] Add button state handling
  - [x] Document backend integration points for mouse data
  - [x] Document memory write operations for info and data structures
  - [ ] Connect to oc-input backend (requires oc-input integration)

#### Audio Modules
- [x] **cellAudio** - Audio Output (Connected to global context)
  - [x] Implement init/quit through global manager
  - [x] Implement port open/close through global manager
  - [x] Implement port start/stop through global manager
  - [x] Document backend integration points for audio operations
  - [x] Document memory read/write operations for parameters and config
  - [ ] Connect to oc-audio backend (requires oc-audio integration)
  - [ ] Add mixing support (requires oc-audio integration)

- [x] **cellMic** - Microphone Input
  - [x] Implement MicManager with init/end
  - [x] Implement get_device_count/get_device_info through global manager
  - [x] Implement open/close through global manager
  - [x] Implement start/stop through global manager
  - [x] Implement read through global manager
  - [x] Add device enumeration
  - [x] Document backend integration points for audio capture
  - [x] Document memory write operations for device info and data
  - [ ] Connect to actual audio capture backend (requires audio capture backend)

#### File System Modules
- [x] **cellFs** - File System (Connected to global context)
  - [x] Implement close through global manager
  - [x] Implement closedir through global manager
  - [x] Document backend integration points for file operations
  - [x] Document memory read/write operations for paths, file descriptors, and data
  - [ ] Connect to oc-vfs backend (requires oc-vfs integration)
  - [ ] Implement file read/write operations (requires oc-vfs integration)
  - [ ] Add directory operations (requires oc-vfs integration)
  - [ ] Support asynchronous I/O (requires oc-vfs integration)

#### Media Decoding Modules
- [x] **cellVdec** - Video Decoder (Connected to global context)
  - [x] Implement open/close through global manager
  - [x] Implement start/end sequence through global manager
  - [x] Implement decode_au through global manager
  - [x] Document backend integration points for video decoding
  - [x] Document memory operations for picture item retrieval
  - [ ] Implement H.264/AVC decoding backend (requires video codec library)
  - [ ] Add MPEG-2 support (requires video codec library)
  - [ ] Support various profiles (requires video codec library)

- [x] **cellAdec** - Audio Decoder (Connected to global context)
  - [x] Implement open/close through global manager
  - [x] Implement start/end sequence through global manager
  - [x] Implement decode_au through global manager
  - [x] Document backend integration points for audio decoding
  - [x] Document memory operations for PCM item retrieval
  - [ ] Implement AAC decoding backend (requires audio codec library)
  - [ ] Add MP3 support (requires audio codec library)
  - [ ] Support ATRAC3+ (requires audio codec library)

- [x] **cellDmux** - Demultiplexer (Connected to global context)
  - [x] Implement open/close through global manager
  - [x] Implement set_stream/reset_stream through global manager
  - [x] Implement enable_es/disable_es through global manager
  - [x] Document backend integration points for stream demuxing
  - [ ] Implement container parsing backend (requires demuxer library)
  - [ ] Add stream separation (requires demuxer library)

- [x] **cellVpost** - Video Post-Processing (Connected to global context)
  - [x] Implement open/close through global manager
  - [x] Implement exec through global manager
  - [x] Document backend integration points for video processing
  - [ ] Implement color conversion (requires video processing backend)
  - [ ] Add scaling support (requires video processing backend)

#### Image Decoding Modules
- [x] **cellPngDec** - PNG Decoder (Connected to global context)
  - [x] Implement create/destroy through global manager
  - [x] Implement open/close through global manager
  - [x] Implement read_header through global manager
  - [x] Implement set_parameter through global manager
  - [x] Implement decode_data through global manager
  - [x] Document backend integration points for PNG decoding
  - [ ] Implement actual PNG decoding backend (requires PNG decoder library)
  - [ ] Support various color formats (requires PNG decoder library)

- [x] **cellJpgDec** - JPEG Decoder (Connected to global context)
  - [x] Implement create/destroy through global manager
  - [x] Implement open/close through global manager
  - [x] Implement read_header through global manager
  - [x] Implement decode_data through global manager
  - [x] Document backend integration points for JPEG decoding
  - [ ] Implement actual JPEG decoding backend (requires JPEG decoder library)
  - [ ] Add progressive JPEG support (requires JPEG decoder library)

- [x] **cellGifDec** - GIF Decoder (Connected to global context)
  - [x] Implement create/destroy through global manager
  - [x] Implement open/close through global manager
  - [x] Implement read_header through global manager
  - [x] Document backend integration points for GIF decoding
  - [ ] Implement GIF decoding backend (requires GIF decoder library)
  - [ ] Support animations (requires GIF decoder library)

#### Network Modules
- [x] **cellNetCtl** - Network Control (Connected to global context)
  - [x] Implement init/term through global manager
  - [x] Implement get_state through global manager
  - [x] Implement add/remove handler through global manager
  - [x] Implement start/unload dialog through global manager
  - [x] Document backend integration points for network operations
  - [x] Document memory write operations for state, info, and handler IDs
  - [ ] Connect to actual network backend (requires network backend)
  - [ ] Support network configuration (requires network backend)

- [x] **cellHttp** - HTTP Client (Connected to global context)
  - [x] Implement init/end through global manager
  - [x] Implement create/destroy client through global manager
  - [x] Implement create/destroy transaction through global manager
  - [x] Implement send_request/recv_response through global manager
  - [x] Implement add_request_header through global manager
  - [x] Implement get_status_code through global manager
  - [x] Implement set_proxy through global manager
  - [x] Document backend integration points for HTTP operations
  - [x] Document memory write operations for client handles
  - [ ] Connect to actual HTTP networking backend (requires HTTP client library)
  - [ ] Add HTTPS support (requires TLS/SSL library)

- [x] **cellSsl** - SSL/TLS (Connected to global context)
  - [x] Implement init/end through global manager
  - [x] Implement certificate loader through global manager
  - [x] Implement certificate unload through global manager
  - [x] Implement cert_get_serial_number through global manager
  - [x] Implement cert_get_public_key through global manager
  - [x] Implement cert_get_rsa_modulus/exponent through global manager
  - [x] Implement cert_get_not_before/not_after through global manager
  - [x] Implement cert_get_subject_name through global manager
  - [x] Implement cert_get_issuer_name through global manager
  - [x] Document backend integration points for SSL/TLS operations
  - [ ] Implement TLS connections (requires TLS/SSL library)
  - [ ] Add full certificate handling (requires TLS/SSL library)

#### Font Modules
- [x] **cellFont** - Font Library (Connected to global context)
  - [x] Implement init/end through global manager
  - [x] Implement close_font through global manager
  - [x] Implement create/destroy_renderer through global manager
  - [x] Document backend integration points for font operations
  - [x] Document memory write operations for font handles and glyph data
  - [ ] Implement font rendering backend (requires font rendering library)
  - [ ] Support various font formats (requires font rendering library)

- [x] **cellFontFT** - FreeType Font Library
  - [x] Implement FontFtManager with init/end
  - [x] Implement open_font_memory/open_font_file through global manager
  - [x] Implement close_font through global manager
  - [x] Implement set_char_size/set_pixel_size through global manager
  - [x] Implement load_glyph/get_char_index through global manager
  - [x] Document backend integration points for FreeType operations
  - [x] Document memory write operations for face handles
  - [ ] Integrate with actual FreeType backend (requires FreeType library)

#### Regular Expression Modules
- [x] **libsre** - Regular Expressions (Connected to global context)
  - [x] Implement compile through global manager
  - [x] Implement free through global manager
  - [x] Implement match through global manager
  - [x] Implement search through global manager
  - [x] Implement replace through global manager
  - [x] Implement get_error through global manager
  - [x] Document backend integration points for regex operations
  - [ ] Integrate actual regex matching backend (requires regex library)

---

### 2. Game Loading Pipeline (Critical)

The game loading pipeline connects all components to enable game execution.

- [ ] **Game Discovery**
  - [ ] Implement game directory scanning (partially done in GameScanner)
  - [ ] Parse PARAM.SFO metadata
  - [ ] Extract game icons and backgrounds
  - [ ] Cache game database

- [ ] **EBOOT.BIN Loading**
  - [ ] Parse EBOOT.BIN format
  - [ ] Handle encrypted executables
  - [ ] Load PRX dependencies

- [ ] **PRX Module Loading**
  - [ ] Implement dynamic PRX loading
  - [ ] Resolve module imports/exports
  - [ ] Handle NID (Native ID) resolution
  - [ ] Support stub libraries

- [ ] **Memory Layout**
  - [ ] Initialize PS3 memory regions (done)
  - [ ] Set up stack for main thread
  - [ ] Configure TLS areas
  - [ ] Initialize kernel objects

- [ ] **Main Thread Creation**
  - [ ] Create initial PPU thread
  - [ ] Set up register state
  - [ ] Initialize thread local storage
  - [ ] Start execution

---

## 🔨 Medium Priority Tasks

### 3. LV2 Kernel Enhancements (95% Complete)

#### Thread Management
- [x] Implement thread priorities properly
- [x] Add thread affinity support
- [ ] Improve context switching
- [x] Support thread-local storage

#### Synchronization Primitives
- [x] Improve mutex implementation
- [x] Add event flag support
- [x] Implement reader-writer locks properly
- [x] Add barrier support

#### Memory Syscalls
- [x] Implement mmap/munmap properly
- [x] Add memory attribute handling
- [ ] Support large pages

#### Time Management
- [x] Improve timer accuracy
- [x] Add high-resolution timers
- [x] Implement usleep properly

### 4. User Interface Improvements (15% → 75%)

#### Main Window
- [ ] Implement game grid view with icons
- [ ] Add game search/filter
- [ ] Support game categories
- [ ] Add recent games list

#### Emulation View
- [ ] Connect RSX output to display
- [ ] Add fullscreen support
- [ ] Implement frame rate limiting
- [ ] Add frame skipping option

#### Settings
- [ ] CPU settings (interpreter/JIT, threads)
- [ ] GPU settings (resolution, scaling)
- [ ] Audio settings (backend, volume)
- [ ] Input settings (controller mapping)
- [ ] Path settings (game directories)

#### Debugger View
- [ ] PPU register display
- [ ] SPU register display
- [ ] Memory hex editor
- [ ] Disassembly view
- [ ] Breakpoint management
- [ ] Call stack view

### 5. RSX/Graphics Improvements (95% → 100%)

- [ ] Implement missing NV4097 methods
- [ ] Add shader caching
- [ ] Improve texture sampling accuracy
- [ ] Fix depth buffer handling
- [ ] Add anti-aliasing support
- [ ] Implement vertex processing optimizations
- [ ] Add asynchronous texture loading

### 6. Audio System Improvements (85% → 100%)

- [ ] Implement proper sample rate conversion
- [ ] Add audio mixing improvements
- [ ] Support all audio formats
- [ ] Improve latency
- [ ] Add audio streaming support

### 7. Input System Improvements (80% → 100%)

- [ ] Add pressure-sensitive button support
- [ ] Implement motion controls
- [ ] Add touchpad support (for dualshock 4)
- [ ] Support multiple controller types
- [ ] Improve input latency

### 8. VFS Improvements (80% → 100%)

- [ ] Implement remaining disc formats
- [ ] Add PKG installation support
- [ ] Improve ISO performance
- [ ] Add network path support

---

## 📌 Low Priority Tasks

### 9. PPU Interpreter Improvements (95% → 100%)

- [ ] Implement remaining privileged instructions
- [ ] Add accurate exception handling
- [ ] Improve cycle counting accuracy
- [ ] Add trace logging for debugging

### 10. SPU Interpreter Improvements (95% → 100%)

- [ ] Implement remaining floating-point instructions
- [ ] Add all permute/shuffle variants
- [ ] Improve timing accuracy
- [ ] Add hint instruction support

### 11. JIT Optimizations

- [ ] Add more PPU instructions to JIT
- [ ] Optimize hot code paths
- [ ] Add block linking
- [ ] Implement profiling-guided optimization

### 12. Debugging Tools

- [ ] Add memory watchpoints
- [ ] Implement trace recording/replay
- [ ] Add RSX command buffer debugging
- [ ] Implement SPU debugger improvements
- [ ] Add performance profiler

---

## 🧪 Testing Tasks

### Unit Tests
- [ ] Add more PPU instruction tests
- [ ] Add more SPU instruction tests
- [ ] Add RSX method tests
- [ ] Add HLE module tests

### Integration Tests
- [ ] Test game loading pipeline
- [ ] Test multi-threaded scenarios
- [ ] Test SPU-PPU communication
- [ ] Test memory mapping

### Compatibility Testing
- [ ] Test with PS3 homebrew
- [ ] Create game compatibility database
- [ ] Document known issues per game

---

## 📚 Documentation Tasks

- [ ] Document PPU instruction implementation details
- [ ] Document SPU instruction implementation details
- [ ] Create RSX method reference
- [ ] Write HLE module documentation
- [ ] Create contributing guidelines
- [ ] Add code style guide
- [ ] Write architecture overview

---

## 🔧 Build & Infrastructure

- [ ] Set up CI/CD pipeline
- [ ] Add Windows build support
- [ ] Add macOS build support
- [ ] Create release packaging
- [ ] Add automated testing in CI
- [ ] Set up code coverage reporting
- [ ] Add benchmarking infrastructure

---

## 📁 Crate-Specific TODOs

### oc-core
- [ ] Improve configuration validation
- [ ] Add runtime configuration reloading
- [ ] Implement proper logging levels

### oc-memory
- [x] Memory manager implementation ✅
- [x] Page table management ✅
- [x] Reservation system ✅
- [ ] Add memory statistics/profiling

### oc-ppu
- [x] Full interpreter implementation ✅
- [x] VMX/AltiVec support ✅
- [x] Breakpoint support ✅
- [ ] Improve performance

### oc-spu
- [x] Full interpreter implementation ✅
- [x] MFC DMA engine ✅
- [x] Channel communication ✅
- [ ] Add isolation mode

### oc-rsx
- [x] Vulkan backend ✅
- [x] NV4097 method handlers ✅
- [x] Texture management ✅
- [ ] Shader cache persistence

### oc-lv2
- [x] Process management ✅
- [x] Thread management ✅
- [x] Synchronization primitives ✅
- [x] Event flags ✅
- [x] Barriers ✅
- [x] High-resolution timers ✅
- [x] Thread affinity ✅
- [x] Thread-local storage ✅
- [ ] Complete all syscalls

### oc-audio
- [x] cpal backend ✅
- [x] Multi-channel support ✅
- [ ] Improve mixing quality

### oc-input
- [x] Keyboard mapping ✅
- [x] Controller support ✅
- [ ] Add more controller types

### oc-vfs
- [x] ISO 9660 support ✅
- [x] PKG support ✅
- [x] PARAM.SFO parsing ✅
- [ ] Add more disc formats

### oc-hle
- [x] Global HLE Context ✅
- [x] cellGcmSys connected to global context ✅
- [x] cellSpurs connected to global context ✅
- [x] cellSpursJq connected to global context ✅
- [x] cellResc connected to global context ✅
- [x] cellKb connected to global context ✅
- [x] cellMouse connected to global context ✅
- [x] cellMic connected to global context ✅
- [x] cellFontFT connected to global context ✅
- [x] cellSysutil connected to global context ✅
- [x] cellPad connected to global context ✅
- [x] cellFs connected to global context ✅
- [x] cellFont connected to global context ✅
- [x] cellVpost connected to global context ✅
- [x] cellGifDec connected to global context ✅
- [x] cellSsl cert unload connected to global context ✅
- [x] libsre (regex) connected to global context ✅
- [x] Document all backend integration points ✅
- [x] Document all memory read/write operations ✅
- [x] Convert all TODO comments to proper documentation ✅
- [ ] Implement memory read/write for all modules (requires memory manager integration)
- [ ] Connect to actual backends (oc-vfs, oc-audio, oc-rsx, oc-input)

### oc-loader
- [x] ELF parsing ✅
- [x] SELF parsing ✅
- [x] PRX loading ✅
- [ ] Handle encrypted content

### oc-ffi
- [x] PPU JIT bindings ✅
- [x] SPU JIT bindings ✅
- [ ] Add more JIT instructions

### oc-ui
- [x] Basic UI framework ✅
- [x] Game list view ✅
- [ ] Complete all views

### oc-integration
- [x] EmulatorRunner ✅
- [x] GameLoader ✅
- [x] GamePipeline ✅
- [ ] Complete game execution

### oc-debug
- [x] PPU debugger ✅
- [x] SPU debugger ✅
- [x] RSX debugger ✅
- [ ] Add profiler

---

## 📅 Development Phases

### Phase 1: Foundation ✅ (Complete)
- Memory management
- CPU interpreters (PPU/SPU)
- Basic RSX implementation

### Phase 2: System Emulation 🔨 (In Progress)
- LV2 kernel syscalls
- File system
- Audio/Input systems

### Phase 3: HLE Modules ✅ (Backend Integration Points Documented)
- [x] Implement priority HLE modules
- [x] Document all backend integration requirements
- [x] Convert all TODO markers to proper documentation
- [ ] Integrate with actual backends
- [ ] Complete game loading pipeline
- [ ] Add PRX module support

### Phase 4: Game Compatibility
- Test with homebrew
- Fix bugs per-game
- Optimize performance

### Phase 5: Polish
- UI improvements
- Documentation
- Release preparation

---

## 🏁 Immediate Next Steps

1. ✅ **Document HLE backend integration points** - All 156 TODO markers converted to proper documentation
2. **Implement memory read/write interface** - Enable HLE functions to access game memory (Critical blocker)
3. **Connect cellGcmSys to RSX backend** - Graphics HLE to actual rendering
4. **Connect cellFs to oc-vfs backend** - File system integration
5. **Connect cellPad to oc-input backend** - Controller input integration
6. **Complete game loading pipeline** - Enable EBOOT.BIN execution
7. **Test with homebrew** - Validate implementation with simple apps

---

## 📞 Contributing

See the [Contributing section in README.md](README.md#contributing) for guidelines on how to contribute to this project.

### Quick Start for Contributors

1. Pick an unchecked item from this TODO
2. Create a branch: `git checkout -b feature/your-feature`
3. Implement the feature with tests
4. Submit a pull request

### Priority Labels
- **HIGH** - Critical for running any games
- **Medium** - Improves compatibility/usability
- **Low** - Nice to have, optimizations

---

*Last updated: December 2024*

**Recent Updates:**
- **December 26, 2024**: Completed documentation of all HLE module TODO markers. All 156 TODO comments across 29 HLE module files have been converted to proper documentation explaining backend integration requirements and memory management needs. This provides clear guidance for future implementation work while maintaining code clarity.

*HLE module update: Implemented all remaining HLE modules (cellResc, cellSpursJq, cellKb, cellMouse, cellMic, cellFontFT) and connected them to global HLE context. All HLE modules now have manager implementations and are registered in the module registry. All backend integration points are now fully documented.*
