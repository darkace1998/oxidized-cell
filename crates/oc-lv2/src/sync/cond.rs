//! Condition variable (sys_cond_*)

use crate::objects::{KernelObject, ObjectId, ObjectManager, ObjectType};
use crate::sync::mutex::Mutex;
use oc_core::error::KernelError;
use parking_lot::{Condvar, Mutex as ParkingMutex};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Condition variable attributes
#[derive(Debug, Clone, Copy)]
pub struct CondAttributes {
    pub flags: u32,
}

impl Default for CondAttributes {
    fn default() -> Self {
        Self { flags: 0 }
    }
}

/// Wait result for condition variables
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CondWaitResult {
    /// Signaled normally
    Signaled,
    /// Timed out waiting
    TimedOut,
}

/// LV2 Condition Variable implementation
pub struct Cond {
    id: ObjectId,
    condvar: Condvar,
    /// Track waiting threads and signal count for spurious wakeup detection
    state: ParkingMutex<CondState>,
    #[allow(dead_code)]
    attributes: CondAttributes,
}

#[derive(Debug, Default)]
struct CondState {
    /// Set of threads currently waiting on this condition variable
    waiting_threads: HashSet<u64>,
    /// Signal counter (incremented on each signal)
    signal_count: u64,
    /// Broadcast counter (incremented on each signal_all)
    broadcast_count: u64,
}

impl Cond {
    pub fn new(id: ObjectId, attributes: CondAttributes) -> Self {
        Self {
            id,
            condvar: Condvar::new(),
            state: ParkingMutex::new(CondState::default()),
            attributes,
        }
    }

    /// Wait on condition variable with optional timeout
    /// Returns the wait result indicating if signaled or timed out
    /// Handles spurious wakeups internally by checking signal counters
    pub fn wait(
        &self,
        mutex: &Arc<Mutex>,
        thread_id: u64,
        timeout: Option<Duration>,
    ) -> Result<CondWaitResult, KernelError> {
        // Record state before waiting to detect spurious wakeups
        let (initial_signal, initial_broadcast) = {
            let mut state = self.state.lock();
            state.waiting_threads.insert(thread_id);
            (state.signal_count, state.broadcast_count)
        };

        // Unlock the mutex before waiting
        mutex.unlock(thread_id)?;

        // Wait on the condition variable with timeout handling
        let result = if let Some(duration) = timeout {
            let start = Instant::now();
            
            // Loop to handle spurious wakeups with timeout
            loop {
                // Calculate remaining time
                let elapsed = start.elapsed();
                if elapsed >= duration {
                    break CondWaitResult::TimedOut;
                }
                let remaining = duration - elapsed;

                // Wait with timeout on our internal state
                let mut guard = self.state.lock();
                let wait_result = self.condvar.wait_for(&mut guard, remaining);

                // Check if we were actually signaled
                let was_signaled = guard.signal_count > initial_signal 
                    || guard.broadcast_count > initial_broadcast;
                drop(guard);

                if was_signaled {
                    break CondWaitResult::Signaled;
                }

                // Check if we timed out
                if wait_result.timed_out() {
                    break CondWaitResult::TimedOut;
                }

                // Otherwise it was a spurious wakeup, continue waiting
            }
        } else {
            // No timeout - wait indefinitely but still check for spurious wakeups
            loop {
                let mut guard = self.state.lock();
                self.condvar.wait(&mut guard);

                // Check if we were actually signaled
                let was_signaled = guard.signal_count > initial_signal 
                    || guard.broadcast_count > initial_broadcast;
                drop(guard);

                if was_signaled {
                    break CondWaitResult::Signaled;
                }
                // Otherwise it was a spurious wakeup, continue waiting
            }
        };

        // Remove thread from waiting set
        {
            let mut state = self.state.lock();
            state.waiting_threads.remove(&thread_id);
        }

        // Re-lock the mutex before returning
        mutex.lock(thread_id)?;

        Ok(result)
    }

    /// Simple wait (returns WouldBlock on timeout, Ok on signal)
    pub fn wait_simple(
        &self,
        mutex: &Arc<Mutex>,
        thread_id: u64,
        timeout: Option<Duration>,
    ) -> Result<(), KernelError> {
        match self.wait(mutex, thread_id, timeout)? {
            CondWaitResult::Signaled => Ok(()),
            CondWaitResult::TimedOut => Err(KernelError::WouldBlock),
        }
    }

    /// Signal one waiting thread
    pub fn signal(&self) {
        {
            let mut state = self.state.lock();
            state.signal_count += 1;
        }
        self.condvar.notify_one();
    }

    /// Signal all waiting threads
    pub fn signal_all(&self) {
        {
            let mut state = self.state.lock();
            state.broadcast_count += 1;
        }
        self.condvar.notify_all();
    }

    /// Get number of waiting threads
    pub fn waiting_count(&self) -> usize {
        self.state.lock().waiting_threads.len()
    }

    /// Check if any threads are waiting
    pub fn has_waiters(&self) -> bool {
        !self.state.lock().waiting_threads.is_empty()
    }
}

impl KernelObject for Cond {
    fn object_type(&self) -> ObjectType {
        ObjectType::Cond
    }

    fn id(&self) -> ObjectId {
        self.id
    }

    fn as_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

/// Condition variable syscall implementations
pub mod syscalls {
    use super::*;

    /// sys_cond_create
    pub fn sys_cond_create(
        manager: &ObjectManager,
        attributes: CondAttributes,
    ) -> Result<ObjectId, KernelError> {
        let id = manager.next_id();
        let cond = Arc::new(Cond::new(id, attributes));
        manager.register(cond);
        Ok(id)
    }

    /// sys_cond_destroy
    pub fn sys_cond_destroy(
        manager: &ObjectManager,
        cond_id: ObjectId,
    ) -> Result<(), KernelError> {
        manager.unregister(cond_id)
    }

    /// sys_cond_wait
    pub fn sys_cond_wait(
        manager: &ObjectManager,
        cond_id: ObjectId,
        mutex_id: ObjectId,
        thread_id: u64,
        timeout_usec: u64,
    ) -> Result<(), KernelError> {
        let cond: Arc<Cond> = manager.get(cond_id)?;
        let mutex: Arc<Mutex> = manager.get(mutex_id)?;

        let timeout = if timeout_usec == 0 {
            None
        } else {
            Some(Duration::from_micros(timeout_usec))
        };

        cond.wait_simple(&mutex, thread_id, timeout)
    }

    /// sys_cond_wait_ex - Wait with detailed result
    pub fn sys_cond_wait_ex(
        manager: &ObjectManager,
        cond_id: ObjectId,
        mutex_id: ObjectId,
        thread_id: u64,
        timeout_usec: u64,
    ) -> Result<CondWaitResult, KernelError> {
        let cond: Arc<Cond> = manager.get(cond_id)?;
        let mutex: Arc<Mutex> = manager.get(mutex_id)?;

        let timeout = if timeout_usec == 0 {
            None
        } else {
            Some(Duration::from_micros(timeout_usec))
        };

        cond.wait(&mutex, thread_id, timeout)
    }

    /// sys_cond_signal
    pub fn sys_cond_signal(
        manager: &ObjectManager,
        cond_id: ObjectId,
    ) -> Result<(), KernelError> {
        let cond: Arc<Cond> = manager.get(cond_id)?;
        cond.signal();
        Ok(())
    }

    /// sys_cond_signal_all
    pub fn sys_cond_signal_all(
        manager: &ObjectManager,
        cond_id: ObjectId,
    ) -> Result<(), KernelError> {
        let cond: Arc<Cond> = manager.get(cond_id)?;
        cond.signal_all();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::mutex::{self, MutexAttributes};

    #[test]
    fn test_cond_create_destroy() {
        let manager = ObjectManager::new();
        let cond_id = syscalls::sys_cond_create(&manager, CondAttributes::default()).unwrap();

        assert!(manager.exists(cond_id));

        syscalls::sys_cond_destroy(&manager, cond_id).unwrap();
        assert!(!manager.exists(cond_id));
    }

    #[test]
    fn test_cond_signal() {
        let manager = ObjectManager::new();
        let cond_id = syscalls::sys_cond_create(&manager, CondAttributes::default()).unwrap();
        let mutex_id =
            mutex::syscalls::sys_mutex_create(&manager, MutexAttributes::default()).unwrap();

        // Signal should not fail
        syscalls::sys_cond_signal(&manager, cond_id).unwrap();
        syscalls::sys_cond_signal_all(&manager, cond_id).unwrap();

        syscalls::sys_cond_destroy(&manager, cond_id).unwrap();
        mutex::syscalls::sys_mutex_destroy(&manager, mutex_id).unwrap();
    }
}

