/**
 * 128-bit atomic operations
 *
 * On x86_64, uses native cmpxchg16b / movdqa for true 128-bit atomicity.
 * On other platforms, falls back to a global mutex to guarantee atomicity.
 */

#include "oc_ffi.h"
#include <cstring>
#include <mutex>

#ifdef __x86_64__
#include <immintrin.h>
#endif

// Global mutex for non-x86_64 platforms to protect 128-bit operations
#if !defined(__x86_64__) || !defined(__GNUC__)
static std::mutex g_atomic128_mutex;
#endif

extern "C" {

int oc_atomic_cas128(void* ptr, oc_v128_t* expected, const oc_v128_t* desired) {
    if (!ptr || !expected || !desired) return 0;
#if defined(__x86_64__) && defined(__GNUC__)
    // Use cmpxchg16b on x86-64
    unsigned char result;
    __asm__ __volatile__ (
        "lock cmpxchg16b %1"
        : "=@ccz" (result), "+m" (*(volatile __int128*)ptr),
          "+a" (((uint64_t*)expected)[0]), "+d" (((uint64_t*)expected)[1])
        : "b" (((const uint64_t*)desired)[0]), "c" (((const uint64_t*)desired)[1])
        : "memory"
    );
    return result;
#else
    // Mutex-guarded CAS for non-x86_64 platforms
    std::lock_guard<std::mutex> lock(g_atomic128_mutex);
    if (std::memcmp(ptr, expected, 16) == 0) {
        std::memcpy(ptr, desired, 16);
        return 1;
    }
    std::memcpy(expected, ptr, 16);
    return 0;
#endif
}

void oc_atomic_load128(const void* ptr, oc_v128_t* result) {
    if (!ptr || !result) return;
#if defined(__x86_64__) && defined(__GNUC__)
    __asm__ __volatile__ (
        "movdqa %1, %%xmm0\n\t"
        "movdqa %%xmm0, %0"
        : "=m" (*result)
        : "m" (*(const volatile oc_v128_t*)ptr)
        : "xmm0", "memory"
    );
#else
    std::lock_guard<std::mutex> lock(g_atomic128_mutex);
    std::memcpy(result, ptr, 16);
#endif
}

void oc_atomic_store128(void* ptr, const oc_v128_t* value) {
    if (!ptr || !value) return;
#if defined(__x86_64__) && defined(__GNUC__)
    __asm__ __volatile__ (
        "movdqa %1, %%xmm0\n\t"
        "movdqa %%xmm0, %0"
        : "=m" (*(volatile oc_v128_t*)ptr)
        : "m" (*value)
        : "xmm0", "memory"
    );
#else
    std::lock_guard<std::mutex> lock(g_atomic128_mutex);
    std::memcpy(ptr, value, 16);
#endif
}

} // extern "C"
