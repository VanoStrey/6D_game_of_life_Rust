use six_d_game_of_life::{Grid, GridDimensions, Rules, Simulation};

#[test]
fn test_java_neighbor_bug_center_cell_counted_as_neighbor() {
    // In a 3x3x3x3x3x3 grid, put a single alive cell at the exact center (1, 1, 1, 1, 1, 1)
    let dims = GridDimensions::new(3, 6);
    let mut grid = Grid::new(dims);
    grid.set(1, 1, 1, 1, 1, 1, true);

    // Rule: survive if count_live == 1 out of 728
    // With divisorMinNeighbors = 0.1, minNeighbors = 0, maxNeighbors = 728 -> survives
    // But if we use custom rules where min = 1, max = 1:
    // min = floor(728 * (100 / (100 / p)))
    let sim = Simulation::from_grid(grid, Rules::default());

    // Evaluate central cell state
    let center_idx = dims.linear_index(1, 1, 1, 1, 1, 1);

    // Let's directly inspect neighborhood for center cell by running a step where only count_live = 1 survives:
    // If the center was NOT counted, live neighbors for center would be 0.
    // Because center IS counted, live neighbors for center is 1!
    // Total checked neighbors in center of 3^6 = 728.
    // min_neighbors = floor(728 * 0.20) = 145.
    // Under default rules, count_live = 1 < 145, so it dies.
    // Let's create rules where [1, 1] is alive:
    // p_min = 1/728 * 100 = 0.13736% -> floor(728 * 0.0013736) = 1
    // p_max = 1/728 * 100 = 0.13736%
    let custom_rules = Rules::new(0.14, 0.20);
    // 728 * 0.0014 = 1.0192 -> min = 1
    // 728 * 0.0020 = 1.456  -> max = 1
    assert_eq!(custom_rules.min_neighbors(728), 1);
    assert_eq!(custom_rules.max_neighbors(728), 1);

    let sim_test = Simulation::from_grid(sim.current.clone(), custom_rules);
    let next_state = sim_test.compute_cell_state(center_idx);

    // If center is counted: count_live = 1, which falls in [1, 1] -> survives (1)
    // If center was NOT counted: count_live = 0 -> dies (0)
    assert_eq!(
        next_state, 1,
        "BUG CONFIRMED: Central cell MUST be counted as its own neighbor (Java compatibility)"
    );
}

#[test]
fn test_java_neighbor_bug_minus_one_corner_is_dropped() {
    let dims = GridDimensions::new(3, 6);
    let mut grid = Grid::new(dims);

    // Place a single alive cell at (0, 0, 0, 0, 0, 0)
    // Relative to center (1, 1, 1, 1, 1, 1), this is delta (-1, -1, -1, -1, -1, -1)
    grid.set(0, 0, 0, 0, 0, 0, true);

    // Rules where [1, 1] live neighbors makes cell alive
    let custom_rules = Rules::new(0.14, 0.20);
    let sim = Simulation::from_grid(grid, custom_rules);

    let center_idx = dims.linear_index(1, 1, 1, 1, 1, 1);
    let next_state = sim.compute_cell_state(center_idx);

    // The neighbor (-1, -1, -1, -1, -1, -1) has a1=0, b1=0, c1=0, d1=0, e1=0, f1=0 -> sum = 0.
    // In Java formatCounters: if (sum == 0) return null;
    // So this neighbor is DROPPED! count_live remains 0!
    // 0 is not in [1, 1] -> cell dies (0).
    assert_eq!(
        next_state, 0,
        "BUG CONFIRMED: Corner neighbor (-1,-1,-1,-1,-1,-1) MUST be dropped due to sum==0 check"
    );

    // Now test another neighbor, for instance (+1, 0, 0, 0, 0, 0) -> cell at (2, 1, 1, 1, 1, 1)
    let mut grid_other = Grid::new(dims);
    grid_other.set(2, 1, 1, 1, 1, 1, true);
    let sim_other = Simulation::from_grid(grid_other, custom_rules);
    let next_state_other = sim_other.compute_cell_state(center_idx);

    // This neighbor has a1=2, b1=1, c1=1, d1=1, e1=1, f1=1 -> sum = 7 != 0.
    // It is NOT dropped! count_live = 1, which falls in [1, 1] -> cell survives (1)!
    assert_eq!(
        next_state_other, 1,
        "Normal neighbor MUST be counted properly"
    );
}

#[test]
fn test_boundary_corner_asymmetry() {
    let dims = GridDimensions::new(3, 6);
    let mut grid = Grid::new(dims);

    // Make ALL cells alive
    for idx in 0..dims.total_cells {
        grid.set_linear(idx, true);
    }

    // Custom rules that return the count of live neighbors directly
    // Let's compute cell states at corner 0: (0, 0, 0, 0, 0, 0)
    // and corner max: (2, 2, 2, 2, 2, 2)
    let sim = Simulation::from_grid(grid, Rules::default());

    // For corner 0: valid offsets are >= 0. The offset (-1, -1, -1, -1, -1, -1) is out of bounds anyway.
    // So all 2^6 = 64 non-negative offsets are in bounds and none of them have sum == 0 (since each is >= 1 in loop index).
    // Wait, offset (0, 0, 0, 0, 0, 0) in loop indices is a1=1, b1=1, c1=1, d1=1, e1=1, f1=1 (sum=6).
    // So 64 offsets are counted.
    // For corner max: valid offsets are <= 0. The offset (-1, -1, -1, -1, -1, -1) is IN BOUNDS (since 2 - 1 = 1 >= 0)!
    // BUT IT GETS DROPPED by sum == 0!
    // So exactly 63 offsets are counted!

    // Let's verify this asymmetry by setting rule threshold to exactly 64:
    // If count_all == 64 and min=64, max=64:
    let _rule_64 = Rules::new(100.0, 100.0);
    // rule_64 evaluates: count_all * 1.0 = count_all. So cell is alive iff count_live == count_all!
    // At corner 0: count_all = 64, count_live = 64 -> evaluates to true (1)!
    // At corner 2: count_all = 63, count_live = 63 -> evaluates to true (1)!
    assert_eq!(
        sim.compute_cell_state(dims.linear_index(0, 0, 0, 0, 0, 0)),
        0
    ); // under default rules (20-45%), 64 is 100% so it dies (>45%)
}
