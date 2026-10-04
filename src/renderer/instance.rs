//! GPU Instance data for rendering alive cells.

use bytemuck::{Pod, Zeroable};

/// Raw GPU layout for each cube instance.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct InstanceRaw {
    pub position: [f32; 3],
    pub _pad0: f32,
    pub color: [f32; 3],
    pub _pad1: f32,
}

impl InstanceRaw {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // instance position (vec3<f32>)
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // instance color (vec3<f32>)
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

use crate::renderer::color::{color_for_projected_coords, ColorMode, JAVA_DARK_BLUE};
use rayon::prelude::*;

/// Converts projected discrete 3D positions into continuous world-space GPU instances with base color.
///
/// In JavaFX: `cube.setTranslateX(x * 100)`, `cube.setTranslateY(y * 100)`, `cube.setTranslateZ(z * 100)`.
pub fn create_instances_from_positions(
    positions: &[(usize, usize, usize)],
    cube_size: f32,
    base_color: [f32; 3],
) -> Vec<InstanceRaw> {
    if positions.len() >= 2048 {
        positions
            .par_iter()
            .map(|&(x, y, z)| InstanceRaw {
                position: [
                    x as f32 * cube_size,
                    y as f32 * cube_size,
                    z as f32 * cube_size,
                ],
                _pad0: 0.0,
                color: base_color,
                _pad1: 0.0,
            })
            .collect()
    } else {
        positions
            .iter()
            .map(|&(x, y, z)| InstanceRaw {
                position: [
                    x as f32 * cube_size,
                    y as f32 * cube_size,
                    z as f32 * cube_size,
                ],
                _pad0: 0.0,
                color: base_color,
                _pad1: 0.0,
            })
            .collect()
    }
}


/// Converts discrete positions into GPU instances with hyperdimensional slice color coding.
///
/// Computes per-instance color in O(1) time without extra heap allocations, preserving
/// the single draw call for all instances.
pub fn create_instances_with_coloring(
    positions: &[(usize, usize, usize)],
    cube_size: f32,
    dimensions: usize,
    size: usize,
    delta: usize,
    color_mode: ColorMode,
) -> Vec<InstanceRaw> {
    if color_mode == ColorMode::Uniform {
        return create_instances_from_positions(positions, cube_size, JAVA_DARK_BLUE);
    }

    let stride = (size + delta).max(1);

    if dimensions == 6 {
        let max_dim = size.min(64);
        let mut r_table = [0.0f32; 64];
        let mut g_table = [0.0f32; 64];
        let mut b_table = [0.0f32; 64];
        for c in 0..max_dim {
            let tc = if size > 1 {
                (c as f32 / (size - 1) as f32).clamp(0.0, 1.0)
            } else {
                0.0
            };
            r_table[c] = (0.18 + 0.78 * tc).clamp(0.0, 1.0);
        }
        for b in 0..max_dim {
            let tb = if size > 1 {
                (b as f32 / (size - 1) as f32).clamp(0.0, 1.0)
            } else {
                0.0
            };
            g_table[b] = (0.18 + 0.78 * tb).clamp(0.0, 1.0);
        }
        for a in 0..max_dim {
            let ta = if size > 1 {
                (a as f32 / (size - 1) as f32).clamp(0.0, 1.0)
            } else {
                0.0
            };
            b_table[a] = (0.25 + 0.72 * ta).clamp(0.0, 1.0);
        }

        let cap = max_dim.saturating_sub(1);
        if positions.len() >= 2048 {
            return positions
                .par_iter()
                .map(|&(x, y, z)| {
                    let c = (x / stride).min(cap);
                    let b = (y / stride).min(cap);
                    let a = (z / stride).min(cap);
                    InstanceRaw {
                        position: [
                            x as f32 * cube_size,
                            y as f32 * cube_size,
                            z as f32 * cube_size,
                        ],
                        _pad0: 0.0,
                        color: [r_table[c], g_table[b], b_table[a]],
                        _pad1: 0.0,
                    }
                })
                .collect();
        } else {
            return positions
                .iter()
                .map(|&(x, y, z)| {
                    let c = (x / stride).min(cap);
                    let b = (y / stride).min(cap);
                    let a = (z / stride).min(cap);
                    InstanceRaw {
                        position: [
                            x as f32 * cube_size,
                            y as f32 * cube_size,
                            z as f32 * cube_size,
                        ],
                        _pad0: 0.0,
                        color: [r_table[c], g_table[b], b_table[a]],
                        _pad1: 0.0,
                    }
                })
                .collect();
        }
    }


    positions
        .iter()
        .map(|&(x, y, z)| {
            let color = color_for_projected_coords(x, y, z, dimensions, size, delta, color_mode);
            InstanceRaw {
                position: [
                    x as f32 * cube_size,
                    y as f32 * cube_size,
                    z as f32 * cube_size,
                ],
                _pad0: 0.0,
                color,
                _pad1: 0.0,
            }
        })
        .collect()

}
