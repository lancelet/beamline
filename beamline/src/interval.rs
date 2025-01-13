/// Interval of floating-point values.
///
/// It includes both its end points.
#[derive(Debug)]
pub struct Interval {
    start: f32,
    end: f32,
}
impl Interval {
    /// Create a singleton interval which contains just one `f32` value.
    pub fn singleton(value: f32) -> Interval {
        Interval {
            start: value,
            end: value,
        }
    }

    /// Expand an interval, if necessary, to include another `f32` value.
    pub fn include(&mut self, value: f32) {
        if value < self.start {
            self.start = value;
        } else if value > self.end {
            self.end = value;
        }
    }

    /// Test if two intervals are completely disjoint from one another.
    pub fn disjoint(&self, other: &Interval) -> bool {
        self.end < other.start || other.end < self.start
    }
}
