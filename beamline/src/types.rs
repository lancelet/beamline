//! Common types for the beamline renderer.

use cgmath::{Point2, Vector2};

/// 2D point: [`Point2<f32>`].
pub type P2 = Point2<f32>;

/// 2D vector: a [`Vector2<f32>`].
pub type V2 = Vector2<f32>;

/// Rotate a V2 vector 90 degrees anti-clockwise.
pub fn v2_rot90_anticlockwise(v: V2) -> V2 {
    V2::new(-v.y, v.x)
}

/// Compute "ceiling division" for `u32` values: `ceil(a / b)`.
pub fn ceil_div_u32(a: u32, b: u32) -> u32 {
    (a + b - 1) / b
}
