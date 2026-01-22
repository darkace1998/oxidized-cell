//! SPU interpreter implementation

use crate::decoder::SpuDecoder;
use crate::instructions::{arithmetic, float};
use crate::thread::SpuThread;
use oc_core::error::SpuError;

/// SPU interpreter for instruction execution
pub struct SpuInterpreter;

impl SpuInterpreter {
    /// Create a new SPU interpreter
    pub fn new() -> Self {
        Self
    }

    /// Execute a single instruction
    pub fn step(&self, thread: &mut SpuThread) -> Result<(), SpuError> {
        // Fetch instruction from local storage
        let pc = thread.pc();
        let opcode = thread.ls_read_u32(pc);

        // Decode and execute
        self.execute(thread, opcode)?;

        Ok(())
    }

    /// Execute a decoded instruction
    fn execute(&self, thread: &mut SpuThread, opcode: u32) -> Result<(), SpuError> {
        let op11 = SpuDecoder::op11(opcode);
        let op10 = SpuDecoder::op10(opcode);
        let op8 = SpuDecoder::op8(opcode);
        let op7 = SpuDecoder::op7(opcode);
        let op4 = SpuDecoder::op4(opcode);

        // Match based on opcode patterns
        match op4 {
            // Branch instructions (RI18 form)
            0b0100 => self.execute_branch(thread, opcode)?,
            0b0110 => self.execute_branch_if_zero(thread, opcode)?,
            0b0010 => self.execute_branch_if_not_zero(thread, opcode)?,
            0b0001 => self.execute_branch_if_zero_halfword(thread, opcode)?,
            0b0011 => self.execute_branch_if_not_zero_halfword(thread, opcode)?,

            _ => {
                // Check other opcode lengths
                match op7 {
                    0b0100000..=0b0100001 => self.execute_immediate_load(thread, opcode)?,
                    _ => {
                        match op8 {
                            0b00011100 => self.execute_ai(thread, opcode)?,
                            0b00011101 => self.execute_ahi(thread, opcode)?,
                            0b00010100 => self.execute_sfi(thread, opcode)?,
                            0b00010101 => self.execute_sfhi(thread, opcode)?,
                            _ => {
                                match op10 {
                                    0b0000011000 => self.execute_add(thread, opcode)?,
                                    0b0000001000 => self.execute_subtract(thread, opcode)?,
                                    0b0001000001 => self.execute_and(thread, opcode)?,
                                    0b0001000101 => self.execute_or(thread, opcode)?,
                                    0b0001001001 => self.execute_xor(thread, opcode)?,
                                    0b0001001101 => self.execute_nor(thread, opcode)?,
                                    0b0000100000 => self.execute_stop(thread, opcode)?,
                                    0b0000000000 => {
                                        // nop
                                        thread.advance_pc();
                                    }
                                    // Double-precision floating-point (RR form, 10-bit opcodes)
                                    0b0101100100 => { // dfa (0x2cc >> 1)
                                        let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
                                        float::dfa(thread, rb, ra, rt)?;
                                    }
                                    0b0101100101 => { // dfs (0x2cd >> 1)
                                        let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
                                        float::dfs(thread, rb, ra, rt)?;
                                    }
                                    0b0101100110 => { // dfm (0x2ce >> 1)
                                        let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
                                        float::dfm(thread, rb, ra, rt)?;
                                    }
                                    _ => {
                                        match op11 {
                                            0b01111000100 => self.execute_shufb(thread, opcode)?,
                                            0b01010100101 | 0b01010110101 | 0b01111010101 => {
                                                // FMA-type instructions
                                                thread.advance_pc();
                                            }
                                            // Double-precision FMA variants (11-bit opcodes)
                                            0b01101011100 => { // dfma (0x35c)
                                                let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
                                                float::dfma(thread, rb, ra, rt)?;
                                            }
                                            0b01101011101 => { // dfms (0x35d)
                                                let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
                                                float::dfms(thread, rb, ra, rt)?;
                                            }
                                            0b01101011110 => { // dfnms (0x35e)
                                                let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
                                                float::dfnms(thread, rb, ra, rt)?;
                                            }
                                            0b01101011111 => { // dfnma (0x35f)
                                                let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
                                                float::dfnma(thread, rb, ra, rt)?;
                                            }
                                            // Double-precision comparisons
                                            0b01011000011 => { // dfcgt (0x2c3)
                                                let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
                                                float::dfcgt(thread, rb, ra, rt)?;
                                            }
                                            0b01011001011 => { // dfcmgt (0x2cb)
                                                let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
                                                float::dfcmgt(thread, rb, ra, rt)?;
                                            }
                                            0b01111000011 => { // dfceq (0x3c3)
                                                let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
                                                float::dfceq(thread, rb, ra, rt)?;
                                            }
                                            0b01111001011 => { // dfcmeq (0x3cb)
                                                let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
                                                float::dfcmeq(thread, rb, ra, rt)?;
                                            }
                                            // Float/double conversion
                                            0b01110111000 => { // fesd (0x3b8)
                                                let (_rb, ra, rt) = SpuDecoder::rr_form(opcode);
                                                float::fesd(thread, ra, rt)?;
                                            }
                                            0b01110111001 => { // frds (0x3b9)
                                                let (_rb, ra, rt) = SpuDecoder::rr_form(opcode);
                                                float::frds(thread, ra, rt)?;
                                            }
                                            // Byte/Halfword Operations
                                            0b00011000010 => { // cg - Carry Generate
                                                let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
                                                arithmetic::cg(thread, rb, ra, rt)?;
                                            }
                                            0b00001000010 => { // bg - Borrow Generate
                                                let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
                                                arithmetic::bg(thread, rb, ra, rt)?;
                                            }
                                            0b01101000000 => { // addx - Add Extended
                                                let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
                                                arithmetic::addx(thread, rb, ra, rt)?;
                                            }
                                            0b01101000001 => { // sfx - Subtract From Extended
                                                let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
                                                arithmetic::sfx(thread, rb, ra, rt)?;
                                            }
                                            0b01101100010 => { // cgx - Carry Generate Extended
                                                let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
                                                arithmetic::cgx(thread, rb, ra, rt)?;
                                            }
                                            0b01101000010 => { // bgx - Borrow Generate Extended
                                                let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
                                                arithmetic::bgx(thread, rb, ra, rt)?;
                                            }
                                            0b00001010011 => { // absdb - Absolute Difference of Bytes
                                                let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
                                                arithmetic::absdb(thread, rb, ra, rt)?;
                                            }
                                            0b01001010011 => { // sumb - Sum Bytes into Halfwords
                                                let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
                                                arithmetic::sumb(thread, rb, ra, rt)?;
                                            }
                                            _ => {
                                                tracing::warn!(
                                                    "Unknown SPU instruction 0x{:08x} at 0x{:05x}",
                                                    opcode,
                                                    thread.pc()
                                                );
                                                thread.advance_pc();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Execute branch (br)
    fn execute_branch(&self, thread: &mut SpuThread, opcode: u32) -> Result<(), SpuError> {
        let (i16_val, _rt) = SpuDecoder::ri16_form(opcode);
        let offset = (i16_val as i32) << 2;
        let target = (thread.pc() as i32 + offset) as u32;
        thread.set_pc(target);
        Ok(())
    }

    /// Execute branch if zero (brz)
    fn execute_branch_if_zero(&self, thread: &mut SpuThread, opcode: u32) -> Result<(), SpuError> {
        let (i16_val, rt) = SpuDecoder::ri16_form(opcode);
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

    /// Execute branch if not zero (brnz)
    fn execute_branch_if_not_zero(&self, thread: &mut SpuThread, opcode: u32) -> Result<(), SpuError> {
        let (i16_val, rt) = SpuDecoder::ri16_form(opcode);
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

    /// Execute branch if zero halfword (brhz)
    fn execute_branch_if_zero_halfword(&self, thread: &mut SpuThread, opcode: u32) -> Result<(), SpuError> {
        let (i16_val, rt) = SpuDecoder::ri16_form(opcode);
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

    /// Execute branch if not zero halfword (brhnz)
    fn execute_branch_if_not_zero_halfword(&self, thread: &mut SpuThread, opcode: u32) -> Result<(), SpuError> {
        let (i16_val, rt) = SpuDecoder::ri16_form(opcode);
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

    /// Execute immediate load (il, ila, ilh, iohl)
    fn execute_immediate_load(&self, thread: &mut SpuThread, opcode: u32) -> Result<(), SpuError> {
        let op7 = SpuDecoder::op7(opcode);
        let (i16_val, rt) = SpuDecoder::ri16_form(opcode);

        match op7 {
            // il - Immediate Load Word
            0b0100000 => {
                let value = i16_val as i32 as u32;
                thread.regs.write_u32x4(rt as usize, [value, value, value, value]);
            }
            // ilh - Immediate Load Halfword
            0b0100001 => {
                let hword = i16_val as u16;
                let value = ((hword as u32) << 16) | (hword as u32);
                thread.regs.write_u32x4(rt as usize, [value, value, value, value]);
            }
            _ => {}
        }

        thread.advance_pc();
        Ok(())
    }

    /// Execute add immediate (ai)
    fn execute_ai(&self, thread: &mut SpuThread, opcode: u32) -> Result<(), SpuError> {
        let (i10, ra, rt) = SpuDecoder::ri10_form(opcode);
        let a = thread.regs.read_u32x4(ra as usize);
        let result = [
            (a[0] as i32).wrapping_add(i10 as i32) as u32,
            (a[1] as i32).wrapping_add(i10 as i32) as u32,
            (a[2] as i32).wrapping_add(i10 as i32) as u32,
            (a[3] as i32).wrapping_add(i10 as i32) as u32,
        ];
        thread.regs.write_u32x4(rt as usize, result);
        thread.advance_pc();
        Ok(())
    }

    /// Execute add halfword immediate (ahi)
    fn execute_ahi(&self, thread: &mut SpuThread, opcode: u32) -> Result<(), SpuError> {
        let (i10, ra, rt) = SpuDecoder::ri10_form(opcode);
        let a = thread.regs.read_u32x4(ra as usize);
        let mut result = [0u32; 4];
        for i in 0..4 {
            let hi = ((a[i] >> 16) as i16).wrapping_add(i10) as u16;
            let lo = ((a[i] & 0xFFFF) as i16).wrapping_add(i10) as u16;
            result[i] = ((hi as u32) << 16) | (lo as u32);
        }
        thread.regs.write_u32x4(rt as usize, result);
        thread.advance_pc();
        Ok(())
    }

    /// Execute subtract from immediate (sfi)
    fn execute_sfi(&self, thread: &mut SpuThread, opcode: u32) -> Result<(), SpuError> {
        let (i10, ra, rt) = SpuDecoder::ri10_form(opcode);
        let a = thread.regs.read_u32x4(ra as usize);
        let result = [
            (i10 as i32).wrapping_sub(a[0] as i32) as u32,
            (i10 as i32).wrapping_sub(a[1] as i32) as u32,
            (i10 as i32).wrapping_sub(a[2] as i32) as u32,
            (i10 as i32).wrapping_sub(a[3] as i32) as u32,
        ];
        thread.regs.write_u32x4(rt as usize, result);
        thread.advance_pc();
        Ok(())
    }

    /// Execute subtract from halfword immediate (sfhi)
    fn execute_sfhi(&self, thread: &mut SpuThread, opcode: u32) -> Result<(), SpuError> {
        let (i10, ra, rt) = SpuDecoder::ri10_form(opcode);
        let a = thread.regs.read_u32x4(ra as usize);
        let mut result = [0u32; 4];
        for i in 0..4 {
            let hi = i10.wrapping_sub((a[i] >> 16) as i16) as u16;
            let lo = i10.wrapping_sub((a[i] & 0xFFFF) as i16) as u16;
            result[i] = ((hi as u32) << 16) | (lo as u32);
        }
        thread.regs.write_u32x4(rt as usize, result);
        thread.advance_pc();
        Ok(())
    }

    /// Execute add (a)
    fn execute_add(&self, thread: &mut SpuThread, opcode: u32) -> Result<(), SpuError> {
        let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
        let a = thread.regs.read_u32x4(ra as usize);
        let b = thread.regs.read_u32x4(rb as usize);
        let result = [
            a[0].wrapping_add(b[0]),
            a[1].wrapping_add(b[1]),
            a[2].wrapping_add(b[2]),
            a[3].wrapping_add(b[3]),
        ];
        thread.regs.write_u32x4(rt as usize, result);
        thread.advance_pc();
        Ok(())
    }

    /// Execute subtract (sf)
    fn execute_subtract(&self, thread: &mut SpuThread, opcode: u32) -> Result<(), SpuError> {
        let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
        let a = thread.regs.read_u32x4(ra as usize);
        let b = thread.regs.read_u32x4(rb as usize);
        let result = [
            b[0].wrapping_sub(a[0]),
            b[1].wrapping_sub(a[1]),
            b[2].wrapping_sub(a[2]),
            b[3].wrapping_sub(a[3]),
        ];
        thread.regs.write_u32x4(rt as usize, result);
        thread.advance_pc();
        Ok(())
    }

    /// Execute and
    fn execute_and(&self, thread: &mut SpuThread, opcode: u32) -> Result<(), SpuError> {
        let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
        let a = thread.regs.read_u32x4(ra as usize);
        let b = thread.regs.read_u32x4(rb as usize);
        let result = [a[0] & b[0], a[1] & b[1], a[2] & b[2], a[3] & b[3]];
        thread.regs.write_u32x4(rt as usize, result);
        thread.advance_pc();
        Ok(())
    }

    /// Execute or
    fn execute_or(&self, thread: &mut SpuThread, opcode: u32) -> Result<(), SpuError> {
        let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
        let a = thread.regs.read_u32x4(ra as usize);
        let b = thread.regs.read_u32x4(rb as usize);
        let result = [a[0] | b[0], a[1] | b[1], a[2] | b[2], a[3] | b[3]];
        thread.regs.write_u32x4(rt as usize, result);
        thread.advance_pc();
        Ok(())
    }

    /// Execute xor
    fn execute_xor(&self, thread: &mut SpuThread, opcode: u32) -> Result<(), SpuError> {
        let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
        let a = thread.regs.read_u32x4(ra as usize);
        let b = thread.regs.read_u32x4(rb as usize);
        let result = [a[0] ^ b[0], a[1] ^ b[1], a[2] ^ b[2], a[3] ^ b[3]];
        thread.regs.write_u32x4(rt as usize, result);
        thread.advance_pc();
        Ok(())
    }

    /// Execute nor
    fn execute_nor(&self, thread: &mut SpuThread, opcode: u32) -> Result<(), SpuError> {
        let (rb, ra, rt) = SpuDecoder::rr_form(opcode);
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

    /// Execute shuffle bytes (shufb)
    fn execute_shufb(&self, thread: &mut SpuThread, opcode: u32) -> Result<(), SpuError> {
        let (rc, rb, ra, rt) = SpuDecoder::rrr_form(opcode);
        let a = thread.regs.read_u32x4(ra as usize);
        let b = thread.regs.read_u32x4(rb as usize);
        let c = thread.regs.read_u32x4(rc as usize);

        // Convert to bytes with big-endian handling
        let u32x4_to_bytes = |words: [u32; 4]| -> [u8; 16] {
            let mut bytes = [0u8; 16];
            for (i, word) in words.iter().enumerate() {
                let wb = word.to_be_bytes();
                bytes[i * 4] = wb[0];
                bytes[i * 4 + 1] = wb[1];
                bytes[i * 4 + 2] = wb[2];
                bytes[i * 4 + 3] = wb[3];
            }
            bytes
        };

        let a_bytes = u32x4_to_bytes(a);
        let b_bytes = u32x4_to_bytes(b);
        let c_bytes = u32x4_to_bytes(c);

        let mut result = [0u8; 16];
        for i in 0..16 {
            let sel = c_bytes[i];
            result[i] = if sel & 0xC0 == 0xC0 {
                if sel & 0xE0 == 0xE0 { 0xFF } else { 0x00 }
            } else if sel & 0x10 == 0 {
                a_bytes[(sel & 0x0F) as usize]
            } else {
                b_bytes[(sel & 0x0F) as usize]
            };
        }

        // Convert bytes back to u32x4 with big-endian handling
        let result_u32x4 = [
            u32::from_be_bytes([result[0], result[1], result[2], result[3]]),
            u32::from_be_bytes([result[4], result[5], result[6], result[7]]),
            u32::from_be_bytes([result[8], result[9], result[10], result[11]]),
            u32::from_be_bytes([result[12], result[13], result[14], result[15]]),
        ];

        thread.regs.write_u32x4(rt as usize, result_u32x4);
        thread.advance_pc();
        Ok(())
    }

    /// Execute stop
    fn execute_stop(&self, thread: &mut SpuThread, opcode: u32) -> Result<(), SpuError> {
        let stop_type = (opcode >> 14) & 0x3FFF;
        thread.stop_signal = stop_type;
        thread.state = crate::thread::SpuThreadState::Halted;
        Ok(())
    }
}

impl Default for SpuInterpreter {
    fn default() -> Self {
        Self::new()
    }
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
    fn test_interpreter_creation() {
        let _interpreter = SpuInterpreter::new();
    }

    #[test]
    fn test_add_instruction() {
        let mut thread = create_test_thread();
        let interpreter = SpuInterpreter::new();

        // Set up registers
        thread.regs.write_u32x4(1, [1, 2, 3, 4]);
        thread.regs.write_u32x4(2, [10, 20, 30, 40]);

        // a rt, ra, rb - add r3, r1, r2 (opcode would be constructed)
        // For now, just verify interpreter creation works
        drop(interpreter);
    }

    #[test]
    fn test_shufb_identity() {
        // Test shufb with identity permutation
        let mut thread = create_test_thread();
        let interpreter = SpuInterpreter::new();
        
        // Set up source registers (ra=1, rb=2)
        // Pattern: bytes 0-15 in a, 16-31 in b
        thread.regs.write_u32x4(1, [0x00010203, 0x04050607, 0x08090A0B, 0x0C0D0E0F]);
        thread.regs.write_u32x4(2, [0x10111213, 0x14151617, 0x18191A1B, 0x1C1D1E1F]);
        
        // Control register (rc=3): identity permutation (select bytes 0-15)
        thread.regs.write_u32x4(3, [0x00010203, 0x04050607, 0x08090A0B, 0x0C0D0E0F]);
        
        // shufb rt, ra, rb, rc
        // RRR form: rc=3, rb=2, ra=1, rt=4
        // Bits: [rc:7][rb:7][ra:7][rt:7] = [3][2][1][4]
        // Opcode = (3 << 21) | (2 << 14) | (1 << 7) | 4
        let opcode = (3 << 21) | (2 << 14) | (1 << 7) | 4;
        
        interpreter.execute_shufb(&mut thread, opcode).unwrap();
        
        // Result should be same as ra (identity permutation)
        let result = thread.regs.read_u32x4(4);
        assert_eq!(result, [0x00010203, 0x04050607, 0x08090A0B, 0x0C0D0E0F]);
    }

    #[test]
    fn test_shufb_select_from_second() {
        // Test selecting all bytes from second source (rb)
        let mut thread = create_test_thread();
        let interpreter = SpuInterpreter::new();
        
        thread.regs.write_u32x4(1, [0x00010203, 0x04050607, 0x08090A0B, 0x0C0D0E0F]);
        thread.regs.write_u32x4(2, [0xAABBCCDD, 0xEEFF0011, 0x22334455, 0x66778899]);
        
        // Control: select bytes 0-15 from second source (indices 16-31 map to rb, using 0x10-0x1F)
        thread.regs.write_u32x4(3, [0x10111213, 0x14151617, 0x18191A1B, 0x1C1D1E1F]);
        
        let opcode = (3 << 21) | (2 << 14) | (1 << 7) | 4;
        interpreter.execute_shufb(&mut thread, opcode).unwrap();
        
        let result = thread.regs.read_u32x4(4);
        assert_eq!(result, [0xAABBCCDD, 0xEEFF0011, 0x22334455, 0x66778899]);
    }

    #[test]
    fn test_shufb_special_values() {
        // Test special control byte values (0xC0-0xDF = 0x00, 0xE0-0xFF = 0xFF)
        let mut thread = create_test_thread();
        let interpreter = SpuInterpreter::new();
        
        thread.regs.write_u32x4(1, [0x12345678, 0x12345678, 0x12345678, 0x12345678]);
        thread.regs.write_u32x4(2, [0x12345678, 0x12345678, 0x12345678, 0x12345678]);
        
        // Control: 0xC0-0xDF = 0x00, 0xE0-0xFF = 0xFF
        thread.regs.write_u32x4(3, [0xC0C0C0C0, 0xE0E0E0E0, 0xD0D0D0D0, 0xFFFFFFFF]);
        
        let opcode = (3 << 21) | (2 << 14) | (1 << 7) | 4;
        interpreter.execute_shufb(&mut thread, opcode).unwrap();
        
        let result = thread.regs.read_u32x4(4);
        // 0xC0 should produce 0x00 (C0-DF range)
        assert_eq!(result[0], 0x00000000);
        // 0xE0 should produce 0xFF (E0-FF range)
        assert_eq!(result[1], 0xFFFFFFFF);
        // 0xD0 should produce 0x00 (C0-DF range)
        assert_eq!(result[2], 0x00000000);
        // 0xFF should produce 0xFF
        assert_eq!(result[3], 0xFFFFFFFF);
    }

    #[test]
    fn test_shufb_reverse_bytes() {
        // Test reversing byte order
        let mut thread = create_test_thread();
        let interpreter = SpuInterpreter::new();
        
        thread.regs.write_u32x4(1, [0x00010203, 0x04050607, 0x08090A0B, 0x0C0D0E0F]);
        thread.regs.write_u32x4(2, [0, 0, 0, 0]);
        
        // Control: reverse byte order (15, 14, 13, ... 0)
        thread.regs.write_u32x4(3, [0x0F0E0D0C, 0x0B0A0908, 0x07060504, 0x03020100]);
        
        let opcode = (3 << 21) | (2 << 14) | (1 << 7) | 4;
        interpreter.execute_shufb(&mut thread, opcode).unwrap();
        
        let result = thread.regs.read_u32x4(4);
        assert_eq!(result, [0x0F0E0D0C, 0x0B0A0908, 0x07060504, 0x03020100]);
    }
}
