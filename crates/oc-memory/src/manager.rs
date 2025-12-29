//! Memory manager implementation

use crate::constants::*;
use crate::pages::PageFlags;
use crate::reservation::Reservation;
use oc_core::error::{AccessKind, MemoryError};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Memory region descriptor
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    /// Base address
    pub base: u32,
    /// Size in bytes
    pub size: u32,
    /// Page flags
    pub flags: PageFlags,
    /// Region name
    pub name: &'static str,
}

/// Memory protection exception type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryException {
    /// Access violation (attempted access not permitted by page flags)
    AccessViolation {
        addr: u32,
        kind: AccessKind,
    },
    /// Page fault (page not committed)
    PageFault {
        addr: u32,
    },
    /// Alignment fault
    AlignmentFault {
        addr: u32,
        required: u32,
        actual: u32,
    },
    /// Reserved address access (attempting to access reserved memory)
    ReservedAccess {
        addr: u32,
    },
}

/// Memory exception handler result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceptionHandlerResult {
    /// Exception was handled, continue execution
    Handled,
    /// Exception not handled, should be reported as error
    NotHandled,
    /// Retry the memory access (e.g., after fixing page tables)
    Retry,
}

/// Shared memory region between PPU and SPU
#[derive(Debug, Clone)]
pub struct SharedMemoryRegion {
    /// Region ID
    pub id: u32,
    /// Main memory base address
    pub main_addr: u32,
    /// Size in bytes
    pub size: u32,
    /// Flags
    pub flags: PageFlags,
    /// SPU IDs that have access
    pub spu_access: Vec<u32>,
    /// Whether PPU has access
    pub ppu_access: bool,
    /// Reference count
    pub ref_count: u32,
}

impl SharedMemoryRegion {
    /// Create a new shared memory region
    pub fn new(id: u32, main_addr: u32, size: u32, flags: PageFlags) -> Self {
        Self {
            id,
            main_addr,
            size,
            flags,
            spu_access: Vec::new(),
            ppu_access: true,
            ref_count: 1,
        }
    }
    
    /// Grant access to an SPU
    pub fn grant_spu_access(&mut self, spu_id: u32) {
        if !self.spu_access.contains(&spu_id) {
            self.spu_access.push(spu_id);
        }
    }
    
    /// Revoke access from an SPU
    pub fn revoke_spu_access(&mut self, spu_id: u32) {
        self.spu_access.retain(|&id| id != spu_id);
    }
    
    /// Check if an SPU has access
    pub fn has_spu_access(&self, spu_id: u32) -> bool {
        self.spu_access.contains(&spu_id)
    }
}

/// RSX memory mapping entry
#[derive(Debug, Clone)]
pub struct RsxMemoryMapping {
    /// Offset in RSX local memory
    pub rsx_offset: u32,
    /// Main memory address
    pub main_addr: u32,
    /// Size of the mapping
    pub size: u32,
    /// Whether this is a tile mapping
    pub tiled: bool,
    /// Tile pitch (for tiled mappings)
    pub tile_pitch: u32,
}

/// Main memory manager for the PS3 emulator
///
/// Manages the 32-bit virtual address space with proper page tracking
/// and reservation system for SPU atomics.
pub struct MemoryManager {
    /// Base pointer for the main address space
    base: *mut u8,
    /// Allocation bitmap (one bit per page)
    allocation_map: RwLock<Vec<u64>>,
    /// Page flags for each page
    page_flags: RwLock<Vec<PageFlags>>,
    /// Reservation array (one per 128-byte cache line)
    reservations: Box<[Reservation]>,
    /// Memory regions
    regions: Vec<MemoryRegion>,
    /// RSX memory (separate allocation for VRAM)
    rsx_mem: *mut u8,
    /// Shared memory regions
    shared_regions: RwLock<HashMap<u32, SharedMemoryRegion>>,
    /// Next shared region ID
    next_shared_id: RwLock<u32>,
    /// RSX memory mappings
    rsx_mappings: RwLock<Vec<RsxMemoryMapping>>,
    /// Exception handler callback (if set)
    exception_handler: RwLock<Option<Box<dyn Fn(MemoryException) -> ExceptionHandlerResult + Send + Sync>>>,
}

// Safety: Memory is accessed through atomic operations and proper synchronization
unsafe impl Send for MemoryManager {}
unsafe impl Sync for MemoryManager {}

impl MemoryManager {
    /// Create a new memory manager
    pub fn new() -> Result<Arc<Self>, MemoryError> {
        // Allocate main address space
        let base = Self::allocate_address_space(ADDRESS_SPACE_SIZE)?;

        // Allocate RSX memory separately
        let rsx_mem = Self::allocate_address_space(RSX_MEM_SIZE as usize)?;

        // Create page tracking
        let allocation_map = RwLock::new(vec![0u64; NUM_PAGES / 64]);
        let page_flags = RwLock::new(vec![PageFlags::empty(); NUM_PAGES]);

        // Create reservations
        let reservations = (0..NUM_RESERVATIONS)
            .map(|_| Reservation::new())
            .collect::<Vec<_>>()
            .into_boxed_slice();

        let regions = vec![
            MemoryRegion {
                base: MAIN_MEM_BASE,
                size: MAIN_MEM_SIZE,
                flags: PageFlags::RWX,
                name: "Main Memory",
            },
            MemoryRegion {
                base: USER_MEM_BASE,
                size: USER_MEM_SIZE,
                flags: PageFlags::RWX,
                name: "User Memory",
            },
            MemoryRegion {
                base: RSX_IO_BASE,
                size: RSX_IO_SIZE,
                flags: PageFlags::RW | PageFlags::MMIO,
                name: "RSX I/O",
            },
            MemoryRegion {
                base: STACK_BASE,
                size: STACK_SIZE,
                flags: PageFlags::RW,
                name: "Stack",
            },
        ];

        let mut manager = Self {
            base,
            allocation_map,
            page_flags,
            reservations,
            regions,
            rsx_mem,
            shared_regions: RwLock::new(HashMap::new()),
            next_shared_id: RwLock::new(1),
            rsx_mappings: RwLock::new(Vec::new()),
            exception_handler: RwLock::new(None),
        };

        // Initialize standard regions
        manager.init_regions()?;

        Ok(Arc::new(manager))
    }

    #[cfg(unix)]
    fn allocate_address_space(size: usize) -> Result<*mut u8, MemoryError> {
        use libc::{mmap, MAP_ANONYMOUS, MAP_PRIVATE, PROT_READ, PROT_WRITE};

        let ptr = unsafe {
            mmap(
                std::ptr::null_mut(),
                size,
                PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANONYMOUS,
                -1,
                0,
            )
        };

        if ptr == libc::MAP_FAILED {
            return Err(MemoryError::OutOfMemory);
        }

        Ok(ptr as *mut u8)
    }

    #[cfg(windows)]
    fn allocate_address_space(size: usize) -> Result<*mut u8, MemoryError> {
        use windows_sys::Win32::System::Memory::*;

        let ptr = unsafe {
            VirtualAlloc(
                std::ptr::null(),
                size,
                MEM_RESERVE | MEM_COMMIT,
                PAGE_READWRITE,
            )
        };

        if ptr.is_null() {
            return Err(MemoryError::OutOfMemory);
        }

        Ok(ptr as *mut u8)
    }

    fn init_regions(&mut self) -> Result<(), MemoryError> {
        // Commit main memory
        self.commit_region(MAIN_MEM_BASE, MAIN_MEM_SIZE, PageFlags::RWX)?;

        // Commit user memory
        self.commit_region(USER_MEM_BASE, USER_MEM_SIZE, PageFlags::RWX)?;

        // Commit stack
        self.commit_region(STACK_BASE, STACK_SIZE, PageFlags::RW)?;

        Ok(())
    }

    fn commit_region(&mut self, addr: u32, size: u32, flags: PageFlags) -> Result<(), MemoryError> {
        let start_page = (addr / PAGE_SIZE) as usize;
        let num_pages = (size / PAGE_SIZE) as usize;

        let mut page_flags = self.page_flags.write();

        for i in start_page..start_page + num_pages {
            if i < page_flags.len() {
                page_flags[i] = flags;
            }
        }

        Ok(())
    }

    /// Get raw pointer for address (unchecked, for hot paths)
    ///
    /// # Safety
    /// Caller must ensure the address is valid and properly aligned.
    #[inline(always)]
    pub unsafe fn ptr(&self, addr: u32) -> *mut u8 {
        self.base.add(addr as usize)
    }

    /// Get pointer with bounds and permission checking
    pub fn get_ptr(&self, addr: u32, size: u32, flags: PageFlags) -> Result<*mut u8, MemoryError> {
        self.check_access(addr, size, flags)?;
        Ok(unsafe { self.ptr(addr) })
    }

    /// Check if memory access is valid
    pub fn check_access(&self, addr: u32, size: u32, required: PageFlags) -> Result<(), MemoryError> {
        let start_page = (addr / PAGE_SIZE) as usize;
        let end_addr = addr.checked_add(size.saturating_sub(1)).ok_or(MemoryError::InvalidAddress(addr))?;
        let end_page = (end_addr / PAGE_SIZE) as usize;

        let page_flags = self.page_flags.read();

        for page in start_page..=end_page {
            if page >= page_flags.len() {
                return Err(MemoryError::InvalidAddress(addr));
            }

            if !page_flags[page].contains(required) {
                return Err(MemoryError::AccessViolation {
                    addr,
                    kind: if required.contains(PageFlags::WRITE) {
                        AccessKind::Write
                    } else if required.contains(PageFlags::EXECUTE) {
                        AccessKind::Execute
                    } else {
                        AccessKind::Read
                    },
                });
            }
        }

        Ok(())
    }

    /// Read a value from memory
    #[inline]
    pub fn read<T: Copy>(&self, addr: u32) -> Result<T, MemoryError> {
        self.check_access(addr, std::mem::size_of::<T>() as u32, PageFlags::READ)?;
        Ok(unsafe { self.read_unchecked(addr) })
    }

    /// Read without checking (for hot paths after validation)
    ///
    /// # Safety
    /// Caller must ensure the address is valid and readable.
    #[inline(always)]
    pub unsafe fn read_unchecked<T: Copy>(&self, addr: u32) -> T {
        std::ptr::read_unaligned(self.ptr(addr) as *const T)
    }

    /// Write a value to memory
    #[inline]
    pub fn write<T: Copy>(&self, addr: u32, value: T) -> Result<(), MemoryError> {
        self.check_access(addr, std::mem::size_of::<T>() as u32, PageFlags::WRITE)?;
        unsafe { self.write_unchecked(addr, value) };
        Ok(())
    }

    /// Write without checking (for hot paths after validation)
    ///
    /// # Safety
    /// Caller must ensure the address is valid and writable.
    #[inline(always)]
    pub unsafe fn write_unchecked<T: Copy>(&self, addr: u32, value: T) {
        std::ptr::write_unaligned(self.ptr(addr) as *mut T, value);
    }

    /// Get reservation for address
    #[inline(always)]
    pub fn reservation(&self, addr: u32) -> &Reservation {
        let index = (addr / RESERVATION_GRANULARITY) as usize;
        &self.reservations[index]
    }

    /// Allocate memory in the user memory region
    pub fn allocate(&self, size: u32, _align: u32, flags: PageFlags) -> Result<u32, MemoryError> {
        let aligned_size = (size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        let num_pages = aligned_size / PAGE_SIZE;

        let mut allocation_map = self.allocation_map.write();
        let mut page_flags = self.page_flags.write();

        // Find contiguous free pages in user memory region
        let start_page = (USER_MEM_BASE / PAGE_SIZE) as usize;
        let end_page = ((USER_MEM_BASE + USER_MEM_SIZE) / PAGE_SIZE) as usize;

        let mut found_start = None;
        let mut consecutive = 0u32;

        for page in start_page..end_page {
            let word_idx = page / 64;
            let bit_idx = page % 64;

            if allocation_map[word_idx] & (1u64 << bit_idx) == 0 {
                if consecutive == 0 {
                    found_start = Some(page);
                }
                consecutive += 1;

                if consecutive >= num_pages {
                    break;
                }
            } else {
                consecutive = 0;
                found_start = None;
            }
        }

        let alloc_start_page = found_start.ok_or(MemoryError::OutOfMemory)?;

        if consecutive < num_pages {
            return Err(MemoryError::OutOfMemory);
        }

        // Mark pages as allocated
        for page in alloc_start_page..alloc_start_page + num_pages as usize {
            let word_idx = page / 64;
            let bit_idx = page % 64;
            allocation_map[word_idx] |= 1u64 << bit_idx;
            page_flags[page] = flags;
        }

        Ok((alloc_start_page as u32) * PAGE_SIZE)
    }

    /// Free allocated memory
    pub fn free(&self, addr: u32, size: u32) -> Result<(), MemoryError> {
        let start_page = (addr / PAGE_SIZE) as usize;
        let num_pages = size.div_ceil(PAGE_SIZE) as usize;

        let mut allocation_map = self.allocation_map.write();
        let mut page_flags = self.page_flags.write();

        for page in start_page..start_page + num_pages {
            if page < allocation_map.len() * 64 {
                let word_idx = page / 64;
                let bit_idx = page % 64;
                allocation_map[word_idx] &= !(1u64 << bit_idx);
            }
            if page < page_flags.len() {
                page_flags[page] = PageFlags::empty();
            }
        }

        Ok(())
    }

    /// Get RSX memory pointer
    pub fn rsx_ptr(&self, offset: u32) -> *mut u8 {
        unsafe { self.rsx_mem.add(offset as usize) }
    }

    /// Copy data to memory
    pub fn write_bytes(&self, addr: u32, data: &[u8]) -> Result<(), MemoryError> {
        self.check_access(addr, data.len() as u32, PageFlags::WRITE)?;
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), self.ptr(addr), data.len());
        }
        Ok(())
    }

    /// Copy data from memory
    pub fn read_bytes(&self, addr: u32, size: u32) -> Result<Vec<u8>, MemoryError> {
        self.check_access(addr, size, PageFlags::READ)?;
        let mut data = vec![0u8; size as usize];
        unsafe {
            std::ptr::copy_nonoverlapping(self.ptr(addr), data.as_mut_ptr(), size as usize);
        }
        Ok(data)
    }

    /// Get memory regions
    pub fn regions(&self) -> &[MemoryRegion] {
        &self.regions
    }

    /// Read a big-endian u16 (PS3 is big-endian)
    #[inline]
    pub fn read_be16(&self, addr: u32) -> Result<u16, MemoryError> {
        let value: u16 = self.read(addr)?;
        Ok(u16::from_be(value))
    }

    /// Write a big-endian u16
    #[inline]
    pub fn write_be16(&self, addr: u32, value: u16) -> Result<(), MemoryError> {
        self.write(addr, value.to_be())
    }

    /// Read a big-endian u32 (PS3 is big-endian)
    #[inline]
    pub fn read_be32(&self, addr: u32) -> Result<u32, MemoryError> {
        let value: u32 = self.read(addr)?;
        Ok(u32::from_be(value))
    }

    /// Write a big-endian u32
    #[inline]
    pub fn write_be32(&self, addr: u32, value: u32) -> Result<(), MemoryError> {
        self.write(addr, value.to_be())
    }

    /// Read a big-endian u64
    #[inline]
    pub fn read_be64(&self, addr: u32) -> Result<u64, MemoryError> {
        let value: u64 = self.read(addr)?;
        Ok(u64::from_be(value))
    }

    /// Write a big-endian u64
    #[inline]
    pub fn write_be64(&self, addr: u32, value: u64) -> Result<(), MemoryError> {
        self.write(addr, value.to_be())
    }
    
    // ============ Shared Memory Region Methods ============
    
    /// Create a new shared memory region
    pub fn create_shared_region(&self, addr: u32, size: u32, flags: PageFlags) -> Result<u32, MemoryError> {
        // Validate the region is allocated
        self.check_access(addr, size, PageFlags::READ)?;
        
        let id = {
            let mut next_id = self.next_shared_id.write();
            let id = *next_id;
            *next_id += 1;
            id
        };
        
        let region = SharedMemoryRegion::new(id, addr, size, flags);
        self.shared_regions.write().insert(id, region);
        
        Ok(id)
    }
    
    /// Get a shared memory region by ID
    pub fn get_shared_region(&self, id: u32) -> Option<SharedMemoryRegion> {
        self.shared_regions.read().get(&id).cloned()
    }
    
    /// Grant SPU access to a shared region
    pub fn grant_spu_access(&self, region_id: u32, spu_id: u32) -> Result<(), MemoryError> {
        let mut regions = self.shared_regions.write();
        if let Some(region) = regions.get_mut(&region_id) {
            region.grant_spu_access(spu_id);
            Ok(())
        } else {
            Err(MemoryError::InvalidAddress(region_id))
        }
    }
    
    /// Revoke SPU access from a shared region
    pub fn revoke_spu_access(&self, region_id: u32, spu_id: u32) -> Result<(), MemoryError> {
        let mut regions = self.shared_regions.write();
        if let Some(region) = regions.get_mut(&region_id) {
            region.revoke_spu_access(spu_id);
            Ok(())
        } else {
            Err(MemoryError::InvalidAddress(region_id))
        }
    }
    
    /// Check if an SPU has access to a shared region
    pub fn check_spu_access(&self, region_id: u32, spu_id: u32) -> bool {
        self.shared_regions.read()
            .get(&region_id)
            .map(|r| r.has_spu_access(spu_id))
            .unwrap_or(false)
    }
    
    /// Destroy a shared memory region
    pub fn destroy_shared_region(&self, id: u32) -> Result<(), MemoryError> {
        if self.shared_regions.write().remove(&id).is_some() {
            Ok(())
        } else {
            Err(MemoryError::InvalidAddress(id))
        }
    }
    
    /// List all shared regions
    pub fn list_shared_regions(&self) -> Vec<SharedMemoryRegion> {
        self.shared_regions.read().values().cloned().collect()
    }
    
    // ============ RSX Memory Mapping Methods ============
    
    /// Create an RSX memory mapping
    pub fn map_rsx_memory(&self, rsx_offset: u32, main_addr: u32, size: u32, tiled: bool, tile_pitch: u32) -> Result<(), MemoryError> {
        // Validate RSX offset
        if rsx_offset.saturating_add(size) > RSX_MEM_SIZE {
            return Err(MemoryError::InvalidAddress(rsx_offset));
        }
        
        // Validate main memory address
        self.check_access(main_addr, size, PageFlags::RW)?;
        
        let mapping = RsxMemoryMapping {
            rsx_offset,
            main_addr,
            size,
            tiled,
            tile_pitch,
        };
        
        self.rsx_mappings.write().push(mapping);
        Ok(())
    }
    
    /// Remove an RSX memory mapping
    pub fn unmap_rsx_memory(&self, rsx_offset: u32) {
        self.rsx_mappings.write().retain(|m| m.rsx_offset != rsx_offset);
    }
    
    /// Find RSX mapping for a given RSX offset
    pub fn find_rsx_mapping(&self, rsx_offset: u32) -> Option<RsxMemoryMapping> {
        self.rsx_mappings.read().iter()
            .find(|m| rsx_offset >= m.rsx_offset && rsx_offset < m.rsx_offset + m.size)
            .cloned()
    }
    
    /// Read from RSX local memory
    pub fn read_rsx(&self, offset: u32, size: u32) -> Result<Vec<u8>, MemoryError> {
        if offset.saturating_add(size) > RSX_MEM_SIZE {
            return Err(MemoryError::InvalidAddress(RSX_MEM_BASE + offset));
        }
        
        let mut data = vec![0u8; size as usize];
        unsafe {
            std::ptr::copy_nonoverlapping(
                self.rsx_mem.add(offset as usize),
                data.as_mut_ptr(),
                size as usize,
            );
        }
        Ok(data)
    }
    
    /// Write to RSX local memory
    pub fn write_rsx(&self, offset: u32, data: &[u8]) -> Result<(), MemoryError> {
        if offset.saturating_add(data.len() as u32) > RSX_MEM_SIZE {
            return Err(MemoryError::InvalidAddress(RSX_MEM_BASE + offset));
        }
        
        unsafe {
            std::ptr::copy_nonoverlapping(
                data.as_ptr(),
                self.rsx_mem.add(offset as usize),
                data.len(),
            );
        }
        Ok(())
    }
    
    /// Get list of RSX mappings
    pub fn list_rsx_mappings(&self) -> Vec<RsxMemoryMapping> {
        self.rsx_mappings.read().clone()
    }
    
    // ============ Memory Protection Exception Methods ============
    
    /// Set a custom memory exception handler
    pub fn set_exception_handler<F>(&self, handler: F) 
    where 
        F: Fn(MemoryException) -> ExceptionHandlerResult + Send + Sync + 'static 
    {
        *self.exception_handler.write() = Some(Box::new(handler));
    }
    
    /// Clear the exception handler
    pub fn clear_exception_handler(&self) {
        *self.exception_handler.write() = None;
    }
    
    /// Handle a memory exception
    pub fn handle_exception(&self, exception: MemoryException) -> ExceptionHandlerResult {
        if let Some(ref handler) = *self.exception_handler.read() {
            handler(exception)
        } else {
            ExceptionHandlerResult::NotHandled
        }
    }
    
    /// Check access with exception handling
    pub fn check_access_with_exception(&self, addr: u32, size: u32, required: PageFlags) -> Result<(), MemoryError> {
        match self.check_access(addr, size, required) {
            Ok(()) => Ok(()),
            Err(MemoryError::AccessViolation { addr, kind }) => {
                let exception = MemoryException::AccessViolation { addr, kind };
                match self.handle_exception(exception) {
                    ExceptionHandlerResult::Handled | ExceptionHandlerResult::Retry => Ok(()),
                    ExceptionHandlerResult::NotHandled => Err(MemoryError::AccessViolation { addr, kind }),
                }
            }
            Err(MemoryError::InvalidAddress(addr)) => {
                let exception = MemoryException::PageFault { addr };
                match self.handle_exception(exception) {
                    ExceptionHandlerResult::Handled | ExceptionHandlerResult::Retry => Ok(()),
                    ExceptionHandlerResult::NotHandled => Err(MemoryError::InvalidAddress(addr)),
                }
            }
            Err(e) => Err(e),
        }
    }
    
    /// Set page protection flags
    pub fn set_page_flags(&self, addr: u32, size: u32, flags: PageFlags) -> Result<(), MemoryError> {
        let start_page = (addr / PAGE_SIZE) as usize;
        let num_pages = size.div_ceil(PAGE_SIZE) as usize;
        
        let mut page_flags = self.page_flags.write();
        
        for page in start_page..start_page + num_pages {
            if page < page_flags.len() {
                page_flags[page] = flags;
            }
        }
        
        Ok(())
    }
    
    /// Get page protection flags
    pub fn get_page_flags(&self, addr: u32) -> PageFlags {
        let page = (addr / PAGE_SIZE) as usize;
        let page_flags = self.page_flags.read();
        if page < page_flags.len() {
            page_flags[page]
        } else {
            PageFlags::empty()
        }
    }
}

impl Drop for MemoryManager {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            libc::munmap(self.base as *mut libc::c_void, ADDRESS_SPACE_SIZE);
            libc::munmap(self.rsx_mem as *mut libc::c_void, RSX_MEM_SIZE as usize);
        }

        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::System::Memory::*;
            VirtualFree(self.base as *mut _, 0, MEM_RELEASE);
            VirtualFree(self.rsx_mem as *mut _, 0, MEM_RELEASE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_creation() {
        let mem = MemoryManager::new().unwrap();
        assert!(!mem.regions().is_empty());
    }

    #[test]
    fn test_memory_allocation() {
        let mem = MemoryManager::new().unwrap();

        let addr = mem.allocate(0x10000, 0x1000, PageFlags::RW).unwrap();
        assert!(addr >= USER_MEM_BASE);
        assert!(addr < USER_MEM_BASE + USER_MEM_SIZE);

        mem.free(addr, 0x10000).unwrap();
    }

    #[test]
    fn test_read_write() {
        let mem = MemoryManager::new().unwrap();

        let addr = mem.allocate(0x1000, 0x1000, PageFlags::RW).unwrap();

        mem.write::<u32>(addr, 0x12345678).unwrap();
        assert_eq!(mem.read::<u32>(addr).unwrap(), 0x12345678);

        mem.write::<u64>(addr + 4, 0xDEADBEEFCAFEBABE).unwrap();
        assert_eq!(mem.read::<u64>(addr + 4).unwrap(), 0xDEADBEEFCAFEBABE);
    }

    #[test]
    fn test_big_endian() {
        let mem = MemoryManager::new().unwrap();

        let addr = mem.allocate(0x1000, 0x1000, PageFlags::RW).unwrap();

        mem.write_be32(addr, 0x12345678).unwrap();
        assert_eq!(mem.read_be32(addr).unwrap(), 0x12345678);

        mem.write_be64(addr + 8, 0xDEADBEEFCAFEBABE).unwrap();
        assert_eq!(mem.read_be64(addr + 8).unwrap(), 0xDEADBEEFCAFEBABE);
    }

    #[test]
    fn test_write_read_bytes() {
        let mem = MemoryManager::new().unwrap();

        let addr = mem.allocate(0x1000, 0x1000, PageFlags::RW).unwrap();

        let data = b"Hello, PS3!";
        mem.write_bytes(addr, data).unwrap();

        let read_data = mem.read_bytes(addr, data.len() as u32).unwrap();
        assert_eq!(read_data, data);
    }

    #[test]
    fn test_reservation() {
        let mem = MemoryManager::new().unwrap();

        let addr = 0x1000u32;
        let res = mem.reservation(addr);

        let time = res.acquire();
        assert!(res.try_lock(time));
        res.unlock_and_increment();

        let new_time = res.acquire();
        assert_eq!(new_time, time + 128);
    }
    
    #[test]
    fn test_shared_memory_region() {
        let mem = MemoryManager::new().unwrap();
        
        // Allocate some memory
        let addr = mem.allocate(0x1000, 0x1000, PageFlags::RW).unwrap();
        
        // Create shared region
        let region_id = mem.create_shared_region(addr, 0x1000, PageFlags::RW).unwrap();
        assert!(region_id > 0);
        
        // Get the region
        let region = mem.get_shared_region(region_id).unwrap();
        assert_eq!(region.main_addr, addr);
        assert_eq!(region.size, 0x1000);
        
        // Grant SPU access
        mem.grant_spu_access(region_id, 0).unwrap();
        assert!(mem.check_spu_access(region_id, 0));
        assert!(!mem.check_spu_access(region_id, 1));
        
        // Revoke SPU access
        mem.revoke_spu_access(region_id, 0).unwrap();
        assert!(!mem.check_spu_access(region_id, 0));
        
        // Destroy the region
        mem.destroy_shared_region(region_id).unwrap();
        assert!(mem.get_shared_region(region_id).is_none());
    }
    
    #[test]
    fn test_rsx_memory_mapping() {
        let mem = MemoryManager::new().unwrap();
        
        // Allocate main memory for mapping
        let addr = mem.allocate(0x10000, 0x1000, PageFlags::RW).unwrap();
        
        // Create RSX mapping
        mem.map_rsx_memory(0x1000, addr, 0x10000, false, 0).unwrap();
        
        // Find the mapping
        let mapping = mem.find_rsx_mapping(0x1000).unwrap();
        assert_eq!(mapping.rsx_offset, 0x1000);
        assert_eq!(mapping.main_addr, addr);
        assert_eq!(mapping.size, 0x10000);
        
        // Find mapping at offset within range
        let mapping2 = mem.find_rsx_mapping(0x5000).unwrap();
        assert_eq!(mapping2.rsx_offset, 0x1000);
        
        // Offset outside range should not find
        assert!(mem.find_rsx_mapping(0x100000).is_none());
        
        // Unmap
        mem.unmap_rsx_memory(0x1000);
        assert!(mem.find_rsx_mapping(0x1000).is_none());
    }
    
    #[test]
    fn test_rsx_read_write() {
        let mem = MemoryManager::new().unwrap();
        
        // Write to RSX memory
        let data = b"RSX Test Data";
        mem.write_rsx(0x1000, data).unwrap();
        
        // Read it back
        let read_data = mem.read_rsx(0x1000, data.len() as u32).unwrap();
        assert_eq!(read_data.as_slice(), data);
    }
    
    #[test]
    fn test_page_flags() {
        let mem = MemoryManager::new().unwrap();
        
        let addr = mem.allocate(0x2000, 0x1000, PageFlags::RW).unwrap();
        
        // Check initial flags
        let flags = mem.get_page_flags(addr);
        assert!(flags.contains(PageFlags::READ));
        assert!(flags.contains(PageFlags::WRITE));
        
        // Change to read-only
        mem.set_page_flags(addr, 0x1000, PageFlags::READ).unwrap();
        let flags = mem.get_page_flags(addr);
        assert!(flags.contains(PageFlags::READ));
        assert!(!flags.contains(PageFlags::WRITE));
    }
    
    #[test]
    fn test_exception_handler() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        
        let mem = MemoryManager::new().unwrap();
        let handler_called = Arc::new(AtomicBool::new(false));
        let handler_called_clone = handler_called.clone();
        
        // Set exception handler
        mem.set_exception_handler(move |exception| {
            handler_called_clone.store(true, Ordering::SeqCst);
            match exception {
                MemoryException::AccessViolation { .. } => ExceptionHandlerResult::Handled,
                _ => ExceptionHandlerResult::NotHandled,
            }
        });
        
        // Try to access invalid address - should trigger handler
        let addr = mem.allocate(0x1000, 0x1000, PageFlags::READ).unwrap();
        
        // Try write access on read-only page
        let result = mem.check_access_with_exception(addr, 4, PageFlags::WRITE);
        
        // Handler was called
        assert!(handler_called.load(Ordering::SeqCst));
        
        // Since handler returned Handled, the result should be Ok
        assert!(result.is_ok());
    }
}
