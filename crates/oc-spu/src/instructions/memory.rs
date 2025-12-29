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

/// Immediate Load Word - il rt, i16
pub fn il(thread: &mut SpuThread, i16_val: i16, rt: u8) -> Result<(), SpuError> {
    // Sign-extend to 32 bits and replicate across all words
    let value = i16_val as i32 as u32;
    thread.regs.write_u32x4(rt as usize, [value, value, value, value]);
    thread.advance_pc();
    Ok(())
}

/// Immediate Load Halfword - ilh rt, i16
pub fn ilh(thread: &mut SpuThread, i16_val: i16, rt: u8) -> Result<(), SpuError> {
    // Replicate halfword across all halfword positions
    let hword = i16_val as u16;
    let value = ((hword as u32) << 16) | (hword as u32);
    thread.regs.write_u32x4(rt as usize, [value, value, value, value]);
    thread.advance_pc();
    Ok(())
}

/// Immediate Load Word Upper - ilhu rt, i16
pub fn ilhu(thread: &mut SpuThread, i16_val: i16, rt: u8) -> Result<(), SpuError> {
    // Load immediate into upper 16 bits, lower 16 bits are zero
    let value = ((i16_val as u16 as u32) << 16) | 0;
    thread.regs.write_u32x4(rt as usize, [value, value, value, value]);
    thread.advance_pc();
    Ok(())
}

/// Immediate Load Address - ila rt, i18
pub fn ila(thread: &mut SpuThread, i18: i32, rt: u8) -> Result<(), SpuError> {
    // Zero-extend 18-bit immediate (no sign extension)
    let value = (i18 & 0x3FFFF) as u32;
    thread.regs.write_u32x4(rt as usize, [value, value, value, value]);
    thread.advance_pc();
    Ok(())
}

/// Immediate Or Halfword Lower - iohl rt, i16
pub fn iohl(thread: &mut SpuThread, i16_val: i16, rt: u8) -> Result<(), SpuError> {
    // OR the lower 16 bits with the immediate
    let a = thread.regs.read_u32x4(rt as usize);
    let imm = (i16_val as u16) as u32;
    let result = [
        a[0] | imm,
        a[1] | imm,
        a[2] | imm,
        a[3] | imm,
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Copy Halfword to Byte Insert - cbd rt, i7(ra)
pub fn cbd(thread: &mut SpuThread, i7: i8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let base = thread.regs.read_preferred_u32(ra as usize);
    let addr = ((base as i32).wrapping_add(i7 as i32)) as u32;
    let byte_index = (addr & 0xF) as usize;
    
    // Generate shuffle mask for inserting byte from preferred slot
    let mut mask = [0u8; 16];
    for i in 0..16 {
        mask[i] = if i == byte_index { 0x03 } else { (0x10 + i) as u8 };
    }
    
    let result = [
        u32::from_be_bytes([mask[0], mask[1], mask[2], mask[3]]),
        u32::from_be_bytes([mask[4], mask[5], mask[6], mask[7]]),
        u32::from_be_bytes([mask[8], mask[9], mask[10], mask[11]]),
        u32::from_be_bytes([mask[12], mask[13], mask[14], mask[15]]),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Copy Halfword to Halfword Insert - chd rt, i7(ra)
pub fn chd(thread: &mut SpuThread, i7: i8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let base = thread.regs.read_preferred_u32(ra as usize);
    let addr = ((base as i32).wrapping_add(i7 as i32)) as u32;
    let hword_index = ((addr >> 1) & 0x7) as usize;
    
    // Generate shuffle mask for inserting halfword from preferred slot
    let mut mask = [0u8; 16];
    for i in 0..16 {
        let hword_pos = i / 2;
        if hword_pos == hword_index {
            mask[i] = if i % 2 == 0 { 0x02 } else { 0x03 };
        } else {
            mask[i] = (0x10 + i) as u8;
        }
    }
    
    let result = [
        u32::from_be_bytes([mask[0], mask[1], mask[2], mask[3]]),
        u32::from_be_bytes([mask[4], mask[5], mask[6], mask[7]]),
        u32::from_be_bytes([mask[8], mask[9], mask[10], mask[11]]),
        u32::from_be_bytes([mask[12], mask[13], mask[14], mask[15]]),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Copy Halfword to Word Insert - cwd rt, i7(ra)
pub fn cwd(thread: &mut SpuThread, i7: i8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let base = thread.regs.read_preferred_u32(ra as usize);
    let addr = ((base as i32).wrapping_add(i7 as i32)) as u32;
    let word_index = ((addr >> 2) & 0x3) as usize;
    
    // Generate shuffle mask for inserting word from preferred slot
    let mut mask = [0u8; 16];
    for i in 0..16 {
        let word_pos = i / 4;
        if word_pos == word_index {
            mask[i] = (i % 4) as u8;  // Select bytes 0-3 from ra
        } else {
            mask[i] = (0x10 + i) as u8;
        }
    }
    
    let result = [
        u32::from_be_bytes([mask[0], mask[1], mask[2], mask[3]]),
        u32::from_be_bytes([mask[4], mask[5], mask[6], mask[7]]),
        u32::from_be_bytes([mask[8], mask[9], mask[10], mask[11]]),
        u32::from_be_bytes([mask[12], mask[13], mask[14], mask[15]]),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Copy Halfword to Doubleword Insert - cdd rt, i7(ra)
pub fn cdd(thread: &mut SpuThread, i7: i8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let base = thread.regs.read_preferred_u32(ra as usize);
    let addr = ((base as i32).wrapping_add(i7 as i32)) as u32;
    let dword_index = ((addr >> 3) & 0x1) as usize;
    
    // Generate shuffle mask for inserting doubleword
    let mut mask = [0u8; 16];
    for i in 0..16 {
        let dword_pos = i / 8;
        if dword_pos == dword_index {
            mask[i] = (i % 8) as u8;  // Select bytes 0-7 from ra
        } else {
            mask[i] = (0x10 + i) as u8;
        }
    }
    
    let result = [
        u32::from_be_bytes([mask[0], mask[1], mask[2], mask[3]]),
        u32::from_be_bytes([mask[4], mask[5], mask[6], mask[7]]),
        u32::from_be_bytes([mask[8], mask[9], mask[10], mask[11]]),
        u32::from_be_bytes([mask[12], mask[13], mask[14], mask[15]]),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Copy Byte to Byte Insert (x-form) - cbx rt, ra, rb
pub fn cbx(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_preferred_u32(ra as usize);
    let b = thread.regs.read_preferred_u32(rb as usize);
    let addr = a.wrapping_add(b);
    let byte_index = (addr & 0xF) as usize;
    
    let mut mask = [0u8; 16];
    for i in 0..16 {
        mask[i] = if i == byte_index { 0x03 } else { (0x10 + i) as u8 };
    }
    
    let result = [
        u32::from_be_bytes([mask[0], mask[1], mask[2], mask[3]]),
        u32::from_be_bytes([mask[4], mask[5], mask[6], mask[7]]),
        u32::from_be_bytes([mask[8], mask[9], mask[10], mask[11]]),
        u32::from_be_bytes([mask[12], mask[13], mask[14], mask[15]]),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Copy Halfword to Halfword Insert (x-form) - chx rt, ra, rb
pub fn chx(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_preferred_u32(ra as usize);
    let b = thread.regs.read_preferred_u32(rb as usize);
    let addr = a.wrapping_add(b);
    let hword_index = ((addr >> 1) & 0x7) as usize;
    
    let mut mask = [0u8; 16];
    for i in 0..16 {
        let hword_pos = i / 2;
        if hword_pos == hword_index {
            mask[i] = if i % 2 == 0 { 0x02 } else { 0x03 };
        } else {
            mask[i] = (0x10 + i) as u8;
        }
    }
    
    let result = [
        u32::from_be_bytes([mask[0], mask[1], mask[2], mask[3]]),
        u32::from_be_bytes([mask[4], mask[5], mask[6], mask[7]]),
        u32::from_be_bytes([mask[8], mask[9], mask[10], mask[11]]),
        u32::from_be_bytes([mask[12], mask[13], mask[14], mask[15]]),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Copy Halfword to Word Insert (x-form) - cwx rt, ra, rb
pub fn cwx(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_preferred_u32(ra as usize);
    let b = thread.regs.read_preferred_u32(rb as usize);
    let addr = a.wrapping_add(b);
    let word_index = ((addr >> 2) & 0x3) as usize;
    
    let mut mask = [0u8; 16];
    for i in 0..16 {
        let word_pos = i / 4;
        if word_pos == word_index {
            mask[i] = (i % 4) as u8;
        } else {
            mask[i] = (0x10 + i) as u8;
        }
    }
    
    let result = [
        u32::from_be_bytes([mask[0], mask[1], mask[2], mask[3]]),
        u32::from_be_bytes([mask[4], mask[5], mask[6], mask[7]]),
        u32::from_be_bytes([mask[8], mask[9], mask[10], mask[11]]),
        u32::from_be_bytes([mask[12], mask[13], mask[14], mask[15]]),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Copy Halfword to Doubleword Insert (x-form) - cdx rt, ra, rb
pub fn cdx(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_preferred_u32(ra as usize);
    let b = thread.regs.read_preferred_u32(rb as usize);
    let addr = a.wrapping_add(b);
    let dword_index = ((addr >> 3) & 0x1) as usize;
    
    let mut mask = [0u8; 16];
    for i in 0..16 {
        let dword_pos = i / 8;
        if dword_pos == dword_index {
            mask[i] = (i % 8) as u8;
        } else {
            mask[i] = (0x10 + i) as u8;
        }
    }
    
    let result = [
        u32::from_be_bytes([mask[0], mask[1], mask[2], mask[3]]),
        u32::from_be_bytes([mask[4], mask[5], mask[6], mask[7]]),
        u32::from_be_bytes([mask[8], mask[9], mask[10], mask[11]]),
        u32::from_be_bytes([mask[12], mask[13], mask[14], mask[15]]),
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
