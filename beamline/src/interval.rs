/// Interval of floating-point values.
///
/// It includes both its end points.
#[derive(Debug)]
pub struct Interval {
    start: f32,
    end: f32,
}
impl Interval {
    /// Creates an interval with two end points.
    ///
    /// The order of `p1` and `p2` is not important.
    pub fn new(p1: f32, p2: f32) -> Interval {
        let mut interval = Interval::singleton(p1);
        interval.include(p2);
        interval
    }

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

    /// Tests if a value is contained by this interval.
    pub fn contains(&self, value: f32) -> bool {
        value >= self.start && value <= self.end
    }

    /// Tests if two intervals are completely disjoint from one another.
    pub fn disjoint(&self, other: &Interval) -> bool {
        self.end < other.start || other.end < self.start
    }

    /// Tests if two intervals overlap.
    pub fn overlaps(&self, other: &Interval) -> bool {
        !self.disjoint(other)
    }

    /// Returns the minimum value of an interval.
    pub fn min(&self) -> f32 {
        self.start
    }

    /// Returns the maximum value of an interval.
    pub fn max(&self) -> f32 {
        self.end
    }
}
