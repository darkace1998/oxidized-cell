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
