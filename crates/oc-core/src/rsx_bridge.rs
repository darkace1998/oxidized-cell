//! RSX Bridge - Communication channel between GCM HLE and RSX backend
//!
//! This module provides a decoupled communication mechanism between the
//! cellGcmSys HLE module and the RSX graphics backend, avoiding circular
//! dependencies between oc-hle and oc-rsx.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use parking_lot::{Mutex, Condvar};
use std::collections::VecDeque;

/// Maximum number of commands in the bridge queue
pub const BRIDGE_QUEUE_CAPACITY: usize = 65536;

/// RSX command to be sent across the bridge
#[derive(Debug, Clone, Copy)]
pub struct BridgeCommand {
    /// Method (register offset)
    pub method: u32,
    /// Data value
    pub data: u32,
}

/// Display buffer configuration to send to RSX
#[derive(Debug, Clone, Copy, Default)]
pub struct BridgeDisplayBuffer {
    /// Buffer ID (0-7)
    pub id: u32,
    /// Buffer offset in memory
    pub offset: u32,
    /// Pitch (bytes per line)
    pub pitch: u32,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
}

/// Flip request sent from GCM to RSX
#[derive(Debug, Clone, Copy)]
pub struct BridgeFlipRequest {
    /// Buffer ID to flip to
    pub buffer_id: u32,
}

/// Bridge message types
#[derive(Debug, Clone)]
pub enum BridgeMessage {
    /// Commands to execute
    Commands(Vec<BridgeCommand>),
    /// Configure a display buffer
    ConfigureDisplayBuffer(BridgeDisplayBuffer),
    /// Request a flip
    Flip(BridgeFlipRequest),
    /// Finish/sync request (wait for all commands to complete)
    Finish,
}

/// Flip status returned from RSX to GCM
#[derive(Debug, Clone, Copy, Default)]
pub struct FlipStatus {
    /// Number of flips completed
    pub flip_count: u32,
    /// Whether a flip is pending
    pub flip_pending: bool,
    /// Current display buffer index
    pub current_buffer: u32,
}

/// The sender side of the RSX bridge (used by GcmManager in oc-hle)
pub struct RsxBridgeSender {
    /// Message queue
    queue: Arc<Mutex<VecDeque<BridgeMessage>>>,
    /// Condvar to signal new messages
    condvar: Arc<Condvar>,
    /// Connection state
    connected: Arc<AtomicBool>,
    /// Flip count (updated by RSX)
    flip_count: Arc<AtomicU32>,
    /// Current display buffer (updated by RSX)
    current_buffer: Arc<AtomicU32>,
    /// Flip pending flag
    flip_pending: Arc<AtomicBool>,
    /// Finish/sync complete flag
    finish_complete: Arc<AtomicBool>,
    /// Finish condvar for waiting
    finish_condvar: Arc<Condvar>,
    /// Finish mutex
    finish_mutex: Arc<Mutex<()>>,
}

impl RsxBridgeSender {
    /// Send commands to RSX
    pub fn send_commands(&self, commands: Vec<BridgeCommand>) -> bool {
        if !self.connected.load(Ordering::Acquire) {
            return false;
        }
        
        let mut queue = self.queue.lock();
        queue.push_back(BridgeMessage::Commands(commands));
        self.condvar.notify_one();
        true
    }
    
    /// Configure a display buffer
    pub fn configure_display_buffer(&self, buffer: BridgeDisplayBuffer) -> bool {
        if !self.connected.load(Ordering::Acquire) {
            return false;
        }
        
        let mut queue = self.queue.lock();
        queue.push_back(BridgeMessage::ConfigureDisplayBuffer(buffer));
        self.condvar.notify_one();
        true
    }
    
    /// Request a flip
    pub fn queue_flip(&self, buffer_id: u32) -> bool {
        if !self.connected.load(Ordering::Acquire) {
            return false;
        }
        
        self.flip_pending.store(true, Ordering::Release);
        
        let mut queue = self.queue.lock();
        queue.push_back(BridgeMessage::Flip(BridgeFlipRequest { buffer_id }));
        self.condvar.notify_one();
        true
    }
    
    /// Request finish (sync) and wait for completion
    pub fn finish(&self) -> bool {
        if !self.connected.load(Ordering::Acquire) {
            return false;
        }
        
        // Reset finish flag
        self.finish_complete.store(false, Ordering::Release);
        
        // Send finish request
        {
            let mut queue = self.queue.lock();
            queue.push_back(BridgeMessage::Finish);
            self.condvar.notify_one();
        }
        
        // Wait for finish to complete
        let mut guard = self.finish_mutex.lock();
        while !self.finish_complete.load(Ordering::Acquire) {
            self.finish_condvar.wait(&mut guard);
        }
        
        true
    }
    
    /// Check if connected to RSX
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }
    
    /// Get current flip status
    pub fn get_flip_status(&self) -> FlipStatus {
        FlipStatus {
            flip_count: self.flip_count.load(Ordering::Acquire),
            flip_pending: self.flip_pending.load(Ordering::Acquire),
            current_buffer: self.current_buffer.load(Ordering::Acquire),
        }
    }
}

impl Clone for RsxBridgeSender {
    fn clone(&self) -> Self {
        Self {
            queue: Arc::clone(&self.queue),
            condvar: Arc::clone(&self.condvar),
            connected: Arc::clone(&self.connected),
            flip_count: Arc::clone(&self.flip_count),
            current_buffer: Arc::clone(&self.current_buffer),
            flip_pending: Arc::clone(&self.flip_pending),
            finish_complete: Arc::clone(&self.finish_complete),
            finish_condvar: Arc::clone(&self.finish_condvar),
            finish_mutex: Arc::clone(&self.finish_mutex),
        }
    }
}

/// The receiver side of the RSX bridge (used by RsxThread in oc-rsx)
pub struct RsxBridgeReceiver {
    /// Message queue (shared with sender)
    queue: Arc<Mutex<VecDeque<BridgeMessage>>>,
    /// Condvar to wait for messages (reserved for blocking receive)
    #[allow(dead_code)]
    condvar: Arc<Condvar>,
    /// Connection state
    connected: Arc<AtomicBool>,
    /// Flip count (we update this)
    flip_count: Arc<AtomicU32>,
    /// Current display buffer (we update this)
    current_buffer: Arc<AtomicU32>,
    /// Flip pending flag (we clear this)
    flip_pending: Arc<AtomicBool>,
    /// Finish complete flag (we set this)
    finish_complete: Arc<AtomicBool>,
    /// Finish condvar (we signal this)
    finish_condvar: Arc<Condvar>,
    /// Finish mutex
    finish_mutex: Arc<Mutex<()>>,
}

impl RsxBridgeReceiver {
    /// Try to receive a message (non-blocking)
    pub fn try_recv(&self) -> Option<BridgeMessage> {
        let mut queue = self.queue.lock();
        queue.pop_front()
    }
    
    /// Drain all pending messages
    pub fn drain(&self) -> Vec<BridgeMessage> {
        let mut queue = self.queue.lock();
        queue.drain(..).collect()
    }
    
    /// Check if there are pending messages
    pub fn has_pending(&self) -> bool {
        let queue = self.queue.lock();
        !queue.is_empty()
    }
    
    /// Signal that a flip has completed
    pub fn signal_flip_complete(&self, buffer_id: u32) {
        self.flip_count.fetch_add(1, Ordering::Release);
        self.current_buffer.store(buffer_id, Ordering::Release);
        self.flip_pending.store(false, Ordering::Release);
    }
    
    /// Signal that a finish/sync has completed
    pub fn signal_finish_complete(&self) {
        self.finish_complete.store(true, Ordering::Release);
        let _guard = self.finish_mutex.lock();
        self.finish_condvar.notify_all();
    }
    
    /// Mark the bridge as connected
    pub fn connect(&self) {
        self.connected.store(true, Ordering::Release);
        tracing::info!("RSX bridge connected");
    }
    
    /// Mark the bridge as disconnected
    pub fn disconnect(&self) {
        self.connected.store(false, Ordering::Release);
        tracing::info!("RSX bridge disconnected");
    }
}

/// Create a new RSX bridge pair (sender, receiver)
pub fn create_rsx_bridge() -> (RsxBridgeSender, RsxBridgeReceiver) {
    let queue = Arc::new(Mutex::new(VecDeque::with_capacity(BRIDGE_QUEUE_CAPACITY)));
    let condvar = Arc::new(Condvar::new());
    let connected = Arc::new(AtomicBool::new(false));
    let flip_count = Arc::new(AtomicU32::new(0));
    let current_buffer = Arc::new(AtomicU32::new(0));
    let flip_pending = Arc::new(AtomicBool::new(false));
    let finish_complete = Arc::new(AtomicBool::new(false));
    let finish_condvar = Arc::new(Condvar::new());
    let finish_mutex = Arc::new(Mutex::new(()));
    
    let sender = RsxBridgeSender {
        queue: Arc::clone(&queue),
        condvar: Arc::clone(&condvar),
        connected: Arc::clone(&connected),
        flip_count: Arc::clone(&flip_count),
        current_buffer: Arc::clone(&current_buffer),
        flip_pending: Arc::clone(&flip_pending),
        finish_complete: Arc::clone(&finish_complete),
        finish_condvar: Arc::clone(&finish_condvar),
        finish_mutex: Arc::clone(&finish_mutex),
    };
    
    let receiver = RsxBridgeReceiver {
        queue,
        condvar,
        connected,
        flip_count,
        current_buffer,
        flip_pending,
        finish_complete,
        finish_condvar,
        finish_mutex,
    };
    
    (sender, receiver)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_bridge_creation() {
        let (sender, receiver) = create_rsx_bridge();
        
        assert!(!sender.is_connected());
        receiver.connect();
        assert!(sender.is_connected());
    }
    
    #[test]
    fn test_command_sending() {
        let (sender, receiver) = create_rsx_bridge();
        receiver.connect();
        
        let commands = vec![
            BridgeCommand { method: 0x100, data: 0x1234 },
            BridgeCommand { method: 0x104, data: 0x5678 },
        ];
        
        assert!(sender.send_commands(commands));
        assert!(receiver.has_pending());
        
        let messages = receiver.drain();
        assert_eq!(messages.len(), 1);
        
        if let BridgeMessage::Commands(cmds) = &messages[0] {
            assert_eq!(cmds.len(), 2);
            assert_eq!(cmds[0].method, 0x100);
            assert_eq!(cmds[1].data, 0x5678);
        } else {
            panic!("Expected Commands message");
        }
    }
    
    #[test]
    fn test_flip_status() {
        let (sender, receiver) = create_rsx_bridge();
        receiver.connect();
        
        assert!(sender.queue_flip(1));
        
        let status = sender.get_flip_status();
        assert!(status.flip_pending);
        assert_eq!(status.flip_count, 0);
        
        receiver.signal_flip_complete(1);
        
        let status = sender.get_flip_status();
        assert!(!status.flip_pending);
        assert_eq!(status.flip_count, 1);
        assert_eq!(status.current_buffer, 1);
    }
}
