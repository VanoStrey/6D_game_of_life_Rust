//! 3D Perspective Camera replicating Java `CameraTransform.java`.
//!
//! Controls:
//! - World-space WASDQE translations (not view-relative, matching Java):
//!   W: -Y, S: +Y, A: -X, D: +X, Q: -Z, E: +Z
//! - Mouse drag pitch/yaw rotations:
//!   Pitch (X-axis) = -deltaY * 0.05 deg
//!   Yaw (Y-axis) = +deltaX * 0.05 deg

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3, Vec4};

/// GPU-compatible camera uniform buffer.
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 4],
}

impl Default for CameraUniform {
    fn default() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

/// Camera model reproducing Java CameraTransform behavior.
#[derive(Debug, Clone)]
pub struct Camera {
    pub position: Vec3,
    pub rotation_x: f32, // Pitch in degrees
    pub rotation_y: f32, // Yaw in degrees
    pub speed_movement: f32,
    pub speed_rotation: f32,
    pub fov_y_rad: f32,
    pub aspect_ratio: f32,
    pub z_near: f32,
    pub z_far: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            // Exact initial values from Java CameraTransform.java
            position: Vec3::new(1000.0, 5500.0, -2500.0),
            rotation_x: 53.0,
            rotation_y: 0.0,
            speed_movement: 20.0,
            speed_rotation: 0.05,
            fov_y_rad: 60.0f32.to_radians(),
            aspect_ratio: 16.0 / 9.0,
            z_near: 1.0,
            z_far: 50000.0,
        }
    }
}

impl Camera {
    /// Creates a camera with Java default parameters.
    pub fn new(aspect_ratio: f32) -> Self {
        Self {
            aspect_ratio,
            ..Default::default()
        }
    }

    /// Rotation quaternion R = rot_y * rot_x
    #[inline]
    pub fn rotation_quat(&self) -> Quat {
        let rot_x = Quat::from_rotation_x(self.rotation_x.to_radians());
        let rot_y = Quat::from_rotation_y(self.rotation_y.to_radians());
        rot_y * rot_x
    }

    /// Forward direction vector in world space (direction camera is looking).
    #[inline]
    pub fn forward_vector(&self) -> Vec3 {
        self.rotation_quat() * Vec3::new(0.0, 0.0, 1.0)
    }

    /// Right direction vector in world space.
    #[inline]
    pub fn right_vector(&self) -> Vec3 {
        self.rotation_quat() * Vec3::new(1.0, 0.0, 0.0)
    }

    /// Up direction vector in world space (relative to camera view).
    #[inline]
    pub fn up_vector(&self) -> Vec3 {
        self.rotation_quat() * Vec3::new(0.0, -1.0, 0.0)
    }

    /// Translates camera along its local view axes (Unity Editor fly-cam style).
    ///
    /// - `forward`: +1 = forward (W), -1 = backward (S)
    /// - `right`: +1 = right (D), -1 = left (A)
    /// - `up`: +1 = up (Space/E), -1 = down (Shift/Q)
    pub fn translate_relative(&mut self, forward: f32, right: f32, up: f32) {
        let fwd = self.forward_vector();
        let rgt = self.right_vector();
        let upv = self.up_vector();

        let move_dir = fwd * forward + rgt * right + upv * up;
        if move_dir.length_squared() > 0.0 {
            self.position += move_dir.normalize() * self.speed_movement;
        }
    }

    /// Translates camera in World Space.
    pub fn translate_world(&mut self, dx: f32, dy: f32, dz: f32) {
        self.position.x += dx * self.speed_movement;
        self.position.y += dy * self.speed_movement;
        self.position.z += dz * self.speed_movement;
    }

    /// Rotates camera by mouse drag delta.
    pub fn rotate_mouse(&mut self, delta_x: f32, delta_y: f32) {
        // Clamping pitch between -89.5 and 89.5 deg prevents camera flipping upside-down
        self.rotation_x = (self.rotation_x - delta_y * self.speed_rotation).clamp(-89.5, 89.5);
        self.rotation_y += delta_x * self.speed_rotation;
    }

    /// Resets camera to default Java starting transform.
    pub fn reset(&mut self) {
        self.position = Vec3::new(1000.0, 5500.0, -2500.0);
        self.rotation_x = 53.0;
        self.rotation_y = 0.0;
        self.z_far = 50000.0;
        self.speed_movement = 20.0;
    }

    /// Automatically frames camera to center and view the 3D visual bounds of any grid scale.
    pub fn frame_bounds(&mut self, bounds: (usize, usize, usize), cube_size: f32) {
        let size_x = (bounds.0 as f32) * cube_size;
        let size_y = (bounds.1 as f32) * cube_size;
        let size_z = (bounds.2 as f32) * cube_size;

        let max_dim = size_x.max(size_y).max(size_z).max(100.0);
        let center = Vec3::new(size_x * 0.5, size_y * 0.5, size_z * 0.5);

        // Dynamically adjust far clipping plane to prevent any clipping of large worlds
        self.z_far = (max_dim * 12.0).max(100_000.0);

        // Dynamically scale movement speed with world scale
        self.speed_movement = (max_dim * 0.005).clamp(10.0, 1000.0);

        // Position camera back and up from center
        let dist = max_dim * 1.5;
        self.position = Vec3::new(
            center.x,
            center.y + dist * 0.7,
            center.z - dist,
        );
        self.rotation_x = 35.0;
        self.rotation_y = 0.0;
    }

    /// Computes View matrix matching JavaFX `Rotate(rotX, X_AXIS)` and `Rotate(rotY, Y_AXIS)`.
    pub fn build_view_matrix(&self) -> Mat4 {
        let rot_x = Quat::from_rotation_x(self.rotation_x.to_radians());
        let rot_y = Quat::from_rotation_y(self.rotation_y.to_radians());
        let rotation = rot_y * rot_x;

        // In JavaFX, camera looks down +Z initially, Y is down.
        // View matrix is inverse of camera transform:
        Mat4::from_quat(rotation.inverse()) * Mat4::from_translation(-self.position)
    }

    /// Computes Projection matrix with wgpu depth range [0, 1] for Left-Handed +Z view.
    pub fn build_proj_matrix(&self) -> Mat4 {
        let f = 1.0 / (self.fov_y_rad * 0.5).tan();
        let xx = f / self.aspect_ratio;
        let yy = -f; // Invert Y so that JavaFX Y-down in world maps to wgpu NDC Y-up
        let z_near = self.z_near;
        let z_far = self.z_far;
        let z_range_inv = 1.0 / (z_far - z_near);
        let zz = z_far * z_range_inv;
        let tz = -z_near * z_far * z_range_inv;

        Mat4::from_cols(
            Vec4::new(xx, 0.0, 0.0, 0.0),
            Vec4::new(0.0, yy, 0.0, 0.0),
            Vec4::new(0.0, 0.0, zz, 1.0),
            Vec4::new(0.0, 0.0, tz, 0.0),
        )
    }

    /// Generates camera uniform struct for GPU.
    pub fn build_uniform(&self) -> CameraUniform {
        let view = self.build_view_matrix();
        let proj = self.build_proj_matrix();
        let view_proj = proj * view;

        CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
            camera_pos: [self.position.x, self.position.y, self.position.z, 1.0],
        }
    }
}
