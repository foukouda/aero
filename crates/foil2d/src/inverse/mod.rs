//! Inverse design: the shape of an airfoil from a prescribed surface speed
//! distribution, ported from XFOIL 6.996.
//!
//! - [`mixed`]: mixed inverse (`QDES`): a segment of the surface is
//!   reshaped to reach a target speed there, the rest is kept;
//!
//! | Function | XFOIL | Routines |
//! |---|---|---|
//! | [`SpeedTarget::from_airfoil`] | `QDES` start | `SPECAL`, `GAMQSP` |
//! | [`SpeedTarget::smooth`] | `QDES` `SMOO` | `SMOOQ`, `TRISOL` |
//! | [`SpeedTarget::symmetrize`] | `QDES` `SYMM` | `SYMQSP` |
//! | [`mixed_inverse`] | `QDES` `EXEC` | `MIXED`, `GAMLIN`, `PSILIN` (`GEOLIN`), `NCALC`, `TECALC`, `GAUSS` |

mod error;
pub mod mixed;

pub use error::{InverseError, Result};
pub use mixed::{MixedInverseOptions, MixedInverseResult, SpeedTarget, mixed_inverse};
