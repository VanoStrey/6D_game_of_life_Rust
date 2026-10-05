use six_d_game_of_life::{GridDimensions, Rules, Simulation};

#[test]
fn test_periodic_size_1_oscillation_1d() {
    let rules = Rules::default(); // 20% to 45%
    let dims = GridDimensions::new(1, 1);
    assert_eq!(dims.total_cells, 1);

    // In 1D PBC, count_all = 3^1 - 1 = 2.
    // min = floor(2 * 0.20) = 0.
    // max = floor(2 * 0.45) = 0.
    // Dead: count_live = 0 in [0, 0] -> born!
    // Alive: count_live = 2 not in [0, 0] -> dies!
    let mut sim = Simulation::new(1, 1, rules);
    assert!(!sim.current.get_linear(0));

    // Step 1: born
    sim.step();
    assert!(sim.current.get_linear(0), "Dead cell in 1D size=1 must become alive on step 1");

    // Step 2: dies
    sim.step();
    assert!(!sim.current.get_linear(0), "Alive cell in 1D size=1 must die on step 2");

    // Step 3: born again
    sim.step();
    assert!(sim.current.get_linear(0), "Cell in 1D size=1 must oscillate to alive on step 3");

    // Step 4: dies again
    sim.step();
    assert!(!sim.current.get_linear(0), "Cell in 1D size=1 must oscillate to dead on step 4");
}

#[test]
fn test_periodic_size_1_oscillation_all_dimensions_with_zero_threshold_rule() {
    // A rule with 0% min and 0% max gives min_neighbors=0, max_neighbors=0 for any count_all.
    // Therefore, any dead cell has count_live=0 in [0, 0] and is born.
    // Any alive cell has count_live = 3^D - 1 > 0 not in [0, 0] and dies.
    // It oscillates strictly between 0 and 1 with period 2 across all dimensions 1D..6D.
    let zero_rule = Rules::new(0.0, 0.0);

    for dim in 1..=6 {
        let mut sim = Simulation::new(1, dim, zero_rule);
        assert!(!sim.current.get_linear(0));

        // Step 1: born
        sim.step();
        assert!(sim.current.get_linear(0), "Step 1 alive (dim={dim})");

        // Step 2: dies
        sim.step();
        assert!(!sim.current.get_linear(0), "Step 2 dead (dim={dim})");

        // Step 3: born
        sim.step();
        assert!(sim.current.get_linear(0), "Step 3 alive (dim={dim})");

        // Step 4: dies
        sim.step();
        assert!(!sim.current.get_linear(0), "Step 4 dead (dim={dim})");
    }
}

#[test]
fn test_periodic_size_1_stability_higher_dimensions_default_rules() {
    let rules = Rules::default(); // 20% to 45%

    for dim in 2..=6 {
        let mut sim = Simulation::new(1, dim, rules);
        assert!(!sim.current.get_linear(0));

        // For D >= 2: count_all = 3^D - 1 >= 8.
        // min = floor((3^D - 1) * 0.20) >= 1.
        // Dead cell has count_live = 0 < min, so it stays dead.
        sim.step();
        assert!(
            !sim.current.get_linear(0),
            "Dead cell in D={dim} size=1 must remain dead under default rules"
        );
    }
}
