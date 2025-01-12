//! Comparison of floating point and vector-floating-point values.

use cgmath::{num_traits::Float, BaseNum, InnerSpace, Point2, Vector2};
use std::ops::{Mul, Neg, Sub};

/// Tolerance to use when performing approximate comparisons.
///
/// When comparing floating-point values, [`Tol::AbsRel`] is a robust choice.
#[derive(Debug, Clone, Copy)]
pub enum Tol<S> {
    /// Absolute tolerance.
    Abs(S),
    /// Relative tolerance.
    Rel(S),
    /// Both absolute and relative tolerance.
    AbsRel { atol: S, rtol: S },
}
impl<S> Tol<S>
where
    S: PartialOrd + Default + Neg<Output = S>,
{
    /// Create a new absolute tolerance.
    pub fn abs(atol: S) -> Tol<S> {
        Tol::Abs(rabs(atol))
    }

    /// Create a new relative tolerance.
    pub fn rel(rtol: S) -> Tol<S> {
        Tol::Rel(rabs(rtol))
    }

    /// Create a new tolerance with absolute and relative components.
    pub fn absrel(atol: S, rtol: S) -> Tol<S> {
        Tol::AbsRel {
            atol: rabs(atol),
            rtol: rabs(rtol),
        }
    }

    pub fn scale(&self, factor: S) -> Tol<S>
    where
        S: Mul<Output = S> + Copy,
    {
        use Tol::{Abs, AbsRel, Rel};
        match self {
            Abs(atol) => Abs(factor * *atol),
            Rel(rtol) => Rel(factor * *rtol),
            AbsRel { atol, rtol } => AbsRel {
                atol: factor * *atol,
                rtol: factor * *rtol,
            },
        }
    }

    /// Fetch a default tolerance, if there is one.
    pub fn default() -> Tol<S>
    where
        S: DefaultTol,
    {
        DefaultTol::default_tol()
    }
}

/// Trait for a default tolerance.
///
/// Default tolerances are associated with the scalar type of a closeness
/// comparison.
pub trait DefaultTol {
    fn default_tol() -> Tol<Self>
    where
        Self: Sized;
}

impl DefaultTol for f32 {
    fn default_tol() -> Tol<f32> {
        Tol::absrel(1e-7, 1e-5)
    }
}

/// Closeness test.
///
/// # Parameters
///
/// - `tol`: Tolerance test to use.
/// - `a`: One value to compare.
/// - `b`: The other value to compare.
///
/// # Returns
///
/// - `true` if the values are sufficiently close.
/// - `false` if the values are not sufficiently close.
pub fn close<T: CloseCmp>(tol: Tol<T::Scalar>, a: &T, b: &T) -> bool {
    CloseCmp::close(tol, a, b)
}

/// Closeness test with a default tolerance.
///
/// # Parameters
///
/// - `a`: One value to compare.
/// - `b`: The other value to compare.
///
/// # Returns
///
/// - `true` if the values are sufficiently close.
/// - `false` if the values are not sufficiently close.
pub fn close_default_tol<T>(a: &T, b: &T) -> bool
where
    T: CloseCmp,
    <T as CloseCmp>::Scalar: DefaultTol,
{
    let tol: Tol<T::Scalar> = DefaultTol::default_tol();
    CloseCmp::close(tol, a, b)
}

/// Trait for types that have a "close" comparison.
pub trait CloseCmp {
    /// Scalar type of the comparison.
    type Scalar;
    /// Closeness test.
    ///
    /// # Parameters
    ///
    /// - `tol`: Tolerance test to use.
    /// - `a`: One value to compare.
    /// - `b`: The other value to compare.
    ///
    /// # Returns
    ///
    /// - `true` if the values are sufficiently close.
    /// - `false` if the values are not sufficiently close.
    fn close(tol: Tol<Self::Scalar>, a: &Self, b: &Self) -> bool;
}

impl CloseCmp for f32 {
    type Scalar = f32;
    fn close(tol: Tol<f32>, a: &f32, b: &f32) -> bool {
        use Tol::{Abs, AbsRel, Rel};
        match tol {
            Abs(atol) => close_atol(atol, *a, *b),
            Rel(rtol) => close_rtol(rtol, *a, *b),
            AbsRel { atol, rtol } => close_artol(atol, rtol, *a, *b),
        }
    }
}

impl<T> CloseCmp for Option<T>
where
    T: CloseCmp,
{
    type Scalar = T::Scalar;
    fn close(tol: Tol<T::Scalar>, a: &Option<T>, b: &Option<T>) -> bool {
        match (a, b) {
            (None, None) => true,
            (Some(x), Some(y)) => CloseCmp::close(tol, x, y),
            (Some(_), None) => false,
            (None, Some(_)) => false,
        }
    }
}

impl<S> CloseCmp for Point2<S>
where
    S: CloseCmp<Scalar = S> + Float + BaseNum,
{
    type Scalar = S;
    fn close(tol: Tol<S>, a: &Point2<S>, b: &Point2<S>) -> bool {
        CloseCmp::close(tol, &(a - b).magnitude(), &S::zero())
    }
}

impl<S> CloseCmp for Vector2<S>
where
    S: CloseCmp<Scalar = S> + Float + BaseNum,
{
    type Scalar = S;
    fn close(tol: Tol<S>, a: &Vector2<S>, b: &Vector2<S>) -> bool {
        CloseCmp::close(tol, &(a - b).magnitude(), &S::zero())
    }
}

/// Check if two numbers are close using both absolute and relative tolerance.
///
/// This implements:
///
/// `(|a - b| <= atol) || (|a - b| <= rtol * max(|a|, |b|)`
fn close_artol<S>(atol: S, rtol: S, a: S, b: S) -> bool
where
    S: Clone + PartialOrd + Default + Neg<Output = S> + Sub<Output = S> + Mul<Output = S>,
{
    close_atol(atol, a.clone(), b.clone()) || close_rtol(rtol, a, b)
}

/// Check if two numbers are close using absolute tolerance.
///
/// This implements:
///
/// `|a - b| <= atol`
///
fn close_atol<S>(atol: S, a: S, b: S) -> bool
where
    S: PartialOrd + Sub<Output = S>,
{
    delta_abs(a, b) <= atol
}

/// Check if two numbers are close using relative tolerance.
///
/// This implements:
///
/// `|a - b| <= rtol * max(|a|, |b|)`
///
fn close_rtol<S>(rtol: S, a: S, b: S) -> bool
where
    S: Clone + PartialOrd + Default + Neg<Output = S> + Sub<Output = S> + Mul<Output = S>,
{
    delta_abs(a.clone(), b.clone()) <= rtol * rmax(rabs(a), rabs(b))
}

/// Return the absolute value of the difference between two values.
///
/// This is equal to: `(a - b).abs()`, but computed without the `abs()`
/// function.
fn delta_abs<S>(a: S, b: S) -> S
where
    S: PartialOrd + Sub<Output = S>,
{
    if a >= b {
        a - b
    } else {
        b - a
    }
}

/// Implements `abs()`.
fn rabs<S>(a: S) -> S
where
    S: PartialOrd + Default + Neg<Output = S>,
{
    if a >= S::default() {
        a
    } else {
        -a
    }
}

/// Return the maximum of two values, using `PartialOrd`.
fn rmax<S>(a: S, b: S) -> S
where
    S: PartialOrd,
{
    if a >= b {
        a
    } else {
        b
    }
}

//// Macros

#[macro_export]
macro_rules! assert_close {
    ($tol:expr, $a: expr, $b: expr) => {
        if (!crate::compare::close($tol, &$a, &$b)) {
            panic!(
                "assertion failed: `(left ≈ right)`
  left:  `{:?}`
  right: `{:?}`
  tol:   `{:?}`",
                $a, $b, $tol
            );
        }
    };
    ($a: expr, $b: expr) => {
        if (!crate::compare::close_default_tol(&$a, &$b)) {
            panic!(
                "assertion failed: `(left ≈ right)`
  left:  `{:?}`
  right: `{:?}`",
                $a, $b
            );
        }
    };
}
