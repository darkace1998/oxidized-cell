//! Branch instructions for PPU
//!
//! This module contains implementations for PowerPC branch instructions,
//! including conditional and unconditional branches.

use crate::thread::PpuThread;

/// Branch option (BO) field bit masks
pub mod bo_bits {
    /// Decrement CTR and branch if CTR != 0
    pub const DECREMENT_CTR: u8 = 0b00100;
    /// Branch if condition is true (or don't test condition)
    pub const CONDITION_TRUE: u8 = 0b01000;
    /// Don't decrement CTR
    pub const NO_DECREMENT: u8 = 0b00100;
    /// Don't test condition
    pub const NO_CONDITION: u8 = 0b10000;
}

/// Evaluate branch condition based on BO and BI fields
pub fn evaluate_branch_condition(thread: &mut PpuThread, bo: u8, bi: u8) -> bool {
    // Check CTR condition
    let ctr_ok = if (bo & 0b00100) != 0 {
        true // Don't decrement CTR
    } else {
        thread.regs.ctr = thread.regs.ctr.wrapping_sub(1);
        let ctr_zero = thread.regs.ctr == 0;
        // BO[1] = 1 means branch if CTR == 0, BO[1] = 0 means branch if CTR != 0
        ((bo >> 1) & 1) != 0 && ctr_zero || ((bo >> 1) & 1) == 0 && !ctr_zero
    };
    
    // Check condition register condition
    let cond_ok = if (bo & 0b10000) != 0 {
        true // Don't test condition
    } else {
        let cr_bit = (thread.regs.cr >> (31 - bi)) & 1;
        // BO[3] = 1 means branch if CR[BI] == 1, BO[3] = 0 means branch if CR[BI] == 0
        ((bo >> 3) & 1) != 0 && cr_bit != 0 || ((bo >> 3) & 1) == 0 && cr_bit == 0
    };
    
    ctr_ok && cond_ok
}

/// Execute unconditional branch (b, ba, bl, bla)
pub fn branch(thread: &mut PpuThread, li: i32, aa: bool, lk: bool) {
    if lk {
        thread.regs.lr = thread.pc() + 4;
    }
    
    let target = if aa {
        li as u64
    } else {
        (thread.pc() as i64 + li as i64) as u64
    };
    
    thread.set_pc(target);
}

/// Execute conditional branch (bc, bca, bcl, bcla)
pub fn branch_conditional(thread: &mut PpuThread, bo: u8, bi: u8, bd: i16, aa: bool, lk: bool) {
    if evaluate_branch_condition(thread, bo, bi) {
        if lk {
            thread.regs.lr = thread.pc() + 4;
        }
        
        let target = if aa {
            bd as u64
        } else {
            (thread.pc() as i64 + bd as i64) as u64
        };
        
        thread.set_pc(target);
    } else {
        thread.advance_pc();
    }
}

/// Execute branch conditional to link register (bclr, bclrl)
pub fn branch_conditional_lr(thread: &mut PpuThread, bo: u8, bi: u8, lk: bool) {
    if evaluate_branch_condition(thread, bo, bi) {
        let target = thread.regs.lr & !3; // Clear low 2 bits
        if lk {
            thread.regs.lr = thread.pc() + 4;
        }
        thread.set_pc(target);
    } else {
        thread.advance_pc();
    }
}

/// Execute branch conditional to count register (bcctr, bcctrl)
pub fn branch_conditional_ctr(thread: &mut PpuThread, bo: u8, bi: u8, lk: bool) {
    // Note: CTR is not decremented for bcctr
    let cond_ok = if (bo & 0b10000) != 0 {
        true
    } else {
        let cr_bit = (thread.regs.cr >> (31 - bi)) & 1;
        ((bo >> 3) & 1) != 0 && cr_bit != 0 || ((bo >> 3) & 1) == 0 && cr_bit == 0
    };
    
    if cond_ok {
        let target = thread.regs.ctr & !3; // Clear low 2 bits
        if lk {
            thread.regs.lr = thread.pc() + 4;
        }
        thread.set_pc(target);
    } else {
        thread.advance_pc();
    }
}

/// Standard branch mnemonics (BO field encodings)
pub mod mnemonics {
    /// Branch always (b)
    pub const BO_ALWAYS: u8 = 0b10100;
    
    /// Branch if equal (beq)
    pub const BO_IF_TRUE: u8 = 0b01100;
    
    /// Branch if not equal (bne)
    pub const BO_IF_FALSE: u8 = 0b00100;
    
    /// Branch if CTR != 0 (bdnz)
    pub const BO_DNZCTR: u8 = 0b10000;
    
    /// Branch if CTR == 0 (bdz)
    pub const BO_DZCTR: u8 = 0b10010;
    
    /// CR bit indices for conditions
    pub const CR_LT: u8 = 0; // Less than
    pub const CR_GT: u8 = 1; // Greater than
    pub const CR_EQ: u8 = 2; // Equal
    pub const CR_SO: u8 = 3; // Summary overflow
}

#[cfg(test)]
mod tests {
    use super::*;
    use oc_memory::MemoryManager;

    fn create_test_thread() -> PpuThread {
        let mem = MemoryManager::new().unwrap();
        PpuThread::new(0, mem)
    }

    #[test]
    fn test_branch_unconditional() {
        let mut thread = create_test_thread();
        thread.set_pc(0x10000);
        
        // b 0x100 (relative)
        branch(&mut thread, 0x100, false, false);
        assert_eq!(thread.pc(), 0x10100);
        
        // ba 0x200 (absolute)
        branch(&mut thread, 0x200, true, false);
        assert_eq!(thread.pc(), 0x200);
    }

    #[test]
    fn test_branch_with_link() {
        let mut thread = create_test_thread();
        thread.set_pc(0x10000);
        
        // bl 0x100
        branch(&mut thread, 0x100, false, true);
        assert_eq!(thread.pc(), 0x10100);
        assert_eq!(thread.regs.lr, 0x10004);
    }

    #[test]
    fn test_branch_conditional() {
        let mut thread = create_test_thread();
        thread.set_pc(0x10000);
        
        // Set CR0 to indicate equal (bit 2 = 1)
        thread.set_cr_field(0, 0b0010);
        
        // beq 0x100 (BO=01100, BI=2)
        branch_conditional(&mut thread, 0b01100, 2, 0x100, false, false);
        assert_eq!(thread.pc(), 0x10100);
    }

    #[test]
    fn test_branch_lr() {
        let mut thread = create_test_thread();
        thread.set_pc(0x10000);
        thread.regs.lr = 0x20000;
        
        // blr (unconditional return)
        branch_conditional_lr(&mut thread, 0b10100, 0, false);
        assert_eq!(thread.pc(), 0x20000);
    }
}
