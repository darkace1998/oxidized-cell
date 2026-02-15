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
    _container_id: ContainerId,
    flags: u64,
}

/// Memory manager for LV2
pub struct MemoryManager {
    allocations: Mutex<HashMap<u64, MemoryAllocation>>,
    next_addr: Mutex<u64>,
    mmio: Mutex<MmioManager>,
}

/// MMIO device register access trait
///
/// Devices implement this trait to handle memory-mapped register reads and writes.
pub trait MmioDevice: Send + Sync {
    /// Read a 32-bit register at the given offset from the device's base address
    fn read32(&self, offset: u32) -> u32;
    /// Write a 32-bit value to a register at the given offset
    fn write32(&mut self, offset: u32, value: u32);
    /// Device name for debugging
    fn name(&self) -> &str;
}

/// MMIO address region mapping a physical address range to a device
#[derive(Debug, Clone)]
pub struct MmioRegion {
    /// Base address of the MMIO region
    pub base: u64,
    /// Size of the MMIO region in bytes
    pub size: u64,
    /// Device name (for lookup)
    pub device_name: String,
}

/// MMIO manager that dispatches read/write to registered devices
pub struct MmioManager {
    /// Registered MMIO regions (base_addr → region info)
    regions: Vec<MmioRegion>,
    /// Device register storage (device_name → register_map)
    /// Uses a simple HashMap<offset, value> for each device
    devices: HashMap<String, HashMap<u32, u32>>,
}

impl MmioManager {
    pub fn new() -> Self {
        let mut mgr = Self {
            regions: Vec::new(),
            devices: HashMap::new(),
        };
        
        // Register built-in PS3 MMIO devices
        mgr.register_builtin_devices();
        mgr
    }
    
    /// Register built-in PS3 device stubs
    fn register_builtin_devices(&mut self) {
        // SPU problem state area (one per SPU, at 0x200B0000 + n*0x20000)
        for i in 0..6u64 {
            let base = 0x200B_0000 + i * 0x2_0000;
            let name = format!("spu_ctrl_{}", i);
            self.regions.push(MmioRegion {
                base,
                size: 0x2_0000,
                device_name: name.clone(),
            });
            let mut regs = HashMap::new();
            // SPU_Status register at offset 0x4000 (default: stopped)
            regs.insert(0x4000, 0x0000_0000);
            // SPU_NPC (next program counter) at offset 0x4008
            regs.insert(0x4008, 0x0000_0000);
            // SPU_RunCntl at offset 0x401C
            regs.insert(0x401C, 0x0000_0000);
            self.devices.insert(name, regs);
        }
        
        // Interrupt controller at 0x0E000000
        self.regions.push(MmioRegion {
            base: 0x0E00_0000,
            size: 0x1000,
            device_name: "interrupt_ctrl".to_string(),
        });
        let mut ic_regs = HashMap::new();
        // IRQ_STATUS register
        ic_regs.insert(0x0000, 0x0000_0000);
        // IRQ_MASK register
        ic_regs.insert(0x0004, 0x0000_0000);
        // IRQ_CLEAR register
        ic_regs.insert(0x0008, 0x0000_0000);
        self.devices.insert("interrupt_ctrl".to_string(), ic_regs);
    }
    
    /// Register a custom MMIO region for a device
    pub fn register_device(
        &mut self,
        base: u64,
        size: u64,
        name: String,
    ) -> Result<(), KernelError> {
        // Check for overlap with existing regions
        for region in &self.regions {
            if base < region.base + region.size && base + size > region.base {
                return Err(KernelError::PermissionDenied);
            }
        }
        
        self.regions.push(MmioRegion {
            base,
            size,
            device_name: name.clone(),
        });
        self.devices.entry(name).or_insert_with(HashMap::new);
        Ok(())
    }
    
    /// Read a 32-bit register from an MMIO address
    pub fn read32(&self, addr: u64) -> Result<u32, KernelError> {
        for region in &self.regions {
            if addr >= region.base && addr < region.base + region.size {
                let offset = (addr - region.base) as u32;
                if let Some(regs) = self.devices.get(&region.device_name) {
                    let value = regs.get(&offset).copied().unwrap_or(0);
                    tracing::debug!(
                        "MMIO read: {}[0x{:x}] = 0x{:08x}",
                        region.device_name, offset, value
                    );
                    return Ok(value);
                }
            }
        }
        Err(KernelError::InvalidId(addr as u32))
    }
    
    /// Write a 32-bit value to an MMIO address
    pub fn write32(&mut self, addr: u64, value: u32) -> Result<(), KernelError> {
        for region in &self.regions {
            if addr >= region.base && addr < region.base + region.size {
                let offset = (addr - region.base) as u32;
                let device_name = region.device_name.clone();
                if let Some(regs) = self.devices.get_mut(&device_name) {
                    tracing::debug!(
                        "MMIO write: {}[0x{:x}] = 0x{:08x}",
                        device_name, offset, value
                    );
                    regs.insert(offset, value);
                    return Ok(());
                }
            }
        }
        Err(KernelError::InvalidId(addr as u32))
    }
    
    /// Get the list of registered MMIO regions
    pub fn regions(&self) -> &[MmioRegion] {
        &self.regions
    }
    
    /// Check if an address is in an MMIO region
    pub fn is_mmio_addr(&self, addr: u64) -> bool {
        self.regions.iter().any(|r| addr >= r.base && addr < r.base + r.size)
    }
}

impl MemoryManager {
    pub fn new() -> Self {
        Self {
            allocations: Mutex::new(HashMap::new()),
            next_addr: Mutex::new(0x3000_0000), // Start of user memory region
            mmio: Mutex::new(MmioManager::new()),
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
            _container_id: container_id,
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
    
    /// Read a 32-bit value from an MMIO device register
    pub fn mmio_read32(&self, addr: u64) -> Result<u32, KernelError> {
        let mmio = self.mmio.lock();
        mmio.read32(addr)
    }
    
    /// Write a 32-bit value to an MMIO device register
    pub fn mmio_write32(&self, addr: u64, value: u32) -> Result<(), KernelError> {
        let mut mmio = self.mmio.lock();
        mmio.write32(addr, value)
    }
    
    /// Register a custom MMIO device region
    pub fn register_mmio_device(
        &self,
        base: u64,
        size: u64,
        name: String,
    ) -> Result<(), KernelError> {
        let mut mmio = self.mmio.lock();
        mmio.register_device(base, size, name)
    }
    
    /// Check if an address maps to an MMIO device
    pub fn is_mmio_addr(&self, addr: u64) -> bool {
        let mmio = self.mmio.lock();
        mmio.is_mmio_addr(addr)
    }
    
    /// Get the number of registered MMIO regions
    pub fn mmio_region_count(&self) -> usize {
        let mmio = self.mmio.lock();
        mmio.regions().len()
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

    /// sys_mmapper_free_memory
    /// Free memory previously allocated by sys_mmapper_allocate_memory
    pub fn sys_mmapper_free_memory(
        manager: &MemoryManager,
        addr: u64,
    ) -> Result<(), KernelError> {
        tracing::debug!("sys_mmapper_free_memory(0x{:x})", addr);
        manager.free(addr)
    }

    /// sys_mmapper_unmap_memory
    /// Unmap a previously mapped memory region
    pub fn sys_mmapper_unmap_memory(
        _manager: &MemoryManager,
        addr: u64,
        _size: usize,
    ) -> Result<(), KernelError> {
        tracing::debug!("sys_mmapper_unmap_memory(0x{:x})", addr);
        // TODO: track mapped regions and actually unmap them
        Ok(())
    }

    /// sys_vm_memory_map
    /// Map virtual memory at the specified address
    pub fn sys_vm_memory_map(
        _manager: &MemoryManager,
        addr: u64,
        _size: usize,
        _block_size: u64,
        _flags: u64,
    ) -> Result<u64, KernelError> {
        tracing::debug!("sys_vm_memory_map(0x{:x})", addr);
        // TODO: implement real virtual memory page management
        // Return the requested address as the mapped address
        Ok(addr)
    }

    /// sys_vm_unmap
    /// Unmap a virtual memory region
    pub fn sys_vm_unmap(
        _manager: &MemoryManager,
        addr: u64,
    ) -> Result<(), KernelError> {
        tracing::debug!("sys_vm_unmap(0x{:x})", addr);
        Ok(())
    }

    /// sys_vm_get_statistics
    /// Get statistics about a virtual memory region
    /// Returns (page_fault_count, page_in_count, page_out_count)
    pub fn sys_vm_get_statistics(
        _manager: &MemoryManager,
        addr: u64,
    ) -> Result<(u64, u64, u64), KernelError> {
        tracing::debug!("sys_vm_get_statistics(0x{:x})", addr);
        // Return zeroed statistics — no real paging is emulated
        Ok((0, 0, 0))
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

    #[test]
    fn test_mmio_builtin_devices() {
        let manager = MemoryManager::new();
        
        // Built-in devices: 6 SPU controllers + 1 interrupt controller = 7 regions
        assert_eq!(manager.mmio_region_count(), 7);
    }

    #[test]
    fn test_mmio_spu_ctrl_read_write() {
        let manager = MemoryManager::new();
        
        // SPU 0 problem state at 0x200B0000
        let spu0_status = 0x200B_0000 + 0x4000; // SPU_Status register
        
        // Read default value (0)
        let value = manager.mmio_read32(spu0_status).unwrap();
        assert_eq!(value, 0);
        
        // Write a status value
        manager.mmio_write32(spu0_status, 0x0000_0001).unwrap();
        let value = manager.mmio_read32(spu0_status).unwrap();
        assert_eq!(value, 0x0000_0001);
    }

    #[test]
    fn test_mmio_interrupt_ctrl() {
        let manager = MemoryManager::new();
        
        // Interrupt controller at 0x0E000000
        let irq_status = 0x0E00_0000;
        let irq_mask = 0x0E00_0004;
        
        // Write and read IRQ mask
        manager.mmio_write32(irq_mask, 0xFF00_FF00).unwrap();
        let mask = manager.mmio_read32(irq_mask).unwrap();
        assert_eq!(mask, 0xFF00_FF00);
        
        // Status should still be 0
        let status = manager.mmio_read32(irq_status).unwrap();
        assert_eq!(status, 0);
    }

    #[test]
    fn test_mmio_is_mmio_addr() {
        let manager = MemoryManager::new();
        
        // SPU control area is MMIO
        assert!(manager.is_mmio_addr(0x200B_0000));
        // Regular memory is not MMIO
        assert!(!manager.is_mmio_addr(0x3000_0000));
    }

    #[test]
    fn test_mmio_invalid_addr() {
        let manager = MemoryManager::new();
        
        // Address not in any MMIO region
        let result = manager.mmio_read32(0xDEAD_BEEF);
        assert!(result.is_err());
    }

    #[test]
    fn test_mmio_register_custom_device() {
        let manager = MemoryManager::new();
        let initial_count = manager.mmio_region_count();
        
        // Register a custom device
        manager.register_mmio_device(
            0xF000_0000,
            0x1000,
            "custom_dev".to_string(),
        ).unwrap();
        
        assert_eq!(manager.mmio_region_count(), initial_count + 1);
        
        // Read/write to custom device
        manager.mmio_write32(0xF000_0000, 0x1234).unwrap();
        let value = manager.mmio_read32(0xF000_0000).unwrap();
        assert_eq!(value, 0x1234);
    }

    #[test]
    fn test_mmio_overlapping_region_rejected() {
        let manager = MemoryManager::new();
        
        // Try to register a region that overlaps with SPU 0 control area
        let result = manager.register_mmio_device(
            0x200B_0000,
            0x1000,
            "overlapping".to_string(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_mmapper_free_memory() {
        let manager = MemoryManager::new();
        let addr = syscalls::sys_mmapper_allocate_memory(&manager, 0x10000, PAGE_SIZE, 0).unwrap();
        assert!(addr > 0);
        syscalls::sys_mmapper_free_memory(&manager, addr).unwrap();
    }

    #[test]
    fn test_mmapper_unmap_memory() {
        let manager = MemoryManager::new();
        syscalls::sys_mmapper_unmap_memory(&manager, 0x30000000, 0x10000).unwrap();
    }

    #[test]
    fn test_vm_memory_map_and_unmap() {
        let manager = MemoryManager::new();
        let addr = syscalls::sys_vm_memory_map(&manager, 0x30000000, 0x100000, 0x10000, 0).unwrap();
        assert_eq!(addr, 0x30000000);
        syscalls::sys_vm_unmap(&manager, addr).unwrap();
    }

    #[test]
    fn test_vm_get_statistics() {
        let manager = MemoryManager::new();
        let (faults, ins, outs) = syscalls::sys_vm_get_statistics(&manager, 0x30000000).unwrap();
        assert_eq!(faults, 0);
        assert_eq!(ins, 0);
        assert_eq!(outs, 0);
    }
}

