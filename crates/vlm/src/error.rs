//! Error type of the vortex lattice.

use thiserror::Error;

/// Errors produced when building or solving a vortex lattice.
#[derive(Debug, Clone, PartialEq, Error)]
#[non_exhaustive]
pub enum VlmError {
    /// The configuration is not valid (a count, a dimension, a spacing…).
    #[error("invalid configuration: {0}")]
    Invalid(String),

    /// An airfoil could not be turned into a camber line.
    #[error("airfoil {name}: {reason}")]
    Airfoil {
        /// Name or file of the airfoil.
        name: String,
        /// What went wrong.
        reason: String,
    },

    /// An AVL geometry file could not be read.
    #[error("AVL file, line {line}: {reason}")]
    AvlFile {
        /// Line number (1-based) in the file.
        line: usize,
        /// What went wrong.
        reason: String,
    },

    /// The influence matrix is singular (coincident or degenerate panels).
    #[error("the vortex lattice is singular (coincident or degenerate panels)")]
    Singular,
}

/// Result type of the vortex lattice.
pub type Result<T> = std::result::Result<T, VlmError>;

/// An [`VlmError::Invalid`] built from a message.
pub(crate) fn invalid(message: impl Into<String>) -> VlmError {
    VlmError::Invalid(message.into())
}
