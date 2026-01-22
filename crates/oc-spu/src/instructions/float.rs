//! SPU floating-point instructions

use crate::thread::SpuThread;
use oc_core::error::SpuError;

/// Floating Add - fa rt, ra, rb
pub fn fa(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        (f32::from_bits(a[0]) + f32::from_bits(b[0])).to_bits(),
        (f32::from_bits(a[1]) + f32::from_bits(b[1])).to_bits(),
        (f32::from_bits(a[2]) + f32::from_bits(b[2])).to_bits(),
        (f32::from_bits(a[3]) + f32::from_bits(b[3])).to_bits(),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Floating Subtract - fs rt, ra, rb
pub fn fs(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        (f32::from_bits(a[0]) - f32::from_bits(b[0])).to_bits(),
        (f32::from_bits(a[1]) - f32::from_bits(b[1])).to_bits(),
        (f32::from_bits(a[2]) - f32::from_bits(b[2])).to_bits(),
        (f32::from_bits(a[3]) - f32::from_bits(b[3])).to_bits(),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Floating Multiply - fm rt, ra, rb
pub fn fm(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        (f32::from_bits(a[0]) * f32::from_bits(b[0])).to_bits(),
        (f32::from_bits(a[1]) * f32::from_bits(b[1])).to_bits(),
        (f32::from_bits(a[2]) * f32::from_bits(b[2])).to_bits(),
        (f32::from_bits(a[3]) * f32::from_bits(b[3])).to_bits(),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Floating Multiply and Add - fma rt, ra, rb, rc
pub fn fma(thread: &mut SpuThread, rc: u8, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let c = thread.regs.read_u32x4(rc as usize);
    let result = [
        f32::from_bits(a[0]).mul_add(f32::from_bits(b[0]), f32::from_bits(c[0])).to_bits(),
        f32::from_bits(a[1]).mul_add(f32::from_bits(b[1]), f32::from_bits(c[1])).to_bits(),
        f32::from_bits(a[2]).mul_add(f32::from_bits(b[2]), f32::from_bits(c[2])).to_bits(),
        f32::from_bits(a[3]).mul_add(f32::from_bits(b[3]), f32::from_bits(c[3])).to_bits(),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Floating Negative Multiply and Subtract - fnms rt, ra, rb, rc
pub fn fnms(thread: &mut SpuThread, rc: u8, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let c = thread.regs.read_u32x4(rc as usize);
    let result = [
        (f32::from_bits(c[0]) - f32::from_bits(a[0]) * f32::from_bits(b[0])).to_bits(),
        (f32::from_bits(c[1]) - f32::from_bits(a[1]) * f32::from_bits(b[1])).to_bits(),
        (f32::from_bits(c[2]) - f32::from_bits(a[2]) * f32::from_bits(b[2])).to_bits(),
        (f32::from_bits(c[3]) - f32::from_bits(a[3]) * f32::from_bits(b[3])).to_bits(),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Floating Reciprocal Estimate - frest rt, ra
pub fn frest(thread: &mut SpuThread, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let result = [
        compute_reciprocal_estimate(f32::from_bits(a[0])).to_bits(),
        compute_reciprocal_estimate(f32::from_bits(a[1])).to_bits(),
        compute_reciprocal_estimate(f32::from_bits(a[2])).to_bits(),
        compute_reciprocal_estimate(f32::from_bits(a[3])).to_bits(),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Floating Reciprocal Square Root Estimate - frsqest rt, ra
pub fn frsqest(thread: &mut SpuThread, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let result = [
        compute_rsqrt_estimate(f32::from_bits(a[0])).to_bits(),
        compute_rsqrt_estimate(f32::from_bits(a[1])).to_bits(),
        compute_rsqrt_estimate(f32::from_bits(a[2])).to_bits(),
        compute_rsqrt_estimate(f32::from_bits(a[3])).to_bits(),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Helper: Compute reciprocal estimate with SPU-compatible special case handling
fn compute_reciprocal_estimate(x: f32) -> f32 {
    if x.is_nan() {
        // NaN input returns NaN
        f32::NAN
    } else if x == 0.0 {
        // Zero returns infinity with appropriate sign
        if x.is_sign_positive() {
            f32::INFINITY
        } else {
            f32::NEG_INFINITY
        }
    } else if x.is_infinite() {
        // Infinity returns zero with appropriate sign
        if x.is_sign_positive() {
            0.0
        } else {
            -0.0
        }
    } else {
        // Normal case: compute reciprocal
        1.0 / x
    }
}

/// Helper: Compute reciprocal square root estimate with SPU-compatible special case handling
fn compute_rsqrt_estimate(x: f32) -> f32 {
    if x.is_nan() {
        // NaN input returns NaN
        f32::NAN
    } else if x < 0.0 {
        // Negative input returns NaN (square root of negative is undefined)
        f32::NAN
    } else if x == 0.0 {
        // Zero returns positive infinity
        f32::INFINITY
    } else if x.is_infinite() {
        // Positive infinity returns zero
        0.0
    } else {
        // Normal case: compute reciprocal square root
        1.0 / x.sqrt()
    }
}

/// Floating Multiply and Subtract - fms rt, ra, rb, rc
pub fn fms(thread: &mut SpuThread, rc: u8, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let c = thread.regs.read_u32x4(rc as usize);
    let result = [
        (f32::from_bits(a[0]) * f32::from_bits(b[0]) - f32::from_bits(c[0])).to_bits(),
        (f32::from_bits(a[1]) * f32::from_bits(b[1]) - f32::from_bits(c[1])).to_bits(),
        (f32::from_bits(a[2]) * f32::from_bits(b[2]) - f32::from_bits(c[2])).to_bits(),
        (f32::from_bits(a[3]) * f32::from_bits(b[3]) - f32::from_bits(c[3])).to_bits(),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Floating Interpolate - fi rt, ra, rb
/// This instruction is used with frest/frsqest to refine reciprocal estimates
/// fi computes: rt = ra + rb * (1 - ra * rb) for Newton-Raphson iteration
pub fn fi(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    // fi is used as: y' = y + y * (1 - x * y) = y * (2 - x * y)
    // This implements one Newton-Raphson step for reciprocal refinement
    // Result = ra + rb where rb contains the correction term from prior calculation
    let result = [
        {
            let y = f32::from_bits(a[0]);
            let correction = f32::from_bits(b[0]);
            // Compute refined estimate using the correction value
            (y + y * correction).to_bits()
        },
        {
            let y = f32::from_bits(a[1]);
            let correction = f32::from_bits(b[1]);
            (y + y * correction).to_bits()
        },
        {
            let y = f32::from_bits(a[2]);
            let correction = f32::from_bits(b[2]);
            (y + y * correction).to_bits()
        },
        {
            let y = f32::from_bits(a[3]);
            let correction = f32::from_bits(b[3]);
            (y + y * correction).to_bits()
        },
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Convert Floating to Signed Integer - cflts rt, ra, i8 (scale)
pub fn cflts(thread: &mut SpuThread, i8_scale: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let scale = 2.0f32.powi((i8_scale as i32) - 155);
    let result = [
        float_to_signed(f32::from_bits(a[0]) * scale),
        float_to_signed(f32::from_bits(a[1]) * scale),
        float_to_signed(f32::from_bits(a[2]) * scale),
        float_to_signed(f32::from_bits(a[3]) * scale),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Convert Floating to Unsigned Integer - cfltu rt, ra, i8 (scale)
pub fn cfltu(thread: &mut SpuThread, i8_scale: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let scale = 2.0f32.powi((i8_scale as i32) - 155);
    let result = [
        float_to_unsigned(f32::from_bits(a[0]) * scale),
        float_to_unsigned(f32::from_bits(a[1]) * scale),
        float_to_unsigned(f32::from_bits(a[2]) * scale),
        float_to_unsigned(f32::from_bits(a[3]) * scale),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Convert Signed Integer to Floating - csflt rt, ra, i8 (scale)
pub fn csflt(thread: &mut SpuThread, i8_scale: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let scale = 2.0f32.powi(155 - (i8_scale as i32));
    let result = [
        ((a[0] as i32 as f32) * scale).to_bits(),
        ((a[1] as i32 as f32) * scale).to_bits(),
        ((a[2] as i32 as f32) * scale).to_bits(),
        ((a[3] as i32 as f32) * scale).to_bits(),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Convert Unsigned Integer to Floating - cuflt rt, ra, i8 (scale)
pub fn cuflt(thread: &mut SpuThread, i8_scale: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let scale = 2.0f32.powi(155 - (i8_scale as i32));
    let result = [
        ((a[0] as f32) * scale).to_bits(),
        ((a[1] as f32) * scale).to_bits(),
        ((a[2] as f32) * scale).to_bits(),
        ((a[3] as f32) * scale).to_bits(),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Helper: Convert float to signed integer with saturation
fn float_to_signed(x: f32) -> u32 {
    if x.is_nan() {
        0
    } else if x >= i32::MAX as f32 {
        i32::MAX as u32
    } else if x <= i32::MIN as f32 {
        i32::MIN as u32
    } else {
        (x as i32) as u32
    }
}

/// Helper: Convert float to unsigned integer with saturation
fn float_to_unsigned(x: f32) -> u32 {
    if x.is_nan() || x < 0.0 {
        0
    } else if x >= u32::MAX as f32 {
        u32::MAX
    } else {
        x as u32
    }
}

/// Floating Compare Greater Than - fcgt rt, ra, rb
pub fn fcgt(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        if f32::from_bits(a[0]) > f32::from_bits(b[0]) { 0xFFFFFFFF } else { 0 },
        if f32::from_bits(a[1]) > f32::from_bits(b[1]) { 0xFFFFFFFF } else { 0 },
        if f32::from_bits(a[2]) > f32::from_bits(b[2]) { 0xFFFFFFFF } else { 0 },
        if f32::from_bits(a[3]) > f32::from_bits(b[3]) { 0xFFFFFFFF } else { 0 },
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Floating Compare Magnitude Greater Than - fcmgt rt, ra, rb
pub fn fcmgt(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        if f32::from_bits(a[0]).abs() > f32::from_bits(b[0]).abs() { 0xFFFFFFFF } else { 0 },
        if f32::from_bits(a[1]).abs() > f32::from_bits(b[1]).abs() { 0xFFFFFFFF } else { 0 },
        if f32::from_bits(a[2]).abs() > f32::from_bits(b[2]).abs() { 0xFFFFFFFF } else { 0 },
        if f32::from_bits(a[3]).abs() > f32::from_bits(b[3]).abs() { 0xFFFFFFFF } else { 0 },
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Floating Compare Equal - fceq rt, ra, rb
pub fn fceq(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        if f32::from_bits(a[0]) == f32::from_bits(b[0]) { 0xFFFFFFFF } else { 0 },
        if f32::from_bits(a[1]) == f32::from_bits(b[1]) { 0xFFFFFFFF } else { 0 },
        if f32::from_bits(a[2]) == f32::from_bits(b[2]) { 0xFFFFFFFF } else { 0 },
        if f32::from_bits(a[3]) == f32::from_bits(b[3]) { 0xFFFFFFFF } else { 0 },
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Floating Compare Magnitude Equal - fcmeq rt, ra, rb
pub fn fcmeq(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        if f32::from_bits(a[0]).abs() == f32::from_bits(b[0]).abs() { 0xFFFFFFFF } else { 0 },
        if f32::from_bits(a[1]).abs() == f32::from_bits(b[1]).abs() { 0xFFFFFFFF } else { 0 },
        if f32::from_bits(a[2]).abs() == f32::from_bits(b[2]).abs() { 0xFFFFFFFF } else { 0 },
        if f32::from_bits(a[3]).abs() == f32::from_bits(b[3]).abs() { 0xFFFFFFFF } else { 0 },
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Double Floating Add - dfa rt, ra, rb
pub fn dfa(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    // Treat as two 64-bit doubles
    let a0 = f64::from_bits(((a[0] as u64) << 32) | (a[1] as u64));
    let a1 = f64::from_bits(((a[2] as u64) << 32) | (a[3] as u64));
    let b0 = f64::from_bits(((b[0] as u64) << 32) | (b[1] as u64));
    let b1 = f64::from_bits(((b[2] as u64) << 32) | (b[3] as u64));
    let r0 = (a0 + b0).to_bits();
    let r1 = (a1 + b1).to_bits();
    let result = [
        (r0 >> 32) as u32,
        r0 as u32,
        (r1 >> 32) as u32,
        r1 as u32,
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Double Floating Subtract - dfs rt, ra, rb
pub fn dfs(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let a0 = f64::from_bits(((a[0] as u64) << 32) | (a[1] as u64));
    let a1 = f64::from_bits(((a[2] as u64) << 32) | (a[3] as u64));
    let b0 = f64::from_bits(((b[0] as u64) << 32) | (b[1] as u64));
    let b1 = f64::from_bits(((b[2] as u64) << 32) | (b[3] as u64));
    let r0 = (a0 - b0).to_bits();
    let r1 = (a1 - b1).to_bits();
    let result = [
        (r0 >> 32) as u32,
        r0 as u32,
        (r1 >> 32) as u32,
        r1 as u32,
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Double Floating Multiply - dfm rt, ra, rb
pub fn dfm(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let a0 = f64::from_bits(((a[0] as u64) << 32) | (a[1] as u64));
    let a1 = f64::from_bits(((a[2] as u64) << 32) | (a[3] as u64));
    let b0 = f64::from_bits(((b[0] as u64) << 32) | (b[1] as u64));
    let b1 = f64::from_bits(((b[2] as u64) << 32) | (b[3] as u64));
    let r0 = (a0 * b0).to_bits();
    let r1 = (a1 * b1).to_bits();
    let result = [
        (r0 >> 32) as u32,
        r0 as u32,
        (r1 >> 32) as u32,
        r1 as u32,
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Double Floating Multiply and Add - dfma rt, ra, rb
pub fn dfma(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let t = thread.regs.read_u32x4(rt as usize);
    let a0 = f64::from_bits(((a[0] as u64) << 32) | (a[1] as u64));
    let a1 = f64::from_bits(((a[2] as u64) << 32) | (a[3] as u64));
    let b0 = f64::from_bits(((b[0] as u64) << 32) | (b[1] as u64));
    let b1 = f64::from_bits(((b[2] as u64) << 32) | (b[3] as u64));
    let t0 = f64::from_bits(((t[0] as u64) << 32) | (t[1] as u64));
    let t1 = f64::from_bits(((t[2] as u64) << 32) | (t[3] as u64));
    let r0 = a0.mul_add(b0, t0).to_bits();
    let r1 = a1.mul_add(b1, t1).to_bits();
    let result = [
        (r0 >> 32) as u32,
        r0 as u32,
        (r1 >> 32) as u32,
        r1 as u32,
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Double Floating Multiply and Subtract - dfms rt, ra, rb
pub fn dfms(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let t = thread.regs.read_u32x4(rt as usize);
    let a0 = f64::from_bits(((a[0] as u64) << 32) | (a[1] as u64));
    let a1 = f64::from_bits(((a[2] as u64) << 32) | (a[3] as u64));
    let b0 = f64::from_bits(((b[0] as u64) << 32) | (b[1] as u64));
    let b1 = f64::from_bits(((b[2] as u64) << 32) | (b[3] as u64));
    let t0 = f64::from_bits(((t[0] as u64) << 32) | (t[1] as u64));
    let t1 = f64::from_bits(((t[2] as u64) << 32) | (t[3] as u64));
    let r0 = (a0 * b0 - t0).to_bits();
    let r1 = (a1 * b1 - t1).to_bits();
    let result = [
        (r0 >> 32) as u32,
        r0 as u32,
        (r1 >> 32) as u32,
        r1 as u32,
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Double Floating Negative Multiply and Subtract - dfnms rt, ra, rb
pub fn dfnms(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let t = thread.regs.read_u32x4(rt as usize);
    let a0 = f64::from_bits(((a[0] as u64) << 32) | (a[1] as u64));
    let a1 = f64::from_bits(((a[2] as u64) << 32) | (a[3] as u64));
    let b0 = f64::from_bits(((b[0] as u64) << 32) | (b[1] as u64));
    let b1 = f64::from_bits(((b[2] as u64) << 32) | (b[3] as u64));
    let t0 = f64::from_bits(((t[0] as u64) << 32) | (t[1] as u64));
    let t1 = f64::from_bits(((t[2] as u64) << 32) | (t[3] as u64));
    let r0 = (t0 - a0 * b0).to_bits();
    let r1 = (t1 - a1 * b1).to_bits();
    let result = [
        (r0 >> 32) as u32,
        r0 as u32,
        (r1 >> 32) as u32,
        r1 as u32,
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Double Floating Negative Multiply and Add - dfnma rt, ra, rb
pub fn dfnma(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let t = thread.regs.read_u32x4(rt as usize);
    let a0 = f64::from_bits(((a[0] as u64) << 32) | (a[1] as u64));
    let a1 = f64::from_bits(((a[2] as u64) << 32) | (a[3] as u64));
    let b0 = f64::from_bits(((b[0] as u64) << 32) | (b[1] as u64));
    let b1 = f64::from_bits(((b[2] as u64) << 32) | (b[3] as u64));
    let t0 = f64::from_bits(((t[0] as u64) << 32) | (t[1] as u64));
    let t1 = f64::from_bits(((t[2] as u64) << 32) | (t[3] as u64));
    let r0 = (-(a0 * b0 + t0)).to_bits();
    let r1 = (-(a1 * b1 + t1)).to_bits();
    let result = [
        (r0 >> 32) as u32,
        r0 as u32,
        (r1 >> 32) as u32,
        r1 as u32,
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Floating Round Double to Single - frds rt, ra
pub fn frds(thread: &mut SpuThread, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    // Convert double in slots 0-1 and 2-3 to single in slots 0 and 2
    let d0 = f64::from_bits(((a[0] as u64) << 32) | (a[1] as u64));
    let d1 = f64::from_bits(((a[2] as u64) << 32) | (a[3] as u64));
    let result = [
        (d0 as f32).to_bits(),
        0,
        (d1 as f32).to_bits(),
        0,
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Floating Extend Single to Double - fesd rt, ra
pub fn fesd(thread: &mut SpuThread, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    // Convert single in slot 0 and 2 to double in slots 0-1 and 2-3
    let s0 = f32::from_bits(a[0]);
    let s1 = f32::from_bits(a[2]);
    let d0 = (s0 as f64).to_bits();
    let d1 = (s1 as f64).to_bits();
    let result = [
        (d0 >> 32) as u32,
        d0 as u32,
        (d1 >> 32) as u32,
        d1 as u32,
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Double Floating Compare Equal - dfceq rt, ra, rb
/// Compares two pairs of double-precision floats for equality
pub fn dfceq(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    // Extract two doubles from each register
    let a0 = f64::from_bits(((a[0] as u64) << 32) | (a[1] as u64));
    let a1 = f64::from_bits(((a[2] as u64) << 32) | (a[3] as u64));
    let b0 = f64::from_bits(((b[0] as u64) << 32) | (b[1] as u64));
    let b1 = f64::from_bits(((b[2] as u64) << 32) | (b[3] as u64));
    // Compare: all bits set if equal, 0 otherwise
    let m0: u64 = if a0 == b0 { 0xFFFFFFFFFFFFFFFF } else { 0 };
    let m1: u64 = if a1 == b1 { 0xFFFFFFFFFFFFFFFF } else { 0 };
    let result = [
        (m0 >> 32) as u32,
        m0 as u32,
        (m1 >> 32) as u32,
        m1 as u32,
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Double Floating Compare Greater Than - dfcgt rt, ra, rb
/// Compares two pairs of double-precision floats for greater than
pub fn dfcgt(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let a0 = f64::from_bits(((a[0] as u64) << 32) | (a[1] as u64));
    let a1 = f64::from_bits(((a[2] as u64) << 32) | (a[3] as u64));
    let b0 = f64::from_bits(((b[0] as u64) << 32) | (b[1] as u64));
    let b1 = f64::from_bits(((b[2] as u64) << 32) | (b[3] as u64));
    let m0: u64 = if a0 > b0 { 0xFFFFFFFFFFFFFFFF } else { 0 };
    let m1: u64 = if a1 > b1 { 0xFFFFFFFFFFFFFFFF } else { 0 };
    let result = [
        (m0 >> 32) as u32,
        m0 as u32,
        (m1 >> 32) as u32,
        m1 as u32,
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Double Floating Compare Magnitude Equal - dfcmeq rt, ra, rb
/// Compares absolute values of two pairs of double-precision floats for equality
pub fn dfcmeq(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let a0 = f64::from_bits(((a[0] as u64) << 32) | (a[1] as u64));
    let a1 = f64::from_bits(((a[2] as u64) << 32) | (a[3] as u64));
    let b0 = f64::from_bits(((b[0] as u64) << 32) | (b[1] as u64));
    let b1 = f64::from_bits(((b[2] as u64) << 32) | (b[3] as u64));
    let m0: u64 = if a0.abs() == b0.abs() { 0xFFFFFFFFFFFFFFFF } else { 0 };
    let m1: u64 = if a1.abs() == b1.abs() { 0xFFFFFFFFFFFFFFFF } else { 0 };
    let result = [
        (m0 >> 32) as u32,
        m0 as u32,
        (m1 >> 32) as u32,
        m1 as u32,
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Double Floating Compare Magnitude Greater Than - dfcmgt rt, ra, rb
/// Compares absolute values of two pairs of double-precision floats for greater than
pub fn dfcmgt(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let a0 = f64::from_bits(((a[0] as u64) << 32) | (a[1] as u64));
    let a1 = f64::from_bits(((a[2] as u64) << 32) | (a[3] as u64));
    let b0 = f64::from_bits(((b[0] as u64) << 32) | (b[1] as u64));
    let b1 = f64::from_bits(((b[2] as u64) << 32) | (b[3] as u64));
    let m0: u64 = if a0.abs() > b0.abs() { 0xFFFFFFFFFFFFFFFF } else { 0 };
    let m1: u64 = if a1.abs() > b1.abs() { 0xFFFFFFFFFFFFFFFF } else { 0 };
    let result = [
        (m0 >> 32) as u32,
        m0 as u32,
        (m1 >> 32) as u32,
        m1 as u32,
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use oc_memory::MemoryManager;

    fn create_test_thread() -> SpuThread {
        let memory = MemoryManager::new().unwrap();
        SpuThread::new(0, memory)
    }

    #[test]
    fn test_fa() {
        let mut thread = create_test_thread();
        thread.regs.write_u32x4(1, [1.0f32.to_bits(), 2.0f32.to_bits(), 3.0f32.to_bits(), 4.0f32.to_bits()]);
        thread.regs.write_u32x4(2, [5.0f32.to_bits(), 6.0f32.to_bits(), 7.0f32.to_bits(), 8.0f32.to_bits()]);
        
        fa(&mut thread, 2, 1, 3).unwrap();
        
        let result = thread.regs.read_u32x4(3);
        assert_eq!(f32::from_bits(result[0]), 6.0);
        assert_eq!(f32::from_bits(result[1]), 8.0);
        assert_eq!(f32::from_bits(result[2]), 10.0);
        assert_eq!(f32::from_bits(result[3]), 12.0);
    }

    #[test]
    fn test_fm() {
        let mut thread = create_test_thread();
        thread.regs.write_u32x4(1, [2.0f32.to_bits(), 3.0f32.to_bits(), 4.0f32.to_bits(), 5.0f32.to_bits()]);
        thread.regs.write_u32x4(2, [3.0f32.to_bits(), 4.0f32.to_bits(), 5.0f32.to_bits(), 6.0f32.to_bits()]);
        
        fm(&mut thread, 2, 1, 3).unwrap();
        
        let result = thread.regs.read_u32x4(3);
        assert_eq!(f32::from_bits(result[0]), 6.0);
        assert_eq!(f32::from_bits(result[1]), 12.0);
        assert_eq!(f32::from_bits(result[2]), 20.0);
        assert_eq!(f32::from_bits(result[3]), 30.0);
    }

    #[test]
    fn test_frest() {
        let mut thread = create_test_thread();
        thread.regs.write_u32x4(1, [2.0f32.to_bits(), 4.0f32.to_bits(), 8.0f32.to_bits(), 10.0f32.to_bits()]);
        
        frest(&mut thread, 1, 2).unwrap();
        
        let result = thread.regs.read_u32x4(2);
        assert!((f32::from_bits(result[0]) - 0.5).abs() < 0.001);
        assert!((f32::from_bits(result[1]) - 0.25).abs() < 0.001);
        assert!((f32::from_bits(result[2]) - 0.125).abs() < 0.001);
        assert!((f32::from_bits(result[3]) - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_dfa() {
        let mut thread = create_test_thread();
        // Set up two doubles: 1.5 and 2.5 in ra, 0.5 and 1.0 in rb
        let d1 = 1.5f64.to_bits();
        let d2 = 2.5f64.to_bits();
        let d3 = 0.5f64.to_bits();
        let d4 = 1.0f64.to_bits();
        thread.regs.write_u32x4(1, [
            (d1 >> 32) as u32, d1 as u32,
            (d2 >> 32) as u32, d2 as u32,
        ]);
        thread.regs.write_u32x4(2, [
            (d3 >> 32) as u32, d3 as u32,
            (d4 >> 32) as u32, d4 as u32,
        ]);
        
        dfa(&mut thread, 2, 1, 3).unwrap();
        
        let result = thread.regs.read_u32x4(3);
        let r0 = f64::from_bits(((result[0] as u64) << 32) | (result[1] as u64));
        let r1 = f64::from_bits(((result[2] as u64) << 32) | (result[3] as u64));
        assert!((r0 - 2.0).abs() < 0.0001);  // 1.5 + 0.5 = 2.0
        assert!((r1 - 3.5).abs() < 0.0001);  // 2.5 + 1.0 = 3.5
    }

    #[test]
    fn test_dfceq() {
        let mut thread = create_test_thread();
        let d1 = 1.5f64.to_bits();
        let d2 = 2.5f64.to_bits();
        let d3 = 1.5f64.to_bits(); // Same as d1
        let d4 = 3.0f64.to_bits(); // Different
        thread.regs.write_u32x4(1, [
            (d1 >> 32) as u32, d1 as u32,
            (d2 >> 32) as u32, d2 as u32,
        ]);
        thread.regs.write_u32x4(2, [
            (d3 >> 32) as u32, d3 as u32,
            (d4 >> 32) as u32, d4 as u32,
        ]);
        
        dfceq(&mut thread, 2, 1, 3).unwrap();
        
        let result = thread.regs.read_u32x4(3);
        // First double should be equal (all ones)
        assert_eq!(result[0], 0xFFFFFFFF);
        assert_eq!(result[1], 0xFFFFFFFF);
        // Second double should not be equal (all zeros)
        assert_eq!(result[2], 0);
        assert_eq!(result[3], 0);
    }

    #[test]
    fn test_dfcgt() {
        let mut thread = create_test_thread();
        let d1 = 2.0f64.to_bits();  // 2.0 > 1.0
        let d2 = 1.0f64.to_bits();  // 1.0 < 2.0
        let d3 = 1.0f64.to_bits();
        let d4 = 2.0f64.to_bits();
        thread.regs.write_u32x4(1, [
            (d1 >> 32) as u32, d1 as u32,
            (d2 >> 32) as u32, d2 as u32,
        ]);
        thread.regs.write_u32x4(2, [
            (d3 >> 32) as u32, d3 as u32,
            (d4 >> 32) as u32, d4 as u32,
        ]);
        
        dfcgt(&mut thread, 2, 1, 3).unwrap();
        
        let result = thread.regs.read_u32x4(3);
        // 2.0 > 1.0 is true
        assert_eq!(result[0], 0xFFFFFFFF);
        assert_eq!(result[1], 0xFFFFFFFF);
        // 1.0 > 2.0 is false
        assert_eq!(result[2], 0);
        assert_eq!(result[3], 0);
    }

    #[test]
    fn test_fesd() {
        let mut thread = create_test_thread();
        // Single precision floats in slots 0 and 2
        thread.regs.write_u32x4(1, [
            1.5f32.to_bits(),
            0,  // ignored
            2.5f32.to_bits(),
            0,  // ignored
        ]);
        
        fesd(&mut thread, 1, 2).unwrap();
        
        let result = thread.regs.read_u32x4(2);
        let r0 = f64::from_bits(((result[0] as u64) << 32) | (result[1] as u64));
        let r1 = f64::from_bits(((result[2] as u64) << 32) | (result[3] as u64));
        assert!((r0 - 1.5).abs() < 0.0001);
        assert!((r1 - 2.5).abs() < 0.0001);
    }

    #[test]
    fn test_frds() {
        let mut thread = create_test_thread();
        let d1 = 1.5f64.to_bits();
        let d2 = 2.5f64.to_bits();
        thread.regs.write_u32x4(1, [
            (d1 >> 32) as u32, d1 as u32,
            (d2 >> 32) as u32, d2 as u32,
        ]);
        
        frds(&mut thread, 1, 2).unwrap();
        
        let result = thread.regs.read_u32x4(2);
        assert!((f32::from_bits(result[0]) - 1.5).abs() < 0.0001);
        assert_eq!(result[1], 0); // Low word of first slot is 0
        assert!((f32::from_bits(result[2]) - 2.5).abs() < 0.0001);
        assert_eq!(result[3], 0); // Low word of second slot is 0
    }
}
