//! Derivative-free minimizers working on [`Ranked`] values: they only
//! compare evaluations, so constraints can be handled by feasibility rules.
//!
//! - [`cmaes`]: covariance matrix adaptation evolution strategy, global;
//!   the population of each generation is evaluated in parallel;
//! - [`nelder_mead`]: simplex method with adaptive parameters, local.
//!
//! Both are deterministic: CMA-ES draws its samples from a seeded
//! generator, and parallel evaluations are collected in sampling order, so
//! a run does not depend on the number of threads.

pub mod cmaes;
pub mod nelder_mead;
pub(crate) mod parallel;
pub(crate) mod random;

use serde::{Deserialize, Serialize};

use crate::rank::Ranked;

/// Why a minimization stopped.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stop {
    /// The evaluation budget is spent.
    MaxEvaluations,
    /// The step size fell below the tolerance.
    Converged,
    /// The best value did not improve for many iterations.
    Stagnation,
    /// Numerical limit reached (ill-conditioned covariance matrix).
    NumericalLimit,
    /// The observer asked to stop.
    Cancelled,
}

/// Report given to the observer after each iteration (CMA-ES generation or
/// Nelder–Mead step).
#[derive(Debug)]
pub struct Step<'a, F> {
    /// Iteration number, from 1.
    pub iteration: usize,
    /// Evaluations so far in this minimization.
    pub evaluations: usize,
    /// Current step size: CMA-ES `σ` times the largest standard deviation
    /// of the sampling distribution, or the simplex size.
    pub step_size: f64,
    /// Points evaluated by this iteration, in evaluation order.
    pub evaluated: &'a [(Vec<f64>, F)],
    /// Best point so far.
    pub best: &'a (Vec<f64>, F),
}

/// Result of a minimization.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome<F> {
    /// Best point found and its value.
    pub best: (Vec<f64>, F),
    /// Evaluations performed.
    pub evaluations: usize,
    /// Iterations performed.
    pub iterations: usize,
    /// Why the minimization stopped.
    pub stop: Stop,
}

/// Replaces `best` by the first strictly better evaluated point.
fn update_best<F: Ranked + Clone>(best: &mut Option<(Vec<f64>, F)>, evaluated: &[(Vec<f64>, F)]) {
    for (x, f) in evaluated {
        if best.as_ref().is_none_or(|(_, b)| f.rank_cmp(b).is_lt()) {
            *best = Some((x.clone(), f.clone()));
        }
    }
}
