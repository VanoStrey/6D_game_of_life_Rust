// 3D Instanced Cube Shader matching JavaFX Phong/Lambert aesthetics

struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct InstanceInput {
    @location(2) inst_position: vec3<f32>,
    @location(3) inst_color: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) world_pos: vec3<f32>,
};

@vertex
fn vs_main(
    vertex: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;

    let world_pos = vertex.position + instance.inst_position;
    out.world_pos = world_pos;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.world_normal = vertex.normal;
    out.color = instance.inst_color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Directional lighting from top-front
    let light_dir1 = normalize(vec3<f32>(0.4, 0.8, -0.6));
    let light_dir2 = normalize(vec3<f32>(-0.5, -0.3, 0.6));

    let diff1 = max(dot(in.world_normal, light_dir1), 0.0);
    let diff2 = max(dot(in.world_normal, light_dir2), 0.0) * 0.3;
    let ambient = 0.35;

    let lighting = ambient + diff1 * 0.65 + diff2;
    let final_rgb = in.color * lighting;

    return vec4<f32>(final_rgb, 1.0);
}
