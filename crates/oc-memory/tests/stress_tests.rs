//! Stress tests for page management under load

use oc_memory::{constants::*, MemoryManager, PageFlags};
use std::sync::Arc;
use std::thread;

#[test]
fn test_heavy_allocation_deallocation() {
    let mem = MemoryManager::new().unwrap();
    
    // Allocate and deallocate many blocks
    let mut allocations = Vec::new();
    
    for _ in 0..100 {
        let size = 0x1000; // 4KB
        let addr = mem.allocate(size, 0x1000, PageFlags::RW).unwrap();
        allocations.push((addr, size));
    }
    
    // Write to each allocation to verify they're valid
    for (addr, _) in &allocations {
        mem.write::<u32>(*addr, 0xDEADBEEF).unwrap();
        assert_eq!(mem.read::<u32>(*addr).unwrap(), 0xDEADBEEF);
    }
    
    // Free all allocations
    for (addr, size) in allocations {
        mem.free(addr, size).unwrap();
    }
}

#[test]
fn test_fragmentation_scenario() {
    let mem = MemoryManager::new().unwrap();
    
    // Allocate many small blocks
    let mut allocations = Vec::new();
    for _ in 0..50 {
        let addr = mem.allocate(0x1000, 0x1000, PageFlags::RW).unwrap();
        allocations.push(addr);
    }
    
    // Free every other block to create fragmentation
    for (i, addr) in allocations.iter().enumerate() {
        if i % 2 == 0 {
            mem.free(*addr, 0x1000).unwrap();
        }
    }
    
    // Try to allocate blocks that might fit in the gaps
    for _ in 0..20 {
        let addr = mem.allocate(0x1000, 0x1000, PageFlags::RW).unwrap();
        mem.write::<u32>(addr, 0x12345678).unwrap();
    }
}

#[test]
fn test_large_allocations() {
    let mem = MemoryManager::new().unwrap();
    
    // Allocate several large blocks
    let sizes = vec![
        0x100000,  // 1 MB
        0x200000,  // 2 MB
        0x400000,  // 4 MB
        0x100000,  // 1 MB
    ];
    
    let mut allocations = Vec::new();
    for size in sizes {
        match mem.allocate(size, 0x1000, PageFlags::RW) {
            Ok(addr) => {
                // Verify allocation by writing to beginning and end (if size >= 4)
                if size >= 4 {
                    mem.write::<u32>(addr, 0xAAAAAAAA).unwrap();
                    mem.write::<u32>(addr + size - 4, 0xBBBBBBBB).unwrap();
                }
                allocations.push((addr, size));
            }
            Err(_) => {
                // Out of memory is acceptable for large allocations
                break;
            }
        }
    }
    
    // Clean up
    for (addr, size) in allocations {
        mem.free(addr, size).unwrap();
    }
}

#[test]
fn test_concurrent_allocations() {
    let mem = Arc::new(MemoryManager::new().unwrap());
    let num_threads = 4;
    let allocs_per_thread = 50;
    
    let mut handles = vec![];
    
    for _ in 0..num_threads {
        let mem_clone = Arc::clone(&mem);
        let handle = thread::spawn(move || {
            let mut local_allocs = Vec::new();
            
            for _ in 0..allocs_per_thread {
                match mem_clone.allocate(0x1000, 0x1000, PageFlags::RW) {
                    Ok(addr) => {
                        // Write to verify
                        mem_clone.write::<u32>(addr, 0xCAFEBABE).unwrap();
                        local_allocs.push(addr);
                    }
                    Err(_) => break,
                }
            }
            
            // Verify all allocations
            for addr in &local_allocs {
                assert_eq!(mem_clone.read::<u32>(*addr).unwrap(), 0xCAFEBABE);
            }
            
            // Free all
            for addr in local_allocs {
                mem_clone.free(addr, 0x1000).unwrap();
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_allocation_patterns() {
    let mem = MemoryManager::new().unwrap();
    
    // Test various allocation sizes
    let sizes = vec![
        0x1000,    // 4 KB (1 page)
        0x2000,    // 8 KB (2 pages)
        0x10000,   // 64 KB (16 pages)
        0x100000,  // 1 MB (256 pages)
    ];
    
    for size in sizes {
        match mem.allocate(size, 0x1000, PageFlags::RW) {
            Ok(addr) => {
                // Verify allocation
                mem.write::<u64>(addr, 0x123456789ABCDEF0).unwrap();
                assert_eq!(mem.read::<u64>(addr).unwrap(), 0x123456789ABCDEF0);
                mem.free(addr, size).unwrap();
            }
            Err(_) => {
                // Out of memory for large allocations is OK
                break;
            }
        }
    }
}

#[test]
fn test_out_of_memory_handling() {
    let mem = MemoryManager::new().unwrap();
    
    let mut allocations = Vec::new();
    let mut total_allocated = 0u64;
    
    // Try to allocate until we run out of memory
    loop {
        match mem.allocate(0x100000, 0x1000, PageFlags::RW) {
            Ok(addr) => {
                allocations.push(addr);
                total_allocated += 0x100000;
                
                // Verify the allocation works
                mem.write::<u32>(addr, 0xDEADBEEF).unwrap();
            }
            Err(_) => {
                // Expected: we've run out of allocatable user memory
                break;
            }
        }
        
        // Safety limit to prevent infinite loop
        if total_allocated > USER_MEM_SIZE as u64 {
            break;
        }
    }
    
    // Should have allocated a reasonable amount
    assert!(total_allocated > 0);
    
    // Clean up
    for addr in allocations {
        mem.free(addr, 0x100000).unwrap();
    }
}

#[test]
fn test_interleaved_alloc_free() {
    let mem = MemoryManager::new().unwrap();
    
    // Interleave allocations and frees
    for _ in 0..10 {
        let mut temp_allocs = Vec::new();
        
        // Allocate batch
        for _ in 0..10 {
            if let Ok(addr) = mem.allocate(0x1000, 0x1000, PageFlags::RW) {
                mem.write::<u32>(addr, 0x11111111).unwrap();
                temp_allocs.push(addr);
            }
        }
        
        // Free batch
        for addr in temp_allocs {
            mem.free(addr, 0x1000).unwrap();
        }
    }
}

#[test]
fn test_write_patterns_across_allocations() {
    let mem = MemoryManager::new().unwrap();
    
    const NUM_ALLOCS: usize = 20;
    const ALLOC_SIZE: u32 = 0x1000; // 4KB
    const WORDS_PER_ALLOC: u32 = 256; // 256 * 4 = 1024 bytes (fits in 4KB)
    
    let mut allocations = Vec::new();
    
    // Allocate multiple blocks
    for i in 0..NUM_ALLOCS {
        if let Ok(addr) = mem.allocate(ALLOC_SIZE, 0x1000, PageFlags::RW) {
            // Write unique pattern to each allocation
            for j in 0..WORDS_PER_ALLOC {
                mem.write::<u32>(addr + j * 4, (i as u32) << 16 | j).unwrap();
            }
            allocations.push(addr);
        }
    }
    
    // Verify all patterns are intact
    for (i, addr) in allocations.iter().enumerate() {
        for j in 0..WORDS_PER_ALLOC {
            let expected = ((i as u32) << 16) | j;
            let actual = mem.read::<u32>(*addr + j * 4).unwrap();
            assert_eq!(actual, expected, 
                "Mismatch at alloc {} offset {}: expected {:08x}, got {:08x}",
                i, j, expected, actual);
        }
    }
    
    // Clean up
    for addr in allocations {
        mem.free(addr, ALLOC_SIZE).unwrap();
    }
}

#[test]
fn test_page_reuse_after_free() {
    let mem = MemoryManager::new().unwrap();
    
    // Allocate and free multiple times to test page reuse
    for iteration in 0..5 {
        let mut allocs = Vec::new();
        
        // Allocate
        for _ in 0..10 {
            if let Ok(addr) = mem.allocate(0x1000, 0x1000, PageFlags::RW) {
                mem.write::<u32>(addr, iteration).unwrap();
                allocs.push(addr);
            }
        }
        
        // Verify
        for addr in &allocs {
            assert_eq!(mem.read::<u32>(*addr).unwrap(), iteration);
        }
        
        // Free
        for addr in allocs {
            mem.free(addr, 0x1000).unwrap();
        }
    }
}

#[test]
fn test_allocation_alignment() {
    let mem = MemoryManager::new().unwrap();
    
    // All allocations should respect page alignment
    for _ in 0..50 {
        if let Ok(addr) = mem.allocate(0x2500, 0x1000, PageFlags::RW) {
            assert_eq!(addr % PAGE_SIZE, 0, "Address {:x} not page-aligned", addr);
            mem.free(addr, 0x3000).unwrap();
        }
    }
}

#[test]
fn test_rapid_alloc_free_cycles() {
    let mem = Arc::new(MemoryManager::new().unwrap());
    let num_threads = 4;
    let cycles_per_thread = 100;
    
    let mut handles = vec![];
    
    for _ in 0..num_threads {
        let mem_clone = Arc::clone(&mem);
        let handle = thread::spawn(move || {
            for _ in 0..cycles_per_thread {
                if let Ok(addr) = mem_clone.allocate(0x1000, 0x1000, PageFlags::RW) {
                    mem_clone.write::<u64>(addr, 0xABCDEF0123456789).unwrap();
                    mem_clone.free(addr, 0x1000).unwrap();
                }
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
}
