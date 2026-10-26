//! Inviscid analysis: XFOIL's linear-vorticity panel method (`xpanel.f`).
//!
//! The formulation is that of XFOIL 6.996:
//! - a vortex sheet of piecewise-linear strength `γ` on the airfoil panels,
//!   with the streamfunction constant (`ψ = ψ₀`) at every node (`PSILIN`,
//!   `GGCALC`);
//! - a trailing-edge panel carrying a source and a vortex tied to the
//!   trailing-edge `γ` values, for blunt trailing edges;
//! - the Kutta condition `γ₁ + γ_N = 0`; for a sharp trailing edge the last
//!   node equation is replaced by a zero internal velocity along the
//!   trailing-edge bisector;
//! - two base solutions (`α = 0°`, `α = 90°`) computed once per geometry
//!   from a single LU factorization, superimposed for any `α`;
//! - pressure integration and Kármán–Tsien correction (`CLCALC`, `CPCALC`).
//!
//! ```
//! use aero_foil2d::inviscid::{InviscidOptions, InviscidSolver, OperatingPoint};
//! use aero_geometry::{NacaOptions, PanelingOptions, naca};
//!
//! let airfoil = naca("2412", &NacaOptions::default())?.paneled(&PanelingOptions::default())?;
//! let solver = InviscidSolver::new(&airfoil, &InviscidOptions::default())?;
//! let solution = solver.solve(OperatingPoint::incompressible(4.0))?;
//! assert!(solution.cl > 0.7 && solution.cl < 0.8);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod error;
mod panel;
mod psilin;
mod solver;

pub use error::{InviscidError, Result};
pub use panel::{PanelGeometry, TrailingEdgePanel};
pub(crate) use psilin::psilin;
pub use psilin::{FieldPoint, Influence};
pub use solver::{
    AlphaSweep, InviscidOptions, InviscidSolution, InviscidSolver, OperatingPoint, Polar,
    PolarPoint, SweepFailure, SweepProgress, sonic_cp,
};
pub(crate) use solver::{clcalc, te_bisector};
