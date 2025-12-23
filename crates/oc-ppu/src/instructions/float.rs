//! Floating-point instructions for PPU
//!
//! This module contains implementations for PowerPC floating-point
//! arithmetic, comparison, and conversion instructions.

use crate::thread::PpuThread;

/// FPSCR bit positions
pub mod fpscr {
    pub const FX: u64 = 0x8000_0000_0000_0000;   // FP Exception Summary
    pub const FEX: u64 = 0x4000_0000_0000_0000;  // FP Enabled Exception Summary
    pub const VX: u64 = 0x2000_0000_0000_0000;   // FP Invalid Operation Exception Summary
    pub const OX: u64 = 0x1000_0000_0000_0000;   // FP Overflow Exception
    pub const UX: u64 = 0x0800_0000_0000_0000;   // FP Underflow Exception
    pub const ZX: u64 = 0x0400_0000_0000_0000;   // FP Zero Divide Exception
    pub const XX: u64 = 0x0200_0000_0000_0000;   // FP Inexact Exception
    pub const VXSNAN: u64 = 0x0100_0000_0000_0000; // FP Invalid Op (SNaN)
    pub const VXISI: u64 = 0x0080_0000_0000_0000;  // FP Invalid Op (∞ - ∞)
    pub const VXIDI: u64 = 0x0040_0000_0000_0000;  // FP Invalid Op (∞ / ∞)
    pub const VXZDZ: u64 = 0x0020_0000_0000_0000;  // FP Invalid Op (0 / 0)
    pub const VXIMZ: u64 = 0x0010_0000_0000_0000;  // FP Invalid Op (∞ * 0)
    pub const VXVC: u64 = 0x0008_0000_0000_0000;   // FP Invalid Op (Invalid Compare)
    pub const FR: u64 = 0x0004_0000_0000_0000;     // FP Fraction Rounded
    pub const FI: u64 = 0x0002_0000_0000_0000;     // FP Fraction Inexact
    
    /// Rounding mode mask (bits 62-63)
    pub const RN_MASK: u64 = 0x0000_0000_0000_0003;
}

/// Rounding modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundingMode {
    /// Round to nearest, ties to even
    RoundToNearest = 0,
    /// Round toward zero
    RoundToZero = 1,
    /// Round toward +∞
    RoundToPositiveInfinity = 2,
    /// Round toward -∞
    RoundToNegativeInfinity = 3,
}

impl From<u64> for RoundingMode {
    fn from(value: u64) -> Self {
        match value & 3 {
            0 => RoundingMode::RoundToNearest,
            1 => RoundingMode::RoundToZero,
            2 => RoundingMode::RoundToPositiveInfinity,
            3 => RoundingMode::RoundToNegativeInfinity,
            _ => unreachable!(),
        }
    }
}

/// Floating-point class
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FpClass {
    QuietNaN,
    NegativeInfinity,
    NegativeNormalized,
    NegativeDenormalized,
    NegativeZero,
    PositiveZero,
    PositiveDenormalized,
    PositiveNormalized,
    PositiveInfinity,
    SignalingNaN,
}

/// Classify a 64-bit floating-point value
pub fn classify_f64(value: f64) -> FpClass {
    let bits = value.to_bits();
    let sign = (bits >> 63) != 0;
    let exp = ((bits >> 52) & 0x7FF) as u16;
    let frac = bits & 0x000F_FFFF_FFFF_FFFF;
    
    match (sign, exp, frac) {
        (_, 0x7FF, 0) if sign => FpClass::NegativeInfinity,
        (_, 0x7FF, 0) => FpClass::PositiveInfinity,
        (_, 0x7FF, f) if (f >> 51) != 0 => FpClass::QuietNaN,
        (_, 0x7FF, _) => FpClass::SignalingNaN,
        (true, 0, 0) => FpClass::NegativeZero,
        (false, 0, 0) => FpClass::PositiveZero,
        (true, 0, _) => FpClass::NegativeDenormalized,
        (false, 0, _) => FpClass::PositiveDenormalized,
        (true, _, _) => FpClass::NegativeNormalized,
        (false, _, _) => FpClass::PositiveNormalized,
    }
}

/// Get the FPRF (Floating-Point Result Flags) for a value
pub fn get_fprf(value: f64) -> u32 {
    match classify_f64(value) {
        FpClass::QuietNaN => 0b10001,
        FpClass::NegativeInfinity => 0b01001,
        FpClass::NegativeNormalized => 0b01000,
        FpClass::NegativeDenormalized => 0b11000,
        FpClass::NegativeZero => 0b10010,
        FpClass::PositiveZero => 0b00010,
        FpClass::PositiveDenormalized => 0b10100,
        FpClass::PositiveNormalized => 0b00100,
        FpClass::PositiveInfinity => 0b00101,
        FpClass::SignalingNaN => 0b10001,
    }
}

/// Update FPSCR FPRF field based on result
pub fn update_fprf(thread: &mut PpuThread, value: f64) {
    let fprf = get_fprf(value);
    // FPRF is in bits 47-51 of FPSCR (counting from bit 0 at the left in PowerPC)
    // In our 64-bit representation, this is bits 12-16 from the right
    thread.regs.fpscr = (thread.regs.fpscr & !0x0001_F000) | ((fprf as u64) << 12);
}

/// Update CR1 based on FPSCR exception bits
pub fn update_cr1(thread: &mut PpuThread) {
    let fpscr = thread.regs.fpscr;
    let cr1 = ((fpscr >> 60) & 0xF) as u32;
    thread.set_cr_field(1, cr1);
}

/// Floating-point compare result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FpCompareResult {
    Less,
    Greater,
    Equal,
    Unordered,
}

impl FpCompareResult {
    /// Convert to CR field value
    pub fn to_cr(self) -> u32 {
        match self {
            FpCompareResult::Less => 0b1000,
            FpCompareResult::Greater => 0b0100,
            FpCompareResult::Equal => 0b0010,
            FpCompareResult::Unordered => 0b0001,
        }
    }
}

/// Compare two floating-point values
pub fn compare_f64(a: f64, b: f64) -> FpCompareResult {
    if a.is_nan() || b.is_nan() {
        FpCompareResult::Unordered
    } else if a < b {
        FpCompareResult::Less
    } else if a > b {
        FpCompareResult::Greater
    } else {
        FpCompareResult::Equal
    }
}

/// Fused multiply-add: (a * c) + b
#[inline]
pub fn fmadd(a: f64, c: f64, b: f64) -> f64 {
    a.mul_add(c, b)
}

/// Fused multiply-subtract: (a * c) - b
#[inline]
pub fn fmsub(a: f64, c: f64, b: f64) -> f64 {
    a.mul_add(c, -b)
}

/// Fused negative multiply-add: -((a * c) + b)
#[inline]
pub fn fnmadd(a: f64, c: f64, b: f64) -> f64 {
    -a.mul_add(c, b)
}

/// Fused negative multiply-subtract: -((a * c) - b)
#[inline]
pub fn fnmsub(a: f64, c: f64, b: f64) -> f64 {
    -a.mul_add(c, -b)
}

/// Convert double to single precision
#[inline]
pub fn frsp(value: f64) -> f64 {
    (value as f32) as f64
}

/// Convert to integer word (toward zero)
#[inline]
pub fn fctiwz(value: f64) -> u64 {
    let result = if value.is_nan() {
        0x8000_0000u32
    } else if value >= (i32::MAX as f64) {
        0x7FFF_FFFFu32
    } else if value <= (i32::MIN as f64) {
        0x8000_0000u32
    } else {
        (value as i32) as u32
    };
    result as u64
}

/// Convert to integer doubleword (toward zero)
#[inline]
pub fn fctidz(value: f64) -> u64 {
    if value.is_nan() {
        0x8000_0000_0000_0000u64
    } else if value >= (i64::MAX as f64) {
        0x7FFF_FFFF_FFFF_FFFFu64
    } else if value <= (i64::MIN as f64) {
        0x8000_0000_0000_0000u64
    } else {
        (value as i64) as u64
    }
}

/// Convert from integer doubleword
#[inline]
pub fn fcfid(value: u64) -> f64 {
    (value as i64) as f64
}

/// Reciprocal estimate
#[inline]
pub fn fre(value: f64) -> f64 {
    1.0 / value
}

/// Reciprocal square root estimate
#[inline]
pub fn frsqrte(value: f64) -> f64 {
    1.0 / value.sqrt()
}

/// Select (a >= 0 ? c : b)
#[inline]
pub fn fsel(a: f64, b: f64, c: f64) -> f64 {
    if a >= 0.0 { c } else { b }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_f64() {
        assert_eq!(classify_f64(0.0), FpClass::PositiveZero);
        assert_eq!(classify_f64(-0.0), FpClass::NegativeZero);
        assert_eq!(classify_f64(1.0), FpClass::PositiveNormalized);
        assert_eq!(classify_f64(-1.0), FpClass::NegativeNormalized);
        assert_eq!(classify_f64(f64::INFINITY), FpClass::PositiveInfinity);
        assert_eq!(classify_f64(f64::NEG_INFINITY), FpClass::NegativeInfinity);
        assert_eq!(classify_f64(f64::NAN), FpClass::QuietNaN);
    }

    #[test]
    fn test_compare_f64() {
        assert_eq!(compare_f64(1.0, 2.0), FpCompareResult::Less);
        assert_eq!(compare_f64(2.0, 1.0), FpCompareResult::Greater);
        assert_eq!(compare_f64(1.0, 1.0), FpCompareResult::Equal);
        assert_eq!(compare_f64(f64::NAN, 1.0), FpCompareResult::Unordered);
    }

    #[test]
    fn test_fmadd() {
        let result = fmadd(2.0, 3.0, 4.0);
        assert_eq!(result, 10.0); // 2 * 3 + 4
    }

    #[test]
    fn test_fctiwz() {
        assert_eq!(fctiwz(1.5) as u32, 1);
        assert_eq!(fctiwz(-1.5) as u32, (-1i32) as u32);
        assert_eq!(fctiwz(f64::NAN) as u32, 0x8000_0000);
    }

    #[test]
    fn test_fsel() {
        assert_eq!(fsel(1.0, 10.0, 20.0), 20.0);
        assert_eq!(fsel(-1.0, 10.0, 20.0), 10.0);
        assert_eq!(fsel(0.0, 10.0, 20.0), 20.0);
    }
}
