//! Viscous analysis: XFOIL's viscous–inviscid interaction (`xbl.f`,
//! `xblsys.f`, `xsolve.f`, wake and coupling routines of `xpanel.f`,
//! `VISCAL`).
//!
//! The formulation is that of XFOIL 6.996:
//! - two-equation integral boundary layer (momentum and kinetic-energy shape
//!   parameter equations) with Drela's laminar and turbulent closures, a
//!   lag equation for the turbulent shear stress and the eᴺ envelope method
//!   for transition (free, or forced by trips);
//! - a wake trailing the airfoil along the inviscid streamline;
//! - the displacement effect modelled by surface and wake sources, coupled
//!   to the panel method through a source influence matrix;
//! - one global Newton system for all boundary-layer variables and edge
//!   velocities, solved by XFOIL's block-elimination solver.

mod blsys;
mod closure;
mod error;
mod march;
mod newton;
mod panels;
mod session;
mod solver;
mod viscal;

pub use error::{Result, ViscousError};
pub use solver::{
    AmplificationModel, BoundaryLayer, MachReynoldsVariation, Transition, ViscousOptions,
    ViscousPolar, ViscousPolarPoint, ViscousSession, ViscousSolution, ViscousSolver,
    ViscousSweepProgress,
};

#[cfg(test)]
mod tests;
