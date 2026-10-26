//! 2D airfoil flow solvers for the `aero` toolkit.
//!
//! This crate is independent of any user interface: it can be used from a
//! CLI, a desktop application, Python bindings or WebAssembly.
//!
//! - [`inviscid`]: XFOIL's inviscid panel method;
//! - [`viscous`]: XFOIL's viscous–inviscid interaction (integral boundary
//!   layer, eᴺ transition, wake, drag), at prescribed angle or lift;
//! - [`inverse`]: XFOIL's inverse design methods (shape from a prescribed
//!   surface speed).
//!
//! The dense linear algebra uses [`faer`], re-exported for the users of the
//! influence matrices.

pub mod inverse;
pub mod inviscid;
pub mod viscous;

pub use faer;
