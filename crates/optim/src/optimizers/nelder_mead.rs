//! Nelder–Mead simplex method with the dimension-dependent parameters of
//! F. Gao and L. Han, "Implementing the Nelder-Mead simplex algorithm with
//! adaptive parameters" (2012).

use std::ops::ControlFlow;

use serde::{Deserialize, Serialize};

use super::{Outcome, Step, Stop, parallel, update_best};
use crate::error::{OptimError, Result};
use crate::rank::Ranked;

/// Options of [`minimize`].
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NelderMeadOptions {
    /// Edge of the initial simplex: vertices `x0 + step·eᵢ`.
    pub step: f64,
    /// Evaluation budget.
    pub max_evaluations: usize,
    /// Stops when every vertex is within this distance (largest coordinate
    /// difference) of the best one.
    pub tol_x: f64,
    /// Threads evaluating the initial simplex and the shrink steps; 0 for
    /// all cores.
    pub threads: usize,
}

impl Default for NelderMeadOptions {
    fn default() -> Self {
        Self {
            step: 0.1,
            max_evaluations: 10_000,
            tol_x: 1e-10,
            threads: 0,
        }
    }
}

/// Minimizes `f` from `x0`, whose value can be given as `f0` to save an
/// evaluation. After the initial simplex and after each step, `observer`
/// receives the points evaluated; returning [`ControlFlow::Break`] stops
/// the search ([`Stop::Cancelled`]).
///
/// The simplex keeps its best vertex, so the result is never worse than
/// `x0`. Errors: empty or non-finite `x0`, invalid options.
pub fn minimize<F, E, O>(
    f: E,
    x0: &[f64],
    f0: Option<F>,
    options: &NelderMeadOptions,
    mut observer: O,
) -> Result<Outcome<F>>
where
    F: Ranked + Clone + Send,
    E: Fn(&[f64]) -> F + Sync,
    O: FnMut(&Step<'_, F>) -> ControlFlow<()>,
{
    let n = x0.len();
    let invalid = |m: &str| Err(OptimError::InvalidProblem(format!("Nelder-Mead: {m}")));
    if n == 0 || x0.iter().any(|v| !v.is_finite()) {
        return invalid("the start point must be non-empty and finite");
    }
    if !(options.step.is_finite() && options.step != 0.0) {
        return invalid("the initial step must be finite and non-zero");
    }
    if options.tol_x.is_nan() || options.tol_x < 0.0 {
        return invalid("the tolerance must be non-negative");
    }
    let needed = n + usize::from(f0.is_none());
    if options.max_evaluations < needed {
        return invalid("the budget is smaller than the initial simplex");
    }

    // Reflection, expansion, contraction and shrink coefficients.
    let nf = n as f64;
    let (alpha, beta, gamma, delta) = if n >= 2 {
        (1.0, 1.0 + 2.0 / nf, 0.75 - 0.5 / nf, 1.0 - 1.0 / nf)
    } else {
        (1.0, 2.0, 0.5, 0.5)
    };

    let mut points: Vec<Vec<f64>> = (0..n)
        .map(|i| {
            let mut x = x0.to_vec();
            x[i] += options.step;
            x
        })
        .collect();
    let mut simplex: Vec<(Vec<f64>, F)> = Vec::with_capacity(n + 1);
    match f0 {
        Some(v) => simplex.push((x0.to_vec(), v)),
        None => points.insert(0, x0.to_vec()),
    }
    let values = parallel::map(&points, options.threads, &|x: &Vec<f64>| f(x));
    let mut evaluations = points.len();
    let first: Vec<(Vec<f64>, F)> = points.into_iter().zip(values).collect();
    simplex.extend(first.iter().cloned());
    let mut best = None;
    update_best(&mut best, &simplex);
    let mut iteration = 0;
    let size = |s: &[(Vec<f64>, F)]| {
        s.iter()
            .flat_map(|(x, _)| x.iter().zip(&s[0].0).map(|(a, b)| (a - b).abs()))
            .fold(0.0, f64::max)
    };

    let mut report = |iteration: usize,
                      evaluations: usize,
                      simplex: &mut Vec<(Vec<f64>, F)>,
                      evaluated: &[(Vec<f64>, F)],
                      best: &mut Option<(Vec<f64>, F)>| {
        simplex.sort_by(|a, b| a.1.rank_cmp(&b.1));
        update_best(best, evaluated);
        let current = best.as_ref()?;
        let step = Step {
            iteration,
            evaluations,
            step_size: size(simplex),
            evaluated,
            best: current,
        };
        Some(observer(&step))
    };

    iteration += 1;
    let stop = 'search: {
        if report(iteration, evaluations, &mut simplex, &first, &mut best)
            .is_none_or(|c| c.is_break())
        {
            break 'search Stop::Cancelled;
        }
        loop {
            if size(&simplex) < options.tol_x {
                break 'search Stop::Converged;
            }
            let worst = n;
            let centroid: Vec<f64> = (0..n)
                .map(|j| simplex[..n].iter().map(|(x, _)| x[j]).sum::<f64>() / nf)
                .collect();
            let along = |t: f64| -> Vec<f64> {
                centroid
                    .iter()
                    .zip(&simplex[worst].0)
                    .map(|(c, w)| c + t * (c - w))
                    .collect()
            };
            let mut evaluated: Vec<(Vec<f64>, F)> = Vec::new();
            let mut eval = |x: Vec<f64>, evaluated: &mut Vec<(Vec<f64>, F)>| {
                if evaluations >= options.max_evaluations {
                    return None;
                }
                evaluations += 1;
                let v = f(&x);
                evaluated.push((x, v.clone()));
                Some(v)
            };

            let Some(fr) = eval(along(alpha), &mut evaluated) else {
                break 'search Stop::MaxEvaluations;
            };
            let xr = evaluated[0].0.clone();
            let mut replacement = None;
            if fr.rank_cmp(&simplex[0].1).is_lt() {
                let Some(fe) = eval(along(alpha * beta), &mut evaluated) else {
                    break 'search Stop::MaxEvaluations;
                };
                replacement = Some(if fe.rank_cmp(&fr).is_lt() {
                    evaluated[1].clone()
                } else {
                    (xr, fr)
                });
            } else if fr.rank_cmp(&simplex[n - 1].1).is_lt() {
                replacement = Some((xr, fr));
            } else if fr.rank_cmp(&simplex[worst].1).is_lt() {
                let Some(fc) = eval(along(alpha * gamma), &mut evaluated) else {
                    break 'search Stop::MaxEvaluations;
                };
                if fc.rank_cmp(&fr).is_le() {
                    replacement = Some(evaluated[1].clone());
                }
            } else {
                let Some(fc) = eval(along(-gamma), &mut evaluated) else {
                    break 'search Stop::MaxEvaluations;
                };
                if fc.rank_cmp(&simplex[worst].1).is_lt() {
                    replacement = Some(evaluated[1].clone());
                }
            }

            match replacement {
                Some(vertex) => simplex[worst] = vertex,
                None => {
                    // Shrink toward the best vertex.
                    if evaluations + n > options.max_evaluations {
                        break 'search Stop::MaxEvaluations;
                    }
                    let x_best = simplex[0].0.clone();
                    let shrunk: Vec<Vec<f64>> = simplex[1..]
                        .iter()
                        .map(|(x, _)| {
                            x.iter()
                                .zip(&x_best)
                                .map(|(xi, bi)| bi + delta * (xi - bi))
                                .collect()
                        })
                        .collect();
                    let values = parallel::map(&shrunk, options.threads, &|x: &Vec<f64>| f(x));
                    evaluations += n;
                    let fresh: Vec<(Vec<f64>, F)> = shrunk.into_iter().zip(values).collect();
                    simplex.truncate(1);
                    simplex.extend(fresh.iter().cloned());
                    evaluated.extend(fresh);
                }
            }

            iteration += 1;
            if report(iteration, evaluations, &mut simplex, &evaluated, &mut best)
                .is_none_or(|c| c.is_break())
            {
                break 'search Stop::Cancelled;
            }
        }
    };

    match best {
        Some(best) => Ok(Outcome {
            best,
            evaluations,
            iterations: iteration,
            stop,
        }),
        None => invalid("nothing was evaluated"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rosenbrock(x: &[f64]) -> f64 {
        x.windows(2)
            .map(|p| 100.0 * (p[1] - p[0] * p[0]).powi(2) + (1.0 - p[0]).powi(2))
            .sum()
    }

    fn run(f: fn(&[f64]) -> f64, x0: &[f64], options: &NelderMeadOptions) -> Outcome<f64> {
        minimize(f, x0, None, options, |_| ControlFlow::Continue(())).unwrap()
    }

    #[test]
    fn rosenbrock_2d() {
        let o = run(rosenbrock, &[-1.2, 1.0], &NelderMeadOptions::default());
        assert_eq!(o.stop, Stop::Converged);
        assert!(o.best.1 < 1e-16, "{o:?}");
        assert!(o.evaluations < 400, "{}", o.evaluations);
    }

    #[test]
    fn quadratic_in_8_dimensions() {
        let f = |x: &[f64]| -> f64 {
            x.iter()
                .enumerate()
                .map(|(i, v)| (i + 1) as f64 * (v - 0.5).powi(2))
                .sum()
        };
        let o = minimize(f, &[0.0; 8], None, &NelderMeadOptions::default(), |_| {
            ControlFlow::Continue(())
        })
        .unwrap();
        assert!(o.best.1 < 1e-14, "{o:?}");
    }

    #[test]
    fn shrinks_and_keeps_the_best_vertex() {
        // Every point but the start is worse: reflections and contractions
        // all fail, and the simplex shrinks toward the start until it
        // converges there.
        let x0 = [0.3, -0.2, 0.1];
        let f = |x: &[f64]| -> f64 { if x == x0 { 0.0 } else { 1.0 } };
        let mut shrinks = 0;
        let o = minimize(f, &x0, None, &NelderMeadOptions::default(), |s| {
            // Reflection, inside contraction and the 3 shrunk vertices.
            if s.iteration > 1 {
                assert_eq!(s.evaluated.len(), 5);
                shrinks += 1;
            }
            assert_eq!(s.best.0, x0);
            ControlFlow::Continue(())
        })
        .unwrap();
        assert_eq!(o.stop, Stop::Converged);
        assert_eq!(o.best, (x0.to_vec(), 0.0));
        assert_eq!(shrinks, o.iterations - 1);
    }

    #[test]
    fn known_start_value_budget_and_cancel() {
        let mut calls = 0;
        let o = minimize(
            rosenbrock,
            &[0.0, 0.0],
            Some(1.0),
            &NelderMeadOptions {
                max_evaluations: 30,
                ..NelderMeadOptions::default()
            },
            |s| {
                calls += 1;
                // The start point is not evaluated again.
                assert!(s.iteration > 1 || s.evaluated.len() == 2);
                ControlFlow::Continue(())
            },
        )
        .unwrap();
        assert_eq!(o.stop, Stop::MaxEvaluations);
        assert!(o.evaluations <= 30 && o.best.1 < 1.0);
        assert_eq!(calls, o.iterations);

        let o = minimize(
            rosenbrock,
            &[0.0, 0.0],
            None,
            &NelderMeadOptions::default(),
            |s| {
                if s.iteration == 4 {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            },
        )
        .unwrap();
        assert_eq!((o.stop, o.iterations), (Stop::Cancelled, 4));
    }

    #[test]
    fn deterministic_whatever_the_threads() {
        let options = |threads| NelderMeadOptions {
            threads,
            max_evaluations: 500,
            ..NelderMeadOptions::default()
        };
        let a = run(rosenbrock, &[0.0; 6], &options(1));
        let b = run(rosenbrock, &[0.0; 6], &options(5));
        assert_eq!(a, b);
    }

    #[test]
    fn invalid_options() {
        let ok = NelderMeadOptions::default();
        let cases = [
            (vec![], ok),
            (vec![f64::INFINITY], ok),
            (vec![1.0], NelderMeadOptions { step: 0.0, ..ok }),
            (vec![1.0], NelderMeadOptions { tol_x: -1.0, ..ok }),
            (
                vec![1.0, 2.0],
                NelderMeadOptions {
                    max_evaluations: 2,
                    ..ok
                },
            ),
        ];
        for (k, (x0, o)) in cases.iter().enumerate() {
            assert!(run_checked(x0, o).is_err(), "case {k}");
        }
    }

    fn run_checked(x0: &[f64], o: &NelderMeadOptions) -> Result<Outcome<f64>> {
        minimize(rosenbrock, x0, None, o, |_| ControlFlow::Continue(()))
    }
}
