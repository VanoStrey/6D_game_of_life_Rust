use rand::rngs::StdRng;
use rand::SeedableRng;
use six_d_game_of_life::{
    compute_cell_state_reference_with_topology, step_reference_with_topology, Camera,
    Grid, GridDimensions, Rules, Simulation, SimulationWorker, VisualBounds,
};

#[test]
fn test_bounded_vs_periodic_boundary_wrap_distinction() {
    let rules = Rules::new(35.0, 55.0);
    let dims = GridDimensions::new(6, 2);

    // Three alive cells at opposite boundaries: (0, 0), (0, 5), (5, 0)
    let mut grid = Grid::new(dims);
    grid.set(0, 0, 0, 0, 0, 0, true);
    grid.set(0, 0, 0, 0, 5, 0, true);
    grid.set(0, 0, 0, 0, 0, 5, true);

    // Target cell at corner (x=5, y=5)
    let idx_5_5 = dims.linear_index(0, 0, 0, 0, 5, 5);

    // In periodic mode:
    // (5, 5) has 8 neighbors. Cells (0,0), (0,5), (5,0) all wrap into (5,5)'s Moore neighborhood!
    // count_live = 3. For 35%-55% of 8, min=2, max=4. 3 is within [2, 4] -> alive (1)!
    let state_pbc = compute_cell_state_reference_with_topology(&grid, &rules, idx_5_5, true);
    assert_eq!(
        state_pbc, 1,
        "In periodic mode (PBC), boundary cells wrap around and keep (5, 5) alive"
    );

    // In bounded mode:
    // None of (0,0), (0,5), (5,0) are adjacent to (5,5) because boundaries do not wrap!
    // (5, 5) has count_live = 0. For in-bounds count_all=3, min=(3*0.35)=1. 0 < 1 -> dead (0)!
    let state_bounded = compute_cell_state_reference_with_topology(&grid, &rules, idx_5_5, false);
    assert_eq!(
        state_bounded, 0,
        "In bounded mode, boundaries do not wrap, so (5, 5) has 0 live neighbors and dies"
    );

    // Verify neighbor count directly with PBC wrap vs Bounded
    let mut pbc_neighbors = 0;
    let mut bounded_neighbors = 0;
    for offset_y in [-1isize, 0, 1] {
        for offset_x in [-1isize, 0, 1] {
            if offset_x == 0 && offset_y == 0 {
                continue;
            }
            // PBC wrap
            let px = (5 + offset_x).rem_euclid(6) as usize;
            let py = (5 + offset_y).rem_euclid(6) as usize;
            if grid.get(0, 0, 0, 0, py, px) {
                pbc_neighbors += 1;
            }

            // Bounded check
            let bx = 5 + offset_x;
            let by = 5 + offset_y;
            if (0..6).contains(&bx) && (0..6).contains(&by) && grid.get(0, 0, 0, 0, by as usize, bx as usize) {
                bounded_neighbors += 1;
            }
        }
    }
    assert_eq!(pbc_neighbors, 3, "PBC should wrap all 3 opposite boundary cells to (5,5)");
    assert_eq!(bounded_neighbors, 0, "Bounded mode should NOT wrap to (5,5)");
}

#[test]
fn test_bounded_mode_separable_equivalence_across_dimensions() {
    let rules = Rules::default();

    for dim in 1..=6 {
        let max_size = match dim {
            1 => 12,
            2 => 8,
            3 => 6,
            4 => 4,
            5 => 3,
            6 => 3,
            _ => 3,
        };

        for size in [2, 3, max_size] {
            let dims = GridDimensions::new(size, dim);
            let mut rng = StdRng::seed_from_u64(9999 + (dim * 100 + size) as u64);

            let mut grid = Grid::new(dims);
            grid.randomize(&mut rng);

            let mut sim_bounded = Simulation::from_grid_with_topology(grid.clone(), rules, false);
            let mut oracle_grid = grid;
            let mut oracle_next = Grid::new(dims);

            for step in 1..=2 {
                step_reference_with_topology(&oracle_grid, &mut oracle_next, &rules, false);
                std::mem::swap(&mut oracle_grid, &mut oracle_next);

                sim_bounded.step();

                assert_eq!(
                    sim_bounded.current.as_slice(),
                    oracle_grid.as_slice(),
                    "Bounded mode separable simulation mismatch for dim={dim}, size={size} at generation {step}"
                );
            }
        }
    }
}

#[test]
fn test_worker_topology_toggle() {
    let epoch = 1;
    let size = 6;
    let dimensions = 3;
    let delta = 2;
    let rules = Rules::default();

    let mut worker = SimulationWorker::new_with_topology(
        epoch,
        size,
        dimensions,
        delta,
        rules,
        true, // start periodic
        Some(42),
        10.0,
    );

    let snap0 = worker.try_recv_snapshot();
    // Allow worker to initialize
    std::thread::sleep(std::time::Duration::from_millis(50));
    let initial = worker.try_recv_snapshot().unwrap_or_else(|_| snap0.unwrap());
    assert_eq!(initial.generation, 0);

    // Reconfigure to bounded mode with periodic = false, preserving alive cells (seed = None)
    let epoch2 = 2;
    worker.reconfigure_with_topology(epoch2, size, dimensions, delta, 20.0, 45.0, false, None);

    std::thread::sleep(std::time::Duration::from_millis(50));
    let snap_reconf = worker.try_recv_snapshot().expect("Snapshot after reconfigure to bounded");
    assert_eq!(snap_reconf.epoch, epoch2);
    assert_eq!(snap_reconf.alive_count, initial.alive_count, "Alive cells preserved on toggle");

    worker.shutdown();
}

#[test]
fn test_camera_distance_invariance_on_dimension_change() {
    const CUBE_SIZE: f32 = 100.0;

    // Simulate switching dimensions from 6D to 3D and 1D
    let bounds_6d = VisualBounds::compute(6, 6, 3);
    let mut camera = Camera::default();
    camera.frame_bounds((bounds_6d.size_x, bounds_6d.size_y, bounds_6d.size_z), CUBE_SIZE);

    let center_6d = glam::Vec3::new(
        (bounds_6d.size_x as f32) * CUBE_SIZE * 0.5,
        (bounds_6d.size_y as f32) * CUBE_SIZE * 0.5,
        (bounds_6d.size_z as f32) * CUBE_SIZE * 0.5,
    );
    let dist_6d = (camera.position - center_6d).length();

    // Now switch to 3D: bounds change, shift camera position by center offset
    let bounds_3d = VisualBounds::compute(3, 6, 3);
    let center_3d = glam::Vec3::new(
        (bounds_3d.size_x as f32) * CUBE_SIZE * 0.5,
        (bounds_3d.size_y as f32) * CUBE_SIZE * 0.5,
        (bounds_3d.size_z as f32) * CUBE_SIZE * 0.5,
    );
    let offset_3d = center_3d - center_6d;
    camera.position += offset_3d;

    let dist_3d = (camera.position - center_3d).length();
    assert!(
        (dist_3d - dist_6d).abs() < 1e-3,
        "Camera distance to center must remain invariant when switching to 3D, keeping single cell visual size unchanged! dist_6d={dist_6d}, dist_3d={dist_3d}"
    );

    // Switch to 1D: bounds change, shift camera position by center offset
    let bounds_1d = VisualBounds::compute(1, 6, 3);
    let center_1d = glam::Vec3::new(
        (bounds_1d.size_x as f32) * CUBE_SIZE * 0.5,
        (bounds_1d.size_y as f32) * CUBE_SIZE * 0.5,
        (bounds_1d.size_z as f32) * CUBE_SIZE * 0.5,
    );
    let offset_1d = center_1d - center_3d;
    camera.position += offset_1d;

    let dist_1d = (camera.position - center_1d).length();
    assert!(
        (dist_1d - dist_6d).abs() < 1e-3,
        "Camera distance to center must remain invariant when switching to 1D, keeping single cell visual size unchanged! dist_6d={dist_6d}, dist_1d={dist_1d}"
    );
}
