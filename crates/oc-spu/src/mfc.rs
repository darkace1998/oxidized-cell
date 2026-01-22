//! SPU Memory Flow Controller (MFC)
//!
//! The MFC handles DMA transfers between SPU local storage and main memory.

use std::collections::VecDeque;

/// MFC command opcodes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MfcCommand {
    /// Put (local to main)
    Put = 0x20,
    /// Put with barrier
    PutB = 0x21,
    /// Put with fence
    PutF = 0x22,
    /// Put unconditional
    PutU = 0x28,
    /// Put list (local to main, list of elements)
    PutL = 0x24,
    /// Put list with barrier
    PutLB = 0x25,
    /// Put list with fence
    PutLF = 0x26,
    /// Get (main to local)
    Get = 0x40,
    /// Get with barrier
    GetB = 0x41,
    /// Get with fence
    GetF = 0x42,
    /// Get unconditional
    GetU = 0x48,
    /// Get list (main to local, list of elements)
    GetL = 0x44,
    /// Get list with barrier
    GetLB = 0x45,
    /// Get list with fence
    GetLF = 0x46,
    /// Get Lock Line Unconditional (atomic reservation)
    GetLLAR = 0xD0,
    /// Put Lock Line Conditional (atomic store)
    PutLLC = 0xB4,
    /// Put Lock Line Unconditional
    PutLLUC = 0xB0,
    /// Barrier
    Barrier = 0xC0,
    /// Unknown/Invalid
    Unknown = 0xFF,
}

impl MfcCommand {
    /// Get the base latency for this command type (in cycles)
    pub fn base_latency(&self) -> u64 {
        match self {
            Self::Get | Self::GetU => 100,
            Self::GetB | Self::GetF => 120,
            Self::GetL => 150, // List commands have higher overhead
            Self::GetLB | Self::GetLF => 170,
            Self::Put | Self::PutU => 80,
            Self::PutB | Self::PutF => 100,
            Self::PutL => 130, // List commands have higher overhead
            Self::PutLB | Self::PutLF => 150,
            Self::GetLLAR => 150,
            Self::PutLLC | Self::PutLLUC => 120,
            Self::Barrier => 50,
            Self::Unknown => 0,
        }
    }

    /// Calculate transfer latency based on size (cycles per 128 bytes)
    pub fn transfer_latency(&self, size: u32) -> u64 {
        let blocks = size.div_ceil(128);
        blocks as u64 * 10 // 10 cycles per 128-byte block
    }

    /// Check if this is a list command
    pub fn is_list_command(&self) -> bool {
        matches!(self, Self::GetL | Self::GetLB | Self::GetLF |
                       Self::PutL | Self::PutLB | Self::PutLF)
    }

    /// Check if this is a GET command (including list variants)
    pub fn is_get(&self) -> bool {
        matches!(self, Self::Get | Self::GetB | Self::GetF | Self::GetU |
                       Self::GetL | Self::GetLB | Self::GetLF | Self::GetLLAR)
    }

    /// Check if this is a PUT command (including list variants)
    pub fn is_put(&self) -> bool {
        matches!(self, Self::Put | Self::PutB | Self::PutF | Self::PutU |
                       Self::PutL | Self::PutLB | Self::PutLF |
                       Self::PutLLC | Self::PutLLUC)
    }
}

impl From<u8> for MfcCommand {
    fn from(value: u8) -> Self {
        match value {
            0x20 => Self::Put,
            0x21 => Self::PutB,
            0x22 => Self::PutF,
            0x24 => Self::PutL,
            0x25 => Self::PutLB,
            0x26 => Self::PutLF,
            0x28 => Self::PutU,
            0x40 => Self::Get,
            0x41 => Self::GetB,
            0x42 => Self::GetF,
            0x44 => Self::GetL,
            0x45 => Self::GetLB,
            0x46 => Self::GetLF,
            0x48 => Self::GetU,
            0xD0 => Self::GetLLAR,
            0xB4 => Self::PutLLC,
            0xB0 => Self::PutLLUC,
            0xC0 => Self::Barrier,
            _ => Self::Unknown,
        }
    }
}

/// MFC DMA command
#[derive(Debug, Clone)]
pub struct MfcDmaCommand {
    /// Local storage address
    pub lsa: u32,
    /// Effective address (main memory)
    pub ea: u64,
    /// Transfer size
    pub size: u32,
    /// Tag ID (0-31)
    pub tag: u8,
    /// Command opcode
    pub cmd: MfcCommand,
    /// Issue cycle (when command was queued)
    pub issue_cycle: u64,
    /// Completion cycle (when command will complete)
    pub completion_cycle: u64,
}

/// DMA list element for list transfers
/// Each element is 8 bytes in local storage (big-endian):
/// - Bytes 0-3: Local Storage Address (LSA)
/// - Bytes 4-5: Transfer size (max 16KB per element)
/// - Byte 6: Reserved (stall-and-notify flag in bit 7)
/// - Byte 7: Reserved
#[derive(Debug, Clone)]
pub struct MfcListElement {
    /// Local storage address
    pub lsa: u32,
    /// Transfer size
    pub size: u16,
    /// Stall-and-notify flag (if set, SPU stalls after this element)
    pub stall_notify: bool,
}

/// List transfer state for tracking in-progress list DMA operations
#[derive(Debug, Clone)]
pub struct ListTransferState {
    /// List address in local storage
    pub list_addr: u32,
    /// Total list size in bytes
    pub list_size: u32,
    /// Base effective address
    pub ea: u64,
    /// Tag for this transfer
    pub tag: u8,
    /// Command type (GetL, PutL, etc.)
    pub cmd: MfcCommand,
    /// Current element index (0-based)
    pub current_element: usize,
    /// Total number of elements
    pub total_elements: usize,
    /// Whether the list is currently stalled
    pub stalled: bool,
}

/// MFC state
pub struct Mfc {
    /// Command queue
    queue: VecDeque<MfcDmaCommand>,
    /// Tag group completion status (bit per tag)
    tag_status: u32,
    /// Tag group query mask (for any/all queries)
    tag_query_mask: u32,
    /// Atomic reservation address
    reservation_addr: u64,
    /// Atomic reservation data (128 bytes)
    reservation_data: [u8; 128],
    /// Reservation valid flag
    reservation_valid: bool,
    /// Current cycle counter
    cycle_counter: u64,
    /// Pending tags (tags with in-flight operations)
    pending_tags: u32,
    /// Stall and notify tag
    stall_notify_tag: u8,
    /// List stall flag
    list_stall: bool,
    /// Active list transfer state (if any)
    list_transfer: Option<ListTransferState>,
}

impl Mfc {
    /// Create a new MFC
    pub fn new() -> Self {
        Self {
            queue: VecDeque::with_capacity(16),
            tag_status: 0xFFFFFFFF, // All tags initially complete
            tag_query_mask: 0,
            reservation_addr: 0,
            reservation_data: [0; 128],
            reservation_valid: false,
            cycle_counter: 0,
            pending_tags: 0,
            stall_notify_tag: 0,
            list_stall: false,
            list_transfer: None,
        }
    }

    /// Advance the cycle counter and update DMA completion status
    pub fn tick(&mut self, cycles: u64) {
        self.cycle_counter += cycles;
        
        // Check for completed DMA operations
        let mut completed_cmds = Vec::new();
        for (idx, cmd) in self.queue.iter().enumerate() {
            if self.cycle_counter >= cmd.completion_cycle {
                completed_cmds.push(idx);
            }
        }
        
        // Remove completed commands and update tag status
        for idx in completed_cmds.into_iter().rev() {
            if let Some(cmd) = self.queue.remove(idx) {
                self.complete_tag(cmd.tag);
            }
        }
    }

    /// Queue a DMA command with timing
    pub fn queue_command(&mut self, mut cmd: MfcDmaCommand) {
        // Calculate completion time
        let base_latency = cmd.cmd.base_latency();
        let transfer_latency = cmd.cmd.transfer_latency(cmd.size);
        
        cmd.issue_cycle = self.cycle_counter;
        cmd.completion_cycle = self.cycle_counter + base_latency + transfer_latency;
        
        // Mark tag as pending
        self.tag_status &= !(1 << cmd.tag);
        self.pending_tags |= 1 << cmd.tag;
        
        self.queue.push_back(cmd);
    }

    /// Check if queue is empty
    pub fn is_queue_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Get next pending command (for processing)
    pub fn peek_next_command(&self) -> Option<&MfcDmaCommand> {
        self.queue.front()
    }

    /// Get next completed command
    pub fn pop_completed_command(&mut self) -> Option<MfcDmaCommand> {
        if let Some(cmd) = self.queue.front() {
            if self.cycle_counter >= cmd.completion_cycle {
                return self.queue.pop_front();
            }
        }
        None
    }

    /// Mark a tag as complete
    pub fn complete_tag(&mut self, tag: u8) {
        self.tag_status |= 1 << tag;
        self.pending_tags &= !(1 << tag);
    }

    /// Get tag status (bitmask of completed tags)
    pub fn get_tag_status(&self) -> u32 {
        self.tag_status
    }

    /// Check if specific tags are complete
    pub fn check_tags(&self, mask: u32) -> bool {
        (self.tag_status & mask) == mask
    }

    /// Get cycles until tag completion
    pub fn cycles_until_tag_completion(&self, tag: u8) -> Option<u64> {
        for cmd in &self.queue {
            if cmd.tag == tag {
                if self.cycle_counter < cmd.completion_cycle {
                    return Some(cmd.completion_cycle - self.cycle_counter);
                } else {
                    return Some(0);
                }
            }
        }
        None
    }

    /// Set atomic reservation
    pub fn set_reservation(&mut self, addr: u64, data: &[u8]) {
        self.reservation_addr = addr & !127; // Align to 128 bytes
        self.reservation_data[..data.len().min(128)].copy_from_slice(&data[..data.len().min(128)]);
        self.reservation_valid = true;
    }

    /// Get reservation address
    pub fn get_reservation_addr(&self) -> u64 {
        self.reservation_addr
    }

    /// Get reservation data
    pub fn get_reservation_data(&self) -> &[u8; 128] {
        &self.reservation_data
    }

    /// Check if reservation is valid
    pub fn has_reservation(&self) -> bool {
        self.reservation_valid
    }

    /// Clear reservation
    pub fn clear_reservation(&mut self) {
        self.reservation_valid = false;
    }

    /// Get queue size
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    /// Check if queue is full (16 entries max)
    pub fn is_queue_full(&self) -> bool {
        self.queue.len() >= 16
    }

    /// Get current cycle counter
    pub fn get_cycle_counter(&self) -> u64 {
        self.cycle_counter
    }

    /// Get pending tags bitmask
    pub fn get_pending_tags(&self) -> u32 {
        self.pending_tags
    }

    /// Execute DMA GET operation (main memory -> local storage)
    /// Returns true if operation completed immediately
    pub fn execute_get(&mut self, lsa: u32, ea: u64, size: u32, tag: u8, 
                       local_storage: &mut [u8], main_memory: &[u8]) -> bool {
        if size == 0 || size > 16384 {
            return false; // Invalid size
        }

        let cmd = MfcDmaCommand {
            lsa,
            ea,
            size,
            tag,
            cmd: MfcCommand::Get,
            issue_cycle: 0,
            completion_cycle: 0,
        };

        // For small transfers, execute immediately
        if size <= 128 {
            self.perform_get_transfer(lsa, ea, size, local_storage, main_memory);
            self.complete_tag(tag);
            true
        } else {
            self.queue_command(cmd);
            false
        }
    }

    /// Execute DMA PUT operation (local storage -> main memory)
    /// Returns true if operation completed immediately
    pub fn execute_put(&mut self, lsa: u32, ea: u64, size: u32, tag: u8,
                       local_storage: &[u8], main_memory: &mut [u8]) -> bool {
        if size == 0 || size > 16384 {
            return false; // Invalid size
        }

        let cmd = MfcDmaCommand {
            lsa,
            ea,
            size,
            tag,
            cmd: MfcCommand::Put,
            issue_cycle: 0,
            completion_cycle: 0,
        };

        // For small transfers, execute immediately
        if size <= 128 {
            self.perform_put_transfer(lsa, ea, size, local_storage, main_memory);
            self.complete_tag(tag);
            true
        } else {
            self.queue_command(cmd);
            false
        }
    }

    /// Perform actual GET transfer (copy from main to local)
    fn perform_get_transfer(&self, lsa: u32, ea: u64, size: u32,
                           local_storage: &mut [u8], main_memory: &[u8]) {
        let lsa_start = (lsa as usize).min(local_storage.len());
        let lsa_end = (lsa_start + size as usize).min(local_storage.len());
        let ea_start = (ea as usize).min(main_memory.len());
        let ea_end = (ea_start + size as usize).min(main_memory.len());

        let copy_size = (lsa_end - lsa_start).min(ea_end - ea_start);
        if copy_size > 0 {
            local_storage[lsa_start..lsa_start + copy_size]
                .copy_from_slice(&main_memory[ea_start..ea_start + copy_size]);
        }
    }

    /// Perform actual PUT transfer (copy from local to main)
    fn perform_put_transfer(&self, lsa: u32, ea: u64, size: u32,
                           local_storage: &[u8], main_memory: &mut [u8]) {
        let lsa_start = (lsa as usize).min(local_storage.len());
        let lsa_end = (lsa_start + size as usize).min(local_storage.len());
        let ea_start = (ea as usize).min(main_memory.len());
        let ea_end = (ea_start + size as usize).min(main_memory.len());

        let copy_size = (lsa_end - lsa_start).min(ea_end - ea_start);
        if copy_size > 0 {
            main_memory[ea_start..ea_start + copy_size]
                .copy_from_slice(&local_storage[lsa_start..lsa_start + copy_size]);
        }
    }

    /// Execute DMA list operation
    /// List is in local storage, each element is 8 bytes: 4 bytes LSA, 2 bytes size, 2 bytes reserved (with stall flag)
    /// Returns true if completed, false if stalled
    pub fn execute_list_get(&mut self, list_addr: u32, ea: u64, list_size: u32, tag: u8,
                            local_storage: &mut [u8], main_memory: &[u8]) -> bool {
        let elements = self.parse_list(list_addr, list_size, local_storage);
        let total_elements = elements.len();
        
        for (idx, elem) in elements.iter().enumerate() {
            if elem.size > 0 {
                let elem_ea = ea.wrapping_add(elem.lsa as u64);
                self.perform_get_transfer(elem.lsa, elem_ea, elem.size as u32, 
                                        local_storage, main_memory);
            }
            
            // Check for stall-and-notify flag
            if elem.stall_notify {
                self.set_list_stall(tag);
                // Save list transfer state for resumption
                self.list_transfer = Some(ListTransferState {
                    list_addr,
                    list_size,
                    ea,
                    tag,
                    cmd: MfcCommand::GetL,
                    current_element: idx + 1,
                    total_elements,
                    stalled: true,
                });
                return false; // Stalled
            }
        }
        
        self.complete_tag(tag);
        self.list_transfer = None;
        true
    }

    /// Execute DMA list PUT operation
    /// Returns true if completed, false if stalled
    pub fn execute_list_put(&mut self, list_addr: u32, ea: u64, list_size: u32, tag: u8,
                            local_storage: &[u8], main_memory: &mut [u8]) -> bool {
        let elements = self.parse_list(list_addr, list_size, local_storage);
        let total_elements = elements.len();
        
        for (idx, elem) in elements.iter().enumerate() {
            if elem.size > 0 {
                let elem_ea = ea.wrapping_add(elem.lsa as u64);
                self.perform_put_transfer(elem.lsa, elem_ea, elem.size as u32,
                                        local_storage, main_memory);
            }
            
            // Check for stall-and-notify flag
            if elem.stall_notify {
                self.set_list_stall(tag);
                // Save list transfer state for resumption
                self.list_transfer = Some(ListTransferState {
                    list_addr,
                    list_size,
                    ea,
                    tag,
                    cmd: MfcCommand::PutL,
                    current_element: idx + 1,
                    total_elements,
                    stalled: true,
                });
                return false; // Stalled
            }
        }
        
        self.complete_tag(tag);
        self.list_transfer = None;
        true
    }

    /// Resume a stalled list transfer after acknowledgment
    /// Returns true if resumed successfully, false if no stalled transfer
    pub fn resume_list_transfer(&mut self, local_storage: &mut [u8], main_memory: &mut [u8]) -> bool {
        let state = match self.list_transfer.take() {
            Some(s) if s.stalled => s,
            other => {
                self.list_transfer = other;
                return false;
            }
        };

        // Parse remaining elements
        let elements = self.parse_list(state.list_addr, state.list_size, local_storage);
        
        for (idx, elem) in elements.iter().enumerate().skip(state.current_element) {
            if elem.size > 0 {
                let elem_ea = state.ea.wrapping_add(elem.lsa as u64);
                
                if state.cmd.is_get() {
                    self.perform_get_transfer(elem.lsa, elem_ea, elem.size as u32,
                                            local_storage, main_memory);
                } else {
                    self.perform_put_transfer(elem.lsa, elem_ea, elem.size as u32,
                                            local_storage, main_memory);
                }
            }
            
            // Check for another stall-and-notify flag
            if elem.stall_notify {
                self.set_list_stall(state.tag);
                self.list_transfer = Some(ListTransferState {
                    list_addr: state.list_addr,
                    list_size: state.list_size,
                    ea: state.ea,
                    tag: state.tag,
                    cmd: state.cmd,
                    current_element: idx + 1,
                    total_elements: state.total_elements,
                    stalled: true,
                });
                return true; // Resumed but stalled again
            }
        }
        
        // List completed
        self.complete_tag(state.tag);
        self.clear_list_stall();
        true
    }

    /// Check if there's a stalled list transfer
    pub fn has_stalled_list_transfer(&self) -> bool {
        self.list_transfer.as_ref().map_or(false, |s| s.stalled)
    }

    /// Get the stalled list transfer tag (for MFC_RD_LIST_STALL channel)
    pub fn get_stalled_list_tag(&self) -> Option<u8> {
        self.list_transfer.as_ref()
            .filter(|s| s.stalled)
            .map(|s| s.tag)
    }

    /// Parse DMA list from local storage
    fn parse_list(&self, list_addr: u32, list_size: u32, local_storage: &[u8]) -> Vec<MfcListElement> {
        let mut elements = Vec::new();
        let list_start = list_addr as usize;
        let num_elements = (list_size / 8).min(2048) as usize; // Max 2048 elements

        for i in 0..num_elements {
            let offset = list_start + i * 8;
            if offset + 8 > local_storage.len() {
                break;
            }

            let lsa = u32::from_be_bytes([
                local_storage[offset],
                local_storage[offset + 1],
                local_storage[offset + 2],
                local_storage[offset + 3],
            ]);
            let size = u16::from_be_bytes([
                local_storage[offset + 4],
                local_storage[offset + 5],
            ]);
            // Byte 6 bit 7 is the stall-and-notify flag
            let stall_notify = (local_storage[offset + 6] & 0x80) != 0;

            if size > 0 {
                elements.push(MfcListElement { lsa, size, stall_notify });
            }
        }

        elements
    }

    /// Set tag query mask for MFC_RD_TAG_STAT operations
    pub fn set_tag_mask(&mut self, mask: u32) {
        self.tag_query_mask = mask;
    }

    /// Get tag query mask
    pub fn get_tag_mask(&self) -> u32 {
        self.tag_query_mask
    }

    /// Check if any specified tags are complete
    pub fn check_tag_status_any(&self) -> bool {
        (self.tag_status & self.tag_query_mask) != 0
    }

    /// Check if all specified tags are complete
    pub fn check_tag_status_all(&self) -> bool {
        (self.tag_status & self.tag_query_mask) == self.tag_query_mask
    }

    /// Get list stall status
    pub fn get_list_stall(&self) -> bool {
        self.list_stall
    }

    /// Set list stall
    pub fn set_list_stall(&mut self, tag: u8) {
        self.list_stall = true;
        self.stall_notify_tag = tag;
    }

    /// Clear list stall
    pub fn clear_list_stall(&mut self) {
        self.list_stall = false;
    }

    /// Get stall notify tag
    pub fn get_stall_notify_tag(&self) -> u8 {
        self.stall_notify_tag
    }
}

impl Default for Mfc {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mfc_creation() {
        let mfc = Mfc::new();
        assert!(mfc.is_queue_empty());
        assert_eq!(mfc.get_tag_status(), 0xFFFFFFFF);
        assert_eq!(mfc.get_cycle_counter(), 0);
    }

    #[test]
    fn test_mfc_command_queue() {
        let mut mfc = Mfc::new();

        let cmd = MfcDmaCommand {
            lsa: 0x1000,
            ea: 0x20000000,
            size: 0x4000,
            tag: 0,
            cmd: MfcCommand::Get,
            issue_cycle: 0,
            completion_cycle: 0,
        };

        mfc.queue_command(cmd);
        assert!(!mfc.is_queue_empty());
        assert_eq!(mfc.get_tag_status() & 1, 0); // Tag 0 pending
        assert_eq!(mfc.get_pending_tags() & 1, 1); // Tag 0 in flight

        // Advance time to complete the DMA
        let latency = MfcCommand::Get.base_latency() + MfcCommand::Get.transfer_latency(0x4000);
        mfc.tick(latency);
        
        assert_eq!(mfc.get_tag_status() & 1, 1); // Tag 0 complete
        assert_eq!(mfc.get_pending_tags() & 1, 0); // Tag 0 no longer pending
    }

    #[test]
    fn test_mfc_timing() {
        let mut mfc = Mfc::new();

        // Queue a GET command
        let cmd = MfcDmaCommand {
            lsa: 0x1000,
            ea: 0x20000000,
            size: 256, // 2 blocks
            tag: 1,
            cmd: MfcCommand::Get,
            issue_cycle: 0,
            completion_cycle: 0,
        };

        mfc.queue_command(cmd);
        
        // Check that completion is in the future
        let cycles_remaining = mfc.cycles_until_tag_completion(1);
        assert!(cycles_remaining.is_some());
        assert!(cycles_remaining.unwrap() > 0);

        // Advance halfway
        mfc.tick(50);
        assert_eq!(mfc.get_tag_status() & 0b10, 0); // Still pending

        // Advance to completion
        mfc.tick(100);
        assert_eq!(mfc.get_tag_status() & 0b10, 0b10); // Now complete
    }

    #[test]
    fn test_mfc_reservation() {
        let mut mfc = Mfc::new();

        assert!(!mfc.has_reservation());

        let data = [0x42u8; 128];
        mfc.set_reservation(0x1000, &data);

        assert!(mfc.has_reservation());
        assert_eq!(mfc.get_reservation_addr(), 0x1000);
        assert_eq!(mfc.get_reservation_data()[0], 0x42);

        mfc.clear_reservation();
        assert!(!mfc.has_reservation());
    }

    #[test]
    fn test_command_latencies() {
        // Test that different commands have different latencies
        assert!(MfcCommand::Get.base_latency() > 0);
        assert!(MfcCommand::Put.base_latency() > 0);
        assert!(MfcCommand::GetB.base_latency() > MfcCommand::Get.base_latency());
        assert!(MfcCommand::Barrier.base_latency() > 0);
        
        // Test transfer latency scales with size
        let small_latency = MfcCommand::Get.transfer_latency(128);
        let large_latency = MfcCommand::Get.transfer_latency(1024);
        assert!(large_latency > small_latency);
    }

    #[test]
    fn test_dma_get_operation() {
        let mut mfc = Mfc::new();
        let mut local_storage = vec![0u8; 1024];
        let main_memory = vec![0x42u8; 1024];

        // Perform a small GET (should be immediate)
        let result = mfc.execute_get(0, 0, 128, 0, &mut local_storage, &main_memory);
        assert!(result); // Should complete immediately
        assert_eq!(local_storage[0], 0x42); // Data transferred
    }

    #[test]
    fn test_dma_put_operation() {
        let mut mfc = Mfc::new();
        let mut local_storage = vec![0x42u8; 1024];
        let mut main_memory = vec![0u8; 1024];

        // Perform a small PUT (should be immediate)
        let result = mfc.execute_put(0, 0, 128, 0, &local_storage, &mut main_memory);
        assert!(result); // Should complete immediately
        assert_eq!(main_memory[0], 0x42); // Data transferred
    }

    #[test]
    fn test_dma_list_get() {
        let mut mfc = Mfc::new();
        let mut local_storage = vec![0u8; 4096];
        let main_memory = vec![0x42u8; 4096];

        // Create a DMA list with 2 elements
        // Element 1: LSA=0x100, size=0x80
        local_storage[0..4].copy_from_slice(&0x100u32.to_be_bytes());
        local_storage[4..6].copy_from_slice(&0x80u16.to_be_bytes());
        // Element 2: LSA=0x200, size=0x80
        local_storage[8..12].copy_from_slice(&0x200u32.to_be_bytes());
        local_storage[12..14].copy_from_slice(&0x80u16.to_be_bytes());

        // Execute list GET
        let result = mfc.execute_list_get(0, 0, 16, 0, &mut local_storage, &main_memory);
        assert!(result);
        assert_eq!(local_storage[0x100], 0x42); // First element transferred
        assert_eq!(local_storage[0x200], 0x42); // Second element transferred
    }

    #[test]
    fn test_tag_management() {
        let mut mfc = Mfc::new();
        
        // Set tag mask
        mfc.set_tag_mask(0b11); // Tags 0 and 1
        
        // All tags initially complete
        assert!(mfc.check_tag_status_all());
        
        // Mark tag 0 as pending
        mfc.tag_status &= !1;
        mfc.pending_tags |= 1;
        
        // Should not be all complete now
        assert!(!mfc.check_tag_status_all());
        
        // But at least one is complete (tag 1)
        assert!(mfc.check_tag_status_any());
    }
}
