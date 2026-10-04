use rand::rngs::StdRng;
use rand::SeedableRng;
use six_d_game_of_life::{Rules, Simulation};

fn step_separable_reference(sim: &Simulation, out: &mut [u8]) {
    let dims = &sim.current.dims;
    let total = dims.total_cells;
    let d = dims.dimensions;
    let s = &dims.size_in_dimensions;
    let strides = &dims.strides;
    let current_data = sim.current.as_slice();

    // Two ping-pong buffers of u16
    let mut buf_a = vec![0u16; total];
    let mut buf_b = vec![0u16; total];

    // Pass 0: dimension 0 (f, stride 1, size s[0])
    let s0 = s[0];
    let rows0 = total / s0;
    for r in 0..rows0 {
        let base = r * s0;
        if s0 == 1 {
            buf_a[base] = current_data[base] as u16;
        } else if s0 == 2 {
            let sum = (current_data[base] + current_data[base + 1]) as u16;
            buf_a[base] = sum;
            buf_a[base + 1] = sum;
        } else {
            buf_a[base] = (current_data[base] + current_data[base + 1]) as u16;
            for f in 1..s0 - 1 {
                buf_a[base + f] = (current_data[base + f - 1]
                    + current_data[base + f]
                    + current_data[base + f + 1]) as u16;
            }
            buf_a[base + s0 - 1] =
                (current_data[base + s0 - 2] + current_data[base + s0 - 1]) as u16;
        }
    }

    // Now passes 1 to d-1
    let mut current_buf_is_a = true;
    for k in 1..d {
        let sk = s[k];
        let stride = strides[k];
        if sk == 1 {
            continue;
        }
        let outer_blocks = total / (sk * stride);

        if current_buf_is_a {
            for b in 0..outer_blocks {
                let block_start = b * sk * stride;
                if sk == 2 {
                    for i in 0..stride {
                        let sum = buf_a[block_start + i] + buf_a[block_start + stride + i];
                        buf_b[block_start + i] = sum;
                        buf_b[block_start + stride + i] = sum;
                    }
                } else {
                    for i in 0..stride {
                        buf_b[block_start + i] =
                            buf_a[block_start + i] + buf_a[block_start + stride + i];
                    }
                    for x in 1..sk - 1 {
                        let x_start = block_start + x * stride;
                        for i in 0..stride {
                            buf_b[x_start + i] = buf_a[x_start - stride + i]
                                + buf_a[x_start + i]
                                + buf_a[x_start + stride + i];
                        }
                    }
                    let last_start = block_start + (sk - 1) * stride;
                    for i in 0..stride {
                        buf_b[last_start + i] =
                            buf_a[last_start - stride + i] + buf_a[last_start + i];
                    }
                }
            }
            current_buf_is_a = false;
        } else {
            for b in 0..outer_blocks {
                let block_start = b * sk * stride;
                if sk == 2 {
                    for i in 0..stride {
                        let sum = buf_b[block_start + i] + buf_b[block_start + stride + i];
                        buf_a[block_start + i] = sum;
                        buf_a[block_start + stride + i] = sum;
                    }
                } else {
                    for i in 0..stride {
                        buf_a[block_start + i] =
                            buf_b[block_start + i] + buf_b[block_start + stride + i];
                    }
                    for x in 1..sk - 1 {
                        let x_start = block_start + x * stride;
                        for i in 0..stride {
                            buf_a[x_start + i] = buf_b[x_start - stride + i]
                                + buf_b[x_start + i]
                                + buf_b[x_start + stride + i];
                        }
                    }
                    let last_start = block_start + (sk - 1) * stride;
                    for i in 0..stride {
                        buf_a[last_start + i] =
                            buf_b[last_start - stride + i] + buf_b[last_start + i];
                    }
                }
            }
            current_buf_is_a = true;
        }
    }

    let box_sum = if current_buf_is_a { &buf_a } else { &buf_b };

    // Constant drop offset
    let mut drop_offset = 0usize;
    for k in 0..d {
        drop_offset += strides[k];
    }

    let mut len_table = [[0i32; 6]; 6];
    for dim_idx in 0..6 {
        let size = s[dim_idx];
        for coord in 0..size.min(6) {
            len_table[dim_idx][coord] = if size == 1 {
                1
            } else if coord == 0 || coord == size - 1 {
                2
            } else {
                3
            };
        }
    }

    let mut idx = 0;
    for a in 0..s[5] {
        let vol_a = if s[5] == 1 { 1 } else if a == 0 || a == s[5] - 1 { 2 } else { 3 };
        let a_pos = a > 0 || d < 6;
        for b in 0..s[4] {
            let vol_b = vol_a * (if s[4] == 1 { 1 } else if b == 0 || b == s[4] - 1 { 2 } else { 3 });
            let b_pos = a_pos && (b > 0 || d < 5);
            for c in 0..s[3] {
                let vol_c = vol_b * (if s[3] == 1 { 1 } else if c == 0 || c == s[3] - 1 { 2 } else { 3 });
                let c_pos = b_pos && (c > 0 || d < 4);
                for d_coord in 0..s[2] {
                    let vol_d = vol_c * (if s[2] == 1 { 1 } else if d_coord == 0 || d_coord == s[2] - 1 { 2 } else { 3 });
                    let d_pos = c_pos && (d_coord > 0 || d < 3);
                    for e in 0..s[1] {
                        let vol_e = vol_d * (if s[1] == 1 { 1 } else if e == 0 || e == s[1] - 1 { 2 } else { 3 });
                        let e_pos = d_pos && (e > 0 || d < 2);
                        for f in 0..s[0] {
                            let vol_f = if s[0] == 1 { 1 } else if f == 0 || f == s[0] - 1 { 2 } else { 3 };
                            let total_vol = vol_e * vol_f;
                            let drop_in_bounds = e_pos && (f > 0);
                            let count_all = if drop_in_bounds { total_vol - 1 } else { total_vol };
                            let mut count_live = box_sum[idx] as i32;
                            if drop_in_bounds {
                                count_live -= current_data[idx - drop_offset] as i32;
                            }
                            out[idx] = if sim.rules.evaluate(count_live, count_all) { 1 } else { 0 };
                            idx += 1;
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn test_separable_equivalence_against_sequential() {
    let mut rng = StdRng::seed_from_u64(12345);

    for dim in 1..=6 {
        let max_size = if dim == 6 { 6 } else if dim >= 4 { 6 } else { 10 };
        for size in [1, 2, 3, 4, 6] {
            if size > max_size { continue; }
            let rules = Rules::default();
            let mut sim = Simulation::new(size, dim, rules);
            sim.randomize(&mut rng);

            let mut out = vec![0u8; sim.current.len()];
            step_separable_reference(&sim, &mut out);

            // Compute sequential reference
            let mut expected = vec![0u8; sim.current.len()];
            for idx in 0..sim.current.len() {
                expected[idx] = sim.compute_cell_state(idx);
            }

            assert_eq!(
                out, expected,
                "Separable convolution mismatch for dim={}, size={}",
                dim, size
            );
        }
    }
}
