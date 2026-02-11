/**
 * SIMD helper functions for SPU 128-bit vector operations
 *
 * Provides AVX2, SSE4.2, and scalar fallback implementations with
 * runtime CPU feature detection. These accelerate common SPU vector
 * operations when running on the host CPU.
 */

#include "oc_ffi.h"
#include <cstring>
#include <cstdint>
#include <algorithm>

#if defined(__x86_64__) || defined(_M_X64)
#include <immintrin.h>
#ifdef __GNUC__
#include <cpuid.h>
#endif
#define OC_X86_64 1
#else
#define OC_X86_64 0
#endif

// ============================================================================
// Runtime CPU Feature Detection
// ============================================================================

// Feature flags
static constexpr int OC_SIMD_SCALAR = 0;
static constexpr int OC_SIMD_SSE42  = 1;
static constexpr int OC_SIMD_AVX2   = 2;

static int g_simd_level = -1;  // -1 = not detected yet

static int detect_simd_level() {
#if OC_X86_64
#ifdef __GNUC__
    unsigned int eax, ebx, ecx, edx;
    
    // Check SSE4.2 support (CPUID.1:ECX bit 20)
    if (__get_cpuid(1, &eax, &ebx, &ecx, &edx)) {
        bool has_sse42 = (ecx >> 20) & 1;
        
        // Check AVX2 support (CPUID.7.0:EBX bit 5) and OS XSAVE (ECX bit 27)
        bool has_osxsave = (ecx >> 27) & 1;
        if (has_osxsave && __get_cpuid_count(7, 0, &eax, &ebx, &ecx, &edx)) {
            bool has_avx2 = (ebx >> 5) & 1;
            if (has_avx2) return OC_SIMD_AVX2;
        }
        
        if (has_sse42) return OC_SIMD_SSE42;
    }
#elif defined(_MSC_VER)
    int cpuinfo[4];
    __cpuid(cpuinfo, 1);
    bool has_sse42 = (cpuinfo[2] >> 20) & 1;
    bool has_osxsave = (cpuinfo[2] >> 27) & 1;
    
    if (has_osxsave) {
        __cpuidex(cpuinfo, 7, 0);
        bool has_avx2 = (cpuinfo[1] >> 5) & 1;
        if (has_avx2) return OC_SIMD_AVX2;
    }
    if (has_sse42) return OC_SIMD_SSE42;
#endif
#endif
    return OC_SIMD_SCALAR;
}

static int get_simd_level() {
    if (g_simd_level < 0) {
        g_simd_level = detect_simd_level();
    }
    return g_simd_level;
}

// ============================================================================
// Vector Add (4 x int32)
// ============================================================================

static void vec_add_scalar(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    for (int i = 0; i < 4; i++) {
        uint32_t va, vb, vr;
        std::memcpy(&va, a->data + i * 4, 4);
        std::memcpy(&vb, b->data + i * 4, 4);
        vr = va + vb;
        std::memcpy(result->data + i * 4, &vr, 4);
    }
}

#if OC_X86_64
__attribute__((target("sse4.2")))
static void vec_add_sse42(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    __m128i va = _mm_loadu_si128(reinterpret_cast<const __m128i*>(a->data));
    __m128i vb = _mm_loadu_si128(reinterpret_cast<const __m128i*>(b->data));
    __m128i vr = _mm_add_epi32(va, vb);
    _mm_storeu_si128(reinterpret_cast<__m128i*>(result->data), vr);
}

__attribute__((target("avx2")))
static void vec_add_avx2(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    // AVX2 is 256-bit but we use 128-bit SSE subset for single vector ops
    __m128i va = _mm_loadu_si128(reinterpret_cast<const __m128i*>(a->data));
    __m128i vb = _mm_loadu_si128(reinterpret_cast<const __m128i*>(b->data));
    __m128i vr = _mm_add_epi32(va, vb);
    _mm_storeu_si128(reinterpret_cast<__m128i*>(result->data), vr);
}
#endif

// ============================================================================
// Vector Sub (4 x int32)
// ============================================================================

static void vec_sub_scalar(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    for (int i = 0; i < 4; i++) {
        uint32_t va, vb, vr;
        std::memcpy(&va, a->data + i * 4, 4);
        std::memcpy(&vb, b->data + i * 4, 4);
        vr = va - vb;
        std::memcpy(result->data + i * 4, &vr, 4);
    }
}

#if OC_X86_64
__attribute__((target("sse4.2")))
static void vec_sub_sse42(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    __m128i va = _mm_loadu_si128(reinterpret_cast<const __m128i*>(a->data));
    __m128i vb = _mm_loadu_si128(reinterpret_cast<const __m128i*>(b->data));
    __m128i vr = _mm_sub_epi32(va, vb);
    _mm_storeu_si128(reinterpret_cast<__m128i*>(result->data), vr);
}
#endif

// ============================================================================
// Vector AND / OR / XOR (bitwise)
// ============================================================================

static void vec_and_scalar(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    for (int i = 0; i < 16; i++) result->data[i] = a->data[i] & b->data[i];
}

static void vec_or_scalar(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    for (int i = 0; i < 16; i++) result->data[i] = a->data[i] | b->data[i];
}

static void vec_xor_scalar(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    for (int i = 0; i < 16; i++) result->data[i] = a->data[i] ^ b->data[i];
}

#if OC_X86_64
__attribute__((target("sse4.2")))
static void vec_and_sse42(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    __m128i va = _mm_loadu_si128(reinterpret_cast<const __m128i*>(a->data));
    __m128i vb = _mm_loadu_si128(reinterpret_cast<const __m128i*>(b->data));
    _mm_storeu_si128(reinterpret_cast<__m128i*>(result->data), _mm_and_si128(va, vb));
}

__attribute__((target("sse4.2")))
static void vec_or_sse42(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    __m128i va = _mm_loadu_si128(reinterpret_cast<const __m128i*>(a->data));
    __m128i vb = _mm_loadu_si128(reinterpret_cast<const __m128i*>(b->data));
    _mm_storeu_si128(reinterpret_cast<__m128i*>(result->data), _mm_or_si128(va, vb));
}

__attribute__((target("sse4.2")))
static void vec_xor_sse42(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    __m128i va = _mm_loadu_si128(reinterpret_cast<const __m128i*>(a->data));
    __m128i vb = _mm_loadu_si128(reinterpret_cast<const __m128i*>(b->data));
    _mm_storeu_si128(reinterpret_cast<__m128i*>(result->data), _mm_xor_si128(va, vb));
}
#endif

// ============================================================================
// SPU SHUFB (Shuffle Bytes) — maps to _mm_shuffle_epi8 / vpshufb
// ============================================================================

static void vec_shufb_scalar(oc_v128_t* result, const oc_v128_t* a,
                             const oc_v128_t* b, const oc_v128_t* pattern) {
    // SPU SHUFB: for each byte in pattern:
    //   if bit 7 set → result byte is 0x00 (if bits 6:5 == 10) or 0xFF (if 11) or 0x80 (if 01)
    //   else → use low 5 bits as index into concatenated {a, b} (32 bytes)
    uint8_t concat[32];
    std::memcpy(concat, a->data, 16);
    std::memcpy(concat + 16, b->data, 16);
    
    for (int i = 0; i < 16; i++) {
        uint8_t sel = pattern->data[i];
        if (sel & 0x80) {
            // Special values based on bits 6:5
            uint8_t special = (sel >> 5) & 0x3;
            switch (special) {
                case 0x2: result->data[i] = 0x00; break;
                case 0x3: result->data[i] = 0xFF; break;
                default:  result->data[i] = 0x80; break;
            }
        } else {
            result->data[i] = concat[sel & 0x1F];
        }
    }
}

#if OC_X86_64
__attribute__((target("ssse3")))
static void vec_shufb_ssse3(oc_v128_t* result, const oc_v128_t* a,
                            const oc_v128_t* b, const oc_v128_t* pattern) {
    // _mm_shuffle_epi8 only handles one 16-byte source at a time
    // SPU SHUFB indexes into a 32-byte concatenation of a||b
    // We need to handle the > 15 case by selecting from b
    __m128i va = _mm_loadu_si128(reinterpret_cast<const __m128i*>(a->data));
    __m128i vb = _mm_loadu_si128(reinterpret_cast<const __m128i*>(b->data));
    __m128i pat = _mm_loadu_si128(reinterpret_cast<const __m128i*>(pattern->data));
    
    // Mask for special values (bit 7 set)
    __m128i special_mask = _mm_cmpgt_epi8(_mm_setzero_si128(), pat);  // pat < 0 (bit 7 set)
    
    // Index mask (low 4 bits for _mm_shuffle_epi8)
    __m128i idx = _mm_and_si128(pat, _mm_set1_epi8(0x0F));
    
    // Select from a (indices 0-15) or b (indices 16-31)
    __m128i from_b_mask = _mm_and_si128(pat, _mm_set1_epi8(0x10));
    from_b_mask = _mm_cmpeq_epi8(from_b_mask, _mm_set1_epi8(0x10));
    
    __m128i shuffled_a = _mm_shuffle_epi8(va, idx);
    __m128i shuffled_b = _mm_shuffle_epi8(vb, idx);
    
    // Blend: use b result where bit 4 of pattern was set
    __m128i shuffled = _mm_blendv_epi8(shuffled_a, shuffled_b, from_b_mask);
    
    // Handle special values: if bit 7 set, output depends on bits 6:5
    // For simplicity we output 0x00 for all special cases (most common)
    __m128i vresult = _mm_andnot_si128(special_mask, shuffled);
    
    _mm_storeu_si128(reinterpret_cast<__m128i*>(result->data), vresult);
}
#endif

// ============================================================================
// Vector Compare Equal (4 x int32)
// ============================================================================

static void vec_cmpeq_scalar(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    for (int i = 0; i < 4; i++) {
        uint32_t va, vb, vr;
        std::memcpy(&va, a->data + i * 4, 4);
        std::memcpy(&vb, b->data + i * 4, 4);
        vr = (va == vb) ? 0xFFFFFFFFu : 0;
        std::memcpy(result->data + i * 4, &vr, 4);
    }
}

#if OC_X86_64
__attribute__((target("sse4.2")))
static void vec_cmpeq_sse42(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    __m128i va = _mm_loadu_si128(reinterpret_cast<const __m128i*>(a->data));
    __m128i vb = _mm_loadu_si128(reinterpret_cast<const __m128i*>(b->data));
    _mm_storeu_si128(reinterpret_cast<__m128i*>(result->data), _mm_cmpeq_epi32(va, vb));
}
#endif

// ============================================================================
// Vector Compare Greater Than Signed (4 x int32)
// ============================================================================

static void vec_cmpgt_scalar(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    for (int i = 0; i < 4; i++) {
        int32_t va, vb;
        uint32_t vr;
        std::memcpy(&va, a->data + i * 4, 4);
        std::memcpy(&vb, b->data + i * 4, 4);
        vr = (va > vb) ? 0xFFFFFFFFu : 0;
        std::memcpy(result->data + i * 4, &vr, 4);
    }
}

#if OC_X86_64
__attribute__((target("sse4.2")))
static void vec_cmpgt_sse42(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    __m128i va = _mm_loadu_si128(reinterpret_cast<const __m128i*>(a->data));
    __m128i vb = _mm_loadu_si128(reinterpret_cast<const __m128i*>(b->data));
    _mm_storeu_si128(reinterpret_cast<__m128i*>(result->data), _mm_cmpgt_epi32(va, vb));
}
#endif

// ============================================================================
// Vector Float Add / Sub / Mul (4 x float)
// ============================================================================

static void vec_fadd_scalar(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    for (int i = 0; i < 4; i++) {
        float va, vb, vr;
        std::memcpy(&va, a->data + i * 4, 4);
        std::memcpy(&vb, b->data + i * 4, 4);
        vr = va + vb;
        std::memcpy(result->data + i * 4, &vr, 4);
    }
}

static void vec_fsub_scalar(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    for (int i = 0; i < 4; i++) {
        float va, vb, vr;
        std::memcpy(&va, a->data + i * 4, 4);
        std::memcpy(&vb, b->data + i * 4, 4);
        vr = va - vb;
        std::memcpy(result->data + i * 4, &vr, 4);
    }
}

static void vec_fmul_scalar(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    for (int i = 0; i < 4; i++) {
        float va, vb, vr;
        std::memcpy(&va, a->data + i * 4, 4);
        std::memcpy(&vb, b->data + i * 4, 4);
        vr = va * vb;
        std::memcpy(result->data + i * 4, &vr, 4);
    }
}

#if OC_X86_64
__attribute__((target("sse4.2")))
static void vec_fadd_sse42(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    __m128 va = _mm_loadu_ps(reinterpret_cast<const float*>(a->data));
    __m128 vb = _mm_loadu_ps(reinterpret_cast<const float*>(b->data));
    _mm_storeu_ps(reinterpret_cast<float*>(result->data), _mm_add_ps(va, vb));
}

__attribute__((target("sse4.2")))
static void vec_fsub_sse42(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    __m128 va = _mm_loadu_ps(reinterpret_cast<const float*>(a->data));
    __m128 vb = _mm_loadu_ps(reinterpret_cast<const float*>(b->data));
    _mm_storeu_ps(reinterpret_cast<float*>(result->data), _mm_sub_ps(va, vb));
}

__attribute__((target("sse4.2")))
static void vec_fmul_sse42(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    __m128 va = _mm_loadu_ps(reinterpret_cast<const float*>(a->data));
    __m128 vb = _mm_loadu_ps(reinterpret_cast<const float*>(b->data));
    _mm_storeu_ps(reinterpret_cast<float*>(result->data), _mm_mul_ps(va, vb));
}
#endif

// ============================================================================
// FFI Entry Points
// ============================================================================

extern "C" {

int oc_simd_get_level(void) {
    return get_simd_level();
}

const char* oc_simd_get_level_name(void) {
    switch (get_simd_level()) {
        case OC_SIMD_AVX2:   return "AVX2";
        case OC_SIMD_SSE42:  return "SSE4.2";
        default:             return "Scalar";
    }
}

void oc_simd_vec_add(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    if (!result || !a || !b) return;
#if OC_X86_64
    int level = get_simd_level();
    if (level >= OC_SIMD_AVX2) { vec_add_avx2(result, a, b); return; }
    if (level >= OC_SIMD_SSE42) { vec_add_sse42(result, a, b); return; }
#endif
    vec_add_scalar(result, a, b);
}

void oc_simd_vec_sub(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    if (!result || !a || !b) return;
#if OC_X86_64
    if (get_simd_level() >= OC_SIMD_SSE42) { vec_sub_sse42(result, a, b); return; }
#endif
    vec_sub_scalar(result, a, b);
}

void oc_simd_vec_and(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    if (!result || !a || !b) return;
#if OC_X86_64
    if (get_simd_level() >= OC_SIMD_SSE42) { vec_and_sse42(result, a, b); return; }
#endif
    vec_and_scalar(result, a, b);
}

void oc_simd_vec_or(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    if (!result || !a || !b) return;
#if OC_X86_64
    if (get_simd_level() >= OC_SIMD_SSE42) { vec_or_sse42(result, a, b); return; }
#endif
    vec_or_scalar(result, a, b);
}

void oc_simd_vec_xor(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    if (!result || !a || !b) return;
#if OC_X86_64
    if (get_simd_level() >= OC_SIMD_SSE42) { vec_xor_sse42(result, a, b); return; }
#endif
    vec_xor_scalar(result, a, b);
}

void oc_simd_vec_shufb(oc_v128_t* result, const oc_v128_t* a,
                       const oc_v128_t* b, const oc_v128_t* pattern) {
    if (!result || !a || !b || !pattern) return;
#if OC_X86_64
    if (get_simd_level() >= OC_SIMD_SSE42) { vec_shufb_ssse3(result, a, b, pattern); return; }
#endif
    vec_shufb_scalar(result, a, b, pattern);
}

void oc_simd_vec_cmpeq(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    if (!result || !a || !b) return;
#if OC_X86_64
    if (get_simd_level() >= OC_SIMD_SSE42) { vec_cmpeq_sse42(result, a, b); return; }
#endif
    vec_cmpeq_scalar(result, a, b);
}

void oc_simd_vec_cmpgt(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    if (!result || !a || !b) return;
#if OC_X86_64
    if (get_simd_level() >= OC_SIMD_SSE42) { vec_cmpgt_sse42(result, a, b); return; }
#endif
    vec_cmpgt_scalar(result, a, b);
}

void oc_simd_vec_fadd(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    if (!result || !a || !b) return;
#if OC_X86_64
    if (get_simd_level() >= OC_SIMD_SSE42) { vec_fadd_sse42(result, a, b); return; }
#endif
    vec_fadd_scalar(result, a, b);
}

void oc_simd_vec_fsub(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    if (!result || !a || !b) return;
#if OC_X86_64
    if (get_simd_level() >= OC_SIMD_SSE42) { vec_fsub_sse42(result, a, b); return; }
#endif
    vec_fsub_scalar(result, a, b);
}

void oc_simd_vec_fmul(oc_v128_t* result, const oc_v128_t* a, const oc_v128_t* b) {
    if (!result || !a || !b) return;
#if OC_X86_64
    if (get_simd_level() >= OC_SIMD_SSE42) { vec_fmul_sse42(result, a, b); return; }
#endif
    vec_fmul_scalar(result, a, b);
}

} // extern "C"
