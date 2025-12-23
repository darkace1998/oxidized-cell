//! Load/Store instructions for PPU
//!
//! This module contains implementations for PowerPC load and store
//! instructions, including byte-reversal and update forms.

use oc_memory::MemoryManager;
use oc_core::error::PpuError;
use crate::thread::PpuThread;

/// Calculate effective address for load/store with displacement
#[inline]
pub fn calc_ea_d(thread: &PpuThread, ra: u8, d: i16) -> u64 {
    if ra == 0 {
        d as i64 as u64
    } else {
        thread.gpr(ra as usize).wrapping_add(d as i64 as u64)
    }
}

/// Calculate effective address for indexed load/store
#[inline]
pub fn calc_ea_x(thread: &PpuThread, ra: u8, rb: u8) -> u64 {
    if ra == 0 {
        thread.gpr(rb as usize)
    } else {
        thread.gpr(ra as usize).wrapping_add(thread.gpr(rb as usize))
    }
}

/// Load byte and zero extend
pub fn lbz(memory: &MemoryManager, ea: u64) -> Result<u64, PpuError> {
    let value: u8 = memory.read(ea as u32).map_err(|_| PpuError::MemoryError {
        addr: ea as u32,
        message: "lbz failed".to_string(),
    })?;
    Ok(value as u64)
}

/// Load halfword and zero extend (big-endian)
pub fn lhz(memory: &MemoryManager, ea: u64) -> Result<u64, PpuError> {
    let value = memory.read_be16(ea as u32).map_err(|_| PpuError::MemoryError {
        addr: ea as u32,
        message: "lhz failed".to_string(),
    })?;
    Ok(value as u64)
}

/// Load halfword algebraic (sign extend, big-endian)
pub fn lha(memory: &MemoryManager, ea: u64) -> Result<u64, PpuError> {
    let value = memory.read_be16(ea as u32).map_err(|_| PpuError::MemoryError {
        addr: ea as u32,
        message: "lha failed".to_string(),
    })?;
    Ok((value as i16) as i64 as u64)
}

/// Load word and zero extend (big-endian)
pub fn lwz(memory: &MemoryManager, ea: u64) -> Result<u64, PpuError> {
    let value = memory.read_be32(ea as u32).map_err(|_| PpuError::MemoryError {
        addr: ea as u32,
        message: "lwz failed".to_string(),
    })?;
    Ok(value as u64)
}

/// Load word algebraic (sign extend, big-endian)
pub fn lwa(memory: &MemoryManager, ea: u64) -> Result<u64, PpuError> {
    let value = memory.read_be32(ea as u32).map_err(|_| PpuError::MemoryError {
        addr: ea as u32,
        message: "lwa failed".to_string(),
    })?;
    Ok((value as i32) as i64 as u64)
}

/// Load doubleword (big-endian)
pub fn ld(memory: &MemoryManager, ea: u64) -> Result<u64, PpuError> {
    memory.read_be64(ea as u32).map_err(|_| PpuError::MemoryError {
        addr: ea as u32,
        message: "ld failed".to_string(),
    })
}

/// Store byte
pub fn stb(memory: &MemoryManager, ea: u64, value: u64) -> Result<(), PpuError> {
    memory.write(ea as u32, value as u8).map_err(|_| PpuError::MemoryError {
        addr: ea as u32,
        message: "stb failed".to_string(),
    })
}

/// Store halfword (big-endian)
pub fn sth(memory: &MemoryManager, ea: u64, value: u64) -> Result<(), PpuError> {
    memory.write_be16(ea as u32, value as u16).map_err(|_| PpuError::MemoryError {
        addr: ea as u32,
        message: "sth failed".to_string(),
    })
}

/// Store word (big-endian)
pub fn stw(memory: &MemoryManager, ea: u64, value: u64) -> Result<(), PpuError> {
    memory.write_be32(ea as u32, value as u32).map_err(|_| PpuError::MemoryError {
        addr: ea as u32,
        message: "stw failed".to_string(),
    })
}

/// Store doubleword (big-endian)
pub fn std(memory: &MemoryManager, ea: u64, value: u64) -> Result<(), PpuError> {
    memory.write_be64(ea as u32, value).map_err(|_| PpuError::MemoryError {
        addr: ea as u32,
        message: "std failed".to_string(),
    })
}

/// Load halfword byte-reverse (little-endian)
pub fn lhbrx(memory: &MemoryManager, ea: u64) -> Result<u64, PpuError> {
    let value: u16 = memory.read(ea as u32).map_err(|_| PpuError::MemoryError {
        addr: ea as u32,
        message: "lhbrx failed".to_string(),
    })?;
    Ok(value as u64)
}

/// Load word byte-reverse (little-endian)
pub fn lwbrx(memory: &MemoryManager, ea: u64) -> Result<u64, PpuError> {
    let value: u32 = memory.read(ea as u32).map_err(|_| PpuError::MemoryError {
        addr: ea as u32,
        message: "lwbrx failed".to_string(),
    })?;
    Ok(value as u64)
}

/// Load doubleword byte-reverse (little-endian)
pub fn ldbrx(memory: &MemoryManager, ea: u64) -> Result<u64, PpuError> {
    memory.read(ea as u32).map_err(|_| PpuError::MemoryError {
        addr: ea as u32,
        message: "ldbrx failed".to_string(),
    })
}

/// Store halfword byte-reverse (little-endian)
pub fn sthbrx(memory: &MemoryManager, ea: u64, value: u64) -> Result<(), PpuError> {
    memory.write(ea as u32, value as u16).map_err(|_| PpuError::MemoryError {
        addr: ea as u32,
        message: "sthbrx failed".to_string(),
    })
}

/// Store word byte-reverse (little-endian)
pub fn stwbrx(memory: &MemoryManager, ea: u64, value: u64) -> Result<(), PpuError> {
    memory.write(ea as u32, value as u32).map_err(|_| PpuError::MemoryError {
        addr: ea as u32,
        message: "stwbrx failed".to_string(),
    })
}

/// Store doubleword byte-reverse (little-endian)
pub fn stdbrx(memory: &MemoryManager, ea: u64, value: u64) -> Result<(), PpuError> {
    memory.write(ea as u32, value).map_err(|_| PpuError::MemoryError {
        addr: ea as u32,
        message: "stdbrx failed".to_string(),
    })
}

/// Load floating-point single
pub fn lfs(memory: &MemoryManager, ea: u64) -> Result<f64, PpuError> {
    let bits = memory.read_be32(ea as u32).map_err(|_| PpuError::MemoryError {
        addr: ea as u32,
        message: "lfs failed".to_string(),
    })?;
    Ok(f32::from_bits(bits) as f64)
}

/// Load floating-point double
pub fn lfd(memory: &MemoryManager, ea: u64) -> Result<f64, PpuError> {
    let bits = memory.read_be64(ea as u32).map_err(|_| PpuError::MemoryError {
        addr: ea as u32,
        message: "lfd failed".to_string(),
    })?;
    Ok(f64::from_bits(bits))
}

/// Store floating-point single
pub fn stfs(memory: &MemoryManager, ea: u64, value: f64) -> Result<(), PpuError> {
    let bits = (value as f32).to_bits();
    memory.write_be32(ea as u32, bits).map_err(|_| PpuError::MemoryError {
        addr: ea as u32,
        message: "stfs failed".to_string(),
    })
}

/// Store floating-point double
pub fn stfd(memory: &MemoryManager, ea: u64, value: f64) -> Result<(), PpuError> {
    let bits = value.to_bits();
    memory.write_be64(ea as u32, bits).map_err(|_| PpuError::MemoryError {
        addr: ea as u32,
        message: "stfd failed".to_string(),
    })
}

/// Load vector (16 bytes, aligned)
pub fn lvx(memory: &MemoryManager, ea: u64) -> Result<[u32; 4], PpuError> {
    let ea = ea & !0xF; // Align to 16 bytes
    let mut result = [0u32; 4];
    for i in 0..4 {
        result[i] = memory.read_be32((ea + i as u64 * 4) as u32).map_err(|_| PpuError::MemoryError {
            addr: ea as u32,
            message: "lvx failed".to_string(),
        })?;
    }
    Ok(result)
}

/// Store vector (16 bytes, aligned)
pub fn stvx(memory: &MemoryManager, ea: u64, value: [u32; 4]) -> Result<(), PpuError> {
    let ea = ea & !0xF; // Align to 16 bytes
    for i in 0..4 {
        memory.write_be32((ea + i as u64 * 4) as u32, value[i]).map_err(|_| PpuError::MemoryError {
            addr: ea as u32,
            message: "stvx failed".to_string(),
        })?;
    }
    Ok(())
}

/// Load word and reserve indexed (for atomic operations)
pub fn lwarx(memory: &MemoryManager, ea: u64) -> Result<u64, PpuError> {
    // Set reservation on this address
    let reservation = memory.reservation(ea as u32);
    let _time = reservation.acquire();
    
    let value = memory.read_be32(ea as u32).map_err(|_| PpuError::MemoryError {
        addr: ea as u32,
        message: "lwarx failed".to_string(),
    })?;
    Ok(value as u64)
}

/// Store word conditional indexed (for atomic operations)
/// Returns true if store succeeded (reservation was valid)
pub fn stwcx(memory: &MemoryManager, ea: u64, value: u64) -> Result<bool, PpuError> {
    let reservation = memory.reservation(ea as u32);
    let time = reservation.acquire();
    
    if reservation.try_lock(time) {
        memory.write_be32(ea as u32, value as u32).map_err(|_| PpuError::MemoryError {
            addr: ea as u32,
            message: "stwcx failed".to_string(),
        })?;
        reservation.unlock_and_increment();
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Load doubleword and reserve indexed (for atomic operations)
pub fn ldarx(memory: &MemoryManager, ea: u64) -> Result<u64, PpuError> {
    let reservation = memory.reservation(ea as u32);
    let _time = reservation.acquire();
    
    memory.read_be64(ea as u32).map_err(|_| PpuError::MemoryError {
        addr: ea as u32,
        message: "ldarx failed".to_string(),
    })
}

/// Store doubleword conditional indexed (for atomic operations)
/// Returns true if store succeeded (reservation was valid)
pub fn stdcx(memory: &MemoryManager, ea: u64, value: u64) -> Result<bool, PpuError> {
    let reservation = memory.reservation(ea as u32);
    let time = reservation.acquire();
    
    if reservation.try_lock(time) {
        memory.write_be64(ea as u32, value).map_err(|_| PpuError::MemoryError {
            addr: ea as u32,
            message: "stdcx failed".to_string(),
        })?;
        reservation.unlock_and_increment();
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn create_test_memory() -> Arc<MemoryManager> {
        MemoryManager::new().unwrap()
    }

    #[test]
    fn test_calc_ea_d() {
        let mem = create_test_memory();
        let mut thread = crate::thread::PpuThread::new(0, mem);
        
        // Test with ra = 0
        assert_eq!(calc_ea_d(&thread, 0, 100), 100);
        assert_eq!(calc_ea_d(&thread, 0, -100), (-100i64) as u64);
        
        // Test with ra != 0
        thread.set_gpr(1, 0x1000);
        assert_eq!(calc_ea_d(&thread, 1, 8), 0x1008);
        assert_eq!(calc_ea_d(&thread, 1, -8), 0x0FF8);
    }

    #[test]
    fn test_load_store_word() {
        let mem = create_test_memory();
        let ea = 0x2000_0000u64;
        
        stw(&mem, ea, 0x12345678).unwrap();
        assert_eq!(lwz(&mem, ea).unwrap(), 0x12345678);
    }

    #[test]
    fn test_load_store_byte() {
        let mem = create_test_memory();
        let ea = 0x2000_0000u64;
        
        stb(&mem, ea, 0xAB).unwrap();
        assert_eq!(lbz(&mem, ea).unwrap(), 0xAB);
    }
}
