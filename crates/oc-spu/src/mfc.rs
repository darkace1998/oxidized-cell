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
    /// Get (main to local)
    Get = 0x40,
    /// Get with barrier
    GetB = 0x41,
    /// Get with fence
    GetF = 0x42,
    /// Get unconditional
    GetU = 0x48,
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
            Self::Put | Self::PutU => 80,
            Self::PutB | Self::PutF => 100,
            Self::GetLLAR => 150,
            Self::PutLLC | Self::PutLLUC => 120,
            Self::Barrier => 50,
            Self::Unknown => 0,
        }
    }

    /// Calculate transfer latency based on size (cycles per 128 bytes)
    pub fn transfer_latency(&self, size: u32) -> u64 {
        let blocks = (size + 127) / 128;
        blocks as u64 * 10 // 10 cycles per 128-byte block
    }
}

impl From<u8> for MfcCommand {
    fn from(value: u8) -> Self {
        match value {
            0x20 => Self::Put,
            0x21 => Self::PutB,
            0x22 => Self::PutF,
            0x28 => Self::PutU,
            0x40 => Self::Get,
            0x41 => Self::GetB,
            0x42 => Self::GetF,
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

/// MFC state
pub struct Mfc {
    /// Command queue
    queue: VecDeque<MfcDmaCommand>,
    /// Tag group completion status (bit per tag)
    tag_status: u32,
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
}

impl Mfc {
    /// Create a new MFC
    pub fn new() -> Self {
        Self {
            queue: VecDeque::with_capacity(16),
            tag_status: 0xFFFFFFFF, // All tags initially complete
            reservation_addr: 0,
            reservation_data: [0; 128],
            reservation_valid: false,
            cycle_counter: 0,
            pending_tags: 0,
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
}
