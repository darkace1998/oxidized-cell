//! Tests for 128-byte reservation system for SPU atomics

use oc_memory::{constants::*, MemoryManager, Reservation};
use std::sync::Arc;
use std::thread;

#[test]
fn test_reservation_granularity() {
    let mem = MemoryManager::new().unwrap();
    
    // Test that reservations are at 128-byte granularity
    let addr1 = 0x0;
    let addr2 = 0x80; // 128 bytes apart
    let addr3 = 0x100; // 256 bytes apart
    
    let res1 = mem.reservation(addr1);
    let res2 = mem.reservation(addr2);
    let res3 = mem.reservation(addr3);
    
    // Different addresses at 128-byte boundaries should have different reservations
    assert_ne!(res1 as *const Reservation, res2 as *const Reservation);
    assert_ne!(res2 as *const Reservation, res3 as *const Reservation);
    
    // Addresses within the same 128-byte block should share the same reservation
    let addr1_same = 0x40; // Within first 128-byte block
    let res1_same = mem.reservation(addr1_same);
    assert_eq!(res1 as *const Reservation, res1_same as *const Reservation);
}

#[test]
fn test_reservation_concurrent_lock() {
    let mem = Arc::new(MemoryManager::new().unwrap());
    let addr = MAIN_MEM_BASE;
    
    // First thread acquires lock
    let mem1 = Arc::clone(&mem);
    let handle1 = thread::spawn(move || {
        let res = mem1.reservation(addr);
        let time = res.acquire();
        assert!(res.try_lock(time));
        
        // Hold the lock briefly
        thread::sleep(std::time::Duration::from_millis(50));
        
        res.unlock_and_increment();
    });
    
    // Second thread tries to acquire the same lock
    let mem2 = Arc::clone(&mem);
    let handle2 = thread::spawn(move || {
        thread::sleep(std::time::Duration::from_millis(10)); // Let first thread lock
        
        let res = mem2.reservation(addr);
        let time = res.acquire();
        
        // Should fail because first thread holds the lock
        assert!(!res.try_lock(time));
        
        // Wait for first thread to release
        thread::sleep(std::time::Duration::from_millis(100));
        
        // Now we should be able to lock
        let new_time = res.acquire();
        assert!(new_time > time); // Timestamp should have incremented
    });
    
    handle1.join().unwrap();
    handle2.join().unwrap();
}

#[test]
fn test_reservation_conflicts_multiple_threads() {
    let mem = Arc::new(MemoryManager::new().unwrap());
    let addr = MAIN_MEM_BASE;
    let num_threads = 10;
    let mut handles = vec![];
    
    // Multiple threads trying to acquire the same reservation
    for i in 0..num_threads {
        let mem_clone = Arc::clone(&mem);
        let handle = thread::spawn(move || {
            let res = mem_clone.reservation(addr);
            
            for _ in 0..100 {
                let time = res.acquire();
                if res.try_lock(time) {
                    // Successfully acquired lock
                    thread::yield_now(); // Simulate some work
                    res.unlock_and_increment();
                    return i;
                }
                // Failed to acquire, retry
                thread::yield_now();
            }
            
            // Should eventually succeed
            let mut time = res.acquire();
            while !res.try_lock(time) {
                thread::yield_now();
                time = res.acquire();
            }
            res.unlock_and_increment();
            i
        });
        handles.push(handle);
    }
    
    // All threads should complete
    for handle in handles {
        let result = handle.join().unwrap();
        assert!(result < num_threads);
    }
}

#[test]
fn test_reservation_invalidation() {
    let mem = MemoryManager::new().unwrap();
    let addr = MAIN_MEM_BASE;
    
    let res = mem.reservation(addr);
    
    // Get initial timestamp
    let time1 = res.acquire();
    
    // Invalidate the reservation
    res.invalidate();
    
    // Timestamp should have incremented
    let time2 = res.acquire();
    assert_eq!(time2, time1 + 128);
    
    // Multiple invalidations
    res.invalidate();
    res.invalidate();
    let time3 = res.acquire();
    assert_eq!(time3, time2 + 256);
}

#[test]
fn test_reservation_lock_status() {
    let mem = MemoryManager::new().unwrap();
    let addr = MAIN_MEM_BASE;
    
    let res = mem.reservation(addr);
    
    // Initially not locked
    assert!(!res.is_locked());
    
    // Acquire lock
    let time = res.acquire();
    assert!(res.try_lock(time));
    assert!(res.is_locked());
    
    // Unlock
    res.unlock_and_increment();
    assert!(!res.is_locked());
}

#[test]
fn test_reservation_timestamp_increment() {
    let mem = MemoryManager::new().unwrap();
    let addr = MAIN_MEM_BASE;
    
    let res = mem.reservation(addr);
    
    let mut last_time = res.acquire();
    assert_eq!(last_time, 0);
    
    // Each lock/unlock cycle should increment by 128
    for i in 1..=10 {
        let time = res.acquire();
        assert_eq!(time, last_time);
        
        assert!(res.try_lock(time));
        res.unlock_and_increment();
        
        let new_time = res.acquire();
        assert_eq!(new_time, i * 128);
        last_time = new_time;
    }
}

#[test]
fn test_reservation_different_cache_lines() {
    let mem = MemoryManager::new().unwrap();
    
    // Test multiple cache lines can be locked simultaneously
    let addr1 = MAIN_MEM_BASE;
    let addr2 = MAIN_MEM_BASE + 128;
    let addr3 = MAIN_MEM_BASE + 256;
    
    let res1 = mem.reservation(addr1);
    let res2 = mem.reservation(addr2);
    let res3 = mem.reservation(addr3);
    
    // Lock all three
    let time1 = res1.acquire();
    let time2 = res2.acquire();
    let time3 = res3.acquire();
    
    assert!(res1.try_lock(time1));
    assert!(res2.try_lock(time2));
    assert!(res3.try_lock(time3));
    
    // All should be locked
    assert!(res1.is_locked());
    assert!(res2.is_locked());
    assert!(res3.is_locked());
    
    // Unlock all
    res1.unlock_and_increment();
    res2.unlock_and_increment();
    res3.unlock_and_increment();
    
    assert!(!res1.is_locked());
    assert!(!res2.is_locked());
    assert!(!res3.is_locked());
}

#[test]
fn test_reservation_stress_single_line() {
    let mem = Arc::new(MemoryManager::new().unwrap());
    let addr = MAIN_MEM_BASE;
    const NUM_THREADS: usize = 8;
    const ITERATIONS_PER_THREAD: usize = 100;
    
    let mut handles = vec![];
    
    for _ in 0..NUM_THREADS {
        let mem_clone = Arc::clone(&mem);
        let handle = thread::spawn(move || {
            let res = mem_clone.reservation(addr);
            let mut success_count = 0;
            
            for _ in 0..ITERATIONS_PER_THREAD {
                // Keep trying until we succeed
                loop {
                    let time = res.acquire();
                    if res.try_lock(time) {
                        // Simulate atomic operation
                        thread::yield_now();
                        res.unlock_and_increment();
                        success_count += 1;
                        break;
                    }
                    // Back off and retry
                    thread::yield_now();
                }
            }
            
            success_count
        });
        handles.push(handle);
    }
    
    let mut total_operations = 0;
    for handle in handles {
        total_operations += handle.join().unwrap();
    }
    
    assert_eq!(total_operations, NUM_THREADS * ITERATIONS_PER_THREAD);
}

#[test]
fn test_reservation_isolation_across_regions() {
    let mem = MemoryManager::new().unwrap();
    
    // Test that reservations in different memory regions work independently
    let main_addr = MAIN_MEM_BASE + 128;
    let stack_addr = STACK_BASE + 128;
    let user_addr = USER_MEM_BASE + 128;
    
    let res_main = mem.reservation(main_addr);
    let res_stack = mem.reservation(stack_addr);
    let res_user = mem.reservation(user_addr);
    
    // Lock all three
    let time_main = res_main.acquire();
    let time_stack = res_stack.acquire();
    let time_user = res_user.acquire();
    
    assert!(res_main.try_lock(time_main));
    assert!(res_stack.try_lock(time_stack));
    assert!(res_user.try_lock(time_user));
    
    // All should be independently locked
    assert!(res_main.is_locked());
    assert!(res_stack.is_locked());
    assert!(res_user.is_locked());
    
    // Unlock main, others should remain locked
    res_main.unlock_and_increment();
    assert!(!res_main.is_locked());
    assert!(res_stack.is_locked());
    assert!(res_user.is_locked());
    
    res_stack.unlock_and_increment();
    res_user.unlock_and_increment();
}
