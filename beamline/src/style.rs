//! Line styles.

/// Describes the cap at the end of lines.
#[derive(Debug)]
pub enum LineCap {
    /// Squared ends that do not extend beyond the end-point of the line.
    Butt,
    /// Rounded end-points. Each end is a semi-circle with radius equal to
    /// half of the line width.
    Round,
    /// Squared ends that extend beyond the end of the line by half of the
    /// line width.
    Square,
}

/// Color for a line.
#[derive(Debug)]
pub struct Color {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}
impl Color {
    pub fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Color {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }
}

/// Style attributes of a line.
#[derive(Debug)]
pub struct LineStyle {
    /// Width of the line.
    pub width: f32,
    /// Line cap.
    pub cap: LineCap,
    /// Color of the line.
    pub color: Color,
}
