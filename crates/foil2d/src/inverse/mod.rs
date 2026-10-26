//! Inverse design: the shape of an airfoil from a prescribed surface speed
//! distribution, ported from XFOIL 6.996.
//!
//! - [`mixed`]: mixed inverse (`QDES`): a segment of the surface is
//!   reshaped to reach a target speed there, the rest is kept;
//! - [`full`]: full inverse (`MDES`): the whole airfoil is generated from
//!   target speed distributions, by conformal mapping of a circle.
//!
//! | Function | XFOIL | Routines |
//! |---|---|---|
//! | [`SpeedTarget::from_airfoil`] | `QDES` start | `SPECAL`, `GAMQSP` |
//! | [`SpeedTarget::smooth`] | `QDES` `SMOO` | `SMOOQ`, `TRISOL` |
//! | [`SpeedTarget::symmetrize`] | `QDES` `SYMM` | `SYMQSP` |
//! | [`mixed_inverse`] | `QDES` `EXEC` | `MIXED`, `GAMLIN`, `PSILIN` (`GEOLIN`), `NCALC`, `TECALC`, `GAUSS` |
//! | [`FullInverse::new`] | `MDES` start | `EIWSET`, `SCINIT`, `FTP`, `PIQSUM`, `ZCCALC`, `ZLEFIND`, `ZCNORM`, `MAPGAM`, `QCCALC`, `CNCALC`, `QSPCIR` |
//! | [`FullInverse::set_alphas`], [`FullInverse::set_lift_coefficients`] | `MDES` `AQ`, `CQ` | `QSPCIR`, `QCCALC` |
//! | [`FullInverse::filter`] | `MDES` `FILT` | `CNFILT` |
//! | [`FullInverse::smooth`], [`FullInverse::set_target`] | `MDES` `SMOO`, `MODI` | `SMOOQ`, `CNCALC` |
//! | [`FullInverse::set_symmetric`] | `MDES` `SYMM` | `CNSYMM` |
//! | [`FullInverse::set_te_gap`], [`FullInverse::set_te_angle`] | `MDES` `TGAP`, `TANG` | `DZTSET`, `AGTSET` |
//! | [`FullInverse::execute`] | `MDES` `EXEC` | `MAPGEN`, `CGAUSS` |

mod complex;
mod error;
pub mod full;
pub mod mixed;

pub use error::{InverseError, Result};
pub use full::{DesignBy, DesignPoint, FullInverse, FullInverseResult};
pub use mixed::{MixedInverseOptions, MixedInverseResult, SpeedTarget, mixed_inverse};
