//! Event flag (sys_event_flag_*)
//!
//! Event flags are synchronization primitives that allow threads to wait
//! for specific bit patterns to be set or cleared.

use crate::objects::{KernelObject, ObjectId, ObjectManager, ObjectType};
use oc_core::error::KernelError;
use parking_lot::{Condvar, Mutex as ParkingMutex};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Event flag wait modes
pub mod wait_mode {
    /// Wait for any of the specified bits
    pub const AND: u32 = 0x0001;
    /// Wait for all of the specified bits
    pub const OR: u32 = 0x0002;
    /// Clear bits after waiting
    pub const CLEAR: u32 = 0x0010;
    /// Clear all bits after waiting
    pub const CLEAR_ALL: u32 = 0x0020;
}

/// Event flag attributes
#[derive(Debug, Clone, Copy)]
pub struct EventFlagAttributes {
    /// Protocol for waiting threads
    pub protocol: u32,
    /// Type of event flag
    pub pshared: u32,
    /// Initial bit pattern
    pub initial_pattern: u64,
}

impl Default for EventFlagAttributes {
    fn default() -> Self {
        Self {
            protocol: 0x02, // SYS_SYNC_PRIORITY
            pshared: 0,     // Process-local
            initial_pattern: 0,
        }
    }
}

/// Wait result for event flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventFlagWaitResult {
    /// Condition met
    Success,
    /// Timed out waiting
    TimedOut,
}

/// LV2 Event Flag implementation
pub struct EventFlag {
    id: ObjectId,
    state: ParkingMutex<EventFlagState>,
    condvar: Condvar,
    /// Stored for introspection
    attributes: EventFlagAttributes,
}

#[derive(Debug)]
struct EventFlagState {
    /// Current bit pattern
    pattern: u64,
    /// Threads waiting on this event flag
    waiting_threads: VecDeque<WaitingThread>,
}

#[derive(Debug, Clone)]
struct WaitingThread {
    thread_id: u64,
    _wait_pattern: u64,
    _wait_mode: u32,
}

impl EventFlag {
    pub fn new(id: ObjectId, attributes: EventFlagAttributes) -> Self {
        Self {
            id,
            state: ParkingMutex::new(EventFlagState {
                pattern: attributes.initial_pattern,
                waiting_threads: VecDeque::new(),
            }),
            condvar: Condvar::new(),
            attributes,
        }
    }

    /// Check if wait condition is satisfied
    fn check_condition(pattern: u64, wait_pattern: u64, mode: u32) -> bool {
        if mode & wait_mode::AND != 0 {
            // All bits must be set
            (pattern & wait_pattern) == wait_pattern
        } else {
            // At least one bit must be set (OR mode is default)
            (pattern & wait_pattern) != 0
        }
    }

    /// Set bits in the event flag pattern
    pub fn set(&self, bit_pattern: u64) -> u64 {
        let mut state = self.state.lock();
        state.pattern |= bit_pattern;
        let result = state.pattern;
        
        // Wake up all waiting threads - they will check if their condition is met
        self.condvar.notify_all();
        
        tracing::debug!("EventFlag {}: set bits 0x{:016x}, pattern now 0x{:016x}", 
                       self.id, bit_pattern, result);
        result
    }

    /// Clear bits in the event flag pattern
    pub fn clear(&self, bit_pattern: u64) -> u64 {
        let mut state = self.state.lock();
        state.pattern &= !bit_pattern;
        let result = state.pattern;
        
        tracing::debug!("EventFlag {}: cleared bits 0x{:016x}, pattern now 0x{:016x}", 
                       self.id, bit_pattern, result);
        result
    }

    /// Wait for bits to be set
    pub fn wait(
        &self,
        thread_id: u64,
        wait_pattern: u64,
        mode: u32,
        timeout: Option<Duration>,
    ) -> Result<(EventFlagWaitResult, u64), KernelError> {
        let start = Instant::now();
        
        loop {
            let mut state = self.state.lock();
            
            // Check if condition is already satisfied
            if Self::check_condition(state.pattern, wait_pattern, mode) {
                let result_pattern = state.pattern;
                
                // Handle clear modes
                if mode & wait_mode::CLEAR_ALL != 0 {
                    state.pattern = 0;
                } else if mode & wait_mode::CLEAR != 0 {
                    state.pattern &= !wait_pattern;
                }
                
                // Remove from waiting list if present
                state.waiting_threads.retain(|w| w.thread_id != thread_id);
                
                tracing::debug!("EventFlag {}: thread {} wait satisfied, pattern 0x{:016x}", 
                               self.id, thread_id, result_pattern);
                return Ok((EventFlagWaitResult::Success, result_pattern));
            }
            
            // Check timeout
            if let Some(duration) = timeout {
                if start.elapsed() >= duration {
                    state.waiting_threads.retain(|w| w.thread_id != thread_id);
                    return Ok((EventFlagWaitResult::TimedOut, state.pattern));
                }
            }
            
            // Register as waiting
            if !state.waiting_threads.iter().any(|w| w.thread_id == thread_id) {
                state.waiting_threads.push_back(WaitingThread {
                    thread_id,
                    _wait_pattern: wait_pattern,
                    _wait_mode: mode,
                });
            }
            
            // Wait for a signal
            if let Some(duration) = timeout {
                let remaining = duration.saturating_sub(start.elapsed());
                if remaining.is_zero() {
                    state.waiting_threads.retain(|w| w.thread_id != thread_id);
                    return Ok((EventFlagWaitResult::TimedOut, state.pattern));
                }
                self.condvar.wait_for(&mut state, remaining);
            } else {
                self.condvar.wait(&mut state);
            }
        }
    }

    /// Try to wait without blocking
    pub fn trywait(
        &self,
        wait_pattern: u64,
        mode: u32,
    ) -> Result<u64, KernelError> {
        let mut state = self.state.lock();
        
        if Self::check_condition(state.pattern, wait_pattern, mode) {
            let result_pattern = state.pattern;
            
            // Handle clear modes
            if mode & wait_mode::CLEAR_ALL != 0 {
                state.pattern = 0;
            } else if mode & wait_mode::CLEAR != 0 {
                state.pattern &= !wait_pattern;
            }
            
            Ok(result_pattern)
        } else {
            Err(KernelError::WouldBlock)
        }
    }

    /// Get current pattern
    pub fn get_pattern(&self) -> u64 {
        self.state.lock().pattern
    }

    /// Get number of waiting threads
    pub fn waiting_count(&self) -> usize {
        self.state.lock().waiting_threads.len()
    }

    /// Cancel wait for a specific thread
    pub fn cancel_wait(&self, thread_id: u64) {
        let mut state = self.state.lock();
        state.waiting_threads.retain(|w| w.thread_id != thread_id);
    }

    /// Get attributes
    pub fn get_attributes(&self) -> EventFlagAttributes {
        self.attributes
    }
}

impl KernelObject for EventFlag {
    fn object_type(&self) -> ObjectType {
        ObjectType::EventFlag
    }

    fn id(&self) -> ObjectId {
        self.id
    }

    fn as_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

/// Event flag syscall implementations
pub mod syscalls {
    use super::*;

    /// sys_event_flag_create
    pub fn sys_event_flag_create(
        manager: &ObjectManager,
        attributes: EventFlagAttributes,
    ) -> Result<ObjectId, KernelError> {
        let id = manager.next_id();
        let event_flag = Arc::new(EventFlag::new(id, attributes));
        manager.register(event_flag);
        tracing::debug!("Created event flag {} with pattern 0x{:016x}", 
                       id, attributes.initial_pattern);
        Ok(id)
    }

    /// sys_event_flag_destroy
    pub fn sys_event_flag_destroy(
        manager: &ObjectManager,
        event_flag_id: ObjectId,
    ) -> Result<(), KernelError> {
        manager.unregister(event_flag_id)
    }

    /// sys_event_flag_set
    pub fn sys_event_flag_set(
        manager: &ObjectManager,
        event_flag_id: ObjectId,
        bit_pattern: u64,
    ) -> Result<(), KernelError> {
        let event_flag: Arc<EventFlag> = manager.get(event_flag_id)?;
        event_flag.set(bit_pattern);
        Ok(())
    }

    /// sys_event_flag_clear
    pub fn sys_event_flag_clear(
        manager: &ObjectManager,
        event_flag_id: ObjectId,
        bit_pattern: u64,
    ) -> Result<(), KernelError> {
        let event_flag: Arc<EventFlag> = manager.get(event_flag_id)?;
        event_flag.clear(bit_pattern);
        Ok(())
    }

    /// sys_event_flag_wait
    pub fn sys_event_flag_wait(
        manager: &ObjectManager,
        event_flag_id: ObjectId,
        thread_id: u64,
        bit_pattern: u64,
        mode: u32,
        timeout_usec: u64,
    ) -> Result<u64, KernelError> {
        let event_flag: Arc<EventFlag> = manager.get(event_flag_id)?;
        
        let timeout = if timeout_usec == 0 {
            None
        } else {
            Some(Duration::from_micros(timeout_usec))
        };

        let (result, pattern) = event_flag.wait(thread_id, bit_pattern, mode, timeout)?;
        
        match result {
            EventFlagWaitResult::Success => Ok(pattern),
            EventFlagWaitResult::TimedOut => Err(KernelError::WouldBlock),
        }
    }

    /// sys_event_flag_trywait
    pub fn sys_event_flag_trywait(
        manager: &ObjectManager,
        event_flag_id: ObjectId,
        bit_pattern: u64,
        mode: u32,
    ) -> Result<u64, KernelError> {
        let event_flag: Arc<EventFlag> = manager.get(event_flag_id)?;
        event_flag.trywait(bit_pattern, mode)
    }

    /// sys_event_flag_get
    pub fn sys_event_flag_get(
        manager: &ObjectManager,
        event_flag_id: ObjectId,
    ) -> Result<u64, KernelError> {
        let event_flag: Arc<EventFlag> = manager.get(event_flag_id)?;
        Ok(event_flag.get_pattern())
    }

    /// sys_event_flag_cancel
    pub fn sys_event_flag_cancel(
        manager: &ObjectManager,
        event_flag_id: ObjectId,
        thread_id: u64,
    ) -> Result<(), KernelError> {
        let event_flag: Arc<EventFlag> = manager.get(event_flag_id)?;
        event_flag.cancel_wait(thread_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_flag_create_destroy() {
        let manager = ObjectManager::new();
        let event_flag_id = syscalls::sys_event_flag_create(
            &manager,
            EventFlagAttributes::default(),
        ).unwrap();

        assert!(manager.exists(event_flag_id));

        syscalls::sys_event_flag_destroy(&manager, event_flag_id).unwrap();
        assert!(!manager.exists(event_flag_id));
    }

    #[test]
    fn test_event_flag_set_clear() {
        let manager = ObjectManager::new();
        let event_flag_id = syscalls::sys_event_flag_create(
            &manager,
            EventFlagAttributes::default(),
        ).unwrap();

        let event_flag: Arc<EventFlag> = manager.get(event_flag_id).unwrap();

        // Initial pattern should be 0
        assert_eq!(event_flag.get_pattern(), 0);

        // Set some bits
        syscalls::sys_event_flag_set(&manager, event_flag_id, 0x0F).unwrap();
        assert_eq!(event_flag.get_pattern(), 0x0F);

        // Set more bits
        syscalls::sys_event_flag_set(&manager, event_flag_id, 0xF0).unwrap();
        assert_eq!(event_flag.get_pattern(), 0xFF);

        // Clear some bits
        syscalls::sys_event_flag_clear(&manager, event_flag_id, 0x0F).unwrap();
        assert_eq!(event_flag.get_pattern(), 0xF0);

        syscalls::sys_event_flag_destroy(&manager, event_flag_id).unwrap();
    }

    #[test]
    fn test_event_flag_initial_pattern() {
        let manager = ObjectManager::new();
        let mut attrs = EventFlagAttributes::default();
        attrs.initial_pattern = 0x1234;
        
        let event_flag_id = syscalls::sys_event_flag_create(&manager, attrs).unwrap();
        
        let pattern = syscalls::sys_event_flag_get(&manager, event_flag_id).unwrap();
        assert_eq!(pattern, 0x1234);

        syscalls::sys_event_flag_destroy(&manager, event_flag_id).unwrap();
    }

    #[test]
    fn test_event_flag_trywait_or_mode() {
        let manager = ObjectManager::new();
        let event_flag_id = syscalls::sys_event_flag_create(
            &manager,
            EventFlagAttributes::default(),
        ).unwrap();

        // Set bit 0
        syscalls::sys_event_flag_set(&manager, event_flag_id, 0x01).unwrap();

        // Trywait should succeed for bit 0 (OR mode)
        let pattern = syscalls::sys_event_flag_trywait(
            &manager,
            event_flag_id,
            0x01,
            wait_mode::OR,
        ).unwrap();
        assert_eq!(pattern, 0x01);

        // Trywait should fail for bit 1 (not set)
        let result = syscalls::sys_event_flag_trywait(
            &manager,
            event_flag_id,
            0x02,
            wait_mode::OR,
        );
        assert!(result.is_err());

        syscalls::sys_event_flag_destroy(&manager, event_flag_id).unwrap();
    }

    #[test]
    fn test_event_flag_trywait_and_mode() {
        let manager = ObjectManager::new();
        let event_flag_id = syscalls::sys_event_flag_create(
            &manager,
            EventFlagAttributes::default(),
        ).unwrap();

        // Set bits 0 and 1
        syscalls::sys_event_flag_set(&manager, event_flag_id, 0x03).unwrap();

        // Trywait should succeed for bits 0 AND 1
        let pattern = syscalls::sys_event_flag_trywait(
            &manager,
            event_flag_id,
            0x03,
            wait_mode::AND,
        ).unwrap();
        assert_eq!(pattern, 0x03);

        // Trywait should fail for bits 0 AND 2 (bit 2 not set)
        let result = syscalls::sys_event_flag_trywait(
            &manager,
            event_flag_id,
            0x05,
            wait_mode::AND,
        );
        assert!(result.is_err());

        syscalls::sys_event_flag_destroy(&manager, event_flag_id).unwrap();
    }

    #[test]
    fn test_event_flag_clear_on_wait() {
        let manager = ObjectManager::new();
        let event_flag_id = syscalls::sys_event_flag_create(
            &manager,
            EventFlagAttributes::default(),
        ).unwrap();

        // Set bits
        syscalls::sys_event_flag_set(&manager, event_flag_id, 0xFF).unwrap();

        // Trywait with CLEAR mode
        let pattern = syscalls::sys_event_flag_trywait(
            &manager,
            event_flag_id,
            0x0F,
            wait_mode::OR | wait_mode::CLEAR,
        ).unwrap();
        assert_eq!(pattern, 0xFF);

        // Pattern should have the waited bits cleared
        let current = syscalls::sys_event_flag_get(&manager, event_flag_id).unwrap();
        assert_eq!(current, 0xF0);

        syscalls::sys_event_flag_destroy(&manager, event_flag_id).unwrap();
    }

    #[test]
    fn test_event_flag_clear_all_on_wait() {
        let manager = ObjectManager::new();
        let event_flag_id = syscalls::sys_event_flag_create(
            &manager,
            EventFlagAttributes::default(),
        ).unwrap();

        // Set bits
        syscalls::sys_event_flag_set(&manager, event_flag_id, 0xFF).unwrap();

        // Trywait with CLEAR_ALL mode
        let pattern = syscalls::sys_event_flag_trywait(
            &manager,
            event_flag_id,
            0x0F,
            wait_mode::OR | wait_mode::CLEAR_ALL,
        ).unwrap();
        assert_eq!(pattern, 0xFF);

        // All bits should be cleared
        let current = syscalls::sys_event_flag_get(&manager, event_flag_id).unwrap();
        assert_eq!(current, 0);

        syscalls::sys_event_flag_destroy(&manager, event_flag_id).unwrap();
    }

    #[test]
    fn test_event_flag_wait_timeout() {
        let manager = ObjectManager::new();
        let event_flag_id = syscalls::sys_event_flag_create(
            &manager,
            EventFlagAttributes::default(),
        ).unwrap();

        // Wait with short timeout should time out
        let result = syscalls::sys_event_flag_wait(
            &manager,
            event_flag_id,
            1, // thread_id
            0x01,
            wait_mode::OR,
            1000, // 1ms timeout
        );
        assert!(result.is_err());

        syscalls::sys_event_flag_destroy(&manager, event_flag_id).unwrap();
    }
}
