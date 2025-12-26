//! Page flags and management

use bitflags::bitflags;

/// Page size enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PageSize {
    /// Standard 4KB pages
    Standard = 0x1000,
    /// Large 1MB pages (64KB on some platforms)
    Large = 0x10_0000,
    /// Huge 16MB pages (for special allocations)
    Huge = 0x100_0000,
}

impl PageSize {
    /// Get the size in bytes
    pub fn bytes(&self) -> u32 {
        *self as u32
    }
    
    /// Check if address is aligned to this page size
    pub fn is_aligned(&self, addr: u32) -> bool {
        addr % self.bytes() == 0
    }
    
    /// Align address down to page boundary
    pub fn align_down(&self, addr: u32) -> u32 {
        addr & !(self.bytes() - 1)
    }
    
    /// Align address up to page boundary
    pub fn align_up(&self, addr: u32) -> u32 {
        (addr + self.bytes() - 1) & !(self.bytes() - 1)
    }
    
    /// Get number of standard pages in this page size
    pub fn standard_pages(&self) -> u32 {
        self.bytes() / PageSize::Standard.bytes()
    }
}

bitflags! {
    /// Page protection and attribute flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct PageFlags: u32 {
        /// Page is readable
        const READ    = 0b0000_0001;
        /// Page is writable
        const WRITE   = 0b0000_0010;
        /// Page is executable
        const EXECUTE = 0b0000_0100;
        /// Page is memory-mapped I/O
        const MMIO    = 0b0000_1000;
        /// Page is a large page (1MB)
        const LARGE   = 0b0001_0000;
        /// Page is a huge page (16MB)
        const HUGE    = 0b0010_0000;
        /// Page is locked in memory (cannot be swapped)
        const LOCKED  = 0b0100_0000;
        /// Page is shared between processes
        const SHARED  = 0b1000_0000;

        /// Read and write access
        const RW  = Self::READ.bits() | Self::WRITE.bits();
        /// Read, write, and execute access
        const RWX = Self::READ.bits() | Self::WRITE.bits() | Self::EXECUTE.bits();
        /// Read and execute access
        const RX  = Self::READ.bits() | Self::EXECUTE.bits();
    }
}

impl Default for PageFlags {
    fn default() -> Self {
        Self::empty()
    }
}

impl PageFlags {
    /// Create flags for a large page with given permissions
    pub fn large_page(perms: PageFlags) -> Self {
        perms | PageFlags::LARGE
    }
    
    /// Create flags for a huge page with given permissions
    pub fn huge_page(perms: PageFlags) -> Self {
        perms | PageFlags::HUGE
    }
    
    /// Check if this is a large page
    pub fn is_large(&self) -> bool {
        self.contains(PageFlags::LARGE)
    }
    
    /// Check if this is a huge page
    pub fn is_huge(&self) -> bool {
        self.contains(PageFlags::HUGE)
    }
    
    /// Get the page size from flags
    pub fn page_size(&self) -> PageSize {
        if self.is_huge() {
            PageSize::Huge
        } else if self.is_large() {
            PageSize::Large
        } else {
            PageSize::Standard
        }
    }
}
