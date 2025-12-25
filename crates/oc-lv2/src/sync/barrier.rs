//! Barrier (sys_barrier_*)
//!
//! Barriers are synchronization primitives that allow multiple threads
//! to wait until all threads reach a specific point before continuing.

use crate::objects::{KernelObject, ObjectId, ObjectManager, ObjectType};
use oc_core::error::KernelError;
use parking_lot::{Condvar, Mutex as ParkingMutex};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Barrier attributes
#[derive(Debug, Clone, Copy)]
pub struct BarrierAttributes {
    /// Protocol for barrier
    pub protocol: u32,
    /// Shared across processes
    pub pshared: u32,
}

impl Default for BarrierAttributes {
    fn default() -> Self {
        Self {
            protocol: 0x02, // SYS_SYNC_PRIORITY
            pshared: 0,     // Process-local
        }
    }
}

/// Wait result for barriers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarrierWaitResult {
    /// Thread is the serial thread (last to arrive)
    Serial,
    /// Thread is a regular participant
    Participant,
    /// Timed out waiting
    TimedOut,
}

/// LV2 Barrier implementation
pub struct Barrier {
    id: ObjectId,
    state: ParkingMutex<BarrierState>,
    condvar: Condvar,
    /// Number of threads required for barrier
    count: u32,
    /// Stored for introspection
    attributes: BarrierAttributes,
}

#[derive(Debug)]
struct BarrierState {
    /// Number of threads currently waiting
    waiting: u32,
    /// Generation counter (incremented each time barrier is released)
    generation: u64,
}

impl Barrier {
    pub fn new(id: ObjectId, count: u32, attributes: BarrierAttributes) -> Self {
        Self {
            id,
            state: ParkingMutex::new(BarrierState {
                waiting: 0,
                generation: 0,
            }),
            condvar: Condvar::new(),
            count,
            attributes,
        }
    }

    /// Wait at the barrier until all threads arrive
    pub fn wait(&self, timeout: Option<Duration>) -> Result<BarrierWaitResult, KernelError> {
        let start = Instant::now();
        let mut state = self.state.lock();
        let my_generation = state.generation;
        
        state.waiting += 1;
        
        if state.waiting >= self.count {
            // We're the last thread - release everyone
            state.waiting = 0;
            state.generation += 1;
            self.condvar.notify_all();
            tracing::debug!("Barrier {}: thread released barrier (serial), generation {}", 
                           self.id, state.generation);
            return Ok(BarrierWaitResult::Serial);
        }
        
        // Wait for the barrier to be released
        loop {
            // Check if barrier was released
            if state.generation != my_generation {
                tracing::debug!("Barrier {}: thread released (participant)", self.id);
                return Ok(BarrierWaitResult::Participant);
            }
            
            // Check timeout
            if let Some(duration) = timeout {
                let elapsed = start.elapsed();
                if elapsed >= duration {
                    state.waiting -= 1;
                    return Ok(BarrierWaitResult::TimedOut);
                }
                let remaining = duration - elapsed;
                let wait_result = self.condvar.wait_for(&mut state, remaining);
                if wait_result.timed_out() && state.generation == my_generation {
                    state.waiting -= 1;
                    return Ok(BarrierWaitResult::TimedOut);
                }
            } else {
                self.condvar.wait(&mut state);
            }
        }
    }

    /// Get the number of threads required
    pub fn get_count(&self) -> u32 {
        self.count
    }

    /// Get the number of threads currently waiting
    pub fn get_waiting(&self) -> u32 {
        self.state.lock().waiting
    }

    /// Get the generation counter
    pub fn get_generation(&self) -> u64 {
        self.state.lock().generation
    }

    /// Get attributes
    pub fn get_attributes(&self) -> BarrierAttributes {
        self.attributes
    }
}

impl KernelObject for Barrier {
    fn object_type(&self) -> ObjectType {
        ObjectType::Barrier
    }

    fn id(&self) -> ObjectId {
        self.id
    }

    fn as_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

/// Barrier syscall implementations
pub mod syscalls {
    use super::*;

    /// sys_barrier_create
    pub fn sys_barrier_create(
        manager: &ObjectManager,
        count: u32,
        attributes: BarrierAttributes,
    ) -> Result<ObjectId, KernelError> {
        if count == 0 {
            return Err(KernelError::ResourceLimit);
        }
        
        let id = manager.next_id();
        let barrier = Arc::new(Barrier::new(id, count, attributes));
        manager.register(barrier);
        tracing::debug!("Created barrier {} with count {}", id, count);
        Ok(id)
    }

    /// sys_barrier_destroy
    pub fn sys_barrier_destroy(
        manager: &ObjectManager,
        barrier_id: ObjectId,
    ) -> Result<(), KernelError> {
        let barrier: Arc<Barrier> = manager.get(barrier_id)?;
        if barrier.get_waiting() > 0 {
            // Cannot destroy barrier with waiting threads
            return Err(KernelError::PermissionDenied);
        }
        manager.unregister(barrier_id)
    }

    /// sys_barrier_wait
    pub fn sys_barrier_wait(
        manager: &ObjectManager,
        barrier_id: ObjectId,
        timeout_usec: u64,
    ) -> Result<BarrierWaitResult, KernelError> {
        let barrier: Arc<Barrier> = manager.get(barrier_id)?;
        
        let timeout = if timeout_usec == 0 {
            None
        } else {
            Some(Duration::from_micros(timeout_usec))
        };

        barrier.wait(timeout)
    }

    /// sys_barrier_get_count
    pub fn sys_barrier_get_count(
        manager: &ObjectManager,
        barrier_id: ObjectId,
    ) -> Result<u32, KernelError> {
        let barrier: Arc<Barrier> = manager.get(barrier_id)?;
        Ok(barrier.get_count())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn test_barrier_create_destroy() {
        let manager = ObjectManager::new();
        let barrier_id = syscalls::sys_barrier_create(
            &manager,
            3,
            BarrierAttributes::default(),
        ).unwrap();

        assert!(manager.exists(barrier_id));

        syscalls::sys_barrier_destroy(&manager, barrier_id).unwrap();
        assert!(!manager.exists(barrier_id));
    }

    #[test]
    fn test_barrier_zero_count_fails() {
        let manager = ObjectManager::new();
        let result = syscalls::sys_barrier_create(
            &manager,
            0,
            BarrierAttributes::default(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_barrier_get_count() {
        let manager = ObjectManager::new();
        let barrier_id = syscalls::sys_barrier_create(
            &manager,
            5,
            BarrierAttributes::default(),
        ).unwrap();

        let count = syscalls::sys_barrier_get_count(&manager, barrier_id).unwrap();
        assert_eq!(count, 5);

        syscalls::sys_barrier_destroy(&manager, barrier_id).unwrap();
    }

    #[test]
    fn test_barrier_single_thread() {
        let manager = ObjectManager::new();
        let barrier_id = syscalls::sys_barrier_create(
            &manager,
            1, // Single thread barrier
            BarrierAttributes::default(),
        ).unwrap();

        // Single thread should immediately be the serial thread
        let result = syscalls::sys_barrier_wait(&manager, barrier_id, 0).unwrap();
        assert_eq!(result, BarrierWaitResult::Serial);

        syscalls::sys_barrier_destroy(&manager, barrier_id).unwrap();
    }

    #[test]
    fn test_barrier_timeout() {
        let manager = ObjectManager::new();
        let barrier_id = syscalls::sys_barrier_create(
            &manager,
            2, // Need 2 threads
            BarrierAttributes::default(),
        ).unwrap();

        // Wait with short timeout should time out
        let result = syscalls::sys_barrier_wait(&manager, barrier_id, 1000).unwrap();
        assert_eq!(result, BarrierWaitResult::TimedOut);

        syscalls::sys_barrier_destroy(&manager, barrier_id).unwrap();
    }

    #[test]
    fn test_barrier_multithreaded() {
        let manager = Arc::new(ObjectManager::new());
        let barrier_id = syscalls::sys_barrier_create(
            &manager,
            3,
            BarrierAttributes::default(),
        ).unwrap();

        let serial_count = Arc::new(AtomicU32::new(0));
        let participant_count = Arc::new(AtomicU32::new(0));
        let mut handles = vec![];

        for _ in 0..3 {
            let manager_clone = Arc::clone(&manager);
            let serial_clone = Arc::clone(&serial_count);
            let participant_clone = Arc::clone(&participant_count);
            
            let handle = thread::spawn(move || {
                let result = syscalls::sys_barrier_wait(&manager_clone, barrier_id, 0).unwrap();
                match result {
                    BarrierWaitResult::Serial => serial_clone.fetch_add(1, Ordering::SeqCst),
                    BarrierWaitResult::Participant => participant_clone.fetch_add(1, Ordering::SeqCst),
                    _ => 0,
                };
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Exactly one thread should be the serial thread
        assert_eq!(serial_count.load(Ordering::SeqCst), 1);
        // The other two should be participants
        assert_eq!(participant_count.load(Ordering::SeqCst), 2);

        syscalls::sys_barrier_destroy(&manager, barrier_id).unwrap();
    }

    #[test]
    fn test_barrier_reuse() {
        let manager = Arc::new(ObjectManager::new());
        let barrier_id = syscalls::sys_barrier_create(
            &manager,
            2,
            BarrierAttributes::default(),
        ).unwrap();

        let barrier: Arc<Barrier> = manager.get(barrier_id).unwrap();
        let initial_gen = barrier.get_generation();

        // First round
        let mut handles = vec![];
        for _ in 0..2 {
            let manager_clone = Arc::clone(&manager);
            let handle = thread::spawn(move || {
                syscalls::sys_barrier_wait(&manager_clone, barrier_id, 0).unwrap();
            });
            handles.push(handle);
        }
        for handle in handles {
            handle.join().unwrap();
        }

        // Generation should have increased
        assert_eq!(barrier.get_generation(), initial_gen + 1);

        // Second round
        let mut handles = vec![];
        for _ in 0..2 {
            let manager_clone = Arc::clone(&manager);
            let handle = thread::spawn(move || {
                syscalls::sys_barrier_wait(&manager_clone, barrier_id, 0).unwrap();
            });
            handles.push(handle);
        }
        for handle in handles {
            handle.join().unwrap();
        }

        // Generation should have increased again
        assert_eq!(barrier.get_generation(), initial_gen + 2);

        syscalls::sys_barrier_destroy(&manager, barrier_id).unwrap();
    }
}
