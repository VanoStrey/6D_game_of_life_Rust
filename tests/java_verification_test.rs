use serde::Deserialize;
use six_d_game_of_life::{Grid, GridDimensions, Rules, Simulation};
use std::fs;

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct JavaTestCase {
    name: String,
    size: usize,
    dimensions: usize,
    minPercent: f64,
    maxPercent: f64,
    initial: Vec<u8>,
    step1: Vec<u8>,
    step2: Vec<u8>,
    step3: Vec<u8>,
}

#[test]
fn test_exact_equivalence_against_java_oracle() {
    let json_path = "tests/java_oracle/oracle_data.json";
    let json_str =
        fs::read_to_string(json_path).expect("Failed to read tests/java_oracle/oracle_data.json");
    let test_cases: Vec<JavaTestCase> =
        serde_json::from_str(&json_str).expect("Failed to parse oracle_data.json");

    println!(
        "Verifying {} test cases from Java oracle...",
        test_cases.len()
    );

    for tc in test_cases {
        let dims = GridDimensions::new(tc.size, tc.dimensions);
        let rules = Rules::new(tc.minPercent, tc.maxPercent);

        // Verify with step_seq()
        {
            let grid = Grid::from_vec(dims, tc.initial.clone());
            let mut sim = Simulation::from_grid(grid, rules);

            sim.step_seq();
            assert_eq!(
                sim.current.as_slice(),
                tc.step1.as_slice(),
                "Sequential step 1 mismatch for test case '{}' (dim={}, size={})",
                tc.name,
                tc.dimensions,
                tc.size
            );

            sim.step_seq();
            assert_eq!(
                sim.current.as_slice(),
                tc.step2.as_slice(),
                "Sequential step 2 mismatch for test case '{}' (dim={}, size={})",
                tc.name,
                tc.dimensions,
                tc.size
            );

            sim.step_seq();
            assert_eq!(
                sim.current.as_slice(),
                tc.step3.as_slice(),
                "Sequential step 3 mismatch for test case '{}' (dim={}, size={})",
                tc.name,
                tc.dimensions,
                tc.size
            );
        }

        // Verify with step_par() (Rayon parallel)
        {
            let grid = Grid::from_vec(dims, tc.initial);
            let mut sim = Simulation::from_grid(grid, rules);

            sim.step_par();
            assert_eq!(
                sim.current.as_slice(),
                tc.step1.as_slice(),
                "Parallel step 1 mismatch for test case '{}' (dim={}, size={})",
                tc.name,
                tc.dimensions,
                tc.size
            );

            sim.step_par();
            assert_eq!(
                sim.current.as_slice(),
                tc.step2.as_slice(),
                "Parallel step 2 mismatch for test case '{}' (dim={}, size={})",
                tc.name,
                tc.dimensions,
                tc.size
            );

            sim.step_par();
            assert_eq!(
                sim.current.as_slice(),
                tc.step3.as_slice(),
                "Parallel step 3 mismatch for test case '{}' (dim={}, size={})",
                tc.name,
                tc.dimensions,
                tc.size
            );
        }
    }

    println!("ALL Java oracle test cases matched 100% identically in both sequential and parallel modes!");
}
