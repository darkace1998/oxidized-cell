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
