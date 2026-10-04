//! Contiguous linear grid storage for multi-dimensional cellular automaton.

use super::coords::GridDimensions;
use rand::Rng;
use serde::{Deserialize, Serialize};

/// High-performance contiguous byte grid.
///
/// Stores cell states in a single flat `Vec<u8>` where `0` is dead and `1` is alive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grid {
    pub dims: GridDimensions,
    pub data: Vec<u8>,
}

impl Grid {
    /// Creates a new grid with all cells initialized to dead (0).
    pub fn new(dims: GridDimensions) -> Self {
        let data = vec![0u8; dims.total_cells];
        Self { dims, data }
    }

    /// Creates a grid from existing data with dimension validation.
    pub fn from_vec(dims: GridDimensions, data: Vec<u8>) -> Self {
        assert_eq!(
            data.len(),
            dims.total_cells,
            "Data length does not match grid dimensions total cells"
        );
        Self { dims, data }
    }

    /// Returns the cell state at linear index.
    #[inline(always)]
    pub fn get_linear(&self, idx: usize) -> bool {
        self.data[idx] != 0
    }

    /// Sets the cell state at linear index.
    #[inline(always)]
    pub fn set_linear(&mut self, idx: usize, val: bool) {
        self.data[idx] = if val { 1 } else { 0 };
    }

    /// Gets raw byte state at linear index (0 or 1).
    #[inline(always)]
    pub fn get_raw(&self, idx: usize) -> u8 {
        self.data[idx]
    }

    /// Sets raw byte state at linear index.
    #[inline(always)]
    pub fn set_raw(&mut self, idx: usize, val: u8) {
        self.data[idx] = val;
    }

    /// Gets cell state using 6D coordinates `(a, b, c, d, e, f)`.
    #[inline(always)]
    pub fn get(&self, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> bool {
        let idx = self.dims.linear_index(a, b, c, d, e, f);
        self.get_linear(idx)
    }

    /// Sets cell state using 6D coordinates `(a, b, c, d, e, f)`.
    #[inline(always)]
    pub fn set(&mut self, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize, val: bool) {
        let idx = self.dims.linear_index(a, b, c, d, e, f);
        self.set_linear(idx, val);
    }

    /// Returns the total number of cells in the grid.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns true if grid has zero cells.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns a reference to the underlying byte slice.
    #[inline(always)]
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    /// Returns a mutable reference to the underlying byte slice.
    #[inline(always)]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Counts the total number of alive cells in the grid using hardware SIMD acceleration.
    #[inline(always)]
    pub fn count_alive(&self) -> usize {
        super::simd_ops::count_alive_simd(&self.data)
    }

    /// Clears all cells to dead (0).
    pub fn clear(&mut self) {
        self.data.fill(0);
    }

    /// Randomizes grid cells with uniform 50% probability (replicates Java `Random.nextBoolean()`).
    pub fn randomize<R: Rng>(&mut self, rng: &mut R) {
        for byte in self.data.iter_mut() {
            *byte = if rng.gen_bool(0.5) { 1 } else { 0 };
        }
    }
}
