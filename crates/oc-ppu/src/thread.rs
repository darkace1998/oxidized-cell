//! PPU thread state

use std::sync::Arc;
use oc_memory::MemoryManager;
use oc_core::error::{PpuExceptionType, PowerState};

/// PPU register set
#[derive(Debug, Clone)]
pub struct PpuRegisters {
    /// General Purpose Registers (64-bit)
    pub gpr: [u64; 32],
    /// Floating Point Registers (64-bit)
    pub fpr: [f64; 32],
    /// Vector Registers (128-bit, stored as 4 x u32)
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
    /// Program Counter / Next Instruction Address
    pub cia: u64,
    /// Machine State Register
    pub msr: u64,
    /// Save/Restore Registers (for exception handling)
    pub srr0: u64,
    pub srr1: u64,
    /// Decrementer register
    pub dec: u32,
    /// Time Base registers (for timing)
    pub tb: u64,
}

impl Default for PpuRegisters {
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
            cia: 0,
            msr: 0x8000_0000_0000_0000, // 64-bit mode enabled by default
            srr0: 0,
            srr1: 0,
            dec: 0,
            tb: 0,
        }
    }
}

/// PPU thread state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PpuThreadState {
    /// Thread is stopped
    Stopped,
    /// Thread is running
    Running,
    /// Thread is waiting (blocked)
    Waiting,
    /// Thread is suspended
    Suspended,
    /// Thread is ready to run (waiting for scheduler)
    Ready,
    /// Thread is sleeping (timed wait)
    Sleeping,
}

/// Thread scheduling priority levels (PS3 uses 0-3071, lower is higher priority)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ThreadPriority(pub u32);

impl ThreadPriority {
    /// Highest priority (most urgent)
    pub const HIGHEST: Self = Self(0);
    /// High priority
    pub const HIGH: Self = Self(256);
    /// Normal/default priority
    pub const NORMAL: Self = Self(1024);
    /// Low priority
    pub const LOW: Self = Self(2048);
    /// Lowest priority (least urgent)
    pub const LOWEST: Self = Self(3071);
    
    /// Create a new priority value
    pub fn new(value: u32) -> Self {
        Self(value.min(3071))
    }
    
    /// Get the raw priority value
    pub fn value(&self) -> u32 {
        self.0
    }
    
    /// Check if this priority is higher than another (lower numeric value = higher priority)
    pub fn is_higher_than(&self, other: &Self) -> bool {
        self.0 < other.0
    }
}

impl Default for ThreadPriority {
    fn default() -> Self {
        Self::NORMAL
    }
}

/// Thread affinity mask (which PPU cores the thread can run on)
/// PS3 has 2 PPU hardware threads (SMT)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThreadAffinity(pub u64);

impl ThreadAffinity {
    /// Can run on any core
    pub const ANY: Self = Self(0xFFFFFFFF_FFFFFFFF);
    /// PPU thread 0 only
    pub const PPU0: Self = Self(0x01);
    /// PPU thread 1 only
    pub const PPU1: Self = Self(0x02);
    /// Both PPU threads
    pub const BOTH: Self = Self(0x03);
    
    /// Create from a bitmask
    pub fn from_mask(mask: u64) -> Self {
        Self(mask)
    }
    
    /// Check if this affinity allows running on a specific core
    pub fn allows_core(&self, core_id: u32) -> bool {
        (self.0 & (1u64 << core_id)) != 0
    }
    
    /// Set affinity to a specific core
    pub fn set_core(&mut self, core_id: u32, allowed: bool) {
        if allowed {
            self.0 |= 1u64 << core_id;
        } else {
            self.0 &= !(1u64 << core_id);
        }
    }
}

impl Default for ThreadAffinity {
    fn default() -> Self {
        Self::ANY
    }
}

/// Wait reason when thread is in Waiting state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitReason {
    /// Waiting on a mutex
    Mutex(u32),
    /// Waiting on a condition variable
    Condition(u32),
    /// Waiting on a semaphore
    Semaphore(u32),
    /// Waiting on an event flag
    EventFlag(u32),
    /// Waiting on a lightweight mutex
    LwMutex(u32),
    /// Waiting on a lightweight condition variable
    LwCond(u32),
    /// Waiting on a message queue
    Queue(u32),
    /// Waiting for SPU thread
    SpuThread(u32),
    /// Waiting for sleep to complete
    Sleep(u64),
    /// Generic wait (unknown reason)
    Generic,
}

/// Synchronization state for LV2 primitives
#[derive(Debug, Clone, Default)]
pub struct SyncState {
    /// Current wait reason (if in Waiting state)
    pub wait_reason: Option<WaitReason>,
    /// Timeout value in microseconds (0 = no timeout)
    pub wait_timeout: u64,
    /// When the wait started (cycle count)
    pub wait_start: u64,
    /// Number of times this thread has been preempted
    pub preempt_count: u64,
    /// Number of times this thread has yielded
    pub yield_count: u64,
    /// Total time spent waiting (cycles)
    pub total_wait_cycles: u64,
}

/// Pipeline stage representation for simulation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PipelineStage {
    #[default]
    Fetch,
    Decode,
    Execute,
    Memory,
    WriteBack,
}

/// Pipeline state for simulation
#[derive(Debug, Clone, Default)]
pub struct PipelineState {
    /// Current pipeline stage
    pub stage: PipelineStage,
    /// Instructions in flight (address, opcode)
    pub in_flight: [(u64, u32); 5],
    /// Pipeline stall cycles
    pub stall_cycles: u32,
    /// Branch prediction hit/miss statistics
    pub branch_hits: u64,
    pub branch_misses: u64,
}

impl PipelineState {
    /// Create a new pipeline state
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a branch prediction result
    pub fn record_branch(&mut self, hit: bool) {
        if hit {
            self.branch_hits += 1;
        } else {
            self.branch_misses += 1;
        }
    }

    /// Get branch prediction accuracy
    pub fn branch_accuracy(&self) -> f64 {
        let total = self.branch_hits + self.branch_misses;
        if total == 0 {
            1.0
        } else {
            self.branch_hits as f64 / total as f64
        }
    }

    /// Flush the pipeline (e.g., after a branch misprediction)
    pub fn flush(&mut self) {
        self.in_flight = [(0, 0); 5];
        self.stall_cycles += 4; // Penalty for flush
    }
}

/// Timing state for cycle-accurate emulation
#[derive(Debug, Clone, Default)]
pub struct TimingState {
    /// Total cycles executed
    pub cycles: u64,
    /// Cycles per instruction (for averaging)
    pub cycles_per_instruction: f64,
    /// Enable cycle-accurate timing
    pub enabled: bool,
    /// Cycle frequency in Hz (Cell BE runs at 3.2 GHz)
    pub frequency_hz: u64,
    /// Last timestamp for real-time sync
    pub last_sync_time: u64,
    /// Instructions since last sync
    pub instructions_since_sync: u64,
}

impl TimingState {
    /// Create a new timing state
    pub fn new(enabled: bool) -> Self {
        Self {
            cycles: 0,
            cycles_per_instruction: 1.0,
            enabled,
            frequency_hz: 3_200_000_000, // 3.2 GHz
            last_sync_time: 0,
            instructions_since_sync: 0,
        }
    }

    /// Add cycles for an instruction
    pub fn add_cycles(&mut self, cycles: u64) {
        self.cycles += cycles;
        self.instructions_since_sync += 1;
    }

    /// Get estimated cycles for an instruction type
    pub fn get_instruction_cycles(&self, instruction_type: InstructionLatency) -> u64 {
        if !self.enabled {
            return 1;
        }
        match instruction_type {
            InstructionLatency::Simple => 1,
            InstructionLatency::Load => 3,
            InstructionLatency::Store => 2,
            InstructionLatency::Branch => 1,
            InstructionLatency::BranchMispredict => 23,
            InstructionLatency::FloatSimple => 6,
            InstructionLatency::FloatComplex => 10,
            InstructionLatency::FloatDivide => 33,
            InstructionLatency::FloatSqrt => 44,
            InstructionLatency::Vector => 4,
            InstructionLatency::Multiply => 4,
            InstructionLatency::Divide => 36,
        }
    }
}

/// Instruction latency categories
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstructionLatency {
    /// Simple ALU operations (add, sub, logic)
    Simple,
    /// Load from memory
    Load,
    /// Store to memory
    Store,
    /// Branch (predicted correctly)
    Branch,
    /// Branch misprediction penalty
    BranchMispredict,
    /// Simple floating-point (add, sub, mul)
    FloatSimple,
    /// Complex floating-point (fma, fms)
    FloatComplex,
    /// Floating-point divide
    FloatDivide,
    /// Floating-point square root
    FloatSqrt,
    /// Vector (VMX/AltiVec) operations
    Vector,
    /// Integer multiply
    Multiply,
    /// Integer divide
    Divide,
}

/// Exception state for full exception model
#[derive(Debug, Clone, Default)]
pub struct ExceptionState {
    /// Pending exception (if any)
    pub pending: Option<PpuExceptionType>,
    /// Exception mask (which exceptions are enabled)
    pub mask: u64,
    /// Exception handler addresses
    pub handlers: [u64; 16],
}

impl ExceptionState {
    /// Create a new exception state
    pub fn new() -> Self {
        Self {
            pending: None,
            mask: 0xFFFF_FFFF_FFFF_FFFF, // All exceptions enabled by default
            handlers: [0; 16],
        }
    }

    /// Raise an exception
    pub fn raise(&mut self, exception: PpuExceptionType) {
        self.pending = Some(exception);
    }

    /// Clear pending exception
    pub fn clear(&mut self) {
        self.pending = None;
    }

    /// Check if exception is masked
    pub fn is_masked(&self, exception: &PpuExceptionType) -> bool {
        let bit = Self::exception_bit(exception);
        (self.mask & (1 << bit)) == 0
    }

    /// Get the bit position for an exception type
    fn exception_bit(exception: &PpuExceptionType) -> u32 {
        match exception {
            PpuExceptionType::SystemReset => 0,
            PpuExceptionType::MachineCheck => 1,
            PpuExceptionType::DataStorage => 2,
            PpuExceptionType::DataSegment => 3,
            PpuExceptionType::InstructionStorage => 4,
            PpuExceptionType::InstructionSegment => 5,
            PpuExceptionType::ExternalInterrupt => 6,
            PpuExceptionType::Alignment => 7,
            PpuExceptionType::Program { .. } => 8,
            PpuExceptionType::FloatingPointUnavailable => 9,
            PpuExceptionType::Decrementer => 10,
            PpuExceptionType::SystemCall => 11,
            PpuExceptionType::Trace => 12,
            PpuExceptionType::FloatingPointAssist => 13,
            PpuExceptionType::PerformanceMonitor => 14,
            PpuExceptionType::VmxUnavailable => 15,
        }
    }
}

/// Power management state
#[derive(Debug, Clone)]
pub struct PowerManagementState {
    /// Current power state
    pub state: PowerState,
    /// Power-on cycles counter
    pub power_on_cycles: u64,
    /// Idle cycles counter
    pub idle_cycles: u64,
    /// Throttle level (0-100, where 100 is full speed)
    pub throttle_level: u8,
}

impl Default for PowerManagementState {
    fn default() -> Self {
        Self {
            state: PowerState::Running,
            power_on_cycles: 0,
            idle_cycles: 0,
            throttle_level: 100,
        }
    }
}

impl PowerManagementState {
    /// Create a new power management state
    pub fn new() -> Self {
        Self::default()
    }

    /// Transition to a new power state
    pub fn transition(&mut self, new_state: PowerState) {
        self.state = new_state;
    }

    /// Check if CPU should execute
    pub fn should_execute(&self) -> bool {
        self.state == PowerState::Running
    }
}

/// PPU thread
pub struct PpuThread {
    /// Thread ID
    pub id: u32,
    /// Thread name
    pub name: String,
    /// Register state
    pub regs: PpuRegisters,
    /// Thread state
    pub state: PpuThreadState,
    /// Memory manager reference
    memory: Arc<MemoryManager>,
    /// Stack address
    pub stack_addr: u32,
    /// Stack size
    pub stack_size: u32,
    /// Priority (legacy field, use scheduling_priority for new code)
    pub priority: u32,
    /// Pipeline state (for simulation)
    pub pipeline: PipelineState,
    /// Timing state (for cycle-accurate emulation)
    pub timing: TimingState,
    /// Exception state (for full exception model)
    pub exceptions: ExceptionState,
    /// Power management state
    pub power: PowerManagementState,
    /// Thread scheduling priority
    pub scheduling_priority: ThreadPriority,
    /// Thread affinity (which cores can run this thread)
    pub affinity: ThreadAffinity,
    /// Synchronization state
    pub sync: SyncState,
    /// Entry point address
    pub entry_point: u64,
    /// Argument passed to thread entry point
    pub arg: u64,
    /// Join value (returned when thread exits)
    pub join_value: u64,
    /// Whether this thread is joinable
    pub joinable: bool,
    /// Whether this thread has been joined
    pub joined: bool,
}

impl PpuThread {
    /// Create a new PPU thread
    pub fn new(id: u32, memory: Arc<MemoryManager>) -> Self {
        Self {
            id,
            name: format!("PPU Thread {}", id),
            regs: PpuRegisters::default(),
            state: PpuThreadState::Stopped,
            memory,
            stack_addr: 0,
            stack_size: 0,
            priority: 0,
            pipeline: PipelineState::new(),
            timing: TimingState::new(false),
            exceptions: ExceptionState::new(),
            power: PowerManagementState::new(),
            scheduling_priority: ThreadPriority::default(),
            affinity: ThreadAffinity::default(),
            sync: SyncState::default(),
            entry_point: 0,
            arg: 0,
            join_value: 0,
            joinable: true,
            joined: false,
        }
    }

    /// Create a new PPU thread with timing enabled
    pub fn new_with_timing(id: u32, memory: Arc<MemoryManager>, timing_enabled: bool) -> Self {
        let mut thread = Self::new(id, memory);
        thread.timing = TimingState::new(timing_enabled);
        thread
    }
    
    /// Create a new PPU thread with full configuration
    pub fn new_with_config(
        id: u32, 
        memory: Arc<MemoryManager>,
        entry_point: u64,
        arg: u64,
        priority: ThreadPriority,
        stack_addr: u32,
        stack_size: u32,
    ) -> Self {
        let mut thread = Self::new(id, memory);
        thread.entry_point = entry_point;
        thread.arg = arg;
        thread.scheduling_priority = priority;
        thread.stack_addr = stack_addr;
        thread.stack_size = stack_size;
        thread.regs.cia = entry_point;
        thread.regs.gpr[3] = arg; // First argument in r3
        thread
    }
    
    /// Set the thread's scheduling priority
    pub fn set_scheduling_priority(&mut self, priority: ThreadPriority) {
        self.scheduling_priority = priority;
        self.priority = priority.value();
    }
    
    /// Get the thread's scheduling priority
    pub fn get_scheduling_priority(&self) -> ThreadPriority {
        self.scheduling_priority
    }
    
    /// Set thread affinity mask
    pub fn set_affinity(&mut self, affinity: ThreadAffinity) {
        self.affinity = affinity;
    }
    
    /// Get thread affinity mask
    pub fn get_affinity(&self) -> ThreadAffinity {
        self.affinity
    }
    
    /// Check if thread can run on specified core
    pub fn can_run_on_core(&self, core_id: u32) -> bool {
        self.affinity.allows_core(core_id)
    }
    
    /// Put thread into waiting state with a reason
    pub fn wait(&mut self, reason: WaitReason, timeout_us: u64) {
        self.state = PpuThreadState::Waiting;
        self.sync.wait_reason = Some(reason);
        self.sync.wait_timeout = timeout_us;
        self.sync.wait_start = self.timing.cycles;
    }
    
    /// Wake thread from waiting state
    pub fn wake(&mut self) {
        if self.state == PpuThreadState::Waiting {
            let wait_cycles = self.timing.cycles - self.sync.wait_start;
            self.sync.total_wait_cycles += wait_cycles;
            self.state = PpuThreadState::Ready;
            self.sync.wait_reason = None;
            self.sync.wait_timeout = 0;
        }
    }
    
    /// Put thread to sleep for specified microseconds
    pub fn sleep(&mut self, timeout_us: u64) {
        self.state = PpuThreadState::Sleeping;
        self.sync.wait_reason = Some(WaitReason::Sleep(timeout_us));
        self.sync.wait_timeout = timeout_us;
        self.sync.wait_start = self.timing.cycles;
    }
    
    /// Check if thread's wait has timed out
    pub fn check_timeout(&self, current_cycles: u64, cycles_per_us: u64) -> bool {
        if self.sync.wait_timeout == 0 {
            return false; // No timeout set
        }
        let elapsed_cycles = current_cycles.saturating_sub(self.sync.wait_start);
        let elapsed_us = elapsed_cycles / cycles_per_us.max(1);
        elapsed_us >= self.sync.wait_timeout
    }
    
    /// Yield execution (voluntary preemption)
    pub fn yield_thread(&mut self) {
        self.sync.yield_count += 1;
        self.state = PpuThreadState::Ready;
    }
    
    /// Record thread preemption
    pub fn preempt(&mut self) {
        self.sync.preempt_count += 1;
        self.state = PpuThreadState::Ready;
    }
    
    /// Mark thread as ready to run
    pub fn make_ready(&mut self) {
        if self.state != PpuThreadState::Stopped {
            self.state = PpuThreadState::Ready;
        }
    }
    
    /// Suspend the thread
    pub fn suspend(&mut self) {
        if self.state != PpuThreadState::Stopped {
            self.state = PpuThreadState::Suspended;
        }
    }
    
    /// Resume a suspended thread
    pub fn resume(&mut self) {
        if self.state == PpuThreadState::Suspended {
            self.state = PpuThreadState::Ready;
        }
    }
    
    /// Exit the thread with a return value
    pub fn exit(&mut self, value: u64) {
        self.join_value = value;
        self.state = PpuThreadState::Stopped;
    }
    
    /// Join this thread (wait for it to exit)
    /// Returns Some(join_value) if successful, None if already joined or not joinable
    pub fn join(&mut self) -> Option<u64> {
        if self.joinable && self.state == PpuThreadState::Stopped && !self.joined {
            self.joined = true;
            Some(self.join_value)
        } else {
            None
        }
    }
    
    /// Check if thread is in a waitable state
    pub fn is_waitable(&self) -> bool {
        matches!(self.state, PpuThreadState::Waiting | PpuThreadState::Sleeping)
    }
    
    /// Check if thread is runnable
    pub fn is_runnable(&self) -> bool {
        matches!(self.state, PpuThreadState::Running | PpuThreadState::Ready)
    }

    /// Get the current instruction address
    pub fn pc(&self) -> u64 {
        self.regs.cia
    }

    /// Set the program counter
    pub fn set_pc(&mut self, addr: u64) {
        self.regs.cia = addr;
    }

    /// Advance the program counter by 4 bytes
    pub fn advance_pc(&mut self) {
        self.regs.cia += 4;
    }

    /// Read a GPR
    #[inline]
    pub fn gpr(&self, index: usize) -> u64 {
        self.regs.gpr[index]
    }

    /// Write a GPR
    #[inline]
    pub fn set_gpr(&mut self, index: usize, value: u64) {
        if index != 0 {
            self.regs.gpr[index] = value;
        }
    }

    /// Read an FPR
    #[inline]
    pub fn fpr(&self, index: usize) -> f64 {
        self.regs.fpr[index]
    }

    /// Write an FPR
    #[inline]
    pub fn set_fpr(&mut self, index: usize, value: f64) {
        self.regs.fpr[index] = value;
    }

    /// Read a VR
    #[inline]
    pub fn vr(&self, index: usize) -> [u32; 4] {
        self.regs.vr[index]
    }

    /// Write a VR
    #[inline]
    pub fn set_vr(&mut self, index: usize, value: [u32; 4]) {
        self.regs.vr[index] = value;
    }

    /// Get a reference to the memory manager
    pub fn memory(&self) -> &Arc<MemoryManager> {
        &self.memory
    }

    /// Start the thread
    pub fn start(&mut self) {
        self.state = PpuThreadState::Running;
    }

    /// Stop the thread
    pub fn stop(&mut self) {
        self.state = PpuThreadState::Stopped;
    }

    /// Check if thread is running
    pub fn is_running(&self) -> bool {
        self.state == PpuThreadState::Running
    }

    /// Get CR field value (0-7)
    pub fn get_cr_field(&self, field: usize) -> u32 {
        (self.regs.cr >> (28 - field * 4)) & 0xF
    }

    /// Set CR field value (0-7)
    pub fn set_cr_field(&mut self, field: usize, value: u32) {
        let shift = 28 - field * 4;
        self.regs.cr = (self.regs.cr & !(0xF << shift)) | ((value & 0xF) << shift);
    }

    /// Get XER CA (Carry) bit
    pub fn get_xer_ca(&self) -> bool {
        (self.regs.xer & 0x20000000) != 0
    }

    /// Set XER CA (Carry) bit
    pub fn set_xer_ca(&mut self, value: bool) {
        if value {
            self.regs.xer |= 0x20000000;
        } else {
            self.regs.xer &= !0x20000000;
        }
    }

    /// Get XER OV (Overflow) bit
    pub fn get_xer_ov(&self) -> bool {
        (self.regs.xer & 0x40000000) != 0
    }

    /// Set XER OV (Overflow) bit
    pub fn set_xer_ov(&mut self, value: bool) {
        if value {
            self.regs.xer |= 0x40000000;
        } else {
            self.regs.xer &= !0x40000000;
        }
    }

    /// Get XER SO (Summary Overflow) bit
    pub fn get_xer_so(&self) -> bool {
        (self.regs.xer & 0x80000000) != 0
    }

    /// Set XER SO (Summary Overflow) bit
    pub fn set_xer_so(&mut self, value: bool) {
        if value {
            self.regs.xer |= 0x80000000;
        } else {
            self.regs.xer &= !0x80000000;
        }
    }

    /// Evaluate a trap condition (used by tw, td, twi, tdi instructions)
    /// Returns true if the trap should be taken
    pub fn evaluate_trap_condition(&self, to: u8, a: i64, b: i64) -> bool {
        let lt = a < b;
        let gt = a > b;
        let eq = a == b;
        let ltu = (a as u64) < (b as u64);
        let gtu = (a as u64) > (b as u64);

        ((to & 0x10) != 0 && lt)
            || ((to & 0x08) != 0 && gt)
            || ((to & 0x04) != 0 && eq)
            || ((to & 0x02) != 0 && ltu)
            || ((to & 0x01) != 0 && gtu)
    }

    /// Handle exception entry
    pub fn enter_exception(&mut self, exception: PpuExceptionType, vector: u64) {
        // Save current state to SRR0/SRR1
        self.regs.srr0 = self.regs.cia;
        self.regs.srr1 = self.regs.msr;

        // Clear recoverable bits in MSR
        self.regs.msr &= !(1 << 15); // Clear EE (External Interrupt Enable)
        self.regs.msr &= !(1 << 14); // Clear PR (Problem State)

        // Set pending exception
        self.exceptions.raise(exception);

        // Jump to exception vector
        self.regs.cia = vector;
    }

    /// Return from exception (rfi instruction)
    pub fn return_from_exception(&mut self) {
        // Restore state from SRR0/SRR1
        self.regs.cia = self.regs.srr0;
        self.regs.msr = self.regs.srr1;

        // Clear pending exception
        self.exceptions.clear();
    }

    /// Update time base register
    pub fn update_time_base(&mut self, cycles: u64) {
        self.regs.tb = self.regs.tb.wrapping_add(cycles);
    }

    /// Decrement the decrementer register and check for exception
    pub fn update_decrementer(&mut self, cycles: u32) -> bool {
        let old_dec = self.regs.dec;
        self.regs.dec = self.regs.dec.wrapping_sub(cycles);

        // Decrementer exception when it crosses from positive to negative
        old_dec > 0 && self.regs.dec == 0
    }

    /// Add timing cycles for the current instruction
    pub fn add_instruction_cycles(&mut self, latency: InstructionLatency) {
        if self.timing.enabled {
            let cycles = self.timing.get_instruction_cycles(latency);
            self.timing.add_cycles(cycles);
            self.update_time_base(cycles);
        }
    }

    /// Get the current cycle count
    pub fn get_cycles(&self) -> u64 {
        self.timing.cycles
    }

    /// Get MSR (Machine State Register)
    pub fn get_msr(&self) -> u64 {
        self.regs.msr
    }

    /// Set MSR (Machine State Register)
    pub fn set_msr(&mut self, value: u64) {
        self.regs.msr = value;
    }

    /// Check if in 64-bit mode
    pub fn is_64bit_mode(&self) -> bool {
        (self.regs.msr & 0x8000_0000_0000_0000) != 0
    }

    /// Check if external interrupts are enabled
    pub fn interrupts_enabled(&self) -> bool {
        (self.regs.msr & (1 << 15)) != 0
    }

    /// Check if in privileged mode (supervisor)
    pub fn is_privileged(&self) -> bool {
        (self.regs.msr & (1 << 14)) == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_memory() -> Arc<MemoryManager> {
        MemoryManager::new().unwrap()
    }

    #[test]
    fn test_ppu_thread_creation() {
        let mem = create_test_memory();
        let thread = PpuThread::new(0, mem);
        
        assert_eq!(thread.id, 0);
        assert_eq!(thread.state, PpuThreadState::Stopped);
        assert_eq!(thread.pc(), 0);
    }

    #[test]
    fn test_gpr_operations() {
        let mem = create_test_memory();
        let mut thread = PpuThread::new(0, mem);

        thread.set_gpr(1, 0x12345678);
        assert_eq!(thread.gpr(1), 0x12345678);

        // R0 should always be writable (unlike some RISC ISAs)
        thread.set_gpr(0, 0xDEADBEEF);
        // Note: In PPU, R0 can be used as a normal register
    }

    #[test]
    fn test_pc_operations() {
        let mem = create_test_memory();
        let mut thread = PpuThread::new(0, mem);

        thread.set_pc(0x10000);
        assert_eq!(thread.pc(), 0x10000);

        thread.advance_pc();
        assert_eq!(thread.pc(), 0x10004);
    }

    #[test]
    fn test_cr_fields() {
        let mem = create_test_memory();
        let mut thread = PpuThread::new(0, mem);

        thread.set_cr_field(0, 0b1010);
        assert_eq!(thread.get_cr_field(0), 0b1010);

        thread.set_cr_field(7, 0b0101);
        assert_eq!(thread.get_cr_field(7), 0b0101);
    }
}
