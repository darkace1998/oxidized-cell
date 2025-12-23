//! SPU compare instructions

use crate::thread::SpuThread;
use oc_core::error::SpuError;

/// Compare Equal Word - ceq rt, ra, rb
pub fn ceq(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        if a[0] == b[0] { 0xFFFFFFFF } else { 0 },
        if a[1] == b[1] { 0xFFFFFFFF } else { 0 },
        if a[2] == b[2] { 0xFFFFFFFF } else { 0 },
        if a[3] == b[3] { 0xFFFFFFFF } else { 0 },
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Compare Equal Word Immediate - ceqi rt, ra, i10
pub fn ceqi(thread: &mut SpuThread, i10: i16, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let imm = i10 as i32 as u32;
    let result = [
        if a[0] == imm { 0xFFFFFFFF } else { 0 },
        if a[1] == imm { 0xFFFFFFFF } else { 0 },
        if a[2] == imm { 0xFFFFFFFF } else { 0 },
        if a[3] == imm { 0xFFFFFFFF } else { 0 },
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Compare Greater Than Word - cgt rt, ra, rb
pub fn cgt(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        if (a[0] as i32) > (b[0] as i32) { 0xFFFFFFFF } else { 0 },
        if (a[1] as i32) > (b[1] as i32) { 0xFFFFFFFF } else { 0 },
        if (a[2] as i32) > (b[2] as i32) { 0xFFFFFFFF } else { 0 },
        if (a[3] as i32) > (b[3] as i32) { 0xFFFFFFFF } else { 0 },
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Compare Greater Than Word Immediate - cgti rt, ra, i10
pub fn cgti(thread: &mut SpuThread, i10: i16, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let imm = i10 as i32;
    let result = [
        if (a[0] as i32) > imm { 0xFFFFFFFF } else { 0 },
        if (a[1] as i32) > imm { 0xFFFFFFFF } else { 0 },
        if (a[2] as i32) > imm { 0xFFFFFFFF } else { 0 },
        if (a[3] as i32) > imm { 0xFFFFFFFF } else { 0 },
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Compare Logical Greater Than Word - clgt rt, ra, rb
pub fn clgt(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        if a[0] > b[0] { 0xFFFFFFFF } else { 0 },
        if a[1] > b[1] { 0xFFFFFFFF } else { 0 },
        if a[2] > b[2] { 0xFFFFFFFF } else { 0 },
        if a[3] > b[3] { 0xFFFFFFFF } else { 0 },
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Compare Logical Greater Than Word Immediate - clgti rt, ra, i10
pub fn clgti(thread: &mut SpuThread, i10: i16, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    // Sign-extend to i32 first, then reinterpret as u32 for unsigned comparison
    let imm = (i10 as i32) as u32;
    let result = [
        if a[0] > imm { 0xFFFFFFFF } else { 0 },
        if a[1] > imm { 0xFFFFFFFF } else { 0 },
        if a[2] > imm { 0xFFFFFFFF } else { 0 },
        if a[3] > imm { 0xFFFFFFFF } else { 0 },
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Compare Equal Halfword - ceqh rt, ra, rb
pub fn ceqh(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let mut result = [0u32; 4];
    for i in 0..4 {
        let a_hi = (a[i] >> 16) as u16;
        let a_lo = (a[i] & 0xFFFF) as u16;
        let b_hi = (b[i] >> 16) as u16;
        let b_lo = (b[i] & 0xFFFF) as u16;
        let hi = if a_hi == b_hi { 0xFFFF } else { 0 };
        let lo = if a_lo == b_lo { 0xFFFF } else { 0 };
        result[i] = ((hi as u32) << 16) | (lo as u32);
    }
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Compare Greater Than Halfword - cgth rt, ra, rb
pub fn cgth(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let mut result = [0u32; 4];
    for i in 0..4 {
        let a_hi = (a[i] >> 16) as i16;
        let a_lo = (a[i] & 0xFFFF) as i16;
        let b_hi = (b[i] >> 16) as i16;
        let b_lo = (b[i] & 0xFFFF) as i16;
        let hi = if a_hi > b_hi { 0xFFFF } else { 0 };
        let lo = if a_lo > b_lo { 0xFFFF } else { 0 };
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
    fn test_ceq() {
        let mut thread = create_test_thread();
        thread.regs.write_u32x4(1, [1, 2, 3, 4]);
        thread.regs.write_u32x4(2, [1, 0, 3, 0]);
        
        ceq(&mut thread, 2, 1, 3).unwrap();
        
        let result = thread.regs.read_u32x4(3);
        assert_eq!(result, [0xFFFFFFFF, 0, 0xFFFFFFFF, 0]);
    }

    #[test]
    fn test_cgt() {
        let mut thread = create_test_thread();
        thread.regs.write_u32x4(1, [5, 3, 2, 1]);
        thread.regs.write_u32x4(2, [3, 3, 4, 0]);
        
        cgt(&mut thread, 2, 1, 3).unwrap();
        
        let result = thread.regs.read_u32x4(3);
        assert_eq!(result[0], 0xFFFFFFFF); // 5 > 3
        assert_eq!(result[1], 0); // 3 == 3
        assert_eq!(result[2], 0); // 2 < 4
        assert_eq!(result[3], 0xFFFFFFFF); // 1 > 0
    }

    #[test]
    fn test_clgt() {
        let mut thread = create_test_thread();
        thread.regs.write_u32x4(1, [5, 3, 2, 1]);
        thread.regs.write_u32x4(2, [3, 3, 4, 0]);
        
        clgt(&mut thread, 2, 1, 3).unwrap();
        
        let result = thread.regs.read_u32x4(3);
        assert_eq!(result[0], 0xFFFFFFFF);
        assert_eq!(result[1], 0);
        assert_eq!(result[2], 0);
        assert_eq!(result[3], 0xFFFFFFFF);
    }
}
