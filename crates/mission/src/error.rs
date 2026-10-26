//! Error type of the mission calculations.

use thiserror::Error;

/// Errors produced by the mission calculations.
#[derive(Debug, Clone, PartialEq, Error)]
#[non_exhaustive]
pub enum MissionError {
    /// An input is outside its valid range (non-positive speed or mass,
    /// altitude beyond the model…).
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

/// Result type of the mission calculations.
pub type Result<T> = std::result::Result<T, MissionError>;

/// Checks that `value` is finite and strictly positive.
pub(crate) fn positive(name: &str, value: f64) -> Result<f64> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(MissionError::InvalidInput(format!(
            "{name} must be positive, got {value}"
        )))
    }
}
