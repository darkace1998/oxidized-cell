//! SPU channel instructions

use crate::thread::SpuThread;
use oc_core::error::SpuError;

/// Read Channel - rdch rt, ca
pub fn rdch(thread: &mut SpuThread, ca: u8, rt: u8) -> Result<(), SpuError> {
    // Try to read from channel
    if let Some(value) = thread.channels.read(ca as u32) {
        thread.regs.write_preferred_u32(rt as usize, value);
        thread.advance_pc();
        Ok(())
    } else {
        // Channel is empty, would block in real hardware
        // For now, return 0 and continue
        thread.regs.write_preferred_u32(rt as usize, 0);
        thread.advance_pc();
        Ok(())
    }
}

/// Read Channel Count - rchcnt rt, ca
pub fn rchcnt(thread: &mut SpuThread, ca: u8, rt: u8) -> Result<(), SpuError> {
    let count = thread.channels.get_count(ca as u32);
    thread.regs.write_preferred_u32(rt as usize, count);
    thread.advance_pc();
    Ok(())
}

/// Write Channel - wrch ca, rt
pub fn wrch(thread: &mut SpuThread, ca: u8, rt: u8) -> Result<(), SpuError> {
    let value = thread.regs.read_preferred_u32(rt as usize);
    // Try to write to channel
    if thread.channels.write(ca as u32, value) {
        thread.advance_pc();
        Ok(())
    } else {
        // Channel is full, would block in real hardware
        // For now, just continue
        thread.advance_pc();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oc_memory::MemoryManager;
    use crate::channels::channel_ids::*;

    fn create_test_thread() -> SpuThread {
        let memory = MemoryManager::new().unwrap();
        SpuThread::new(0, memory)
    }

    #[test]
    fn test_wrch_rdch() {
        let mut thread = create_test_thread();
        
        // Write to outbound mailbox
        thread.regs.write_preferred_u32(1, 0x12345678);
        wrch(&mut thread, SPU_WR_OUT_MBOX as u8, 1).unwrap();
        
        // Read back (using internal channel access)
        let value = thread.channels.get_outbound_mailbox();
        assert_eq!(value, Some(0x12345678));
    }

    #[test]
    fn test_rchcnt() {
        let mut thread = create_test_thread();
        
        // Check count of a channel
        rchcnt(&mut thread, SPU_RD_IN_MBOX as u8, 1).unwrap();
        
        let count = thread.regs.read_preferred_u32(1);
        assert_eq!(count, 0); // Should be empty initially
    }

    #[test]
    fn test_channel_communication() {
        let mut thread = create_test_thread();
        
        // Put value in inbound mailbox
        thread.channels.put_inbound_mailbox(0xDEADBEEF);
        
        // Read it via rdch
        rdch(&mut thread, SPU_RD_IN_MBOX as u8, 2).unwrap();
        
        let value = thread.regs.read_preferred_u32(2);
        assert_eq!(value, 0xDEADBEEF);
    }
}
