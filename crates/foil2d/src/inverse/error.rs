//! Error type of the inverse design methods.

use aero_geometry::GeometryError;
use thiserror::Error;

use crate::inviscid::InviscidError;

/// Errors produced by the inverse design methods.
#[derive(Debug, Clone, PartialEq, Error)]
#[non_exhaustive]
pub enum InverseError {
    /// The target segment is empty or outside the airfoil.
    #[error("invalid target segment {first}..{last} for {nodes} nodes")]
    InvalidSegment {
        /// First node.
        first: usize,
        /// Last node.
        last: usize,
        /// Number of nodes.
        nodes: usize,
    },

    /// The target segment of a mixed inverse contains the stagnation point.
    #[error("the target segment cannot include the stagnation point")]
    StagnationInSegment,

    /// An option or the target is invalid.
    #[error("invalid inverse design input: {0}")]
    InvalidOption(String),

    /// The Newton system is singular.
    #[error("the inverse design system is singular")]
    Singular,

    /// The analysis of the airfoil failed.
    #[error(transparent)]
    Inviscid(#[from] InviscidError),

    /// The new geometry is invalid.
    #[error(transparent)]
    Geometry(#[from] GeometryError),
}

/// Result type of the inverse design methods.
pub type Result<T> = std::result::Result<T, InverseError>;
