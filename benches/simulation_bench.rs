use criterion::{criterion_group, criterion_main, Criterion};
use rand::rngs::StdRng;
use rand::SeedableRng;
use six_d_game_of_life::{Rules, Simulation};

fn bench_simulations(c: &mut Criterion) {
    let rules = Rules::default();

    // 1D: size = 100
    {
        let mut sim = Simulation::new(100, 1, rules);
        let mut rng = StdRng::seed_from_u64(42);
        sim.randomize(&mut rng);
        c.bench_function("step_1d_size100", |b| {
            b.iter(|| {
                sim.step_seq();
            })
        });
    }

    // 3D: size = 10
    {
        let mut sim = Simulation::new(10, 3, rules);
        let mut rng = StdRng::seed_from_u64(42);
        sim.randomize(&mut rng);
        c.bench_function("step_3d_size10", |b| {
            b.iter(|| {
                sim.step();
            })
        });
    }

    // 4D: size = 6 (default)
    {
        let mut sim = Simulation::new(6, 4, rules);
        let mut rng = StdRng::seed_from_u64(42);
        sim.randomize(&mut rng);
        c.bench_function("step_4d_size6", |b| {
            b.iter(|| {
                sim.step();
            })
        });
    }

    // 6D: size = 6 (heavy baseline)
    {
        let mut sim = Simulation::new(6, 6, rules);
        let mut rng = StdRng::seed_from_u64(42);
        sim.randomize(&mut rng);
        c.bench_function("step_6d_size6_parallel", |b| {
            b.iter(|| {
                sim.step_par();
            })
        });
    }
}

criterion_group!(benches, bench_simulations);
criterion_main!(benches);
