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
}
