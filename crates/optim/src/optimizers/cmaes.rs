//! CMA-ES, the covariance matrix adaptation evolution strategy, with the
//! default parameters of N. Hansen, "The CMA Evolution Strategy: A
//! Tutorial" (2016): weighted recombination of the best half of the
//! population, cumulative step-size adaptation, rank-one and rank-μ
//! covariance updates.

use std::ops::ControlFlow;

use faer::{Mat, Side};
use serde::{Deserialize, Serialize};

use super::random::Random;
use super::{Outcome, Step, Stop, parallel, update_best};
use crate::error::{OptimError, Result};
use crate::rank::Ranked;

/// Options of [`minimize`].
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CmaEsOptions {
    /// Initial step size `σ₀`, in units of the variables.
    pub sigma: f64,
    /// Population size `λ`; 0 for the default `4 + ⌊3 ln n⌋`.
    pub population: usize,
    /// Evaluation budget; a generation that would exceed it is not started.
    pub max_evaluations: usize,
    /// Stops when `σ` times the largest standard deviation of the sampling
    /// distribution falls below this value.
    pub tol_x: f64,
    /// Stops after this many generations without improvement of the best
    /// value; 0 for the default `10 + ⌈30 n / λ⌉`.
    pub stagnation: usize,
    /// Seed of the sampling.
    pub seed: u64,
    /// Threads evaluating a generation; 0 for all cores.
    pub threads: usize,
}

impl Default for CmaEsOptions {
    fn default() -> Self {
        Self {
            sigma: 0.1,
            population: 0,
            max_evaluations: 10_000,
            tol_x: 1e-10,
            stagnation: 0,
            seed: 0,
            threads: 0,
        }
    }
}

impl CmaEsOptions {
    /// Population size for `n` variables.
    pub fn population_size(&self, n: usize) -> usize {
        if self.population > 0 {
            self.population
        } else {
            4 + (3.0 * (n.max(1) as f64).ln()).floor() as usize
        }
    }
}

/// Condition number of the covariance matrix at which the search stops.
const MAX_CONDITION: f64 = 1e14;

/// Minimizes `f` from `x0`. After each generation, `observer` receives the
/// population in sampling order; returning [`ControlFlow::Break`] stops the
/// search ([`Stop::Cancelled`]).
///
/// The point `x0` itself is not evaluated. Errors: empty or non-finite
/// `x0`, invalid options, budget smaller than one generation.
pub fn minimize<F, E, O>(
    f: E,
    x0: &[f64],
    options: &CmaEsOptions,
    mut observer: O,
) -> Result<Outcome<F>>
where
    F: Ranked + Clone + Send,
    E: Fn(&[f64]) -> F + Sync,
    O: FnMut(&Step<'_, F>) -> ControlFlow<()>,
{
    let n = x0.len();
    let lambda = options.population_size(n);
    let invalid = |m: &str| Err(OptimError::InvalidProblem(format!("CMA-ES: {m}")));
    if n == 0 || x0.iter().any(|v| !v.is_finite()) {
        return invalid("the start point must be non-empty and finite");
    }
    if !(options.sigma.is_finite() && options.sigma > 0.0) {
        return invalid("the initial step size must be positive");
    }
    if lambda < 2 {
        return invalid("the population needs at least 2 members");
    }
    if options.max_evaluations < lambda {
        return invalid("the budget is smaller than one generation");
    }
    if options.tol_x.is_nan() || options.tol_x < 0.0 {
        return invalid("the tolerance must be non-negative");
    }

    // Strategy parameters (tutorial, table 1).
    let nf = n as f64;
    let mu = lambda / 2;
    let mut w: Vec<f64> = (1..=mu)
        .map(|i| ((lambda as f64 + 1.0) / 2.0).ln() - (i as f64).ln())
        .collect();
    let sum: f64 = w.iter().sum();
    w.iter_mut().for_each(|v| *v /= sum);
    let mueff = 1.0 / w.iter().map(|v| v * v).sum::<f64>();
    let cc = (4.0 + mueff / nf) / (nf + 4.0 + 2.0 * mueff / nf);
    let cs = (mueff + 2.0) / (nf + mueff + 5.0);
    let c1 = 2.0 / ((nf + 1.3).powi(2) + mueff);
    let cmu = (1.0 - c1).min(2.0 * (mueff - 2.0 + 1.0 / mueff) / ((nf + 2.0).powi(2) + mueff));
    let damps = 1.0 + 2.0 * (((mueff - 1.0) / (nf + 1.0)).sqrt() - 1.0).max(0.0) + cs;
    let chin = nf.sqrt() * (1.0 - 1.0 / (4.0 * nf) + 1.0 / (21.0 * nf * nf));
    let stagnation = if options.stagnation > 0 {
        options.stagnation
    } else {
        10 + (30.0 * nf / lambda as f64).ceil() as usize
    };

    // State: mean, step size, covariance C = B D² Bᵀ, evolution paths.
    let identity = |i: usize, j: usize| if i == j { 1.0 } else { 0.0 };
    let mut mean = x0.to_vec();
    let mut sigma = options.sigma;
    let mut c = Mat::<f64>::from_fn(n, n, identity);
    let mut b = Mat::<f64>::from_fn(n, n, identity);
    let mut d = vec![1.0; n];
    let mut pc = vec![0.0; n];
    let mut ps = vec![0.0; n];
    let mut rng = Random::new(options.seed);
    let mut best: Option<(Vec<f64>, F)> = None;
    let (mut evaluations, mut generation, mut since_improvement) = (0, 0, 0);

    let stop = loop {
        if evaluations + lambda > options.max_evaluations {
            break Stop::MaxEvaluations;
        }
        generation += 1;
        let ys: Vec<Vec<f64>> = (0..lambda)
            .map(|_| {
                let z: Vec<f64> = (0..n).map(|_| rng.normal()).collect();
                (0..n)
                    .map(|i| (0..n).map(|j| b[(i, j)] * d[j] * z[j]).sum())
                    .collect()
            })
            .collect();
        let xs: Vec<Vec<f64>> = ys
            .iter()
            .map(|y| mean.iter().zip(y).map(|(m, y)| m + sigma * y).collect())
            .collect();
        let values = parallel::map(&xs, options.threads, &|x: &Vec<f64>| f(x));
        evaluations += lambda;
        let evaluated: Vec<(Vec<f64>, F)> = xs.into_iter().zip(values).collect();
        let previous = best.as_ref().map(|(x, _)| x.clone());
        update_best(&mut best, &evaluated);
        let Some(current) = best.as_ref() else {
            break Stop::NumericalLimit;
        };
        if previous.as_ref() == Some(&current.0) {
            since_improvement += 1;
        } else {
            since_improvement = 0;
        }
        let max_std = (0..n).map(|i| c[(i, i)]).fold(0.0, f64::max).sqrt();
        let step = Step {
            iteration: generation,
            evaluations,
            step_size: sigma * max_std,
            evaluated: &evaluated,
            best: current,
        };
        if observer(&step).is_break() {
            break Stop::Cancelled;
        }

        // Selection and recombination.
        let mut order: Vec<usize> = (0..lambda).collect();
        order.sort_by(|&i, &j| evaluated[i].1.rank_cmp(&evaluated[j].1));
        let selected: Vec<&Vec<f64>> = order[..mu].iter().map(|&k| &ys[k]).collect();
        let yw: Vec<f64> = (0..n)
            .map(|i| w.iter().zip(&selected).map(|(wk, y)| wk * y[i]).sum())
            .collect();
        for (m, y) in mean.iter_mut().zip(&yw) {
            *m += sigma * y;
        }

        // Step-size path, with C^(-1/2) = B D⁻¹ Bᵀ.
        let bt_yw: Vec<f64> = (0..n)
            .map(|j| (0..n).map(|i| b[(i, j)] * yw[i]).sum::<f64>() / d[j])
            .collect();
        let ks = (cs * (2.0 - cs) * mueff).sqrt();
        for (i, p) in ps.iter_mut().enumerate() {
            let cinv_yw: f64 = (0..n).map(|j| b[(i, j)] * bt_yw[j]).sum();
            *p = (1.0 - cs) * *p + ks * cinv_yw;
        }
        let ps_norm = ps.iter().map(|v| v * v).sum::<f64>().sqrt();
        let hsig = ps_norm / (1.0 - (1.0 - cs).powf(2.0 * generation as f64)).sqrt() / chin
            < 1.4 + 2.0 / (nf + 1.0);
        let h = if hsig { 1.0 } else { 0.0 };

        // Covariance path and update.
        let kc = (cc * (2.0 - cc) * mueff).sqrt();
        for (p, y) in pc.iter_mut().zip(&yw) {
            *p = (1.0 - cc) * *p + h * kc * y;
        }
        let decay = 1.0 - c1 - cmu + (1.0 - h) * c1 * cc * (2.0 - cc);
        for i in 0..n {
            for j in 0..=i {
                let rank_mu: f64 = w
                    .iter()
                    .zip(&selected)
                    .map(|(wk, y)| wk * y[i] * y[j])
                    .sum();
                let v = decay * c[(i, j)] + c1 * pc[i] * pc[j] + cmu * rank_mu;
                c[(i, j)] = v;
                c[(j, i)] = v;
            }
        }
        sigma *= ((cs / damps) * (ps_norm / chin - 1.0)).min(1.0).exp();

        // C = B D² Bᵀ.
        let Ok(eigen) = c.self_adjoint_eigen(Side::Lower) else {
            break Stop::NumericalLimit;
        };
        let values = eigen.S().column_vector();
        let (lo, hi) = (values[0], values[n - 1]);
        if !(lo > 0.0 && hi.is_finite() && hi / lo < MAX_CONDITION && sigma.is_finite()) {
            break Stop::NumericalLimit;
        }
        for (j, dj) in d.iter_mut().enumerate() {
            *dj = values[j].sqrt();
        }
        b = eigen.U().to_owned();

        let max_std = (0..n).map(|i| c[(i, i)]).fold(0.0, f64::max).sqrt();
        if sigma * max_std < options.tol_x {
            break Stop::Converged;
        }
        if since_improvement >= stagnation {
            break Stop::Stagnation;
        }
    };

    match best {
        Some(best) => Ok(Outcome {
            best,
            evaluations,
            iterations: generation,
            stop,
        }),
        None => invalid("no generation was evaluated"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rank::feasibility_cmp;
    use std::cmp::Ordering;

    fn sphere(x: &[f64]) -> f64 {
        x.iter().map(|v| v * v).sum()
    }

    fn rosenbrock(x: &[f64]) -> f64 {
        x.windows(2)
            .map(|p| 100.0 * (p[1] - p[0] * p[0]).powi(2) + (1.0 - p[0]).powi(2))
            .sum()
    }

    fn run(f: fn(&[f64]) -> f64, x0: &[f64], options: &CmaEsOptions) -> Outcome<f64> {
        minimize(f, x0, options, |_| ControlFlow::Continue(())).unwrap()
    }

    #[test]
    fn sphere_converges() {
        let o = run(
            sphere,
            &[1.0; 10],
            &CmaEsOptions {
                sigma: 0.5,
                ..CmaEsOptions::default()
            },
        );
        assert_eq!(o.stop, Stop::Converged);
        assert!(o.best.1 < 1e-18, "{o:?}");
        // About 1500 evaluations in 10 dimensions (Hansen's figures).
        assert!(o.evaluations < 4000, "{}", o.evaluations);
    }

    #[test]
    fn rosenbrock_converges() {
        let o = run(
            rosenbrock,
            &[0.0; 8],
            &CmaEsOptions {
                sigma: 0.3,
                seed: 3,
                ..CmaEsOptions::default()
            },
        );
        assert!(o.best.1 < 1e-14, "{o:?}");
        assert!(o.best.0.iter().all(|v| (v - 1.0).abs() < 1e-6));
    }

    #[test]
    fn deterministic_whatever_the_threads() {
        let options = |threads| CmaEsOptions {
            sigma: 0.3,
            max_evaluations: 600,
            seed: 11,
            threads,
            ..CmaEsOptions::default()
        };
        let a = run(rosenbrock, &[0.0; 5], &options(1));
        let b = run(rosenbrock, &[0.0; 5], &options(4));
        assert_eq!(a, b);
        assert_eq!(a.stop, Stop::MaxEvaluations);
        assert!(a.evaluations <= 600);
        let c = run(
            rosenbrock,
            &[0.0; 5],
            &CmaEsOptions {
                seed: 12,
                ..options(1)
            },
        );
        assert_ne!(a.best.0, c.best.0);
    }

    #[test]
    fn observer_sees_every_evaluation_and_can_stop() {
        let mut seen = 0;
        let mut generations = 0;
        let o = minimize(sphere, &[1.0; 4], &CmaEsOptions::default(), |s| {
            seen += s.evaluated.len();
            generations += 1;
            assert_eq!(s.iteration, generations);
            assert!(s.evaluated.iter().all(|(_, v)| *v >= s.best.1));
            if generations == 3 {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        })
        .unwrap();
        assert_eq!(o.stop, Stop::Cancelled);
        assert_eq!((o.iterations, o.evaluations, seen), (3, 3 * 8, 3 * 8));
    }

    /// Value with a constraint violation, ranked by the feasibility rules.
    #[derive(Debug, Clone, PartialEq)]
    struct Constrained {
        objective: f64,
        violation: f64,
    }

    impl Ranked for Constrained {
        fn rank_cmp(&self, other: &Self) -> Ordering {
            feasibility_cmp(
                self.violation,
                Some(self.objective),
                other.violation,
                Some(other.objective),
            )
        }
    }

    #[test]
    fn constrained_optimum_on_the_boundary() {
        // min Σ x², subject to x₀ + x₁ ≥ 1: optimum (0.5, 0.5, 0, 0).
        let f = |x: &[f64]| Constrained {
            objective: sphere(x),
            violation: (1.0 - x[0] - x[1]).max(0.0),
        };
        let o = minimize(
            f,
            &[2.0, 2.0, 1.0, 1.0],
            &CmaEsOptions {
                sigma: 0.5,
                seed: 5,
                ..CmaEsOptions::default()
            },
            |_| ControlFlow::Continue(()),
        )
        .unwrap();
        let x = &o.best.0;
        assert_eq!(o.best.1.violation, 0.0);
        assert!(
            (x[0] - 0.5).abs() < 1e-3 && (x[1] - 0.5).abs() < 1e-3,
            "{o:?}"
        );
        assert!(x[2].abs() < 1e-3 && x[3].abs() < 1e-3);
    }

    #[test]
    fn invalid_options() {
        let ok = CmaEsOptions::default();
        let cases = [
            (vec![], ok),
            (vec![f64::NAN], ok),
            (vec![1.0], CmaEsOptions { sigma: 0.0, ..ok }),
            (
                vec![1.0],
                CmaEsOptions {
                    population: 1,
                    ..ok
                },
            ),
            (
                vec![1.0],
                CmaEsOptions {
                    max_evaluations: 2,
                    ..ok
                },
            ),
            (vec![1.0], CmaEsOptions { tol_x: -1.0, ..ok }),
        ];
        for (k, (x0, o)) in cases.iter().enumerate() {
            assert!(
                minimize(sphere, x0, o, |_| ControlFlow::Continue(())).is_err(),
                "case {k}"
            );
        }
    }
}
