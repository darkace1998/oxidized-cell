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
}

impl EventQueue {
    pub fn new(id: ObjectId, attributes: EventQueueAttributes, size: usize) -> Self {
        Self {
            id,
            inner: Mutex::new(EventQueueState {
                events: VecDeque::with_capacity(size),
                max_size: size,
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
        Ok(())
    }

    pub fn receive(&self, timeout: Option<Duration>) -> Result<Event, KernelError> {
        let mut state = self.inner.lock();

        if let Some(event) = state.events.pop_front() {
            Ok(event)
        } else {
            Err(KernelError::WouldBlock)
        }
    }

    pub fn tryreceive(&self) -> Result<Event, KernelError> {
        let mut state = match self.inner.try_lock() {
            Some(s) => s,
            None => return Err(KernelError::WouldBlock),
        };

        state.events.pop_front().ok_or(KernelError::WouldBlock)
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
}

