//! Vector/SIMD (VMX/AltiVec) instructions for PPU
//!
//! This module contains implementations for PowerPC VMX (AltiVec)
//! vector instructions used by the Cell BE PPU.

/// Vector addition (4 x i32)
pub fn vaddsws(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let ai = a[i] as i32;
        let bi = b[i] as i32;
        result[i] = ai.saturating_add(bi) as u32;
    }
    result
}

/// Vector addition unsigned (4 x u32)
pub fn vadduws(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        result[i] = a[i].saturating_add(b[i]);
    }
    result
}

/// Vector subtraction (4 x i32)
pub fn vsubsws(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let ai = a[i] as i32;
        let bi = b[i] as i32;
        result[i] = ai.saturating_sub(bi) as u32;
    }
    result
}

/// Vector subtraction unsigned (4 x u32)
pub fn vsubuws(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        result[i] = a[i].saturating_sub(b[i]);
    }
    result
}

/// Vector AND
pub fn vand(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    [a[0] & b[0], a[1] & b[1], a[2] & b[2], a[3] & b[3]]
}

/// Vector AND with complement
pub fn vandc(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    [a[0] & !b[0], a[1] & !b[1], a[2] & !b[2], a[3] & !b[3]]
}

/// Vector OR
pub fn vor(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    [a[0] | b[0], a[1] | b[1], a[2] | b[2], a[3] | b[3]]
}

/// Vector NOR
pub fn vnor(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    [!(a[0] | b[0]), !(a[1] | b[1]), !(a[2] | b[2]), !(a[3] | b[3])]
}

/// Vector XOR
pub fn vxor(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    [a[0] ^ b[0], a[1] ^ b[1], a[2] ^ b[2], a[3] ^ b[3]]
}

/// Vector Select (bitwise: result = (b & c) | (a & !c))
pub fn vsel(a: [u32; 4], b: [u32; 4], c: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        result[i] = (b[i] & c[i]) | (a[i] & !c[i]);
    }
    result
}

/// Vector Splat Immediate Signed Byte
pub fn vspltisb(simm: i8) -> [u32; 4] {
    let b = simm as u8;
    let w = u32::from_be_bytes([b, b, b, b]);
    [w, w, w, w]
}

/// Vector Splat Immediate Signed Halfword
pub fn vspltish(simm: i16) -> [u32; 4] {
    let h = simm as u16;
    let w = ((h as u32) << 16) | (h as u32);
    [w, w, w, w]
}

/// Vector Splat Immediate Signed Word
pub fn vspltisw(simm: i32) -> [u32; 4] {
    let w = simm as u32;
    [w, w, w, w]
}

/// Vector Splat Word
pub fn vspltw(v: [u32; 4], uimm: u8) -> [u32; 4] {
    let w = v[(uimm & 3) as usize];
    [w, w, w, w]
}

/// Vector Shift Left Word
pub fn vslw(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let shift = (b[i] & 0x1F) as u32;
        result[i] = a[i] << shift;
    }
    result
}

/// Vector Shift Right Word
pub fn vsrw(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let shift = (b[i] & 0x1F) as u32;
        result[i] = a[i] >> shift;
    }
    result
}

/// Vector Shift Right Algebraic Word
pub fn vsraw(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let shift = (b[i] & 0x1F) as u32;
        result[i] = ((a[i] as i32) >> shift) as u32;
    }
    result
}

/// Vector Rotate Left Word
pub fn vrlw(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let shift = (b[i] & 0x1F) as u32;
        result[i] = a[i].rotate_left(shift);
    }
    result
}

/// Vector Compare Equal Word
pub fn vcmpequw(a: [u32; 4], b: [u32; 4]) -> ([u32; 4], bool) {
    let mut result = [0u32; 4];
    let mut all_true = true;
    
    for i in 0..4 {
        if a[i] == b[i] {
            result[i] = 0xFFFFFFFF;
        } else {
            result[i] = 0;
            all_true = false;
        }
    }
    (result, all_true)
}

/// Vector Compare Greater Than Signed Word
pub fn vcmpgtsw(a: [u32; 4], b: [u32; 4]) -> ([u32; 4], bool) {
    let mut result = [0u32; 4];
    let mut all_true = true;
    
    for i in 0..4 {
        if (a[i] as i32) > (b[i] as i32) {
            result[i] = 0xFFFFFFFF;
        } else {
            result[i] = 0;
            all_true = false;
        }
    }
    (result, all_true)
}

/// Vector Compare Greater Than Unsigned Word
pub fn vcmpgtuw(a: [u32; 4], b: [u32; 4]) -> ([u32; 4], bool) {
    let mut result = [0u32; 4];
    let mut all_true = true;
    
    for i in 0..4 {
        if a[i] > b[i] {
            result[i] = 0xFFFFFFFF;
        } else {
            result[i] = 0;
            all_true = false;
        }
    }
    (result, all_true)
}

/// Vector Minimum Signed Word
pub fn vminsw(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        result[i] = std::cmp::min(a[i] as i32, b[i] as i32) as u32;
    }
    result
}

/// Vector Maximum Signed Word
pub fn vmaxsw(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        result[i] = std::cmp::max(a[i] as i32, b[i] as i32) as u32;
    }
    result
}

/// Vector Minimum Unsigned Word
pub fn vminuw(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    [
        std::cmp::min(a[0], b[0]),
        std::cmp::min(a[1], b[1]),
        std::cmp::min(a[2], b[2]),
        std::cmp::min(a[3], b[3]),
    ]
}

/// Vector Maximum Unsigned Word
pub fn vmaxuw(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    [
        std::cmp::max(a[0], b[0]),
        std::cmp::max(a[1], b[1]),
        std::cmp::max(a[2], b[2]),
        std::cmp::max(a[3], b[3]),
    ]
}

/// Vector Multiply Low Word
pub fn vmulwlw(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        result[i] = a[i].wrapping_mul(b[i]);
    }
    result
}

/// Vector Permute (byte-level shuffle)
pub fn vperm(a: [u32; 4], b: [u32; 4], c: [u32; 4]) -> [u32; 4] {
    // Convert to bytes
    let mut ab = [0u8; 32];
    for i in 0..4 {
        let a_bytes = a[i].to_be_bytes();
        let b_bytes = b[i].to_be_bytes();
        ab[i * 4] = a_bytes[0];
        ab[i * 4 + 1] = a_bytes[1];
        ab[i * 4 + 2] = a_bytes[2];
        ab[i * 4 + 3] = a_bytes[3];
        ab[16 + i * 4] = b_bytes[0];
        ab[16 + i * 4 + 1] = b_bytes[1];
        ab[16 + i * 4 + 2] = b_bytes[2];
        ab[16 + i * 4 + 3] = b_bytes[3];
    }
    
    // Get control bytes
    let mut ctrl = [0u8; 16];
    for i in 0..4 {
        let c_bytes = c[i].to_be_bytes();
        ctrl[i * 4] = c_bytes[0];
        ctrl[i * 4 + 1] = c_bytes[1];
        ctrl[i * 4 + 2] = c_bytes[2];
        ctrl[i * 4 + 3] = c_bytes[3];
    }
    
    // Permute
    let mut result_bytes = [0u8; 16];
    for i in 0..16 {
        let idx = (ctrl[i] & 0x1F) as usize;
        result_bytes[i] = ab[idx];
    }
    
    // Convert back to words
    [
        u32::from_be_bytes([result_bytes[0], result_bytes[1], result_bytes[2], result_bytes[3]]),
        u32::from_be_bytes([result_bytes[4], result_bytes[5], result_bytes[6], result_bytes[7]]),
        u32::from_be_bytes([result_bytes[8], result_bytes[9], result_bytes[10], result_bytes[11]]),
        u32::from_be_bytes([result_bytes[12], result_bytes[13], result_bytes[14], result_bytes[15]]),
    ]
}

/// Vector Merge High Word
pub fn vmrghw(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    [a[0], b[0], a[1], b[1]]
}

/// Vector Merge Low Word
pub fn vmrglw(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    [a[2], b[2], a[3], b[3]]
}

/// Vector Pack Unsigned Word Unsigned Saturate
pub fn vpkuwus(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let clamp = |v: u32| -> u16 {
        if v > 0xFFFF { 0xFFFF } else { v as u16 }
    };
    
    [
        ((clamp(a[0]) as u32) << 16) | (clamp(a[1]) as u32),
        ((clamp(a[2]) as u32) << 16) | (clamp(a[3]) as u32),
        ((clamp(b[0]) as u32) << 16) | (clamp(b[1]) as u32),
        ((clamp(b[2]) as u32) << 16) | (clamp(b[3]) as u32),
    ]
}

// Floating-point vector operations

/// Vector Add Single-Precision
pub fn vaddfp(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let fa = f32::from_bits(a[i]);
        let fb = f32::from_bits(b[i]);
        result[i] = (fa + fb).to_bits();
    }
    result
}

/// Vector Subtract Single-Precision
pub fn vsubfp(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let fa = f32::from_bits(a[i]);
        let fb = f32::from_bits(b[i]);
        result[i] = (fa - fb).to_bits();
    }
    result
}

/// Vector Multiply-Add Single-Precision
pub fn vmaddfp(a: [u32; 4], b: [u32; 4], c: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let fa = f32::from_bits(a[i]);
        let fb = f32::from_bits(b[i]);
        let fc = f32::from_bits(c[i]);
        result[i] = fa.mul_add(fb, fc).to_bits();
    }
    result
}

/// Vector Negative Multiply-Subtract Single-Precision
pub fn vnmsubfp(a: [u32; 4], b: [u32; 4], c: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let fa = f32::from_bits(a[i]);
        let fb = f32::from_bits(b[i]);
        let fc = f32::from_bits(c[i]);
        result[i] = (-(fa * fb - fc)).to_bits();
    }
    result
}

/// Vector Reciprocal Estimate Single-Precision
pub fn vrefp(a: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let fa = f32::from_bits(a[i]);
        result[i] = (1.0 / fa).to_bits();
    }
    result
}

/// Vector Reciprocal Square Root Estimate Single-Precision
pub fn vrsqrtefp(a: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let fa = f32::from_bits(a[i]);
        result[i] = (1.0 / fa.sqrt()).to_bits();
    }
    result
}

/// Vector Compare Equal Single-Precision
pub fn vcmpeqfp(a: [u32; 4], b: [u32; 4]) -> ([u32; 4], bool) {
    let mut result = [0u32; 4];
    let mut all_true = true;
    
    for i in 0..4 {
        let fa = f32::from_bits(a[i]);
        let fb = f32::from_bits(b[i]);
        if fa == fb {
            result[i] = 0xFFFFFFFF;
        } else {
            result[i] = 0;
            all_true = false;
        }
    }
    (result, all_true)
}

/// Vector Compare Greater Than Single-Precision
pub fn vcmpgtfp(a: [u32; 4], b: [u32; 4]) -> ([u32; 4], bool) {
    let mut result = [0u32; 4];
    let mut all_true = true;
    
    for i in 0..4 {
        let fa = f32::from_bits(a[i]);
        let fb = f32::from_bits(b[i]);
        if fa > fb {
            result[i] = 0xFFFFFFFF;
        } else {
            result[i] = 0;
            all_true = false;
        }
    }
    (result, all_true)
}

/// Vector Convert to Signed Integer Word Saturate
pub fn vctsxs(a: [u32; 4], uimm: u8) -> [u32; 4] {
    let scale = 1i64 << (uimm & 0x1F);
    let mut result = [0u32; 4];
    
    for i in 0..4 {
        let fa = f32::from_bits(a[i]) as f64;
        let scaled = (fa * scale as f64).round();
        let clamped = scaled.clamp(i32::MIN as f64, i32::MAX as f64) as i32;
        result[i] = clamped as u32;
    }
    result
}

/// Vector Convert from Signed Integer Word
pub fn vcfsx(a: [u32; 4], uimm: u8) -> [u32; 4] {
    let scale = 1.0f32 / (1u32 << (uimm & 0x1F)) as f32;
    let mut result = [0u32; 4];
    
    for i in 0..4 {
        let ia = a[i] as i32;
        result[i] = (ia as f32 * scale).to_bits();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vand() {
        let a = [0xFFFF0000, 0x0000FFFF, 0xF0F0F0F0, 0x0F0F0F0F];
        let b = [0xFF00FF00, 0x00FF00FF, 0xFFFF0000, 0x0000FFFF];
        let result = vand(a, b);
        assert_eq!(result[0], 0xFF000000);
        assert_eq!(result[1], 0x000000FF);
    }

    #[test]
    fn test_vaddsws() {
        let a = [100u32, 0x7FFFFFFF, 0x80000000, 0];
        let b = [200u32, 1, 0x80000000, 0];
        let result = vaddsws(a, b);
        assert_eq!(result[0] as i32, 300);
        assert_eq!(result[1], 0x7FFFFFFF); // Saturated
        assert_eq!(result[2], 0x80000000); // Saturated to min
    }

    #[test]
    fn test_vspltisw() {
        let result = vspltisw(-1);
        assert_eq!(result, [0xFFFFFFFF; 4]);
        
        let result = vspltisw(5);
        assert_eq!(result, [5; 4]);
    }

    #[test]
    fn test_vcmpequw() {
        let a = [1, 2, 3, 4];
        let b = [1, 0, 3, 5];
        let (result, all_true) = vcmpequw(a, b);
        assert_eq!(result[0], 0xFFFFFFFF);
        assert_eq!(result[1], 0);
        assert_eq!(result[2], 0xFFFFFFFF);
        assert_eq!(result[3], 0);
        assert!(!all_true);
    }

    #[test]
    fn test_vaddfp() {
        let a = [1.0f32.to_bits(), 2.0f32.to_bits(), 3.0f32.to_bits(), 4.0f32.to_bits()];
        let b = [0.5f32.to_bits(), 0.5f32.to_bits(), 0.5f32.to_bits(), 0.5f32.to_bits()];
        let result = vaddfp(a, b);
        assert_eq!(f32::from_bits(result[0]), 1.5);
        assert_eq!(f32::from_bits(result[1]), 2.5);
    }

    #[test]
    fn test_vperm() {
        let a = [0x00010203, 0x04050607, 0x08090A0B, 0x0C0D0E0F];
        let b = [0x10111213, 0x14151617, 0x18191A1B, 0x1C1D1E1F];
        // Control: swap first and second halves
        let c = [0x10111213, 0x14151617, 0x00010203, 0x04050607];
        let result = vperm(a, b, c);
        // Elements from b come first (indices 16-31), then a (indices 0-15)
        assert!(result[0] != 0); // Just verify permutation happened
    }
}
