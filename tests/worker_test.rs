//! Tests for the SimulationWorker background thread, communication, determinism, and backpressure.

use std::thread;
use std::time::{Duration, Instant};

use rand::rngs::StdRng;
use rand::SeedableRng;
use six_d_game_of_life::{Rules, Simulation, SimulationWorker};

#[test]
fn test_worker_step_produces_single_generation() {
    let epoch = 1;
    let size = 4;
    let dimensions = 4;
    let delta = 3;
    let rules = Rules::default();
    let seed = Some(12345);

    let mut worker = SimulationWorker::new(epoch, size, dimensions, delta, rules, seed, 5.0);

    // Initial snapshot should be generation 0
    let mut initial = None;
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(500) {
        if let Ok(snap) = worker.try_recv_snapshot() {
            initial = Some(snap);
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }

    let snap0 = initial.expect("Failed to receive initial generation 0 snapshot");
    assert_eq!(snap0.generation, 0);
    assert_eq!(snap0.epoch, epoch);

    // Send Step
    worker.step();

    // Next snapshot must be exactly generation 1
    let mut step1 = None;
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(500) {
        if let Ok(snap) = worker.try_recv_snapshot() {
            step1 = Some(snap);
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }

    let snap1 = step1.expect("Failed to receive generation 1 snapshot after Step");
    assert_eq!(snap1.generation, 1);
    assert_eq!(snap1.epoch, epoch);

    // Wait a bit to ensure no additional generations are produced (idle state)
    thread::sleep(Duration::from_millis(100));
    assert!(
        worker.try_recv_snapshot().is_err(),
        "Worker should not produce extra generations while paused"
    );

    worker.shutdown();
}

#[test]
fn test_worker_play_pause_lifecycle() {
    let epoch = 1;
    let size = 4;
    let dimensions = 3;
    let delta = 2;
    let rules = Rules::default();
    let seed = Some(42);

    let mut worker = SimulationWorker::new(epoch, size, dimensions, delta, rules, seed, 20.0);

    // Drain initial
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(500) {
        if worker.try_recv_snapshot().is_ok() {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }

    // Start playing
    worker.set_playing(true);

    // Collect multiple generations
    let mut received_generations = Vec::new();
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(300) {
        while let Ok(snap) = worker.try_recv_snapshot() {
            received_generations.push(snap.generation);
        }
        thread::sleep(Duration::from_millis(20));
    }

    assert!(
        !received_generations.is_empty(),
        "Should have received multiple generations during Play"
    );
    // Ensure monotonically increasing generations
    for i in 1..received_generations.len() {
        assert!(
            received_generations[i] >= received_generations[i - 1],
            "Generations must be non-decreasing"
        );
    }

    // Now pause
    worker.set_playing(false);

    // Drain any remaining snapshot
    thread::sleep(Duration::from_millis(50));
    while worker.try_recv_snapshot().is_ok() {}

    // Verify no new snapshots arrive after pause
    thread::sleep(Duration::from_millis(150));
    assert!(
        worker.try_recv_snapshot().is_err(),
        "No snapshots should be generated while paused"
    );

    worker.shutdown();
}

#[test]
fn test_worker_determinism_vs_single_and_rayon_simulation() {
    let size = 5;
    let dimensions = 4;
    let delta = 3;
    let rules = Rules::default();
    let seed = 99999;
    let steps = 4;

    // 1. Reference single-thread simulation
    let mut ref_sim_seq = Simulation::new(size, dimensions, rules);
    let mut rng1 = StdRng::seed_from_u64(seed);
    ref_sim_seq.randomize(&mut rng1);
    for _ in 0..steps {
        ref_sim_seq.step_seq();
    }

    // 2. Reference rayon parallel simulation
    let mut ref_sim_par = Simulation::new(size, dimensions, rules);
    let mut rng2 = StdRng::seed_from_u64(seed);
    ref_sim_par.randomize(&mut rng2);
    for _ in 0..steps {
        ref_sim_par.step_par();
    }

    assert_eq!(
        ref_sim_seq.current.as_slice(),
        ref_sim_par.current.as_slice(),
        "Sequential and parallel simulation must produce identical state"
    );

    // 3. Worker simulation
    let epoch = 10;
    let mut worker = SimulationWorker::new(epoch, size, dimensions, delta, rules, Some(seed), 10.0);

    // Wait for gen 0
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(500) {
        if worker.try_recv_snapshot().is_ok() {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }

    // Execute exact number of steps via Step command
    let mut last_snap = None;
    for expected_gen in 1..=steps {
        worker.step();
        let step_start = Instant::now();
        let mut got_step = None;
        while step_start.elapsed() < Duration::from_millis(1000) {
            if let Ok(snap) = worker.try_recv_snapshot() {
                if snap.generation == expected_gen as u64 {
                    got_step = Some(snap);
                    break;
                }
            }
            thread::sleep(Duration::from_millis(5));
        }
        last_snap = Some(got_step.expect("Failed to receive expected step generation"));
    }

    let final_snap = last_snap.unwrap();
    assert_eq!(final_snap.generation, steps as u64);
    assert_eq!(
        final_snap.alive_count,
        ref_sim_par.count_alive(),
        "Worker alive count must match reference simulation"
    );

    worker.shutdown();
}

#[test]
fn test_worker_backpressure_does_not_grow_channel() {
    let epoch = 1;
    let size = 3;
    let dimensions = 3;
    let delta = 1;
    let rules = Rules::default();
    let seed = Some(777);
    let result_bound = 2;

    // Create worker with strict result_bound = 2
    let mut worker = SimulationWorker::with_bounds(
        epoch,
        size,
        dimensions,
        delta,
        rules,
        seed,
        100.0, // very high speed
        16,
        result_bound,
    );

    // Start playing without reading from result_rx
    worker.set_playing(true);

    // Let the worker run for 100ms with fast speed
    thread::sleep(Duration::from_millis(100));

    // Pause worker
    worker.set_playing(false);
    thread::sleep(Duration::from_millis(20));

    // Drain all snapshots in the channel: should NOT exceed result_bound + 1
    let mut count = 0;
    while worker.try_recv_snapshot().is_ok() {
        count += 1;
    }

    assert!(
        count <= result_bound + 1,
        "Channel backlog must be bounded; got {count} snapshots, expected <= {}",
        result_bound + 1
    );

    worker.shutdown();
}

#[test]
fn test_configuration_changes_and_stale_epoch_rejection() {
    let mut current_epoch = 1;
    let mut worker =
        SimulationWorker::new(current_epoch, 6, 4, 3, Rules::default(), Some(100), 30.0);

    // Receive initial epoch 1
    let start = Instant::now();
    let mut initial_snap = None;
    while start.elapsed() < Duration::from_millis(500) {
        if let Ok(snap) = worker.try_recv_snapshot() {
            if snap.epoch == current_epoch {
                initial_snap = Some(snap);
                break;
            }
        }
        thread::sleep(Duration::from_millis(5));
    }
    assert!(initial_snap.is_some());

    // Start playback
    worker.set_playing(true);
    thread::sleep(Duration::from_millis(50));

    // Now reconfigure: change to 1D, new epoch
    current_epoch += 1;
    let new_epoch = current_epoch;
    worker.reconfigure(new_epoch, 8, 1, 1, 20.0, 45.0, Some(200));

    // Collect snapshots, verify that any received snapshot for new_epoch has the new dimensions/size
    let start = Instant::now();
    let mut new_epoch_received = false;
    while start.elapsed() < Duration::from_millis(500) {
        if let Ok(snap) = worker.try_recv_snapshot() {
            if snap.epoch < new_epoch {
                // Stale snapshot from epoch 1: perfectly expected before reconfigure completed
                continue;
            } else if snap.epoch == new_epoch {
                assert_eq!(snap.total_cells, 8); // 8^1 = 8 in 1D
                new_epoch_received = true;
                break;
            }
        }
        thread::sleep(Duration::from_millis(10));
    }

    assert!(
        new_epoch_received,
        "Should receive new snapshot with updated epoch and configuration"
    );

    worker.shutdown();
}

#[test]
fn test_dimension_cycling_via_worker_6d_to_1d() {
    let mut epoch = 1;
    let mut worker = SimulationWorker::new(epoch, 3, 6, 2, Rules::default(), Some(42), 10.0);

    // Switch through dimensions 1 to 6 and back to 1
    for dim in [1, 2, 3, 4, 5, 6, 1] {
        epoch += 1;
        worker.reconfigure(epoch, 3, dim, 2, 20.0, 45.0, Some(42 + dim as u64));

        let start = Instant::now();
        let mut received = false;
        while start.elapsed() < Duration::from_millis(500) {
            if let Ok(snap) = worker.try_recv_snapshot() {
                if snap.epoch == epoch {
                    assert_eq!(snap.total_cells, 3usize.pow(dim as u32));
                    received = true;
                    break;
                }
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(received, "Must receive snapshot for dimension {dim}");
    }

    worker.shutdown();
}

#[test]
fn test_reconfigure_preserves_existing_alive_cells_without_randomizing() {
    let mut epoch = 1;
    let size = 4;
    let dimensions = 3;
    let delta = 2;
    let rules = Rules::default();

    let mut worker = SimulationWorker::new(epoch, size, dimensions, delta, rules, Some(777), 10.0);

    // Receive initial snapshot
    let mut snap0 = None;
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(500) {
        if let Ok(snap) = worker.try_recv_snapshot() {
            snap0 = Some(snap);
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    let initial = snap0.expect("Initial snapshot");
    let initial_alive = initial.alive_count;
    assert!(
        initial_alive > 0,
        "Initial grid should have some alive cells"
    );

    // Reconfigure ONLY rules (percent_min / percent_max): dimensions/size are identical
    epoch += 1;
    worker.reconfigure(epoch, size, dimensions, delta, 30.0, 60.0, None);

    let mut snap_reconf = None;
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(500) {
        if let Ok(snap) = worker.try_recv_snapshot() {
            if snap.epoch == epoch {
                snap_reconf = Some(snap);
                break;
            }
        }
        thread::sleep(Duration::from_millis(5));
    }
    let reconf = snap_reconf.expect("Snapshot after reconfigure");
    assert_eq!(
        reconf.alive_count, initial_alive,
        "Reconfiguring rules must NOT randomize; existing alive cell count must be preserved exactly!"
    );
    assert_eq!(reconf.alive_positions, initial.alive_positions);

    worker.shutdown();
}
