//! Hardware SIMD and Assembly acceleration primitives for multi-dimensional convolution.
//! Provides native ARM64 inline assembly & NEON, x86_64 AVX2/SSE, and auto-vectorized fallbacks.

/// Elementwise vector addition of two slices: `dst[i] = src0[i] + src1[i]`.
#[inline(always)]
pub fn add2(dst: &mut [u16], src0: &[u16], src1: &[u16]) {
    debug_assert_eq!(dst.len(), src0.len());
    debug_assert_eq!(dst.len(), src1.len());
    let len = dst.len();

    #[cfg(target_arch = "aarch64")]
    unsafe {
        add2_aarch64(dst.as_mut_ptr(), src0.as_ptr(), src1.as_ptr(), len);
        return;
    }

    #[cfg(target_arch = "x86_64")]
    unsafe {
        if is_x86_feature_detected!("avx2") {
            add2_avx2(dst.as_mut_ptr(), src0.as_ptr(), src1.as_ptr(), len);
            return;
        }
    }

    #[allow(unreachable_code)]
    add2_fallback(dst, src0, src1);
}

/// Elementwise vector addition of three slices: `dst[i] = src0[i] + src1[i] + src2[i]`.
#[inline(always)]
pub fn add3(dst: &mut [u16], src0: &[u16], src1: &[u16], src2: &[u16]) {
    debug_assert_eq!(dst.len(), src0.len());
    debug_assert_eq!(dst.len(), src1.len());
    debug_assert_eq!(dst.len(), src2.len());
    let len = dst.len();

    #[cfg(target_arch = "aarch64")]
    unsafe {
        add3_aarch64(
            dst.as_mut_ptr(),
            src0.as_ptr(),
            src1.as_ptr(),
            src2.as_ptr(),
            len,
        );
        return;
    }

    #[cfg(target_arch = "x86_64")]
    unsafe {
        if is_x86_feature_detected!("avx2") {
            add3_avx2(
                dst.as_mut_ptr(),
                src0.as_ptr(),
                src1.as_ptr(),
                src2.as_ptr(),
                len,
            );
            return;
        }
    }

    #[allow(unreachable_code)]
    add3_fallback(dst, src0, src1, src2);
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn add2_aarch64(dst: *mut u16, src0: *const u16, src1: *const u16, len: usize) {
    use std::arch::aarch64::*;

    // Fast-path branchless specialization for len == 6 (standard size for 6D GoL)
    if len == 6 {
        let a = vld1_u16(src0);
        let b = vld1_u16(src1);
        vst1_u16(dst, vadd_u16(a, b));
        *dst.add(4) = *src0.add(4) + *src1.add(4);
        *dst.add(5) = *src0.add(5) + *src1.add(5);
        return;
    }

    let mut ptr_d = dst;
    let mut ptr0 = src0;
    let mut ptr1 = src1;
    let mut rem = len;

    // 64-element (128-byte) unrolled vector loop using ARM64 inline assembly
    if rem >= 64 {
        let blocks = rem / 64;
        rem %= 64;
        std::arch::asm!(
            "2:",
            "prfm pldl1keep, [{src0}, #256]",
            "prfm pldl1keep, [{src1}, #256]",
            "ld1 {{v0.8h, v1.8h, v2.8h, v3.8h}}, [{src0}], #64",
            "ld1 {{v4.8h, v5.8h, v6.8h, v7.8h}}, [{src1}], #64",
            "ld1 {{v16.8h, v17.8h, v18.8h, v19.8h}}, [{src0}], #64",
            "ld1 {{v20.8h, v21.8h, v22.8h, v23.8h}}, [{src1}], #64",
            "add v0.8h, v0.8h, v4.8h",
            "add v1.8h, v1.8h, v5.8h",
            "add v2.8h, v2.8h, v6.8h",
            "add v3.8h, v3.8h, v7.8h",
            "add v16.8h, v16.8h, v20.8h",
            "add v17.8h, v17.8h, v21.8h",
            "add v18.8h, v18.8h, v22.8h",
            "add v19.8h, v19.8h, v23.8h",
            "st1 {{v0.8h, v1.8h, v2.8h, v3.8h}}, [{dst}], #64",
            "st1 {{v16.8h, v17.8h, v18.8h, v19.8h}}, [{dst}], #64",
            "subs {count}, {count}, #1",
            "b.ne 2b",
            src0 = inout(reg) ptr0,
            src1 = inout(reg) ptr1,
            dst = inout(reg) ptr_d,
            count = inout(reg) blocks => _,
            out("v0") _, out("v1") _, out("v2") _, out("v3") _,
            out("v4") _, out("v5") _, out("v6") _, out("v7") _,
            out("v16") _, out("v17") _, out("v18") _, out("v19") _,
            out("v20") _, out("v21") _, out("v22") _, out("v23") _,
            options(nostack)
        );
    }

    while rem >= 16 {
        let a0 = vld1q_u16(ptr0);
        let a1 = vld1q_u16(ptr0.add(8));
        let b0 = vld1q_u16(ptr1);
        let b1 = vld1q_u16(ptr1.add(8));
        vst1q_u16(ptr_d, vaddq_u16(a0, b0));
        vst1q_u16(ptr_d.add(8), vaddq_u16(a1, b1));
        ptr0 = ptr0.add(16);
        ptr1 = ptr1.add(16);
        ptr_d = ptr_d.add(16);
        rem -= 16;
    }
    while rem >= 8 {
        let a = vld1q_u16(ptr0);
        let b = vld1q_u16(ptr1);
        vst1q_u16(ptr_d, vaddq_u16(a, b));
        ptr0 = ptr0.add(8);
        ptr1 = ptr1.add(8);
        ptr_d = ptr_d.add(8);
        rem -= 8;
    }
    while rem > 0 {
        *ptr_d = *ptr0 + *ptr1;
        ptr0 = ptr0.add(1);
        ptr1 = ptr1.add(1);
        ptr_d = ptr_d.add(1);
        rem -= 1;
    }
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn add3_aarch64(
    dst: *mut u16,
    src0: *const u16,
    src1: *const u16,
    src2: *const u16,
    len: usize,
) {
    use std::arch::aarch64::*;

    // Fast-path branchless specialization for len == 6
    if len == 6 {
        let a = vld1_u16(src0);
        let b = vld1_u16(src1);
        let c = vld1_u16(src2);
        vst1_u16(dst, vadd_u16(vadd_u16(a, b), c));
        *dst.add(4) = *src0.add(4) + *src1.add(4) + *src2.add(4);
        *dst.add(5) = *src0.add(5) + *src1.add(5) + *src2.add(5);
        return;
    }

    let mut ptr_d = dst;
    let mut ptr0 = src0;
    let mut ptr1 = src1;
    let mut ptr2 = src2;
    let mut rem = len;

    // 64-element (128-byte) unrolled 3-input addition via ARM64 inline assembly
    if rem >= 64 {
        let blocks = rem / 64;
        rem %= 64;
        std::arch::asm!(
            "3:",
            "prfm pldl1keep, [{src0}, #256]",
            "prfm pldl1keep, [{src1}, #256]",
            "prfm pldl1keep, [{src2}, #256]",
            "ld1 {{v0.8h, v1.8h, v2.8h, v3.8h}}, [{src0}], #64",
            "ld1 {{v4.8h, v5.8h, v6.8h, v7.8h}}, [{src1}], #64",
            "ld1 {{v8.8h, v9.8h, v10.8h, v11.8h}}, [{src2}], #64",
            "add v0.8h, v0.8h, v4.8h",
            "add v1.8h, v1.8h, v5.8h",
            "add v2.8h, v2.8h, v6.8h",
            "add v3.8h, v3.8h, v7.8h",
            "add v0.8h, v0.8h, v8.8h",
            "add v1.8h, v1.8h, v9.8h",
            "add v2.8h, v2.8h, v10.8h",
            "add v3.8h, v3.8h, v11.8h",
            "st1 {{v0.8h, v1.8h, v2.8h, v3.8h}}, [{dst}], #64",
            "ld1 {{v16.8h, v17.8h, v18.8h, v19.8h}}, [{src0}], #64",
            "ld1 {{v20.8h, v21.8h, v22.8h, v23.8h}}, [{src1}], #64",
            "ld1 {{v24.8h, v25.8h, v26.8h, v27.8h}}, [{src2}], #64",
            "add v16.8h, v16.8h, v20.8h",
            "add v17.8h, v17.8h, v21.8h",
            "add v18.8h, v18.8h, v22.8h",
            "add v19.8h, v19.8h, v23.8h",
            "add v16.8h, v16.8h, v24.8h",
            "add v17.8h, v17.8h, v25.8h",
            "add v18.8h, v18.8h, v26.8h",
            "add v19.8h, v19.8h, v27.8h",
            "st1 {{v16.8h, v17.8h, v18.8h, v19.8h}}, [{dst}], #64",
            "subs {count}, {count}, #1",
            "b.ne 3b",
            src0 = inout(reg) ptr0,
            src1 = inout(reg) ptr1,
            src2 = inout(reg) ptr2,
            dst = inout(reg) ptr_d,
            count = inout(reg) blocks => _,
            out("v0") _, out("v1") _, out("v2") _, out("v3") _,
            out("v4") _, out("v5") _, out("v6") _, out("v7") _,
            out("v8") _, out("v9") _, out("v10") _, out("v11") _,
            out("v16") _, out("v17") _, out("v18") _, out("v19") _,
            out("v20") _, out("v21") _, out("v22") _, out("v23") _,
            out("v24") _, out("v25") _, out("v26") _, out("v27") _,
            options(nostack)
        );
    }

    while rem >= 16 {
        let a0 = vld1q_u16(ptr0);
        let a1 = vld1q_u16(ptr0.add(8));
        let b0 = vld1q_u16(ptr1);
        let b1 = vld1q_u16(ptr1.add(8));
        let c0 = vld1q_u16(ptr2);
        let c1 = vld1q_u16(ptr2.add(8));
        vst1q_u16(ptr_d, vaddq_u16(vaddq_u16(a0, b0), c0));
        vst1q_u16(ptr_d.add(8), vaddq_u16(vaddq_u16(a1, b1), c1));
        ptr0 = ptr0.add(16);
        ptr1 = ptr1.add(16);
        ptr2 = ptr2.add(16);
        ptr_d = ptr_d.add(16);
        rem -= 16;
    }
    while rem >= 8 {
        let a = vld1q_u16(ptr0);
        let b = vld1q_u16(ptr1);
        let c = vld1q_u16(ptr2);
        vst1q_u16(ptr_d, vaddq_u16(vaddq_u16(a, b), c));
        ptr0 = ptr0.add(8);
        ptr1 = ptr1.add(8);
        ptr2 = ptr2.add(8);
        ptr_d = ptr_d.add(8);
        rem -= 8;
    }
    while rem > 0 {
        *ptr_d = *ptr0 + *ptr1 + *ptr2;
        ptr0 = ptr0.add(1);
        ptr1 = ptr1.add(1);
        ptr2 = ptr2.add(1);
        ptr_d = ptr_d.add(1);
        rem -= 1;
    }
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
unsafe fn add2_avx2(dst: *mut u16, src0: *const u16, src1: *const u16, len: usize) {
    use std::arch::x86_64::*;
    let mut i = 0;
    while i + 64 <= len {
        let a0 = _mm256_loadu_si256(src0.add(i) as *const __m256i);
        let a1 = _mm256_loadu_si256(src0.add(i + 16) as *const __m256i);
        let a2 = _mm256_loadu_si256(src0.add(i + 32) as *const __m256i);
        let a3 = _mm256_loadu_si256(src0.add(i + 48) as *const __m256i);

        let b0 = _mm256_loadu_si256(src1.add(i) as *const __m256i);
        let b1 = _mm256_loadu_si256(src1.add(i + 16) as *const __m256i);
        let b2 = _mm256_loadu_si256(src1.add(i + 32) as *const __m256i);
        let b3 = _mm256_loadu_si256(src1.add(i + 48) as *const __m256i);

        _mm256_storeu_si256(dst.add(i) as *mut __m256i, _mm256_add_epi16(a0, b0));
        _mm256_storeu_si256(dst.add(i + 16) as *mut __m256i, _mm256_add_epi16(a1, b1));
        _mm256_storeu_si256(dst.add(i + 32) as *mut __m256i, _mm256_add_epi16(a2, b2));
        _mm256_storeu_si256(dst.add(i + 48) as *mut __m256i, _mm256_add_epi16(a3, b3));
        i += 64;
    }
    while i + 16 <= len {
        let a = _mm256_loadu_si256(src0.add(i) as *const __m256i);
        let b = _mm256_loadu_si256(src1.add(i) as *const __m256i);
        _mm256_storeu_si256(dst.add(i) as *mut __m256i, _mm256_add_epi16(a, b));
        i += 16;
    }
    while i < len {
        *dst.add(i) = *src0.add(i) + *src1.add(i);
        i += 1;
    }
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
unsafe fn add3_avx2(
    dst: *mut u16,
    src0: *const u16,
    src1: *const u16,
    src2: *const u16,
    len: usize,
) {
    use std::arch::x86_64::*;
    let mut i = 0;
    while i + 64 <= len {
        let a0 = _mm256_loadu_si256(src0.add(i) as *const __m256i);
        let a1 = _mm256_loadu_si256(src0.add(i + 16) as *const __m256i);
        let a2 = _mm256_loadu_si256(src0.add(i + 32) as *const __m256i);
        let a3 = _mm256_loadu_si256(src0.add(i + 48) as *const __m256i);

        let b0 = _mm256_loadu_si256(src1.add(i) as *const __m256i);
        let b1 = _mm256_loadu_si256(src1.add(i + 16) as *const __m256i);
        let b2 = _mm256_loadu_si256(src1.add(i + 32) as *const __m256i);
        let b3 = _mm256_loadu_si256(src1.add(i + 48) as *const __m256i);

        let c0 = _mm256_loadu_si256(src2.add(i) as *const __m256i);
        let c1 = _mm256_loadu_si256(src2.add(i + 16) as *const __m256i);
        let c2 = _mm256_loadu_si256(src2.add(i + 32) as *const __m256i);
        let c3 = _mm256_loadu_si256(src2.add(i + 48) as *const __m256i);

        _mm256_storeu_si256(
            dst.add(i) as *mut __m256i,
            _mm256_add_epi16(_mm256_add_epi16(a0, b0), c0),
        );
        _mm256_storeu_si256(
            dst.add(i + 16) as *mut __m256i,
            _mm256_add_epi16(_mm256_add_epi16(a1, b1), c1),
        );
        _mm256_storeu_si256(
            dst.add(i + 32) as *mut __m256i,
            _mm256_add_epi16(_mm256_add_epi16(a2, b2), c2),
        );
        _mm256_storeu_si256(
            dst.add(i + 48) as *mut __m256i,
            _mm256_add_epi16(_mm256_add_epi16(a3, b3), c3),
        );
        i += 64;
    }
    while i + 16 <= len {
        let a = _mm256_loadu_si256(src0.add(i) as *const __m256i);
        let b = _mm256_loadu_si256(src1.add(i) as *const __m256i);
        let c = _mm256_loadu_si256(src2.add(i) as *const __m256i);
        _mm256_storeu_si256(
            dst.add(i) as *mut __m256i,
            _mm256_add_epi16(_mm256_add_epi16(a, b), c),
        );
        i += 16;
    }
    while i < len {
        *dst.add(i) = *src0.add(i) + *src1.add(i) + *src2.add(i);
        i += 1;
    }
}

#[inline(always)]
fn add2_fallback(dst: &mut [u16], src0: &[u16], src1: &[u16]) {
    let len = dst.len();
    for i in 0..len {
        dst[i] = src0[i] + src1[i];
    }
}

#[inline(always)]
fn add3_fallback(dst: &mut [u16], src0: &[u16], src1: &[u16], src2: &[u16]) {
    let len = dst.len();
    for i in 0..len {
        dst[i] = src0[i] + src1[i] + src2[i];
    }
}

/// Computes alive cells count using SIMD hardware acceleration & assembly.
#[inline(always)]
pub fn count_alive_simd(data: &[u8]) -> usize {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        return count_alive_aarch64_asm(data);
    }

    #[cfg(target_arch = "x86_64")]
    unsafe {
        if is_x86_feature_detected!("avx2") {
            return count_alive_avx2(data);
        }
    }

    #[allow(unreachable_code)]
    count_alive_popcnt(data)
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn count_alive_aarch64_asm(data: &[u8]) -> usize {
    let len = data.len();
    let mut ptr = data.as_ptr();
    let mut total = 0usize;

    // Process blocks of 64 bytes using pure ARM64 assembly with pairwise vector addition
    let mut blocks = len / 64;
    while blocks > 0 {
        // Limit chunk to 16384 blocks to prevent 16-bit lane saturation
        let chunk_blocks = blocks.min(16384);
        blocks -= chunk_blocks;
        let mut sum_scalar: u64 = 0;
        std::arch::asm!(
            "movi v16.8h, #0",
            "movi v17.8h, #0",
            "movi v18.8h, #0",
            "movi v19.8h, #0",
            "1:",
            "prfm pldl1keep, [{ptr}, #256]",
            "ld1 {{v0.16b, v1.16b, v2.16b, v3.16b}}, [{ptr}], #64",
            "uaddlp v4.8h, v0.16b",
            "uaddlp v5.8h, v1.16b",
            "uaddlp v6.8h, v2.16b",
            "uaddlp v7.8h, v3.16b",
            "add v16.8h, v16.8h, v4.8h",
            "add v17.8h, v17.8h, v5.8h",
            "add v18.8h, v18.8h, v6.8h",
            "add v19.8h, v19.8h, v7.8h",
            "subs {count}, {count}, #1",
            "b.ne 1b",
            "add v16.8h, v16.8h, v17.8h",
            "add v18.8h, v18.8h, v19.8h",
            "add v16.8h, v16.8h, v18.8h",
            "uaddlp v20.4s, v16.8h",
            "addv s20, v20.4s",
            "fmov {out:w}, s20",
            ptr = inout(reg) ptr,
            count = inout(reg) chunk_blocks => _,
            out = out(reg) sum_scalar,
            out("v0") _, out("v1") _, out("v2") _, out("v3") _,
            out("v4") _, out("v5") _, out("v6") _, out("v7") _,
            out("v16") _, out("v17") _, out("v18") _, out("v19") _,
            out("v20") _,
            options(nostack)
        );
        total += sum_scalar as usize;
    }

    let mut rem = len % 64;
    while rem >= 16 {
        use std::arch::aarch64::*;
        let v = vld1q_u8(ptr);
        total += vaddlvq_u8(v) as usize;
        ptr = ptr.add(16);
        rem -= 16;
    }
    while rem > 0 {
        total += *ptr as usize;
        ptr = ptr.add(1);
        rem -= 1;
    }
    total
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
unsafe fn count_alive_avx2(data: &[u8]) -> usize {
    use std::arch::x86_64::*;
    let mut total = 0usize;
    let len = data.len();
    let mut i = 0;
    let ptr = data.as_ptr();
    let zero = _mm256_setzero_si256();
    let mut acc = _mm256_setzero_si256();

    while i + 128 <= len {
        let v0 = _mm256_loadu_si256(ptr.add(i) as *const __m256i);
        let v1 = _mm256_loadu_si256(ptr.add(i + 32) as *const __m256i);
        let v2 = _mm256_loadu_si256(ptr.add(i + 64) as *const __m256i);
        let v3 = _mm256_loadu_si256(ptr.add(i + 96) as *const __m256i);

        let sad0 = _mm256_sad_epu8(v0, zero);
        let sad1 = _mm256_sad_epu8(v1, zero);
        let sad2 = _mm256_sad_epu8(v2, zero);
        let sad3 = _mm256_sad_epu8(v3, zero);

        let s01 = _mm256_add_epi64(sad0, sad1);
        let s23 = _mm256_add_epi64(sad2, sad3);
        acc = _mm256_add_epi64(acc, _mm256_add_epi64(s01, s23));
        i += 128;
    }

    let low = _mm256_castsi256_si128(acc);
    let high = _mm256_extracti128_si256(acc, 1);
    let sum_128 = _mm_add_epi64(low, high);
    total += (_mm_cvtsi128_si64(sum_128) as usize) + (_mm_extract_epi64(sum_128, 1) as usize);

    while i + 32 <= len {
        let v = _mm256_loadu_si256(ptr.add(i) as *const __m256i);
        let sad = _mm256_sad_epu8(v, zero);
        let low = _mm256_castsi256_si128(sad);
        let high = _mm256_extracti128_si256(sad, 1);
        let sum_128 = _mm_add_epi64(low, high);
        total += (_mm_cvtsi128_si64(sum_128) as usize) + (_mm_extract_epi64(sum_128, 1) as usize);
        i += 32;
    }
    while i < len {
        total += *ptr.add(i) as usize;
        i += 1;
    }
    total
}

#[inline(always)]
fn count_alive_popcnt(data: &[u8]) -> usize {
    let mut total = 0usize;
    let chunks = data.chunks_exact(8);
    let rem = chunks.remainder();
    for chunk in chunks {
        let val = u64::from_ne_bytes(chunk.try_into().unwrap());
        total += val.count_ones() as usize;
    }
    for &b in rem {
        total += b as usize;
    }
    total
}
