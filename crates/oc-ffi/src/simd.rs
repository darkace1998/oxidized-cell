//! SIMD helper interface
//!
//! Safe Rust wrappers for SIMD-accelerated 128-bit vector operations.
//! Automatically selects AVX2, SSE4.2, or scalar fallback at runtime.

use crate::types::V128;
use std::ffi::CStr;

extern "C" {
    fn oc_simd_get_level() -> i32;
    fn oc_simd_get_level_name() -> *const std::os::raw::c_char;
    fn oc_simd_vec_add(result: *mut V128, a: *const V128, b: *const V128);
    fn oc_simd_vec_sub(result: *mut V128, a: *const V128, b: *const V128);
    fn oc_simd_vec_and(result: *mut V128, a: *const V128, b: *const V128);
    fn oc_simd_vec_or(result: *mut V128, a: *const V128, b: *const V128);
    fn oc_simd_vec_xor(result: *mut V128, a: *const V128, b: *const V128);
    fn oc_simd_vec_shufb(result: *mut V128, a: *const V128, b: *const V128, pattern: *const V128);
    fn oc_simd_vec_cmpeq(result: *mut V128, a: *const V128, b: *const V128);
    fn oc_simd_vec_cmpgt(result: *mut V128, a: *const V128, b: *const V128);
    fn oc_simd_vec_fadd(result: *mut V128, a: *const V128, b: *const V128);
    fn oc_simd_vec_fsub(result: *mut V128, a: *const V128, b: *const V128);
    fn oc_simd_vec_fmul(result: *mut V128, a: *const V128, b: *const V128);
}

/// SIMD acceleration level detected at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum SimdLevel {
    /// No SIMD — scalar fallback
    Scalar = 0,
    /// SSE4.2 (x86_64)
    Sse42 = 1,
    /// AVX2 (x86_64)
    Avx2 = 2,
}

impl From<i32> for SimdLevel {
    fn from(value: i32) -> Self {
        match value {
            2 => SimdLevel::Avx2,
            1 => SimdLevel::Sse42,
            _ => SimdLevel::Scalar,
        }
    }
}

/// Get the detected SIMD acceleration level.
pub fn get_simd_level() -> SimdLevel {
    SimdLevel::from(unsafe { oc_simd_get_level() })
}

/// Get the human-readable name of the detected SIMD level.
pub fn get_simd_level_name() -> &'static str {
    unsafe {
        let ptr = oc_simd_get_level_name();
        CStr::from_ptr(ptr).to_str().unwrap_or("Unknown")
    }
}

/// Vector add: result = a + b (4 x int32)
pub fn vec_add(a: &V128, b: &V128) -> V128 {
    let mut result = V128::new();
    unsafe { oc_simd_vec_add(&mut result, a, b) };
    result
}

/// Vector sub: result = a - b (4 x int32)
pub fn vec_sub(a: &V128, b: &V128) -> V128 {
    let mut result = V128::new();
    unsafe { oc_simd_vec_sub(&mut result, a, b) };
    result
}

/// Vector AND: result = a & b
pub fn vec_and(a: &V128, b: &V128) -> V128 {
    let mut result = V128::new();
    unsafe { oc_simd_vec_and(&mut result, a, b) };
    result
}

/// Vector OR: result = a | b
pub fn vec_or(a: &V128, b: &V128) -> V128 {
    let mut result = V128::new();
    unsafe { oc_simd_vec_or(&mut result, a, b) };
    result
}

/// Vector XOR: result = a ^ b
pub fn vec_xor(a: &V128, b: &V128) -> V128 {
    let mut result = V128::new();
    unsafe { oc_simd_vec_xor(&mut result, a, b) };
    result
}

/// SPU SHUFB (shuffle bytes): select bytes from {a||b} based on pattern.
pub fn vec_shufb(a: &V128, b: &V128, pattern: &V128) -> V128 {
    let mut result = V128::new();
    unsafe { oc_simd_vec_shufb(&mut result, a, b, pattern) };
    result
}

/// Vector compare equal: result = (a == b) ? 0xFFFFFFFF : 0 (4 x int32)
pub fn vec_cmpeq(a: &V128, b: &V128) -> V128 {
    let mut result = V128::new();
    unsafe { oc_simd_vec_cmpeq(&mut result, a, b) };
    result
}

/// Vector compare greater than (signed): result = (a > b) ? 0xFFFFFFFF : 0 (4 x int32)
pub fn vec_cmpgt(a: &V128, b: &V128) -> V128 {
    let mut result = V128::new();
    unsafe { oc_simd_vec_cmpgt(&mut result, a, b) };
    result
}

/// Vector float add: result = a + b (4 x float32)
pub fn vec_fadd(a: &V128, b: &V128) -> V128 {
    let mut result = V128::new();
    unsafe { oc_simd_vec_fadd(&mut result, a, b) };
    result
}

/// Vector float sub: result = a - b (4 x float32)
pub fn vec_fsub(a: &V128, b: &V128) -> V128 {
    let mut result = V128::new();
    unsafe { oc_simd_vec_fsub(&mut result, a, b) };
    result
}

/// Vector float mul: result = a * b (4 x float32)
pub fn vec_fmul(a: &V128, b: &V128) -> V128 {
    let mut result = V128::new();
    unsafe { oc_simd_vec_fmul(&mut result, a, b) };
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_level_detection() {
        let level = get_simd_level();
        let name = get_simd_level_name();
        // On any modern x86_64, should be at least SSE4.2
        assert!(
            matches!(level, SimdLevel::Scalar | SimdLevel::Sse42 | SimdLevel::Avx2),
            "SIMD level should be valid: {:?} ({})", level, name
        );
    }

    #[test]
    fn test_vec_add() {
        let a = V128::from_u32x4([1, 2, 3, 4]);
        let b = V128::from_u32x4([10, 20, 30, 40]);
        let result = vec_add(&a, &b);
        assert_eq!(result.to_u32x4(), [11, 22, 33, 44]);
    }

    #[test]
    fn test_vec_sub() {
        let a = V128::from_u32x4([10, 20, 30, 40]);
        let b = V128::from_u32x4([1, 2, 3, 4]);
        let result = vec_sub(&a, &b);
        assert_eq!(result.to_u32x4(), [9, 18, 27, 36]);
    }

    #[test]
    fn test_vec_and() {
        let a = V128::from_u32x4([0xFF00FF00, 0xFF00FF00, 0xFF00FF00, 0xFF00FF00]);
        let b = V128::from_u32x4([0x0F0F0F0F, 0x0F0F0F0F, 0x0F0F0F0F, 0x0F0F0F0F]);
        let result = vec_and(&a, &b);
        assert_eq!(result.to_u32x4(), [0x0F000F00, 0x0F000F00, 0x0F000F00, 0x0F000F00]);
    }

    #[test]
    fn test_vec_or() {
        let a = V128::from_u32x4([0xFF000000, 0, 0, 0]);
        let b = V128::from_u32x4([0x00FF0000, 0, 0, 0]);
        let result = vec_or(&a, &b);
        assert_eq!(result.to_u32x4()[0], 0xFFFF0000);
    }

    #[test]
    fn test_vec_xor() {
        let a = V128::from_u32x4([0xFFFF_FFFF, 0, 0xAAAA_AAAA, 0]);
        let b = V128::from_u32x4([0xFFFF_FFFF, 0xFFFF_FFFF, 0x5555_5555, 0]);
        let result = vec_xor(&a, &b);
        assert_eq!(result.to_u32x4(), [0, 0xFFFF_FFFF, 0xFFFF_FFFF, 0]);
    }

    #[test]
    fn test_vec_cmpeq() {
        let a = V128::from_u32x4([1, 2, 3, 4]);
        let b = V128::from_u32x4([1, 99, 3, 99]);
        let result = vec_cmpeq(&a, &b);
        let vals = result.to_u32x4();
        assert_eq!(vals[0], 0xFFFF_FFFF);
        assert_eq!(vals[1], 0);
        assert_eq!(vals[2], 0xFFFF_FFFF);
        assert_eq!(vals[3], 0);
    }

    #[test]
    fn test_vec_cmpgt() {
        let a = V128::from_u32x4([5, 2, 10, 4]);
        let b = V128::from_u32x4([3, 2, 10, 5]);
        let result = vec_cmpgt(&a, &b);
        let vals = result.to_u32x4();
        assert_eq!(vals[0], 0xFFFF_FFFF);  // 5 > 3
        assert_eq!(vals[1], 0);             // 2 == 2
        assert_eq!(vals[2], 0);             // 10 == 10
        // vals[3]: 4 vs 5 signed — 4 is NOT > 5
        assert_eq!(vals[3], 0);
    }

    #[test]
    fn test_vec_fadd() {
        let a = V128::from_u32x4([
            f32::to_bits(1.0), f32::to_bits(2.0), f32::to_bits(3.0), f32::to_bits(4.0)
        ]);
        let b = V128::from_u32x4([
            f32::to_bits(0.5), f32::to_bits(0.25), f32::to_bits(0.125), f32::to_bits(0.0625)
        ]);
        let result = vec_fadd(&a, &b);
        let vals = result.to_u32x4();
        assert_eq!(f32::from_bits(vals[0]), 1.5);
        assert_eq!(f32::from_bits(vals[1]), 2.25);
        assert_eq!(f32::from_bits(vals[2]), 3.125);
        assert_eq!(f32::from_bits(vals[3]), 4.0625);
    }

    #[test]
    fn test_vec_shufb_identity() {
        // Identity pattern: select bytes 0-15 from a
        let a = V128::from_u32x4([0x03020100, 0x07060504, 0x0B0A0908, 0x0F0E0D0C]);
        let b = V128::new();
        let pattern = V128::from_u32x4([0x03020100, 0x07060504, 0x0B0A0908, 0x0F0E0D0C]);
        let result = vec_shufb(&a, &b, &pattern);
        assert_eq!(result.to_u32x4(), a.to_u32x4());
    }
}
