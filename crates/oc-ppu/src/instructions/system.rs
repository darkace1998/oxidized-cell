//! System instructions for PPU
//!
//! This module contains implementations for PowerPC system-level
//! instructions including SPR access, synchronization, and traps.

use crate::thread::PpuThread;

/// Special Purpose Register numbers
pub mod spr {
    pub const XER: u16 = 1;
    pub const LR: u16 = 8;
    pub const CTR: u16 = 9;
    pub const VRSAVE: u16 = 256;
    pub const SPRG0: u16 = 272;
    pub const SPRG1: u16 = 273;
    pub const SPRG2: u16 = 274;
    pub const SPRG3: u16 = 275;
    pub const TB: u16 = 268;      // Time Base (read-only)
    pub const TBU: u16 = 269;     // Time Base Upper (read-only)
    pub const PVR: u16 = 287;     // Processor Version Register (read-only)
    pub const HID0: u16 = 1008;
    pub const HID1: u16 = 1009;
    pub const HID4: u16 = 1012;
    pub const HID5: u16 = 1014;
    pub const HID6: u16 = 1017;
    pub const PIR: u16 = 1023;    // Processor Identification Register
}

/// Get the Cell BE Processor Version Register value
/// This identifies it as a Cell Broadband Engine
pub const CELL_PVR: u64 = 0x0070_0100; // Cell BE

/// Read from Special Purpose Register
pub fn mfspr(thread: &PpuThread, spr_num: u16) -> u64 {
    match spr_num {
        spr::XER => thread.regs.xer,
        spr::LR => thread.regs.lr,
        spr::CTR => thread.regs.ctr,
        spr::PVR => CELL_PVR,
        spr::TB => {
            // Time base - use system time for now
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64 / 40 // ~25MHz TB frequency
        }
        spr::TBU => {
            let tb = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64 / 40;
            (tb >> 32) & 0xFFFF_FFFF
        }
        spr::VRSAVE => 0, // VMX register save mask
        spr::PIR => thread.id as u64,
        _ => {
            tracing::warn!("mfspr: Unimplemented SPR {}", spr_num);
            0
        }
    }
}

/// Write to Special Purpose Register
pub fn mtspr(thread: &mut PpuThread, spr_num: u16, value: u64) {
    match spr_num {
        spr::XER => thread.regs.xer = value,
        spr::LR => thread.regs.lr = value,
        spr::CTR => thread.regs.ctr = value,
        spr::VRSAVE => { /* Ignored for now */ }
        spr::PVR => { /* Read-only, ignore */ }
        spr::TB | spr::TBU => { /* Time base is read-only in user mode */ }
        _ => {
            tracing::warn!("mtspr: Unimplemented SPR {} = 0x{:016x}", spr_num, value);
        }
    }
}

/// Move from Condition Register
pub fn mfcr(thread: &PpuThread) -> u64 {
    thread.regs.cr as u64
}

/// Move to Condition Register Fields
pub fn mtcrf(thread: &mut PpuThread, crm: u8, value: u64) {
    for i in 0..8 {
        if (crm >> (7 - i)) & 1 != 0 {
            let field = ((value >> (28 - i * 4)) & 0xF) as u32;
            thread.set_cr_field(i, field);
        }
    }
}

/// Move from One Condition Register Field
pub fn mfocrf(thread: &PpuThread, crm: u8) -> u64 {
    let mut result = 0u64;
    for i in 0..8 {
        if (crm >> (7 - i)) & 1 != 0 {
            result |= (thread.get_cr_field(i) as u64) << (28 - i * 4);
        }
    }
    result
}

/// Move to One Condition Register Field
pub fn mtocrf(thread: &mut PpuThread, crm: u8, value: u64) {
    // Same as mtcrf for our purposes
    mtcrf(thread, crm, value);
}

/// Move from FPSCR
pub fn mffs(thread: &PpuThread) -> f64 {
    f64::from_bits(thread.regs.fpscr)
}

/// Move to FPSCR Fields
pub fn mtfsf(thread: &mut PpuThread, fm: u8, value: f64) {
    let bits = value.to_bits();
    let mut fpscr = thread.regs.fpscr;
    
    for i in 0..8 {
        if (fm >> (7 - i)) & 1 != 0 {
            let mask = 0xF << (28 - i * 4);
            fpscr = (fpscr & !mask) | (bits & mask);
        }
    }
    
    thread.regs.fpscr = fpscr;
}

/// Move to FPSCR Field Immediate
pub fn mtfsfi(thread: &mut PpuThread, bf: u8, imm: u8) {
    let shift = 28 - (bf as u32) * 4;
    let mask = 0xFu64 << shift;
    thread.regs.fpscr = (thread.regs.fpscr & !mask) | ((imm as u64 & 0xF) << shift);
}

/// Move to FPSCR Bit 0 (set)
pub fn mtfsb0(thread: &mut PpuThread, bt: u8) {
    thread.regs.fpscr &= !(1u64 << (31 - bt));
}

/// Move to FPSCR Bit 1 (clear)
pub fn mtfsb1(thread: &mut PpuThread, bt: u8) {
    thread.regs.fpscr |= 1u64 << (31 - bt);
}

/// Condition Register AND
pub fn crand(thread: &mut PpuThread, bt: u8, ba: u8, bb: u8) {
    let a = (thread.regs.cr >> (31 - ba)) & 1;
    let b = (thread.regs.cr >> (31 - bb)) & 1;
    let result = a & b;
    thread.regs.cr = (thread.regs.cr & !(1 << (31 - bt))) | (result << (31 - bt));
}

/// Condition Register OR
pub fn cror(thread: &mut PpuThread, bt: u8, ba: u8, bb: u8) {
    let a = (thread.regs.cr >> (31 - ba)) & 1;
    let b = (thread.regs.cr >> (31 - bb)) & 1;
    let result = a | b;
    thread.regs.cr = (thread.regs.cr & !(1 << (31 - bt))) | (result << (31 - bt));
}

/// Condition Register XOR
pub fn crxor(thread: &mut PpuThread, bt: u8, ba: u8, bb: u8) {
    let a = (thread.regs.cr >> (31 - ba)) & 1;
    let b = (thread.regs.cr >> (31 - bb)) & 1;
    let result = a ^ b;
    thread.regs.cr = (thread.regs.cr & !(1 << (31 - bt))) | (result << (31 - bt));
}

/// Condition Register NAND
pub fn crnand(thread: &mut PpuThread, bt: u8, ba: u8, bb: u8) {
    let a = (thread.regs.cr >> (31 - ba)) & 1;
    let b = (thread.regs.cr >> (31 - bb)) & 1;
    let result = !(a & b) & 1;
    thread.regs.cr = (thread.regs.cr & !(1 << (31 - bt))) | (result << (31 - bt));
}

/// Condition Register NOR
pub fn crnor(thread: &mut PpuThread, bt: u8, ba: u8, bb: u8) {
    let a = (thread.regs.cr >> (31 - ba)) & 1;
    let b = (thread.regs.cr >> (31 - bb)) & 1;
    let result = !(a | b) & 1;
    thread.regs.cr = (thread.regs.cr & !(1 << (31 - bt))) | (result << (31 - bt));
}

/// Condition Register EQV (XNOR)
pub fn creqv(thread: &mut PpuThread, bt: u8, ba: u8, bb: u8) {
    let a = (thread.regs.cr >> (31 - ba)) & 1;
    let b = (thread.regs.cr >> (31 - bb)) & 1;
    let result = !(a ^ b) & 1;
    thread.regs.cr = (thread.regs.cr & !(1 << (31 - bt))) | (result << (31 - bt));
}

/// Condition Register AND with Complement
pub fn crandc(thread: &mut PpuThread, bt: u8, ba: u8, bb: u8) {
    let a = (thread.regs.cr >> (31 - ba)) & 1;
    let b = (thread.regs.cr >> (31 - bb)) & 1;
    let result = a & (!b & 1);
    thread.regs.cr = (thread.regs.cr & !(1 << (31 - bt))) | (result << (31 - bt));
}

/// Condition Register OR with Complement
pub fn crorc(thread: &mut PpuThread, bt: u8, ba: u8, bb: u8) {
    let a = (thread.regs.cr >> (31 - ba)) & 1;
    let b = (thread.regs.cr >> (31 - bb)) & 1;
    let result = a | (!b & 1);
    thread.regs.cr = (thread.regs.cr & !(1 << (31 - bt))) | (result << (31 - bt));
}

/// Move Condition Register Field
pub fn mcrf(thread: &mut PpuThread, bf: u8, bfa: u8) {
    let field = thread.get_cr_field(bfa as usize);
    thread.set_cr_field(bf as usize, field);
}

/// Trap Word (check condition and trap)
pub fn tw(_thread: &PpuThread, to: u8, ra: u64, rb: u64) -> bool {
    let a = ra as i32;
    let b = rb as i32;
    
    ((to & 0x10) != 0 && a < b) ||
    ((to & 0x08) != 0 && a > b) ||
    ((to & 0x04) != 0 && a == b) ||
    ((to & 0x02) != 0 && (ra as u32) < (rb as u32)) ||
    ((to & 0x01) != 0 && (ra as u32) > (rb as u32))
}

/// Trap Doubleword (check condition and trap)
pub fn td(_thread: &PpuThread, to: u8, ra: u64, rb: u64) -> bool {
    let a = ra as i64;
    let b = rb as i64;
    
    ((to & 0x10) != 0 && a < b) ||
    ((to & 0x08) != 0 && a > b) ||
    ((to & 0x04) != 0 && a == b) ||
    ((to & 0x02) != 0 && ra < rb) ||
    ((to & 0x01) != 0 && ra > rb)
}

/// Trap Word Immediate (check condition with immediate and trap)
pub fn twi(_thread: &PpuThread, to: u8, ra: u64, si: i16) -> bool {
    let a = ra as i32;
    let b = si as i32;
    let ra_u = ra as u32;
    let si_u = si as u16 as u32;
    
    ((to & 0x10) != 0 && a < b) ||
    ((to & 0x08) != 0 && a > b) ||
    ((to & 0x04) != 0 && a == b) ||
    ((to & 0x02) != 0 && ra_u < si_u) ||
    ((to & 0x01) != 0 && ra_u > si_u)
}

/// Trap Doubleword Immediate (check condition with immediate and trap)
pub fn tdi(_thread: &PpuThread, to: u8, ra: u64, si: i16) -> bool {
    let a = ra as i64;
    let b = si as i64;
    let si_u = si as u16 as u64;
    
    ((to & 0x10) != 0 && a < b) ||
    ((to & 0x08) != 0 && a > b) ||
    ((to & 0x04) != 0 && a == b) ||
    ((to & 0x02) != 0 && ra < si_u) ||
    ((to & 0x01) != 0 && ra > si_u)
}

/// Synchronization instructions (these are mostly no-ops in emulation)
pub fn sync(_thread: &mut PpuThread, _l: u8) {
    // Memory barrier - in emulation, memory is coherent
    std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
}

pub fn lwsync(_thread: &mut PpuThread) {
    // Lightweight sync
    std::sync::atomic::fence(std::sync::atomic::Ordering::AcqRel);
}

pub fn isync(_thread: &mut PpuThread) {
    // Instruction sync - no-op in emulation
}

pub fn eieio(_thread: &mut PpuThread) {
    // Enforce In-Order Execution of I/O
    std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
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
    fn test_mfspr_mtspr_lr() {
        let mut thread = create_test_thread();
        
        mtspr(&mut thread, spr::LR, 0x12345678);
        assert_eq!(mfspr(&thread, spr::LR), 0x12345678);
    }

    #[test]
    fn test_mfspr_mtspr_ctr() {
        let mut thread = create_test_thread();
        
        mtspr(&mut thread, spr::CTR, 0xDEADBEEF);
        assert_eq!(mfspr(&thread, spr::CTR), 0xDEADBEEF);
    }

    #[test]
    fn test_mfcr_mtcrf() {
        let mut thread = create_test_thread();
        
        thread.set_cr_field(0, 0b1010);
        assert_eq!((mfcr(&thread) >> 28) & 0xF, 0b1010);
        
        mtcrf(&mut thread, 0xFF, 0x12345678);
        assert_eq!(thread.regs.cr, 0x12345678);
    }

    #[test]
    fn test_cr_operations() {
        let mut thread = create_test_thread();
        
        // Set bits 0 and 1
        thread.regs.cr = 0xC000_0000;
        
        // AND bits 0 and 1, store in bit 2
        crand(&mut thread, 2, 0, 1);
        assert!((thread.regs.cr >> 29) & 1 == 1);
        
        // XOR should give 0
        crxor(&mut thread, 3, 0, 0);
        assert!((thread.regs.cr >> 28) & 1 == 0);
    }

    #[test]
    fn test_tw_trap() {
        let thread = create_test_thread();
        
        // Trap if a == b
        assert!(tw(&thread, 0x04, 5, 5));
        assert!(!tw(&thread, 0x04, 5, 6));
        
        // Trap if a < b (signed)
        assert!(tw(&thread, 0x10, (-1i32) as u64, 0));
        assert!(!tw(&thread, 0x10, 5, 3));
    }

    #[test]
    fn test_twi_trap() {
        let thread = create_test_thread();
        
        // Trap if a == b (immediate)
        assert!(twi(&thread, 0x04, 5, 5));
        assert!(!twi(&thread, 0x04, 5, 6));
        
        // Trap if a < b (signed, immediate)
        assert!(twi(&thread, 0x10, (-1i32) as u64, 0));
        assert!(!twi(&thread, 0x10, 5, 3));
        
        // Trap if a > b (signed, immediate)
        assert!(twi(&thread, 0x08, 10, 5));
        assert!(!twi(&thread, 0x08, 3, 5));
    }

    #[test]
    fn test_td_trap() {
        let thread = create_test_thread();
        
        // Trap if a == b (64-bit)
        assert!(td(&thread, 0x04, 0x1_0000_0000, 0x1_0000_0000));
        assert!(!td(&thread, 0x04, 0x1_0000_0000, 0x2_0000_0000));
        
        // Trap if a < b (signed, 64-bit)
        assert!(td(&thread, 0x10, (-1i64) as u64, 0));
        assert!(!td(&thread, 0x10, 5, 3));
    }

    #[test]
    fn test_tdi_trap() {
        let thread = create_test_thread();
        
        // Trap if a == b (immediate, 64-bit)
        assert!(tdi(&thread, 0x04, 5, 5));
        assert!(!tdi(&thread, 0x04, 5, 6));
        
        // Trap if a < b (signed, immediate, 64-bit)
        assert!(tdi(&thread, 0x10, (-1i64) as u64, 0));
        assert!(!tdi(&thread, 0x10, 5, 3));
    }
}
