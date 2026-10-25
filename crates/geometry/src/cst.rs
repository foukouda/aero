//! Class/Shape function Transformation (CST) parameterization (Kulfan).
//!
//! Each surface is written in chord-line axes as
//!
//! ```text
//! y(x) = C(x) · S(x) + x · y_te,   C(x) = x^N1 · (1 − x)^N2,
//! S(x) = Σ_{i=0..n} w_i · K_i · x^i · (1 − x)^(n−i),   K_i = n! / (i! (n − i)!)
//! ```
//!
//! where `n` is the order, `w_i` the shape weights and `y_te` the trailing
//! edge ordinate of the surface. `N1 = 0.5, N2 = 1` gives round-nosed
//! airfoils with a finite trailing-edge angle.

use faer::Mat;
use faer::linalg::solvers::SolveLstsq;
use serde::{Deserialize, Serialize};

use crate::airfoil::Airfoil;
use crate::error::{GeometryError, Result};
use crate::leading_edge::lefind;
use crate::transform::Normalization;

/// CST parameters of an airfoil.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CstParameters {
    /// Upper-surface shape weights (`order + 1` values).
    pub upper: Vec<f64>,
    /// Lower-surface shape weights (`order + 1` values, usually negative).
    pub lower: Vec<f64>,
    /// Class function leading-edge exponent `N1`.
    pub n1: f64,
    /// Class function trailing-edge exponent `N2`.
    pub n2: f64,
    /// Upper-surface trailing-edge ordinate `y_u(1)`.
    pub te_upper: f64,
    /// Lower-surface trailing-edge ordinate `y_l(1)`.
    pub te_lower: f64,
}

/// Options of [`fit_cst`].
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CstFitOptions {
    /// Bernstein polynomial order `n` (each surface gets `n + 1` weights).
    pub order: usize,
    /// Class function exponent `N1`.
    pub n1: f64,
    /// Class function exponent `N2`.
    pub n2: f64,
    /// Tikhonov (ridge) regularization weight, 0 for a plain least-squares
    /// fit. Useful for high orders where the problem is ill-conditioned.
    pub regularization: f64,
    /// Normalize the airfoil ([`Normalization::ChordAligned`]) before
    /// fitting. Disable only for coordinates that are already in unit-chord,
    /// chord-aligned axes, to avoid the small shift caused by locating the
    /// leading edge on the spline.
    pub normalize: bool,
}

impl Default for CstFitOptions {
    fn default() -> Self {
        Self {
            order: 8,
            n1: 0.5,
            n2: 1.0,
            regularization: 0.0,
            normalize: true,
        }
    }
}

/// Result of a CST fit.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CstFit {
    /// Fitted parameters (in normalized, chord-aligned axes).
    pub parameters: CstParameters,
    /// Root-mean-square ordinate error over the fitted points (chord units).
    pub rms_error: f64,
    /// Maximum absolute ordinate error (chord units).
    pub max_error: f64,
}

impl CstParameters {
    /// Creates parameters with the usual exponents `N1 = 0.5`, `N2 = 1` and a
    /// closed trailing edge.
    pub fn new(upper: Vec<f64>, lower: Vec<f64>) -> Result<Self> {
        let p = Self {
            upper,
            lower,
            n1: 0.5,
            n2: 1.0,
            te_upper: 0.0,
            te_lower: 0.0,
        };
        p.validate()?;
        Ok(p)
    }

    /// Bernstein order `n`.
    pub fn order(&self) -> usize {
        self.upper.len().saturating_sub(1)
    }

    fn validate(&self) -> Result<()> {
        if self.upper.is_empty() || self.upper.len() != self.lower.len() {
            return Err(GeometryError::InvalidOption(
                "CST upper and lower weights must be non-empty and of equal length".into(),
            ));
        }
        let all = self.upper.iter().chain(&self.lower).chain([
            &self.n1,
            &self.n2,
            &self.te_upper,
            &self.te_lower,
        ]);
        if all.into_iter().any(|v| !v.is_finite()) {
            return Err(GeometryError::InvalidOption(
                "CST parameters must be finite".into(),
            ));
        }
        Ok(())
    }

    /// Upper-surface ordinate at `x ∈ [0, 1]`.
    pub fn upper_y(&self, x: f64) -> f64 {
        surface(&self.upper, self.n1, self.n2, self.te_upper, x)
    }

    /// Lower-surface ordinate at `x ∈ [0, 1]`.
    pub fn lower_y(&self, x: f64) -> f64 {
        surface(&self.lower, self.n1, self.n2, self.te_lower, x)
    }

    /// Generates a Selig-ordered airfoil with cosine spacing and
    /// `points_per_side` points per surface (leading edge shared).
    pub fn to_airfoil(&self, name: impl Into<String>, points_per_side: usize) -> Result<Airfoil> {
        self.validate()?;
        if points_per_side < 3 {
            return Err(GeometryError::InvalidOption(
                "at least 3 points per side are required".into(),
            ));
        }
        let xs: Vec<f64> = (0..points_per_side)
            .map(|k| {
                let t = std::f64::consts::PI * k as f64 / (points_per_side - 1) as f64;
                0.5 * (1.0 - t.cos())
            })
            .collect();
        let mut x = Vec::with_capacity(2 * points_per_side - 1);
        let mut y = Vec::with_capacity(2 * points_per_side - 1);
        for &xi in xs.iter().rev() {
            x.push(xi);
            y.push(self.upper_y(xi));
        }
        for &xi in &xs[1..] {
            x.push(xi);
            y.push(self.lower_y(xi));
        }
        Airfoil::new(name, x, y)
    }
}

fn binomial(n: usize, k: usize) -> f64 {
    (0..k).fold(1.0, |acc, i| acc * (n - i) as f64 / (i + 1) as f64)
}

fn class(x: f64, n1: f64, n2: f64) -> f64 {
    x.powf(n1) * (1.0 - x).powf(n2)
}

/// Basis value `C(x)·K_i·x^i·(1−x)^(n−i)`.
fn basis(n: usize, i: usize, n1: f64, n2: f64, x: f64) -> f64 {
    class(x, n1, n2) * binomial(n, i) * x.powi(i as i32) * (1.0 - x).powi((n - i) as i32)
}

fn surface(weights: &[f64], n1: f64, n2: f64, te: f64, x: f64) -> f64 {
    let n = weights.len().saturating_sub(1);
    let shape: f64 = weights
        .iter()
        .enumerate()
        .map(|(i, w)| w * basis(n, i, n1, n2, x))
        .sum();
    shape + x * te
}

/// Fits CST parameters to an airfoil by linear least squares.
///
/// Unless disabled by [`CstFitOptions::normalize`], the airfoil is first
/// normalized with [`Normalization::ChordAligned`] (leading edge at the
/// origin, trailing edge at `(1, 0)`); the returned parameters live in those
/// axes. Points are split into surfaces at the leading edge found by
/// `LEFIND`. Errors are vertical distances in chord units at the input points.
pub fn fit_cst(airfoil: &Airfoil, options: &CstFitOptions) -> Result<CstFit> {
    if options.order == 0 || options.order > 30 {
        return Err(GeometryError::InvalidOption(format!(
            "CST order must be between 1 and 30 (got {})",
            options.order
        )));
    }
    let valid = options.n1.is_finite()
        && options.n2.is_finite()
        && options.regularization.is_finite()
        && options.regularization >= 0.0;
    if !valid {
        return Err(GeometryError::InvalidOption(
            "CST exponents and regularization must be finite (regularization >= 0)".into(),
        ));
    }

    let af = if options.normalize {
        airfoil.normalized(Normalization::ChordAligned)?
    } else {
        airfoil.clone()
    };
    let curve = af.spline();
    let (sle, _) = lefind(&curve);
    let s = curve.arc_lengths();
    let last = af.len() - 1;
    let te_upper = af.y()[0];
    let te_lower = af.y()[last];

    let mut upper = Vec::new();
    let mut lower = Vec::new();
    for (i, [x, y]) in af.points().enumerate() {
        let x = x.clamp(0.0, 1.0);
        if s[i] < sle {
            upper.push((x, y));
        } else if s[i] > sle {
            lower.push((x, y));
        }
    }

    let fit_side = |pts: &[(f64, f64)], te: f64| -> Result<Vec<f64>> {
        let n = options.order;
        let m = pts.len();
        if m < n + 1 {
            return Err(GeometryError::TooFewPoints {
                found: m,
                min: n + 1,
            });
        }
        let ridge = options.regularization.sqrt();
        let rows = if ridge > 0.0 { m + n + 1 } else { m };
        let a = Mat::<f64>::from_fn(rows, n + 1, |r, c| {
            if r < m {
                basis(n, c, options.n1, options.n2, pts[r].0)
            } else if r - m == c {
                ridge
            } else {
                0.0
            }
        });
        let b = Mat::<f64>::from_fn(
            rows,
            1,
            |r, _| {
                if r < m { pts[r].1 - pts[r].0 * te } else { 0.0 }
            },
        );
        let w = a.qr().solve_lstsq(&b);
        let weights: Vec<f64> = (0..=n).map(|i| w[(i, 0)]).collect();
        if weights.iter().any(|v| !v.is_finite()) {
            return Err(GeometryError::Numerical(
                "CST least-squares problem is singular".into(),
            ));
        }
        Ok(weights)
    };

    let parameters = CstParameters {
        upper: fit_side(&upper, te_upper)?,
        lower: fit_side(&lower, te_lower)?,
        n1: options.n1,
        n2: options.n2,
        te_upper,
        te_lower,
    };

    let errors: Vec<f64> = upper
        .iter()
        .map(|&(x, y)| parameters.upper_y(x) - y)
        .chain(lower.iter().map(|&(x, y)| parameters.lower_y(x) - y))
        .collect();
    let rms_error = (errors.iter().map(|e| e * e).sum::<f64>() / errors.len() as f64).sqrt();
    let max_error = errors.iter().fold(0.0_f64, |m, e| m.max(e.abs()));

    Ok(CstFit {
        parameters,
        rms_error,
        max_error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naca::{NacaOptions, naca};

    fn sample() -> CstParameters {
        let mut p = CstParameters::new(
            vec![0.17, 0.16, 0.19, 0.15, 0.18],
            vec![-0.15, -0.10, -0.12, -0.05, -0.08],
        )
        .unwrap();
        p.te_upper = 0.001;
        p.te_lower = -0.001;
        p
    }

    #[test]
    fn binomials_and_partition_of_unity() {
        assert_eq!(binomial(8, 0), 1.0);
        assert_eq!(binomial(8, 3), 56.0);
        // Bernstein polynomials sum to one.
        for x in [0.1_f64, 0.5, 0.9] {
            let sum: f64 = (0..=6)
                .map(|i| binomial(6, i) * x.powi(i as i32) * (1.0 - x).powi(6 - i as i32))
                .sum();
            assert!((sum - 1.0).abs() < 1e-14);
        }
    }

    #[test]
    fn generated_airfoil_shape() {
        let p = sample();
        assert_eq!(p.order(), 4);
        let af = p.to_airfoil("cst", 81).unwrap();
        assert_eq!(af.len(), 161);
        assert_eq!(af.x()[80], 0.0);
        assert_eq!(af.y()[80], 0.0);
        assert!((af.y()[0] - 0.001).abs() < 1e-15);
        assert!((af.y()[160] + 0.001).abs() < 1e-15);
        assert!(p.upper_y(0.3) > 0.0 && p.lower_y(0.3) < 0.0);
    }

    #[test]
    fn fit_recovers_exact_cst_parameters() {
        let p = sample();
        let af = p.to_airfoil("cst", 101).unwrap();
        let exact = CstFitOptions {
            order: 4,
            normalize: false,
            ..CstFitOptions::default()
        };
        let fit = fit_cst(&af, &exact).unwrap();
        assert!(fit.max_error < 1e-12, "{}", fit.max_error);
        for (a, b) in p.upper.iter().zip(&fit.parameters.upper) {
            assert!((a - b).abs() < 1e-9, "{a} {b}");
        }
        for (a, b) in p.lower.iter().zip(&fit.parameters.lower) {
            assert!((a - b).abs() < 1e-9, "{a} {b}");
        }
        // With normalization the leading edge is located on the spline,
        // which shifts the axes very slightly.
        let normalized = fit_cst(
            &af,
            &CstFitOptions {
                normalize: true,
                ..exact
            },
        )
        .unwrap();
        assert!(normalized.max_error < 2e-4, "{}", normalized.max_error);
        for (a, b) in p.upper.iter().zip(&normalized.parameters.upper) {
            assert!((a - b).abs() < 1e-3, "{a} {b}");
        }
    }

    #[test]
    fn fit_naca0012_converges_with_order() {
        let af = naca("0012", &NacaOptions::default()).unwrap();
        let low = fit_cst(
            &af,
            &CstFitOptions {
                order: 3,
                ..CstFitOptions::default()
            },
        )
        .unwrap();
        let high = fit_cst(&af, &CstFitOptions::default()).unwrap();
        assert!(high.rms_error < low.rms_error);
        assert!(high.rms_error < 1e-4, "rms {}", high.rms_error);
        assert!(high.max_error < 5e-4, "max {}", high.max_error);
        // Symmetric section: lower weights mirror the upper ones.
        for (u, l) in high.parameters.upper.iter().zip(&high.parameters.lower) {
            assert!((u + l).abs() < 1e-6);
        }
    }

    #[test]
    fn fit_is_invariant_to_scaling_and_rotation() {
        let af = naca("2412", &NacaOptions::default()).unwrap();
        let (s, c) = 0.2_f64.sin_cos();
        let moved = af
            .mapped(|x, y| (3.0 * (c * x - s * y) + 1.0, 3.0 * (s * x + c * y) - 2.0))
            .unwrap();
        let a = fit_cst(&af, &CstFitOptions::default()).unwrap();
        let b = fit_cst(&moved, &CstFitOptions::default()).unwrap();
        for (u, v) in a.parameters.upper.iter().zip(&b.parameters.upper) {
            assert!((u - v).abs() < 1e-6, "{u} {v}");
        }
    }

    #[test]
    fn regularization_and_invalid_options() {
        let af = naca("0012", &NacaOptions::default()).unwrap();
        let ridge = fit_cst(
            &af,
            &CstFitOptions {
                order: 12,
                regularization: 1e-8,
                ..CstFitOptions::default()
            },
        )
        .unwrap();
        assert!(ridge.rms_error < 1e-3);
        for order in [0, 31] {
            let o = CstFitOptions {
                order,
                ..CstFitOptions::default()
            };
            assert!(fit_cst(&af, &o).is_err());
        }
        let neg = CstFitOptions {
            regularization: -1.0,
            ..CstFitOptions::default()
        };
        assert!(fit_cst(&af, &neg).is_err());
        assert!(CstParameters::new(vec![], vec![]).is_err());
        assert!(CstParameters::new(vec![0.1], vec![0.1, 0.2]).is_err());
        assert!(sample().to_airfoil("x", 2).is_err());
    }

    #[test]
    fn serde_round_trip() {
        let p = sample();
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(serde_json::from_str::<CstParameters>(&json).unwrap(), p);
    }
}
