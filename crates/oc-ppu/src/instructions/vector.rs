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

/// Vector Add Byte Modulo
pub fn vaddubm(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let bytes_a = a[i].to_be_bytes();
        let bytes_b = b[i].to_be_bytes();
        let result_bytes = [
            bytes_a[0].wrapping_add(bytes_b[0]),
            bytes_a[1].wrapping_add(bytes_b[1]),
            bytes_a[2].wrapping_add(bytes_b[2]),
            bytes_a[3].wrapping_add(bytes_b[3]),
        ];
        result[i] = u32::from_be_bytes(result_bytes);
    }
    result
}

/// Vector Add Halfword Modulo
pub fn vadduhm(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let hi_a = (a[i] >> 16) as u16;
        let lo_a = a[i] as u16;
        let hi_b = (b[i] >> 16) as u16;
        let lo_b = b[i] as u16;
        let hi_res = hi_a.wrapping_add(hi_b);
        let lo_res = lo_a.wrapping_add(lo_b);
        result[i] = ((hi_res as u32) << 16) | (lo_res as u32);
    }
    result
}

/// Vector Add Word Modulo
pub fn vadduwm(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    [
        a[0].wrapping_add(b[0]),
        a[1].wrapping_add(b[1]),
        a[2].wrapping_add(b[2]),
        a[3].wrapping_add(b[3]),
    ]
}

/// Vector Add Byte Saturate Signed
pub fn vaddsbs(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let bytes_a = a[i].to_be_bytes();
        let bytes_b = b[i].to_be_bytes();
        let result_bytes = [
            (bytes_a[0] as i8).saturating_add(bytes_b[0] as i8) as u8,
            (bytes_a[1] as i8).saturating_add(bytes_b[1] as i8) as u8,
            (bytes_a[2] as i8).saturating_add(bytes_b[2] as i8) as u8,
            (bytes_a[3] as i8).saturating_add(bytes_b[3] as i8) as u8,
        ];
        result[i] = u32::from_be_bytes(result_bytes);
    }
    result
}

/// Vector Add Halfword Saturate Signed
pub fn vaddshs(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let hi_a = (a[i] >> 16) as i16;
        let lo_a = a[i] as i16;
        let hi_b = (b[i] >> 16) as i16;
        let lo_b = b[i] as i16;
        let hi_res = hi_a.saturating_add(hi_b) as u16;
        let lo_res = lo_a.saturating_add(lo_b) as u16;
        result[i] = ((hi_res as u32) << 16) | (lo_res as u32);
    }
    result
}

/// Vector Subtract Byte Modulo
pub fn vsububm(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let bytes_a = a[i].to_be_bytes();
        let bytes_b = b[i].to_be_bytes();
        let result_bytes = [
            bytes_a[0].wrapping_sub(bytes_b[0]),
            bytes_a[1].wrapping_sub(bytes_b[1]),
            bytes_a[2].wrapping_sub(bytes_b[2]),
            bytes_a[3].wrapping_sub(bytes_b[3]),
        ];
        result[i] = u32::from_be_bytes(result_bytes);
    }
    result
}

/// Vector Subtract Halfword Modulo
pub fn vsubuhm(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let hi_a = (a[i] >> 16) as u16;
        let lo_a = a[i] as u16;
        let hi_b = (b[i] >> 16) as u16;
        let lo_b = b[i] as u16;
        let hi_res = hi_a.wrapping_sub(hi_b);
        let lo_res = lo_a.wrapping_sub(lo_b);
        result[i] = ((hi_res as u32) << 16) | (lo_res as u32);
    }
    result
}

/// Vector Subtract Word Modulo
pub fn vsubuwm(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    [
        a[0].wrapping_sub(b[0]),
        a[1].wrapping_sub(b[1]),
        a[2].wrapping_sub(b[2]),
        a[3].wrapping_sub(b[3]),
    ]
}

/// Vector Subtract Byte Saturate Signed
pub fn vsubsbs(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let bytes_a = a[i].to_be_bytes();
        let bytes_b = b[i].to_be_bytes();
        let result_bytes = [
            (bytes_a[0] as i8).saturating_sub(bytes_b[0] as i8) as u8,
            (bytes_a[1] as i8).saturating_sub(bytes_b[1] as i8) as u8,
            (bytes_a[2] as i8).saturating_sub(bytes_b[2] as i8) as u8,
            (bytes_a[3] as i8).saturating_sub(bytes_b[3] as i8) as u8,
        ];
        result[i] = u32::from_be_bytes(result_bytes);
    }
    result
}

/// Vector Subtract Halfword Saturate Signed
pub fn vsubshs(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let hi_a = (a[i] >> 16) as i16;
        let lo_a = a[i] as i16;
        let hi_b = (b[i] >> 16) as i16;
        let lo_b = b[i] as i16;
        let hi_res = hi_a.saturating_sub(hi_b) as u16;
        let lo_res = lo_a.saturating_sub(lo_b) as u16;
        result[i] = ((hi_res as u32) << 16) | (lo_res as u32);
    }
    result
}

/// Vector Pack Signed Word Saturate Signed Halfword
pub fn vpkswss(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    let pack = |val: i32| -> i16 {
        if val > i16::MAX as i32 {
            i16::MAX
        } else if val < i16::MIN as i32 {
            i16::MIN
        } else {
            val as i16
        }
    };
    
    // Pack a's elements into first two words
    result[0] = ((pack(a[0] as i32) as u16 as u32) << 16) | (pack(a[1] as i32) as u16 as u32);
    result[1] = ((pack(a[2] as i32) as u16 as u32) << 16) | (pack(a[3] as i32) as u16 as u32);
    
    // Pack b's elements into last two words
    result[2] = ((pack(b[0] as i32) as u16 as u32) << 16) | (pack(b[1] as i32) as u16 as u32);
    result[3] = ((pack(b[2] as i32) as u16 as u32) << 16) | (pack(b[3] as i32) as u16 as u32);
    
    result
}

/// Vector Pack Halfword Saturate Signed Byte
pub fn vpkshss(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    let pack = |val: i16| -> i8 {
        if val > i8::MAX as i16 {
            i8::MAX
        } else if val < i8::MIN as i16 {
            i8::MIN
        } else {
            val as i8
        }
    };
    
    // Pack 8 halfwords from a and b into 16 bytes
    let mut all_bytes = [0u8; 16];
    let mut byte_idx = 0;
    
    // Pack from vector a (4 words = 8 halfwords)
    for i in 0..4 {
        let hi = (a[i] >> 16) as i16;
        let lo = a[i] as i16;
        all_bytes[byte_idx] = pack(hi) as u8;
        all_bytes[byte_idx + 1] = pack(lo) as u8;
        byte_idx += 2;
    }
    
    // Pack from vector b (4 words = 8 halfwords)
    for i in 0..4 {
        let hi = (b[i] >> 16) as i16;
        let lo = b[i] as i16;
        all_bytes[byte_idx] = pack(hi) as u8;
        all_bytes[byte_idx + 1] = pack(lo) as u8;
        byte_idx += 2;
    }
    
    // Convert bytes back to words
    for i in 0..4 {
        result[i] = u32::from_be_bytes([
            all_bytes[i * 4],
            all_bytes[i * 4 + 1],
            all_bytes[i * 4 + 2],
            all_bytes[i * 4 + 3],
        ]);
    }
    
    result
}

/// Vector Unpack High Signed Byte to Signed Halfword
pub fn vupkhsb(a: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    let bytes = [
        ((a[0] >> 24) & 0xFF) as i8,
        ((a[0] >> 16) & 0xFF) as i8,
        ((a[0] >> 8) & 0xFF) as i8,
        (a[0] & 0xFF) as i8,
        ((a[1] >> 24) & 0xFF) as i8,
        ((a[1] >> 16) & 0xFF) as i8,
        ((a[1] >> 8) & 0xFF) as i8,
        (a[1] & 0xFF) as i8,
    ];
    
    for i in 0..4 {
        let hi = bytes[i * 2] as i16 as u16;
        let lo = bytes[i * 2 + 1] as i16 as u16;
        result[i] = ((hi as u32) << 16) | (lo as u32);
    }
    
    result
}

/// Vector Unpack Low Signed Byte to Signed Halfword
pub fn vupklsb(a: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    let bytes = [
        ((a[2] >> 24) & 0xFF) as i8,
        ((a[2] >> 16) & 0xFF) as i8,
        ((a[2] >> 8) & 0xFF) as i8,
        (a[2] & 0xFF) as i8,
        ((a[3] >> 24) & 0xFF) as i8,
        ((a[3] >> 16) & 0xFF) as i8,
        ((a[3] >> 8) & 0xFF) as i8,
        (a[3] & 0xFF) as i8,
    ];
    
    for i in 0..4 {
        let hi = bytes[i * 2] as i16 as u16;
        let lo = bytes[i * 2 + 1] as i16 as u16;
        result[i] = ((hi as u32) << 16) | (lo as u32);
    }
    
    result
}

/// Vector Multiply Even Unsigned Word
pub fn vmuleuw(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    // Multiply even elements (0, 2) and store as 64-bit results
    // Result is stored as two 32-bit words per multiplication
    let prod0 = (a[0] as u64) * (b[0] as u64);
    let prod2 = (a[2] as u64) * (b[2] as u64);
    
    [
        (prod0 >> 32) as u32,
        prod0 as u32,
        (prod2 >> 32) as u32,
        prod2 as u32,
    ]
}

/// Vector Multiply Odd Unsigned Word
pub fn vmulouw(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    // Multiply odd elements (1, 3) and store as 64-bit results
    let prod1 = (a[1] as u64) * (b[1] as u64);
    let prod3 = (a[3] as u64) * (b[3] as u64);
    
    [
        (prod1 >> 32) as u32,
        prod1 as u32,
        (prod3 >> 32) as u32,
        prod3 as u32,
    ]
}

/// Vector Multiply High Unsigned Word
pub fn vmulhuw(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    [
        ((a[0] as u64 * b[0] as u64) >> 32) as u32,
        ((a[1] as u64 * b[1] as u64) >> 32) as u32,
        ((a[2] as u64 * b[2] as u64) >> 32) as u32,
        ((a[3] as u64 * b[3] as u64) >> 32) as u32,
    ]
}

/// Vector Sum Across Quarter Unsigned Byte Saturate
pub fn vsum4ubs(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        // Sum 4 bytes of a[i]
        let bytes = a[i].to_be_bytes();
        let sum = (bytes[0] as u32) + (bytes[1] as u32) + (bytes[2] as u32) + (bytes[3] as u32);
        // Add to b[i] and saturate
        result[i] = sum.saturating_add(b[i]);
    }
    result
}

/// Vector Maximum Floating Point
pub fn vmaxfp(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    [
        if f32::from_bits(a[0]) > f32::from_bits(b[0]) { a[0] } else { b[0] },
        if f32::from_bits(a[1]) > f32::from_bits(b[1]) { a[1] } else { b[1] },
        if f32::from_bits(a[2]) > f32::from_bits(b[2]) { a[2] } else { b[2] },
        if f32::from_bits(a[3]) > f32::from_bits(b[3]) { a[3] } else { b[3] },
    ]
}

/// Vector Minimum Floating Point
pub fn vminfp(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    [
        if f32::from_bits(a[0]) < f32::from_bits(b[0]) { a[0] } else { b[0] },
        if f32::from_bits(a[1]) < f32::from_bits(b[1]) { a[1] } else { b[1] },
        if f32::from_bits(a[2]) < f32::from_bits(b[2]) { a[2] } else { b[2] },
        if f32::from_bits(a[3]) < f32::from_bits(b[3]) { a[3] } else { b[3] },
    ]
}

// ============================================================================
// Additional VMX/AltiVec instructions
// ============================================================================

/// Vector Shift Left Double by Octet Immediate (VA-form)
/// Concatenates vA and vB, then shifts left by SH bytes
pub fn vsldoi(a: [u32; 4], b: [u32; 4], sh: u8) -> [u32; 4] {
    // Convert to bytes (big-endian)
    let mut concat = [0u8; 32];
    for i in 0..4 {
        let a_bytes = a[i].to_be_bytes();
        let b_bytes = b[i].to_be_bytes();
        concat[i * 4] = a_bytes[0];
        concat[i * 4 + 1] = a_bytes[1];
        concat[i * 4 + 2] = a_bytes[2];
        concat[i * 4 + 3] = a_bytes[3];
        concat[16 + i * 4] = b_bytes[0];
        concat[16 + i * 4 + 1] = b_bytes[1];
        concat[16 + i * 4 + 2] = b_bytes[2];
        concat[16 + i * 4 + 3] = b_bytes[3];
    }
    
    // Extract 16 bytes starting at offset sh
    let sh = (sh & 0xF) as usize;
    let mut result_bytes = [0u8; 16];
    for i in 0..16 {
        result_bytes[i] = concat[sh + i];
    }
    
    // Convert back to words
    [
        u32::from_be_bytes([result_bytes[0], result_bytes[1], result_bytes[2], result_bytes[3]]),
        u32::from_be_bytes([result_bytes[4], result_bytes[5], result_bytes[6], result_bytes[7]]),
        u32::from_be_bytes([result_bytes[8], result_bytes[9], result_bytes[10], result_bytes[11]]),
        u32::from_be_bytes([result_bytes[12], result_bytes[13], result_bytes[14], result_bytes[15]]),
    ]
}

/// Load Vector for Shift Left - generates permute control for lvsl
pub fn lvsl(addr: u64) -> [u32; 4] {
    let sh = (addr & 0xF) as u8;
    let mut result = [0u32; 4];
    for i in 0..16 {
        let byte = (sh + i as u8) & 0x1F;
        let word_idx = i / 4;
        let byte_idx = i % 4;
        result[word_idx] |= (byte as u32) << ((3 - byte_idx) * 8);
    }
    result
}

/// Load Vector for Shift Right - generates permute control for lvsr
pub fn lvsr(addr: u64) -> [u32; 4] {
    let sh = (16 - (addr & 0xF)) as u8;
    let mut result = [0u32; 4];
    for i in 0..16 {
        let byte = (sh + i as u8) & 0x1F;
        let word_idx = i / 4;
        let byte_idx = i % 4;
        result[word_idx] |= (byte as u32) << ((3 - byte_idx) * 8);
    }
    result
}

/// Vector Compare Greater Than or Equal Single-Precision
pub fn vcmpgefp(a: [u32; 4], b: [u32; 4]) -> ([u32; 4], bool) {
    let mut result = [0u32; 4];
    let mut all_true = true;
    
    for i in 0..4 {
        let fa = f32::from_bits(a[i]);
        let fb = f32::from_bits(b[i]);
        if fa >= fb {
            result[i] = 0xFFFFFFFF;
        } else {
            result[i] = 0;
            all_true = false;
        }
    }
    (result, all_true)
}

/// Vector Compare Bounds Single-Precision
/// Returns all 1s if |a| <= b, else 0
pub fn vcmpbfp(a: [u32; 4], b: [u32; 4]) -> ([u32; 4], bool) {
    let mut result = [0u32; 4];
    let mut all_in_bounds = true;
    
    for i in 0..4 {
        let fa = f32::from_bits(a[i]);
        let fb = f32::from_bits(b[i]);
        // Bit 0: a > b, Bit 1: a < -b
        let gt = if fa > fb { 0x80000000u32 } else { 0 };
        let lt = if fa < -fb { 0x40000000u32 } else { 0 };
        result[i] = gt | lt;
        if result[i] != 0 {
            all_in_bounds = false;
        }
    }
    (result, all_in_bounds)
}

/// Vector Log2 Estimate Single-Precision
pub fn vlogefp(a: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let fa = f32::from_bits(a[i]);
        result[i] = fa.log2().to_bits();
    }
    result
}

/// Vector 2^x Estimate Single-Precision
pub fn vexptefp(a: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let fa = f32::from_bits(a[i]);
        result[i] = (2.0f32).powf(fa).to_bits();
    }
    result
}

/// Vector Convert to Unsigned Fixed-Point Word Saturate
pub fn vctuxs(a: [u32; 4], uimm: u8) -> [u32; 4] {
    let scale = 1u64 << (uimm & 0x1F);
    let mut result = [0u32; 4];
    
    for i in 0..4 {
        let fa = f32::from_bits(a[i]) as f64;
        let scaled = (fa * scale as f64).round();
        let clamped = scaled.clamp(0.0, u32::MAX as f64) as u32;
        result[i] = clamped;
    }
    result
}

/// Vector Convert from Unsigned Fixed-Point Word
pub fn vcfux(a: [u32; 4], uimm: u8) -> [u32; 4] {
    let scale = 1.0f32 / (1u32 << (uimm & 0x1F)) as f32;
    let mut result = [0u32; 4];
    
    for i in 0..4 {
        result[i] = (a[i] as f32 * scale).to_bits();
    }
    result
}

/// Vector Splat Byte
pub fn vspltb(v: [u32; 4], uimm: u8) -> [u32; 4] {
    let word_idx = (uimm >> 2) as usize & 3;
    let byte_idx = (uimm & 3) as usize;
    let byte = ((v[word_idx] >> ((3 - byte_idx) * 8)) & 0xFF) as u8;
    let w = u32::from_be_bytes([byte, byte, byte, byte]);
    [w, w, w, w]
}

/// Vector Splat Halfword
pub fn vsplth(v: [u32; 4], uimm: u8) -> [u32; 4] {
    let word_idx = (uimm >> 1) as usize & 3;
    let half_idx = (uimm & 1) as usize;
    let half = if half_idx == 0 {
        (v[word_idx] >> 16) as u16
    } else {
        v[word_idx] as u16
    };
    let w = ((half as u32) << 16) | (half as u32);
    [w, w, w, w]
}

/// Vector Merge High Byte
pub fn vmrghb(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let a_bytes = [
        a[0].to_be_bytes(), a[1].to_be_bytes(),
    ];
    let b_bytes = [
        b[0].to_be_bytes(), b[1].to_be_bytes(),
    ];
    
    // Interleave high 8 bytes from a and b
    [
        u32::from_be_bytes([a_bytes[0][0], b_bytes[0][0], a_bytes[0][1], b_bytes[0][1]]),
        u32::from_be_bytes([a_bytes[0][2], b_bytes[0][2], a_bytes[0][3], b_bytes[0][3]]),
        u32::from_be_bytes([a_bytes[1][0], b_bytes[1][0], a_bytes[1][1], b_bytes[1][1]]),
        u32::from_be_bytes([a_bytes[1][2], b_bytes[1][2], a_bytes[1][3], b_bytes[1][3]]),
    ]
}

/// Vector Merge Low Byte
pub fn vmrglb(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let a_bytes = [
        a[2].to_be_bytes(), a[3].to_be_bytes(),
    ];
    let b_bytes = [
        b[2].to_be_bytes(), b[3].to_be_bytes(),
    ];
    
    // Interleave low 8 bytes from a and b
    [
        u32::from_be_bytes([a_bytes[0][0], b_bytes[0][0], a_bytes[0][1], b_bytes[0][1]]),
        u32::from_be_bytes([a_bytes[0][2], b_bytes[0][2], a_bytes[0][3], b_bytes[0][3]]),
        u32::from_be_bytes([a_bytes[1][0], b_bytes[1][0], a_bytes[1][1], b_bytes[1][1]]),
        u32::from_be_bytes([a_bytes[1][2], b_bytes[1][2], a_bytes[1][3], b_bytes[1][3]]),
    ]
}

/// Vector Merge High Halfword
pub fn vmrghh(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    [
        (a[0] & 0xFFFF0000) | ((b[0] >> 16) & 0xFFFF),
        ((a[0] << 16) & 0xFFFF0000) | (b[0] & 0xFFFF),
        (a[1] & 0xFFFF0000) | ((b[1] >> 16) & 0xFFFF),
        ((a[1] << 16) & 0xFFFF0000) | (b[1] & 0xFFFF),
    ]
}

/// Vector Merge Low Halfword
pub fn vmrglh(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    [
        (a[2] & 0xFFFF0000) | ((b[2] >> 16) & 0xFFFF),
        ((a[2] << 16) & 0xFFFF0000) | (b[2] & 0xFFFF),
        (a[3] & 0xFFFF0000) | ((b[3] >> 16) & 0xFFFF),
        ((a[3] << 16) & 0xFFFF0000) | (b[3] & 0xFFFF),
    ]
}

/// Vector Average Unsigned Byte
pub fn vavgub(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let a_bytes = a[i].to_be_bytes();
        let b_bytes = b[i].to_be_bytes();
        let r_bytes = [
            ((a_bytes[0] as u16 + b_bytes[0] as u16 + 1) >> 1) as u8,
            ((a_bytes[1] as u16 + b_bytes[1] as u16 + 1) >> 1) as u8,
            ((a_bytes[2] as u16 + b_bytes[2] as u16 + 1) >> 1) as u8,
            ((a_bytes[3] as u16 + b_bytes[3] as u16 + 1) >> 1) as u8,
        ];
        result[i] = u32::from_be_bytes(r_bytes);
    }
    result
}

/// Vector Average Unsigned Halfword
pub fn vavguh(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let a_hi = (a[i] >> 16) as u16;
        let a_lo = a[i] as u16;
        let b_hi = (b[i] >> 16) as u16;
        let b_lo = b[i] as u16;
        let r_hi = ((a_hi as u32 + b_hi as u32 + 1) >> 1) as u16;
        let r_lo = ((a_lo as u32 + b_lo as u32 + 1) >> 1) as u16;
        result[i] = ((r_hi as u32) << 16) | (r_lo as u32);
    }
    result
}

/// Vector Average Unsigned Word
pub fn vavguw(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    [
        ((a[0] as u64 + b[0] as u64 + 1) >> 1) as u32,
        ((a[1] as u64 + b[1] as u64 + 1) >> 1) as u32,
        ((a[2] as u64 + b[2] as u64 + 1) >> 1) as u32,
        ((a[3] as u64 + b[3] as u64 + 1) >> 1) as u32,
    ]
}

/// Vector Average Signed Byte
pub fn vavgsb(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let a_bytes = a[i].to_be_bytes();
        let b_bytes = b[i].to_be_bytes();
        let r_bytes = [
            ((a_bytes[0] as i8 as i16 + b_bytes[0] as i8 as i16 + 1) >> 1) as u8,
            ((a_bytes[1] as i8 as i16 + b_bytes[1] as i8 as i16 + 1) >> 1) as u8,
            ((a_bytes[2] as i8 as i16 + b_bytes[2] as i8 as i16 + 1) >> 1) as u8,
            ((a_bytes[3] as i8 as i16 + b_bytes[3] as i8 as i16 + 1) >> 1) as u8,
        ];
        result[i] = u32::from_be_bytes(r_bytes);
    }
    result
}

/// Vector Average Signed Halfword
pub fn vavgsh(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let a_hi = (a[i] >> 16) as i16;
        let a_lo = a[i] as i16;
        let b_hi = (b[i] >> 16) as i16;
        let b_lo = b[i] as i16;
        let r_hi = ((a_hi as i32 + b_hi as i32 + 1) >> 1) as u16;
        let r_lo = ((a_lo as i32 + b_lo as i32 + 1) >> 1) as u16;
        result[i] = ((r_hi as u32) << 16) | (r_lo as u32);
    }
    result
}

/// Vector Average Signed Word
pub fn vavgsw(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    [
        ((a[0] as i32 as i64 + b[0] as i32 as i64 + 1) >> 1) as u32,
        ((a[1] as i32 as i64 + b[1] as i32 as i64 + 1) >> 1) as u32,
        ((a[2] as i32 as i64 + b[2] as i32 as i64 + 1) >> 1) as u32,
        ((a[3] as i32 as i64 + b[3] as i32 as i64 + 1) >> 1) as u32,
    ]
}

/// Vector Multiply Even Signed Byte
pub fn vmulesb(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let a_bytes = a[i].to_be_bytes();
        let b_bytes = b[i].to_be_bytes();
        // Multiply even bytes (0, 2) producing 16-bit results
        let hi = (a_bytes[0] as i8 as i16).wrapping_mul(b_bytes[0] as i8 as i16) as u16;
        let lo = (a_bytes[2] as i8 as i16).wrapping_mul(b_bytes[2] as i8 as i16) as u16;
        result[i] = ((hi as u32) << 16) | (lo as u32);
    }
    result
}

/// Vector Multiply Odd Signed Byte
pub fn vmulosb(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let a_bytes = a[i].to_be_bytes();
        let b_bytes = b[i].to_be_bytes();
        // Multiply odd bytes (1, 3) producing 16-bit results
        let hi = (a_bytes[1] as i8 as i16).wrapping_mul(b_bytes[1] as i8 as i16) as u16;
        let lo = (a_bytes[3] as i8 as i16).wrapping_mul(b_bytes[3] as i8 as i16) as u16;
        result[i] = ((hi as u32) << 16) | (lo as u32);
    }
    result
}

/// Vector Multiply Even Unsigned Byte  
pub fn vmuleub(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let a_bytes = a[i].to_be_bytes();
        let b_bytes = b[i].to_be_bytes();
        // Multiply even bytes (0, 2) producing 16-bit results
        let hi = (a_bytes[0] as u16) * (b_bytes[0] as u16);
        let lo = (a_bytes[2] as u16) * (b_bytes[2] as u16);
        result[i] = ((hi as u32) << 16) | (lo as u32);
    }
    result
}

/// Vector Multiply Odd Unsigned Byte
pub fn vmuloub(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let a_bytes = a[i].to_be_bytes();
        let b_bytes = b[i].to_be_bytes();
        // Multiply odd bytes (1, 3) producing 16-bit results
        let hi = (a_bytes[1] as u16) * (b_bytes[1] as u16);
        let lo = (a_bytes[3] as u16) * (b_bytes[3] as u16);
        result[i] = ((hi as u32) << 16) | (lo as u32);
    }
    result
}

/// Vector Multiply Even Signed Halfword
pub fn vmulesh(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..2 {
        let a_hi = (a[i * 2] >> 16) as i16;
        let b_hi = (b[i * 2] >> 16) as i16;
        result[i * 2] = (a_hi as i32).wrapping_mul(b_hi as i32) as u32;
        
        let a_hi2 = (a[i * 2 + 1] >> 16) as i16;
        let b_hi2 = (b[i * 2 + 1] >> 16) as i16;
        result[i * 2 + 1] = (a_hi2 as i32).wrapping_mul(b_hi2 as i32) as u32;
    }
    result
}

/// Vector Multiply Odd Signed Halfword
pub fn vmulosh(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..2 {
        let a_lo = a[i * 2] as i16;
        let b_lo = b[i * 2] as i16;
        result[i * 2] = (a_lo as i32).wrapping_mul(b_lo as i32) as u32;
        
        let a_lo2 = a[i * 2 + 1] as i16;
        let b_lo2 = b[i * 2 + 1] as i16;
        result[i * 2 + 1] = (a_lo2 as i32).wrapping_mul(b_lo2 as i32) as u32;
    }
    result
}

/// Vector Multiply Even Unsigned Halfword
pub fn vmuleuh(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..2 {
        let a_hi = (a[i * 2] >> 16) as u16;
        let b_hi = (b[i * 2] >> 16) as u16;
        result[i * 2] = (a_hi as u32) * (b_hi as u32);
        
        let a_hi2 = (a[i * 2 + 1] >> 16) as u16;
        let b_hi2 = (b[i * 2 + 1] >> 16) as u16;
        result[i * 2 + 1] = (a_hi2 as u32) * (b_hi2 as u32);
    }
    result
}

/// Vector Multiply Odd Unsigned Halfword
pub fn vmulouh(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..2 {
        let a_lo = a[i * 2] as u16;
        let b_lo = b[i * 2] as u16;
        result[i * 2] = (a_lo as u32) * (b_lo as u32);
        
        let a_lo2 = a[i * 2 + 1] as u16;
        let b_lo2 = b[i * 2 + 1] as u16;
        result[i * 2 + 1] = (a_lo2 as u32) * (b_lo2 as u32);
    }
    result
}

// ============================================================================
// Additional VMX/AltiVec Instructions - Completion
// ============================================================================

/// Vector Pack Signed Halfword Unsigned Saturate
/// Packs 8 halfwords from two vectors into 16 unsigned bytes with saturation
pub fn vpkshus(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    let pack = |val: i16| -> u8 {
        if val < 0 {
            0
        } else if val > u8::MAX as i16 {
            u8::MAX
        } else {
            val as u8
        }
    };
    
    // Pack 8 halfwords from a and b into 16 bytes
    let mut all_bytes = [0u8; 16];
    let mut byte_idx = 0;
    
    // Pack from vector a (4 words = 8 halfwords)
    for i in 0..4 {
        let hi = (a[i] >> 16) as i16;
        let lo = a[i] as i16;
        all_bytes[byte_idx] = pack(hi);
        all_bytes[byte_idx + 1] = pack(lo);
        byte_idx += 2;
    }
    
    // Pack from vector b (4 words = 8 halfwords)
    for i in 0..4 {
        let hi = (b[i] >> 16) as i16;
        let lo = b[i] as i16;
        all_bytes[byte_idx] = pack(hi);
        all_bytes[byte_idx + 1] = pack(lo);
        byte_idx += 2;
    }
    
    // Convert bytes back to words
    for i in 0..4 {
        result[i] = u32::from_be_bytes([
            all_bytes[i * 4],
            all_bytes[i * 4 + 1],
            all_bytes[i * 4 + 2],
            all_bytes[i * 4 + 3],
        ]);
    }
    
    result
}

/// Vector Unpack High Signed Halfword to Signed Word
/// Unpacks the high 4 halfwords from a vector into 4 signed words
pub fn vupkhsh(a: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    // Extract high 4 halfwords (from words 0 and 1)
    let halfwords = [
        ((a[0] >> 16) & 0xFFFF) as i16,
        (a[0] & 0xFFFF) as i16,
        ((a[1] >> 16) & 0xFFFF) as i16,
        (a[1] & 0xFFFF) as i16,
    ];
    
    for i in 0..4 {
        result[i] = halfwords[i] as i32 as u32;
    }
    
    result
}

/// Vector Unpack Low Signed Halfword to Signed Word
/// Unpacks the low 4 halfwords from a vector into 4 signed words
pub fn vupklsh(a: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    // Extract low 4 halfwords (from words 2 and 3)
    let halfwords = [
        ((a[2] >> 16) & 0xFFFF) as i16,
        (a[2] & 0xFFFF) as i16,
        ((a[3] >> 16) & 0xFFFF) as i16,
        (a[3] & 0xFFFF) as i16,
    ];
    
    for i in 0..4 {
        result[i] = halfwords[i] as i32 as u32;
    }
    
    result
}

/// Vector Multiply High Signed Word
/// Returns the high 32 bits of a 64-bit signed product
pub fn vmulhsw(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let prod = (a[i] as i32 as i64) * (b[i] as i32 as i64);
        result[i] = (prod >> 32) as u32;
    }
    result
}

/// Vector Sum Across Signed Byte Saturate
/// Sums 4 signed bytes per word element and adds to b, with saturation
pub fn vsum4sbs(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let bytes = a[i].to_be_bytes();
        let sum: i32 = (bytes[0] as i8 as i32) + (bytes[1] as i8 as i32) 
                     + (bytes[2] as i8 as i32) + (bytes[3] as i8 as i32);
        let total = (b[i] as i32).saturating_add(sum);
        result[i] = total as u32;
    }
    result
}

/// Vector Sum Across Signed Halfword Saturate
/// Sums 2 signed halfwords per word element and adds to b, with saturation
pub fn vsum4shs(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let hi = (a[i] >> 16) as i16 as i32;
        let lo = a[i] as i16 as i32;
        let sum = hi + lo;
        let total = (b[i] as i32).saturating_add(sum);
        result[i] = total as u32;
    }
    result
}

/// Vector Sum Across 2 Signed Words Saturate
/// Sums pairs of signed words and adds to odd elements of b (b[1] and b[3])
pub fn vsum2sws(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    // Sum words 0,1 of a and add to b[1], result in word 1
    let sum01 = (a[0] as i32 as i64) + (a[1] as i32 as i64) + (b[1] as i32 as i64);
    result[1] = sum01.clamp(i32::MIN as i64, i32::MAX as i64) as i32 as u32;
    
    // Sum words 2,3 of a and add to b[3], result in word 3
    let sum23 = (a[2] as i32 as i64) + (a[3] as i32 as i64) + (b[3] as i32 as i64);
    result[3] = sum23.clamp(i32::MIN as i64, i32::MAX as i64) as i32 as u32;
    
    result
}

/// Vector Sum Across Signed Word Saturate
/// Sums all 4 signed words and adds to b[3], result in word 3
pub fn vsumsws(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    let sum: i64 = (a[0] as i32 as i64) + (a[1] as i32 as i64) 
                 + (a[2] as i32 as i64) + (a[3] as i32 as i64) 
                 + (b[3] as i32 as i64);
    result[3] = sum.clamp(i32::MIN as i64, i32::MAX as i64) as i32 as u32;
    result
}

/// Vector Minimum Unsigned Byte
pub fn vminub(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let a_bytes = a[i].to_be_bytes();
        let b_bytes = b[i].to_be_bytes();
        result[i] = u32::from_be_bytes([
            std::cmp::min(a_bytes[0], b_bytes[0]),
            std::cmp::min(a_bytes[1], b_bytes[1]),
            std::cmp::min(a_bytes[2], b_bytes[2]),
            std::cmp::min(a_bytes[3], b_bytes[3]),
        ]);
    }
    result
}

/// Vector Maximum Unsigned Byte
pub fn vmaxub(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let a_bytes = a[i].to_be_bytes();
        let b_bytes = b[i].to_be_bytes();
        result[i] = u32::from_be_bytes([
            std::cmp::max(a_bytes[0], b_bytes[0]),
            std::cmp::max(a_bytes[1], b_bytes[1]),
            std::cmp::max(a_bytes[2], b_bytes[2]),
            std::cmp::max(a_bytes[3], b_bytes[3]),
        ]);
    }
    result
}

/// Vector Minimum Signed Byte
pub fn vminsb(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let a_bytes = a[i].to_be_bytes();
        let b_bytes = b[i].to_be_bytes();
        result[i] = u32::from_be_bytes([
            std::cmp::min(a_bytes[0] as i8, b_bytes[0] as i8) as u8,
            std::cmp::min(a_bytes[1] as i8, b_bytes[1] as i8) as u8,
            std::cmp::min(a_bytes[2] as i8, b_bytes[2] as i8) as u8,
            std::cmp::min(a_bytes[3] as i8, b_bytes[3] as i8) as u8,
        ]);
    }
    result
}

/// Vector Maximum Signed Byte
pub fn vmaxsb(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let a_bytes = a[i].to_be_bytes();
        let b_bytes = b[i].to_be_bytes();
        result[i] = u32::from_be_bytes([
            std::cmp::max(a_bytes[0] as i8, b_bytes[0] as i8) as u8,
            std::cmp::max(a_bytes[1] as i8, b_bytes[1] as i8) as u8,
            std::cmp::max(a_bytes[2] as i8, b_bytes[2] as i8) as u8,
            std::cmp::max(a_bytes[3] as i8, b_bytes[3] as i8) as u8,
        ]);
    }
    result
}

/// Vector Minimum Unsigned Halfword
pub fn vminuh(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let a_hi = (a[i] >> 16) as u16;
        let a_lo = a[i] as u16;
        let b_hi = (b[i] >> 16) as u16;
        let b_lo = b[i] as u16;
        let min_hi = std::cmp::min(a_hi, b_hi);
        let min_lo = std::cmp::min(a_lo, b_lo);
        result[i] = ((min_hi as u32) << 16) | (min_lo as u32);
    }
    result
}

/// Vector Maximum Unsigned Halfword
pub fn vmaxuh(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let a_hi = (a[i] >> 16) as u16;
        let a_lo = a[i] as u16;
        let b_hi = (b[i] >> 16) as u16;
        let b_lo = b[i] as u16;
        let max_hi = std::cmp::max(a_hi, b_hi);
        let max_lo = std::cmp::max(a_lo, b_lo);
        result[i] = ((max_hi as u32) << 16) | (max_lo as u32);
    }
    result
}

/// Vector Minimum Signed Halfword
pub fn vminsh(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let a_hi = (a[i] >> 16) as i16;
        let a_lo = a[i] as i16;
        let b_hi = (b[i] >> 16) as i16;
        let b_lo = b[i] as i16;
        let min_hi = std::cmp::min(a_hi, b_hi) as u16;
        let min_lo = std::cmp::min(a_lo, b_lo) as u16;
        result[i] = ((min_hi as u32) << 16) | (min_lo as u32);
    }
    result
}

/// Vector Maximum Signed Halfword
pub fn vmaxsh(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let mut result = [0u32; 4];
    for i in 0..4 {
        let a_hi = (a[i] >> 16) as i16;
        let a_lo = a[i] as i16;
        let b_hi = (b[i] >> 16) as i16;
        let b_lo = b[i] as i16;
        let max_hi = std::cmp::max(a_hi, b_hi) as u16;
        let max_lo = std::cmp::max(a_lo, b_lo) as u16;
        result[i] = ((max_hi as u32) << 16) | (max_lo as u32);
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
    
    #[test]
    fn test_vaddubm() {
        let a = [0x01020304, 0x05060708, 0x090A0B0C, 0x0D0E0F10];
        let b = [0x10203040, 0x50607080, 0x90A0B0C0, 0xD0E0F000];
        let result = vaddubm(a, b);
        // Check modulo behavior (wrapping)
        assert_eq!(result[0] & 0xFF000000, 0x11000000);
    }
    
    #[test]
    fn test_vaddsbs() {
        let a = [0x7F000000, 0x80000000, 0x00000000, 0x00000000];
        let b = [0x01000000, 0xFF000000, 0x00000000, 0x00000000];
        let result = vaddsbs(a, b);
        // First byte should saturate to 0x7F (i8::MAX)
        assert_eq!(result[0] & 0xFF000000, 0x7F000000);
        // Second byte: 0x80 + 0xFF = i8::MIN + (-1) should saturate to i8::MIN (0x80)
        assert_eq!(result[1] & 0xFF000000, 0x80000000);
    }
    
    #[test]
    fn test_vmaxfp() {
        let a = [1.0f32.to_bits(), 2.0f32.to_bits(), 3.0f32.to_bits(), 4.0f32.to_bits()];
        let b = [2.0f32.to_bits(), 1.0f32.to_bits(), 4.0f32.to_bits(), 3.0f32.to_bits()];
        let result = vmaxfp(a, b);
        assert_eq!(f32::from_bits(result[0]), 2.0);
        assert_eq!(f32::from_bits(result[1]), 2.0);
        assert_eq!(f32::from_bits(result[2]), 4.0);
        assert_eq!(f32::from_bits(result[3]), 4.0);
    }
    
    #[test]
    fn test_vpkshus() {
        // Test packing signed halfwords to unsigned bytes with saturation
        // Positive values within range
        let a = [0x00100020, 0x00300040, 0x00500060, 0x00700080];
        // Negative and overflow values
        let b = [0xFFFF0000, 0x01000200, 0xFFFFFFFE, 0x00FF00FF];
        let result = vpkshus(a, b);
        // First byte from a[0] high halfword (0x0010 = 16) -> 16
        assert_eq!((result[0] >> 24) & 0xFF, 16);
        // Negative values should saturate to 0
        assert_eq!((result[2] >> 24) & 0xFF, 0);
    }
    
    #[test]
    fn test_vupkhsh() {
        // High halfwords: 0xFF80 (=-128), 0x0010, 0xFF00, 0x007F
        let a = [0xFF800010, 0xFF00007F, 0x12345678, 0x9ABCDEF0];
        let result = vupkhsh(a);
        // Should sign-extend to words
        assert_eq!(result[0], 0xFFFFFF80); // -128 sign-extended
        assert_eq!(result[1], 0x00000010); // 16 positive
        assert_eq!(result[2], 0xFFFFFF00); // -256 sign-extended
        assert_eq!(result[3], 0x0000007F); // 127 positive
    }
    
    #[test]
    fn test_vupklsh() {
        // Low halfwords are in words 2 and 3
        let a = [0x12345678, 0x9ABCDEF0, 0xFF800010, 0xFF00007F];
        let result = vupklsh(a);
        // Should sign-extend to words
        assert_eq!(result[0], 0xFFFFFF80); // -128 sign-extended
        assert_eq!(result[1], 0x00000010); // 16 positive
        assert_eq!(result[2], 0xFFFFFF00); // -256 sign-extended
        assert_eq!(result[3], 0x0000007F); // 127 positive
    }
    
    #[test]
    fn test_vmulhsw() {
        // Test high word of signed multiply
        let a = [0x00010000, 0xFFFF0000, 0x7FFFFFFF, 0x80000000];
        let b = [0x00010000, 0x00020000, 0x00000002, 0x00000002];
        let result = vmulhsw(a, b);
        // 0x10000 * 0x10000 = 0x100000000, high word = 1
        assert_eq!(result[0], 1);
        // 0xFFFF0000 (-65536) * 0x20000 (131072), high word should be negative
        assert_eq!(result[1] as i32, -2);
    }
    
    #[test]
    fn test_vsum4sbs() {
        // 4 signed bytes summed with saturation
        let a = [0x01020304, 0, 0, 0]; // 1+2+3+4 = 10
        let b = [5, 0, 0, 0];
        let result = vsum4sbs(a, b);
        assert_eq!(result[0], 15); // 10 + 5
    }
    
    #[test]
    fn test_vsum4shs() {
        // 2 signed halfwords summed per word
        let a = [0x00010002, 0, 0, 0]; // 1 + 2 = 3
        let b = [5, 0, 0, 0];
        let result = vsum4shs(a, b);
        assert_eq!(result[0], 8); // 3 + 5
    }
    
    #[test]
    fn test_vsumsws() {
        // Sum all 4 words to element 3
        let a = [1, 2, 3, 4];
        let b = [0, 0, 0, 10];
        let result = vsumsws(a, b);
        assert_eq!(result[3], 20); // 1+2+3+4+10
        assert_eq!(result[0], 0);
        assert_eq!(result[1], 0);
        assert_eq!(result[2], 0);
    }
    
    #[test]
    fn test_vminub() {
        let a = [0x10203040, 0x50607080, 0x00000000, 0xFFFFFFFF];
        let b = [0x05251545, 0x60708090, 0xFFFFFFFF, 0x00000000];
        let result = vminub(a, b);
        // Min of each byte
        assert_eq!((result[0] >> 24) & 0xFF, 5);  // min(0x10, 0x05)
        assert_eq!((result[0] >> 16) & 0xFF, 0x20); // min(0x20, 0x25)
    }
    
    #[test]
    fn test_vmaxub() {
        let a = [0x10203040, 0, 0, 0];
        let b = [0x05251545, 0, 0, 0];
        let result = vmaxub(a, b);
        assert_eq!((result[0] >> 24) & 0xFF, 0x10); // max(0x10, 0x05)
        assert_eq!((result[0] >> 16) & 0xFF, 0x25); // max(0x20, 0x25)
    }
    
    #[test]
    fn test_vminsb() {
        // Signed byte comparison: 0x80 (-128) < 0x7F (127)
        let a = [0x80000000, 0, 0, 0];
        let b = [0x7F000000, 0, 0, 0];
        let result = vminsb(a, b);
        assert_eq!((result[0] >> 24) & 0xFF, 0x80); // -128 is min
    }
    
    #[test]
    fn test_vmaxsb() {
        // Signed byte comparison: 0x7F (127) > 0x80 (-128)
        let a = [0x80000000, 0, 0, 0];
        let b = [0x7F000000, 0, 0, 0];
        let result = vmaxsb(a, b);
        assert_eq!((result[0] >> 24) & 0xFF, 0x7F); // 127 is max
    }
    
    #[test]
    fn test_vminuh() {
        let a = [0x10002000, 0, 0, 0];
        let b = [0x05003000, 0, 0, 0];
        let result = vminuh(a, b);
        assert_eq!((result[0] >> 16) & 0xFFFF, 0x0500); // min(0x1000, 0x0500)
        assert_eq!(result[0] & 0xFFFF, 0x2000); // min(0x2000, 0x3000)
    }
    
    #[test]
    fn test_vmaxuh() {
        let a = [0x10002000, 0, 0, 0];
        let b = [0x05003000, 0, 0, 0];
        let result = vmaxuh(a, b);
        assert_eq!((result[0] >> 16) & 0xFFFF, 0x1000); // max(0x1000, 0x0500)
        assert_eq!(result[0] & 0xFFFF, 0x3000); // max(0x2000, 0x3000)
    }
    
    #[test]
    fn test_vminsh() {
        // Signed halfword: 0x8000 (-32768) < 0x7FFF (32767)
        let a = [0x80000000, 0, 0, 0];
        let b = [0x7FFF0000, 0, 0, 0];
        let result = vminsh(a, b);
        assert_eq!((result[0] >> 16) & 0xFFFF, 0x8000); // -32768 is min
    }
    
    #[test]
    fn test_vmaxsh() {
        // Signed halfword: 0x7FFF (32767) > 0x8000 (-32768)
        let a = [0x80000000, 0, 0, 0];
        let b = [0x7FFF0000, 0, 0, 0];
        let result = vmaxsh(a, b);
        assert_eq!((result[0] >> 16) & 0xFFFF, 0x7FFF); // 32767 is max
    }
}
