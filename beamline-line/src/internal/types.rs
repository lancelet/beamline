use bytemuck::{Pod, Zeroable};

/// A styled line.
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct StyledLine {
    line: Line,
    style: Style,
}

/// A line with two end points.
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct Line {
    start: P2,
    end: P2,
}

/// A 3D point.
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct P2 {
    x: f32,
    y: f32,
}

/// Line style.
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct Style {
    width: f32,
    cap: Cap,
    color: Color,
}

/// Color.
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct Color {
    red: f32,
    green: f32,
    blue: f32,
    alpha: f32,
}

/// Line cap.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cap {
    Round = 0,
    Butt = 1,
    Square = 2,
}
unsafe impl Pod for Cap {}
unsafe impl Zeroable for Cap {}
