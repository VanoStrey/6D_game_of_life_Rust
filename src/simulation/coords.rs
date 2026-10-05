//! Multi-dimensional coordinate transformations and index mappings.
//!
//! Replicates the Java coordinate system from `LogicGameOfLive.java`.
//! In Java:
//! - Array definition: `boolean[a][b][c][d][e][f]`
//! - Index 0 (`f`): dimension 0, active if dimensions >= 1, size = `size`
//! - Index 1 (`e`): dimension 1, active if dimensions >= 2, size = `size` (else 1)
//! - Index 2 (`d`): dimension 2, active if dimensions >= 3, size = `size` (else 1)
//! - Index 3 (`c`): dimension 3, active if dimensions >= 4, size = `size` (else 1)
//! - Index 4 (`b`): dimension 4, active if dimensions >= 5, size = `size` (else 1)
//! - Index 5 (`a`): dimension 5, active if dimensions == 6, size = `size` (else 1)

use serde::{Deserialize, Serialize};

/// Maximum supported dimensionality in 6D Game of Life.
pub const MAX_DIMENSIONS: usize = 6;

/// 6D Coordinates tuple corresponding to `(a, b, c, d, e, f)` in Java.
pub type Coords6D = (usize, usize, usize, usize, usize, usize);

/// Dimensions configuration and strides calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridDimensions {
    pub size: usize,
    pub dimensions: usize,
    pub size_in_dimensions: [usize; 6],
    pub delta_neighbors: [usize; 6],
    pub strides: [usize; 6],
    pub total_cells: usize,
}

impl GridDimensions {
    /// Creates a new `GridDimensions` following the exact Java constructor rules:
    /// if `dimensions <= 6 && dimensions > 0` then `dimensions` is kept, otherwise it defaults to 6.
    /// Supports edge size (длина ребра) from 1 up to 40 and beyond.
    pub fn new(size: usize, dimensions: usize) -> Self {
        let eff_dims = if (1..=6).contains(&dimensions) {
            dimensions
        } else {
            6
        };

        let mut size_in_dimensions = [1usize; 6];
        let mut delta_neighbors = [1usize; 6];

        for i in 0..eff_dims {
            size_in_dimensions[i] = size;
            delta_neighbors[i] = 3;
        }
        for i in eff_dims..6 {
            size_in_dimensions[i] = 1;
            delta_neighbors[i] = 1;
        }

        // Strides in row-major order:
        // linear_index = f + e*S0 + d*(S0*S1) + c*(S0*S1*S2) + b*(S0*S1*S2*S3) + a*(S0*S1*S2*S3*S4)
        let mut strides = [0usize; 6];
        strides[0] = 1;
        for i in 1..6 {
            strides[i] = strides[i - 1].saturating_mul(size_in_dimensions[i - 1]);
        }

        let total_cells = strides[5].saturating_mul(size_in_dimensions[5]);

        Self {
            size,
            dimensions: eff_dims,
            size_in_dimensions,
            delta_neighbors,
            strides,
            total_cells,
        }
    }

    /// Converts 6D coordinates `(a, b, c, d, e, f)` to a linear 1D index.
    #[inline(always)]
    pub fn linear_index(
        &self,
        a: usize,
        b: usize,
        c: usize,
        d: usize,
        e: usize,
        f: usize,
    ) -> usize {
        f * self.strides[0]
            + e * self.strides[1]
            + d * self.strides[2]
            + c * self.strides[3]
            + b * self.strides[4]
            + a * self.strides[5]
    }

    /// Converts an N-dimensional coordinate slice to a linear 1D index.
    #[inline(always)]
    pub fn linear_index_nd(&self, coords: &[usize]) -> usize {
        let mut idx = 0usize;
        for (dim, &c) in coords.iter().enumerate().take(self.dimensions) {
            idx += c * self.strides[dim];
        }
        idx
    }

    /// Converts a linear 1D index back to 6D coordinates `(a, b, c, d, e, f)`.
    #[inline(always)]
    pub fn coords_from_index(&self, idx: usize) -> Coords6D {
        let f = (idx / self.strides[0]) % self.size_in_dimensions[0];
        let e = (idx / self.strides[1]) % self.size_in_dimensions[1];
        let d = (idx / self.strides[2]) % self.size_in_dimensions[2];
        let c = (idx / self.strides[3]) % self.size_in_dimensions[3];
        let b = (idx / self.strides[4]) % self.size_in_dimensions[4];
        let a = (idx / self.strides[5]) % self.size_in_dimensions[5];

        (a, b, c, d, e, f)
    }

    /// Converts a linear 1D index to N-dimensional coordinates into an array buffer.
    #[inline(always)]
    pub fn coords_nd(&self, idx: usize, out: &mut [usize; MAX_DIMENSIONS]) {
        for dim in 0..self.dimensions {
            out[dim] = (idx / self.strides[dim]) % self.size_in_dimensions[dim];
        }
        for dim in self.dimensions..MAX_DIMENSIONS {
            out[dim] = 0;
        }
    }

    /// Retrieves coordinate along a specific dimension `dim`.
    #[inline(always)]
    pub fn coord_at(&self, idx: usize, dim: usize) -> usize {
        if dim < self.dimensions && self.strides[dim] > 0 {
            (idx / self.strides[dim]) % self.size_in_dimensions[dim]
        } else {
            0
        }
    }

    /// Checks if the signed 6D coordinates are within the grid bounds.
    #[inline(always)]
    pub fn is_in_bounds(&self, a: isize, b: isize, c: isize, d: isize, e: isize, f: isize) -> bool {
        a >= 0
            && (a as usize) < self.size_in_dimensions[5]
            && b >= 0
            && (b as usize) < self.size_in_dimensions[4]
            && c >= 0
            && (c as usize) < self.size_in_dimensions[3]
            && d >= 0
            && (d as usize) < self.size_in_dimensions[2]
            && e >= 0
            && (e as usize) < self.size_in_dimensions[1]
            && f >= 0
            && (f as usize) < self.size_in_dimensions[0]
    }

    /// Checks if the signed N-dimensional coordinates are within the grid bounds.
    #[inline(always)]
    pub fn is_in_bounds_nd(&self, coords: &[isize]) -> bool {
        for (dim, &c) in coords.iter().enumerate().take(self.dimensions) {
            if c < 0 || (c as usize) >= self.size_in_dimensions[dim] {
                return false;
            }
        }
        true
    }

    /// Wraps 6D signed coordinates periodically according to the torus topology T^D.
    #[inline(always)]
    pub fn wrap_coords_6d(&self, a: isize, b: isize, c: isize, d: isize, e: isize, f: isize) -> Coords6D {
        (
            wrap_coord(a, self.size_in_dimensions[5]),
            wrap_coord(b, self.size_in_dimensions[4]),
            wrap_coord(c, self.size_in_dimensions[3]),
            wrap_coord(d, self.size_in_dimensions[2]),
            wrap_coord(e, self.size_in_dimensions[1]),
            wrap_coord(f, self.size_in_dimensions[0]),
        )
    }

    /// Wraps N-dimensional signed coordinates periodically according to the torus topology T^D.
    #[inline(always)]
    pub fn wrap_coords_nd(&self, in_coords: &[isize], out_coords: &mut [usize]) {
        for (dim, &c) in in_coords.iter().enumerate().take(self.dimensions) {
            out_coords[dim] = wrap_coord(c, self.size_in_dimensions[dim]);
        }
    }
}

/// Wraps a signed coordinate into the range `[0, size)` using Euclidean modulo.
///
/// Mathematical properties for Periodic Boundary Conditions (PBC):
/// - `wrap_coord(-1, size) == size - 1`
/// - `wrap_coord(size, size) == 0`
/// - `wrap_coord(0, size) == 0`
#[inline(always)]
pub fn wrap_coord(coord: isize, size: usize) -> usize {
    debug_assert!(size > 0);
    coord.rem_euclid(size as isize) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_dimensions_1d() {
        let dims = GridDimensions::new(10, 1);
        assert_eq!(dims.dimensions, 1);
        assert_eq!(&dims.size_in_dimensions[0..6], &[10, 1, 1, 1, 1, 1]);
        assert_eq!(dims.total_cells, 10);
        assert_eq!(dims.linear_index(0, 0, 0, 0, 0, 5), 5);
        assert_eq!(dims.coords_from_index(5), (0, 0, 0, 0, 0, 5));
    }

    #[test]
    fn test_grid_dimensions_3d() {
        let dims = GridDimensions::new(4, 3);
        assert_eq!(dims.dimensions, 3);
        assert_eq!(&dims.size_in_dimensions[0..6], &[4, 4, 4, 1, 1, 1]);
        assert_eq!(dims.total_cells, 64);

        // Origin
        assert_eq!(dims.linear_index(0, 0, 0, 0, 0, 0), 0);
        assert_eq!(dims.coords_from_index(0), (0, 0, 0, 0, 0, 0));

        // Max corner (d=3, e=3, f=3)
        let max_idx = dims.linear_index(0, 0, 0, 3, 3, 3);
        assert_eq!(max_idx, 63);
        assert_eq!(dims.coords_from_index(63), (0, 0, 0, 3, 3, 3));
    }

    #[test]
    fn test_grid_dimensions_6d() {
        let dims = GridDimensions::new(3, 6);
        assert_eq!(dims.dimensions, 6);
        assert_eq!(&dims.size_in_dimensions[0..6], &[3, 3, 3, 3, 3, 3]);
        assert_eq!(dims.total_cells, 729);

        let max_idx = dims.linear_index(2, 2, 2, 2, 2, 2);
        assert_eq!(max_idx, 728);
        assert_eq!(dims.coords_from_index(728), (2, 2, 2, 2, 2, 2));

        // Roundtrip for all cells
        for idx in 0..dims.total_cells {
            let (a, b, c, d, e, f) = dims.coords_from_index(idx);
            assert_eq!(dims.linear_index(a, b, c, d, e, f), idx);
        }
    }

    #[test]
    fn test_grid_dimensions_size_40_6d() {
        let dims = GridDimensions::new(40, 6);
        assert_eq!(dims.dimensions, 6);
        assert_eq!(dims.size, 40);
        assert_eq!(dims.size_in_dimensions, [40, 40, 40, 40, 40, 40]);
        assert_eq!(dims.total_cells, 4_096_000_000);
        assert_eq!(dims.strides[0], 1);
        assert_eq!(dims.strides[1], 40);
        assert_eq!(dims.strides[2], 1600);
        assert_eq!(dims.strides[3], 64000);
        assert_eq!(dims.strides[4], 2560000);
        assert_eq!(dims.strides[5], 102400000);
    }

    #[test]
    fn test_fallback_dimensions() {
        assert_eq!(GridDimensions::new(5, 0).dimensions, 6);
        assert_eq!(GridDimensions::new(5, 7).dimensions, 6);
    }

    #[test]
    fn test_periodic_wrapping() {
        let size = 6;
        assert_eq!(wrap_coord(-1, size), 5);
        assert_eq!(wrap_coord(0, size), 0);
        assert_eq!(wrap_coord(5, size), 5);
        assert_eq!(wrap_coord(6, size), 0);
        assert_eq!(wrap_coord(7, size), 1);
        assert_eq!(wrap_coord(-6, size), 0);
        assert_eq!(wrap_coord(-7, size), 5);

        // size = 1
        assert_eq!(wrap_coord(-1, 1), 0);
        assert_eq!(wrap_coord(0, 1), 0);
        assert_eq!(wrap_coord(1, 1), 0);

        // size = 2
        assert_eq!(wrap_coord(-1, 2), 1);
        assert_eq!(wrap_coord(0, 2), 0);
        assert_eq!(wrap_coord(1, 2), 1);
        assert_eq!(wrap_coord(2, 2), 0);

        let dims = GridDimensions::new(6, 6);
        let wrapped = dims.wrap_coords_6d(-1, 0, 5, 6, -6, 7);
        assert_eq!(wrapped, (5, 0, 5, 0, 0, 1));
    }
}
