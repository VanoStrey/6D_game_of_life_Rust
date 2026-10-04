use rand::rngs::StdRng;
use rand::SeedableRng;
use six_d_game_of_life::projection::{collect_alive_positions, VisualBounds};
use six_d_game_of_life::renderer::{create_instances_with_coloring, ColorMode};
use six_d_game_of_life::{Grid, GridDimensions, Rules, Simulation};

#[test]
fn test_edge_size_1_to_40_grid_dimensions() {
    for size in 1..=40 {
        let dims = GridDimensions::new(size, 6);
        assert_eq!(dims.size, size);
        assert_eq!(dims.dimensions, 6);
        assert_eq!(dims.size_in_dimensions, [size; 6]);
        assert_eq!(dims.delta_neighbors, [3; 6]);

        let mut expected_stride = 1usize;
        for k in 0..6 {
            assert_eq!(dims.strides[k], expected_stride);
            expected_stride = expected_stride.saturating_mul(size);
        }
        assert_eq!(dims.total_cells, (size as u128).pow(6) as usize);
    }
}

#[test]
fn test_edge_size_40_1d_to_3d_seq_par_equivalence() {
    let rules = Rules::new(20.0, 45.0);

    // 1D with size 40
    {
        let dims = GridDimensions::new(40, 1);
        assert_eq!(dims.total_cells, 40);
        let mut rng = StdRng::seed_from_u64(11111);
        let mut grid = Grid::new(dims);
        grid.randomize(&mut rng);
        let mut sim_seq = Simulation::from_grid(grid.clone(), rules);
        let mut sim_par = Simulation::from_grid(grid, rules);

        for step in 1..=3 {
            sim_seq.step_seq();
            sim_par.step_par();
            assert_eq!(
                sim_seq.current.as_slice(),
                sim_par.current.as_slice(),
                "1D size 40 mismatch on step {step}"
            );
        }
    }

    // 2D with size 40 (1,600 cells)
    {
        let dims = GridDimensions::new(40, 2);
        assert_eq!(dims.total_cells, 1600);
        let mut rng = StdRng::seed_from_u64(22222);
        let mut grid = Grid::new(dims);
        grid.randomize(&mut rng);
        let mut sim_seq = Simulation::from_grid(grid.clone(), rules);
        let mut sim_par = Simulation::from_grid(grid, rules);

        for step in 1..=3 {
            sim_seq.step_seq();
            sim_par.step_par();
            assert_eq!(
                sim_seq.current.as_slice(),
                sim_par.current.as_slice(),
                "2D size 40 mismatch on step {step}"
            );
        }
    }

    // 3D with size 40 (64,000 cells)
    {
        let dims = GridDimensions::new(40, 3);
        assert_eq!(dims.total_cells, 64_000);
        let mut rng = StdRng::seed_from_u64(33333);
        let mut grid = Grid::new(dims);
        grid.randomize(&mut rng);
        let mut sim_seq = Simulation::from_grid(grid.clone(), rules);
        let mut sim_par = Simulation::from_grid(grid, rules);

        for step in 1..=2 {
            sim_seq.step_seq();
            sim_par.step_par();
            assert_eq!(
                sim_seq.current.as_slice(),
                sim_par.current.as_slice(),
                "3D size 40 mismatch on step {step}"
            );
        }
    }
}

#[test]
fn test_edge_size_40_6d_reconfigure_and_projection() {
    let rules = Rules::default();
    let mut sim = Simulation::new(6, 6, rules);

    // Initial state: populate a small cluster
    sim.current.set(0, 0, 0, 0, 0, 0, true);
    sim.current.set(1, 1, 1, 1, 1, 1, true);

    // Visual bounds for 6D with size 40
    let bounds = VisualBounds::compute(6, 40, 2);
    assert!(bounds.total_volume() > 0);

    // Collect alive positions for 6D
    let positions = collect_alive_positions(&sim.current, 2);
    assert_eq!(positions.len(), 2);

    let instances = create_instances_with_coloring(
        &positions,
        1.0,
        6,
        40,
        2,
        ColorMode::Hyperdimension,
    );
    assert_eq!(instances.len(), 2);
}
