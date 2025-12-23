//! SPU channel system
//!
//! SPU channels are used for communication between SPU and PPU/MFC.

use std::collections::VecDeque;

/// SPU channel numbers
pub mod channel_ids {
    /// SPU Read Event Status
    pub const SPU_RD_EVENT_STAT: u32 = 0;
    /// SPU Write Event Mask
    pub const SPU_WR_EVENT_MASK: u32 = 1;
    /// SPU Write Event Ack
    pub const SPU_WR_EVENT_ACK: u32 = 2;
    /// SPU Signal Notify 1
    pub const SPU_RD_SIGNAL1: u32 = 3;
    /// SPU Signal Notify 2
    pub const SPU_RD_SIGNAL2: u32 = 4;
    /// SPU Write Decrementer
    pub const SPU_WR_DECR: u32 = 7;
    /// SPU Read Decrementer
    pub const SPU_RD_DECR: u32 = 8;
    /// MFC Write Tag Mask
    pub const MFC_WR_TAG_MASK: u32 = 12;
    /// MFC Read Tag Status
    pub const MFC_RD_TAG_STAT: u32 = 13;
    /// MFC Read List Stall Notify
    pub const MFC_RD_LIST_STALL: u32 = 14;
    /// MFC Write List Stall Ack
    pub const MFC_WR_LIST_STALL_ACK: u32 = 15;
    /// MFC Read Atomic Status
    pub const MFC_RD_ATOMIC_STAT: u32 = 16;
    /// SPU Write Outbound Mailbox
    pub const SPU_WR_OUT_MBOX: u32 = 28;
    /// SPU Read Inbound Mailbox
    pub const SPU_RD_IN_MBOX: u32 = 29;
    /// SPU Write Outbound Interrupt Mailbox
    pub const SPU_WR_OUT_INTR_MBOX: u32 = 30;
    /// Number of channels
    pub const NUM_CHANNELS: usize = 32;
}

use channel_ids::*;

/// SPU channel
#[derive(Debug, Clone)]
pub struct SpuChannel {
    /// Channel data queue
    data: VecDeque<u32>,
    /// Maximum queue depth
    max_depth: usize,
    /// Channel count (for count channels)
    count: u32,
    /// Timeout in cycles (0 = no timeout)
    timeout_cycles: u64,
    /// Cycle when waiting started (0 = not waiting)
    wait_start_cycle: u64,
}

impl SpuChannel {
    /// Create a new channel with specified depth
    pub fn new(max_depth: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(max_depth),
            max_depth,
            count: 0,
            timeout_cycles: 10000, // Default timeout: 10000 cycles
            wait_start_cycle: 0,
        }
    }

    /// Set timeout in cycles
    pub fn set_timeout(&mut self, cycles: u64) {
        self.timeout_cycles = cycles;
    }

    /// Start waiting (called when trying to read/write but channel not ready)
    pub fn start_wait(&mut self, current_cycle: u64) {
        if self.wait_start_cycle == 0 {
            self.wait_start_cycle = current_cycle;
        }
    }

    /// Check if timeout has occurred
    pub fn check_timeout(&self, current_cycle: u64) -> bool {
        if self.wait_start_cycle == 0 || self.timeout_cycles == 0 {
            return false;
        }
        (current_cycle - self.wait_start_cycle) >= self.timeout_cycles
    }

    /// Clear wait state
    pub fn clear_wait(&mut self) {
        self.wait_start_cycle = 0;
    }

    /// Push data to channel
    pub fn push(&mut self, value: u32) -> bool {
        if self.data.len() < self.max_depth {
            self.data.push_back(value);
            self.count = self.count.saturating_add(1);
            self.clear_wait();
            true
        } else {
            false
        }
    }

    /// Pop data from channel
    pub fn pop(&mut self) -> Option<u32> {
        let value = self.data.pop_front();
        if value.is_some() {
            self.count = self.count.saturating_sub(1);
            self.clear_wait();
        }
        value
    }

    /// Peek at front of channel
    pub fn peek(&self) -> Option<u32> {
        self.data.front().copied()
    }

    /// Get channel count
    pub fn count(&self) -> u32 {
        self.data.len() as u32
    }

    /// Check if channel is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Check if channel is full
    pub fn is_full(&self) -> bool {
        self.data.len() >= self.max_depth
    }

    /// Clear channel
    pub fn clear(&mut self) {
        self.data.clear();
        self.count = 0;
        self.clear_wait();
    }

    /// Set direct value (for status channels)
    pub fn set(&mut self, value: u32) {
        self.data.clear();
        self.data.push_back(value);
    }
}

/// SPU channels collection
pub struct SpuChannels {
    /// All channels
    channels: [SpuChannel; NUM_CHANNELS],
    /// Event mask
    event_mask: u32,
    /// Tag mask for MFC
    tag_mask: u32,
    /// Decrementer value
    decrementer: u32,
    /// Current cycle counter
    cycle_counter: u64,
}

impl SpuChannels {
    /// Create new channels
    pub fn new() -> Self {
        let channels = std::array::from_fn(|i| {
            let depth = match i as u32 {
                SPU_WR_OUT_MBOX => 1,
                SPU_RD_IN_MBOX => 4,
                SPU_WR_OUT_INTR_MBOX => 1,
                _ => 1,
            };
            SpuChannel::new(depth)
        });

        Self {
            channels,
            event_mask: 0,
            tag_mask: 0,
            decrementer: 0,
            cycle_counter: 0,
        }
    }

    /// Advance cycle counter
    pub fn tick(&mut self, cycles: u64) {
        self.cycle_counter += cycles;
    }

    /// Check for channel timeout on given channel
    pub fn check_channel_timeout(&self, channel: u32) -> bool {
        if (channel as usize) < NUM_CHANNELS {
            self.channels[channel as usize].check_timeout(self.cycle_counter)
        } else {
            false
        }
    }

    /// Read from channel
    pub fn read(&mut self, channel: u32) -> Option<u32> {
        match channel {
            SPU_RD_EVENT_STAT => Some(0), // Simplified
            SPU_RD_DECR => Some(self.decrementer),
            MFC_RD_TAG_STAT => Some(0xFFFFFFFF), // All tags complete (simplified)
            _ if (channel as usize) < NUM_CHANNELS => {
                let ch = &mut self.channels[channel as usize];
                let result = ch.pop();
                if result.is_none() {
                    ch.start_wait(self.cycle_counter);
                }
                result
            }
            _ => None,
        }
    }

    /// Try to read from channel (non-blocking, returns error if would block)
    pub fn try_read(&mut self, channel: u32) -> Result<u32, ()> {
        match channel {
            SPU_RD_EVENT_STAT => Ok(0),
            SPU_RD_DECR => Ok(self.decrementer),
            MFC_RD_TAG_STAT => Ok(0xFFFFFFFF),
            _ if (channel as usize) < NUM_CHANNELS => {
                self.channels[channel as usize].pop().ok_or(())
            }
            _ => Err(()),
        }
    }

    /// Write to channel
    pub fn write(&mut self, channel: u32, value: u32) -> bool {
        match channel {
            SPU_WR_EVENT_MASK => {
                self.event_mask = value;
                true
            }
            SPU_WR_EVENT_ACK => {
                // Acknowledge events (simplified)
                true
            }
            SPU_WR_DECR => {
                self.decrementer = value;
                true
            }
            MFC_WR_TAG_MASK => {
                self.tag_mask = value;
                true
            }
            _ if (channel as usize) < NUM_CHANNELS => {
                let ch = &mut self.channels[channel as usize];
                let success = ch.push(value);
                if !success {
                    ch.start_wait(self.cycle_counter);
                }
                success
            }
            _ => false,
        }
    }

    /// Try to write to channel (non-blocking, returns error if would block)
    pub fn try_write(&mut self, channel: u32, value: u32) -> Result<(), ()> {
        if self.write(channel, value) {
            Ok(())
        } else {
            Err(())
        }
    }

    /// Get channel count
    pub fn get_count(&self, channel: u32) -> u32 {
        match channel {
            SPU_RD_EVENT_STAT => 1,
            SPU_RD_DECR => 1,
            MFC_RD_TAG_STAT => 1,
            _ if (channel as usize) < NUM_CHANNELS => {
                self.channels[channel as usize].count()
            }
            _ => 0,
        }
    }

    /// Get outbound mailbox
    pub fn get_outbound_mailbox(&mut self) -> Option<u32> {
        self.channels[SPU_WR_OUT_MBOX as usize].pop()
    }

    /// Put to inbound mailbox
    pub fn put_inbound_mailbox(&mut self, value: u32) -> bool {
        self.channels[SPU_RD_IN_MBOX as usize].push(value)
    }

    /// Get event mask
    pub fn get_event_mask(&self) -> u32 {
        self.event_mask
    }

    /// Get tag mask
    pub fn get_tag_mask(&self) -> u32 {
        self.tag_mask
    }
}

impl Default for SpuChannels {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_operations() {
        let mut channel = SpuChannel::new(4);

        assert!(channel.is_empty());
        assert!(channel.push(1));
        assert!(channel.push(2));
        assert_eq!(channel.count(), 2);

        assert_eq!(channel.pop(), Some(1));
        assert_eq!(channel.pop(), Some(2));
        assert!(channel.is_empty());
    }

    #[test]
    fn test_channels_mailbox() {
        let mut channels = SpuChannels::new();

        // Test inbound mailbox
        assert!(channels.put_inbound_mailbox(0x12345678));
        assert_eq!(channels.read(SPU_RD_IN_MBOX), Some(0x12345678));

        // Test outbound mailbox
        assert!(channels.write(SPU_WR_OUT_MBOX, 0xDEADBEEF));
        assert_eq!(channels.get_outbound_mailbox(), Some(0xDEADBEEF));
    }
}
