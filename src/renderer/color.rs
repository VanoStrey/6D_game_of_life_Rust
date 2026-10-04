//! Hyperdimensional Color Mapping for 4D/5D/6D cellular automata visualization.
//!
//! Maps multi-dimensional hypercoordinates (a, b, c) to deterministic,
//! high-contrast, visually distinguishable RGB colors.

/// Color mode for rendering alive cells.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum ColorMode {
    /// Classic JavaFX uniform sapphire dark blue (#00008b).
    Uniform,
    /// Hyperdimensional color scheme mapping 4D/5D/6D slice coordinates to distinct hues.
    #[default]
    Hyperdimension,
}

impl ColorMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ColorMode::Uniform => "Uniform (Dark Blue)",
            ColorMode::Hyperdimension => "Hyperdimension (4D-6D)",
        }
    }
}

/// JavaFX legacy dark blue: Color.DARKBLUE = #00008b
pub const JAVA_DARK_BLUE: [f32; 3] = [0.0, 0.0, 0.545];

/// Neutral vibrant blue used for single-slice 1D-3D in Hyperdimension mode.
pub const BASE_3D_BLUE: [f32; 3] = [0.15, 0.45, 0.90];

/// Converts HSV color values to RGB [0.0..1.0].
///
/// `h` is hue in degrees [0.0..360.0), `s` is saturation [0.0..1.0], `v` is value [0.0..1.0].
#[inline]
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h = (h % 360.0 + 360.0) % 360.0;
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r1, g1, b1) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    [r1 + m, g1 + m, b1 + m]
}

/// Determines the RGB color for a cell based on its hypercoordinates.
///
/// - `a`: 6th dimension index (blocks along Z)
/// - `b`: 5th dimension index (blocks along Y)
/// - `c`: 4th dimension index (blocks along X)
/// - `dimensions`: hypergrid dimensionality (1..=6)
/// - `size`: size of each hypergrid dimension
/// - `mode`: active `ColorMode`
#[inline]
pub fn color_for_hypercoords(
    a: usize,
    b: usize,
    c: usize,
    dimensions: usize,
    size: usize,
    mode: ColorMode,
) -> [f32; 3] {
    match mode {
        ColorMode::Uniform => JAVA_DARK_BLUE,
        ColorMode::Hyperdimension => match dimensions {
            // 1D, 2D, 3D: only a single 3D block exists (a=0, b=0, c=0)
            1..=3 => BASE_3D_BLUE,

            // 4D: coordinate `c` varies across slices along X-axis.
            // Map c to a wide spectrum from Azure/Cyan (200°) to Magenta/Red (340°).
            4 => {
                let tc = if size > 1 {
                    (c as f32 / (size - 1) as f32).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let hue = (200.0 + tc * 280.0) % 360.0;
                hsv_to_rgb(hue, 0.88, 0.95)
            }

            // 5D: coordinates `c` (4th dim) and `b` (5th dim) form a 2D grid of 3D slices.
            // `c` controls primary hue, `b` modulates lightness and saturation.
            5 => {
                let tc = if size > 1 {
                    (c as f32 / (size - 1) as f32).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let tb = if size > 1 {
                    (b as f32 / (size - 1) as f32).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let hue = (190.0 + tc * 260.0) % 360.0;
                let sat = (0.95 - 0.40 * tb).clamp(0.0, 1.0);
                let val = (0.60 + 0.38 * tb).clamp(0.0, 1.0);
                hsv_to_rgb(hue, sat, val)
            }

            // 6D: coordinates `c` (4th dim, X), `b` (5th dim, Y), `a` (6th dim, Z).
            // Direct 3-axis chromatic embedding in RGB color space:
            // - c adds Warmth / Red (Coral)
            // - b adds Green (Emerald)
            // - a adds Blue / Violet (Sapphire)
            // Every 3D slice in the 6D hypercube has an unmistakably unique color!
            _ => {
                let tc = if size > 1 {
                    (c as f32 / (size - 1) as f32).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let tb = if size > 1 {
                    (b as f32 / (size - 1) as f32).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let ta = if size > 1 {
                    (a as f32 / (size - 1) as f32).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                let r = (0.18 + 0.78 * tc).clamp(0.0, 1.0);
                let g = (0.18 + 0.78 * tb).clamp(0.0, 1.0);
                let b = (0.25 + 0.72 * ta).clamp(0.0, 1.0);
                [r, g, b]
            }
        },
    }
}

/// Computes the RGB color directly from discrete projected 3D coordinates.
///
/// Inverts `X = d + c * stride`, `Y = e + b * stride`, `Z = f + a * stride`
/// in O(1) time without heap allocations.
#[inline(always)]
pub fn color_for_projected_coords(
    x: usize,
    y: usize,
    z: usize,
    dimensions: usize,
    size: usize,
    delta: usize,
    mode: ColorMode,
) -> [f32; 3] {
    if mode == ColorMode::Uniform || dimensions <= 3 {
        return color_for_hypercoords(0, 0, 0, dimensions, size, mode);
    }

    let stride = (size + delta).max(1);
    let c = x / stride;
    let b = y / stride;
    let a = z / stride;

    color_for_hypercoords(a, b, c, dimensions, size, mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniform_mode_always_returns_legacy_dark_blue() {
        for dim in 1..=6 {
            for a in 0..3 {
                for b in 0..3 {
                    for c in 0..3 {
                        let color = color_for_hypercoords(a, b, c, dim, 6, ColorMode::Uniform);
                        assert_eq!(color, JAVA_DARK_BLUE);
                    }
                }
            }
        }
    }

    #[test]
    fn test_deterministic_output() {
        let c1 = color_for_hypercoords(2, 3, 4, 6, 6, ColorMode::Hyperdimension);
        let c2 = color_for_hypercoords(2, 3, 4, 6, 6, ColorMode::Hyperdimension);
        assert_eq!(c1, c2, "Color mapping must be strictly deterministic");
    }

    #[test]
    fn test_dimensions_1d_to_40d_no_panic_and_valid_rgb() {
        for dim in 1..=40 {
            for size in 1..=8 {
                let color = color_for_hypercoords(
                    size - 1,
                    size - 1,
                    size - 1,
                    dim,
                    size,
                    ColorMode::Hyperdimension,
                );
                for channel in color {
                    assert!(
                        (0.0..=1.0).contains(&channel),
                        "RGB channels must be in [0.0, 1.0], got {channel} for dim {dim}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_distinguishability_4d_slices() {
        let size = 6;
        let dim = 4;
        let mut colors = Vec::new();
        for c in 0..size {
            let color = color_for_hypercoords(0, 0, c, dim, size, ColorMode::Hyperdimension);
            colors.push(color);
        }

        // Verify that different slices have noticeably different colors
        for i in 0..colors.len() {
            for j in (i + 1)..colors.len() {
                let dist = (colors[i][0] - colors[j][0]).abs()
                    + (colors[i][1] - colors[j][1]).abs()
                    + (colors[i][2] - colors[j][2]).abs();
                assert!(
                    dist > 0.10,
                    "Slice {i} and slice {j} should be visually distinct (dist = {dist})"
                );
            }
        }
    }

    #[test]
    fn test_projected_coords_color_inversion() {
        let size = 6;
        let delta = 3;
        let stride = size + delta; // 9
        let dim = 6;

        // Position corresponding to (a=2, b=1, c=4, d=3, e=2, f=1)
        let x = 3 + 4 * stride; // 39
        let y = 2 + 1 * stride; // 11
        let z = 1 + 2 * stride; // 19

        let color_direct = color_for_hypercoords(2, 1, 4, dim, size, ColorMode::Hyperdimension);
        let color_projected =
            color_for_projected_coords(x, y, z, dim, size, delta, ColorMode::Hyperdimension);

        assert_eq!(color_direct, color_projected);
    }

    #[test]
    fn test_distinguishability_5d_and_6d_slices() {
        let size = 5;

        // 5D: check different (b, c) pairs produce distinct colors
        let c_5d_0 = color_for_hypercoords(0, 0, 0, 5, size, ColorMode::Hyperdimension);
        let c_5d_1 = color_for_hypercoords(0, 1, 0, 5, size, ColorMode::Hyperdimension);
        let c_5d_2 = color_for_hypercoords(0, 0, 1, 5, size, ColorMode::Hyperdimension);
        assert_ne!(c_5d_0, c_5d_1);
        assert_ne!(c_5d_0, c_5d_2);
        assert_ne!(c_5d_1, c_5d_2);

        // 6D: check that variation along a, b, c changes the color distinctly
        let c_6d_base = color_for_hypercoords(0, 0, 0, 6, size, ColorMode::Hyperdimension);
        let c_6d_a = color_for_hypercoords(2, 0, 0, 6, size, ColorMode::Hyperdimension);
        let c_6d_b = color_for_hypercoords(0, 2, 0, 6, size, ColorMode::Hyperdimension);
        let c_6d_c = color_for_hypercoords(0, 0, 2, 6, size, ColorMode::Hyperdimension);
        assert_ne!(c_6d_base, c_6d_a);
        assert_ne!(c_6d_base, c_6d_b);
        assert_ne!(c_6d_base, c_6d_c);
        assert_ne!(c_6d_a, c_6d_b);
        assert_ne!(c_6d_b, c_6d_c);
    }

    #[test]
    fn test_bounds_and_overflow_protection() {
        // Test size=1 (edge case)
        let c_sz1 = color_for_hypercoords(0, 0, 0, 6, 1, ColorMode::Hyperdimension);
        for ch in c_sz1 {
            assert!((0.0..=1.0).contains(&ch));
        }

        // Test coordinates >= size
        let c_overflow = color_for_hypercoords(100, 100, 100, 6, 5, ColorMode::Hyperdimension);
        for ch in c_overflow {
            assert!((0.0..=1.0).contains(&ch));
        }
    }
}
