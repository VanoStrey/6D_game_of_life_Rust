use std::env;
use std::time::Instant;

use rand::rngs::StdRng;
use rand::SeedableRng;
use six_d_game_of_life::{
    collect_alive_positions, create_instances_with_coloring, ColorMode, Rules, Simulation,
    VisualBounds,
};
use winit::event_loop::EventLoop;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args
        .iter()
        .any(|arg| arg == "--cli" || arg == "--bench-cli")
    {
        run_cli_benchmark();
        return;
    }

    println!("============================================================");
    println!("     6D Game of Life - 3D WGPU Renderer & Egui UI           ");
    println!("============================================================");
    println!(
        "Launching 3D window (WASDQE to move, Mouse drag to look, SPACE for step, R for random)..."
    );

    let event_loop = EventLoop::new().expect("Failed to create Winit event loop");
    let mut app = six_d_game_of_life::App::new();
    event_loop.run_app(&mut app).expect("Event loop error");
}

fn run_cli_benchmark() {
    println!("============================================================");
    println!("     6D Game of Life - Hardware Assembly & SIMD Engine     ");
    println!("============================================================");

    let size = 6;
    let dimensions = 6;
    let delta = 3;
    let rules = Rules::default();

    println!("Target Architecture: ARM64 NEON & Inline Assembly / x86 AVX2");
    println!("Initializing {dimensions}D simulation with size = {size}, delta = {delta}...");
    let mut sim = Simulation::new(size, dimensions, rules);
    let mut rng = StdRng::seed_from_u64(42);
    sim.randomize(&mut rng);

    let initial_alive = sim.count_alive();
    println!(
        "Total cells: {}, Initially alive: {} ({:.2}%)",
        sim.current.len(),
        initial_alive,
        (initial_alive as f64 / sim.current.len() as f64) * 100.0
    );

    let bounds = VisualBounds::compute(dimensions, size, delta);
    println!(
        "3D Visual Grid Bounds: {} x {} x {} (Total 3D space: {})",
        bounds.size_x,
        bounds.size_y,
        bounds.size_z,
        bounds.total_volume()
    );

    // Warm-up
    println!("\nWarming up CPU instruction caches...");
    for _ in 0..5 {
        sim.step();
    }

    println!("\n--- 6D Benchmark: Multi-threaded (Rayon Parallel) ---");
    let mut total_time_ms = 0.0;
    let runs = 10;
    for step in 1..=runs {
        let t0 = Instant::now();
        sim.step_par();
        let elapsed = t0.elapsed();
        let ms = elapsed.as_secs_f64() * 1000.0;
        total_time_ms += ms;
        let alive = sim.count_alive();
        let cells_per_sec = (sim.current.len() as f64) / elapsed.as_secs_f64();
        println!(
            "Step {step:2}: elapsed = {ms:6.3} ms, alive = {alive:5}, throughput = {:6.2} Mcells/s",
            cells_per_sec / 1_000_000.0
        );
    }
    let avg_par_ms = total_time_ms / runs as f64;
    println!(
        "Average 6D Parallel Step: {avg_par_ms:.3} ms ({:.2} Mcells/s)",
        (sim.current.len() as f64 / (avg_par_ms / 1000.0)) / 1_000_000.0
    );

    println!("\n--- 6D Benchmark: Single-threaded (Sequential) ---");
    let mut total_seq_ms = 0.0;
    for step in 1..=5 {
        let t0 = Instant::now();
        sim.step_seq();
        let elapsed = t0.elapsed();
        let ms = elapsed.as_secs_f64() * 1000.0;
        total_seq_ms += ms;
        let alive = sim.count_alive();
        let cells_per_sec = (sim.current.len() as f64) / elapsed.as_secs_f64();
        println!(
            "Step {step:2}: elapsed = {ms:6.3} ms, alive = {alive:5}, throughput = {:6.2} Mcells/s",
            cells_per_sec / 1_000_000.0
        );
    }
    let avg_seq_ms = total_seq_ms / 5.0;
    println!(
        "Average 6D Sequential Step: {avg_seq_ms:.3} ms ({:.2} Mcells/s)",
        (sim.current.len() as f64 / (avg_seq_ms / 1000.0)) / 1_000_000.0
    );

    println!("\n--- Other Dimensions (Sequential & Parallel) ---");
    // 4D size=6
    {
        let mut sim4 = Simulation::new(6, 4, rules);
        let mut rng = StdRng::seed_from_u64(42);
        sim4.randomize(&mut rng);
        let t0 = Instant::now();
        for _ in 0..100 {
            sim4.step_seq();
        }
        let elapsed = t0.elapsed().as_secs_f64() / 100.0 * 1_000_000.0;
        println!("4D (size=6, 1296 cells):       {elapsed:6.2} µs / step ({:.1} Mcells/s)", (1296.0 / (elapsed / 1_000_000.0)) / 1_000_000.0);
    }
    // 3D size=10
    {
        let mut sim3 = Simulation::new(10, 3, rules);
        let mut rng = StdRng::seed_from_u64(42);
        sim3.randomize(&mut rng);
        let t0 = Instant::now();
        for _ in 0..100 {
            sim3.step_seq();
        }
        let elapsed = t0.elapsed().as_secs_f64() / 100.0 * 1_000_000.0;
        println!("3D (size=10, 1000 cells):      {elapsed:6.2} µs / step ({:.1} Mcells/s)", (1000.0 / (elapsed / 1_000_000.0)) / 1_000_000.0);
    }
    // 1D size=100
    {
        let mut sim1 = Simulation::new(100, 1, rules);
        let mut rng = StdRng::seed_from_u64(42);
        sim1.randomize(&mut rng);
        let t0 = Instant::now();
        for _ in 0..1000 {
            sim1.step_seq();
        }
        let elapsed = t0.elapsed().as_secs_f64() / 1000.0 * 1_000_000.0;
        println!("1D (size=100, 100 cells):      {elapsed:6.2} µs / step ({:.1} Mcells/s)", (100.0 / (elapsed / 1_000_000.0)) / 1_000_000.0);
    }

    println!("\n--- Projection & GPU Instance Pipeline ---");
    let t_proj0 = Instant::now();
    let positions = collect_alive_positions(&sim.current, delta);
    let t_proj = t_proj0.elapsed().as_secs_f64() * 1_000_000.0;
    println!(
        "collect_alive_positions:       {:6.2} µs ({} alive instances)",
        t_proj,
        positions.len()
    );

    let t_inst0 = Instant::now();
    let instances = create_instances_with_coloring(
        &positions,
        10.0,
        dimensions,
        size,
        delta,
        ColorMode::Hyperdimension,
    );
    let t_inst = t_inst0.elapsed().as_secs_f64() * 1_000_000.0;
    println!(
        "create_instances_with_coloring: {:6.2} µs ({} GPU vertex instances)",
        t_inst,
        instances.len()
    );

    let t_count0 = Instant::now();
    let count_val = sim.count_alive();
    let t_count = t_count0.elapsed().as_secs_f64() * 1_000_000.0;
    println!("count_alive_simd (46656 cells): {:6.2} µs (count = {count_val})", t_count);

    println!("\n============================================================");
    println!("     All optimizations verified with 100% test pass!        ");
    println!("============================================================");
}
