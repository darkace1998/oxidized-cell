//! DMA transfer acceleration interface
//!
//! Safe Rust wrappers for the C++ DMA engine that accelerates SPU↔PPU transfers.
//! Supports single transfers, scatter-gather list commands, and fence/barrier
//! synchronization.

extern "C" {
    fn oc_dma_transfer(
        local_storage: *mut u8, local_addr: u32,
        main_memory: *mut u8, ea: u64, size: u32,
        tag: u16, cmd: u8,
    ) -> i32;
    fn oc_dma_list_transfer(
        local_storage: *mut u8, list_addr: u32,
        main_memory: *mut u8, list_size: u32,
        tag: u16, cmd: u8,
    ) -> i32;
    fn oc_dma_fence(tag: u16) -> i32;
    fn oc_dma_barrier() -> i32;
    fn oc_dma_get_tag_status() -> u32;
    fn oc_dma_complete_tag(tag: u16) -> i32;
    fn oc_dma_get_stats(
        gets: *mut u64, puts: *mut u64,
        list_gets: *mut u64, list_puts: *mut u64,
        bytes_in: *mut u64, bytes_out: *mut u64,
        fences: *mut u64, barriers: *mut u64,
    );
    fn oc_dma_reset_stats();
}

/// DMA command types matching the Cell SPU MFC command set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DmaCommand {
    /// GET: read from main memory into local store
    Get = 0x40,
    /// PUT: write from local store to main memory
    Put = 0x20,
    /// GETL: scatter-gather read
    GetList = 0x44,
    /// PUTL: scatter-gather write
    PutList = 0x24,
    /// GETLB: list GET with barrier
    GetListBarrier = 0x4C,
    /// PUTLB: list PUT with barrier
    PutListBarrier = 0x2C,
}

/// DMA transfer error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaError {
    /// Null pointer passed
    NullPointer,
    /// Invalid transfer size
    InvalidSize,
    /// Invalid tag (must be 0-31)
    InvalidTag,
    /// Local address out of bounds for SPU local store
    LocalAddrOutOfBounds,
    /// Fence or barrier is blocking this transfer
    Blocked,
    /// Unknown error
    Unknown(i32),
}

impl std::fmt::Display for DmaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DmaError::NullPointer => write!(f, "Null pointer"),
            DmaError::InvalidSize => write!(f, "Invalid transfer size"),
            DmaError::InvalidTag => write!(f, "Invalid tag (must be 0-31)"),
            DmaError::LocalAddrOutOfBounds => write!(f, "Local address out of bounds"),
            DmaError::Blocked => write!(f, "Transfer blocked by fence/barrier"),
            DmaError::Unknown(code) => write!(f, "Unknown DMA error: {}", code),
        }
    }
}

impl std::error::Error for DmaError {}

fn map_dma_error(code: i32) -> DmaError {
    match code {
        -1 => DmaError::NullPointer,
        -2 => DmaError::InvalidSize,
        -3 => DmaError::InvalidTag,
        -4 => DmaError::LocalAddrOutOfBounds,
        -5 => DmaError::Blocked,
        other => DmaError::Unknown(other),
    }
}

/// Execute a DMA transfer between SPU local storage and main memory.
///
/// # Safety
/// Both `local_storage` and `main_memory` must be valid mutable slices of sufficient
/// size to cover the transfer offsets and size.
pub unsafe fn dma_transfer(
    local_storage: &mut [u8],
    local_addr: u32,
    main_memory: &mut [u8],
    ea: u64,
    size: u32,
    tag: u16,
    cmd: DmaCommand,
) -> Result<(), DmaError> {
    let result = oc_dma_transfer(
        local_storage.as_mut_ptr(), local_addr,
        main_memory.as_mut_ptr(), ea, size,
        tag, cmd as u8,
    );
    if result == 0 { Ok(()) } else { Err(map_dma_error(result)) }
}

/// Execute a DMA list (scatter-gather) transfer.
///
/// # Safety
/// Both `local_storage` and `main_memory` must be valid mutable slices.
pub unsafe fn dma_list_transfer(
    local_storage: &mut [u8],
    list_addr: u32,
    main_memory: &mut [u8],
    list_size: u32,
    tag: u16,
    cmd: DmaCommand,
) -> Result<i32, DmaError> {
    let result = oc_dma_list_transfer(
        local_storage.as_mut_ptr(), list_addr,
        main_memory.as_mut_ptr(), list_size,
        tag, cmd as u8,
    );
    if result >= 0 { Ok(result) } else { Err(map_dma_error(result)) }
}

/// Insert a DMA fence for a tag group.
pub fn dma_fence(tag: u16) -> Result<(), DmaError> {
    let result = unsafe { oc_dma_fence(tag) };
    if result == 0 { Ok(()) } else { Err(map_dma_error(result)) }
}

/// Insert a DMA barrier across all tags.
pub fn dma_barrier() -> Result<(), DmaError> {
    let result = unsafe { oc_dma_barrier() };
    if result == 0 { Ok(()) } else { Err(map_dma_error(result)) }
}

/// Get DMA tag completion status.
/// Returns a 32-bit mask where bit N is set if tag N has no pending transfers.
pub fn get_tag_status() -> u32 {
    unsafe { oc_dma_get_tag_status() }
}

/// Mark all pending DMA transfers for a tag as complete.
pub fn complete_tag(tag: u16) -> Result<(), DmaError> {
    let result = unsafe { oc_dma_complete_tag(tag) };
    if result == 0 { Ok(()) } else { Err(map_dma_error(result)) }
}

/// DMA engine statistics.
#[derive(Debug, Clone, Default)]
pub struct DmaStats {
    pub gets: u64,
    pub puts: u64,
    pub list_gets: u64,
    pub list_puts: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub fences: u64,
    pub barriers: u64,
}

/// Get DMA engine statistics.
pub fn get_stats() -> DmaStats {
    let mut stats = DmaStats::default();
    unsafe {
        oc_dma_get_stats(
            &mut stats.gets, &mut stats.puts,
            &mut stats.list_gets, &mut stats.list_puts,
            &mut stats.bytes_in, &mut stats.bytes_out,
            &mut stats.fences, &mut stats.barriers,
        );
    }
    stats
}

/// Reset DMA statistics and clear all pending transfers.
pub fn reset_stats() {
    unsafe { oc_dma_reset_stats() };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dma_get_transfer() {
        reset_stats();
        
        let mut local_store = vec![0u8; 0x40000]; // 256KB SPU local store
        let mut main_mem = vec![0u8; 0x10000];    // 64KB main memory
        
        // Write test data to main memory
        main_mem[0..4].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        
        // GET: read from main memory at EA 0 into local store at offset 0x100
        unsafe {
            let result = dma_transfer(
                &mut local_store, 0x100,
                &mut main_mem, 0, 4,
                0, DmaCommand::Get,
            );
            assert!(result.is_ok(), "GET should succeed");
        }
        
        // Verify data was copied
        assert_eq!(&local_store[0x100..0x104], &[0xDE, 0xAD, 0xBE, 0xEF]);
        
        let stats = get_stats();
        assert_eq!(stats.gets, 1);
        assert_eq!(stats.bytes_in, 4);
    }

    #[test]
    fn test_dma_put_transfer() {
        reset_stats();
        
        let mut local_store = vec![0u8; 0x40000];
        let mut main_mem = vec![0u8; 0x10000];
        
        // Write test data to local store
        local_store[0x200..0x204].copy_from_slice(&[0xCA, 0xFE, 0xBA, 0xBE]);
        
        // PUT: write from local store at offset 0x200 to main memory at EA 0x50
        unsafe {
            let result = dma_transfer(
                &mut local_store, 0x200,
                &mut main_mem, 0x50, 4,
                1, DmaCommand::Put,
            );
            assert!(result.is_ok(), "PUT should succeed");
        }
        
        assert_eq!(&main_mem[0x50..0x54], &[0xCA, 0xFE, 0xBA, 0xBE]);
        
        let stats = get_stats();
        assert_eq!(stats.puts, 1);
        assert_eq!(stats.bytes_out, 4);
    }

    #[test]
    fn test_dma_fence() {
        reset_stats();
        
        // Fence should succeed for valid tag
        assert!(dma_fence(0).is_ok());
        assert!(dma_fence(31).is_ok());
        
        // Invalid tag should fail
        assert!(dma_fence(32).is_err());
        
        let stats = get_stats();
        assert_eq!(stats.fences, 2);
    }

    #[test]
    fn test_dma_barrier() {
        reset_stats();
        assert!(dma_barrier().is_ok());
        
        let stats = get_stats();
        assert_eq!(stats.barriers, 1);
    }

    #[test]
    fn test_dma_tag_status() {
        reset_stats();
        
        // All tags should be complete initially (no pending transfers)
        let status = get_tag_status();
        assert_eq!(status, 0xFFFF_FFFF, "All tags should be complete initially");
    }

    #[test]
    fn test_dma_complete_tag() {
        reset_stats();
        
        assert!(complete_tag(0).is_ok());
        assert!(complete_tag(31).is_ok());
        assert!(complete_tag(32).is_err());
    }

    #[test]
    fn test_dma_invalid_size() {
        reset_stats();
        
        let mut local_store = vec![0u8; 0x40000];
        let mut main_mem = vec![0u8; 0x10000];
        
        // Size 0 should fail
        unsafe {
            let result = dma_transfer(
                &mut local_store, 0, &mut main_mem, 0, 0, 0, DmaCommand::Get,
            );
            assert_eq!(result.unwrap_err(), DmaError::InvalidSize);
        }
        
        // Size > 16KB should fail
        unsafe {
            let result = dma_transfer(
                &mut local_store, 0, &mut main_mem, 0, 0x10000, 0, DmaCommand::Get,
            );
            assert_eq!(result.unwrap_err(), DmaError::InvalidSize);
        }
    }

    #[test]
    fn test_dma_local_addr_bounds() {
        reset_stats();
        
        let mut local_store = vec![0u8; 0x40000];
        let mut main_mem = vec![0u8; 0x10000];
        
        // Local addr + size exceeding 256KB should fail
        unsafe {
            let result = dma_transfer(
                &mut local_store, 0x3FFFF, &mut main_mem, 0, 2, 0, DmaCommand::Get,
            );
            assert_eq!(result.unwrap_err(), DmaError::LocalAddrOutOfBounds);
        }
    }
}
