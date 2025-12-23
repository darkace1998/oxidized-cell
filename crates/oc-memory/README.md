# oc-memory

Memory management subsystem for the oxidized-cell PS3 emulator.

## Overview

This crate provides a complete, production-ready memory management system that emulates the PlayStation 3's memory architecture, including:

- **4KB page-based memory management** - Efficient allocation and tracking
- **32-bit address space emulation** - Full 4GB PS3 virtual memory
- **Memory protection flags** - Read/Write/Execute permissions per page
- **128-byte reservation system** - Support for PPU and SPU atomic operations
- **PS3 memory region mapping** - Accurate emulation of all PS3 memory regions

## Features

### Memory Manager

- Full 32-bit (4GB) virtual address space
- 4KB page granularity with bitmap tracking
- Dynamic memory allocation/deallocation
- Permission-checked and unchecked access modes
- Big-endian memory operations (PS3 native byte order)
- Cross-platform support (Unix, Linux, macOS, Windows)

### PS3 Memory Regions

| Region | Base Address | Size | Flags | Description |
|--------|-------------|------|-------|-------------|
| Main Memory | 0x00000000 | 256 MB | RWX | Primary system memory |
| User Memory | 0x20000000 | 256 MB | RWX | Applications and heap |
| RSX I/O | 0x40000000 | 1 MB | RW+MMIO | Graphics registers |
| RSX Memory | 0xC0000000 | 256 MB | RWX | Video RAM |
| Stack | 0xD0000000 | 256 MB | RW | Thread stacks |
| SPU LS | 0xE0000000 | 256 KB/SPU | RWX | SPU local storage |

### Atomic Operations

- **PPU Atomics**: lwarx/stwcx (Load Word And Reserve / Store Word Conditional)
- **SPU Atomics**: GETLLAR/PUTLLC (Get Lock Line And Reserve / Put Lock Line Conditional)
- **Reservation System**: 128-byte cache line granularity, lock-free implementation

## Usage

### Basic Memory Operations

```rust
use oc_memory::{MemoryManager, PageFlags};

// Create memory manager
let memory = MemoryManager::new()?;

// Allocate memory
let addr = memory.allocate(0x10000, 0x1000, PageFlags::RW)?;

// Write data
memory.write_be32(addr, 0x12345678)?;

// Read data
let value = memory.read_be32(addr)?;
assert_eq!(value, 0x12345678);

// Free memory
memory.free(addr, 0x10000)?;
```

### Atomic Operations

```rust
// Acquire reservation
let reservation = memory.reservation(addr);
let timestamp = reservation.acquire();

// Try atomic lock
if reservation.try_lock(timestamp) {
    // Perform atomic operation
    memory.write_be32(addr, new_value)?;
    
    // Release and increment
    reservation.unlock_and_increment();
}
```

### Bulk Operations

```rust
// Write multiple bytes
let data = b"Hello, PS3!";
memory.write_bytes(addr, data)?;

// Read multiple bytes
let read_data = memory.read_bytes(addr, data.len() as u32)?;
```

## Testing

Comprehensive test suite with 128+ tests:

```bash
# Run all tests
cargo test -p oc-memory

# Run specific test suites
cargo test -p oc-memory --lib                     # Unit tests
cargo test -p oc-memory --test address_space_tests  # Address space tests
cargo test -p oc-memory --test reservation_tests    # Atomic reservation tests
cargo test -p oc-memory --test stress_tests         # Stress tests

# Run benchmarks
cargo bench -p oc-memory
```

All tests pass on:
- ✅ Linux (x86_64, aarch64)
- ✅ macOS (x86_64, Apple Silicon)
- ✅ Windows (x86_64)

## Performance

- **Unchecked access**: Single instruction pointer arithmetic
- **Reservation operations**: Lock-free atomic operations
- **Cache-aligned structures**: 64-byte alignment for optimal CPU cache usage
- **Efficient allocation**: O(n) first-fit search with bitmap tracking

## Documentation

- [API Documentation](https://docs.rs/oc-memory) - Full API reference
- [TESTING.md](TESTING.md) - Comprehensive testing guide
- [Phase 2 Implementation](../../docs/phase2-memory-management.md) - Detailed implementation documentation

## Architecture

The memory system is organized into several modules:

- **`manager.rs`** - Main memory manager implementation
- **`pages.rs`** - Page protection flags and management
- **`reservation.rs`** - 128-byte atomic reservation system
- **`constants.rs`** - PS3 memory map constants

## Requirements

- Rust 1.80 or later
- Unix: libc for mmap
- Windows: windows-sys for VirtualAlloc

## License

GPL-3.0 - See LICENSE file for details

## Status

✅ **Production Ready** - Phase 2 implementation is complete and fully tested.

All PS3 memory management features are implemented and ready for use by other emulator components (PPU, SPU, RSX, LV2 kernel).
