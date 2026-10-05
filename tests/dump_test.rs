use std::time::Duration;

use rand::SeedableRng;
use six_d_game_of_life::renderer::ColorMode;
use six_d_game_of_life::simulation::{
    DumpError, GridDimensions, Rules, Simulation, SimulationDump, SimulationWorker,
};

#[test]
fn test_dump_worker_export_and_file_compression() {
    let temp_dir = std::env::temp_dir().join("gol6d_tests");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let dump_file = temp_dir.join("test_export_6d.gol6d");

    // Initialize worker with 6D, size 6: 6^6 = 46,656 cells
    let rules = Rules::new(20.0, 45.0);
    let worker = SimulationWorker::new_with_topology(
        1,
        6,
        6,
        2,
        rules,
        true,
        Some(12345),
        30.0,
    );

    // Drain initial snapshot 0
    let mut initial_snap = None;
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_millis(500) {
        if let Ok(snap) = worker.try_recv_snapshot() {
            initial_snap = Some(snap);
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut last_snap = initial_snap.expect("Initial snapshot should be delivered");
    assert_eq!(last_snap.generation, 0);

    // Perform 5 steps, draining each snapshot
    for expected_gen in 1..=5 {
        worker.step();
        let start = std::time::Instant::now();
        let mut got_snap = None;
        while start.elapsed() < Duration::from_millis(500) {
            if let Ok(snap) = worker.try_recv_snapshot() {
                got_snap = Some(snap);
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        last_snap = got_snap.unwrap_or_else(|| panic!("Snapshot for generation {} not received", expected_gen));
        assert_eq!(last_snap.generation, expected_gen);
    }

    // Export state from worker
    let exported = worker
        .export_state(Duration::from_millis(500))
        .expect("Worker export_state should succeed");

    assert_eq!(exported.dimensions, 6);
    assert_eq!(exported.size, 6);
    assert_eq!(exported.generation, last_snap.generation);
    assert_eq!(exported.alive_count, last_snap.alive_count);
    assert!(exported.periodic);

    // Create SimulationDump and write to file
    let dump = SimulationDump {
        dimensions: exported.dimensions,
        size: exported.size,
        delta: exported.delta,
        percent_min: exported.percent_min,
        percent_max: exported.percent_max,
        periodic: exported.periodic,
        color_mode: ColorMode::Hyperdimension,
        generation: exported.generation,
        total_cells: exported.total_cells,
        alive_count: exported.alive_count,
        grid: exported.grid.clone(),
    };

    let bytes_written = dump.write_to_file(&dump_file).expect("Write to file failed");
    let file_meta = std::fs::metadata(&dump_file).unwrap();
    assert_eq!(file_meta.len(), bytes_written as u64);

    // Uncompressed 46,656 cells:
    // Raw bytes = 46,656 B (~45.5 KB)
    // Bitpacked = 5,832 B
    // Deflate-compressed with 60-byte header should be under 1,500 bytes (over 96% compression!)
    println!(
        "6D Size 6 (46,656 cells): Dump file size = {} bytes (Uncompressed: 46,656 bytes)",
        bytes_written
    );
    assert!(
        bytes_written < 2000,
        "Expected ultra-compact file < 2000 bytes, got {}",
        bytes_written
    );

    // Read back and verify exact bit-for-bit reconstruction
    let loaded_dump = SimulationDump::read_from_file(&dump_file).expect("Read from file failed");
    assert_eq!(loaded_dump.dimensions, 6);
    assert_eq!(loaded_dump.size, 6);
    assert_eq!(loaded_dump.delta, 2);
    assert_eq!(loaded_dump.percent_min, 20.0);
    assert_eq!(loaded_dump.percent_max, 45.0);
    assert!(loaded_dump.periodic);
    assert_eq!(loaded_dump.color_mode, ColorMode::Hyperdimension);
    assert_eq!(loaded_dump.generation, exported.generation);
    assert_eq!(loaded_dump.total_cells, 46656);
    assert_eq!(loaded_dump.alive_count, exported.alive_count);
    assert_eq!(loaded_dump.grid, exported.grid);

    // Clean up
    let _ = std::fs::remove_file(&dump_file);
}

#[test]
fn test_dump_import_into_new_worker_continues_deterministically() {
    // 1. Setup simulation, step it, and dump
    let dims = GridDimensions::new(8, 4); // 4D grid, size 8 = 4096 cells
    let rules = Rules::new(25.0, 40.0);
    let mut rng = rand::rngs::StdRng::seed_from_u64(999);
    let mut sim1 = Simulation::new_with_topology(dims.size, dims.dimensions, rules, false);
    sim1.randomize(&mut rng);
    for _ in 0..10 {
        sim1.step();
    }

    let dump = SimulationDump {
        dimensions: dims.dimensions,
        size: dims.size,
        delta: 3,
        percent_min: 25.0,
        percent_max: 40.0,
        periodic: false,
        color_mode: ColorMode::Uniform,
        generation: sim1.generation,
        total_cells: dims.total_cells,
        alive_count: sim1.count_alive(),
        grid: sim1.current.clone(),
    };

    let bytes = dump.to_bytes().expect("Serialization failed");

    // 2. Load into a fresh worker thread
    let new_rules = Rules::new(dump.percent_min, dump.percent_max);
    let worker = SimulationWorker::new_with_topology(
        10,
        dump.size,
        dump.dimensions,
        dump.delta,
        new_rules,
        dump.periodic,
        None,
        30.0,
    );

    // Now load exact dumped state
    let loaded = SimulationDump::from_bytes(&bytes).expect("Deserialization failed");
    worker.load_state(
        11,
        loaded.delta,
        Rules::new(loaded.percent_min, loaded.percent_max),
        loaded.periodic,
        loaded.generation,
        loaded.grid.clone(),
    );

    // Drain snapshot 0 of loaded state
    std::thread::sleep(Duration::from_millis(20));
    let mut initial_loaded_snap = None;
    while let Ok(snap) = worker.try_recv_snapshot() {
        if snap.epoch == 11 {
            initial_loaded_snap = Some(snap);
        }
    }
    let snap0 = initial_loaded_snap.expect("Loaded state snapshot should be delivered");
    assert_eq!(snap0.generation, 10);
    assert_eq!(snap0.alive_count, dump.alive_count);

    // 3. Step both sim1 and worker in lockstep for 5 more generations
    for step_num in 1..=5 {
        sim1.step();
        worker.step();
        std::thread::sleep(Duration::from_millis(25));

        let mut step_snap = None;
        while let Ok(snap) = worker.try_recv_snapshot() {
            if snap.epoch == 11 {
                step_snap = Some(snap);
            }
        }
        let snap = step_snap.expect("Step snapshot expected");
        assert_eq!(snap.generation, 10 + step_num);
        assert_eq!(
            snap.alive_count,
            sim1.count_alive(),
            "Mismatch at step {}",
            step_num
        );
    }
}

#[test]
fn test_dump_integrity_checks() {
    let dims = GridDimensions::new(5, 3);
    let rules = Rules::new(20.0, 45.0);
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    let mut sim = Simulation::new(dims.size, dims.dimensions, rules);
    sim.randomize(&mut rng);

    let dump = SimulationDump {
        dimensions: dims.dimensions,
        size: dims.size,
        delta: 2,
        percent_min: 20.0,
        percent_max: 45.0,
        periodic: true,
        color_mode: ColorMode::Hyperdimension,
        generation: 0,
        total_cells: dims.total_cells,
        alive_count: sim.count_alive(),
        grid: sim.current.clone(),
    };

    let mut bytes = dump.to_bytes().unwrap();

    // 1. Truncated header
    let truncated = &bytes[..40];
    assert!(matches!(
        SimulationDump::from_bytes(truncated),
        Err(DumpError::TruncatedHeader { .. })
    ));

    // 2. Corrupted magic
    bytes[0] = b'X';
    assert!(matches!(
        SimulationDump::from_bytes(&bytes),
        Err(DumpError::InvalidMagic)
    ));
    bytes[0] = b'G'; // restore

    // 3. Corrupted payload (triggers CRC32 ChecksumMismatch)
    let last_idx = bytes.len() - 1;
    bytes[last_idx] ^= 0xFF;
    let res = SimulationDump::from_bytes(&bytes);
    assert!(
        matches!(res, Err(DumpError::ChecksumMismatch { .. }) | Err(DumpError::Decompress(_))),
        "Expected checksum or decompress error, got {:?}",
        res
    );
}
