//! From a flight to the conditions of its airfoil, for the `aero` toolkit.
//!
//! A beginner knows the flight they have in mind — a speed, an altitude, a
//! mass, a wing size — not the Reynolds number or the lift coefficient an
//! airfoil analysis needs. This crate makes the link:
//!
//! - [`atmosphere`]: the International Standard Atmosphere (U.S. Standard
//!   Atmosphere 1976, up to 20 km), with an optional temperature offset;
//! - [`flight`]: a level flight and the conditions its wing meets —
//!   dynamic pressure, Reynolds and Mach numbers, wing loading, required
//!   lift coefficient — and the wing size for a chosen lift coefficient;
//! - [`wing`]: the wing's drag, power, glide ratio and stall speed
//!   estimated from its airfoil polar with the classical finite-wing
//!   formulas;
//! - [`recommend`]: each candidate airfoil analysed at the flight's
//!   conditions (viscous polar, maximum lift, drag at the required lift
//!   coefficient, stall margin, ease of building) and the candidates ranked
//!   by the user's goal;
//! - [`analysis`]: how a whole aircraft flies — trim, stability, drag of
//!   every part, stall — from its vortex lattice and the viscous polars of
//!   its airfoils.
//!
//! All quantities are in SI units (m, m/s, kg, N, Pa, K).
//!
//! # Example
//!
//! ```
//! use aero_mission::{Flight, SectionData, Wing, WingModel, performance};
//!
//! // A 300 g model at 20 km/h, 100 m high, with a 1.2 m × 20 cm wing.
//! let flight = Flight { speed: 20.0 / 3.6, altitude: 100.0, temperature_offset: 0.0, mass: 0.3 };
//! let conditions = flight.conditions(&Wing { span: 1.2, chord: 0.2 })?;
//! assert!((conditions.reynolds - 75_500.0).abs() < 100.0);
//! assert!((conditions.required_cl - 0.65).abs() < 0.01);
//!
//! // Airfoil polar at that Reynolds number: Cd 0.018 at Cl 0.65, Cl max 1.3.
//! let wing = performance(&conditions, &SectionData { cd: 0.018, cl_max: 1.3 }, &WingModel::default())?;
//! println!("glide ratio {:.1}, stall at {:.1} km/h", wing.lift_to_drag, wing.stall_speed * 3.6);
//! # Ok::<(), aero_mission::MissionError>(())
//! ```

pub mod analysis;
pub mod atmosphere;
mod error;
pub mod flight;
pub mod recommend;
pub mod wing;

pub use analysis::{
    AircraftAnalysis, AnalysisWarning, DragBreakdown, PolarCache, PolarRequest, PolarSource,
    PreparedAnalysis, SectionPolar, StabilityResult, StallResult, StripResult, TrimResult, analyze,
    section_polar,
};
pub use atmosphere::{Atmosphere, isa};
pub use error::{MissionError, Result};
pub use flight::{Conditions, Flight};
pub use recommend::{
    AirfoilEvaluation, AnalysisSettings, Buildability, Fit, Goal, SectionPoint, evaluate, rank,
};
pub use wing::{Performance, SectionData, Wing, WingConstraint, WingModel, performance};
