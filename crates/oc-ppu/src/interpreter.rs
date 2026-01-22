//! PPU interpreter implementation
//!
//! This module implements the PPU instruction interpreter, dispatching decoded
//! instructions to the appropriate handlers in the instruction modules.
//!
//! The interpreter supports three execution modes:
//! - **Interpreter**: Pure interpretation of every instruction
//! - **JIT**: Just-In-Time compilation for all code (requires C++ backend)
//! - **Hybrid**: Uses JIT for hot code paths, falls back to interpreter for cold code

use std::sync::Arc;
use std::collections::HashSet;
use parking_lot::{RwLock, Mutex};
use oc_memory::MemoryManager;
use oc_core::error::PpuError;
use oc_hle::dispatch_hle_call;
use oc_ffi::jit::{PpuJitCompiler, BranchHint, LazyState, PpuContext, PpuExitReason};
use crate::decoder::{PpuDecoder, InstructionForm};
use crate::thread::PpuThread;
use crate::instructions::{float, system, vector};

/// JIT execution mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JitMode {
    /// Pure interpreter mode - no JIT compilation
    #[default]
    Interpreter,
    /// Pure JIT mode - all code is compiled before execution
    Jit,
    /// Hybrid mode - JIT for hot paths, interpreter for cold code
    Hybrid,
}

/// JIT statistics for performance monitoring
#[derive(Debug, Clone, Default)]
pub struct JitStats {
    /// Number of blocks compiled
    pub blocks_compiled: u64,
    /// Number of JIT executions
    pub jit_executions: u64,
    /// Number of interpreter fallbacks
    pub interpreter_fallbacks: u64,
    /// Total instructions executed via JIT
    pub jit_instructions: u64,
    /// Cache hits
    pub cache_hits: u64,
    /// Cache misses
    pub cache_misses: u64,
}

/// Breakpoint type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakpointType {
    /// Unconditional breakpoint - always breaks
    Unconditional,
    /// Conditional breakpoint - breaks when condition is met
    Conditional(BreakpointCondition),
}

/// Breakpoint condition
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakpointCondition {
    /// Break when GPR equals value
    GprEquals { reg: usize, value: u64 },
    /// Break when instruction count reaches value
    InstructionCount { count: u64 },
}

/// Breakpoint information
#[derive(Debug, Clone)]
pub struct Breakpoint {
    /// Address of the breakpoint
    pub addr: u64,
    /// Type of breakpoint
    pub bp_type: BreakpointType,
    /// Whether the breakpoint is enabled
    pub enabled: bool,
    /// Hit count
    pub hit_count: u64,
}

/// PPU interpreter for instruction execution
pub struct PpuInterpreter {
    /// Memory manager
    memory: Arc<MemoryManager>,
    /// Breakpoints (address -> breakpoint)
    breakpoints: RwLock<HashSet<u64>>,
    /// Breakpoint details
    breakpoint_details: RwLock<std::collections::HashMap<u64, Breakpoint>>,
    /// Total instruction count (for conditional breakpoints)
    instruction_count: Mutex<u64>,
    /// JIT execution mode
    jit_mode: RwLock<JitMode>,
    /// JIT compiler (optional, created on demand)
    jit_compiler: Mutex<Option<PpuJitCompiler>>,
    /// JIT statistics
    jit_stats: Mutex<JitStats>,
    /// Hot block threshold for hybrid mode (execution count before compilation)
    hot_threshold: u32,
    /// Block execution counts for hybrid mode
    block_exec_counts: RwLock<std::collections::HashMap<u32, u32>>,
}

impl PpuInterpreter {
    /// Create a new PPU interpreter
    pub fn new(memory: Arc<MemoryManager>) -> Self {
        Self {
            memory,
            breakpoints: RwLock::new(HashSet::new()),
            breakpoint_details: RwLock::new(std::collections::HashMap::new()),
            instruction_count: Mutex::new(0),
            jit_mode: RwLock::new(JitMode::Interpreter),
            jit_compiler: Mutex::new(None),
            jit_stats: Mutex::new(JitStats::default()),
            hot_threshold: 100, // Compile after 100 executions
            block_exec_counts: RwLock::new(std::collections::HashMap::new()),
        }
    }

    // ========================================================================
    // JIT Control Methods
    // ========================================================================

    /// Set the JIT execution mode
    pub fn set_jit_mode(&self, mode: JitMode) {
        *self.jit_mode.write() = mode;
        tracing::info!("PPU JIT mode set to {:?}", mode);
        
        // Initialize JIT compiler if needed
        if mode != JitMode::Interpreter {
            self.ensure_jit_compiler();
        }
    }

    /// Get the current JIT execution mode
    pub fn jit_mode(&self) -> JitMode {
        *self.jit_mode.read()
    }

    /// Enable JIT compilation (shorthand for Hybrid mode)
    pub fn enable_jit(&self) {
        self.set_jit_mode(JitMode::Hybrid);
    }

    /// Disable JIT compilation (pure interpreter mode)
    pub fn disable_jit(&self) {
        self.set_jit_mode(JitMode::Interpreter);
    }

    /// Check if JIT is available (C++ backend compiled)
    pub fn is_jit_available(&self) -> bool {
        self.ensure_jit_compiler();
        self.jit_compiler.lock().is_some()
    }

    /// Ensure JIT compiler is initialized
    fn ensure_jit_compiler(&self) {
        let mut jit = self.jit_compiler.lock();
        if jit.is_none() {
            *jit = PpuJitCompiler::new();
            if jit.is_some() {
                tracing::info!("PPU JIT compiler initialized");
            } else {
                tracing::warn!("PPU JIT compiler not available (C++ backend not linked)");
            }
        }
    }

    /// Get JIT statistics
    pub fn jit_stats(&self) -> JitStats {
        self.jit_stats.lock().clone()
    }

    /// Reset JIT statistics
    pub fn reset_jit_stats(&self) {
        *self.jit_stats.lock() = JitStats::default();
    }

    /// Set the hot block threshold for hybrid mode
    pub fn set_hot_threshold(&self, threshold: u32) {
        // Note: Can't use self here for mutable field, but this is a const field
        // For now, log the intended change
        tracing::info!("Hot threshold would be set to {} (currently fixed at construction)", threshold);
    }

    /// Compile a specific block address
    pub fn compile_block(&self, address: u32) -> Result<(), String> {
        let mut jit = self.jit_compiler.lock();
        let jit = jit.as_mut().ok_or("JIT compiler not available")?;

        // Read the code block from memory
        let code = self.read_block_code(address)?;
        
        jit.compile(address, &code)
            .map_err(|e| format!("JIT compilation failed: {:?}", e))?;

        self.jit_stats.lock().blocks_compiled += 1;
        tracing::debug!("Compiled block at 0x{:08x} ({} bytes)", address, code.len());
        
        Ok(())
    }

    /// Read a basic block of code from memory for compilation
    fn read_block_code(&self, start_address: u32) -> Result<Vec<u8>, String> {
        let mut code = Vec::with_capacity(256);
        let mut address = start_address;
        
        // Read instructions until we hit a branch or limit
        for _ in 0..64 { // Max 64 instructions per block
            let opcode = self.memory.read_be32(address)
                .map_err(|e| format!("Failed to read instruction at 0x{:08x}: {:?}", address, e))?;
            
            // Add instruction bytes to code
            code.extend_from_slice(&opcode.to_be_bytes());
            address += 4;
            
            // Check if this is a block-ending instruction
            if Self::is_block_end(opcode) {
                break;
            }
        }
        
        Ok(code)
    }

    /// Check if an instruction ends a basic block
    fn is_block_end(opcode: u32) -> bool {
        let primary = (opcode >> 26) & 0x3F;
        
        match primary {
            // Branch instructions
            16 => true, // bc (conditional branch)
            18 => true, // b (unconditional branch)
            19 => {
                // XL-form: check extended opcode for bclr, bcctr, etc.
                let xo = (opcode >> 1) & 0x3FF;
                matches!(xo, 16 | 528) // bclr, bcctr
            }
            17 => true, // sc (system call)
            _ => false,
        }
    }

    // ========================================================================
    // JIT Execution Bridge
    // ========================================================================

    /// Create a PpuContext from thread state for JIT execution
    fn thread_to_context(&self, thread: &PpuThread) -> PpuContext {
        let regs = &thread.regs;
        
        PpuContext {
            gpr: regs.gpr,
            fpr: regs.fpr,
            vr: regs.vr,
            cr: regs.cr,
            lr: regs.lr,
            ctr: regs.ctr,
            xer: regs.xer,
            fpscr: regs.fpscr,
            vscr: regs.vscr,
            pc: regs.cia,
            msr: regs.msr,
            next_pc: regs.cia + 4,
            instructions_executed: 0,
            exit_reason: 0,
            memory_base: self.memory.base_ptr(),
            memory_size: self.memory.address_space_size(),
        }
    }

    /// Copy context state back to thread after JIT execution
    fn context_to_thread(&self, context: &PpuContext, thread: &mut PpuThread) {
        let regs = &mut thread.regs;
        
        regs.gpr = context.gpr;
        regs.fpr = context.fpr;
        regs.vr = context.vr;
        regs.cr = context.cr;
        regs.lr = context.lr;
        regs.ctr = context.ctr;
        regs.xer = context.xer;
        regs.fpscr = context.fpscr;
        regs.vscr = context.vscr;
        regs.cia = context.next_pc;
        regs.msr = context.msr;
    }

    /// Try to execute code via JIT, returns true if successful
    fn try_jit_execute(&self, thread: &mut PpuThread) -> Result<bool, PpuError> {
        let mode = *self.jit_mode.read();
        
        if mode == JitMode::Interpreter {
            return Ok(false);
        }

        let pc = thread.pc() as u32;
        let mut jit = self.jit_compiler.lock();
        
        let jit = match jit.as_mut() {
            Some(j) => j,
            None => return Ok(false),
        };

        // Check for compiled code
        if jit.get_compiled(pc).is_some() {
            // JIT code is available - execute it!
            self.jit_stats.lock().cache_hits += 1;
            
            // Create execution context from thread state
            let mut context = self.thread_to_context(thread);
            
            // Execute the JIT-compiled code
            match jit.execute(&mut context, pc) {
                Ok(instructions_executed) => {
                    // Copy state back to thread
                    self.context_to_thread(&context, thread);
                    
                    // Update statistics
                    {
                        let mut stats = self.jit_stats.lock();
                        stats.jit_executions += 1;
                        stats.jit_instructions += instructions_executed as u64;
                    }
                    
                    tracing::trace!(
                        "JIT executed {} instructions at 0x{:08x}, next PC: 0x{:08x}",
                        instructions_executed,
                        pc,
                        context.next_pc
                    );
                    
                    return Ok(true);
                }
                Err(exit_reason) => {
                    // Copy state back even on error (partial execution)
                    self.context_to_thread(&context, thread);
                    
                    match exit_reason {
                        PpuExitReason::Syscall => {
                            // Syscall encountered - let interpreter handle it
                            tracing::trace!("JIT hit syscall at 0x{:08x}", pc);
                            return Ok(false);
                        }
                        PpuExitReason::Breakpoint => {
                            // Breakpoint hit
                            tracing::debug!("JIT hit breakpoint at 0x{:08x}", pc);
                            return Err(PpuError::Breakpoint { addr: context.pc });
                        }
                        PpuExitReason::Error => {
                            // JIT execution error - fall back to interpreter
                            tracing::warn!("JIT execution error at 0x{:08x}", pc);
                            self.jit_stats.lock().interpreter_fallbacks += 1;
                            return Ok(false);
                        }
                        _ => {
                            // Normal or branch - should have been Ok
                            return Ok(true);
                        }
                    }
                }
            }
        }

        self.jit_stats.lock().cache_misses += 1;

        // In hybrid mode, check if block is hot
        if mode == JitMode::Hybrid {
            let should_compile = {
                let mut counts = self.block_exec_counts.write();
                let count = counts.entry(pc).or_insert(0);
                *count += 1;
                *count >= self.hot_threshold
            };

            if should_compile {
                // Check lazy compilation state
                match jit.get_lazy_state(pc) {
                    LazyState::NotCompiled => {
                        // Register for lazy compilation
                        if let Ok(code) = self.read_block_code(pc) {
                            jit.register_lazy(pc, &code, self.hot_threshold);
                            tracing::debug!("Registered block 0x{:08x} for lazy compilation", pc);
                        }
                    }
                    LazyState::Pending | LazyState::Compiling => {
                        // Compilation in progress
                    }
                    LazyState::Compiled => {
                        // Should have been caught above
                    }
                    LazyState::Failed => {
                        // Don't retry failed compilations
                    }
                }
            }
        }

        Ok(false) // Fall back to interpreter
    }

    /// Add branch prediction hint from interpreter observation
    #[allow(dead_code)] // Will be used when branch recording is enabled
    fn record_branch(&self, address: u32, target: u32, taken: bool) {
        let mut jit = self.jit_compiler.lock();
        if let Some(jit) = jit.as_mut() {
            jit.update_branch(address, taken);
            
            // Add hint if this is a new branch
            let hint = if taken { BranchHint::Likely } else { BranchHint::Unlikely };
            jit.add_branch_hint(address, target, hint);
        }
    }

    // ========================================================================
    // End JIT Control Methods
    // ========================================================================

    /// Add a breakpoint at the specified address
    pub fn add_breakpoint(&self, addr: u64, bp_type: BreakpointType) {
        self.breakpoints.write().insert(addr);
        self.breakpoint_details.write().insert(
            addr,
            Breakpoint {
                addr,
                bp_type,
                enabled: true,
                hit_count: 0,
            },
        );
    }

    /// Remove a breakpoint at the specified address
    pub fn remove_breakpoint(&self, addr: u64) {
        self.breakpoints.write().remove(&addr);
        self.breakpoint_details.write().remove(&addr);
    }

    /// Enable a breakpoint
    pub fn enable_breakpoint(&self, addr: u64) {
        if let Some(bp) = self.breakpoint_details.write().get_mut(&addr) {
            bp.enabled = true;
        }
    }

    /// Disable a breakpoint
    pub fn disable_breakpoint(&self, addr: u64) {
        if let Some(bp) = self.breakpoint_details.write().get_mut(&addr) {
            bp.enabled = false;
        }
    }

    /// Clear all breakpoints
    pub fn clear_breakpoints(&self) {
        self.breakpoints.write().clear();
        self.breakpoint_details.write().clear();
    }

    /// Get all breakpoints
    pub fn get_breakpoints(&self) -> Vec<Breakpoint> {
        self.breakpoint_details
            .read()
            .values()
            .cloned()
            .collect()
    }

    /// Check if we should break at this address
    #[inline]
    fn should_break(&self, thread: &PpuThread) -> bool {
        let pc = thread.pc();
        
        // Fast path: check if there's a breakpoint at this address
        if !self.breakpoints.read().contains(&pc) {
            return false;
        }

        // Check breakpoint condition
        let details = self.breakpoint_details.read();
        if let Some(bp) = details.get(&pc) {
            if !bp.enabled {
                return false;
            }

            match bp.bp_type {
                BreakpointType::Unconditional => true,
                BreakpointType::Conditional(condition) => match condition {
                    BreakpointCondition::GprEquals { reg, value } => {
                        thread.gpr(reg) == value
                    }
                    BreakpointCondition::InstructionCount { count } => {
                        *self.instruction_count.lock() >= count
                    }
                },
            }
        } else {
            false
        }
    }

    /// Execute a single instruction
    pub fn step(&self, thread: &mut PpuThread) -> Result<(), PpuError> {
        // Check for breakpoints before execution
        if self.should_break(thread) {
            // Update hit count
            let pc = thread.pc();
            if let Some(bp) = self.breakpoint_details.write().get_mut(&pc) {
                bp.hit_count += 1;
            }
            return Err(PpuError::Breakpoint { addr: pc });
        }

        // Try JIT execution first (if enabled and available)
        if self.try_jit_execute(thread)? {
            // JIT execution succeeded - stats already updated
            self.jit_stats.lock().jit_executions += 1;
            return Ok(());
        }
        
        // Fall back to interpreter
        self.jit_stats.lock().interpreter_fallbacks += 1;

        // Increment instruction count for conditional breakpoints
        let inst_count = {
            let mut count = self.instruction_count.lock();
            *count += 1;
            *count
        };

        // Fetch instruction
        let pc = thread.pc() as u32;
        
        // Check for thread exit condition (PC at 0 typically means the main function returned)
        // This happens when blr is executed with LR=0
        if pc == 0 {
            tracing::info!("PPU thread exiting after {} instructions: returned to address 0 (normal exit)", inst_count);
            // Return value is in R3 per PPC64 ABI
            let exit_code = thread.gpr(3);
            return Err(PpuError::ThreadExit { exit_code });
        }
        
        // Check if this is an HLE stub address (our generated stubs)
        const STUB_REGION_BASE: u32 = 0x2F00_0000;
        const STUB_REGION_END: u32 = 0x3000_0000;
        if pc >= STUB_REGION_BASE && pc < STUB_REGION_END {
            return self.execute_hle_stub(thread, pc);
        }
        
        // NOTE: We used to have PLT stub detection here that would intercept
        // function calls in the 0x800000-0x1000000 range if they started with
        // certain patterns (li r12, 0). This was removed because it incorrectly
        // intercepted legitimate game code functions that happened to start with
        // the same instruction patterns. The proper HLE import stubs at 0x2F000000
        // are handled above and are the correct mechanism for import handling.
        
        let opcode = self.memory.read_be32(pc).map_err(|_| PpuError::InvalidInstruction {
            addr: pc,
            opcode: 0,
        })?;

        // Decode instruction
        let decoded = PpuDecoder::decode(opcode);
        
        // Trace instruction execution (log first 50 instructions at INFO level for debugging)
        if inst_count <= 50 {
            let mnemonic = PpuDecoder::get_mnemonic(opcode);
            tracing::info!(
                "[{}] 0x{:08x}: {:08x} {} (form={:?})",
                inst_count, pc, opcode, mnemonic, decoded.form
            );
        }

        // Execute instruction
        self.execute(thread, opcode, decoded)?;

        Ok(())
    }

    /// Get the current instruction count
    pub fn instruction_count(&self) -> u64 {
        *self.instruction_count.lock()
    }

    /// Reset the instruction count
    pub fn reset_instruction_count(&self) {
        *self.instruction_count.lock() = 0;
    }

    /// Execute a decoded instruction
    #[inline]
    fn execute(&self, thread: &mut PpuThread, opcode: u32, decoded: crate::decoder::DecodedInstruction) -> Result<(), PpuError> {
        match decoded.form {
            InstructionForm::D => self.execute_d_form(thread, opcode, decoded.op),
            InstructionForm::DS => self.execute_ds_form(thread, opcode, decoded.op),
            InstructionForm::I => self.execute_i_form(thread, opcode),
            InstructionForm::B => self.execute_b_form(thread, opcode),
            InstructionForm::X => self.execute_x_form(thread, opcode, decoded.xo),
            InstructionForm::XO => self.execute_xo_form(thread, opcode, decoded.xo),
            InstructionForm::XL => self.execute_xl_form(thread, opcode, decoded.xo),
            InstructionForm::XS => self.execute_xs_form(thread, opcode),
            InstructionForm::M => self.execute_m_form(thread, opcode, decoded.op),
            InstructionForm::MD => self.execute_md_form(thread, opcode),
            InstructionForm::MDS => self.execute_mds_form(thread, opcode),
            InstructionForm::A => self.execute_a_form(thread, opcode, decoded.xo),
            InstructionForm::VA => self.execute_va_form(thread, opcode),
            InstructionForm::SC => self.execute_sc(thread, opcode),
            _ => {
                // Decode the raw opcode for better diagnostics
                let primary_op = (opcode >> 26) & 0x3F;
                let mnemonic = PpuDecoder::get_mnemonic(opcode);
                tracing::warn!(
                    "Unimplemented instruction form: {:?} at 0x{:08x} (opcode: 0x{:08x}, primary_op: {}, mnemonic: '{}')",
                    decoded.form, thread.pc(), opcode, primary_op, mnemonic
                );
                tracing::debug!(
                    "Instruction bytes at 0x{:08x}: [{:02x} {:02x} {:02x} {:02x}]",
                    thread.pc(),
                    (opcode >> 24) & 0xFF,
                    (opcode >> 16) & 0xFF,
                    (opcode >> 8) & 0xFF,
                    opcode & 0xFF
                );
                // Return error instead of silently continuing for Unknown form
                if decoded.form == InstructionForm::Unknown {
                    return Err(PpuError::InvalidInstruction {
                        addr: thread.pc() as u32,
                        opcode,
                    });
                }
                thread.advance_pc();
                Ok(())
            }
        }
    }

    /// Handle entry into a PLT stub area
    /// 
    /// When execution enters the PLT stub area (typically 0x80XXXX - 0x90XXXX),
    /// we intercept it immediately instead of letting the trampoline execute.
    /// This allows us to handle imports that weren't patched at load time.
    /// 
    /// NOTE: Currently unused because the PLT stub detection was causing false
    /// positives with legitimate game code. Kept for potential future use.
    #[allow(dead_code)]
    fn handle_plt_stub_entry(&self, thread: &mut PpuThread, plt_addr: u32) -> Result<(), PpuError> {
        // The PLT stub loads a descriptor address and calls it
        // We can parse the PLT stub to find the descriptor address, or just
        // use the LR (return address) to return cleanly
        
        tracing::debug!(
            "Intercepted PLT stub at 0x{:08x}, LR=0x{:x}, R3=0x{:x}",
            plt_addr,
            thread.regs.lr,
            thread.gpr(3)
        );
        
        // For now, just return success and go back to caller
        // In the future, we can parse the PLT stub to identify which import
        // and dispatch to the appropriate HLE handler
        thread.set_gpr(3, 0);  // Return 0 (success/CELL_OK)
        
        let return_addr = (thread.regs.lr & !3) as u32;
        if return_addr == 0 {
            tracing::info!("PLT stub has no return address, thread exiting");
            return Err(PpuError::ThreadExit { exit_code: 0 });
        }
        thread.set_pc(return_addr as u64);
        
        Ok(())
    }

    /// Handle an unresolved import call (bctr to unpatched descriptor)
    /// 
    /// This handles cases where the import descriptor wasn't patched (e.g., 
    /// the descriptor had a non-zero address pointing to stub code).
    fn handle_unresolved_import(&self, thread: &mut PpuThread) -> Result<(), PpuError> {
        let r12 = thread.gpr(12);
        
        tracing::warn!(
            "Unresolved import call at PC=0x{:08x}: returning stub value 0",
            thread.pc()
        );
        tracing::warn!(
            "  LR=0x{:x}, R2(TOC)=0x{:x}, R3(arg0)=0x{:x}, R12(descriptor)=0x{:x}",
            thread.regs.lr,
            thread.gpr(2),
            thread.gpr(3),
            r12
        );
        
        // Stub behavior: set R3 to 0 (success) and return to caller
        thread.set_gpr(3, 0);  // Return 0 (success/CELL_OK)
        
        // Return to caller (jump to LR)
        let return_addr = thread.regs.lr & !3;
        if return_addr == 0 {
            // No valid return address - thread exit
            tracing::info!("No return address for stub call, thread exiting");
            return Err(PpuError::ThreadExit { exit_code: 0 });
        }
        thread.set_pc(return_addr);
        
        Ok(())
    }

    /// Execute an HLE stub function
    /// 
    /// When execution reaches an address in the HLE stub region, this function
    /// is called to dispatch to the appropriate HLE handler.
    /// 
    /// HLE stubs use a function descriptor format:
    ///   [stub_addr + 0]: code_addr (points to stub + 8)
    ///   [stub_addr + 4]: toc (0 for HLE stubs)
    ///   [stub_addr + 8]: li r3, 0
    ///   [stub_addr + 12]: blr
    /// 
    /// When called via PLT, the PC will be at stub_addr + 8 (the code_addr).
    /// We need to convert back to the descriptor address for dispatch lookup.
    fn execute_hle_stub(&self, thread: &mut PpuThread, code_addr: u32) -> Result<(), PpuError> {
        // Convert code address to descriptor address
        // HLE stubs are 16 bytes aligned, with code at offset 8
        // If code_addr is aligned to 16 + 8, then descriptor is at code_addr - 8
        const STUB_SIZE: u32 = 16;
        let offset_in_stub = code_addr % STUB_SIZE;
        let stub_addr = if offset_in_stub == 8 {
            // PC is at the code section (offset 8 in stub)
            code_addr - 8
        } else if offset_in_stub == 0 {
            // PC is at the descriptor (old format or direct call)
            code_addr
        } else {
            // Unexpected offset, use as-is
            code_addr
        };
        
        // Gather arguments from registers (R3-R10 per PPC64 ABI)
        let args = [
            thread.gpr(3),
            thread.gpr(4),
            thread.gpr(5),
            thread.gpr(6),
            thread.gpr(7),
            thread.gpr(8),
            thread.gpr(9),
            thread.gpr(10),
        ];
        let toc = thread.gpr(2);
        let lr = thread.regs.lr;
        
        // Try to dispatch through the HLE system
        if let Some(result) = dispatch_hle_call(stub_addr, &args, toc, lr) {
            // Set return value in R3
            thread.set_gpr(3, result as u64);
            
            // Return to caller (branch to LR)
            let return_addr = (thread.regs.lr & !3) as u32;
            if return_addr == 0 {
                // No valid return address - thread exit
                tracing::info!("HLE function returned but no valid LR, thread exiting");
                return Err(PpuError::ThreadExit { exit_code: result as u64 });
            }
            thread.set_pc(return_addr as u64);
        } else {
            // Unknown HLE stub - this is a dynamically created stub for an import
            // that wasn't matched to a known HLE function. Return 0 (CELL_OK) as
            // a safe default. This is expected behavior for unimplemented imports.
            tracing::debug!(
                "Unimplemented HLE import at stub 0x{:08x} (LR=0x{:x}, R3=0x{:x}), returning CELL_OK",
                stub_addr, lr, args[0]
            );
            thread.set_gpr(3, 0);
            
            let return_addr = (thread.regs.lr & !3) as u32;
            if return_addr == 0 {
                return Err(PpuError::ThreadExit { exit_code: 0 });
            }
            thread.set_pc(return_addr as u64);
        }
        
        Ok(())
    }

    /// Execute D-form instructions (most common form - optimized hot path)
    #[inline]
    fn execute_d_form(&self, thread: &mut PpuThread, opcode: u32, op: u8) -> Result<(), PpuError> {
        let (rt, ra, d) = PpuDecoder::d_form(opcode);
        let d = d as i64;

        match op {
            // addi - Add Immediate
            14 => {
                let value = if ra == 0 {
                    d as u64
                } else {
                    (thread.gpr(ra as usize) as i64).wrapping_add(d) as u64
                };
                thread.set_gpr(rt as usize, value);
            }
            // addis - Add Immediate Shifted
            15 => {
                let value = if ra == 0 {
                    (d << 16) as u64
                } else {
                    (thread.gpr(ra as usize) as i64).wrapping_add(d << 16) as u64
                };
                thread.set_gpr(rt as usize, value);
            }
            // lwz - Load Word and Zero
            32 => {
                let ea = if ra == 0 { d as u64 } else { thread.gpr(ra as usize).wrapping_add(d as u64) };
                let value = self.memory.read_be32(ea as u32).map_err(|e| PpuError::MemoryError {
                    addr: ea as u32,
                    message: format!("lwz failed at PC=0x{:08x}: {} (r{}={:#x}, d={})", 
                        thread.pc(), e, ra, thread.gpr(ra as usize), d as i16),
                })?;
                thread.set_gpr(rt as usize, value as u64);
            }
            // stw - Store Word
            36 => {
                let ea = if ra == 0 { d as u64 } else { thread.gpr(ra as usize).wrapping_add(d as u64) };
                let value = thread.gpr(rt as usize) as u32;
                self.memory.write_be32(ea as u32, value).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
            }
            // lbz - Load Byte and Zero
            34 => {
                let ea = if ra == 0 { d as u64 } else { thread.gpr(ra as usize).wrapping_add(d as u64) };
                let value: u8 = self.memory.read(ea as u32).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_gpr(rt as usize, value as u64);
            }
            // stb - Store Byte
            38 => {
                let ea = if ra == 0 { d as u64 } else { thread.gpr(ra as usize).wrapping_add(d as u64) };
                let value = thread.gpr(rt as usize) as u8;
                self.memory.write(ea as u32, value).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
            }
            // ori - OR Immediate
            24 => {
                let value = thread.gpr(rt as usize) | (d as u64 & 0xFFFF);
                thread.set_gpr(ra as usize, value);
            }
            // oris - OR Immediate Shifted
            25 => {
                let value = thread.gpr(rt as usize) | ((d as u64 & 0xFFFF) << 16);
                thread.set_gpr(ra as usize, value);
            }
            // andi. - AND Immediate
            28 => {
                let value = thread.gpr(rt as usize) & (d as u64 & 0xFFFF);
                thread.set_gpr(ra as usize, value);
                self.update_cr0(thread, value);
            }
            // cmpi - Compare Immediate (signed)
            11 => {
                let bf = (rt >> 2) & 7;
                let l = (rt & 1) != 0;
                let a = if l { thread.gpr(ra as usize) as i64 } else { thread.gpr(ra as usize) as i32 as i64 };
                let b = if l { d } else { d as i32 as i64 };
                let c = if a < b { 0b1000 } else if a > b { 0b0100 } else { 0b0010 };
                let c = c | if thread.get_xer_so() { 1 } else { 0 };
                thread.set_cr_field(bf as usize, c);
            }
            // cmpli - Compare Logical Immediate (unsigned)
            10 => {
                let bf = (rt >> 2) & 7;
                let l = (rt & 1) != 0;
                let a = if l { thread.gpr(ra as usize) } else { thread.gpr(ra as usize) as u32 as u64 };
                let b = if l { d as u64 & 0xFFFF } else { (d as u64 & 0xFFFF) as u32 as u64 };
                let c = if a < b { 0b1000 } else if a > b { 0b0100 } else { 0b0010 };
                let c = c | if thread.get_xer_so() { 1 } else { 0 };
                thread.set_cr_field(bf as usize, c);
            }
            // lwzu - Load Word and Zero with Update
            33 => {
                let ea = thread.gpr(ra as usize).wrapping_add(d as u64);
                let value = self.memory.read_be32(ea as u32).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_gpr(rt as usize, value as u64);
                thread.set_gpr(ra as usize, ea);
            }
            // lbzu - Load Byte and Zero with Update
            35 => {
                let ea = thread.gpr(ra as usize).wrapping_add(d as u64);
                let value: u8 = self.memory.read(ea as u32).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_gpr(rt as usize, value as u64);
                thread.set_gpr(ra as usize, ea);
            }
            // stwu - Store Word with Update
            37 => {
                let ea = thread.gpr(ra as usize).wrapping_add(d as u64);
                let value = thread.gpr(rt as usize) as u32;
                self.memory.write_be32(ea as u32, value).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_gpr(ra as usize, ea);
            }
            // stbu - Store Byte with Update
            39 => {
                let ea = thread.gpr(ra as usize).wrapping_add(d as u64);
                let value = thread.gpr(rt as usize) as u8;
                self.memory.write(ea as u32, value).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_gpr(ra as usize, ea);
            }
            // lhz - Load Halfword and Zero
            40 => {
                let ea = if ra == 0 { d as u64 } else { thread.gpr(ra as usize).wrapping_add(d as u64) };
                let value = self.memory.read_be16(ea as u32).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_gpr(rt as usize, value as u64);
            }
            // lhzu - Load Halfword and Zero with Update
            41 => {
                let ea = thread.gpr(ra as usize).wrapping_add(d as u64);
                let value = self.memory.read_be16(ea as u32).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_gpr(rt as usize, value as u64);
                thread.set_gpr(ra as usize, ea);
            }
            // lha - Load Halfword Algebraic
            42 => {
                let ea = if ra == 0 { d as u64 } else { thread.gpr(ra as usize).wrapping_add(d as u64) };
                let value = self.memory.read_be16(ea as u32).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_gpr(rt as usize, (value as i16) as i64 as u64);
            }
            // lhau - Load Halfword Algebraic with Update
            43 => {
                let ea = thread.gpr(ra as usize).wrapping_add(d as u64);
                let value = self.memory.read_be16(ea as u32).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_gpr(rt as usize, (value as i16) as i64 as u64);
                thread.set_gpr(ra as usize, ea);
            }
            // sth - Store Halfword
            44 => {
                let ea = if ra == 0 { d as u64 } else { thread.gpr(ra as usize).wrapping_add(d as u64) };
                let value = thread.gpr(rt as usize) as u16;
                self.memory.write_be16(ea as u32, value).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
            }
            // sthu - Store Halfword with Update
            45 => {
                let ea = thread.gpr(ra as usize).wrapping_add(d as u64);
                let value = thread.gpr(rt as usize) as u16;
                self.memory.write_be16(ea as u32, value).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_gpr(ra as usize, ea);
            }
            // lmw - Load Multiple Word
            46 => {
                let mut ea = if ra == 0 { d as u64 } else { thread.gpr(ra as usize).wrapping_add(d as u64) };
                for r in rt..32 {
                    let value = self.memory.read_be32(ea as u32).map_err(|_| PpuError::InvalidInstruction {
                        addr: thread.pc() as u32,
                        opcode,
                    })?;
                    thread.set_gpr(r as usize, value as u64);
                    ea = ea.wrapping_add(4);
                }
            }
            // stmw - Store Multiple Word
            47 => {
                let mut ea = if ra == 0 { d as u64 } else { thread.gpr(ra as usize).wrapping_add(d as u64) };
                for r in rt..32 {
                    let value = thread.gpr(r as usize) as u32;
                    self.memory.write_be32(ea as u32, value).map_err(|e| {
                        tracing::error!("stmw: memory write failed at EA=0x{:08x}, r{}=0x{:08x}, RA(r{})=0x{:016x}, D={}: {:?}",
                            ea, r, value, ra, thread.gpr(ra as usize), d as i16, e);
                        PpuError::MemoryError { addr: ea as u32, message: format!("stmw write failed: {:?}", e) }
                    })?;
                    ea = ea.wrapping_add(4);
                }
            }
            // lfs - Load Floating-Point Single
            48 => {
                let ea = if ra == 0 { d as u64 } else { thread.gpr(ra as usize).wrapping_add(d as u64) };
                let bits = self.memory.read_be32(ea as u32).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_fpr(rt as usize, f32::from_bits(bits) as f64);
            }
            // lfsu - Load Floating-Point Single with Update
            49 => {
                let ea = thread.gpr(ra as usize).wrapping_add(d as u64);
                let bits = self.memory.read_be32(ea as u32).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_fpr(rt as usize, f32::from_bits(bits) as f64);
                thread.set_gpr(ra as usize, ea);
            }
            // lfd - Load Floating-Point Double
            50 => {
                let ea = if ra == 0 { d as u64 } else { thread.gpr(ra as usize).wrapping_add(d as u64) };
                let bits = self.memory.read_be64(ea as u32).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_fpr(rt as usize, f64::from_bits(bits));
            }
            // lfdu - Load Floating-Point Double with Update
            51 => {
                let ea = thread.gpr(ra as usize).wrapping_add(d as u64);
                let bits = self.memory.read_be64(ea as u32).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_fpr(rt as usize, f64::from_bits(bits));
                thread.set_gpr(ra as usize, ea);
            }
            // stfs - Store Floating-Point Single
            52 => {
                let ea = if ra == 0 { d as u64 } else { thread.gpr(ra as usize).wrapping_add(d as u64) };
                let bits = (thread.fpr(rt as usize) as f32).to_bits();
                self.memory.write_be32(ea as u32, bits).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
            }
            // stfsu - Store Floating-Point Single with Update
            53 => {
                let ea = thread.gpr(ra as usize).wrapping_add(d as u64);
                let bits = (thread.fpr(rt as usize) as f32).to_bits();
                self.memory.write_be32(ea as u32, bits).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_gpr(ra as usize, ea);
            }
            // stfd - Store Floating-Point Double
            54 => {
                let ea = if ra == 0 { d as u64 } else { thread.gpr(ra as usize).wrapping_add(d as u64) };
                let bits = thread.fpr(rt as usize).to_bits();
                self.memory.write_be64(ea as u32, bits).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
            }
            // stfdu - Store Floating-Point Double with Update
            55 => {
                let ea = thread.gpr(ra as usize).wrapping_add(d as u64);
                let bits = thread.fpr(rt as usize).to_bits();
                self.memory.write_be64(ea as u32, bits).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_gpr(ra as usize, ea);
            }
            // xori - XOR Immediate
            26 => {
                let value = thread.gpr(rt as usize) ^ (d as u64 & 0xFFFF);
                thread.set_gpr(ra as usize, value);
            }
            // xoris - XOR Immediate Shifted
            27 => {
                let value = thread.gpr(rt as usize) ^ ((d as u64 & 0xFFFF) << 16);
                thread.set_gpr(ra as usize, value);
            }
            // andis. - AND Immediate Shifted
            29 => {
                let value = thread.gpr(rt as usize) & ((d as u64 & 0xFFFF) << 16);
                thread.set_gpr(ra as usize, value);
                self.update_cr0(thread, value);
            }
            // subfic - Subtract From Immediate Carrying
            8 => {
                let a = thread.gpr(ra as usize);
                let result = (d as u64).wrapping_sub(a);
                thread.set_gpr(rt as usize, result);
                thread.set_xer_ca((d as u64) >= a);
            }
            // mulli - Multiply Low Immediate
            7 => {
                let a = thread.gpr(ra as usize) as i64;
                let result = a.wrapping_mul(d) as u64;
                thread.set_gpr(rt as usize, result);
            }
            // addic - Add Immediate Carrying
            12 => {
                let a = thread.gpr(ra as usize);
                let (result, carry) = a.overflowing_add(d as u64);
                thread.set_gpr(rt as usize, result);
                thread.set_xer_ca(carry);
            }
            // addic. - Add Immediate Carrying and Record
            13 => {
                let a = thread.gpr(ra as usize);
                let (result, carry) = a.overflowing_add(d as u64);
                thread.set_gpr(rt as usize, result);
                thread.set_xer_ca(carry);
                self.update_cr0(thread, result);
            }
            _ => {
                tracing::warn!(
                    "Unimplemented D-form op {} at 0x{:08x} (opcode: 0x{:08x}, rt={}, ra={}, d={})",
                    op, thread.pc(), opcode, rt, ra, d
                );
                return Err(PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                });
            }
        }

        thread.advance_pc();
        Ok(())
    }

    /// Execute I-form instructions (branches)
    #[inline]
    fn execute_i_form(&self, thread: &mut PpuThread, opcode: u32) -> Result<(), PpuError> {
        let (li, aa, lk) = PpuDecoder::i_form(opcode);

        if lk {
            thread.regs.lr = thread.pc() + 4;
        }

        let target = if aa {
            li as u64
        } else {
            (thread.pc() as i64 + li as i64) as u64
        };

        thread.set_pc(target);
        Ok(())
    }

    /// Execute B-form instructions (conditional branches)
    #[inline]
    fn execute_b_form(&self, thread: &mut PpuThread, opcode: u32) -> Result<(), PpuError> {
        let (bo, bi, bd, aa, lk) = PpuDecoder::b_form(opcode);

        let ctr_ok = if (bo & 0x04) != 0 {
            true
        } else {
            thread.regs.ctr = thread.regs.ctr.wrapping_sub(1);
            ((thread.regs.ctr != 0) as u8) ^ ((bo >> 1) & 1) != 0
        };

        let cond_ok = if (bo & 0x10) != 0 {
            true
        } else {
            let cr_bit = (thread.regs.cr >> (31 - bi)) & 1;
            (cr_bit as u8) == ((bo >> 3) & 1)
        };

        if ctr_ok && cond_ok {
            if lk {
                thread.regs.lr = thread.pc() + 4;
            }

            let target = if aa {
                bd as u64
            } else {
                (thread.pc() as i64 + bd as i64) as u64
            };

            thread.set_pc(target);
        } else {
            thread.advance_pc();
        }

        Ok(())
    }

    /// Execute X-form instructions
    #[inline]
    fn execute_x_form(&self, thread: &mut PpuThread, opcode: u32, xo: u16) -> Result<(), PpuError> {
        let (rt, ra, rb, _, rc) = PpuDecoder::x_form(opcode);
        let primary = (opcode >> 26) & 0x3F;

        match (primary, xo) {
            // Integer cmp (primary 31, xo 0) vs FP fcmpu (primary 63, xo 0)
            (31, 0) => {
                // cmp - Integer compare (signed)
                let bf = (rt >> 2) & 7;
                let l = (rt & 1) != 0;
                let a = if l { thread.gpr(ra as usize) as i64 } else { thread.gpr(ra as usize) as i32 as i64 };
                let b = if l { thread.gpr(rb as usize) as i64 } else { thread.gpr(rb as usize) as i32 as i64 };
                let c = if a < b { 0b1000 } else if a > b { 0b0100 } else { 0b0010 };
                let c = c | if thread.get_xer_so() { 1 } else { 0 };
                thread.set_cr_field(bf as usize, c);
            }
            (63, 0) => {
                // fcmpu - Floating-point compare unordered
                let bf = (rt >> 2) & 7;
                let fa = thread.fpr(ra as usize);
                let fb = thread.fpr(rb as usize);
                let result = float::compare_f64(fa, fb);
                let c = match result {
                    float::FpCompareResult::Less => 0b1000,
                    float::FpCompareResult::Greater => 0b0100,
                    float::FpCompareResult::Equal => 0b0010,
                    float::FpCompareResult::Unordered => 0b0001,
                };
                thread.set_cr_field(bf as usize, c);
            }
            // Integer cmpl (primary 31, xo 32) vs FP fcmpo (primary 63, xo 32)
            (31, 32) => {
                // cmpl - Integer compare (unsigned)
                let bf = (rt >> 2) & 7;
                let l = (rt & 1) != 0;
                let a = if l { thread.gpr(ra as usize) } else { thread.gpr(ra as usize) as u32 as u64 };
                let b = if l { thread.gpr(rb as usize) } else { thread.gpr(rb as usize) as u32 as u64 };
                let c = if a < b { 0b1000 } else if a > b { 0b0100 } else { 0b0010 };
                let c = c | if thread.get_xer_so() { 1 } else { 0 };
                thread.set_cr_field(bf as usize, c);
            }
            (63, 32) => {
                // fcmpo - Floating-point compare ordered
                let bf = (rt >> 2) & 7;
                let fa = thread.fpr(ra as usize);
                let fb = thread.fpr(rb as usize);
                let result = float::compare_f64(fa, fb);
                let c = match result {
                    float::FpCompareResult::Less => 0b1000,
                    float::FpCompareResult::Greater => 0b0100,
                    float::FpCompareResult::Equal => 0b0010,
                    float::FpCompareResult::Unordered => 0b0001,
                };
                thread.set_cr_field(bf as usize, c);
                // fcmpo may raise exceptions on unordered (SNaN), but we'll skip that for now
            }
            // All other instructions dispatch based on xo only
            (_, xo) => match xo {
            // and - AND
            28 => {
                let value = thread.gpr(rt as usize) & thread.gpr(rb as usize);
                thread.set_gpr(ra as usize, value);
                if rc { self.update_cr0(thread, value); }
            }
            // or - OR
            444 => {
                let value = thread.gpr(rt as usize) | thread.gpr(rb as usize);
                thread.set_gpr(ra as usize, value);
                if rc { self.update_cr0(thread, value); }
            }
            // xor - XOR
            316 => {
                let value = thread.gpr(rt as usize) ^ thread.gpr(rb as usize);
                thread.set_gpr(ra as usize, value);
                if rc { self.update_cr0(thread, value); }
            }
            // nor - NOR
            124 => {
                let value = !(thread.gpr(rt as usize) | thread.gpr(rb as usize));
                thread.set_gpr(ra as usize, value);
                if rc { self.update_cr0(thread, value); }
            }
            // lwzx - Load Word and Zero Indexed
            23 => {
                let ea = if ra == 0 { thread.gpr(rb as usize) } else { thread.gpr(ra as usize).wrapping_add(thread.gpr(rb as usize)) };
                let value = self.memory.read_be32(ea as u32).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_gpr(rt as usize, value as u64);
            }
            // stwx - Store Word Indexed
            151 => {
                let ea = if ra == 0 { thread.gpr(rb as usize) } else { thread.gpr(ra as usize).wrapping_add(thread.gpr(rb as usize)) };
                let value = thread.gpr(rt as usize) as u32;
                self.memory.write_be32(ea as u32, value).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
            }
            // mfspr - Move From Special Purpose Register
            339 => {
                let spr = ((rb as u16) << 5) | (ra as u16);
                let value = match spr {
                    1 => thread.regs.xer,     // XER
                    8 => thread.regs.lr,      // LR
                    9 => thread.regs.ctr,     // CTR
                    _ => {
                        tracing::warn!("Unimplemented mfspr SPR {} at 0x{:08x}", spr, thread.pc());
                        0
                    }
                };
                thread.set_gpr(rt as usize, value);
            }
            // mtspr - Move To Special Purpose Register
            467 => {
                let spr = ((rb as u16) << 5) | (ra as u16);
                let value = thread.gpr(rt as usize);
                match spr {
                    1 => thread.regs.xer = value,    // XER
                    8 => thread.regs.lr = value,     // LR
                    9 => thread.regs.ctr = value,    // CTR
                    _ => {
                        tracing::warn!("Unimplemented mtspr SPR {} at 0x{:08x}", spr, thread.pc());
                    }
                }
            }
            // andc - AND with Complement
            60 => {
                let value = thread.gpr(rt as usize) & !thread.gpr(rb as usize);
                thread.set_gpr(ra as usize, value);
                if rc { self.update_cr0(thread, value); }
            }
            // orc - OR with Complement
            412 => {
                let value = thread.gpr(rt as usize) | !thread.gpr(rb as usize);
                thread.set_gpr(ra as usize, value);
                if rc { self.update_cr0(thread, value); }
            }
            // nand - NAND
            476 => {
                let value = !(thread.gpr(rt as usize) & thread.gpr(rb as usize));
                thread.set_gpr(ra as usize, value);
                if rc { self.update_cr0(thread, value); }
            }
            // eqv - Equivalent (XNOR)
            284 => {
                let value = !(thread.gpr(rt as usize) ^ thread.gpr(rb as usize));
                thread.set_gpr(ra as usize, value);
                if rc { self.update_cr0(thread, value); }
            }
            // slw - Shift Left Word
            24 => {
                let n = thread.gpr(rb as usize) & 0x3F;
                let value = if n > 31 {
                    0
                } else {
                    (thread.gpr(rt as usize) as u32).wrapping_shl(n as u32) as u64
                };
                thread.set_gpr(ra as usize, value);
                if rc { self.update_cr0(thread, value); }
            }
            // srw - Shift Right Word
            536 => {
                let n = thread.gpr(rb as usize) & 0x3F;
                let value = if n > 31 {
                    0
                } else {
                    (thread.gpr(rt as usize) as u32).wrapping_shr(n as u32) as u64
                };
                thread.set_gpr(ra as usize, value);
                if rc { self.update_cr0(thread, value); }
            }
            // sraw - Shift Right Algebraic Word
            792 => {
                let n = thread.gpr(rb as usize) & 0x3F;
                let (value, ca) = if n > 31 {
                    let s = (thread.gpr(rt as usize) as i32) >> 31;
                    (s as u64, s != 0)
                } else {
                    let s = (thread.gpr(rt as usize) as i32) >> (n as u32);
                    let ca = s < 0 && (thread.gpr(rt as usize) as u32 & ((1u32 << n) - 1)) != 0;
                    (s as u64, ca)
                };
                thread.set_gpr(ra as usize, value);
                thread.set_xer_ca(ca);
                if rc { self.update_cr0(thread, value); }
            }
            // sld - Shift Left Doubleword
            27 => {
                let n = thread.gpr(rb as usize) & 0x7F;
                let value = if n > 63 {
                    0
                } else {
                    thread.gpr(rt as usize).wrapping_shl(n as u32)
                };
                thread.set_gpr(ra as usize, value);
                if rc { self.update_cr0(thread, value); }
            }
            // srd - Shift Right Doubleword
            539 => {
                let n = thread.gpr(rb as usize) & 0x7F;
                let value = if n > 63 {
                    0
                } else {
                    thread.gpr(rt as usize).wrapping_shr(n as u32)
                };
                thread.set_gpr(ra as usize, value);
                if rc { self.update_cr0(thread, value); }
            }
            // srad - Shift Right Algebraic Doubleword
            794 => {
                let n = thread.gpr(rb as usize) & 0x7F;
                let (value, ca) = if n > 63 {
                    let s = (thread.gpr(rt as usize) as i64) >> 63;
                    (s as u64, s != 0)
                } else {
                    let s = (thread.gpr(rt as usize) as i64) >> (n as u32);
                    let ca = s < 0 && (thread.gpr(rt as usize) & ((1u64 << n) - 1)) != 0;
                    (s as u64, ca)
                };
                thread.set_gpr(ra as usize, value);
                thread.set_xer_ca(ca);
                if rc { self.update_cr0(thread, value); }
            }
            // cntlzw - Count Leading Zeros Word
            26 => {
                let value = (thread.gpr(rt as usize) as u32).leading_zeros() as u64;
                thread.set_gpr(ra as usize, value);
                if rc { self.update_cr0(thread, value); }
            }
            // frsqrte - Floating Reciprocal Square Root Estimate (opcode 31, xo 26)
            // This is handled in A-form, not X-form
            // cntlzd - Count Leading Zeros Doubleword
            58 => {
                let value = thread.gpr(rt as usize).leading_zeros() as u64;
                thread.set_gpr(ra as usize, value);
                if rc { self.update_cr0(thread, value); }
            }
            // popcntw - Population Count Word
            378 => {
                let value = (thread.gpr(rt as usize) as u32).count_ones() as u64;
                thread.set_gpr(ra as usize, value);
            }
            // popcntd - Population Count Doubleword
            506 => {
                let value = thread.gpr(rt as usize).count_ones() as u64;
                thread.set_gpr(ra as usize, value);
            }
            // extsb - Extend Sign Byte
            954 => {
                let value = (thread.gpr(rt as usize) as i8) as i64 as u64;
                thread.set_gpr(ra as usize, value);
                if rc { self.update_cr0(thread, value); }
            }
            // extsh - Extend Sign Halfword
            922 => {
                let value = (thread.gpr(rt as usize) as i16) as i64 as u64;
                thread.set_gpr(ra as usize, value);
                if rc { self.update_cr0(thread, value); }
            }
            // extsw - Extend Sign Word
            986 => {
                let value = (thread.gpr(rt as usize) as i32) as i64 as u64;
                thread.set_gpr(ra as usize, value);
                if rc { self.update_cr0(thread, value); }
            }
            // lbzx - Load Byte and Zero Indexed
            87 => {
                let ea = if ra == 0 { thread.gpr(rb as usize) } else { thread.gpr(ra as usize).wrapping_add(thread.gpr(rb as usize)) };
                let value: u8 = self.memory.read(ea as u32).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_gpr(rt as usize, value as u64);
            }
            // lhzx - Load Halfword and Zero Indexed
            279 => {
                let ea = if ra == 0 { thread.gpr(rb as usize) } else { thread.gpr(ra as usize).wrapping_add(thread.gpr(rb as usize)) };
                let value = self.memory.read_be16(ea as u32).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_gpr(rt as usize, value as u64);
            }
            // lhax - Load Halfword Algebraic Indexed
            343 => {
                let ea = if ra == 0 { thread.gpr(rb as usize) } else { thread.gpr(ra as usize).wrapping_add(thread.gpr(rb as usize)) };
                let value = self.memory.read_be16(ea as u32).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_gpr(rt as usize, (value as i16) as i64 as u64);
            }
            // lwax - Load Word Algebraic Indexed
            341 => {
                let ea = if ra == 0 { thread.gpr(rb as usize) } else { thread.gpr(ra as usize).wrapping_add(thread.gpr(rb as usize)) };
                let value = self.memory.read_be32(ea as u32).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_gpr(rt as usize, (value as i32) as i64 as u64);
            }
            // ldx - Load Doubleword Indexed
            21 => {
                let ea = if ra == 0 { thread.gpr(rb as usize) } else { thread.gpr(ra as usize).wrapping_add(thread.gpr(rb as usize)) };
                let value = self.memory.read_be64(ea as u32).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_gpr(rt as usize, value);
            }
            // stbx - Store Byte Indexed
            215 => {
                let ea = if ra == 0 { thread.gpr(rb as usize) } else { thread.gpr(ra as usize).wrapping_add(thread.gpr(rb as usize)) };
                let value = thread.gpr(rt as usize) as u8;
                self.memory.write(ea as u32, value).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
            }
            // sthx - Store Halfword Indexed
            407 => {
                let ea = if ra == 0 { thread.gpr(rb as usize) } else { thread.gpr(ra as usize).wrapping_add(thread.gpr(rb as usize)) };
                let value = thread.gpr(rt as usize) as u16;
                self.memory.write_be16(ea as u32, value).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
            }
            // stdx - Store Doubleword Indexed
            149 => {
                let ea = if ra == 0 { thread.gpr(rb as usize) } else { thread.gpr(ra as usize).wrapping_add(thread.gpr(rb as usize)) };
                let value = thread.gpr(rt as usize);
                self.memory.write_be64(ea as u32, value).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
            }
            // lwarx - Load Word and Reserve Indexed
            20 => {
                let ea = if ra == 0 { thread.gpr(rb as usize) } else { thread.gpr(ra as usize).wrapping_add(thread.gpr(rb as usize)) };
                let reservation = self.memory.reservation(ea as u32);
                let _time = reservation.acquire();
                let value = self.memory.read_be32(ea as u32).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_gpr(rt as usize, value as u64);
            }
            // ldarx - Load Doubleword and Reserve Indexed
            84 => {
                let ea = if ra == 0 { thread.gpr(rb as usize) } else { thread.gpr(ra as usize).wrapping_add(thread.gpr(rb as usize)) };
                let reservation = self.memory.reservation(ea as u32);
                let _time = reservation.acquire();
                let value = self.memory.read_be64(ea as u32).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_gpr(rt as usize, value);
            }
            // stwcx. - Store Word Conditional Indexed
            150 => {
                let ea = if ra == 0 { thread.gpr(rb as usize) } else { thread.gpr(ra as usize).wrapping_add(thread.gpr(rb as usize)) };
                let value = thread.gpr(rt as usize) as u32;
                let reservation = self.memory.reservation(ea as u32);
                let time = reservation.acquire();
                let success = if reservation.try_lock(time) {
                    self.memory.write_be32(ea as u32, value).map_err(|_| PpuError::InvalidInstruction {
                        addr: thread.pc() as u32,
                        opcode,
                    })?;
                    reservation.unlock_and_increment();
                    true
                } else {
                    false
                };
                let cr0 = if success { 0b0010 } else { 0b0000 } | if thread.get_xer_so() { 1 } else { 0 };
                thread.set_cr_field(0, cr0);
            }
            // stdcx. - Store Doubleword Conditional Indexed
            214 => {
                let ea = if ra == 0 { thread.gpr(rb as usize) } else { thread.gpr(ra as usize).wrapping_add(thread.gpr(rb as usize)) };
                let value = thread.gpr(rt as usize);
                let reservation = self.memory.reservation(ea as u32);
                let time = reservation.acquire();
                let success = if reservation.try_lock(time) {
                    self.memory.write_be64(ea as u32, value).map_err(|_| PpuError::InvalidInstruction {
                        addr: thread.pc() as u32,
                        opcode,
                    })?;
                    reservation.unlock_and_increment();
                    true
                } else {
                    false
                };
                let cr0 = if success { 0b0010 } else { 0b0000 } | if thread.get_xer_so() { 1 } else { 0 };
                thread.set_cr_field(0, cr0);
            }
            // lfdx - Load Floating-Point Double Indexed
            599 => {
                let ea = if ra == 0 { thread.gpr(rb as usize) } else { thread.gpr(ra as usize).wrapping_add(thread.gpr(rb as usize)) };
                let bits = self.memory.read_be64(ea as u32).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_fpr(rt as usize, f64::from_bits(bits));
            }
            // lfsx - Load Floating-Point Single Indexed
            535 => {
                let ea = if ra == 0 { thread.gpr(rb as usize) } else { thread.gpr(ra as usize).wrapping_add(thread.gpr(rb as usize)) };
                let bits = self.memory.read_be32(ea as u32).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
                thread.set_fpr(rt as usize, f32::from_bits(bits) as f64);
            }
            // stfdx - Store Floating-Point Double Indexed
            727 => {
                let ea = if ra == 0 { thread.gpr(rb as usize) } else { thread.gpr(ra as usize).wrapping_add(thread.gpr(rb as usize)) };
                let bits = thread.fpr(rt as usize).to_bits();
                self.memory.write_be64(ea as u32, bits).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
            }
            // stfsx - Store Floating-Point Single Indexed
            663 => {
                let ea = if ra == 0 { thread.gpr(rb as usize) } else { thread.gpr(ra as usize).wrapping_add(thread.gpr(rb as usize)) };
                let bits = (thread.fpr(rt as usize) as f32).to_bits();
                self.memory.write_be32(ea as u32, bits).map_err(|_| PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                })?;
            }
            // fmr - Floating Move Register
            72 => {
                thread.set_fpr(rt as usize, thread.fpr(rb as usize));
                if rc { float::update_cr1(thread); }
            }
            // fneg - Floating Negate
            40 => {
                thread.set_fpr(rt as usize, -thread.fpr(rb as usize));
                if rc { float::update_cr1(thread); }
            }
            // fabs - Floating Absolute Value
            264 => {
                thread.set_fpr(rt as usize, thread.fpr(rb as usize).abs());
                if rc { float::update_cr1(thread); }
            }
            // fnabs - Floating Negative Absolute Value
            136 => {
                thread.set_fpr(rt as usize, -thread.fpr(rb as usize).abs());
                if rc { float::update_cr1(thread); }
            }
            // frsp - Floating Round to Single Precision
            12 => {
                let result = float::frsp(thread.fpr(rb as usize));
                thread.set_fpr(rt as usize, result);
                float::update_fprf(thread, result);
                if rc { float::update_cr1(thread); }
            }
            // fctiw - Floating Convert To Integer Word
            14 => {
                let value = thread.fpr(rb as usize);
                let result = float::fctiwz(value);
                thread.set_fpr(rt as usize, f64::from_bits(result));
                if rc { float::update_cr1(thread); }
            }
            // fctiwz - Floating Convert To Integer Word with Round Toward Zero
            15 => {
                let value = thread.fpr(rb as usize);
                let result = float::fctiwz(value);
                thread.set_fpr(rt as usize, f64::from_bits(result));
                if rc { float::update_cr1(thread); }
            }
            // fctid - Floating Convert To Integer Doubleword
            814 => {
                let value = thread.fpr(rb as usize);
                let result = float::fctidz(value);
                thread.set_fpr(rt as usize, f64::from_bits(result));
                if rc { float::update_cr1(thread); }
            }
            // fctidz - Floating Convert To Integer Doubleword with Round Toward Zero
            815 => {
                let value = thread.fpr(rb as usize);
                let result = float::fctidz(value);
                thread.set_fpr(rt as usize, f64::from_bits(result));
                if rc { float::update_cr1(thread); }
            }
            // fcfid - Floating Convert From Integer Doubleword
            846 => {
                let bits = thread.fpr(rb as usize).to_bits();
                let result = float::fcfid(bits);
                thread.set_fpr(rt as usize, result);
                float::update_fprf(thread, result);
                if rc { float::update_cr1(thread); }
            }
            // fre - Floating Reciprocal Estimate (opcode 59, xo 24, A-form not X-form)
            // Removed from X-form dispatch
            // frsqrte - Floating Reciprocal Square Root Estimate (opcode 59, xo 26, A-form not X-form)
            // Removed from X-form dispatch
            // fcmpu - Floating Compare Unordered (xo 0, but needs different dispatch)
            // Handled separately based on primary opcode 63
            // fcmpo - Floating Compare Ordered (xo 32, but needs context)
            // Handled separately based on primary opcode 63
            // mtfsf - Move To FPSCR Fields
            711 => {
                let fm = ((opcode >> 17) & 0xFF) as u8;
                let value = thread.fpr(rb as usize);
                system::mtfsf(thread, fm, value);
                if rc { float::update_cr1(thread); }
            }
            // mtfsfi - Move To FPSCR Field Immediate
            134 => {
                let bf = ((opcode >> 23) & 7) as u8;
                let imm = ((opcode >> 12) & 0xF) as u8;
                system::mtfsfi(thread, bf, imm);
                if rc { float::update_cr1(thread); }
            }
            // mtfsb0 - Move To FPSCR Bit 0
            70 => {
                let bt = ((opcode >> 21) & 0x1F) as u8;
                system::mtfsb0(thread, bt);
                if rc { float::update_cr1(thread); }
            }
            // mtfsb1 - Move To FPSCR Bit 1
            38 => {
                let bt = ((opcode >> 21) & 0x1F) as u8;
                system::mtfsb1(thread, bt);
                if rc { float::update_cr1(thread); }
            }
            // mffs - Move From FPSCR
            583 => {
                let result = system::mffs(thread);
                thread.set_fpr(rt as usize, result);
                if rc { float::update_cr1(thread); }
            }
            // mcrfs - Move to CR from FPSCR
            64 => {
                let bf = (rt >> 2) & 7;
                let bfa = (ra >> 2) & 7;
                system::mcrfs(thread, bf, bfa);
            }
            // sync - Synchronize
            598 => {
                std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
            }
            // lwsync - Lightweight Synchronize (alias of sync with L=1)
            // Handled same as sync with xo=598 but different L field
            // eieio - Enforce In-Order Execution of I/O
            854 => {
                std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
            }
            // dcbt - Data Cache Block Touch (hint, no-op in emulator)
            278 => {
                // No-op
            }
            // dcbst - Data Cache Block Store (no-op in emulator)
            54 => {
                // No-op
            }
            // dcbf - Data Cache Block Flush (no-op in emulator)
            86 => {
                // No-op
            }
            // icbi - Instruction Cache Block Invalidate (no-op in emulator)
            982 => {
                // No-op
            }
            // mfcr - Move From Condition Register
            19 => {
                thread.set_gpr(rt as usize, thread.regs.cr as u64);
            }
            // mfocrf - Move From One Condition Register Field
            // Same xo as mfcr but with FXM field set
            // mtcrf - Move To Condition Register Fields
            144 => {
                let crm = ((opcode >> 12) & 0xFF) as u8;
                let value = thread.gpr(rt as usize);
                for i in 0..8 {
                    if (crm >> (7 - i)) & 1 != 0 {
                        let field = ((value >> (28 - i * 4)) & 0xF) as u32;
                        thread.set_cr_field(i, field);
                    }
                }
            }
            // mtocrf - Move To One Condition Register Field
            // Same implementation as mtcrf
            // mfmsr - Move From Machine State Register (privileged)
            83 => {
                // Return the actual MSR value
                thread.set_gpr(rt as usize, system::mfmsr(thread));
            }
            // mtmsr - Move To Machine State Register (privileged)
            146 => {
                let value = thread.gpr(rt as usize);
                system::mtmsr(thread, value);
            }
            // mtmsrd - Move To Machine State Register Doubleword (privileged)
            178 => {
                let value = thread.gpr(rt as usize);
                let l = (opcode >> 16) & 1 != 0; // L bit
                system::mtmsrd(thread, value, l);
            }
            // XO-form arithmetic instructions (dispatched as X-form by decoder)
            // Note: These have a 10-bit XO in the decoder, but only 9-bit in the instruction
            // So we need to mask to 9 bits for matching
            _ if (xo & 0x1FF) == 266 => {
                // add - Add
                let (rt, ra, rb, _, _) = PpuDecoder::x_form(opcode);
                let oe = ((opcode >> 10) & 1) != 0;
                let rc = (opcode & 1) != 0;
                
                let a = thread.gpr(ra as usize);
                let b = thread.gpr(rb as usize);
                let result = a.wrapping_add(b);
                thread.set_gpr(rt as usize, result);
                if oe {
                    let overflow = ((a as i64).overflowing_add(b as i64)).1;
                    thread.set_xer_ov(overflow);
                    if overflow { thread.set_xer_so(true); }
                }
                if rc { self.update_cr0(thread, result); }
            }
            _ if (xo & 0x1FF) == 40 => {
                // subf - Subtract From
                let (rt, ra, rb, _, _) = PpuDecoder::x_form(opcode);
                let oe = ((opcode >> 10) & 1) != 0;
                let rc = (opcode & 1) != 0;
                
                let a = thread.gpr(ra as usize);
                let b = thread.gpr(rb as usize);
                let result = b.wrapping_sub(a);
                thread.set_gpr(rt as usize, result);
                if oe {
                    let overflow = ((b as i64).overflowing_sub(a as i64)).1;
                    thread.set_xer_ov(overflow);
                    if overflow { thread.set_xer_so(true); }
                }
                if rc { self.update_cr0(thread, result); }
            }
            // srawi - Shift Right Algebraic Word Immediate
            824 => {
                let sh = rb; // For srawi, the shift amount is in the RB field position
                let value = thread.gpr(rt as usize) as i32;
                let result = (value >> (sh as u32)) as i64 as u64;
                // CA is set if (RS)[32:63] is negative and any 1-bits are shifted out
                let ca = value < 0 && (thread.gpr(rt as usize) as u32 & ((1u32 << sh) - 1)) != 0;
                thread.set_gpr(ra as usize, result);
                thread.set_xer_ca(ca);
                if rc { self.update_cr0(thread, result); }
            }
            _ => {
                tracing::warn!(
                    "Unimplemented X-form xo {} at 0x{:08x} (opcode: 0x{:08x}, rt={}, ra={}, rb={})",
                    xo, thread.pc(), opcode, rt, ra, rb
                );
                return Err(PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                });
            }
            } // End inner match
        } // End outer match

        thread.advance_pc();
        Ok(())
    }

    /// Execute XO-form instructions (integer arithmetic)
    fn execute_xo_form(&self, thread: &mut PpuThread, opcode: u32, xo: u16) -> Result<(), PpuError> {
        let (rt, ra, rb, oe, _, rc) = PpuDecoder::xo_form(opcode);

        match xo {
            // add
            266 => {
                let a = thread.gpr(ra as usize);
                let b = thread.gpr(rb as usize);
                let result = a.wrapping_add(b);
                thread.set_gpr(rt as usize, result);
                if oe {
                    let overflow = ((a as i64).overflowing_add(b as i64)).1;
                    thread.set_xer_ov(overflow);
                    if overflow { thread.set_xer_so(true); }
                }
                if rc { self.update_cr0(thread, result); }
            }
            // subf - Subtract From
            40 => {
                let a = thread.gpr(ra as usize);
                let b = thread.gpr(rb as usize);
                let result = b.wrapping_sub(a);
                thread.set_gpr(rt as usize, result);
                if oe {
                    let overflow = ((b as i64).overflowing_sub(a as i64)).1;
                    thread.set_xer_ov(overflow);
                    if overflow { thread.set_xer_so(true); }
                }
                if rc { self.update_cr0(thread, result); }
            }
            // neg - Negate
            104 => {
                let a = thread.gpr(ra as usize);
                let result = (-(a as i64)) as u64;
                thread.set_gpr(rt as usize, result);
                if oe {
                    let overflow = a == 0x8000_0000_0000_0000;
                    thread.set_xer_ov(overflow);
                    if overflow { thread.set_xer_so(true); }
                }
                if rc { self.update_cr0(thread, result); }
            }
            // addme - Add to Minus One Extended
            234 => {
                let a = thread.gpr(ra as usize);
                let ca = thread.get_xer_ca();
                let result = a.wrapping_add(if ca { 0 } else { u64::MAX });
                thread.set_gpr(rt as usize, result);
                let new_ca = ca || a != 0;
                thread.set_xer_ca(new_ca);
                if oe {
                    let overflow = (a as i64).checked_add(if ca { 0 } else { -1 }).is_none();
                    thread.set_xer_ov(overflow);
                    if overflow { thread.set_xer_so(true); }
                }
                if rc { self.update_cr0(thread, result); }
            }
            // addze - Add to Zero Extended
            202 => {
                let a = thread.gpr(ra as usize);
                let ca = thread.get_xer_ca();
                let result = a.wrapping_add(if ca { 1 } else { 0 });
                thread.set_gpr(rt as usize, result);
                let new_ca = ca && a == u64::MAX;
                thread.set_xer_ca(new_ca);
                if oe {
                    let overflow = ca && a == u64::MAX;
                    thread.set_xer_ov(overflow);
                    if overflow { thread.set_xer_so(true); }
                }
                if rc { self.update_cr0(thread, result); }
            }
            // subfme - Subtract From Minus One Extended
            232 => {
                let a = thread.gpr(ra as usize);
                let ca = thread.get_xer_ca();
                let result = (!a).wrapping_add(if ca { 1 } else { 0 });
                thread.set_gpr(rt as usize, result);
                thread.set_xer_ca(ca || a != u64::MAX);
                if oe {
                    let overflow = a == 0x8000_0000_0000_0000 && !ca;
                    thread.set_xer_ov(overflow);
                    if overflow { thread.set_xer_so(true); }
                }
                if rc { self.update_cr0(thread, result); }
            }
            // subfze - Subtract From Zero Extended
            200 => {
                let a = thread.gpr(ra as usize);
                let ca = thread.get_xer_ca();
                let result = (!a).wrapping_add(if ca { 1 } else { 0 });
                thread.set_gpr(rt as usize, result);
                thread.set_xer_ca(ca || a != 0);
                if oe {
                    let overflow = a == 0x8000_0000_0000_0000 && !ca;
                    thread.set_xer_ov(overflow);
                    if overflow { thread.set_xer_so(true); }
                }
                if rc { self.update_cr0(thread, result); }
            }
            // addc - Add Carrying
            10 => {
                let a = thread.gpr(ra as usize);
                let b = thread.gpr(rb as usize);
                let (result, carry) = a.overflowing_add(b);
                thread.set_gpr(rt as usize, result);
                thread.set_xer_ca(carry);
                if oe {
                    let overflow = ((a as i64).overflowing_add(b as i64)).1;
                    thread.set_xer_ov(overflow);
                    if overflow { thread.set_xer_so(true); }
                }
                if rc { self.update_cr0(thread, result); }
            }
            // adde - Add Extended
            138 => {
                let a = thread.gpr(ra as usize);
                let b = thread.gpr(rb as usize);
                let ca = thread.get_xer_ca();
                let (temp, c1) = a.overflowing_add(b);
                let (result, c2) = temp.overflowing_add(if ca { 1 } else { 0 });
                thread.set_gpr(rt as usize, result);
                thread.set_xer_ca(c1 || c2);
                if oe {
                    let overflow = ((a as i64).overflowing_add(b as i64)).1 || 
                                   ((temp as i64).overflowing_add(if ca { 1 } else { 0 })).1;
                    thread.set_xer_ov(overflow);
                    if overflow { thread.set_xer_so(true); }
                }
                if rc { self.update_cr0(thread, result); }
            }
            // subfc - Subtract From Carrying
            8 => {
                let a = thread.gpr(ra as usize);
                let b = thread.gpr(rb as usize);
                let result = b.wrapping_sub(a);
                thread.set_gpr(rt as usize, result);
                thread.set_xer_ca(b >= a); // Carry set if no borrow
                if oe {
                    let overflow = ((b as i64).overflowing_sub(a as i64)).1;
                    thread.set_xer_ov(overflow);
                    if overflow { thread.set_xer_so(true); }
                }
                if rc { self.update_cr0(thread, result); }
            }
            // subfe - Subtract From Extended
            136 => {
                let a = thread.gpr(ra as usize);
                let b = thread.gpr(rb as usize);
                let ca = thread.get_xer_ca();
                let (temp, c1) = b.overflowing_sub(a);
                let (result, c2) = temp.overflowing_sub(if ca { 0 } else { 1 });
                thread.set_gpr(rt as usize, result);
                thread.set_xer_ca(!(c1 || c2));
                if oe {
                    let overflow = ((b as i64).overflowing_sub(a as i64)).1;
                    thread.set_xer_ov(overflow);
                    if overflow { thread.set_xer_so(true); }
                }
                if rc { self.update_cr0(thread, result); }
            }
            // mullw - Multiply Low Word
            235 => {
                let a = thread.gpr(ra as usize) as i32;
                let b = thread.gpr(rb as usize) as i32;
                let result = (a as i64 * b as i64) as u64;
                thread.set_gpr(rt as usize, result);
                if oe {
                    let full_result = (a as i64) * (b as i64);
                    let overflow = full_result != (result as i32 as i64);
                    thread.set_xer_ov(overflow);
                    if overflow { thread.set_xer_so(true); }
                }
                if rc { self.update_cr0(thread, result); }
            }
            // mulld - Multiply Low Doubleword
            233 => {
                let a = thread.gpr(ra as usize) as i64;
                let b = thread.gpr(rb as usize) as i64;
                let result = a.wrapping_mul(b) as u64;
                thread.set_gpr(rt as usize, result);
                if oe {
                    let overflow = a.checked_mul(b).is_none();
                    thread.set_xer_ov(overflow);
                    if overflow { thread.set_xer_so(true); }
                }
                if rc { self.update_cr0(thread, result); }
            }
            // mulhw - Multiply High Word
            75 => {
                let a = (thread.gpr(ra as usize) as i32) as i64;
                let b = (thread.gpr(rb as usize) as i32) as i64;
                let result = ((a * b) >> 32) as u64;
                thread.set_gpr(rt as usize, result);
                if rc { self.update_cr0(thread, result); }
            }
            // mulhwu - Multiply High Word Unsigned
            11 => {
                let a = (thread.gpr(ra as usize) as u32) as u64;
                let b = (thread.gpr(rb as usize) as u32) as u64;
                let result = (a * b) >> 32;
                thread.set_gpr(rt as usize, result);
                if rc { self.update_cr0(thread, result); }
            }
            // mulhd - Multiply High Doubleword
            73 => {
                let a = thread.gpr(ra as usize) as i64 as i128;
                let b = thread.gpr(rb as usize) as i64 as i128;
                let result = ((a * b) >> 64) as u64;
                thread.set_gpr(rt as usize, result);
                if rc { self.update_cr0(thread, result); }
            }
            // mulhdu - Multiply High Doubleword Unsigned
            9 => {
                let a = thread.gpr(ra as usize) as u128;
                let b = thread.gpr(rb as usize) as u128;
                let result = ((a * b) >> 64) as u64;
                thread.set_gpr(rt as usize, result);
                if rc { self.update_cr0(thread, result); }
            }
            // divw - Divide Word
            491 => {
                let a = thread.gpr(ra as usize) as i32;
                let b = thread.gpr(rb as usize) as i32;
                if b != 0 && !(a == i32::MIN && b == -1) {
                    let result = (a / b) as i64 as u64;
                    thread.set_gpr(rt as usize, result);
                    if oe {
                        thread.set_xer_ov(false);
                    }
                } else {
                    // On overflow or divide by zero, result is 0
                    thread.set_gpr(rt as usize, 0);
                    if oe {
                        thread.set_xer_ov(true);
                        thread.set_xer_so(true);
                    }
                }
                if rc { self.update_cr0(thread, thread.gpr(rt as usize)); }
            }
            // divwu - Divide Word Unsigned
            459 => {
                let a = thread.gpr(ra as usize) as u32;
                let b = thread.gpr(rb as usize) as u32;
                if b != 0 {
                    let result = (a / b) as u64;
                    thread.set_gpr(rt as usize, result);
                    if oe {
                        thread.set_xer_ov(false);
                    }
                } else {
                    thread.set_gpr(rt as usize, 0);
                    if oe {
                        thread.set_xer_ov(true);
                        thread.set_xer_so(true);
                    }
                }
                if rc { self.update_cr0(thread, thread.gpr(rt as usize)); }
            }
            // divd - Divide Doubleword
            489 => {
                let a = thread.gpr(ra as usize) as i64;
                let b = thread.gpr(rb as usize) as i64;
                if b != 0 && !(a == i64::MIN && b == -1) {
                    let result = (a / b) as u64;
                    thread.set_gpr(rt as usize, result);
                    if oe {
                        thread.set_xer_ov(false);
                    }
                } else {
                    // On overflow or divide by zero, result is 0
                    thread.set_gpr(rt as usize, 0);
                    if oe {
                        thread.set_xer_ov(true);
                        thread.set_xer_so(true);
                    }
                }
                if rc { self.update_cr0(thread, thread.gpr(rt as usize)); }
            }
            // divdu - Divide Doubleword Unsigned
            457 => {
                let a = thread.gpr(ra as usize);
                let b = thread.gpr(rb as usize);
                if b != 0 {
                    let result = a / b;
                    thread.set_gpr(rt as usize, result);
                    if oe {
                        thread.set_xer_ov(false);
                    }
                } else {
                    thread.set_gpr(rt as usize, 0);
                    if oe {
                        thread.set_xer_ov(true);
                        thread.set_xer_so(true);
                    }
                }
                if rc { self.update_cr0(thread, thread.gpr(rt as usize)); }
            }
            _ => {
                tracing::warn!(
                    "Unimplemented XO-form xo {} at 0x{:08x} (opcode: 0x{:08x}, rt={}, ra={}, rb={})",
                    xo, thread.pc(), opcode, rt, ra, rb
                );
                return Err(PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                });
            }
        }

        thread.advance_pc();
        Ok(())
    }

    /// Execute XL-form instructions (branch to LR/CTR and CR logical)
    fn execute_xl_form(&self, thread: &mut PpuThread, opcode: u32, xo: u16) -> Result<(), PpuError> {
        let bo = ((opcode >> 21) & 0x1F) as u8;
        let bi = ((opcode >> 16) & 0x1F) as u8;
        let bb = ((opcode >> 11) & 0x1F) as u8;
        let lk = (opcode & 1) != 0;

        match xo {
            // bclr - Branch Conditional to Link Register
            16 => {
                let ctr_ok = if (bo & 0x04) != 0 {
                    true
                } else {
                    thread.regs.ctr = thread.regs.ctr.wrapping_sub(1);
                    ((thread.regs.ctr != 0) as u8) ^ ((bo >> 1) & 1) != 0
                };

                let cond_ok = if (bo & 0x10) != 0 {
                    true
                } else {
                    let cr_bit = (thread.regs.cr >> (31 - bi)) & 1;
                    (cr_bit as u8) == ((bo >> 3) & 1)
                };

                if ctr_ok && cond_ok {
                    let target = thread.regs.lr & !3;
                    if lk {
                        thread.regs.lr = thread.pc() + 4;
                    }
                    thread.set_pc(target);
                } else {
                    thread.advance_pc();
                }
            }
            // bcctr - Branch Conditional to Count Register
            528 => {
                let cond_ok = if (bo & 0x10) != 0 {
                    true
                } else {
                    let cr_bit = (thread.regs.cr >> (31 - bi)) & 1;
                    (cr_bit as u8) == ((bo >> 3) & 1)
                };

                if cond_ok {
                    let target = thread.regs.ctr & !3;
                    
                    // Check if target is in the HLE stub region
                    const STUB_REGION_BASE: u64 = 0x2F00_0000;
                    const STUB_REGION_END: u64 = 0x3000_0000;
                    
                    // R12 typically contains the descriptor address for import calls
                    // Check R12 first as it's the most reliable indicator for HLE imports
                    let r12 = thread.gpr(12);
                    
                    if target >= STUB_REGION_BASE && target < STUB_REGION_END {
                        // This is an HLE stub call - dispatch via the stub handler
                        if lk {
                            thread.regs.lr = thread.pc() + 4;
                        }
                        // Set PC to stub address, step_once will handle it on next iteration
                        thread.set_pc(target);
                    } else if r12 >= STUB_REGION_BASE && r12 < STUB_REGION_END {
                        // R12 contains an HLE stub descriptor - this is an import call
                        // via a PLT stub. Redirect to the HLE handler.
                        tracing::debug!(
                            "Redirecting bctr to HLE stub via R12: CTR=0x{:x}, R12=0x{:x}",
                            target, r12
                        );
                        // LR is already set by the caller (bl instruction before PLT stub)
                        // Set PC to the HLE stub address for dispatch
                        thread.set_pc(r12);
                    } else if target == 0 {
                        // Unresolved import (descriptor not patched) - handle as stub
                        self.handle_unresolved_import(thread)?;
                    } else {
                        // Check if this looks like a PS3 import trampoline
                        if r12 >= 0x10000 && r12 < 0x1000000 {
                            // If the target is in a "trampoline" area (typically near descriptors)
                            // and we have a valid descriptor pointer in R12, try to handle as import
                            // Check if target contains stub-like code (li r3,0; blr pattern)
                            let target_code = self.memory.read_be32(target as u32).unwrap_or(0);
                            
                            // li r3, imm pattern: 0x3860XXXX (addi r3, 0, imm)
                            // or it might be a simple blr: 0x4E800020
                            if target_code == 0x4e800020 || (target_code & 0xFFFF0000) == 0x38600000 {
                                // This looks like an uninitialized/stub trampoline
                                self.handle_unresolved_import(thread)?;
                            } else {
                                // Normal branch
                                if lk {
                                    thread.regs.lr = thread.pc() + 4;
                                }
                                thread.set_pc(target);
                            }
                        } else {
                            // Normal branch to CTR
                            if lk {
                                thread.regs.lr = thread.pc() + 4;
                            }
                            thread.set_pc(target);
                        }
                    }
                } else {
                    thread.advance_pc();
                }
            }
            // mcrf - Move Condition Register Field
            0 => {
                let bf = (bo >> 2) & 7;
                let bfa = (bi >> 2) & 7;
                system::mcrf(thread, bf, bfa);
                thread.advance_pc();
            }
            // crand - Condition Register AND
            257 => {
                system::crand(thread, bo, bi, bb);
                thread.advance_pc();
            }
            // cror - Condition Register OR
            449 => {
                system::cror(thread, bo, bi, bb);
                thread.advance_pc();
            }
            // crxor - Condition Register XOR
            193 => {
                system::crxor(thread, bo, bi, bb);
                thread.advance_pc();
            }
            // crnand - Condition Register NAND
            225 => {
                system::crnand(thread, bo, bi, bb);
                thread.advance_pc();
            }
            // crnor - Condition Register NOR
            33 => {
                system::crnor(thread, bo, bi, bb);
                thread.advance_pc();
            }
            // creqv - Condition Register EQV (XNOR)
            289 => {
                system::creqv(thread, bo, bi, bb);
                thread.advance_pc();
            }
            // crandc - Condition Register AND with Complement
            129 => {
                system::crandc(thread, bo, bi, bb);
                thread.advance_pc();
            }
            // crorc - Condition Register OR with Complement
            417 => {
                system::crorc(thread, bo, bi, bb);
                thread.advance_pc();
            }
            // isync - Instruction Synchronize
            150 => {
                system::isync(thread);
                thread.advance_pc();
            }
            _ => {
                tracing::warn!(
                    "Unimplemented XL-form xo {} at 0x{:08x} (opcode: 0x{:08x}, bo={}, bi={})",
                    xo, thread.pc(), opcode, bo, bi
                );
                return Err(PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                });
            }
        }

        Ok(())
    }

    /// Execute M-form instructions (rotate)
    fn execute_m_form(&self, thread: &mut PpuThread, opcode: u32, op: u8) -> Result<(), PpuError> {
        let (rs, ra, rb_sh, mb, me, rc) = PpuDecoder::m_form(opcode);

        match op {
            // rlwinm - Rotate Left Word Immediate then AND with Mask
            21 => {
                let sh = rb_sh as u32;
                let value = thread.gpr(rs as usize) as u32;
                let rotated = value.rotate_left(sh);
                let mask = Self::generate_mask_32(mb, me);
                let result = (rotated & mask) as u64;
                thread.set_gpr(ra as usize, result);
                if rc { self.update_cr0(thread, result); }
            }
            // rlwimi - Rotate Left Word Immediate then Mask Insert
            20 => {
                let sh = rb_sh as u32;
                let value = thread.gpr(rs as usize) as u32;
                let rotated = value.rotate_left(sh);
                let mask = Self::generate_mask_32(mb, me);
                let result = ((rotated & mask) | (thread.gpr(ra as usize) as u32 & !mask)) as u64;
                thread.set_gpr(ra as usize, result);
                if rc { self.update_cr0(thread, result); }
            }
            // rlwnm - Rotate Left Word then AND with Mask
            23 => {
                let sh = (thread.gpr(rb_sh as usize) & 0x1F) as u32;
                let value = thread.gpr(rs as usize) as u32;
                let rotated = value.rotate_left(sh);
                let mask = Self::generate_mask_32(mb, me);
                let result = (rotated & mask) as u64;
                thread.set_gpr(ra as usize, result);
                if rc { self.update_cr0(thread, result); }
            }
            _ => {
                tracing::warn!(
                    "Unimplemented M-form op {} at 0x{:08x} (opcode: 0x{:08x}, rs={}, ra={}, mb={}, me={})",
                    op, thread.pc(), opcode, rs, ra, mb, me
                );
                return Err(PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                });
            }
        }

        thread.advance_pc();
        Ok(())
    }

    /// Execute DS-form instructions (64-bit load/store with displacement)
    /// Opcode 58: ld (xo=0), ldu (xo=1), lwa (xo=2)
    /// Opcode 62: std (xo=0), stdu (xo=1)
    fn execute_ds_form(&self, thread: &mut PpuThread, opcode: u32, op: u8) -> Result<(), PpuError> {
        let (rt, ra, ds, xo) = PpuDecoder::ds_form(opcode);
        
        match op {
            // ld/ldu/lwa - Load Doubleword / Load Doubleword with Update / Load Word Algebraic
            58 => {
                match xo {
                    // ld - Load Doubleword
                    0 => {
                        let ea = if ra == 0 {
                            ds as i64 as u64
                        } else {
                            thread.gpr(ra as usize).wrapping_add(ds as i64 as u64)
                        };
                        let value = self.memory.read_be64(ea as u32).map_err(|_| PpuError::InvalidInstruction {
                            addr: thread.pc() as u32,
                            opcode,
                        })?;
                        thread.set_gpr(rt as usize, value);
                    }
                    // ldu - Load Doubleword with Update
                    1 => {
                        if ra == 0 || ra == rt {
                            return Err(PpuError::InvalidInstruction {
                                addr: thread.pc() as u32,
                                opcode,
                            });
                        }
                        let ea = thread.gpr(ra as usize).wrapping_add(ds as i64 as u64);
                        let value = self.memory.read_be64(ea as u32).map_err(|_| PpuError::InvalidInstruction {
                            addr: thread.pc() as u32,
                            opcode,
                        })?;
                        thread.set_gpr(rt as usize, value);
                        thread.set_gpr(ra as usize, ea);
                    }
                    // lwa - Load Word Algebraic
                    2 => {
                        let ea = if ra == 0 {
                            ds as i64 as u64
                        } else {
                            thread.gpr(ra as usize).wrapping_add(ds as i64 as u64)
                        };
                        let value = self.memory.read_be32(ea as u32).map_err(|_| PpuError::InvalidInstruction {
                            addr: thread.pc() as u32,
                            opcode,
                        })? as i32 as i64 as u64; // Sign extend
                        thread.set_gpr(rt as usize, value);
                    }
                    _ => {
                        tracing::warn!(
                            "Invalid DS-form xo {} at 0x{:08x} (opcode: 0x{:08x})",
                            xo, thread.pc(), opcode
                        );
                        return Err(PpuError::InvalidInstruction {
                            addr: thread.pc() as u32,
                            opcode,
                        });
                    }
                }
            }
            // std/stdu - Store Doubleword / Store Doubleword with Update
            62 => {
                match xo {
                    // std - Store Doubleword
                    0 => {
                        let ea = if ra == 0 {
                            ds as i64 as u64
                        } else {
                            thread.gpr(ra as usize).wrapping_add(ds as i64 as u64)
                        };
                        let value = thread.gpr(rt as usize);
                        self.memory.write_be64(ea as u32, value).map_err(|_| PpuError::InvalidInstruction {
                            addr: thread.pc() as u32,
                            opcode,
                        })?;
                    }
                    // stdu - Store Doubleword with Update
                    1 => {
                        if ra == 0 {
                            return Err(PpuError::InvalidInstruction {
                                addr: thread.pc() as u32,
                                opcode,
                            });
                        }
                        let ea = thread.gpr(ra as usize).wrapping_add(ds as i64 as u64);
                        let value = thread.gpr(rt as usize);
                        self.memory.write_be64(ea as u32, value).map_err(|_| PpuError::InvalidInstruction {
                            addr: thread.pc() as u32,
                            opcode,
                        })?;
                        thread.set_gpr(ra as usize, ea);
                    }
                    _ => {
                        tracing::warn!(
                            "Invalid DS-form xo {} at 0x{:08x} (opcode: 0x{:08x})",
                            xo, thread.pc(), opcode
                        );
                        return Err(PpuError::InvalidInstruction {
                            addr: thread.pc() as u32,
                            opcode,
                        });
                    }
                }
            }
            _ => {
                tracing::warn!(
                    "Unimplemented DS-form op {} at 0x{:08x} (opcode: 0x{:08x})",
                    op, thread.pc(), opcode
                );
                return Err(PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                });
            }
        }

        thread.advance_pc();
        Ok(())
    }

    /// Execute MD-form instructions (64-bit rotate)
    /// rldicl (xo=0), rldicr (xo=1), rldic (xo=2), rldimi (xo=3)
    fn execute_md_form(&self, thread: &mut PpuThread, opcode: u32) -> Result<(), PpuError> {
        let (rs, ra, sh, mb, xo, rc) = PpuDecoder::md_form(opcode);
        let value = thread.gpr(rs as usize);
        let rotated = value.rotate_left(sh as u32);

        let result = match xo {
            // rldicl - Rotate Left Doubleword Immediate then Clear Left
            0 => {
                // Mask from mb to 63
                let mask = Self::generate_mask_64(mb, 63);
                rotated & mask
            }
            // rldicr - Rotate Left Doubleword Immediate then Clear Right
            1 => {
                // mb is actually 'me' for rldicr
                // Mask from 0 to me (which is stored in mb field)
                let me = mb;
                let mask = Self::generate_mask_64(0, me);
                rotated & mask
            }
            // rldic - Rotate Left Doubleword Immediate then Clear
            2 => {
                // Mask from mb to 63-sh
                let me = (63u8).wrapping_sub(sh);
                let mask = Self::generate_mask_64(mb, me);
                rotated & mask
            }
            // rldimi - Rotate Left Doubleword Immediate then Mask Insert
            3 => {
                // Insert rotated bits into ra using mask
                let me = (63u8).wrapping_sub(sh);
                let mask = Self::generate_mask_64(mb, me);
                (rotated & mask) | (thread.gpr(ra as usize) & !mask)
            }
            _ => {
                tracing::warn!(
                    "Unimplemented MD-form xo {} at 0x{:08x} (opcode: 0x{:08x})",
                    xo, thread.pc(), opcode
                );
                return Err(PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                });
            }
        };

        thread.set_gpr(ra as usize, result);
        if rc {
            self.update_cr0(thread, result);
        }

        thread.advance_pc();
        Ok(())
    }

    /// Execute MDS-form instructions (64-bit rotate with register shift)
    /// rldcl (xo=8), rldcr (xo=9)
    fn execute_mds_form(&self, thread: &mut PpuThread, opcode: u32) -> Result<(), PpuError> {
        let (rs, ra, rb, mb, xo, rc) = PpuDecoder::mds_form(opcode);
        let value = thread.gpr(rs as usize);
        let sh = (thread.gpr(rb as usize) & 0x3F) as u32; // 6-bit shift from register
        let rotated = value.rotate_left(sh);

        let result = match xo {
            // rldcl - Rotate Left Doubleword then Clear Left
            8 => {
                // Mask from mb to 63
                let mask = Self::generate_mask_64(mb, 63);
                rotated & mask
            }
            // rldcr - Rotate Left Doubleword then Clear Right
            9 => {
                // mb is actually 'me' for rldcr
                let me = mb;
                let mask = Self::generate_mask_64(0, me);
                rotated & mask
            }
            _ => {
                tracing::warn!(
                    "Unimplemented MDS-form xo {} at 0x{:08x} (opcode: 0x{:08x})",
                    xo, thread.pc(), opcode
                );
                return Err(PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                });
            }
        };

        thread.set_gpr(ra as usize, result);
        if rc {
            self.update_cr0(thread, result);
        }

        thread.advance_pc();
        Ok(())
    }

    /// Execute XS-form instructions (64-bit shift with immediate)
    /// Currently handles: sradi (xo=413)
    fn execute_xs_form(&self, thread: &mut PpuThread, opcode: u32) -> Result<(), PpuError> {
        let (rs, ra, sh, xo, rc) = PpuDecoder::xs_form(opcode);

        match xo {
            // sradi - Shift Right Algebraic Doubleword Immediate
            413 => {
                let value = thread.gpr(rs as usize) as i64;
                let result = value >> (sh as u32);
                // CA is set if (RS) is negative and any 1-bits are shifted out
                let ca = value < 0 && (thread.gpr(rs as usize) & ((1u64 << sh) - 1)) != 0;
                thread.set_gpr(ra as usize, result as u64);
                thread.set_xer_ca(ca);
                if rc {
                    self.update_cr0(thread, result as u64);
                }
            }
            _ => {
                tracing::warn!(
                    "Unimplemented XS-form xo {} at 0x{:08x} (opcode: 0x{:08x})",
                    xo, thread.pc(), opcode
                );
                return Err(PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                });
            }
        }

        thread.advance_pc();
        Ok(())
    }

    /// Generate 64-bit mask for rotate instructions
    /// Creates a mask with 1s from bit mb to bit me (inclusive)
    #[inline]
    fn generate_mask_64(mb: u8, me: u8) -> u64 {
        let mb = mb as u64;
        let me = me as u64;
        if mb <= me {
            // Normal case: mask from mb to me
            (u64::MAX >> mb) & (u64::MAX << (63 - me))
        } else {
            // Wrap-around case: mask from 0 to me AND from mb to 63
            (u64::MAX >> mb) | (u64::MAX << (63 - me))
        }
    }

    /// Execute system call
    fn execute_sc(&self, thread: &mut PpuThread, _opcode: u32) -> Result<(), PpuError> {
        // System call - the syscall number is in R11
        let syscall_num = thread.gpr(11);
        tracing::trace!("System call {} at 0x{:08x}", syscall_num, thread.pc());
        
        // For now, just advance PC. LV2 kernel will handle syscalls.
        thread.advance_pc();
        Ok(())
    }

    /// Update CR0 based on result (for Rc=1 instructions)
    #[inline]
    fn update_cr0(&self, thread: &mut PpuThread, value: u64) {
        let value = value as i64;
        let c = if value < 0 { 0b1000 } else if value > 0 { 0b0100 } else { 0b0010 };
        let c = c | if thread.get_xer_so() { 1 } else { 0 };
        thread.set_cr_field(0, c);
    }

    /// Generate 32-bit mask for rotate instructions
    #[inline]
    fn generate_mask_32(mb: u8, me: u8) -> u32 {
        let mb = mb as u32;
        let me = me as u32;
        if mb <= me {
            (u32::MAX >> mb) & (u32::MAX << (31 - me))
        } else {
            (u32::MAX >> mb) | (u32::MAX << (31 - me))
        }
    }

    /// Execute A-form instructions (floating-point multiply-add)
    fn execute_a_form(&self, thread: &mut PpuThread, opcode: u32, xo: u16) -> Result<(), PpuError> {
        let frt = ((opcode >> 21) & 0x1F) as usize;
        let fra = ((opcode >> 16) & 0x1F) as usize;
        let frb = ((opcode >> 11) & 0x1F) as usize;
        let frc = ((opcode >> 6) & 0x1F) as usize;
        let rc = (opcode & 1) != 0;
        let primary = (opcode >> 26) & 0x3F;

        // Get operand values
        let a = thread.fpr(fra);
        let b = thread.fpr(frb);
        let c = thread.fpr(frc);

        let result = match (primary, xo) {
            // fmadd - Floating Multiply-Add (Double)
            (63, 29) => float::fmadd(a, c, b),
            // fmsub - Floating Multiply-Subtract (Double)
            (63, 28) => float::fmsub(a, c, b),
            // fnmadd - Floating Negative Multiply-Add (Double)
            (63, 31) => float::fnmadd(a, c, b),
            // fnmsub - Floating Negative Multiply-Subtract (Double)
            (63, 30) => float::fnmsub(a, c, b),
            // fmadds - Floating Multiply-Add Single
            (59, 29) => float::frsp(float::fmadd(a, c, b)),
            // fmsubs - Floating Multiply-Subtract Single
            (59, 28) => float::frsp(float::fmsub(a, c, b)),
            // fnmadds - Floating Negative Multiply-Add Single
            (59, 31) => float::frsp(float::fnmadd(a, c, b)),
            // fnmsubs - Floating Negative Multiply-Subtract Single
            (59, 30) => float::frsp(float::fnmsub(a, c, b)),
            // fmul - Floating Multiply
            (63, 25) => a * c,
            // fmuls - Floating Multiply Single
            (59, 25) => float::frsp(a * c),
            // fadd - Floating Add
            (63, 21) => a + b,
            // fadds - Floating Add Single
            (59, 21) => float::frsp(a + b),
            // fsub - Floating Subtract
            (63, 20) => a - b,
            // fsubs - Floating Subtract Single
            (59, 20) => float::frsp(a - b),
            // fdiv - Floating Divide
            (63, 18) => a / b,
            // fdivs - Floating Divide Single
            (59, 18) => float::frsp(a / b),
            // fsel - Floating Select
            (63, 23) => float::fsel(a, b, c),
            // fsqrt - Floating Square Root (uses FRB only)
            (63, 22) => b.sqrt(),
            // fsqrts - Floating Square Root Single (uses FRB only)
            (59, 22) => float::frsp(b.sqrt()),
            // fre - Floating Reciprocal Estimate (uses FRB only)
            (59, 24) => float::fre(b),
            // frsqrte - Floating Reciprocal Square Root Estimate (uses FRB only)
            (63, 26) => float::frsqrte(b),
            // frsqrtes - Floating Reciprocal Square Root Estimate Single (uses FRB only)
            (59, 26) => float::frsp(float::frsqrte(b)),
            _ => {
                tracing::warn!(
                    "Unimplemented A-form primary={} xo={} at 0x{:08x} (opcode: 0x{:08x}, frt={}, fra={}, frb={}, frc={})",
                    primary, xo, thread.pc(), opcode, frt, fra, frb, frc
                );
                return Err(PpuError::InvalidInstruction {
                    addr: thread.pc() as u32,
                    opcode,
                });
            }
        };

        thread.set_fpr(frt, result);
        float::update_fprf(thread, result);
        
        if rc {
            float::update_cr1(thread);
        }

        thread.advance_pc();
        Ok(())
    }

    /// Execute VA-form instructions (vector three-operand)
    fn execute_va_form(&self, thread: &mut PpuThread, opcode: u32) -> Result<(), PpuError> {
        let vrt = ((opcode >> 21) & 0x1F) as usize;
        let vra = ((opcode >> 16) & 0x1F) as usize;
        let vrb = ((opcode >> 11) & 0x1F) as usize;
        let vrc = ((opcode >> 6) & 0x1F) as usize;
        
        // For VA-form (3-operand), xo is in bits 26-31 (low 6 bits)
        let xo_6bit = (opcode & 0x3F) as u8;
        // For VX-form (2-operand), xo is in bits 21-31 (11 bits)
        let xo_11bit = ((opcode >> 0) & 0x7FF) as u16;
        
        // Determine if this is VA-form or VX-form based on the opcode structure
        // VA-form: bits 6-10 (vrc) are used, bits 0-5 are xo (small range)
        // VX-form: bits 0-10 are xo (larger range, typically >= 64)
        
        // Check common VA-form opcodes first
        if xo_6bit <= 0x2F {
            // This is likely VA-form
            let a = thread.vr(vra);
            let b = thread.vr(vrb);
            let c = thread.vr(vrc);

            let result = match xo_6bit {
                // vperm - Vector Permute
                0x2B => vector::vperm(a, b, c),
                // vmaddfp - Vector Multiply-Add Floating-Point
                0x2E => vector::vmaddfp(a, c, b),
                // vnmsubfp - Vector Negative Multiply-Subtract Floating-Point
                0x2F => vector::vnmsubfp(a, c, b),
                // vsel - Vector Select
                0x2A => vector::vsel(a, b, c),
                // vsldoi - Vector Shift Left Double by Octet Immediate
                0x2C => {
                    let sh = ((opcode >> 6) & 0xF) as u8;
                    vector::vsldoi(a, b, sh)
                }
                _ => {
                    tracing::warn!(
                        "Unimplemented VA-form xo {} at 0x{:08x} (opcode: 0x{:08x}, vrt={}, vra={}, vrb={}, vrc={})",
                        xo_6bit, thread.pc(), opcode, vrt, vra, vrb, vrc
                    );
                    return Err(PpuError::InvalidInstruction {
                        addr: thread.pc() as u32,
                        opcode,
                    });
                }
            };

            thread.set_vr(vrt, result);
        } else {
            // VX-form (2-operand) instructions
            let a = thread.vr(vra);
            let b = thread.vr(vrb);
            let uimm = vrc as u8; // For immediate instructions

            let result = match xo_11bit {
                // vaddubm - Vector Add Unsigned Byte Modulo
                0x000 => vector::vaddubm(a, b),
                // vadduhm - Vector Add Unsigned Halfword Modulo
                0x040 => vector::vadduhm(a, b),
                // vadduwm - Vector Add Unsigned Word Modulo
                0x080 => vector::vadduwm(a, b),
                // vaddsws - Vector Add Signed Word Saturate
                0x180 => vector::vaddsws(a, b),
                // vaddubs - Vector Add Unsigned Byte Saturate
                0x200 => {
                    let mut result = [0u32; 4];
                    for i in 0..4 {
                        let a_bytes = a[i].to_be_bytes();
                        let b_bytes = b[i].to_be_bytes();
                        let mut r_bytes = [0u8; 4];
                        for j in 0..4 {
                            r_bytes[j] = a_bytes[j].saturating_add(b_bytes[j]);
                        }
                        result[i] = u32::from_be_bytes(r_bytes);
                    }
                    result
                }
                // vadduws - Vector Add Unsigned Word Saturate
                0x280 => vector::vadduws(a, b),
                // vaddsbs - Vector Add Signed Byte Saturate
                0x300 => vector::vaddsbs(a, b),
                // vaddshs - Vector Add Signed Halfword Saturate
                0x340 => vector::vaddshs(a, b),
                // vsubsws - Vector Subtract Signed Word Saturate
                0x380 => vector::vsubsws(a, b),
                // vsububm - Vector Subtract Unsigned Byte Modulo
                0x400 => vector::vsububm(a, b),
                // vsubuhm - Vector Subtract Unsigned Halfword Modulo
                0x440 => vector::vsubuhm(a, b),
                // vsubuwm - Vector Subtract Unsigned Word Modulo
                0x480 => vector::vsubuwm(a, b),
                // vsubuws - Vector Subtract Unsigned Word Saturate (corrected opcode from 0x480 to 0x580)
                0x580 => vector::vsubuws(a, b),
                // vsubsbs - Vector Subtract Signed Byte Saturate
                0x700 => vector::vsubsbs(a, b),
                // vsubshs - Vector Subtract Signed Halfword Saturate
                0x740 => vector::vsubshs(a, b),
                // vand - Vector AND
                0x404 => vector::vand(a, b),
                // vandc - Vector AND with Complement
                0x444 => vector::vandc(a, b),
                // vor - Vector OR
                0x484 => vector::vor(a, b),
                // vnor - Vector NOR
                0x504 => vector::vnor(a, b),
                // vxor - Vector XOR
                0x4C4 => vector::vxor(a, b),
                // vslw - Vector Shift Left Word
                0x184 => vector::vslw(a, b),
                // vsrw - Vector Shift Right Word
                0x284 => vector::vsrw(a, b),
                // vsraw - Vector Shift Right Algebraic Word
                0x384 => vector::vsraw(a, b),
                // vrlw - Vector Rotate Left Word
                0x084 => vector::vrlw(a, b),
                // vminsw - Vector Minimum Signed Word
                0x382 => vector::vminsw(a, b),
                // vmaxsw - Vector Maximum Signed Word
                0x182 => vector::vmaxsw(a, b),
                // vminuw - Vector Minimum Unsigned Word
                0x282 => vector::vminuw(a, b),
                // vmaxuw - Vector Maximum Unsigned Word
                0x082 => vector::vmaxuw(a, b),
                // vmulwlw - Vector Multiply Low Word
                0x089 => vector::vmulwlw(a, b),
                // vmulouw - Vector Multiply Odd Unsigned Word
                0x088 => vector::vmulouw(a, b),
                // vcmpequw - Vector Compare Equal Unsigned Word
                0x086 => {
                    let (result, all_true) = vector::vcmpequw(a, b);
                    if (opcode & 0x400) != 0 { // Rc bit
                        let cr6 = if all_true { 0b1000 } else { 0b0000 };
                        thread.set_cr_field(6, cr6);
                    }
                    result
                }
                // vcmpgtsw - Vector Compare Greater Than Signed Word
                0x386 => {
                    let (result, all_true) = vector::vcmpgtsw(a, b);
                    if (opcode & 0x400) != 0 { // Rc bit
                        let cr6 = if all_true { 0b1000 } else { 0b0000 };
                        thread.set_cr_field(6, cr6);
                    }
                    result
                }
                // vcmpgtuw - Vector Compare Greater Than Unsigned Word
                0x286 => {
                    let (result, all_true) = vector::vcmpgtuw(a, b);
                    if (opcode & 0x400) != 0 { // Rc bit
                        let cr6 = if all_true { 0b1000 } else { 0b0000 };
                        thread.set_cr_field(6, cr6);
                    }
                    result
                }
                // vaddfp - Vector Add Single-Precision
                0x00A => vector::vaddfp(a, b),
                // vsubfp - Vector Subtract Single-Precision
                0x04A => vector::vsubfp(a, b),
                // vrefp - Vector Reciprocal Estimate Single-Precision
                0x10A => vector::vrefp(a),
                // vrsqrtefp - Vector Reciprocal Square Root Estimate Single-Precision
                0x14A => vector::vrsqrtefp(a),
                // vcmpeqfp - Vector Compare Equal Single-Precision
                0x0C6 => {
                    let (result, all_true) = vector::vcmpeqfp(a, b);
                    if (opcode & 0x400) != 0 {
                        let cr6 = if all_true { 0b1000 } else { 0b0000 };
                        thread.set_cr_field(6, cr6);
                    }
                    result
                }
                // vcmpgtfp - Vector Compare Greater Than Single-Precision
                0x2C6 => {
                    let (result, all_true) = vector::vcmpgtfp(a, b);
                    if (opcode & 0x400) != 0 {
                        let cr6 = if all_true { 0b1000 } else { 0b0000 };
                        thread.set_cr_field(6, cr6);
                    }
                    result
                }
                // vctsxs - Vector Convert to Signed Integer Word Saturate
                0x3CA => vector::vctsxs(a, uimm),
                // vcfsx - Vector Convert from Signed Integer Word
                0x34A => vector::vcfsx(a, uimm),
                // vspltw - Vector Splat Word
                0x28C => vector::vspltw(b, uimm),
                // vspltisw - Vector Splat Immediate Signed Word
                0x38C => {
                    let simm = ((opcode >> 16) & 0x1F) as i8;
                    let simm = if (simm & 0x10) != 0 { (simm as u8 | 0xE0) as i8 } else { simm };
                    vector::vspltisw(simm as i32)
                }
                // vspltish - Vector Splat Immediate Signed Halfword
                0x34C => {
                    let simm = ((opcode >> 16) & 0x1F) as i8;
                    let simm = if (simm & 0x10) != 0 { (simm as u8 | 0xE0) as i8 } else { simm };
                    vector::vspltish(simm as i16)
                }
                // vspltisb - Vector Splat Immediate Signed Byte
                0x30C => {
                    let simm = ((opcode >> 16) & 0x1F) as i8;
                    let simm = if (simm & 0x10) != 0 { (simm as u8 | 0xE0) as i8 } else { simm };
                    vector::vspltisb(simm)
                }
                // vmrghw - Vector Merge High Word
                0x04C => vector::vmrghw(a, b),
                // vmrglw - Vector Merge Low Word
                0x10C => vector::vmrglw(a, b),
                // vpkuwus - Vector Pack Unsigned Word Unsigned Saturate
                0x0CE => vector::vpkuwus(a, b),
                // vmuleuw - Vector Multiply Even Unsigned Word
                0x288 => vector::vmuleuw(a, b),
                // vmulhuw - Vector Multiply High Unsigned Word
                0x48A => vector::vmulhuw(a, b),
                // vpkshss - Vector Pack Signed Halfword to Signed Byte Saturate
                0x18E => vector::vpkshss(a, b),
                // vpkswss - Vector Pack Signed Word to Signed Halfword Saturate
                0x1CE => vector::vpkswss(a, b),
                // vupkhsb - Vector Unpack High Signed Byte
                0x20E => vector::vupkhsb(a),
                // vupklsb - Vector Unpack Low Signed Byte
                0x28E => vector::vupklsb(a),
                // vmaxfp - Vector Maximum Floating-Point
                0x40A => vector::vmaxfp(a, b),
                // vminfp - Vector Minimum Floating-Point
                0x44A => vector::vminfp(a, b),
                // vsum4ubs - Vector Sum Across Quarter Unsigned Byte Saturate
                0x608 => vector::vsum4ubs(a, b),
                
                // ===== New VMX instructions =====
                
                // lvsl - Load Vector for Shift Left
                0x006 => {
                    let ea = if vra == 0 { thread.gpr(vrb) } else { thread.gpr(vra).wrapping_add(thread.gpr(vrb)) };
                    vector::lvsl(ea)
                }
                // lvsr - Load Vector for Shift Right
                0x046 => {
                    let ea = if vra == 0 { thread.gpr(vrb) } else { thread.gpr(vra).wrapping_add(thread.gpr(vrb)) };
                    vector::lvsr(ea)
                }
                // vspltb - Vector Splat Byte
                0x20C => vector::vspltb(b, uimm),
                // vsplth - Vector Splat Halfword
                0x24C => vector::vsplth(b, uimm),
                // vmrghb - Vector Merge High Byte
                0x00C => vector::vmrghb(a, b),
                // vmrglb - Vector Merge Low Byte
                0x10D => vector::vmrglb(a, b),
                // vmrghh - Vector Merge High Halfword
                0x04D => vector::vmrghh(a, b),
                // vmrglh - Vector Merge Low Halfword
                0x14C => vector::vmrglh(a, b),
                // vcmpgefp - Vector Compare Greater Than or Equal FP
                0x1C6 => {
                    let (result, all_true) = vector::vcmpgefp(a, b);
                    if (opcode & 0x400) != 0 {
                        let cr6 = if all_true { 0b1000 } else { 0b0000 };
                        thread.set_cr_field(6, cr6);
                    }
                    result
                }
                // vcmpbfp - Vector Compare Bounds FP
                0x3C6 => {
                    let (result, all_in_bounds) = vector::vcmpbfp(a, b);
                    if (opcode & 0x400) != 0 {
                        let cr6 = if all_in_bounds { 0b0010 } else { 0b0000 };
                        thread.set_cr_field(6, cr6);
                    }
                    result
                }
                // vlogefp - Vector Log2 Estimate FP
                0x1CA => vector::vlogefp(a),
                // vexptefp - Vector 2^x Estimate FP
                0x18A => vector::vexptefp(a),
                // vctuxs - Vector Convert to Unsigned Fixed-Point Word Saturate
                0x38A => vector::vctuxs(a, uimm),
                // vcfux - Vector Convert from Unsigned Fixed-Point Word
                0x30A => vector::vcfux(a, uimm),
                // vavgub - Vector Average Unsigned Byte
                0x402 => vector::vavgub(a, b),
                // vavguh - Vector Average Unsigned Halfword
                0x442 => vector::vavguh(a, b),
                // vavguw - Vector Average Unsigned Word
                0x482 => vector::vavguw(a, b),
                // vavgsb - Vector Average Signed Byte
                0x502 => vector::vavgsb(a, b),
                // vavgsh - Vector Average Signed Halfword
                0x542 => vector::vavgsh(a, b),
                // vavgsw - Vector Average Signed Word
                0x582 => vector::vavgsw(a, b),
                // vmulesb - Vector Multiply Even Signed Byte
                0x308 => vector::vmulesb(a, b),
                // vmulosb - Vector Multiply Odd Signed Byte
                0x108 => vector::vmulosb(a, b),
                // vmuleub - Vector Multiply Even Unsigned Byte
                0x208 => vector::vmuleub(a, b),
                // vmuloub - Vector Multiply Odd Unsigned Byte
                0x008 => vector::vmuloub(a, b),
                // vmulesh - Vector Multiply Even Signed Halfword
                0x348 => vector::vmulesh(a, b),
                // vmulosh - Vector Multiply Odd Signed Halfword
                0x148 => vector::vmulosh(a, b),
                // vmuleuh - Vector Multiply Even Unsigned Halfword
                0x248 => vector::vmuleuh(a, b),
                // vmulouh - Vector Multiply Odd Unsigned Halfword
                0x048 => vector::vmulouh(a, b),
                
                // lvx - Load Vector Indexed (handled specially with memory)
                0x007 => {
                    // This shouldn't be in VA-form dispatch, but handle it anyway
                    let ea = if vra == 0 { thread.gpr(vrb) } else { thread.gpr(vra).wrapping_add(thread.gpr(vrb)) };
                    let ea = ea & !0xF; // Align to 16 bytes
                    let mut result = [0u32; 4];
                    for i in 0..4 {
                        result[i] = self.memory.read_be32((ea + i as u64 * 4) as u32).unwrap_or(0);
                    }
                    result
                }
                // stvx - Store Vector Indexed (handled specially with memory)
                0x087 => {
                    let ea = if vra == 0 { thread.gpr(vrb) } else { thread.gpr(vra).wrapping_add(thread.gpr(vrb)) };
                    let ea = ea & !0xF; // Align to 16 bytes
                    let value = a;
                    for i in 0..4 {
                        let _ = self.memory.write_be32((ea + i as u64 * 4) as u32, value[i]);
                    }
                    a // Return unchanged
                }
                _ => {
                    tracing::warn!(
                        "Unimplemented VX-form xo 0x{:03x} ({}) at 0x{:08x} (opcode: 0x{:08x}, vrt={}, vra={}, vrb={})",
                        xo_11bit, xo_11bit, thread.pc(), opcode, vrt, vra, vrb
                    );
                    return Err(PpuError::InvalidInstruction {
                        addr: thread.pc() as u32,
                        opcode,
                    });
                }
            };

            thread.set_vr(vrt, result);
        }

        thread.advance_pc();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_env() -> (PpuInterpreter, PpuThread) {
        let memory = MemoryManager::new().unwrap();
        let interpreter = PpuInterpreter::new(memory.clone());
        let thread = PpuThread::new(0, memory);
        (interpreter, thread)
    }

    /// Helper to write an instruction to memory and execute it
    fn execute_instruction(interpreter: &PpuInterpreter, thread: &mut PpuThread, opcode: u32) -> Result<(), PpuError> {
        let pc = thread.pc() as u32;
        interpreter.memory.write_be32(pc, opcode).unwrap();
        interpreter.step(thread)
    }

    #[test]
    fn test_interpreter_creation() {
        let (interpreter, thread) = create_test_env();
        assert_eq!(thread.pc(), 0);
        drop(interpreter);
    }

    #[test]
    fn test_mask_generation() {
        assert_eq!(PpuInterpreter::generate_mask_32(0, 31), 0xFFFFFFFF);
        assert_eq!(PpuInterpreter::generate_mask_32(16, 31), 0x0000FFFF);
        assert_eq!(PpuInterpreter::generate_mask_32(0, 15), 0xFFFF0000);
    }

    // ===== ADDI Tests =====
    
    #[test]
    fn test_addi_basic() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // addi r3, r0, 100  (opcode 14, rt=3, ra=0, simm=100)
        // When ra=0, addi loads the immediate directly
        let opcode = 0x38600064u32; // addi r3, r0, 100
        execute_instruction(&interpreter, &mut thread, opcode).unwrap();
        
        assert_eq!(thread.gpr(3), 100);
        assert_eq!(thread.pc(), 0x2000_0004);
    }

    #[test]
    fn test_addi_with_register() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        thread.set_gpr(4, 1000);
        
        // addi r3, r4, 50  (r3 = r4 + 50 = 1050)
        let opcode = 0x38640032u32; // addi r3, r4, 50
        execute_instruction(&interpreter, &mut thread, opcode).unwrap();
        
        assert_eq!(thread.gpr(3), 1050);
    }

    #[test]
    fn test_addi_negative_immediate() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        thread.set_gpr(5, 100);
        
        // addi r3, r5, -50  (r3 = r5 - 50 = 50)
        // -50 in 16-bit signed = 0xFFCE
        let opcode = 0x3865FFCEu32; // addi r3, r5, -50
        execute_instruction(&interpreter, &mut thread, opcode).unwrap();
        
        assert_eq!(thread.gpr(3), 50);
    }

    // ===== LWZ/STW Tests =====
    
    #[test]
    fn test_stw_lwz_basic() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Store value to memory
        thread.set_gpr(3, 0xDEADBEEF);
        thread.set_gpr(4, 0x2000_1000); // Base address
        
        // stw r3, 0(r4)
        let stw_opcode = 0x90640000u32; // stw r3, 0(r4)
        execute_instruction(&interpreter, &mut thread, stw_opcode).unwrap();
        
        // Clear r3 and load back
        thread.set_gpr(3, 0);
        
        // lwz r3, 0(r4)
        let lwz_opcode = 0x80640000u32; // lwz r3, 0(r4)
        execute_instruction(&interpreter, &mut thread, lwz_opcode).unwrap();
        
        assert_eq!(thread.gpr(3), 0xDEADBEEF);
    }

    #[test]
    fn test_lwz_stw_with_displacement() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        thread.set_gpr(3, 0x12345678);
        thread.set_gpr(4, 0x2000_1000);
        
        // stw r3, 16(r4) - store at base + 16
        let stw_opcode = 0x90640010u32; // stw r3, 16(r4)
        execute_instruction(&interpreter, &mut thread, stw_opcode).unwrap();
        
        thread.set_gpr(3, 0);
        
        // lwz r3, 16(r4) - load from base + 16
        let lwz_opcode = 0x80640010u32; // lwz r3, 16(r4)
        execute_instruction(&interpreter, &mut thread, lwz_opcode).unwrap();
        
        assert_eq!(thread.gpr(3), 0x12345678);
    }

    // ===== Branch Tests =====
    
    #[test]
    fn test_branch_unconditional() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // b 0x100 (relative branch forward 0x100 bytes)
        let opcode = 0x48000100u32;
        execute_instruction(&interpreter, &mut thread, opcode).unwrap();
        
        assert_eq!(thread.pc(), 0x2000_0100);
    }

    #[test]
    fn test_branch_with_link() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // bl 0x200 (branch and link)
        let opcode = 0x48000201u32; // bl 0x200
        execute_instruction(&interpreter, &mut thread, opcode).unwrap();
        
        assert_eq!(thread.pc(), 0x2000_0200);
        assert_eq!(thread.regs.lr, 0x2000_0004); // Return address
    }

    #[test]
    fn test_branch_backward() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_1000);
        
        // b -0x100 (branch backward)
        // -0x100 in 26-bit signed, left-shifted by 2
        let opcode = 0x4BFFFF00u32; // b -0x100
        execute_instruction(&interpreter, &mut thread, opcode).unwrap();
        
        assert_eq!(thread.pc(), 0x2000_0F00);
    }

    // ===== Conditional Branch (bc) Tests =====
    
    #[test]
    fn test_bc_branch_if_equal() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Set CR0 EQ bit (bit 2 of CR0, which is bit 30 in CR register)
        thread.set_cr_field(0, 0b0010); // EQ set
        
        // beq 0x40 (branch if CR0 EQ set)
        // BO=01100 (branch if condition true), BI=2 (CR0 EQ)
        let opcode = 0x41820040u32; // beq 0x40
        execute_instruction(&interpreter, &mut thread, opcode).unwrap();
        
        assert_eq!(thread.pc(), 0x2000_0040);
    }

    #[test]
    fn test_bc_no_branch_if_not_equal() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Set CR0 to GT (not equal)
        thread.set_cr_field(0, 0b0100); // GT set, EQ clear
        
        // beq 0x40 (should NOT branch since EQ is not set)
        let opcode = 0x41820040u32; // beq 0x40
        execute_instruction(&interpreter, &mut thread, opcode).unwrap();
        
        // Should just advance PC since condition is false
        assert_eq!(thread.pc(), 0x2000_0004);
    }

    #[test]
    fn test_bc_branch_if_less_than() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Set CR0 LT bit
        thread.set_cr_field(0, 0b1000); // LT set
        
        // blt 0x80 (branch if less than)
        // BO=01100 (branch if condition true), BI=0 (CR0 LT)
        let opcode = 0x41800080u32; // blt 0x80
        execute_instruction(&interpreter, &mut thread, opcode).unwrap();
        
        assert_eq!(thread.pc(), 0x2000_0080);
    }

    // ===== CR Logical Operations Tests =====
    
    #[test]
    fn test_crand() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Set bits 0 and 1 of CR (bits 31 and 30 in position terms)
        thread.regs.cr = 0xC000_0000; // bits 0 and 1 set
        
        // crand bt=2, ba=0, bb=1 (CR[2] = CR[0] & CR[1])
        // XL-form: [0:5]=19, [6:10]=bt=2, [11:15]=ba=0, [16:20]=bb=1, [21:30]=xo=257, [31]=0
        // Binary: 010011 00010 00000 00001 0100000001 0
        let opcode = 0x4C40_0A02u32; // crand 2, 0, 1
        execute_instruction(&interpreter, &mut thread, opcode).unwrap();
        
        // Bit 2 should be set (1 & 1 = 1)
        assert!((thread.regs.cr >> 29) & 1 == 1);
    }

    #[test]
    fn test_cror() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Set only bit 0
        thread.regs.cr = 0x8000_0000;
        
        // cror bt=2, ba=0, bb=1 (CR[2] = CR[0] | CR[1])
        // XL-form: [0:5]=19, [6:10]=bt=2, [11:15]=ba=0, [16:20]=bb=1, [21:30]=xo=449, [31]=0
        // xo=449 in bits 21-30, shifted: 449 << 1 = 0x382
        // Binary: 010011 00010 00000 00001 0111000001 0
        let opcode = 0x4C40_0B82u32; // cror 2, 0, 1
        execute_instruction(&interpreter, &mut thread, opcode).unwrap();
        
        // Bit 2 should be set (1 | 0 = 1)
        assert!((thread.regs.cr >> 29) & 1 == 1);
    }

    #[test]
    fn test_crxor_clear() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        thread.regs.cr = 0xFFFF_FFFF;
        
        // crxor bt=0, ba=0, bb=0 (CR[0] = CR[0] ^ CR[0] = 0)
        // XL-form: [0:5]=19, [6:10]=bt=0, [11:15]=ba=0, [16:20]=bb=0, [21:30]=xo=193, [31]=0
        // xo=193 = 0b0011000001
        // Binary: 010011 00000 00000 00000 0011000001 0
        let opcode = 0x4C00_0182u32; // crxor 0, 0, 0
        execute_instruction(&interpreter, &mut thread, opcode).unwrap();
        
        // Bit 0 should be cleared
        assert!((thread.regs.cr >> 31) & 1 == 0);
    }

    // ===== FMADD Tests =====
    
    #[test]
    fn test_fmadd_basic() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // fmadd f3, f1, f2, f4 => f3 = (f1 * f2) + f4
        // A-form: fmadd frt, fra, frc, frb  => frt = (fra * frc) + frb
        // So fmadd f3, f1, f2, f4 means f3 = (f1 * f2) + f4
        thread.set_fpr(1, 2.0);  // fra
        thread.set_fpr(2, 3.0);  // frc
        thread.set_fpr(4, 4.0);  // frb
        
        // fmadd f3, f1, f2, f4
        // Primary opcode 63 (0x3F), A-form
        // [0:5]=63, [6:10]=frt=3, [11:15]=fra=1, [16:20]=frb=4, [21:25]=frc=2, [26:30]=xo=29, [31]=rc=0
        // Binary: 111111 00011 00001 00100 00010 11101 0
        let opcode = 0xFC61_20BAu32; // fmadd f3, f1, f2, f4
        execute_instruction(&interpreter, &mut thread, opcode).unwrap();
        
        // Result should be (2.0 * 3.0) + 4.0 = 10.0
        assert_eq!(thread.fpr(3), 10.0);
    }

    #[test]
    fn test_fmsub_basic() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // fmsub f3, f1, f2, f4 => f3 = (f1 * f2) - f4
        thread.set_fpr(1, 5.0);  // fra
        thread.set_fpr(2, 2.0);  // frc
        thread.set_fpr(4, 3.0);  // frb
        
        // fmsub f3, f1, f2, f4
        // [0:5]=63, [6:10]=frt=3, [11:15]=fra=1, [16:20]=frb=4, [21:25]=frc=2, [26:30]=xo=28, [31]=rc=0
        // Binary: 111111 00011 00001 00100 00010 11100 0
        let opcode = 0xFC61_20B8u32; // fmsub f3, f1, f2, f4
        execute_instruction(&interpreter, &mut thread, opcode).unwrap();
        
        // Result = (5.0 * 2.0) - 3.0 = 7.0
        assert_eq!(thread.fpr(3), 7.0);
    }

    // ===== VPERM (Vector Permute) Tests =====
    
    #[test]
    fn test_vperm_identity() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Set up source vectors
        thread.set_vr(1, [0x00010203, 0x04050607, 0x08090A0B, 0x0C0D0E0F]);
        thread.set_vr(2, [0x10111213, 0x14151617, 0x18191A1B, 0x1C1D1E1F]);
        
        // Control vector: identity permutation (0,1,2,3,4,5,6,7,8,9,A,B,C,D,E,F)
        thread.set_vr(3, [0x00010203, 0x04050607, 0x08090A0B, 0x0C0D0E0F]);
        
        // vperm v4, v1, v2, v3
        // VA-form: [0:5]=4, [6:10]=vrt=4, [11:15]=vra=1, [16:20]=vrb=2, [21:25]=vrc=3, [26:31]=xo=43
        // Binary: 000100 00100 00001 00010 00011 101011
        let opcode = 0x1081_10EBu32; // vperm v4, v1, v2, v3
        execute_instruction(&interpreter, &mut thread, opcode).unwrap();
        
        // Result should be same as v1 (identity permutation selects first 16 bytes)
        let result = thread.vr(4);
        assert_eq!(result, [0x00010203, 0x04050607, 0x08090A0B, 0x0C0D0E0F]);
    }

    #[test]
    fn test_vperm_swap_halves() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Source vectors
        thread.set_vr(1, [0x00010203, 0x04050607, 0x08090A0B, 0x0C0D0E0F]);
        thread.set_vr(2, [0x10111213, 0x14151617, 0x18191A1B, 0x1C1D1E1F]);
        
        // Control: select bytes 8-15 then 0-7 from first vector
        thread.set_vr(3, [0x08090A0B, 0x0C0D0E0F, 0x00010203, 0x04050607]);
        
        // vperm v4, v1, v2, v3
        let opcode = 0x1081_10EBu32;
        execute_instruction(&interpreter, &mut thread, opcode).unwrap();
        
        // Result: bytes 8-15 followed by 0-7
        let result = thread.vr(4);
        assert_eq!(result, [0x08090A0B, 0x0C0D0E0F, 0x00010203, 0x04050607]);
    }

    #[test]
    fn test_vmaddfp() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // vmaddfp v4, v1, v2, v3 => v4 = (v1 * v2) + v3
        // VA-form: vmaddfp vrt, vra, vrc, vrb  => (vra * vrc) + vrb
        let a = [2.0f32.to_bits(), 3.0f32.to_bits(), 4.0f32.to_bits(), 5.0f32.to_bits()];
        let c = [1.5f32.to_bits(), 2.0f32.to_bits(), 0.5f32.to_bits(), 1.0f32.to_bits()];
        let b = [1.0f32.to_bits(), 1.0f32.to_bits(), 1.0f32.to_bits(), 1.0f32.to_bits()];
        
        thread.set_vr(1, a);  // vra
        thread.set_vr(2, c);  // vrc
        thread.set_vr(3, b);  // vrb
        
        // vmaddfp v4, v1, v2, v3
        // VA-form: [0:5]=4, [6:10]=vrt=4, [11:15]=vra=1, [16:20]=vrb=3, [21:25]=vrc=2, [26:31]=xo=46
        // Note: vrb is the addend, vrc is the multiplier
        // Binary: 000100 00100 00001 00011 00010 101110
        let opcode = 0x1081_18AEu32; // vmaddfp v4, v1, v2, v3
        execute_instruction(&interpreter, &mut thread, opcode).unwrap();
        
        let result = thread.vr(4);
        // v4[0] = 2.0*1.5 + 1.0 = 4.0
        // v4[1] = 3.0*2.0 + 1.0 = 7.0
        // v4[2] = 4.0*0.5 + 1.0 = 3.0
        // v4[3] = 5.0*1.0 + 1.0 = 6.0
        assert_eq!(f32::from_bits(result[0]), 4.0);
        assert_eq!(f32::from_bits(result[1]), 7.0);
        assert_eq!(f32::from_bits(result[2]), 3.0);
        assert_eq!(f32::from_bits(result[3]), 6.0);
    }

    // ===== Edge Case Tests =====

    #[test]
    fn test_addi_overflow() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Test overflow with max i64 value
        thread.set_gpr(4, i64::MAX as u64);
        
        // addi r3, r4, 1 (should wrap around)
        let opcode = 0x38640001u32;
        execute_instruction(&interpreter, &mut thread, opcode).unwrap();
        
        // Result should wrap to min value (wrapping add)
        assert_eq!(thread.gpr(3) as i64, i64::MIN);
    }

    #[test]
    fn test_addi_ra_zero_special_case() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Set r0 to a value (should be ignored)
        thread.set_gpr(0, 999);
        
        // addi r3, r0, 42 (ra=0 means load immediate, not add to r0)
        let opcode = 0x3860002Au32;
        execute_instruction(&interpreter, &mut thread, opcode).unwrap();
        
        assert_eq!(thread.gpr(3), 42);
    }

    #[test]
    fn test_divw_by_zero() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Setup: divide by zero
        thread.set_gpr(4, 100);
        thread.set_gpr(5, 0);
        
        // divw r3, r4, r5 with OE=0 (no overflow exception)
        // XO-form: op=31, rt=3, ra=4, rb=5, oe=0, xo=491, rc=0
        let opcode = 0x7C64_2BD6u32;
        
        // Write instruction and execute
        interpreter.memory.write_be32(0x2000_0000, opcode).unwrap();
        interpreter.step(&mut thread).unwrap();
        
        // Result should be 0 on divide by zero
        assert_eq!(thread.gpr(3), 0);
    }

    #[test]
    fn test_divw_overflow() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Setup: i32::MIN / -1 causes overflow
        thread.set_gpr(4, i32::MIN as u64);
        thread.set_gpr(5, (-1i32) as u32 as u64);
        
        // divw r3, r4, r5 with OE=0
        let opcode = 0x7C64_2BD6u32;
        interpreter.memory.write_be32(0x2000_0000, opcode).unwrap();
        interpreter.step(&mut thread).unwrap();
        
        // Result should be 0 on overflow
        assert_eq!(thread.gpr(3), 0);
    }

    #[test]
    fn test_branch_boundary() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Branch forward by a reasonable offset (not near 32-bit boundary for test safety)
        let offset = 0x1000i32;
        
        // Create branch instruction
        let li = ((offset >> 2) & 0x00FFFFFF) as u32;
        let opcode = 0x48000000u32 | (li << 2);
        
        execute_instruction(&interpreter, &mut thread, opcode).unwrap();
        assert_eq!(thread.pc(), 0x2000_0000 + offset as u64);
    }

    #[test]
    fn test_cmp_signed_vs_unsigned() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Test signed comparison with negative value
        thread.set_gpr(4, (-10i64) as u64);
        thread.set_gpr(5, 10u64);
        
        // cmp cr0, 0, r4, r5 (signed comparison, 64-bit)
        // X-form: op=31, bf=0, l=1, ra=4, rb=5, xo=0
        let opcode = 0x7C04_2800u32 | (1 << 21); // l=1 for 64-bit
        interpreter.memory.write_be32(0x2000_0000, opcode).unwrap();
        interpreter.step(&mut thread).unwrap();
        
        // -10 < 10, so LT bit should be set in CR0
        let cr0 = thread.get_cr_field(0);
        assert_eq!(cr0 & 0b1000, 0b1000); // LT bit set
    }

    #[test]
    fn test_cmpl_unsigned() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Test unsigned comparison
        thread.set_gpr(4, (-10i64) as u64); // Large unsigned value
        thread.set_gpr(5, 10u64);
        
        // cmpl cr0, 1, r4, r5 (unsigned comparison, 64-bit)
        // X-form: op=31, bf=0, l=1, ra=4, rb=5, xo=32
        let opcode = 0x7C04_2840u32 | (1 << 21); // l=1 for 64-bit
        interpreter.memory.write_be32(0x2000_0000, opcode).unwrap();
        interpreter.step(&mut thread).unwrap();
        
        // As unsigned, -10 > 10, so GT bit should be set
        let cr0 = thread.get_cr_field(0);
        assert_eq!(cr0 & 0b0100, 0b0100); // GT bit set
    }

    #[test]
    fn test_rotate_mask_edge_cases() {
        // Test mask generation
        // When mb <= me: mask includes bits mb through me
        // When mb > me: mask wraps around
        
        // Test full mask (mb=0, me=31)
        assert_eq!(PpuInterpreter::generate_mask_32(0, 31), 0xFFFFFFFF);
        
        // Test single bit mask at bit 31 (mb=31, me=31)
        assert_eq!(PpuInterpreter::generate_mask_32(31, 31), 0x00000001);
        
        // Test single bit mask at bit 0 (mb=0, me=0)
        assert_eq!(PpuInterpreter::generate_mask_32(0, 0), 0x80000000);
        
        // Test contiguous mask (mb=8, me=15) - bits 8-15
        assert_eq!(PpuInterpreter::generate_mask_32(8, 15), 0x00FF0000);
    }

    #[test]
    fn test_rlwinm_extract_bits() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Test basic rotate and mask (simplified version)
        thread.set_gpr(4, 0xABCD1234);
        
        // rlwinm r3, r4, 8, 24, 31 - rotate left 8 bits and mask bits 24-31
        // This should give us the second byte rotated to the last position
        // M-form: op=21, rs=4, ra=3, sh=8, mb=24, me=31, rc=0
        let opcode = 0x5483_443Eu32;
        execute_instruction(&interpreter, &mut thread, opcode).unwrap();
        
        // After rotating 0xABCD1234 left by 8: 0xCD1234AB
        // Mask bits 24-31: 0x000000AB
        assert_eq!(thread.gpr(3) & 0xFF, 0xAB);
    }

    #[test]
    fn test_overflow_flag_propagation() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Simple test for overflow detection
        // Test with i64::MAX + 1 which should overflow
        thread.set_gpr(5, 0x7FFFFFFFFFFFFFFF_u64); // i64::MAX
        thread.set_gpr(6, 1);
        
        // add r4, r5, r6 with OE=1 (enable overflow detection)
        let opcode = 0x7C85_3614u32;
        interpreter.memory.write_be32(0x2000_0000, opcode).unwrap();
        interpreter.step(&mut thread).unwrap();
        
        // Overflow should be detected (i64::MAX + 1 overflows in signed arithmetic)
        assert!(thread.get_xer_ov(), "OV bit should be set on overflow");
        assert!(thread.get_xer_so(), "SO bit should be set on overflow");
    }

    #[test]
    fn test_conditional_branch_ctr() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Set CTR to 5
        thread.regs.ctr = 5;
        
        // bdnz 0x40 (branch if --CTR != 0)
        // BO=16 (decrement CTR, branch if CTR != 0)
        let opcode = 0x42000040u32;
        execute_instruction(&interpreter, &mut thread, opcode).unwrap();
        
        // CTR should be decremented
        assert_eq!(thread.regs.ctr, 4);
        // Should have branched
        assert_eq!(thread.pc(), 0x2000_0040);
    }

    #[test]
    fn test_conditional_branch_no_ctr_decrement() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Set CTR to 5
        thread.regs.ctr = 5;
        
        // Branch with BO bit 2 set (don't modify CTR)
        // BO=20 (ignore CTR)
        let opcode = 0x42800040u32; // bc with BO=20
        execute_instruction(&interpreter, &mut thread, opcode).unwrap();
        
        // CTR should NOT be decremented
        assert_eq!(thread.regs.ctr, 5);
    }

    // ===== Breakpoint Tests =====

    #[test]
    fn test_breakpoint_unconditional() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Add breakpoint at current PC
        interpreter.add_breakpoint(0x2000_0000, BreakpointType::Unconditional);
        
        // Write a simple instruction
        interpreter.memory.write_be32(0x2000_0000, 0x38600064).unwrap();
        
        // Step should hit breakpoint
        let result = interpreter.step(&mut thread);
        assert!(matches!(result, Err(PpuError::Breakpoint { addr: 0x2000_0000 })));
    }

    #[test]
    fn test_breakpoint_conditional_gpr() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Add conditional breakpoint that triggers when r3 == 42
        interpreter.add_breakpoint(
            0x2000_0000,
            BreakpointType::Conditional(BreakpointCondition::GprEquals {
                reg: 3,
                value: 42,
            }),
        );
        
        thread.set_gpr(3, 41);
        interpreter.memory.write_be32(0x2000_0000, 0x38600064).unwrap();
        
        // Should not break (r3 != 42)
        assert!(interpreter.step(&mut thread).is_ok());
        
        // Set r3 to 42
        thread.set_pc(0x2000_0000);
        thread.set_gpr(3, 42);
        
        // Should break now
        let result = interpreter.step(&mut thread);
        assert!(matches!(result, Err(PpuError::Breakpoint { .. })));
    }

    #[test]
    fn test_breakpoint_disable() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Add and then disable breakpoint
        interpreter.add_breakpoint(0x2000_0000, BreakpointType::Unconditional);
        interpreter.disable_breakpoint(0x2000_0000);
        
        interpreter.memory.write_be32(0x2000_0000, 0x38600064).unwrap();
        
        // Should not break (disabled)
        assert!(interpreter.step(&mut thread).is_ok());
    }

    #[test]
    fn test_breakpoint_hit_count() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        interpreter.add_breakpoint(0x2000_0000, BreakpointType::Unconditional);
        interpreter.memory.write_be32(0x2000_0000, 0x38600064).unwrap();
        
        // Hit breakpoint once
        let _ = interpreter.step(&mut thread);
        
        // Check hit count
        let breakpoints = interpreter.get_breakpoints();
        assert_eq!(breakpoints[0].hit_count, 1);
    }

    #[test]
    fn test_instruction_count() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Write some instructions
        for i in 0..5 {
            interpreter
                .memory
                .write_be32(0x2000_0000 + i * 4, 0x60000000)
                .unwrap(); // nop
        }
        
        // Execute 3 instructions
        for _ in 0..3 {
            let _ = interpreter.step(&mut thread);
        }
        
        assert_eq!(interpreter.instruction_count(), 3);
        
        interpreter.reset_instruction_count();
        assert_eq!(interpreter.instruction_count(), 0);
    }

    #[test]
    fn test_srawi_positive() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Setup: positive value to shift right algebraically
        thread.set_gpr(3, 0x1000_0000u64); // 268,435,456 in r3
        
        // srawi r4, r3, 4 - shift right by 4 bits
        // X-form: op=31, rs=3, ra=4, sh=4, xo=824, rc=0
        // Bits: 31 << 26 | 3 << 21 | 4 << 16 | 4 << 11 | 824 << 1 | 0
        let opcode = 0x7C64_2670u32;
        
        interpreter.memory.write_be32(0x2000_0000, opcode).unwrap();
        interpreter.step(&mut thread).unwrap();
        
        // Result should be 0x0100_0000 (16,777,216)
        assert_eq!(thread.gpr(4), 0x0100_0000u64);
        // CA should be 0 (positive value, no bits shifted out)
        assert!(!thread.get_xer_ca());
    }

    #[test]
    fn test_srawi_negative() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Setup: negative value (sign-extended 32-bit)
        thread.set_gpr(3, 0xFFFF_FFFF_F000_0010u64); // -268,435,440 as 64-bit sign-extended
        
        // srawi r4, r3, 4 - shift right by 4 bits
        let opcode = 0x7C64_2670u32;
        
        interpreter.memory.write_be32(0x2000_0000, opcode).unwrap();
        interpreter.step(&mut thread).unwrap();
        
        // The low 32 bits are 0xF000_0010, shifted right 4 = 0xFF00_0001
        // Sign-extended to 64-bit: 0xFFFF_FFFF_FF00_0001
        assert_eq!(thread.gpr(4), 0xFFFF_FFFF_FF00_0001u64);
        // CA should be 0 because no 1-bits were shifted out (low 4 bits of 0xF0000010 are 0)
        assert!(!thread.get_xer_ca());
    }

    #[test]
    fn test_srawi_with_carry() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Setup: negative value with 1-bits in low positions
        thread.set_gpr(3, 0xFFFF_FFFF_F000_000Fu64); // negative, low 4 bits are 1s
        
        // srawi r4, r3, 4 - shift right by 4 bits
        let opcode = 0x7C64_2670u32;
        
        interpreter.memory.write_be32(0x2000_0000, opcode).unwrap();
        interpreter.step(&mut thread).unwrap();
        
        // Result: 0xFFFF_FFFF_FF00_0000 (sign-extended)
        assert_eq!(thread.gpr(4), 0xFFFF_FFFF_FF00_0000u64);
        // CA should be 1 because negative and 1-bits were shifted out
        assert!(thread.get_xer_ca());
    }

    #[test]
    fn test_sradi_positive() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Setup: positive 64-bit value
        thread.set_gpr(3, 0x1000_0000_0000_0000u64);
        
        // sradi r4, r3, 4 - shift right by 4 bits
        // XS-form: op=31, rs=3, ra=4, sh[0:4]=4, xo=413, sh[5]=0, rc=0
        // The encoding is: 31 << 26 | 3 << 21 | 4 << 16 | 4 << 11 | 413 << 2 | 0 << 1 | 0
        let opcode = 0x7C64_2674u32;
        
        interpreter.memory.write_be32(0x2000_0000, opcode).unwrap();
        interpreter.step(&mut thread).unwrap();
        
        // Result should be 0x0100_0000_0000_0000
        assert_eq!(thread.gpr(4), 0x0100_0000_0000_0000u64);
        // CA should be 0 (positive value)
        assert!(!thread.get_xer_ca());
    }

    #[test]
    fn test_sradi_negative() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Setup: negative 64-bit value
        thread.set_gpr(3, 0xF000_0000_0000_0010u64);
        
        // sradi r4, r3, 4 - shift right by 4 bits  
        let opcode = 0x7C64_2674u32;
        
        interpreter.memory.write_be32(0x2000_0000, opcode).unwrap();
        interpreter.step(&mut thread).unwrap();
        
        // Result should be 0xFF00_0000_0000_0001 (sign extended)
        assert_eq!(thread.gpr(4), 0xFF00_0000_0000_0001u64);
        // CA should be 0 (low 4 bits of source were 0)
        assert!(!thread.get_xer_ca());
    }

    #[test]
    fn test_sradi_with_carry() {
        let (interpreter, mut thread) = create_test_env();
        thread.set_pc(0x2000_0000);
        
        // Setup: negative 64-bit value with 1-bits in low positions
        thread.set_gpr(3, 0xF000_0000_0000_000Fu64);
        
        // sradi r4, r3, 4 - shift right by 4 bits
        let opcode = 0x7C64_2674u32;
        
        interpreter.memory.write_be32(0x2000_0000, opcode).unwrap();
        interpreter.step(&mut thread).unwrap();
        
        // Result should be 0xFF00_0000_0000_0000 (sign extended)
        assert_eq!(thread.gpr(4), 0xFF00_0000_0000_0000u64);
        // CA should be 1 (negative and 1-bits shifted out)
        assert!(thread.get_xer_ca());
    }
}
