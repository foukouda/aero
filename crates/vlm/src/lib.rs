//! The vortex lattice method of AVL 3.52 (Mark Drela and Harold Youngren,
//! MIT, GPL) in Rust: lifting surfaces of an aircraft as horseshoe vortices
//! on their camber surfaces, the forces and moments, the induced drag in
//! the Trefftz plane, and the stability and control derivatives.
//!
//! - [`model`]: the configuration, as an AVL geometry file describes it
//!   (surfaces made of sections, airfoil camber lines, control surfaces);
//! - [`VortexLattice`]: the lattice built and factored once, then solved at
//!   any [`FlowCondition`] in a time proportional to its size;
//! - [`Solution`]: AVL's total forces (`FT`), surface and strip forces
//!   (`FS`) and stability derivatives (`ST`), with AVL's signs;
//! - [`parse_avl`]: reading AVL geometry files.
//!
//! The port follows AVL's formulation step by step — spacing, camber
//! slopes, normals, Prandtl–Glauert compressibility, vortex cores between
//! components, `NOWAKE` surfaces, linearized control deflections — and is
//! checked against AVL 3.52 itself (`tests/avl`). Bodies, ground effect,
//! design variables and `CDCL` viscous polars are left out: the profile
//! drag comes from the 2D viscous solver of `aero-foil2d` instead.
//!
//! Axes as in AVL: x downstream, y towards the right tip, z up.
//!
//! # Example
//!
//! ```
//! use aero_vlm::{Camber, FlowCondition, LiftingSurface, Reference, Spacing, SurfaceSection,
//!     VlmModel, VortexCore, VortexLattice};
//!
//! // A rectangular wing of span 8 and chord 1, cambered (NACA 2412).
//! let mut root = SurfaceSection::new([0.0, 0.0, 0.0], 1.0);
//! let mut tip = SurfaceSection::new([0.0, 4.0, 0.0], 1.0);
//! root.camber = Camber::naca4(2412, (0.0, 1.0))?;
//! tip.camber = root.camber.clone();
//! let mut wing = LiftingSurface::new("Wing", Spacing::new(8, 1.0), vec![root, tip]);
//! wing.spanwise = Some(Spacing::new(16, -2.0));
//! let model = VlmModel {
//!     title: "Rectangular wing".into(),
//!     mach: 0.0,
//!     y_symmetry: true,
//!     reference: Reference { area: 8.0, chord: 1.0, span: 8.0, point: [0.25, 0.0, 0.0] },
//!     cd_p: 0.0,
//!     core: VortexCore::default(),
//!     surfaces: vec![wing],
//! };
//! let lattice = VortexLattice::new(&model)?;
//! let s = lattice.solve(&FlowCondition::alpha(4.0))?;
//! assert!(s.totals.cl > 0.4 && s.totals.cl < 0.7);
//! println!("CL {:.3}  CDi {:.4}  e {:.3}", s.totals.cl, s.totals.cd_trefftz, s.totals.span_efficiency);
//! # Ok::<(), aero_vlm::VlmError>(())
//! ```

mod avl_file;
mod camber;
mod error;
mod forces;
mod influence;
mod lattice;
pub mod model;
mod solution;
mod solver;
mod spacing;
mod vec3;

pub use avl_file::parse_avl;
pub use camber::Camber;
pub use error::{Result, VlmError};
pub use model::{
    ControlSurface, LiftingSurface, MAX_CHORDWISE, Reference, Spacing, SurfaceSection, VlmModel,
    VortexCore,
};
pub use solution::{
    CoefficientDerivatives, ControlDerivatives, FlowCondition, Solution, StabilityDerivatives,
    StripForces, SurfaceForces, Totals,
};
pub use solver::VortexLattice;
