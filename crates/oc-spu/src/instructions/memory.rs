//! SPU memory instructions

use crate::thread::SpuThread;
use oc_core::error::SpuError;

/// Load Quadword (d-form) - lqd rt, i10(ra)
pub fn lqd(thread: &mut SpuThread, i10: i16, ra: u8, rt: u8) -> Result<(), SpuError> {
    let base = thread.regs.read_preferred_u32(ra as usize);
    let addr = ((base as i32).wrapping_add((i10 as i32) << 4)) as u32;
    let value = thread.ls_read_u128(addr);
    thread.regs.write_u32x4(rt as usize, value);
    thread.advance_pc();
    Ok(())
}

/// Load Quadword (a-form) - lqa rt, i16
pub fn lqa(thread: &mut SpuThread, i16_val: i16, rt: u8) -> Result<(), SpuError> {
    // Sign-extend to i32 first, then shift and cast to u32
    let addr = ((i16_val as i32) << 2) as u32;
    let value = thread.ls_read_u128(addr);
    thread.regs.write_u32x4(rt as usize, value);
    thread.advance_pc();
    Ok(())
}

/// Load Quadword (x-form) - lqx rt, ra, rb
pub fn lqx(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_preferred_u32(ra as usize);
    let b = thread.regs.read_preferred_u32(rb as usize);
    let addr = a.wrapping_add(b);
    let value = thread.ls_read_u128(addr);
    thread.regs.write_u32x4(rt as usize, value);
    thread.advance_pc();
    Ok(())
}

/// Load Quadword Instruction Relative (a-form) - lqr rt, i16
pub fn lqr(thread: &mut SpuThread, i16_val: i16, rt: u8) -> Result<(), SpuError> {
    let pc = thread.pc();
    let offset = (i16_val as i32) << 2;
    let addr = (pc as i32).wrapping_add(offset) as u32;
    let value = thread.ls_read_u128(addr);
    thread.regs.write_u32x4(rt as usize, value);
    thread.advance_pc();
    Ok(())
}

/// Store Quadword (d-form) - stqd rt, i10(ra)
pub fn stqd(thread: &mut SpuThread, i10: i16, ra: u8, rt: u8) -> Result<(), SpuError> {
    let base = thread.regs.read_preferred_u32(ra as usize);
    let addr = ((base as i32).wrapping_add((i10 as i32) << 4)) as u32;
    let value = thread.regs.read_u32x4(rt as usize);
    thread.ls_write_u128(addr, value);
    thread.advance_pc();
    Ok(())
}

/// Store Quadword (a-form) - stqa rt, i16
pub fn stqa(thread: &mut SpuThread, i16_val: i16, rt: u8) -> Result<(), SpuError> {
    let addr = ((i16_val as i32) << 2) as u32;
    let value = thread.regs.read_u32x4(rt as usize);
    thread.ls_write_u128(addr, value);
    thread.advance_pc();
    Ok(())
}

/// Store Quadword (x-form) - stqx rt, ra, rb
pub fn stqx(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_preferred_u32(ra as usize);
    let b = thread.regs.read_preferred_u32(rb as usize);
    let addr = a.wrapping_add(b);
    let value = thread.regs.read_u32x4(rt as usize);
    thread.ls_write_u128(addr, value);
    thread.advance_pc();
    Ok(())
}

/// Store Quadword Instruction Relative (a-form) - stqr rt, i16
pub fn stqr(thread: &mut SpuThread, i16_val: i16, rt: u8) -> Result<(), SpuError> {
    let pc = thread.pc();
    let offset = (i16_val as i32) << 2;
    let addr = (pc as i32).wrapping_add(offset) as u32;
    let value = thread.regs.read_u32x4(rt as usize);
    thread.ls_write_u128(addr, value);
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
    fn test_lqd_stqd() {
        let mut thread = create_test_thread();
        
        // Store a value
        thread.regs.write_u32x4(1, [0x11111111, 0x22222222, 0x33333333, 0x44444444]);
        thread.regs.write_u32x4(2, [0x100, 0, 0, 0]); // Base address
        
        stqd(&mut thread, 0, 2, 1).unwrap();
        
        // Load it back
        thread.regs.write_u32x4(3, [0, 0, 0, 0]); // Clear target
        lqd(&mut thread, 0, 2, 3).unwrap();
        
        let result = thread.regs.read_u32x4(3);
        assert_eq!(result, [0x11111111, 0x22222222, 0x33333333, 0x44444444]);
    }

    #[test]
    fn test_lqa_stqa() {
        let mut thread = create_test_thread();
        
        thread.regs.write_u32x4(1, [0xAAAAAAAA, 0xBBBBBBBB, 0xCCCCCCCC, 0xDDDDDDDD]);
        
        stqa(&mut thread, 0x40, 1).unwrap(); // Store at absolute address 0x100
        
        thread.regs.write_u32x4(2, [0, 0, 0, 0]);
        lqa(&mut thread, 0x40, 2).unwrap(); // Load from absolute address 0x100
        
        let result = thread.regs.read_u32x4(2);
        assert_eq!(result, [0xAAAAAAAA, 0xBBBBBBBB, 0xCCCCCCCC, 0xDDDDDDDD]);
    }

    #[test]
    fn test_lqr_stqr() {
        let mut thread = create_test_thread();
        
        thread.set_pc(0x200);
        thread.regs.write_u32x4(1, [0x12345678, 0x9ABCDEF0, 0x11223344, 0x55667788]);
        
        stqr(&mut thread, 0x10, 1).unwrap(); // Store at PC + 0x40
        
        thread.set_pc(0x200);
        thread.regs.write_u32x4(2, [0, 0, 0, 0]);
        lqr(&mut thread, 0x10, 2).unwrap(); // Load from PC + 0x40
        
        let result = thread.regs.read_u32x4(2);
        assert_eq!(result, [0x12345678, 0x9ABCDEF0, 0x11223344, 0x55667788]);
    }
}
