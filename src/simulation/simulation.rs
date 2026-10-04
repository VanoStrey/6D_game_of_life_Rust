use super::coords::GridDimensions;
use super::engine::{step_separable, SimulationAux};
use super::grid::Grid;
use super::rules::Rules;
use rand::Rng;
use serde::{Deserialize, Serialize};

/// High-performance multi-dimensional cellular automaton simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Simulation {
    pub current: Grid,
    pub next: Grid,
    pub rules: Rules,
    pub generation: u64,
    #[serde(skip)]
    aux: Option<SimulationAux>,
}

impl Simulation {
    /// Creates a new simulation with given size, dimensions, and transition rules.
    pub fn new(size: usize, dimensions: usize, rules: Rules) -> Self {
        let dims = GridDimensions::new(size, dimensions);
        let current = Grid::new(dims);
        let next = Grid::new(dims);
        let aux = Some(SimulationAux::new(&dims, &rules));
        Self {
            current,
            next,
            rules,
            generation: 0,
            aux,
        }
    }

    /// Creates a simulation from an initial grid state.
    pub fn from_grid(grid: Grid, rules: Rules) -> Self {
        let next = Grid::new(grid.dims);
        let aux = Some(SimulationAux::new(&grid.dims, &rules));
        Self {
            current: grid,
            next,
            rules,
            generation: 0,
            aux,
        }
    }

    /// Randomizes the current grid cells using uniform distribution.
    pub fn randomize<R: Rng>(&mut self, rng: &mut R) {
        self.current.randomize(rng);
        self.generation = 0;
    }

    /// Reconfigures simulation with new size, dimensions, and transition rules.
    ///
    /// Preserves all existing alive cells that fit within the new grid dimensions,
    /// ensuring state is never unexpectedly randomized upon parameter adjustments.
    pub fn reconfigure(&mut self, new_size: usize, new_dimensions: usize, new_rules: Rules) {
        self.rules = new_rules;

        let new_dims = GridDimensions::new(new_size, new_dimensions);
        if self.current.dims == new_dims {
            // Rules updated, grid geometry identical: keep existing state 100%!
            if let Some(aux) = &mut self.aux {
                aux.update_rules(&self.rules);
            }
            return;
        }

        let mut new_current = Grid::new(new_dims);
        let cur_dims = self.current.dims;
        let mut coords = [0usize; 6];
        for idx in 0..self.current.len() {
            if self.current.get_linear(idx) {
                cur_dims.coords_nd(idx, &mut coords);
                let mut fits = true;
                for dim in 0..cur_dims.dimensions {
                    if coords[dim] >= new_dims.size_in_dimensions[dim] {
                        fits = false;
                        break;
                    }
                }
                if fits {
                    let mut new_idx = 0usize;
                    for dim in 0..new_dims.dimensions {
                        new_idx += coords[dim] * new_dims.strides[dim];
                    }
                    if new_idx < new_current.len() {
                        new_current.set_linear(new_idx, true);
                    }
                }
            }
        }

        self.current = new_current;
        self.next = Grid::new(new_dims);
        self.aux = Some(SimulationAux::new(&new_dims, &self.rules));
    }

    /// Computes next state of a single cell using exact Java neighborhood logic.
    ///
    /// Preserves Java defect:
    /// - `a1 + b1 + c1 + d1 + e1 + f1 == 0` discards `(-1, -1, -1, -1, -1, -1)`.
    /// - Central cell `(0, 0, 0, 0, 0, 0)` gives non-zero sum and is included as neighbor.
    #[inline(always)]
    pub fn compute_cell_state(&self, idx: usize) -> u8 {
        let dims = &self.current.dims;
        let d = dims.dimensions;

        if d <= 6 {
            let (a, b, c, d_coord, e, f) = dims.coords_from_index(idx);

            let dn = &dims.delta_neighbors;
            let sn = &dims.size_in_dimensions;

            let mut count_live = 0i32;
            let mut count_die = 0i32;

            for a1 in 0..dn[5] {
                for b1 in 0..dn[4] {
                    for c1 in 0..dn[3] {
                        for d1 in 0..dn[2] {
                            for e1 in 0..dn[1] {
                                for f1 in 0..dn[0] {
                                    // EXACT Java check in `formatCounters`:
                                    // if (a1 + b1 + c1 + d1 + e1 + f1 == 0) return null;
                                    if a1 + b1 + c1 + d1 + e1 + f1 == 0 {
                                        continue;
                                    }

                                    let na = if dn[5] == 3 {
                                        a as isize - 1 + a1 as isize
                                    } else {
                                        0
                                    };
                                    let nb = if dn[4] == 3 {
                                        b as isize - 1 + b1 as isize
                                    } else {
                                        0
                                    };
                                    let nc = if dn[3] == 3 {
                                        c as isize - 1 + c1 as isize
                                    } else {
                                        0
                                    };
                                    let nd = if dn[2] == 3 {
                                        d_coord as isize - 1 + d1 as isize
                                    } else {
                                        0
                                    };
                                    let ne = if dn[1] == 3 {
                                        e as isize - 1 + e1 as isize
                                    } else {
                                        0
                                    };
                                    let nf = if dn[0] == 3 {
                                        f as isize - 1 + f1 as isize
                                    } else {
                                        0
                                    };

                                    if na >= 0
                                        && (na as usize) < sn[5]
                                        && nb >= 0
                                        && (nb as usize) < sn[4]
                                        && nc >= 0
                                        && (nc as usize) < sn[3]
                                        && nd >= 0
                                        && (nd as usize) < sn[2]
                                        && ne >= 0
                                        && (ne as usize) < sn[1]
                                        && nf >= 0
                                        && (nf as usize) < sn[0]
                                    {
                                        let n_idx = dims.linear_index(
                                            na as usize,
                                            nb as usize,
                                            nc as usize,
                                            nd as usize,
                                            ne as usize,
                                            nf as usize,
                                        );
                                        if self.current.get_raw(n_idx) != 0 {
                                            count_live += 1;
                                        } else {
                                            count_die += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let count_all = count_live + count_die;
            if self.rules.evaluate(count_live, count_all) {
                1
            } else {
                0
            }
        } else {
            // General N-dimensional neighbor evaluation
            let mut center_coords = [0usize; 6];
            dims.coords_nd(idx, &mut center_coords);

            let mut count_live = 0i32;
            let mut count_die = 0i32;

            let mut deltas = [0usize; 6];
            let mut dim_idx = 0;
            let mut delta_sum = 0usize;

            loop {
                if dim_idx == d {
                    if delta_sum > 0 {
                        let mut in_bounds = true;
                        let mut n_idx = 0usize;
                        for k in 0..d {
                            let nk = if dims.delta_neighbors[k] == 3 {
                                (center_coords[k] as isize) - 1 + (deltas[k] as isize)
                            } else {
                                0
                            };
                            if nk < 0 || (nk as usize) >= dims.size_in_dimensions[k] {
                                in_bounds = false;
                                break;
                            }
                            n_idx += (nk as usize) * dims.strides[k];
                        }
                        if in_bounds {
                            if self.current.get_raw(n_idx) != 0 {
                                count_live += 1;
                            } else {
                                count_die += 1;
                            }
                        }
                    }

                    while dim_idx > 0 {
                        dim_idx -= 1;
                        delta_sum -= deltas[dim_idx];
                        deltas[dim_idx] += 1;
                        if deltas[dim_idx] < dims.delta_neighbors[dim_idx] {
                            delta_sum += deltas[dim_idx];
                            dim_idx += 1;
                            break;
                        }
                        deltas[dim_idx] = 0;
                    }
                    if dim_idx == 0 && deltas[0] == 0 {
                        break;
                    }
                } else {
                    deltas[dim_idx] = 0;
                    dim_idx += 1;
                }
            }

            let count_all = count_live + count_die;
            if self.rules.evaluate(count_live, count_all) {
                1
            } else {
                0
            }
        }
    }

    /// Performs one sequential step of simulation using hardware SIMD separable convolution.
    pub fn step_seq(&mut self) {
        if self.aux.is_none() {
            self.aux = Some(SimulationAux::new(&self.current.dims, &self.rules));
        }
        let dims = self.current.dims;
        let aux = self.aux.as_mut().unwrap();
        step_separable(
            self.current.as_slice(),
            self.next.as_mut_slice(),
            &dims,
            aux,
            false,
        );
        std::mem::swap(&mut self.current, &mut self.next);
        self.generation += 1;
    }

    /// Performs one parallelized step of simulation using multi-threaded SIMD separable convolution.
    pub fn step_par(&mut self) {
        if self.aux.is_none() {
            self.aux = Some(SimulationAux::new(&self.current.dims, &self.rules));
        }
        let dims = self.current.dims;
        let aux = self.aux.as_mut().unwrap();
        step_separable(
            self.current.as_slice(),
            self.next.as_mut_slice(),
            &dims,
            aux,
            true,
        );
        std::mem::swap(&mut self.current, &mut self.next);
        self.generation += 1;
    }

    /// Performs one simulation step, automatically selecting parallel execution for grids with >= 65536 cells.
    #[inline(always)]
    pub fn step(&mut self) {
        if self.current.len() >= 65_536 {
            self.step_par();
        } else {
            self.step_seq();
        }
    }

    /// Counts alive cells in current generation.
    pub fn count_alive(&self) -> usize {
        self.current.count_alive()
    }
}
