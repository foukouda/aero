//! Which airfoil for a flight: each candidate is analysed at the flight's
//! Reynolds and Mach numbers, then the candidates are ranked by what the
//! user cares about.
//!
//! For one airfoil ([`evaluate`]): a viscous polar swept from 0° upward
//! until the stall, and downward only as far as the required lift
//! coefficient needs, each point starting from the previous boundary
//! layer; the maximum lift coefficient; the angle, drag and moment at the
//! required lift coefficient, interpolated between the two converged points
//! around it; the wing estimate of [`crate::wing`]; and whether the shape is
//! easy to build. [`rank`] then orders the candidates.

use aero_foil2d::viscous::{ViscousOptions, ViscousSolver};
use aero_geometry::{Airfoil, PanelingOptions};
use serde::{Deserialize, Serialize};

use crate::error::{MissionError, Result};
use crate::flight::Conditions;
use crate::wing::{Performance, SectionData, WingModel, performance};

/// How the candidates are analysed.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnalysisSettings {
    /// Critical amplification exponent of the transition model: 9 for a
    /// clean wind tunnel or calm air, lower for turbulent air.
    pub ncrit: f64,
    /// Angle step of the polar, degrees.
    pub alpha_step: f64,
    /// Highest angle swept, degrees.
    pub alpha_max: f64,
    /// Lowest angle swept, degrees.
    pub alpha_min: f64,
    /// Paneling of every candidate.
    pub paneling: PanelingOptions,
}

impl Default for AnalysisSettings {
    fn default() -> Self {
        Self {
            ncrit: 9.0,
            alpha_step: 0.5,
            alpha_max: 20.0,
            alpha_min: -8.0,
            paneling: PanelingOptions::default(),
        }
    }
}

/// A converged point of a candidate's polar.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SectionPoint {
    /// Angle of attack, degrees.
    pub alpha_deg: f64,
    /// Lift coefficient.
    pub cl: f64,
    /// Drag coefficient.
    pub cd: f64,
    /// Pitching moment coefficient about the quarter chord.
    pub cm: f64,
}

/// How the flight suits an airfoil.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fit {
    /// The wing flies at most at 70 % of its maximum lift.
    Comfortable,
    /// Between 70 % and 85 % of the maximum lift: little margin for gusts
    /// and turns.
    Tight,
    /// Above 85 % of the maximum lift: too close to the stall.
    NearStall,
    /// The wing cannot lift the aircraft at this speed.
    CannotFly,
    /// The polar does not reach the required lift coefficient without
    /// stalling or converging (e.g. a very small lift coefficient below the
    /// swept angles).
    NoData,
}

impl Fit {
    /// The flight can be recommended on this airfoil.
    pub fn suited(&self) -> bool {
        matches!(self, Self::Comfortable | Self::Tight)
    }

    fn from_fraction(fraction: f64) -> Self {
        if fraction <= 0.70 {
            Self::Comfortable
        } else if fraction <= 0.85 {
            Self::Tight
        } else if fraction <= 1.0 {
            Self::NearStall
        } else {
            Self::CannotFly
        }
    }
}

/// Features of the shape that matter to a builder.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Buildability {
    /// Maximum thickness `t/c`.
    pub thickness: f64,
    /// Highest point of the lower surface between 30 % and 95 % of the
    /// chord, above the chord line (`y/c`): positive for an undercambered
    /// (hollow) lower surface.
    pub undercamber: f64,
    /// Thick enough (at least 8 %) and without a hollow lower surface:
    /// easy to build with ribs on a flat board, or from foam.
    pub easy: bool,
}

/// A candidate analysed for a flight.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AirfoilEvaluation {
    /// Converged points of the polar, by increasing angle.
    pub polar: Vec<SectionPoint>,
    /// Maximum lift coefficient of the polar, and its angle.
    pub cl_max: Option<SectionPoint>,
    /// The airfoil at the required lift coefficient (interpolated).
    pub operating_point: Option<SectionPoint>,
    /// Wing estimate at the operating point.
    pub performance: Option<Performance>,
    /// How the flight suits the airfoil.
    pub fit: Fit,
    /// Shape features for the builder.
    pub buildability: Buildability,
}

/// What the user wants most.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Goal {
    /// The least drag at the flight's speed: the longest glide, the longest
    /// flight on a battery (for a given flight both come down to the
    /// lowest drag at the required lift coefficient).
    #[default]
    LeastDrag,
    /// The largest maximum lift: the slowest stall, the largest load.
    SlowFlight,
    /// The least drag among the shapes that are easy to build.
    EasyToBuild,
}

/// Analyses `airfoil` for the flight `conditions`.
pub fn evaluate(
    airfoil: &Airfoil,
    conditions: &Conditions,
    settings: &AnalysisSettings,
    model: &WingModel,
) -> Result<AirfoilEvaluation> {
    if !(settings.alpha_step > 0.0
        && settings.alpha_min < 0.0
        && settings.alpha_max > 0.0
        && settings.alpha_max <= 30.0
        && settings.alpha_min >= -30.0)
    {
        return Err(MissionError::InvalidInput(
            "the polar must sweep from a negative to a positive angle (at most 30°)".to_string(),
        ));
    }
    let paneled = airfoil.paneled(&settings.paneling)?;
    let options = ViscousOptions {
        reynolds: conditions.reynolds,
        mach: conditions.mach,
        ncrit: [settings.ncrit; 2],
        ..ViscousOptions::default()
    };
    let solver = ViscousSolver::new(&paneled, &options)?;
    let required = conditions.required_cl;

    // Upward from 0° to the stall: stop after four unconverged points in a
    // row (as XFOIL's sequences do) or once the lift has fallen 0.1 below
    // its maximum on two converged points.
    let mut up = Vec::new();
    let mut session = solver.session();
    let mut failures = 0;
    let mut past_maximum = 0;
    for alpha in angles(0.0, settings.alpha_max, settings.alpha_step) {
        match session.solve(alpha) {
            Ok(s) if s.converged => {
                failures = 0;
                let best = up
                    .iter()
                    .map(|p: &SectionPoint| p.cl)
                    .fold(f64::NEG_INFINITY, f64::max);
                up.push(SectionPoint {
                    alpha_deg: alpha,
                    cl: s.cl,
                    cd: s.cd,
                    cm: s.cm,
                });
                if s.cl < best - 0.1 {
                    past_maximum += 1;
                    if past_maximum == 2 {
                        break;
                    }
                }
            }
            _ => {
                failures += 1;
                if failures == 4 {
                    break;
                }
            }
        }
    }

    // Downward only while the required lift is below the polar.
    let mut down = Vec::new();
    if up.first().is_none_or(|p| p.cl > required) {
        let mut session = solver.session();
        let mut failures = 0;
        for alpha in angles(0.0, settings.alpha_min, -settings.alpha_step).skip(1) {
            match session.solve(alpha) {
                Ok(s) if s.converged => {
                    failures = 0;
                    down.push(SectionPoint {
                        alpha_deg: alpha,
                        cl: s.cl,
                        cd: s.cd,
                        cm: s.cm,
                    });
                    if s.cl < required {
                        break;
                    }
                }
                _ => {
                    failures += 1;
                    if failures == 4 {
                        break;
                    }
                }
            }
        }
    }
    down.reverse();
    let polar: Vec<SectionPoint> = down.into_iter().chain(up).collect();

    let cl_max = polar.iter().copied().max_by(|a, b| a.cl.total_cmp(&b.cl));
    let operating_point = cl_max.and_then(|top| at_cl(&polar, top.alpha_deg, required));
    let buildability = buildability(airfoil);
    let (performance, fit) = match (cl_max, operating_point) {
        (Some(top), Some(op)) => {
            let p = performance(
                conditions,
                &SectionData {
                    cd: op.cd,
                    cl_max: top.cl,
                },
                model,
            )?;
            (Some(p), Fit::from_fraction(p.cl_fraction))
        }
        (Some(top), None) if required > model.cl_max_factor * top.cl => (None, Fit::CannotFly),
        _ => (None, Fit::NoData),
    };
    Ok(AirfoilEvaluation {
        polar,
        cl_max,
        operating_point,
        performance,
        fit,
        buildability,
    })
}

/// `from`, `from + step`, … up to `to` (included when on the grid).
fn angles(from: f64, to: f64, step: f64) -> impl Iterator<Item = f64> {
    let n = ((to - from) / step + 1e-9).floor().max(0.0) as usize;
    (0..=n).map(move |i| from + step * i as f64)
}

/// The point of `polar` at lift coefficient `cl`, interpolated linearly
/// between the two converged points around it, below the angle of maximum
/// lift (the attached-flow branch).
fn at_cl(polar: &[SectionPoint], alpha_cl_max: f64, cl: f64) -> Option<SectionPoint> {
    let attached: Vec<&SectionPoint> = polar
        .iter()
        .filter(|p| p.alpha_deg <= alpha_cl_max)
        .collect();
    attached.windows(2).find_map(|w| {
        let (a, b) = (w[0], w[1]);
        if (a.cl - cl) * (b.cl - cl) > 0.0 || a.cl == b.cl {
            return None;
        }
        let t = (cl - a.cl) / (b.cl - a.cl);
        let lerp = |u: f64, v: f64| u + t * (v - u);
        Some(SectionPoint {
            alpha_deg: lerp(a.alpha_deg, b.alpha_deg),
            cl,
            cd: lerp(a.cd, b.cd),
            cm: lerp(a.cm, b.cm),
        })
    })
}

/// Thickness and hollowness of the lower surface, in the chord frame.
fn buildability(airfoil: &Airfoil) -> Buildability {
    let props = airfoil.properties();
    let [lx, ly] = props.leading_edge;
    let [tx, ty] = props.trailing_edge;
    let chord = props.chord;
    let (ux, uy) = ((tx - lx) / chord, (ty - ly) / chord);
    let frame = |[x, y]: [f64; 2]| {
        let (dx, dy) = (x - lx, y - ly);
        ((dx * ux + dy * uy) / chord, (dy * ux - dx * uy) / chord)
    };
    // Points are ordered from the upper trailing edge round the leading
    // edge (the leftmost point) to the lower trailing edge.
    let points: Vec<[f64; 2]> = airfoil.points().collect();
    let le = points
        .iter()
        .enumerate()
        .min_by(|a, b| a.1[0].total_cmp(&b.1[0]))
        .map_or(0, |(i, _)| i);
    let undercamber = points[le..]
        .iter()
        .map(|&p| frame(p))
        .filter(|(x, _)| (0.30..=0.95).contains(x))
        .map(|(_, y)| y)
        .fold(f64::NEG_INFINITY, f64::max);
    let undercamber = if undercamber.is_finite() {
        undercamber
    } else {
        0.0
    };
    let thickness = props.max_thickness;
    Buildability {
        thickness,
        undercamber,
        easy: thickness >= 0.08 && undercamber <= 0.005,
    }
}

/// Orders `evaluations` for `goal`, best first: the suited candidates by
/// the goal's criterion, then the others (closest to suited first). Returns
/// indices into `evaluations`.
pub fn rank(evaluations: &[AirfoilEvaluation], goal: Goal) -> Vec<usize> {
    let drag = |e: &AirfoilEvaluation| e.operating_point.map_or(f64::INFINITY, |p| p.cd);
    let cl_max = |e: &AirfoilEvaluation| e.cl_max.map_or(f64::NEG_INFINITY, |p| p.cl);
    let tier = |e: &AirfoilEvaluation| {
        let suited = match goal {
            Goal::EasyToBuild => e.fit.suited() && e.buildability.easy,
            Goal::LeastDrag | Goal::SlowFlight => e.fit.suited(),
        };
        if suited {
            0
        } else {
            match e.fit {
                Fit::Comfortable | Fit::Tight => 1,
                Fit::NearStall => 2,
                Fit::CannotFly => 3,
                Fit::NoData => 4,
            }
        }
    };
    let mut order: Vec<usize> = (0..evaluations.len()).collect();
    order.sort_by(|&i, &j| {
        let (a, b) = (&evaluations[i], &evaluations[j]);
        tier(a).cmp(&tier(b)).then_with(|| match goal {
            Goal::SlowFlight => cl_max(b).total_cmp(&cl_max(a)),
            Goal::LeastDrag | Goal::EasyToBuild => drag(a).total_cmp(&drag(b)),
        })
    });
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(alpha_deg: f64, cl: f64, cd: f64) -> SectionPoint {
        SectionPoint {
            alpha_deg,
            cl,
            cd,
            cm: -0.05,
        }
    }

    #[test]
    fn angles_cover_the_range() {
        let up: Vec<f64> = angles(0.0, 2.0, 0.5).collect();
        assert_eq!(up, [0.0, 0.5, 1.0, 1.5, 2.0]);
        let down: Vec<f64> = angles(0.0, -1.0, -0.5).collect();
        assert_eq!(down, [0.0, -0.5, -1.0]);
    }

    #[test]
    fn interpolates_on_the_attached_branch() {
        // Lift rises to 1.2 at 10°, then falls: 1.0 is met twice, the
        // attached-flow crossing is the one kept.
        let polar = [
            point(0.0, 0.4, 0.010),
            point(5.0, 0.9, 0.012),
            point(7.5, 1.1, 0.016),
            point(10.0, 1.2, 0.030),
            point(12.0, 1.0, 0.080),
        ];
        let p = at_cl(&polar, 10.0, 1.0).unwrap();
        assert!((p.alpha_deg - 6.25).abs() < 1e-12);
        assert!((p.cd - 0.014).abs() < 1e-12);
        assert!(at_cl(&polar, 10.0, 1.3).is_none());
        assert!(at_cl(&polar, 10.0, 0.2).is_none());
        // A polar starting above the required lift: the stall drop crosses
        // it, but that separated point is not an operating point.
        let high_lift = [
            point(0.0, 0.6, 0.02),
            point(10.0, 1.2, 0.03),
            point(12.0, 1.0, 0.08),
            point(13.0, 0.5, 0.15),
        ];
        assert!(at_cl(&high_lift, 10.0, 0.55).is_none());
    }

    fn evaluation(fit: Fit, cd: f64, cl_max: f64, easy: bool) -> AirfoilEvaluation {
        AirfoilEvaluation {
            polar: Vec::new(),
            cl_max: Some(point(12.0, cl_max, 0.03)),
            operating_point: Some(point(4.0, 0.6, cd)),
            performance: None,
            fit,
            buildability: Buildability {
                thickness: 0.1,
                undercamber: 0.0,
                easy,
            },
        }
    }

    #[test]
    fn ranking_by_goal() {
        let e = [
            evaluation(Fit::Comfortable, 0.012, 1.2, true),  // 0
            evaluation(Fit::Tight, 0.010, 1.3, false),       // 1
            evaluation(Fit::NearStall, 0.008, 1.0, true),    // 2: too close to stall
            evaluation(Fit::Comfortable, 0.015, 1.8, false), // 3: high lift
            evaluation(Fit::NoData, 0.0, 0.9, true),         // 4
        ];
        assert_eq!(rank(&e, Goal::LeastDrag), [1, 0, 3, 2, 4]);
        assert_eq!(rank(&e, Goal::SlowFlight), [3, 1, 0, 2, 4]);
        // Suited and easy first, then the suited but hard to build.
        assert_eq!(rank(&e, Goal::EasyToBuild), [0, 1, 3, 2, 4]);
    }

    #[test]
    fn fit_thresholds() {
        assert_eq!(Fit::from_fraction(0.5), Fit::Comfortable);
        assert_eq!(Fit::from_fraction(0.8), Fit::Tight);
        assert_eq!(Fit::from_fraction(0.9), Fit::NearStall);
        assert_eq!(Fit::from_fraction(1.1), Fit::CannotFly);
        assert!(Fit::Tight.suited() && !Fit::NearStall.suited());
    }
}
