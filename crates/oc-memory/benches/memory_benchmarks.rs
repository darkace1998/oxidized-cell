//! Benchmarks for memory access patterns

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use oc_memory::{constants::*, MemoryManager, PageFlags};
use std::sync::Arc;

fn bench_sequential_read_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("sequential_access");
    
    for size in [1024, 4096, 16384, 65536].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        
        group.bench_with_input(BenchmarkId::new("write", size), size, |b, &size| {
            let mem = MemoryManager::new().unwrap();
            let addr = mem.allocate(size, 0x1000, PageFlags::RW).unwrap();
            
            b.iter(|| {
                for i in (0..size).step_by(4) {
                    unsafe {
                        mem.write_unchecked(addr + i, black_box(0xDEADBEEFu32));
                    }
                }
            });
        });
        
        group.bench_with_input(BenchmarkId::new("read", size), size, |b, &size| {
            let mem = MemoryManager::new().unwrap();
            let addr = mem.allocate(size, 0x1000, PageFlags::RW).unwrap();
            
            // Pre-fill with data
            for i in (0..size).step_by(4) {
                unsafe {
                    mem.write_unchecked(addr + i, 0xDEADBEEFu32);
                }
            }
            
            b.iter(|| {
                let mut sum = 0u64;
                for i in (0..size).step_by(4) {
                    let val: u32 = unsafe { mem.read_unchecked(addr + i) };
                    sum = sum.wrapping_add(val as u64);
                }
                black_box(sum);
            });
        });
    }
    
    group.finish();
}

fn bench_random_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("random_access");
    
    const NUM_OPERATIONS: usize = 1000;
    const RANDOM_STEP: u32 = 97; // Prime number for pseudo-random distribution
    
    let mem = MemoryManager::new().unwrap();
    let size = 65536u32;
    let addr = mem.allocate(size, 0x1000, PageFlags::RW).unwrap();
    
    // Pre-generate random offsets using a pseudo-random sequence
    let offsets: Vec<u32> = (0..NUM_OPERATIONS)
        .map(|i| ((i as u32) * RANDOM_STEP) % (size / 4))
        .map(|i| i * 4)
        .collect();
    
    group.bench_function("random_write", |b| {
        b.iter(|| {
            for &offset in &offsets {
                unsafe {
                    mem.write_unchecked(addr + offset, black_box(0xCAFEBABEu32));
                }
            }
        });
    });
    
    // Pre-fill with data
    for i in (0..size).step_by(4) {
        unsafe {
            mem.write_unchecked(addr + i, i);
        }
    }
    
    group.bench_function("random_read", |b| {
        b.iter(|| {
            let mut sum = 0u64;
            for &offset in &offsets {
                let val: u32 = unsafe { mem.read_unchecked(addr + offset) };
                sum = sum.wrapping_add(val as u64);
            }
            black_box(sum);
        });
    });
    
    group.finish();
}

fn bench_reservation_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("reservation");
    
    let mem = Arc::new(MemoryManager::new().unwrap());
    let addr = MAIN_MEM_BASE;
    
    group.bench_function("acquire", |b| {
        let res = mem.reservation(addr);
        b.iter(|| {
            black_box(res.acquire());
        });
    });
    
    group.bench_function("try_lock_success", |b| {
        let res = mem.reservation(addr);
        b.iter(|| {
            let time = res.acquire();
            let locked = res.try_lock(time);
            if locked {
                res.unlock_and_increment();
            }
            black_box(locked);
        });
    });
    
    group.bench_function("invalidate", |b| {
        let res = mem.reservation(addr);
        b.iter(|| {
            res.invalidate();
        });
    });
    
    group.bench_function("lock_unlock_cycle", |b| {
        let res = mem.reservation(addr);
        b.iter(|| {
            let time = res.acquire();
            if res.try_lock(time) {
                res.unlock_and_increment();
            }
        });
    });
    
    group.finish();
}

fn bench_allocation_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocation");
    
    for size in [0x1000, 0x10000, 0x100000].iter() {
        group.bench_with_input(BenchmarkId::new("allocate", size), size, |b, &size| {
            let mem = MemoryManager::new().unwrap();
            
            b.iter(|| {
                let addr = mem.allocate(size, 0x1000, PageFlags::RW).unwrap();
                black_box(addr);
                mem.free(addr, size).unwrap();
            });
        });
    }
    
    group.finish();
}

fn bench_big_endian_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("big_endian");
    
    let mem = MemoryManager::new().unwrap();
    let addr = MAIN_MEM_BASE;
    
    group.bench_function("write_be32", |b| {
        b.iter(|| {
            mem.write_be32(addr, black_box(0x12345678)).unwrap();
        });
    });
    
    group.bench_function("read_be32", |b| {
        mem.write_be32(addr, 0x12345678).unwrap();
        b.iter(|| {
            black_box(mem.read_be32(addr).unwrap());
        });
    });
    
    group.bench_function("write_be64", |b| {
        b.iter(|| {
            mem.write_be64(addr, black_box(0x123456789ABCDEF0)).unwrap();
        });
    });
    
    group.bench_function("read_be64", |b| {
        mem.write_be64(addr, 0x123456789ABCDEF0).unwrap();
        b.iter(|| {
            black_box(mem.read_be64(addr).unwrap());
        });
    });
    
    group.finish();
}

fn bench_checked_vs_unchecked(c: &mut Criterion) {
    let mut group = c.benchmark_group("checked_vs_unchecked");
    
    let mem = MemoryManager::new().unwrap();
    let addr = MAIN_MEM_BASE + 0x1000;
    
    group.bench_function("checked_read", |b| {
        b.iter(|| {
            black_box(mem.read::<u32>(addr).unwrap());
        });
    });
    
    group.bench_function("unchecked_read", |b| {
        b.iter(|| {
            unsafe {
                black_box(mem.read_unchecked::<u32>(addr));
            }
        });
    });
    
    group.bench_function("checked_write", |b| {
        b.iter(|| {
            mem.write(addr, black_box(0xDEADBEEFu32)).unwrap();
        });
    });
    
    group.bench_function("unchecked_write", |b| {
        b.iter(|| {
            unsafe {
                mem.write_unchecked(addr, black_box(0xDEADBEEFu32));
            }
        });
    });
    
    group.finish();
}

fn bench_bulk_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("bulk_operations");
    
    for size in [1024, 4096, 65536].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        
        group.bench_with_input(BenchmarkId::new("write_bytes", size), size, |b, &size| {
            let mem = MemoryManager::new().unwrap();
            let addr = mem.allocate(size, 0x1000, PageFlags::RW).unwrap();
            let data = vec![0xABu8; size as usize];
            
            b.iter(|| {
                mem.write_bytes(addr, black_box(&data)).unwrap();
            });
        });
        
        group.bench_with_input(BenchmarkId::new("read_bytes", size), size, |b, &size| {
            let mem = MemoryManager::new().unwrap();
            let addr = mem.allocate(size, 0x1000, PageFlags::RW).unwrap();
            let data = vec![0xABu8; size as usize];
            mem.write_bytes(addr, &data).unwrap();
            
            b.iter(|| {
                black_box(mem.read_bytes(addr, size).unwrap());
            });
        });
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_sequential_read_write,
    bench_random_access,
    bench_reservation_operations,
    bench_allocation_patterns,
    bench_big_endian_access,
    bench_checked_vs_unchecked,
    bench_bulk_operations
);
criterion_main!(benches);
