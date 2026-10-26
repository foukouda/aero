//! Error type of the optimizer.

use aero_geometry::GeometryError;
use thiserror::Error;

/// Errors produced when setting up an optimization.
///
/// A design whose analysis fails is not an error: it is ranked as
/// infeasible (see [`crate::Evaluation`]).
#[derive(Debug, Clone, PartialEq, Error)]
#[non_exhaustive]
pub enum OptimError {
    /// The problem definition or an option is invalid.
    #[error("invalid optimization problem: {0}")]
    InvalidProblem(String),

    /// The initial airfoil cannot be fitted.
    #[error(transparent)]
    Geometry(#[from] GeometryError),
}

/// Result type of the optimizer.
pub type Result<T> = std::result::Result<T, OptimError>;
