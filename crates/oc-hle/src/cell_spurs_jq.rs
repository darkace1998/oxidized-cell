//! cellSpursJq HLE - SPURS Job Queue
//!
//! This module provides HLE implementations for the PS3's SPURS Job Queue system.
//! Job queues allow efficient submission and scheduling of SPU workloads.

use std::collections::HashMap;
use tracing::{debug, trace};

/// Error codes
pub const CELL_SPURS_JQ_ERROR_NOT_INITIALIZED: i32 = 0x80410901u32 as i32;
pub const CELL_SPURS_JQ_ERROR_ALREADY_INITIALIZED: i32 = 0x80410902u32 as i32;
pub const CELL_SPURS_JQ_ERROR_INVALID_ARGUMENT: i32 = 0x80410903u32 as i32;
pub const CELL_SPURS_JQ_ERROR_BUSY: i32 = 0x80410904u32 as i32;
pub const CELL_SPURS_JQ_ERROR_NO_MEMORY: i32 = 0x80410905u32 as i32;
pub const CELL_SPURS_JQ_ERROR_QUEUE_FULL: i32 = 0x80410906u32 as i32;
pub const CELL_SPURS_JQ_ERROR_JOB_ABORT: i32 = 0x80410907u32 as i32;

/// Maximum number of jobs in a queue
pub const CELL_SPURS_JQ_MAX_JOBS: usize = 256;

/// Maximum number of job queues
pub const CELL_SPURS_JQ_MAX_QUEUES: usize = 16;

/// Job priority levels
pub const CELL_SPURS_JQ_PRIORITY_HIGH: u32 = 0;
pub const CELL_SPURS_JQ_PRIORITY_NORMAL: u32 = 1;
pub const CELL_SPURS_JQ_PRIORITY_LOW: u32 = 2;

/// Job state
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellSpursJobState {
    /// Job is pending
    Pending = 0,
    /// Job is running
    Running = 1,
    /// Job is complete
    Complete = 2,
    /// Job was aborted
    Aborted = 3,
}

impl Default for CellSpursJobState {
    fn default() -> Self {
        CellSpursJobState::Pending
    }
}

/// Job descriptor
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellSpursJob {
    /// Job ID
    pub id: u32,
    /// Job code address (SPU program)
    pub code_addr: u32,
    /// Job data address
    pub data_addr: u32,
    /// Job data size
    pub data_size: u32,
    /// Job priority
    pub priority: u32,
    /// Job tag
    pub tag: u32,
}

impl Default for CellSpursJob {
    fn default() -> Self {
        Self {
            id: 0,
            code_addr: 0,
            data_addr: 0,
            data_size: 0,
            priority: CELL_SPURS_JQ_PRIORITY_NORMAL,
            tag: 0,
        }
    }
}

/// Job queue attribute
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CellSpursJobQueueAttribute {
    /// Maximum number of jobs
    pub max_jobs: u32,
    /// Maximum number of SPUs
    pub max_spus: u32,
    /// Priority
    pub priority: u32,
}

impl Default for CellSpursJobQueueAttribute {
    fn default() -> Self {
        Self {
            max_jobs: CELL_SPURS_JQ_MAX_JOBS as u32,
            max_spus: 6,
            priority: CELL_SPURS_JQ_PRIORITY_NORMAL,
        }
    }
}

/// Internal job entry
#[derive(Debug, Clone)]
struct JobEntry {
    /// Job descriptor
    job: CellSpursJob,
    /// Job state
    state: CellSpursJobState,
    /// Completion callback address
    callback: u32,
    /// Callback user data
    userdata: u32,
}

/// Job queue entry
#[derive(Debug)]
struct JobQueueEntry {
    /// Queue ID
    id: u32,
    /// Queue attributes
    attr: CellSpursJobQueueAttribute,
    /// Jobs in queue
    jobs: HashMap<u32, JobEntry>,
    /// Next job ID
    next_job_id: u32,
    /// Is queue active
    active: bool,
}

impl JobQueueEntry {
    fn new(id: u32, attr: CellSpursJobQueueAttribute) -> Self {
        Self {
            id,
            attr,
            jobs: HashMap::new(),
            next_job_id: 1,
            active: true,
        }
    }
}

/// SPURS Job Queue manager
pub struct SpursJqManager {
    /// Initialization flag
    initialized: bool,
    /// SPURS instance address
    spurs_addr: u32,
    /// Job queues
    queues: HashMap<u32, JobQueueEntry>,
    /// Next queue ID
    next_queue_id: u32,
}

impl SpursJqManager {
    /// Create a new SPURS Job Queue manager
    pub fn new() -> Self {
        Self {
            initialized: false,
            spurs_addr: 0,
            queues: HashMap::new(),
            next_queue_id: 1,
        }
    }

    /// Initialize with SPURS instance
    pub fn init(&mut self, spurs_addr: u32) -> i32 {
        if self.initialized {
            return CELL_SPURS_JQ_ERROR_ALREADY_INITIALIZED;
        }

        debug!("SpursJqManager::init: spurs_addr=0x{:08X}", spurs_addr);

        self.spurs_addr = spurs_addr;
        self.initialized = true;

        0 // CELL_OK
    }

    /// Finalize
    pub fn finalize(&mut self) -> i32 {
        if !self.initialized {
            return CELL_SPURS_JQ_ERROR_NOT_INITIALIZED;
        }

        debug!("SpursJqManager::finalize");

        self.queues.clear();
        self.initialized = false;

        0 // CELL_OK
    }

    /// Create a job queue
    pub fn create_queue(&mut self, attr: CellSpursJobQueueAttribute) -> Result<u32, i32> {
        if !self.initialized {
            return Err(CELL_SPURS_JQ_ERROR_NOT_INITIALIZED);
        }

        if self.queues.len() >= CELL_SPURS_JQ_MAX_QUEUES {
            return Err(CELL_SPURS_JQ_ERROR_NO_MEMORY);
        }

        let queue_id = self.next_queue_id;
        self.next_queue_id += 1;

        debug!("SpursJqManager::create_queue: id={}, max_jobs={}", queue_id, attr.max_jobs);

        let queue = JobQueueEntry::new(queue_id, attr);
        self.queues.insert(queue_id, queue);

        Ok(queue_id)
    }

    /// Destroy a job queue
    pub fn destroy_queue(&mut self, queue_id: u32) -> i32 {
        if !self.initialized {
            return CELL_SPURS_JQ_ERROR_NOT_INITIALIZED;
        }

        if self.queues.remove(&queue_id).is_some() {
            debug!("SpursJqManager::destroy_queue: id={}", queue_id);
            0 // CELL_OK
        } else {
            CELL_SPURS_JQ_ERROR_INVALID_ARGUMENT
        }
    }

    /// Push a job to queue
    pub fn push_job(
        &mut self,
        queue_id: u32,
        job: CellSpursJob,
        callback: u32,
        userdata: u32,
    ) -> Result<u32, i32> {
        if !self.initialized {
            return Err(CELL_SPURS_JQ_ERROR_NOT_INITIALIZED);
        }

        let queue = self.queues.get_mut(&queue_id)
            .ok_or(CELL_SPURS_JQ_ERROR_INVALID_ARGUMENT)?;

        if queue.jobs.len() >= queue.attr.max_jobs as usize {
            return Err(CELL_SPURS_JQ_ERROR_QUEUE_FULL);
        }

        let job_id = queue.next_job_id;
        queue.next_job_id += 1;

        let mut job = job;
        job.id = job_id;

        trace!("SpursJqManager::push_job: queue={}, job_id={}", queue_id, job_id);

        let entry = JobEntry {
            job,
            state: CellSpursJobState::Pending,
            callback,
            userdata,
        };

        queue.jobs.insert(job_id, entry);

        Ok(job_id)
    }

    /// Get job status
    pub fn get_job_status(&self, queue_id: u32, job_id: u32) -> Result<CellSpursJobState, i32> {
        if !self.initialized {
            return Err(CELL_SPURS_JQ_ERROR_NOT_INITIALIZED);
        }

        let queue = self.queues.get(&queue_id)
            .ok_or(CELL_SPURS_JQ_ERROR_INVALID_ARGUMENT)?;

        let entry = queue.jobs.get(&job_id)
            .ok_or(CELL_SPURS_JQ_ERROR_INVALID_ARGUMENT)?;

        Ok(entry.state)
    }

    /// Abort a job
    pub fn abort_job(&mut self, queue_id: u32, job_id: u32) -> i32 {
        if !self.initialized {
            return CELL_SPURS_JQ_ERROR_NOT_INITIALIZED;
        }

        let queue = match self.queues.get_mut(&queue_id) {
            Some(q) => q,
            None => return CELL_SPURS_JQ_ERROR_INVALID_ARGUMENT,
        };

        let entry = match queue.jobs.get_mut(&job_id) {
            Some(e) => e,
            None => return CELL_SPURS_JQ_ERROR_INVALID_ARGUMENT,
        };

        // Can only abort pending jobs
        if entry.state != CellSpursJobState::Pending {
            return CELL_SPURS_JQ_ERROR_BUSY;
        }

        trace!("SpursJqManager::abort_job: queue={}, job_id={}", queue_id, job_id);

        entry.state = CellSpursJobState::Aborted;

        0 // CELL_OK
    }

    /// Sync on a job (wait for completion)
    pub fn sync_job(&self, queue_id: u32, job_id: u32) -> i32 {
        if !self.initialized {
            return CELL_SPURS_JQ_ERROR_NOT_INITIALIZED;
        }

        let queue = match self.queues.get(&queue_id) {
            Some(q) => q,
            None => return CELL_SPURS_JQ_ERROR_INVALID_ARGUMENT,
        };

        let entry = match queue.jobs.get(&job_id) {
            Some(e) => e,
            None => return CELL_SPURS_JQ_ERROR_INVALID_ARGUMENT,
        };

        trace!("SpursJqManager::sync_job: queue={}, job_id={}, state={:?}", 
            queue_id, job_id, entry.state);

        // TODO: Actually wait for job completion
        // For now, return immediately

        if entry.state == CellSpursJobState::Aborted {
            return CELL_SPURS_JQ_ERROR_JOB_ABORT;
        }

        0 // CELL_OK
    }

    /// Sync on all jobs in a queue
    pub fn sync_all(&self, queue_id: u32) -> i32 {
        if !self.initialized {
            return CELL_SPURS_JQ_ERROR_NOT_INITIALIZED;
        }

        if !self.queues.contains_key(&queue_id) {
            return CELL_SPURS_JQ_ERROR_INVALID_ARGUMENT;
        }

        trace!("SpursJqManager::sync_all: queue={}", queue_id);

        // TODO: Wait for all jobs to complete

        0 // CELL_OK
    }

    /// Get queue count
    pub fn queue_count(&self) -> usize {
        self.queues.len()
    }

    /// Get job count in a queue
    pub fn job_count(&self, queue_id: u32) -> Option<usize> {
        self.queues.get(&queue_id).map(|q| q.jobs.len())
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

impl Default for SpursJqManager {
    fn default() -> Self {
        Self::new()
    }
}

/// cellSpursJobQueueInitialize - Initialize job queue system
///
/// # Arguments
/// * `spurs_addr` - SPURS instance address
///
/// # Returns
/// * 0 on success
pub fn cell_spurs_job_queue_initialize(spurs_addr: u32) -> i32 {
    debug!("cellSpursJobQueueInitialize(spurs=0x{:08X})", spurs_addr);

    crate::context::get_hle_context_mut().spurs_jq.init(spurs_addr)
}

/// cellSpursJobQueueFinalize - Finalize job queue system
///
/// # Returns
/// * 0 on success
pub fn cell_spurs_job_queue_finalize() -> i32 {
    debug!("cellSpursJobQueueFinalize()");

    crate::context::get_hle_context_mut().spurs_jq.finalize()
}

/// cellSpursJobQueueCreate - Create a job queue
///
/// # Arguments
/// * `attr_addr` - Attribute address
/// * `queue_addr` - Address to write queue handle
///
/// # Returns
/// * 0 on success
pub fn cell_spurs_job_queue_create(_attr_addr: u32, _queue_addr: u32) -> i32 {
    debug!("cellSpursJobQueueCreate()");

    // Use default attributes when memory read is not yet implemented
    let attr = CellSpursJobQueueAttribute::default();
    match crate::context::get_hle_context_mut().spurs_jq.create_queue(attr) {
        Ok(_queue_id) => {
            // TODO: Write queue ID to memory at _queue_addr
            0 // CELL_OK
        }
        Err(e) => e,
    }
}

/// cellSpursJobQueueDestroy - Destroy a job queue
///
/// # Arguments
/// * `queue` - Queue handle
///
/// # Returns
/// * 0 on success
pub fn cell_spurs_job_queue_destroy(queue: u32) -> i32 {
    debug!("cellSpursJobQueueDestroy(queue={})", queue);

    crate::context::get_hle_context_mut().spurs_jq.destroy_queue(queue)
}

/// cellSpursJobQueuePushJob - Push a job to queue
///
/// # Arguments
/// * `queue` - Queue handle
/// * `job_addr` - Job descriptor address
/// * `callback` - Completion callback
/// * `userdata` - User data for callback
///
/// # Returns
/// * 0 on success
pub fn cell_spurs_job_queue_push_job(
    queue: u32,
    _job_addr: u32,
    callback: u32,
    userdata: u32,
) -> i32 {
    trace!("cellSpursJobQueuePushJob(queue={})", queue);

    // Use default job when memory read is not yet implemented
    let job = CellSpursJob::default();
    match crate::context::get_hle_context_mut().spurs_jq.push_job(queue, job, callback, userdata) {
        Ok(_job_id) => 0, // CELL_OK
        Err(e) => e,
    }
}

/// cellSpursJobQueueSync - Wait for job completion
///
/// # Arguments
/// * `queue` - Queue handle
/// * `job_id` - Job ID
///
/// # Returns
/// * 0 on success
pub fn cell_spurs_job_queue_sync(queue: u32, job_id: u32) -> i32 {
    trace!("cellSpursJobQueueSync(queue={}, job_id={})", queue, job_id);

    crate::context::get_hle_context().spurs_jq.sync_job(queue, job_id)
}

/// cellSpursJobQueueSyncAll - Wait for all jobs in queue
///
/// # Arguments
/// * `queue` - Queue handle
///
/// # Returns
/// * 0 on success
pub fn cell_spurs_job_queue_sync_all(queue: u32) -> i32 {
    trace!("cellSpursJobQueueSyncAll(queue={})", queue);

    crate::context::get_hle_context().spurs_jq.sync_all(queue)
}

/// cellSpursJobQueueAbort - Abort a pending job
///
/// # Arguments
/// * `queue` - Queue handle
/// * `job_id` - Job ID
///
/// # Returns
/// * 0 on success
pub fn cell_spurs_job_queue_abort(queue: u32, job_id: u32) -> i32 {
    trace!("cellSpursJobQueueAbort(queue={}, job_id={})", queue, job_id);

    crate::context::get_hle_context_mut().spurs_jq.abort_job(queue, job_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spurs_jq_manager_lifecycle() {
        let mut manager = SpursJqManager::new();
        
        assert_eq!(manager.init(0x10000000), 0);
        assert!(manager.is_initialized());
        
        // Double init should fail
        assert_eq!(manager.init(0x10000000), CELL_SPURS_JQ_ERROR_ALREADY_INITIALIZED);
        
        assert_eq!(manager.finalize(), 0);
        assert!(!manager.is_initialized());
        
        // Double finalize should fail
        assert_eq!(manager.finalize(), CELL_SPURS_JQ_ERROR_NOT_INITIALIZED);
    }

    #[test]
    fn test_spurs_jq_manager_queues() {
        let mut manager = SpursJqManager::new();
        manager.init(0x10000000);
        
        // Create queues
        let attr = CellSpursJobQueueAttribute::default();
        let queue1 = manager.create_queue(attr).unwrap();
        let queue2 = manager.create_queue(attr).unwrap();
        
        assert_eq!(manager.queue_count(), 2);
        assert_ne!(queue1, queue2);
        
        // Destroy queues
        assert_eq!(manager.destroy_queue(queue1), 0);
        assert_eq!(manager.queue_count(), 1);
        
        // Double destroy should fail
        assert_eq!(manager.destroy_queue(queue1), CELL_SPURS_JQ_ERROR_INVALID_ARGUMENT);
        
        manager.finalize();
    }

    #[test]
    fn test_spurs_jq_manager_jobs() {
        let mut manager = SpursJqManager::new();
        manager.init(0x10000000);
        
        let attr = CellSpursJobQueueAttribute::default();
        let queue = manager.create_queue(attr).unwrap();
        
        // Push jobs
        let job = CellSpursJob::default();
        let job_id = manager.push_job(queue, job, 0, 0).unwrap();
        
        assert_eq!(manager.job_count(queue), Some(1));
        
        // Check job status
        let status = manager.get_job_status(queue, job_id).unwrap();
        assert_eq!(status, CellSpursJobState::Pending);
        
        // Sync on job (should succeed)
        assert_eq!(manager.sync_job(queue, job_id), 0);
        
        manager.finalize();
    }

    #[test]
    fn test_spurs_jq_manager_abort() {
        let mut manager = SpursJqManager::new();
        manager.init(0x10000000);
        
        let attr = CellSpursJobQueueAttribute::default();
        let queue = manager.create_queue(attr).unwrap();
        
        let job = CellSpursJob::default();
        let job_id = manager.push_job(queue, job, 0, 0).unwrap();
        
        // Abort pending job
        assert_eq!(manager.abort_job(queue, job_id), 0);
        
        // Check status is aborted
        let status = manager.get_job_status(queue, job_id).unwrap();
        assert_eq!(status, CellSpursJobState::Aborted);
        
        // Sync on aborted job should return error
        assert_eq!(manager.sync_job(queue, job_id), CELL_SPURS_JQ_ERROR_JOB_ABORT);
        
        manager.finalize();
    }

    #[test]
    fn test_spurs_jq_manager_not_initialized() {
        let mut manager = SpursJqManager::new();
        
        let attr = CellSpursJobQueueAttribute::default();
        assert!(manager.create_queue(attr).is_err());
    }

    #[test]
    fn test_spurs_jq_constants() {
        assert_eq!(CELL_SPURS_JQ_PRIORITY_HIGH, 0);
        assert_eq!(CELL_SPURS_JQ_PRIORITY_NORMAL, 1);
        assert_eq!(CELL_SPURS_JQ_PRIORITY_LOW, 2);
    }

    #[test]
    fn test_spurs_jq_job_default() {
        let job = CellSpursJob::default();
        assert_eq!(job.priority, CELL_SPURS_JQ_PRIORITY_NORMAL);
    }

    #[test]
    fn test_spurs_jq_attr_default() {
        let attr = CellSpursJobQueueAttribute::default();
        assert_eq!(attr.max_jobs, CELL_SPURS_JQ_MAX_JOBS as u32);
    }
}
