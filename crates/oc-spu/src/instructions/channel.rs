//! SPU channel instructions
//!
//! Implements proper blocking behavior for channel operations as per Cell BE specification.

use crate::thread::{SpuThread, SpuThreadState};
use oc_core::error::SpuError;

/// Blocking behavior for SPU channel operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockingBehavior {
    /// Operation completed without blocking
    NonBlocking,
    /// Thread is blocked waiting for channel read (channel empty)
    BlockingRead { channel: u32, target_reg: u8 },
    /// Thread is blocked waiting for channel write (channel full)
    BlockingWrite { channel: u32, value: u32 },
}

/// Channel operation result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelResult {
    /// Operation completed
    Completed,
    /// Operation would block - caller should handle stalling
    WouldBlock(BlockingBehavior),
}

/// Context saved when a channel operation blocks
#[derive(Debug, Clone, Copy, Default)]
pub struct ChannelContext {
    /// The channel being operated on
    pub channel: u32,
    /// Target register for read operations
    pub target_reg: u8,
    /// Value to write (for write operations)
    pub write_value: u32,
    /// Whether this is a read (true) or write (false) operation
    pub is_read: bool,
    /// PC at time of blocking
    pub blocked_pc: u32,
}

/// Check if a channel read would stall (channel is empty)
pub fn is_channel_stalled(thread: &SpuThread, ca: u32) -> bool {
    thread.channels.get_count(ca) == 0
}

/// Check if a channel write would stall (channel is full)
pub fn is_channel_write_stalled(thread: &SpuThread, ca: u32) -> bool {
    thread.channels.is_channel_full(ca)
}

/// Read Channel - rdch rt, ca
/// 
/// If the channel has data, reads it into rt and advances PC.
/// If the channel is empty, returns WouldBlock and thread should stall.
pub fn rdch(thread: &mut SpuThread, ca: u8, rt: u8) -> Result<ChannelResult, SpuError> {
    // Try to read from channel
    if let Some(value) = thread.channels.read(ca as u32) {
        thread.regs.write_preferred_u32(rt as usize, value);
        thread.advance_pc();
        Ok(ChannelResult::Completed)
    } else {
        // Channel is empty - signal blocking
        // Do NOT advance PC - instruction will be retried when channel has data
        Ok(ChannelResult::WouldBlock(BlockingBehavior::BlockingRead {
            channel: ca as u32,
            target_reg: rt,
        }))
    }
}

/// Read Channel with stall handling - rdch rt, ca
/// 
/// This version properly handles blocking semantics:
/// - If channel has data: reads value, advances PC, returns Ok
/// - If channel is empty: sets thread to Waiting state, does NOT advance PC
pub fn rdch_blocking(thread: &mut SpuThread, ca: u8, rt: u8) -> Result<(), SpuError> {
    match rdch(thread, ca, rt)? {
        ChannelResult::Completed => Ok(()),
        ChannelResult::WouldBlock(_) => {
            // Set thread to waiting state - will be resumed when data arrives
            thread.state = SpuThreadState::Waiting;
            Ok(())
        }
    }
}

/// Read Channel Count - rchcnt rt, ca
/// Returns the number of elements available in the channel
pub fn rchcnt(thread: &mut SpuThread, ca: u8, rt: u8) -> Result<(), SpuError> {
    let count = thread.channels.get_count(ca as u32);
    thread.regs.write_preferred_u32(rt as usize, count);
    thread.advance_pc();
    Ok(())
}

/// Write Channel - wrch ca, rt
/// 
/// If the channel has space, writes the value and advances PC.
/// If the channel is full, returns WouldBlock and thread should stall.
pub fn wrch(thread: &mut SpuThread, ca: u8, rt: u8) -> Result<ChannelResult, SpuError> {
    let value = thread.regs.read_preferred_u32(rt as usize);
    // Try to write to channel
    if thread.channels.write(ca as u32, value) {
        thread.advance_pc();
        Ok(ChannelResult::Completed)
    } else {
        // Channel is full - signal blocking
        // Do NOT advance PC - instruction will be retried when channel has space
        Ok(ChannelResult::WouldBlock(BlockingBehavior::BlockingWrite {
            channel: ca as u32,
            value,
        }))
    }
}

/// Write Channel with stall handling - wrch ca, rt
/// 
/// This version properly handles blocking semantics:
/// - If channel has space: writes value, advances PC, returns Ok
/// - If channel is full: sets thread to Waiting state, does NOT advance PC
pub fn wrch_blocking(thread: &mut SpuThread, ca: u8, rt: u8) -> Result<(), SpuError> {
    match wrch(thread, ca, rt)? {
        ChannelResult::Completed => Ok(()),
        ChannelResult::WouldBlock(_) => {
            // Set thread to waiting state - will be resumed when space available
            thread.state = SpuThreadState::Waiting;
            Ok(())
        }
    }
}

/// Save channel context when blocking
pub fn save_channel_context(
    thread: &SpuThread,
    channel: u32,
    target_reg: u8,
    is_read: bool,
    write_value: u32,
) -> ChannelContext {
    ChannelContext {
        channel,
        target_reg,
        write_value,
        is_read,
        blocked_pc: thread.regs.pc,
    }
}

/// Restore channel context and resume operation
/// Returns true if the operation could now complete
pub fn restore_and_resume(thread: &mut SpuThread, ctx: &ChannelContext) -> Result<bool, SpuError> {
    if ctx.is_read {
        // Try to complete the blocked read
        if let Some(value) = thread.channels.read(ctx.channel) {
            thread.regs.write_preferred_u32(ctx.target_reg as usize, value);
            thread.advance_pc();
            thread.state = SpuThreadState::Running;
            Ok(true)
        } else {
            // Still blocked
            Ok(false)
        }
    } else {
        // Try to complete the blocked write
        if thread.channels.write(ctx.channel, ctx.write_value) {
            thread.advance_pc();
            thread.state = SpuThreadState::Running;
            Ok(true)
        } else {
            // Still blocked
            Ok(false)
        }
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
        let result = rdch(&mut thread, SPU_RD_IN_MBOX as u8, 2).unwrap();
        assert_eq!(result, ChannelResult::Completed);
        
        let value = thread.regs.read_preferred_u32(2);
        assert_eq!(value, 0xDEADBEEF);
    }

    #[test]
    fn test_rdch_blocking() {
        let mut thread = create_test_thread();
        
        // Try to read from empty channel
        let result = rdch(&mut thread, SPU_RD_IN_MBOX as u8, 1).unwrap();
        
        // Should indicate blocking
        match result {
            ChannelResult::WouldBlock(BlockingBehavior::BlockingRead { channel, target_reg }) => {
                assert_eq!(channel, SPU_RD_IN_MBOX);
                assert_eq!(target_reg, 1);
            }
            _ => panic!("Expected WouldBlock"),
        }
    }

    #[test]
    fn test_wrch_blocking() {
        let mut thread = create_test_thread();
        
        // Fill the outbound mailbox (depth 1)
        thread.regs.write_preferred_u32(1, 0x11111111);
        let result = wrch(&mut thread, SPU_WR_OUT_MBOX as u8, 1).unwrap();
        assert_eq!(result, ChannelResult::Completed);
        
        // Try to write again - should block
        thread.regs.write_preferred_u32(2, 0x22222222);
        let result = wrch(&mut thread, SPU_WR_OUT_MBOX as u8, 2).unwrap();
        
        // Should indicate blocking
        match result {
            ChannelResult::WouldBlock(BlockingBehavior::BlockingWrite { channel, value }) => {
                assert_eq!(channel, SPU_WR_OUT_MBOX);
                assert_eq!(value, 0x22222222);
            }
            _ => panic!("Expected WouldBlock"),
        }
    }

    #[test]
    fn test_channel_context_save_restore() {
        let mut thread = create_test_thread();
        
        // Try to read from empty channel - would block
        let _result = rdch(&mut thread, SPU_RD_IN_MBOX as u8, 5).unwrap();
        
        // Save context
        let ctx = save_channel_context(&thread, SPU_RD_IN_MBOX, 5, true, 0);
        assert_eq!(ctx.channel, SPU_RD_IN_MBOX);
        assert_eq!(ctx.target_reg, 5);
        assert!(ctx.is_read);
        
        // Now put data in the mailbox
        thread.channels.put_inbound_mailbox(0xCAFEBABE);
        
        // Resume should succeed
        let resumed = restore_and_resume(&mut thread, &ctx).unwrap();
        assert!(resumed);
        
        // Value should be in register
        let value = thread.regs.read_preferred_u32(5);
        assert_eq!(value, 0xCAFEBABE);
    }

    #[test]
    fn test_is_channel_stalled() {
        let mut thread = create_test_thread();
        
        // Empty channel should be stalled for reads
        assert!(is_channel_stalled(&thread, SPU_RD_IN_MBOX));
        
        // Add data
        thread.channels.put_inbound_mailbox(0x12345678);
        
        // Now should not be stalled
        assert!(!is_channel_stalled(&thread, SPU_RD_IN_MBOX));
    }
}
