//! Error type of the aircraft model.

use aero_geometry::GeometryError;
use thiserror::Error;

/// Errors produced when building an aircraft.
#[derive(Debug, Clone, PartialEq, Error)]
#[non_exhaustive]
pub enum AircraftError {
    /// A dimension or an angle is outside its valid range.
    #[error("invalid aircraft: {0}")]
    Invalid(String),

    /// An airfoil key could not be resolved to coordinates.
    #[error("unknown airfoil {0}")]
    UnknownAirfoil(String),

    /// An airfoil could not be normalized or resampled.
    #[error(transparent)]
    Geometry(#[from] GeometryError),
}

/// Result type of the aircraft model.
pub type Result<T> = std::result::Result<T, AircraftError>;

/// Checks that `value` is finite and strictly positive.
pub(crate) fn positive(name: &str, value: f64) -> Result<f64> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(AircraftError::Invalid(format!(
            "{name} must be positive, got {value}"
        )))
    }
}

/// Checks that `value` is finite and within `[min, max]`.
pub(crate) fn within(name: &str, value: f64, min: f64, max: f64) -> Result<f64> {
    if (min..=max).contains(&value) {
        Ok(value)
    } else {
        Err(AircraftError::Invalid(format!(
            "{name} must be between {min} and {max}, got {value}"
        )))
    }
}
