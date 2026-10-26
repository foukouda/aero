//! Design space: the CST shape weights of both surfaces.

use aero_geometry::{Airfoil, CstFit, CstFitOptions, CstParameters, fit_cst};
use serde::{Deserialize, Serialize};

use crate::error::{OptimError, Result};

/// Options of the design space.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DesignOptions {
    /// Bernstein order `n` of each surface: `n + 1` weights per surface,
    /// `2 (n + 1)` design variables.
    pub order: usize,
    /// Points per surface of the generated airfoils (cosine spacing,
    /// leading edge shared), before paneling.
    pub points_per_side: usize,
}

impl Default for DesignOptions {
    fn default() -> Self {
        Self {
            order: 6,
            points_per_side: 101,
        }
    }
}

impl DesignOptions {
    /// Checks that the options are in range.
    pub fn validate(&self) -> Result<()> {
        if !(2..=20).contains(&self.order) {
            return Err(OptimError::InvalidProblem(format!(
                "CST order must be between 2 and 20 (got {})",
                self.order
            )));
        }
        if !(21..=1000).contains(&self.points_per_side) {
            return Err(OptimError::InvalidProblem(format!(
                "points per side must be between 21 and 1000 (got {})",
                self.points_per_side
            )));
        }
        Ok(())
    }
}

/// Stations where the surfaces must not cross, as a fraction of the chord.
const VALIDITY_STATIONS: usize = 200;

/// The design space: maps a vector of design variables to an airfoil.
///
/// The variables are the CST weights `[upper_0 … upper_n, lower_0 …
/// lower_n]`, started from a least-squares fit of the initial airfoil
/// (normalized: leading edge at the origin, unit chord along x). The class
/// exponents and the trailing-edge ordinates, hence the trailing-edge
/// thickness, are those of the fit and stay fixed.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignSpace {
    fit: CstFit,
    points_per_side: usize,
}

impl DesignSpace {
    /// Fits the initial airfoil.
    pub fn new(airfoil: &Airfoil, options: &DesignOptions) -> Result<Self> {
        options.validate()?;
        let fit = fit_cst(
            airfoil,
            &CstFitOptions {
                order: options.order,
                ..CstFitOptions::default()
            },
        )?;
        Ok(Self {
            fit,
            points_per_side: options.points_per_side,
        })
    }

    /// Number of design variables.
    pub fn dimension(&self) -> usize {
        self.fit.parameters.upper.len() + self.fit.parameters.lower.len()
    }

    /// The least-squares fit of the initial airfoil.
    pub fn fit(&self) -> &CstFit {
        &self.fit
    }

    /// Design variables of the fitted initial airfoil.
    pub fn initial(&self) -> Vec<f64> {
        let p = &self.fit.parameters;
        p.upper.iter().chain(&p.lower).copied().collect()
    }

    /// CST parameters of a design.
    pub fn parameters(&self, x: &[f64]) -> Result<CstParameters> {
        if x.len() != self.dimension() {
            return Err(OptimError::InvalidProblem(format!(
                "{} design variables expected, {} given",
                self.dimension(),
                x.len()
            )));
        }
        if x.iter().any(|v| !v.is_finite()) {
            return Err(OptimError::InvalidProblem(
                "design variables must be finite".into(),
            ));
        }
        let half = self.fit.parameters.upper.len();
        Ok(CstParameters {
            upper: x[..half].to_vec(),
            lower: x[half..].to_vec(),
            ..self.fit.parameters.clone()
        })
    }

    /// Airfoil coordinates of a design (Selig order, not paneled).
    pub fn airfoil(&self, x: &[f64], name: &str) -> Result<Airfoil> {
        Ok(self.parameters(x)?.to_airfoil(name, self.points_per_side)?)
    }
}

/// Thickness `t/c` at the chord station `x/c`: distance between the two
/// surfaces at that station, in chord-line axes (XFOIL's definition).
pub fn thickness_at(p: &CstParameters, x: f64) -> f64 {
    p.upper_y(x) - p.lower_y(x)
}

/// Smallest thickness over the interior of the chord, at cosine-spaced
/// stations; negative when the surfaces cross.
pub(crate) fn min_interior_thickness(p: &CstParameters) -> f64 {
    (1..VALIDITY_STATIONS)
        .map(|k| {
            let t = std::f64::consts::PI * k as f64 / VALIDITY_STATIONS as f64;
            thickness_at(p, 0.5 * (1.0 - t.cos()))
        })
        .fold(f64::INFINITY, f64::min)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aero_geometry::{NacaOptions, naca};

    fn space(code: &str) -> DesignSpace {
        let af = naca(code, &NacaOptions::default()).unwrap();
        DesignSpace::new(&af, &DesignOptions::default()).unwrap()
    }

    #[test]
    fn initial_design_is_the_fit() {
        let s = space("2412");
        assert_eq!(s.dimension(), 14);
        let x = s.initial();
        assert_eq!(s.parameters(&x).unwrap(), s.fit().parameters);
        assert!(s.fit().max_error < 1e-3, "{}", s.fit().max_error);
        let af = s.airfoil(&x, "cst").unwrap();
        assert_eq!(af.len(), 201);
        // Same shape as the NACA section, to the fit accuracy.
        let props = af.properties();
        assert!((props.max_thickness - 0.12).abs() < 1e-3);
        assert!((props.max_camber - 0.02).abs() < 1e-3);
    }

    #[test]
    fn analytic_thickness_matches_the_geometric_properties() {
        let s = space("4415");
        let p = s.parameters(&s.initial()).unwrap();
        let props = s.airfoil(&s.initial(), "cst").unwrap().properties();
        // The properties locate the leading edge on a spline through the
        // generated points, hence a slightly different chord line.
        let t = thickness_at(&p, props.max_thickness_x);
        assert!((t - props.max_thickness).abs() < 1e-5, "{t} {props:?}");
        assert!(min_interior_thickness(&p) > 0.0);
        // Crossed surfaces.
        let mut x = s.initial();
        x[..7].fill(-0.3);
        assert!(min_interior_thickness(&s.parameters(&x).unwrap()) < 0.0);
    }

    #[test]
    fn invalid_designs_and_options() {
        let s = space("0012");
        assert!(s.parameters(&[0.1; 3]).is_err());
        let mut x = s.initial();
        x[0] = f64::NAN;
        assert!(s.parameters(&x).is_err());
        let af = naca("0012", &NacaOptions::default()).unwrap();
        for options in [
            DesignOptions {
                order: 1,
                ..DesignOptions::default()
            },
            DesignOptions {
                points_per_side: 10,
                ..DesignOptions::default()
            },
        ] {
            assert!(DesignSpace::new(&af, &options).is_err());
        }
    }
}
