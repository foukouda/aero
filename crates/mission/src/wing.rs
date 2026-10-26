//! A simple wing: its size, and the performance estimated from the polar
//! of its airfoil with the classical finite-wing formulas.
//!
//! These are first estimates for a straight, untwisted wing: the airfoil
//! works at the wing's lift coefficient, the induced drag is that of an
//! elliptic loading reduced by a span efficiency, and the wing stalls a
//! little before its airfoil. A lifting-line or vortex-lattice analysis
//! replaces them when the wing shape matters.

use std::f64::consts::PI;

use serde::{Deserialize, Serialize};

use crate::error::{MissionError, Result, positive};
use crate::flight::Conditions;

/// Size of a wing.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Wing {
    /// Span, tip to tip, m.
    pub span: f64,
    /// Mean chord, m: the wing area is span × chord.
    pub chord: f64,
}

impl Wing {
    /// Checks that both dimensions are positive.
    pub fn validate(&self) -> Result<()> {
        positive("span", self.span)?;
        positive("chord", self.chord)?;
        Ok(())
    }

    /// Wing area, m².
    pub fn area(&self) -> f64 {
        self.span * self.chord
    }

    /// Aspect ratio, span² / area.
    pub fn aspect_ratio(&self) -> f64 {
        self.span / self.chord
    }
}

/// What is kept when a wing is sized for an area.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum WingConstraint {
    /// The span is given (m); the chord follows.
    Span(f64),
    /// The aspect ratio is given; span and chord follow.
    AspectRatio(f64),
}

impl WingConstraint {
    /// The wing of `area` (m²) that satisfies the constraint.
    pub fn wing(&self, area: f64) -> Result<Wing> {
        positive("wing area", area)?;
        let wing = match *self {
            Self::Span(span) => Wing {
                span: positive("span", span)?,
                chord: area / span,
            },
            Self::AspectRatio(ar) => {
                let span = (positive("aspect ratio", ar)? * area).sqrt();
                Wing {
                    span,
                    chord: area / span,
                }
            }
        };
        Ok(wing)
    }
}

/// Coefficients of the finite-wing estimate.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WingModel {
    /// Span efficiency `e` of the induced drag `CL² / (π e AR)`: 1 for an
    /// elliptic loading, about 0.9 for a straight rectangular wing.
    pub span_efficiency: f64,
    /// Wing maximum lift as a fraction of the airfoil's: the wing stalls
    /// a little before its section.
    pub cl_max_factor: f64,
}

impl Default for WingModel {
    fn default() -> Self {
        Self {
            span_efficiency: 0.9,
            cl_max_factor: 0.9,
        }
    }
}

impl WingModel {
    /// Checks that both factors are in (0, 1].
    pub fn validate(&self) -> Result<()> {
        for (name, value) in [
            ("span efficiency", self.span_efficiency),
            ("maximum lift factor", self.cl_max_factor),
        ] {
            if !(value > 0.0 && value <= 1.0) {
                return Err(MissionError::InvalidInput(format!(
                    "{name} must be in (0, 1], got {value}"
                )));
            }
        }
        Ok(())
    }
}

/// What the wing estimate needs from the airfoil polar, at the flight's
/// Reynolds number.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SectionData {
    /// Profile drag coefficient at the required lift coefficient.
    pub cd: f64,
    /// Maximum lift coefficient of the airfoil.
    pub cl_max: f64,
}

/// Estimated performance of the wing in level flight.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Performance {
    /// Profile drag coefficient (the airfoil's).
    pub profile_cd: f64,
    /// Induced drag coefficient `CL² / (π e AR)`.
    pub induced_cd: f64,
    /// Wing drag coefficient, profile + induced.
    pub cd: f64,
    /// Wing drag, N.
    pub drag: f64,
    /// Power to overcome the wing drag, drag × speed, W.
    pub power: f64,
    /// Lift-to-drag ratio of the wing; also its glide ratio.
    pub lift_to_drag: f64,
    /// Sink rate in a glide at this speed, speed / (L/D), m/s.
    pub sink_rate: f64,
    /// Maximum lift coefficient of the wing.
    pub wing_cl_max: f64,
    /// Speed at which the wing stalls in level flight, m/s.
    pub stall_speed: f64,
    /// Required lift coefficient over the wing's maximum: above 1 the wing
    /// cannot fly this slowly.
    pub cl_fraction: f64,
}

/// Performance of the wing at `conditions`, from the airfoil data at the
/// flight's Reynolds number.
pub fn performance(
    conditions: &Conditions,
    section: &SectionData,
    model: &WingModel,
) -> Result<Performance> {
    model.validate()?;
    positive("profile drag coefficient", section.cd)?;
    positive("maximum lift coefficient", section.cl_max)?;
    let cl = conditions.required_cl;
    let induced_cd = cl * cl / (PI * model.span_efficiency * conditions.aspect_ratio);
    let cd = section.cd + induced_cd;
    let drag = conditions.dynamic_pressure * conditions.wing_area * cd;
    let lift_to_drag = cl / cd;
    let wing_cl_max = model.cl_max_factor * section.cl_max;
    let stall_speed = (2.0 * conditions.weight
        / (conditions.atmosphere.density * conditions.wing_area * wing_cl_max))
        .sqrt();
    Ok(Performance {
        profile_cd: section.cd,
        induced_cd,
        cd,
        drag,
        power: drag * conditions.speed,
        lift_to_drag,
        sink_rate: conditions.speed / lift_to_drag,
        wing_cl_max,
        stall_speed,
        cl_fraction: cl / wing_cl_max,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flight::Flight;

    #[test]
    fn dimensions() {
        let wing = Wing {
            span: 1.2,
            chord: 0.2,
        };
        assert!((wing.area() - 0.24).abs() < 1e-15);
        assert!((wing.aspect_ratio() - 6.0).abs() < 1e-14);
        assert!(
            Wing {
                span: 0.0,
                chord: 0.2
            }
            .validate()
            .is_err()
        );
        assert!(
            Wing {
                span: 1.0,
                chord: f64::INFINITY
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn constraints_give_the_area() {
        let by_span = WingConstraint::Span(1.5).wing(0.3).unwrap();
        assert!((by_span.chord - 0.2).abs() < 1e-15);
        let by_ar = WingConstraint::AspectRatio(7.5).wing(0.3).unwrap();
        assert!((by_ar.area() - 0.3).abs() < 1e-15);
        assert!((by_ar.aspect_ratio() - 7.5).abs() < 1e-12);
        assert!(WingConstraint::Span(-1.0).wing(0.3).is_err());
        assert!(WingConstraint::AspectRatio(6.0).wing(0.0).is_err());
    }

    #[test]
    fn hand_calculation() {
        // 500 g, 1.5 m × 0.2 m, 20 km/h at sea level; airfoil Cd 0.02 and
        // Cl max 1.4 at that Reynolds number.
        let flight = Flight {
            speed: 20.0 / 3.6,
            altitude: 0.0,
            temperature_offset: 0.0,
            mass: 0.5,
        };
        let conditions = flight
            .conditions(&Wing {
                span: 1.5,
                chord: 0.2,
            })
            .unwrap();
        let section = SectionData {
            cd: 0.02,
            cl_max: 1.4,
        };
        let p = performance(&conditions, &section, &WingModel::default()).unwrap();

        let q = 0.5 * 1.225 * (20.0_f64 / 3.6).powi(2);
        let weight = 0.5 * 9.806_65;
        let cl = weight / (q * 0.3);
        let cdi = cl * cl / (PI * 0.9 * 7.5);
        assert!((conditions.required_cl - cl).abs() < 1e-4 * cl);
        assert!((p.induced_cd - cdi).abs() < 1e-4 * cdi);
        assert!((p.cd - (0.02 + cdi)).abs() < 1e-4 * p.cd);
        assert!((p.drag - q * 0.3 * (0.02 + cdi)).abs() < 1e-4 * p.drag);
        assert!((p.power - p.drag * 20.0 / 3.6).abs() < 1e-12);
        assert!((p.lift_to_drag * p.drag - weight).abs() < 1e-4 * weight);
        assert!((p.sink_rate * p.lift_to_drag - 20.0 / 3.6).abs() < 1e-12);
        assert!((p.wing_cl_max - 1.26).abs() < 1e-12);
        let stall = (2.0 * weight / (1.225 * 0.3 * 1.26)).sqrt();
        assert!((p.stall_speed - stall).abs() < 1e-4 * stall);
        // Flying at the stall speed needs exactly the wing's maximum lift.
        let at_stall = Flight {
            speed: p.stall_speed,
            ..flight
        }
        .conditions(&Wing {
            span: 1.5,
            chord: 0.2,
        })
        .unwrap();
        assert!((at_stall.required_cl - p.wing_cl_max).abs() < 1e-12);
        assert!((p.cl_fraction - conditions.required_cl / 1.26).abs() < 1e-12);
    }

    #[test]
    fn rejects_invalid_models_and_sections() {
        let flight = Flight {
            speed: 10.0,
            altitude: 0.0,
            temperature_offset: 0.0,
            mass: 1.0,
        };
        let conditions = flight
            .conditions(&Wing {
                span: 1.0,
                chord: 0.2,
            })
            .unwrap();
        let section = SectionData {
            cd: 0.01,
            cl_max: 1.2,
        };
        let bad = WingModel {
            span_efficiency: 1.2,
            ..Default::default()
        };
        assert!(performance(&conditions, &section, &bad).is_err());
        let bad = WingModel {
            cl_max_factor: 0.0,
            ..Default::default()
        };
        assert!(performance(&conditions, &section, &bad).is_err());
        let bad = SectionData {
            cd: 0.0,
            cl_max: 1.2,
        };
        assert!(performance(&conditions, &bad, &WingModel::default()).is_err());
    }
}
