//! SPU quadword shift and rotate instructions
//!
//! These instructions operate on the entire 128-bit register as a single unit.

use crate::thread::SpuThread;
use oc_core::error::SpuError;

/// Helper: Convert [u32; 4] to [u8; 16] (big-endian)
#[inline]
fn u32x4_to_bytes(words: [u32; 4]) -> [u8; 16] {
    let mut bytes = [0u8; 16];
    for (i, word) in words.iter().enumerate() {
        let wb = word.to_be_bytes();
        bytes[i * 4] = wb[0];
        bytes[i * 4 + 1] = wb[1];
        bytes[i * 4 + 2] = wb[2];
        bytes[i * 4 + 3] = wb[3];
    }
    bytes
}

/// Helper: Convert [u8; 16] to [u32; 4] (big-endian)
#[inline]
fn bytes_to_u32x4(bytes: [u8; 16]) -> [u32; 4] {
    [
        u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
        u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
        u32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]),
    ]
}

/// Helper: Convert [u32; 4] to u128 (big-endian)
#[inline]
fn u32x4_to_u128(words: [u32; 4]) -> u128 {
    ((words[0] as u128) << 96) |
    ((words[1] as u128) << 64) |
    ((words[2] as u128) << 32) |
    (words[3] as u128)
}

/// Helper: Convert u128 to [u32; 4] (big-endian)
#[inline]
fn u128_to_u32x4(val: u128) -> [u32; 4] {
    [
        (val >> 96) as u32,
        (val >> 64) as u32,
        (val >> 32) as u32,
        val as u32,
    ]
}

/// Shift Left Quadword by Bytes - shlqby rt, ra, rb
pub fn shlqby(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_preferred_u32(rb as usize);
    let shift = (b & 0x1F) as usize;
    
    let bytes = u32x4_to_bytes(a);
    let mut result_bytes = [0u8; 16];
    
    if shift < 16 {
        for i in 0..(16 - shift) {
            result_bytes[i] = bytes[i + shift];
        }
    }
    
    thread.regs.write_u32x4(rt as usize, bytes_to_u32x4(result_bytes));
    thread.advance_pc();
    Ok(())
}

/// Shift Left Quadword by Bytes Immediate - shlqbyi rt, ra, i7
pub fn shlqbyi(thread: &mut SpuThread, i7: i8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let shift = (i7 & 0x1F) as usize;
    
    let bytes = u32x4_to_bytes(a);
    let mut result_bytes = [0u8; 16];
    
    if shift < 16 {
        for i in 0..(16 - shift) {
            result_bytes[i] = bytes[i + shift];
        }
    }
    
    thread.regs.write_u32x4(rt as usize, bytes_to_u32x4(result_bytes));
    thread.advance_pc();
    Ok(())
}

/// Shift Left Quadword by Bits - shlqbi rt, ra, rb
pub fn shlqbi(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_preferred_u32(rb as usize);
    let shift = (b & 0x7) as u32;
    
    let val = u32x4_to_u128(a);
    let result = val << shift;
    
    thread.regs.write_u32x4(rt as usize, u128_to_u32x4(result));
    thread.advance_pc();
    Ok(())
}

/// Shift Left Quadword by Bits Immediate - shlqbii rt, ra, i7
pub fn shlqbii(thread: &mut SpuThread, i7: i8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let shift = (i7 & 0x7) as u32;
    
    let val = u32x4_to_u128(a);
    let result = val << shift;
    
    thread.regs.write_u32x4(rt as usize, u128_to_u32x4(result));
    thread.advance_pc();
    Ok(())
}

/// Shift Left Quadword by Bytes from Bit Shift Count - shlqbybi rt, ra, rb
pub fn shlqbybi(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_preferred_u32(rb as usize);
    let shift = ((b >> 3) & 0x1F) as usize;  // Byte shift from bits 3-7
    
    let bytes = u32x4_to_bytes(a);
    let mut result_bytes = [0u8; 16];
    
    if shift < 16 {
        for i in 0..(16 - shift) {
            result_bytes[i] = bytes[i + shift];
        }
    }
    
    thread.regs.write_u32x4(rt as usize, bytes_to_u32x4(result_bytes));
    thread.advance_pc();
    Ok(())
}

/// Rotate Quadword by Bytes - rotqby rt, ra, rb
pub fn rotqby(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_preferred_u32(rb as usize);
    let rot = (b & 0xF) as usize;
    
    let bytes = u32x4_to_bytes(a);
    let mut result_bytes = [0u8; 16];
    
    for i in 0..16 {
        result_bytes[i] = bytes[(i + rot) % 16];
    }
    
    thread.regs.write_u32x4(rt as usize, bytes_to_u32x4(result_bytes));
    thread.advance_pc();
    Ok(())
}

/// Rotate Quadword by Bytes Immediate - rotqbyi rt, ra, i7
pub fn rotqbyi(thread: &mut SpuThread, i7: i8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let rot = (i7 & 0xF) as usize;
    
    let bytes = u32x4_to_bytes(a);
    let mut result_bytes = [0u8; 16];
    
    for i in 0..16 {
        result_bytes[i] = bytes[(i + rot) % 16];
    }
    
    thread.regs.write_u32x4(rt as usize, bytes_to_u32x4(result_bytes));
    thread.advance_pc();
    Ok(())
}

/// Rotate Quadword by Bits - rotqbi rt, ra, rb
pub fn rotqbi(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_preferred_u32(rb as usize);
    let rot = (b & 0x7) as u32;
    
    let val = u32x4_to_u128(a);
    let result = val.rotate_left(rot);
    
    thread.regs.write_u32x4(rt as usize, u128_to_u32x4(result));
    thread.advance_pc();
    Ok(())
}

/// Rotate Quadword by Bits Immediate - rotqbii rt, ra, i7
pub fn rotqbii(thread: &mut SpuThread, i7: i8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let rot = (i7 & 0x7) as u32;
    
    let val = u32x4_to_u128(a);
    let result = val.rotate_left(rot);
    
    thread.regs.write_u32x4(rt as usize, u128_to_u32x4(result));
    thread.advance_pc();
    Ok(())
}

/// Rotate Quadword by Bytes from Bit Shift Count - rotqbybi rt, ra, rb
pub fn rotqbybi(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_preferred_u32(rb as usize);
    let rot = ((b >> 3) & 0xF) as usize;  // Byte rotation from bits 3-6
    
    let bytes = u32x4_to_bytes(a);
    let mut result_bytes = [0u8; 16];
    
    for i in 0..16 {
        result_bytes[i] = bytes[(i + rot) % 16];
    }
    
    thread.regs.write_u32x4(rt as usize, bytes_to_u32x4(result_bytes));
    thread.advance_pc();
    Ok(())
}

/// Rotate and Mask Quadword by Bytes - rotqmby rt, ra, rb
pub fn rotqmby(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_preferred_u32(rb as usize);
    let shift = (0u32.wrapping_sub(b) & 0x1F) as usize;
    
    let bytes = u32x4_to_bytes(a);
    let mut result_bytes = [0u8; 16];
    
    if shift < 16 {
        for i in shift..16 {
            result_bytes[i] = bytes[i - shift];
        }
    }
    
    thread.regs.write_u32x4(rt as usize, bytes_to_u32x4(result_bytes));
    thread.advance_pc();
    Ok(())
}

/// Rotate and Mask Quadword by Bytes Immediate - rotqmbyi rt, ra, i7
pub fn rotqmbyi(thread: &mut SpuThread, i7: i8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let shift = (0i8.wrapping_sub(i7) & 0x1F) as usize;
    
    let bytes = u32x4_to_bytes(a);
    let mut result_bytes = [0u8; 16];
    
    if shift < 16 {
        for i in shift..16 {
            result_bytes[i] = bytes[i - shift];
        }
    }
    
    thread.regs.write_u32x4(rt as usize, bytes_to_u32x4(result_bytes));
    thread.advance_pc();
    Ok(())
}

/// Rotate and Mask Quadword by Bits - rotqmbi rt, ra, rb
pub fn rotqmbi(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_preferred_u32(rb as usize);
    let shift = (0u32.wrapping_sub(b) & 0x7) as u32;
    
    let val = u32x4_to_u128(a);
    let result = val >> shift;
    
    thread.regs.write_u32x4(rt as usize, u128_to_u32x4(result));
    thread.advance_pc();
    Ok(())
}

/// Rotate and Mask Quadword by Bits Immediate - rotqmbii rt, ra, i7
pub fn rotqmbii(thread: &mut SpuThread, i7: i8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let shift = (0i8.wrapping_sub(i7) & 0x7) as u32;
    
    let val = u32x4_to_u128(a);
    let result = val >> shift;
    
    thread.regs.write_u32x4(rt as usize, u128_to_u32x4(result));
    thread.advance_pc();
    Ok(())
}

/// Rotate and Mask Quadword by Bytes from Bit Shift Count - rotqmbybi rt, ra, rb
pub fn rotqmbybi(thread: &mut SpuThread, rb: u8, ra: u8, rt: u8) -> Result<(), SpuError> {
    let a = thread.regs.read_u32x4(ra as usize);
    let b = thread.regs.read_preferred_u32(rb as usize);
    let shift = (((0u32.wrapping_sub(b)) >> 3) & 0x1F) as usize;
    
    let bytes = u32x4_to_bytes(a);
    let mut result_bytes = [0u8; 16];
    
    if shift < 16 {
        for i in shift..16 {
            result_bytes[i] = bytes[i - shift];
        }
    }
    
    thread.regs.write_u32x4(rt as usize, bytes_to_u32x4(result_bytes));
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
    fn test_shlqbyi() {
        let mut thread = create_test_thread();
        // Set up source register with sequential bytes
        thread.regs.write_u32x4(1, [0x00010203, 0x04050607, 0x08090A0B, 0x0C0D0E0F]);
        
        // Shift left by 4 bytes
        shlqbyi(&mut thread, 4, 1, 2).unwrap();
        
        let result = thread.regs.read_u32x4(2);
        assert_eq!(result, [0x04050607, 0x08090A0B, 0x0C0D0E0F, 0x00000000]);
    }

    #[test]
    fn test_rotqbyi() {
        let mut thread = create_test_thread();
        thread.regs.write_u32x4(1, [0x00010203, 0x04050607, 0x08090A0B, 0x0C0D0E0F]);
        
        // Rotate left by 4 bytes
        rotqbyi(&mut thread, 4, 1, 2).unwrap();
        
        let result = thread.regs.read_u32x4(2);
        assert_eq!(result, [0x04050607, 0x08090A0B, 0x0C0D0E0F, 0x00010203]);
    }

    #[test]
    fn test_shlqbii() {
        let mut thread = create_test_thread();
        thread.regs.write_u32x4(1, [0x80000000, 0x00000000, 0x00000000, 0x00000001]);
        
        // Shift left by 1 bit
        shlqbii(&mut thread, 1, 1, 2).unwrap();
        
        let result = thread.regs.read_u32x4(2);
        // Bit 127 shifts out, bit 0 becomes 0
        assert_eq!(result, [0x00000000, 0x00000000, 0x00000000, 0x00000002]);
    }

    #[test]
    fn test_rotqmbyi() {
        let mut thread = create_test_thread();
        thread.regs.write_u32x4(1, [0x00010203, 0x04050607, 0x08090A0B, 0x0C0D0E0F]);
        
        // Rotate and mask by -4 bytes (shift right by 4)
        rotqmbyi(&mut thread, -4i8, 1, 2).unwrap();
        
        let result = thread.regs.read_u32x4(2);
        assert_eq!(result, [0x00000000, 0x00010203, 0x04050607, 0x08090A0B]);
    }
}
