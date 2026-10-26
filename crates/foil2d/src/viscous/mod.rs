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

// Not reachable from the public API until the solver is in place.
#[allow(dead_code)]
mod blsys;
#[allow(dead_code)]
mod closure;
#[allow(dead_code)]
mod march;
#[allow(dead_code)]
mod newton;
#[allow(dead_code)]
mod panels;
#[allow(dead_code)]
mod session;
#[allow(dead_code)]
mod viscal;

#[cfg(test)]
mod tests;
