use rand::rngs::StdRng;
use rand::SeedableRng;
use six_d_game_of_life::{step_reference, Grid, GridDimensions, Rules, Simulation};

#[test]
fn test_pbc_randomized_equivalence_all_dimensions_and_sizes() {
    let rules = Rules::default();

    // Test a wide matrix of dimensions (1D to 6D) and sizes (1 to 6)
    for dim in 1..=6 {
        let max_size = match dim {
            1 => 12,
            2 => 8,
            3 => 6,
            4 => 5,
            5 => 4,
            6 => 3,
            _ => 3,
        };

        for size in 1..=max_size {
            let dims = GridDimensions::new(size, dim);
            let mut rng = StdRng::seed_from_u64(1000 + (dim * 100 + size) as u64);

            // Test 3 consecutive generations from a random state
            let mut grid = Grid::new(dims);
            grid.randomize(&mut rng);

            let mut sim_seq = Simulation::from_grid(grid.clone(), rules);
            let mut sim_par = Simulation::from_grid(grid.clone(), rules);
            let mut oracle_grid = grid;
            let mut oracle_next = Grid::new(dims);

            for step in 1..=3 {
                // Oracle step
                step_reference(&oracle_grid, &mut oracle_next, &rules);
                std::mem::swap(&mut oracle_grid, &mut oracle_next);

                // Hardware-accelerated steps
                sim_seq.step_seq();
                sim_par.step_par();

                assert_eq!(
                    sim_seq.current.as_slice(),
                    oracle_grid.as_slice(),
                    "Mismatch in sequential separable engine for dim={dim}, size={size} at generation {step}"
                );
                assert_eq!(
                    sim_par.current.as_slice(),
                    oracle_grid.as_slice(),
                    "Mismatch in parallel separable engine for dim={dim}, size={size} at generation {step}"
                );
            }
        }
    }
}

#[test]
fn test_pbc_empty_grid_across_all_dimensions() {
    // With 50% min, for any D >= 1, min_neighbors >= 1, so 0 neighbors causes all cells to remain dead
    let rules = Rules::new(50.0, 100.0);

    for dim in 1..=6 {
        let dims = GridDimensions::new(4, dim);
        let grid = Grid::new(dims);

        let mut sim = Simulation::from_grid(grid.clone(), rules);
        let oracle = grid;
        let mut next = Grid::new(dims);

        step_reference(&oracle, &mut next, &rules);
        sim.step();

        assert_eq!(
            sim.current.as_slice(),
            next.as_slice(),
            "Empty grid mismatch for dim={dim}"
        );
        assert_eq!(sim.current.count_alive(), 0, "Empty grid should remain empty");
    }
}

#[test]
fn test_pbc_all_ones_grid_across_all_dimensions() {
    let rules = Rules::default();

    for dim in 1..=6 {
        let dims = GridDimensions::new(3, dim);
        let mut grid = Grid::new(dims);
        grid.as_mut_slice().fill(1);

        let mut sim = Simulation::from_grid(grid.clone(), rules);
        let oracle = grid;
        let mut next = Grid::new(dims);

        step_reference(&oracle, &mut next, &rules);
        sim.step();

        assert_eq!(
            sim.current.as_slice(),
            next.as_slice(),
            "All-ones grid mismatch for dim={dim}"
        );
    }
}

#[test]
fn test_pbc_single_corner_cell_across_all_dimensions() {
    let rules = Rules::new(1.0, 100.0);

    for dim in 1..=6 {
        let dims = GridDimensions::new(4, dim);
        let mut grid = Grid::new(dims);
        grid.set_linear(0, true); // Origin corner

        let mut sim_seq = Simulation::from_grid(grid.clone(), rules);
        let mut sim_par = Simulation::from_grid(grid.clone(), rules);
        let oracle = grid;
        let mut next = Grid::new(dims);

        step_reference(&oracle, &mut next, &rules);
        sim_seq.step_seq();
        sim_par.step_par();

        assert_eq!(
            sim_seq.current.as_slice(),
            next.as_slice(),
            "Single corner cell sequential mismatch for dim={dim}"
        );
        assert_eq!(
            sim_par.current.as_slice(),
            next.as_slice(),
            "Single corner cell parallel mismatch for dim={dim}"
        );
    }
}

#[test]
fn test_pbc_6d_size6_multi_step_equivalence() {
    // Heavy test on standard 6D grid: size=6, D=6 (46,656 cells)
    let rules = Rules::default();
    let dims = GridDimensions::new(6, 6);
    let mut rng = StdRng::seed_from_u64(42);

    let mut grid = Grid::new(dims);
    grid.randomize(&mut rng);

    let mut sim_seq = Simulation::from_grid(grid.clone(), rules);
    let mut sim_par = Simulation::from_grid(grid.clone(), rules);
    let mut oracle = grid;
    let mut next = Grid::new(dims);

    // Verify 2 generations on full 6D grid
    for step in 1..=2 {
        step_reference(&oracle, &mut next, &rules);
        std::mem::swap(&mut oracle, &mut next);

        sim_seq.step_seq();
        sim_par.step_par();

        assert_eq!(
            sim_seq.current.as_slice(),
            oracle.as_slice(),
            "Sequential step {step} mismatch on 6D size=6"
        );
        assert_eq!(
            sim_par.current.as_slice(),
            oracle.as_slice(),
            "Parallel step {step} mismatch on 6D size=6"
        );
    }
}
