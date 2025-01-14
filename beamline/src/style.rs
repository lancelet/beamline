//! Line styles.

use crate::{polygon::Polygon, Line, V2};
use cgmath::InnerSpace;

/// Describes the cap at the end of lines.
#[repr(u32)]
#[derive(Debug, Copy, Clone)]
pub enum LineCap {
    /// Squared ends that do not extend beyond the end-point of the line.
    Butt = 1,
    /// Rounded end-points. Each end is a semi-circle with radius equal to
    /// half of the line width.
    Round = 2,
    /// Squared ends that extend beyond the end of the line by half of the
    /// line width.
    Square = 3,
}

/// Color for a line.
#[derive(Debug, Copy, Clone)]
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
            alpha: alpha.clamp(0.0, 1.0),
        }
    }

    /// Return the color components as an RGBA array.
    pub fn as_array(&self) -> [f32; 4] {
        [self.red, self.green, self.blue, self.alpha]
    }
}

/// Style attributes of a line.
#[derive(Debug, Clone)]
pub struct LineStyle {
    /// Width of the line.
    pub width: f32,
    /// Line cap.
    pub cap: LineCap,
    /// Color of the line.
    pub color: Color,
}

/// A line with an associated style.
#[derive(Debug, Clone)]
pub struct StyledLine {
    pub line: Line,
    pub style: LineStyle,
}
impl StyledLine {
    // Returns a bounding-polygon describing the line.
    //
    // The polygon accounts for the line width and end-cap style.
    pub fn bounding_polygon(&self) -> Polygon {
        assert!(self.style.width > 0.0);

        let w2 = self.style.width / 2.0;
        let v = self.line.ab_vec().normalize();
        let t = V2::new(-v.y, v.x); // Rotate v by 90 degrees.

        // Offset for the end of the line.
        let ofs = match self.style.cap {
            LineCap::Butt => 0.0,
            LineCap::Square => w2,
            LineCap::Round => w2,
        };
        let ov = ofs * v;
        let wt = w2 * t;
        let ovp = ov + wt;
        let ovn = ov - wt;

        let polygon = Polygon::new(vec![
            self.line.start() - ovp,
            self.line.start() - ovn,
            self.line.end() + ovp,
            self.line.end() + ovn,
        ]);

        polygon
    }
}
