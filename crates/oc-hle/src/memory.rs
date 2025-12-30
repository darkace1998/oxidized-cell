//! Memory Access Helpers for HLE Functions
//!
//! This module provides safe memory access helpers that bridge HLE functions
//! to the oc-memory subsystem. It enables reading and writing PS3 memory
//! from HLE function implementations.

use oc_memory::{MemoryManager, PageFlags};
use std::sync::Arc;
use parking_lot::RwLock;
use once_cell::sync::Lazy;

/// Global memory manager instance for HLE access
/// This is initialized by the emulator core and accessed by HLE functions.
static HLE_MEMORY: Lazy<RwLock<Option<Arc<MemoryManager>>>> = Lazy::new(|| {
    RwLock::new(None)
});

/// Initialize the HLE memory access with a MemoryManager instance
pub fn init_hle_memory(memory: Arc<MemoryManager>) {
    let mut guard = HLE_MEMORY.write();
    *guard = Some(memory);
}

/// Clear the HLE memory access (for cleanup/reset)
pub fn clear_hle_memory() {
    let mut guard = HLE_MEMORY.write();
    *guard = None;
}

/// Check if HLE memory is initialized
pub fn is_hle_memory_initialized() -> bool {
    HLE_MEMORY.read().is_some()
}

/// Get a clone of the memory manager reference
pub fn get_memory_manager() -> Option<Arc<MemoryManager>> {
    HLE_MEMORY.read().clone()
}

// ============================================================================
// Memory Read Operations
// ============================================================================

/// Read a u8 from guest memory
pub fn read_u8(addr: u32) -> Result<u8, i32> {
    let guard = HLE_MEMORY.read();
    let mem = guard.as_ref().ok_or(0x80010002u32 as i32)?;
    mem.read::<u8>(addr).map_err(|_| 0x80010002u32 as i32)
}

/// Read a u16 from guest memory (big-endian)
pub fn read_be16(addr: u32) -> Result<u16, i32> {
    let guard = HLE_MEMORY.read();
    let mem = guard.as_ref().ok_or(0x80010002u32 as i32)?;
    mem.read_be16(addr).map_err(|_| 0x80010002u32 as i32)
}

/// Read a u32 from guest memory (big-endian)
pub fn read_be32(addr: u32) -> Result<u32, i32> {
    let guard = HLE_MEMORY.read();
    let mem = guard.as_ref().ok_or(0x80010002u32 as i32)?;
    mem.read_be32(addr).map_err(|_| 0x80010002u32 as i32)
}

/// Read a u64 from guest memory (big-endian)
pub fn read_be64(addr: u32) -> Result<u64, i32> {
    let guard = HLE_MEMORY.read();
    let mem = guard.as_ref().ok_or(0x80010002u32 as i32)?;
    mem.read_be64(addr).map_err(|_| 0x80010002u32 as i32)
}

/// Read bytes from guest memory
pub fn read_bytes(addr: u32, size: u32) -> Result<Vec<u8>, i32> {
    let guard = HLE_MEMORY.read();
    let mem = guard.as_ref().ok_or(0x80010002u32 as i32)?;
    mem.read_bytes(addr, size).map_err(|_| 0x80010002u32 as i32)
}

/// Read a null-terminated string from guest memory
pub fn read_string(addr: u32, max_len: u32) -> Result<String, i32> {
    let bytes = read_bytes(addr, max_len)?;
    
    // Find null terminator
    let len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    
    String::from_utf8(bytes[..len].to_vec())
        .map_err(|_| 0x80010002u32 as i32)
}

// ============================================================================
// Memory Write Operations
// ============================================================================

/// Write a u8 to guest memory
pub fn write_u8(addr: u32, value: u8) -> Result<(), i32> {
    let guard = HLE_MEMORY.read();
    let mem = guard.as_ref().ok_or(0x80010002u32 as i32)?;
    mem.write::<u8>(addr, value).map_err(|_| 0x80010002u32 as i32)
}

/// Write a u16 to guest memory (big-endian)
pub fn write_be16(addr: u32, value: u16) -> Result<(), i32> {
    let guard = HLE_MEMORY.read();
    let mem = guard.as_ref().ok_or(0x80010002u32 as i32)?;
    mem.write_be16(addr, value).map_err(|_| 0x80010002u32 as i32)
}

/// Write a u32 to guest memory (big-endian)
pub fn write_be32(addr: u32, value: u32) -> Result<(), i32> {
    let guard = HLE_MEMORY.read();
    let mem = guard.as_ref().ok_or(0x80010002u32 as i32)?;
    mem.write_be32(addr, value).map_err(|_| 0x80010002u32 as i32)
}

/// Write a u64 to guest memory (big-endian)
pub fn write_be64(addr: u32, value: u64) -> Result<(), i32> {
    let guard = HLE_MEMORY.read();
    let mem = guard.as_ref().ok_or(0x80010002u32 as i32)?;
    mem.write_be64(addr, value).map_err(|_| 0x80010002u32 as i32)
}

/// Write bytes to guest memory
pub fn write_bytes(addr: u32, data: &[u8]) -> Result<(), i32> {
    let guard = HLE_MEMORY.read();
    let mem = guard.as_ref().ok_or(0x80010002u32 as i32)?;
    mem.write_bytes(addr, data).map_err(|_| 0x80010002u32 as i32)
}

/// Write a null-terminated string to guest memory
/// 
/// # Arguments
/// * `addr` - Memory address to write string to
/// * `s` - String to write
/// * `max_len` - Maximum buffer size (must be at least 1 for null terminator)
///
/// # Returns
/// * Ok(()) on success
/// * Err with error code on failure (invalid params or memory access error)
pub fn write_string(addr: u32, s: &str, max_len: u32) -> Result<(), i32> {
    // Buffer must have room for at least the null terminator
    if max_len == 0 {
        return Err(ERROR_INVALID_ADDRESS);
    }
    
    let bytes = s.as_bytes();
    let write_len = std::cmp::min(bytes.len(), (max_len - 1) as usize);
    
    // Write string bytes (if any)
    if write_len > 0 {
        write_bytes(addr, &bytes[..write_len])?;
    }
    
    // Write null terminator
    write_u8(addr + write_len as u32, 0)?;
    
    Ok(())
}

// ============================================================================
// Structure Read/Write Helpers
// ============================================================================

/// Read a structure from guest memory by reading its fields individually
/// This is a trait that can be implemented for structures that need to be
/// read from guest memory.
pub trait FromGuestMemory: Sized {
    /// Read the structure from guest memory at the given address
    fn from_guest_memory(addr: u32) -> Result<Self, i32>;
}

/// Write a structure to guest memory by writing its fields individually
pub trait ToGuestMemory {
    /// Write the structure to guest memory at the given address
    fn to_guest_memory(&self, addr: u32) -> Result<(), i32>;
}

// ============================================================================
// Memory Validation
// ============================================================================

/// Check if an address range is valid for reading
pub fn is_readable(addr: u32, size: u32) -> bool {
    let guard = HLE_MEMORY.read();
    if let Some(mem) = guard.as_ref() {
        mem.check_access(addr, size, PageFlags::READ).is_ok()
    } else {
        false
    }
}

/// Check if an address range is valid for writing
pub fn is_writable(addr: u32, size: u32) -> bool {
    let guard = HLE_MEMORY.read();
    if let Some(mem) = guard.as_ref() {
        mem.check_access(addr, size, PageFlags::WRITE).is_ok()
    } else {
        false
    }
}

/// Check if an address is a valid pointer (non-zero and within valid range)
pub fn is_valid_pointer(addr: u32) -> bool {
    addr != 0 && is_readable(addr, 4)
}

// ============================================================================
// Error Codes
// ============================================================================

/// Memory access error - invalid address
pub const ERROR_INVALID_ADDRESS: i32 = 0x80010002u32 as i32;

/// Memory access error - memory not initialized
pub const ERROR_NOT_INITIALIZED: i32 = 0x80010013u32 as i32;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hle_memory_not_initialized() {
        // Clear any existing memory
        clear_hle_memory();
        
        // All operations should fail when not initialized
        assert!(!is_hle_memory_initialized());
        assert!(read_be32(0x1000).is_err());
        assert!(write_be32(0x1000, 0x12345678).is_err());
        assert!(!is_readable(0x1000, 4));
        assert!(!is_writable(0x1000, 4));
    }

    #[test]
    fn test_hle_memory_initialization() {
        // Create a memory manager
        let mem = MemoryManager::new().unwrap();
        
        // Initialize HLE memory
        init_hle_memory(mem.clone());
        assert!(is_hle_memory_initialized());
        
        // Get manager reference
        let manager = get_memory_manager();
        assert!(manager.is_some());
        
        // Clean up
        clear_hle_memory();
        assert!(!is_hle_memory_initialized());
    }

    #[test]
    fn test_hle_memory_read_write() {
        // Create and initialize memory manager
        let mem = MemoryManager::new().unwrap();
        init_hle_memory(mem.clone());
        
        // Allocate some memory
        let addr = mem.allocate(0x1000, 0x1000, PageFlags::RW).unwrap();
        
        // Test u32 read/write
        write_be32(addr, 0x12345678).unwrap();
        assert_eq!(read_be32(addr).unwrap(), 0x12345678);
        
        // Test u64 read/write
        write_be64(addr + 8, 0xDEADBEEFCAFEBABE).unwrap();
        assert_eq!(read_be64(addr + 8).unwrap(), 0xDEADBEEFCAFEBABE);
        
        // Test bytes read/write
        let data = b"Hello";
        write_bytes(addr + 0x100, data).unwrap();
        let read_data = read_bytes(addr + 0x100, data.len() as u32).unwrap();
        assert_eq!(read_data, data);
        
        // Test string read/write
        write_string(addr + 0x200, "Test String", 32).unwrap();
        let read_str = read_string(addr + 0x200, 32).unwrap();
        assert_eq!(read_str, "Test String");
        
        // Test write_string with zero max_len should fail
        assert!(write_string(addr + 0x300, "Test", 0).is_err());
        
        // Test write_string with max_len = 1 (only room for null terminator)
        write_string(addr + 0x400, "Test", 1).unwrap();
        let read_empty = read_string(addr + 0x400, 10).unwrap();
        assert_eq!(read_empty, "");
        
        // Clean up
        clear_hle_memory();
    }

    #[test]
    fn test_hle_memory_validation() {
        let mem = MemoryManager::new().unwrap();
        init_hle_memory(mem.clone());
        
        // Allocate memory
        let addr = mem.allocate(0x1000, 0x1000, PageFlags::RW).unwrap();
        
        // Test validation
        assert!(is_readable(addr, 4));
        assert!(is_writable(addr, 4));
        assert!(is_valid_pointer(addr));
        
        // Invalid address
        assert!(!is_valid_pointer(0));
        
        clear_hle_memory();
    }
}
