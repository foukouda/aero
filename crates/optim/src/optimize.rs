//! The optimization driver: fit of the initial airfoil, global search by
//! CMA-ES, local polish by Nelder–Mead.

use std::ops::ControlFlow;

use aero_geometry::{Airfoil, CstParameters};
use serde::{Deserialize, Serialize};

use crate::error::{OptimError, Result};
use crate::evaluate::{Evaluation, Evaluator};
use crate::optimizers::cmaes::{self, CmaEsOptions};
use crate::optimizers::nelder_mead::{self, NelderMeadOptions};
use crate::optimizers::{Step, Stop};
use crate::problem::Problem;
use crate::rank::Ranked;

/// Search method.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    /// CMA-ES, then Nelder–Mead from the best design.
    #[default]
    CmaEsThenNelderMead,
    /// CMA-ES only.
    CmaEs,
    /// Nelder–Mead only, from the initial design.
    NelderMead,
}

/// Options of [`optimize`].
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OptimizeOptions {
    /// Search method.
    pub method: Method,
    /// Evaluation budget, the initial design included.
    pub max_evaluations: usize,
    /// Share of the budget kept for the Nelder–Mead polish with
    /// [`Method::CmaEsThenNelderMead`]; the polish also gets what CMA-ES
    /// leaves when it converges early.
    pub polish_fraction: f64,
    /// Initial step on the CST weights: CMA-ES `σ₀`, or the Nelder–Mead
    /// simplex edge with [`Method::NelderMead`].
    pub step: f64,
    /// CMA-ES population; 0 for the default `4 + ⌊3 ln n⌋`.
    pub population: usize,
    /// Seed of the CMA-ES sampling.
    pub seed: u64,
    /// Threads evaluating designs; 0 for all cores. The results do not
    /// depend on it.
    pub threads: usize,
}

impl Default for OptimizeOptions {
    fn default() -> Self {
        Self {
            method: Method::CmaEsThenNelderMead,
            max_evaluations: 1500,
            polish_fraction: 0.2,
            step: 0.01,
            population: 0,
            seed: 1,
            threads: 0,
        }
    }
}

/// Stops CMA-ES when its steps on the CST weights fall below this value.
const CMAES_TOL_X: f64 = 1e-5;
/// Stops Nelder–Mead when its simplex is smaller than this.
const NELDER_MEAD_TOL_X: f64 = 1e-6;

impl OptimizeOptions {
    /// Checks the options.
    pub fn validate(&self) -> Result<()> {
        let invalid = |m: &str| Err(OptimError::InvalidProblem(m.into()));
        if self.max_evaluations < 2 {
            return invalid("the budget must allow at least 2 evaluations");
        }
        if !(0.0..=1.0).contains(&self.polish_fraction) {
            return invalid("the polish fraction must be between 0 and 1");
        }
        if !(self.step.is_finite() && self.step > 0.0) {
            return invalid("the initial step must be positive");
        }
        if self.population == 1 {
            return invalid("the CMA-ES population needs at least 2 members");
        }
        Ok(())
    }
}

/// Phase of an optimization.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Evaluation of the fitted initial airfoil.
    Initial,
    /// CMA-ES.
    Global,
    /// Nelder–Mead.
    Local,
}

/// One evaluation of the history.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Evaluation number, from 1 (the initial design).
    pub evaluation: usize,
    /// Phase that made it.
    pub phase: Phase,
    /// Objective, when every operating point converged.
    pub objective: Option<f64>,
    /// Constraint violation, 0 when feasible.
    pub violation: f64,
}

/// Progress report, after the initial evaluation and after each iteration
/// of the optimizers.
#[derive(Debug)]
pub struct Progress<'a> {
    /// Current phase.
    pub phase: Phase,
    /// Evaluations so far.
    pub evaluations: usize,
    /// Evaluation budget.
    pub max_evaluations: usize,
    /// Step size of the optimizer (CMA-ES spread or simplex size).
    pub step_size: f64,
    /// Evaluations since the previous report.
    pub new: &'a [HistoryEntry],
    /// Best design so far.
    pub best: &'a Evaluation,
    /// True if `best` changed since the previous report.
    pub improved: bool,
}

/// Result of an optimization.
#[derive(Debug, Clone, PartialEq)]
pub struct OptimizationResult {
    /// The fitted initial design.
    pub initial: Evaluation,
    /// The best design (the initial one if nothing better was found).
    pub best: Evaluation,
    /// CST parameters of the best design.
    pub parameters: CstParameters,
    /// Coordinates of the best design (unit chord, not paneled).
    pub airfoil: Airfoil,
    /// Rms ordinate error of the CST fit of the initial airfoil.
    pub fit_rms_error: f64,
    /// Maximum ordinate error of the fit.
    pub fit_max_error: f64,
    /// Every evaluation, in order.
    pub history: Vec<HistoryEntry>,
    /// Why the search stopped.
    pub stop: Stop,
}

/// Bookkeeping shared by the phases.
struct Tracker<'a, P> {
    history: Vec<HistoryEntry>,
    best: Evaluation,
    max_evaluations: usize,
    on_progress: &'a mut P,
}

impl<P: FnMut(&Progress<'_>) -> ControlFlow<()>> Tracker<'_, P> {
    fn record(
        &mut self,
        phase: Phase,
        step_size: f64,
        evaluated: &[&Evaluation],
    ) -> ControlFlow<()> {
        let start = self.history.len();
        let mut improved = false;
        for e in evaluated {
            self.history.push(HistoryEntry {
                evaluation: self.history.len() + 1,
                phase,
                objective: e.objective,
                violation: e.violation,
            });
            if e.rank_cmp(&self.best).is_lt() {
                self.best = (*e).clone();
                improved = true;
            }
        }
        (self.on_progress)(&Progress {
            phase,
            evaluations: self.history.len(),
            max_evaluations: self.max_evaluations,
            step_size,
            new: &self.history[start..],
            best: &self.best,
            improved,
        })
    }

    fn observe(&mut self, phase: Phase, step: &Step<'_, Evaluation>) -> ControlFlow<()> {
        let evaluated: Vec<&Evaluation> = step.evaluated.iter().map(|(_, e)| e).collect();
        self.record(phase, step.step_size, &evaluated)
    }
}

/// Optimizes the shape of `airfoil` for `problem`.
///
/// The airfoil is fitted with CST weights ([`crate::DesignSpace`]), the
/// fitted design is evaluated, then searched from by the chosen method.
/// `on_progress` is called after the initial evaluation and after each
/// iteration; returning [`ControlFlow::Break`] stops the search, and the
/// best design so far is returned with [`Stop::Cancelled`].
///
/// Errors: invalid problem or options, airfoil that cannot be fitted.
pub fn optimize(
    airfoil: &Airfoil,
    problem: &Problem,
    options: &OptimizeOptions,
    mut on_progress: impl FnMut(&Progress<'_>) -> ControlFlow<()>,
) -> Result<OptimizationResult> {
    options.validate()?;
    let evaluator = Evaluator::new(airfoil, problem)?;
    let space = evaluator.space();
    let x0 = space.initial();
    let n = x0.len();
    let initial = evaluator.evaluate(&x0);
    let mut tracker = Tracker {
        history: Vec::with_capacity(options.max_evaluations),
        best: initial.clone(),
        max_evaluations: options.max_evaluations,
        on_progress: &mut on_progress,
    };
    let evaluate = |x: &[f64]| evaluator.evaluate(x);

    let mut stop = Stop::MaxEvaluations;
    let mut polish_step = options.step;
    if tracker
        .record(Phase::Initial, options.step, &[&initial])
        .is_break()
    {
        stop = Stop::Cancelled;
    }

    let global = matches!(options.method, Method::CmaEsThenNelderMead | Method::CmaEs);
    let cmaes = CmaEsOptions {
        sigma: options.step,
        population: options.population,
        max_evaluations: match options.method {
            Method::CmaEs => options.max_evaluations - 1,
            _ => ((options.max_evaluations - 1) as f64 * (1.0 - options.polish_fraction)) as usize,
        },
        tol_x: CMAES_TOL_X,
        stagnation: 0,
        seed: options.seed,
        threads: options.threads,
    };
    if stop != Stop::Cancelled && global && cmaes.max_evaluations >= cmaes.population_size(n) {
        let mut last_step = options.step;
        let outcome = cmaes::minimize(evaluate, &x0, &cmaes, |step| {
            last_step = step.step_size;
            tracker.observe(Phase::Global, step)
        })?;
        stop = outcome.stop;
        polish_step = last_step.clamp(10.0 * NELDER_MEAD_TOL_X, options.step);
    }

    let local = matches!(
        options.method,
        Method::CmaEsThenNelderMead | Method::NelderMead
    );
    let remaining = options
        .max_evaluations
        .saturating_sub(tracker.history.len());
    if stop != Stop::Cancelled && local && remaining >= n {
        let nelder_mead = NelderMeadOptions {
            step: polish_step,
            max_evaluations: remaining,
            tol_x: NELDER_MEAD_TOL_X,
            threads: options.threads,
        };
        let start = tracker.best.clone();
        let outcome = nelder_mead::minimize(
            evaluate,
            &start.x,
            Some(start.clone()),
            &nelder_mead,
            |step| tracker.observe(Phase::Local, step),
        )?;
        stop = outcome.stop;
    }

    let best = tracker.best;
    let name = format!("{} optimized", airfoil.name());
    Ok(OptimizationResult {
        parameters: space.parameters(&best.x)?,
        airfoil: space.airfoil(&best.x, &name)?,
        fit_rms_error: space.fit().rms_error,
        fit_max_error: space.fit().max_error,
        initial,
        best,
        history: tracker.history,
        stop,
    })
}
