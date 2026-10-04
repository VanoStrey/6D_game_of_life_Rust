//! 3D visual projection of N-dimensional cellular automata.
//!
//! Faithfully implements Java visual projection from `LogicGameOfLive.java`:
//! - `X = d + c * (size + delta)`
//! - `Y = e + b * (size + delta)`
//! - `Z = f + a * (size + delta)`

use crate::simulation::grid::Grid;
use serde::{Deserialize, Serialize};

/// 3D Visual grid bounds matching Java `sizeGameBoardVizual()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualBounds {
    pub size_x: usize,
    pub size_y: usize,
    pub size_z: usize,
}

impl VisualBounds {
    /// Computes visual dimensions matching Java `sizes[2]` (sizeX), `sizes[1]` (sizeY), `sizes[0]` (sizeZ).
    pub fn compute(dimensions: usize, size: usize, delta: usize) -> Self {
        let (size_x, size_y, size_z) = match dimensions {
            6 => (
                (size + delta) * size,
                (size + delta) * size,
                (size + delta) * size,
            ),
            5 => ((size + delta) * size, (size + delta) * size, size),
            4 => ((size + delta) * size, size, size),
            3 => (size, size, size),
            2 => (1, size, size),
            1 => (1, 1, size),
            d if d > 6 => {
                if size == 1 {
                    (1, 1, 1)
                } else {
                    let base_stride = size + delta;
                    let mut sx = base_stride * size;
                    let mut sy = base_stride * size;
                    let mut sz = base_stride * size;
                    let mut mult = base_stride * size + delta;
                    for k in 6..d {
                        match (k - 6) % 3 {
                            0 => sx = sx.saturating_add(mult.saturating_mul(size - 1)),
                            1 => sy = sy.saturating_add(mult.saturating_mul(size - 1)),
                            2 => {
                                sz = sz.saturating_add(mult.saturating_mul(size - 1));
                                mult = mult.saturating_mul(size).saturating_add(delta);
                            }
                            _ => unreachable!(),
                        }
                    }
                    (sx, sy, sz)
                }
            }
            _ => (
                (size + delta) * size,
                (size + delta) * size,
                (size + delta) * size,
            ),
        };

        Self {
            size_x,
            size_y,
            size_z,
        }
    }

    /// Total 3D potential positions count.
    pub fn total_volume(&self) -> usize {
        self.size_x
            .saturating_mul(self.size_y)
            .saturating_mul(self.size_z)
    }
}

/// Projects a 6D coordinate `(a, b, c, d, e, f)` to 3D position `(x, y, z)`.
#[inline(always)]
pub fn project_coords(
    a: usize,
    b: usize,
    c: usize,
    d: usize,
    e: usize,
    f: usize,
    size: usize,
    delta: usize,
) -> (usize, usize, usize) {
    let stride = size + delta;
    let x = d + c * stride;
    let y = e + b * stride;
    let z = f + a * stride;
    (x, y, z)
}

/// Projects an N-dimensional coordinate slice to 3D position `(x, y, z)`.
#[inline(always)]
pub fn project_coords_nd(coords: &[usize], size: usize, delta: usize) -> (usize, usize, usize) {
    let d = coords.len();
    if d == 0 || size == 1 {
        return (0, 0, 0);
    }

    let f = coords[0];
    let e = if d >= 2 { coords[1] } else { 0 };
    let d_coord = if d >= 3 { coords[2] } else { 0 };
    let c = if d >= 4 { coords[3] } else { 0 };
    let b = if d >= 5 { coords[4] } else { 0 };
    let a = if d >= 6 { coords[5] } else { 0 };

    let stride = size + delta;
    let mut x = d_coord + c * stride;
    let mut y = e + b * stride;
    let mut z = f + a * stride;

    if d > 6 {
        let mut mult = stride * size + delta;
        for k in 6..d {
            let ck = coords[k];
            if ck > 0 {
                match (k - 6) % 3 {
                    0 => x += ck * mult,
                    1 => y += ck * mult,
                    2 => {
                        z += ck * mult;
                        mult = mult.saturating_mul(size).saturating_add(delta);
                    }
                    _ => unreachable!(),
                }
            }
        }
    }

    (x, y, z)
}

/// Maximum number of 3D cube instances rendered simultaneously on GPU.
pub const MAX_RENDER_INSTANCES: usize = 1_048_576;

/// Collects 3D positions of alive cells in the grid with high-speed zero-row skipping.
/// Safely caps output to `MAX_RENDER_INSTANCES` using uniform strided sampling across the entire
/// 6D hypercube so that all dimensions and slices remain visible without cutoff.
pub fn collect_alive_positions(grid: &Grid, delta: usize) -> Vec<(usize, usize, usize)> {
    let dims = &grid.dims;
    let size = dims.size;
    let stride = size + delta;
    let s = &dims.size_in_dimensions;
    let s0 = s[0];
    let data = grid.as_slice();
    let alive_count = grid.count_alive();
    let alloc_cap = alive_count.min(MAX_RENDER_INSTANCES);
    let mut positions = Vec::with_capacity(alloc_cap);

    if alloc_cap == 0 {
        return positions;
    }

    let sample_step = if alive_count > MAX_RENDER_INSTANCES {
        ((alive_count + MAX_RENDER_INSTANCES - 1) / MAX_RENDER_INSTANCES).max(1)
    } else {
        1
    };

    let mut alive_counter = 0usize;

    if s0 == 6 && dims.dimensions == 6 && sample_step == 1 {
        // Fast-path for standard 6D GoL (s0 == 6) with 100% display:
        // Inspects 6 bytes with 32-bit + 16-bit unaligned reads, skipping empty rows in 1 instruction
        let mut idx = 0;
        let ptr = data.as_ptr();
        for a in 0..s[5] {
            let z_base = a * stride;
            for b in 0..s[4] {
                let y_base = b * stride;
                for c in 0..s[3] {
                    let x_base = c * stride;
                    for d in 0..s[2] {
                        let x = d + x_base;
                        for e in 0..s[1] {
                            let y = e + y_base;
                            unsafe {
                                let row_ptr = ptr.add(idx);
                                let u4 = std::ptr::read_unaligned(row_ptr as *const u32);
                                let u2 = std::ptr::read_unaligned(row_ptr.add(4) as *const u16);
                                if (u4 | (u2 as u32)) != 0 {
                                    if *row_ptr != 0 {
                                        positions.push((x, y, z_base));
                                    }
                                    if *row_ptr.add(1) != 0 {
                                        positions.push((x, y, z_base + 1));
                                    }
                                    if *row_ptr.add(2) != 0 {
                                        positions.push((x, y, z_base + 2));
                                    }
                                    if *row_ptr.add(3) != 0 {
                                        positions.push((x, y, z_base + 3));
                                    }
                                    if *row_ptr.add(4) != 0 {
                                        positions.push((x, y, z_base + 4));
                                    }
                                    if *row_ptr.add(5) != 0 {
                                        positions.push((x, y, z_base + 5));
                                    }
                                }
                            }
                            idx += 6;
                            if positions.len() >= MAX_RENDER_INSTANCES {
                                return positions;
                            }
                        }
                    }
                }
            }
        }
        return positions;
    }

    // Generic path with uniform sampling across ALL 6 dimensions:
    let mut idx = 0;
    for a in 0..s[5] {
        let z_base = a * stride;
        for b in 0..s[4] {
            let y_base = b * stride;
            for c in 0..s[3] {
                let x_base = c * stride;
                for d in 0..s[2] {
                    let x = d + x_base;
                    for e in 0..s[1] {
                        let y = e + y_base;
                        let row = &data[idx..idx + s0];
                        for (f, &val) in row.iter().enumerate() {
                            if val != 0 {
                                alive_counter += 1;
                                if alive_counter % sample_step == 0 {
                                    positions.push((x, y, z_base + f));
                                    if positions.len() >= MAX_RENDER_INSTANCES {
                                        return positions;
                                    }
                                }
                            }
                        }
                        idx += s0;
                    }
                }
            }
        }
    }

    positions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visual_bounds_matching_java() {
        let size = 6;
        let delta = 3;

        let b1 = VisualBounds::compute(1, size, delta);
        assert_eq!(b1.size_x, 1);
        assert_eq!(b1.size_y, 1);
        assert_eq!(b1.size_z, 6);

        let b2 = VisualBounds::compute(2, size, delta);
        assert_eq!(b2.size_x, 1);
        assert_eq!(b2.size_y, 6);
        assert_eq!(b2.size_z, 6);

        let b3 = VisualBounds::compute(3, size, delta);
        assert_eq!(b3.size_x, 6);
        assert_eq!(b3.size_y, 6);
        assert_eq!(b3.size_z, 6);

        let b4 = VisualBounds::compute(4, size, delta);
        assert_eq!(b4.size_x, 54);
        assert_eq!(b4.size_y, 6);
        assert_eq!(b4.size_z, 6);

        let b5 = VisualBounds::compute(5, size, delta);
        assert_eq!(b5.size_x, 54);
        assert_eq!(b5.size_y, 54);
        assert_eq!(b5.size_z, 6);

        let b6 = VisualBounds::compute(6, size, delta);
        assert_eq!(b6.size_x, 54);
        assert_eq!(b6.size_y, 54);
        assert_eq!(b6.size_z, 54);
    }

    #[test]
    fn test_exact_coordinate_projection() {
        let size = 6;
        let delta = 3;

        assert_eq!(project_coords(0, 0, 0, 0, 0, 0, size, delta), (0, 0, 0));
        assert_eq!(project_coords(1, 2, 3, 4, 5, 0, size, delta), (31, 23, 9));
    }
}
