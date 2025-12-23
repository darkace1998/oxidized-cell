//! Thread management (sys_ppu_thread_*)

use crate::objects::{ObjectId, ObjectManager};
use oc_core::error::KernelError;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Thread ID type
pub type ThreadId = u64;

/// Thread priority range (0-3071)
pub const PRIORITY_MIN: u32 = 0;
pub const PRIORITY_MAX: u32 = 3071;

/// Thread attributes
#[derive(Debug, Clone)]
pub struct ThreadAttributes {
    pub priority: u32,
    pub stack_size: usize,
    pub name: String,
}

impl Default for ThreadAttributes {
    fn default() -> Self {
        Self {
            priority: 1000,
            stack_size: 0x4000, // 16KB default stack
            name: String::from("PPU_Thread"),
        }
    }
}

/// Thread state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    Ready,
    Running,
    Waiting,
    Suspended,
    Terminated,
}

/// PPU Thread
pub struct Thread {
    id: ThreadId,
    inner: Mutex<ThreadInner>,
}

#[derive(Debug)]
struct ThreadInner {
    state: ThreadState,
    priority: u32,
    stack_addr: u64,
    stack_size: usize,
    entry_point: u64,
    attributes: ThreadAttributes,
}

impl Thread {
    pub fn new(
        id: ThreadId,
        entry_point: u64,
        stack_addr: u64,
        attributes: ThreadAttributes,
    ) -> Self {
        Self {
            id,
            inner: Mutex::new(ThreadInner {
                state: ThreadState::Ready,
                priority: attributes.priority,
                stack_addr,
                stack_size: attributes.stack_size,
                entry_point,
                attributes,
            }),
        }
    }

    pub fn id(&self) -> ThreadId {
        self.id
    }

    pub fn state(&self) -> ThreadState {
        self.inner.lock().state
    }

    pub fn set_state(&self, state: ThreadState) {
        self.inner.lock().state = state;
    }

    pub fn priority(&self) -> u32 {
        self.inner.lock().priority
    }

    pub fn set_priority(&self, priority: u32) -> Result<(), KernelError> {
        if priority > PRIORITY_MAX {
            return Err(KernelError::ResourceLimit);
        }
        self.inner.lock().priority = priority;
        Ok(())
    }

    pub fn join(&self) -> Result<(), KernelError> {
        let state = self.state();
        if state == ThreadState::Terminated {
            Ok(())
        } else {
            // In real implementation, would wait for thread to terminate
            tracing::debug!("Joining thread {}", self.id);
            self.set_state(ThreadState::Terminated);
            Ok(())
        }
    }
}

/// Thread manager for scheduling
pub struct ThreadManager {
    next_id: AtomicU64,
    threads: Mutex<HashMap<ThreadId, Arc<Thread>>>,
    current_thread: Mutex<Option<ThreadId>>,
}

impl ThreadManager {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1), // Start from 1 (0 is invalid)
            threads: Mutex::new(HashMap::new()),
            current_thread: Mutex::new(Some(1)), // Main thread is ID 1
        }
    }

    /// Get next thread ID
    pub fn next_id(&self) -> ThreadId {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Create a new thread
    pub fn create(
        &self,
        entry_point: u64,
        stack_addr: u64,
        attributes: ThreadAttributes,
    ) -> Result<ThreadId, KernelError> {
        let id = self.next_id();
        let thread = Arc::new(Thread::new(id, entry_point, stack_addr, attributes));

        self.threads.lock().insert(id, thread);

        tracing::debug!("Created thread {}", id);
        Ok(id)
    }

    /// Get a thread by ID
    pub fn get(&self, id: ThreadId) -> Result<Arc<Thread>, KernelError> {
        self.threads
            .lock()
            .get(&id)
            .cloned()
            .ok_or(KernelError::InvalidId(id as u32))
    }

    /// Destroy a thread
    pub fn destroy(&self, id: ThreadId) -> Result<(), KernelError> {
        let mut threads = self.threads.lock();
        let thread = threads
            .get(&id)
            .ok_or(KernelError::InvalidId(id as u32))?;

        thread.set_state(ThreadState::Terminated);
        threads.remove(&id);

        tracing::debug!("Destroyed thread {}", id);
        Ok(())
    }

    /// Get current thread ID
    pub fn current(&self) -> ThreadId {
        self.current_thread.lock().unwrap_or(1)
    }

    /// Set current thread
    pub fn set_current(&self, id: ThreadId) {
        *self.current_thread.lock() = Some(id);
    }

    /// Yield current thread
    pub fn yield_thread(&self) -> Result<(), KernelError> {
        let current = self.current();
        tracing::trace!("Thread {} yielding", current);
        // In real implementation, would schedule next thread
        Ok(())
    }

    /// Simple round-robin scheduling
    pub fn schedule(&self) -> Option<ThreadId> {
        let threads = self.threads.lock();

        // Find the next ready thread with highest priority (lowest number)
        let ready_thread = threads
            .values()
            .filter(|t| t.state() == ThreadState::Ready)
            .min_by_key(|t| t.priority()) // Lower priority number = higher priority
            .map(|t| t.id());

        if let Some(id) = ready_thread {
            if let Some(thread) = threads.get(&id) {
                thread.set_state(ThreadState::Running);
                drop(threads);
                self.set_current(id);
            }
        }

        ready_thread
    }

    /// Get thread count
    pub fn count(&self) -> usize {
        self.threads.lock().len()
    }
}

impl Default for ThreadManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread syscall implementations
pub mod syscalls {
    use super::*;

    /// sys_ppu_thread_create
    pub fn sys_ppu_thread_create(
        manager: &ThreadManager,
        entry_point: u64,
        arg: u64,
        priority: u32,
        stack_size: usize,
        flags: u64,
        name: &str,
    ) -> Result<ThreadId, KernelError> {
        let mut attributes = ThreadAttributes::default();
        attributes.priority = priority.min(PRIORITY_MAX);
        attributes.stack_size = if stack_size == 0 {
            attributes.stack_size
        } else {
            stack_size
        };
        attributes.name = name.to_string();

        // Allocate stack (simplified - would use memory manager)
        let stack_addr = 0xD000_0000 + (manager.count() as u64 * 0x10000);

        manager.create(entry_point, stack_addr, attributes)
    }

    /// sys_ppu_thread_start
    pub fn sys_ppu_thread_start(
        manager: &ThreadManager,
        thread_id: ThreadId,
    ) -> Result<(), KernelError> {
        let thread = manager.get(thread_id)?;
        thread.set_state(ThreadState::Ready);
        tracing::debug!("Started thread {}", thread_id);
        Ok(())
    }

    /// sys_ppu_thread_join
    pub fn sys_ppu_thread_join(
        manager: &ThreadManager,
        thread_id: ThreadId,
    ) -> Result<u64, KernelError> {
        let thread = manager.get(thread_id)?;
        thread.join()?;
        Ok(0) // Exit status
    }

    /// sys_ppu_thread_detach
    pub fn sys_ppu_thread_detach(
        manager: &ThreadManager,
        thread_id: ThreadId,
    ) -> Result<(), KernelError> {
        let thread = manager.get(thread_id)?;
        tracing::debug!("Detached thread {}", thread_id);
        Ok(())
    }

    /// sys_ppu_thread_yield
    pub fn sys_ppu_thread_yield(manager: &ThreadManager) -> Result<(), KernelError> {
        manager.yield_thread()
    }

    /// sys_ppu_thread_get_id
    pub fn sys_ppu_thread_get_id(manager: &ThreadManager) -> ThreadId {
        manager.current()
    }

    /// sys_ppu_thread_set_priority
    pub fn sys_ppu_thread_set_priority(
        manager: &ThreadManager,
        thread_id: ThreadId,
        priority: u32,
    ) -> Result<(), KernelError> {
        let thread = manager.get(thread_id)?;
        thread.set_priority(priority)
    }

    /// sys_ppu_thread_get_priority
    pub fn sys_ppu_thread_get_priority(
        manager: &ThreadManager,
        thread_id: ThreadId,
    ) -> Result<u32, KernelError> {
        let thread = manager.get(thread_id)?;
        Ok(thread.priority())
    }

    /// sys_ppu_thread_exit
    pub fn sys_ppu_thread_exit(
        manager: &ThreadManager,
        exit_code: u64,
    ) -> Result<(), KernelError> {
        let thread_id = manager.current();
        let thread = manager.get(thread_id)?;
        thread.set_state(ThreadState::Terminated);
        tracing::info!("Thread {} exited with code {}", thread_id, exit_code);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_create() {
        let manager = ThreadManager::new();

        let thread_id = syscalls::sys_ppu_thread_create(
            &manager,
            0x1000,
            0,
            1000,
            0x4000,
            0,
            "TestThread",
        )
        .unwrap();

        assert!(thread_id > 0);
        assert_eq!(manager.count(), 1);

        let thread = manager.get(thread_id).unwrap();
        assert_eq!(thread.state(), ThreadState::Ready);
    }

    #[test]
    fn test_thread_lifecycle() {
        let manager = ThreadManager::new();

        let thread_id = syscalls::sys_ppu_thread_create(
            &manager,
            0x1000,
            0,
            1000,
            0x4000,
            0,
            "TestThread",
        )
        .unwrap();

        // Start thread
        syscalls::sys_ppu_thread_start(&manager, thread_id).unwrap();
        assert_eq!(manager.get(thread_id).unwrap().state(), ThreadState::Ready);

        // Join thread
        syscalls::sys_ppu_thread_join(&manager, thread_id).unwrap();
        assert_eq!(
            manager.get(thread_id).unwrap().state(),
            ThreadState::Terminated
        );
    }

    #[test]
    fn test_thread_priority() {
        let manager = ThreadManager::new();

        let thread_id = syscalls::sys_ppu_thread_create(
            &manager,
            0x1000,
            0,
            1000,
            0x4000,
            0,
            "TestThread",
        )
        .unwrap();

        // Get initial priority
        let priority = syscalls::sys_ppu_thread_get_priority(&manager, thread_id).unwrap();
        assert_eq!(priority, 1000);

        // Set new priority
        syscalls::sys_ppu_thread_set_priority(&manager, thread_id, 500).unwrap();
        let priority = syscalls::sys_ppu_thread_get_priority(&manager, thread_id).unwrap();
        assert_eq!(priority, 500);
    }

    #[test]
    fn test_thread_scheduling() {
        let manager = ThreadManager::new();

        // Create multiple threads with different priorities (lower number = higher priority)
        let t1 = syscalls::sys_ppu_thread_create(&manager, 0x1000, 0, 200, 0x4000, 0, "T1")
            .unwrap();
        let t2 = syscalls::sys_ppu_thread_create(&manager, 0x2000, 0, 100, 0x4000, 0, "T2")
            .unwrap();
        let t3 = syscalls::sys_ppu_thread_create(&manager, 0x3000, 0, 150, 0x4000, 0, "T3")
            .unwrap();

        // Start all threads
        syscalls::sys_ppu_thread_start(&manager, t1).unwrap();
        syscalls::sys_ppu_thread_start(&manager, t2).unwrap();
        syscalls::sys_ppu_thread_start(&manager, t3).unwrap();

        // Schedule - should pick highest priority (t2 with priority 100, lowest number)
        let scheduled = manager.schedule();
        assert_eq!(scheduled, Some(t2));
    }
}

