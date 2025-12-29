//! SPU thread state

use std::collections::VecDeque;
use std::sync::Arc;
use oc_memory::MemoryManager;
use crate::channels::SpuChannels;
use crate::mfc::Mfc;

/// SPU local storage size (256 KB)
pub const SPU_LS_SIZE: usize = 256 * 1024;

/// Maximum number of SPU threads per group
pub const MAX_SPU_THREADS_PER_GROUP: usize = 8;

/// SPU thread state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpuThreadState {
    /// Thread is stopped
    Stopped,
    /// Thread is running
    Running,
    /// Thread is waiting on channel
    Waiting,
    /// Thread is halted (stop instruction)
    Halted,
    /// Thread is ready to run
    Ready,
    /// Thread has an exception pending
    Exception,
    /// Thread is in isolation mode
    Isolated,
}

/// SPU thread priority (0-255, lower is higher priority)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SpuPriority(pub u8);

impl SpuPriority {
    /// Highest priority
    pub const HIGHEST: Self = Self(0);
    /// High priority
    pub const HIGH: Self = Self(64);
    /// Normal priority
    pub const NORMAL: Self = Self(128);
    /// Low priority
    pub const LOW: Self = Self(192);
    /// Lowest priority
    pub const LOWEST: Self = Self(255);
    
    /// Create a new priority
    pub fn new(value: u8) -> Self {
        Self(value)
    }
    
    /// Get the value
    pub fn value(&self) -> u8 {
        self.0
    }
}

impl Default for SpuPriority {
    fn default() -> Self {
        Self::NORMAL
    }
}

/// SPU thread affinity (which physical SPU to run on)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpuAffinity(pub u8);

impl SpuAffinity {
    /// Can run on any SPU
    pub const ANY: Self = Self(0x3F); // Bits 0-5 for 6 SPUs
    
    /// Create affinity for a specific SPU
    pub fn specific(spu_id: u8) -> Self {
        Self(1 << spu_id)
    }
    
    /// Check if can run on a specific SPU
    pub fn allows_spu(&self, spu_id: u8) -> bool {
        (self.0 & (1 << spu_id)) != 0
    }
}

impl Default for SpuAffinity {
    fn default() -> Self {
        Self::ANY
    }
}

/// SPU exception type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpuExceptionType {
    /// Invalid instruction
    InvalidInstruction,
    /// Memory access violation
    MemoryViolation,
    /// Channel error
    ChannelError,
    /// DMA error
    DmaError,
    /// Stop and signal
    StopSignal(u32),
    /// Halt instruction
    Halt,
    /// Isolation error
    IsolationError,
}

/// SPU exception state
#[derive(Debug, Clone, Default)]
pub struct SpuExceptionState {
    /// Pending exception
    pub pending: Option<SpuExceptionType>,
    /// Exception program counter (where exception occurred)
    pub exception_pc: u32,
    /// Exception handler address (in local storage)
    pub handler_address: u32,
    /// Exception mask (which exceptions are enabled)
    pub mask: u32,
}

impl SpuExceptionState {
    /// Create new exception state
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Raise an exception
    pub fn raise(&mut self, exception: SpuExceptionType, pc: u32) {
        self.pending = Some(exception);
        self.exception_pc = pc;
    }
    
    /// Clear pending exception
    pub fn clear(&mut self) {
        self.pending = None;
    }
    
    /// Check if an exception is pending
    pub fn has_exception(&self) -> bool {
        self.pending.is_some()
    }
}

/// SPU event queue entry
#[derive(Debug, Clone)]
pub struct SpuEvent {
    /// Event source (SPU ID or group ID)
    pub source: u32,
    /// Event data (usually stop signal value)
    pub data: u64,
    /// Event type
    pub event_type: SpuEventType,
}

/// SPU event types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpuEventType {
    /// Thread finished normally
    ThreadFinish,
    /// Thread stopped with signal
    StopSignal,
    /// DMA completion
    DmaComplete,
    /// Mailbox message available
    MailboxReady,
    /// Exception occurred
    Exception,
    /// Thread group finished
    GroupFinish,
}

/// SPU event queue
#[derive(Debug, Clone)]
pub struct SpuEventQueue {
    /// Queue ID
    pub id: u32,
    /// Events in the queue (using VecDeque for O(1) pop_front)
    events: VecDeque<SpuEvent>,
    /// Maximum queue size
    max_size: usize,
    /// Waiting threads (PPU thread IDs)
    waiters: Vec<u32>,
}

impl SpuEventQueue {
    /// Create a new event queue
    pub fn new(id: u32, max_size: usize) -> Self {
        Self {
            id,
            events: VecDeque::with_capacity(max_size),
            max_size,
            waiters: Vec::new(),
        }
    }
    
    /// Push an event to the queue
    pub fn push(&mut self, event: SpuEvent) -> bool {
        if self.events.len() < self.max_size {
            self.events.push_back(event);
            true
        } else {
            false
        }
    }
    
    /// Pop an event from the queue (O(1) operation)
    pub fn pop(&mut self) -> Option<SpuEvent> {
        self.events.pop_front()
    }
    
    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
    
    /// Get number of events in queue
    pub fn len(&self) -> usize {
        self.events.len()
    }
    
    /// Add a waiter (PPU thread waiting for events)
    pub fn add_waiter(&mut self, thread_id: u32) {
        if !self.waiters.contains(&thread_id) {
            self.waiters.push(thread_id);
        }
    }
    
    /// Remove a waiter
    pub fn remove_waiter(&mut self, thread_id: u32) {
        self.waiters.retain(|&id| id != thread_id);
    }
    
    /// Get waiters
    pub fn waiters(&self) -> &[u32] {
        &self.waiters
    }
}

impl Default for SpuEventQueue {
    fn default() -> Self {
        Self::new(0, 64)
    }
}

/// SPU thread group
#[derive(Debug)]
pub struct SpuThreadGroup {
    /// Group ID
    pub id: u32,
    /// Group name
    pub name: String,
    /// Thread IDs in this group
    pub thread_ids: Vec<u32>,
    /// Group priority
    pub priority: SpuPriority,
    /// Group state
    pub state: SpuGroupState,
    /// Event queue ID (if attached)
    pub event_queue_id: Option<u32>,
    /// Group-level exception mask
    pub exception_mask: u32,
    /// Whether the group is joinable
    pub joinable: bool,
    /// Exit status (when all threads finish)
    pub exit_status: i32,
}

/// SPU thread group state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpuGroupState {
    /// Not initialized
    NotInitialized,
    /// Initialized but not started
    Initialized,
    /// Running
    Running,
    /// Waiting (all threads waiting)
    Waiting,
    /// Suspended
    Suspended,
    /// Stopped (all threads stopped)
    Stopped,
}

impl SpuThreadGroup {
    /// Create a new SPU thread group
    pub fn new(id: u32, name: &str, num_threads: usize, priority: SpuPriority) -> Self {
        Self {
            id,
            name: name.to_string(),
            thread_ids: Vec::with_capacity(num_threads),
            priority,
            state: SpuGroupState::NotInitialized,
            event_queue_id: None,
            exception_mask: 0xFFFFFFFF,
            joinable: true,
            exit_status: 0,
        }
    }
    
    /// Add a thread to the group
    pub fn add_thread(&mut self, thread_id: u32) -> bool {
        if self.thread_ids.len() < MAX_SPU_THREADS_PER_GROUP {
            self.thread_ids.push(thread_id);
            if self.state == SpuGroupState::NotInitialized {
                self.state = SpuGroupState::Initialized;
            }
            true
        } else {
            false
        }
    }
    
    /// Remove a thread from the group
    pub fn remove_thread(&mut self, thread_id: u32) {
        self.thread_ids.retain(|&id| id != thread_id);
    }
    
    /// Start the group
    pub fn start(&mut self) {
        if self.state == SpuGroupState::Initialized || self.state == SpuGroupState::Stopped {
            self.state = SpuGroupState::Running;
        }
    }
    
    /// Stop the group
    pub fn stop(&mut self) {
        self.state = SpuGroupState::Stopped;
    }
    
    /// Suspend the group
    pub fn suspend(&mut self) {
        if self.state == SpuGroupState::Running {
            self.state = SpuGroupState::Suspended;
        }
    }
    
    /// Resume the group
    pub fn resume(&mut self) {
        if self.state == SpuGroupState::Suspended {
            self.state = SpuGroupState::Running;
        }
    }
    
    /// Attach an event queue to the group
    pub fn attach_event_queue(&mut self, queue_id: u32) {
        self.event_queue_id = Some(queue_id);
    }
    
    /// Detach the event queue
    pub fn detach_event_queue(&mut self) {
        self.event_queue_id = None;
    }
}

/// SPU isolation mode state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationState {
    /// Not in isolation mode
    Normal,
    /// Entering isolation mode
    Entering,
    /// Fully isolated
    Isolated,
    /// Exiting isolation mode  
    Exiting,
}

impl Default for IsolationState {
    fn default() -> Self {
        Self::Normal
    }
}

/// SPU register set (128 x 128-bit)
#[derive(Clone)]
pub struct SpuRegisters {
    /// General Purpose Registers (128-bit each)
    pub gpr: [[u32; 4]; 128],
    /// Program Counter (instruction address in local storage)
    pub pc: u32,
}

impl Default for SpuRegisters {
    fn default() -> Self {
        Self {
            gpr: [[0; 4]; 128],
            pc: 0,
        }
    }
}

impl SpuRegisters {
    /// Read a register as 4 x u32
    #[inline]
    pub fn read_u32x4(&self, index: usize) -> [u32; 4] {
        self.gpr[index]
    }

    /// Write a register as 4 x u32
    #[inline]
    pub fn write_u32x4(&mut self, index: usize, value: [u32; 4]) {
        self.gpr[index] = value;
    }

    /// Read preferred slot (word 0) as u32
    #[inline]
    pub fn read_preferred_u32(&self, index: usize) -> u32 {
        self.gpr[index][0]
    }

    /// Write to preferred slot (word 0)
    #[inline]
    pub fn write_preferred_u32(&mut self, index: usize, value: u32) {
        self.gpr[index] = [value, 0, 0, 0];
    }
}

/// SPU thread
pub struct SpuThread {
    /// SPU ID (0-5 for PS3)
    pub id: u32,
    /// Thread name
    pub name: String,
    /// Register state
    pub regs: SpuRegisters,
    /// Local storage (256 KB)
    pub local_storage: Box<[u8; SPU_LS_SIZE]>,
    /// Thread state
    pub state: SpuThreadState,
    /// MFC (Memory Flow Controller)
    pub mfc: Mfc,
    /// SPU Channels
    pub channels: SpuChannels,
    /// Reference to main memory
    memory: Arc<MemoryManager>,
    /// Interrupt enabled
    pub interrupt_enabled: bool,
    /// Stop and signal value
    pub stop_signal: u32,
    /// Thread priority
    pub priority: SpuPriority,
    /// Thread affinity
    pub affinity: SpuAffinity,
    /// Exception state
    pub exceptions: SpuExceptionState,
    /// Thread group ID (if part of a group)
    pub group_id: Option<u32>,
    /// Isolation mode state
    pub isolation: IsolationState,
    /// Entry point address
    pub entry_point: u32,
    /// Argument passed to thread
    pub arg: u64,
    /// Exit value (when thread stops)
    pub exit_value: i32,
    /// Total cycles executed
    pub cycles: u64,
}

impl SpuThread {
    /// Create a new SPU thread
    pub fn new(id: u32, memory: Arc<MemoryManager>) -> Self {
        Self {
            id,
            name: format!("SPU Thread {}", id),
            regs: SpuRegisters::default(),
            local_storage: Box::new([0; SPU_LS_SIZE]),
            state: SpuThreadState::Stopped,
            mfc: Mfc::new(),
            channels: SpuChannels::new(),
            memory,
            interrupt_enabled: false,
            stop_signal: 0,
            priority: SpuPriority::default(),
            affinity: SpuAffinity::default(),
            exceptions: SpuExceptionState::default(),
            group_id: None,
            isolation: IsolationState::default(),
            entry_point: 0,
            arg: 0,
            exit_value: 0,
            cycles: 0,
        }
    }
    
    /// Create a new SPU thread with configuration
    pub fn new_with_config(
        id: u32,
        memory: Arc<MemoryManager>,
        entry_point: u32,
        arg: u64,
        priority: SpuPriority,
    ) -> Self {
        let mut thread = Self::new(id, memory);
        thread.entry_point = entry_point;
        thread.arg = arg;
        thread.priority = priority;
        thread.set_pc(entry_point);
        thread
    }
    
    /// Set thread priority
    pub fn set_priority(&mut self, priority: SpuPriority) {
        self.priority = priority;
    }
    
    /// Get thread priority
    pub fn get_priority(&self) -> SpuPriority {
        self.priority
    }
    
    /// Set thread affinity
    pub fn set_affinity(&mut self, affinity: SpuAffinity) {
        self.affinity = affinity;
    }
    
    /// Get thread affinity
    pub fn get_affinity(&self) -> SpuAffinity {
        self.affinity
    }
    
    /// Check if thread can run on specified SPU
    pub fn can_run_on_spu(&self, spu_id: u8) -> bool {
        self.affinity.allows_spu(spu_id)
    }
    
    /// Raise an exception
    pub fn raise_exception(&mut self, exception: SpuExceptionType) {
        self.exceptions.raise(exception, self.pc());
        self.state = SpuThreadState::Exception;
    }
    
    /// Clear exception and resume execution
    pub fn clear_exception(&mut self) {
        self.exceptions.clear();
        if self.state == SpuThreadState::Exception {
            self.state = SpuThreadState::Ready;
        }
    }
    
    /// Enter isolation mode
    /// 
    /// In isolation mode, the SPU's local storage is encrypted and protected.
    /// The PPU cannot access the SPU's local storage or registers.
    /// This is used for secure code execution (e.g., SPU security modules).
    pub fn enter_isolation(&mut self) {
        if self.isolation == IsolationState::Normal {
            // Transition directly to isolated state
            // The Entering state is tracked internally for debugging/introspection
            self.isolation = IsolationState::Isolated;
            self.state = SpuThreadState::Isolated;
        }
    }
    
    /// Exit isolation mode
    ///
    /// Clears the isolation flag and returns the SPU to normal operation.
    /// Note: In a real PS3, exiting isolation would also clear sensitive data.
    pub fn exit_isolation(&mut self) {
        if self.isolation == IsolationState::Isolated {
            // Transition directly to normal state
            self.isolation = IsolationState::Normal;
            if self.state == SpuThreadState::Isolated {
                self.state = SpuThreadState::Ready;
            }
        }
    }
    
    /// Check if in isolation mode
    pub fn is_isolated(&self) -> bool {
        self.isolation == IsolationState::Isolated
    }
    
    /// Set group ID
    pub fn set_group(&mut self, group_id: u32) {
        self.group_id = Some(group_id);
    }
    
    /// Clear group ID
    pub fn clear_group(&mut self) {
        self.group_id = None;
    }
    
    /// Exit the thread with a value
    pub fn exit(&mut self, value: i32) {
        self.exit_value = value;
        self.state = SpuThreadState::Stopped;
    }
    
    /// Check if thread is runnable
    pub fn is_runnable(&self) -> bool {
        matches!(self.state, SpuThreadState::Running | SpuThreadState::Ready)
    }
    
    /// Make thread ready to run
    pub fn make_ready(&mut self) {
        if self.state != SpuThreadState::Stopped && self.state != SpuThreadState::Exception {
            self.state = SpuThreadState::Ready;
        }
    }
    
    /// Add executed cycles
    pub fn add_cycles(&mut self, count: u64) {
        self.cycles = self.cycles.wrapping_add(count);
    }

    /// Get the current program counter
    pub fn pc(&self) -> u32 {
        self.regs.pc
    }

    /// Set the program counter
    pub fn set_pc(&mut self, addr: u32) {
        self.regs.pc = addr & (SPU_LS_SIZE as u32 - 1);
    }

    /// Advance the program counter by 4 bytes
    pub fn advance_pc(&mut self) {
        self.regs.pc = (self.regs.pc + 4) & (SPU_LS_SIZE as u32 - 1);
    }

    /// Read from local storage (u32, big-endian)
    #[inline]
    pub fn ls_read_u32(&self, addr: u32) -> u32 {
        let addr = (addr & (SPU_LS_SIZE as u32 - 1)) as usize;
        u32::from_be_bytes([
            self.local_storage[addr],
            self.local_storage[addr + 1],
            self.local_storage[addr + 2],
            self.local_storage[addr + 3],
        ])
    }

    /// Write to local storage (u32, big-endian)
    #[inline]
    pub fn ls_write_u32(&mut self, addr: u32, value: u32) {
        let addr = (addr & (SPU_LS_SIZE as u32 - 1)) as usize;
        let bytes = value.to_be_bytes();
        self.local_storage[addr] = bytes[0];
        self.local_storage[addr + 1] = bytes[1];
        self.local_storage[addr + 2] = bytes[2];
        self.local_storage[addr + 3] = bytes[3];
    }

    /// Read from local storage (128-bit, big-endian)
    #[inline]
    pub fn ls_read_u128(&self, addr: u32) -> [u32; 4] {
        let addr = (addr & (SPU_LS_SIZE as u32 - 1) & !0xF) as usize;
        [
            u32::from_be_bytes([
                self.local_storage[addr],
                self.local_storage[addr + 1],
                self.local_storage[addr + 2],
                self.local_storage[addr + 3],
            ]),
            u32::from_be_bytes([
                self.local_storage[addr + 4],
                self.local_storage[addr + 5],
                self.local_storage[addr + 6],
                self.local_storage[addr + 7],
            ]),
            u32::from_be_bytes([
                self.local_storage[addr + 8],
                self.local_storage[addr + 9],
                self.local_storage[addr + 10],
                self.local_storage[addr + 11],
            ]),
            u32::from_be_bytes([
                self.local_storage[addr + 12],
                self.local_storage[addr + 13],
                self.local_storage[addr + 14],
                self.local_storage[addr + 15],
            ]),
        ]
    }

    /// Write to local storage (128-bit, big-endian)
    #[inline]
    pub fn ls_write_u128(&mut self, addr: u32, value: [u32; 4]) {
        let addr = (addr & (SPU_LS_SIZE as u32 - 1) & !0xF) as usize;
        for (i, word) in value.iter().enumerate() {
            let bytes = word.to_be_bytes();
            let offset = addr + i * 4;
            self.local_storage[offset] = bytes[0];
            self.local_storage[offset + 1] = bytes[1];
            self.local_storage[offset + 2] = bytes[2];
            self.local_storage[offset + 3] = bytes[3];
        }
    }

    /// Get reference to main memory
    pub fn memory(&self) -> &Arc<MemoryManager> {
        &self.memory
    }

    /// Start the thread
    pub fn start(&mut self) {
        self.state = SpuThreadState::Running;
    }

    /// Stop the thread
    pub fn stop(&mut self) {
        self.state = SpuThreadState::Stopped;
    }

    /// Check if thread is running
    pub fn is_running(&self) -> bool {
        self.state == SpuThreadState::Running
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_memory() -> Arc<MemoryManager> {
        MemoryManager::new().unwrap()
    }

    #[test]
    fn test_spu_thread_creation() {
        let mem = create_test_memory();
        let thread = SpuThread::new(0, mem);
        
        assert_eq!(thread.id, 0);
        assert_eq!(thread.state, SpuThreadState::Stopped);
        assert_eq!(thread.pc(), 0);
    }

    #[test]
    fn test_local_storage_access() {
        let mem = create_test_memory();
        let mut thread = SpuThread::new(0, mem);

        thread.ls_write_u32(0x100, 0x12345678);
        assert_eq!(thread.ls_read_u32(0x100), 0x12345678);
    }

    #[test]
    fn test_local_storage_u128() {
        let mem = create_test_memory();
        let mut thread = SpuThread::new(0, mem);

        let value = [0x11111111, 0x22222222, 0x33333333, 0x44444444];
        thread.ls_write_u128(0x100, value);
        assert_eq!(thread.ls_read_u128(0x100), value);
    }

    #[test]
    fn test_pc_wrap() {
        let mem = create_test_memory();
        let mut thread = SpuThread::new(0, mem);

        thread.set_pc(SPU_LS_SIZE as u32 + 0x100);
        assert_eq!(thread.pc(), 0x100);
    }
    
    #[test]
    fn test_spu_priority() {
        let mem = create_test_memory();
        let mut thread = SpuThread::new(0, mem);
        
        // Default priority
        assert_eq!(thread.get_priority(), SpuPriority::NORMAL);
        
        // Set priority
        thread.set_priority(SpuPriority::HIGH);
        assert_eq!(thread.get_priority().value(), SpuPriority::HIGH.value());
    }
    
    #[test]
    fn test_spu_affinity() {
        let mem = create_test_memory();
        let mut thread = SpuThread::new(0, mem);
        
        // Default is any SPU
        assert_eq!(thread.get_affinity(), SpuAffinity::ANY);
        assert!(thread.can_run_on_spu(0));
        assert!(thread.can_run_on_spu(5));
        
        // Set specific SPU
        thread.set_affinity(SpuAffinity::specific(2));
        assert!(!thread.can_run_on_spu(0));
        assert!(thread.can_run_on_spu(2));
    }
    
    #[test]
    fn test_spu_exception() {
        let mem = create_test_memory();
        let mut thread = SpuThread::new(0, mem);
        
        thread.set_pc(0x1000);
        thread.state = SpuThreadState::Running;
        
        // Raise exception
        thread.raise_exception(SpuExceptionType::InvalidInstruction);
        assert_eq!(thread.state, SpuThreadState::Exception);
        assert!(thread.exceptions.has_exception());
        assert_eq!(thread.exceptions.exception_pc, 0x1000);
        
        // Clear exception
        thread.clear_exception();
        assert_eq!(thread.state, SpuThreadState::Ready);
        assert!(!thread.exceptions.has_exception());
    }
    
    #[test]
    fn test_spu_isolation() {
        let mem = create_test_memory();
        let mut thread = SpuThread::new(0, mem);
        
        thread.state = SpuThreadState::Running;
        
        // Enter isolation mode
        assert!(!thread.is_isolated());
        thread.enter_isolation();
        assert!(thread.is_isolated());
        assert_eq!(thread.state, SpuThreadState::Isolated);
        
        // Exit isolation mode
        thread.exit_isolation();
        assert!(!thread.is_isolated());
        assert_eq!(thread.state, SpuThreadState::Ready);
    }
    
    #[test]
    fn test_spu_thread_group() {
        let mut group = SpuThreadGroup::new(1, "test_group", 4, SpuPriority::NORMAL);
        
        assert_eq!(group.id, 1);
        assert_eq!(group.state, SpuGroupState::NotInitialized);
        
        // Add threads
        assert!(group.add_thread(0));
        assert!(group.add_thread(1));
        assert_eq!(group.thread_ids.len(), 2);
        assert_eq!(group.state, SpuGroupState::Initialized);
        
        // Start group
        group.start();
        assert_eq!(group.state, SpuGroupState::Running);
        
        // Suspend
        group.suspend();
        assert_eq!(group.state, SpuGroupState::Suspended);
        
        // Resume
        group.resume();
        assert_eq!(group.state, SpuGroupState::Running);
        
        // Stop
        group.stop();
        assert_eq!(group.state, SpuGroupState::Stopped);
        
        // Remove thread
        group.remove_thread(0);
        assert_eq!(group.thread_ids.len(), 1);
    }
    
    #[test]
    fn test_spu_event_queue() {
        let mut queue = SpuEventQueue::new(1, 16);
        
        assert!(queue.is_empty());
        
        // Push event
        let event = SpuEvent {
            source: 0,
            data: 0x42,
            event_type: SpuEventType::ThreadFinish,
        };
        assert!(queue.push(event));
        assert_eq!(queue.len(), 1);
        
        // Pop event
        let popped = queue.pop().unwrap();
        assert_eq!(popped.data, 0x42);
        assert!(queue.is_empty());
        
        // Test waiters
        queue.add_waiter(100);
        assert_eq!(queue.waiters().len(), 1);
        queue.remove_waiter(100);
        assert!(queue.waiters().is_empty());
    }
    
    #[test]
    fn test_spu_thread_with_config() {
        let mem = create_test_memory();
        let thread = SpuThread::new_with_config(
            0,
            mem,
            0x1000,  // entry point
            0xDEAD,  // argument
            SpuPriority::HIGH,
        );
        
        assert_eq!(thread.pc(), 0x1000);
        assert_eq!(thread.arg, 0xDEAD);
        assert_eq!(thread.priority, SpuPriority::HIGH);
    }
}
