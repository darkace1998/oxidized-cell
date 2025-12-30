//! cellSpurs HLE - SPURS Task Scheduler
//!
//! This module provides HLE implementations for the PS3's SPURS (SPU Runtime System).
//! SPURS is a task scheduler for managing SPU workloads.

use std::collections::HashMap;
use tracing::{debug, trace, info};
use oc_core::{SpuBridgeSender, SpuWorkload, SpuGroupRequest};

/// Maximum number of SPUs
pub const CELL_SPURS_MAX_SPU: usize = 8;

/// Maximum number of workloads
pub const CELL_SPURS_MAX_WORKLOAD: usize = 16;

/// SPURS attribute flags
pub const CELL_SPURS_ATTRIBUTE_FLAG_NONE: u32 = 0;
pub const CELL_SPURS_ATTRIBUTE_FLAG_SIGNAL_TO_PPU: u32 = 1;

/// SPURS priorities
pub const CELL_SPURS_MAX_PRIORITY: u32 = 16;

/// SPURS workload state
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WorkloadState {
    /// Workload is idle
    #[default]
    Idle = 0,
    /// Workload is running
    Running = 1,
    /// Workload is ready to run
    Ready = 2,
    /// Workload is waiting
    Waiting = 3,
}

/// SPURS task queue entry
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct Task {
    /// Task ID
    id: u32,
    /// Task entry point address
    entry: u32,
    /// Task argument
    argument: u64,
    /// Task priority
    priority: u8,
    /// Task state
    state: WorkloadState,
}

/// Task queue
#[allow(dead_code)]
#[derive(Debug)]
struct TaskQueue {
    /// Queue ID
    id: u32,
    /// Pending tasks (ordered by priority)
    tasks: Vec<Task>,
    /// Next task ID
    next_task_id: u32,
}

impl TaskQueue {
    fn new(id: u32) -> Self {
        Self {
            id,
            tasks: Vec::new(),
            next_task_id: 1,
        }
    }

    fn push_task(&mut self, entry: u32, argument: u64, priority: u8) -> u32 {
        let task_id = self.next_task_id;
        self.next_task_id += 1;

        let task = Task {
            id: task_id,
            entry,
            argument,
            priority,
            state: WorkloadState::Ready,
        };

        // Insert task in priority order
        let pos = self.tasks.iter().position(|t| t.priority > priority)
            .unwrap_or(self.tasks.len());
        self.tasks.insert(pos, task);

        task_id
    }

    fn pop_task(&mut self) -> Option<Task> {
        if !self.tasks.is_empty() {
            Some(self.tasks.remove(0))
        } else {
            None
        }
    }

    #[allow(dead_code)]
    fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    #[allow(dead_code)]
    fn task_count(&self) -> usize {
        self.tasks.len()
    }
}

/// Job chain entry
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct JobChain {
    /// Chain ID
    id: u32,
    /// Jobs in chain
    jobs: Vec<u32>,
    /// Current job index
    current_index: usize,
    /// Chain complete flag
    complete: bool,
}

impl JobChain {
    fn new(id: u32, jobs: Vec<u32>) -> Self {
        Self {
            id,
            jobs,
            current_index: 0,
            complete: false,
        }
    }

    fn get_current_job(&self) -> Option<u32> {
        if self.current_index < self.jobs.len() {
            Some(self.jobs[self.current_index])
        } else {
            None
        }
    }

    fn advance(&mut self) -> bool {
        self.current_index += 1;
        if self.current_index >= self.jobs.len() {
            self.complete = true;
            false
        } else {
            true
        }
    }

    fn is_complete(&self) -> bool {
        self.complete
    }
}

/// Taskset for managing multiple tasks
#[allow(dead_code)]
#[derive(Debug)]
struct Taskset {
    /// Taskset ID
    id: u32,
    /// Tasks in set
    tasks: Vec<u32>,
    /// Completed tasks
    completed: Vec<u32>,
    /// Taskset enabled
    enabled: bool,
}

impl Taskset {
    fn new(id: u32) -> Self {
        Self {
            id,
            tasks: Vec::new(),
            completed: Vec::new(),
            enabled: true,
        }
    }

    fn add_task(&mut self, task_id: u32) {
        if !self.tasks.contains(&task_id) {
            self.tasks.push(task_id);
        }
    }

    fn remove_task(&mut self, task_id: u32) {
        self.tasks.retain(|&t| t != task_id);
        self.completed.retain(|&t| t != task_id);
    }

    fn mark_complete(&mut self, task_id: u32) {
        if !self.completed.contains(&task_id) {
            self.completed.push(task_id);
        }
    }

    fn is_complete(&self) -> bool {
        self.enabled && self.completed.len() == self.tasks.len()
    }

    #[allow(dead_code)]
    fn task_count(&self) -> usize {
        self.tasks.len()
    }
}

/// Event flag for synchronization
#[allow(dead_code)]
#[derive(Debug)]
struct EventFlag {
    /// Flag ID
    id: u32,
    /// Flag pattern
    pattern: u64,
    /// Mode (AND/OR)
    mode: u32,
}

impl EventFlag {
    fn new(id: u32, pattern: u64, mode: u32) -> Self {
        Self { id, pattern, mode }
    }

    fn set(&mut self, bits: u64) {
        self.pattern |= bits;
    }

    fn clear(&mut self, bits: u64) {
        self.pattern &= !bits;
    }

    fn test(&self, bits: u64, mode: u32) -> bool {
        if mode == 0 {
            // AND mode: all bits must be set
            (self.pattern & bits) == bits
        } else {
            // OR mode: any bit must be set
            (self.pattern & bits) != 0
        }
    }
}

/// Barrier for synchronization
#[allow(dead_code)]
#[derive(Debug)]
struct Barrier {
    /// Barrier ID
    id: u32,
    /// Total count required
    total_count: u32,
    /// Current count
    current_count: u32,
}

impl Barrier {
    fn new(id: u32, total_count: u32) -> Self {
        Self {
            id,
            total_count,
            current_count: 0,
        }
    }

    fn wait(&mut self) -> bool {
        self.current_count += 1;
        if self.current_count >= self.total_count {
            self.current_count = 0;
            true
        } else {
            false
        }
    }

    fn reset(&mut self) {
        self.current_count = 0;
    }
}

/// SPURS workload
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct Workload {
    /// Workload ID
    id: u32,
    /// Workload state
    state: WorkloadState,
    /// Priority levels for 8 SPUs
    priorities: [u8; CELL_SPURS_MAX_SPU],
}

/// SPURS manager
pub struct SpursManager {
    /// Initialization flag
    initialized: bool,
    /// Number of SPUs allocated
    num_spus: u32,
    /// SPU priority
    spu_priority: u32,
    /// PPU priority
    ppu_priority: u32,
    /// Exit if no work flag
    exit_if_no_work: bool,
    /// Workloads
    workloads: HashMap<u32, Workload>,
    /// Attached event queues
    event_queues: HashMap<u32, u32>, // port -> queue_id
    /// SPU thread IDs
    spu_thread_ids: Vec<u32>,
    /// Task queues
    task_queues: HashMap<u32, TaskQueue>,
    /// Next task queue ID
    next_queue_id: u32,
    /// Job chains
    job_chains: HashMap<u32, JobChain>,
    /// Next job chain ID
    next_chain_id: u32,
    /// Tasksets
    tasksets: HashMap<u32, Taskset>,
    /// Next taskset ID
    next_taskset_id: u32,
    /// Event flags
    event_flags: HashMap<u32, EventFlag>,
    /// Next event flag ID
    next_event_flag_id: u32,
    /// Barriers
    barriers: HashMap<u32, Barrier>,
    /// Next barrier ID
    next_barrier_id: u32,
    /// SPU bridge sender for forwarding workloads to SPU interpreter
    spu_bridge: Option<SpuBridgeSender>,
    /// Next workload ID for bridge
    next_workload_id: u32,
    /// SPU thread group ID
    spu_group_id: Option<u32>,
}

impl SpursManager {
    /// Create a new SPURS manager
    pub fn new() -> Self {
        Self {
            initialized: false,
            num_spus: 0,
            spu_priority: 0,
            ppu_priority: 0,
            exit_if_no_work: false,
            workloads: HashMap::new(),
            event_queues: HashMap::new(),
            spu_thread_ids: Vec::new(),
            task_queues: HashMap::new(),
            next_queue_id: 1,
            job_chains: HashMap::new(),
            next_chain_id: 1,
            tasksets: HashMap::new(),
            next_taskset_id: 1,
            event_flags: HashMap::new(),
            next_event_flag_id: 1,
            barriers: HashMap::new(),
            next_barrier_id: 1,
            spu_bridge: None,
            next_workload_id: 1,
            spu_group_id: None,
        }
    }

    /// Set the SPU bridge sender for forwarding workloads to SPU interpreter
    pub fn set_spu_bridge(&mut self, bridge: SpuBridgeSender) {
        self.spu_bridge = Some(bridge);
    }

    /// Check if SPU bridge is connected
    pub fn has_spu_bridge(&self) -> bool {
        self.spu_bridge.is_some()
    }

    /// Initialize SPURS instance
    pub fn initialize(
        &mut self,
        num_spus: u32,
        spu_priority: u32,
        ppu_priority: u32,
        exit_if_no_work: bool,
    ) -> i32 {
        if self.initialized {
            return 0x80410801u32 as i32; // CELL_SPURS_ERROR_ALREADY_INITIALIZED
        }

        if num_spus == 0 || num_spus > CELL_SPURS_MAX_SPU as u32 {
            return 0x80410802u32 as i32; // CELL_SPURS_ERROR_INVALID_ARGUMENT
        }

        debug!(
            "SpursManager::initialize: num_spus={}, spu_priority={}, ppu_priority={}, exit_if_no_work={}",
            num_spus, spu_priority, ppu_priority, exit_if_no_work
        );

        self.num_spus = num_spus;
        self.spu_priority = spu_priority;
        self.ppu_priority = ppu_priority;
        self.exit_if_no_work = exit_if_no_work;
        self.initialized = true;

        // Create SPU thread IDs (simulated)
        for i in 0..num_spus {
            self.spu_thread_ids.push(0x1000 + i);
        }

        // Create actual SPU thread group through the bridge
        if let Some(ref bridge) = self.spu_bridge {
            let group_id = 1u32; // SPURS uses a single main group
            let group_request = SpuGroupRequest {
                group_id,
                num_threads: num_spus,
                priority: spu_priority as u8,
                name: "SPURS".to_string(),
            };
            
            if !bridge.create_group(group_request) {
                debug!("Failed to create SPU thread group");
                return 0x80410805u32 as i32; // CELL_SPURS_ERROR_INTERNAL
            }
            
            // Start the group
            if !bridge.start_group(group_id) {
                debug!("Failed to start SPU thread group");
                return 0x80410805u32 as i32; // CELL_SPURS_ERROR_INTERNAL
            }
            
            self.spu_group_id = Some(group_id);
            info!("SPURS: Created and started SPU thread group {} with {} SPUs", group_id, num_spus);
        } else {
            debug!("SPURS: No SPU bridge connected, SPU thread group creation skipped");
        }

        0 // CELL_OK
    }

    /// Finalize SPURS instance
    pub fn finalize(&mut self) -> i32 {
        if !self.initialized {
            return 0x80410803u32 as i32; // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        debug!("SpursManager::finalize");

        // Destroy SPU thread group through the bridge
        if let Some(ref bridge) = self.spu_bridge {
            if let Some(group_id) = self.spu_group_id {
                // Stop the group first
                if !bridge.stop_group(group_id) {
                    debug!("Failed to stop SPU thread group");
                }
                
                // Destroy the group
                if !bridge.destroy_group(group_id) {
                    debug!("Failed to destroy SPU thread group");
                }
                
                info!("SPURS: Destroyed SPU thread group {}", group_id);
            }
        }

        self.initialized = false;
        self.workloads.clear();
        self.event_queues.clear();
        self.spu_thread_ids.clear();
        self.task_queues.clear();
        self.job_chains.clear();
        self.tasksets.clear();
        self.event_flags.clear();
        self.barriers.clear();
        self.spu_group_id = None;

        0 // CELL_OK
    }

    /// Attach LV2 event queue
    pub fn attach_lv2_event_queue(
        &mut self,
        queue_id: u32,
        port: u32,
        is_dynamic: bool,
    ) -> i32 {
        if !self.initialized {
            return 0x80410803u32 as i32; // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        debug!(
            "SpursManager::attach_lv2_event_queue: queue_id={}, port={}, is_dynamic={}",
            queue_id, port, is_dynamic
        );

        if self.event_queues.contains_key(&port) {
            return 0x80410804u32 as i32; // CELL_SPURS_ERROR_BUSY
        }

        self.event_queues.insert(port, queue_id);

        // TODO: Actually attach event queue

        0 // CELL_OK
    }

    /// Detach LV2 event queue
    pub fn detach_lv2_event_queue(&mut self, port: u32) -> i32 {
        if !self.initialized {
            return 0x80410803u32 as i32; // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        debug!("SpursManager::detach_lv2_event_queue: port={}", port);

        if self.event_queues.remove(&port).is_none() {
            return 0x80410802u32 as i32; // CELL_SPURS_ERROR_INVALID_ARGUMENT
        }

        // TODO: Actually detach event queue

        0 // CELL_OK
    }

    /// Set workload priorities
    pub fn set_priorities(&mut self, wid: u32, priorities: &[u8]) -> i32 {
        if !self.initialized {
            return 0x80410803u32 as i32; // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        if wid >= CELL_SPURS_MAX_WORKLOAD as u32 {
            return 0x80410802u32 as i32; // CELL_SPURS_ERROR_INVALID_ARGUMENT
        }

        if priorities.len() != CELL_SPURS_MAX_SPU {
            return 0x80410802u32 as i32; // CELL_SPURS_ERROR_INVALID_ARGUMENT
        }

        trace!("SpursManager::set_priorities: wid={}", wid);

        // Create or update workload
        let workload = self.workloads.entry(wid).or_insert_with(|| Workload {
            id: wid,
            state: WorkloadState::Idle,
            priorities: [0; CELL_SPURS_MAX_SPU],
        });

        workload.priorities.copy_from_slice(priorities);

        0 // CELL_OK
    }

    /// Get SPU thread ID
    pub fn get_spu_thread_id(&self, thread: u32) -> Result<u32, i32> {
        if !self.initialized {
            return Err(0x80410803u32 as i32); // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        if thread >= self.num_spus {
            return Err(0x80410802u32 as i32); // CELL_SPURS_ERROR_INVALID_ARGUMENT
        }

        Ok(self.spu_thread_ids[thread as usize])
    }

    /// Get number of SPUs
    pub fn get_num_spus(&self) -> u32 {
        self.num_spus
    }

    /// Get number of workloads
    pub fn get_workload_count(&self) -> usize {
        self.workloads.len()
    }

    /// Get number of attached event queues
    pub fn get_event_queue_count(&self) -> usize {
        self.event_queues.len()
    }

    // ========================================================================
    // Task Queue Management
    // ========================================================================

    /// Create a task queue
    pub fn create_task_queue(&mut self) -> Result<u32, i32> {
        if !self.initialized {
            return Err(0x80410803u32 as i32); // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        let queue_id = self.next_queue_id;
        self.next_queue_id += 1;

        debug!("SpursManager::create_task_queue: id={}", queue_id);

        self.task_queues.insert(queue_id, TaskQueue::new(queue_id));

        Ok(queue_id)
    }

    /// Destroy a task queue
    pub fn destroy_task_queue(&mut self, queue_id: u32) -> i32 {
        if !self.initialized {
            return 0x80410803u32 as i32; // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        if self.task_queues.remove(&queue_id).is_some() {
            debug!("SpursManager::destroy_task_queue: id={}", queue_id);
            0 // CELL_OK
        } else {
            0x80410802u32 as i32 // CELL_SPURS_ERROR_INVALID_ARGUMENT
        }
    }

    /// Push a task to queue
    pub fn push_task(&mut self, queue_id: u32, entry: u32, argument: u64, priority: u8) -> Result<u32, i32> {
        if !self.initialized {
            return Err(0x80410803u32 as i32); // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        let queue = self.task_queues.get_mut(&queue_id)
            .ok_or(0x80410802u32 as i32)?; // CELL_SPURS_ERROR_INVALID_ARGUMENT

        let task_id = queue.push_task(entry, argument, priority);

        trace!("SpursManager::push_task: queue={}, task={}", queue_id, task_id);

        // Submit task to SPU through the bridge for execution
        if let Some(ref bridge) = self.spu_bridge {
            let workload_id = self.next_workload_id;
            self.next_workload_id += 1;
            
            // Task workloads can run on any available SPU
            let affinity = (1u8 << self.num_spus) - 1; // All SPUs in the group
            
            let spu_workload = SpuWorkload {
                id: workload_id,
                entry_point: entry,
                program_size: 0, // Will be loaded from SPU image
                argument,
                priority,
                affinity,
            };
            
            if !bridge.submit_workload(spu_workload) {
                debug!("Failed to submit task {} to SPU", task_id);
                // Task is still in queue, just not submitted to SPU yet
            } else {
                trace!("SPURS: Submitted task {} (workload_id={}) to SPU queue", task_id, workload_id);
            }
        }

        Ok(task_id)
    }

    /// Pop a task from queue
    pub fn pop_task(&mut self, queue_id: u32) -> Result<Option<u32>, i32> {
        if !self.initialized {
            return Err(0x80410803u32 as i32); // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        let queue = self.task_queues.get_mut(&queue_id)
            .ok_or(0x80410802u32 as i32)?; // CELL_SPURS_ERROR_INVALID_ARGUMENT

        Ok(queue.pop_task().map(|t| t.id))
    }

    /// Get task queue count
    pub fn get_task_queue_count(&self) -> usize {
        self.task_queues.len()
    }

    // ========================================================================
    // Workload Scheduling
    // ========================================================================

    /// Schedule workload on SPU
    pub fn schedule_workload(&mut self, wid: u32, spu_id: u32) -> i32 {
        if !self.initialized {
            return 0x80410803u32 as i32; // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        if wid >= CELL_SPURS_MAX_WORKLOAD as u32 || spu_id >= self.num_spus {
            return 0x80410802u32 as i32; // CELL_SPURS_ERROR_INVALID_ARGUMENT
        }

        debug!("SpursManager::schedule_workload: wid={}, spu={}", wid, spu_id);

        // Update workload state
        if let Some(workload) = self.workloads.get_mut(&wid) {
            workload.state = WorkloadState::Running;
        }

        // Submit workload to SPU through the bridge
        if let Some(ref bridge) = self.spu_bridge {
            let bridge_workload_id = self.next_workload_id;
            self.next_workload_id += 1;
            
            // Create SPU affinity bitmask (target specific SPU)
            let affinity = 1u8 << spu_id;
            
            let spu_workload = SpuWorkload {
                id: bridge_workload_id,
                entry_point: 0, // Workload entry - would come from workload info
                program_size: 0, // Program size - would be determined from SPU binary
                argument: wid as u64, // Pass workload ID as argument
                priority: self.spu_priority as u8,
                affinity,
            };
            
            if !bridge.submit_workload(spu_workload) {
                debug!("Failed to submit workload {} to SPU", wid);
                return 0x80410805u32 as i32; // CELL_SPURS_ERROR_INTERNAL
            }
            
            trace!("SPURS: Submitted workload {} to SPU {} (bridge_id={})", wid, spu_id, bridge_workload_id);
        }

        0 // CELL_OK
    }

    /// Unschedule workload
    pub fn unschedule_workload(&mut self, wid: u32) -> i32 {
        if !self.initialized {
            return 0x80410803u32 as i32; // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        if wid >= CELL_SPURS_MAX_WORKLOAD as u32 {
            return 0x80410802u32 as i32; // CELL_SPURS_ERROR_INVALID_ARGUMENT
        }

        debug!("SpursManager::unschedule_workload: wid={}", wid);

        // Update workload state
        if let Some(workload) = self.workloads.get_mut(&wid) {
            workload.state = WorkloadState::Idle;
        }

        0 // CELL_OK
    }

    // ========================================================================
    // Job Chains
    // ========================================================================

    /// Create a job chain
    pub fn create_job_chain(&mut self, jobs: Vec<u32>) -> Result<u32, i32> {
        if !self.initialized {
            return Err(0x80410803u32 as i32); // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        let chain_id = self.next_chain_id;
        self.next_chain_id += 1;

        debug!("SpursManager::create_job_chain: id={}, jobs={}", chain_id, jobs.len());

        self.job_chains.insert(chain_id, JobChain::new(chain_id, jobs));

        Ok(chain_id)
    }

    /// Get current job in chain
    pub fn get_chain_current_job(&self, chain_id: u32) -> Result<Option<u32>, i32> {
        if !self.initialized {
            return Err(0x80410803u32 as i32); // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        let chain = self.job_chains.get(&chain_id)
            .ok_or(0x80410802u32 as i32)?; // CELL_SPURS_ERROR_INVALID_ARGUMENT

        Ok(chain.get_current_job())
    }

    /// Advance job chain to next job
    pub fn advance_job_chain(&mut self, chain_id: u32) -> Result<bool, i32> {
        if !self.initialized {
            return Err(0x80410803u32 as i32); // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        let chain = self.job_chains.get_mut(&chain_id)
            .ok_or(0x80410802u32 as i32)?; // CELL_SPURS_ERROR_INVALID_ARGUMENT

        Ok(chain.advance())
    }

    /// Check if job chain is complete
    pub fn is_chain_complete(&self, chain_id: u32) -> Result<bool, i32> {
        if !self.initialized {
            return Err(0x80410803u32 as i32); // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        let chain = self.job_chains.get(&chain_id)
            .ok_or(0x80410802u32 as i32)?; // CELL_SPURS_ERROR_INVALID_ARGUMENT

        Ok(chain.is_complete())
    }

    /// Get job chain count
    pub fn get_job_chain_count(&self) -> usize {
        self.job_chains.len()
    }

    // ========================================================================
    // Taskset Operations
    // ========================================================================

    /// Create a taskset
    pub fn create_taskset(&mut self) -> Result<u32, i32> {
        if !self.initialized {
            return Err(0x80410803u32 as i32); // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        let taskset_id = self.next_taskset_id;
        self.next_taskset_id += 1;

        debug!("SpursManager::create_taskset: id={}", taskset_id);

        self.tasksets.insert(taskset_id, Taskset::new(taskset_id));

        Ok(taskset_id)
    }

    /// Add task to taskset
    pub fn taskset_add_task(&mut self, taskset_id: u32, task_id: u32) -> i32 {
        if !self.initialized {
            return 0x80410803u32 as i32; // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        let taskset = self.tasksets.get_mut(&taskset_id)
            .ok_or(0x80410802u32 as i32)
            .unwrap();

        taskset.add_task(task_id);

        trace!("SpursManager::taskset_add_task: taskset={}, task={}", taskset_id, task_id);

        0 // CELL_OK
    }

    /// Remove task from taskset
    pub fn taskset_remove_task(&mut self, taskset_id: u32, task_id: u32) -> i32 {
        if !self.initialized {
            return 0x80410803u32 as i32; // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        let taskset = self.tasksets.get_mut(&taskset_id)
            .ok_or(0x80410802u32 as i32)
            .unwrap();

        taskset.remove_task(task_id);

        0 // CELL_OK
    }

    /// Mark task complete in taskset
    pub fn taskset_mark_complete(&mut self, taskset_id: u32, task_id: u32) -> i32 {
        if !self.initialized {
            return 0x80410803u32 as i32; // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        let taskset = self.tasksets.get_mut(&taskset_id)
            .ok_or(0x80410802u32 as i32)
            .unwrap();

        taskset.mark_complete(task_id);

        0 // CELL_OK
    }

    /// Check if taskset is complete
    pub fn is_taskset_complete(&self, taskset_id: u32) -> Result<bool, i32> {
        if !self.initialized {
            return Err(0x80410803u32 as i32); // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        let taskset = self.tasksets.get(&taskset_id)
            .ok_or(0x80410802u32 as i32)?; // CELL_SPURS_ERROR_INVALID_ARGUMENT

        Ok(taskset.is_complete())
    }

    /// Get taskset count
    pub fn get_taskset_count(&self) -> usize {
        self.tasksets.len()
    }

    // ========================================================================
    // Event Flags and Barriers
    // ========================================================================

    /// Create an event flag
    pub fn create_event_flag(&mut self, pattern: u64, mode: u32) -> Result<u32, i32> {
        if !self.initialized {
            return Err(0x80410803u32 as i32); // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        let flag_id = self.next_event_flag_id;
        self.next_event_flag_id += 1;

        debug!("SpursManager::create_event_flag: id={}, pattern=0x{:X}", flag_id, pattern);

        self.event_flags.insert(flag_id, EventFlag::new(flag_id, pattern, mode));

        Ok(flag_id)
    }

    /// Set event flag bits
    pub fn event_flag_set(&mut self, flag_id: u32, bits: u64) -> i32 {
        if !self.initialized {
            return 0x80410803u32 as i32; // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        let flag = self.event_flags.get_mut(&flag_id)
            .ok_or(0x80410802u32 as i32)
            .unwrap();

        flag.set(bits);

        trace!("SpursManager::event_flag_set: flag={}, bits=0x{:X}", flag_id, bits);

        0 // CELL_OK
    }

    /// Clear event flag bits
    pub fn event_flag_clear(&mut self, flag_id: u32, bits: u64) -> i32 {
        if !self.initialized {
            return 0x80410803u32 as i32; // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        let flag = self.event_flags.get_mut(&flag_id)
            .ok_or(0x80410802u32 as i32)
            .unwrap();

        flag.clear(bits);

        0 // CELL_OK
    }

    /// Test event flag
    pub fn event_flag_test(&self, flag_id: u32, bits: u64, mode: u32) -> Result<bool, i32> {
        if !self.initialized {
            return Err(0x80410803u32 as i32); // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        let flag = self.event_flags.get(&flag_id)
            .ok_or(0x80410802u32 as i32)?; // CELL_SPURS_ERROR_INVALID_ARGUMENT

        Ok(flag.test(bits, mode))
    }

    /// Get event flag count
    pub fn get_event_flag_count(&self) -> usize {
        self.event_flags.len()
    }

    /// Create a barrier
    pub fn create_barrier(&mut self, total_count: u32) -> Result<u32, i32> {
        if !self.initialized {
            return Err(0x80410803u32 as i32); // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        let barrier_id = self.next_barrier_id;
        self.next_barrier_id += 1;

        debug!("SpursManager::create_barrier: id={}, count={}", barrier_id, total_count);

        self.barriers.insert(barrier_id, Barrier::new(barrier_id, total_count));

        Ok(barrier_id)
    }

    /// Wait on barrier
    pub fn barrier_wait(&mut self, barrier_id: u32) -> Result<bool, i32> {
        if !self.initialized {
            return Err(0x80410803u32 as i32); // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        let barrier = self.barriers.get_mut(&barrier_id)
            .ok_or(0x80410802u32 as i32)?; // CELL_SPURS_ERROR_INVALID_ARGUMENT

        Ok(barrier.wait())
    }

    /// Reset barrier
    pub fn barrier_reset(&mut self, barrier_id: u32) -> i32 {
        if !self.initialized {
            return 0x80410803u32 as i32; // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        let barrier = self.barriers.get_mut(&barrier_id)
            .ok_or(0x80410802u32 as i32)
            .unwrap();

        barrier.reset();

        0 // CELL_OK
    }

    /// Get barrier count
    pub fn get_barrier_count(&self) -> usize {
        self.barriers.len()
    }

    // ========================================================================
    // SPU Workload Scheduling (Stubs)
    // ========================================================================

    /// Get next ready workload
    /// 
    /// Returns the workload ID with the highest priority that is ready to run.
    /// This is used by the SPURS kernel to schedule workloads on SPUs.
    pub fn get_next_workload(&self, spu_id: u32) -> Option<u32> {
        // Validate spu_id is within bounds of both num_spus and priorities array
        if !self.initialized || spu_id >= self.num_spus || spu_id as usize >= CELL_SPURS_MAX_SPU {
            return None;
        }

        // Find workload with highest priority for this SPU
        let mut best_wid: Option<u32> = None;
        let mut best_priority: u8 = u8::MAX;

        for (wid, workload) in &self.workloads {
            if workload.state == WorkloadState::Ready {
                let priority = workload.priorities[spu_id as usize];
                if priority > 0 && priority < best_priority {
                    best_priority = priority;
                    best_wid = Some(*wid);
                }
            }
        }

        best_wid
    }

    /// Mark workload as ready
    pub fn set_workload_ready(&mut self, wid: u32) -> i32 {
        if !self.initialized {
            return 0x80410803u32 as i32; // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        if let Some(workload) = self.workloads.get_mut(&wid) {
            workload.state = WorkloadState::Ready;
            trace!("SpursManager::set_workload_ready: wid={}", wid);
            0 // CELL_OK
        } else {
            0x80410802u32 as i32 // CELL_SPURS_ERROR_INVALID_ARGUMENT
        }
    }

    /// Mark workload as waiting
    pub fn set_workload_waiting(&mut self, wid: u32) -> i32 {
        if !self.initialized {
            return 0x80410803u32 as i32; // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        if let Some(workload) = self.workloads.get_mut(&wid) {
            workload.state = WorkloadState::Waiting;
            trace!("SpursManager::set_workload_waiting: wid={}", wid);
            0 // CELL_OK
        } else {
            0x80410802u32 as i32 // CELL_SPURS_ERROR_INVALID_ARGUMENT
        }
    }

    /// Get workload state
    pub fn get_workload_state(&self, wid: u32) -> Option<WorkloadState> {
        self.workloads.get(&wid).map(|w| w.state)
    }

    /// Check if any SPU is idle
    pub fn has_idle_spu(&self) -> bool {
        // For HLE, we assume SPUs are always available
        self.initialized && self.num_spus > 0
    }

    // ========================================================================
    // SPURS Kernel Integration (Stubs)
    // ========================================================================

    /// Process pending workloads
    /// 
    /// This is called by the SPURS kernel to process all pending workloads.
    /// In a real implementation, this would schedule workloads on actual SPUs.
    pub fn process_workloads(&mut self) -> u32 {
        if !self.initialized {
            return 0;
        }

        let mut processed = 0u32;

        // Get list of ready workloads
        let ready_wids: Vec<u32> = self.workloads.iter()
            .filter(|(_, w)| w.state == WorkloadState::Ready)
            .map(|(wid, _)| *wid)
            .collect();

        // "Execute" each workload (simulated)
        for wid in ready_wids {
            if let Some(workload) = self.workloads.get_mut(&wid) {
                // In a real implementation:
                // 1. Load workload program to SPU
                // 2. Execute on SPU
                // 3. Handle completion
                
                // For HLE, just mark as running then idle
                workload.state = WorkloadState::Running;
                trace!("SpursManager: Processing workload {}", wid);
                workload.state = WorkloadState::Idle;
                processed += 1;
            }
        }

        processed
    }

    /// Yield current workload
    /// 
    /// Allows the current workload to yield execution to other workloads.
    pub fn yield_workload(&mut self, wid: u32) -> i32 {
        if !self.initialized {
            return 0x80410803u32 as i32; // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        if let Some(workload) = self.workloads.get_mut(&wid) {
            if workload.state == WorkloadState::Running {
                workload.state = WorkloadState::Ready;
                trace!("SpursManager::yield_workload: wid={}", wid);
            }
            0 // CELL_OK
        } else {
            0x80410802u32 as i32 // CELL_SPURS_ERROR_INVALID_ARGUMENT
        }
    }

    // ========================================================================
    // SPURS Handler Implementation (Stubs)
    // ========================================================================

    /// Register workload handler
    pub fn register_handler(&mut self, wid: u32, handler_addr: u32) -> i32 {
        if !self.initialized {
            return 0x80410803u32 as i32; // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        debug!("SpursManager::register_handler: wid={}, handler=0x{:08X}", wid, handler_addr);

        // For HLE, we just acknowledge the registration
        // Real implementation would store and call the handler
        
        if wid >= CELL_SPURS_MAX_WORKLOAD as u32 {
            return 0x80410802u32 as i32; // CELL_SPURS_ERROR_INVALID_ARGUMENT
        }

        0 // CELL_OK
    }

    /// Unregister workload handler
    pub fn unregister_handler(&mut self, wid: u32) -> i32 {
        if !self.initialized {
            return 0x80410803u32 as i32; // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        debug!("SpursManager::unregister_handler: wid={}", wid);

        if wid >= CELL_SPURS_MAX_WORKLOAD as u32 {
            return 0x80410802u32 as i32; // CELL_SPURS_ERROR_INVALID_ARGUMENT
        }

        0 // CELL_OK
    }

    // ========================================================================
    // Trace Buffer Support
    // ========================================================================

    /// Enable trace buffer
    /// 
    /// Allocates and enables a trace buffer for debugging SPURS execution.
    pub fn enable_trace(&mut self, buffer_addr: u32, buffer_size: u32) -> i32 {
        if !self.initialized {
            return 0x80410803u32 as i32; // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        debug!(
            "SpursManager::enable_trace: buffer=0x{:08X}, size={}",
            buffer_addr, buffer_size
        );

        // For HLE, we acknowledge but don't actually trace
        // Real implementation would set up DMA to trace buffer
        
        0 // CELL_OK
    }

    /// Disable trace buffer
    pub fn disable_trace(&mut self) -> i32 {
        if !self.initialized {
            return 0x80410803u32 as i32; // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        debug!("SpursManager::disable_trace");

        0 // CELL_OK
    }

    /// Get trace data
    /// 
    /// Returns the current trace buffer contents (for debugging).
    pub fn get_trace_data(&self) -> Vec<u8> {
        if !self.initialized {
            return Vec::new();
        }

        // For HLE, return empty trace data
        // Real implementation would return actual trace buffer contents
        Vec::new()
    }

    /// Clear trace buffer
    pub fn clear_trace(&mut self) -> i32 {
        if !self.initialized {
            return 0x80410803u32 as i32; // CELL_SPURS_ERROR_NOT_INITIALIZED
        }

        trace!("SpursManager::clear_trace");

        0 // CELL_OK
    }
}

impl Default for SpursManager {
    fn default() -> Self {
        Self::new()
    }
}

/// SPURS instance structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellSpurs {
    /// Reserved internal data
    _internal: [u8; 4096],
}

impl Default for CellSpurs {
    fn default() -> Self {
        Self {
            _internal: [0; 4096],
        }
    }
}

/// SPURS attribute
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellSpursAttribute {
    /// Revision
    pub revision: u32,
    /// SPU thread group priority
    pub spu_thread_group_priority: u32,
    /// PPU thread priority
    pub ppu_thread_priority: u32,
    /// Exit if no work flag
    pub exit_if_no_work: bool,
    /// Attribute flags
    pub flags: u32,
    /// Name prefix
    pub name_prefix: [u8; 16],
    /// Container
    pub container: u32,
}

impl Default for CellSpursAttribute {
    fn default() -> Self {
        Self {
            revision: 1,
            spu_thread_group_priority: 0,
            ppu_thread_priority: 0,
            exit_if_no_work: false,
            flags: CELL_SPURS_ATTRIBUTE_FLAG_NONE,
            name_prefix: [0; 16],
            container: 0,
        }
    }
}

/// SPURS task attribute
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellSpursTaskAttribute {
    /// Revision
    pub revision: u32,
    /// Entry address
    pub entry: u32,
    /// Argument
    pub argument: u64,
    /// ELF address
    pub elf_addr: u32,
}

/// cellSpursInitialize - Initialize SPURS instance
///
/// # Arguments
/// * `spurs` - SPURS instance address
/// * `nSpus` - Number of SPUs to use
/// * `spuPriority` - SPU priority
/// * `ppuPriority` - PPU priority
/// * `exitIfNoWork` - Exit if no work flag
///
/// # Returns
/// * 0 on success
pub fn cell_spurs_initialize(
    _spurs_addr: u32,
    n_spus: u32,
    spu_priority: u32,
    ppu_priority: u32,
    exit_if_no_work: bool,
) -> i32 {
    debug!(
        "cellSpursInitialize(nSpus={}, spuPriority={}, ppuPriority={}, exitIfNoWork={})",
        n_spus, spu_priority, ppu_priority, exit_if_no_work
    );

    // Validate parameters
    if n_spus == 0 || n_spus > CELL_SPURS_MAX_SPU as u32 {
        return 0x80410802u32 as i32; // CELL_SPURS_ERROR_INVALID_ARGUMENT
    }

    crate::context::get_hle_context_mut().spurs.initialize(n_spus, spu_priority, ppu_priority, exit_if_no_work)
}

/// cellSpursFinalize - Finalize SPURS instance
///
/// # Arguments
/// * `spurs` - SPURS instance address
///
/// # Returns
/// * 0 on success
pub fn cell_spurs_finalize(_spurs_addr: u32) -> i32 {
    debug!("cellSpursFinalize()");

    crate::context::get_hle_context_mut().spurs.finalize()
}

/// cellSpursAttachLv2EventQueue - Attach LV2 event queue to SPURS
///
/// # Arguments
/// * `spurs` - SPURS instance address
/// * `queue` - Event queue ID
/// * `port` - Port number
/// * `isDynamic` - Dynamic flag
///
/// # Returns
/// * 0 on success
pub fn cell_spurs_attach_lv2_event_queue(
    _spurs_addr: u32,
    queue: u32,
    port: u32,
    is_dynamic: bool,
) -> i32 {
    debug!(
        "cellSpursAttachLv2EventQueue(queue={}, port={}, isDynamic={})",
        queue, port, is_dynamic
    );

    crate::context::get_hle_context_mut().spurs.attach_lv2_event_queue(queue, port, is_dynamic)
}

/// cellSpursDetachLv2EventQueue - Detach LV2 event queue from SPURS
///
/// # Arguments
/// * `spurs` - SPURS instance address
/// * `port` - Port number
///
/// # Returns
/// * 0 on success
pub fn cell_spurs_detach_lv2_event_queue(_spurs_addr: u32, port: u32) -> i32 {
    debug!("cellSpursDetachLv2EventQueue(port={})", port);

    crate::context::get_hle_context_mut().spurs.detach_lv2_event_queue(port)
}

/// Default priority level for SPU workloads (1 = normal priority)
const DEFAULT_SPU_PRIORITY: u8 = 1;

/// cellSpursSetPriorities - Set workload priorities
///
/// Reads the priority array from memory and sets priorities for each SPU
/// for the specified workload.
///
/// # Arguments
/// * `spurs` - SPURS instance address
/// * `wid` - Workload ID
/// * `priorities` - Priority array address (8 bytes, one per SPU)
///
/// # Returns
/// * 0 on success
pub fn cell_spurs_set_priorities(_spurs_addr: u32, wid: u32, priorities_addr: u32) -> i32 {
    trace!("cellSpursSetPriorities(wid={}, priorities_addr=0x{:08X})", wid, priorities_addr);

    // Validate workload ID
    if wid >= CELL_SPURS_MAX_WORKLOAD as u32 {
        return 0x80410802u32 as i32; // CELL_SPURS_ERROR_INVALID_ARGUMENT
    }

    // Read priorities from memory (8 bytes, one per SPU)
    let priorities: [u8; CELL_SPURS_MAX_SPU] = if priorities_addr != 0 && crate::memory::is_hle_memory_initialized() {
        if let Ok(bytes) = crate::memory::read_bytes(priorities_addr, CELL_SPURS_MAX_SPU as u32) {
            let mut arr = [DEFAULT_SPU_PRIORITY; CELL_SPURS_MAX_SPU];
            for (i, &b) in bytes.iter().enumerate() {
                if i < CELL_SPURS_MAX_SPU {
                    arr[i] = b;
                }
            }
            arr
        } else {
            [DEFAULT_SPU_PRIORITY; CELL_SPURS_MAX_SPU]
        }
    } else {
        // Use default priorities when memory not available
        [DEFAULT_SPU_PRIORITY; CELL_SPURS_MAX_SPU]
    };
    
    crate::context::get_hle_context_mut().spurs.set_priorities(wid, &priorities)
}

/// cellSpursGetSpuThreadId - Get SPU thread ID
///
/// Retrieves the SPU thread ID for the specified thread index and writes it
/// to the provided memory address.
///
/// # Arguments
/// * `spurs` - SPURS instance address
/// * `thread` - Thread number (0-7)
/// * `threadId_addr` - Address to write thread ID to
///
/// # Returns
/// * 0 on success
pub fn cell_spurs_get_spu_thread_id(
    _spurs_addr: u32,
    thread: u32,
    thread_id_addr: u32,
) -> i32 {
    trace!("cellSpursGetSpuThreadId(thread={}, thread_id_addr=0x{:08X})", thread, thread_id_addr);

    // Validate thread number
    if thread >= CELL_SPURS_MAX_SPU as u32 {
        return 0x80410802u32 as i32; // CELL_SPURS_ERROR_INVALID_ARGUMENT
    }
    
    // Validate output address
    if thread_id_addr == 0 {
        return 0x80410802u32 as i32; // CELL_SPURS_ERROR_INVALID_ARGUMENT
    }

    match crate::context::get_hle_context().spurs.get_spu_thread_id(thread) {
        Ok(thread_id) => {
            // Write thread ID to memory
            if let Err(_) = crate::memory::write_be32(thread_id_addr, thread_id) {
                return 0x80410801u32 as i32; // CELL_SPURS_ERROR_STAT
            }
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spurs_manager() {
        let mut manager = SpursManager::new();
        assert_eq!(manager.initialize(6, 100, 100, false), 0);
        assert_eq!(manager.get_num_spus(), 6);
        assert_eq!(manager.finalize(), 0);
    }

    #[test]
    fn test_spurs_manager_lifecycle() {
        let mut manager = SpursManager::new();
        
        // Initialize
        assert_eq!(manager.initialize(4, 100, 100, false), 0);
        
        // Try to initialize again (should fail)
        assert!(manager.initialize(4, 100, 100, false) != 0);
        
        // Finalize
        assert_eq!(manager.finalize(), 0);
        
        // Try to finalize again (should fail)
        assert!(manager.finalize() != 0);
    }

    #[test]
    fn test_spurs_manager_event_queues() {
        let mut manager = SpursManager::new();
        manager.initialize(6, 100, 100, false);
        
        // Attach event queues
        assert_eq!(manager.attach_lv2_event_queue(1, 0, false), 0);
        assert_eq!(manager.attach_lv2_event_queue(2, 1, true), 0);
        assert_eq!(manager.get_event_queue_count(), 2);
        
        // Try to attach to same port (should fail)
        assert!(manager.attach_lv2_event_queue(3, 0, false) != 0);
        
        // Detach event queue
        assert_eq!(manager.detach_lv2_event_queue(0), 0);
        assert_eq!(manager.get_event_queue_count(), 1);
        
        // Try to detach again (should fail)
        assert!(manager.detach_lv2_event_queue(0) != 0);
        
        manager.finalize();
    }

    #[test]
    fn test_spurs_manager_workloads() {
        let mut manager = SpursManager::new();
        manager.initialize(8, 100, 100, false);
        
        // Set priorities for workload
        let priorities = [1, 2, 3, 4, 5, 6, 7, 8];
        assert_eq!(manager.set_priorities(0, &priorities), 0);
        assert_eq!(manager.get_workload_count(), 1);
        
        // Add more workloads
        assert_eq!(manager.set_priorities(1, &priorities), 0);
        assert_eq!(manager.set_priorities(2, &priorities), 0);
        assert_eq!(manager.get_workload_count(), 3);
        
        manager.finalize();
    }

    #[test]
    fn test_spurs_manager_spu_threads() {
        let mut manager = SpursManager::new();
        manager.initialize(6, 100, 100, false);
        
        // Get SPU thread IDs
        for i in 0..6 {
            let thread_id = manager.get_spu_thread_id(i);
            assert!(thread_id.is_ok());
            assert_eq!(thread_id.unwrap(), 0x1000 + i);
        }
        
        // Invalid thread number
        assert!(manager.get_spu_thread_id(10).is_err());
        
        manager.finalize();
    }

    #[test]
    fn test_spurs_manager_validation() {
        let mut manager = SpursManager::new();
        
        // Invalid num_spus (0)
        assert!(manager.initialize(0, 100, 100, false) != 0);
        
        // Invalid num_spus (too many)
        assert!(manager.initialize(10, 100, 100, false) != 0);
        
        // Valid initialization
        assert_eq!(manager.initialize(6, 100, 100, false), 0);
        manager.finalize();
    }

    #[test]
    fn test_spurs_attribute_default() {
        let attr = CellSpursAttribute::default();
        assert_eq!(attr.revision, 1);
        assert_eq!(attr.flags, CELL_SPURS_ATTRIBUTE_FLAG_NONE);
    }

    #[test]
    fn test_spurs_initialize() {
        let result = cell_spurs_initialize(0x10000000, 6, 100, 100, false);
        assert_eq!(result, 0);
        
        // Invalid num_spus
        let result = cell_spurs_initialize(0x10000000, 0, 100, 100, false);
        assert!(result != 0);
    }

    #[test]
    fn test_spurs_constants() {
        assert_eq!(CELL_SPURS_MAX_PRIORITY, 16);
        assert_eq!(CELL_SPURS_ATTRIBUTE_FLAG_NONE, 0);
        assert_eq!(CELL_SPURS_MAX_SPU, 8);
        assert_eq!(CELL_SPURS_MAX_WORKLOAD, 16);
    }

    // ========================================================================
    // SPU Workload Scheduling Tests
    // ========================================================================

    #[test]
    fn test_spurs_workload_states() {
        let mut manager = SpursManager::new();
        manager.initialize(6, 100, 100, false);
        
        // Create workload
        let priorities = [1, 2, 3, 4, 5, 6, 7, 8];
        manager.set_priorities(0, &priorities);
        
        // Initial state is idle
        assert_eq!(manager.get_workload_state(0), Some(WorkloadState::Idle));
        
        // Set to ready
        assert_eq!(manager.set_workload_ready(0), 0);
        assert_eq!(manager.get_workload_state(0), Some(WorkloadState::Ready));
        
        // Set to waiting
        assert_eq!(manager.set_workload_waiting(0), 0);
        assert_eq!(manager.get_workload_state(0), Some(WorkloadState::Waiting));
        
        manager.finalize();
    }

    #[test]
    fn test_spurs_get_next_workload() {
        let mut manager = SpursManager::new();
        manager.initialize(6, 100, 100, false);
        
        // No workloads ready
        assert!(manager.get_next_workload(0).is_none());
        
        // Create and set workload ready
        let priorities = [1, 0, 0, 0, 0, 0, 0, 0]; // Only SPU 0 has priority
        manager.set_priorities(0, &priorities);
        manager.set_workload_ready(0);
        
        // Should get workload for SPU 0
        assert_eq!(manager.get_next_workload(0), Some(0));
        
        // SPU 1 has no priority for this workload
        assert!(manager.get_next_workload(1).is_none());
        
        manager.finalize();
    }

    #[test]
    fn test_spurs_process_workloads() {
        let mut manager = SpursManager::new();
        manager.initialize(6, 100, 100, false);
        
        // No workloads to process
        assert_eq!(manager.process_workloads(), 0);
        
        // Add ready workloads
        let priorities = [1; 8];
        manager.set_priorities(0, &priorities);
        manager.set_priorities(1, &priorities);
        manager.set_workload_ready(0);
        manager.set_workload_ready(1);
        
        // Process workloads
        assert_eq!(manager.process_workloads(), 2);
        
        // Workloads should be idle now
        assert_eq!(manager.get_workload_state(0), Some(WorkloadState::Idle));
        assert_eq!(manager.get_workload_state(1), Some(WorkloadState::Idle));
        
        manager.finalize();
    }

    #[test]
    fn test_spurs_yield_workload() {
        let mut manager = SpursManager::new();
        manager.initialize(6, 100, 100, false);
        
        let priorities = [1; 8];
        manager.set_priorities(0, &priorities);
        
        // Can't yield non-running workload
        manager.set_workload_ready(0);
        assert_eq!(manager.yield_workload(0), 0); // Should still succeed but not change
        
        manager.finalize();
    }

    #[test]
    fn test_spurs_has_idle_spu() {
        let mut manager = SpursManager::new();
        
        // Not initialized
        assert!(!manager.has_idle_spu());
        
        manager.initialize(6, 100, 100, false);
        assert!(manager.has_idle_spu());
        
        manager.finalize();
    }

    // ========================================================================
    // SPURS Handler Tests
    // ========================================================================

    #[test]
    fn test_spurs_register_handler() {
        let mut manager = SpursManager::new();
        manager.initialize(6, 100, 100, false);
        
        assert_eq!(manager.register_handler(0, 0x10000000), 0);
        assert_eq!(manager.unregister_handler(0), 0);
        
        // Invalid workload ID
        assert!(manager.register_handler(100, 0x10000000) != 0);
        
        manager.finalize();
    }

    // ========================================================================
    // Trace Buffer Tests
    // ========================================================================

    #[test]
    fn test_spurs_trace_buffer() {
        let mut manager = SpursManager::new();
        manager.initialize(6, 100, 100, false);
        
        // Enable trace
        assert_eq!(manager.enable_trace(0x20000000, 0x10000), 0);
        
        // Get trace data (empty for HLE)
        let trace_data = manager.get_trace_data();
        assert!(trace_data.is_empty());
        
        // Clear trace
        assert_eq!(manager.clear_trace(), 0);
        
        // Disable trace
        assert_eq!(manager.disable_trace(), 0);
        
        manager.finalize();
    }

    #[test]
    fn test_spurs_trace_not_initialized() {
        let mut manager = SpursManager::new();
        
        assert!(manager.enable_trace(0x20000000, 0x10000) != 0);
        assert!(manager.disable_trace() != 0);
        assert!(manager.clear_trace() != 0);
    }

    #[test]
    fn test_workload_state_enum() {
        assert_eq!(WorkloadState::Idle as u32, 0);
        assert_eq!(WorkloadState::Running as u32, 1);
        assert_eq!(WorkloadState::Ready as u32, 2);
        assert_eq!(WorkloadState::Waiting as u32, 3);
    }
}
