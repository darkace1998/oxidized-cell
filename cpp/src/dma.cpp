/**
 * DMA transfer acceleration engine
 *
 * Implements SPU↔PPU DMA transfers, scatter-gather list commands,
 * and fence/barrier synchronization.
 */

#include "oc_ffi.h"
#include <cstring>
#include <cstdlib>
#include <algorithm>
#include <mutex>
#include <atomic>

// DMA transfer directions
static constexpr uint8_t DMA_CMD_GET      = 0x40;  // EA → LS (read from main memory)
static constexpr uint8_t DMA_CMD_PUT      = 0x20;  // LS → EA (write to main memory)
static constexpr uint8_t DMA_CMD_GETL     = 0x44;  // List GET (scatter-gather read)
static constexpr uint8_t DMA_CMD_PUTL     = 0x24;  // List PUT (scatter-gather write)
static constexpr uint8_t DMA_CMD_GETLB    = 0x4C;  // List GET with barrier
static constexpr uint8_t DMA_CMD_PUTLB    = 0x2C;  // List PUT with barrier
static constexpr uint8_t DMA_CMD_BARRIER  = 0x80;  // DMA barrier
static constexpr uint8_t DMA_CMD_FENCE    = 0xC0;  // DMA fence

// Maximum pending transfers and list entries
static constexpr size_t MAX_DMA_PENDING = 256;
static constexpr size_t MAX_LIST_ENTRIES = 2048;
static constexpr size_t MAX_DMA_SIZE = 16384;  // 16KB per single transfer

// DMA list element (matches PS3 MFC list element format)
struct DmaListElement {
    uint32_t notify;   // Upper 16 bits: stall-and-notify flag
    uint32_t ea_low;   // Effective address (low 32 bits)
    uint32_t size;     // Transfer size in bytes
};

// Single DMA transfer descriptor
struct DmaTransfer {
    uint32_t local_addr;  // SPU local store address
    uint64_t ea;          // Main memory effective address
    uint32_t size;        // Transfer size
    uint16_t tag;         // Tag group (0-31)
    uint8_t  cmd;         // Command type
    bool     active;      // Whether this slot is in use
};

// DMA fence/barrier state per tag group
struct DmaTagState {
    std::atomic<uint32_t> pending_count{0};  // Number of pending transfers
    std::atomic<bool> fence_active{false};   // Fence pending
    std::atomic<bool> barrier_active{false}; // Barrier pending
    std::atomic<uint64_t> sequence{0};       // Sequence number for ordering
};

// DMA engine state
struct DmaEngine {
    DmaTransfer transfers[MAX_DMA_PENDING];
    DmaTagState tag_state[32];  // 32 tag groups
    std::mutex transfer_mutex;
    
    // Statistics
    std::atomic<uint64_t> total_gets{0};
    std::atomic<uint64_t> total_puts{0};
    std::atomic<uint64_t> total_list_gets{0};
    std::atomic<uint64_t> total_list_puts{0};
    std::atomic<uint64_t> total_bytes_in{0};
    std::atomic<uint64_t> total_bytes_out{0};
    std::atomic<uint64_t> total_fences{0};
    std::atomic<uint64_t> total_barriers{0};
    
    DmaEngine() {
        for (auto& t : transfers) {
            t.active = false;
        }
    }
    
    // Find a free transfer slot
    int find_free_slot() {
        for (size_t i = 0; i < MAX_DMA_PENDING; i++) {
            if (!transfers[i].active) return static_cast<int>(i);
        }
        return -1;
    }
};

static DmaEngine g_dma_engine;

extern "C" {

// ============================================================================
// DMA Transfer Acceleration
// ============================================================================

int oc_dma_transfer(void* local_storage, uint32_t local_addr,
                    void* main_memory, uint64_t ea, uint32_t size,
                    uint16_t tag, uint8_t cmd) {
    if (!local_storage || !main_memory) return -1;
    if (size == 0 || size > MAX_DMA_SIZE) return -2;
    if (tag > 31) return -3;
    if (local_addr + size > 0x40000) return -4;  // 256KB SPU local store
    
    auto& engine = g_dma_engine;
    auto& ts = engine.tag_state[tag];
    
    // Check for fence - must wait for all prior transfers on this tag
    if (ts.fence_active.load()) return -5;
    // Check for barrier - must wait for all prior transfers on ALL tags
    if (ts.barrier_active.load()) return -5;
    
    uint8_t* ls = static_cast<uint8_t*>(local_storage) + local_addr;
    uint8_t* mm = static_cast<uint8_t*>(main_memory) + static_cast<uint32_t>(ea);
    
    bool is_get = (cmd == DMA_CMD_GET || cmd == DMA_CMD_GETL || cmd == DMA_CMD_GETLB);
    
    if (is_get) {
        // EA → LS (read from main memory into local store)
        std::memcpy(ls, mm, size);
        engine.total_gets.fetch_add(1);
        engine.total_bytes_in.fetch_add(size);
    } else {
        // LS → EA (write from local store to main memory)
        std::memcpy(mm, ls, size);
        engine.total_puts.fetch_add(1);
        engine.total_bytes_out.fetch_add(size);
    }
    
    // Track in pending for tag completion
    {
        std::lock_guard<std::mutex> lock(engine.transfer_mutex);
        int slot = engine.find_free_slot();
        if (slot >= 0) {
            engine.transfers[slot] = {local_addr, ea, size, tag, cmd, true};
            ts.pending_count.fetch_add(1);
            ts.sequence.fetch_add(1);
        }
    }
    
    return 0;
}

// ============================================================================
// DMA List Commands (Scatter-Gather)
// ============================================================================

int oc_dma_list_transfer(void* local_storage, uint32_t list_addr,
                         void* main_memory, uint32_t list_size,
                         uint16_t tag, uint8_t cmd) {
    if (!local_storage || !main_memory) return -1;
    if (list_size == 0) return -2;
    if (tag > 31) return -3;
    
    bool is_get = (cmd == DMA_CMD_GETL || cmd == DMA_CMD_GETLB);
    bool has_barrier = (cmd == DMA_CMD_GETLB || cmd == DMA_CMD_PUTLB);
    
    auto& engine = g_dma_engine;
    
    // If barrier variant, wait for prior transfers on this tag
    if (has_barrier) {
        auto& ts = engine.tag_state[tag];
        ts.barrier_active.store(true);
        // In a real implementation, we'd spin/wait here
        ts.barrier_active.store(false);
        engine.total_barriers.fetch_add(1);
    }
    
    // Parse the list from local storage
    uint8_t* ls = static_cast<uint8_t*>(local_storage);
    uint32_t local_offset = list_addr;
    uint32_t entries_processed = 0;
    uint32_t bytes_remaining = list_size;
    
    while (bytes_remaining >= 8 && entries_processed < MAX_LIST_ENTRIES) {
        // Each list element: 4 bytes (size + stall), 4 bytes (ea_low)
        uint32_t size_and_stall;
        uint32_t ea_low;
        std::memcpy(&size_and_stall, ls + local_offset, 4);
        std::memcpy(&ea_low, ls + local_offset + 4, 4);
        
        // Big-endian to host conversion (PS3 is big-endian)
        uint32_t transfer_size = __builtin_bswap32(size_and_stall) & 0x7FFF;
        bool stall_and_notify = (__builtin_bswap32(size_and_stall) >> 31) != 0;
        uint64_t ea = __builtin_bswap32(ea_low);
        
        if (transfer_size > 0 && transfer_size <= MAX_DMA_SIZE) {
            uint8_t* mm = static_cast<uint8_t*>(main_memory) + static_cast<uint32_t>(ea);
            
            if (is_get) {
                std::memcpy(ls + local_offset, mm, transfer_size);
                engine.total_bytes_in.fetch_add(transfer_size);
            } else {
                std::memcpy(mm, ls + local_offset, transfer_size);
                engine.total_bytes_out.fetch_add(transfer_size);
            }
        }
        
        local_offset += 8;
        bytes_remaining -= 8;
        entries_processed++;
        
        if (stall_and_notify) break;  // Stall-and-notify terminates the list
    }
    
    if (is_get) engine.total_list_gets.fetch_add(1);
    else engine.total_list_puts.fetch_add(1);
    
    return static_cast<int>(entries_processed);
}

// ============================================================================
// DMA Fence/Barrier Synchronization
// ============================================================================

int oc_dma_fence(uint16_t tag) {
    if (tag > 31) return -1;
    
    auto& engine = g_dma_engine;
    auto& ts = engine.tag_state[tag];
    
    // Fence: all subsequent transfers on this tag must wait for prior ones
    ts.fence_active.store(true);
    engine.total_fences.fetch_add(1);
    
    // Complete all pending transfers for this tag
    {
        std::lock_guard<std::mutex> lock(engine.transfer_mutex);
        for (auto& t : engine.transfers) {
            if (t.active && t.tag == tag) {
                t.active = false;
                ts.pending_count.fetch_sub(1);
            }
        }
    }
    
    ts.fence_active.store(false);
    return 0;
}

int oc_dma_barrier(void) {
    auto& engine = g_dma_engine;
    engine.total_barriers.fetch_add(1);
    
    // Barrier: all subsequent transfers on ALL tags must wait for ALL prior
    for (int tag = 0; tag < 32; tag++) {
        auto& ts = engine.tag_state[tag];
        ts.barrier_active.store(true);
    }
    
    // Complete all pending transfers
    {
        std::lock_guard<std::mutex> lock(engine.transfer_mutex);
        for (auto& t : engine.transfers) {
            if (t.active) {
                engine.tag_state[t.tag].pending_count.fetch_sub(1);
                t.active = false;
            }
        }
    }
    
    for (int tag = 0; tag < 32; tag++) {
        engine.tag_state[tag].barrier_active.store(false);
    }
    
    return 0;
}

uint32_t oc_dma_get_tag_status(void) {
    auto& engine = g_dma_engine;
    uint32_t mask = 0;
    for (int tag = 0; tag < 32; tag++) {
        if (engine.tag_state[tag].pending_count.load() == 0) {
            mask |= (1u << tag);
        }
    }
    return mask;
}

int oc_dma_complete_tag(uint16_t tag) {
    if (tag > 31) return -1;
    
    auto& engine = g_dma_engine;
    auto& ts = engine.tag_state[tag];
    
    std::lock_guard<std::mutex> lock(engine.transfer_mutex);
    for (auto& t : engine.transfers) {
        if (t.active && t.tag == tag) {
            t.active = false;
            ts.pending_count.fetch_sub(1);
        }
    }
    return 0;
}

void oc_dma_get_stats(uint64_t* gets, uint64_t* puts,
                      uint64_t* list_gets, uint64_t* list_puts,
                      uint64_t* bytes_in, uint64_t* bytes_out,
                      uint64_t* fences, uint64_t* barriers) {
    auto& engine = g_dma_engine;
    if (gets) *gets = engine.total_gets.load();
    if (puts) *puts = engine.total_puts.load();
    if (list_gets) *list_gets = engine.total_list_gets.load();
    if (list_puts) *list_puts = engine.total_list_puts.load();
    if (bytes_in) *bytes_in = engine.total_bytes_in.load();
    if (bytes_out) *bytes_out = engine.total_bytes_out.load();
    if (fences) *fences = engine.total_fences.load();
    if (barriers) *barriers = engine.total_barriers.load();
}

void oc_dma_reset_stats(void) {
    auto& engine = g_dma_engine;
    engine.total_gets.store(0);
    engine.total_puts.store(0);
    engine.total_list_gets.store(0);
    engine.total_list_puts.store(0);
    engine.total_bytes_in.store(0);
    engine.total_bytes_out.store(0);
    engine.total_fences.store(0);
    engine.total_barriers.store(0);
    
    // Clear pending transfers
    std::lock_guard<std::mutex> lock(engine.transfer_mutex);
    for (auto& t : engine.transfers) {
        t.active = false;
    }
    for (auto& ts : engine.tag_state) {
        ts.pending_count.store(0);
        ts.fence_active.store(false);
        ts.barrier_active.store(false);
        ts.sequence.store(0);
    }
}

} // extern "C"
