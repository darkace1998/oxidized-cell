//! Atomic operations interface
//!
//! This module provides FFI for 128-bit atomic operations.
//! On x86_64, these use hardware cmpxchg16b/movdqa for true atomicity.
//! On other platforms, a global mutex in C++ ensures thread safety.

use crate::types::V128;

extern "C" {
    fn oc_atomic_cas128(ptr: *mut V128, expected: *mut V128, desired: *const V128) -> i32;
    fn oc_atomic_load128(ptr: *const V128, result: *mut V128);
    fn oc_atomic_store128(ptr: *mut V128, value: *const V128);
}

/// Perform a 128-bit atomic compare-and-swap.
///
/// If the value at `ptr` equals `expected`, it is replaced with `desired` and
/// returns `true`. Otherwise, `expected` is updated with the current value and
/// returns `false`.
///
/// # Safety
/// `ptr` must point to a valid, 16-byte aligned `V128`.
pub unsafe fn atomic_cas128(ptr: *mut V128, expected: &mut V128, desired: &V128) -> bool {
    oc_atomic_cas128(ptr, expected as *mut V128, desired as *const V128) != 0
}

/// Perform a 128-bit atomic load.
///
/// # Safety
/// `ptr` must point to a valid, 16-byte aligned `V128`.
pub unsafe fn atomic_load128(ptr: *const V128) -> V128 {
    let mut result = V128::new();
    oc_atomic_load128(ptr, &mut result as *mut V128);
    result
}

/// Perform a 128-bit atomic store.
///
/// # Safety
/// `ptr` must point to a valid, 16-byte aligned `V128`.
pub unsafe fn atomic_store128(ptr: *mut V128, value: &V128) {
    oc_atomic_store128(ptr, value as *const V128);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_load_store_roundtrip() {
        let mut storage = V128::from_u32x4([0xAAAA_BBBB, 0xCCCC_DDDD, 0x1111_2222, 0x3333_4444]);
        let value = V128::from_u32x4([0xDEAD_BEEF, 0xCAFE_BABE, 0x1234_5678, 0x9ABC_DEF0]);
        
        unsafe {
            atomic_store128(&mut storage as *mut V128, &value);
            let loaded = atomic_load128(&storage as *const V128);
            assert_eq!(loaded.to_u32x4(), value.to_u32x4());
        }
    }

    #[test]
    fn test_atomic_cas128_success() {
        let mut storage = V128::from_u32x4([1, 2, 3, 4]);
        let mut expected = V128::from_u32x4([1, 2, 3, 4]);
        let desired = V128::from_u32x4([5, 6, 7, 8]);
        
        unsafe {
            let success = atomic_cas128(&mut storage as *mut V128, &mut expected, &desired);
            assert!(success, "CAS should succeed when expected matches");
            let loaded = atomic_load128(&storage as *const V128);
            assert_eq!(loaded.to_u32x4(), [5, 6, 7, 8]);
        }
    }

    #[test]
    fn test_atomic_cas128_failure() {
        let mut storage = V128::from_u32x4([1, 2, 3, 4]);
        let mut expected = V128::from_u32x4([5, 6, 7, 8]);  // Wrong expected value
        let desired = V128::from_u32x4([9, 10, 11, 12]);
        
        unsafe {
            let success = atomic_cas128(&mut storage as *mut V128, &mut expected, &desired);
            assert!(!success, "CAS should fail when expected doesn't match");
            // expected should be updated to the current value
            assert_eq!(expected.to_u32x4(), [1, 2, 3, 4]);
            // storage should be unchanged
            let loaded = atomic_load128(&storage as *const V128);
            assert_eq!(loaded.to_u32x4(), [1, 2, 3, 4]);
        }
    }
}
