//! Thread scheduler for PPU and SPU threads
//!
//! This module implements a basic priority-based scheduler with time slicing
//! for managing PPU and SPU thread execution.

use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;

/// Thread identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThreadId {
    /// PPU thread
    Ppu(u32),
    /// SPU thread
    Spu(u32),
}

/// Thread state in scheduler
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    /// Thread is ready to run
    Ready,
    /// Thread is currently running
    Running,
    /// Thread is waiting (blocked)
    Waiting,
    /// Thread is stopped
    Stopped,
}

/// Scheduled thread information
#[derive(Debug, Clone)]
struct ScheduledThread {
    /// Thread identifier
    id: ThreadId,
    /// Thread priority (lower value = higher priority)
    priority: u32,
    /// Thread state
    state: ThreadState,
    /// Remaining time slice in microseconds
    time_slice_us: u64,
    /// Total execution time in microseconds
    total_time_us: u64,
}

impl PartialEq for ScheduledThread {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.id == other.id
    }
}

impl Eq for ScheduledThread {}

impl PartialOrd for ScheduledThread {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledThread {
    fn cmp(&self, other: &Self) -> Ordering {
        // Lower priority value means higher priority
        // Reverse ordering for max heap to work as min heap
        other.priority.cmp(&self.priority)
            .then_with(|| self.id.cmp(&other.id))
    }
}

impl PartialOrd for ThreadId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ThreadId {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (ThreadId::Ppu(a), ThreadId::Ppu(b)) => a.cmp(b),
            (ThreadId::Spu(a), ThreadId::Spu(b)) => a.cmp(b),
            (ThreadId::Ppu(_), ThreadId::Spu(_)) => Ordering::Less,
            (ThreadId::Spu(_), ThreadId::Ppu(_)) => Ordering::Greater,
        }
    }
}

/// Thread scheduler
pub struct Scheduler {
    /// Ready queue (priority queue)
    ready_queue: BinaryHeap<ScheduledThread>,
    /// All threads by ID
    threads: HashMap<ThreadId, ScheduledThread>,
    /// Currently running thread
    current: Option<ThreadId>,
    /// Default time slice in microseconds (1ms)
    default_time_slice_us: u64,
}

impl Scheduler {
    /// Create a new scheduler
    pub fn new() -> Self {
        Self {
            ready_queue: BinaryHeap::new(),
            threads: HashMap::new(),
            current: None,
            default_time_slice_us: 1000, // 1ms
        }
    }

    /// Add a thread to the scheduler
    pub fn add_thread(&mut self, id: ThreadId, priority: u32) {
        let thread = ScheduledThread {
            id,
            priority,
            state: ThreadState::Ready,
            time_slice_us: self.default_time_slice_us,
            total_time_us: 0,
        };

        self.threads.insert(id, thread.clone());
        self.ready_queue.push(thread);

        tracing::debug!("Added thread {:?} with priority {}", id, priority);
    }

    /// Remove a thread from the scheduler
    pub fn remove_thread(&mut self, id: ThreadId) {
        if let Some(thread) = self.threads.remove(&id) {
            // If it's the current thread, clear current
            if self.current == Some(id) {
                self.current = None;
            }
            
            tracing::debug!("Removed thread {:?}", id);
        }
    }

    /// Set thread state
    pub fn set_thread_state(&mut self, id: ThreadId, state: ThreadState) {
        if let Some(thread) = self.threads.get_mut(&id) {
            thread.state = state;

            // If transitioning to ready, add to ready queue
            if state == ThreadState::Ready && thread.state != ThreadState::Ready {
                self.ready_queue.push(thread.clone());
            }

            tracing::trace!("Thread {:?} state changed to {:?}", id, state);
        }
    }

    /// Get thread state
    pub fn get_thread_state(&self, id: ThreadId) -> Option<ThreadState> {
        self.threads.get(&id).map(|t| t.state)
    }

    /// Set thread priority
    pub fn set_thread_priority(&mut self, id: ThreadId, priority: u32) {
        if let Some(thread) = self.threads.get_mut(&id) {
            thread.priority = priority;
            
            // If the thread is ready, we need to re-insert it into the ready queue
            // For simplicity, we rebuild the ready queue
            if thread.state == ThreadState::Ready {
                self.rebuild_ready_queue();
            }

            tracing::debug!("Thread {:?} priority changed to {}", id, priority);
        }
    }

    /// Get thread priority
    pub fn get_thread_priority(&self, id: ThreadId) -> Option<u32> {
        self.threads.get(&id).map(|t| t.priority)
    }

    /// Schedule next thread to run
    pub fn schedule(&mut self) -> Option<ThreadId> {
        // If there's a current thread, save its state
        if let Some(current_id) = self.current {
            if let Some(thread) = self.threads.get_mut(&current_id) {
                if thread.state == ThreadState::Running {
                    // If time slice expired or yielded, put back in ready queue
                    thread.state = ThreadState::Ready;
                    thread.time_slice_us = self.default_time_slice_us;
                    self.ready_queue.push(thread.clone());
                }
            }
        }

        // Get next ready thread
        while let Some(mut thread) = self.ready_queue.pop() {
            // Check if thread still exists and is ready
            if let Some(stored_thread) = self.threads.get_mut(&thread.id) {
                if stored_thread.state == ThreadState::Ready {
                    stored_thread.state = ThreadState::Running;
                    self.current = Some(thread.id);
                    
                    tracing::trace!("Scheduled thread {:?}", thread.id);
                    return Some(thread.id);
                }
            }
        }

        // No ready threads
        self.current = None;
        None
    }

    /// Get currently running thread
    pub fn current_thread(&self) -> Option<ThreadId> {
        self.current
    }

    /// Update time slice for current thread
    pub fn update_time_slice(&mut self, elapsed_us: u64) {
        if let Some(current_id) = self.current {
            if let Some(thread) = self.threads.get_mut(&current_id) {
                thread.total_time_us += elapsed_us;
                
                if thread.time_slice_us > elapsed_us {
                    thread.time_slice_us -= elapsed_us;
                } else {
                    // Time slice expired, trigger rescheduling
                    thread.time_slice_us = 0;
                }
            }
        }
    }

    /// Check if current thread's time slice has expired
    pub fn time_slice_expired(&self) -> bool {
        if let Some(current_id) = self.current {
            if let Some(thread) = self.threads.get(&current_id) {
                return thread.time_slice_us == 0;
            }
        }
        false
    }

    /// Yield current thread (voluntary context switch)
    pub fn yield_current(&mut self) {
        if let Some(current_id) = self.current {
            if let Some(thread) = self.threads.get_mut(&current_id) {
                if thread.state == ThreadState::Running {
                    thread.state = ThreadState::Ready;
                    thread.time_slice_us = self.default_time_slice_us;
                    self.ready_queue.push(thread.clone());
                }
            }
            self.current = None;
        }
    }

    /// Get total number of threads
    pub fn thread_count(&self) -> usize {
        self.threads.len()
    }

    /// Get number of ready threads
    pub fn ready_count(&self) -> usize {
        self.threads.values()
            .filter(|t| t.state == ThreadState::Ready)
            .count()
    }

    /// Get statistics for a thread
    pub fn get_thread_stats(&self, id: ThreadId) -> Option<ThreadStats> {
        self.threads.get(&id).map(|thread| ThreadStats {
            id: thread.id,
            priority: thread.priority,
            state: thread.state,
            total_time_us: thread.total_time_us,
        })
    }

    /// Rebuild the ready queue from scratch
    fn rebuild_ready_queue(&mut self) {
        self.ready_queue.clear();
        for thread in self.threads.values() {
            if thread.state == ThreadState::Ready {
                self.ready_queue.push(thread.clone());
            }
        }
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread statistics
#[derive(Debug, Clone, Copy)]
pub struct ThreadStats {
    /// Thread identifier
    pub id: ThreadId,
    /// Thread priority
    pub priority: u32,
    /// Thread state
    pub state: ThreadState,
    /// Total execution time in microseconds
    pub total_time_us: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_creation() {
        let scheduler = Scheduler::new();
        assert_eq!(scheduler.thread_count(), 0);
        assert_eq!(scheduler.ready_count(), 0);
        assert_eq!(scheduler.current_thread(), None);
    }

    #[test]
    fn test_add_thread() {
        let mut scheduler = Scheduler::new();
        
        scheduler.add_thread(ThreadId::Ppu(1), 100);
        assert_eq!(scheduler.thread_count(), 1);
        assert_eq!(scheduler.ready_count(), 1);
    }

    #[test]
    fn test_schedule_single_thread() {
        let mut scheduler = Scheduler::new();
        
        scheduler.add_thread(ThreadId::Ppu(1), 100);
        let thread = scheduler.schedule();
        
        assert_eq!(thread, Some(ThreadId::Ppu(1)));
        assert_eq!(scheduler.current_thread(), Some(ThreadId::Ppu(1)));
    }

    #[test]
    fn test_schedule_priority() {
        let mut scheduler = Scheduler::new();
        
        scheduler.add_thread(ThreadId::Ppu(1), 200); // Lower priority
        scheduler.add_thread(ThreadId::Ppu(2), 100); // Higher priority
        
        let thread = scheduler.schedule();
        assert_eq!(thread, Some(ThreadId::Ppu(2))); // Higher priority scheduled first
    }

    #[test]
    fn test_thread_state_transitions() {
        let mut scheduler = Scheduler::new();
        
        scheduler.add_thread(ThreadId::Ppu(1), 100);
        assert_eq!(scheduler.get_thread_state(ThreadId::Ppu(1)), Some(ThreadState::Ready));
        
        scheduler.schedule();
        assert_eq!(scheduler.get_thread_state(ThreadId::Ppu(1)), Some(ThreadState::Running));
        
        scheduler.set_thread_state(ThreadId::Ppu(1), ThreadState::Waiting);
        assert_eq!(scheduler.get_thread_state(ThreadId::Ppu(1)), Some(ThreadState::Waiting));
    }

    #[test]
    fn test_yield() {
        let mut scheduler = Scheduler::new();
        
        scheduler.add_thread(ThreadId::Ppu(1), 100);
        scheduler.schedule();
        
        assert_eq!(scheduler.current_thread(), Some(ThreadId::Ppu(1)));
        
        scheduler.yield_current();
        assert_eq!(scheduler.current_thread(), None);
        assert_eq!(scheduler.get_thread_state(ThreadId::Ppu(1)), Some(ThreadState::Ready));
    }

    #[test]
    fn test_time_slice() {
        let mut scheduler = Scheduler::new();
        
        scheduler.add_thread(ThreadId::Ppu(1), 100);
        scheduler.schedule();
        
        scheduler.update_time_slice(500);
        assert!(!scheduler.time_slice_expired());
        
        scheduler.update_time_slice(600);
        assert!(scheduler.time_slice_expired());
    }

    #[test]
    fn test_priority_change() {
        let mut scheduler = Scheduler::new();
        
        scheduler.add_thread(ThreadId::Ppu(1), 200);
        assert_eq!(scheduler.get_thread_priority(ThreadId::Ppu(1)), Some(200));
        
        scheduler.set_thread_priority(ThreadId::Ppu(1), 100);
        assert_eq!(scheduler.get_thread_priority(ThreadId::Ppu(1)), Some(100));
    }

    #[test]
    fn test_remove_thread() {
        let mut scheduler = Scheduler::new();
        
        scheduler.add_thread(ThreadId::Ppu(1), 100);
        assert_eq!(scheduler.thread_count(), 1);
        
        scheduler.remove_thread(ThreadId::Ppu(1));
        assert_eq!(scheduler.thread_count(), 0);
    }

    #[test]
    fn test_thread_stats() {
        let mut scheduler = Scheduler::new();
        
        scheduler.add_thread(ThreadId::Ppu(1), 100);
        scheduler.schedule();
        scheduler.update_time_slice(250);
        
        let stats = scheduler.get_thread_stats(ThreadId::Ppu(1)).unwrap();
        assert_eq!(stats.id, ThreadId::Ppu(1));
        assert_eq!(stats.priority, 100);
        assert_eq!(stats.total_time_us, 250);
    }

    #[test]
    fn test_mixed_thread_types() {
        let mut scheduler = Scheduler::new();
        
        scheduler.add_thread(ThreadId::Ppu(1), 100);
        scheduler.add_thread(ThreadId::Spu(0), 50); // Higher priority
        scheduler.add_thread(ThreadId::Ppu(2), 150);
        
        // SPU thread should be scheduled first (highest priority)
        let thread1 = scheduler.schedule();
        assert_eq!(thread1, Some(ThreadId::Spu(0)));
        
        // Mark it as done (back to ready for this test)
        scheduler.set_thread_state(ThreadId::Spu(0), ThreadState::Stopped);
        
        // Then PPU thread 1
        let thread2 = scheduler.schedule();
        assert_eq!(thread2, Some(ThreadId::Ppu(1)));
        
        // Mark it as done
        scheduler.set_thread_state(ThreadId::Ppu(1), ThreadState::Stopped);
        
        // Then PPU thread 2
        let thread3 = scheduler.schedule();
        assert_eq!(thread3, Some(ThreadId::Ppu(2)));
    }
}
