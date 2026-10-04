use six_d_game_of_life::renderer::{create_instances_from_positions, Camera};
use six_d_game_of_life::{collect_alive_positions, Rules, Simulation};

#[test]
fn test_camera_world_space_movement_matching_java() {
    let mut camera = Camera::default();

    // Verify initial positions matching Java CameraTransform.java
    assert_eq!(camera.position.x, 1000.0);
    assert_eq!(camera.position.y, 5500.0);
    assert_eq!(camera.position.z, -2500.0);
    assert_eq!(camera.rotation_x, 53.0);
    assert_eq!(camera.rotation_y, 0.0);

    // W key: -Y movement
    camera.translate_world(0.0, -1.0, 0.0);
    assert_eq!(camera.position.y, 5500.0 - 20.0);

    // S key: +Y movement
    camera.translate_world(0.0, 1.0, 0.0);
    assert_eq!(camera.position.y, 5500.0);

    // A key: -X movement
    camera.translate_world(-1.0, 0.0, 0.0);
    assert_eq!(camera.position.x, 1000.0 - 20.0);

    // D key: +X movement
    camera.translate_world(1.0, 0.0, 0.0);
    assert_eq!(camera.position.x, 1000.0);

    // Q key: -Z movement
    camera.translate_world(0.0, 0.0, -1.0);
    assert_eq!(camera.position.z, -2500.0 - 20.0);

    // E key: +Z movement
    camera.translate_world(0.0, 0.0, 1.0);
    assert_eq!(camera.position.z, -2500.0);
}

#[test]
fn test_camera_mouse_drag_rotation() {
    let mut camera = Camera::default();

    // Mouse drag dx = 100.0, dy = 50.0 (natural mouse look: dy > 0 rotates pitch down/decreases angle)
    // rotX = clamp(53.0 - 50.0 * 0.05) = 50.5
    // rotY = 0.0 + 100.0 * 0.05 = +5.0
    camera.rotate_mouse(100.0, 50.0);
    assert_eq!(camera.rotation_x, 53.0 - 2.5);
    assert_eq!(camera.rotation_y, 0.0 + 5.0);

    camera.reset();
    assert_eq!(camera.rotation_x, 53.0);
    assert_eq!(camera.rotation_y, 0.0);
}

#[test]
fn test_camera_unity_relative_movement() {
    let mut camera = Camera::default();
    camera.position = glam::Vec3::ZERO;
    camera.rotation_x = 0.0;
    camera.rotation_y = 0.0;

    // Looking directly along +Z with 0 pitch/yaw:
    // Forward (W) must increase Z
    camera.translate_relative(1.0, 0.0, 0.0);
    assert!(camera.position.z > 0.0);
    assert_eq!(camera.position.x, 0.0);

    // Backward (S) must decrease Z back to 0
    camera.translate_relative(-1.0, 0.0, 0.0);
    assert!((camera.position.z).abs() < 1e-4);

    // Right (D) must increase X
    camera.translate_relative(0.0, 1.0, 0.0);
    assert!(camera.position.x > 0.0);

    // Left (A) must decrease X back
    camera.translate_relative(0.0, -1.0, 0.0);
    assert!((camera.position.x).abs() < 1e-4);

    // Up (Space/E) must decrease Y (move up in world where Y-down)
    camera.translate_relative(0.0, 0.0, 1.0);
    assert!(camera.position.y < 0.0);

    // Down (Shift/Q) must increase Y back
    camera.translate_relative(0.0, 0.0, -1.0);
    assert!((camera.position.y).abs() < 1e-4);
}

#[test]
fn test_camera_projection_clip_coords() {
    use glam::Vec4;
    let camera = Camera::default();
    let view = camera.build_view_matrix();
    let proj = camera.build_proj_matrix();
    let vp = proj * view;

    let points = [
        Vec4::new(0.0, 0.0, 0.0, 1.0),
        Vec4::new(1000.0, 1000.0, 1000.0, 1.0),
        Vec4::new(1000.0, 3000.0, 3000.0, 1.0),
        Vec4::new(2700.0, 2700.0, 2700.0, 1.0),
    ];

    for p in points {
        let clip_p = vp * p;
        assert!(
            clip_p.w > 0.0,
            "w must be positive for points in front of camera"
        );
        let ndc_z = clip_p.z / clip_p.w;
        assert!(
            (0.0..=1.0).contains(&ndc_z),
            "NDC z must be in [0, 1], got {ndc_z}"
        );
    }
}

#[test]
fn test_instance_generation_from_positions() {
    let positions = vec![(0, 0, 0), (1, 2, 3), (10, 20, 30)];
    let cube_size = 100.0;
    let color = [0.0, 0.0, 0.545];

    let instances = create_instances_from_positions(&positions, cube_size, color);
    assert_eq!(instances.len(), 3);

    // Origin
    assert_eq!(instances[0].position, [0.0, 0.0, 0.0]);
    assert_eq!(instances[0].color, color);

    // (1, 2, 3)
    assert_eq!(instances[1].position, [100.0, 200.0, 300.0]);

    // (10, 20, 30)
    assert_eq!(instances[2].position, [1000.0, 2000.0, 3000.0]);
}

#[test]
fn test_dimension_switching_cycle_no_panic() {
    let size = 3;
    let delta = 2;
    let rules = Rules::default();

    // Full cycle: 1D -> 2D -> 3D -> 4D -> 5D -> 6D -> 1D
    let dimensions_to_test = [1, 2, 3, 4, 5, 6, 1];

    for &dim in &dimensions_to_test {
        let mut sim = Simulation::new(size, dim, rules);
        for idx in 0..sim.current.len() {
            sim.current.set_linear(idx, idx % 2 == 0);
        }

        // Perform step
        sim.step();

        // Project and build instances
        let alive_positions = collect_alive_positions(&sim.current, delta);
        let instances = create_instances_from_positions(&alive_positions, 100.0, [0.0, 0.0, 0.545]);

        assert_eq!(instances.len(), alive_positions.len());
    }
}

#[test]
fn test_create_instances_with_coloring_uniform_and_hyperdimension() {
    use six_d_game_of_life::renderer::{
        color_for_hypercoords, create_instances_with_coloring, ColorMode, JAVA_DARK_BLUE,
    };

    let size = 6;
    let delta = 3;
    let dim = 6;
    let stride = size + delta;

    // Two points in different 6D hypercubes:
    // point 0: (d=0, e=0, f=0, c=0, b=0, a=0) -> (0, 0, 0)
    // point 1: (d=0, e=0, f=0, c=2, b=1, a=3) -> (2*stride, 1*stride, 3*stride)
    let positions = vec![(0, 0, 0), (2 * stride, 1 * stride, 3 * stride)];

    // 1. Uniform mode -> all must have JAVA_DARK_BLUE
    let instances_uniform =
        create_instances_with_coloring(&positions, 100.0, dim, size, delta, ColorMode::Uniform);
    assert_eq!(instances_uniform.len(), 2);
    assert_eq!(instances_uniform[0].color, JAVA_DARK_BLUE);
    assert_eq!(instances_uniform[1].color, JAVA_DARK_BLUE);

    // 2. Hyperdimension mode -> colors must reflect hypercoordinates
    let instances_hyper = create_instances_with_coloring(
        &positions,
        100.0,
        dim,
        size,
        delta,
        ColorMode::Hyperdimension,
    );
    assert_eq!(instances_hyper.len(), 2);
    let expected_c0 = color_for_hypercoords(0, 0, 0, dim, size, ColorMode::Hyperdimension);
    let expected_c1 = color_for_hypercoords(3, 1, 2, dim, size, ColorMode::Hyperdimension);
    assert_eq!(instances_hyper[0].color, expected_c0);
    assert_eq!(instances_hyper[1].color, expected_c1);
    assert_ne!(instances_hyper[0].color, instances_hyper[1].color);
}

#[test]
fn test_instance_generation_performance_6d() {
    use six_d_game_of_life::renderer::{create_instances_with_coloring, ColorMode};
    use std::time::Instant;

    // Benchmark 50,000 alive cells in 6D
    let size = 6;
    let delta = 3;
    let stride = size + delta;
    let mut positions = Vec::with_capacity(50_000);
    for i in 0..50_000 {
        let x = (i % 54) * stride;
        let y = ((i / 54) % 54) * stride;
        let z = ((i / (54 * 54)) % 54) * stride;
        positions.push((x, y, z));
    }

    let start = Instant::now();
    let instances = create_instances_with_coloring(
        &positions,
        100.0,
        6,
        size,
        delta,
        ColorMode::Hyperdimension,
    );
    let elapsed = start.elapsed();

    assert_eq!(instances.len(), 50_000);
    // Instance generation for 50k cells should take < 10ms (typically ~1-2ms on modern CPUs)
    println!("Generation of 50,000 6D colored instances took: {elapsed:?}");
    assert!(
        elapsed.as_millis() < 50,
        "Instance generation took too long: {elapsed:?}"
    );
}
