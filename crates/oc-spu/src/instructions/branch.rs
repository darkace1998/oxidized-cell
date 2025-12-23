//! SPU branch instructions

use crate::thread::SpuThread;
use oc_core::error::SpuError;

/// Branch Relative - br i16
pub fn br(thread: &mut SpuThread, i16_val: i16) -> Result<(), SpuError> {
    let offset = (i16_val as i32) << 2;
    let target = (thread.pc() as i32 + offset) as u32;
    thread.set_pc(target);
    Ok(())
}

/// Branch Absolute - bra i16
pub fn bra(thread: &mut SpuThread, i16_val: i16) -> Result<(), SpuError> {
    let target = ((i16_val as i32) << 2) as u32;
    thread.set_pc(target);
    Ok(())
}

/// Branch Relative and Set Link - brsl rt, i16
pub fn brsl(thread: &mut SpuThread, i16_val: i16, rt: u8) -> Result<(), SpuError> {
    let next_pc = thread.pc().wrapping_add(4);
    thread.regs.write_preferred_u32(rt as usize, next_pc);
    
    let offset = (i16_val as i32) << 2;
    let target = (thread.pc() as i32 + offset) as u32;
    thread.set_pc(target);
    Ok(())
}

/// Branch Absolute and Set Link - brasl rt, i16
pub fn brasl(thread: &mut SpuThread, i16_val: i16, rt: u8) -> Result<(), SpuError> {
    let next_pc = thread.pc().wrapping_add(4);
    thread.regs.write_preferred_u32(rt as usize, next_pc);
    
    let target = ((i16_val as i32) << 2) as u32;
    thread.set_pc(target);
    Ok(())
}

/// Branch Indirect - bi ra
pub fn bi(thread: &mut SpuThread, ra: u8) -> Result<(), SpuError> {
    let target = thread.regs.read_preferred_u32(ra as usize) & !0x3;
    thread.set_pc(target);
    Ok(())
}

/// Branch Indirect and Set Link - bisl rt, ra
pub fn bisl(thread: &mut SpuThread, ra: u8, rt: u8) -> Result<(), SpuError> {
    let next_pc = thread.pc().wrapping_add(4);
    thread.regs.write_preferred_u32(rt as usize, next_pc);
    
    let target = thread.regs.read_preferred_u32(ra as usize) & !0x3;
    thread.set_pc(target);
    Ok(())
}

/// Branch if Zero - brz rt, i16
pub fn brz(thread: &mut SpuThread, i16_val: i16, rt: u8) -> Result<(), SpuError> {
    let value = thread.regs.read_preferred_u32(rt as usize);
    if value == 0 {
        let offset = (i16_val as i32) << 2;
        let target = (thread.pc() as i32 + offset) as u32;
        thread.set_pc(target);
    } else {
        thread.advance_pc();
    }
    Ok(())
}

/// Branch if Not Zero - brnz rt, i16
pub fn brnz(thread: &mut SpuThread, i16_val: i16, rt: u8) -> Result<(), SpuError> {
    let value = thread.regs.read_preferred_u32(rt as usize);
    if value != 0 {
        let offset = (i16_val as i32) << 2;
        let target = (thread.pc() as i32 + offset) as u32;
        thread.set_pc(target);
    } else {
        thread.advance_pc();
    }
    Ok(())
}

/// Branch if Zero Halfword - brhz rt, i16
pub fn brhz(thread: &mut SpuThread, i16_val: i16, rt: u8) -> Result<(), SpuError> {
    let value = thread.regs.read_preferred_u32(rt as usize) & 0xFFFF;
    if value == 0 {
        let offset = (i16_val as i32) << 2;
        let target = (thread.pc() as i32 + offset) as u32;
        thread.set_pc(target);
    } else {
        thread.advance_pc();
    }
    Ok(())
}

/// Branch if Not Zero Halfword - brhnz rt, i16
pub fn brhnz(thread: &mut SpuThread, i16_val: i16, rt: u8) -> Result<(), SpuError> {
    let value = thread.regs.read_preferred_u32(rt as usize) & 0xFFFF;
    if value != 0 {
        let offset = (i16_val as i32) << 2;
        let target = (thread.pc() as i32 + offset) as u32;
        thread.set_pc(target);
    } else {
        thread.advance_pc();
    }
    Ok(())
}

/// Branch Indirect if Zero - biz rt, ra
pub fn biz(thread: &mut SpuThread, ra: u8, rt: u8) -> Result<(), SpuError> {
    let value = thread.regs.read_preferred_u32(rt as usize);
    if value == 0 {
        let target = thread.regs.read_preferred_u32(ra as usize) & !0x3;
        thread.set_pc(target);
    } else {
        thread.advance_pc();
    }
    Ok(())
}

/// Branch Indirect if Not Zero - binz rt, ra
pub fn binz(thread: &mut SpuThread, ra: u8, rt: u8) -> Result<(), SpuError> {
    let value = thread.regs.read_preferred_u32(rt as usize);
    if value != 0 {
        let target = thread.regs.read_preferred_u32(ra as usize) & !0x3;
        thread.set_pc(target);
    } else {
        thread.advance_pc();
    }
    Ok(())
}

/// Branch Indirect if Zero Halfword - bihz rt, ra
pub fn bihz(thread: &mut SpuThread, ra: u8, rt: u8) -> Result<(), SpuError> {
    let value = thread.regs.read_preferred_u32(rt as usize) & 0xFFFF;
    if value == 0 {
        let target = thread.regs.read_preferred_u32(ra as usize) & !0x3;
        thread.set_pc(target);
    } else {
        thread.advance_pc();
    }
    Ok(())
}

/// Branch Indirect if Not Zero Halfword - bihnz rt, ra
pub fn bihnz(thread: &mut SpuThread, ra: u8, rt: u8) -> Result<(), SpuError> {
    let value = thread.regs.read_preferred_u32(rt as usize) & 0xFFFF;
    if value != 0 {
        let target = thread.regs.read_preferred_u32(ra as usize) & !0x3;
        thread.set_pc(target);
    } else {
        thread.advance_pc();
    }
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
    fn test_br() {
        let mut thread = create_test_thread();
        thread.set_pc(0x100);
        
        br(&mut thread, 10).unwrap();
        assert_eq!(thread.pc(), 0x100 + (10 << 2));
    }

    #[test]
    fn test_bra() {
        let mut thread = create_test_thread();
        
        bra(&mut thread, 20).unwrap();
        assert_eq!(thread.pc(), 20 << 2);
    }

    #[test]
    fn test_brsl() {
        let mut thread = create_test_thread();
        thread.set_pc(0x100);
        
        brsl(&mut thread, 10, 5).unwrap();
        assert_eq!(thread.pc(), 0x100 + (10 << 2));
        assert_eq!(thread.regs.read_preferred_u32(5), 0x104);
    }

    #[test]
    fn test_bi() {
        let mut thread = create_test_thread();
        thread.regs.write_preferred_u32(3, 0x200);
        
        bi(&mut thread, 3).unwrap();
        assert_eq!(thread.pc(), 0x200);
    }

    #[test]
    fn test_brz() {
        let mut thread = create_test_thread();
        thread.set_pc(0x100);
        
        // Test branch taken
        thread.regs.write_preferred_u32(2, 0);
        brz(&mut thread, 10, 2).unwrap();
        assert_eq!(thread.pc(), 0x100 + (10 << 2));
        
        // Test branch not taken
        thread.set_pc(0x100);
        thread.regs.write_preferred_u32(2, 1);
        brz(&mut thread, 10, 2).unwrap();
        assert_eq!(thread.pc(), 0x104);
    }

    #[test]
    fn test_brnz() {
        let mut thread = create_test_thread();
        thread.set_pc(0x100);
        
        // Test branch taken
        thread.regs.write_preferred_u32(2, 1);
        brnz(&mut thread, 10, 2).unwrap();
        assert_eq!(thread.pc(), 0x100 + (10 << 2));
        
        // Test branch not taken
        thread.set_pc(0x100);
        thread.regs.write_preferred_u32(2, 0);
        brnz(&mut thread, 10, 2).unwrap();
        assert_eq!(thread.pc(), 0x104);
    }

    #[test]
    fn test_brhz() {
        let mut thread = create_test_thread();
        thread.set_pc(0x100);
        
        // Test with zero halfword (upper word non-zero)
        thread.regs.write_preferred_u32(2, 0x12340000);
        brhz(&mut thread, 10, 2).unwrap();
        assert_eq!(thread.pc(), 0x100 + (10 << 2));
    }

    #[test]
    fn test_bisl() {
        let mut thread = create_test_thread();
        thread.set_pc(0x100);
        thread.regs.write_preferred_u32(3, 0x200);
        
        bisl(&mut thread, 3, 5).unwrap();
        assert_eq!(thread.pc(), 0x200);
        assert_eq!(thread.regs.read_preferred_u32(5), 0x104);
    }
}
