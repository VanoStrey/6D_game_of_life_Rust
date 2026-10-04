//! Shared 3D Cube geometry for instanced rendering.

use bytemuck::{Pod, Zeroable};

/// Vertex format with position and surface normal.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

impl Vertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position (vec3<f32>)
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // normal (vec3<f32>)
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

/// Generates a standard unit cube of half-size 50.0 (total side 100.0, matching JavaFX cubeSize = 100.0).
pub fn create_cube_geometry() -> (Vec<Vertex>, Vec<u16>) {
    let h = 50.0f32; // half-size

    #[rustfmt::skip]
    let vertices = vec![
        // Front (+Z)
        Vertex { position: [-h, -h,  h], normal: [ 0.0,  0.0,  1.0] },
        Vertex { position: [ h, -h,  h], normal: [ 0.0,  0.0,  1.0] },
        Vertex { position: [ h,  h,  h], normal: [ 0.0,  0.0,  1.0] },
        Vertex { position: [-h,  h,  h], normal: [ 0.0,  0.0,  1.0] },

        // Back (-Z)
        Vertex { position: [ h, -h, -h], normal: [ 0.0,  0.0, -1.0] },
        Vertex { position: [-h, -h, -h], normal: [ 0.0,  0.0, -1.0] },
        Vertex { position: [-h,  h, -h], normal: [ 0.0,  0.0, -1.0] },
        Vertex { position: [ h,  h, -h], normal: [ 0.0,  0.0, -1.0] },

        // Top (+Y)
        Vertex { position: [-h,  h,  h], normal: [ 0.0,  1.0,  0.0] },
        Vertex { position: [ h,  h,  h], normal: [ 0.0,  1.0,  0.0] },
        Vertex { position: [ h,  h, -h], normal: [ 0.0,  1.0,  0.0] },
        Vertex { position: [-h,  h, -h], normal: [ 0.0,  1.0,  0.0] },

        // Bottom (-Y)
        Vertex { position: [-h, -h, -h], normal: [ 0.0, -1.0,  0.0] },
        Vertex { position: [ h, -h, -h], normal: [ 0.0, -1.0,  0.0] },
        Vertex { position: [ h, -h,  h], normal: [ 0.0, -1.0,  0.0] },
        Vertex { position: [-h, -h,  h], normal: [ 0.0, -1.0,  0.0] },

        // Right (+X)
        Vertex { position: [ h, -h,  h], normal: [ 1.0,  0.0,  0.0] },
        Vertex { position: [ h, -h, -h], normal: [ 1.0,  0.0,  0.0] },
        Vertex { position: [ h,  h, -h], normal: [ 1.0,  0.0,  0.0] },
        Vertex { position: [ h,  h,  h], normal: [ 1.0,  0.0,  0.0] },

        // Left (-X)
        Vertex { position: [-h, -h, -h], normal: [-1.0,  0.0,  0.0] },
        Vertex { position: [-h, -h,  h], normal: [-1.0,  0.0,  0.0] },
        Vertex { position: [-h,  h,  h], normal: [-1.0,  0.0,  0.0] },
        Vertex { position: [-h,  h, -h], normal: [-1.0,  0.0,  0.0] },
    ];

    #[rustfmt::skip]
    let indices: Vec<u16> = vec![
        0,  1,  2,   0,  2,  3,  // Front
        4,  5,  6,   4,  6,  7,  // Back
        8,  9, 10,   8, 10, 11,  // Top
        12, 13, 14,  12, 14, 15,  // Bottom
        16, 17, 18,  16, 18, 19,  // Right
        20, 21, 22,  20, 22, 23,  // Left
    ];

    (vertices, indices)
}
