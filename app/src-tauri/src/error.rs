//! Errors returned to the user interface.

use aero_foil2d::inverse::InverseError;
use aero_foil2d::inviscid::InviscidError;
use aero_foil2d::viscous::ViscousError;
use aero_geometry::GeometryError;
use aero_optim::OptimError;
use serde::Serialize;
use specta::Type;

/// Category of an error, for the user interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// An airfoil file could not be read or parsed.
    InvalidFile,
    /// The geometry is invalid or degenerate.
    Geometry,
    /// The analysis failed (singular system, ...).
    Solver,
    /// The Mach number is too high (critical Mach number exceeded, ...).
    Mach,
    /// Invalid user input.
    InvalidInput,
    /// Unknown airfoil.
    NotFound,
    /// Project file problem.
    Project,
    /// File system error.
    Io,
    /// Unexpected internal error.
    Internal,
}

/// An error with a message meant for the user.
#[derive(Debug, Clone, PartialEq, Serialize, Type, thiserror::Error)]
#[error("{message}")]
pub struct AppError {
    /// Category.
    pub kind: ErrorKind,
    /// Human readable message.
    pub message: String,
}

impl AppError {
    /// Creates an error.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl From<GeometryError> for AppError {
    fn from(e: GeometryError) -> Self {
        let kind = match e {
            GeometryError::Parse { .. } => ErrorKind::InvalidFile,
            GeometryError::InvalidNaca(_) | GeometryError::InvalidOption(_) => {
                ErrorKind::InvalidInput
            }
            _ => ErrorKind::Geometry,
        };
        Self::new(kind, e.to_string())
    }
}

impl From<InviscidError> for AppError {
    fn from(e: InviscidError) -> Self {
        match e {
            InviscidError::Geometry(g) => g.into(),
            InviscidError::CriticalMachExceeded { .. }
            | InviscidError::CompressibilityBreakdown { .. }
            | InviscidError::InvalidMach(_) => Self::new(ErrorKind::Mach, e.to_string()),
            InviscidError::InvalidAlpha(_) | InviscidError::InvalidSweep(_) => {
                Self::new(ErrorKind::InvalidInput, e.to_string())
            }
            _ => Self::new(ErrorKind::Solver, e.to_string()),
        }
    }
}

impl From<ViscousError> for AppError {
    fn from(e: ViscousError) -> Self {
        match e {
            ViscousError::Inviscid(e) => e.into(),
            ViscousError::InvalidOption(_) => Self::new(ErrorKind::InvalidInput, e.to_string()),
            _ => Self::new(ErrorKind::Solver, e.to_string()),
        }
    }
}

impl From<InverseError> for AppError {
    fn from(e: InverseError) -> Self {
        match e {
            InverseError::Inviscid(e) => e.into(),
            InverseError::Geometry(g) => g.into(),
            InverseError::InvalidSegment { .. }
            | InverseError::StagnationInSegment
            | InverseError::NoStagnationPoint
            | InverseError::InvalidOption(_) => Self::new(ErrorKind::InvalidInput, e.to_string()),
            _ => Self::new(ErrorKind::Solver, e.to_string()),
        }
    }
}

impl From<OptimError> for AppError {
    fn from(e: OptimError) -> Self {
        match e {
            OptimError::Geometry(g) => g.into(),
            _ => Self::new(ErrorKind::InvalidInput, e.to_string()),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::new(ErrorKind::Io, e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions_pick_a_kind() {
        let e: AppError = GeometryError::Parse {
            line: 3,
            message: "bad".into(),
        }
        .into();
        assert_eq!(e.kind, ErrorKind::InvalidFile);
        assert_eq!(e.message, "line 3: bad");
        let e: AppError = InviscidError::InvalidMach(1.2).into();
        assert_eq!(e.kind, ErrorKind::Mach);
        let e: AppError = InviscidError::SingularMatrix.into();
        assert_eq!(e.kind, ErrorKind::Solver);
        let e: AppError = InviscidError::Geometry(GeometryError::InvalidNaca("x".into())).into();
        assert_eq!(e.kind, ErrorKind::InvalidInput);
        let e: AppError = std::io::Error::other("disk").into();
        assert_eq!(e.kind, ErrorKind::Io);
        assert_eq!(e.to_string(), "disk");
    }
}
