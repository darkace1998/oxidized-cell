//! JIT interface
//!
//! This module provides the FFI interface to the C++ JIT compilers for both PPU and SPU.
//! It includes support for JIT invocation, code cache management, breakpoint integration,
//! branch prediction, inline caching, register allocation, lazy compilation, and multi-threaded compilation.

/// Opaque handle to PPU JIT compiler
#[repr(C)]
pub struct PpuJit {
    _private: [u8; 0],
}

/// Opaque handle to SPU JIT compiler
#[repr(C)]
pub struct SpuJit {
    _private: [u8; 0],
}

/// Opaque handle to RSX shader compiler
#[repr(C)]
pub struct RsxShader {
    _private: [u8; 0],
}

/// PPU execution context structure
/// 
/// This structure holds the complete PPU state and is passed to JIT-compiled
/// code for reading and writing registers. Matches the C++ `oc_ppu_context_t`.
#[repr(C)]
#[derive(Clone)]
pub struct PpuContext {
    /// General Purpose Registers (64-bit)
    pub gpr: [u64; 32],
    
    /// Floating Point Registers (64-bit IEEE double)
    pub fpr: [f64; 32],
    
    /// Vector Registers (128-bit, stored as 4 x u32)
    pub vr: [[u32; 4]; 32],
    
    /// Condition Register (32-bit)
    pub cr: u32,
    
    /// Link Register (64-bit)
    pub lr: u64,
    
    /// Count Register (64-bit)
    pub ctr: u64,
    
    /// Fixed-Point Exception Register (64-bit)
    pub xer: u64,
    
    /// Floating-Point Status and Control Register (64-bit)
    pub fpscr: u64,
    
    /// Vector Status and Control Register (32-bit)
    pub vscr: u32,
    
    /// Program Counter / Current Instruction Address (64-bit)
    pub pc: u64,
    
    /// Machine State Register (64-bit)
    pub msr: u64,
    
    /// Next instruction address after block execution
    pub next_pc: u64,
    
    /// Number of instructions executed in this block
    pub instructions_executed: u32,
    
    /// Execution result/status
    /// 0 = normal, 1 = branch, 2 = syscall, 3 = breakpoint, 4 = error
    pub exit_reason: i32,
    
    /// Memory base pointer (set before execution)
    pub memory_base: *mut u8,
    
    /// Memory size (for bounds checking in debug builds)
    pub memory_size: u64,
}

impl Default for PpuContext {
    fn default() -> Self {
        Self {
            gpr: [0; 32],
            fpr: [0.0; 32],
            vr: [[0; 4]; 32],
            cr: 0,
            lr: 0,
            ctr: 0,
            xer: 0,
            fpscr: 0,
            vscr: 0,
            pc: 0,
            msr: 0x8000_0000_0000_0000, // 64-bit mode
            next_pc: 0,
            instructions_executed: 0,
            exit_reason: 0,
            memory_base: std::ptr::null_mut(),
            memory_size: 0,
        }
    }
}

/// Exit reason codes from JIT execution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum PpuExitReason {
    /// Block completed normally
    Normal = 0,
    /// Block ended with branch
    Branch = 1,
    /// System call encountered
    Syscall = 2,
    /// Breakpoint hit
    Breakpoint = 3,
    /// Execution error
    Error = 4,
}

impl From<i32> for PpuExitReason {
    fn from(value: i32) -> Self {
        match value {
            0 => PpuExitReason::Normal,
            1 => PpuExitReason::Branch,
            2 => PpuExitReason::Syscall,
            3 => PpuExitReason::Breakpoint,
            _ => PpuExitReason::Error,
        }
    }
}

/// SPU execution context structure
/// 
/// This structure holds the complete SPU state and is passed to JIT-compiled
/// code for reading and writing registers. The SPU has 128 128-bit registers.
/// Matches the C++ `oc_spu_context_t`.
#[repr(C)]
#[derive(Clone)]
pub struct SpuContext {
    /// 128 vector registers (128-bit each, stored as 4 x u32)
    pub gpr: [[u32; 4]; 128],
    
    /// SPU PC (Local Store address, 18 bits used, within 256KB)
    pub pc: u32,
    
    /// Link Register (for BRSL/BRASL)
    pub lr: u32,
    
    /// Next PC after block execution
    pub next_pc: u32,
    
    /// SPU Status Register (for stop instruction status)
    pub status: u32,
    
    /// Channel count register values (for rchcnt instruction)
    pub channel_count: [u32; 32],
    
    /// Number of instructions executed in this block
    pub instructions_executed: u32,
    
    /// Execution result/status
    /// 0 = normal, 1 = branch, 2 = stop, 3 = breakpoint, 4 = error
    pub exit_reason: i32,
    
    /// Local Storage base pointer (256KB SPU local memory)
    pub local_storage: *mut u8,
    
    /// Local Storage size (256KB)
    pub local_storage_size: u32,
    
    /// SPU ID (0-7 for Cell's SPUs)
    pub spu_id: u8,
    
    /// Decrementer value
    pub decrementer: u32,
    
    /// MFC tag mask for DMA completion
    pub mfc_tag_mask: u32,
    
    /// Padding for alignment
    _padding: [u8; 3],
}

impl Default for SpuContext {
    fn default() -> Self {
        Self {
            gpr: [[0; 4]; 128],
            pc: 0,
            lr: 0,
            next_pc: 0,
            status: 0,
            channel_count: [0; 32],
            instructions_executed: 0,
            exit_reason: 0,
            local_storage: std::ptr::null_mut(),
            local_storage_size: 0x40000, // 256KB
            spu_id: 0,
            decrementer: 0,
            mfc_tag_mask: 0,
            _padding: [0; 3],
        }
    }
}

/// Exit reason codes from SPU JIT execution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum SpuExitReason {
    /// Block completed normally
    Normal = 0,
    /// Block ended with branch
    Branch = 1,
    /// Stop instruction encountered
    Stop = 2,
    /// Breakpoint hit
    Breakpoint = 3,
    /// Execution error
    Error = 4,
}

impl From<i32> for SpuExitReason {
    fn from(value: i32) -> Self {
        match value {
            0 => SpuExitReason::Normal,
            1 => SpuExitReason::Branch,
            2 => SpuExitReason::Stop,
            3 => SpuExitReason::Breakpoint,
            _ => SpuExitReason::Error,
        }
    }
}

// FFI declarations for PPU JIT
extern "C" {
    fn oc_ppu_jit_create() -> *mut PpuJit;
    fn oc_ppu_jit_destroy(jit: *mut PpuJit);
    fn oc_ppu_jit_compile(jit: *mut PpuJit, address: u32, code: *const u8, size: usize) -> i32;
    fn oc_ppu_jit_get_compiled(jit: *mut PpuJit, address: u32) -> *mut u8;
    fn oc_ppu_jit_invalidate(jit: *mut PpuJit, address: u32);
    fn oc_ppu_jit_clear_cache(jit: *mut PpuJit);
    fn oc_ppu_jit_add_breakpoint(jit: *mut PpuJit, address: u32);
    fn oc_ppu_jit_remove_breakpoint(jit: *mut PpuJit, address: u32);
    fn oc_ppu_jit_has_breakpoint(jit: *mut PpuJit, address: u32) -> i32;
    
    // Branch prediction APIs
    fn oc_ppu_jit_add_branch_hint(jit: *mut PpuJit, address: u32, target: u32, hint: i32);
    fn oc_ppu_jit_predict_branch(jit: *mut PpuJit, address: u32) -> i32;
    fn oc_ppu_jit_update_branch(jit: *mut PpuJit, address: u32, taken: i32);
    
    // Inline cache APIs
    fn oc_ppu_jit_add_inline_cache(jit: *mut PpuJit, call_site: u32, target: u32);
    fn oc_ppu_jit_lookup_inline_cache(jit: *mut PpuJit, call_site: u32) -> *mut u8;
    fn oc_ppu_jit_invalidate_inline_cache(jit: *mut PpuJit, target: u32);
    
    // Register allocation APIs
    fn oc_ppu_jit_analyze_registers(jit: *mut PpuJit, address: u32, instructions: *const u32, count: usize);
    fn oc_ppu_jit_get_reg_hint(jit: *mut PpuJit, address: u32, reg: u8) -> i32;
    fn oc_ppu_jit_get_live_gprs(jit: *mut PpuJit, address: u32) -> u32;
    fn oc_ppu_jit_get_modified_gprs(jit: *mut PpuJit, address: u32) -> u32;
    
    // Lazy compilation APIs
    fn oc_ppu_jit_enable_lazy(jit: *mut PpuJit, enable: i32);
    fn oc_ppu_jit_is_lazy_enabled(jit: *mut PpuJit) -> i32;
    fn oc_ppu_jit_register_lazy(jit: *mut PpuJit, address: u32, code: *const u8, size: usize, threshold: u32);
    fn oc_ppu_jit_should_compile_lazy(jit: *mut PpuJit, address: u32) -> i32;
    fn oc_ppu_jit_get_lazy_state(jit: *mut PpuJit, address: u32) -> i32;
    
    // Multi-threaded compilation APIs
    fn oc_ppu_jit_start_compile_threads(jit: *mut PpuJit, num_threads: usize);
    fn oc_ppu_jit_stop_compile_threads(jit: *mut PpuJit);
    fn oc_ppu_jit_submit_compile_task(jit: *mut PpuJit, address: u32, code: *const u8, size: usize, priority: i32);
    fn oc_ppu_jit_get_pending_tasks(jit: *mut PpuJit) -> usize;
    fn oc_ppu_jit_get_completed_tasks(jit: *mut PpuJit) -> usize;
    fn oc_ppu_jit_is_multithreaded(jit: *mut PpuJit) -> i32;
    
    // Execution APIs
    fn oc_ppu_jit_execute(jit: *mut PpuJit, context: *mut PpuContext, address: u32) -> i32;
    fn oc_ppu_jit_execute_block(jit: *mut PpuJit, context: *mut PpuContext, address: u32) -> i32;
    
    // Block linking APIs
    fn oc_ppu_jit_link_add(jit: *mut PpuJit, source: u32, target: u32, conditional: i32);
    fn oc_ppu_jit_link_blocks(jit: *mut PpuJit, source: u32, target: u32) -> i32;
    fn oc_ppu_jit_unlink_source(jit: *mut PpuJit, source: u32);
    fn oc_ppu_jit_unlink_target(jit: *mut PpuJit, target: u32);
    fn oc_ppu_jit_link_get_target(jit: *mut PpuJit, source: u32, target: u32) -> *mut u8;
    fn oc_ppu_jit_link_record_hit(jit: *mut PpuJit);
    fn oc_ppu_jit_link_record_miss(jit: *mut PpuJit);
    fn oc_ppu_jit_link_get_count(jit: *mut PpuJit) -> usize;
    fn oc_ppu_jit_link_get_active(jit: *mut PpuJit) -> usize;
    fn oc_ppu_jit_link_clear(jit: *mut PpuJit);
    
    // Trace compilation APIs
    fn oc_ppu_jit_trace_set_hot_threshold(jit: *mut PpuJit, threshold: u64);
    fn oc_ppu_jit_trace_get_hot_threshold(jit: *mut PpuJit) -> u64;
    fn oc_ppu_jit_trace_set_max_length(jit: *mut PpuJit, length: usize);
    fn oc_ppu_jit_trace_detect(jit: *mut PpuJit, header: u32, block_addrs: *const u32, count: usize, back_edge: u32);
    fn oc_ppu_jit_trace_record_execution(jit: *mut PpuJit, header: u32) -> i32;
    #[allow(dead_code)]
    fn oc_ppu_jit_trace_mark_compiled(jit: *mut PpuJit, header: u32, code: *mut u8);
    fn oc_ppu_jit_trace_get_compiled(jit: *mut PpuJit, header: u32) -> *mut u8;
    fn oc_ppu_jit_trace_is_header(jit: *mut PpuJit, address: u32) -> i32;
    fn oc_ppu_jit_trace_clear(jit: *mut PpuJit);
    
    // Code verification API
    fn oc_ppu_jit_verify_codegen(jit: *mut PpuJit) -> i32;
}

// FFI declarations for SPU JIT
extern "C" {
    fn oc_spu_jit_create() -> *mut SpuJit;
    fn oc_spu_jit_destroy(jit: *mut SpuJit);
    fn oc_spu_jit_compile(jit: *mut SpuJit, address: u32, code: *const u8, size: usize) -> i32;
    fn oc_spu_jit_get_compiled(jit: *mut SpuJit, address: u32) -> *mut u8;
    fn oc_spu_jit_invalidate(jit: *mut SpuJit, address: u32);
    fn oc_spu_jit_clear_cache(jit: *mut SpuJit);
    fn oc_spu_jit_add_breakpoint(jit: *mut SpuJit, address: u32);
    fn oc_spu_jit_remove_breakpoint(jit: *mut SpuJit, address: u32);
    fn oc_spu_jit_has_breakpoint(jit: *mut SpuJit, address: u32) -> i32;
    
    // Channel operations APIs
    fn oc_spu_jit_enable_channel_ops(jit: *mut SpuJit, enable: i32);
    fn oc_spu_jit_is_channel_ops_enabled(jit: *mut SpuJit) -> i32;
    fn oc_spu_jit_register_channel_op(jit: *mut SpuJit, channel: u8, is_read: i32, address: u32, reg: u8);
    #[allow(dead_code)]
    fn oc_spu_jit_set_channel_callbacks(jit: *mut SpuJit, read_callback: *mut u8, write_callback: *mut u8);
    fn oc_spu_jit_get_channel_op_count(jit: *mut SpuJit) -> usize;
    
    // MFC DMA APIs
    fn oc_spu_jit_enable_mfc_dma(jit: *mut SpuJit, enable: i32);
    fn oc_spu_jit_is_mfc_dma_enabled(jit: *mut SpuJit) -> i32;
    fn oc_spu_jit_queue_dma(jit: *mut SpuJit, local_addr: u32, ea: u64, size: u32, tag: u16, cmd: u8);
    fn oc_spu_jit_get_pending_dma_count(jit: *mut SpuJit) -> usize;
    fn oc_spu_jit_get_pending_dma_for_tag(jit: *mut SpuJit, tag: u16) -> usize;
    fn oc_spu_jit_complete_dma_tag(jit: *mut SpuJit, tag: u16);
    #[allow(dead_code)]
    fn oc_spu_jit_set_dma_callback(jit: *mut SpuJit, callback: *mut u8);
    
    // Loop optimization APIs
    fn oc_spu_jit_enable_loop_opt(jit: *mut SpuJit, enable: i32);
    fn oc_spu_jit_is_loop_opt_enabled(jit: *mut SpuJit) -> i32;
    fn oc_spu_jit_detect_loop(jit: *mut SpuJit, header: u32, back_edge: u32, exit: u32);
    fn oc_spu_jit_set_loop_count(jit: *mut SpuJit, header: u32, count: u32);
    fn oc_spu_jit_set_loop_vectorizable(jit: *mut SpuJit, header: u32, vectorizable: i32);
    fn oc_spu_jit_is_in_loop(jit: *mut SpuJit, address: u32) -> i32;
    fn oc_spu_jit_get_loop_info(jit: *mut SpuJit, header: u32, back_edge: *mut u32, exit: *mut u32, iteration_count: *mut u32, is_vectorizable: *mut i32) -> i32;
    
    // SIMD intrinsics APIs
    fn oc_spu_jit_enable_simd_intrinsics(jit: *mut SpuJit, enable: i32);
    fn oc_spu_jit_is_simd_intrinsics_enabled(jit: *mut SpuJit) -> i32;
    fn oc_spu_jit_get_simd_intrinsic(jit: *mut SpuJit, opcode: u32) -> i32;
    fn oc_spu_jit_has_simd_intrinsic(jit: *mut SpuJit, opcode: u32) -> i32;
    
    // SPU-to-SPU Mailbox Fast Path APIs
    fn oc_spu_jit_mailbox_send(jit: *mut SpuJit, src_spu: u8, dst_spu: u8, value: u32) -> i32;
    fn oc_spu_jit_mailbox_receive(jit: *mut SpuJit, src_spu: u8, dst_spu: u8, value: *mut u32) -> i32;
    fn oc_spu_jit_mailbox_pending(jit: *mut SpuJit, src_spu: u8, dst_spu: u8) -> u32;
    fn oc_spu_jit_mailbox_reset(jit: *mut SpuJit);
    fn oc_spu_jit_mailbox_get_stats(jit: *mut SpuJit, total_sends: *mut u64, total_receives: *mut u64, send_blocked: *mut u64, receive_blocked: *mut u64);
    
    // Loop-Aware Block Merging API
    fn oc_spu_jit_merge_loop_blocks(jit: *mut SpuJit, loop_header: u32, back_edge_addr: u32, body_addresses: *const u32, body_count: usize) -> i32;
}

// FFI declarations for RSX Shader Compiler
extern "C" {
    fn oc_rsx_shader_create() -> *mut RsxShader;
    fn oc_rsx_shader_destroy(shader: *mut RsxShader);
    fn oc_rsx_shader_compile_vertex(shader: *mut RsxShader, code: *const u32, size: usize, out_spirv: *mut *mut u32, out_size: *mut usize) -> i32;
    fn oc_rsx_shader_compile_fragment(shader: *mut RsxShader, code: *const u32, size: usize, out_spirv: *mut *mut u32, out_size: *mut usize) -> i32;
    fn oc_rsx_shader_free_spirv(spirv: *mut u32);
    fn oc_rsx_shader_link(shader: *mut RsxShader, vs_spirv: *const u32, vs_size: usize, fs_spirv: *const u32, fs_size: usize) -> i32;
    fn oc_rsx_shader_get_linked_count(shader: *mut RsxShader) -> usize;
    #[allow(dead_code)]
    fn oc_rsx_shader_set_pipeline_callbacks(shader: *mut RsxShader, create_callback: *mut u8, destroy_callback: *mut u8);
    fn oc_rsx_shader_get_pipeline(shader: *mut RsxShader, vs_hash: u64, fs_hash: u64, vertex_mask: u32, cull_mode: u8, blend_enable: u8) -> *mut u8;
    fn oc_rsx_shader_advance_frame(shader: *mut RsxShader);
    fn oc_rsx_shader_get_pipeline_count(shader: *mut RsxShader) -> usize;
    fn oc_rsx_shader_clear_caches(shader: *mut RsxShader);
    fn oc_rsx_shader_get_vertex_cache_count(shader: *mut RsxShader) -> usize;
    fn oc_rsx_shader_get_fragment_cache_count(shader: *mut RsxShader) -> usize;
}

/// Branch prediction hint types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum BranchHint {
    None = 0,
    Likely = 1,
    Unlikely = 2,
    Static = 3,
}

/// Register allocation hint types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum RegAllocHint {
    None = 0,
    Caller = 1,
    Callee = 2,
    Float = 3,
    Vector = 4,
}

/// Lazy compilation state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum LazyState {
    NotCompiled = 0,
    Pending = 1,
    Compiling = 2,
    Compiled = 3,
    Failed = 4,
}

/// Safe wrapper for PPU JIT compiler
pub struct PpuJitCompiler {
    handle: *mut PpuJit,
}

impl PpuJitCompiler {
    /// Create a new PPU JIT compiler
    pub fn new() -> Option<Self> {
        let handle = unsafe { oc_ppu_jit_create() };
        if handle.is_null() {
            None
        } else {
            Some(Self { handle })
        }
    }

    /// Compile a PPU code block starting at the given address
    pub fn compile(&mut self, address: u32, code: &[u8]) -> Result<(), JitError> {
        let result = unsafe {
            oc_ppu_jit_compile(self.handle, address, code.as_ptr(), code.len())
        };
        
        if result == 0 {
            Ok(())
        } else {
            Err(JitError::from_error_code(result))
        }
    }

    /// Get compiled code for a given address
    /// 
    /// # Safety
    /// Returns a raw pointer to compiled machine code. The pointer is valid as long as:
    /// - The JIT compiler instance is alive
    /// - The code at this address has not been invalidated
    /// - No cache clear operation has been performed
    /// 
    /// Calling compiled code directly requires understanding the calling convention
    /// and ensuring proper register state.
    pub fn get_compiled(&self, address: u32) -> Option<*mut u8> {
        let ptr = unsafe { oc_ppu_jit_get_compiled(self.handle, address) };
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }

    /// Invalidate compiled code at a specific address
    pub fn invalidate(&mut self, address: u32) {
        unsafe { oc_ppu_jit_invalidate(self.handle, address) }
    }

    /// Clear the entire code cache
    pub fn clear_cache(&mut self) {
        unsafe { oc_ppu_jit_clear_cache(self.handle) }
    }

    /// Add a breakpoint at the specified address
    pub fn add_breakpoint(&mut self, address: u32) {
        unsafe { oc_ppu_jit_add_breakpoint(self.handle, address) }
    }

    /// Remove a breakpoint at the specified address
    pub fn remove_breakpoint(&mut self, address: u32) {
        unsafe { oc_ppu_jit_remove_breakpoint(self.handle, address) }
    }

    /// Check if a breakpoint exists at the specified address
    pub fn has_breakpoint(&self, address: u32) -> bool {
        unsafe { oc_ppu_jit_has_breakpoint(self.handle, address) != 0 }
    }
    
    // ========================================================================
    // Branch Prediction APIs
    // ========================================================================
    
    /// Add a branch prediction hint
    pub fn add_branch_hint(&mut self, address: u32, target: u32, hint: BranchHint) {
        unsafe { oc_ppu_jit_add_branch_hint(self.handle, address, target, hint as i32) }
    }
    
    /// Predict branch direction (true = taken, false = not taken)
    pub fn predict_branch(&self, address: u32) -> bool {
        unsafe { oc_ppu_jit_predict_branch(self.handle, address) != 0 }
    }
    
    /// Update branch prediction based on actual behavior
    pub fn update_branch(&mut self, address: u32, taken: bool) {
        unsafe { oc_ppu_jit_update_branch(self.handle, address, if taken { 1 } else { 0 }) }
    }
    
    // ========================================================================
    // Inline Cache APIs
    // ========================================================================
    
    /// Add an inline cache entry for a call site
    pub fn add_inline_cache(&mut self, call_site: u32, target: u32) {
        unsafe { oc_ppu_jit_add_inline_cache(self.handle, call_site, target) }
    }
    
    /// Lookup cached compiled code for a call site
    pub fn lookup_inline_cache(&self, call_site: u32) -> Option<*mut u8> {
        let ptr = unsafe { oc_ppu_jit_lookup_inline_cache(self.handle, call_site) };
        if ptr.is_null() { None } else { Some(ptr) }
    }
    
    /// Invalidate inline cache entries for a target address
    pub fn invalidate_inline_cache(&mut self, target: u32) {
        unsafe { oc_ppu_jit_invalidate_inline_cache(self.handle, target) }
    }
    
    // ========================================================================
    // Register Allocation APIs
    // ========================================================================
    
    /// Analyze register usage in a basic block
    pub fn analyze_registers(&mut self, address: u32, instructions: &[u32]) {
        unsafe {
            oc_ppu_jit_analyze_registers(
                self.handle, address, instructions.as_ptr(), instructions.len()
            )
        }
    }
    
    /// Get register allocation hint
    pub fn get_reg_hint(&self, address: u32, reg: u8) -> RegAllocHint {
        let hint = unsafe { oc_ppu_jit_get_reg_hint(self.handle, address, reg) };
        match hint {
            1 => RegAllocHint::Caller,
            2 => RegAllocHint::Callee,
            3 => RegAllocHint::Float,
            4 => RegAllocHint::Vector,
            _ => RegAllocHint::None,
        }
    }
    
    /// Get live GPR mask for a block
    pub fn get_live_gprs(&self, address: u32) -> u32 {
        unsafe { oc_ppu_jit_get_live_gprs(self.handle, address) }
    }
    
    /// Get modified GPR mask for a block
    pub fn get_modified_gprs(&self, address: u32) -> u32 {
        unsafe { oc_ppu_jit_get_modified_gprs(self.handle, address) }
    }
    
    // ========================================================================
    // Lazy Compilation APIs
    // ========================================================================
    
    /// Enable or disable lazy compilation
    pub fn enable_lazy(&mut self, enable: bool) {
        unsafe { oc_ppu_jit_enable_lazy(self.handle, if enable { 1 } else { 0 }) }
    }
    
    /// Check if lazy compilation is enabled
    pub fn is_lazy_enabled(&self) -> bool {
        unsafe { oc_ppu_jit_is_lazy_enabled(self.handle) != 0 }
    }
    
    /// Register code for lazy compilation
    pub fn register_lazy(&mut self, address: u32, code: &[u8], threshold: u32) {
        unsafe {
            oc_ppu_jit_register_lazy(
                self.handle, address, code.as_ptr(), code.len(), threshold
            )
        }
    }
    
    /// Check if code should be compiled (based on execution count)
    pub fn should_compile_lazy(&self, address: u32) -> bool {
        unsafe { oc_ppu_jit_should_compile_lazy(self.handle, address) != 0 }
    }
    
    /// Get lazy compilation state
    pub fn get_lazy_state(&self, address: u32) -> LazyState {
        let state = unsafe { oc_ppu_jit_get_lazy_state(self.handle, address) };
        match state {
            1 => LazyState::Pending,
            2 => LazyState::Compiling,
            3 => LazyState::Compiled,
            4 => LazyState::Failed,
            _ => LazyState::NotCompiled,
        }
    }
    
    // ========================================================================
    // Multi-threaded Compilation APIs
    // ========================================================================
    
    /// Start compilation thread pool
    pub fn start_compile_threads(&mut self, num_threads: usize) {
        unsafe { oc_ppu_jit_start_compile_threads(self.handle, num_threads) }
    }
    
    /// Stop compilation thread pool
    pub fn stop_compile_threads(&mut self) {
        unsafe { oc_ppu_jit_stop_compile_threads(self.handle) }
    }
    
    /// Submit a compilation task
    pub fn submit_compile_task(&mut self, address: u32, code: &[u8], priority: i32) {
        unsafe {
            oc_ppu_jit_submit_compile_task(
                self.handle, address, code.as_ptr(), code.len(), priority
            )
        }
    }
    
    /// Get number of pending compilation tasks
    pub fn get_pending_tasks(&self) -> usize {
        unsafe { oc_ppu_jit_get_pending_tasks(self.handle) }
    }
    
    /// Get number of completed compilation tasks
    pub fn get_completed_tasks(&self) -> usize {
        unsafe { oc_ppu_jit_get_completed_tasks(self.handle) }
    }
    
    /// Check if multi-threaded compilation is enabled
    pub fn is_multithreaded(&self) -> bool {
        unsafe { oc_ppu_jit_is_multithreaded(self.handle) != 0 }
    }
    
    // ========================================================================
    // Execution APIs
    // ========================================================================
    
    /// Execute JIT-compiled code at the given address
    /// 
    /// This executes a compiled basic block, reading and writing registers
    /// through the provided context. The context should be populated with
    /// the current PPU state before calling, and will contain the updated
    /// state after execution.
    /// 
    /// # Arguments
    /// * `context` - PPU context with register state and memory pointer
    /// * `address` - Address of the compiled block to execute
    /// 
    /// # Returns
    /// * `Ok(count)` - Number of instructions executed
    /// * `Err(reason)` - Execution failed or interrupted
    pub fn execute(&mut self, context: &mut PpuContext, address: u32) -> Result<u32, PpuExitReason> {
        let result = unsafe { oc_ppu_jit_execute(self.handle, context, address) };
        
        if result < 0 {
            return Err(PpuExitReason::from(context.exit_reason));
        }
        
        let exit_reason = PpuExitReason::from(context.exit_reason);
        match exit_reason {
            PpuExitReason::Normal | PpuExitReason::Branch => Ok(result as u32),
            _ => Err(exit_reason),
        }
    }
    
    /// Execute a single JIT block (does not follow branches)
    /// 
    /// Similar to `execute`, but only executes one basic block without
    /// following any branches. Useful for step-through debugging.
    pub fn execute_block(&mut self, context: &mut PpuContext, address: u32) -> Result<u32, PpuExitReason> {
        let result = unsafe { oc_ppu_jit_execute_block(self.handle, context, address) };
        
        if result < 0 {
            return Err(PpuExitReason::from(context.exit_reason));
        }
        
        let exit_reason = PpuExitReason::from(context.exit_reason);
        match exit_reason {
            PpuExitReason::Normal | PpuExitReason::Branch => Ok(result as u32),
            _ => Err(exit_reason),
        }
    }

    // ========== Block Linking APIs ==========

    /// Register a potential link between two compiled blocks
    pub fn link_add(&mut self, source: u32, target: u32, conditional: bool) {
        unsafe { oc_ppu_jit_link_add(self.handle, source, target, conditional as i32) }
    }

    /// Activate a link: patch source block to jump directly to target
    pub fn link_blocks(&mut self, source: u32, target: u32) -> bool {
        unsafe { oc_ppu_jit_link_blocks(self.handle, source, target) != 0 }
    }

    /// Unlink all outgoing links from a source block
    pub fn unlink_source(&mut self, source: u32) {
        unsafe { oc_ppu_jit_unlink_source(self.handle, source) }
    }

    /// Unlink all incoming links to a target block
    pub fn unlink_target(&mut self, target: u32) {
        unsafe { oc_ppu_jit_unlink_target(self.handle, target) }
    }

    /// Get the linked native code pointer for a source→target edge
    pub fn link_get_target(&self, source: u32, target: u32) -> Option<*mut u8> {
        let ptr = unsafe { oc_ppu_jit_link_get_target(self.handle, source, target) };
        if ptr.is_null() { None } else { Some(ptr) }
    }

    /// Record a block link hit (direct jump taken)
    pub fn link_record_hit(&mut self) {
        unsafe { oc_ppu_jit_link_record_hit(self.handle) }
    }

    /// Record a block link miss (fell back to dispatcher)
    pub fn link_record_miss(&mut self) {
        unsafe { oc_ppu_jit_link_record_miss(self.handle) }
    }

    /// Get total link count
    pub fn link_get_count(&self) -> usize {
        unsafe { oc_ppu_jit_link_get_count(self.handle) }
    }

    /// Get active link count
    pub fn link_get_active(&self) -> usize {
        unsafe { oc_ppu_jit_link_get_active(self.handle) }
    }

    /// Clear all block links
    pub fn link_clear(&mut self) {
        unsafe { oc_ppu_jit_link_clear(self.handle) }
    }

    // ========== Trace Compilation APIs ==========

    /// Set execution count threshold for trace compilation
    pub fn trace_set_hot_threshold(&mut self, threshold: u64) {
        unsafe { oc_ppu_jit_trace_set_hot_threshold(self.handle, threshold) }
    }

    /// Get trace hot threshold
    pub fn trace_get_hot_threshold(&self) -> u64 {
        unsafe { oc_ppu_jit_trace_get_hot_threshold(self.handle) }
    }

    /// Set maximum trace length (number of blocks)
    pub fn trace_set_max_length(&mut self, length: usize) {
        unsafe { oc_ppu_jit_trace_set_max_length(self.handle, length) }
    }

    /// Detect a trace (hot path) starting at the given header
    pub fn trace_detect(&mut self, header: u32, block_addrs: &[u32], back_edge: u32) {
        unsafe {
            oc_ppu_jit_trace_detect(self.handle, header, block_addrs.as_ptr(),
                                     block_addrs.len(), back_edge)
        }
    }

    /// Record trace execution, returns true if trace should be compiled
    pub fn trace_record_execution(&mut self, header: u32) -> bool {
        unsafe { oc_ppu_jit_trace_record_execution(self.handle, header) != 0 }
    }

    /// Check if an address is a trace header
    pub fn trace_is_header(&self, address: u32) -> bool {
        unsafe { oc_ppu_jit_trace_is_header(self.handle, address) != 0 }
    }

    /// Get compiled trace code
    pub fn trace_get_compiled(&self, header: u32) -> Option<*mut u8> {
        let ptr = unsafe { oc_ppu_jit_trace_get_compiled(self.handle, header) };
        if ptr.is_null() { None } else { Some(ptr) }
    }

    /// Clear all traces
    pub fn trace_clear(&mut self) {
        unsafe { oc_ppu_jit_trace_clear(self.handle) }
    }

    // ========== Code Verification API ==========

    /// Verify JIT code generation produces valid machine code
    pub fn verify_codegen(&mut self) -> bool {
        unsafe { oc_ppu_jit_verify_codegen(self.handle) == 1 }
    }
}

impl Drop for PpuJitCompiler {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { oc_ppu_jit_destroy(self.handle) }
        }
    }
}

unsafe impl Send for PpuJitCompiler {}

/// Safe wrapper for SPU JIT compiler
pub struct SpuJitCompiler {
    handle: *mut SpuJit,
}

impl SpuJitCompiler {
    /// Create a new SPU JIT compiler
    pub fn new() -> Option<Self> {
        let handle = unsafe { oc_spu_jit_create() };
        if handle.is_null() {
            None
        } else {
            Some(Self { handle })
        }
    }

    /// Compile an SPU code block starting at the given address
    pub fn compile(&mut self, address: u32, code: &[u8]) -> Result<(), JitError> {
        let result = unsafe {
            oc_spu_jit_compile(self.handle, address, code.as_ptr(), code.len())
        };
        
        if result == 0 {
            Ok(())
        } else {
            Err(JitError::from_error_code(result))
        }
    }

    /// Get compiled code for a given address
    /// 
    /// # Safety
    /// Returns a raw pointer to compiled machine code. The pointer is valid as long as:
    /// - The JIT compiler instance is alive
    /// - The code at this address has not been invalidated
    /// - No cache clear operation has been performed
    /// 
    /// Calling compiled code directly requires understanding the SPU calling convention
    /// and ensuring proper register state.
    pub fn get_compiled(&self, address: u32) -> Option<*mut u8> {
        let ptr = unsafe { oc_spu_jit_get_compiled(self.handle, address) };
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }

    /// Invalidate compiled code at a specific address
    pub fn invalidate(&mut self, address: u32) {
        unsafe { oc_spu_jit_invalidate(self.handle, address) }
    }

    /// Clear the entire code cache
    pub fn clear_cache(&mut self) {
        unsafe { oc_spu_jit_clear_cache(self.handle) }
    }

    /// Add a breakpoint at the specified address
    pub fn add_breakpoint(&mut self, address: u32) {
        unsafe { oc_spu_jit_add_breakpoint(self.handle, address) }
    }

    /// Remove a breakpoint at the specified address
    pub fn remove_breakpoint(&mut self, address: u32) {
        unsafe { oc_spu_jit_remove_breakpoint(self.handle, address) }
    }

    /// Check if a breakpoint exists at the specified address
    pub fn has_breakpoint(&self, address: u32) -> bool {
        unsafe { oc_spu_jit_has_breakpoint(self.handle, address) != 0 }
    }
    
    // ========================================================================
    // Channel Operations APIs
    // ========================================================================
    
    /// Enable or disable channel operations in JIT
    pub fn enable_channel_ops(&mut self, enable: bool) {
        unsafe { oc_spu_jit_enable_channel_ops(self.handle, if enable { 1 } else { 0 }) }
    }
    
    /// Check if channel operations are enabled
    pub fn is_channel_ops_enabled(&self) -> bool {
        unsafe { oc_spu_jit_is_channel_ops_enabled(self.handle) != 0 }
    }
    
    /// Register a channel operation for JIT compilation
    pub fn register_channel_op(&mut self, channel: u8, is_read: bool, address: u32, reg: u8) {
        unsafe {
            oc_spu_jit_register_channel_op(
                self.handle, channel, if is_read { 1 } else { 0 }, address, reg
            )
        }
    }
    
    /// Get number of registered channel operations
    pub fn get_channel_op_count(&self) -> usize {
        unsafe { oc_spu_jit_get_channel_op_count(self.handle) }
    }
    
    // ========================================================================
    // MFC DMA APIs
    // ========================================================================
    
    /// Enable or disable MFC DMA in JIT
    pub fn enable_mfc_dma(&mut self, enable: bool) {
        unsafe { oc_spu_jit_enable_mfc_dma(self.handle, if enable { 1 } else { 0 }) }
    }
    
    /// Check if MFC DMA is enabled
    pub fn is_mfc_dma_enabled(&self) -> bool {
        unsafe { oc_spu_jit_is_mfc_dma_enabled(self.handle) != 0 }
    }
    
    /// Queue a DMA operation
    pub fn queue_dma(&mut self, local_addr: u32, ea: u64, size: u32, tag: u16, cmd: u8) {
        unsafe { oc_spu_jit_queue_dma(self.handle, local_addr, ea, size, tag, cmd) }
    }
    
    /// Get number of pending DMA operations
    pub fn get_pending_dma_count(&self) -> usize {
        unsafe { oc_spu_jit_get_pending_dma_count(self.handle) }
    }
    
    /// Get number of pending DMA operations for a specific tag
    pub fn get_pending_dma_for_tag(&self, tag: u16) -> usize {
        unsafe { oc_spu_jit_get_pending_dma_for_tag(self.handle, tag) }
    }
    
    /// Mark all DMA operations for a tag as complete
    pub fn complete_dma_tag(&mut self, tag: u16) {
        unsafe { oc_spu_jit_complete_dma_tag(self.handle, tag) }
    }
    
    // ========================================================================
    // Loop Optimization APIs
    // ========================================================================
    
    /// Enable or disable loop optimization
    pub fn enable_loop_opt(&mut self, enable: bool) {
        unsafe { oc_spu_jit_enable_loop_opt(self.handle, if enable { 1 } else { 0 }) }
    }
    
    /// Check if loop optimization is enabled
    pub fn is_loop_opt_enabled(&self) -> bool {
        unsafe { oc_spu_jit_is_loop_opt_enabled(self.handle) != 0 }
    }
    
    /// Detect a loop structure
    pub fn detect_loop(&mut self, header: u32, back_edge: u32, exit: u32) {
        unsafe { oc_spu_jit_detect_loop(self.handle, header, back_edge, exit) }
    }
    
    /// Set loop iteration count (for counted loops)
    pub fn set_loop_count(&mut self, header: u32, count: u32) {
        unsafe { oc_spu_jit_set_loop_count(self.handle, header, count) }
    }
    
    /// Set whether a loop is vectorizable
    pub fn set_loop_vectorizable(&mut self, header: u32, vectorizable: bool) {
        unsafe { oc_spu_jit_set_loop_vectorizable(self.handle, header, if vectorizable { 1 } else { 0 }) }
    }
    
    /// Check if an address is inside a known loop
    pub fn is_in_loop(&self, address: u32) -> bool {
        unsafe { oc_spu_jit_is_in_loop(self.handle, address) != 0 }
    }
    
    /// Get loop information
    pub fn get_loop_info(&self, header: u32) -> Option<LoopInfo> {
        let mut back_edge: u32 = 0;
        let mut exit: u32 = 0;
        let mut iteration_count: u32 = 0;
        let mut is_vectorizable: i32 = 0;
        
        let found = unsafe {
            oc_spu_jit_get_loop_info(
                self.handle, header,
                &mut back_edge, &mut exit,
                &mut iteration_count, &mut is_vectorizable
            )
        };
        
        if found != 0 {
            Some(LoopInfo {
                header,
                back_edge,
                exit,
                iteration_count,
                is_vectorizable: is_vectorizable != 0,
            })
        } else {
            None
        }
    }
    
    // ========================================================================
    // SIMD Intrinsics APIs
    // ========================================================================
    
    /// Enable or disable SIMD intrinsics usage
    pub fn enable_simd_intrinsics(&mut self, enable: bool) {
        unsafe { oc_spu_jit_enable_simd_intrinsics(self.handle, if enable { 1 } else { 0 }) }
    }
    
    /// Check if SIMD intrinsics are enabled
    pub fn is_simd_intrinsics_enabled(&self) -> bool {
        unsafe { oc_spu_jit_is_simd_intrinsics_enabled(self.handle) != 0 }
    }
    
    /// Get SIMD intrinsic for an opcode
    pub fn get_simd_intrinsic(&self, opcode: u32) -> i32 {
        unsafe { oc_spu_jit_get_simd_intrinsic(self.handle, opcode) }
    }
    
    /// Check if opcode has a SIMD intrinsic mapping
    pub fn has_simd_intrinsic(&self, opcode: u32) -> bool {
        unsafe { oc_spu_jit_has_simd_intrinsic(self.handle, opcode) != 0 }
    }
    
    // ========================================================================
    // SPU-to-SPU Mailbox Fast Path
    // ========================================================================
    
    /// Send a value through the SPU-to-SPU mailbox fast path.
    /// Returns true on success, false if the mailbox is full.
    pub fn mailbox_send(&mut self, src_spu: u8, dst_spu: u8, value: u32) -> bool {
        unsafe { oc_spu_jit_mailbox_send(self.handle, src_spu, dst_spu, value) != 0 }
    }
    
    /// Receive a value from the SPU-to-SPU mailbox fast path.
    /// Returns Some(value) on success, None if the mailbox is empty.
    pub fn mailbox_receive(&mut self, src_spu: u8, dst_spu: u8) -> Option<u32> {
        let mut value: u32 = 0;
        let result = unsafe { oc_spu_jit_mailbox_receive(self.handle, src_spu, dst_spu, &mut value) };
        if result != 0 { Some(value) } else { None }
    }
    
    /// Get the number of pending messages in a mailbox slot.
    pub fn mailbox_pending(&self, src_spu: u8, dst_spu: u8) -> u32 {
        unsafe { oc_spu_jit_mailbox_pending(self.handle, src_spu, dst_spu) }
    }
    
    /// Reset all mailbox slots.
    pub fn mailbox_reset(&mut self) {
        unsafe { oc_spu_jit_mailbox_reset(self.handle) }
    }
    
    /// Get mailbox statistics: (total_sends, total_receives, send_blocked, receive_blocked)
    pub fn mailbox_get_stats(&self) -> (u64, u64, u64, u64) {
        let mut sends: u64 = 0;
        let mut receives: u64 = 0;
        let mut send_blocked: u64 = 0;
        let mut receive_blocked: u64 = 0;
        unsafe {
            oc_spu_jit_mailbox_get_stats(
                self.handle, &mut sends, &mut receives, &mut send_blocked, &mut receive_blocked,
            );
        }
        (sends, receives, send_blocked, receive_blocked)
    }
    
    // ========================================================================
    // Loop-Aware Block Merging
    // ========================================================================
    
    /// Merge basic blocks within a loop body for cross-iteration optimization.
    /// Returns the number of merged blocks created.
    pub fn merge_loop_blocks(&mut self, loop_header: u32, back_edge: u32, body_addresses: &[u32]) -> i32 {
        unsafe {
            oc_spu_jit_merge_loop_blocks(
                self.handle, loop_header, back_edge,
                body_addresses.as_ptr(), body_addresses.len(),
            )
        }
    }
}

/// Loop information
#[derive(Debug, Clone)]
pub struct LoopInfo {
    /// Header address
    pub header: u32,
    /// Back edge address
    pub back_edge: u32,
    /// Exit address
    pub exit: u32,
    /// Iteration count (0 = unknown)
    pub iteration_count: u32,
    /// Whether the loop is vectorizable
    pub is_vectorizable: bool,
}

impl Drop for SpuJitCompiler {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { oc_spu_jit_destroy(self.handle) }
        }
    }
}

unsafe impl Send for SpuJitCompiler {}

// ============================================================================
// RSX Shader Compiler
// ============================================================================

/// Safe wrapper for RSX shader compiler
pub struct RsxShaderCompiler {
    handle: *mut RsxShader,
}

impl RsxShaderCompiler {
    /// Create a new RSX shader compiler
    pub fn new() -> Option<Self> {
        let handle = unsafe { oc_rsx_shader_create() };
        if handle.is_null() {
            None
        } else {
            Some(Self { handle })
        }
    }
    
    /// Compile RSX vertex program to SPIR-V
    pub fn compile_vertex(&mut self, code: &[u32]) -> Result<Vec<u32>, JitError> {
        let mut out_spirv: *mut u32 = std::ptr::null_mut();
        let mut out_size: usize = 0;
        
        let result = unsafe {
            oc_rsx_shader_compile_vertex(
                self.handle, code.as_ptr(), code.len(),
                &mut out_spirv, &mut out_size
            )
        };
        
        if result != 0 {
            return Err(JitError::CompilationFailed);
        }
        
        if out_spirv.is_null() || out_size == 0 {
            return Err(JitError::CompilationFailed);
        }
        
        // Copy to Vec and free the C allocation
        let spirv = unsafe {
            let slice = std::slice::from_raw_parts(out_spirv, out_size);
            let vec = slice.to_vec();
            oc_rsx_shader_free_spirv(out_spirv);
            vec
        };
        
        Ok(spirv)
    }
    
    /// Compile RSX fragment program to SPIR-V
    pub fn compile_fragment(&mut self, code: &[u32]) -> Result<Vec<u32>, JitError> {
        let mut out_spirv: *mut u32 = std::ptr::null_mut();
        let mut out_size: usize = 0;
        
        let result = unsafe {
            oc_rsx_shader_compile_fragment(
                self.handle, code.as_ptr(), code.len(),
                &mut out_spirv, &mut out_size
            )
        };
        
        if result != 0 {
            return Err(JitError::CompilationFailed);
        }
        
        if out_spirv.is_null() || out_size == 0 {
            return Err(JitError::CompilationFailed);
        }
        
        // Copy to Vec and free the C allocation
        let spirv = unsafe {
            let slice = std::slice::from_raw_parts(out_spirv, out_size);
            let vec = slice.to_vec();
            oc_rsx_shader_free_spirv(out_spirv);
            vec
        };
        
        Ok(spirv)
    }
    
    /// Link vertex and fragment shaders
    pub fn link(&mut self, vs_spirv: &[u32], fs_spirv: &[u32]) -> Result<(), JitError> {
        let result = unsafe {
            oc_rsx_shader_link(
                self.handle,
                vs_spirv.as_ptr(), vs_spirv.len(),
                fs_spirv.as_ptr(), fs_spirv.len()
            )
        };
        
        if result == 0 {
            Ok(())
        } else {
            Err(JitError::CompilationFailed)
        }
    }
    
    /// Get number of linked shader programs
    pub fn get_linked_count(&self) -> usize {
        unsafe { oc_rsx_shader_get_linked_count(self.handle) }
    }
    
    /// Get or create a cached graphics pipeline
    pub fn get_pipeline(&mut self, vs_hash: u64, fs_hash: u64, vertex_mask: u32, 
                        cull_mode: u8, blend_enable: bool) -> Option<*mut u8> {
        let ptr = unsafe {
            oc_rsx_shader_get_pipeline(
                self.handle, vs_hash, fs_hash, vertex_mask, cull_mode,
                if blend_enable { 1 } else { 0 }
            )
        };
        if ptr.is_null() { None } else { Some(ptr) }
    }
    
    /// Advance frame counter for LRU eviction
    pub fn advance_frame(&mut self) {
        unsafe { oc_rsx_shader_advance_frame(self.handle) }
    }
    
    /// Get number of cached pipelines
    pub fn get_pipeline_count(&self) -> usize {
        unsafe { oc_rsx_shader_get_pipeline_count(self.handle) }
    }
    
    /// Clear all shader caches
    pub fn clear_caches(&mut self) {
        unsafe { oc_rsx_shader_clear_caches(self.handle) }
    }
    
    /// Get vertex shader cache count
    pub fn get_vertex_cache_count(&self) -> usize {
        unsafe { oc_rsx_shader_get_vertex_cache_count(self.handle) }
    }
    
    /// Get fragment shader cache count
    pub fn get_fragment_cache_count(&self) -> usize {
        unsafe { oc_rsx_shader_get_fragment_cache_count(self.handle) }
    }
}

impl Drop for RsxShaderCompiler {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { oc_rsx_shader_destroy(self.handle) }
        }
    }
}

unsafe impl Send for RsxShaderCompiler {}

/// JIT compilation errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JitError {
    /// Invalid input parameters
    InvalidInput,
    /// JIT compiler is disabled
    Disabled,
    /// Compilation failed
    CompilationFailed,
    /// LLVM compilation failed (message may be generic or detail-specific)
    LlvmError(String),
    /// Block is empty (no instructions)
    EmptyBlock,
}

impl JitError {
    /// Create a JitError from a C++ error code, with optional LLVM error detail.
    pub fn from_error_code(code: i32) -> Self {
        match code {
            -1 => JitError::InvalidInput,
            -2 => JitError::Disabled,
            -3 => JitError::EmptyBlock,
            -4 => JitError::LlvmError("LLVM IR generation or compilation failed".into()),
            _ => JitError::CompilationFailed,
        }
    }
}

impl std::fmt::Display for JitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JitError::InvalidInput => write!(f, "Invalid input parameters"),
            JitError::Disabled => write!(f, "JIT compiler is disabled"),
            JitError::CompilationFailed => write!(f, "JIT compilation failed"),
            JitError::LlvmError(msg) => write!(f, "LLVM compilation error: {}", msg),
            JitError::EmptyBlock => write!(f, "Empty code block"),
        }
    }
}

impl std::error::Error for JitError {}

/// Callback type for interpreter fallback when JIT compilation fails.
///
/// When a JIT block fails to compile, this callback is invoked with:
/// - `address`: the PPU/SPU address that failed to compile
/// - `error`: the JitError describing the failure
///
/// The callback should interpret the block directly and return the number
/// of instructions executed, or a negative value on interpreter failure.
pub type InterpreterFallbackFn = Box<dyn Fn(u32, &JitError) -> i32 + Send + Sync>;

/// Manages JIT-to-interpreter fallback for failed compilations.
///
/// When the LLVM backend fails to compile a block (e.g. unsupported instruction,
/// out of memory, LLVM internal error), blocks are routed to the Rust interpreter
/// instead of silently failing.
pub struct JitFallbackManager {
    ppu_fallback: Option<InterpreterFallbackFn>,
    spu_fallback: Option<InterpreterFallbackFn>,
    /// Addresses that failed compilation and should always use interpreter
    failed_addresses: std::collections::HashSet<u32>,
    /// Statistics
    total_fallbacks: u64,
    total_ppu_fallbacks: u64,
    total_spu_fallbacks: u64,
}

impl JitFallbackManager {
    /// Create a new fallback manager with no callbacks registered.
    pub fn new() -> Self {
        Self {
            ppu_fallback: None,
            spu_fallback: None,
            failed_addresses: std::collections::HashSet::new(),
            total_fallbacks: 0,
            total_ppu_fallbacks: 0,
            total_spu_fallbacks: 0,
        }
    }

    /// Register a PPU interpreter fallback callback.
    pub fn set_ppu_fallback(&mut self, callback: InterpreterFallbackFn) {
        self.ppu_fallback = Some(callback);
    }

    /// Register an SPU interpreter fallback callback.
    pub fn set_spu_fallback(&mut self, callback: InterpreterFallbackFn) {
        self.spu_fallback = Some(callback);
    }

    /// Try to execute a failed PPU block via the interpreter fallback.
    /// Returns `Some(instructions_executed)` if the fallback was invoked,
    /// or `None` if no fallback is registered.
    pub fn fallback_ppu(&mut self, address: u32, error: &JitError) -> Option<i32> {
        self.failed_addresses.insert(address);
        self.total_fallbacks += 1;
        self.total_ppu_fallbacks += 1;
        self.ppu_fallback.as_ref().map(|cb| cb(address, error))
    }

    /// Try to execute a failed SPU block via the interpreter fallback.
    pub fn fallback_spu(&mut self, address: u32, error: &JitError) -> Option<i32> {
        self.failed_addresses.insert(address);
        self.total_fallbacks += 1;
        self.total_spu_fallbacks += 1;
        self.spu_fallback.as_ref().map(|cb| cb(address, error))
    }

    /// Check if an address has previously failed JIT compilation.
    pub fn is_failed(&self, address: u32) -> bool {
        self.failed_addresses.contains(&address)
    }

    /// Clear the failed address set (e.g. after re-enabling LLVM or code invalidation).
    pub fn clear_failed(&mut self) {
        self.failed_addresses.clear();
    }

    /// Get fallback statistics: (total, ppu, spu).
    pub fn get_stats(&self) -> (u64, u64, u64) {
        (self.total_fallbacks, self.total_ppu_fallbacks, self.total_spu_fallbacks)
    }

    /// Reset statistics.
    pub fn reset_stats(&mut self) {
        self.total_fallbacks = 0;
        self.total_ppu_fallbacks = 0;
        self.total_spu_fallbacks = 0;
    }
}

impl Default for JitFallbackManager {
    fn default() -> Self {
        Self::new()
    }
}

/// JIT compiler handle (legacy, for backwards compatibility)
#[derive(Default)]
pub struct JitCompiler {
    _private: (),
}

impl JitCompiler {
    /// Create a new JIT compiler (placeholder for backwards compatibility)
    pub fn new() -> Option<Self> {
        Some(Self { _private: () })
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ppu_jit_creation() {
        let jit = PpuJitCompiler::new();
        assert!(jit.is_some(), "PPU JIT should be created successfully");
    }

    #[test]
    fn test_spu_jit_creation() {
        let jit = SpuJitCompiler::new();
        assert!(jit.is_some(), "SPU JIT should be created successfully");
    }

    #[test]
    fn test_ppu_jit_compile() {
        let mut jit = PpuJitCompiler::new().expect("JIT creation failed");
        // Simple PPU instruction: nop (ori r0, r0, 0) = 0x60000000
        let code = [0x60, 0x00, 0x00, 0x00];
        let result = jit.compile(0x1000, &code);
        assert!(result.is_ok(), "Compilation should succeed");
    }

    #[test]
    fn test_spu_jit_compile() {
        let mut jit = SpuJitCompiler::new().expect("JIT creation failed");
        // Simple SPU instruction: nop = 0x40200000
        let code = [0x40, 0x20, 0x00, 0x00];
        let result = jit.compile(0x1000, &code);
        assert!(result.is_ok(), "Compilation should succeed");
    }

    #[test]
    fn test_ppu_breakpoint() {
        let mut jit = PpuJitCompiler::new().expect("JIT creation failed");
        let address = 0x1000;
        
        assert!(!jit.has_breakpoint(address), "Should not have breakpoint initially");
        
        jit.add_breakpoint(address);
        assert!(jit.has_breakpoint(address), "Should have breakpoint after adding");
        
        jit.remove_breakpoint(address);
        assert!(!jit.has_breakpoint(address), "Should not have breakpoint after removing");
    }

    #[test]
    fn test_spu_breakpoint() {
        let mut jit = SpuJitCompiler::new().expect("JIT creation failed");
        let address = 0x1000;
        
        assert!(!jit.has_breakpoint(address), "Should not have breakpoint initially");
        
        jit.add_breakpoint(address);
        assert!(jit.has_breakpoint(address), "Should have breakpoint after adding");
        
        jit.remove_breakpoint(address);
        assert!(!jit.has_breakpoint(address), "Should not have breakpoint after removing");
    }

    #[test]
    fn test_ppu_cache_operations() {
        let mut jit = PpuJitCompiler::new().expect("JIT creation failed");
        let code = [0x60, 0x00, 0x00, 0x00];
        let address = 0x1000;
        
        jit.compile(address, &code).expect("Compilation failed");
        
        // Should have compiled code
        let compiled = jit.get_compiled(address);
        assert!(compiled.is_some(), "Should have compiled code");
        
        // Invalidate it
        jit.invalidate(address);
        
        // Clear entire cache
        jit.clear_cache();
    }

    #[test]
    fn test_spu_cache_operations() {
        let mut jit = SpuJitCompiler::new().expect("JIT creation failed");
        let code = [0x40, 0x20, 0x00, 0x00];
        let address = 0x1000;
        
        jit.compile(address, &code).expect("Compilation failed");
        
        // Should have compiled code
        let compiled = jit.get_compiled(address);
        assert!(compiled.is_some(), "Should have compiled code");
        
        // Invalidate it
        jit.invalidate(address);
        
        // Clear entire cache
        jit.clear_cache();
    }

    #[test]
    fn test_ppu_block_linking() {
        let mut jit = PpuJitCompiler::new().expect("JIT creation failed");

        // Register a link between two blocks
        jit.link_add(0x1000, 0x2000, false);
        assert_eq!(jit.link_get_count(), 1);
        assert_eq!(jit.link_get_active(), 0);

        // Compile target block first
        let code = [0x60, 0x00, 0x00, 0x00]; // nop
        jit.compile(0x2000, &code).expect("Compilation failed");

        // Now link them
        let linked = jit.link_blocks(0x1000, 0x2000);
        assert!(linked, "Should successfully link blocks");
        assert_eq!(jit.link_get_active(), 1);

        // Get linked target
        let target = jit.link_get_target(0x1000, 0x2000);
        assert!(target.is_some(), "Should have linked target");

        // Unlink source
        jit.unlink_source(0x1000);
        assert_eq!(jit.link_get_active(), 0);

        // Record hits and misses
        jit.link_record_hit();
        jit.link_record_miss();

        // Clear all
        jit.link_clear();
        assert_eq!(jit.link_get_count(), 0);
    }

    #[test]
    fn test_ppu_trace_compilation() {
        let mut jit = PpuJitCompiler::new().expect("JIT creation failed");

        // Set threshold
        jit.trace_set_hot_threshold(5);
        assert_eq!(jit.trace_get_hot_threshold(), 5);

        // Set max length
        jit.trace_set_max_length(16);

        // Detect a loop trace
        let blocks = [0x1000u32, 0x1010, 0x1020];
        jit.trace_detect(0x1000, &blocks, 0x1000);
        assert!(jit.trace_is_header(0x1000));
        assert!(!jit.trace_is_header(0x2000));

        // Record executions
        for _ in 0..4 {
            assert!(!jit.trace_record_execution(0x1000));
        }
        // Fifth execution should trigger compilation
        assert!(jit.trace_record_execution(0x1000));

        // No compiled trace yet (would need actual compilation)
        assert!(jit.trace_get_compiled(0x1000).is_none());

        // Clear
        jit.trace_clear();
        assert!(!jit.trace_is_header(0x1000));
    }

    #[test]
    fn test_ppu_verify_codegen() {
        let mut jit = PpuJitCompiler::new().expect("JIT creation failed");
        // Verify code generation produces valid output
        let result = jit.verify_codegen();
        assert!(result, "Code verification should pass");
    }

    #[test]
    fn test_ppu_compile_fallback_on_empty() {
        let mut jit = PpuJitCompiler::new().expect("JIT creation failed");
        // Empty code should fail gracefully
        let result = jit.compile(0x1000, &[]);
        assert!(result.is_err(), "Empty code should fail");
    }

    #[test]
    fn test_ppu_block_linking_conditional() {
        let mut jit = PpuJitCompiler::new().expect("JIT creation failed");

        // Register conditional link
        jit.link_add(0x1000, 0x2000, true);
        assert_eq!(jit.link_get_count(), 1);

        // Unlink non-existent target is a no-op
        jit.unlink_target(0x3000);
        assert_eq!(jit.link_get_count(), 1);
    }

    #[test]
    fn test_ppu_trace_linear() {
        let mut jit = PpuJitCompiler::new().expect("JIT creation failed");
        
        // Linear trace (no back-edge)
        let blocks = [0x1000u32, 0x1020, 0x1040, 0x1060];
        jit.trace_detect(0x1000, &blocks, 0); // back_edge=0 means linear
        assert!(jit.trace_is_header(0x1000));
    }

    #[test]
    fn test_spu_mailbox_send_receive() {
        let mut jit = SpuJitCompiler::new().expect("JIT creation failed");
        
        // Initially empty
        assert_eq!(jit.mailbox_pending(0, 1), 0);
        assert!(jit.mailbox_receive(0, 1).is_none());
        
        // Send a message from SPU 0 to SPU 1
        assert!(jit.mailbox_send(0, 1, 0x42));
        assert_eq!(jit.mailbox_pending(0, 1), 1);
        
        // Receive it
        let val = jit.mailbox_receive(0, 1);
        assert_eq!(val, Some(0x42));
        assert_eq!(jit.mailbox_pending(0, 1), 0);
    }

    #[test]
    fn test_spu_mailbox_fifo_order() {
        let mut jit = SpuJitCompiler::new().expect("JIT creation failed");
        
        // Send 4 messages (FIFO depth)
        for i in 0..4u32 {
            assert!(jit.mailbox_send(2, 3, i + 100));
        }
        assert_eq!(jit.mailbox_pending(2, 3), 4);
        
        // 5th should fail (full)
        assert!(!jit.mailbox_send(2, 3, 999));
        
        // Receive in FIFO order
        for i in 0..4u32 {
            assert_eq!(jit.mailbox_receive(2, 3), Some(i + 100));
        }
        
        // Empty now
        assert!(jit.mailbox_receive(2, 3).is_none());
    }

    #[test]
    fn test_spu_mailbox_stats() {
        let mut jit = SpuJitCompiler::new().expect("JIT creation failed");
        
        jit.mailbox_send(0, 1, 1);
        jit.mailbox_send(0, 1, 2);
        jit.mailbox_receive(0, 1);
        
        let (sends, receives, _, _) = jit.mailbox_get_stats();
        assert_eq!(sends, 2);
        assert_eq!(receives, 1);
        
        // Reset
        jit.mailbox_reset();
        let (sends, receives, _, _) = jit.mailbox_get_stats();
        assert_eq!(sends, 0);
        assert_eq!(receives, 0);
    }

    #[test]
    fn test_spu_mailbox_invalid_spu() {
        let mut jit = SpuJitCompiler::new().expect("JIT creation failed");
        
        // SPU IDs >= 8 should fail
        assert!(!jit.mailbox_send(8, 0, 42));
        assert!(jit.mailbox_receive(0, 8).is_none());
        assert_eq!(jit.mailbox_pending(8, 8), 0);
    }

    #[test]
    fn test_spu_merge_loop_blocks_empty() {
        let mut jit = SpuJitCompiler::new().expect("JIT creation failed");
        
        // Empty body list should return 0
        let result = jit.merge_loop_blocks(0x100, 0x200, &[]);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_spu_merge_loop_blocks_no_cache() {
        let mut jit = SpuJitCompiler::new().expect("JIT creation failed");
        
        // Body addresses not in cache should return 0
        let body = [0x100u32, 0x110, 0x120];
        let result = jit.merge_loop_blocks(0x100, 0x120, &body);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_jit_error_from_error_code() {
        assert_eq!(JitError::from_error_code(-1), JitError::InvalidInput);
        assert_eq!(JitError::from_error_code(-2), JitError::Disabled);
        assert_eq!(JitError::from_error_code(-3), JitError::EmptyBlock);
        assert!(matches!(JitError::from_error_code(-4), JitError::LlvmError(_)));
        assert_eq!(JitError::from_error_code(-99), JitError::CompilationFailed);
    }

    #[test]
    fn test_jit_error_display() {
        let err = JitError::LlvmError("test error".into());
        let msg = format!("{}", err);
        assert!(msg.contains("LLVM"), "Display should mention LLVM: {}", msg);
        assert!(msg.contains("test error"));
    }

    #[test]
    fn test_fallback_manager_ppu() {
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::sync::Arc;
        
        let mut mgr = JitFallbackManager::new();
        
        // No fallback registered — should return None
        assert!(mgr.fallback_ppu(0x1000, &JitError::CompilationFailed).is_none());
        
        // Register a PPU fallback
        let call_count = Arc::new(AtomicU32::new(0));
        let cc = call_count.clone();
        mgr.set_ppu_fallback(Box::new(move |addr, _err| {
            cc.fetch_add(1, Ordering::SeqCst);
            assert_eq!(addr, 0x2000);
            42  // "interpreted 42 instructions"
        }));
        
        let result = mgr.fallback_ppu(0x2000, &JitError::EmptyBlock);
        assert_eq!(result, Some(42));
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
        assert!(mgr.is_failed(0x2000));
        
        let (total, ppu, spu) = mgr.get_stats();
        assert_eq!(total, 2);  // 1 from the None case + 1 from the Some case
        assert_eq!(ppu, 2);
        assert_eq!(spu, 0);
    }

    #[test]
    fn test_fallback_manager_spu() {
        let mut mgr = JitFallbackManager::new();
        
        mgr.set_spu_fallback(Box::new(|_addr, _err| 10));
        
        let result = mgr.fallback_spu(0x100, &JitError::LlvmError("oops".into()));
        assert_eq!(result, Some(10));
        assert!(mgr.is_failed(0x100));
        
        // Clear failed set
        mgr.clear_failed();
        assert!(!mgr.is_failed(0x100));
    }

    #[test]
    fn test_fallback_manager_stats_reset() {
        let mut mgr = JitFallbackManager::new();
        mgr.set_ppu_fallback(Box::new(|_, _| 0));
        mgr.fallback_ppu(0x1000, &JitError::CompilationFailed);
        mgr.fallback_ppu(0x2000, &JitError::CompilationFailed);
        
        let (total, _, _) = mgr.get_stats();
        assert_eq!(total, 2);
        
        mgr.reset_stats();
        let (total, _, _) = mgr.get_stats();
        assert_eq!(total, 0);
    }

    #[test]
    fn test_ppu_compile_empty_returns_error() {
        let mut jit = PpuJitCompiler::new().expect("JIT creation failed");
        let result = jit.compile(0x1000, &[]);
        assert!(result.is_err(), "Empty block should return an error");
        // May return EmptyBlock (-3) or InvalidInput (-1) depending on C++ validation order
        let err = result.unwrap_err();
        assert!(
            matches!(err, JitError::EmptyBlock | JitError::InvalidInput),
            "Expected EmptyBlock or InvalidInput, got: {:?}", err
        );
    }

    #[test]
    fn test_spu_compile_empty_returns_error() {
        let mut jit = SpuJitCompiler::new().expect("JIT creation failed");
        let result = jit.compile(0x1000, &[]);
        assert!(result.is_err(), "Empty block should return an error");
        let err = result.unwrap_err();
        assert!(
            matches!(err, JitError::EmptyBlock | JitError::InvalidInput),
            "Expected EmptyBlock or InvalidInput, got: {:?}", err
        );
    }
}
