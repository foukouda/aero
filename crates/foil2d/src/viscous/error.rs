//! Error type of the viscous solver.

use thiserror::Error;

use crate::inviscid::InviscidError;

/// Errors produced by the viscous solver.
///
/// A solution that does not converge is not an error: it is returned with
/// [`super::ViscousSolution::converged`] unset, as XFOIL reports it.
#[derive(Debug, Clone, PartialEq, Error)]
#[non_exhaustive]
pub enum ViscousError {
    /// Error of the underlying inviscid solution (geometry, Mach number,
    /// critical Mach number exceeded, Kármán–Tsien breakdown, sweep).
    #[error(transparent)]
    Inviscid(#[from] InviscidError),

    /// An option is out of range.
    #[error("invalid viscous option: {0}")]
    InvalidOption(String),
}

/// Result type of the viscous solver.
pub type Result<T> = std::result::Result<T, ViscousError>;
