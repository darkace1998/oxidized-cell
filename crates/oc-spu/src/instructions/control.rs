//! SPU control and hint instructions

use crate::thread::{SpuThread, SpuThreadState};
use oc_core::error::SpuError;

/// No Operation - nop
pub fn nop(thread: &mut SpuThread) -> Result<(), SpuError> {
    thread.advance_pc();
    Ok(())
}

/// Long No Operation - lnop (used for scheduling)
pub fn lnop(thread: &mut SpuThread) -> Result<(), SpuError> {
    thread.advance_pc();
    Ok(())
}

/// Stop - stop (signal value in opcode)
pub fn stop(thread: &mut SpuThread, signal: u16) -> Result<(), SpuError> {
    thread.stop_signal = signal as u32;
    thread.state = SpuThreadState::Halted;
    Ok(())
}

/// Stop with Dependencies - stopd rt, ra, rb
pub fn stopd(thread: &mut SpuThread, _rb: u8, _ra: u8, _rt: u8) -> Result<(), SpuError> {
    // Dependencies are for the pipeline, just stop
    thread.state = SpuThreadState::Halted;
    Ok(())
}

/// Synchronize - sync
pub fn sync(thread: &mut SpuThread) -> Result<(), SpuError> {
    // Memory barrier - ensure all previous operations complete
    // In the interpreter, this is a no-op as operations are sequential
    thread.advance_pc();
    Ok(())
}

/// Data Synchronize - dsync
pub fn dsync(thread: &mut SpuThread) -> Result<(), SpuError> {
    // Data memory barrier
    thread.advance_pc();
    Ok(())
}

/// Hint for Branch (a-form) - hbra i16, ro
pub fn hbra(thread: &mut SpuThread, _i16_val: i16, _ro: i16) -> Result<(), SpuError> {
    // Branch hint - used by hardware branch predictor
    // In the interpreter, this is a no-op
    thread.advance_pc();
    Ok(())
}

/// Hint for Branch (r-form) - hbrr i16, ro
pub fn hbrr(thread: &mut SpuThread, _i16_val: i16, _ro: i16) -> Result<(), SpuError> {
    // Branch hint relative - used by hardware branch predictor
    thread.advance_pc();
    Ok(())
}

/// Hint for Branch - hbr i16, ra
pub fn hbr(thread: &mut SpuThread, _i16_val: i16, _ra: u8) -> Result<(), SpuError> {
    // Branch hint indirect
    thread.advance_pc();
    Ok(())
}

/// Halt if Equal - heq ra, rb
pub fn heq(thread: &mut SpuThread, rb: u8, ra: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_preferred_u32(ra as usize);
    let b = thread.regs.read_preferred_u32(rb as usize);
    if a == b {
        thread.state = SpuThreadState::Halted;
    } else {
        thread.advance_pc();
    }
    Ok(())
}

/// Halt if Equal Immediate - heqi ra, i10
pub fn heqi(thread: &mut SpuThread, i10: i16, ra: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_preferred_u32(ra as usize);
    let imm = i10 as i32 as u32;
    if a == imm {
        thread.state = SpuThreadState::Halted;
    } else {
        thread.advance_pc();
    }
    Ok(())
}

/// Halt if Greater Than - hgt ra, rb
pub fn hgt(thread: &mut SpuThread, rb: u8, ra: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_preferred_u32(ra as usize) as i32;
    let b = thread.regs.read_preferred_u32(rb as usize) as i32;
    if a > b {
        thread.state = SpuThreadState::Halted;
    } else {
        thread.advance_pc();
    }
    Ok(())
}

/// Halt if Greater Than Immediate - hgti ra, i10
pub fn hgti(thread: &mut SpuThread, i10: i16, ra: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_preferred_u32(ra as usize) as i32;
    let imm = i10 as i32;
    if a > imm {
        thread.state = SpuThreadState::Halted;
    } else {
        thread.advance_pc();
    }
    Ok(())
}

/// Halt if Logically Greater Than - hlgt ra, rb
pub fn hlgt(thread: &mut SpuThread, rb: u8, ra: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_preferred_u32(ra as usize);
    let b = thread.regs.read_preferred_u32(rb as usize);
    if a > b {
        thread.state = SpuThreadState::Halted;
    } else {
        thread.advance_pc();
    }
    Ok(())
}

/// Halt if Logically Greater Than Immediate - hlgti ra, i10
pub fn hlgti(thread: &mut SpuThread, i10: i16, ra: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_preferred_u32(ra as usize);
    // For logical comparison, zero-extend the 10-bit immediate
    let imm = (i10 & 0x3FF) as u32;
    if a > imm {
        thread.state = SpuThreadState::Halted;
    } else {
        thread.advance_pc();
    }
    Ok(())
}

/// Move from Special Purpose Register - mfspr rt, sa
pub fn mfspr(thread: &mut SpuThread, _sa: u8, rt: u8) -> Result<(), SpuError> {
    // SPU has limited SPRs - for now, return 0
    // In real hardware, specific SPRs would be read
    thread.regs.write_preferred_u32(rt as usize, 0);
    thread.advance_pc();
    Ok(())
}

/// Move to Special Purpose Register - mtspr sa, rt
pub fn mtspr(thread: &mut SpuThread, _sa: u8, _rt: u8) -> Result<(), SpuError> {
    // SPU has limited SPRs - for now, ignore
    thread.advance_pc();
    Ok(())
}

/// Interrupt Return - iret
pub fn iret(thread: &mut SpuThread) -> Result<(), SpuError> {
    // Return from interrupt - restore state
    // For now, just advance PC
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
    fn test_nop() {
        let mut thread = create_test_thread();
        thread.set_pc(0x100);
        
        nop(&mut thread).unwrap();
        
        assert_eq!(thread.pc(), 0x104);
    }

    #[test]
    fn test_stop() {
        let mut thread = create_test_thread();
        
        stop(&mut thread, 0x1234).unwrap();
        
        assert_eq!(thread.state, SpuThreadState::Halted);
        assert_eq!(thread.stop_signal, 0x1234);
    }

    #[test]
    fn test_heq_halt() {
        let mut thread = create_test_thread();
        thread.regs.write_preferred_u32(1, 42);
        thread.regs.write_preferred_u32(2, 42);
        
        heq(&mut thread, 2, 1).unwrap();
        
        assert_eq!(thread.state, SpuThreadState::Halted);
    }

    #[test]
    fn test_heq_no_halt() {
        let mut thread = create_test_thread();
        thread.set_pc(0x100);
        thread.regs.write_preferred_u32(1, 42);
        thread.regs.write_preferred_u32(2, 100);
        
        heq(&mut thread, 2, 1).unwrap();
        
        assert_eq!(thread.state, SpuThreadState::Stopped);  // Not halted
        assert_eq!(thread.pc(), 0x104);
    }

    #[test]
    fn test_hgt() {
        let mut thread = create_test_thread();
        thread.regs.write_preferred_u32(1, 100);
        thread.regs.write_preferred_u32(2, 50);
        
        hgt(&mut thread, 2, 1).unwrap();
        
        assert_eq!(thread.state, SpuThreadState::Halted);
    }

    #[test]
    fn test_sync() {
        let mut thread = create_test_thread();
        thread.set_pc(0x100);
        
        sync(&mut thread).unwrap();
        
        assert_eq!(thread.pc(), 0x104);
    }
}
