//! Error type of the inviscid solver.

use aero_geometry::GeometryError;
use thiserror::Error;

/// Errors produced by the inviscid solver.
#[derive(Debug, Clone, PartialEq, Error)]
#[non_exhaustive]
pub enum InviscidError {
    /// The airfoil geometry is invalid.
    #[error(transparent)]
    Geometry(#[from] GeometryError),

    /// The influence matrix is singular or ill-conditioned.
    #[error("the panel influence matrix is singular; the geometry is probably degenerate")]
    SingularMatrix,

    /// The freestream Mach number is outside `[0, 1)`.
    #[error("invalid Mach number {0}: it must be between 0 and 1 (subsonic)")]
    InvalidMach(f64),

    /// The angle of attack is not a finite number.
    #[error("invalid angle of attack {0}")]
    InvalidAlpha(f64),

    /// The flow becomes locally supersonic: the minimum pressure coefficient
    /// is below the sonic value.
    #[error(
        "critical Mach number exceeded at M = {mach}: Cp min = {cp_min:.3} at x = {x:.3} \
         is below the sonic Cp* = {cp_star:.3}"
    )]
    CriticalMachExceeded {
        /// Freestream Mach number.
        mach: f64,
        /// Minimum pressure coefficient.
        cp_min: f64,
        /// Its x location.
        x: f64,
        /// Sonic pressure coefficient.
        cp_star: f64,
    },

    /// The Kármán–Tsien correction is no longer defined (local speed too
    /// large, XFOIL `CPCALC` "compressibility corrections invalid").
    #[error(
        "the Kármán–Tsien compressibility correction breaks down at M = {mach}: local speed too large"
    )]
    CompressibilityBreakdown {
        /// Freestream Mach number.
        mach: f64,
    },

    /// The angle-of-attack sweep definition is invalid.
    #[error("invalid sweep: {0}")]
    InvalidSweep(String),
}

/// Result type of the inviscid solver.
pub type Result<T> = std::result::Result<T, InviscidError>;
