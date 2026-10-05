use rand::rngs::StdRng;
use rand::SeedableRng;
use six_d_game_of_life::{Rules, Simulation};

#[test]
fn test_separable_equivalence_against_sequential() {
    let mut rng = StdRng::seed_from_u64(12345);

    for dim in 1..=6 {
        let max_size = if dim >= 4 { 6 } else { 10 };
        for size in [1, 2, 3, 4, 6] {
            if size > max_size {
                continue;
            }
            let rules = Rules::default();
            let mut sim = Simulation::new(size, dim, rules);
            sim.randomize(&mut rng);

            // Compute expected state using cell-by-cell reference oracle
            let mut expected = vec![0u8; sim.current.len()];
            for (idx, slot) in expected.iter_mut().enumerate() {
                *slot = sim.compute_cell_state(idx);
            }

            // Compute actual state using separable SIMD engine
            let mut sim_step = sim.clone();
            sim_step.step_seq();

            assert_eq!(
                sim_step.current.as_slice(),
                expected.as_slice(),
                "Separable sequential convolution mismatch for dim={}, size={}",
                dim,
                size
            );

            // Also test parallel separable execution
            let mut sim_par = sim.clone();
            sim_par.step_par();

            assert_eq!(
                sim_par.current.as_slice(),
                expected.as_slice(),
                "Separable parallel convolution mismatch for dim={}, size={}",
                dim,
                size
            );
        }
    }
}
