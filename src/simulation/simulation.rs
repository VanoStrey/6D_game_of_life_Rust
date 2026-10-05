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
    pub periodic: bool,
    pub generation: u64,
    #[serde(skip)]
    aux: Option<SimulationAux>,
}

impl Simulation {
    /// Creates a new simulation with given size, dimensions, and transition rules (defaults to Periodic).
    pub fn new(size: usize, dimensions: usize, rules: Rules) -> Self {
        Self::new_with_topology(size, dimensions, rules, true)
    }

    /// Creates a new simulation with given size, dimensions, rules, and periodic boundary mode.
    pub fn new_with_topology(size: usize, dimensions: usize, rules: Rules, periodic: bool) -> Self {
        let dims = GridDimensions::new(size, dimensions);
        let current = Grid::new(dims);
        let next = Grid::new(dims);
        let aux = Some(SimulationAux::new_with_mode(&dims, &rules, periodic));
        Self {
            current,
            next,
            rules,
            periodic,
            generation: 0,
            aux,
        }
    }

    /// Creates a simulation from an initial grid state (defaults to Periodic).
    pub fn from_grid(grid: Grid, rules: Rules) -> Self {
        Self::from_grid_with_topology(grid, rules, true)
    }

    /// Creates a simulation from an initial grid state and periodic boundary mode.
    pub fn from_grid_with_topology(grid: Grid, rules: Rules, periodic: bool) -> Self {
        let next = Grid::new(grid.dims);
        let aux = Some(SimulationAux::new_with_mode(&grid.dims, &rules, periodic));
        Self {
            current: grid,
            next,
            rules,
            periodic,
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
    pub fn reconfigure(&mut self, new_size: usize, new_dimensions: usize, new_rules: Rules) {
        self.reconfigure_with_topology(new_size, new_dimensions, new_rules, self.periodic);
    }

    /// Reconfigures simulation with new size, dimensions, transition rules, and periodic boundary mode.
    ///
    /// Preserves all existing alive cells that fit within the new grid dimensions,
    /// ensuring state is never unexpectedly randomized upon parameter adjustments.
    pub fn reconfigure_with_topology(
        &mut self,
        new_size: usize,
        new_dimensions: usize,
        new_rules: Rules,
        periodic: bool,
    ) {
        self.rules = new_rules;
        self.periodic = periodic;

        let new_dims = GridDimensions::new(new_size, new_dimensions);
        if self.current.dims == new_dims {
            // Rules updated, grid geometry identical: keep existing state 100%!
            if let Some(aux) = &mut self.aux {
                aux.update_rules(&new_dims, &self.rules, self.periodic);
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
        self.aux = Some(SimulationAux::new_with_mode(&new_dims, &self.rules, self.periodic));
    }

    /// Sets periodic boundary conditions mode on/off.
    pub fn set_periodic(&mut self, periodic: bool) {
        if self.periodic != periodic {
            self.periodic = periodic;
            if let Some(aux) = &mut self.aux {
                aux.update_rules(&self.current.dims, &self.rules, periodic);
            }
        }
    }

    /// Computes next state of a single cell using pure Moore neighborhood.
    #[inline(always)]
    pub fn compute_cell_state(&self, idx: usize) -> u8 {
        compute_cell_state_reference_with_topology(&self.current, &self.rules, idx, self.periodic)
    }

    /// Performs one sequential step of simulation using the reference oracle (for testing).
    pub fn step_reference(&mut self) {
        step_reference_with_topology(&self.current, &mut self.next, &self.rules, self.periodic);
        std::mem::swap(&mut self.current, &mut self.next);
        self.generation += 1;
    }

    /// Performs one sequential step of simulation using hardware SIMD separable convolution.
    pub fn step_seq(&mut self) {
        if self.aux.is_none() {
            self.aux = Some(SimulationAux::new_with_mode(&self.current.dims, &self.rules, self.periodic));
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
            self.aux = Some(SimulationAux::new_with_mode(&self.current.dims, &self.rules, self.periodic));
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

/// Reference oracle: computes next cell state on n-dimensional torus T^D (periodic).
#[inline(always)]
pub fn compute_cell_state_reference(grid: &Grid, rules: &Rules, idx: usize) -> u8 {
    compute_cell_state_reference_with_topology(grid, rules, idx, true)
}

/// Reference oracle: computes next cell state for either Periodic or Bounded topology.
///
/// Directly iterates all 3^D kernel positions {-1, 0, 1}^D, skipping (0,...,0).
/// When periodic, wraps each coordinate periodically.
/// When bounded, only positions that fall strictly inside [0, size-1] count.
pub fn compute_cell_state_reference_with_topology(
    grid: &Grid,
    rules: &Rules,
    idx: usize,
    periodic: bool,
) -> u8 {
    let dims = &grid.dims;
    let d = dims.dimensions;
    let mut coords = [0usize; 6];
    dims.coords_nd(idx, &mut coords);

    let mut count_live = 0i32;
    let mut count_all = 0i32;
    let total_kernel_positions = 3usize.pow(d as u32);

    for p in 0..total_kernel_positions {
        let mut temp = p;
        let mut is_center = true;
        let mut n_coords = [0usize; 6];
        let mut in_bounds = true;

        for k in 0..d {
            let delta = (temp % 3) as isize - 1; // -1, 0, +1
            temp /= 3;
            if delta != 0 {
                is_center = false;
            }
            let sk = dims.size_in_dimensions[k] as isize;
            let nc = coords[k] as isize + delta;
            if periodic {
                n_coords[k] = nc.rem_euclid(sk) as usize;
            } else if nc >= 0 && nc < sk {
                n_coords[k] = nc as usize;
            } else {
                in_bounds = false;
            }
        }

        if is_center {
            continue; // Skip center cell (0, ..., 0)
        }

        if periodic || in_bounds {
            count_all += 1;
            let n_idx = dims.linear_index_nd(&n_coords[..d]);
            if grid.get_raw(n_idx) != 0 {
                count_live += 1;
            }
        }
    }

    if rules.evaluate(count_live, count_all) {
        1
    } else {
        0
    }
}

/// Steps the entire grid using the reference implementation (used as test oracle).
pub fn step_reference(current: &Grid, next: &mut Grid, rules: &Rules) {
    step_reference_with_topology(current, next, rules, true);
}

/// Steps the entire grid using the reference implementation with specified topology.
pub fn step_reference_with_topology(
    current: &Grid,
    next: &mut Grid,
    rules: &Rules,
    periodic: bool,
) {
    for idx in 0..current.len() {
        let state = compute_cell_state_reference_with_topology(current, rules, idx, periodic);
        next.set_raw(idx, state);
    }
}
