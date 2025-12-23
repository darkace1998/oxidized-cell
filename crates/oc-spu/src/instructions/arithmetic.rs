//! SPU arithmetic instructions

use crate::thread::SpuThread;
use oc_core::error::SpuError;

/// Multiply - mpy rt, ra, rb
pub fn mpy(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        ((a[0] as i32).wrapping_mul(b[0] as i32)) as u32,
        ((a[1] as i32).wrapping_mul(b[1] as i32)) as u32,
        ((a[2] as i32).wrapping_mul(b[2] as i32)) as u32,
        ((a[3] as i32).wrapping_mul(b[3] as i32)) as u32,
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Multiply Unsigned - mpyu rt, ra, rb
pub fn mpyu(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        (a[0] & 0xFFFF).wrapping_mul(b[0] & 0xFFFF),
        (a[1] & 0xFFFF).wrapping_mul(b[1] & 0xFFFF),
        (a[2] & 0xFFFF).wrapping_mul(b[2] & 0xFFFF),
        (a[3] & 0xFFFF).wrapping_mul(b[3] & 0xFFFF),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Multiply High - mpyh rt, ra, rb
pub fn mpyh(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        ((a[0] >> 16) * (b[0] & 0xFFFF)) << 16,
        ((a[1] >> 16) * (b[1] & 0xFFFF)) << 16,
        ((a[2] >> 16) * (b[2] & 0xFFFF)) << 16,
        ((a[3] >> 16) * (b[3] & 0xFFFF)) << 16,
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Shift Left Word - shl rt, ra, rb
pub fn shl(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        a[0] << (b[0] & 0x3F),
        a[1] << (b[1] & 0x3F),
        a[2] << (b[2] & 0x3F),
        a[3] << (b[3] & 0x3F),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Shift Left Word Immediate - shli rt, ra, i7
pub fn shli(thread: &mut SpuThread, i7: i8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let shift = (i7 & 0x3F) as u32;
    let result = [
        a[0] << shift,
        a[1] << shift,
        a[2] << shift,
        a[3] << shift,
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Rotate Word - rot rt, ra, rb
pub fn rot(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        a[0].rotate_left(b[0] & 0x1F),
        a[1].rotate_left(b[1] & 0x1F),
        a[2].rotate_left(b[2] & 0x1F),
        a[3].rotate_left(b[3] & 0x1F),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Rotate Word Immediate - roti rt, ra, i7
pub fn roti(thread: &mut SpuThread, i7: i8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let shift = (i7 & 0x1F) as u32;
    let result = [
        a[0].rotate_left(shift),
        a[1].rotate_left(shift),
        a[2].rotate_left(shift),
        a[3].rotate_left(shift),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Add Halfword - ah rt, ra, rb
pub fn ah(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let mut result = [0u32; 4];
    for i in 0..4 {
        let hi = ((a[i] >> 16) as u16).wrapping_add((b[i] >> 16) as u16);
        let lo = ((a[i] & 0xFFFF) as u16).wrapping_add((b[i] & 0xFFFF) as u16);
        result[i] = ((hi as u32) << 16) | (lo as u32);
    }
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Subtract Halfword - sfh rt, ra, rb
pub fn sfh(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let mut result = [0u32; 4];
    for i in 0..4 {
        let hi = ((b[i] >> 16) as u16).wrapping_sub((a[i] >> 16) as u16);
        let lo = ((b[i] & 0xFFFF) as u16).wrapping_sub((a[i] & 0xFFFF) as u16);
        result[i] = ((hi as u32) << 16) | (lo as u32);
    }
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
    fn test_mpy() {
        let mut thread = create_test_thread();
        thread.regs.write_u32x4(1, [2, 3, 4, 5]);
        thread.regs.write_u32x4(2, [3, 4, 5, 6]);
        
        mpy(&mut thread, 2, 1, 3).unwrap();
        
        let result = thread.regs.read_u32x4(3);
        assert_eq!(result, [6, 12, 20, 30]);
    }

    #[test]
    fn test_shl() {
        let mut thread = create_test_thread();
        thread.regs.write_u32x4(1, [1, 2, 4, 8]);
        thread.regs.write_u32x4(2, [1, 2, 3, 4]);
        
        shl(&mut thread, 2, 1, 3).unwrap();
        
        let result = thread.regs.read_u32x4(3);
        assert_eq!(result, [2, 8, 32, 128]);
    }

    #[test]
    fn test_rot() {
        let mut thread = create_test_thread();
        thread.regs.write_u32x4(1, [0x12345678, 0xABCDEF00, 0x11223344, 0x55667788]);
        thread.regs.write_u32x4(2, [8, 16, 4, 12]);
        
        rot(&mut thread, 2, 1, 3).unwrap();
        
        let result = thread.regs.read_u32x4(3);
        assert_eq!(result[0], 0x12345678u32.rotate_left(8));
        assert_eq!(result[1], 0xABCDEF00u32.rotate_left(16));
    }
}
