//! A simplified aircraft for the `aero` toolkit: the parts a beginner
//! designs, and what can be read from them before any analysis.
//!
//! - [`params`]: the aircraft as a few dimensions per part — a
//!   straight-tapered wing, horizontal tail and fin, a fuselage, the centre
//!   of gravity — and example aircraft;
//! - [`planform`]: the sections of each surface (as AVL places them), their
//!   areas, mean aerodynamic chords and aspect ratios, the tail volume
//!   coefficients and the position of the centre of gravity;
//! - [`mesh`](mod@mesh): a triangle mesh of the aircraft, lofted from the real airfoil
//!   coordinates, for a 3D view.
//!
//! Axes as in AVL: x from the nose towards the tail, y towards the right
//! wing tip, z up; metres and degrees.
//!
//! # Example
//!
//! ```
//! use aero_aircraft::{Preset, layout};
//!
//! let trainer = Preset::Trainer.aircraft();
//! let l = layout(&trainer)?;
//! assert!((l.wing.area - 1.2 * 0.20).abs() < 1e-9);
//! println!("tail volume {:.2}, CG {:.0} mm behind the nose", l.tail_volume.unwrap_or(0.0), l.cg_x * 1e3);
//! # Ok::<(), aero_aircraft::AircraftError>(())
//! ```

mod error;
pub mod mesh;
pub mod params;
pub mod planform;

pub use error::{AircraftError, Result};
pub use mesh::{MeshOptions, MeshPart, PartKind, mesh};
pub use params::{
    Aircraft, Fuselage, MAX_PANEL_ANGLE, MAX_SECTIONS, Preset, SectionParams, SurfaceParams,
    TaperedParams,
};
pub use planform::{Layout, Planform, Section, Surface, SurfaceKind, layout, surfaces};
