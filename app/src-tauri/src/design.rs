//! Inverse design sessions (XFOIL `QDES` and `MDES`): the target speed
//! edited from the interface, the preview of the new airfoil.

use aero_foil2d::inverse::{
    DesignBy, FullInverse, MixedInverseOptions, SpeedTarget, mixed_inverse,
};
use aero_geometry::Airfoil;

use crate::dto::{DesignEdit, DesignMethod, DesignPreview, DesignView};
use crate::error::{AppError, ErrorKind};
use crate::model::AirfoilId;

/// Newton iterations allowed for the mixed inverse (XFOIL `EXEC n`; it
/// usually converges in 3 or 4).
const MIXED_ITERATIONS: usize = 20;

/// An inverse design of one airfoil.
#[derive(Debug, Clone)]
pub struct DesignSession {
    /// Airfoil being redesigned.
    pub id: AirfoilId,
    paneled: Airfoil,
    alpha_deg: f64,
    /// Speed of the original airfoil at the design angle.
    current: SpeedTarget,
    kind: Kind,
}

#[derive(Debug, Clone)]
enum Kind {
    Mixed {
        target: SpeedTarget,
        /// Modified points (first, last), reshaped by the mixed inverse.
        segment: Option<(usize, usize)>,
        curvature: bool,
    },
    Full(Box<FullInverse>),
}

fn invalid(message: &str) -> AppError {
    AppError::new(ErrorKind::InvalidInput, message)
}

fn index(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

/// Number for a name: at most three decimals, no trailing zeros.
fn short(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

impl DesignSession {
    /// Starts a design of a paneled airfoil at `alpha_deg`, the target
    /// being its current speed distribution.
    pub fn new(
        id: AirfoilId,
        paneled: Airfoil,
        method: DesignMethod,
        alpha_deg: f64,
    ) -> Result<Self, AppError> {
        let current = SpeedTarget::from_airfoil(&paneled, alpha_deg)?;
        let kind = match method {
            DesignMethod::Mixed => Kind::Mixed {
                target: current.clone(),
                segment: None,
                curvature: false,
            },
            DesignMethod::Full => Kind::Full(Box::new(FullInverse::new(&paneled, alpha_deg)?)),
        };
        Ok(Self {
            id,
            paneled,
            alpha_deg,
            current,
            kind,
        })
    }

    /// The state shown by the interface.
    pub fn view(&self) -> DesignView {
        let current_x = self.paneled.x().to_vec();
        let base = DesignView {
            id: self.id,
            method: DesignMethod::Mixed,
            alpha_deg: self.alpha_deg,
            cl: None,
            by_lift: false,
            current_x: current_x.clone(),
            current_speed: self.current.speed.clone(),
            target_x: current_x,
            target: Vec::new(),
            segment: None,
            curvature_constraints: false,
            symmetric: false,
            te_gap: [0.0, 0.0],
            te_angle_deg: 0.0,
        };
        match &self.kind {
            Kind::Mixed {
                target,
                segment,
                curvature,
            } => DesignView {
                target: target.speed.clone(),
                segment: segment.map(|(a, b)| [index(a), index(b)]),
                curvature_constraints: *curvature,
                ..base
            },
            Kind::Full(f) => {
                let point = &f.points()[0];
                DesignView {
                    method: DesignMethod::Full,
                    cl: Some(point.cl),
                    by_lift: f.design_by() == DesignBy::Lift,
                    target_x: f.circle_points().0.to_vec(),
                    target: point.speed.clone(),
                    symmetric: f.is_symmetric(),
                    te_gap: f.te_gap(),
                    te_angle_deg: f.te_angle_deg(),
                    ..base
                }
            }
        }
    }

    fn set_alpha(&mut self, alpha_deg: f64) -> Result<(), AppError> {
        self.current = SpeedTarget::from_airfoil(&self.paneled, alpha_deg)?;
        self.alpha_deg = alpha_deg;
        Ok(())
    }

    /// Last node before the stagnation point of the original airfoil at the
    /// design angle.
    fn stagnation(&self) -> usize {
        let s = &self.current.speed;
        (0..s.len().saturating_sub(1))
            .find(|&i| s[i] >= 0.0 && s[i + 1] < 0.0)
            .unwrap_or(0)
    }

    fn full(&mut self) -> Result<&mut FullInverse, AppError> {
        match &mut self.kind {
            Kind::Full(f) => Ok(f),
            Kind::Mixed { .. } => Err(invalid("only available for the full inverse")),
        }
    }

    /// Applies a change.
    pub fn edit(&mut self, edit: DesignEdit) -> Result<(), AppError> {
        match edit {
            DesignEdit::Alpha { alpha_deg } => {
                if !alpha_deg.is_finite() {
                    return Err(invalid("the angle of attack must be finite"));
                }
                if let Kind::Full(f) = &mut self.kind {
                    f.set_alphas(&[alpha_deg])?;
                }
                self.set_alpha(alpha_deg)?;
            }
            DesignEdit::Lift { cl } => {
                let f = self.full()?;
                f.set_lift_coefficients(&[cl])?;
                let alpha_deg = f.points()[0].alpha_deg;
                self.set_alpha(alpha_deg)?;
            }
            DesignEdit::Target { first, speed } => {
                let first = first as usize;
                let Some(last) = (first + speed.len()).checked_sub(1) else {
                    return Err(invalid("no target value given"));
                };
                if speed.iter().any(|v| !v.is_finite()) {
                    return Err(invalid("the target speed must be finite"));
                }
                let stagnation = self.stagnation();
                match &mut self.kind {
                    Kind::Mixed {
                        target, segment, ..
                    } => {
                        let values = target
                            .speed
                            .get_mut(first..=last)
                            .ok_or_else(|| invalid("target outside the airfoil"))?;
                        values.copy_from_slice(&speed);
                        // The segment grows over the modified points while
                        // it stays on one side of the stagnation point.
                        let crosses = |a: usize, b: usize| a <= stagnation && stagnation < b;
                        *segment = Some(match *segment {
                            Some((a, b)) if !crosses(a.min(first), b.max(last)) => {
                                (a.min(first), b.max(last))
                            }
                            _ => (first, last),
                        });
                    }
                    Kind::Full(f) => {
                        let mut target = f.points()[0].speed.clone();
                        target
                            .get_mut(first..=last)
                            .ok_or_else(|| invalid("target outside the airfoil"))?
                            .copy_from_slice(&speed);
                        f.set_target(&target)?;
                    }
                }
            }
            DesignEdit::Smooth => match &mut self.kind {
                Kind::Mixed {
                    target,
                    segment: Some((a, b)),
                    ..
                } => target.smooth(*a, *b, true)?,
                Kind::Mixed { segment: None, .. } => {
                    return Err(invalid("draw a new target over part of the surface first"));
                }
                Kind::Full(f) => {
                    let last = f.points()[0].speed.len() - 1;
                    f.smooth(0, last, true)?;
                }
            },
            DesignEdit::Filter => self.full()?.filter(),
            DesignEdit::Symmetric { on } => self.full()?.set_symmetric(on),
            DesignEdit::Curvature { on } => match &mut self.kind {
                Kind::Mixed { curvature, .. } => *curvature = on,
                Kind::Full(_) => return Err(invalid("only available for the mixed inverse")),
            },
            DesignEdit::TeGap { dx, dy } => self.full()?.set_te_gap(dx, dy)?,
            DesignEdit::TeAngle { angle_deg } => self.full()?.set_te_angle(angle_deg)?,
            DesignEdit::Reset => match &mut self.kind {
                Kind::Mixed {
                    target, segment, ..
                } => {
                    *target = self.current.clone();
                    *segment = None;
                }
                Kind::Full(f) => f.reset()?,
            },
        }
        // Designed for a lift coefficient, the full inverse solves for the
        // angle again whenever the target changes.
        if let Kind::Full(f) = &self.kind {
            let alpha_deg = f.points()[0].alpha_deg;
            if alpha_deg != self.alpha_deg {
                self.set_alpha(alpha_deg)?;
            }
        }
        Ok(())
    }

    /// The new airfoil, or `None` while the mixed inverse has no modified
    /// segment.
    pub fn preview(&self) -> Result<Option<(Airfoil, DesignPreview)>, AppError> {
        let (airfoil, speed, converged, iterations, description) = match &self.kind {
            Kind::Mixed {
                target,
                segment,
                curvature,
            } => {
                let Some((first, last)) = *segment else {
                    return Ok(None);
                };
                let options = MixedInverseOptions {
                    first,
                    last,
                    curvature_constraints: *curvature,
                    max_iterations: MIXED_ITERATIONS,
                };
                let r = mixed_inverse(&self.paneled, self.alpha_deg, &target.speed, &options)?;
                (
                    r.airfoil,
                    Some(r.speed),
                    r.converged,
                    r.iterations,
                    format!("mixed inverse at {}°", short(self.alpha_deg)),
                )
            }
            Kind::Full(f) => {
                let mut f = f.clone();
                let r = f.execute()?;
                let point = &f.points()[0];
                let description = if f.design_by() == DesignBy::Lift {
                    format!("full inverse for CL {}", short(point.cl))
                } else {
                    format!("full inverse at {}°", short(point.alpha_deg))
                };
                (r.airfoil, None, r.converged, r.iterations, description)
            }
        };
        let preview = DesignPreview {
            x: airfoil.x().to_vec(),
            y: airfoil.y().to_vec(),
            properties: airfoil.properties(),
            speed,
            converged,
            iterations: index(iterations),
            description,
        };
        Ok(Some((airfoil, preview)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aero_geometry::{NacaOptions, PanelingOptions, naca, panel};

    fn paneled() -> Airfoil {
        panel(
            &naca("2412", &NacaOptions::default()).unwrap(),
            &PanelingOptions::default(),
        )
        .unwrap()
    }

    #[test]
    fn mixed_design_from_a_drawn_target() {
        let mut d = DesignSession::new(1, paneled(), DesignMethod::Mixed, 3.0).unwrap();
        assert!(d.preview().unwrap().is_none());
        let v = d.view();
        assert_eq!(v.target, v.current_speed);
        // Lower the upper-surface speed peak a little.
        let speed: Vec<f64> = v.target[30..60].iter().map(|q| q * 0.98).collect();
        d.edit(DesignEdit::Target { first: 30, speed }).unwrap();
        d.edit(DesignEdit::Smooth).unwrap();
        assert_eq!(d.view().segment, Some([30, 59]));
        let (a, p) = d.preview().unwrap().unwrap();
        assert!(p.converged && p.speed.is_some());
        assert_eq!(a.len(), v.target.len());
        assert!(p.description.contains("3°"));
        // A second stroke on the lower surface replaces the segment.
        let n = v.target.len();
        let speed = v.target[n - 40..n - 20].to_vec();
        d.edit(DesignEdit::Target {
            first: index(n - 40),
            speed,
        })
        .unwrap();
        assert_eq!(d.view().segment, Some([index(n - 40), index(n - 21)]));
        d.edit(DesignEdit::Reset).unwrap();
        assert!(d.view().segment.is_none());
        assert!(d.edit(DesignEdit::Filter).is_err());
    }

    #[test]
    fn full_design_by_lift() {
        let mut d = DesignSession::new(1, paneled(), DesignMethod::Full, 3.0).unwrap();
        d.edit(DesignEdit::Lift { cl: 0.7 }).unwrap();
        let v = d.view();
        assert!(v.by_lift && (v.cl.unwrap() - 0.7).abs() < 1e-4);
        assert!(v.alpha_deg > 3.0);
        // A lower speed on the upper surface: the angle for that lift rises.
        let speed: Vec<f64> = v.target[40..80].iter().map(|q| q * 0.95).collect();
        d.edit(DesignEdit::Target { first: 40, speed }).unwrap();
        let after = d.view();
        assert!((after.cl.unwrap() - 0.7).abs() < 1e-4);
        assert!(
            after.alpha_deg > v.alpha_deg + 0.1,
            "{} {}",
            after.alpha_deg,
            v.alpha_deg
        );
        assert_ne!(after.current_speed, v.current_speed);
        d.edit(DesignEdit::Filter).unwrap();
        d.edit(DesignEdit::TeGap { dx: 0.0, dy: 0.004 }).unwrap();
        let (a, p) = d.preview().unwrap().unwrap();
        assert!(p.converged);
        assert_eq!(a.len(), 257);
        assert!(p.description.contains("CL 0.7"), "{}", p.description);
        assert!(d.edit(DesignEdit::Curvature { on: true }).is_err());
    }

    #[test]
    fn short_numbers() {
        assert_eq!(short(3.0), "3");
        assert_eq!(short(2.5), "2.5");
        assert_eq!(short(-0.0001), "0");
        assert_eq!(short(0.123456), "0.123");
    }
}
