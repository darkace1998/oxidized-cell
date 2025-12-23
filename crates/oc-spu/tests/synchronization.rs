//! Tests for SPU-to-PPU synchronization primitives

use oc_spu::{SpuThread, Mfc};
use oc_spu::mfc::{MfcCommand, MfcDmaCommand};
use oc_spu::channels::channel_ids::*;
use oc_memory::MemoryManager;

fn create_test_thread() -> SpuThread {
    let memory = MemoryManager::new().unwrap();
    SpuThread::new(0, memory)
}

#[test]
fn test_atomic_reservation_getllar() {
    // Test Get Lock Line And Reserve (GETLLAR) operation
    let mut thread = create_test_thread();
    
    // Set up a memory location with full 128-byte data
    let test_addr = 0x20001000u64;
    let mut test_data = [0u8; 128];
    // Fill with pattern
    for i in 0..128 {
        test_data[i] = (i % 256) as u8;
    }
    
    // Perform GETLLAR (simulated)
    thread.mfc.set_reservation(test_addr, &test_data);
    
    assert!(thread.mfc.has_reservation());
    assert_eq!(thread.mfc.get_reservation_addr(), test_addr & !127); // Aligned to 128 bytes
    assert_eq!(thread.mfc.get_reservation_data()[0], 0x00);
    assert_eq!(thread.mfc.get_reservation_data()[127], 127);
}

#[test]
fn test_atomic_putllc_success() {
    // Test Put Lock Line Conditional (PUTLLC) - successful case
    let mut thread = create_test_thread();
    
    let test_addr = 0x20001000u64;
    let initial_data = [0x00u8; 128];
    
    // Establish reservation
    thread.mfc.set_reservation(test_addr, &initial_data);
    assert!(thread.mfc.has_reservation());
    
    // Simulated PUTLLC would check reservation is still valid
    // In a real system, this would write back if reservation is valid
    let success = thread.mfc.has_reservation();
    assert!(success, "PUTLLC should succeed when reservation is valid");
    
    // Clear reservation after successful PUTLLC
    thread.mfc.clear_reservation();
    assert!(!thread.mfc.has_reservation());
}

#[test]
fn test_atomic_putllc_failure() {
    // Test Put Lock Line Conditional (PUTLLC) - failure case
    let thread = create_test_thread();
    
    // Try PUTLLC without establishing reservation first
    assert!(!thread.mfc.has_reservation());
    
    // PUTLLC would fail
    let success = thread.mfc.has_reservation();
    assert!(!success, "PUTLLC should fail without valid reservation");
}

#[test]
fn test_mailbox_spu_to_ppu() {
    // Test outbound mailbox (SPU to PPU)
    let mut thread = create_test_thread();
    
    // SPU writes to outbound mailbox
    let test_value = 0xDEADBEEF;
    thread.channels.write(SPU_WR_OUT_MBOX, test_value);
    
    // PPU reads from SPU outbound mailbox
    let received = thread.channels.get_outbound_mailbox();
    assert_eq!(received, Some(test_value));
    
    // Mailbox should be empty after read
    assert_eq!(thread.channels.get_outbound_mailbox(), None);
}

#[test]
fn test_mailbox_ppu_to_spu() {
    // Test inbound mailbox (PPU to SPU)
    let mut thread = create_test_thread();
    
    // PPU writes to SPU inbound mailbox
    let test_value = 0xCAFEBABE;
    thread.channels.put_inbound_mailbox(test_value);
    
    // SPU reads from inbound mailbox
    let received = thread.channels.read(SPU_RD_IN_MBOX);
    assert_eq!(received, Some(test_value));
    
    // Mailbox should be empty after read
    assert_eq!(thread.channels.read(SPU_RD_IN_MBOX), None);
}

#[test]
fn test_mailbox_multi_value() {
    // Test that inbound mailbox can hold multiple values (depth 4)
    let mut thread = create_test_thread();
    
    // PPU writes multiple values
    assert!(thread.channels.put_inbound_mailbox(0x11111111));
    assert!(thread.channels.put_inbound_mailbox(0x22222222));
    assert!(thread.channels.put_inbound_mailbox(0x33333333));
    assert!(thread.channels.put_inbound_mailbox(0x44444444));
    
    // Check count
    assert_eq!(thread.channels.get_count(SPU_RD_IN_MBOX), 4);
    
    // SPU reads them in order
    assert_eq!(thread.channels.read(SPU_RD_IN_MBOX), Some(0x11111111));
    assert_eq!(thread.channels.read(SPU_RD_IN_MBOX), Some(0x22222222));
    assert_eq!(thread.channels.read(SPU_RD_IN_MBOX), Some(0x33333333));
    assert_eq!(thread.channels.read(SPU_RD_IN_MBOX), Some(0x44444444));
    
    // Should be empty now
    assert_eq!(thread.channels.read(SPU_RD_IN_MBOX), None);
}

#[test]
fn test_signal_notification() {
    // Test signal notification channels
    let mut thread = create_test_thread();
    
    // PPU sends signal 1
    let signal_value = 0x12345678;
    thread.channels.put_inbound_mailbox(signal_value); // Using mailbox as signal proxy
    
    // SPU reads signal
    let received = thread.channels.read(SPU_RD_IN_MBOX);
    assert_eq!(received, Some(signal_value));
}

#[test]
fn test_event_mask_and_ack() {
    // Test event mask and acknowledgment
    let mut thread = create_test_thread();
    
    // Set event mask
    let event_mask = 0b1010; // Enable events 1 and 3
    assert!(thread.channels.write(SPU_WR_EVENT_MASK, event_mask));
    assert_eq!(thread.channels.get_event_mask(), event_mask);
    
    // Acknowledge event
    assert!(thread.channels.write(SPU_WR_EVENT_ACK, 0b0010)); // Ack event 1
}

#[test]
fn test_mfc_tag_completion_wait() {
    // Test waiting for MFC tag completion
    let mut mfc = Mfc::new();
    
    // Queue a DMA command
    let cmd = MfcDmaCommand {
        lsa: 0x1000,
        ea: 0x20000000,
        size: 256,
        tag: 5,
        cmd: MfcCommand::Get,
        issue_cycle: 0,
        completion_cycle: 0,
    };
    
    mfc.queue_command(cmd);
    
    // Tag should be pending
    assert_eq!(mfc.get_tag_status() & (1 << 5), 0);
    assert!(mfc.get_pending_tags() & (1 << 5) != 0);
    
    // Check cycles until completion
    let cycles = mfc.cycles_until_tag_completion(5);
    assert!(cycles.is_some());
    assert!(cycles.unwrap() > 0);
    
    // Advance to completion
    let latency = MfcCommand::Get.base_latency() + MfcCommand::Get.transfer_latency(256);
    mfc.tick(latency);
    
    // Tag should now be complete
    assert_eq!(mfc.get_tag_status() & (1 << 5), 1 << 5);
    assert_eq!(mfc.get_pending_tags() & (1 << 5), 0);
}

#[test]
fn test_mfc_tag_group_completion() {
    // Test waiting for multiple tags (tag group)
    let mut mfc = Mfc::new();
    
    // Queue multiple DMA commands with different tags
    for tag in 0..3 {
        let cmd = MfcDmaCommand {
            lsa: 0x1000 + (tag as u32 * 0x100),
            ea: 0x20000000 + (tag as u64 * 0x1000),
            size: 128,
            tag,
            cmd: MfcCommand::Get,
            issue_cycle: 0,
            completion_cycle: 0,
        };
        mfc.queue_command(cmd);
    }
    
    // Check that tags 0, 1, 2 are pending
    let tag_mask = 0b111; // Tags 0, 1, 2
    assert!(!mfc.check_tags(tag_mask));
    
    // Advance to complete all DMAs
    let latency = MfcCommand::Get.base_latency() + MfcCommand::Get.transfer_latency(128);
    mfc.tick(latency);
    
    // All tags should be complete
    assert!(mfc.check_tags(tag_mask));
}

#[test]
fn test_channel_timeout() {
    // Test channel timeout mechanism
    let mut thread = create_test_thread();
    
    // Start at a non-zero cycle to avoid edge case
    thread.channels.tick(100);
    
    // Try to read from empty channel (would block and start wait)
    let channel_id = SPU_RD_IN_MBOX;
    assert_eq!(thread.channels.read(channel_id), None);
    
    // Advance cycles (less than timeout)
    thread.channels.tick(5000);
    
    // Check if timeout would occur
    let timed_out = thread.channels.check_channel_timeout(channel_id);
    // Timeout shouldn't occur yet (default is 10000 cycles)
    assert!(!timed_out);
    
    // Advance more cycles to reach timeout
    thread.channels.tick(5001);
    
    // Now timeout should occur
    let timed_out = thread.channels.check_channel_timeout(channel_id);
    assert!(timed_out, "Channel should timeout after 10000+ cycles");
    
    // Put data and read - this should clear the wait state
    thread.channels.put_inbound_mailbox(0x12345678);
    let value = thread.channels.read(channel_id);
    assert_eq!(value, Some(0x12345678));
    
    // After successful read, wait state should be cleared
    // If we try to check timeout again, it should not be timed out
    let timed_out_after_read = thread.channels.check_channel_timeout(channel_id);
    assert!(!timed_out_after_read, "Wait state should be cleared after successful read");
}

#[test]
fn test_decrementer() {
    // Test decrementer for timing
    let mut thread = create_test_thread();
    
    // Write to decrementer
    let initial_value = 1000u32;
    assert!(thread.channels.write(SPU_WR_DECR, initial_value));
    
    // Read back
    let value = thread.channels.read(SPU_RD_DECR);
    assert_eq!(value, Some(initial_value));
}

#[test]
fn test_barrier_synchronization() {
    // Test barrier command for synchronization
    let mut mfc = Mfc::new();
    
    // Queue some regular DMAs
    for i in 0..2 {
        let cmd = MfcDmaCommand {
            lsa: 0x1000 + (i * 0x100),
            ea: 0x20000000 + ((i as u64) * 0x1000),
            size: 128,
            tag: i as u8,
            cmd: MfcCommand::Put,
            issue_cycle: 0,
            completion_cycle: 0,
        };
        mfc.queue_command(cmd);
    }
    
    // Queue a barrier
    let barrier_cmd = MfcDmaCommand {
        lsa: 0,
        ea: 0,
        size: 0,
        tag: 10,
        cmd: MfcCommand::Barrier,
        issue_cycle: 0,
        completion_cycle: 0,
    };
    mfc.queue_command(barrier_cmd);
    
    // All should complete after sufficient time
    let max_latency = MfcCommand::Put.base_latency() 
        + MfcCommand::Put.transfer_latency(128)
        + MfcCommand::Barrier.base_latency();
    
    mfc.tick(max_latency);
    
    // All tags should be complete
    let all_tags = (1 << 0) | (1 << 1) | (1 << 10);
    assert!(mfc.check_tags(all_tags));
}

#[test]
fn test_non_blocking_channel_operations() {
    // Test non-blocking channel read/write
    let mut thread = create_test_thread();
    
    // Try non-blocking read on empty channel
    let result = thread.channels.try_read(SPU_RD_IN_MBOX);
    assert!(result.is_err(), "Non-blocking read should fail on empty channel");
    
    // Put data
    thread.channels.put_inbound_mailbox(0x12345678);
    
    // Try non-blocking read again
    let result = thread.channels.try_read(SPU_RD_IN_MBOX);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0x12345678);
    
    // Try non-blocking write to full channel
    // Outbound mailbox has depth 1, so fill it
    assert!(thread.channels.write(SPU_WR_OUT_MBOX, 0xAAAAAAAA));
    
    // Try to write again (should fail as it's full)
    let result = thread.channels.try_write(SPU_WR_OUT_MBOX, 0xBBBBBBBB);
    assert!(result.is_err(), "Non-blocking write should fail on full channel");
}
