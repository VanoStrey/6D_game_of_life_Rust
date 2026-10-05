//! Ultra-high-performance multi-dimensional cellular automaton engine.
//! Supports both n-dimensional torus (T^D / Periodic Boundary Conditions) and
//! bounded space (Hard / Zero Boundary Conditions).
//!
//! Replaces $O(3^D \times N)$ neighbor searching with separable $O(D \times N)$
//! 1D box convolutions using hardware SIMD vector intrinsics
//! (ARM NEON & x86 AVX2) and assembly acceleration.

use super::coords::GridDimensions;
use super::rules::Rules;
use super::simd_ops::{add2, add3};
use rayon::prelude::*;

/// Precomputed metadata and reusable scratch buffers for separable simulation.
#[derive(Debug, Clone)]
pub struct SimulationAux {
    pub buf_a: Vec<u16>,
    pub buf_b: Vec<u16>,
    pub thresh_min: u16,
    pub thresh_max: u16,
    pub thresh_min_lut: Vec<u16>,
    pub thresh_max_lut: Vec<u16>,
    pub periodic: bool,
}

impl SimulationAux {
    /// Builds ping-pong buffers and precomputes thresholds (defaults to Periodic).
    pub fn new(dims: &GridDimensions, rules: &Rules) -> Self {
        Self::new_with_mode(dims, rules, true)
    }

    /// Builds ping-pong buffers and precomputes thresholds for either Periodic (Torus) or Bounded space.
    pub fn new_with_mode(dims: &GridDimensions, rules: &Rules, periodic: bool) -> Self {
        let total = dims.total_cells;
        let buf_a = vec![0u16; total];
        let buf_b = vec![0u16; total];

        let d = dims.dimensions;
        let count_all = (3usize.pow(d as u32) - 1) as i32;
        let thresh_min = rules.min_neighbors(count_all).max(0) as u16;
        let thresh_max = rules.max_neighbors(count_all).max(0) as u16;

        let (thresh_min_lut, thresh_max_lut) = if periodic {
            (Vec::new(), Vec::new())
        } else {
            let mut min_lut = vec![0u16; total];
            let mut max_lut = vec![0u16; total];
            let mut coords = [0usize; 6];
            for idx in 0..total {
                dims.coords_nd(idx, &mut coords);
                let mut vol = 1usize;
                for k in 0..dims.dimensions {
                    let sk = dims.size_in_dimensions[k];
                    let v = if sk == 1 {
                        1
                    } else if coords[k] == 0 || coords[k] == sk - 1 {
                        2
                    } else {
                        3
                    };
                    vol *= v;
                }
                let in_bounds_count = (vol - 1) as i32;
                min_lut[idx] = rules.min_neighbors(in_bounds_count).max(0) as u16;
                max_lut[idx] = rules.max_neighbors(in_bounds_count).max(0) as u16;
            }
            (min_lut, max_lut)
        };

        Self {
            buf_a,
            buf_b,
            thresh_min,
            thresh_max,
            thresh_min_lut,
            thresh_max_lut,
            periodic,
        }
    }

    /// Re-evaluates rule thresholds when rules, dimensions, or periodic mode change.
    pub fn update_rules(&mut self, dims: &GridDimensions, rules: &Rules, periodic: bool) {
        self.periodic = periodic;
        let d = dims.dimensions;
        let count_all = (3usize.pow(d as u32) - 1) as i32;
        self.thresh_min = rules.min_neighbors(count_all).max(0) as u16;
        self.thresh_max = rules.max_neighbors(count_all).max(0) as u16;

        let total = dims.total_cells;
        if periodic {
            self.thresh_min_lut.clear();
            self.thresh_max_lut.clear();
        } else {
            if self.thresh_min_lut.len() != total {
                self.thresh_min_lut = vec![0u16; total];
                self.thresh_max_lut = vec![0u16; total];
            }
            let mut coords = [0usize; 6];
            for idx in 0..total {
                dims.coords_nd(idx, &mut coords);
                let mut vol = 1usize;
                for k in 0..dims.dimensions {
                    let sk = dims.size_in_dimensions[k];
                    let v = if sk == 1 {
                        1
                    } else if coords[k] == 0 || coords[k] == sk - 1 {
                        2
                    } else {
                        3
                    };
                    vol *= v;
                }
                let in_bounds_count = (vol - 1) as i32;
                self.thresh_min_lut[idx] = rules.min_neighbors(in_bounds_count).max(0) as u16;
                self.thresh_max_lut[idx] = rules.max_neighbors(in_bounds_count).max(0) as u16;
            }
        }
    }
}

/// Executes a 1D window-3 box sum for a single contiguous line of length `len`.
#[inline(always)]
fn pass0_row(src: &[u8], dst: &mut [u16], periodic: bool) {
    let len = src.len();
    if periodic {
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

            // Periodic wrap: left of 0 is 5, right of 5 is 0
            dst[0] = sum01 + s5;
            dst[1] = sum01 + s2;
            dst[2] = sum12 + s3;
            dst[3] = sum23 + s4;
            dst[4] = sum34 + s5;
            dst[5] = sum45 + s0;
        } else if len == 1 {
            dst[0] = (src[0] as u16) * 3;
        } else if len == 2 {
            let s0 = src[0] as u16;
            let s1 = src[1] as u16;
            dst[0] = s0 + (s1 << 1);
            dst[1] = s1 + (s0 << 1);
        } else {
            let s_first = src[0] as u16;
            let s_last = src[len - 1] as u16;

            dst[0] = s_last + s_first + (src[1] as u16);
            for i in 1..len - 1 {
                dst[i] = (src[i - 1] as u16) + (src[i] as u16) + (src[i + 1] as u16);
            }
            dst[len - 1] = (src[len - 2] as u16) + s_last + s_first;
        }
    } else {
        // Bounded space (Zero / Hard Boundaries)
        if len == 1 {
            dst[0] = src[0] as u16;
        } else if len == 2 {
            let sum = src[0] as u16 + src[1] as u16;
            dst[0] = sum;
            dst[1] = sum;
        } else if len == 6 {
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
        } else {
            dst[0] = (src[0] as u16) + (src[1] as u16);
            for i in 1..len - 1 {
                dst[i] = (src[i - 1] as u16) + (src[i] as u16) + (src[i + 1] as u16);
            }
            dst[len - 1] = (src[len - 2] as u16) + (src[len - 1] as u16);
        }
    }
}

/// Computes separable pass 0 along dimension 0.
#[inline(always)]
fn run_pass0(src: &[u8], dst: &mut [u16], s0: usize, parallel: bool, periodic: bool) {
    let total = src.len();
    let num_rows = total / s0;

    if parallel && num_rows >= 64 {
        let chunk_rows = (num_rows / rayon::current_num_threads()).clamp(32, 512);
        let chunk_size = chunk_rows * s0;

        dst.par_chunks_mut(chunk_size)
            .zip(src.par_chunks(chunk_size))
            .for_each(|(dst_chunk, src_chunk)| {
                for (d, s) in dst_chunk.chunks_exact_mut(s0).zip(src_chunk.chunks_exact(s0)) {
                    pass0_row(s, d, periodic);
                }
            });
    } else {
        for (d, s) in dst.chunks_exact_mut(s0).zip(src.chunks_exact(s0)) {
            pass0_row(s, d, periodic);
        }
    }
}

/// Executes single-pass stream periodic convolution for a block of size 36 (sk=6, stride=6).
#[inline(always)]
unsafe fn process_block_sk6_stride6_pbc(out_ptr: *mut u16, in_ptr: *const u16) {
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

        // Periodic wrap: top of 0 is 5, bottom of 5 is 0
        *o0.add(j) = sum01 + v5;
        *o1.add(j) = sum01 + v2;
        *o2.add(j) = sum12 + v3;
        *o3.add(j) = sum23 + v4;
        *o4.add(j) = sum34 + v5;
        *o5.add(j) = sum45 + v0;
    }
}

/// Executes single-pass stream bounded convolution for a block of size 36 (sk=6, stride=6).
#[inline(always)]
unsafe fn process_block_sk6_stride6_bounded(out_ptr: *mut u16, in_ptr: *const u16) {
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

        // Bounded (zero-boundary): no wrap
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
    periodic: bool,
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
                            if periodic {
                                process_block_sk6_stride6_pbc(d_p, s_p);
                            } else {
                                process_block_sk6_stride6_bounded(d_p, s_p);
                            }
                            d_p = d_p.add(36);
                            s_p = s_p.add(36);
                        }
                    }
                });
        } else {
            for b in 0..outer_blocks {
                unsafe {
                    if periodic {
                        process_block_sk6_stride6_pbc(out_ptr.add(b * 36), in_ptr.add(b * 36));
                    } else {
                        process_block_sk6_stride6_bounded(out_ptr.add(b * 36), in_ptr.add(b * 36));
                    }
                }
            }
        }
        return;
    }

    let process_block = |b: usize, out_slice: &mut [u16], in_slice: &[u16]| {
        let block_start = b * block_size;
        let block_in = &in_slice[block_start..block_start + block_size];
        let block_out = &mut out_slice[block_start..block_start + block_size];

        if sk == 1 {
            if periodic {
                for (d, s) in block_out.iter_mut().zip(block_in.iter()) {
                    *d = *s * 3;
                }
            } else {
                block_out.copy_from_slice(block_in);
            }
        } else if sk == 2 {
            let (out0, out1) = block_out.split_at_mut(stride);
            let in0 = &block_in[..stride];
            let in1 = &block_in[stride..2 * stride];
            if periodic {
                add3(out0, in0, in1, in1);
                add3(out1, in0, in0, in1);
            } else {
                add2(out0, in0, in1);
                add2(out1, in0, in1);
            }
        } else if periodic {
            let last_start = (sk - 1) * stride;

            // Slice 0: periodic left neighbor wraps to slice sk - 1
            add3(
                &mut block_out[..stride],
                &block_in[last_start..last_start + stride],
                &block_in[..stride],
                &block_in[stride..2 * stride],
            );

            // Interior slices x in 1..sk-1
            for x in 1..sk - 1 {
                let x_start = x * stride;
                let (src0, rest) = block_in[x_start - stride..].split_at(stride);
                let (src1, src2_rest) = rest.split_at(stride);
                let src2 = &src2_rest[..stride];
                let dst_slice = &mut block_out[x_start..x_start + stride];
                add3(dst_slice, src0, src1, src2);
            }

            // Slice sk - 1: periodic right neighbor wraps to slice 0
            add3(
                &mut block_out[last_start..last_start + stride],
                &block_in[last_start - stride..last_start],
                &block_in[last_start..last_start + stride],
                &block_in[..stride],
            );
        } else {
            // Bounded space (Zero / Hard Boundaries)
            let last_start = (sk - 1) * stride;

            add2(
                &mut block_out[..stride],
                &block_in[..stride],
                &block_in[stride..2 * stride],
            );

            for x in 1..sk - 1 {
                let x_start = x * stride;
                let (src0, rest) = block_in[x_start - stride..].split_at(stride);
                let (src1, src2_rest) = rest.split_at(stride);
                let src2 = &src2_rest[..stride];
                let dst_slice = &mut block_out[x_start..x_start + stride];
                add3(dst_slice, src0, src1, src2);
            }

            add2(
                &mut block_out[last_start..last_start + stride],
                &block_in[last_start - stride..last_start],
                &block_in[last_start..last_start + stride],
            );
        }
    };

    if parallel && outer_blocks >= 4 {
        dst.par_chunks_exact_mut(block_size)
            .zip(src.par_chunks_exact(block_size))
            .for_each(|(dst_block, src_block)| {
                if sk == 1 {
                    if periodic {
                        for (d, s) in dst_block.iter_mut().zip(src_block.iter()) {
                            *d = *s * 3;
                        }
                    } else {
                        dst_block.copy_from_slice(src_block);
                    }
                } else if sk == 2 {
                    let (out0, out1) = dst_block.split_at_mut(stride);
                    let in0 = &src_block[..stride];
                    let in1 = &src_block[stride..2 * stride];
                    if periodic {
                        add3(out0, in0, in1, in1);
                        add3(out1, in0, in0, in1);
                    } else {
                        add2(out0, in0, in1);
                        add2(out1, in0, in1);
                    }
                } else if periodic {
                    let last_start = (sk - 1) * stride;
                    add3(
                        &mut dst_block[..stride],
                        &src_block[last_start..last_start + stride],
                        &src_block[..stride],
                        &src_block[stride..2 * stride],
                    );
                    for x in 1..sk - 1 {
                        let x_start = x * stride;
                        let src0 = &src_block[x_start - stride..x_start];
                        let src1 = &src_block[x_start..x_start + stride];
                        let src2 = &src_block[x_start + stride..x_start + 2 * stride];
                        add3(&mut dst_block[x_start..x_start + stride], src0, src1, src2);
                    }
                    add3(
                        &mut dst_block[last_start..last_start + stride],
                        &src_block[last_start - stride..last_start],
                        &src_block[last_start..last_start + stride],
                        &src_block[..stride],
                    );
                } else {
                    let last_start = (sk - 1) * stride;
                    add2(
                        &mut dst_block[..stride],
                        &src_block[..stride],
                        &src_block[stride..2 * stride],
                    );
                    for x in 1..sk - 1 {
                        let x_start = x * stride;
                        let src0 = &src_block[x_start - stride..x_start];
                        let src1 = &src_block[x_start..x_start + stride];
                        let src2 = &src_block[x_start + stride..x_start + 2 * stride];
                        add3(&mut dst_block[x_start..x_start + stride], src0, src1, src2);
                    }
                    add2(
                        &mut dst_block[last_start..last_start + stride],
                        &src_block[last_start - stride..last_start],
                        &src_block[last_start..last_start + stride],
                    );
                }
            });
    } else {
        for b in 0..outer_blocks {
            process_block(b, dst, src);
        }
    }
}

/// Executes vector-accelerated thresholding pass for a contiguous chunk.
/// Subtracts self-cell state from full box sum to obtain exact live Moore neighbors.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn threshold_chunk(
    chunk_dst: &mut [u8],
    box_sum: &[u16],
    current_data: &[u8],
    thresh_min: u16,
    thresh_max: u16,
    thresh_min_lut: &[u16],
    thresh_max_lut: &[u16],
    periodic: bool,
    start: usize,
) {
    let len = chunk_dst.len();
    let dst_ptr = chunk_dst.as_mut_ptr();
    let bs_ptr = box_sum.as_ptr();
    let cur_ptr = current_data.as_ptr();

    #[cfg(target_arch = "aarch64")]
    unsafe {
        use std::arch::aarch64::*;
        let min_scalar_vec = vdupq_n_u16(thresh_min);
        let max_scalar_vec = vdupq_n_u16(thresh_max);
        let min_ptr = thresh_min_lut.as_ptr();
        let max_ptr = thresh_max_lut.as_ptr();

        let mut i = 0;

        // 32-cell unrolled NEON SIMD loop
        while i + 32 <= len {
            let idx0 = start + i;
            let bs0 = vld1q_u16(bs_ptr.add(idx0));
            let bs1 = vld1q_u16(bs_ptr.add(idx0 + 8));
            let bs2 = vld1q_u16(bs_ptr.add(idx0 + 16));
            let bs3 = vld1q_u16(bs_ptr.add(idx0 + 24));

            let cur0 = vmovl_u8(vld1_u8(cur_ptr.add(idx0)));
            let cur1 = vmovl_u8(vld1_u8(cur_ptr.add(idx0 + 8)));
            let cur2 = vmovl_u8(vld1_u8(cur_ptr.add(idx0 + 16)));
            let cur3 = vmovl_u8(vld1_u8(cur_ptr.add(idx0 + 24)));

            // Subtract center cell: count_live = FullBox - cell
            let live0 = vsubq_u16(bs0, cur0);
            let live1 = vsubq_u16(bs1, cur1);
            let live2 = vsubq_u16(bs2, cur2);
            let live3 = vsubq_u16(bs3, cur3);

            let (min0, min1, min2, min3, max0, max1, max2, max3) = if periodic {
                (
                    min_scalar_vec,
                    min_scalar_vec,
                    min_scalar_vec,
                    min_scalar_vec,
                    max_scalar_vec,
                    max_scalar_vec,
                    max_scalar_vec,
                    max_scalar_vec,
                )
            } else {
                (
                    vld1q_u16(min_ptr.add(idx0)),
                    vld1q_u16(min_ptr.add(idx0 + 8)),
                    vld1q_u16(min_ptr.add(idx0 + 16)),
                    vld1q_u16(min_ptr.add(idx0 + 24)),
                    vld1q_u16(max_ptr.add(idx0)),
                    vld1q_u16(max_ptr.add(idx0 + 8)),
                    vld1q_u16(max_ptr.add(idx0 + 16)),
                    vld1q_u16(max_ptr.add(idx0 + 24)),
                )
            };

            let alive0 = vandq_u16(vcgeq_u16(live0, min0), vcleq_u16(live0, max0));
            let alive1 = vandq_u16(vcgeq_u16(live1, min1), vcleq_u16(live1, max1));
            let alive2 = vandq_u16(vcgeq_u16(live2, min2), vcleq_u16(live2, max2));
            let alive3 = vandq_u16(vcgeq_u16(live3, min3), vcleq_u16(live3, max3));

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
            let count_live = *bs_ptr.add(idx) - (*cur_ptr.add(idx) as u16);
            let (min_val, max_val) = if periodic {
                (thresh_min, thresh_max)
            } else {
                (*min_ptr.add(idx), *max_ptr.add(idx))
            };
            *dst_ptr.add(i) = if count_live >= min_val && count_live <= max_val {
                1
            } else {
                0
            };
            i += 1;
        }
        return;
    }

    #[allow(unreachable_code)]
    {
        for (offset, cell) in chunk_dst.iter_mut().enumerate() {
            let idx = start + offset;
            let count_live = box_sum[idx] - (current_data[idx] as u16);
            let (min_val, max_val) = if periodic {
                (thresh_min, thresh_max)
            } else {
                (thresh_min_lut[idx], thresh_max_lut[idx])
            };
            *cell = if count_live >= min_val && count_live <= max_val {
                1
            } else {
                0
            };
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
    let periodic = aux.periodic;

    // Pass 0: dimension 0 (f, stride 1, size s[0])
    run_pass0(current_data, &mut aux.buf_a, s[0], parallel, periodic);

    // Passes 1..d-1
    let mut current_buf_is_a = true;
    for k in 1..d {
        let sk = s[k];
        let stride = strides[k];

        if current_buf_is_a {
            run_pass_k(
                &aux.buf_a,
                &mut aux.buf_b,
                sk,
                stride,
                total,
                parallel,
                periodic,
            );
            current_buf_is_a = false;
        } else {
            run_pass_k(
                &aux.buf_b,
                &mut aux.buf_a,
                sk,
                stride,
                total,
                parallel,
                periodic,
            );
            current_buf_is_a = true;
        }
    }

    let box_sum = if current_buf_is_a {
        &aux.buf_a[..total]
    } else {
        &aux.buf_b[..total]
    };

    let thresh_min = aux.thresh_min;
    let thresh_max = aux.thresh_max;
    let thresh_min_lut = &aux.thresh_min_lut;
    let thresh_max_lut = &aux.thresh_max_lut;

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
                    thresh_min,
                    thresh_max,
                    thresh_min_lut,
                    thresh_max_lut,
                    periodic,
                    start,
                );
            });
    } else {
        threshold_chunk(
            next_data,
            box_sum,
            current_data,
            thresh_min,
            thresh_max,
            thresh_min_lut,
            thresh_max_lut,
            periodic,
            0,
        );
    }
}
