use six_d_game_of_life::{compute_cell_state_reference, Grid, GridDimensions, Rules, Simulation};

#[test]
fn test_1d_periodic_boundary_wrap() {
    let rules = Rules::new(50.0, 100.0); // min_neighbors(2) = floor(2 * 0.5) = 1
    let dims = GridDimensions::new(3, 1);
    let mut grid = Grid::new(dims);

    // [A, B, C] = [0, 1, 2]
    // Put live cell at C (idx 2)
    grid.set_linear(2, true);

    // In 1D torus, left neighbor of A (idx 0) is C (idx 2).
    // So cell A (idx 0) must see 1 live neighbor.
    let next_a = compute_cell_state_reference(&grid, &rules, 0);
    assert_eq!(next_a, 1, "Cell A (0) must see wrapped left neighbor C (2)");

    // Put live cell at A (idx 0) only
    grid.clear();
    grid.set_linear(0, true);

    // In 1D torus, right neighbor of C (idx 2) is A (idx 0).
    let next_c = compute_cell_state_reference(&grid, &rules, 2);
    assert_eq!(next_c, 1, "Cell C (2) must see wrapped right neighbor A (0)");
}

#[test]
fn test_2d_periodic_boundary_wrap() {
    let rules = Rules::new(15.0, 100.0); // min_neighbors(8) = floor(8 * 0.15) = 1
    let size = 4;
    let dims = GridDimensions::new(size, 2);
    let mut grid = Grid::new(dims);

    // Set top-left corner (0, 0)
    grid.set(0, 0, 0, 0, 0, 0, true);

    // In 2D torus:
    // Left of (0, 0) is (0, size-1)
    // Above (0, 0) is (size-1, 0)
    // Diagonal top-left is (size-1, size-1)
    let idx_right = dims.linear_index(0, 0, 0, 0, 0, size - 1);
    let idx_bottom = dims.linear_index(0, 0, 0, 0, size - 1, 0);
    let idx_diag = dims.linear_index(0, 0, 0, 0, size - 1, size - 1);

    assert_eq!(compute_cell_state_reference(&grid, &rules, idx_right), 1, "Horizontal wrap");
    assert_eq!(compute_cell_state_reference(&grid, &rules, idx_bottom), 1, "Vertical wrap");
    assert_eq!(compute_cell_state_reference(&grid, &rules, idx_diag), 1, "Diagonal corner wrap");

    // Non-adjacent cell (2, 2) has 0 live neighbors -> must stay 0
    let idx_far = dims.linear_index(0, 0, 0, 0, 2, 2);
    assert_eq!(compute_cell_state_reference(&grid, &rules, idx_far), 0, "Non-adjacent cell must remain dead");
}

#[test]
fn test_3d_periodic_boundary_wrap() {
    let rules = Rules::new(5.0, 100.0); // min_neighbors(26) = floor(26 * 0.05) = 1
    let size = 4;
    let dims = GridDimensions::new(size, 3);
    let mut grid = Grid::new(dims);

    // Live cell at origin (0, 0, 0)
    grid.set(0, 0, 0, 0, 0, 0, true);

    // Test opposite corner (size-1, size-1, size-1)
    let opp_idx = dims.linear_index(0, 0, 0, size - 1, size - 1, size - 1);
    assert_eq!(
        compute_cell_state_reference(&grid, &rules, opp_idx),
        1,
        "Opposite corner in 3D must be adjacent via periodic wrap across all 3 axes"
    );

    // Non-adjacent cell (2, 2, 2) has 0 live neighbors -> must stay 0
    let far_idx = dims.linear_index(0, 0, 0, 2, 2, 2);
    assert_eq!(compute_cell_state_reference(&grid, &rules, far_idx), 0, "Non-adjacent cell must remain dead");
}

#[test]
fn test_6d_periodic_boundary_wrap_all_six_axes() {
    let rules = Rules::new(0.2, 100.0); // min_neighbors(728) = floor(728 * 0.002) = 1
    let size = 4;
    let dims = GridDimensions::new(size, 6);
    let mut grid = Grid::new(dims);

    // Live cell at origin (0, 0, 0, 0, 0, 0)
    grid.set(0, 0, 0, 0, 0, 0, true);

    // Verify wrap on each of the 6 independent axes:
    // Axis 0 (f): coord (0,0,0,0,0, size-1)
    let idx_f = dims.linear_index(0, 0, 0, 0, 0, size - 1);
    assert_eq!(compute_cell_state_reference(&grid, &rules, idx_f), 1, "Axis 0 wrap");

    // Axis 1 (e): coord (0,0,0,0, size-1, 0)
    let idx_e = dims.linear_index(0, 0, 0, 0, size - 1, 0);
    assert_eq!(compute_cell_state_reference(&grid, &rules, idx_e), 1, "Axis 1 wrap");

    // Axis 2 (d): coord (0,0,0, size-1, 0, 0)
    let idx_d = dims.linear_index(0, 0, 0, size - 1, 0, 0);
    assert_eq!(compute_cell_state_reference(&grid, &rules, idx_d), 1, "Axis 2 wrap");

    // Axis 3 (c): coord (0,0, size-1, 0, 0, 0)
    let idx_c = dims.linear_index(0, 0, size - 1, 0, 0, 0);
    assert_eq!(compute_cell_state_reference(&grid, &rules, idx_c), 1, "Axis 3 wrap");

    // Axis 4 (b): coord (0, size-1, 0, 0, 0, 0)
    let idx_b = dims.linear_index(0, size - 1, 0, 0, 0, 0);
    assert_eq!(compute_cell_state_reference(&grid, &rules, idx_b), 1, "Axis 4 wrap");

    // Axis 5 (a): coord (size-1, 0, 0, 0, 0, 0)
    let idx_a = dims.linear_index(size - 1, 0, 0, 0, 0, 0);
    assert_eq!(compute_cell_state_reference(&grid, &rules, idx_a), 1, "Axis 5 wrap");

    // Fully opposite 6D corner (size-1, size-1, size-1, size-1, size-1, size-1)
    let opp_idx = dims.linear_index(size - 1, size - 1, size - 1, size - 1, size - 1, size - 1);
    assert_eq!(
        compute_cell_state_reference(&grid, &rules, opp_idx),
        1,
        "6D opposite corner wrap across all six dimensions"
    );

    // Non-adjacent cell (2,2,2,2,2,2) has 0 live neighbors -> must stay 0
    let far_idx = dims.linear_index(2, 2, 2, 2, 2, 2);
    assert_eq!(compute_cell_state_reference(&grid, &rules, far_idx), 0, "Non-adjacent cell must remain dead");
}

#[test]
fn test_moore_neighbor_counts_across_all_dimensions() {
    // Expected Moore neighborhood sizes: 3^D - 1
    // D=1: 2
    // D=2: 8
    // D=3: 26
    // D=4: 80
    // D=5: 242
    // D=6: 728
    let expected_counts = [2, 8, 26, 80, 242, 728];

    for dim in 1..=6 {
        let expected = expected_counts[dim - 1];
        let calculated = 3usize.pow(dim as u32) - 1;
        assert_eq!(calculated, expected, "Moore neighbor count for D={dim}");

        // Create a completely filled grid (all cells alive) of size 4
        let dims = GridDimensions::new(4, dim);
        let mut grid = Grid::new(dims);
        grid.as_mut_slice().fill(1);

        // Define a strict rule that requires EXACTLY `expected` live neighbors to survive
        // percent_min such that floor(expected * min / 100) == expected
        let rules = Rules::new(100.0, 100.0);
        assert_eq!(rules.min_neighbors(expected as i32), expected as i32);
        assert_eq!(rules.max_neighbors(expected as i32), expected as i32);

        // Any cell must see exactly `expected` live neighbors (since all other cells are alive)
        let state = compute_cell_state_reference(&grid, &rules, 0);
        assert_eq!(
            state, 1,
            "Every cell in D={dim} must have exactly {expected} neighbors (excluding itself)"
        );
    }
}

#[test]
fn test_center_cell_is_not_counted_as_neighbor() {
    // If the center cell WERE counted as a neighbor, a grid with ONLY the center cell alive
    // would see 1 live neighbor for the center cell.
    // In pure Moore neighborhood, the center cell has 0 neighbors alive!
    for dim in 1..=6 {
        let dims = GridDimensions::new(5, dim);
        let mut grid = Grid::new(dims);
        grid.set_linear(0, true); // Only cell 0 is alive

        let k = 3usize.pow(dim as u32) - 1;
        // Rule: requires >= 1 live neighbor to survive
        // 150.0 / k ensures floor(k * (1.5 / k)) == floor(1.5) == 1
        let rules = Rules::new(150.0 / (k as f64), 100.0);
        assert_eq!(rules.min_neighbors(k as i32), 1);

        // Center cell 0 must see 0 live neighbors -> fails rule -> dies!
        let next_0 = compute_cell_state_reference(&grid, &rules, 0);
        assert_eq!(
            next_0, 0,
            "Center cell must NOT count itself as a neighbor in D={dim}"
        );
    }
}

#[test]
fn test_translational_symmetry_on_torus() {
    // On a torus T^D, space is completely homogeneous.
    // A single live cell placed at ANY position in the grid must produce the
    // exact same number of activated neighbor cells in the next generation.
    let dims = GridDimensions::new(4, 3);
    let rules = Rules::new(5.0, 100.0); // min_neighbors(26) = 1

    let test_positions = [
        0,                      // Origin
        dims.total_cells - 1,   // Max corner
        dims.total_cells / 2,   // Center
        5,                      // Edge
    ];

    let mut activated_counts = Vec::new();

    for &pos in &test_positions {
        let mut sim = Simulation::new(4, 3, rules);
        sim.current.set_linear(pos, true);
        sim.step();
        activated_counts.push(sim.current.count_alive());
    }

    let first = activated_counts[0];
    for (i, &count) in activated_counts.iter().enumerate() {
        assert_eq!(
            count, first,
            "Toroidal translational symmetry violated at test position index {i}: count={count} vs {first}"
        );
    }
}

#[test]
fn test_periodic_size_1_repeating_references() {
    // For size=1 in D=1:
    // Kernel has 2 neighbor positions (offset -1 and +1).
    // Both wrap to cell 0.
    // If cell 0 is alive (1), count_live = 2.
    // If cell 0 is dead (0), count_live = 0.
    let dims = GridDimensions::new(1, 1);
    let mut grid = Grid::new(dims);

    let rules = Rules::new(50.0, 100.0); // requires count_live >= 1 (floor(2 * 0.5) = 1)
    grid.set_linear(0, true);

    let next_state = compute_cell_state_reference(&grid, &rules, 0);
    // count_live = 2 >= 1 -> survives!
    assert_eq!(next_state, 1, "size=1 with live cell sees 2 live neighbor kernel positions");

    grid.set_linear(0, false);
    let next_dead = compute_cell_state_reference(&grid, &rules, 0);
    // count_live = 0 < 1 -> stays dead!
    assert_eq!(next_dead, 0, "size=1 with dead cell sees 0 live neighbor kernel positions");
}

#[test]
fn test_periodic_size_2_repeating_directions() {
    // For size=2 in D=1:
    // Cells {0, 1}.
    // Cell 0: offset -1 wraps to 1, offset +1 wraps to 1. Both sample cell 1!
    // If cell 1 is alive (1) and cell 0 is dead (0):
    // count_live for cell 0 = 2!
    let dims = GridDimensions::new(2, 1);
    let mut grid = Grid::new(dims);
    grid.set_linear(1, true);

    // Rule: survives or born if count_live == 2
    let rules = Rules::new(100.0, 100.0); // min = floor(2 * 1.0) = 2, max = 2
    let next_0 = compute_cell_state_reference(&grid, &rules, 0);
    assert_eq!(next_0, 1, "Cell 0 must see 2 live neighbor kernel positions from cell 1 in size=2");

    let next_1 = compute_cell_state_reference(&grid, &rules, 1);
    assert_eq!(next_1, 0, "Cell 1 sees cell 0 which is dead -> 0 live neighbors");
}
