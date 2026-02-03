/**
 * oxidized-cell FFI header
 * 
 * This header defines the C interface between Rust and C++ components.
 */

#ifndef OC_FFI_H
#define OC_FFI_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * 128-bit vector type
 */
typedef struct {
    uint8_t data[16];
} oc_v128_t;

/**
 * Initialize C++ runtime
 */
int oc_init(void);

/**
 * Shutdown C++ runtime
 */
void oc_shutdown(void);

// ============================================================================
// PPU JIT Compiler
// ============================================================================

/**
 * PPU JIT compiler handle
 */
typedef struct oc_ppu_jit_t oc_ppu_jit_t;

/**
 * Create PPU JIT compiler
 */
oc_ppu_jit_t* oc_ppu_jit_create(void);

/**
 * Destroy PPU JIT compiler
 */
void oc_ppu_jit_destroy(oc_ppu_jit_t* jit);

/**
 * Compile PPU function
 */
int oc_ppu_jit_compile(oc_ppu_jit_t* jit, uint32_t address, const uint8_t* code, size_t size);

/**
 * Get compiled code for address
 */
void* oc_ppu_jit_get_compiled(oc_ppu_jit_t* jit, uint32_t address);

/**
 * Invalidate compiled code at address
 */
void oc_ppu_jit_invalidate(oc_ppu_jit_t* jit, uint32_t address);

/**
 * Clear entire JIT cache
 */
void oc_ppu_jit_clear_cache(oc_ppu_jit_t* jit);

/**
 * Add breakpoint at address
 */
void oc_ppu_jit_add_breakpoint(oc_ppu_jit_t* jit, uint32_t address);

/**
 * Remove breakpoint at address
 */
void oc_ppu_jit_remove_breakpoint(oc_ppu_jit_t* jit, uint32_t address);

/**
 * Check if breakpoint exists at address
 */
int oc_ppu_jit_has_breakpoint(oc_ppu_jit_t* jit, uint32_t address);

// PPU JIT Branch Prediction APIs

/**
 * Add branch prediction hint
 * hint: 0=none, 1=likely, 2=unlikely, 3=static
 */
void oc_ppu_jit_add_branch_hint(oc_ppu_jit_t* jit, uint32_t address, 
                                 uint32_t target, int hint);

/**
 * Predict branch direction
 * Returns: 1=taken, 0=not taken
 */
int oc_ppu_jit_predict_branch(oc_ppu_jit_t* jit, uint32_t address);

/**
 * Update branch prediction based on actual behavior
 */
void oc_ppu_jit_update_branch(oc_ppu_jit_t* jit, uint32_t address, int taken);

/**
 * Set default branch prediction thresholds for new branches
 * likely_threshold: multiplier for taken_count > not_taken_count * threshold
 * unlikely_threshold: multiplier for not_taken_count > taken_count * threshold
 */
void oc_ppu_jit_set_branch_thresholds(oc_ppu_jit_t* jit, uint32_t likely_threshold,
                                       uint32_t unlikely_threshold);

/**
 * Set branch prediction thresholds for a specific branch address
 */
void oc_ppu_jit_set_branch_thresholds_for_address(oc_ppu_jit_t* jit, uint32_t address,
                                                   uint32_t likely_threshold,
                                                   uint32_t unlikely_threshold);

/**
 * Get prediction accuracy for a specific branch
 * Returns: accuracy percentage (0-100), or -1 if branch not found
 */
double oc_ppu_jit_get_branch_accuracy(oc_ppu_jit_t* jit, uint32_t address);

/**
 * Get aggregate branch prediction statistics
 * Outputs: total_correct, total_incorrect predictions, and overall_accuracy percentage
 */
void oc_ppu_jit_get_branch_stats(oc_ppu_jit_t* jit, uint64_t* total_correct,
                                  uint64_t* total_incorrect, double* overall_accuracy);

/**
 * Reset all branch prediction statistics
 */
void oc_ppu_jit_reset_branch_stats(oc_ppu_jit_t* jit);

// PPU JIT Inline Cache APIs

/**
 * Add inline cache entry for call site
 */
void oc_ppu_jit_add_inline_cache(oc_ppu_jit_t* jit, uint32_t call_site, 
                                  uint32_t target);

/**
 * Lookup cached compiled code for call site
 */
void* oc_ppu_jit_lookup_inline_cache(oc_ppu_jit_t* jit, uint32_t call_site);

/**
 * Invalidate inline cache entries for target
 */
void oc_ppu_jit_invalidate_inline_cache(oc_ppu_jit_t* jit, uint32_t target);

// PPU JIT Branch Target Cache (BTB) APIs

/**
 * Add entry to Branch Target Buffer
 * Maps an indirect branch address to its target
 */
void oc_ppu_jit_btb_add(oc_ppu_jit_t* jit, uint32_t branch_address, 
                        uint32_t target_address);

/**
 * Lookup predicted target for indirect branch
 * Returns: predicted target address, or 0 if not found
 */
uint32_t oc_ppu_jit_btb_lookup(oc_ppu_jit_t* jit, uint32_t branch_address);

/**
 * Update BTB with actual target taken
 * Promotes monomorphic to polymorphic if different targets observed
 */
void oc_ppu_jit_btb_update(oc_ppu_jit_t* jit, uint32_t branch_address, 
                           uint32_t actual_target);

/**
 * Validate that cached target matches expected
 * Returns: 1 if target is cached, 0 otherwise
 */
int oc_ppu_jit_btb_validate(oc_ppu_jit_t* jit, uint32_t branch_address, 
                            uint32_t expected_target);

/**
 * Invalidate BTB entry for branch address
 */
void oc_ppu_jit_btb_invalidate(oc_ppu_jit_t* jit, uint32_t branch_address);

/**
 * Invalidate all BTB entries pointing to a target
 */
void oc_ppu_jit_btb_invalidate_target(oc_ppu_jit_t* jit, uint32_t target_address);

/**
 * Update compiled code pointer for branch -> target mapping
 */
void oc_ppu_jit_btb_update_compiled(oc_ppu_jit_t* jit, uint32_t branch_address,
                                     uint32_t target_address, void* compiled);

/**
 * Get compiled code for branch -> target mapping
 * Returns: compiled code pointer, or NULL if not found
 */
void* oc_ppu_jit_btb_get_compiled(oc_ppu_jit_t* jit, uint32_t branch_address,
                                   uint32_t target_address);

/**
 * Get BTB statistics
 * Outputs: total_lookups, total_hits, total_misses, hit_rate percentage
 */
void oc_ppu_jit_btb_get_stats(oc_ppu_jit_t* jit, uint64_t* total_lookups,
                               uint64_t* total_hits, uint64_t* total_misses,
                               double* hit_rate);

/**
 * Reset BTB statistics
 */
void oc_ppu_jit_btb_reset_stats(oc_ppu_jit_t* jit);

/**
 * Clear all BTB entries
 */
void oc_ppu_jit_btb_clear(oc_ppu_jit_t* jit);

// PPU JIT Constant Propagation Cache APIs

/**
 * Cache an immediate value from an instruction
 */
void oc_ppu_jit_const_set_imm(oc_ppu_jit_t* jit, uint32_t instr_addr, uint64_t value);

/**
 * Get cached immediate value
 * Returns: 1 if found, 0 if not found
 */
int oc_ppu_jit_const_get_imm(oc_ppu_jit_t* jit, uint32_t instr_addr, uint64_t* out_value);

/**
 * Set known value for a register at a specific block
 * is_constant: 1 if compile-time constant, 0 otherwise
 */
void oc_ppu_jit_const_set_reg(oc_ppu_jit_t* jit, uint32_t block_addr, uint8_t reg_num,
                               uint64_t value, uint32_t def_addr, int is_constant);

/**
 * Get known value for a register at a specific block
 * Returns: 1 if found, 0 if not found
 * out_is_constant: set to 1 if value is compile-time constant
 */
int oc_ppu_jit_const_get_reg(oc_ppu_jit_t* jit, uint32_t block_addr, uint8_t reg_num,
                              uint64_t* out_value, int* out_is_constant);

/**
 * Invalidate a register value at a specific block
 */
void oc_ppu_jit_const_invalidate_reg(oc_ppu_jit_t* jit, uint32_t block_addr, uint8_t reg_num);

/**
 * Invalidate all register values for a block (e.g., at function call)
 */
void oc_ppu_jit_const_invalidate_all_regs(oc_ppu_jit_t* jit, uint32_t block_addr);

/**
 * Cache a memory load value
 * size: load size in bytes (1, 2, 4, 8)
 */
void oc_ppu_jit_const_set_mem(oc_ppu_jit_t* jit, uint32_t mem_addr, uint64_t value,
                               uint8_t size, uint32_t load_addr);

/**
 * Get cached memory load value
 * Returns: 1 if found, 0 if not found
 */
int oc_ppu_jit_const_get_mem(oc_ppu_jit_t* jit, uint32_t mem_addr, uint64_t* out_value,
                              uint8_t* out_size);

/**
 * Invalidate cached memory value at address
 */
void oc_ppu_jit_const_invalidate_mem(oc_ppu_jit_t* jit, uint32_t mem_addr);

/**
 * Invalidate cached memory values in range (for stores)
 */
void oc_ppu_jit_const_invalidate_mem_range(oc_ppu_jit_t* jit, uint32_t start_addr, 
                                            uint32_t size);

/**
 * Get constant propagation cache statistics
 */
void oc_ppu_jit_const_get_stats(oc_ppu_jit_t* jit, uint64_t* imm_hits, uint64_t* imm_misses,
                                 uint64_t* reg_hits, uint64_t* reg_misses,
                                 uint64_t* mem_hits, uint64_t* mem_misses);

/**
 * Reset constant propagation cache statistics
 */
void oc_ppu_jit_const_reset_stats(oc_ppu_jit_t* jit);

/**
 * Clear all constant propagation cache entries
 */
void oc_ppu_jit_const_clear(oc_ppu_jit_t* jit);

// PPU JIT Register Allocation APIs

/**
 * Analyze register usage in a basic block
 */
void oc_ppu_jit_analyze_registers(oc_ppu_jit_t* jit, uint32_t address,
                                   const uint32_t* instructions, size_t count);

/**
 * Get register allocation hint
 * Returns: 0=none, 1=caller, 2=callee, 3=float, 4=vector
 */
int oc_ppu_jit_get_reg_hint(oc_ppu_jit_t* jit, uint32_t address, uint8_t reg);

/**
 * Get live GPR mask for a block
 */
uint32_t oc_ppu_jit_get_live_gprs(oc_ppu_jit_t* jit, uint32_t address);

/**
 * Get modified GPR mask for a block
 */
uint32_t oc_ppu_jit_get_modified_gprs(oc_ppu_jit_t* jit, uint32_t address);

// PPU JIT Enhanced Register Allocation APIs

/**
 * Add control flow edge for cross-block liveness analysis
 */
void oc_ppu_jit_reg_add_edge(oc_ppu_jit_t* jit, uint32_t from_addr, uint32_t to_addr);

/**
 * Propagate liveness across blocks (backwards dataflow analysis)
 * Returns: 1 if converged, 0 if still iterating
 */
int oc_ppu_jit_reg_propagate_liveness(oc_ppu_jit_t* jit);

/**
 * Allocate a spill slot for a register
 * reg_type: 0=GPR, 1=FPR, 2=VR
 * Returns: slot ID for later retrieval
 */
uint32_t oc_ppu_jit_reg_allocate_spill(oc_ppu_jit_t* jit, uint8_t reg_num, 
                                        uint8_t reg_type, uint32_t spill_addr);

/**
 * Free a spill slot after filling
 */
void oc_ppu_jit_reg_free_spill(oc_ppu_jit_t* jit, uint32_t slot_id, uint32_t fill_addr);

/**
 * Get stack offset for a spill slot
 * Returns: stack offset, or -1 if not found
 */
int oc_ppu_jit_reg_get_spill_offset(oc_ppu_jit_t* jit, uint32_t slot_id);

/**
 * Check if a register needs to be spilled at a block
 * Returns: 1 if spill needed, 0 otherwise
 */
int oc_ppu_jit_reg_needs_spill(oc_ppu_jit_t* jit, uint32_t block_addr, 
                                uint8_t reg_num, uint8_t reg_type);

/**
 * Get live-in register mask for a block
 * reg_type: 0=GPR, 1=FPR, 2=VR
 */
uint32_t oc_ppu_jit_reg_get_live_in(oc_ppu_jit_t* jit, uint32_t block_addr, uint8_t reg_type);

/**
 * Get live-out register mask for a block
 * reg_type: 0=GPR, 1=FPR, 2=VR
 */
uint32_t oc_ppu_jit_reg_get_live_out(oc_ppu_jit_t* jit, uint32_t block_addr, uint8_t reg_type);

/**
 * Run register copy coalescing pass
 * Returns: number of copies eliminated
 */
size_t oc_ppu_jit_reg_coalesce_copies(oc_ppu_jit_t* jit);

/**
 * Get coalesced register (after copy elimination)
 */
uint8_t oc_ppu_jit_reg_get_coalesced(oc_ppu_jit_t* jit, uint8_t reg, uint8_t reg_type);

/**
 * Get register allocation statistics
 */
void oc_ppu_jit_reg_get_stats(oc_ppu_jit_t* jit, uint64_t* blocks_analyzed,
                               uint64_t* total_spills, uint64_t* total_fills,
                               uint64_t* copies_eliminated);

/**
 * Reset register allocation statistics
 */
void oc_ppu_jit_reg_reset_stats(oc_ppu_jit_t* jit);

/**
 * Clear all register allocation state
 */
void oc_ppu_jit_reg_clear(oc_ppu_jit_t* jit);

// PPU JIT Lazy Compilation APIs

/**
 * Enable/disable lazy compilation
 */
void oc_ppu_jit_enable_lazy(oc_ppu_jit_t* jit, int enable);

/**
 * Check if lazy compilation is enabled
 */
int oc_ppu_jit_is_lazy_enabled(oc_ppu_jit_t* jit);

/**
 * Register code for lazy compilation
 */
void oc_ppu_jit_register_lazy(oc_ppu_jit_t* jit, uint32_t address,
                               const uint8_t* code, size_t size, 
                               uint32_t threshold);

/**
 * Check if code should be compiled (based on execution count)
 */
int oc_ppu_jit_should_compile_lazy(oc_ppu_jit_t* jit, uint32_t address);

/**
 * Get lazy compilation state
 * Returns: 0=not compiled, 1=pending, 2=compiling, 3=compiled, 4=failed
 */
int oc_ppu_jit_get_lazy_state(oc_ppu_jit_t* jit, uint32_t address);

// PPU JIT Enhanced Lazy Compilation APIs

/**
 * Set default compilation threshold for new registrations
 */
void oc_ppu_jit_lazy_set_default_threshold(oc_ppu_jit_t* jit, uint32_t threshold);

/**
 * Get default compilation threshold
 */
uint32_t oc_ppu_jit_lazy_get_default_threshold(oc_ppu_jit_t* jit);

/**
 * Set hot path threshold (paths with >= this count are considered hot)
 */
void oc_ppu_jit_lazy_set_hot_threshold(oc_ppu_jit_t* jit, uint32_t threshold);

/**
 * Register code for lazy compilation (enhanced version)
 * Uses default threshold if threshold=0
 */
void oc_ppu_jit_lazy_register(oc_ppu_jit_t* jit, uint32_t address,
                               const uint8_t* code, size_t size, 
                               uint32_t threshold);

/**
 * Record execution and check if should compile
 * Returns: 1 if should compile now, 0 otherwise
 */
int oc_ppu_jit_lazy_record_execution(oc_ppu_jit_t* jit, uint32_t address);

/**
 * Get execution count for an address
 */
uint32_t oc_ppu_jit_lazy_get_execution_count(oc_ppu_jit_t* jit, uint32_t address);

/**
 * Get lazy state (enhanced version with same return values)
 */
int oc_ppu_jit_lazy_get_enhanced_state(oc_ppu_jit_t* jit, uint32_t address);

/**
 * Get next hot address to compile (highest priority)
 * Returns: address or 0 if none pending
 */
uint32_t oc_ppu_jit_lazy_get_next_hot_address(oc_ppu_jit_t* jit);

/**
 * Get list of hot addresses sorted by execution count
 * Returns: number of entries written
 */
size_t oc_ppu_jit_lazy_get_hot_addresses(oc_ppu_jit_t* jit, uint32_t* addresses, 
                                          uint32_t* exec_counts, int* compiled,
                                          size_t max_count);

/**
 * Get count of pending compilations
 */
size_t oc_ppu_jit_lazy_get_pending_count(oc_ppu_jit_t* jit);

/**
 * Mark entry as compiling
 */
void oc_ppu_jit_lazy_mark_compiling(oc_ppu_jit_t* jit, uint32_t address);

/**
 * Mark entry as compiled
 */
void oc_ppu_jit_lazy_mark_compiled(oc_ppu_jit_t* jit, uint32_t address);

/**
 * Mark entry as failed
 */
void oc_ppu_jit_lazy_mark_failed(oc_ppu_jit_t* jit, uint32_t address);

/**
 * Get lazy compilation statistics
 */
void oc_ppu_jit_lazy_get_stats(oc_ppu_jit_t* jit, uint64_t* total_registered,
                                uint64_t* total_compiled, uint64_t* total_failed,
                                uint64_t* total_executions, uint64_t* hot_promotions,
                                uint64_t* stub_calls);

/**
 * Reset lazy compilation statistics
 */
void oc_ppu_jit_lazy_reset_stats(oc_ppu_jit_t* jit);

/**
 * Clear all lazy compilation entries and state
 */
void oc_ppu_jit_lazy_clear(oc_ppu_jit_t* jit);

// PPU JIT Tiered Compilation APIs

/**
 * Set tier promotion thresholds
 * tier0_to_1: Executions before Interpreter → Baseline (default: 10)
 * tier1_to_2: Executions at tier 1 before Baseline → Optimizing (default: 1000)
 */
void oc_ppu_jit_tiered_set_thresholds(oc_ppu_jit_t* jit, uint32_t tier0_to_1, uint32_t tier1_to_2);

/**
 * Get current tier promotion thresholds
 */
void oc_ppu_jit_tiered_get_thresholds(oc_ppu_jit_t* jit, uint32_t* tier0_to_1, uint32_t* tier1_to_2);

/**
 * Register code for tiered compilation
 * Use 0 for thresholds to use defaults
 */
void oc_ppu_jit_tiered_register(oc_ppu_jit_t* jit, uint32_t address,
                                 const uint8_t* code, size_t size,
                                 uint32_t tier0_to_1, uint32_t tier1_to_2);

/**
 * Record execution and check if promotion is needed
 * Returns: current or target tier (0=Interpreter, 1=Baseline, 2=Optimizing)
 */
int oc_ppu_jit_tiered_record_execution(oc_ppu_jit_t* jit, uint32_t address);

/**
 * Get current tier for address
 * Returns: 0=Interpreter, 1=Baseline, 2=Optimizing
 */
int oc_ppu_jit_tiered_get_tier(oc_ppu_jit_t* jit, uint32_t address);

/**
 * Promote code to specified tier
 * target_tier: 1=Baseline, 2=Optimizing
 * Returns: 1 if successful, 0 if failed
 */
int oc_ppu_jit_tiered_promote(oc_ppu_jit_t* jit, uint32_t address, int target_tier);

/**
 * Get compiled code pointer for address (at current tier)
 * Returns: code pointer or NULL if at interpreter tier
 */
void* oc_ppu_jit_tiered_get_code(oc_ppu_jit_t* jit, uint32_t address);

/**
 * Get execution count for address
 */
uint32_t oc_ppu_jit_tiered_get_execution_count(oc_ppu_jit_t* jit, uint32_t address);

/**
 * Get count of entries at each tier
 */
void oc_ppu_jit_tiered_get_tier_counts(oc_ppu_jit_t* jit, size_t* tier0, size_t* tier1, size_t* tier2);

/**
 * Get tiered compilation statistics
 */
void oc_ppu_jit_tiered_get_stats(oc_ppu_jit_t* jit, uint64_t* total_registered,
                                  uint64_t* tier0_execs, uint64_t* tier1_execs, uint64_t* tier2_execs,
                                  uint64_t* tier0_to_1_promotions, uint64_t* tier1_to_2_promotions);

/**
 * Reset tiered compilation statistics
 */
void oc_ppu_jit_tiered_reset_stats(oc_ppu_jit_t* jit);

/**
 * Clear all tiered compilation entries
 */
void oc_ppu_jit_tiered_clear(oc_ppu_jit_t* jit);

// PPU JIT Multi-threaded Compilation APIs

/**
 * Start compilation thread pool
 */
void oc_ppu_jit_start_compile_threads(oc_ppu_jit_t* jit, size_t num_threads);

/**
 * Stop compilation thread pool
 */
void oc_ppu_jit_stop_compile_threads(oc_ppu_jit_t* jit);

/**
 * Submit compilation task
 */
void oc_ppu_jit_submit_compile_task(oc_ppu_jit_t* jit, uint32_t address,
                                     const uint8_t* code, size_t size,
                                     int priority);

/**
 * Get number of pending compilation tasks
 */
size_t oc_ppu_jit_get_pending_tasks(oc_ppu_jit_t* jit);

/**
 * Get number of completed compilation tasks
 */
size_t oc_ppu_jit_get_completed_tasks(oc_ppu_jit_t* jit);

/**
 * Check if multi-threaded compilation is enabled
 */
int oc_ppu_jit_is_multithreaded(oc_ppu_jit_t* jit);

// PPU JIT Enhanced Thread Pool APIs

/**
 * Submit task to enhanced thread pool with timing tracking
 */
void oc_ppu_jit_pool_submit(oc_ppu_jit_t* jit, uint32_t address,
                             const uint8_t* code, size_t size, int priority);

/**
 * Wait for all pending tasks to complete
 * timeout_ms: 0 = wait indefinitely
 * Returns: 1 if all completed, 0 if timeout
 */
int oc_ppu_jit_pool_wait_all(oc_ppu_jit_t* jit, uint32_t timeout_ms);

/**
 * Cancel all pending tasks
 * Returns: number of cancelled tasks
 */
size_t oc_ppu_jit_pool_cancel_all(oc_ppu_jit_t* jit);

/**
 * Get thread count in pool
 */
size_t oc_ppu_jit_pool_get_thread_count(oc_ppu_jit_t* jit);

/**
 * Get number of currently active workers
 */
size_t oc_ppu_jit_pool_get_active_workers(oc_ppu_jit_t* jit);

/**
 * Get pending task count (enhanced pool)
 */
size_t oc_ppu_jit_pool_get_pending(oc_ppu_jit_t* jit);

/**
 * Get completed task count (enhanced pool)
 */
size_t oc_ppu_jit_pool_get_completed(oc_ppu_jit_t* jit);

/**
 * Get enhanced thread pool statistics
 */
void oc_ppu_jit_pool_get_stats(oc_ppu_jit_t* jit, uint64_t* total_submitted,
                                uint64_t* total_completed, uint64_t* total_failed,
                                uint64_t* peak_queue_size, uint64_t* avg_wait_ms,
                                uint64_t* avg_exec_ms);

/**
 * Reset thread pool statistics
 */
void oc_ppu_jit_pool_reset_stats(oc_ppu_jit_t* jit);

// PPU JIT Background Compilation APIs

/**
 * Enable or disable background compilation
 */
void oc_ppu_jit_bg_enable(oc_ppu_jit_t* jit, int enable);

/**
 * Check if background compilation is enabled
 */
int oc_ppu_jit_bg_is_enabled(oc_ppu_jit_t* jit);

/**
 * Set idle mode (for idle-time compilation)
 */
void oc_ppu_jit_bg_set_idle_mode(oc_ppu_jit_t* jit, int idle);

/**
 * Check if in idle mode
 */
int oc_ppu_jit_bg_is_idle(oc_ppu_jit_t* jit);

/**
 * Configure background compilation parameters
 * speculation_depth: How many blocks ahead to speculate
 * branch_priority: Priority boost for branch targets
 * hot_threshold: Execution count to consider "hot"
 * max_queue: Maximum speculative queue size
 */
void oc_ppu_jit_bg_configure(oc_ppu_jit_t* jit, uint32_t speculation_depth,
                              int branch_priority, int hot_threshold, size_t max_queue);

/**
 * Queue a block for speculative compilation
 * score: Base priority score (higher = more likely to be compiled first)
 * Returns: 1 if queued, 0 if not (already compiled/queued or disabled)
 */
int oc_ppu_jit_bg_queue_speculative(oc_ppu_jit_t* jit, uint32_t address,
                                     const uint8_t* code, size_t size, int score);

/**
 * Queue a branch target for precompilation (higher priority)
 * Returns: 1 if queued, 0 if not
 */
int oc_ppu_jit_bg_queue_branch_target(oc_ppu_jit_t* jit, uint32_t address,
                                       const uint8_t* code, size_t size);

/**
 * Check if an address has been background-compiled
 */
int oc_ppu_jit_bg_is_compiled(oc_ppu_jit_t* jit, uint32_t address);

/**
 * Check if an address is queued for background compilation
 */
int oc_ppu_jit_bg_is_queued(oc_ppu_jit_t* jit, uint32_t address);

/**
 * Mark an address as compiled (for external compilation tracking)
 */
void oc_ppu_jit_bg_mark_compiled(oc_ppu_jit_t* jit, uint32_t address);

/**
 * Process background compilation during idle time
 * max_count: Maximum number of blocks to compile
 * Returns: Number of blocks compiled
 */
size_t oc_ppu_jit_bg_process_idle(oc_ppu_jit_t* jit, size_t max_count);

/**
 * Record that a speculatively compiled block was executed (hit)
 */
void oc_ppu_jit_bg_record_hit(oc_ppu_jit_t* jit, uint32_t address);

/**
 * Get speculative queue size
 */
size_t oc_ppu_jit_bg_get_queue_size(oc_ppu_jit_t* jit);

/**
 * Get count of background-compiled blocks
 */
size_t oc_ppu_jit_bg_get_compiled_count(oc_ppu_jit_t* jit);

/**
 * Get background compilation statistics
 */
void oc_ppu_jit_bg_get_stats(oc_ppu_jit_t* jit, uint64_t* speculative_queued,
                              uint64_t* speculative_compiled, uint64_t* speculative_hits,
                              uint64_t* branch_targets_queued, uint64_t* branch_targets_compiled,
                              uint64_t* idle_compilations);

/**
 * Reset background compilation statistics
 */
void oc_ppu_jit_bg_reset_stats(oc_ppu_jit_t* jit);

/**
 * Clear all background compilation state
 */
void oc_ppu_jit_bg_clear(oc_ppu_jit_t* jit);

// ============================================================================
// PPU JIT Execution Context
// ============================================================================

/**
 * PPU execution context structure
 * 
 * This structure holds the complete PPU state and is passed to JIT-compiled
 * code for reading and writing registers. The compiled code operates on this
 * context directly, allowing seamless transition between interpreter and JIT.
 */
typedef struct oc_ppu_context_t {
    // General Purpose Registers (64-bit)
    uint64_t gpr[32];
    
    // Floating Point Registers (64-bit IEEE double)
    double fpr[32];
    
    // Vector Registers (128-bit, stored as 4 x uint32_t)
    uint32_t vr[32][4];
    
    // Condition Register (32-bit)
    uint32_t cr;
    
    // Link Register (64-bit)
    uint64_t lr;
    
    // Count Register (64-bit)
    uint64_t ctr;
    
    // Fixed-Point Exception Register (64-bit)
    uint64_t xer;
    
    // Floating-Point Status and Control Register (64-bit)
    uint64_t fpscr;
    
    // Vector Status and Control Register (32-bit)
    uint32_t vscr;
    
    // Program Counter / Current Instruction Address (64-bit)
    uint64_t pc;
    
    // Machine State Register (64-bit)
    uint64_t msr;
    
    // Next instruction address after block execution
    uint64_t next_pc;
    
    // Number of instructions executed in this block
    uint32_t instructions_executed;
    
    // Execution result/status
    // 0 = normal, 1 = branch, 2 = syscall, 3 = breakpoint, 4 = error
    int32_t exit_reason;
    
    // Memory base pointer (set before execution)
    void* memory_base;
    
    // Memory size (for bounds checking in debug builds)
    uint64_t memory_size;
} oc_ppu_context_t;

/**
 * Exit reason codes from JIT execution
 */
typedef enum {
    OC_PPU_EXIT_NORMAL = 0,      // Block completed normally
    OC_PPU_EXIT_BRANCH = 1,      // Block ended with branch
    OC_PPU_EXIT_SYSCALL = 2,     // System call encountered
    OC_PPU_EXIT_BREAKPOINT = 3,  // Breakpoint hit
    OC_PPU_EXIT_ERROR = 4        // Execution error
} oc_ppu_exit_reason_t;

/**
 * Execute JIT-compiled code with context
 * 
 * @param jit JIT compiler handle
 * @param context PPU context (registers read/written here)
 * @param address Start address of compiled block
 * @return Number of instructions executed, or negative on error
 */
int oc_ppu_jit_execute(oc_ppu_jit_t* jit, oc_ppu_context_t* context, uint32_t address);

/**
 * Execute a single JIT block (does not handle branches)
 */
int oc_ppu_jit_execute_block(oc_ppu_jit_t* jit, oc_ppu_context_t* context, uint32_t address);

// ============================================================================
// SPU JIT Compiler
// ============================================================================

/**
 * SPU JIT compiler handle
 */
typedef struct oc_spu_jit_t oc_spu_jit_t;

/**
 * Create SPU JIT compiler
 */
oc_spu_jit_t* oc_spu_jit_create(void);

/**
 * Destroy SPU JIT compiler
 */
void oc_spu_jit_destroy(oc_spu_jit_t* jit);

/**
 * Compile SPU function
 */
int oc_spu_jit_compile(oc_spu_jit_t* jit, uint32_t address, const uint8_t* code, size_t size);

/**
 * Get compiled code for address
 */
void* oc_spu_jit_get_compiled(oc_spu_jit_t* jit, uint32_t address);

/**
 * Invalidate compiled code at address
 */
void oc_spu_jit_invalidate(oc_spu_jit_t* jit, uint32_t address);

/**
 * Clear entire JIT cache
 */
void oc_spu_jit_clear_cache(oc_spu_jit_t* jit);

/**
 * Add breakpoint at address
 */
void oc_spu_jit_add_breakpoint(oc_spu_jit_t* jit, uint32_t address);

/**
 * Remove breakpoint at address
 */
void oc_spu_jit_remove_breakpoint(oc_spu_jit_t* jit, uint32_t address);

/**
 * Check if breakpoint exists at address
 */
int oc_spu_jit_has_breakpoint(oc_spu_jit_t* jit, uint32_t address);

// SPU JIT Channel Operations APIs

/**
 * Enable/disable channel operations in JIT
 */
void oc_spu_jit_enable_channel_ops(oc_spu_jit_t* jit, int enable);

/**
 * Check if channel operations are enabled
 */
int oc_spu_jit_is_channel_ops_enabled(oc_spu_jit_t* jit);

/**
 * Register a channel operation for JIT compilation
 */
void oc_spu_jit_register_channel_op(oc_spu_jit_t* jit, uint8_t channel,
                                     int is_read, uint32_t address, uint8_t reg);

/**
 * Set channel read/write callbacks
 */
void oc_spu_jit_set_channel_callbacks(oc_spu_jit_t* jit,
                                       void* read_callback,
                                       void* write_callback);

/**
 * Get number of registered channel operations
 */
size_t oc_spu_jit_get_channel_op_count(oc_spu_jit_t* jit);

// SPU JIT MFC DMA APIs

/**
 * Enable/disable MFC DMA in JIT
 */
void oc_spu_jit_enable_mfc_dma(oc_spu_jit_t* jit, int enable);

/**
 * Check if MFC DMA is enabled
 */
int oc_spu_jit_is_mfc_dma_enabled(oc_spu_jit_t* jit);

/**
 * Queue a DMA operation
 */
void oc_spu_jit_queue_dma(oc_spu_jit_t* jit, uint32_t local_addr, 
                           uint64_t ea, uint32_t size, uint16_t tag, uint8_t cmd);

/**
 * Get number of pending DMA operations
 */
size_t oc_spu_jit_get_pending_dma_count(oc_spu_jit_t* jit);

/**
 * Get number of pending DMA operations for a specific tag
 */
size_t oc_spu_jit_get_pending_dma_for_tag(oc_spu_jit_t* jit, uint16_t tag);

/**
 * Mark all DMA operations for a tag as complete
 */
void oc_spu_jit_complete_dma_tag(oc_spu_jit_t* jit, uint16_t tag);

/**
 * Set DMA transfer callback
 */
void oc_spu_jit_set_dma_callback(oc_spu_jit_t* jit, void* callback);

// SPU JIT Loop Optimization APIs

/**
 * Enable/disable loop optimization
 */
void oc_spu_jit_enable_loop_opt(oc_spu_jit_t* jit, int enable);

/**
 * Check if loop optimization is enabled
 */
int oc_spu_jit_is_loop_opt_enabled(oc_spu_jit_t* jit);

/**
 * Detect a loop structure
 */
void oc_spu_jit_detect_loop(oc_spu_jit_t* jit, uint32_t header, 
                             uint32_t back_edge, uint32_t exit);

/**
 * Set loop iteration count (for counted loops)
 */
void oc_spu_jit_set_loop_count(oc_spu_jit_t* jit, uint32_t header, 
                                uint32_t count);

/**
 * Set whether a loop is vectorizable
 */
void oc_spu_jit_set_loop_vectorizable(oc_spu_jit_t* jit, uint32_t header, 
                                       int vectorizable);

/**
 * Check if an address is inside a known loop
 */
int oc_spu_jit_is_in_loop(oc_spu_jit_t* jit, uint32_t address);

/**
 * Get loop information
 * Returns: 1 if loop found, 0 otherwise
 */
int oc_spu_jit_get_loop_info(oc_spu_jit_t* jit, uint32_t header,
                              uint32_t* back_edge, uint32_t* exit,
                              uint32_t* iteration_count, int* is_vectorizable);

// SPU JIT Loop Unrolling APIs

/**
 * Set the body size (instruction count) for a loop
 */
void oc_spu_jit_set_loop_body_size(oc_spu_jit_t* jit, uint32_t header, uint32_t size);

/**
 * Configure loop unrolling parameters
 * max_factor: Maximum unroll factor (default: 4)
 * max_body_size: Max body size (instructions) to consider unrolling (default: 16)
 * min_iterations: Minimum iterations to consider unrolling (default: 4)
 * vectorizable_only: Only unroll vectorizable loops if 1
 */
void oc_spu_jit_set_unroll_config(oc_spu_jit_t* jit, uint32_t max_factor,
                                   uint32_t max_body_size, uint32_t min_iterations,
                                   int vectorizable_only);

/**
 * Get loop unrolling configuration
 */
void oc_spu_jit_get_unroll_config(oc_spu_jit_t* jit, uint32_t* max_factor,
                                   uint32_t* max_body_size, uint32_t* min_iterations,
                                   int* vectorizable_only);

/**
 * Check if a loop can be unrolled
 * Returns: 1 if unrollable, 0 otherwise
 */
int oc_spu_jit_can_unroll_loop(oc_spu_jit_t* jit, uint32_t header);

/**
 * Perform loop unrolling analysis and mark as unrolled
 * Returns: The unroll factor used (1 = not unrolled)
 */
uint32_t oc_spu_jit_unroll_loop(oc_spu_jit_t* jit, uint32_t header);

/**
 * Get the unroll factor for a loop
 * Returns: The unroll factor (1 = not unrolled)
 */
uint32_t oc_spu_jit_get_unroll_factor(oc_spu_jit_t* jit, uint32_t header);

/**
 * Check if a loop has been unrolled
 * Returns: 1 if unrolled, 0 otherwise
 */
int oc_spu_jit_is_loop_unrolled(oc_spu_jit_t* jit, uint32_t header);

/**
 * Get total number of detected loops
 */
size_t oc_spu_jit_get_loop_count(oc_spu_jit_t* jit);

/**
 * Get loop optimization statistics
 */
void oc_spu_jit_get_loop_stats(oc_spu_jit_t* jit, uint64_t* loops_detected,
                                uint64_t* loops_with_count, uint64_t* loops_vectorizable,
                                uint64_t* loops_unrolled, uint64_t* rejected_size,
                                uint64_t* rejected_count);

/**
 * Reset loop optimization statistics
 */
void oc_spu_jit_reset_loop_stats(oc_spu_jit_t* jit);

/**
 * Clear all detected loops and statistics
 */
void oc_spu_jit_clear_loops(oc_spu_jit_t* jit);

// SPU JIT SIMD Intrinsics APIs

/**
 * Enable/disable SIMD intrinsics usage
 */
void oc_spu_jit_enable_simd_intrinsics(oc_spu_jit_t* jit, int enable);

/**
 * Check if SIMD intrinsics are enabled
 */
int oc_spu_jit_is_simd_intrinsics_enabled(oc_spu_jit_t* jit);

/**
 * Get SIMD intrinsic for an opcode
 * Returns: intrinsic ID or 0 if not mapped
 */
int oc_spu_jit_get_simd_intrinsic(oc_spu_jit_t* jit, uint32_t opcode);

/**
 * Check if opcode has a SIMD intrinsic mapping
 */
int oc_spu_jit_has_simd_intrinsic(oc_spu_jit_t* jit, uint32_t opcode);

// ============================================================================
// RSX Shader Compiler
// ============================================================================

/**
 * RSX shader compiler handle
 */
typedef struct oc_rsx_shader_t oc_rsx_shader_t;

/**
 * Create RSX shader compiler
 */
oc_rsx_shader_t* oc_rsx_shader_create(void);

/**
 * Destroy RSX shader compiler
 */
void oc_rsx_shader_destroy(oc_rsx_shader_t* shader);

/**
 * Compile RSX vertex program to SPIR-V
 * Returns: 0 on success, negative on error
 */
int oc_rsx_shader_compile_vertex(oc_rsx_shader_t* shader, const uint32_t* code,
                                  size_t size, uint32_t** out_spirv, size_t* out_size);

/**
 * Compile RSX fragment program to SPIR-V
 * Returns: 0 on success, negative on error
 */
int oc_rsx_shader_compile_fragment(oc_rsx_shader_t* shader, const uint32_t* code,
                                    size_t size, uint32_t** out_spirv, size_t* out_size);

/**
 * Free SPIR-V bytecode allocated by compilation
 */
void oc_rsx_shader_free_spirv(uint32_t* spirv);

// RSX Shader Linking APIs

/**
 * Link vertex and fragment shaders
 * Returns: 0 on success, negative on error
 */
int oc_rsx_shader_link(oc_rsx_shader_t* shader, 
                        const uint32_t* vs_spirv, size_t vs_size,
                        const uint32_t* fs_spirv, size_t fs_size);

/**
 * Get number of linked shader programs
 */
size_t oc_rsx_shader_get_linked_count(oc_rsx_shader_t* shader);

// RSX Pipeline Caching APIs

/**
 * Set pipeline creation/destruction callbacks
 */
void oc_rsx_shader_set_pipeline_callbacks(oc_rsx_shader_t* shader,
                                           void* create_callback,
                                           void* destroy_callback);

/**
 * Get or create a cached graphics pipeline
 */
void* oc_rsx_shader_get_pipeline(oc_rsx_shader_t* shader,
                                  uint64_t vs_hash, uint64_t fs_hash,
                                  uint32_t vertex_mask, uint8_t cull_mode,
                                  uint8_t blend_enable);

/**
 * Advance frame counter for LRU eviction
 */
void oc_rsx_shader_advance_frame(oc_rsx_shader_t* shader);

/**
 * Get number of cached pipelines
 */
size_t oc_rsx_shader_get_pipeline_count(oc_rsx_shader_t* shader);

// RSX Shader Cache Management APIs

/**
 * Clear all shader caches
 */
void oc_rsx_shader_clear_caches(oc_rsx_shader_t* shader);

/**
 * Get vertex shader cache count
 */
size_t oc_rsx_shader_get_vertex_cache_count(oc_rsx_shader_t* shader);

/**
 * Get fragment shader cache count
 */
size_t oc_rsx_shader_get_fragment_cache_count(oc_rsx_shader_t* shader);

// ============================================================================
// Atomics
// ============================================================================

/**
 * 128-bit atomic compare-and-swap
 */
int oc_atomic_cas128(void* ptr, oc_v128_t* expected, const oc_v128_t* desired);

/**
 * 128-bit atomic load
 */
void oc_atomic_load128(const void* ptr, oc_v128_t* result);

/**
 * 128-bit atomic store
 */
void oc_atomic_store128(void* ptr, const oc_v128_t* value);

#ifdef __cplusplus
}
#endif

#endif /* OC_FFI_H */
