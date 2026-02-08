//! SPU debugger for local storage viewing and register inspection

use oc_spu::thread::{SpuThread, SPU_LS_SIZE};
use crate::breakpoint::BreakpointManager;
use crate::disassembler::SpuDisassembler;

/// SPU debug state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpuDebugState {
    /// Running normally
    Running,
    /// Paused (by user or breakpoint)
    Paused,
    /// Single stepping
    Stepping,
}

/// SPU channel state for debugging
#[derive(Debug, Clone)]
pub struct ChannelDebugInfo {
    /// Channel name
    pub name: String,
    /// Channel number
    pub channel: u32,
    /// Current value (if readable)
    pub value: Option<u32>,
    /// Count of items in channel
    pub count: u32,
    /// Is channel stalled (waiting)
    pub stalled: bool,
}

/// MFC command debug info
#[derive(Debug, Clone)]
pub struct MfcCommandDebugInfo {
    /// Command type (GET/PUT/etc)
    pub cmd: u32,
    /// Local storage address
    pub lsa: u32,
    /// Effective address (main memory)
    pub ea: u64,
    /// Transfer size
    pub size: u32,
    /// Tag
    pub tag: u32,
    /// Status
    pub status: String,
}

/// DMA queue visualization
#[derive(Debug, Clone)]
pub struct DmaQueueVisualization {
    /// Pending DMA commands
    pub pending_commands: Vec<MfcCommandDebugInfo>,
    /// Completed DMA tags (bit mask)
    pub completed_tags: u32,
    /// Tag mask for waiting
    pub tag_mask: u32,
    /// Total DMA transfers since reset
    pub total_transfers: u64,
    /// Total bytes transferred
    pub total_bytes: u64,
}

/// Local storage region info for visualization
#[derive(Debug, Clone)]
pub struct LocalStorageRegion {
    /// Region start address
    pub start: u32,
    /// Region end address (exclusive)
    pub end: u32,
    /// Region name/description
    pub name: String,
    /// Region type
    pub region_type: LocalStorageRegionType,
}

/// Type of local storage region
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalStorageRegionType {
    /// Code section
    Code,
    /// Data section
    Data,
    /// Stack
    Stack,
    /// DMA buffer
    DmaBuffer,
    /// Unknown/Other
    Unknown,
}

/// Local storage hexdump line
#[derive(Debug, Clone)]
pub struct HexdumpLine {
    /// Address
    pub address: u32,
    /// Hex bytes (up to 16)
    pub hex: String,
    /// ASCII representation
    pub ascii: String,
}

/// SPU debugger
pub struct SpuDebugger {
    /// Debug state per SPU (indexed by SPU ID)
    pub states: [SpuDebugState; 6],
    /// Breakpoint managers per SPU
    pub breakpoints: [BreakpointManager; 6],
    /// Tracing enabled per SPU
    pub tracing_enabled: [bool; 6],
    /// Trace buffers per SPU
    trace_buffers: [Vec<SpuTraceEntry>; 6],
    /// Max trace entries
    max_trace_entries: usize,
}

/// SPU trace entry
#[derive(Debug, Clone)]
pub struct SpuTraceEntry {
    /// Instruction address in local storage
    pub address: u32,
    /// Raw opcode
    pub opcode: u32,
    /// Disassembled instruction
    pub disasm: String,
    /// Cycle when executed
    pub cycle: u64,
}

impl Default for SpuDebugger {
    fn default() -> Self {
        Self::new()
    }
}

impl SpuDebugger {
    /// Create a new SPU debugger
    pub fn new() -> Self {
        Self {
            states: [SpuDebugState::Running; 6],
            breakpoints: [
                BreakpointManager::new(),
                BreakpointManager::new(),
                BreakpointManager::new(),
                BreakpointManager::new(),
                BreakpointManager::new(),
                BreakpointManager::new(),
            ],
            tracing_enabled: [false; 6],
            trace_buffers: [
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ],
            max_trace_entries: 10000,
        }
    }

    /// Pause an SPU
    pub fn pause(&mut self, spu_id: usize) {
        if spu_id < 6 {
            self.states[spu_id] = SpuDebugState::Paused;
            tracing::info!("SPU {} debugger: paused", spu_id);
        }
    }

    /// Resume an SPU
    pub fn resume(&mut self, spu_id: usize) {
        if spu_id < 6 {
            self.states[spu_id] = SpuDebugState::Running;
            tracing::info!("SPU {} debugger: resumed", spu_id);
        }
    }

    /// Single step an SPU
    pub fn step(&mut self, spu_id: usize) {
        if spu_id < 6 {
            self.states[spu_id] = SpuDebugState::Stepping;
            tracing::debug!("SPU {} debugger: stepping", spu_id);
        }
    }

    /// Check if execution should stop before executing an instruction
    pub fn check_before_execute(&mut self, spu_id: usize, pc: u32) -> bool {
        if spu_id >= 6 {
            return false;
        }

        match self.states[spu_id] {
            SpuDebugState::Running => {
                // Check for breakpoints
                if self.breakpoints[spu_id].check_execution(pc as u64).is_some() {
                    tracing::info!("SPU {} debugger: breakpoint hit at 0x{:08x}", spu_id, pc);
                    self.states[spu_id] = SpuDebugState::Paused;
                    return true;
                }
                false
            }
            SpuDebugState::Paused => true,
            SpuDebugState::Stepping => {
                self.states[spu_id] = SpuDebugState::Paused;
                true
            }
        }
    }

    /// Record instruction execution for tracing
    pub fn trace_instruction(&mut self, spu_id: usize, pc: u32, opcode: u32, cycle: u64) {
        if spu_id >= 6 || !self.tracing_enabled[spu_id] {
            return;
        }

        let disasm = SpuDisassembler::disassemble(pc, opcode);
        let entry = SpuTraceEntry {
            address: pc,
            opcode,
            disasm: disasm.to_string(),
            cycle,
        };

        self.trace_buffers[spu_id].push(entry);

        // Limit buffer size
        if self.trace_buffers[spu_id].len() > self.max_trace_entries {
            self.trace_buffers[spu_id].remove(0);
        }
    }

    /// Enable tracing for an SPU
    pub fn enable_tracing(&mut self, spu_id: usize) {
        if spu_id < 6 {
            self.tracing_enabled[spu_id] = true;
            tracing::info!("SPU {} instruction tracing enabled", spu_id);
        }
    }

    /// Disable tracing for an SPU
    pub fn disable_tracing(&mut self, spu_id: usize) {
        if spu_id < 6 {
            self.tracing_enabled[spu_id] = false;
            tracing::info!("SPU {} instruction tracing disabled", spu_id);
        }
    }

    /// Get trace entries for an SPU
    pub fn get_trace(&self, spu_id: usize, count: usize) -> &[SpuTraceEntry] {
        if spu_id >= 6 {
            return &[];
        }
        let buffer = &self.trace_buffers[spu_id];
        let start = buffer.len().saturating_sub(count);
        &buffer[start..]
    }

    /// Clear trace buffer for an SPU
    pub fn clear_trace(&mut self, spu_id: usize) {
        if spu_id < 6 {
            self.trace_buffers[spu_id].clear();
        }
    }

    /// Get register snapshot from an SPU thread
    pub fn get_register_snapshot(&self, thread: &SpuThread) -> SpuRegisterSnapshot {
        SpuRegisterSnapshot {
            gpr: thread.regs.gpr,
            pc: thread.regs.pc,
            spu_id: thread.id,
        }
    }

    /// Get local storage view
    pub fn get_local_storage_view(&self, thread: &SpuThread, offset: u32, size: usize) -> Vec<u8> {
        let offset = (offset as usize) & (SPU_LS_SIZE - 1);
        let end = (offset + size).min(SPU_LS_SIZE);
        thread.local_storage[offset..end].to_vec()
    }

    /// Get channel debug info for an SPU
    pub fn get_channel_info(&self, thread: &SpuThread) -> Vec<ChannelDebugInfo> {
        let mut info = Vec::new();
        
        // SPU Read channels
        info.push(ChannelDebugInfo {
            name: "SPU_RdEventStat".to_string(),
            channel: 0,
            value: Some(thread.channels.get_event_status()),
            count: 1,
            stalled: false,
        });
        
        info.push(ChannelDebugInfo {
            name: "SPU_RdEventMask".to_string(),
            channel: 1,
            value: Some(thread.channels.get_event_mask()),
            count: 1,
            stalled: false,
        });
        
        info.push(ChannelDebugInfo {
            name: "SPU_RdSigNotify1".to_string(),
            channel: 3,
            value: None, // Would need to peek without consuming
            count: if thread.channels.has_signal1() { 1 } else { 0 },
            stalled: false,
        });
        
        info.push(ChannelDebugInfo {
            name: "SPU_RdSigNotify2".to_string(),
            channel: 4,
            value: None, // Would need to peek without consuming
            count: if thread.channels.has_signal2() { 1 } else { 0 },
            stalled: false,
        });
        
        info.push(ChannelDebugInfo {
            name: "SPU_RdDec".to_string(),
            channel: 8,
            value: Some(thread.channels.get_count(8)), // Use count as a proxy
            count: 1,
            stalled: false,
        });
        
        // MFC channels
        info.push(ChannelDebugInfo {
            name: "MFC_WrTagMask".to_string(),
            channel: 22,
            value: Some(thread.mfc.get_tag_mask()),
            count: 1,
            stalled: false,
        });
        
        info.push(ChannelDebugInfo {
            name: "MFC_RdTagStat".to_string(),
            channel: 24,
            value: Some(thread.mfc.get_tag_status()),
            count: 1,
            stalled: false,
        });
        
        info
    }

    /// Get MFC command queue info
    pub fn get_mfc_queue(&self, _thread: &SpuThread) -> Vec<MfcCommandDebugInfo> {
        // The MFC queue is private, so we can't iterate directly
        // Return empty for now - this would require adding a public API to Mfc
        Vec::new()
    }

    /// Disassemble SPU local storage at address
    pub fn disassemble_at(&self, thread: &SpuThread, address: u32, count: usize) -> Vec<crate::disassembler::DisassembledInstruction> {
        let mut result = Vec::with_capacity(count);
        
        for i in 0..count {
            let addr = (address + (i as u32 * 4)) & (SPU_LS_SIZE as u32 - 1);
            let opcode = thread.ls_read_u32(addr);
            result.push(SpuDisassembler::disassemble(addr, opcode));
        }
        
        result
    }

    /// Check if SPU is paused
    pub fn is_paused(&self, spu_id: usize) -> bool {
        spu_id < 6 && self.states[spu_id] == SpuDebugState::Paused
    }

    /// Check if SPU is running
    pub fn is_running(&self, spu_id: usize) -> bool {
        spu_id < 6 && self.states[spu_id] == SpuDebugState::Running
    }

    /// Get DMA queue visualization
    pub fn get_dma_visualization(&self, thread: &SpuThread) -> DmaQueueVisualization {
        DmaQueueVisualization {
            pending_commands: self.get_mfc_queue(thread),
            completed_tags: thread.mfc.get_tag_status(),
            tag_mask: thread.mfc.get_tag_mask(),
            total_transfers: 0, // Would need MFC stats
            total_bytes: 0,
        }
    }

    /// Get hexdump of local storage region
    pub fn get_hexdump(&self, thread: &SpuThread, start: u32, lines: usize) -> Vec<HexdumpLine> {
        let mut result = Vec::with_capacity(lines);
        let bytes_per_line = 16;
        
        for line in 0..lines {
            let addr = (start + (line as u32 * bytes_per_line)) & (SPU_LS_SIZE as u32 - 1);
            let mut hex_parts = Vec::new();
            let mut ascii_parts = Vec::new();
            
            for i in 0..bytes_per_line {
                let byte_addr = (addr + i) & (SPU_LS_SIZE as u32 - 1);
                let byte = thread.local_storage[byte_addr as usize];
                hex_parts.push(format!("{:02X}", byte));
                
                // ASCII representation (printable chars only)
                let ch = if byte >= 0x20 && byte < 0x7F {
                    byte as char
                } else {
                    '.'
                };
                ascii_parts.push(ch);
            }
            
            result.push(HexdumpLine {
                address: addr,
                hex: hex_parts.join(" "),
                ascii: ascii_parts.into_iter().collect(),
            });
        }
        
        result
    }

    /// Get extended channel info (all 32 SPU channels)
    pub fn get_extended_channel_info(&self, thread: &SpuThread) -> Vec<ChannelDebugInfo> {
        let mut info = self.get_channel_info(thread);
        
        // Add more channels
        let additional_channels = [
            (2, "SPU_WrEventMask"),
            (5, "SPU_RdMachStat"),
            (7, "SPU_WrSRR0"),
            (11, "SPU_RdSRR0"),
            (13, "SPU_RdEventAck"),
            (14, "SPU_WrEventAck"),
            (15, "SPU_RdInMbox"),
            (28, "SPU_WrOutMbox"),
            (29, "SPU_RdOutMbox"),
            (30, "SPU_WrOutIntrMbox"),
        ];
        
        for (channel, name) in additional_channels {
            info.push(ChannelDebugInfo {
                name: name.to_string(),
                channel,
                value: None,
                count: thread.channels.get_count(channel),
                stalled: false,
            });
        }
        
        // Sort by channel number
        info.sort_by_key(|c| c.channel);
        info
    }

    /// Search local storage for a byte pattern
    pub fn search_local_storage(&self, thread: &SpuThread, pattern: &[u8]) -> Vec<u32> {
        let mut results = Vec::new();
        if pattern.is_empty() {
            return results;
        }
        
        for addr in 0..(SPU_LS_SIZE - pattern.len()) {
            let mut found = true;
            for (i, &byte) in pattern.iter().enumerate() {
                if thread.local_storage[addr + i] != byte {
                    found = false;
                    break;
                }
            }
            if found {
                results.push(addr as u32);
            }
        }
        
        results
    }
}

/// Snapshot of SPU registers for display
#[derive(Debug, Clone)]
pub struct SpuRegisterSnapshot {
    /// 128 x 128-bit registers (as 4 x u32)
    pub gpr: [[u32; 4]; 128],
    /// Program Counter
    pub pc: u32,
    /// SPU ID
    pub spu_id: u32,
}

impl SpuRegisterSnapshot {
    /// Format register as hex string
    pub fn reg_hex(&self, index: usize) -> String {
        let r = self.gpr[index];
        format!("{:08X} {:08X} {:08X} {:08X}", r[0], r[1], r[2], r[3])
    }

    /// Format register preferred slot (word 0) as hex
    pub fn reg_preferred_hex(&self, index: usize) -> String {
        format!("0x{:08X}", self.gpr[index][0])
    }

    /// Format PC as hex string
    pub fn pc_hex(&self) -> String {
        format!("0x{:08X}", self.pc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spu_debugger_creation() {
        let debugger = SpuDebugger::new();
        for i in 0..6 {
            assert_eq!(debugger.states[i], SpuDebugState::Running);
            assert!(!debugger.tracing_enabled[i]);
        }
    }

    #[test]
    fn test_spu_pause_resume() {
        let mut debugger = SpuDebugger::new();
        
        debugger.pause(0);
        assert_eq!(debugger.states[0], SpuDebugState::Paused);
        
        debugger.resume(0);
        assert_eq!(debugger.states[0], SpuDebugState::Running);
    }

    #[test]
    fn test_spu_stepping() {
        let mut debugger = SpuDebugger::new();
        
        debugger.step(0);
        assert_eq!(debugger.states[0], SpuDebugState::Stepping);
        
        // After check, should be paused
        assert!(debugger.check_before_execute(0, 0x100));
        assert_eq!(debugger.states[0], SpuDebugState::Paused);
    }

    #[test]
    fn test_spu_breakpoint() {
        let mut debugger = SpuDebugger::new();
        
        debugger.breakpoints[0].add_execution_breakpoint(0x100);
        
        // Should not stop at other addresses
        assert!(!debugger.check_before_execute(0, 0x104));
        
        // Should stop at breakpoint
        assert!(debugger.check_before_execute(0, 0x100));
        assert_eq!(debugger.states[0], SpuDebugState::Paused);
    }

    #[test]
    fn test_spu_tracing() {
        let mut debugger = SpuDebugger::new();
        debugger.enable_tracing(0);
        
        debugger.trace_instruction(0, 0x100, 0x40200000, 0); // nop
        debugger.trace_instruction(0, 0x104, 0x40200000, 1); // nop
        
        let trace = debugger.get_trace(0, 10);
        assert_eq!(trace.len(), 2);
    }
}
