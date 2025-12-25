//! Event (sys_event_*)

use crate::objects::{KernelObject, ObjectId, ObjectManager, ObjectType};
use oc_core::error::KernelError;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

/// Event queue attributes
#[derive(Debug, Clone, Copy)]
pub struct EventQueueAttributes {
    pub protocol: u32,
    pub queue_type: u32,
}

impl Default for EventQueueAttributes {
    fn default() -> Self {
        Self {
            protocol: 1,
            queue_type: 1, // SPU_QUEUE_TYPE_LOCAL
        }
    }
}

/// Event port attributes
#[derive(Debug, Clone, Copy)]
pub struct EventPortAttributes {
    pub name: u64,
}

impl Default for EventPortAttributes {
    fn default() -> Self {
        Self { name: 0 }
    }
}

/// Event data
#[derive(Debug, Clone, Copy)]
pub struct Event {
    pub source: u64,
    pub data1: u64,
    pub data2: u64,
    pub data3: u64,
}

/// LV2 Event Queue implementation
pub struct EventQueue {
    id: ObjectId,
    inner: Mutex<EventQueueState>,
    attributes: EventQueueAttributes,
}

#[derive(Debug)]
struct EventQueueState {
    events: VecDeque<Event>,
    max_size: usize,
    /// Thread IDs waiting for events
    waiting_threads: VecDeque<u64>,
}

impl EventQueue {
    pub fn new(id: ObjectId, attributes: EventQueueAttributes, size: usize) -> Self {
        Self {
            id,
            inner: Mutex::new(EventQueueState {
                events: VecDeque::with_capacity(size),
                max_size: size,
                waiting_threads: VecDeque::new(),
            }),
            attributes,
        }
    }

    pub fn send(&self, event: Event) -> Result<(), KernelError> {
        let mut state = self.inner.lock();

        if state.events.len() >= state.max_size {
            return Err(KernelError::ResourceLimit);
        }

        state.events.push_back(event);
        // Wake up first waiting thread if any (will be scheduled by thread manager)
        if let Some(thread_id) = state.waiting_threads.pop_front() {
            tracing::debug!("Event queue {}: waking thread {}", self.id, thread_id);
        }
        Ok(())
    }

    /// Receive an event from the queue
    /// Note: Timeout parameter is accepted for API compatibility but actual
    /// blocking with timeout requires thread scheduler integration (not yet implemented)
    pub fn receive(&self, _timeout: Option<Duration>) -> Result<Event, KernelError> {
        let mut state = self.inner.lock();

        if let Some(event) = state.events.pop_front() {
            Ok(event)
        } else {
            Err(KernelError::WouldBlock)
        }
    }

    /// Receive with thread registration for waiting
    /// Note: Timeout parameter is accepted for API compatibility but actual
    /// blocking with timeout requires thread scheduler integration (not yet implemented)
    pub fn receive_with_wait(&self, thread_id: u64, _timeout: Option<Duration>) -> Result<Event, KernelError> {
        let mut state = self.inner.lock();

        if let Some(event) = state.events.pop_front() {
            Ok(event)
        } else {
            // Register thread as waiting
            if !state.waiting_threads.contains(&thread_id) {
                state.waiting_threads.push_back(thread_id);
                tracing::debug!("Event queue {}: thread {} waiting", self.id, thread_id);
            }
            Err(KernelError::WouldBlock)
        }
    }

    /// Cancel wait for a specific thread
    pub fn cancel_wait(&self, thread_id: u64) {
        let mut state = self.inner.lock();
        state.waiting_threads.retain(|&id| id != thread_id);
    }

    pub fn tryreceive(&self) -> Result<Event, KernelError> {
        let mut state = match self.inner.try_lock() {
            Some(s) => s,
            None => return Err(KernelError::WouldBlock),
        };

        state.events.pop_front().ok_or(KernelError::WouldBlock)
    }

    /// Clear all events from the queue
    pub fn clear(&self) {
        let mut state = self.inner.lock();
        state.events.clear();
        tracing::debug!("Event queue {}: cleared all events", self.id);
    }

    /// Drain all events from the queue and return them
    pub fn drain(&self) -> Vec<Event> {
        let mut state = self.inner.lock();
        let events: Vec<Event> = state.events.drain(..).collect();
        tracing::debug!("Event queue {}: drained {} events", self.id, events.len());
        events
    }

    /// Get number of pending events
    pub fn pending_count(&self) -> usize {
        self.inner.lock().events.len()
    }

    /// Get number of waiting threads
    pub fn waiting_count(&self) -> usize {
        self.inner.lock().waiting_threads.len()
    }

    /// Check if the queue is empty
    pub fn is_empty(&self) -> bool {
        self.inner.lock().events.is_empty()
    }

    /// Check if the queue is full
    pub fn is_full(&self) -> bool {
        let state = self.inner.lock();
        state.events.len() >= state.max_size
    }
}

impl KernelObject for EventQueue {
    fn object_type(&self) -> ObjectType {
        ObjectType::EventQueue
    }

    fn id(&self) -> ObjectId {
        self.id
    }

    fn as_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

/// LV2 Event Port implementation
pub struct EventPort {
    id: ObjectId,
    queue_id: ObjectId,
    attributes: EventPortAttributes,
}

impl EventPort {
    pub fn new(id: ObjectId, queue_id: ObjectId, attributes: EventPortAttributes) -> Self {
        Self {
            id,
            queue_id,
            attributes,
        }
    }

    pub fn queue_id(&self) -> ObjectId {
        self.queue_id
    }
}

impl KernelObject for EventPort {
    fn object_type(&self) -> ObjectType {
        ObjectType::EventPort
    }

    fn id(&self) -> ObjectId {
        self.id
    }

    fn as_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

/// Event syscall implementations
pub mod syscalls {
    use super::*;

    /// sys_event_queue_create
    pub fn sys_event_queue_create(
        manager: &ObjectManager,
        attributes: EventQueueAttributes,
        size: usize,
    ) -> Result<ObjectId, KernelError> {
        let id = manager.next_id();
        let queue = Arc::new(EventQueue::new(id, attributes, size));
        manager.register(queue);
        Ok(id)
    }

    /// sys_event_queue_destroy
    pub fn sys_event_queue_destroy(
        manager: &ObjectManager,
        queue_id: ObjectId,
    ) -> Result<(), KernelError> {
        manager.unregister(queue_id)
    }

    /// sys_event_queue_receive
    pub fn sys_event_queue_receive(
        manager: &ObjectManager,
        queue_id: ObjectId,
        timeout_usec: u64,
    ) -> Result<Event, KernelError> {
        let queue: Arc<EventQueue> = manager.get(queue_id)?;

        let timeout = if timeout_usec == 0 {
            None
        } else {
            Some(Duration::from_micros(timeout_usec))
        };

        queue.receive(timeout)
    }

    /// sys_event_queue_tryreceive
    pub fn sys_event_queue_tryreceive(
        manager: &ObjectManager,
        queue_id: ObjectId,
    ) -> Result<Event, KernelError> {
        let queue: Arc<EventQueue> = manager.get(queue_id)?;
        queue.tryreceive()
    }

    /// sys_event_port_create
    pub fn sys_event_port_create(
        manager: &ObjectManager,
        queue_id: ObjectId,
        attributes: EventPortAttributes,
    ) -> Result<ObjectId, KernelError> {
        // Verify queue exists
        let _queue: Arc<EventQueue> = manager.get(queue_id)?;

        let id = manager.next_id();
        let port = Arc::new(EventPort::new(id, queue_id, attributes));
        manager.register(port);
        Ok(id)
    }

    /// sys_event_port_destroy
    pub fn sys_event_port_destroy(
        manager: &ObjectManager,
        port_id: ObjectId,
    ) -> Result<(), KernelError> {
        manager.unregister(port_id)
    }

    /// sys_event_port_send
    pub fn sys_event_port_send(
        manager: &ObjectManager,
        port_id: ObjectId,
        data1: u64,
        data2: u64,
        data3: u64,
    ) -> Result<(), KernelError> {
        let port: Arc<EventPort> = manager.get(port_id)?;
        let queue: Arc<EventQueue> = manager.get(port.queue_id())?;

        let event = Event {
            source: port_id as u64,
            data1,
            data2,
            data3,
        };

        queue.send(event)
    }

    /// sys_event_queue_clear - Clear all events from the queue
    pub fn sys_event_queue_clear(
        manager: &ObjectManager,
        queue_id: ObjectId,
    ) -> Result<(), KernelError> {
        let queue: Arc<EventQueue> = manager.get(queue_id)?;
        queue.clear();
        Ok(())
    }

    /// sys_event_queue_drain - Remove and return all events from the queue
    pub fn sys_event_queue_drain(
        manager: &ObjectManager,
        queue_id: ObjectId,
    ) -> Result<Vec<Event>, KernelError> {
        let queue: Arc<EventQueue> = manager.get(queue_id)?;
        Ok(queue.drain())
    }

    /// sys_event_queue_receive_wait - Receive with thread waiting support
    pub fn sys_event_queue_receive_wait(
        manager: &ObjectManager,
        queue_id: ObjectId,
        thread_id: u64,
        timeout_usec: u64,
    ) -> Result<Event, KernelError> {
        let queue: Arc<EventQueue> = manager.get(queue_id)?;

        let timeout = if timeout_usec == 0 {
            None
        } else {
            Some(Duration::from_micros(timeout_usec))
        };

        queue.receive_with_wait(thread_id, timeout)
    }

    /// Cancel wait for a specific thread
    pub fn sys_event_queue_cancel_wait(
        manager: &ObjectManager,
        queue_id: ObjectId,
        thread_id: u64,
    ) -> Result<(), KernelError> {
        let queue: Arc<EventQueue> = manager.get(queue_id)?;
        queue.cancel_wait(thread_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_queue() {
        let manager = ObjectManager::new();
        let queue_id = syscalls::sys_event_queue_create(
            &manager,
            EventQueueAttributes::default(),
            10,
        )
        .unwrap();

        let queue: Arc<EventQueue> = manager.get(queue_id).unwrap();

        // Send event
        let event = Event {
            source: 1,
            data1: 0x123,
            data2: 0x456,
            data3: 0x789,
        };
        queue.send(event).unwrap();

        // Receive event
        let received = syscalls::sys_event_queue_tryreceive(&manager, queue_id).unwrap();
        assert_eq!(received.source, 1);
        assert_eq!(received.data1, 0x123);

        syscalls::sys_event_queue_destroy(&manager, queue_id).unwrap();
    }

    #[test]
    fn test_event_port() {
        let manager = ObjectManager::new();
        let queue_id = syscalls::sys_event_queue_create(
            &manager,
            EventQueueAttributes::default(),
            10,
        )
        .unwrap();

        let port_id = syscalls::sys_event_port_create(
            &manager,
            queue_id,
            EventPortAttributes::default(),
        )
        .unwrap();

        // Send via port
        syscalls::sys_event_port_send(&manager, port_id, 0x111, 0x222, 0x333).unwrap();

        // Receive from queue
        let event = syscalls::sys_event_queue_tryreceive(&manager, queue_id).unwrap();
        assert_eq!(event.data1, 0x111);

        syscalls::sys_event_port_destroy(&manager, port_id).unwrap();
        syscalls::sys_event_queue_destroy(&manager, queue_id).unwrap();
    }

    #[test]
    fn test_event_queue_clear() {
        let manager = ObjectManager::new();
        let queue_id = syscalls::sys_event_queue_create(
            &manager,
            EventQueueAttributes::default(),
            10,
        )
        .unwrap();

        let queue: Arc<EventQueue> = manager.get(queue_id).unwrap();

        // Send multiple events
        for i in 0..5 {
            let event = Event {
                source: 1,
                data1: i,
                data2: 0,
                data3: 0,
            };
            queue.send(event).unwrap();
        }

        assert_eq!(queue.pending_count(), 5);

        // Clear the queue
        syscalls::sys_event_queue_clear(&manager, queue_id).unwrap();
        assert_eq!(queue.pending_count(), 0);
        assert!(queue.is_empty());

        syscalls::sys_event_queue_destroy(&manager, queue_id).unwrap();
    }

    #[test]
    fn test_event_queue_drain() {
        let manager = ObjectManager::new();
        let queue_id = syscalls::sys_event_queue_create(
            &manager,
            EventQueueAttributes::default(),
            10,
        )
        .unwrap();

        let queue: Arc<EventQueue> = manager.get(queue_id).unwrap();

        // Send multiple events
        for i in 0..3 {
            let event = Event {
                source: 1,
                data1: i,
                data2: 0,
                data3: 0,
            };
            queue.send(event).unwrap();
        }

        assert_eq!(queue.pending_count(), 3);

        // Drain the queue
        let events = syscalls::sys_event_queue_drain(&manager, queue_id).unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].data1, 0);
        assert_eq!(events[1].data1, 1);
        assert_eq!(events[2].data1, 2);

        // Queue should be empty after drain
        assert!(queue.is_empty());

        syscalls::sys_event_queue_destroy(&manager, queue_id).unwrap();
    }

    #[test]
    fn test_event_queue_waiting_threads() {
        let manager = ObjectManager::new();
        let queue_id = syscalls::sys_event_queue_create(
            &manager,
            EventQueueAttributes::default(),
            10,
        )
        .unwrap();

        let queue: Arc<EventQueue> = manager.get(queue_id).unwrap();

        // Try to receive from empty queue - should register as waiting
        let result = syscalls::sys_event_queue_receive_wait(&manager, queue_id, 1, 0);
        assert!(result.is_err());
        assert_eq!(queue.waiting_count(), 1);

        // Register another waiting thread
        let result = syscalls::sys_event_queue_receive_wait(&manager, queue_id, 2, 0);
        assert!(result.is_err());
        assert_eq!(queue.waiting_count(), 2);

        // Same thread shouldn't be registered twice
        let result = syscalls::sys_event_queue_receive_wait(&manager, queue_id, 1, 0);
        assert!(result.is_err());
        assert_eq!(queue.waiting_count(), 2);

        // Cancel wait for thread 1
        syscalls::sys_event_queue_cancel_wait(&manager, queue_id, 1).unwrap();
        assert_eq!(queue.waiting_count(), 1);

        syscalls::sys_event_queue_destroy(&manager, queue_id).unwrap();
    }

    #[test]
    fn test_event_queue_full_check() {
        let manager = ObjectManager::new();
        let queue_id = syscalls::sys_event_queue_create(
            &manager,
            EventQueueAttributes::default(),
            3, // Small capacity
        )
        .unwrap();

        let queue: Arc<EventQueue> = manager.get(queue_id).unwrap();

        assert!(!queue.is_full());

        // Fill the queue
        for i in 0..3 {
            let event = Event {
                source: 1,
                data1: i,
                data2: 0,
                data3: 0,
            };
            queue.send(event).unwrap();
        }

        assert!(queue.is_full());

        // Should fail to send when full
        let event = Event {
            source: 1,
            data1: 99,
            data2: 0,
            data3: 0,
        };
        assert!(queue.send(event).is_err());

        syscalls::sys_event_queue_destroy(&manager, queue_id).unwrap();
    }
}

