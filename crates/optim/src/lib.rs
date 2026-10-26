//! Airfoil shape optimization for the `aero` toolkit.
//!
//! This crate is independent of any user interface: it can be used from a
//! CLI, a desktop application, Python bindings or WebAssembly.
//!
//! - [`design`]: the design space, CST weights of both surfaces fitted to
//!   the initial airfoil;
//! - [`problem`]: operating points, objective and constraints;
//! - [`evaluate`]: geometry and viscous analysis of a design (the XFOIL
//!   port of `aero-foil2d`), ranked by feasibility then objective;
//! - [`optimizers`]: CMA-ES and Nelder–Mead;
//! - [`optimize`](mod@optimize): the driver, from the initial airfoil to
//!   the optimized one.
//!
//! # Example
//!
//! ```
//! use std::ops::ControlFlow;
//! use aero_geometry::{NacaOptions, naca};
//! use aero_optim::{Constraints, DesignPoint, OptimizeOptions, Problem, Target, optimize};
//!
//! // Minimum drag at CL = 0.5 and Re = 1e6 (the default viscous options),
//! // keeping the thickness at least 12 %.
//! let problem = Problem {
//!     points: vec![DesignPoint { target: Target::Cl(0.5), weight: 1.0 }],
//!     objective: Default::default(), // weighted mean Cd
//!     constraints: Constraints { min_thickness: Some(0.12), ..Default::default() },
//!     viscous: Default::default(),
//!     paneling: Default::default(),
//!     design: Default::default(), // CST of order 6: 14 variables
//! };
//! let options = OptimizeOptions { max_evaluations: 40, ..Default::default() };
//! let result = optimize(&naca("2412", &NacaOptions::default())?, &problem, &options, |progress| {
//!     println!("{} evaluations, best Cd {:?}", progress.evaluations, progress.best.objective);
//!     ControlFlow::Continue(()) // Break cancels and keeps the best design
//! })?;
//! assert!(result.best.feasible() && result.best.objective <= result.initial.objective);
//! let optimized = result.airfoil; // regular Airfoil: analyze it, export it as .dat
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod design;
mod error;
pub mod evaluate;
pub mod optimize;
pub mod optimizers;
pub mod problem;
pub mod rank;

pub use design::{DesignOptions, DesignSpace};
pub use error::{OptimError, Result};
pub use evaluate::{Evaluation, Evaluator, GeometrySummary, PointResult, PointValues};
pub use optimize::{
    HistoryEntry, Method, OptimizationResult, OptimizeOptions, Phase, Progress, optimize,
};
pub use optimizers::Stop;
pub use problem::{Constraints, DesignPoint, Objective, Problem, Target, ThicknessStation};
pub use rank::Ranked;
