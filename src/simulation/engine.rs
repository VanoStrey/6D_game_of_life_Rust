//! Ultra-high-performance multi-dimensional cellular automaton engine.
//!
//! Replaces $O(3^D \times N)$ neighbor searching with separable $O(D \times N)$
//! 1D box convolutions using hardware SIMD vector intrinsics (ARM NEON & x86 AVX2)
//! and assembly acceleration.

use super::coords::GridDimensions;
use super::rules::Rules;
use super::simd_ops::{add2, add3};
use rayon::prelude::*;

/// Maximum cell count to precompute full flat threshold lookup arrays (16M cells = 32 MB).
pub const PRECOMPUTE_MAX_CELLS: usize = 16_777_216;

/// Precomputed metadata and reusable scratch buffers for separable simulation.
#[derive(Debug, Clone)]
pub struct SimulationAux {
    pub buf_a: Vec<u16>,
    pub buf_b: Vec<u16>,
    pub count_all: Vec<u16>,
    pub has_dropped_mask: Vec<u8>,
    pub thresh_min: Vec<u16>,
    pub thresh_max: Vec<u16>,
    pub drop_offset: usize,
    pub rule_min_table: Vec<i16>,
    pub rule_max_table: Vec<i16>,
}

impl SimulationAux {
    /// Builds precomputed tables and allocates ping-pong buffers for given dimensions and rules.
    pub fn new(dims: &GridDimensions, rules: &Rules) -> Self {
        let total = dims.total_cells;
        let buf_a = vec![0u16; total];
        let buf_b = vec![0u16; total];
        let d = dims.dimensions;
        let s = dims.size_in_dimensions;
        let strides = dims.strides;

        let mut drop_offset = 0usize;
        for k in 0..d {
            drop_offset += strides[k];
        }

        let len_dim = |coord: usize, size: usize| -> usize {
            if size == 1 {
                1
            } else if coord == 0 || coord == size - 1 {
                2
            } else {
                3
            }
        };

        let precompute = total <= PRECOMPUTE_MAX_CELLS;
        let mut count_all = if precompute { vec![0u16; total] } else { Vec::new() };
        let mut has_dropped_mask = if precompute { vec![0u8; total] } else { Vec::new() };

        let mut max_ca = 729usize;

        if precompute {
            for idx in 0..total {
                let mut total_vol = 1usize;
                let mut drop_in_bounds = true;

                for k in 0..d {
                    let coord_k = (idx / strides[k]) % s[k];
                    if coord_k == 0 {
                        drop_in_bounds = false;
                    }
                    total_vol = total_vol.saturating_mul(len_dim(coord_k, s[k]));
                }

                let ca = if drop_in_bounds {
                    total_vol.saturating_sub(1)
                } else {
                    total_vol
                };

                count_all[idx] = (ca.min(u16::MAX as usize)) as u16;
                has_dropped_mask[idx] = if drop_in_bounds { 0xFF } else { 0x00 };
                max_ca = max_ca.max(ca);
            }
        }

        let table_len = (max_ca + 1).max(730);
        let mut rule_min_table = vec![0i16; table_len];
        let mut rule_max_table = vec![0i16; table_len];
        for k in 0..table_len {
            rule_min_table[k] = rules.min_neighbors(k as i32) as i16;
            rule_max_table[k] = rules.max_neighbors(k as i32) as i16;
        }

        let mut thresh_min = if precompute { vec![0u16; total] } else { Vec::new() };
        let mut thresh_max = if precompute { vec![0u16; total] } else { Vec::new() };
        if precompute {
            for idx in 0..total {
                let ca = count_all[idx] as usize;
                thresh_min[idx] = rule_min_table[ca].max(0) as u16;
                thresh_max[idx] = rule_max_table[ca].max(0) as u16;
            }
        }

        Self {
            buf_a,
            buf_b,
            count_all,
            has_dropped_mask,
            thresh_min,
            thresh_max,
            drop_offset,
            rule_min_table,
            rule_max_table,
        }
    }

    /// Re-evaluates rule lookup tables if rules changed while geometry remained identical.
    pub fn update_rules(&mut self, rules: &Rules) {
        let table_len = self.rule_min_table.len();
        for k in 0..table_len {
            self.rule_min_table[k] = rules.min_neighbors(k as i32) as i16;
            self.rule_max_table[k] = rules.max_neighbors(k as i32) as i16;
        }
        if !self.thresh_min.is_empty() {
            for idx in 0..self.thresh_min.len() {
                let ca = self.count_all[idx] as usize;
                self.thresh_min[idx] = self.rule_min_table[ca].max(0) as u16;
                self.thresh_max[idx] = self.rule_max_table[ca].max(0) as u16;
            }
        }
    }
}

/// Executes a 1D window-3 box sum for a single contiguous line of length `len`.
#[inline(always)]
fn pass0_row(src: &[u8], dst: &mut [u16]) {
    let len = src.len();
    if len == 6 {
        let s0 = src[0] as u16;
        let s1 = src[1] as u16;
        let s2 = src[2] as u16;
        let s3 = src[3] as u16;
        let s4 = src[4] as u16;
        let s5 = src[5] as u16;

        let sum01 = s0 + s1;
        let sum12 = s1 + s2;
        let sum23 = s2 + s3;
        let sum34 = s3 + s4;
        let sum45 = s4 + s5;

        dst[0] = sum01;
        dst[1] = sum01 + s2;
        dst[2] = sum12 + s3;
        dst[3] = sum23 + s4;
        dst[4] = sum34 + s5;
        dst[5] = sum45;
    } else if len == 1 {
        dst[0] = src[0] as u16;
    } else if len == 2 {
        let sum = (src[0] as u16) + (src[1] as u16);
        dst[0] = sum;
        dst[1] = sum;
    } else {
        dst[0] = (src[0] as u16) + (src[1] as u16);
        for i in 1..len - 1 {
            dst[i] = (src[i - 1] as u16) + (src[i] as u16) + (src[i + 1] as u16);
        }
        dst[len - 1] = (src[len - 2] as u16) + (src[len - 1] as u16);
    }
}

/// Computes separable pass 0 along dimension 0.
#[inline(always)]
fn run_pass0(src: &[u8], dst: &mut [u16], s0: usize, parallel: bool) {
    let total = src.len();
    let num_rows = total / s0;

    if parallel && num_rows >= 64 {
        let chunk_rows = (num_rows / rayon::current_num_threads()).clamp(32, 512);
        let chunk_size = chunk_rows * s0;

        dst.par_chunks_mut(chunk_size)
            .zip(src.par_chunks(chunk_size))
            .for_each(|(dst_chunk, src_chunk)| {
                for (d, s) in dst_chunk.chunks_exact_mut(s0).zip(src_chunk.chunks_exact(s0)) {
                    pass0_row(s, d);
                }
            });
    } else {
        for (d, s) in dst.chunks_exact_mut(s0).zip(src.chunks_exact(s0)) {
            pass0_row(s, d);
        }
    }
}

/// Executes single-pass stream convolution for a block of size 36 (sk=6, stride=6).
#[inline(always)]
unsafe fn process_block_sk6_stride6(out_ptr: *mut u16, in_ptr: *const u16) {
    let s0 = in_ptr;
    let s1 = in_ptr.add(6);
    let s2 = in_ptr.add(12);
    let s3 = in_ptr.add(18);
    let s4 = in_ptr.add(24);
    let s5 = in_ptr.add(30);

    let o0 = out_ptr;
    let o1 = out_ptr.add(6);
    let o2 = out_ptr.add(12);
    let o3 = out_ptr.add(18);
    let o4 = out_ptr.add(24);
    let o5 = out_ptr.add(30);

    for j in 0..6 {
        let v0 = *s0.add(j);
        let v1 = *s1.add(j);
        let v2 = *s2.add(j);
        let v3 = *s3.add(j);
        let v4 = *s4.add(j);
        let v5 = *s5.add(j);

        let sum01 = v0 + v1;
        let sum12 = v1 + v2;
        let sum23 = v2 + v3;
        let sum34 = v3 + v4;
        let sum45 = v4 + v5;

        *o0.add(j) = sum01;
        *o1.add(j) = sum01 + v2;
        *o2.add(j) = sum12 + v3;
        *o3.add(j) = sum23 + v4;
        *o4.add(j) = sum34 + v5;
        *o5.add(j) = sum45;
    }
}

/// Computes separable pass for dimension `k >= 1`.
#[inline(always)]
fn run_pass_k(
    src: &[u16],
    dst: &mut [u16],
    sk: usize,
    stride: usize,
    total: usize,
    parallel: bool,
) {
    let block_size = sk * stride;
    let outer_blocks = total / block_size;

    // Fast-path specialization for sk == 6 and stride == 6 (dimension 1 in standard 6D grid)
    if sk == 6 && stride == 6 {
        let in_ptr = src.as_ptr();
        let out_ptr = dst.as_mut_ptr();
        if parallel && outer_blocks >= 64 {
            let chunk_blocks = (outer_blocks / rayon::current_num_threads()).clamp(16, 256);
            let chunk_size = chunk_blocks * 36;
            dst.par_chunks_exact_mut(chunk_size)
                .zip(src.par_chunks_exact(chunk_size))
                .for_each(|(dst_chunk, src_chunk)| {
                    let mut d_p = dst_chunk.as_mut_ptr();
                    let mut s_p = src_chunk.as_ptr();
                    for _ in 0..dst_chunk.len() / 36 {
                        unsafe {
                            process_block_sk6_stride6(d_p, s_p);
                            d_p = d_p.add(36);
                            s_p = s_p.add(36);
                        }
                    }
                });
        } else {
            for b in 0..outer_blocks {
                unsafe {
                    process_block_sk6_stride6(out_ptr.add(b * 36), in_ptr.add(b * 36));
                }
            }
        }
        return;
    }

    let process_block = |b: usize, out_slice: &mut [u16], in_slice: &[u16]| {
        let block_start = b * block_size;
        let block_in = &in_slice[block_start..block_start + block_size];
        let block_out = &mut out_slice[block_start..block_start + block_size];

        if sk == 2 {
            let (out0, out1) = block_out.split_at_mut(stride);
            let in0 = &block_in[..stride];
            let in1 = &block_in[stride..2 * stride];
            add2(out0, in0, in1);
            out1.copy_from_slice(out0);
        } else {
            add2(&mut block_out[..stride], &block_in[..stride], &block_in[stride..2 * stride]);

            for x in 1..sk - 1 {
                let x_start = x * stride;
                let (src0, rest) = block_in[x_start - stride..].split_at(stride);
                let (src1, src2_rest) = rest.split_at(stride);
                let src2 = &src2_rest[..stride];
                let dst_slice = &mut block_out[x_start..x_start + stride];
                add3(dst_slice, src0, src1, src2);
            }

            let last_start = (sk - 1) * stride;
            let src0 = &block_in[last_start - stride..last_start];
            let src1 = &block_in[last_start..last_start + stride];
            add2(&mut block_out[last_start..last_start + stride], src0, src1);
        }
    };

    if parallel && outer_blocks >= 4 {
        dst.par_chunks_exact_mut(block_size)
            .zip(src.par_chunks_exact(block_size))
            .for_each(|(dst_block, src_block)| {
                if sk == 2 {
                    let (out0, out1) = dst_block.split_at_mut(stride);
                    let in0 = &src_block[..stride];
                    let in1 = &src_block[stride..2 * stride];
                    add2(out0, in0, in1);
                    out1.copy_from_slice(out0);
                } else {
                    add2(&mut dst_block[..stride], &src_block[..stride], &src_block[stride..2 * stride]);
                    for x in 1..sk - 1 {
                        let x_start = x * stride;
                        let src0 = &src_block[x_start - stride..x_start];
                        let src1 = &src_block[x_start..x_start + stride];
                        let src2 = &src_block[x_start + stride..x_start + 2 * stride];
                        add3(&mut dst_block[x_start..x_start + stride], src0, src1, src2);
                    }
                    let last_start = (sk - 1) * stride;
                    let src0 = &src_block[last_start - stride..last_start];
                    let src1 = &src_block[last_start..last_start + stride];
                    add2(&mut dst_block[last_start..last_start + stride], src0, src1);
                }
            });
    } else {
        for b in 0..outer_blocks {
            process_block(b, dst, src);
        }
    }
}

/// Executes vector-accelerated thresholding pass for a contiguous chunk.
#[inline(always)]
fn threshold_chunk(
    chunk_dst: &mut [u8],
    box_sum: &[u16],
    current_data: &[u8],
    has_dropped_mask: &[u8],
    thresh_min: &[u16],
    thresh_max: &[u16],
    drop_offset: usize,
    start: usize,
) {
    let len = chunk_dst.len();
    let dst_ptr = chunk_dst.as_mut_ptr();
    let bs_ptr = box_sum.as_ptr();
    let cur_ptr = current_data.as_ptr();
    let mask_ptr = has_dropped_mask.as_ptr();
    let min_ptr = thresh_min.as_ptr();
    let max_ptr = thresh_max.as_ptr();

    #[cfg(target_arch = "aarch64")]
    unsafe {
        use std::arch::aarch64::*;
        let mut i = 0;

        // 32-cell unrolled NEON SIMD loop
        while i + 32 <= len {
            let idx0 = start + i;
            let bs0 = vld1q_u16(bs_ptr.add(idx0));
            let bs1 = vld1q_u16(bs_ptr.add(idx0 + 8));
            let bs2 = vld1q_u16(bs_ptr.add(idx0 + 16));
            let bs3 = vld1q_u16(bs_ptr.add(idx0 + 24));

            let cl0;
            let cl1;
            let cl2;
            let cl3;

            if idx0 >= drop_offset {
                let cur0 = vld1_u8(cur_ptr.add(idx0 - drop_offset));
                let cur1 = vld1_u8(cur_ptr.add(idx0 + 8 - drop_offset));
                let cur2 = vld1_u8(cur_ptr.add(idx0 + 16 - drop_offset));
                let cur3 = vld1_u8(cur_ptr.add(idx0 + 24 - drop_offset));

                let msk0 = vld1_u8(mask_ptr.add(idx0));
                let msk1 = vld1_u8(mask_ptr.add(idx0 + 8));
                let msk2 = vld1_u8(mask_ptr.add(idx0 + 16));
                let msk3 = vld1_u8(mask_ptr.add(idx0 + 24));

                let d0 = vmovl_u8(vand_u8(cur0, msk0));
                let d1 = vmovl_u8(vand_u8(cur1, msk1));
                let d2 = vmovl_u8(vand_u8(cur2, msk2));
                let d3 = vmovl_u8(vand_u8(cur3, msk3));

                cl0 = vsubq_u16(bs0, d0);
                cl1 = vsubq_u16(bs1, d1);
                cl2 = vsubq_u16(bs2, d2);
                cl3 = vsubq_u16(bs3, d3);
            } else {
                let mut d_buf = [0u16; 32];
                for k in 0..32 {
                    let cur_idx = idx0 + k;
                    if cur_idx >= drop_offset && *mask_ptr.add(cur_idx) != 0 {
                        d_buf[k] = *cur_ptr.add(cur_idx - drop_offset) as u16;
                    }
                }
                cl0 = vsubq_u16(bs0, vld1q_u16(d_buf.as_ptr()));
                cl1 = vsubq_u16(bs1, vld1q_u16(d_buf.as_ptr().add(8)));
                cl2 = vsubq_u16(bs2, vld1q_u16(d_buf.as_ptr().add(16)));
                cl3 = vsubq_u16(bs3, vld1q_u16(d_buf.as_ptr().add(24)));
            }

            let min0 = vld1q_u16(min_ptr.add(idx0));
            let min1 = vld1q_u16(min_ptr.add(idx0 + 8));
            let min2 = vld1q_u16(min_ptr.add(idx0 + 16));
            let min3 = vld1q_u16(min_ptr.add(idx0 + 24));

            let max0 = vld1q_u16(max_ptr.add(idx0));
            let max1 = vld1q_u16(max_ptr.add(idx0 + 8));
            let max2 = vld1q_u16(max_ptr.add(idx0 + 16));
            let max3 = vld1q_u16(max_ptr.add(idx0 + 24));

            let alive0 = vandq_u16(vcgeq_u16(cl0, min0), vcleq_u16(cl0, max0));
            let alive1 = vandq_u16(vcgeq_u16(cl1, min1), vcleq_u16(cl1, max1));
            let alive2 = vandq_u16(vcgeq_u16(cl2, min2), vcleq_u16(cl2, max2));
            let alive3 = vandq_u16(vcgeq_u16(cl3, min3), vcleq_u16(cl3, max3));

            let b0 = vmovn_u16(vshrq_n_u16(alive0, 15));
            let b1 = vmovn_u16(vshrq_n_u16(alive1, 15));
            let b2 = vmovn_u16(vshrq_n_u16(alive2, 15));
            let b3 = vmovn_u16(vshrq_n_u16(alive3, 15));

            let b01 = vcombine_u8(b0, b1);
            let b23 = vcombine_u8(b2, b3);

            vst1q_u8(dst_ptr.add(i), b01);
            vst1q_u8(dst_ptr.add(i + 16), b23);
            i += 32;
        }

        while i < len {
            let idx = start + i;
            let drop_sub = if idx >= drop_offset && *mask_ptr.add(idx) != 0 {
                *cur_ptr.add(idx - drop_offset) as u16
            } else {
                0
            };
            let count_live = *bs_ptr.add(idx) - drop_sub;
            let min_k = *min_ptr.add(idx);
            let max_k = *max_ptr.add(idx);
            *dst_ptr.add(i) = if count_live >= min_k && count_live <= max_k { 1 } else { 0 };
            i += 1;
        }
        return;
    }

    #[allow(unreachable_code)]
    {
        for (offset, cell) in chunk_dst.iter_mut().enumerate() {
            let idx = start + offset;
            let drop_sub = if idx >= drop_offset && has_dropped_mask[idx] != 0 {
                current_data[idx - drop_offset] as u16
            } else {
                0
            };
            let count_live = box_sum[idx] - drop_sub;
            let min_k = thresh_min[idx];
            let max_k = thresh_max[idx];
            *cell = if count_live >= min_k && count_live <= max_k {
                1
            } else {
                0
            };
        }
    }
}

/// Thresholding pass for very large grids where precomputed lookup tables would exceed memory.
/// Evaluates coordinate bounds and rule thresholds on the fly row-by-row with minimal memory overhead.
fn threshold_large(
    next_data: &mut [u8],
    box_sum: &[u16],
    current_data: &[u8],
    dims: &GridDimensions,
    aux: &SimulationAux,
    parallel: bool,
) {
    let total = dims.total_cells;
    let d = dims.dimensions;
    let s = dims.size_in_dimensions;
    let s0 = s[0];
    let drop_offset = aux.drop_offset;
    let rule_min = &aux.rule_min_table;
    let rule_max = &aux.rule_max_table;

    let num_rows = total / s0;

    let process_row = |r: usize, dst_row: &mut [u8]| {
        let base = r * s0;
        let mut temp = r;
        let e = temp % s[1];
        temp /= s[1];
        let d_coord = temp % s[2];
        temp /= s[2];
        let c = temp % s[3];
        temp /= s[3];
        let b = temp % s[4];
        temp /= s[4];
        let a = temp;

        let vol_a = if s[5] == 1 { 1 } else if a == 0 || a == s[5] - 1 { 2 } else { 3 };
        let a_pos = a > 0 || d < 6;

        let vol_b = vol_a * (if s[4] == 1 { 1 } else if b == 0 || b == s[4] - 1 { 2 } else { 3 });
        let b_pos = a_pos && (b > 0 || d < 5);

        let vol_c = vol_b * (if s[3] == 1 { 1 } else if c == 0 || c == s[3] - 1 { 2 } else { 3 });
        let c_pos = b_pos && (c > 0 || d < 4);

        let vol_d = vol_c * (if s[2] == 1 { 1 } else if d_coord == 0 || d_coord == s[2] - 1 { 2 } else { 3 });
        let d_pos = c_pos && (d_coord > 0 || d < 3);

        let vol_e = vol_d * (if s[1] == 1 { 1 } else if e == 0 || e == s[1] - 1 { 2 } else { 3 });
        let e_pos = d_pos && (e > 0 || d < 2);

        for f in 0..s0 {
            let idx = base + f;
            let vol_f = if s0 == 1 { 1 } else if f == 0 || f == s0 - 1 { 2 } else { 3 };
            let total_vol = vol_e * vol_f;
            let drop_in_bounds = e_pos && (f > 0);
            let count_all = if drop_in_bounds { total_vol - 1 } else { total_vol };

            let drop_val = if drop_in_bounds {
                current_data[idx - drop_offset] as i32
            } else {
                0
            };
            let count_live = (box_sum[idx] as i32) - drop_val;
            let min_k = rule_min[count_all] as i32;
            let max_k = rule_max[count_all] as i32;
            dst_row[f] = if count_live >= min_k && count_live <= max_k { 1 } else { 0 };
        }
    };

    if parallel && num_rows >= 64 {
        let chunk_rows = (num_rows / rayon::current_num_threads()).clamp(16, 512);
        let chunk_size = chunk_rows * s0;
        next_data
            .par_chunks_mut(chunk_size)
            .enumerate()
            .for_each(|(chunk_idx, dst_chunk)| {
                let row_start = chunk_idx * chunk_rows;
                for (local_row, dst_row) in dst_chunk.chunks_exact_mut(s0).enumerate() {
                    let r = row_start + local_row;
                    process_row(r, dst_row);
                }
            });
    } else {
        for (r, dst_row) in next_data.chunks_exact_mut(s0).enumerate() {
            process_row(r, dst_row);
        }
    }
}

/// Executes full multidimensional simulation step using separable SIMD convolution.
pub fn step_separable(
    current_data: &[u8],
    next_data: &mut [u8],
    dims: &GridDimensions,
    aux: &mut SimulationAux,
    parallel: bool,
) {
    let total = dims.total_cells;
    let d = dims.dimensions;
    let s = dims.size_in_dimensions;
    let strides = dims.strides;

    // Pass 0: dimension 0 (f, stride 1, size s[0])
    run_pass0(current_data, &mut aux.buf_a, s[0], parallel);

    // Passes 1..d-1
    let mut current_buf_is_a = true;
    for k in 1..d {
        let sk = s[k];
        let stride = strides[k];
        if sk == 1 {
            continue;
        }

        if current_buf_is_a {
            run_pass_k(&aux.buf_a, &mut aux.buf_b, sk, stride, total, parallel);
            current_buf_is_a = false;
        } else {
            run_pass_k(&aux.buf_b, &mut aux.buf_a, sk, stride, total, parallel);
            current_buf_is_a = true;
        }
    }

    let box_sum = if current_buf_is_a {
        &aux.buf_a[..total]
    } else {
        &aux.buf_b[..total]
    };

    if aux.thresh_min.is_empty() {
        threshold_large(next_data, box_sum, current_data, dims, aux, parallel);
        return;
    }

    let has_dropped_mask = &aux.has_dropped_mask;
    let drop_offset = aux.drop_offset;
    let thresh_min = &aux.thresh_min;
    let thresh_max = &aux.thresh_max;

    // Final branchless SIMD thresholding pass
    if parallel && total >= 512 {
        let chunk_size = (total / rayon::current_num_threads()).clamp(64, 4096);
        next_data
            .par_chunks_mut(chunk_size)
            .enumerate()
            .for_each(|(chunk_idx, chunk)| {
                let start = chunk_idx * chunk_size;
                threshold_chunk(
                    chunk,
                    box_sum,
                    current_data,
                    has_dropped_mask,
                    thresh_min,
                    thresh_max,
                    drop_offset,
                    start,
                );
            });
    } else {
        threshold_chunk(
            next_data,
            box_sum,
            current_data,
            has_dropped_mask,
            thresh_min,
            thresh_max,
            drop_offset,
            0,
        );
    }
}
