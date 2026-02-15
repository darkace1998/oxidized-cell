//! Thread management (sys_ppu_thread_*)

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

/// Stack constants
const STACK_BASE: u64 = 0xD000_0000;
const STACK_OFFSET: u64 = 0x10000;

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
    _stack_addr: u64,
    _stack_size: usize,
    _entry_point: u64,
    _attributes: ThreadAttributes,
    /// CPU affinity mask (bit N = can run on CPU N)
    affinity_mask: u64,
    /// Thread-local storage pointer
    tls_pointer: u64,
    /// Thread-local storage data
    tls_data: HashMap<u32, u64>,
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
                _stack_addr: stack_addr,
                _stack_size: attributes.stack_size,
                _entry_point: entry_point,
                _attributes: attributes,
                affinity_mask: 0xFF, // All CPUs by default (8 cores max)
                tls_pointer: 0,
                tls_data: HashMap::new(),
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

    /// Get thread affinity mask
    pub fn get_affinity(&self) -> u64 {
        self.inner.lock().affinity_mask
    }

    /// Set thread affinity mask
    pub fn set_affinity(&self, mask: u64) -> Result<(), KernelError> {
        if mask == 0 {
            return Err(KernelError::ResourceLimit);
        }
        self.inner.lock().affinity_mask = mask;
        tracing::debug!("Thread {} affinity set to 0x{:x}", self.id, mask);
        Ok(())
    }

    /// Get TLS pointer
    pub fn get_tls_pointer(&self) -> u64 {
        self.inner.lock().tls_pointer
    }

    /// Set TLS pointer
    pub fn set_tls_pointer(&self, pointer: u64) {
        self.inner.lock().tls_pointer = pointer;
    }

    /// Get TLS value by key
    pub fn get_tls_value(&self, key: u32) -> Option<u64> {
        self.inner.lock().tls_data.get(&key).copied()
    }

    /// Set TLS value by key
    pub fn set_tls_value(&self, key: u32, value: u64) {
        self.inner.lock().tls_data.insert(key, value);
    }

    /// Delete TLS value by key
    pub fn delete_tls_value(&self, key: u32) -> bool {
        self.inner.lock().tls_data.remove(&key).is_some()
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
    /// Dedicated thread ID counter that always increases (never reuses IDs)
    next_id: AtomicU64,
    /// Stack address counter for unique stack allocation (never reuses addresses)
    next_stack_index: AtomicU64,
    threads: Mutex<HashMap<ThreadId, Arc<Thread>>>,
    current_thread: Mutex<Option<ThreadId>>,
}

impl ThreadManager {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1), // Start from 1 (0 is invalid)
            next_stack_index: AtomicU64::new(0), // Stack index counter
            threads: Mutex::new(HashMap::new()),
            current_thread: Mutex::new(Some(1)), // Main thread is ID 1
        }
    }

    /// Get next thread ID (monotonically increasing, never reuses IDs)
    pub fn next_id(&self) -> ThreadId {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Get next stack index (monotonically increasing for unique stack addresses)
    fn next_stack_index(&self) -> u64 {
        self.next_stack_index.fetch_add(1, Ordering::Relaxed)
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
        _arg: u64,
        priority: u32,
        stack_size: usize,
        _flags: u64,
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

        // Allocate stack using dedicated counter (ensures unique addresses even after thread removal)
        let stack_index = manager.next_stack_index();
        let stack_addr = STACK_BASE + (stack_index * STACK_OFFSET);

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
        let _thread = manager.get(thread_id)?;
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

    /// sys_ppu_thread_get_affinity_mask
    pub fn sys_ppu_thread_get_affinity_mask(
        manager: &ThreadManager,
        thread_id: ThreadId,
    ) -> Result<u64, KernelError> {
        let thread = manager.get(thread_id)?;
        Ok(thread.get_affinity())
    }

    /// sys_ppu_thread_set_affinity_mask
    pub fn sys_ppu_thread_set_affinity_mask(
        manager: &ThreadManager,
        thread_id: ThreadId,
        affinity_mask: u64,
    ) -> Result<(), KernelError> {
        let thread = manager.get(thread_id)?;
        thread.set_affinity(affinity_mask)
    }

    /// sys_ppu_thread_get_tls
    pub fn sys_ppu_thread_get_tls(
        manager: &ThreadManager,
        thread_id: ThreadId,
    ) -> Result<u64, KernelError> {
        let thread = manager.get(thread_id)?;
        Ok(thread.get_tls_pointer())
    }

    /// sys_ppu_thread_set_tls
    pub fn sys_ppu_thread_set_tls(
        manager: &ThreadManager,
        thread_id: ThreadId,
        tls_pointer: u64,
    ) -> Result<(), KernelError> {
        let thread = manager.get(thread_id)?;
        thread.set_tls_pointer(tls_pointer);
        Ok(())
    }

    /// sys_ppu_thread_get_tls_value
    pub fn sys_ppu_thread_get_tls_value(
        manager: &ThreadManager,
        thread_id: ThreadId,
        key: u32,
    ) -> Result<u64, KernelError> {
        let thread = manager.get(thread_id)?;
        thread
            .get_tls_value(key)
            .ok_or(KernelError::InvalidId(key))
    }

    /// sys_ppu_thread_set_tls_value
    pub fn sys_ppu_thread_set_tls_value(
        manager: &ThreadManager,
        thread_id: ThreadId,
        key: u32,
        value: u64,
    ) -> Result<(), KernelError> {
        let thread = manager.get(thread_id)?;
        thread.set_tls_value(key, value);
        Ok(())
    }

    /// sys_ppu_thread_get_tls_addr
    /// Calculate thread-local storage address for a given TLS module offset.
    /// TLS base is at 0x28000000, each thread gets a 64KB page.
    pub fn sys_ppu_thread_get_tls_addr(
        manager: &ThreadManager,
        thread_id: ThreadId,
        tls_offset: u64,
    ) -> Result<u64, KernelError> {
        const TLS_BASE: u64 = 0x28000000;
        const TLS_PAGE_SIZE: u64 = 0x10000; // 64KB per thread

        // Validate the thread exists
        let _thread = manager.get(thread_id)?;
        // Mask to 8 bits — supports up to 256 concurrent threads, matching the
        // practical limit of the Cell BE (limited SPU + PPU hardware threads).
        let thread_index = (thread_id as u64) & 0xFF;
        let addr = TLS_BASE + thread_index * TLS_PAGE_SIZE + tls_offset;
        Ok(addr)
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

    #[test]
    fn test_thread_affinity() {
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

        // Default affinity should be 0xFF (all CPUs)
        let affinity = syscalls::sys_ppu_thread_get_affinity_mask(&manager, thread_id).unwrap();
        assert_eq!(affinity, 0xFF);

        // Set affinity to CPU 0 and 1 only
        syscalls::sys_ppu_thread_set_affinity_mask(&manager, thread_id, 0x03).unwrap();
        let affinity = syscalls::sys_ppu_thread_get_affinity_mask(&manager, thread_id).unwrap();
        assert_eq!(affinity, 0x03);

        // Zero affinity should fail
        let result = syscalls::sys_ppu_thread_set_affinity_mask(&manager, thread_id, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_thread_tls() {
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

        // Default TLS pointer should be 0
        let tls = syscalls::sys_ppu_thread_get_tls(&manager, thread_id).unwrap();
        assert_eq!(tls, 0);

        // Set TLS pointer
        syscalls::sys_ppu_thread_set_tls(&manager, thread_id, 0x12345678).unwrap();
        let tls = syscalls::sys_ppu_thread_get_tls(&manager, thread_id).unwrap();
        assert_eq!(tls, 0x12345678);
    }

    #[test]
    fn test_thread_tls_values() {
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

        // Getting non-existent key should fail
        let result = syscalls::sys_ppu_thread_get_tls_value(&manager, thread_id, 1);
        assert!(result.is_err());

        // Set TLS values
        syscalls::sys_ppu_thread_set_tls_value(&manager, thread_id, 1, 0xAAAA).unwrap();
        syscalls::sys_ppu_thread_set_tls_value(&manager, thread_id, 2, 0xBBBB).unwrap();

        // Get TLS values
        let val1 = syscalls::sys_ppu_thread_get_tls_value(&manager, thread_id, 1).unwrap();
        let val2 = syscalls::sys_ppu_thread_get_tls_value(&manager, thread_id, 2).unwrap();
        assert_eq!(val1, 0xAAAA);
        assert_eq!(val2, 0xBBBB);
    }

    #[test]
    fn test_tls_addr_calculation() {
        let manager = ThreadManager::new();
        let thread_id = syscalls::sys_ppu_thread_create(
            &manager, 0x1000, 0, 1000, 0x4000, 0, "test_tls_addr",
        ).unwrap();

        // Get TLS address with offset 0
        let addr0 = syscalls::sys_ppu_thread_get_tls_addr(&manager, thread_id, 0).unwrap();
        assert!(addr0 >= 0x28000000, "TLS addr should be at or above TLS_BASE");

        // Get TLS address with offset 0x100
        let addr1 = syscalls::sys_ppu_thread_get_tls_addr(&manager, thread_id, 0x100).unwrap();
        assert_eq!(addr1, addr0 + 0x100, "TLS addr with offset should be base + offset");
    }
}

