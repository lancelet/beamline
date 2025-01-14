use crate::{P2, V2};

/// Line.
///
/// To construct a line, use [`Line::new`].
#[derive(Debug, Clone)]
pub struct Line {
    /// Start point of the line.
    a: P2,
    /// End point of the line.
    b: P2,
}
impl Line {
    /// Create a new line from two points.
    ///
    /// The points must not (approximately) lie on top of each other, otherwise
    /// a degenerate line is produced.
    ///
    /// # Parameters
    ///
    /// - `a`: An end-point of the line.
    /// - `b`: The other end-point of the line.
    ///
    /// # Returns
    ///
    /// A new line.
    pub fn new(a: P2, b: P2) -> Line {
        Line { a, b }
    }

    /// Returns the start point of the line.
    pub fn start(&self) -> P2 {
        self.a
    }

    /// Returns the end point of the line.
    pub fn end(&self) -> P2 {
        self.b
    }

    /// Return the vector along the line.
    ///
    /// The returned vector starts from the beginning of the line, and is the
    /// length of the line, in the direction of the end of the line.
    pub fn ab_vec(&self) -> V2 {
        self.b - self.a
    }

    /// Evaluate the line at parameter value `t`.
    ///
    /// `t` is a parameter which is in the range `[0.0, 1.0]` for the line.
    /// A value of `t=0` corresponds to the point `a` at the start of the line,
    /// while a value of `t=1` corresponds to the point `b` a the end of the
    /// line. `t` can take values outside this range to obtain collinear points
    /// that are on the same line but outside the start-end range.
    ///
    /// # Parameters
    ///
    /// - `t`: Parameter value at which to evaluate the line.
    ///
    /// # Returns
    ///
    /// Point of the line at `t`.
    pub fn eval_param(&self, t: f32) -> P2 {
        self.a + t * self.ab_vec()
    }
}
