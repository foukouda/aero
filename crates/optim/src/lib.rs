//! Airfoil shape optimization for the `aero` toolkit.
//!
//! This crate is independent of any user interface: it can be used from a
//! CLI, a desktop application, Python bindings or WebAssembly.
//!
//! - [`design`]: the design space, CST weights of both surfaces fitted to
//!   the initial airfoil;
//! - [`problem`]: operating points, objective and constraints;
//! - [`evaluate`]: geometry and viscous analysis of a design (the XFOIL
//!   port of `aero-foil2d`), ranked by feasibility then objective.

pub mod design;
mod error;
pub mod evaluate;
pub mod problem;
pub mod rank;

pub use design::{DesignOptions, DesignSpace};
pub use error::{OptimError, Result};
pub use evaluate::{Evaluation, Evaluator, GeometrySummary, PointResult, PointValues};
pub use problem::{Constraints, DesignPoint, Objective, Problem, Target, ThicknessStation};
pub use rank::Ranked;
