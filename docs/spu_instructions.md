# SPU (Synergistic Processing Unit) Instruction Set Implementation

## Overview

The SPU emulation in oxidized-cell implements the Cell Broadband Engine's Synergistic Processing Unit architecture. Each SPU is a specialized SIMD processor with its own local storage and Memory Flow Controller (MFC) for DMA operations. The PS3 has 8 SPUs, with 6 available to user applications.

## Architecture

### Register Set

#### General Purpose Registers (GPRs)
- **128 × 128-bit registers** (r0-r127)
- Each register contains 4 × 32-bit words
- All operations are SIMD by default (operate on all 4 words simultaneously)
- **Preferred slot**: Word 0 (leftmost) is used for scalar operations

Example register layout:
```
Register rt: [word0, word1, word2, word3]
             ^
             |
         Preferred slot
```

### Local Storage

- **256 KB per SPU** (262,144 bytes)
- Directly addressable memory space
- No caching - all loads/stores are to/from local storage
- Isolated from main memory (requires DMA via MFC)
- 16-byte aligned for quadword operations

### Memory Flow Controller (MFC)

The MFC handles DMA transfers between SPU local storage and main memory:

- **DMA Commands**: GET, PUT, GETLLAR, PUTLLC, etc.
- **Tag Groups**: 32 tags for tracking DMA operations
- **Queue Depth**: Up to 16 commands in flight
- **Transfer Size**: Multiples of 16 bytes, up to 16 KB
- **Barrier/Fence**: Synchronization primitives

### SPU Channels

SPU channels provide communication between SPU and PPU/system:

| Channel | ID | Direction | Description |
|---------|-----|-----------|-------------|
| SPU_RdEventStat | 0 | Read | Event status |
| SPU_WrEventMask | 1 | Write | Event mask |
| SPU_WrEventAck | 2 | Write | Event acknowledgment |
| SPU_RdSigNotify1 | 3 | Read | Signal notification 1 |
| SPU_RdSigNotify2 | 4 | Read | Signal notification 2 |
| SPU_WrDec | 7 | Write | Decrementer |
| SPU_RdDec | 8 | Read | Decrementer |
| MFC_WrTagMask | 12 | Write | DMA tag mask |
| MFC_RdTagStat | 13 | Read | DMA tag status |
| MFC_RdListStall | 14 | Read | List stall notify |
| MFC_WrListStallAck | 15 | Write | List stall ack |
| MFC_RdAtomicStat | 16 | Read | Atomic status |
| SPU_WrOutMbox | 28 | Write | Outbound mailbox |
| SPU_RdInMbox | 29 | Read | Inbound mailbox |
| SPU_WrOutIntrMbox | 30 | Write | Outbound interrupt mailbox |

## Instruction Formats

The SPU uses 32-bit big-endian instructions with variable-length opcodes:

- **RRR-Form**: 3 register operands (rc, rb, ra, rt)
- **RR-Form**: 2 register operands (rb, ra, rt)
- **RI7-Form**: Register + 7-bit signed immediate
- **RI10-Form**: Register + 10-bit signed immediate
- **RI16-Form**: Register + 16-bit immediate
- **RI18-Form**: Register + 18-bit immediate (branches)

Opcode identification uses variable-length prefixes (11, 10, 9, 8, 7, or 4 bits).

## Implemented Instructions

### 1. Branch Instructions (14 implementations)

#### Unconditional Branches
- **`br`** - Branch relative
- **`bra`** - Branch absolute
- **`brsl`** - Branch relative and set link
- **`brasl`** - Branch absolute and set link
- **`bi`** - Branch indirect
- **`bisl`** - Branch indirect and set link

```rust
// Example: br 10 - Branch forward by (10 << 2) bytes
br(&mut thread, 10).unwrap();
```

#### Conditional Branches (Register-based)
- **`brz`** - Branch if zero (word)
- **`brnz`** - Branch if not zero (word)
- **`brhz`** - Branch if zero (halfword)
- **`brhnz`** - Branch if not zero (halfword)

```rust
// Example: Branch if register 2 is zero
thread.regs.write_preferred_u32(2, 0);
brz(&mut thread, 10, 2).unwrap();
```

#### Conditional Branches (Indirect)
- **`biz`** - Branch indirect if zero
- **`binz`** - Branch indirect if not zero
- **`bihz`** - Branch indirect if zero halfword
- **`bihnz`** - Branch indirect if not zero halfword

**Key Features:**
- All branch targets are 4-byte aligned (bottom 2 bits ignored)
- Branch offsets are in instructions (multiply by 4 for bytes)
- Link register stores return address for function calls

### 2. Logical Instructions (18 implementations)

#### Basic Logical Operations
- **`and`** - Bitwise AND
- **`or`** - Bitwise OR
- **`xor`** - Bitwise XOR
- **`andc`** - AND with complement (a & ~b)
- **`orc`** - OR with complement (a | ~b)

```rust
// Example: AND two registers
thread.regs.write_u32x4(1, [0xFFFF0000, 0xFF00FF00, 0xF0F0F0F0, 0xAAAAAAAA]);
thread.regs.write_u32x4(2, [0x0000FFFF, 0x00FF00FF, 0x0F0F0F0F, 0x55555555]);
and(&mut thread, 2, 1, 3).unwrap();
// Result in r3: [0x00000000, 0x00000000, 0x00000000, 0x00000000]
```

#### Immediate Logical Operations
- **`andbi`** - AND byte immediate (broadcast to all bytes)
- **`andhi`** - AND halfword immediate (broadcast to all halfwords)
- **`andi`** - AND word immediate (broadcast to all words)
- **`orbi`** - OR byte immediate
- **`orhi`** - OR halfword immediate
- **`ori`** - OR word immediate
- **`xorbi`** - XOR byte immediate
- **`xorhi`** - XOR halfword immediate
- **`xori`** - XOR word immediate

```rust
// Example: Mask all registers to 0xFF
andi(&mut thread, 0xFF, 1, 2).unwrap();
```

#### Advanced Logical Operations
- **`nand`** - NAND (!(a & b))
- **`nor`** - NOR (!(a | b))
- **`eqv`** - Equivalent / XNOR (!(a ^ b))
- **`selb`** - Select bits based on mask

```rust
// Example: Select bits using mask
// Where mask bit is 1, select from rb; where 0, select from ra
thread.regs.write_u32x4(1, [0xAAAAAAAA, 0xAAAAAAAA, 0xAAAAAAAA, 0xAAAAAAAA]);
thread.regs.write_u32x4(2, [0x55555555, 0x55555555, 0x55555555, 0x55555555]);
thread.regs.write_u32x4(3, [0xFFFF0000, 0x00000000, 0xFFFFFFFF, 0x00000000]);
selb(&mut thread, 3, 2, 1, 4).unwrap();
// Result: [0x5555AAAA, 0xAAAAAAAA, 0x55555555, 0xAAAAAAAA]
```

### 3. Arithmetic Instructions

#### Integer Arithmetic
- **`mpy`** - Multiply (signed)
- **`mpyu`** - Multiply unsigned (16-bit × 16-bit)
- **`mpyh`** - Multiply high (upper 16 bits × lower 16 bits)
- **`a`** - Add word
- **`ah`** - Add halfword
- **`sf`** - Subtract from (b - a)
- **`sfh`** - Subtract from halfword

#### Immediate Arithmetic
- **`ai`** - Add word immediate
- **`ahi`** - Add halfword immediate
- **`sfi`** - Subtract from immediate
- **`sfhi`** - Subtract from halfword immediate

#### Shift and Rotate
- **`shl`** - Shift left logical
- **`shlh`** - Shift left logical halfword
- **`shlhi`** - Shift left logical halfword immediate
- **`rot`** - Rotate
- **`roth`** - Rotate halfword
- **`rothi`** - Rotate halfword immediate

```rust
// Example: Multiply two registers
thread.regs.write_u32x4(1, [2, 3, 4, 5]);
thread.regs.write_u32x4(2, [10, 20, 30, 40]);
mpy(&mut thread, 2, 1, 3).unwrap();
// Result: [20, 60, 120, 200]
```

### 4. Memory Instructions

#### Load Quadword
- **`lqd`** - Load quadword (d-form with offset)
- **`lqa`** - Load quadword (absolute address)
- **`lqr`** - Load quadword (PC-relative)
- **`lqx`** - Load quadword (indexed)

```rust
// Example: Load from local storage
let test_data = [0x11111111, 0x22222222, 0x33333333, 0x44444444];
thread.ls_write_u128(0x100, test_data);
lqd(&mut thread, 16, 0, 1).unwrap(); // Load from LS[0x100]
```

#### Store Quadword
- **`stqd`** - Store quadword (d-form with offset)
- **`stqa`** - Store quadword (absolute address)
- **`stqr`** - Store quadword (PC-relative)
- **`stqx`** - Store quadword (indexed)

**Key Features:**
- All loads/stores are 16-byte (quadword) aligned
- Addresses are masked to 16-byte boundaries
- Non-aligned addresses are automatically aligned down

### 5. Compare Instructions

#### Integer Comparisons
- **`ceq`** - Compare equal word
- **`ceqb`** - Compare equal byte
- **`ceqh`** - Compare equal halfword
- **`cgt`** - Compare greater than (signed)
- **`cgth`** - Compare greater than halfword
- **`clgt`** - Compare logical greater than (unsigned)
- **`clgth`** - Compare logical greater than halfword

#### Immediate Comparisons
- **`ceqi`** - Compare equal word immediate
- **`ceqbi`** - Compare equal byte immediate
- **`ceqhi`** - Compare equal halfword immediate
- **`cgti`** - Compare greater than immediate
- **`cgthi`** - Compare greater than halfword immediate
- **`clgti`** - Compare logical greater than immediate
- **`clgthi`** - Compare logical greater than halfword immediate

```rust
// Example: Compare for equality
thread.regs.write_u32x4(1, [10, 20, 30, 40]);
thread.regs.write_u32x4(2, [10, 25, 30, 50]);
ceq(&mut thread, 2, 1, 3).unwrap();
// Result: [0xFFFFFFFF, 0x00000000, 0xFFFFFFFF, 0x00000000]
// (all 1s where equal, all 0s where not equal)
```

### 6. Floating-Point Instructions

#### Basic Arithmetic
- **`fa`** - Floating-point add
- **`fs`** - Floating-point subtract
- **`fm`** - Floating-point multiply

#### Special Functions
- **`fma`** - Floating-point multiply-add (a*b + c)
- **`fms`** - Floating-point multiply-subtract (a*b - c)
- **`fnms`** - Negative floating-point multiply-subtract
- **`frest`** - Floating-point reciprocal estimate
- **`frsqest`** - Floating-point reciprocal square root estimate

```rust
// Example: Floating-point add
let a = [1.0f32.to_bits(), 2.0f32.to_bits(), 3.0f32.to_bits(), 4.0f32.to_bits()];
let b = [0.5f32.to_bits(), 1.5f32.to_bits(), 2.5f32.to_bits(), 3.5f32.to_bits()];
thread.regs.write_u32x4(1, a);
thread.regs.write_u32x4(2, b);
fa(&mut thread, 2, 1, 3).unwrap();
// Result: [1.5, 3.5, 5.5, 7.5]
```

**Precision Notes:**
- SPU uses single-precision (32-bit) floating-point
- Reciprocal estimates have 12-bit precision
- Full IEEE 754 compliance not guaranteed (fast approximate math)

### 7. Channel Instructions

#### Channel Read/Write
- **`rdch`** - Read channel (blocking)
- **`wrch`** - Write channel (blocking)
- **`rchcnt`** - Read channel count (non-blocking status)

```rust
// Example: Write to outbound mailbox
wrch(&mut thread, 28, 3).unwrap(); // Channel 28 = SPU_WR_OUT_MBOX
// Writes value from register 3 to mailbox

// Example: Read from inbound mailbox
rdch(&mut thread, 29, 5).unwrap(); // Channel 29 = SPU_RD_IN_MBOX
// Reads value into register 5
```

**Channel Behavior:**
- Blocking: Thread stalls until channel is ready
- Non-blocking: `rchcnt` returns available count
- Mailboxes: Limited depth (1-4 entries)
- Timeout: Channels can timeout after configurable cycles

### 8. Special Instructions

#### Shuffle and Permute
- **`shufb`** - Shuffle bytes
  - Uses control register to select bytes from two source registers
  - Special values: 0xC0-0xDF = 0x00, 0xE0-0xFF = 0xFF

```rust
// Example: Shuffle bytes - identity permutation
thread.regs.write_u32x4(1, [0x00010203, 0x04050607, 0x08090A0B, 0x0C0D0E0F]);
thread.regs.write_u32x4(2, [0x10111213, 0x14151617, 0x18191A1B, 0x1C1D1E1F]);
thread.regs.write_u32x4(3, [0x00010203, 0x04050607, 0x08090A0B, 0x0C0D0E0F]);
shufb(&mut thread, 3, 2, 1, 4).unwrap();
// Result = ra (identity mapping)
```

#### Control Instructions
- **`stop`** - Stop execution with signal
- **`stopd`** - Stop with dependencies
- **`nop`** - No operation
- **`lnop`** - Long no operation (for scheduling)

#### Synchronization
- **`sync`** - Synchronize (memory barrier)
- **`dsync`** - Data synchronize

## Atomic Operations

### GETLLAR/PUTLLC - Load-Link/Store-Conditional

SPU atomic operations use 128-byte cache-line reservation:

```rust
// Example atomic operation sequence:

// 1. GETLLAR - Get Lock Line And Reserve
// Sets reservation on 128-byte aligned address
thread.mfc.set_reservation(addr, &data);

// 2. Modify data in local storage
// ... perform operations ...

// 3. PUTLLC - Put Lock Line Conditional
// Succeeds only if reservation still valid
if thread.mfc.has_reservation() {
    // Write succeeds - reservation was held
    thread.mfc.clear_reservation();
} else {
    // Write fails - retry needed
}
```

**Reservation Rules:**
- Aligned to 128-byte boundaries
- Lost on any write to the cache line by another processor
- Lost on context switch
- Must be refreshed if operation takes too long

## MFC (Memory Flow Controller) Operations

### DMA Commands

#### GET Commands (Main Memory → Local Storage)
- **GET** - Standard get with dependency
- **GETB** - Get with barrier
- **GETF** - Get with fence  
- **GETL** - Get with lock

#### PUT Commands (Local Storage → Main Memory)
- **PUT** - Standard put with dependency
- **PUTB** - Put with barrier
- **PUTF** - Put with fence
- **PUTL** - Put with lock

```rust
// Example: DMA transfer from main memory
let cmd = MfcDmaCommand {
    lsa: 0x1000,          // Local storage address
    ea: 0x20000000,       // Effective address (main memory)
    size: 0x4000,         // Transfer size (16 KB)
    tag: 0,               // Tag ID for tracking
    cmd: MfcCommand::Get,
    issue_cycle: 0,
    completion_cycle: 0,
};
mfc.queue_command(cmd);

// Wait for completion
while !mfc.check_tags(1 << 0) {
    mfc.tick(1); // Advance simulation
}
```

### Tag Management

- **32 tags** (0-31) for tracking DMA operations
- Tag groups allow waiting on multiple operations
- MFC_WrTagMask channel selects tags to wait on
- MFC_RdTagStat returns completion status

```rust
// Wait for tags 0, 1, 2 to complete
let tag_mask = 0b111; // Tags 0, 1, 2
thread.channels.write(MFC_WR_TAG_MASK, tag_mask);
let status = thread.channels.read(MFC_RD_TAG_STAT);
```

### Timing Model

DMA operations have realistic latency:

| Operation | Base Latency | Transfer Rate |
|-----------|--------------|---------------|
| GET | 100 cycles | 10 cycles/128 bytes |
| GETB/GETF | 120 cycles | 10 cycles/128 bytes |
| PUT | 80 cycles | 10 cycles/128 bytes |
| PUTB/PUTF | 100 cycles | 10 cycles/128 bytes |
| GETLLAR | 150 cycles | - |
| PUTLLC | 120 cycles | - |
| Barrier | 50 cycles | - |

## Programming Model

### SPU Execution Flow

```
1. PPU creates SPU thread context
2. PPU loads program into SPU local storage (via DMA)
3. PPU starts SPU execution
4. SPU runs independently:
   - Executes instructions from local storage
   - Uses MFC for DMA transfers
   - Communicates via mailboxes/signals
5. SPU signals completion (stop instruction or mailbox)
6. PPU reads results and terminates SPU thread
```

### Best Practices

1. **Alignment**: Always align data to 16-byte boundaries
2. **DMA**: Overlap computation with DMA (double buffering)
3. **Branches**: Minimize branching (use SIMD select operations)
4. **Loop Unrolling**: Manually unroll loops for better performance
5. **Register Pressure**: Use all 128 registers effectively
6. **Local Storage**: Keep working set in LS (avoid repeated DMA)

### Example: Vector Addition

```rust
// Vector addition: c = a + b
fn vector_add_spu(a_ea: u64, b_ea: u64, c_ea: u64, count: u32) {
    const BLOCK_SIZE: u32 = 1024; // Process 1024 elements at a time
    
    for i in (0..count).step_by(BLOCK_SIZE as usize) {
        // DMA in block A
        let get_a = MfcDmaCommand {
            lsa: 0x0,
            ea: a_ea + (i * 4) as u64,
            size: BLOCK_SIZE * 4,
            tag: 0,
            cmd: MfcCommand::Get,
            ...
        };
        
        // DMA in block B
        let get_b = MfcDmaCommand {
            lsa: 0x4000,
            ea: b_ea + (i * 4) as u64,
            size: BLOCK_SIZE * 4,
            tag: 1,
            cmd: MfcCommand::Get,
            ...
        };
        
        // Wait for DMAs
        mfc.queue_command(get_a);
        mfc.queue_command(get_b);
        wait_for_tags(0b11);
        
        // Add vectors (SIMD operations)
        for j in (0..BLOCK_SIZE).step_by(4) {
            let a_vec = load_quadword(0x0 + j * 4);
            let b_vec = load_quadword(0x4000 + j * 4);
            let c_vec = add_word(a_vec, b_vec);
            store_quadword(0x8000 + j * 4, c_vec);
        }
        
        // DMA out results
        let put_c = MfcDmaCommand {
            lsa: 0x8000,
            ea: c_ea + (i * 4) as u64,
            size: BLOCK_SIZE * 4,
            tag: 2,
            cmd: MfcCommand::Put,
            ...
        };
        mfc.queue_command(put_c);
        wait_for_tags(0b100);
    }
}
```

## Differences from PPU

| Feature | PPU | SPU |
|---------|-----|-----|
| Register Count | 32 | 128 |
| Register Size | 64-bit | 128-bit |
| Memory Model | Virtual memory | Local storage only |
| SIMD | Optional (AltiVec) | Mandatory (all ops) |
| Branch Prediction | Hardware | Software hints |
| Cache | L1/L2 cache hierarchy | No cache |
| Addressing | 64-bit | 32-bit (LS) |
| Atomic Ops | lwarx/stwcx (8 bytes) | GETLLAR/PUTLLC (128 bytes) |

## Performance Characteristics

### Instruction Throughput

- **Dual-issue**: Two instructions per cycle
- **Even/Odd Pipes**: Instructions dispatched to separate pipelines
- **No Stalls**: With proper scheduling

### Pipeline Stages

1. **Fetch**: Read from LS
2. **Decode**: Parse instruction
3. **Execute**: Perform operation
4. **Writeback**: Update registers

### Latency Guidelines

| Instruction Class | Latency | Notes |
|-------------------|---------|-------|
| Arithmetic | 2 cycles | Pipelined |
| Logical | 2 cycles | Pipelined |
| Shift/Rotate | 4 cycles | Even pipe only |
| Load/Store | 6 cycles | To/from LS |
| Branch | 0 cycles | If predicted |
| Floating-point | 6-7 cycles | Variable |
| Shuffle | 4 cycles | Odd pipe only |

## Testing

The SPU implementation includes comprehensive tests:

### Unit Tests (52 tests)
- Decoder tests (instruction format parsing)
- Thread state tests (register operations)
- Arithmetic tests (mpy, rot, shl)
- Logical tests (and, or, xor, selb)
- Branch tests (br, brz, bi, bisl)
- Memory tests (lqd, stqd, lqa, lqr)
- Compare tests (ceq, cgt, clgt)
- Float tests (fa, fm, frest)
- Channel tests (rdch, wrch, rchcnt)
- Interpreter tests (shufb, execution flow)
- MFC tests (DMA queueing, timing)

### Integration Tests (14 tests)
- Atomic operation tests (GETLLAR/PUTLLC)
- Mailbox communication (SPU ↔ PPU)
- Signal notification
- Event mask/acknowledge
- MFC tag completion
- MFC tag groups
- Channel timeout
- Decrementer
- Barrier synchronization
- Non-blocking channel operations

### Running Tests

```bash
# Run all SPU tests
cargo test --package oc-spu

# Run specific test
cargo test --package oc-spu test_branch_instructions

# Run with output
cargo test --package oc-spu -- --nocapture
```

## Implementation Status

### ✅ Complete
- [x] Thread state (128 registers, 256 KB LS)
- [x] Instruction decoder (all formats)
- [x] Interpreter (fetch-decode-execute)
- [x] MFC (DMA engine with timing)
- [x] Channels (32 channels with mailboxes)
- [x] Branch instructions (14 types)
- [x] Logical instructions (18 types)
- [x] Arithmetic instructions (50+ types: multiply, add, shift, rotate, carry/borrow, extended add/sub, count leading zeros, form select mask, gather bits, sign extension, byte operations)
- [x] Memory instructions (load/store quadword + immediate loads: il, ilh, ilhu, ila, iohl)
- [x] Compare instructions (20+ types: equal, greater than for word/halfword/byte with immediate variants)
- [x] Float instructions (20+ types: add, subtract, multiply, FMA/FMS/FNMS, reciprocal estimates, conversions, double-precision operations)
- [x] Channel instructions (read/write/count)
- [x] Quadword shift/rotate instructions (15 types: shlqby, shlqbyi, shlqbi, rotqby, rotqbyi, rotqbi, rotqmby, rotqmbyi, rotqmbi, and bit-rotate variants)
- [x] Control/hint instructions (16 types: nop, lnop, stop, stopd, sync, dsync, hbr hints, halt variants)
- [x] Atomic operations (reservation system)
- [x] Copy-to-insert instructions (cbd, chd, cwd, cdd, cbx, chx, cwx, cdx)
- [x] Integration with memory manager
- [x] Comprehensive test suite (86+ tests)

### 📋 Future Work
- [ ] JIT compilation (via C++ LLVM backend)
- [ ] SPU isolation mode
- [ ] Interrupt handling
- [ ] Performance profiling
- [ ] SPU debugging tools

## References

- Cell Broadband Engine Architecture (Version 1.02)
- SPU Instruction Set Architecture (Version 1.2)
- SPU Assembly Language Specification (Version 1.6)
- SPU C/C++ Language Extensions (Version 2.0)
- oxidized-cell PPU documentation (for comparison)

## See Also

- [PPU Instructions](./ppu_instructions.md) - PowerPC instruction set
- [Phase 2 Memory Management](./phase2-memory-management.md) - Memory system
- [LV2 Syscalls](./syscalls.md) - Kernel interface (future)
