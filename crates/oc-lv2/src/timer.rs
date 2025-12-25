//! Timer (sys_timer_*)
//!
//! High-resolution timers for the PS3 LV2 kernel.

use crate::objects::{KernelObject, ObjectId, ObjectManager, ObjectType};
use oc_core::error::KernelError;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Timer states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerState {
    /// Timer is stopped
    Stopped,
    /// Timer is running
    Running,
    /// Timer has expired
    Expired,
}

/// Timer types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerType {
    /// One-shot timer
    OneShot,
    /// Periodic timer
    Periodic,
}

/// Timer attributes
#[derive(Debug, Clone, Copy)]
pub struct TimerAttributes {
    /// Timer name ID
    pub name: u32,
    /// Timer type
    pub timer_type: TimerType,
}

impl Default for TimerAttributes {
    fn default() -> Self {
        Self {
            name: 0,
            timer_type: TimerType::OneShot,
        }
    }
}

/// Timer information returned by get_info
#[derive(Debug, Clone)]
pub struct TimerInfo {
    /// Timer state
    pub state: TimerState,
    /// Timer type
    pub timer_type: TimerType,
    /// Remaining time in microseconds (0 if not running)
    pub remaining_usec: u64,
    /// Period in microseconds (for periodic timers)
    pub period_usec: u64,
}

/// LV2 Timer implementation
pub struct Timer {
    id: ObjectId,
    state: Mutex<TimerInnerState>,
    attributes: TimerAttributes,
}

#[derive(Debug)]
struct TimerInnerState {
    /// Current timer state
    state: TimerState,
    /// When the timer was started
    start_time: Option<Instant>,
    /// Timer duration
    duration: Duration,
    /// Period for periodic timers
    period: Duration,
    /// Event queue ID to notify when timer expires
    event_queue_id: Option<ObjectId>,
    /// Event source ID
    event_source: u64,
    /// Expiration count (for periodic timers)
    expiration_count: u64,
}

impl Timer {
    pub fn new(id: ObjectId, attributes: TimerAttributes) -> Self {
        Self {
            id,
            state: Mutex::new(TimerInnerState {
                state: TimerState::Stopped,
                start_time: None,
                duration: Duration::ZERO,
                period: Duration::ZERO,
                event_queue_id: None,
                event_source: 0,
                expiration_count: 0,
            }),
            attributes,
        }
    }

    /// Start the timer with the specified duration
    pub fn start(&self, duration_usec: u64, period_usec: u64) -> Result<(), KernelError> {
        let mut state = self.state.lock();
        
        state.state = TimerState::Running;
        state.start_time = Some(Instant::now());
        state.duration = Duration::from_micros(duration_usec);
        state.period = Duration::from_micros(period_usec);
        state.expiration_count = 0;
        
        tracing::debug!("Timer {} started: duration={}us, period={}us", 
                       self.id, duration_usec, period_usec);
        Ok(())
    }

    /// Stop the timer
    pub fn stop(&self) -> Result<(), KernelError> {
        let mut state = self.state.lock();
        state.state = TimerState::Stopped;
        state.start_time = None;
        tracing::debug!("Timer {} stopped", self.id);
        Ok(())
    }

    /// Check if timer has expired and update state
    pub fn check(&self) -> TimerState {
        let mut state = self.state.lock();
        
        if state.state != TimerState::Running {
            return state.state;
        }
        
        if let Some(start_time) = state.start_time {
            let elapsed = start_time.elapsed();
            if elapsed >= state.duration {
                state.expiration_count += 1;
                
                if self.attributes.timer_type == TimerType::Periodic && !state.period.is_zero() {
                    // Reset for next period
                    state.start_time = Some(Instant::now());
                    state.duration = state.period;
                } else {
                    state.state = TimerState::Expired;
                    state.start_time = None;
                }
                
                tracing::debug!("Timer {} expired (count={})", self.id, state.expiration_count);
            }
        }
        
        state.state
    }

    /// Get remaining time in microseconds
    pub fn get_remaining(&self) -> u64 {
        let state = self.state.lock();
        
        if state.state != TimerState::Running {
            return 0;
        }
        
        if let Some(start_time) = state.start_time {
            let elapsed = start_time.elapsed();
            if elapsed < state.duration {
                return (state.duration - elapsed).as_micros() as u64;
            }
        }
        
        0
    }

    /// Get timer information
    pub fn get_info(&self) -> TimerInfo {
        let state = self.state.lock();
        TimerInfo {
            state: state.state,
            timer_type: self.attributes.timer_type,
            remaining_usec: self.get_remaining_internal(&state),
            period_usec: state.period.as_micros() as u64,
        }
    }

    fn get_remaining_internal(&self, state: &TimerInnerState) -> u64 {
        if state.state != TimerState::Running {
            return 0;
        }
        
        if let Some(start_time) = state.start_time {
            let elapsed = start_time.elapsed();
            if elapsed < state.duration {
                return (state.duration - elapsed).as_micros() as u64;
            }
        }
        
        0
    }

    /// Get expiration count
    pub fn get_expiration_count(&self) -> u64 {
        self.state.lock().expiration_count
    }

    /// Connect timer to an event queue
    pub fn connect(&self, event_queue_id: ObjectId, event_source: u64) -> Result<(), KernelError> {
        let mut state = self.state.lock();
        state.event_queue_id = Some(event_queue_id);
        state.event_source = event_source;
        tracing::debug!("Timer {} connected to event queue {}", self.id, event_queue_id);
        Ok(())
    }

    /// Disconnect timer from event queue
    pub fn disconnect(&self) -> Result<(), KernelError> {
        let mut state = self.state.lock();
        state.event_queue_id = None;
        state.event_source = 0;
        tracing::debug!("Timer {} disconnected", self.id);
        Ok(())
    }

    /// Get the current state
    pub fn get_state(&self) -> TimerState {
        self.state.lock().state
    }
}

impl KernelObject for Timer {
    fn object_type(&self) -> ObjectType {
        ObjectType::Timer
    }

    fn id(&self) -> ObjectId {
        self.id
    }

    fn as_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

/// Timer syscall implementations
pub mod syscalls {
    use super::*;

    /// sys_timer_create
    pub fn sys_timer_create(
        manager: &ObjectManager,
        attributes: TimerAttributes,
    ) -> Result<ObjectId, KernelError> {
        let id = manager.next_id();
        let timer = Arc::new(Timer::new(id, attributes));
        manager.register(timer);
        tracing::debug!("Created timer {}", id);
        Ok(id)
    }

    /// sys_timer_destroy
    pub fn sys_timer_destroy(
        manager: &ObjectManager,
        timer_id: ObjectId,
    ) -> Result<(), KernelError> {
        let timer: Arc<Timer> = manager.get(timer_id)?;
        timer.stop()?;
        manager.unregister(timer_id)
    }

    /// sys_timer_start
    pub fn sys_timer_start(
        manager: &ObjectManager,
        timer_id: ObjectId,
        base_time: u64,
        period: u64,
    ) -> Result<(), KernelError> {
        let timer: Arc<Timer> = manager.get(timer_id)?;
        timer.start(base_time, period)
    }

    /// sys_timer_stop
    pub fn sys_timer_stop(
        manager: &ObjectManager,
        timer_id: ObjectId,
    ) -> Result<(), KernelError> {
        let timer: Arc<Timer> = manager.get(timer_id)?;
        timer.stop()
    }

    /// sys_timer_get_information
    pub fn sys_timer_get_information(
        manager: &ObjectManager,
        timer_id: ObjectId,
    ) -> Result<TimerInfo, KernelError> {
        let timer: Arc<Timer> = manager.get(timer_id)?;
        Ok(timer.get_info())
    }

    /// sys_timer_connect_event_queue
    pub fn sys_timer_connect_event_queue(
        manager: &ObjectManager,
        timer_id: ObjectId,
        event_queue_id: ObjectId,
        event_source: u64,
    ) -> Result<(), KernelError> {
        // Verify event queue exists
        let _: Arc<crate::sync::event::EventQueue> = manager.get(event_queue_id)?;
        
        let timer: Arc<Timer> = manager.get(timer_id)?;
        timer.connect(event_queue_id, event_source)
    }

    /// sys_timer_disconnect_event_queue
    pub fn sys_timer_disconnect_event_queue(
        manager: &ObjectManager,
        timer_id: ObjectId,
    ) -> Result<(), KernelError> {
        let timer: Arc<Timer> = manager.get(timer_id)?;
        timer.disconnect()
    }

    /// Sleep for a specified duration using high-resolution timing
    pub fn sys_timer_usleep(duration_usec: u64) -> Result<(), KernelError> {
        if duration_usec == 0 {
            return Ok(());
        }
        std::thread::sleep(Duration::from_micros(duration_usec));
        Ok(())
    }

    /// Sleep for a specified duration in seconds
    pub fn sys_timer_sleep(duration_sec: u32) -> Result<(), KernelError> {
        if duration_sec == 0 {
            return Ok(());
        }
        std::thread::sleep(Duration::from_secs(duration_sec as u64));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_create_destroy() {
        let manager = ObjectManager::new();
        let timer_id = syscalls::sys_timer_create(
            &manager,
            TimerAttributes::default(),
        ).unwrap();

        assert!(manager.exists(timer_id));

        syscalls::sys_timer_destroy(&manager, timer_id).unwrap();
        assert!(!manager.exists(timer_id));
    }

    #[test]
    fn test_timer_start_stop() {
        let manager = ObjectManager::new();
        let timer_id = syscalls::sys_timer_create(
            &manager,
            TimerAttributes::default(),
        ).unwrap();

        let timer: Arc<Timer> = manager.get(timer_id).unwrap();
        assert_eq!(timer.get_state(), TimerState::Stopped);

        // Start timer
        syscalls::sys_timer_start(&manager, timer_id, 1_000_000, 0).unwrap();
        assert_eq!(timer.get_state(), TimerState::Running);

        // Stop timer
        syscalls::sys_timer_stop(&manager, timer_id).unwrap();
        assert_eq!(timer.get_state(), TimerState::Stopped);

        syscalls::sys_timer_destroy(&manager, timer_id).unwrap();
    }

    #[test]
    fn test_timer_remaining_time() {
        let manager = ObjectManager::new();
        let timer_id = syscalls::sys_timer_create(
            &manager,
            TimerAttributes::default(),
        ).unwrap();

        let timer: Arc<Timer> = manager.get(timer_id).unwrap();

        // Start timer with 100ms duration
        syscalls::sys_timer_start(&manager, timer_id, 100_000, 0).unwrap();
        
        // Remaining time should be close to 100ms
        let remaining = timer.get_remaining();
        assert!(remaining > 0);
        assert!(remaining <= 100_000);

        syscalls::sys_timer_destroy(&manager, timer_id).unwrap();
    }

    #[test]
    fn test_timer_expiration() {
        let manager = ObjectManager::new();
        let timer_id = syscalls::sys_timer_create(
            &manager,
            TimerAttributes::default(),
        ).unwrap();

        let timer: Arc<Timer> = manager.get(timer_id).unwrap();

        // Start timer with very short duration
        syscalls::sys_timer_start(&manager, timer_id, 1, 0).unwrap();
        
        // Wait a bit
        std::thread::sleep(Duration::from_millis(10));
        
        // Check should show expired
        let state = timer.check();
        assert_eq!(state, TimerState::Expired);
        assert_eq!(timer.get_expiration_count(), 1);

        syscalls::sys_timer_destroy(&manager, timer_id).unwrap();
    }

    #[test]
    fn test_timer_get_information() {
        let manager = ObjectManager::new();
        let timer_id = syscalls::sys_timer_create(
            &manager,
            TimerAttributes::default(),
        ).unwrap();

        // Start timer
        syscalls::sys_timer_start(&manager, timer_id, 100_000, 50_000).unwrap();
        
        let info = syscalls::sys_timer_get_information(&manager, timer_id).unwrap();
        assert_eq!(info.state, TimerState::Running);
        assert_eq!(info.period_usec, 50_000);
        assert!(info.remaining_usec > 0);

        syscalls::sys_timer_destroy(&manager, timer_id).unwrap();
    }

    #[test]
    fn test_timer_periodic() {
        let manager = ObjectManager::new();
        let mut attrs = TimerAttributes::default();
        attrs.timer_type = TimerType::Periodic;
        
        let timer_id = syscalls::sys_timer_create(&manager, attrs).unwrap();

        let timer: Arc<Timer> = manager.get(timer_id).unwrap();

        // Start timer with very short initial and period
        syscalls::sys_timer_start(&manager, timer_id, 1, 1).unwrap();
        
        // Wait a bit and check multiple times
        std::thread::sleep(Duration::from_millis(10));
        timer.check();
        std::thread::sleep(Duration::from_millis(10));
        timer.check();
        
        // Should have expired multiple times
        let count = timer.get_expiration_count();
        assert!(count >= 1, "Expected at least 1 expiration, got {}", count);

        syscalls::sys_timer_destroy(&manager, timer_id).unwrap();
    }

    #[test]
    fn test_timer_usleep() {
        let start = Instant::now();
        syscalls::sys_timer_usleep(10_000).unwrap(); // 10ms
        let elapsed = start.elapsed();
        
        // Should have slept at least 8ms (allowing for some variance)
        assert!(elapsed >= Duration::from_millis(8));
    }

    #[test]
    fn test_timer_sleep_zero() {
        // Zero duration should return immediately
        let start = Instant::now();
        syscalls::sys_timer_usleep(0).unwrap();
        let elapsed = start.elapsed();
        
        // Should be very fast
        assert!(elapsed < Duration::from_millis(10));
    }
}
