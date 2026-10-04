use six_d_game_of_life::{GridDimensions, Rules, Simulation};

#[test]
fn test_size_1_oscillation_across_all_dimensions() {
    let rules = Rules::default();

    for dim in 1..=6 {
        let dims = GridDimensions::new(1, dim);
        assert_eq!(
            dims.total_cells, 1,
            "size=1 must always have exactly 1 cell"
        );

        // Start dead
        let mut sim = Simulation::new(1, dim, rules);
        assert_eq!(sim.current.get_linear(0), false);

        // Step 1: dead cell has count_all=1, count_live=0 -> in [0, 0] -> lives!
        sim.step();
        assert_eq!(
            sim.current.get_linear(0),
            true,
            "Dead cell in size=1 must become alive on step 1 (dim={dim})"
        );

        // Step 2: alive cell has count_all=1, count_live=1 -> not in [0, 0] -> dies!
        sim.step();
        assert_eq!(
            sim.current.get_linear(0),
            false,
            "Alive cell in size=1 must die on step 2 (dim={dim})"
        );

        // Step 3: lives again
        sim.step();
        assert_eq!(
            sim.current.get_linear(0),
            true,
            "Cell must oscillate to alive on step 3 (dim={dim})"
        );

        // Step 4: dies again
        sim.step();
        assert_eq!(
            sim.current.get_linear(0),
            false,
            "Cell must oscillate to dead on step 4 (dim={dim})"
        );
    }
}
