//! SPU Bridge - Communication channel between SPURS HLE and SPU backend
//!
//! This module provides a decoupled communication mechanism between the
//! cellSpurs HLE module and the SPU interpreter/threads, avoiding circular
//! dependencies between oc-hle and oc-spu.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use parking_lot::Mutex;
use std::collections::VecDeque;

/// Maximum number of workloads in the queue
pub const SPU_BRIDGE_QUEUE_CAPACITY: usize = 1024;

/// Maximum number of SPUs
pub const SPU_MAX_COUNT: usize = 6;

/// SPU workload request sent from SPURS to SPU manager
#[derive(Debug, Clone)]
pub struct SpuWorkload {
    /// Workload ID
    pub id: u32,
    /// SPU program entry point (address in main memory where SPU ELF is located)
    pub entry_point: u32,
    /// SPU program size in bytes
    pub program_size: u32,
    /// Argument passed to SPU program
    pub argument: u64,
    /// Priority (0 = highest)
    pub priority: u8,
    /// Which SPU(s) can run this workload (bitmask)
    pub affinity: u8,
}

/// SPU thread creation request
#[derive(Debug, Clone)]
pub struct SpuThreadRequest {
    /// Thread ID to assign
    pub thread_id: u32,
    /// SPU ID to run on (0-5)
    pub spu_id: u8,
    /// Entry point address
    pub entry_point: u32,
    /// Argument
    pub argument: u64,
    /// Priority
    pub priority: u8,
}

/// SPU thread group creation request
#[derive(Debug, Clone)]
pub struct SpuGroupRequest {
    /// Group ID
    pub group_id: u32,
    /// Number of threads in group
    pub num_threads: u32,
    /// Group priority
    pub priority: u8,
    /// Group name
    pub name: String,
}

/// SPU DMA request (for SPURS-initiated transfers)
#[derive(Debug, Clone)]
pub struct SpuDmaRequest {
    /// SPU ID
    pub spu_id: u8,
    /// Local storage address
    pub ls_addr: u32,
    /// Main memory address
    pub ea_addr: u64,
    /// Transfer size
    pub size: u32,
    /// Tag
    pub tag: u8,
    /// Direction: true = put (LS -> main), false = get (main -> LS)
    pub is_put: bool,
}

/// SPU event sent back from SPU to SPURS
#[derive(Debug, Clone)]
pub struct SpuEvent {
    /// Event type
    pub event_type: SpuEventType,
    /// SPU ID that generated the event
    pub spu_id: u8,
    /// Associated data (stop signal, error code, etc.)
    pub data: u64,
}

/// SPU event types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpuEventType {
    /// Workload completed successfully
    WorkloadComplete,
    /// Workload failed with error
    WorkloadError,
    /// SPU thread stopped
    ThreadStopped,
    /// SPU thread finished (exit)
    ThreadFinished,
    /// DMA transfer complete
    DmaComplete,
    /// Mailbox message available
    MailboxReady,
    /// Signal notification
    Signal,
}

/// Bridge message types from SPURS to SPU
#[derive(Debug, Clone)]
pub enum SpuBridgeMessage {
    /// Submit a workload
    SubmitWorkload(SpuWorkload),
    /// Create a thread group
    CreateGroup(SpuGroupRequest),
    /// Destroy a thread group
    DestroyGroup(u32),
    /// Start a thread group
    StartGroup(u32),
    /// Stop a thread group
    StopGroup(u32),
    /// Create an SPU thread
    CreateThread(SpuThreadRequest),
    /// Terminate an SPU thread
    TerminateThread(u32),
    /// Initiate DMA transfer
    DmaTransfer(SpuDmaRequest),
    /// Send signal to SPU
    SendSignal { spu_id: u8, signal: u32 },
    /// Write to SPU mailbox
    WriteMailbox { spu_id: u8, value: u32 },
    /// Attach an event queue to SPURS
    AttachEventQueue { queue_id: u32, port: u32 },
    /// Detach an event queue from SPURS
    DetachEventQueue { queue_id: u32, port: u32 },
}

/// The sender side of the SPU bridge (used by SpursManager in oc-hle)
pub struct SpuBridgeSender {
    /// Message queue
    queue: Arc<Mutex<VecDeque<SpuBridgeMessage>>>,
    /// Event queue (events from SPU back to SPURS)
    events: Arc<Mutex<VecDeque<SpuEvent>>>,
    /// Connection state
    connected: Arc<AtomicBool>,
    /// Number of active SPU threads
    active_threads: Arc<AtomicU32>,
    /// Completed workload count
    completed_workloads: Arc<AtomicU32>,
}

impl SpuBridgeSender {
    /// Submit a workload to run on SPU
    pub fn submit_workload(&self, workload: SpuWorkload) -> bool {
        if !self.connected.load(Ordering::Acquire) {
            return false;
        }
        
        let mut queue = self.queue.lock();
        queue.push_back(SpuBridgeMessage::SubmitWorkload(workload));
        true
    }
    
    /// Create an SPU thread group
    pub fn create_group(&self, request: SpuGroupRequest) -> bool {
        if !self.connected.load(Ordering::Acquire) {
            return false;
        }
        
        let mut queue = self.queue.lock();
        queue.push_back(SpuBridgeMessage::CreateGroup(request));
        true
    }
    
    /// Destroy an SPU thread group
    pub fn destroy_group(&self, group_id: u32) -> bool {
        if !self.connected.load(Ordering::Acquire) {
            return false;
        }
        
        let mut queue = self.queue.lock();
        queue.push_back(SpuBridgeMessage::DestroyGroup(group_id));
        true
    }
    
    /// Start an SPU thread group
    pub fn start_group(&self, group_id: u32) -> bool {
        if !self.connected.load(Ordering::Acquire) {
            return false;
        }
        
        let mut queue = self.queue.lock();
        queue.push_back(SpuBridgeMessage::StartGroup(group_id));
        true
    }
    
    /// Stop an SPU thread group
    pub fn stop_group(&self, group_id: u32) -> bool {
        if !self.connected.load(Ordering::Acquire) {
            return false;
        }
        
        let mut queue = self.queue.lock();
        queue.push_back(SpuBridgeMessage::StopGroup(group_id));
        true
    }
    
    /// Create an SPU thread
    pub fn create_thread(&self, request: SpuThreadRequest) -> bool {
        if !self.connected.load(Ordering::Acquire) {
            return false;
        }
        
        let mut queue = self.queue.lock();
        queue.push_back(SpuBridgeMessage::CreateThread(request));
        true
    }
    
    /// Terminate an SPU thread
    pub fn terminate_thread(&self, thread_id: u32) -> bool {
        if !self.connected.load(Ordering::Acquire) {
            return false;
        }
        
        let mut queue = self.queue.lock();
        queue.push_back(SpuBridgeMessage::TerminateThread(thread_id));
        true
    }
    
    /// Send a signal to an SPU
    pub fn send_signal(&self, spu_id: u8, signal: u32) -> bool {
        if !self.connected.load(Ordering::Acquire) {
            return false;
        }
        
        let mut queue = self.queue.lock();
        queue.push_back(SpuBridgeMessage::SendSignal { spu_id, signal });
        true
    }
    
    /// Write to SPU mailbox
    pub fn write_mailbox(&self, spu_id: u8, value: u32) -> bool {
        if !self.connected.load(Ordering::Acquire) {
            return false;
        }
        
        let mut queue = self.queue.lock();
        queue.push_back(SpuBridgeMessage::WriteMailbox { spu_id, value });
        true
    }
    
    /// Attach an event queue to SPURS
    pub fn attach_event_queue(&self, queue_id: u32, port: u32) -> bool {
        if !self.connected.load(Ordering::Acquire) {
            // In HLE mode without full SPU bridge, always succeed
            return true;
        }
        
        let mut queue = self.queue.lock();
        queue.push_back(SpuBridgeMessage::AttachEventQueue { queue_id, port });
        true
    }
    
    /// Detach an event queue from SPURS
    pub fn detach_event_queue(&self, queue_id: u32, port: u32) -> bool {
        if !self.connected.load(Ordering::Acquire) {
            // In HLE mode without full SPU bridge, always succeed
            return true;
        }
        
        let mut queue = self.queue.lock();
        queue.push_back(SpuBridgeMessage::DetachEventQueue { queue_id, port });
        true
    }
    
    /// Check if connected to SPU manager
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }
    
    /// Get number of active SPU threads
    pub fn active_thread_count(&self) -> u32 {
        self.active_threads.load(Ordering::Acquire)
    }
    
    /// Get number of completed workloads
    pub fn completed_workload_count(&self) -> u32 {
        self.completed_workloads.load(Ordering::Acquire)
    }
    
    /// Poll for events from SPU
    pub fn poll_events(&self) -> Vec<SpuEvent> {
        let mut events = self.events.lock();
        events.drain(..).collect()
    }
    
    /// Check if there are pending events
    pub fn has_events(&self) -> bool {
        let events = self.events.lock();
        !events.is_empty()
    }
}

impl Clone for SpuBridgeSender {
    fn clone(&self) -> Self {
        Self {
            queue: Arc::clone(&self.queue),
            events: Arc::clone(&self.events),
            connected: Arc::clone(&self.connected),
            active_threads: Arc::clone(&self.active_threads),
            completed_workloads: Arc::clone(&self.completed_workloads),
        }
    }
}

/// The receiver side of the SPU bridge (used by SPU manager in oc-integration)
pub struct SpuBridgeReceiver {
    /// Message queue (shared with sender)
    queue: Arc<Mutex<VecDeque<SpuBridgeMessage>>>,
    /// Event queue (we push events here)
    events: Arc<Mutex<VecDeque<SpuEvent>>>,
    /// Connection state
    connected: Arc<AtomicBool>,
    /// Active thread count (we update this)
    active_threads: Arc<AtomicU32>,
    /// Completed workload count (we update this)
    completed_workloads: Arc<AtomicU32>,
}

impl SpuBridgeReceiver {
    /// Try to receive a message (non-blocking)
    pub fn try_recv(&self) -> Option<SpuBridgeMessage> {
        let mut queue = self.queue.lock();
        queue.pop_front()
    }
    
    /// Drain all pending messages
    pub fn drain(&self) -> Vec<SpuBridgeMessage> {
        let mut queue = self.queue.lock();
        queue.drain(..).collect()
    }
    
    /// Check if there are pending messages
    pub fn has_pending(&self) -> bool {
        let queue = self.queue.lock();
        !queue.is_empty()
    }
    
    /// Send an event back to SPURS
    pub fn send_event(&self, event: SpuEvent) {
        let mut events = self.events.lock();
        events.push_back(event);
    }
    
    /// Update active thread count
    pub fn set_active_threads(&self, count: u32) {
        self.active_threads.store(count, Ordering::Release);
    }
    
    /// Increment active thread count
    pub fn increment_active_threads(&self) {
        self.active_threads.fetch_add(1, Ordering::Release);
    }
    
    /// Decrement active thread count
    pub fn decrement_active_threads(&self) {
        self.active_threads.fetch_sub(1, Ordering::Release);
    }
    
    /// Increment completed workload count
    pub fn increment_completed_workloads(&self) {
        self.completed_workloads.fetch_add(1, Ordering::Release);
    }
    
    /// Mark the bridge as connected
    pub fn connect(&self) {
        self.connected.store(true, Ordering::Release);
        tracing::info!("SPU bridge connected");
    }
    
    /// Mark the bridge as disconnected
    pub fn disconnect(&self) {
        self.connected.store(false, Ordering::Release);
        tracing::info!("SPU bridge disconnected");
    }
}

/// Create a new SPU bridge pair (sender, receiver)
pub fn create_spu_bridge() -> (SpuBridgeSender, SpuBridgeReceiver) {
    let queue = Arc::new(Mutex::new(VecDeque::with_capacity(SPU_BRIDGE_QUEUE_CAPACITY)));
    let events = Arc::new(Mutex::new(VecDeque::with_capacity(256)));
    let connected = Arc::new(AtomicBool::new(false));
    let active_threads = Arc::new(AtomicU32::new(0));
    let completed_workloads = Arc::new(AtomicU32::new(0));
    
    let sender = SpuBridgeSender {
        queue: Arc::clone(&queue),
        events: Arc::clone(&events),
        connected: Arc::clone(&connected),
        active_threads: Arc::clone(&active_threads),
        completed_workloads: Arc::clone(&completed_workloads),
    };
    
    let receiver = SpuBridgeReceiver {
        queue,
        events,
        connected,
        active_threads,
        completed_workloads,
    };
    
    (sender, receiver)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_bridge_creation() {
        let (sender, receiver) = create_spu_bridge();
        assert!(!sender.is_connected());
        receiver.connect();
        assert!(sender.is_connected());
    }
    
    #[test]
    fn test_workload_submission() {
        let (sender, receiver) = create_spu_bridge();
        receiver.connect();
        
        let workload = SpuWorkload {
            id: 1,
            entry_point: 0x10000,
            program_size: 0x1000,
            argument: 42,
            priority: 0,
            affinity: 0x3F,
        };
        
        assert!(sender.submit_workload(workload));
        assert!(receiver.has_pending());
        
        let msg = receiver.try_recv().unwrap();
        match msg {
            SpuBridgeMessage::SubmitWorkload(w) => {
                assert_eq!(w.id, 1);
                assert_eq!(w.entry_point, 0x10000);
            }
            _ => panic!("Wrong message type"),
        }
    }
    
    #[test]
    fn test_event_sending() {
        let (sender, receiver) = create_spu_bridge();
        receiver.connect();
        
        receiver.send_event(SpuEvent {
            event_type: SpuEventType::WorkloadComplete,
            spu_id: 0,
            data: 123,
        });
        
        assert!(sender.has_events());
        let events = sender.poll_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, SpuEventType::WorkloadComplete);
    }
    
    #[test]
    fn test_active_thread_tracking() {
        let (sender, receiver) = create_spu_bridge();
        receiver.connect();
        
        assert_eq!(sender.active_thread_count(), 0);
        
        receiver.increment_active_threads();
        assert_eq!(sender.active_thread_count(), 1);
        
        receiver.increment_active_threads();
        assert_eq!(sender.active_thread_count(), 2);
        
        receiver.decrement_active_threads();
        assert_eq!(sender.active_thread_count(), 1);
    }
}
