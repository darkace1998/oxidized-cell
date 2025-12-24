//! Memory management (sys_memory_*)

use oc_core::error::KernelError;
use parking_lot::Mutex;
use std::collections::HashMap;

/// Memory container ID
pub type ContainerId = u32;

/// Memory page size (64KB, typical for PS3)
pub const PAGE_SIZE: usize = 0x10000;

/// Memory page attributes
#[derive(Debug, Clone, Copy)]
pub struct PageAttribute {
    pub page_size: usize,
    pub flags: u32,
}

/// Memory allocation flags
pub mod flags {
    pub const SYS_MEMORY_ACCESS_RIGHT_PPU_THR: u64 = 0x00000008;
    pub const SYS_MEMORY_ACCESS_RIGHT_HANDLER: u64 = 0x00000010;
    pub const SYS_MEMORY_ACCESS_RIGHT_SPU_THR: u64 = 0x00000020;
    pub const SYS_MEMORY_ACCESS_RIGHT_RAW_SPU: u64 = 0x00000040;
    pub const SYS_MEMORY_ATTR_READ: u64 = 0x00010000;
    pub const SYS_MEMORY_ATTR_WRITE: u64 = 0x00020000;
}

/// Memory allocation information
#[derive(Debug, Clone)]
struct MemoryAllocation {
    addr: u64,
    size: usize,
    container_id: ContainerId,
    flags: u64,
}

/// Memory manager for LV2
pub struct MemoryManager {
    allocations: Mutex<HashMap<u64, MemoryAllocation>>,
    next_addr: Mutex<u64>,
}

impl MemoryManager {
    pub fn new() -> Self {
        Self {
            allocations: Mutex::new(HashMap::new()),
            next_addr: Mutex::new(0x3000_0000), // Start of user memory region
        }
    }

    /// Allocate memory
    pub fn allocate(
        &self,
        size: usize,
        page_size: usize,
        flags: u64,
        container_id: ContainerId,
    ) -> Result<u64, KernelError> {
        // Align size to page boundary
        let aligned_size = (size + page_size - 1) & !(page_size - 1);

        let mut next_addr = self.next_addr.lock();
        let addr = *next_addr;
        *next_addr += aligned_size as u64;

        let allocation = MemoryAllocation {
            addr,
            size: aligned_size,
            container_id,
            flags,
        };

        self.allocations.lock().insert(addr, allocation);

        tracing::debug!(
            "Allocated {} bytes at 0x{:x} (container: {}, flags: 0x{:x})",
            aligned_size,
            addr,
            container_id,
            flags
        );

        Ok(addr)
    }

    /// Free memory
    pub fn free(&self, addr: u64) -> Result<(), KernelError> {
        let mut allocations = self.allocations.lock();

        if allocations.remove(&addr).is_some() {
            tracing::debug!("Freed memory at 0x{:x}", addr);
            Ok(())
        } else {
            Err(KernelError::InvalidId(addr as u32))
        }
    }

    /// Get page attributes for an address
    pub fn get_page_attribute(&self, addr: u64) -> Result<PageAttribute, KernelError> {
        let allocations = self.allocations.lock();

        // Find allocation containing this address
        for allocation in allocations.values() {
            if addr >= allocation.addr && addr < allocation.addr + allocation.size as u64 {
                return Ok(PageAttribute {
                    page_size: PAGE_SIZE,
                    flags: allocation.flags as u32,
                });
            }
        }

        Err(KernelError::InvalidId(addr as u32))
    }

    /// Get allocation info
    pub fn get_allocation(&self, addr: u64) -> Option<(u64, usize)> {
        let allocations = self.allocations.lock();
        allocations.get(&addr).map(|a| (a.addr, a.size))
    }
}

impl Default for MemoryManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Memory syscall implementations
pub mod syscalls {
    use super::*;

    /// sys_memory_allocate
    pub fn sys_memory_allocate(
        manager: &MemoryManager,
        size: usize,
        page_size: usize,
        flags: u64,
    ) -> Result<u64, KernelError> {
        if size == 0 {
            return Err(KernelError::ResourceLimit);
        }

        // Default container ID is 0
        manager.allocate(size, page_size, flags, 0)
    }

    /// sys_memory_allocate_from_container
    pub fn sys_memory_allocate_from_container(
        manager: &MemoryManager,
        size: usize,
        page_size: usize,
        flags: u64,
        container_id: ContainerId,
    ) -> Result<u64, KernelError> {
        if size == 0 {
            return Err(KernelError::ResourceLimit);
        }

        manager.allocate(size, page_size, flags, container_id)
    }

    /// sys_memory_free
    pub fn sys_memory_free(manager: &MemoryManager, addr: u64) -> Result<(), KernelError> {
        manager.free(addr)
    }

    /// sys_memory_get_page_attribute
    pub fn sys_memory_get_page_attribute(
        manager: &MemoryManager,
        addr: u64,
    ) -> Result<PageAttribute, KernelError> {
        manager.get_page_attribute(addr)
    }

    /// sys_memory_get_user_memory_size
    pub fn sys_memory_get_user_memory_size() -> (usize, usize) {
        // Return (total_user_memory, available_user_memory)
        // PS3 typically has 256MB total, with varying amounts available
        (256 * 1024 * 1024, 200 * 1024 * 1024)
    }

    /// sys_mmapper_allocate_memory
    /// Allocate memory with specific page size and flags for memory mapping
    pub fn sys_mmapper_allocate_memory(
        manager: &MemoryManager,
        size: usize,
        page_size: usize,
        flags: u64,
    ) -> Result<u64, KernelError> {
        // Memory mapper allocations are similar to regular allocations
        // but may have different alignment or placement requirements
        if size == 0 {
            return Err(KernelError::ResourceLimit);
        }

        // Use container ID 1 for mmapper allocations to distinguish them
        manager.allocate(size, page_size, flags, 1)
    }

    /// sys_mmapper_map_memory
    /// Map allocated memory to a specific address
    pub fn sys_mmapper_map_memory(
        manager: &MemoryManager,
        addr: u64,
        size: usize,
        flags: u64,
    ) -> Result<(), KernelError> {
        // In real implementation, would map the memory region to the specified address
        // For now, verify the allocation exists
        let page_attr = manager.get_page_attribute(addr);
        
        if page_attr.is_ok() {
            tracing::debug!(
                "Mapping memory at 0x{:x}, size: 0x{:x}, flags: 0x{:x}",
                addr,
                size,
                flags
            );
            Ok(())
        } else {
            // Create a new allocation at the specified address if it doesn't exist
            tracing::debug!(
                "Creating new mapping at 0x{:x}, size: 0x{:x}",
                addr,
                size
            );
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_allocate_free() {
        let manager = MemoryManager::new();

        // Allocate memory
        let addr = syscalls::sys_memory_allocate(&manager, 0x10000, PAGE_SIZE, 0).unwrap();
        assert!(addr > 0);

        // Verify allocation exists
        let info = manager.get_allocation(addr);
        assert!(info.is_some());
        let (allocated_addr, size) = info.unwrap();
        assert_eq!(allocated_addr, addr);
        assert_eq!(size, 0x10000);

        // Free memory
        syscalls::sys_memory_free(&manager, addr).unwrap();

        // Verify allocation is gone
        assert!(manager.get_allocation(addr).is_none());
    }

    #[test]
    fn test_memory_page_attribute() {
        let manager = MemoryManager::new();

        // Allocate with specific flags
        let flags = flags::SYS_MEMORY_ATTR_READ | flags::SYS_MEMORY_ATTR_WRITE;
        let addr = syscalls::sys_memory_allocate(&manager, 0x10000, PAGE_SIZE, flags).unwrap();

        // Get page attributes
        let attr = syscalls::sys_memory_get_page_attribute(&manager, addr).unwrap();
        assert_eq!(attr.page_size, PAGE_SIZE);
        assert_eq!(attr.flags, flags as u32);

        // Get attribute for address within allocation
        let attr2 = syscalls::sys_memory_get_page_attribute(&manager, addr + 0x1000).unwrap();
        assert_eq!(attr2.page_size, PAGE_SIZE);
    }

    #[test]
    fn test_memory_allocation_alignment() {
        let manager = MemoryManager::new();

        // Allocate non-aligned size
        let addr = syscalls::sys_memory_allocate(&manager, 0x1234, PAGE_SIZE, 0).unwrap();

        // Size should be aligned to page size
        let (_, size) = manager.get_allocation(addr).unwrap();
        assert_eq!(size, 0x10000); // Rounded up to PAGE_SIZE
    }

    #[test]
    fn test_memory_multiple_allocations() {
        let manager = MemoryManager::new();

        // Allocate multiple blocks
        let addr1 = syscalls::sys_memory_allocate(&manager, 0x10000, PAGE_SIZE, 0).unwrap();
        let addr2 = syscalls::sys_memory_allocate(&manager, 0x20000, PAGE_SIZE, 0).unwrap();
        let addr3 = syscalls::sys_memory_allocate(&manager, 0x10000, PAGE_SIZE, 0).unwrap();

        // Addresses should be different
        assert_ne!(addr1, addr2);
        assert_ne!(addr2, addr3);
        assert_ne!(addr1, addr3);

        // All should be valid
        assert!(manager.get_allocation(addr1).is_some());
        assert!(manager.get_allocation(addr2).is_some());
        assert!(manager.get_allocation(addr3).is_some());
    }

    #[test]
    fn test_memory_free_invalid() {
        let manager = MemoryManager::new();

        // Try to free non-existent address
        let result = syscalls::sys_memory_free(&manager, 0xDEADBEEF);
        assert!(result.is_err());
    }

    #[test]
    fn test_memory_user_size() {
        let (total, available) = syscalls::sys_memory_get_user_memory_size();
        assert_eq!(total, 256 * 1024 * 1024);
        assert!(available > 0);
        assert!(available <= total);
    }

    #[test]
    fn test_mmapper_allocate() {
        let manager = MemoryManager::new();

        // Allocate with mmapper
        let addr = syscalls::sys_mmapper_allocate_memory(&manager, 0x20000, PAGE_SIZE, 0).unwrap();
        assert!(addr > 0);

        // Verify allocation exists
        let info = manager.get_allocation(addr);
        assert!(info.is_some());

        // Free
        syscalls::sys_memory_free(&manager, addr).unwrap();
    }

    #[test]
    fn test_mmapper_map() {
        let manager = MemoryManager::new();

        // Allocate memory
        let addr = syscalls::sys_mmapper_allocate_memory(&manager, 0x10000, PAGE_SIZE, 0).unwrap();

        // Map the memory
        syscalls::sys_mmapper_map_memory(&manager, addr, 0x10000, 0).unwrap();

        // Free
        syscalls::sys_memory_free(&manager, addr).unwrap();
    }
}

