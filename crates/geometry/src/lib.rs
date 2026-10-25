//! Airfoil geometry for the `aero` toolkit.
//!
//! This crate is independent of any user interface: it can be used from a
//! CLI, a desktop application, Python bindings or WebAssembly.
//!
//! Several algorithms are faithful ports of XFOIL 6.996 routines (Mark
//! Drela, GPL); each ported function names its XFOIL counterpart:
//!
//! | Module | XFOIL |
//! |---|---|
//! | [`spline`] | `spline.f`: `SPLIND`, `SEGSPL`, `SEVAL`, `DEVAL`, `D2VAL`, `CURV`, `SINVRT`, `SCALC`, `TRISOL` |
//! | [`naca`](mod@naca) | `naca.f`: `NACA4`, `NACA5` |
//! | [`leading_edge`] | `xgeom.f`: `LEFIND`, `NSFIND` |
//! | [`transform`] | `xgeom.f`: `NORM` |
//! | [`paneling`] | `xfoil.f`: `PANGEN` |
//! | [`properties`] | `xgeom.f`: `GEOPAR`, `TCCALC`, `SOPPS`; `xutils.f`: `ATANC` |
//!
//! # Example
//!
//! ```
//! use aero_geometry::{NacaOptions, PanelingOptions, naca};
//!
//! let airfoil = naca("2412", &NacaOptions::default())?;
//! let paneled = airfoil.paneled(&PanelingOptions::default())?;
//! assert_eq!(paneled.len(), 160);
//! let props = airfoil.properties();
//! assert!((props.max_camber - 0.02).abs() < 1e-3);
//! # Ok::<(), aero_geometry::GeometryError>(())
//! ```

mod airfoil;
pub mod cst;
mod error;
pub mod io;
pub mod leading_edge;
pub mod naca;
pub mod paneling;
pub mod properties;
pub mod spline;
pub mod transform;

pub use airfoil::{Airfoil, MIN_POINTS};
pub use cst::{CstFit, CstFitOptions, CstParameters, fit_cst};
pub use error::{GeometryError, Result};
pub use io::{
    DatFormat, ImportOptions, ImportReport, ImportWarning, parse_dat, parse_dat_with,
    to_selig_string,
};
pub use leading_edge::{LeadingEdge, LeadingEdgeMethod};
pub use naca::{NacaOptions, TrailingEdge, naca};
pub use paneling::{Paneling, PanelingOptions, panel, panel_with_report};
pub use properties::GeometricProperties;
pub use spline::{EndCondition, Spline, SplineCurve};
pub use transform::{Normalization, NormalizationTransform};
