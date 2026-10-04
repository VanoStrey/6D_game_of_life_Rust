pub mod camera;
pub mod color;
pub mod gpu_state;
pub mod instance;
pub mod mesh;

pub use camera::{Camera, CameraUniform};
pub use color::{
    color_for_hypercoords, color_for_projected_coords, hsv_to_rgb, ColorMode, BASE_3D_BLUE,
    JAVA_DARK_BLUE,
};
pub use gpu_state::GpuState;
pub use instance::{create_instances_from_positions, create_instances_with_coloring, InstanceRaw};
pub use mesh::{create_cube_geometry, Vertex};
