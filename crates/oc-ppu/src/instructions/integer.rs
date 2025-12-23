//! Integer arithmetic instructions for PPU
//!
//! This module contains implementations for PowerPC integer arithmetic,
//! logical, shift, and rotate instructions.

use crate::thread::PpuThread;

/// Sign extend a value from a given bit width
#[inline]
pub fn sign_extend(value: u64, bits: u32) -> i64 {
    let shift = 64 - bits;
    ((value as i64) << shift) >> shift
}

/// Carry out for add operation
#[inline]
pub fn add_carry(a: u64, b: u64) -> bool {
    a.checked_add(b).is_none()
}

/// Carry out for add with carry operation
#[inline]
pub fn add_with_carry(a: u64, b: u64, carry_in: bool) -> (u64, bool) {
    let (result1, carry1) = a.overflowing_add(b);
    let (result2, carry2) = result1.overflowing_add(carry_in as u64);
    (result2, carry1 || carry2)
}

/// Overflow for signed add
#[inline]
pub fn add_overflow_64(a: i64, b: i64) -> bool {
    a.checked_add(b).is_none()
}

/// Overflow for signed add (32-bit)
#[inline]
pub fn add_overflow_32(a: i32, b: i32) -> bool {
    a.checked_add(b).is_none()
}

/// Overflow for signed subtract
#[inline]
pub fn sub_overflow_64(a: i64, b: i64) -> bool {
    a.checked_sub(b).is_none()
}

/// Overflow for signed subtract (32-bit)
#[inline]
pub fn sub_overflow_32(a: i32, b: i32) -> bool {
    a.checked_sub(b).is_none()
}

/// Update CR0 based on 64-bit result
#[inline]
pub fn update_cr0_64(thread: &mut PpuThread, value: u64) {
    let value = value as i64;
    let c = if value < 0 { 0b1000 } else if value > 0 { 0b0100 } else { 0b0010 };
    let c = c | if thread.get_xer_so() { 1 } else { 0 };
    thread.set_cr_field(0, c);
}

/// Update CR0 based on 32-bit result (sign extended comparison)
#[inline]
pub fn update_cr0_32(thread: &mut PpuThread, value: u32) {
    let value = value as i32;
    let c = if value < 0 { 0b1000 } else if value > 0 { 0b0100 } else { 0b0010 };
    let c = c | if thread.get_xer_so() { 1 } else { 0 };
    thread.set_cr_field(0, c as u32);
}

/// Count leading zeros (64-bit)
#[inline]
pub fn count_leading_zeros_64(value: u64) -> u64 {
    value.leading_zeros() as u64
}

/// Count leading zeros (32-bit)
#[inline]
pub fn count_leading_zeros_32(value: u32) -> u32 {
    value.leading_zeros()
}

/// Population count (number of 1 bits)
#[inline]
pub fn population_count_64(value: u64) -> u64 {
    value.count_ones() as u64
}

/// Generate 64-bit mask for rotate instructions
#[inline]
pub fn generate_mask_64(mb: u8, me: u8) -> u64 {
    let mb = mb as u32;
    let me = me as u32;
    if mb <= me {
        (u64::MAX >> mb) & (u64::MAX << (63 - me))
    } else {
        (u64::MAX >> mb) | (u64::MAX << (63 - me))
    }
}

/// Generate 32-bit mask for rotate instructions
#[inline]
pub fn generate_mask_32(mb: u8, me: u8) -> u32 {
    let mb = mb as u32;
    let me = me as u32;
    if mb <= me {
        (u32::MAX >> mb) & (u32::MAX << (31 - me))
    } else {
        (u32::MAX >> mb) | (u32::MAX << (31 - me))
    }
}

/// Byte reverse word
#[inline]
pub fn byte_reverse_word(value: u32) -> u32 {
    value.swap_bytes()
}

/// Byte reverse doubleword
#[inline]
pub fn byte_reverse_doubleword(value: u64) -> u64 {
    value.swap_bytes()
}

/// Byte reverse halfword
#[inline]
pub fn byte_reverse_halfword(value: u16) -> u16 {
    value.swap_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_extend() {
        // Sign extend from 16 bits
        assert_eq!(sign_extend(0x7FFF, 16), 0x7FFF);
        assert_eq!(sign_extend(0x8000, 16), -32768);
        assert_eq!(sign_extend(0xFFFF, 16), -1);
    }

    #[test]
    fn test_add_carry() {
        assert!(!add_carry(1, 2));
        assert!(add_carry(u64::MAX, 1));
        assert!(add_carry(u64::MAX, u64::MAX));
    }

    #[test]
    fn test_generate_mask_64() {
        assert_eq!(generate_mask_64(0, 63), u64::MAX);
        assert_eq!(generate_mask_64(32, 63), 0x00000000_FFFFFFFF);
        assert_eq!(generate_mask_64(0, 31), 0xFFFFFFFF_00000000);
    }

    #[test]
    fn test_count_leading_zeros() {
        assert_eq!(count_leading_zeros_64(0), 64);
        assert_eq!(count_leading_zeros_64(1), 63);
        assert_eq!(count_leading_zeros_64(0x8000_0000_0000_0000), 0);
    }

    #[test]
    fn test_byte_reverse() {
        assert_eq!(byte_reverse_word(0x12345678), 0x78563412);
        assert_eq!(byte_reverse_doubleword(0x0102030405060708), 0x0807060504030201);
    }
}
