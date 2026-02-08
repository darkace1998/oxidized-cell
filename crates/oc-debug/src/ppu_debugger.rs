//! PPU debugger for instruction tracing and register inspection

use std::sync::Arc;
use oc_memory::MemoryManager;
use oc_ppu::thread::PpuThread;
use crate::breakpoint::BreakpointManager;
use crate::disassembler::PpuDisassembler;

/// Debug execution state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugState {
    /// Running normally
    Running,
    /// Paused (by user or breakpoint)
    Paused,
    /// Single stepping
    Stepping,
    /// Step over (step but skip function calls)
    SteppingOver,
    /// Step out (run until function returns)
    SteppingOut,
}

/// Watch expression for debugging
#[derive(Debug, Clone)]
pub struct WatchExpression {
    /// Unique ID for this watch
    pub id: u32,
    /// Human-readable name/description
    pub name: String,
    /// Watch type
    pub watch_type: WatchType,
    /// Current value (cached)
    pub current_value: u64,
    /// Previous value (for change detection)
    pub previous_value: u64,
    /// Has the value changed since last update
    pub changed: bool,
}

/// Type of watch expression
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchType {
    /// Watch a GPR register (index 0-31)
    Gpr(usize),
    /// Watch a memory address (address, size in bytes: 1, 2, 4, or 8)
    Memory(u32, u8),
    /// Watch the LR register
    Lr,
    /// Watch the CTR register
    Ctr,
    /// Watch the CR register
    Cr,
    /// Watch the XER register
    Xer,
}

/// Call stack entry
#[derive(Debug, Clone)]
pub struct CallStackEntry {
    /// Function address (estimated)
    pub function_addr: u64,
    /// Return address (from LR when called)
    pub return_addr: u64,
    /// Stack pointer at call time
    pub stack_ptr: u64,
    /// Optional function name/label
    pub name: Option<String>,
}

/// Instruction trace entry
#[derive(Debug, Clone)]
pub struct TraceEntry {
    /// Instruction address
    pub address: u64,
    /// Raw opcode
    pub opcode: u32,
    /// Disassembled instruction
    pub disasm: String,
    /// Cycle count when executed (if available)
    pub cycle: u64,
}

/// PPU debugger
pub struct PpuDebugger {
    /// Current debug state
    pub state: DebugState,
    /// Breakpoint manager
    pub breakpoints: BreakpointManager,
    /// Instruction tracing enabled
    pub tracing_enabled: bool,
    /// Instruction trace buffer
    trace_buffer: Vec<TraceEntry>,
    /// Maximum trace buffer size
    max_trace_entries: usize,
    /// Call stack
    call_stack: Vec<CallStackEntry>,
    /// Current cycle count
    cycle_count: u64,
    /// Step over return address (for step-over functionality)
    step_over_return_addr: Option<u64>,
    /// Step out target stack depth (for step-out functionality)
    step_out_target_depth: Option<usize>,
    /// Memory manager for memory inspection
    memory: Option<Arc<MemoryManager>>,
    /// Watch expressions
    watches: Vec<WatchExpression>,
    /// Next watch ID
    next_watch_id: u32,
    /// Function symbol table for call stack names (address -> name)
    symbol_table: std::collections::HashMap<u64, String>,
}

impl Default for PpuDebugger {
    fn default() -> Self {
        Self::new()
    }
}

impl PpuDebugger {
    /// Create a new PPU debugger
    pub fn new() -> Self {
        Self {
            state: DebugState::Running,
            breakpoints: BreakpointManager::new(),
            tracing_enabled: false,
            trace_buffer: Vec::new(),
            max_trace_entries: 10000,
            call_stack: Vec::new(),
            cycle_count: 0,
            step_over_return_addr: None,
            step_out_target_depth: None,
            memory: None,
            watches: Vec::new(),
            next_watch_id: 1,
            symbol_table: std::collections::HashMap::new(),
        }
    }

    /// Create a new PPU debugger with memory manager
    pub fn with_memory(memory: Arc<MemoryManager>) -> Self {
        Self {
            memory: Some(memory),
            ..Self::new()
        }
    }

    /// Set memory manager
    pub fn set_memory(&mut self, memory: Arc<MemoryManager>) {
        self.memory = Some(memory);
    }

    /// Pause execution
    pub fn pause(&mut self) {
        self.state = DebugState::Paused;
        tracing::info!("PPU debugger: paused");
    }

    /// Resume execution
    pub fn resume(&mut self) {
        self.state = DebugState::Running;
        self.step_over_return_addr = None;
        self.step_out_target_depth = None;
        tracing::info!("PPU debugger: resumed");
    }

    /// Single step one instruction
    pub fn step(&mut self) {
        self.state = DebugState::Stepping;
        tracing::debug!("PPU debugger: stepping");
    }

    /// Step over (execute but don't step into function calls)
    pub fn step_over(&mut self, current_pc: u64, _current_lr: u64) {
        self.state = DebugState::SteppingOver;
        // Set return address to current PC + 4 (after the current instruction)
        self.step_over_return_addr = Some(current_pc + 4);
        tracing::debug!("PPU debugger: step over, return at 0x{:016x}", current_pc + 4);
    }

    /// Step out (run until returning from current function)
    pub fn step_out(&mut self) {
        if self.call_stack.is_empty() {
            // No call stack, just resume
            tracing::warn!("PPU debugger: step out with empty call stack, resuming");
            self.resume();
            return;
        }
        self.state = DebugState::SteppingOut;
        // Target depth is one less than current (we want to return from current function)
        self.step_out_target_depth = Some(self.call_stack.len().saturating_sub(1));
        tracing::debug!("PPU debugger: step out, target depth {}", self.call_stack.len().saturating_sub(1));
    }

    /// Check if execution should stop before executing an instruction
    /// Returns true if a breakpoint was hit or we're stepping
    pub fn check_before_execute(&mut self, pc: u64) -> bool {
        match self.state {
            DebugState::Running => {
                // Check for breakpoints
                if self.breakpoints.check_execution(pc).is_some() {
                    tracing::info!("PPU debugger: breakpoint hit at 0x{:016x}", pc);
                    self.state = DebugState::Paused;
                    return true;
                }
                false
            }
            DebugState::Paused => true,
            DebugState::Stepping => {
                self.state = DebugState::Paused;
                true
            }
            DebugState::SteppingOver => {
                if let Some(return_addr) = self.step_over_return_addr {
                    if pc == return_addr {
                        self.state = DebugState::Paused;
                        self.step_over_return_addr = None;
                        return true;
                    }
                }
                // Also check breakpoints while stepping over
                if self.breakpoints.check_execution(pc).is_some() {
                    tracing::info!("PPU debugger: breakpoint hit at 0x{:016x}", pc);
                    self.state = DebugState::Paused;
                    return true;
                }
                false
            }
            DebugState::SteppingOut => {
                // Check if we've returned from the target function
                if let Some(target_depth) = self.step_out_target_depth {
                    if self.call_stack.len() <= target_depth {
                        self.state = DebugState::Paused;
                        self.step_out_target_depth = None;
                        tracing::info!("PPU debugger: step out complete at 0x{:016x}", pc);
                        return true;
                    }
                }
                // Also check breakpoints while stepping out
                if self.breakpoints.check_execution(pc).is_some() {
                    tracing::info!("PPU debugger: breakpoint hit at 0x{:016x}", pc);
                    self.state = DebugState::Paused;
                    self.step_out_target_depth = None;
                    return true;
                }
                false
            }
        }
    }

    /// Record instruction execution for tracing
    pub fn trace_instruction(&mut self, pc: u64, opcode: u32) {
        if !self.tracing_enabled {
            return;
        }

        let disasm = PpuDisassembler::disassemble(pc, opcode);
        let entry = TraceEntry {
            address: pc,
            opcode,
            disasm: disasm.to_string(),
            cycle: self.cycle_count,
        };

        self.trace_buffer.push(entry);

        // Limit buffer size
        if self.trace_buffer.len() > self.max_trace_entries {
            self.trace_buffer.remove(0);
        }

        self.cycle_count += 1;
    }

    /// Track function call (when bl instruction is executed)
    pub fn track_call(&mut self, from_addr: u64, to_addr: u64, lr: u64, sp: u64) {
        // Look up function name in symbol table
        let name = self.symbol_table.get(&to_addr).cloned();
        let entry = CallStackEntry {
            function_addr: to_addr,
            return_addr: lr,
            stack_ptr: sp,
            name,
        };
        self.call_stack.push(entry);
        tracing::trace!("PPU call: 0x{:016x} -> 0x{:016x}", from_addr, to_addr);
    }

    /// Track function return (when blr instruction is executed)
    pub fn track_return(&mut self, return_addr: u64) {
        // Pop entries until we find one with matching return address
        while let Some(entry) = self.call_stack.pop() {
            if entry.return_addr == return_addr {
                tracing::trace!("PPU return to 0x{:016x}", return_addr);
                break;
            }
        }
    }

    /// Get the current call stack
    pub fn get_call_stack(&self) -> &[CallStackEntry] {
        &self.call_stack
    }

    /// Get recent trace entries
    pub fn get_trace(&self, count: usize) -> &[TraceEntry] {
        let start = self.trace_buffer.len().saturating_sub(count);
        &self.trace_buffer[start..]
    }

    /// Clear trace buffer
    pub fn clear_trace(&mut self) {
        self.trace_buffer.clear();
    }

    /// Enable instruction tracing
    pub fn enable_tracing(&mut self) {
        self.tracing_enabled = true;
        tracing::info!("PPU instruction tracing enabled");
    }

    /// Disable instruction tracing
    pub fn disable_tracing(&mut self) {
        self.tracing_enabled = false;
        tracing::info!("PPU instruction tracing disabled");
    }

    /// Set maximum trace buffer size
    pub fn set_max_trace_entries(&mut self, max: usize) {
        self.max_trace_entries = max;
    }

    /// Get register snapshot from a PPU thread
    pub fn get_register_snapshot(&self, thread: &PpuThread) -> RegisterSnapshot {
        RegisterSnapshot {
            gpr: thread.regs.gpr,
            fpr: thread.regs.fpr,
            vr: thread.regs.vr,
            cr: thread.regs.cr,
            lr: thread.regs.lr,
            ctr: thread.regs.ctr,
            xer: thread.regs.xer,
            fpscr: thread.regs.fpscr,
            vscr: thread.regs.vscr,
            pc: thread.regs.cia,
        }
    }

    /// Read memory for inspection
    pub fn read_memory(&self, address: u32, size: usize) -> Option<Vec<u8>> {
        let memory = self.memory.as_ref()?;
        
        // Use read_bytes if available, otherwise read byte by byte
        if let Ok(bytes) = memory.read_bytes(address, size as u32) {
            return Some(bytes);
        }
        
        let mut buffer = vec![0u8; size];
        for (i, byte) in buffer.iter_mut().enumerate() {
            if let Ok(val) = memory.read::<u8>(address + i as u32) {
                *byte = val;
            }
        }
        
        Some(buffer)
    }

    /// Disassemble memory at address
    pub fn disassemble_at(&self, address: u64, count: usize) -> Vec<crate::disassembler::DisassembledInstruction> {
        if let Some(memory) = &self.memory {
            let mut result = Vec::with_capacity(count);
            for i in 0..count {
                let addr = address + (i as u64 * 4);
                if let Ok(opcode) = memory.read_be32(addr as u32) {
                    result.push(PpuDisassembler::disassemble(addr, opcode));
                }
            }
            result
        } else {
            Vec::new()
        }
    }

    /// Get debug state
    pub fn is_paused(&self) -> bool {
        self.state == DebugState::Paused
    }

    /// Get debug state
    pub fn is_running(&self) -> bool {
        self.state == DebugState::Running
    }

    // === Watch Expression Methods ===

    /// Add a watch expression
    pub fn add_watch(&mut self, name: &str, watch_type: WatchType) -> u32 {
        let id = self.next_watch_id;
        self.next_watch_id += 1;
        
        let watch = WatchExpression {
            id,
            name: name.to_string(),
            watch_type,
            current_value: 0,
            previous_value: 0,
            changed: false,
        };
        
        self.watches.push(watch);
        tracing::debug!("PPU debugger: added watch '{}' (id={})", name, id);
        id
    }

    /// Remove a watch expression by ID
    pub fn remove_watch(&mut self, id: u32) -> bool {
        if let Some(pos) = self.watches.iter().position(|w| w.id == id) {
            self.watches.remove(pos);
            tracing::debug!("PPU debugger: removed watch id={}", id);
            true
        } else {
            false
        }
    }

    /// Update all watch expressions with current values from thread
    pub fn update_watches(&mut self, thread: &PpuThread) {
        for watch in &mut self.watches {
            watch.previous_value = watch.current_value;
            
            let new_value = match watch.watch_type {
                WatchType::Gpr(idx) => {
                    if idx < 32 { thread.regs.gpr[idx] } else { 0 }
                }
                WatchType::Memory(addr, size) => {
                    if let Some(ref mem) = self.memory {
                        match size {
                            1 => mem.read::<u8>(addr).unwrap_or(0) as u64,
                            2 => mem.read::<u16>(addr).unwrap_or(0) as u64,
                            4 => mem.read::<u32>(addr).unwrap_or(0) as u64,
                            8 => mem.read::<u64>(addr).unwrap_or(0),
                            _ => 0,
                        }
                    } else {
                        0
                    }
                }
                WatchType::Lr => thread.regs.lr,
                WatchType::Ctr => thread.regs.ctr,
                WatchType::Cr => thread.regs.cr as u64,
                WatchType::Xer => thread.regs.xer,
            };
            
            watch.current_value = new_value;
            watch.changed = watch.current_value != watch.previous_value;
        }
    }

    /// Get all watch expressions
    pub fn get_watches(&self) -> &[WatchExpression] {
        &self.watches
    }

    /// Get watches that have changed since last update
    pub fn get_changed_watches(&self) -> Vec<&WatchExpression> {
        self.watches.iter().filter(|w| w.changed).collect()
    }

    /// Clear all watch expressions
    pub fn clear_watches(&mut self) {
        self.watches.clear();
        tracing::debug!("PPU debugger: cleared all watches");
    }

    // === Symbol Table Methods ===

    /// Add a symbol to the symbol table
    pub fn add_symbol(&mut self, address: u64, name: &str) {
        self.symbol_table.insert(address, name.to_string());
    }

    /// Look up a symbol name by address
    pub fn lookup_symbol(&self, address: u64) -> Option<&str> {
        self.symbol_table.get(&address).map(|s| s.as_str())
    }

    /// Load symbols from a map (address -> name)
    pub fn load_symbols(&mut self, symbols: std::collections::HashMap<u64, String>) {
        self.symbol_table.extend(symbols);
        tracing::info!("PPU debugger: loaded {} symbols", self.symbol_table.len());
    }

    /// Clear the symbol table
    pub fn clear_symbols(&mut self) {
        self.symbol_table.clear();
    }

    /// Get call stack with resolved function names
    pub fn get_call_stack_with_names(&self) -> Vec<CallStackEntry> {
        self.call_stack.iter().map(|entry| {
            let mut resolved = entry.clone();
            if resolved.name.is_none() {
                resolved.name = self.lookup_symbol(entry.function_addr).map(String::from);
            }
            resolved
        }).collect()
    }

    /// Get number of symbols loaded
    pub fn symbol_count(&self) -> usize {
        self.symbol_table.len()
    }
}

/// Snapshot of PPU registers for display
#[derive(Debug, Clone)]
pub struct RegisterSnapshot {
    /// General Purpose Registers
    pub gpr: [u64; 32],
    /// Floating Point Registers
    pub fpr: [f64; 32],
    /// Vector Registers
    pub vr: [[u32; 4]; 32],
    /// Condition Register
    pub cr: u32,
    /// Link Register
    pub lr: u64,
    /// Count Register
    pub ctr: u64,
    /// Fixed-Point Exception Register
    pub xer: u64,
    /// FP Status and Control Register
    pub fpscr: u64,
    /// Vector Status and Control Register
    pub vscr: u32,
    /// Program Counter
    pub pc: u64,
}

impl RegisterSnapshot {
    /// Format GPR as hex string
    pub fn gpr_hex(&self, index: usize) -> String {
        format!("0x{:016X}", self.gpr[index])
    }

    /// Format FPR as string
    pub fn fpr_str(&self, index: usize) -> String {
        format!("{:.6}", self.fpr[index])
    }

    /// Format VR as hex string
    pub fn vr_hex(&self, index: usize) -> String {
        let v = self.vr[index];
        format!("{:08X} {:08X} {:08X} {:08X}", v[0], v[1], v[2], v[3])
    }

    /// Format PC as hex string
    pub fn pc_hex(&self) -> String {
        format!("0x{:016X}", self.pc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debugger_creation() {
        let debugger = PpuDebugger::new();
        assert_eq!(debugger.state, DebugState::Running);
        assert!(!debugger.tracing_enabled);
    }

    #[test]
    fn test_pause_resume() {
        let mut debugger = PpuDebugger::new();
        
        debugger.pause();
        assert_eq!(debugger.state, DebugState::Paused);
        
        debugger.resume();
        assert_eq!(debugger.state, DebugState::Running);
    }

    #[test]
    fn test_stepping() {
        let mut debugger = PpuDebugger::new();
        
        debugger.step();
        assert_eq!(debugger.state, DebugState::Stepping);
        
        // After check, should be paused
        assert!(debugger.check_before_execute(0x10000));
        assert_eq!(debugger.state, DebugState::Paused);
    }

    #[test]
    fn test_breakpoint_hit() {
        let mut debugger = PpuDebugger::new();
        
        debugger.breakpoints.add_execution_breakpoint(0x10000);
        
        // Should not stop at other addresses
        assert!(!debugger.check_before_execute(0x10004));
        
        // Should stop at breakpoint
        assert!(debugger.check_before_execute(0x10000));
        assert_eq!(debugger.state, DebugState::Paused);
    }

    #[test]
    fn test_tracing() {
        let mut debugger = PpuDebugger::new();
        debugger.enable_tracing();
        
        debugger.trace_instruction(0x10000, 0x38600064); // li r3, 100
        debugger.trace_instruction(0x10004, 0x4E800020); // blr
        
        let trace = debugger.get_trace(10);
        assert_eq!(trace.len(), 2);
        assert_eq!(trace[0].address, 0x10000);
        assert_eq!(trace[1].address, 0x10004);
    }

    #[test]
    fn test_call_stack() {
        let mut debugger = PpuDebugger::new();
        
        // Simulate function call
        debugger.track_call(0x10000, 0x20000, 0x10004, 0x100000);
        assert_eq!(debugger.get_call_stack().len(), 1);
        
        // Simulate return
        debugger.track_return(0x10004);
        assert_eq!(debugger.get_call_stack().len(), 0);
    }
}
