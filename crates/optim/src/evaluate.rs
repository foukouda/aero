//! Evaluation of a design: geometry, viscous analysis of the operating
//! points, objective and constraint violations.

use std::cmp::Ordering;

use aero_foil2d::inviscid::{InviscidOptions, InviscidSolver};
use aero_foil2d::viscous::{ViscousSession, ViscousSolution, ViscousSolver};
use aero_geometry::{Airfoil, panel};
use serde::{Deserialize, Serialize};

use crate::design::{DesignSpace, min_interior_thickness, thickness_at};
use crate::error::Result;
use crate::problem::{Objective, Problem, Target};
use crate::rank::{Ranked, feasibility_cmp};

/// Violation of a design that cannot be analyzed (crossed surfaces,
/// paneling failure), plus the amount of crossing.
pub const INVALID_DESIGN: f64 = 1000.0;

/// Violation added by each operating point that does not converge.
pub const FAILED_POINT: f64 = 10.0;

/// Moment coefficient violations are divided by this value.
pub const CM_SCALE: f64 = 0.1;

/// Steps of the approach to a target that failed from a fresh boundary
/// layer.
const RAMP_STEPS: usize = 4;

/// Geometric properties of a design (chord units).
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeometrySummary {
    /// Maximum thickness `t/c`.
    pub max_thickness: f64,
    /// Its station `x/c`.
    pub max_thickness_x: f64,
    /// Maximum camber `f/c`.
    pub max_camber: f64,
    /// Its station `x/c`.
    pub max_camber_x: f64,
    /// Trailing-edge included angle, degrees.
    pub te_angle_deg: f64,
    /// Cross-section area.
    pub area: f64,
    /// Thickness at the constrained stations, in the order of
    /// [`crate::Constraints::stations`].
    pub station_thickness: Vec<f64>,
}

/// Aerodynamic coefficients of an operating point.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PointValues {
    /// Angle of attack, degrees.
    pub alpha_deg: f64,
    /// Lift coefficient.
    pub cl: f64,
    /// Drag coefficient.
    pub cd: f64,
    /// Pressure drag coefficient.
    pub cdp: f64,
    /// Skin-friction drag coefficient.
    pub cdf: f64,
    /// Moment coefficient about the quarter chord.
    pub cm: f64,
    /// Transition `x/c` on the top and bottom surfaces.
    pub transition: [f64; 2],
}

impl From<&ViscousSolution> for PointValues {
    fn from(s: &ViscousSolution) -> Self {
        Self {
            alpha_deg: s.alpha_deg,
            cl: s.cl,
            cd: s.cd,
            cdp: s.cdp,
            cdf: s.cdf,
            cm: s.cm,
            transition: [s.transition[0].x, s.transition[1].x],
        }
    }
}

/// Analysis of one operating point.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointResult {
    /// The prescribed value.
    pub target: Target,
    /// True if the viscous solution converged.
    pub converged: bool,
    /// Coefficients (of the last Newton iterate if not converged), absent
    /// after an analysis error or if a coefficient is not finite.
    pub values: Option<PointValues>,
    /// Analysis error, e.g. critical Mach number exceeded.
    pub error: Option<String>,
}

/// Evaluation of a design.
///
/// Designs are ranked with the feasibility rules of [`Ranked`]: the
/// violation is the sum of the relative constraint violations (thickness
/// bounds divided by the bound, `Cm` by [`CM_SCALE`]), plus
/// [`FAILED_POINT`] per unconverged operating point, or
/// [`INVALID_DESIGN`] when the design cannot be analyzed.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evaluation {
    /// Design variables.
    pub x: Vec<f64>,
    /// Objective, when every operating point converged.
    pub objective: Option<f64>,
    /// Constraint violation, 0 for a feasible design.
    pub violation: f64,
    /// Geometric properties, unless the design is invalid.
    pub geometry: Option<GeometrySummary>,
    /// Operating points, unless the design is invalid.
    pub points: Vec<PointResult>,
    /// Why the design could not be analyzed.
    pub failure: Option<String>,
}

impl Evaluation {
    /// True if the design satisfies every constraint.
    pub fn feasible(&self) -> bool {
        self.violation <= 0.0 && self.objective.is_some()
    }

    fn invalid(x: &[f64], amount: f64, message: String) -> Self {
        Self {
            x: x.to_vec(),
            objective: None,
            violation: INVALID_DESIGN + amount,
            geometry: None,
            points: Vec::new(),
            failure: Some(message),
        }
    }
}

impl Ranked for Evaluation {
    fn rank_cmp(&self, other: &Self) -> Ordering {
        feasibility_cmp(
            self.violation,
            self.objective,
            other.violation,
            other.objective,
        )
    }
}

/// Evaluates designs of a problem.
#[derive(Debug, Clone)]
pub struct Evaluator {
    problem: Problem,
    space: DesignSpace,
}

impl Evaluator {
    /// Checks the problem and fits the initial airfoil.
    pub fn new(airfoil: &Airfoil, problem: &Problem) -> Result<Self> {
        problem.validate()?;
        Ok(Self {
            problem: problem.clone(),
            space: DesignSpace::new(airfoil, &problem.design)?,
        })
    }

    /// The problem.
    pub fn problem(&self) -> &Problem {
        &self.problem
    }

    /// The design space.
    pub fn space(&self) -> &DesignSpace {
        &self.space
    }

    /// Evaluates a design. Analysis failures are reported in the
    /// evaluation, never as errors.
    pub fn evaluate(&self, x: &[f64]) -> Evaluation {
        let params = match self.space.parameters(x) {
            Ok(p) => p,
            Err(e) => return Evaluation::invalid(x, 0.0, e.to_string()),
        };
        let overlap = -min_interior_thickness(&params);
        if overlap >= 0.0 {
            return Evaluation::invalid(x, overlap, "the surfaces cross".into());
        }
        let airfoil = match self.space.airfoil(x, "design") {
            Ok(a) => a,
            Err(e) => return Evaluation::invalid(x, 0.0, e.to_string()),
        };

        let c = &self.problem.constraints;
        let props = airfoil.properties();
        let measures = [
            props.max_thickness,
            props.max_thickness_x,
            props.max_camber,
            props.max_camber_x,
            props.te_angle_deg,
            props.area,
        ];
        if measures.iter().any(|v| !v.is_finite()) {
            return Evaluation::invalid(x, 0.0, "degenerate geometry".into());
        }
        let station_thickness: Vec<f64> = c
            .stations
            .iter()
            .map(|s| thickness_at(&params, s.x))
            .collect();
        let mut violation = 0.0;
        let below = |value: f64, bound: f64| ((bound - value) / bound).max(0.0);
        if let Some(b) = c.min_thickness {
            violation += below(props.max_thickness, b);
        }
        if let Some(b) = c.max_thickness {
            violation += ((props.max_thickness - b) / b).max(0.0);
        }
        for (s, t) in c.stations.iter().zip(&station_thickness) {
            violation += below(*t, s.min);
        }
        if let Some(b) = c.min_te_angle_deg {
            violation += below(props.te_angle_deg, b);
        }
        let geometry = GeometrySummary {
            max_thickness: props.max_thickness,
            max_thickness_x: props.max_thickness_x,
            max_camber: props.max_camber,
            max_camber_x: props.max_camber_x,
            te_angle_deg: props.te_angle_deg,
            area: props.area,
            station_thickness,
        };

        let solver = panel(&airfoil, &self.problem.paneling)
            .map_err(|e| e.to_string())
            .and_then(|p| {
                InviscidSolver::new(&p, &InviscidOptions::default()).map_err(|e| e.to_string())
            })
            .and_then(|i| {
                ViscousSolver::from_inviscid(i, &self.problem.viscous).map_err(|e| e.to_string())
            });
        let solver = match solver {
            Ok(s) => s,
            Err(e) => {
                let mut ev = Evaluation::invalid(x, 0.0, e);
                ev.geometry = Some(geometry);
                return ev;
            }
        };

        let mut points = Vec::with_capacity(self.problem.points.len());
        let (mut sum, mut weights) = (0.0, 0.0);
        let mut all_converged = true;
        for p in &self.problem.points {
            let r = solve_point(&solver, p.target);
            match r.values.filter(|_| r.converged) {
                Some(v) => {
                    if let Some(b) = c.min_cm {
                        violation += ((b - v.cm) / CM_SCALE).max(0.0);
                    }
                    let f = match self.problem.objective {
                        Objective::MinDrag => v.cd,
                        Objective::MaxLiftToDrag => -v.cl / v.cd,
                    };
                    sum += p.weight * f;
                    weights += p.weight;
                }
                None => {
                    violation += FAILED_POINT;
                    all_converged = false;
                }
            }
            points.push(r);
        }

        Evaluation {
            x: x.to_vec(),
            objective: all_converged.then(|| sum / weights),
            violation,
            geometry: Some(geometry),
            points,
            failure: None,
        }
    }
}

fn usable(s: &ViscousSolution) -> bool {
    s.converged && s.cd.is_finite() && s.cd > 0.0
}

fn solve(
    session: &mut ViscousSession<'_>,
    target: Target,
    value: f64,
) -> aero_foil2d::viscous::Result<ViscousSolution> {
    match target {
        Target::Alpha(_) => session.solve(value),
        Target::Cl(_) => session.solve_cl(value),
    }
}

/// Solves an operating point from a fresh boundary layer. If that fails,
/// approaches the target from zero in [`RAMP_STEPS`] steps, each starting
/// from the previous boundary layer (restarted after a failed step), as
/// one does with XFOIL.
fn solve_point(solver: &ViscousSolver, target: Target) -> PointResult {
    let value = target.value();
    let first = solve(&mut solver.session(), target, value);
    let retry = match &first {
        Ok(s) if usable(s) => None,
        _ => {
            let mut session = solver.session();
            let mut last = None;
            for k in 1..=RAMP_STEPS {
                let r = solve(&mut session, target, value * k as f64 / RAMP_STEPS as f64);
                if !matches!(&r, Ok(s) if usable(s)) {
                    session.reset();
                }
                last = Some(r);
            }
            last.filter(|r| matches!(r, Ok(s) if usable(s)))
        }
    };
    match retry.unwrap_or(first) {
        Ok(s) => {
            let v = PointValues::from(&s);
            let finite = [v.alpha_deg, v.cl, v.cd, v.cdp, v.cdf, v.cm]
                .iter()
                .chain(&v.transition)
                .all(|c| c.is_finite());
            PointResult {
                target,
                converged: usable(&s) && finite,
                values: finite.then_some(v),
                error: None,
            }
        }
        Err(e) => PointResult {
            target,
            converged: false,
            values: None,
            error: Some(e.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::problem::{Constraints, DesignPoint, ThicknessStation};
    use aero_foil2d::viscous::ViscousOptions;
    use aero_geometry::{NacaOptions, PanelingOptions, naca};

    fn problem(points: &[Target], constraints: Constraints) -> Problem {
        Problem {
            points: points
                .iter()
                .map(|&target| DesignPoint {
                    target,
                    weight: 1.0,
                })
                .collect(),
            objective: Objective::MinDrag,
            constraints,
            viscous: ViscousOptions::default(),
            paneling: PanelingOptions::default(),
            design: Default::default(),
        }
    }

    fn evaluator(p: &Problem) -> Evaluator {
        Evaluator::new(&naca("2412", &NacaOptions::default()).unwrap(), p).unwrap()
    }

    #[test]
    fn objective_is_an_independent_analysis_of_the_design() {
        let p = problem(
            &[Target::Cl(0.4), Target::Alpha(4.0)],
            Constraints::default(),
        );
        let ev = evaluator(&p);
        let x = ev.space().initial();
        let e = ev.evaluate(&x);
        assert!(e.feasible() && e.failure.is_none(), "{e:?}");
        // The same analysis done by hand.
        let airfoil = ev.space().airfoil(&x, "check").unwrap();
        let paneled = panel(&airfoil, &PanelingOptions::default()).unwrap();
        let solver = ViscousSolver::new(&paneled, &ViscousOptions::default()).unwrap();
        let a = solver.solve_cl(0.4).unwrap();
        let b = solver.solve(4.0).unwrap();
        assert!(a.converged && b.converged);
        assert_eq!(e.objective, Some(0.5 * (a.cd + b.cd)));
        let v = e.points[0].values.unwrap();
        assert_eq!((v.cl, v.cd, v.cm), (a.cl, a.cd, a.cm));
        assert!((v.cl - 0.4).abs() < 1e-6);

        let mut p2 = p.clone();
        p2.objective = Objective::MaxLiftToDrag;
        let e2 = evaluator(&p2).evaluate(&x);
        assert_eq!(e2.objective, Some(-0.5 * (a.cl / a.cd + b.cl / b.cd)));
    }

    #[test]
    fn constraint_violations() {
        let base = problem(&[Target::Cl(0.4)], Constraints::default());
        let x = evaluator(&base).space().initial();
        let free = evaluator(&base).evaluate(&x);
        let g = free.geometry.clone().unwrap();
        let cm = free.points[0].values.unwrap().cm;

        let c = Constraints {
            min_thickness: Some(0.15),
            max_thickness: Some(0.2),
            stations: vec![ThicknessStation { x: 0.9, min: 0.05 }],
            min_cm: Some(cm + 0.01),
            min_te_angle_deg: Some(g.te_angle_deg + 1.0),
        };
        let e = evaluator(&problem(&[Target::Cl(0.4)], c)).evaluate(&x);
        let t90 = e.geometry.as_ref().unwrap().station_thickness[0];
        let expected = (0.15 - g.max_thickness) / 0.15
            + (0.05 - t90) / 0.05
            + 0.01 / CM_SCALE
            + 1.0 / (g.te_angle_deg + 1.0);
        assert!(
            (e.violation - expected).abs() < 1e-12,
            "{} {expected}",
            e.violation
        );
        assert!(!e.feasible() && e.objective == free.objective);
        // Same design, satisfied constraints.
        let ok = Constraints {
            min_thickness: Some(0.11),
            max_thickness: Some(0.13),
            stations: vec![ThicknessStation { x: 0.5, min: 0.05 }],
            min_cm: Some(cm - 0.01),
            min_te_angle_deg: Some(1.0),
        };
        let e = evaluator(&problem(&[Target::Cl(0.4)], ok)).evaluate(&x);
        assert!(e.feasible(), "{}", e.violation);
        assert!(e.rank_cmp(&free).is_eq());
    }

    #[test]
    fn failures_are_infeasible_not_errors() {
        // Unreachable lift: the point fails.
        let p = problem(&[Target::Cl(0.4), Target::Cl(3.5)], Constraints::default());
        let ev = evaluator(&p);
        let x = ev.space().initial();
        let e = ev.evaluate(&x);
        assert!(e.points[0].converged && !e.points[1].converged);
        assert_eq!(e.objective, None);
        assert_eq!(e.violation, FAILED_POINT);
        // Crossed surfaces: not analyzed.
        let half = x.len() / 2;
        let mut bad = x.clone();
        bad[..half].fill(-0.3);
        let e = ev.evaluate(&bad);
        assert!(e.violation > INVALID_DESIGN && e.points.is_empty());
        assert_eq!(e.failure.as_deref(), Some("the surfaces cross"));
        // Wrong dimension.
        assert!(ev.evaluate(&x[1..]).violation >= INVALID_DESIGN);
        // Ranking: feasible, then by violation.
        let good = ev.evaluate(&x);
        let one_failure = Evaluation {
            violation: FAILED_POINT,
            ..good.clone()
        };
        let mut v = [e.clone(), one_failure.clone(), good.clone()];
        v.sort_by(Ranked::rank_cmp);
        assert_eq!(v[0].violation, good.violation);
        assert_eq!(v[1].violation, FAILED_POINT);
        assert!(v[2].violation > INVALID_DESIGN);
    }

    #[test]
    fn hard_points_are_approached_by_steps() {
        // NACA 4415 at Re = 1e6, CL = 0.8 is a limit cycle from a fresh
        // boundary layer (the bottom transition jumps between x/c = 0.99
        // and the trailing edge); approached from CL = 0, it converges.
        let af = naca("4415", &NacaOptions::default()).unwrap();
        let paneled = panel(&af, &PanelingOptions::default()).unwrap();
        let solver = ViscousSolver::new(&paneled, &ViscousOptions::default()).unwrap();
        assert!(!solver.solve_cl(0.8).unwrap().converged);
        let r = solve_point(&solver, Target::Cl(0.8));
        assert!(r.converged, "{r:?}");
        assert!((r.values.unwrap().cl - 0.8).abs() < 1e-6);
    }
}
