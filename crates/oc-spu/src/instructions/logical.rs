//! SPU logical instructions

use crate::thread::SpuThread;
use oc_core::error::SpuError;

/// AND - and rt, ra, rb
pub fn and(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [a[0] & b[0], a[1] & b[1], a[2] & b[2], a[3] & b[3]];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// AND with Complement - andc rt, ra, rb
pub fn andc(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [a[0] & !b[0], a[1] & !b[1], a[2] & !b[2], a[3] & !b[3]];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// AND Byte Immediate - andbi rt, ra, i10
pub fn andbi(thread: &mut SpuThread, i10: i16, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let imm_byte = (i10 & 0xFF) as u32;
    let mask = imm_byte | (imm_byte << 8) | (imm_byte << 16) | (imm_byte << 24);
    let result = [a[0] & mask, a[1] & mask, a[2] & mask, a[3] & mask];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// AND Halfword Immediate - andhi rt, ra, i10
pub fn andhi(thread: &mut SpuThread, i10: i16, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let imm_hword = (i10 as u16) as u32;
    let mask = imm_hword | (imm_hword << 16);
    let result = [a[0] & mask, a[1] & mask, a[2] & mask, a[3] & mask];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// AND Word Immediate - andi rt, ra, i10
pub fn andi(thread: &mut SpuThread, i10: i16, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    // Sign extend i10 to i32, then convert to u32 (keeps sign-extended bits)
    let imm = (i10 as i32) as u32;
    let result = [a[0] & imm, a[1] & imm, a[2] & imm, a[3] & imm];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// OR - or rt, ra, rb
pub fn or(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [a[0] | b[0], a[1] | b[1], a[2] | b[2], a[3] | b[3]];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// OR with Complement - orc rt, ra, rb
pub fn orc(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [a[0] | !b[0], a[1] | !b[1], a[2] | !b[2], a[3] | !b[3]];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// OR Byte Immediate - orbi rt, ra, i10
pub fn orbi(thread: &mut SpuThread, i10: i16, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let imm_byte = (i10 & 0xFF) as u32;
    let mask = imm_byte | (imm_byte << 8) | (imm_byte << 16) | (imm_byte << 24);
    let result = [a[0] | mask, a[1] | mask, a[2] | mask, a[3] | mask];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// OR Halfword Immediate - orhi rt, ra, i10
pub fn orhi(thread: &mut SpuThread, i10: i16, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let imm_hword = (i10 as u16) as u32;
    let mask = imm_hword | (imm_hword << 16);
    let result = [a[0] | mask, a[1] | mask, a[2] | mask, a[3] | mask];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// OR Word Immediate - ori rt, ra, i10
pub fn ori(thread: &mut SpuThread, i10: i16, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    // Sign extend i10 to i32, then convert to u32 (keeps sign-extended bits)
    let imm = (i10 as i32) as u32;
    let result = [a[0] | imm, a[1] | imm, a[2] | imm, a[3] | imm];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// XOR - xor rt, ra, rb
pub fn xor(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [a[0] ^ b[0], a[1] ^ b[1], a[2] ^ b[2], a[3] ^ b[3]];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// XOR Byte Immediate - xorbi rt, ra, i10
pub fn xorbi(thread: &mut SpuThread, i10: i16, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let imm_byte = (i10 & 0xFF) as u32;
    let mask = imm_byte | (imm_byte << 8) | (imm_byte << 16) | (imm_byte << 24);
    let result = [a[0] ^ mask, a[1] ^ mask, a[2] ^ mask, a[3] ^ mask];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// XOR Halfword Immediate - xorhi rt, ra, i10
pub fn xorhi(thread: &mut SpuThread, i10: i16, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let imm_hword = (i10 as u16) as u32;
    let mask = imm_hword | (imm_hword << 16);
    let result = [a[0] ^ mask, a[1] ^ mask, a[2] ^ mask, a[3] ^ mask];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// XOR Word Immediate - xori rt, ra, i10
pub fn xori(thread: &mut SpuThread, i10: i16, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    // Sign extend i10 to i32, then convert to u32 (keeps sign-extended bits)
    let imm = (i10 as i32) as u32;
    let result = [a[0] ^ imm, a[1] ^ imm, a[2] ^ imm, a[3] ^ imm];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// NAND - nand rt, ra, rb
pub fn nand(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        !(a[0] & b[0]),
        !(a[1] & b[1]),
        !(a[2] & b[2]),
        !(a[3] & b[3]),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// NOR - nor rt, ra, rb
pub fn nor(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        !(a[0] | b[0]),
        !(a[1] | b[1]),
        !(a[2] | b[2]),
        !(a[3] | b[3]),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Equivalent - eqv rt, ra, rb (XNOR)
pub fn eqv(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let result = [
        !(a[0] ^ b[0]),
        !(a[1] ^ b[1]),
        !(a[2] ^ b[2]),
        !(a[3] ^ b[3]),
    ];
    thread.regs.write_u32x4(rt as usize, result);
    thread.advance_pc();
    Ok(())
}

/// Select Bits - selb rt, ra, rb, rc
pub fn selb(thread: &mut SpuThread, rc: u8, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_u32x4(rb as usize);
    let c = thread.regs.read_u32x4(rc as usize);
    let result = [
        (a[0] & !c[0]) | (b[0] & c[0]),
        (a[1] & !c[1]) | (b[1] & c[1]),
        (a[2] & !c[2]) | (b[2] & c[2]),
        (a[3] & !c[3]) | (b[3] & c[3]),
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
    fn test_and() {
        let mut thread = create_test_thread();
        thread.regs.write_u32x4(1, [0xFFFF0000, 0xFF00FF00, 0xF0F0F0F0, 0xAAAAAAAA]);
        thread.regs.write_u32x4(2, [0x0000FFFF, 0x00FF00FF, 0x0F0F0F0F, 0x55555555]);
        
        and(&mut thread, 2, 1, 3).unwrap();
        
        let result = thread.regs.read_u32x4(3);
        assert_eq!(result, [0x00000000, 0x00000000, 0x00000000, 0x00000000]);
    }

    #[test]
    fn test_or() {
        let mut thread = create_test_thread();
        thread.regs.write_u32x4(1, [0xFFFF0000, 0xFF00FF00, 0xF0F0F0F0, 0xAAAAAAAA]);
        thread.regs.write_u32x4(2, [0x0000FFFF, 0x00FF00FF, 0x0F0F0F0F, 0x55555555]);
        
        or(&mut thread, 2, 1, 3).unwrap();
        
        let result = thread.regs.read_u32x4(3);
        assert_eq!(result, [0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF]);
    }

    #[test]
    fn test_xor() {
        let mut thread = create_test_thread();
        thread.regs.write_u32x4(1, [0xFFFF0000, 0xFF00FF00, 0xF0F0F0F0, 0xAAAAAAAA]);
        thread.regs.write_u32x4(2, [0x0000FFFF, 0x00FF00FF, 0x0F0F0F0F, 0x55555555]);
        
        xor(&mut thread, 2, 1, 3).unwrap();
        
        let result = thread.regs.read_u32x4(3);
        assert_eq!(result, [0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF]);
    }

    #[test]
    fn test_andc() {
        let mut thread = create_test_thread();
        thread.regs.write_u32x4(1, [0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF]);
        thread.regs.write_u32x4(2, [0x0000FFFF, 0xFF00FF00, 0xF0F0F0F0, 0xAAAAAAAA]);
        
        andc(&mut thread, 2, 1, 3).unwrap();
        
        let result = thread.regs.read_u32x4(3);
        assert_eq!(result, [0xFFFF0000, 0x00FF00FF, 0x0F0F0F0F, 0x55555555]);
    }

    #[test]
    fn test_nand() {
        let mut thread = create_test_thread();
        thread.regs.write_u32x4(1, [0x00000000, 0x00000000, 0x00000000, 0x00000000]);
        thread.regs.write_u32x4(2, [0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF]);
        
        nand(&mut thread, 2, 1, 3).unwrap();
        
        let result = thread.regs.read_u32x4(3);
        assert_eq!(result, [0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF]);
    }

    #[test]
    fn test_nor() {
        let mut thread = create_test_thread();
        thread.regs.write_u32x4(1, [0xFFFF0000, 0xFF00FF00, 0xF0F0F0F0, 0xAAAAAAAA]);
        thread.regs.write_u32x4(2, [0x0000FFFF, 0x00FF00FF, 0x0F0F0F0F, 0x55555555]);
        
        nor(&mut thread, 2, 1, 3).unwrap();
        
        let result = thread.regs.read_u32x4(3);
        assert_eq!(result, [0x00000000, 0x00000000, 0x00000000, 0x00000000]);
    }

    #[test]
    fn test_eqv() {
        let mut thread = create_test_thread();
        thread.regs.write_u32x4(1, [0xFFFF0000, 0xFF00FF00, 0xF0F0F0F0, 0xAAAAAAAA]);
        thread.regs.write_u32x4(2, [0xFFFF0000, 0xFF00FF00, 0xF0F0F0F0, 0xAAAAAAAA]);
        
        eqv(&mut thread, 2, 1, 3).unwrap();
        
        let result = thread.regs.read_u32x4(3);
        assert_eq!(result, [0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF]);
    }

    #[test]
    fn test_selb() {
        let mut thread = create_test_thread();
        thread.regs.write_u32x4(1, [0xAAAAAAAA, 0xAAAAAAAA, 0xAAAAAAAA, 0xAAAAAAAA]);
        thread.regs.write_u32x4(2, [0x55555555, 0x55555555, 0x55555555, 0x55555555]);
        thread.regs.write_u32x4(3, [0xFFFF0000, 0x00000000, 0xFFFFFFFF, 0x00000000]);
        
        selb(&mut thread, 3, 2, 1, 4).unwrap();
        
        let result = thread.regs.read_u32x4(4);
        // Where mask is 1, select from rb (0x55555555)
        // Where mask is 0, select from ra (0xAAAAAAAA)
        assert_eq!(result[0], 0x5555AAAA);
        assert_eq!(result[1], 0xAAAAAAAA);
        assert_eq!(result[2], 0x55555555);
        assert_eq!(result[3], 0xAAAAAAAA);
    }

    #[test]
    fn test_andi() {
        let mut thread = create_test_thread();
        thread.regs.write_u32x4(1, [0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF]);
        
        andi(&mut thread, 0xFF, 1, 2).unwrap();
        
        let result = thread.regs.read_u32x4(2);
        assert_eq!(result, [0xFF, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn test_ori() {
        let mut thread = create_test_thread();
        thread.regs.write_u32x4(1, [0x00000000, 0x00000000, 0x00000000, 0x00000000]);
        
        ori(&mut thread, 0xAB, 1, 2).unwrap();
        
        let result = thread.regs.read_u32x4(2);
        assert_eq!(result, [0xAB, 0xAB, 0xAB, 0xAB]);
    }
}
