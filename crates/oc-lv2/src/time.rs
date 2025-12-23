//! Time functions (sys_time_*)

use oc_core::error::KernelError;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Timebase frequency for PS3 (79.8 MHz)
pub const TIMEBASE_FREQUENCY: u64 = 79_800_000;

/// Get current system time in microseconds since UNIX epoch
pub fn get_system_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}

/// Get timebase frequency
pub fn get_timebase_frequency() -> u64 {
    TIMEBASE_FREQUENCY
}

/// Sleep for a given number of microseconds
pub fn usleep(usec: u64) -> Result<(), KernelError> {
    if usec == 0 {
        return Ok(());
    }

    let duration = Duration::from_micros(usec);
    std::thread::sleep(duration);
    Ok(())
}

/// Time syscall implementations
pub mod syscalls {
    use super::*;

    /// sys_time_get_current_time
    pub fn sys_time_get_current_time() -> u64 {
        get_system_time()
    }

    /// sys_time_get_timebase_frequency
    pub fn sys_time_get_timebase_frequency() -> u64 {
        get_timebase_frequency()
    }

    /// sys_time_get_system_time (alternative name)
    pub fn sys_time_get_system_time() -> u64 {
        get_system_time()
    }

    /// sys_time_usleep
    pub fn sys_time_usleep(usec: u64) -> Result<(), KernelError> {
        usleep(usec)
    }

    /// sys_time_sleep (sleep in seconds)
    pub fn sys_time_sleep(seconds: u64) -> Result<(), KernelError> {
        usleep(seconds * 1_000_000)
    }

    /// Get current timebase value (simulated)
    pub fn sys_time_get_timebase() -> u64 {
        // Simulate timebase counter
        // In reality, this would be a hardware counter
        let time = get_system_time();
        // Convert microseconds to timebase ticks
        // Use u128 to avoid overflow
        ((time as u128 * TIMEBASE_FREQUENCY as u128) / 1_000_000) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_system_time() {
        let time = syscalls::sys_time_get_current_time();
        assert!(time > 0);

        // Time should advance
        let time2 = syscalls::sys_time_get_current_time();
        assert!(time2 >= time);
    }

    #[test]
    fn test_timebase_frequency() {
        let freq = syscalls::sys_time_get_timebase_frequency();
        assert_eq!(freq, TIMEBASE_FREQUENCY);
        assert_eq!(freq, 79_800_000);
    }

    #[test]
    fn test_usleep() {
        let start = syscalls::sys_time_get_current_time();

        // Sleep for 10ms
        syscalls::sys_time_usleep(10_000).unwrap();

        let end = syscalls::sys_time_get_current_time();
        let elapsed = end - start;

        // Should have slept at least 10ms (10,000 microseconds)
        // Allow some tolerance for scheduling overhead
        assert!(elapsed >= 8_000, "Elapsed time too short: {}", elapsed);
    }

    #[test]
    fn test_sleep_seconds() {
        let start = syscalls::sys_time_get_current_time();

        // Sleep for 0 seconds (should return immediately)
        syscalls::sys_time_sleep(0).unwrap();

        let end = syscalls::sys_time_get_current_time();
        let elapsed = end - start;

        // Should be very quick (less than 100ms)
        assert!(elapsed < 100_000, "Zero sleep took too long: {}", elapsed);
    }

    #[test]
    fn test_timebase() {
        let tb1 = syscalls::sys_time_get_timebase();
        assert!(tb1 > 0);

        // Timebase should advance
        std::thread::sleep(Duration::from_millis(1));
        let tb2 = syscalls::sys_time_get_timebase();
        assert!(tb2 > tb1);
    }

    #[test]
    fn test_timebase_conversion() {
        // Test that timebase relates properly to time
        let time_us = 1_000_000; // 1 second in microseconds
        let expected_ticks = TIMEBASE_FREQUENCY; // Should be 79.8M ticks

        let calculated_ticks = (time_us * TIMEBASE_FREQUENCY) / 1_000_000;
        assert_eq!(calculated_ticks, expected_ticks);
    }
}

