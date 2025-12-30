# oc-hle - High Level Emulation Modules

This crate provides HLE (High Level Emulation) implementations of PS3 system libraries for oxidized-cell.

## Overview

HLE modules emulate PS3 system libraries at a high level, allowing games to run without requiring full low-level implementations. Each module corresponds to a PS3 system library (PRX file).

## Module Categories

### Graphics Modules
- **cell_gcm_sys** - RSX graphics command management
- **cell_gif_dec** - GIF image decoding
- **cell_png_dec** - PNG image decoding  
- **cell_jpg_dec** - JPEG image decoding
- **cell_resc** - Resolution scaling and conversion

### System Modules
- **cell_sysutil** - System utilities (callbacks, dialogs, settings)
- **cell_game** - Game data management
- **cell_save_data** - Save data management

### Multimedia Modules
- **cell_dmux** - Demultiplexer for media streams
- **cell_vdec** - Video decoding
- **cell_adec** - Audio decoding
- **cell_vpost** - Video post-processing

### Network Modules
- **cell_net_ctl** - Network control
- **cell_http** - HTTP client
- **cell_ssl** - SSL/TLS support

### Input Modules
- **cell_pad** - Controller input
- **cell_kb** - Keyboard input
- **cell_mouse** - Mouse input
- **cell_mic** - Microphone input

### Utility Modules
- **cell_font** - Font rendering
- **cell_font_ft** - FreeType-based font rendering
- **cell_spurs** - SPU task scheduling
- **cell_spurs_jq** - SPURS job queues
- **libsre** - Regular expressions

### Other Modules
- **cell_audio** - Audio output
- **cell_fs** - File system operations

## Architecture

Each HLE module follows a consistent pattern:

```rust
// Manager struct holding module state
pub struct ModuleManager {
    // State fields
}

impl ModuleManager {
    pub fn new() -> Self { ... }
    // Manager methods
}

// C-compatible structures matching PS3 SDK
#[repr(C)]
pub struct CellModuleParam { ... }

// HLE function implementations
pub fn cell_module_function(args...) -> i32 { ... }
```

## Usage

```rust
use oc_hle::{HleContext, get_hle_context, get_hle_context_mut};

// Access global HLE context
let ctx = get_hle_context();
let mut ctx = get_hle_context_mut();

// Use module managers
ctx.sysutil.register_callback(slot, func, userdata);
ctx.game.boot_check();
ctx.save_data.create_directory("SAVE0001");
```

## Testing

```bash
# Run all HLE tests
cargo test --package oc-hle

# Run specific module tests
cargo test --package oc-hle cell_sysutil
cargo test --package oc-hle cell_game
```

## Implementation Status

See [docs/HLE_STATUS.md](../../docs/HLE_STATUS.md) for detailed implementation status of each module.

## Contributing

1. Check the status document for areas needing work
2. Follow existing code patterns in similar modules
3. Add comprehensive unit tests
4. Update the status document when adding features
