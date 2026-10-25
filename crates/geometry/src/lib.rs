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

mod airfoil;
mod error;
pub mod io;
pub mod spline;

pub use airfoil::{Airfoil, MIN_POINTS};
pub use error::{GeometryError, Result};
pub use io::{DatFormat, ImportReport, ImportWarning, parse_dat, to_selig_string};
pub use spline::{EndCondition, Spline, SplineCurve};
