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
    pub const DSISR: u16 = 18;    // Data Storage Interrupt Status Register
    pub const DAR: u16 = 19;      // Data Address Register
    pub const DEC: u16 = 22;      // Decrementer
    pub const SDR1: u16 = 25;     // Storage Description Register 1
    pub const SRR0: u16 = 26;     // Save/Restore Register 0
    pub const SRR1: u16 = 27;     // Save/Restore Register 1
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

/// Machine State Register (MSR) bit positions
pub mod msr {
    /// SF - Sixty-Four-Bit Mode (bit 0)
    pub const SF: u64 = 0x8000_0000_0000_0000;
    /// HV - Hypervisor State (bit 3)
    pub const HV: u64 = 0x1000_0000_0000_0000;
    /// VEC - VMX/AltiVec Enable (bit 38 from right, bit 25 from left in 64-bit)
    pub const VEC: u64 = 0x0000_0000_0200_0000;
    /// EE - External Interrupt Enable (bit 48 from right)
    pub const EE: u64 = 0x0000_0000_0000_8000;
    /// PR - Problem State / User Mode (bit 49 from right)
    pub const PR: u64 = 0x0000_0000_0000_4000;
    /// FP - Floating-Point Available (bit 50 from right)
    pub const FP: u64 = 0x0000_0000_0000_2000;
    /// ME - Machine Check Interrupt Enable (bit 51 from right)
    pub const ME: u64 = 0x0000_0000_0000_1000;
    /// FE0 - Floating-Point Exception Mode 0 (bit 52 from right)
    pub const FE0: u64 = 0x0000_0000_0000_0800;
    /// SE - Single-Step Trace Enable (bit 53 from right)
    pub const SE: u64 = 0x0000_0000_0000_0400;
    /// BE - Branch Trace Enable (bit 54 from right)
    pub const BE: u64 = 0x0000_0000_0000_0200;
    /// FE1 - Floating-Point Exception Mode 1 (bit 55 from right)
    pub const FE1: u64 = 0x0000_0000_0000_0100;
    /// IR - Instruction Relocate (bit 58 from right)
    pub const IR: u64 = 0x0000_0000_0000_0020;
    /// DR - Data Relocate (bit 59 from right)
    pub const DR: u64 = 0x0000_0000_0000_0010;
    /// PMM - Performance Monitor Mark (bit 61 from right)
    pub const PMM: u64 = 0x0000_0000_0000_0004;
    /// RI - Recoverable Interrupt (bit 62 from right)
    pub const RI: u64 = 0x0000_0000_0000_0002;
    /// LE - Little-Endian Mode (bit 63 from right)
    pub const LE: u64 = 0x0000_0000_0000_0001;
    
    /// Default MSR for user mode: 64-bit mode, FP enabled, VMX enabled
    pub const USER_MODE_DEFAULT: u64 = SF | FP | VEC | RI;
    /// Default MSR for supervisor mode: adds hypervisor, problem state cleared
    pub const SUPERVISOR_MODE_DEFAULT: u64 = SF | HV | FP | VEC | EE | ME | RI;
}

/// Time Base frequency in Hz (Cell BE uses 79.8 MHz timebase)
pub const TB_FREQUENCY: u64 = 79_800_000;

/// Decrementer frequency in Hz (same as timebase on Cell BE)
pub const DEC_FREQUENCY: u64 = TB_FREQUENCY;

/// Get the Cell BE Processor Version Register value
/// This identifies it as a Cell Broadband Engine
pub const CELL_PVR: u64 = 0x0070_0100; // Cell BE

/// Get current time base value based on system time
/// This provides a consistent timebase across reads
fn get_current_tb() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    // Convert nanoseconds to timebase ticks: nanos * TB_FREQUENCY / 1e9
    // Use 128-bit arithmetic to avoid overflow
    ((nanos as u128 * TB_FREQUENCY as u128) / 1_000_000_000) as u64
}

/// Move From Time Base (mftb) - Returns the current time base value
pub fn mftb(thread: &PpuThread) -> u64 {
    // Use stored time base if available, otherwise compute from system time
    if thread.regs.tb != 0 {
        thread.regs.tb
    } else {
        get_current_tb()
    }
}

/// Move From Time Base Upper (mftbu) - Returns upper 32 bits of time base
pub fn mftbu(thread: &PpuThread) -> u64 {
    let tb = if thread.regs.tb != 0 {
        thread.regs.tb
    } else {
        get_current_tb()
    };
    (tb >> 32) & 0xFFFF_FFFF
}

/// Move From Machine State Register (mfmsr)
pub fn mfmsr(thread: &PpuThread) -> u64 {
    thread.regs.msr
}

/// Move To Machine State Register (mtmsr)
/// Note: This is a privileged instruction - in user mode it would cause an exception
pub fn mtmsr(thread: &mut PpuThread, value: u64) {
    // In emulation, we allow MSR writes but log them
    // Real hardware would check privilege level
    tracing::trace!("mtmsr: MSR = 0x{:016x}", value);
    thread.regs.msr = value;
}

/// Move To Machine State Register Direct (mtmsrd)
/// Sets full 64-bit MSR value
pub fn mtmsrd(thread: &mut PpuThread, value: u64, l: bool) {
    if l {
        // L=1: Only update EE and RI bits
        let mask = msr::EE | msr::RI;
        thread.regs.msr = (thread.regs.msr & !mask) | (value & mask);
    } else {
        // L=0: Update all bits
        thread.regs.msr = value;
    }
    tracing::trace!("mtmsrd: MSR = 0x{:016x} (L={})", thread.regs.msr, l);
}

/// Update the decrementer value
/// Returns true if decrementer has reached zero (and would cause an interrupt)
pub fn update_decrementer(thread: &mut PpuThread, cycles: u32) -> bool {
    if thread.regs.dec == 0 {
        return false;
    }
    
    // Decrement by the number of cycles
    let (new_dec, overflow) = thread.regs.dec.overflowing_sub(cycles);
    thread.regs.dec = new_dec;
    
    // Decrementer exception occurs when it goes negative (overflows)
    overflow
}

/// Check if decrementer interrupt should fire
/// Returns true if DEC is negative and EE is enabled in MSR
pub fn check_decrementer_interrupt(thread: &PpuThread) -> bool {
    // Check if decrementer has gone negative (high bit set)
    let dec_negative = (thread.regs.dec as i32) < 0;
    // Check if external interrupts are enabled
    let ee_enabled = (thread.regs.msr & msr::EE) != 0;
    
    dec_negative && ee_enabled
}

/// Update time base by a number of cycles
pub fn update_timebase(thread: &mut PpuThread, cycles: u64) {
    thread.regs.tb = thread.regs.tb.wrapping_add(cycles);
}

/// Read from Special Purpose Register
pub fn mfspr(thread: &PpuThread, spr_num: u16) -> u64 {
    match spr_num {
        spr::XER => thread.regs.xer,
        spr::LR => thread.regs.lr,
        spr::CTR => thread.regs.ctr,
        spr::PVR => CELL_PVR,
        spr::TB => mftb(thread),
        spr::TBU => mftbu(thread),
        spr::VRSAVE => 0, // VMX register save mask
        spr::PIR => thread.id as u64,
        // Save/Restore registers for exception handling
        spr::SRR0 => thread.regs.srr0,
        spr::SRR1 => thread.regs.srr1,
        // Decrementer (returns stored value, sign-extended for 64-bit)
        spr::DEC => thread.regs.dec as i32 as i64 as u64,
        // SPRG scratch registers - commonly used by OS/hypervisor
        spr::SPRG0 | spr::SPRG1 | spr::SPRG2 | spr::SPRG3 => {
            // SPRGs are typically supervisor-only, return 0 for user mode
            tracing::trace!("mfspr: SPRG{} read (supervisor register)", spr_num - spr::SPRG0);
            0
        }
        // Hardware Implementation Dependent registers - return safe defaults
        spr::HID0 | spr::HID1 | spr::HID4 | spr::HID5 | spr::HID6 => {
            tracing::trace!("mfspr: HID register {} read", spr_num);
            0
        }
        // Data storage interrupt registers
        spr::DSISR | spr::DAR | spr::SDR1 => {
            tracing::trace!("mfspr: Memory management SPR {} read", spr_num);
            0
        }
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
        // Save/Restore registers for exception handling
        spr::SRR0 => thread.regs.srr0 = value,
        spr::SRR1 => thread.regs.srr1 = value,
        // Decrementer
        spr::DEC => thread.regs.dec = value as u32,
        // SPRG scratch registers - supervisor only, ignore in user mode
        spr::SPRG0 | spr::SPRG1 | spr::SPRG2 | spr::SPRG3 => {
            tracing::trace!("mtspr: SPRG{} write (ignored - supervisor register)", spr_num - spr::SPRG0);
        }
        // Hardware Implementation Dependent registers - ignore writes
        spr::HID0 | spr::HID1 | spr::HID4 | spr::HID5 | spr::HID6 => {
            tracing::trace!("mtspr: HID register {} write (ignored)", spr_num);
        }
        // Data storage interrupt registers - supervisor only
        spr::DSISR | spr::DAR | spr::SDR1 => {
            tracing::trace!("mtspr: Memory management SPR {} write (ignored)", spr_num);
        }
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

/// Move to CR from FPSCR (mcrfs)
/// Copies a 4-bit field from FPSCR to the specified CR field
/// bf specifies which CR field (0-7) to write
/// bfa specifies which FPSCR field (0-7) to read
/// After copying, sticky exception bits in the source FPSCR field are cleared
pub fn mcrfs(thread: &mut PpuThread, bf: u8, bfa: u8) {
    // FPSCR fields are numbered 0-7, each 4 bits
    // Field 0 is bits 32-35 (FX, FEX, VX, OX), etc.
    let shift = (7 - bfa) * 4;
    let fpscr_field = ((thread.regs.fpscr >> (32 + shift)) & 0xF) as u32;
    
    // Set the CR field
    thread.set_cr_field(bf as usize, fpscr_field);
    
    // Clear only sticky exception bits in the copied FPSCR field
    // FEX and VX are summary bits and should NOT be cleared directly
    // Per PowerPC specification, only specific sticky bits are cleared:
    let clear_mask: u64 = match bfa {
        0 => 0b1001, // Field 0: FX, FEX, VX, OX - clear FX (bit 0) and OX (bit 3) only
        1 => 0b1111, // Field 1: UX, ZX, XX, VXSNAN - clear all (all are sticky)
        2 => 0b1111, // Field 2: VXISI, VXIDI, VXZDZ, VXIMZ - clear all (all are sticky)
        3 => 0b1000, // Field 3: VXVC, FR, FI, FPRF[C] - clear only VXVC (bit 0)
        4 => 0b0100, // Field 4: VXSQRT (bit 1), VXCVI (bit 0) - clear VXSQRT only
        5 => 0b0001, // Field 5: VE, OE, UE, ZE - no sticky bits here, but VXCVI extends here
        _ => 0x0,    // Fields 6-7: no sticky bits to clear
    };
    
    if clear_mask != 0 {
        let clear_bits = clear_mask << (32 + shift);
        thread.regs.fpscr &= !clear_bits;
    }
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
    
    #[test]
    fn test_mcrfs() {
        let mut thread = create_test_thread();
        
        // Set up FPSCR with test values in field 0 (FX, FEX, VX, OX)
        // Field 0 is bits 32-35 (counting from 0 at MSB)
        thread.regs.fpscr = 0xF000_0000_0000_0000; // All 4 bits of field 0 set
        
        // Copy FPSCR field 0 to CR field 0
        mcrfs(&mut thread, 0, 0);
        
        // Check that CR field 0 has the value
        let cr_field0 = (thread.regs.cr >> 28) & 0xF;
        assert_eq!(cr_field0, 0xF, "CR field 0 should have FPSCR field 0 value");
        
        // Reset and test field 1 (UX, ZX, XX, VXSNAN)
        thread.regs.fpscr = 0x0A00_0000_0000_0000; // Some bits in field 1
        thread.regs.cr = 0;
        
        mcrfs(&mut thread, 2, 1); // Copy FPSCR field 1 to CR field 2
        
        let cr_field2 = (thread.regs.cr >> 20) & 0xF;
        assert_eq!(cr_field2, 0xA, "CR field 2 should have FPSCR field 1 value");
    }
    
    #[test]
    fn test_msr_constants() {
        // Verify MSR bit positions are correct
        assert_eq!(msr::SF, 0x8000_0000_0000_0000, "SF bit should be bit 0 (MSB)");
        assert_eq!(msr::HV, 0x1000_0000_0000_0000, "HV bit should be bit 3");
        assert_eq!(msr::EE, 0x0000_0000_0000_8000, "EE bit position");
        assert_eq!(msr::PR, 0x0000_0000_0000_4000, "PR bit position");
    }
    
    #[test]
    fn test_mfmsr_mtmsr() {
        let mut thread = create_test_thread();
        
        // Initial MSR should be default value (64-bit mode)
        let initial_msr = mfmsr(&thread);
        assert!((initial_msr & msr::SF) != 0, "MSR should have 64-bit mode enabled");
        
        // Set a new MSR value
        let new_msr = msr::SF | msr::FP | msr::VEC | msr::EE;
        mtmsr(&mut thread, new_msr);
        
        assert_eq!(mfmsr(&thread), new_msr, "MSR should be updated");
    }
    
    #[test]
    fn test_mtmsrd_l_bit() {
        let mut thread = create_test_thread();
        
        // Set initial MSR with some bits
        thread.regs.msr = msr::SF | msr::FP | msr::VEC;
        
        // With L=1, only EE and RI should be updated
        let value = msr::EE | msr::RI | msr::PR; // Try to set PR too
        mtmsrd(&mut thread, value, true);
        
        // EE and RI should be set, but PR should NOT be set
        assert!((thread.regs.msr & msr::EE) != 0, "EE should be set");
        assert!((thread.regs.msr & msr::RI) != 0, "RI should be set");
        assert!((thread.regs.msr & msr::PR) == 0, "PR should NOT be set with L=1");
        assert!((thread.regs.msr & msr::SF) != 0, "SF should still be set");
        
        // With L=0, all bits should be updated
        mtmsrd(&mut thread, msr::SF | msr::PR, false);
        assert!((thread.regs.msr & msr::EE) == 0, "EE should be cleared with L=0");
        assert!((thread.regs.msr & msr::PR) != 0, "PR should be set with L=0");
    }
    
    #[test]
    fn test_mftb() {
        let thread = create_test_thread();
        
        // Time base should return a non-zero value from system time
        let tb1 = mftb(&thread);
        std::thread::sleep(std::time::Duration::from_millis(1));
        let tb2 = mftb(&thread);
        
        // TB should be increasing
        assert!(tb2 > tb1, "Time base should increase over time");
    }
    
    #[test]
    fn test_decrementer_update() {
        let mut thread = create_test_thread();
        
        // Set decrementer
        thread.regs.dec = 1000;
        
        // Decrement by 500 cycles
        let overflow = update_decrementer(&mut thread, 500);
        assert!(!overflow, "No overflow expected");
        assert_eq!(thread.regs.dec, 500, "Decrementer should be 500");
        
        // Decrement by another 600 cycles - should overflow
        let overflow = update_decrementer(&mut thread, 600);
        assert!(overflow, "Overflow expected when going negative");
    }
    
    #[test]
    fn test_decrementer_interrupt_check() {
        let mut thread = create_test_thread();
        
        // Set decrementer to positive value
        thread.regs.dec = 100;
        thread.regs.msr = msr::SF | msr::EE; // Enable external interrupts
        
        assert!(!check_decrementer_interrupt(&thread), "No interrupt when DEC is positive");
        
        // Make decrementer negative
        thread.regs.dec = 0xFFFF_FFFF; // -1 as signed
        assert!(check_decrementer_interrupt(&thread), "Interrupt when DEC is negative and EE enabled");
        
        // Disable EE - no interrupt should fire
        thread.regs.msr &= !msr::EE;
        assert!(!check_decrementer_interrupt(&thread), "No interrupt when EE disabled");
    }
}
