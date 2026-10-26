//! Definition of an optimization problem: operating points, objective,
//! constraints and analysis options.

use aero_foil2d::viscous::ViscousOptions;
use aero_geometry::PanelingOptions;
use serde::{Deserialize, Serialize};

use crate::design::DesignOptions;
use crate::error::{OptimError, Result};

/// What is prescribed at an operating point.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Target {
    /// Angle of attack, degrees (XFOIL `ALFA`).
    Alpha(f64),
    /// Lift coefficient; the angle of attack is solved for (XFOIL `CL`).
    Cl(f64),
}

impl Target {
    /// The prescribed value.
    pub fn value(&self) -> f64 {
        match *self {
            Self::Alpha(v) | Self::Cl(v) => v,
        }
    }
}

/// An operating point of the design.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DesignPoint {
    /// Prescribed angle of attack or lift coefficient.
    pub target: Target,
    /// Weight of the point in the objective (positive).
    pub weight: f64,
}

/// Quantity minimized, as a weighted mean over the operating points.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Objective {
    /// Minimum drag: weighted mean of `Cd`.
    #[default]
    MinDrag,
    /// Maximum lift-to-drag ratio: weighted mean of `−Cl/Cd`.
    MaxLiftToDrag,
}

/// Minimum thickness at a chord station.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ThicknessStation {
    /// Station `x/c`, strictly between 0 and 1.
    pub x: f64,
    /// Minimum thickness `t/c` there.
    pub min: f64,
}

/// Constraints of the design; `None` or empty disables one.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Constraints {
    /// Minimum of the maximum thickness `t/c`.
    pub min_thickness: Option<f64>,
    /// Maximum of the maximum thickness `t/c`.
    pub max_thickness: Option<f64>,
    /// Minimum thickness at given chord stations.
    pub stations: Vec<ThicknessStation>,
    /// Minimum moment coefficient `Cm` (about the quarter chord) at every
    /// operating point.
    pub min_cm: Option<f64>,
    /// Minimum trailing-edge included angle, degrees.
    pub min_te_angle_deg: Option<f64>,
}

/// An airfoil optimization problem.
///
/// Every operating point is analyzed by the viscous solver from a fresh
/// boundary layer; a point that does not converge makes the design
/// infeasible.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Problem {
    /// Operating points (at least one).
    pub points: Vec<DesignPoint>,
    /// Quantity minimized.
    #[serde(default)]
    pub objective: Objective,
    /// Constraints.
    #[serde(default)]
    pub constraints: Constraints,
    /// Viscous analysis options, shared by the operating points.
    #[serde(default)]
    pub viscous: ViscousOptions,
    /// Paneling of the generated airfoils.
    #[serde(default)]
    pub paneling: PanelingOptions,
    /// Design space.
    #[serde(default)]
    pub design: DesignOptions,
}

impl Problem {
    /// Checks the problem definition.
    pub fn validate(&self) -> Result<()> {
        let invalid = |m: &str| Err(OptimError::InvalidProblem(m.into()));
        if self.points.is_empty() {
            return invalid("at least one operating point is required");
        }
        for p in &self.points {
            if !p.target.value().is_finite() {
                return invalid("operating point values must be finite");
            }
            if !(p.weight.is_finite() && p.weight > 0.0) {
                return invalid("operating point weights must be positive");
            }
        }
        let c = &self.constraints;
        let positive = |v: Option<f64>| v.is_none_or(|v| v.is_finite() && v > 0.0);
        if !positive(c.min_thickness) || !positive(c.max_thickness) {
            return invalid("thickness bounds must be positive");
        }
        if let (Some(lo), Some(hi)) = (c.min_thickness, c.max_thickness)
            && lo > hi
        {
            return invalid("the minimum thickness exceeds the maximum thickness");
        }
        for s in &c.stations {
            if !(s.x > 0.0 && s.x < 1.0 && s.min.is_finite() && s.min > 0.0) {
                return invalid("thickness stations need 0 < x < 1 and a positive thickness");
            }
        }
        if c.min_cm.is_some_and(|v| !v.is_finite()) {
            return invalid("the minimum moment coefficient must be finite");
        }
        if c.min_te_angle_deg
            .is_some_and(|v| !(v.is_finite() && v > 0.0 && v < 180.0))
        {
            return invalid("the minimum trailing-edge angle must be between 0 and 180 degrees");
        }
        self.viscous
            .validate()
            .map_err(|e| OptimError::InvalidProblem(e.to_string()))?;
        self.design.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn sample() -> Problem {
        Problem {
            points: vec![DesignPoint {
                target: Target::Cl(0.5),
                weight: 1.0,
            }],
            objective: Objective::MinDrag,
            constraints: Constraints {
                min_thickness: Some(0.12),
                ..Constraints::default()
            },
            viscous: ViscousOptions::default(),
            paneling: PanelingOptions::default(),
            design: DesignOptions::default(),
        }
    }

    #[test]
    fn validation() {
        assert!(sample().validate().is_ok());
        let mut cases = Vec::new();
        let mut p = sample();
        p.points.clear();
        cases.push(p);
        let mut p = sample();
        p.points[0].weight = 0.0;
        cases.push(p);
        let mut p = sample();
        p.points[0].target = Target::Alpha(f64::NAN);
        cases.push(p);
        let mut p = sample();
        p.constraints.max_thickness = Some(0.1);
        cases.push(p);
        let mut p = sample();
        p.constraints
            .stations
            .push(ThicknessStation { x: 1.0, min: 0.01 });
        cases.push(p);
        let mut p = sample();
        p.constraints.min_te_angle_deg = Some(-1.0);
        cases.push(p);
        let mut p = sample();
        p.viscous.reynolds = -1.0;
        cases.push(p);
        let mut p = sample();
        p.design.order = 50;
        cases.push(p);
        for (k, p) in cases.iter().enumerate() {
            assert!(p.validate().is_err(), "case {k}");
        }
    }

    #[test]
    fn serde_round_trip_and_defaults() {
        let p = sample();
        let json = serde_json::to_string(&p).unwrap();
        assert!(
            json.contains(r#""target":{"kind":"cl","value":0.5}"#),
            "{json}"
        );
        assert_eq!(serde_json::from_str::<Problem>(&json).unwrap(), p);
        let minimal: Problem = serde_json::from_str(
            r#"{"points":[{"target":{"kind":"alpha","value":2.0},"weight":1.0}]}"#,
        )
        .unwrap();
        assert_eq!(minimal.objective, Objective::MinDrag);
        assert_eq!(minimal.constraints, Constraints::default());
    }
}
