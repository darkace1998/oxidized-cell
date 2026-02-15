//! SPU management (sys_spu_*)

use crate::objects::{KernelObject, ObjectId, ObjectManager, ObjectType};
use oc_core::error::KernelError;
use parking_lot::Mutex;
use std::collections::{BinaryHeap, VecDeque};
use std::sync::Arc;

/// Maximum number of SPU threads per thread group
const MAX_SPU_THREADS: u32 = 6;

/// SPU local storage size
const SPU_LS_SIZE: u32 = 256 * 1024; // 256KB

/// Number of physical SPU run slots (Cell BE has 6 usable SPUs)
const NUM_SPU_SLOTS: usize = 6;

/// SPU thread group attributes
#[derive(Debug, Clone)]
pub struct SpuThreadGroupAttributes {
    pub name: String,
    pub priority: u32,
    pub thread_type: u32,
}

impl Default for SpuThreadGroupAttributes {
    fn default() -> Self {
        Self {
            name: String::from("SPU_TG"),
            priority: 100,
            thread_type: 0,
        }
    }
}

/// SPU thread attributes
#[derive(Debug, Clone)]
pub struct SpuThreadAttributes {
    pub name: String,
    pub option: u32,
}

impl Default for SpuThreadAttributes {
    fn default() -> Self {
        Self {
            name: String::from("SPU_Thread"),
            option: 0,
        }
    }
}

/// SPU image information
#[derive(Debug, Clone)]
pub struct SpuImage {
    pub entry_point: u32,
    pub local_storage_size: u32,
    pub segments: Vec<SpuSegment>,
}

#[derive(Debug, Clone)]
pub struct SpuSegment {
    pub addr: u32,
    pub size: u32,
    pub data: Vec<u8>,
}

/// SPU thread group
pub struct SpuThreadGroup {
    id: ObjectId,
    inner: Mutex<SpuThreadGroupState>,
    _attributes: SpuThreadGroupAttributes,
}

#[derive(Debug)]
struct SpuThreadGroupState {
    threads: Vec<ObjectId>,
    status: SpuThreadGroupStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpuThreadGroupStatus {
    NotInitialized,
    #[allow(dead_code)]
    Initialized,
    Running,
    Stopped,
}

impl SpuThreadGroup {
    pub fn new(id: ObjectId, attributes: SpuThreadGroupAttributes, num_threads: u32) -> Self {
        Self {
            id,
            inner: Mutex::new(SpuThreadGroupState {
                threads: Vec::with_capacity(num_threads as usize),
                status: SpuThreadGroupStatus::NotInitialized,
            }),
            _attributes: attributes,
        }
    }

    pub fn add_thread(&self, thread_id: ObjectId) -> Result<(), KernelError> {
        let mut state = self.inner.lock();
        state.threads.push(thread_id);
        Ok(())
    }

    pub fn start(&self) -> Result<(), KernelError> {
        let mut state = self.inner.lock();
        if state.status == SpuThreadGroupStatus::Running {
            return Err(KernelError::PermissionDenied);
        }
        state.status = SpuThreadGroupStatus::Running;
        tracing::debug!("Started SPU thread group {}", self.id);
        Ok(())
    }

    pub fn join(&self) -> Result<(), KernelError> {
        let mut state = self.inner.lock();
        if state.status != SpuThreadGroupStatus::Running {
            return Err(KernelError::PermissionDenied);
        }
        state.status = SpuThreadGroupStatus::Stopped;
        tracing::debug!("Joined SPU thread group {}", self.id);
        Ok(())
    }
}

impl KernelObject for SpuThreadGroup {
    fn object_type(&self) -> ObjectType {
        ObjectType::SpuThreadGroup
    }

    fn id(&self) -> ObjectId {
        self.id
    }

    fn as_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

/// SPU thread
pub struct SpuThread {
    id: ObjectId,
    group_id: ObjectId,
    inner: Mutex<SpuThreadState>,
    _attributes: SpuThreadAttributes,
}

#[derive(Debug)]
struct SpuThreadState {
    image: Option<SpuImage>,
    status: SpuThreadStatus,
    local_storage: Vec<u8>,
    signals: SpuSignals,
    mailbox: SpuMailbox,
}

/// SPU signal management
#[derive(Debug, Clone, Copy)]
struct SpuSignals {
    signal1: u32,
    signal2: u32,
}

/// SPU mailbox state for SPU↔PPU communication
#[derive(Debug, Clone)]
struct SpuMailbox {
    /// Outbound mailbox: SPU writes, PPU reads (FIFO, up to 4 entries)
    outbound: VecDeque<u32>,
    /// Inbound mailbox: PPU writes, SPU reads (FIFO, up to 4 entries)
    inbound: VecDeque<u32>,
}

impl SpuMailbox {
    fn new() -> Self {
        Self {
            outbound: VecDeque::with_capacity(4),
            inbound: VecDeque::with_capacity(4),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum SpuThreadStatus {
    NotInitialized,
    Initialized,
    Running,
    Stopped,
}

impl SpuThread {
    pub fn new(id: ObjectId, group_id: ObjectId, attributes: SpuThreadAttributes) -> Self {
        Self {
            id,
            group_id,
            inner: Mutex::new(SpuThreadState {
                image: None,
                status: SpuThreadStatus::NotInitialized,
                local_storage: vec![0u8; SPU_LS_SIZE as usize],
                signals: SpuSignals {
                    signal1: 0,
                    signal2: 0,
                },
                mailbox: SpuMailbox::new(),
            }),
            _attributes: attributes,
        }
    }

    pub fn initialize(&self, image: SpuImage) -> Result<(), KernelError> {
        let mut state = self.inner.lock();
        state.image = Some(image);
        state.status = SpuThreadStatus::Initialized;
        tracing::debug!("Initialized SPU thread {}", self.id);
        Ok(())
    }

    pub fn get_group_id(&self) -> ObjectId {
        self.group_id
    }

    /// Write signal to SPU thread
    pub fn write_signal(&self, signal_reg: u32, value: u32) -> Result<(), KernelError> {
        let mut state = self.inner.lock();
        match signal_reg {
            1 => state.signals.signal1 = value,
            2 => state.signals.signal2 = value,
            _ => return Err(KernelError::InvalidId(self.id)),
        }
        tracing::debug!("SPU thread {} signal{} = 0x{:x}", self.id, signal_reg, value);
        Ok(())
    }

    /// Read signal from SPU thread
    pub fn read_signal(&self, signal_reg: u32) -> Result<u32, KernelError> {
        let state = self.inner.lock();
        match signal_reg {
            1 => Ok(state.signals.signal1),
            2 => Ok(state.signals.signal2),
            _ => Err(KernelError::InvalidId(self.id)),
        }
    }

    /// Write to local storage
    pub fn write_ls(&self, addr: u32, data: &[u8]) -> Result<(), KernelError> {
        let mut state = self.inner.lock();
        let addr = addr as usize;
        
        if addr + data.len() > state.local_storage.len() {
            return Err(KernelError::PermissionDenied);
        }
        
        state.local_storage[addr..addr + data.len()].copy_from_slice(data);
        tracing::debug!("SPU thread {} wrote {} bytes to LS at 0x{:x}", self.id, data.len(), addr);
        Ok(())
    }

    /// Read from local storage
    pub fn read_ls(&self, addr: u32, size: u32) -> Result<Vec<u8>, KernelError> {
        let state = self.inner.lock();
        let addr = addr as usize;
        let size = size as usize;
        
        if addr + size > state.local_storage.len() {
            return Err(KernelError::PermissionDenied);
        }
        
        Ok(state.local_storage[addr..addr + size].to_vec())
    }

    /// Write a value to the SPU outbound mailbox (SPU → PPU)
    pub fn write_outbound_mailbox(&self, value: u32) -> Result<(), KernelError> {
        let mut state = self.inner.lock();
        if state.mailbox.outbound.len() >= 4 {
            return Err(KernelError::ResourceLimit);
        }
        state.mailbox.outbound.push_back(value);
        tracing::debug!("SPU thread {} outbound mailbox write: 0x{:08x}", self.id, value);
        Ok(())
    }

    /// Read a value from the SPU outbound mailbox (PPU reads what SPU wrote)
    pub fn read_outbound_mailbox(&self) -> Result<u32, KernelError> {
        let mut state = self.inner.lock();
        state.mailbox.outbound.pop_front()
            .ok_or(KernelError::WouldBlock)
    }

    /// Write a value to the SPU inbound mailbox (PPU → SPU)
    pub fn write_inbound_mailbox(&self, value: u32) -> Result<(), KernelError> {
        let mut state = self.inner.lock();
        if state.mailbox.inbound.len() >= 4 {
            return Err(KernelError::ResourceLimit);
        }
        state.mailbox.inbound.push_back(value);
        tracing::debug!("SPU thread {} inbound mailbox write: 0x{:08x}", self.id, value);
        Ok(())
    }

    /// Read a value from the SPU inbound mailbox (SPU reads what PPU wrote)
    pub fn read_inbound_mailbox(&self) -> Result<u32, KernelError> {
        let mut state = self.inner.lock();
        state.mailbox.inbound.pop_front()
            .ok_or(KernelError::WouldBlock)
    }

    /// Get number of pending outbound mailbox entries
    pub fn outbound_mailbox_count(&self) -> usize {
        self.inner.lock().mailbox.outbound.len()
    }

    /// Get number of pending inbound mailbox entries
    pub fn inbound_mailbox_count(&self) -> usize {
        self.inner.lock().mailbox.inbound.len()
    }
}

impl KernelObject for SpuThread {
    fn object_type(&self) -> ObjectType {
        ObjectType::SpuThread
    }

    fn id(&self) -> ObjectId {
        self.id
    }

    fn as_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

/// SPU syscall implementations
pub mod syscalls {
    use super::*;

    /// sys_spu_thread_group_create
    pub fn sys_spu_thread_group_create(
        manager: &ObjectManager,
        attributes: SpuThreadGroupAttributes,
        num_threads: u32,
        _priority: i32,
    ) -> Result<ObjectId, KernelError> {
        if num_threads == 0 || num_threads > MAX_SPU_THREADS {
            return Err(KernelError::ResourceLimit);
        }

        let id = manager.next_id();
        let group = Arc::new(SpuThreadGroup::new(id, attributes, num_threads));
        manager.register(group);
        Ok(id)
    }

    /// sys_spu_thread_group_destroy
    pub fn sys_spu_thread_group_destroy(
        manager: &ObjectManager,
        group_id: ObjectId,
    ) -> Result<(), KernelError> {
        manager.unregister(group_id)
    }

    /// sys_spu_thread_group_start
    pub fn sys_spu_thread_group_start(
        manager: &ObjectManager,
        group_id: ObjectId,
    ) -> Result<(), KernelError> {
        let group: Arc<SpuThreadGroup> = manager.get(group_id)?;
        group.start()
    }

    /// sys_spu_thread_group_join
    pub fn sys_spu_thread_group_join(
        manager: &ObjectManager,
        group_id: ObjectId,
    ) -> Result<(), KernelError> {
        let group: Arc<SpuThreadGroup> = manager.get(group_id)?;
        group.join()
    }

    /// sys_spu_thread_initialize
    pub fn sys_spu_thread_initialize(
        manager: &ObjectManager,
        group_id: ObjectId,
        _thread_num: u32,
        attributes: SpuThreadAttributes,
    ) -> Result<ObjectId, KernelError> {
        let group: Arc<SpuThreadGroup> = manager.get(group_id)?;

        let thread_id = manager.next_id();
        let thread = Arc::new(SpuThread::new(thread_id, group_id, attributes));

        group.add_thread(thread_id)?;
        manager.register(thread);

        Ok(thread_id)
    }

    /// sys_spu_image_open
    pub fn sys_spu_image_open(
        manager: &ObjectManager,
        thread_id: ObjectId,
        entry_point: u32,
    ) -> Result<(), KernelError> {
        let thread: Arc<SpuThread> = manager.get(thread_id)?;

        // Create a simple image with just the entry point
        let image = SpuImage {
            entry_point,
            local_storage_size: SPU_LS_SIZE,
            segments: Vec::new(),
        };

        thread.initialize(image)
    }

    /// sys_spu_thread_write_ls
    pub fn sys_spu_thread_write_ls(
        manager: &ObjectManager,
        thread_id: ObjectId,
        addr: u32,
        data: &[u8],
    ) -> Result<(), KernelError> {
        let thread: Arc<SpuThread> = manager.get(thread_id)?;
        thread.write_ls(addr, data)
    }

    /// sys_spu_thread_read_ls
    pub fn sys_spu_thread_read_ls(
        manager: &ObjectManager,
        thread_id: ObjectId,
        addr: u32,
        size: u32,
    ) -> Result<Vec<u8>, KernelError> {
        let thread: Arc<SpuThread> = manager.get(thread_id)?;
        thread.read_ls(addr, size)
    }

    /// sys_spu_thread_write_signal
    pub fn sys_spu_thread_write_signal(
        manager: &ObjectManager,
        thread_id: ObjectId,
        signal_reg: u32,
        value: u32,
    ) -> Result<(), KernelError> {
        let thread: Arc<SpuThread> = manager.get(thread_id)?;
        thread.write_signal(signal_reg, value)
    }

    /// sys_spu_thread_read_signal
    pub fn sys_spu_thread_read_signal(
        manager: &ObjectManager,
        thread_id: ObjectId,
        signal_reg: u32,
    ) -> Result<u32, KernelError> {
        let thread: Arc<SpuThread> = manager.get(thread_id)?;
        thread.read_signal(signal_reg)
    }

    /// Validate a DMA transfer size (must be 1–16384 bytes)
    fn validate_dma_size(size: u32) -> Result<(), KernelError> {
        if size == 0 || size > 16384 {
            return Err(KernelError::InvalidArgument);
        }
        Ok(())
    }

    /// Validate that an effective address range fits within a memory slice
    fn validate_ea_bounds(ea_addr: u64, size: u32, mem_len: usize) -> Result<(usize, usize), KernelError> {
        let ea_start = ea_addr as usize;
        let ea_end = ea_start.saturating_add(size as usize);
        if ea_end > mem_len {
            return Err(KernelError::PermissionDenied);
        }
        Ok((ea_start, ea_end))
    }

    /// sys_spu_thread_transfer_data — DMA GET operation
    ///
    /// Transfer data from main memory to SPU local storage.
    /// Implements MFC_GET semantics (main memory → LS).
    ///
    /// # Arguments
    /// * `thread_id` - SPU thread to transfer data to
    /// * `ls_addr` - Destination address in local storage
    /// * `ea_addr` - Source effective address in main memory
    /// * `size` - Number of bytes to transfer (max 16384, must be 1/2/4/8/16-byte aligned for < 16 bytes)
    /// * `main_memory` - Main memory slice to read from
    pub fn sys_spu_thread_transfer_data_get(
        manager: &ObjectManager,
        thread_id: ObjectId,
        ls_addr: u32,
        ea_addr: u64,
        size: u32,
        main_memory: &[u8],
    ) -> Result<(), KernelError> {
        validate_dma_size(size)?;
        let thread: Arc<SpuThread> = manager.get(thread_id)?;
        let (ea_start, ea_end) = validate_ea_bounds(ea_addr, size, main_memory.len())?;
        thread.write_ls(ls_addr, &main_memory[ea_start..ea_end])
    }

    /// sys_spu_thread_transfer_data — DMA PUT operation
    ///
    /// Transfer data from SPU local storage to main memory.
    /// Implements MFC_PUT semantics (LS → main memory).
    ///
    /// # Arguments
    /// * `thread_id` - SPU thread to transfer data from
    /// * `ls_addr` - Source address in local storage
    /// * `ea_addr` - Destination effective address in main memory
    /// * `size` - Number of bytes to transfer (max 16384)
    /// * `main_memory` - Main memory slice to write to
    pub fn sys_spu_thread_transfer_data_put(
        manager: &ObjectManager,
        thread_id: ObjectId,
        ls_addr: u32,
        ea_addr: u64,
        size: u32,
        main_memory: &mut [u8],
    ) -> Result<(), KernelError> {
        validate_dma_size(size)?;
        let thread: Arc<SpuThread> = manager.get(thread_id)?;
        let data = thread.read_ls(ls_addr, size)?;
        let (ea_start, ea_end) = validate_ea_bounds(ea_addr, size, main_memory.len())?;
        main_memory[ea_start..ea_end].copy_from_slice(&data);
        Ok(())
    }

    /// sys_spu_thread_atomic_get — MFC_GETLLAR operation
    ///
    /// Atomically reads a 128-byte cache line from main memory and establishes
    /// a reservation. The reservation is invalidated if another agent writes
    /// to the same cache line.
    ///
    /// # Arguments
    /// * `thread_id` - SPU thread
    /// * `ls_addr` - Destination address in local storage (must be 128-byte aligned)
    /// * `ea_addr` - Source effective address in main memory (must be 128-byte aligned)
    /// * `main_memory` - Main memory slice
    pub fn sys_spu_thread_atomic_get(
        manager: &ObjectManager,
        thread_id: ObjectId,
        ls_addr: u32,
        ea_addr: u64,
        main_memory: &[u8],
    ) -> Result<(), KernelError> {
        // GETLLAR is always 128 bytes, must be 128-byte aligned
        if (ls_addr & 0x7F) != 0 || (ea_addr & 0x7F) != 0 {
            return Err(KernelError::InvalidArgument);
        }

        let thread: Arc<SpuThread> = manager.get(thread_id)?;
        let (ea_start, _) = validate_ea_bounds(ea_addr, 128, main_memory.len())?;
        thread.write_ls(ls_addr, &main_memory[ea_start..ea_start + 128])
    }

    /// sys_spu_thread_atomic_put — MFC_PUTLLC operation
    ///
    /// Conditionally writes a 128-byte cache line from SPU local storage to
    /// main memory, succeeding only if the reservation is still valid.
    ///
    /// # Arguments
    /// * `thread_id` - SPU thread
    /// * `ls_addr` - Source address in local storage (must be 128-byte aligned)
    /// * `ea_addr` - Destination effective address in main memory (must be 128-byte aligned)
    /// * `main_memory` - Main memory slice
    ///
    /// # Returns
    /// * `Ok(true)` if the conditional store succeeded
    /// * `Ok(false)` if the reservation was lost
    pub fn sys_spu_thread_atomic_put(
        manager: &ObjectManager,
        thread_id: ObjectId,
        ls_addr: u32,
        ea_addr: u64,
        main_memory: &mut [u8],
    ) -> Result<bool, KernelError> {
        // PUTLLC is always 128 bytes, must be 128-byte aligned
        if (ls_addr & 0x7F) != 0 || (ea_addr & 0x7F) != 0 {
            return Err(KernelError::InvalidArgument);
        }

        let thread: Arc<SpuThread> = manager.get(thread_id)?;
        let data = thread.read_ls(ls_addr, 128)?;
        let (ea_start, _) = validate_ea_bounds(ea_addr, 128, main_memory.len())?;

        // In HLE mode, unconditionally succeed since we don't track reservations
        // at the system level (individual MFC instances track their own)
        main_memory[ea_start..ea_start + 128].copy_from_slice(&data);
        Ok(true)
    }

    /// sys_spu_thread_write_mailbox — PPU writes to SPU inbound mailbox
    ///
    /// Sends a 32-bit value from PPU to the SPU's inbound mailbox.
    ///
    /// # Arguments
    /// * `thread_id` - SPU thread to write to
    /// * `value` - 32-bit value to send
    pub fn sys_spu_thread_write_mailbox(
        manager: &ObjectManager,
        thread_id: ObjectId,
        value: u32,
    ) -> Result<(), KernelError> {
        let thread: Arc<SpuThread> = manager.get(thread_id)?;
        thread.write_inbound_mailbox(value)
    }

    /// sys_spu_thread_read_mailbox — PPU reads from SPU outbound mailbox
    ///
    /// Reads a 32-bit value that the SPU wrote to its outbound mailbox.
    ///
    /// # Arguments
    /// * `thread_id` - SPU thread to read from
    ///
    /// # Returns
    /// * `Ok(value)` if a value was available
    /// * `Err(WouldBlock)` if the mailbox is empty
    pub fn sys_spu_thread_read_mailbox(
        manager: &ObjectManager,
        thread_id: ObjectId,
    ) -> Result<u32, KernelError> {
        let thread: Arc<SpuThread> = manager.get(thread_id)?;
        thread.read_outbound_mailbox()
    }

    /// sys_spu_thread_transfer_data_list — MFC list DMA operation
    ///
    /// Transfer a list of DMA operations (scatter-gather) between
    /// SPU local storage and main memory.
    ///
    /// Each entry in the list is an (ea_addr, ls_addr, size) tuple.
    /// All individual transfers follow the same rules as GET/PUT.
    ///
    /// # Arguments
    /// * `thread_id` - SPU thread
    /// * `entries` - List of (ls_addr, ea_addr, size) transfer descriptors
    /// * `is_get` - true for GET (main→LS), false for PUT (LS→main)
    /// * `main_memory` - Main memory slice
    pub fn sys_spu_thread_transfer_data_list(
        manager: &ObjectManager,
        thread_id: ObjectId,
        entries: &[(u32, u64, u32)],
        is_get: bool,
        main_memory: &mut [u8],
    ) -> Result<(), KernelError> {
        let thread: Arc<SpuThread> = manager.get(thread_id)?;

        for &(ls_addr, ea_addr, size) in entries {
            validate_dma_size(size)?;
            if is_get {
                let (ea_start, ea_end) = validate_ea_bounds(ea_addr, size, main_memory.len())?;
                thread.write_ls(ls_addr, &main_memory[ea_start..ea_end])?;
            } else {
                let data = thread.read_ls(ls_addr, size)?;
                let (ea_start, ea_end) = validate_ea_bounds(ea_addr, size, main_memory.len())?;
                main_memory[ea_start..ea_end].copy_from_slice(&data);
            }
        }

        Ok(())
    }
}

/// SPU run slot state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpuSlotState {
    /// Slot is idle (no thread assigned)
    Idle,
    /// Slot is occupied by an SPU thread (thread_id, priority, group_id)
    Running(ObjectId, u32, ObjectId),
    /// Slot is suspended (thread yielded) (thread_id, priority, group_id)
    Suspended(ObjectId, u32, ObjectId),
}

/// Priority-ordered entry for the scheduler's run queue
#[derive(Debug, Clone, Eq, PartialEq)]
struct SchedulerEntry {
    priority: u32,
    group_id: ObjectId,
    thread_id: ObjectId,
}

impl Ord for SchedulerEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Lower priority number = higher priority (invert for max-heap)
        other.priority.cmp(&self.priority)
    }
}

impl PartialOrd for SchedulerEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// SPU scheduling statistics
#[derive(Debug, Clone, Default)]
pub struct SpuSchedulerStats {
    /// Total scheduling decisions made
    pub schedule_count: u64,
    /// Total preemptions (higher-priority thread displaced lower-priority)
    pub preemption_count: u64,
    /// Total yields (thread voluntarily gave up slot)
    pub yield_count: u64,
    /// Total suspends
    pub suspend_count: u64,
    /// Total resumes
    pub resume_count: u64,
}

/// SPU scheduler with priority-based run queue and round-robin time slicing
///
/// Manages assignment of SPU thread groups to physical SPU run slots.
/// The Cell BE has 6 usable SPUs; threads are scheduled based on priority
/// with preemption support.
pub struct SpuScheduler {
    inner: Mutex<SpuSchedulerInner>,
}

struct SpuSchedulerInner {
    /// Physical SPU run slots (6 for Cell BE)
    slots: [SpuSlotState; NUM_SPU_SLOTS],
    /// Priority queue of threads waiting for a slot
    run_queue: BinaryHeap<SchedulerEntry>,
    /// Scheduling statistics
    stats: SpuSchedulerStats,
    /// Round-robin index for tie-breaking same-priority threads
    rr_index: usize,
}

impl SpuScheduler {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(SpuSchedulerInner {
                slots: [SpuSlotState::Idle; NUM_SPU_SLOTS],
                run_queue: BinaryHeap::new(),
                stats: SpuSchedulerStats::default(),
                rr_index: 0,
            }),
        }
    }
    
    /// Submit a thread group for scheduling. Threads are enqueued by priority.
    pub fn submit(
        &self,
        group_id: ObjectId,
        thread_id: ObjectId,
        priority: u32,
    ) -> Result<(), KernelError> {
        let mut inner = self.inner.lock();
        inner.run_queue.push(SchedulerEntry {
            priority,
            group_id,
            thread_id,
        });
        tracing::debug!(
            "SPU scheduler: submitted thread {} (group {}, priority {})",
            thread_id, group_id, priority
        );
        Ok(())
    }
    
    /// Run one scheduling pass: assign highest-priority waiting threads to idle slots.
    /// Returns the number of threads that were assigned to slots.
    pub fn schedule(&self) -> usize {
        let mut inner = self.inner.lock();
        let mut assigned = 0;
        
        inner.stats.schedule_count += 1;
        
        // Find idle slots and assign highest-priority waiting threads
        for slot_idx in 0..NUM_SPU_SLOTS {
            if inner.slots[slot_idx] != SpuSlotState::Idle {
                continue;
            }
            
            if let Some(entry) = inner.run_queue.pop() {
                inner.slots[slot_idx] = SpuSlotState::Running(entry.thread_id, entry.priority, entry.group_id);
                assigned += 1;
                tracing::debug!(
                    "SPU scheduler: assigned thread {} to slot {} (priority {})",
                    entry.thread_id, slot_idx, entry.priority
                );
            }
        }
        
        // Round-robin index for time slicing
        inner.rr_index = (inner.rr_index + 1) % NUM_SPU_SLOTS;
        
        assigned
    }
    
    /// Preempt: if a higher-priority thread is waiting, displace the lowest-priority
    /// running thread. Returns (displaced_thread_id, new_thread_id) if preemption occurred.
    pub fn try_preempt(&self) -> Option<(ObjectId, ObjectId)> {
        let mut inner = self.inner.lock();
        
        // Peek at highest-priority waiting thread
        let waiting = inner.run_queue.peek()?;
        let waiting_priority = waiting.priority;
        
        // Find the lowest-priority running thread (highest priority number)
        let mut worst_slot = None;
        let mut worst_priority = 0u32;
        let mut worst_group_id = 0u32;
        
        for (idx, slot) in inner.slots.iter().enumerate() {
            if let SpuSlotState::Running(_thread_id, priority, group_id) = *slot {
                if priority > worst_priority || worst_slot.is_none() {
                    worst_priority = priority;
                    worst_group_id = group_id;
                    worst_slot = Some(idx);
                }
            }
        }
        
        let slot_idx = worst_slot?;
        
        // Only preempt if waiting thread has strictly higher priority (lower number)
        if waiting_priority < worst_priority {
            let displaced = match inner.slots[slot_idx] {
                SpuSlotState::Running(id, _, _) => id,
                _ => return None,
            };
            
            let new_entry = inner.run_queue.pop().unwrap();
            let new_id = new_entry.thread_id;
            
            // Displaced thread goes back to run queue preserving its priority and group
            inner.run_queue.push(SchedulerEntry {
                priority: worst_priority,
                group_id: worst_group_id,
                thread_id: displaced,
            });
            
            inner.slots[slot_idx] = SpuSlotState::Running(new_id, new_entry.priority, new_entry.group_id);
            inner.stats.preemption_count += 1;
            
            tracing::debug!(
                "SPU scheduler: preempted thread {} (priority {}) with thread {} (priority {}) on slot {}",
                displaced, worst_priority, new_id, new_entry.priority, slot_idx
            );
            
            Some((displaced, new_id))
        } else {
            None
        }
    }
    
    /// Yield: thread voluntarily gives up its slot and returns to the run queue.
    pub fn yield_thread(&self, thread_id: ObjectId, priority: u32) -> Result<(), KernelError> {
        let mut inner = self.inner.lock();
        
        for slot in inner.slots.iter_mut() {
            if let SpuSlotState::Running(id, _, group_id) = *slot {
                if id == thread_id {
                    let gid = group_id;
                    *slot = SpuSlotState::Idle;
                    inner.run_queue.push(SchedulerEntry {
                        priority,
                        group_id: gid,
                        thread_id,
                    });
                    inner.stats.yield_count += 1;
                    tracing::debug!("SPU scheduler: thread {} yielded", thread_id);
                    return Ok(());
                }
            }
        }
        
        Err(KernelError::InvalidId(thread_id))
    }
    
    /// Suspend: thread is paused but retains its slot (marked Suspended).
    pub fn suspend_thread(&self, thread_id: ObjectId) -> Result<(), KernelError> {
        let mut inner = self.inner.lock();
        
        for slot in inner.slots.iter_mut() {
            if let SpuSlotState::Running(id, priority, group_id) = *slot {
                if id == thread_id {
                    *slot = SpuSlotState::Suspended(thread_id, priority, group_id);
                    inner.stats.suspend_count += 1;
                    tracing::debug!("SPU scheduler: thread {} suspended", thread_id);
                    return Ok(());
                }
            }
        }
        
        Err(KernelError::InvalidId(thread_id))
    }
    
    /// Resume: wake a suspended thread back to Running state.
    pub fn resume_thread(&self, thread_id: ObjectId) -> Result<(), KernelError> {
        let mut inner = self.inner.lock();
        
        for slot in inner.slots.iter_mut() {
            if let SpuSlotState::Suspended(id, priority, group_id) = *slot {
                if id == thread_id {
                    *slot = SpuSlotState::Running(thread_id, priority, group_id);
                    inner.stats.resume_count += 1;
                    tracing::debug!("SPU scheduler: thread {} resumed", thread_id);
                    return Ok(());
                }
            }
        }
        
        Err(KernelError::InvalidId(thread_id))
    }
    
    /// Remove a thread from all scheduler state (slot + run queue).
    pub fn remove_thread(&self, thread_id: ObjectId) {
        let mut inner = self.inner.lock();
        
        // Clear from slots
        for slot in inner.slots.iter_mut() {
            match *slot {
                SpuSlotState::Running(id, _, _) | SpuSlotState::Suspended(id, _, _) if id == thread_id => {
                    *slot = SpuSlotState::Idle;
                }
                _ => {}
            }
        }
        
        // Rebuild run queue without this thread
        let remaining: Vec<_> = inner.run_queue.drain().filter(|e| e.thread_id != thread_id).collect();
        inner.run_queue = BinaryHeap::from(remaining);
    }
    
    /// Get the state of a specific run slot
    pub fn get_slot_state(&self, slot: usize) -> Option<SpuSlotState> {
        let inner = self.inner.lock();
        inner.slots.get(slot).copied()
    }
    
    /// Get current scheduling statistics
    pub fn stats(&self) -> SpuSchedulerStats {
        let inner = self.inner.lock();
        inner.stats.clone()
    }
    
    /// Get the number of idle slots
    pub fn idle_slot_count(&self) -> usize {
        let inner = self.inner.lock();
        inner.slots.iter().filter(|s| **s == SpuSlotState::Idle).count()
    }
    
    /// Get the number of threads in the run queue
    pub fn pending_count(&self) -> usize {
        let inner = self.inner.lock();
        inner.run_queue.len()
    }
}

impl Default for SpuScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spu_thread_group() {
        let manager = ObjectManager::new();
        let group_id = syscalls::sys_spu_thread_group_create(
            &manager,
            SpuThreadGroupAttributes::default(),
            2,
            100,
        )
        .unwrap();

        assert!(manager.exists(group_id));

        // Initialize threads
        let thread_id1 = syscalls::sys_spu_thread_initialize(
            &manager,
            group_id,
            0,
            SpuThreadAttributes::default(),
        )
        .unwrap();

        let thread_id2 = syscalls::sys_spu_thread_initialize(
            &manager,
            group_id,
            1,
            SpuThreadAttributes::default(),
        )
        .unwrap();

        // Open images
        syscalls::sys_spu_image_open(&manager, thread_id1, 0x1000).unwrap();
        syscalls::sys_spu_image_open(&manager, thread_id2, 0x2000).unwrap();

        // Start group
        syscalls::sys_spu_thread_group_start(&manager, group_id).unwrap();

        // Join group
        syscalls::sys_spu_thread_group_join(&manager, group_id).unwrap();

        // Destroy
        syscalls::sys_spu_thread_group_destroy(&manager, group_id).unwrap();
    }

    #[test]
    fn test_spu_ls_access() {
        let manager = ObjectManager::new();
        let group_id = syscalls::sys_spu_thread_group_create(
            &manager,
            SpuThreadGroupAttributes::default(),
            1,
            100,
        )
        .unwrap();

        let thread_id = syscalls::sys_spu_thread_initialize(
            &manager,
            group_id,
            0,
            SpuThreadAttributes::default(),
        )
        .unwrap();

        syscalls::sys_spu_image_open(&manager, thread_id, 0x1000).unwrap();

        // Write to LS
        let data = vec![1, 2, 3, 4];
        syscalls::sys_spu_thread_write_ls(&manager, thread_id, 0x100, &data).unwrap();

        // Read from LS
        let read_data = syscalls::sys_spu_thread_read_ls(&manager, thread_id, 0x100, 4).unwrap();
        assert_eq!(read_data.len(), 4);
        assert_eq!(read_data, data);

        syscalls::sys_spu_thread_group_destroy(&manager, group_id).unwrap();
    }

    #[test]
    fn test_spu_signals() {
        let manager = ObjectManager::new();
        let group_id = syscalls::sys_spu_thread_group_create(
            &manager,
            SpuThreadGroupAttributes::default(),
            1,
            100,
        )
        .unwrap();

        let thread_id = syscalls::sys_spu_thread_initialize(
            &manager,
            group_id,
            0,
            SpuThreadAttributes::default(),
        )
        .unwrap();

        syscalls::sys_spu_image_open(&manager, thread_id, 0x1000).unwrap();

        // Write and read signal1
        syscalls::sys_spu_thread_write_signal(&manager, thread_id, 1, 0x12345678).unwrap();
        let signal1 = syscalls::sys_spu_thread_read_signal(&manager, thread_id, 1).unwrap();
        assert_eq!(signal1, 0x12345678);

        // Write and read signal2
        syscalls::sys_spu_thread_write_signal(&manager, thread_id, 2, 0xABCDEF00).unwrap();
        let signal2 = syscalls::sys_spu_thread_read_signal(&manager, thread_id, 2).unwrap();
        assert_eq!(signal2, 0xABCDEF00);

        syscalls::sys_spu_thread_group_destroy(&manager, group_id).unwrap();
    }

    #[test]
    fn test_spu_scheduler_creation() {
        let scheduler = SpuScheduler::new();
        assert_eq!(scheduler.idle_slot_count(), NUM_SPU_SLOTS);
        assert_eq!(scheduler.pending_count(), 0);
    }

    #[test]
    fn test_spu_scheduler_submit_and_schedule() {
        let scheduler = SpuScheduler::new();
        
        // Submit 3 threads with different priorities
        scheduler.submit(1, 100, 10).unwrap(); // highest priority
        scheduler.submit(1, 101, 50).unwrap();
        scheduler.submit(1, 102, 90).unwrap(); // lowest priority
        
        assert_eq!(scheduler.pending_count(), 3);
        
        // Schedule: should assign all 3 to slots (we have 6 idle)
        let assigned = scheduler.schedule();
        assert_eq!(assigned, 3);
        assert_eq!(scheduler.idle_slot_count(), 3);
        assert_eq!(scheduler.pending_count(), 0);
    }

    #[test]
    fn test_spu_scheduler_yield() {
        let scheduler = SpuScheduler::new();
        scheduler.submit(1, 100, 10).unwrap();
        scheduler.schedule();
        
        // Thread should be running in a slot
        assert_eq!(scheduler.idle_slot_count(), 5);
        
        // Yield: thread goes back to run queue, slot becomes idle
        scheduler.yield_thread(100, 10).unwrap();
        assert_eq!(scheduler.idle_slot_count(), 6);
        assert_eq!(scheduler.pending_count(), 1);
        
        let stats = scheduler.stats();
        assert_eq!(stats.yield_count, 1);
    }

    #[test]
    fn test_spu_scheduler_suspend_resume() {
        let scheduler = SpuScheduler::new();
        scheduler.submit(1, 100, 10).unwrap();
        scheduler.schedule();
        
        // Suspend: slot changes to Suspended
        scheduler.suspend_thread(100).unwrap();
        let stats = scheduler.stats();
        assert_eq!(stats.suspend_count, 1);
        
        // Slot should show suspended
        let slot0 = scheduler.get_slot_state(0).unwrap();
        assert_eq!(slot0, SpuSlotState::Suspended(100, 10, 1));
        
        // Resume: slot goes back to Running
        scheduler.resume_thread(100).unwrap();
        let slot0 = scheduler.get_slot_state(0).unwrap();
        assert_eq!(slot0, SpuSlotState::Running(100, 10, 1));
        
        let stats = scheduler.stats();
        assert_eq!(stats.resume_count, 1);
    }

    #[test]
    fn test_spu_scheduler_remove() {
        let scheduler = SpuScheduler::new();
        scheduler.submit(1, 100, 10).unwrap();
        scheduler.submit(1, 101, 20).unwrap();
        scheduler.schedule();
        
        // Remove thread 100 from its slot
        scheduler.remove_thread(100);
        assert_eq!(scheduler.idle_slot_count(), 5);
        
        // Remove thread 101
        scheduler.remove_thread(101);
        assert_eq!(scheduler.idle_slot_count(), 6);
    }

    #[test]
    fn test_spu_scheduler_stats() {
        let scheduler = SpuScheduler::new();
        let stats = scheduler.stats();
        assert_eq!(stats.schedule_count, 0);
        assert_eq!(stats.preemption_count, 0);
        assert_eq!(stats.yield_count, 0);
        
        scheduler.submit(1, 100, 10).unwrap();
        scheduler.schedule();
        
        let stats = scheduler.stats();
        assert_eq!(stats.schedule_count, 1);
    }

    #[test]
    fn test_spu_scheduler_slot_overflow() {
        let scheduler = SpuScheduler::new();
        
        // Submit more threads than available slots
        for i in 0..10 {
            scheduler.submit(1, 200 + i, i as u32 * 10).unwrap();
        }
        
        // Schedule: should fill all 6 slots, leaving 4 pending
        let assigned = scheduler.schedule();
        assert_eq!(assigned, 6);
        assert_eq!(scheduler.idle_slot_count(), 0);
        assert_eq!(scheduler.pending_count(), 4);
    }

    #[test]
    fn test_spu_scheduler_priority_ordering() {
        let scheduler = SpuScheduler::new();
        
        // Submit in reverse priority order
        scheduler.submit(1, 300, 100).unwrap(); // lowest priority
        scheduler.submit(1, 301, 1).unwrap();   // highest priority
        scheduler.submit(1, 302, 50).unwrap();  // medium
        
        scheduler.schedule();
        
        // Slot 0 should have the highest-priority thread (301, priority 1)
        let slot0 = scheduler.get_slot_state(0).unwrap();
        assert_eq!(slot0, SpuSlotState::Running(301, 1, 1));
    }

    // Helper to create a thread for DMA tests
    fn create_test_thread(manager: &ObjectManager) -> (ObjectId, ObjectId) {
        let group_id = syscalls::sys_spu_thread_group_create(
            manager,
            SpuThreadGroupAttributes::default(),
            1,
            100,
        ).unwrap();

        let thread_id = syscalls::sys_spu_thread_initialize(
            manager,
            group_id,
            0,
            SpuThreadAttributes::default(),
        ).unwrap();

        syscalls::sys_spu_image_open(manager, thread_id, 0x1000).unwrap();
        (group_id, thread_id)
    }

    #[test]
    fn test_spu_dma_get() {
        let manager = ObjectManager::new();
        let (_group_id, thread_id) = create_test_thread(&manager);

        // Set up main memory with known data
        let mut main_memory = vec![0u8; 1024];
        for i in 0..256u16 {
            main_memory[i as usize] = i as u8;
        }

        // DMA GET: copy from main memory to local storage
        syscalls::sys_spu_thread_transfer_data_get(
            &manager, thread_id, 0x0000, 0, 256, &main_memory,
        ).unwrap();

        // Verify data was copied to LS
        let ls_data = syscalls::sys_spu_thread_read_ls(&manager, thread_id, 0, 256).unwrap();
        for i in 0..256usize {
            assert_eq!(ls_data[i], i as u8, "Mismatch at byte {}", i);
        }
    }

    #[test]
    fn test_spu_dma_put() {
        let manager = ObjectManager::new();
        let (_group_id, thread_id) = create_test_thread(&manager);

        // Write known data to local storage
        let ls_data: Vec<u8> = (0..128u8).collect();
        syscalls::sys_spu_thread_write_ls(&manager, thread_id, 0x100, &ls_data).unwrap();

        // Set up main memory
        let mut main_memory = vec![0u8; 1024];

        // DMA PUT: copy from local storage to main memory
        syscalls::sys_spu_thread_transfer_data_put(
            &manager, thread_id, 0x100, 256, 128, &mut main_memory,
        ).unwrap();

        // Verify data was copied to main memory
        for i in 0..128usize {
            assert_eq!(main_memory[256 + i], i as u8, "Mismatch at byte {}", i);
        }
    }

    #[test]
    fn test_spu_dma_getllar() {
        let manager = ObjectManager::new();
        let (_group_id, thread_id) = create_test_thread(&manager);

        // Set up main memory with known 128-byte cache line
        let mut main_memory = vec![0u8; 1024];
        for i in 0..128u8 {
            main_memory[128 + i as usize] = i.wrapping_add(0xAA);
        }

        // GETLLAR: atomically read 128-byte cache line
        syscalls::sys_spu_thread_atomic_get(
            &manager, thread_id, 0x0000, 128, &main_memory,
        ).unwrap();

        // Verify data in LS
        let ls_data = syscalls::sys_spu_thread_read_ls(&manager, thread_id, 0, 128).unwrap();
        for i in 0..128usize {
            assert_eq!(ls_data[i], (i as u8).wrapping_add(0xAA), "Mismatch at byte {}", i);
        }
    }

    #[test]
    fn test_spu_dma_putllc() {
        let manager = ObjectManager::new();
        let (_group_id, thread_id) = create_test_thread(&manager);

        // Write 128 bytes to LS
        let ls_data: Vec<u8> = (0..128u8).map(|i| i.wrapping_mul(3)).collect();
        syscalls::sys_spu_thread_write_ls(&manager, thread_id, 0x80, &ls_data).unwrap();

        // Set up main memory
        let mut main_memory = vec![0u8; 1024];

        // PUTLLC: conditionally store 128-byte cache line
        let success = syscalls::sys_spu_thread_atomic_put(
            &manager, thread_id, 0x80, 256, &mut main_memory,
        ).unwrap();
        assert!(success, "PUTLLC should succeed");

        // Verify data in main memory
        for i in 0..128usize {
            assert_eq!(main_memory[256 + i], (i as u8).wrapping_mul(3), "Mismatch at byte {}", i);
        }
    }

    #[test]
    fn test_spu_dma_alignment_check() {
        let manager = ObjectManager::new();
        let (_group_id, thread_id) = create_test_thread(&manager);
        let main_memory = vec![0u8; 1024];

        // GETLLAR with unaligned LS address should fail
        let result = syscalls::sys_spu_thread_atomic_get(
            &manager, thread_id, 0x0001, 0, &main_memory,
        );
        assert!(result.is_err(), "Unaligned LS address should be rejected");

        // GETLLAR with unaligned EA address should fail
        let result = syscalls::sys_spu_thread_atomic_get(
            &manager, thread_id, 0x0000, 1, &main_memory,
        );
        assert!(result.is_err(), "Unaligned EA address should be rejected");
    }

    #[test]
    fn test_spu_dma_bounds_check() {
        let manager = ObjectManager::new();
        let (_group_id, thread_id) = create_test_thread(&manager);
        let main_memory = vec![0u8; 64]; // Too small

        // DMA GET with out-of-bounds EA should fail
        let result = syscalls::sys_spu_thread_transfer_data_get(
            &manager, thread_id, 0x0000, 0, 128, &main_memory,
        );
        assert!(result.is_err(), "Out-of-bounds EA should be rejected");

        // Size 0 should be rejected
        let result = syscalls::sys_spu_thread_transfer_data_get(
            &manager, thread_id, 0x0000, 0, 0, &main_memory,
        );
        assert!(result.is_err(), "Zero size should be rejected");

        // Size > 16384 should be rejected
        let big_memory = vec![0u8; 32768];
        let result = syscalls::sys_spu_thread_transfer_data_get(
            &manager, thread_id, 0x0000, 0, 16385, &big_memory,
        );
        assert!(result.is_err(), "Size > 16384 should be rejected");
    }

    #[test]
    fn test_spu_mailbox_outbound() {
        let manager = ObjectManager::new();
        let group_id = syscalls::sys_spu_thread_group_create(
            &manager, SpuThreadGroupAttributes::default(), 1, 100,
        ).unwrap();
        let thread_id = syscalls::sys_spu_thread_initialize(
            &manager, group_id, 0, SpuThreadAttributes::default(),
        ).unwrap();

        let thread: Arc<SpuThread> = manager.get(thread_id).unwrap();

        // Outbound: SPU writes, PPU reads
        thread.write_outbound_mailbox(0xDEADBEEF).unwrap();
        thread.write_outbound_mailbox(0xCAFEBABE).unwrap();

        // PPU reads FIFO order
        assert_eq!(thread.read_outbound_mailbox().unwrap(), 0xDEADBEEF);
        assert_eq!(thread.read_outbound_mailbox().unwrap(), 0xCAFEBABE);

        // Empty mailbox returns WouldBlock
        assert!(thread.read_outbound_mailbox().is_err());
    }

    #[test]
    fn test_spu_mailbox_inbound() {
        let manager = ObjectManager::new();
        let group_id = syscalls::sys_spu_thread_group_create(
            &manager, SpuThreadGroupAttributes::default(), 1, 100,
        ).unwrap();
        let thread_id = syscalls::sys_spu_thread_initialize(
            &manager, group_id, 0, SpuThreadAttributes::default(),
        ).unwrap();

        // PPU writes to inbound, SPU reads
        syscalls::sys_spu_thread_write_mailbox(&manager, thread_id, 0x12345678).unwrap();
        syscalls::sys_spu_thread_write_mailbox(&manager, thread_id, 0xABCDEF00).unwrap();

        let thread: Arc<SpuThread> = manager.get(thread_id).unwrap();
        assert_eq!(thread.read_inbound_mailbox().unwrap(), 0x12345678);
        assert_eq!(thread.read_inbound_mailbox().unwrap(), 0xABCDEF00);
    }

    #[test]
    fn test_spu_mailbox_syscall_read() {
        let manager = ObjectManager::new();
        let group_id = syscalls::sys_spu_thread_group_create(
            &manager, SpuThreadGroupAttributes::default(), 1, 100,
        ).unwrap();
        let thread_id = syscalls::sys_spu_thread_initialize(
            &manager, group_id, 0, SpuThreadAttributes::default(),
        ).unwrap();

        let thread: Arc<SpuThread> = manager.get(thread_id).unwrap();
        thread.write_outbound_mailbox(0x42).unwrap();

        // PPU reads via syscall
        let value = syscalls::sys_spu_thread_read_mailbox(&manager, thread_id).unwrap();
        assert_eq!(value, 0x42);

        // Empty
        assert!(syscalls::sys_spu_thread_read_mailbox(&manager, thread_id).is_err());
    }

    #[test]
    fn test_spu_mailbox_capacity() {
        let manager = ObjectManager::new();
        let group_id = syscalls::sys_spu_thread_group_create(
            &manager, SpuThreadGroupAttributes::default(), 1, 100,
        ).unwrap();
        let thread_id = syscalls::sys_spu_thread_initialize(
            &manager, group_id, 0, SpuThreadAttributes::default(),
        ).unwrap();

        let thread: Arc<SpuThread> = manager.get(thread_id).unwrap();

        // Fill outbound mailbox to capacity (4)
        for i in 0..4 {
            thread.write_outbound_mailbox(i).unwrap();
        }
        assert_eq!(thread.outbound_mailbox_count(), 4);

        // 5th write should fail
        assert!(thread.write_outbound_mailbox(4).is_err());
    }

    #[test]
    fn test_spu_list_dma_get() {
        let manager = ObjectManager::new();
        let group_id = syscalls::sys_spu_thread_group_create(
            &manager, SpuThreadGroupAttributes::default(), 1, 100,
        ).unwrap();
        let thread_id = syscalls::sys_spu_thread_initialize(
            &manager, group_id, 0, SpuThreadAttributes::default(),
        ).unwrap();

        let mut main_memory = vec![0u8; 4096];
        main_memory[100..104].copy_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD]);
        main_memory[200..204].copy_from_slice(&[0x11, 0x22, 0x33, 0x44]);

        // Two GET entries
        let entries = vec![
            (0x0000u32, 100u64, 4u32),
            (0x0100u32, 200u64, 4u32),
        ];

        syscalls::sys_spu_thread_transfer_data_list(
            &manager, thread_id, &entries, true, &mut main_memory,
        ).unwrap();

        // Verify LS contents
        let data1 = syscalls::sys_spu_thread_read_ls(&manager, thread_id, 0x0000, 4).unwrap();
        assert_eq!(data1, vec![0xAA, 0xBB, 0xCC, 0xDD]);

        let data2 = syscalls::sys_spu_thread_read_ls(&manager, thread_id, 0x0100, 4).unwrap();
        assert_eq!(data2, vec![0x11, 0x22, 0x33, 0x44]);
    }

    #[test]
    fn test_spu_list_dma_put() {
        let manager = ObjectManager::new();
        let group_id = syscalls::sys_spu_thread_group_create(
            &manager, SpuThreadGroupAttributes::default(), 1, 100,
        ).unwrap();
        let thread_id = syscalls::sys_spu_thread_initialize(
            &manager, group_id, 0, SpuThreadAttributes::default(),
        ).unwrap();

        // Write to LS first
        syscalls::sys_spu_thread_write_ls(&manager, thread_id, 0, &[0xDE, 0xAD]).unwrap();
        syscalls::sys_spu_thread_write_ls(&manager, thread_id, 0x100, &[0xBE, 0xEF]).unwrap();

        let mut main_memory = vec![0u8; 4096];

        // Two PUT entries
        let entries = vec![
            (0x0000u32, 500u64, 2u32),
            (0x0100u32, 600u64, 2u32),
        ];

        syscalls::sys_spu_thread_transfer_data_list(
            &manager, thread_id, &entries, false, &mut main_memory,
        ).unwrap();

        assert_eq!(&main_memory[500..502], &[0xDE, 0xAD]);
        assert_eq!(&main_memory[600..602], &[0xBE, 0xEF]);
    }
}

