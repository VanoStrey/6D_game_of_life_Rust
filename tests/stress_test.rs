//! Concurrency stress tests for 6D Game of Life SimulationWorker.
//!
//! Validates stability, deadlock-freedom, epoch ordering, and clean shutdown under
//! heavy load, rapid reconfigurations, and large hypergrids.

use std::time::{Duration, Instant};

use six_d_game_of_life::{Rules, SimulationWorker};

#[test]
fn test_rapid_lifecycle_stress_sequence() {
    let mut epoch = 1;
    let mut worker = SimulationWorker::new(epoch, 4, 3, 2, Rules::default(), Some(42), 30.0);

    // Drain initial generation 0
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(500) {
        if worker.try_recv_snapshot().is_ok() {
            break;
        }
        std::thread::yield_now();
    }

    // Run rapid stress sequence multiple times
    for iteration in 0..10 {
        // 1. Play
        worker.set_playing(true);

        // 2. Pause
        worker.set_playing(false);

        // 3. Step, Step
        worker.step();
        worker.step();

        // 4. Change dimension
        epoch += 1;
        let dim = (iteration % 5) + 1;
        worker.reconfigure(epoch, 3, dim, 2, 20.0, 45.0, Some(100 + iteration as u64));

        // 5. Play
        worker.set_playing(true);

        // 6. Change size
        epoch += 1;
        let size = (iteration % 3) + 3;
        worker.reconfigure(epoch, size, 3, 2, 20.0, 45.0, Some(200 + iteration as u64));

        // 7. Pause
        worker.set_playing(false);

        // 8. Randomize
        epoch += 1;
        worker.randomize(epoch, Some(300 + iteration as u64));

        // 9. Play
        worker.set_playing(true);

        // 10. Change rules
        epoch += 1;
        worker.reconfigure(epoch, 3, 3, 2, 15.0, 50.0, Some(400 + iteration as u64));

        // 11. Pause
        worker.set_playing(false);

        // Drain any pending snapshots
        while worker.try_recv_snapshot().is_ok() {}
    }

    // Verify worker is alive and responsive after stress loop
    epoch += 1;
    let final_epoch = epoch;
    worker.reconfigure(final_epoch, 4, 3, 2, 20.0, 45.0, Some(9999));

    let start = Instant::now();
    let mut final_snapshot = None;
    while start.elapsed() < Duration::from_secs(2) {
        if let Ok(snap) = worker.try_recv_snapshot() {
            if snap.epoch == final_epoch {
                final_snapshot = Some(snap);
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(
        final_snapshot.is_some(),
        "Worker must be responsive and deliver final epoch snapshot after stress cycle"
    );

    // Clean shutdown
    worker.shutdown();
}

#[test]
fn test_heavy_6d_size7_reconfigure_and_shutdown() {
    let epoch = 1;
    // 7^6 = 117,649 cells - computationally heavy
    let mut worker = SimulationWorker::new(epoch, 7, 6, 3, Rules::default(), Some(1234), 10.0);

    // Start playing immediately on heavy grid
    worker.set_playing(true);

    // Give it a brief moment to start computing a step
    std::thread::sleep(Duration::from_millis(20));

    // While computing heavy step, reconfigure to small 1D grid
    let new_epoch = 2;
    worker.reconfigure(new_epoch, 10, 1, 1, 20.0, 45.0, Some(5678));

    // Wait for new epoch to be received
    let start = Instant::now();
    let mut received_new_epoch = false;
    while start.elapsed() < Duration::from_secs(3) {
        if let Ok(snap) = worker.try_recv_snapshot() {
            if snap.epoch == new_epoch {
                assert_eq!(snap.total_cells, 10);
                received_new_epoch = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(
        received_new_epoch,
        "Worker must cleanly transition from heavy 6D computation to 1D reconfigured state"
    );

    // Shutdown must complete without hanging
    let shutdown_start = Instant::now();
    worker.shutdown();
    assert!(
        shutdown_start.elapsed() < Duration::from_secs(2),
        "Shutdown must not hang or deadlock"
    );
}

#[test]
fn test_rapid_reconfiguration_epoch_monotonicity() {
    let mut current_epoch = 10;
    let mut worker =
        SimulationWorker::new(current_epoch, 5, 4, 2, Rules::default(), Some(42), 20.0);

    // Rapidly send reconfigurations: 6D -> 4D -> 1D -> 3D without waiting
    current_epoch += 1;
    worker.reconfigure(current_epoch, 3, 6, 2, 20.0, 45.0, Some(101));

    current_epoch += 1;
    worker.reconfigure(current_epoch, 4, 4, 2, 20.0, 45.0, Some(102));

    current_epoch += 1;
    worker.reconfigure(current_epoch, 8, 1, 1, 20.0, 45.0, Some(103));

    current_epoch += 1;
    let final_epoch = current_epoch;
    worker.reconfigure(final_epoch, 4, 3, 2, 20.0, 45.0, Some(104));

    // Collect snapshots, verify monotonic epochs and arrival of final_epoch
    let mut highest_epoch_seen = 0;
    let mut received_final = false;
    let start = Instant::now();

    while start.elapsed() < Duration::from_secs(3) {
        while let Ok(snap) = worker.try_recv_snapshot() {
            assert!(
                snap.epoch >= highest_epoch_seen,
                "Received epoch ({}) must not regress below previously observed epoch ({})",
                snap.epoch,
                highest_epoch_seen
            );
            highest_epoch_seen = snap.epoch;

            if snap.epoch == final_epoch {
                assert_eq!(snap.total_cells, 4usize.pow(3)); // 4^3 = 64
                received_final = true;
                break;
            }
        }
        if received_final {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(
        received_final,
        "Final epoch must be delivered and processed"
    );
    worker.shutdown();
}

#[test]
fn test_backpressure_shutdown_with_full_channel() {
    let epoch = 1;
    // Bounded with strict capacity 1
    let mut worker =
        SimulationWorker::with_bounds(epoch, 3, 3, 1, Rules::default(), Some(77), 50.0, 16, 1);

    // Start playing at high speed and do NOT drain snapshots
    worker.set_playing(true);
    std::thread::sleep(Duration::from_millis(80));

    // Channel is now guaranteed to be full, worker is blocked waiting in select
    let shutdown_start = Instant::now();
    worker.shutdown();

    assert!(
        shutdown_start.elapsed() < Duration::from_secs(1),
        "Shutdown must complete swiftly even when snapshot channel is saturated"
    );
}

#[test]
fn test_play_and_step_interleaving_maintains_order() {
    let epoch = 1;
    let mut worker = SimulationWorker::new(epoch, 4, 3, 2, Rules::default(), Some(123), 5.0);

    // Wait for gen 0
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(500) {
        if worker.try_recv_snapshot().is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }

    // Start play
    worker.set_playing(true);

    // Issue manual steps while playing
    for _ in 0..3 {
        worker.step();
        std::thread::sleep(Duration::from_millis(30));
    }

    worker.set_playing(false);

    // Collect all snapshots, ensure generation strictly increases
    let mut gens = Vec::new();
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(300) {
        while let Ok(snap) = worker.try_recv_snapshot() {
            gens.push(snap.generation);
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(
        !gens.is_empty(),
        "Must receive snapshots during play + step"
    );
    for i in 1..gens.len() {
        assert!(
            gens[i] > gens[i - 1],
            "Generations must strictly increase: {:?} at index {}",
            gens,
            i
        );
    }

    worker.shutdown();
}
