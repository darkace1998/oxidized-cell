# PS3 Hardware Configuration Guide

This guide explains how to customize the emulated PlayStation 3 hardware specifications (CPU and memory) to optimize performance or test different configurations.

## Overview

The oxidized-cell emulator allows you to configure the virtual PS3's hardware specifications through the `config.toml` file. This is similar to building a custom PC where you can swap CPU and memory components.

## Configuration File Location

The configuration file is located at:
- **Linux/macOS**: `~/.config/oxidized-cell/config.toml`
- **Windows**: `%APPDATA%\oxidized-cell\config.toml`

If the file doesn't exist, the emulator will create it with default values on first run.

## Hardware Configuration Options

### CPU Configuration

The `[cpu]` section includes hardware-related settings:

```toml
[cpu]
# PPU (PowerPC Processing Unit) frequency in MHz
# Default: 3200 MHz (3.2 GHz) - matches real PS3
# Valid range: 1000-4000 MHz
ppu_frequency_mhz = 3200

# SPU (Synergistic Processing Unit) frequency in MHz
# Default: 3200 MHz (3.2 GHz) - matches real PS3
# Valid range: 1000-4000 MHz
spu_frequency_mhz = 3200

# Main system memory size in MB (XDR DRAM)
# Default: 256 MB - matches real PS3
# Valid range: 128-512 MB
main_memory_mb = 256

# RSX video memory size in MB (GDDR3 VRAM)
# Default: 256 MB - matches real PS3
# Valid range: 128-512 MB
video_memory_mb = 256
```

## Example Configurations

### Standard PS3 (Default)

This matches the specifications of a real PlayStation 3:

```toml
[cpu]
ppu_frequency_mhz = 3200  # 3.2 GHz
spu_frequency_mhz = 3200  # 3.2 GHz
main_memory_mb = 256      # 256 MB XDR DRAM
video_memory_mb = 256     # 256 MB GDDR3 VRAM
```

### High-Performance Configuration

For modern systems with more resources, you can increase the memory:

```toml
[cpu]
ppu_frequency_mhz = 3200  # Keep CPU at 3.2 GHz for accuracy
spu_frequency_mhz = 3200
main_memory_mb = 512      # Double the memory
video_memory_mb = 512     # Double the video memory
```

### Low-Memory Configuration

For testing or systems with limited resources:

```toml
[cpu]
ppu_frequency_mhz = 3200
spu_frequency_mhz = 3200
main_memory_mb = 128      # Minimum supported
video_memory_mb = 128     # Minimum supported
```

### Underclocked Configuration

For testing or power-saving (may affect performance):

```toml
[cpu]
ppu_frequency_mhz = 2000  # Underclocked to 2.0 GHz
spu_frequency_mhz = 2000
main_memory_mb = 256
video_memory_mb = 256
```

## Important Notes

1. **CPU Frequency**: Changing the CPU frequency affects cycle-accurate timing emulation. The default 3200 MHz (3.2 GHz) matches the real PS3 Cell Broadband Engine.

2. **Memory Sizes**: 
   - The standard PS3 has 256 MB of main memory (XDR DRAM) and 256 MB of video memory (GDDR3).
   - Increasing memory may improve performance in memory-intensive games but may cause compatibility issues.
   - Decreasing memory below 256 MB is not recommended and may cause games to fail.

3. **Validation**: The emulator validates all values on startup:
   - CPU frequencies outside 1000-4000 MHz range will be reset to 3200 MHz
   - Memory sizes outside 128-512 MB range will be reset to 256 MB
   - Warning messages will be displayed for out-of-range values

4. **Compatibility**: For best compatibility with PS3 games, use the default values that match the real hardware.

## Technical Details

### Cell Broadband Engine Architecture

The PS3's Cell BE processor consists of:
- **1x PPE (PowerPC Processing Element)**: 3.2 GHz, 64-bit dual-threaded
- **6-8x SPEs (Synergistic Processing Elements)**: 3.2 GHz each, 128-bit SIMD

The emulator simulates this architecture with configurable frequencies.

### Memory Layout

The PS3 memory layout includes:
- **Main Memory (XDR DRAM)**: System RAM at 0x00000000
- **User Memory**: Available to applications at 0x20000000
- **RSX Mapped Memory**: Shared with GPU at 0x30000000
- **RSX Local Memory (GDDR3)**: Video RAM at 0xC0000000

Configurable sizes apply to the main and video memory regions.

## Troubleshooting

### Out-of-Range Values

If you set values outside the valid range, you'll see warnings like:

```
Warning: PPU frequency 5000 MHz is outside typical range (1000-4000 MHz). Using default 3200 MHz.
```

The emulator will automatically use safe default values.

### Performance Issues

- **Too slow?** Try reducing memory sizes if running on limited hardware.
- **Accuracy issues?** Use default values (3200 MHz, 256 MB) that match real PS3 hardware.
- **Games crashing?** Reset to default configuration.

## See Also

- [README.md](../README.md) - Main documentation
- [Building Guide](../README.md#building) - How to build the emulator
- Configuration is defined in `crates/oc-core/src/config.rs`
