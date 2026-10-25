//! Error type of the geometry crate.

use thiserror::Error;

/// Errors produced by geometry operations.
#[derive(Debug, Clone, PartialEq, Error)]
#[non_exhaustive]
pub enum GeometryError {
    /// The coordinate arrays do not have the same length.
    #[error("coordinate arrays have different lengths ({x} x values, {y} y values)")]
    LengthMismatch {
        /// Number of x values.
        x: usize,
        /// Number of y values.
        y: usize,
    },

    /// Not enough points to define the geometry.
    #[error("not enough points: {found} given, at least {min} required")]
    TooFewPoints {
        /// Number of points given.
        found: usize,
        /// Minimum number of points required.
        min: usize,
    },

    /// A coordinate is NaN or infinite.
    #[error("coordinate {index} is not a finite number")]
    NonFinite {
        /// Index of the offending point.
        index: usize,
    },

    /// The first or last two points coincide, or three successive points
    /// coincide (two coincident points mark a corner, more is ambiguous).
    #[error("invalid duplicated point at index {index}")]
    DuplicatePoint {
        /// Index of the offending point.
        index: usize,
    },

    /// The contour is ordered clockwise; airfoils must be counterclockwise
    /// (trailing edge → upper surface → leading edge → lower surface).
    #[error("airfoil points are ordered clockwise; expected counterclockwise (Selig) ordering")]
    ClockwiseOrdering,

    /// The geometry is degenerate (zero length, zero chord, ...).
    #[error("degenerate geometry: {0}")]
    Degenerate(String),

    /// Spline abscissas are not non-decreasing, or a segment is too short.
    #[error("invalid spline parameter: {0}")]
    InvalidSpline(String),

    /// An airfoil file could not be parsed.
    #[error("line {line}: {message}")]
    Parse {
        /// 1-based line number in the input.
        line: usize,
        /// Human readable description of the problem.
        message: String,
    },

    /// A NACA designation is not supported.
    #[error("invalid NACA designation \"{0}\"")]
    InvalidNaca(String),

    /// An invalid option value was given.
    #[error("invalid option: {0}")]
    InvalidOption(String),

    /// A numerical procedure failed.
    #[error("numerical failure: {0}")]
    Numerical(String),
}

/// Result type of the geometry crate.
pub type Result<T> = std::result::Result<T, GeometryError>;
