//! Read-write lock (sys_rwlock_*)

use crate::objects::{KernelObject, ObjectId, ObjectManager, ObjectType};
use oc_core::error::KernelError;
use parking_lot::Mutex as ParkingMutex;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// RwLock attributes
#[derive(Debug, Clone, Copy)]
pub struct RwLockAttributes {
    pub flags: u32,
}

impl Default for RwLockAttributes {
    fn default() -> Self {
        Self { flags: 0 }
    }
}

/// Result of a rwlock wait operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RwLockWaitResult {
    /// Lock acquired successfully
    Acquired,
    /// Timed out waiting for lock
    TimedOut,
}

/// LV2 RwLock implementation with proper thread tracking
pub struct RwLock {
    id: ObjectId,
    state: ParkingMutex<RwLockState>,
    /// Attributes for future use (protocol flags, etc.)
    #[allow(dead_code)]
    attributes: RwLockAttributes,
}

#[derive(Debug, Default)]
struct RwLockState {
    /// Set of threads currently holding read locks
    readers: HashSet<u64>,
    /// Thread ID of the current writer (None if no writer)
    writer: Option<u64>,
    /// Threads waiting for read lock
    waiting_readers: VecDeque<u64>,
    /// Threads waiting for write lock
    waiting_writers: VecDeque<u64>,
}

impl RwLock {
    pub fn new(id: ObjectId, attributes: RwLockAttributes) -> Self {
        Self {
            id,
            state: ParkingMutex::new(RwLockState::default()),
            attributes,
        }
    }

    /// Acquire read lock for the given thread
    pub fn rlock(&self, thread_id: u64) -> Result<(), KernelError> {
        let mut state = self.state.lock();

        // Check if this thread already has a read lock
        if state.readers.contains(&thread_id) {
            // Allow recursive read locks
            return Ok(());
        }

        // Check if this thread is the writer (upgrade not supported, would deadlock)
        if state.writer == Some(thread_id) {
            return Err(KernelError::WouldBlock);
        }

        // Wait until no writer holds the lock
        // Note: Using spin-wait with yield is intentional for HLE emulation.
        // In a real kernel, this would integrate with the thread scheduler.
        // The yield_now() allows cooperative scheduling in the emulator.
        while state.writer.is_some() {
            // Register as waiting
            if !state.waiting_readers.contains(&thread_id) {
                state.waiting_readers.push_back(thread_id);
            }
            drop(state);
            std::thread::yield_now();
            state = self.state.lock();
        }

        // Remove from waiting list if present
        state.waiting_readers.retain(|&id| id != thread_id);

        // Acquire the read lock
        state.readers.insert(thread_id);
        tracing::debug!("RwLock {}: thread {} acquired read lock", self.id, thread_id);
        Ok(())
    }

    /// Try to acquire read lock without blocking
    pub fn try_rlock(&self, thread_id: u64) -> Result<(), KernelError> {
        let mut state = self.state.lock();

        // Check if this thread already has a read lock
        if state.readers.contains(&thread_id) {
            return Ok(());
        }

        // Check if this thread is the writer
        if state.writer == Some(thread_id) {
            return Err(KernelError::WouldBlock);
        }

        // Check if a writer holds the lock
        if state.writer.is_some() {
            return Err(KernelError::WouldBlock);
        }

        // Acquire the read lock
        state.readers.insert(thread_id);
        tracing::debug!("RwLock {}: thread {} acquired read lock (try)", self.id, thread_id);
        Ok(())
    }

    /// Acquire read lock with timeout
    pub fn rlock_timeout(&self, thread_id: u64, timeout: Duration) -> Result<RwLockWaitResult, KernelError> {
        let start = Instant::now();

        loop {
            let mut state = self.state.lock();

            // Check if this thread already has a read lock
            if state.readers.contains(&thread_id) {
                return Ok(RwLockWaitResult::Acquired);
            }

            // Check if this thread is the writer
            if state.writer == Some(thread_id) {
                return Err(KernelError::WouldBlock);
            }

            // Check if we can acquire
            if state.writer.is_none() {
                state.readers.insert(thread_id);
                state.waiting_readers.retain(|&id| id != thread_id);
                tracing::debug!("RwLock {}: thread {} acquired read lock", self.id, thread_id);
                return Ok(RwLockWaitResult::Acquired);
            }

            // Check timeout
            if start.elapsed() >= timeout {
                state.waiting_readers.retain(|&id| id != thread_id);
                return Ok(RwLockWaitResult::TimedOut);
            }

            // Register as waiting
            if !state.waiting_readers.contains(&thread_id) {
                state.waiting_readers.push_back(thread_id);
            }

            drop(state);
            std::thread::yield_now();
        }
    }

    /// Acquire write lock for the given thread
    pub fn wlock(&self, thread_id: u64) -> Result<(), KernelError> {
        let mut state = self.state.lock();

        // Check if this thread already has write lock
        if state.writer == Some(thread_id) {
            // Write lock is not recursive
            return Err(KernelError::WouldBlock);
        }

        // Check if this thread has a read lock (downgrade not automatic)
        if state.readers.contains(&thread_id) {
            return Err(KernelError::WouldBlock);
        }

        // Wait until no readers and no writer
        // Note: Using spin-wait with yield is intentional for HLE emulation.
        // In a real kernel, this would integrate with the thread scheduler.
        // The yield_now() allows cooperative scheduling in the emulator.
        while !state.readers.is_empty() || state.writer.is_some() {
            // Register as waiting
            if !state.waiting_writers.contains(&thread_id) {
                state.waiting_writers.push_back(thread_id);
            }
            drop(state);
            std::thread::yield_now();
            state = self.state.lock();
        }

        // Remove from waiting list if present
        state.waiting_writers.retain(|&id| id != thread_id);

        // Acquire the write lock
        state.writer = Some(thread_id);
        tracing::debug!("RwLock {}: thread {} acquired write lock", self.id, thread_id);
        Ok(())
    }

    /// Try to acquire write lock without blocking
    pub fn try_wlock(&self, thread_id: u64) -> Result<(), KernelError> {
        let mut state = self.state.lock();

        // Check if this thread already has write lock
        if state.writer == Some(thread_id) {
            return Err(KernelError::WouldBlock);
        }

        // Check if this thread has a read lock
        if state.readers.contains(&thread_id) {
            return Err(KernelError::WouldBlock);
        }

        // Check if lock is available
        if !state.readers.is_empty() || state.writer.is_some() {
            return Err(KernelError::WouldBlock);
        }

        // Acquire the write lock
        state.writer = Some(thread_id);
        tracing::debug!("RwLock {}: thread {} acquired write lock (try)", self.id, thread_id);
        Ok(())
    }

    /// Acquire write lock with timeout
    pub fn wlock_timeout(&self, thread_id: u64, timeout: Duration) -> Result<RwLockWaitResult, KernelError> {
        let start = Instant::now();

        loop {
            let mut state = self.state.lock();

            // Check if this thread already has write lock
            if state.writer == Some(thread_id) {
                return Err(KernelError::WouldBlock);
            }

            // Check if this thread has a read lock
            if state.readers.contains(&thread_id) {
                return Err(KernelError::WouldBlock);
            }

            // Check if we can acquire
            if state.readers.is_empty() && state.writer.is_none() {
                state.writer = Some(thread_id);
                state.waiting_writers.retain(|&id| id != thread_id);
                tracing::debug!("RwLock {}: thread {} acquired write lock", self.id, thread_id);
                return Ok(RwLockWaitResult::Acquired);
            }

            // Check timeout
            if start.elapsed() >= timeout {
                state.waiting_writers.retain(|&id| id != thread_id);
                return Ok(RwLockWaitResult::TimedOut);
            }

            // Register as waiting
            if !state.waiting_writers.contains(&thread_id) {
                state.waiting_writers.push_back(thread_id);
            }

            drop(state);
            std::thread::yield_now();
        }
    }

    /// Release lock (works for both read and write locks)
    pub fn unlock(&self, thread_id: u64) -> Result<(), KernelError> {
        let mut state = self.state.lock();

        // Check if this thread has a write lock
        if state.writer == Some(thread_id) {
            state.writer = None;
            tracing::debug!("RwLock {}: thread {} released write lock", self.id, thread_id);
            return Ok(());
        }

        // Check if this thread has a read lock
        if state.readers.remove(&thread_id) {
            tracing::debug!("RwLock {}: thread {} released read lock", self.id, thread_id);
            return Ok(());
        }

        // Thread doesn't hold any lock
        Err(KernelError::PermissionDenied)
    }

    /// Get the number of active readers
    pub fn reader_count(&self) -> usize {
        self.state.lock().readers.len()
    }

    /// Get the number of waiting readers
    pub fn waiting_reader_count(&self) -> usize {
        self.state.lock().waiting_readers.len()
    }

    /// Get the number of waiting writers
    pub fn waiting_writer_count(&self) -> usize {
        self.state.lock().waiting_writers.len()
    }

    /// Check if a writer holds the lock
    pub fn is_write_locked(&self) -> bool {
        self.state.lock().writer.is_some()
    }

    /// Check if any readers hold the lock
    pub fn is_read_locked(&self) -> bool {
        !self.state.lock().readers.is_empty()
    }

    /// Check if a specific thread holds a read lock
    pub fn has_read_lock(&self, thread_id: u64) -> bool {
        self.state.lock().readers.contains(&thread_id)
    }

    /// Check if a specific thread holds the write lock
    pub fn has_write_lock(&self, thread_id: u64) -> bool {
        self.state.lock().writer == Some(thread_id)
    }
}

impl KernelObject for RwLock {
    fn object_type(&self) -> ObjectType {
        ObjectType::RwLock
    }

    fn id(&self) -> ObjectId {
        self.id
    }

    fn as_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

/// RwLock syscall implementations
pub mod syscalls {
    use super::*;

    /// sys_rwlock_create
    pub fn sys_rwlock_create(
        manager: &ObjectManager,
        attributes: RwLockAttributes,
    ) -> Result<ObjectId, KernelError> {
        let id = manager.next_id();
        let rwlock = Arc::new(RwLock::new(id, attributes));
        manager.register(rwlock);
        Ok(id)
    }

    /// sys_rwlock_destroy
    pub fn sys_rwlock_destroy(
        manager: &ObjectManager,
        rwlock_id: ObjectId,
    ) -> Result<(), KernelError> {
        manager.unregister(rwlock_id)
    }

    /// sys_rwlock_rlock
    pub fn sys_rwlock_rlock(
        manager: &ObjectManager,
        rwlock_id: ObjectId,
        thread_id: u64,
    ) -> Result<(), KernelError> {
        let rwlock: Arc<RwLock> = manager.get(rwlock_id)?;
        rwlock.rlock(thread_id)
    }

    /// sys_rwlock_try_rlock
    pub fn sys_rwlock_try_rlock(
        manager: &ObjectManager,
        rwlock_id: ObjectId,
        thread_id: u64,
    ) -> Result<(), KernelError> {
        let rwlock: Arc<RwLock> = manager.get(rwlock_id)?;
        rwlock.try_rlock(thread_id)
    }

    /// sys_rwlock_rlock_timeout
    pub fn sys_rwlock_rlock_timeout(
        manager: &ObjectManager,
        rwlock_id: ObjectId,
        thread_id: u64,
        timeout_usec: u64,
    ) -> Result<RwLockWaitResult, KernelError> {
        let rwlock: Arc<RwLock> = manager.get(rwlock_id)?;
        let timeout = Duration::from_micros(timeout_usec);
        rwlock.rlock_timeout(thread_id, timeout)
    }

    /// sys_rwlock_wlock
    pub fn sys_rwlock_wlock(
        manager: &ObjectManager,
        rwlock_id: ObjectId,
        thread_id: u64,
    ) -> Result<(), KernelError> {
        let rwlock: Arc<RwLock> = manager.get(rwlock_id)?;
        rwlock.wlock(thread_id)
    }

    /// sys_rwlock_try_wlock
    pub fn sys_rwlock_try_wlock(
        manager: &ObjectManager,
        rwlock_id: ObjectId,
        thread_id: u64,
    ) -> Result<(), KernelError> {
        let rwlock: Arc<RwLock> = manager.get(rwlock_id)?;
        rwlock.try_wlock(thread_id)
    }

    /// sys_rwlock_wlock_timeout
    pub fn sys_rwlock_wlock_timeout(
        manager: &ObjectManager,
        rwlock_id: ObjectId,
        thread_id: u64,
        timeout_usec: u64,
    ) -> Result<RwLockWaitResult, KernelError> {
        let rwlock: Arc<RwLock> = manager.get(rwlock_id)?;
        let timeout = Duration::from_micros(timeout_usec);
        rwlock.wlock_timeout(thread_id, timeout)
    }

    /// sys_rwlock_unlock
    pub fn sys_rwlock_unlock(
        manager: &ObjectManager,
        rwlock_id: ObjectId,
        thread_id: u64,
    ) -> Result<(), KernelError> {
        let rwlock: Arc<RwLock> = manager.get(rwlock_id)?;
        rwlock.unlock(thread_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rwlock_basic() {
        let manager = ObjectManager::new();
        let rwlock_id =
            syscalls::sys_rwlock_create(&manager, RwLockAttributes::default()).unwrap();

        // Multiple read locks should work
        syscalls::sys_rwlock_rlock(&manager, rwlock_id, 1).unwrap();
        syscalls::sys_rwlock_try_rlock(&manager, rwlock_id, 2).unwrap();

        let rwlock: Arc<RwLock> = manager.get(rwlock_id).unwrap();
        assert_eq!(rwlock.reader_count(), 2);

        // Unlock both readers
        syscalls::sys_rwlock_unlock(&manager, rwlock_id, 1).unwrap();
        syscalls::sys_rwlock_unlock(&manager, rwlock_id, 2).unwrap();

        assert_eq!(rwlock.reader_count(), 0);

        syscalls::sys_rwlock_destroy(&manager, rwlock_id).unwrap();
    }

    #[test]
    fn test_rwlock_write() {
        let manager = ObjectManager::new();
        let rwlock_id =
            syscalls::sys_rwlock_create(&manager, RwLockAttributes::default()).unwrap();

        // Write lock
        syscalls::sys_rwlock_wlock(&manager, rwlock_id, 1).unwrap();

        let rwlock: Arc<RwLock> = manager.get(rwlock_id).unwrap();
        assert!(rwlock.is_write_locked());
        assert!(rwlock.has_write_lock(1));

        // Try read should fail while write locked
        assert!(syscalls::sys_rwlock_try_rlock(&manager, rwlock_id, 2).is_err());

        // Try write should fail while write locked
        assert!(syscalls::sys_rwlock_try_wlock(&manager, rwlock_id, 2).is_err());

        syscalls::sys_rwlock_unlock(&manager, rwlock_id, 1).unwrap();
        assert!(!rwlock.is_write_locked());

        syscalls::sys_rwlock_destroy(&manager, rwlock_id).unwrap();
    }

    #[test]
    fn test_rwlock_read_blocks_write() {
        let manager = ObjectManager::new();
        let rwlock_id =
            syscalls::sys_rwlock_create(&manager, RwLockAttributes::default()).unwrap();

        // Acquire read lock
        syscalls::sys_rwlock_rlock(&manager, rwlock_id, 1).unwrap();

        // Try write should fail while read locked
        assert!(syscalls::sys_rwlock_try_wlock(&manager, rwlock_id, 2).is_err());

        // Release read and acquire write
        syscalls::sys_rwlock_unlock(&manager, rwlock_id, 1).unwrap();
        syscalls::sys_rwlock_wlock(&manager, rwlock_id, 2).unwrap();

        let rwlock: Arc<RwLock> = manager.get(rwlock_id).unwrap();
        assert!(rwlock.has_write_lock(2));

        syscalls::sys_rwlock_unlock(&manager, rwlock_id, 2).unwrap();
        syscalls::sys_rwlock_destroy(&manager, rwlock_id).unwrap();
    }

    #[test]
    fn test_rwlock_unlock_wrong_thread() {
        let manager = ObjectManager::new();
        let rwlock_id =
            syscalls::sys_rwlock_create(&manager, RwLockAttributes::default()).unwrap();

        // Acquire read lock
        syscalls::sys_rwlock_rlock(&manager, rwlock_id, 1).unwrap();

        // Wrong thread trying to unlock should fail
        assert!(syscalls::sys_rwlock_unlock(&manager, rwlock_id, 2).is_err());

        // Correct thread unlocks
        syscalls::sys_rwlock_unlock(&manager, rwlock_id, 1).unwrap();

        syscalls::sys_rwlock_destroy(&manager, rwlock_id).unwrap();
    }

    #[test]
    fn test_rwlock_recursive_read() {
        let manager = ObjectManager::new();
        let rwlock_id =
            syscalls::sys_rwlock_create(&manager, RwLockAttributes::default()).unwrap();

        // Same thread can acquire read lock multiple times (but only tracked once)
        syscalls::sys_rwlock_rlock(&manager, rwlock_id, 1).unwrap();
        syscalls::sys_rwlock_rlock(&manager, rwlock_id, 1).unwrap();

        let rwlock: Arc<RwLock> = manager.get(rwlock_id).unwrap();
        assert_eq!(rwlock.reader_count(), 1); // Only tracked once

        syscalls::sys_rwlock_unlock(&manager, rwlock_id, 1).unwrap();

        syscalls::sys_rwlock_destroy(&manager, rwlock_id).unwrap();
    }

    #[test]
    fn test_rwlock_timeout() {
        let manager = ObjectManager::new();
        let rwlock_id =
            syscalls::sys_rwlock_create(&manager, RwLockAttributes::default()).unwrap();

        // Acquire write lock
        syscalls::sys_rwlock_wlock(&manager, rwlock_id, 1).unwrap();

        // Try read with short timeout should time out
        let result = syscalls::sys_rwlock_rlock_timeout(
            &manager,
            rwlock_id,
            2,
            1000, // 1ms timeout
        );
        assert_eq!(result.unwrap(), RwLockWaitResult::TimedOut);

        // Try write with short timeout should time out
        let result = syscalls::sys_rwlock_wlock_timeout(
            &manager,
            rwlock_id,
            2,
            1000, // 1ms timeout
        );
        assert_eq!(result.unwrap(), RwLockWaitResult::TimedOut);

        syscalls::sys_rwlock_unlock(&manager, rwlock_id, 1).unwrap();
        syscalls::sys_rwlock_destroy(&manager, rwlock_id).unwrap();
    }

    #[test]
    fn test_rwlock_state_queries() {
        let manager = ObjectManager::new();
        let rwlock_id =
            syscalls::sys_rwlock_create(&manager, RwLockAttributes::default()).unwrap();

        let rwlock: Arc<RwLock> = manager.get(rwlock_id).unwrap();

        // Initially unlocked
        assert!(!rwlock.is_read_locked());
        assert!(!rwlock.is_write_locked());

        // After read lock
        syscalls::sys_rwlock_rlock(&manager, rwlock_id, 1).unwrap();
        assert!(rwlock.is_read_locked());
        assert!(rwlock.has_read_lock(1));
        assert!(!rwlock.has_read_lock(2));

        syscalls::sys_rwlock_unlock(&manager, rwlock_id, 1).unwrap();

        // After write lock
        syscalls::sys_rwlock_wlock(&manager, rwlock_id, 2).unwrap();
        assert!(rwlock.is_write_locked());
        assert!(rwlock.has_write_lock(2));
        assert!(!rwlock.has_write_lock(1));

        syscalls::sys_rwlock_unlock(&manager, rwlock_id, 2).unwrap();
        syscalls::sys_rwlock_destroy(&manager, rwlock_id).unwrap();
    }
}