//! The aircraft as a beginner describes it: each lifting surface a list of
//! sections, as in XFLR5 and AVL, and simpler straight-tapered parts to
//! start from.
//!
//! Axes as in AVL: x from the nose towards the tail, y towards the right
//! wing tip, z up; metres and degrees.

use serde::{Deserialize, Serialize};

use crate::error::{AircraftError, Result, positive, within};

/// Most sections in a surface.
pub const MAX_SECTIONS: usize = 30;

/// Steepest sweep or dihedral of a panel between two sections, degrees.
pub const MAX_PANEL_ANGLE: f64 = 75.0;

/// A section of a surface: an airfoil placed relative to the surface's root
/// leading edge.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionParams {
    /// Distance from the plane of symmetry, m; for a fin, the height above
    /// its root.
    pub y: f64,
    /// Chord, m.
    pub chord: f64,
    /// Leading edge behind the root leading edge, m (sweep).
    pub x: f64,
    /// Leading edge above the root leading edge, m (dihedral; not used by
    /// a fin).
    pub z: f64,
    /// Twist relative to the surface's incidence, degrees, positive nose
    /// up; negative at the tips is washout, which delays their stall.
    pub twist_deg: f64,
    /// Airfoil (a key the caller resolves, such as `naca:2412` or
    /// `catalogue:sd7037`).
    pub airfoil: String,
}

/// A lifting surface — wing, canard or horizontal tail, mirrored about the
/// plane of symmetry — or the vertical fin on that plane.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SurfaceParams {
    /// Root leading edge: distance behind the nose, m.
    pub x: f64,
    /// Root leading edge: height above the fuselage axis, m.
    pub z: f64,
    /// Incidence of the whole surface relative to the fuselage axis,
    /// degrees, positive nose up (not used by a fin).
    pub incidence_deg: f64,
    /// Sections from the root (y = 0) outwards.
    pub sections: Vec<SectionParams>,
}

impl SurfaceParams {
    /// Checks the sections: from y = 0 outwards, positive chords, panels
    /// no steeper than [`MAX_PANEL_ANGLE`].
    pub fn validate(&self, name: &str) -> Result<()> {
        let n = self.sections.len();
        if !(2..=MAX_SECTIONS).contains(&n) {
            return Err(AircraftError::Invalid(format!(
                "{name} needs 2 to {MAX_SECTIONS} sections, has {n}"
            )));
        }
        within(&format!("{name} position x"), self.x, -1e3, 1e3)?;
        within(&format!("{name} position z"), self.z, -1e3, 1e3)?;
        within(
            &format!("{name} incidence"),
            self.incidence_deg,
            -15.0,
            15.0,
        )?;
        let limit = MAX_PANEL_ANGLE.to_radians().tan();
        for (i, s) in self.sections.iter().enumerate() {
            let what = format!("{name} section {}", i + 1);
            positive(&format!("{what} chord"), s.chord)?;
            within(&format!("{what} twist"), s.twist_deg, -20.0, 20.0)?;
            within(&format!("{what} offset"), s.x, -1e3, 1e3)?;
            within(&format!("{what} height"), s.z, -1e3, 1e3)?;
            within(&format!("{what} position"), s.y, 0.0, 1e3)?;
        }
        if self.sections[0].y != 0.0 {
            return Err(AircraftError::Invalid(format!(
                "{name}: the first section must be at the root (0 m)"
            )));
        }
        for (i, w) in self.sections.windows(2).enumerate() {
            let dy = w[1].y - w[0].y;
            if dy <= 0.0 {
                return Err(AircraftError::Invalid(format!(
                    "{name}: section {} must be further out than section {}",
                    i + 2,
                    i + 1
                )));
            }
            if (w[1].x - w[0].x).abs() > limit * dy || (w[1].z - w[0].z).abs() > limit * dy {
                return Err(AircraftError::Invalid(format!(
                    "{name}: the panel between sections {} and {} is steeper than {MAX_PANEL_ANGLE}°",
                    i + 1,
                    i + 2
                )));
            }
        }
        Ok(())
    }

    /// Distance of the outermost section from the root, m.
    pub fn half_span(&self) -> f64 {
        self.sections.last().map_or(0.0, |s| s.y)
    }

    /// The same surface with every length multiplied by `k` (angles and
    /// airfoils kept).
    pub fn scaled(&self, k: f64) -> Self {
        Self {
            x: k * self.x,
            z: k * self.z,
            incidence_deg: self.incidence_deg,
            sections: self
                .sections
                .iter()
                .map(|s| SectionParams {
                    y: k * s.y,
                    chord: k * s.chord,
                    x: k * s.x,
                    z: k * s.z,
                    ..s.clone()
                })
                .collect(),
        }
    }

    /// The same surface with every chord multiplied by `k`.
    pub fn with_chords_scaled(&self, k: f64) -> Self {
        let mut out = self.clone();
        for s in &mut out.sections {
            s.chord *= k;
        }
        out
    }
}

/// A straight-tapered surface: the quick way to describe a part, turned into
/// sections by [`TaperedParams::surface`].
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaperedParams {
    /// Span, tip to tip, seen from above, m. For a fin: its height.
    pub span: f64,
    /// Chord at the root, m.
    pub root_chord: f64,
    /// Chord at the tip, m.
    pub tip_chord: f64,
    /// Sweep of the leading edge, degrees, positive backwards.
    pub sweep_deg: f64,
    /// Dihedral, degrees, positive tips up (not used by a fin).
    pub dihedral_deg: f64,
    /// Incidence, degrees, positive nose up (not used by a fin).
    pub incidence_deg: f64,
    /// Twist of the tip relative to the root, degrees.
    pub twist_deg: f64,
    /// Root leading edge: distance behind the nose, m.
    pub x: f64,
    /// Root leading edge: height above the fuselage axis, m.
    pub z: f64,
    /// Airfoil of the root section.
    pub root_airfoil: String,
    /// Airfoil of the tip section.
    pub tip_airfoil: String,
}

impl TaperedParams {
    /// The two sections of this surface; `mirrored` for a wing, canard or
    /// tail (the span is tip to tip), not for a fin (the span is its
    /// height).
    pub fn surface(&self, mirrored: bool) -> SurfaceParams {
        let outer = if mirrored { 0.5 * self.span } else { self.span };
        let dihedral = if mirrored { self.dihedral_deg } else { 0.0 };
        SurfaceParams {
            x: self.x,
            z: self.z,
            incidence_deg: if mirrored { self.incidence_deg } else { 0.0 },
            sections: vec![
                SectionParams {
                    y: 0.0,
                    chord: self.root_chord,
                    x: 0.0,
                    z: 0.0,
                    twist_deg: 0.0,
                    airfoil: self.root_airfoil.clone(),
                },
                SectionParams {
                    y: outer,
                    chord: self.tip_chord,
                    x: outer * self.sweep_deg.to_radians().tan(),
                    z: outer * dihedral.to_radians().tan(),
                    twist_deg: if mirrored { self.twist_deg } else { 0.0 },
                    airfoil: self.tip_airfoil.clone(),
                },
            ],
        }
    }
}

/// A fuselage: a body of revolution along the x axis, starting at the nose.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Fuselage {
    /// Length, m.
    pub length: f64,
    /// Largest diameter, m.
    pub diameter: f64,
}

impl Fuselage {
    /// Fraction of the length taken by the rounded nose.
    pub const NOSE: f64 = 0.2;
    /// Fraction of the length where the tail cone starts.
    pub const TAIL_CONE: f64 = 0.55;
    /// Radius at the end of the tail cone, as a fraction of the largest.
    pub const TAIL_END: f64 = 0.15;

    /// Radius at `x` behind the nose, m: an elliptic nose, a cylinder, a
    /// straight tail cone.
    pub fn radius(&self, x: f64) -> f64 {
        let r = 0.5 * self.diameter;
        let t = x / self.length;
        if !(0.0..=1.0).contains(&t) {
            0.0
        } else if t < Self::NOSE {
            let u = (Self::NOSE - t) / Self::NOSE;
            r * (1.0 - u * u).max(0.0).sqrt()
        } else if t <= Self::TAIL_CONE {
            r
        } else {
            let u = (t - Self::TAIL_CONE) / (1.0 - Self::TAIL_CONE);
            r * (1.0 - u * (1.0 - Self::TAIL_END))
        }
    }
}

/// A simplified aircraft.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Aircraft {
    /// The wing.
    pub wing: SurfaceParams,
    /// Canard: a lifting surface ahead of the wing.
    pub canard: Option<SurfaceParams>,
    /// Horizontal tail.
    pub tail: Option<SurfaceParams>,
    /// Vertical fin (its sections' `y` is the height).
    pub fin: Option<SurfaceParams>,
    /// Fuselage; none for a pure flying wing.
    pub fuselage: Option<Fuselage>,
    /// Centre of gravity, as a fraction of the wing's mean aerodynamic chord
    /// behind its leading edge (0.25 to 0.33 is usual with a tail, 0.15 to
    /// 0.25 for a flying wing; ahead of the wing with a canard).
    pub cg_mac_fraction: f64,
}

impl Aircraft {
    /// Checks every part.
    pub fn validate(&self) -> Result<()> {
        self.wing.validate("wing")?;
        if let Some(c) = &self.canard {
            c.validate("canard")?;
        }
        if let Some(t) = &self.tail {
            t.validate("tail")?;
        }
        if let Some(f) = &self.fin {
            f.validate("fin")?;
        }
        if let Some(f) = &self.fuselage {
            positive("fuselage length", f.length)?;
            positive("fuselage diameter", f.diameter)?;
        }
        within("centre of gravity", self.cg_mac_fraction, -2.0, 2.0)?;
        Ok(())
    }

    /// No horizontal tail and no canard.
    pub fn is_flying_wing(&self) -> bool {
        self.tail.is_none() && self.canard.is_none()
    }

    /// The same aircraft with every length multiplied by `k`.
    pub fn scaled(&self, k: f64) -> Self {
        Self {
            wing: self.wing.scaled(k),
            canard: self.canard.as_ref().map(|c| c.scaled(k)),
            tail: self.tail.as_ref().map(|t| t.scaled(k)),
            fin: self.fin.as_ref().map(|f| f.scaled(k)),
            fuselage: self.fuselage.map(|f| Fuselage {
                length: k * f.length,
                diameter: k * f.diameter,
            }),
            cg_mac_fraction: self.cg_mac_fraction,
        }
    }
}

/// Example aircraft.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    /// A 1.2 m high-wing trainer.
    Trainer,
    /// A 1.5 m sailplane.
    Glider,
    /// A 1 m swept flying wing.
    FlyingWing,
    /// A light aircraft close to the Cessna 172 (11 m, 16.2 m²).
    LightAircraft,
}

fn tapered(
    span: f64,
    chords: (f64, f64),
    sweep_deg: f64,
    dihedral_deg: f64,
    angles: (f64, f64),
    position: (f64, f64),
    airfoil: &str,
) -> TaperedParams {
    TaperedParams {
        span,
        root_chord: chords.0,
        tip_chord: chords.1,
        sweep_deg,
        dihedral_deg,
        incidence_deg: angles.0,
        twist_deg: angles.1,
        x: position.0,
        z: position.1,
        root_airfoil: airfoil.to_string(),
        tip_airfoil: airfoil.to_string(),
    }
}

impl Preset {
    /// Every preset.
    pub const ALL: [Self; 4] = [
        Self::Trainer,
        Self::Glider,
        Self::FlyingWing,
        Self::LightAircraft,
    ];

    /// The aircraft of the preset.
    pub fn aircraft(self) -> Aircraft {
        let wing = |t: TaperedParams| t.surface(true);
        let fin = |t: TaperedParams| t.surface(false);
        match self {
            Self::Trainer => Aircraft {
                wing: wing(tapered(
                    1.2,
                    (0.22, 0.18),
                    0.0,
                    3.0,
                    (2.0, 0.0),
                    (0.25, 0.04),
                    "catalogue:clarky",
                )),
                canard: None,
                tail: Some(wing(tapered(
                    0.42,
                    (0.13, 0.10),
                    0.0,
                    0.0,
                    (0.0, 0.0),
                    (0.82, 0.0),
                    "naca:0009",
                ))),
                fin: Some(fin(tapered(
                    0.16,
                    (0.15, 0.08),
                    30.0,
                    0.0,
                    (0.0, 0.0),
                    (0.80, 0.02),
                    "naca:0009",
                ))),
                fuselage: Some(Fuselage {
                    length: 0.95,
                    diameter: 0.09,
                }),
                cg_mac_fraction: 0.30,
            },
            Self::Glider => Aircraft {
                wing: wing(tapered(
                    1.5,
                    (0.19, 0.12),
                    2.0,
                    5.0,
                    (2.0, -1.0),
                    (0.22, 0.02),
                    "catalogue:sd7037",
                )),
                canard: None,
                tail: Some(wing(tapered(
                    0.32,
                    (0.10, 0.07),
                    0.0,
                    0.0,
                    (0.0, 0.0),
                    (0.95, 0.0),
                    "naca:0009",
                ))),
                fin: Some(fin(tapered(
                    0.15,
                    (0.13, 0.07),
                    25.0,
                    0.0,
                    (0.0, 0.0),
                    (0.93, 0.02),
                    "naca:0009",
                ))),
                fuselage: Some(Fuselage {
                    length: 1.05,
                    diameter: 0.06,
                }),
                cg_mac_fraction: 0.33,
            },
            Self::FlyingWing => Aircraft {
                wing: wing(tapered(
                    1.0,
                    (0.28, 0.14),
                    25.0,
                    0.0,
                    (0.0, -3.0),
                    (0.0, 0.0),
                    "catalogue:mh60",
                )),
                canard: None,
                tail: None,
                fin: Some(fin(tapered(
                    0.10,
                    (0.12, 0.06),
                    35.0,
                    0.0,
                    (0.0, 0.0),
                    (0.22, 0.0),
                    "naca:0009",
                ))),
                fuselage: None,
                cg_mac_fraction: 0.20,
            },
            Self::LightAircraft => Aircraft {
                wing: wing(tapered(
                    11.0,
                    (1.63, 1.32),
                    0.0,
                    1.7,
                    (1.5, -3.0),
                    (2.2, 0.7),
                    "naca:2412",
                )),
                canard: None,
                tail: Some(wing(tapered(
                    3.45,
                    (1.2, 0.8),
                    0.0,
                    0.0,
                    (0.0, 0.0),
                    (6.4, 0.1),
                    "naca:0012",
                ))),
                fin: Some(fin(tapered(
                    1.5,
                    (1.6, 0.8),
                    35.0,
                    0.0,
                    (0.0, 0.0),
                    (6.0, 0.3),
                    "naca:0012",
                ))),
                fuselage: Some(Fuselage {
                    length: 8.28,
                    diameter: 1.2,
                }),
                cg_mac_fraction: 0.25,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_valid() {
        for p in Preset::ALL {
            let a = p.aircraft();
            a.validate().unwrap();
            assert_eq!(a.is_flying_wing(), p == Preset::FlyingWing);
        }
    }

    #[test]
    fn tapered_parts_become_two_sections() {
        let t = tapered(
            1.5,
            (0.19, 0.12),
            2.0,
            5.0,
            (2.0, -1.0),
            (0.22, 0.02),
            "naca:2412",
        );
        let s = t.surface(true);
        assert_eq!(s.sections.len(), 2);
        assert_eq!((s.x, s.z, s.incidence_deg), (0.22, 0.02, 2.0));
        let tip = &s.sections[1];
        assert_eq!(tip.y, 0.75);
        assert!((tip.x - 0.75 * 2f64.to_radians().tan()).abs() < 1e-15);
        assert!((tip.z - 0.75 * 5f64.to_radians().tan()).abs() < 1e-15);
        assert_eq!((tip.chord, tip.twist_deg), (0.12, -1.0));
        // A fin: the span is its height; no dihedral, incidence or twist.
        let f = t.surface(false);
        assert_eq!(f.sections[1].y, 1.5);
        assert_eq!(
            (f.sections[1].z, f.sections[1].twist_deg, f.incidence_deg),
            (0.0, 0.0, 0.0)
        );
        assert_eq!(s.half_span(), 0.75);
    }

    #[test]
    fn invalid_surfaces_are_rejected() {
        let ok = Preset::Trainer.aircraft();
        let mut a = ok.clone();
        a.wing.sections[1].chord = 0.0;
        assert!(a.validate().is_err());
        let mut a = ok.clone();
        a.wing.sections.truncate(1);
        assert!(a.validate().is_err());
        let mut a = ok.clone();
        a.wing.sections[0].y = 0.1;
        assert!(a.validate().is_err(), "the root must be at 0");
        let mut a = ok.clone();
        let mut middle = a.wing.sections[1].clone();
        middle.y = 0.6;
        a.wing.sections.insert(1, middle);
        assert!(a.validate().is_err(), "sections must go outwards");
        let mut a = ok.clone();
        a.wing.sections[1].z = 3.0;
        assert!(a.validate().is_err(), "a vertical panel");
        let mut a = ok.clone();
        a.wing.sections[1].twist_deg = f64::NAN;
        assert!(a.validate().is_err());
        let mut a = ok.clone();
        a.fuselage = Some(Fuselage {
            length: 1.0,
            diameter: -0.1,
        });
        assert!(a.validate().is_err());
        let mut a = ok;
        a.canard = Some(a.wing.scaled(0.4));
        a.validate().unwrap();
        assert!(!a.is_flying_wing());
    }

    #[test]
    fn scaling_keeps_the_proportions() {
        let a = Preset::Glider.aircraft();
        let b = a.scaled(2.0);
        assert_eq!(b.wing.half_span(), 2.0 * a.wing.half_span());
        assert_eq!(b.wing.sections[1].chord, 2.0 * a.wing.sections[1].chord);
        assert_eq!(b.wing.sections[1].z, 2.0 * a.wing.sections[1].z);
        assert_eq!(b.wing.incidence_deg, a.wing.incidence_deg);
        assert_eq!(b.tail.as_ref().unwrap().x, 2.0 * a.tail.as_ref().unwrap().x);
        assert_eq!(
            b.fuselage.unwrap().diameter,
            2.0 * a.fuselage.unwrap().diameter
        );
        let c = a.wing.with_chords_scaled(1.5);
        assert_eq!(c.sections[0].chord, 1.5 * a.wing.sections[0].chord);
        assert_eq!(c.sections[1].y, a.wing.sections[1].y);
    }

    #[test]
    fn fuselage_shape() {
        let f = Fuselage {
            length: 1.0,
            diameter: 0.1,
        };
        assert_eq!(f.radius(0.0), 0.0);
        assert!((f.radius(0.1) - 0.05 * (0.75_f64).sqrt()).abs() < 1e-12);
        assert_eq!(f.radius(0.4), 0.05);
        assert!((f.radius(1.0) - 0.05 * Fuselage::TAIL_END).abs() < 1e-12);
        assert_eq!(f.radius(1.5), 0.0);
        for t in [Fuselage::NOSE, Fuselage::TAIL_CONE] {
            assert!((f.radius(t - 1e-9) - f.radius(t + 1e-9)).abs() < 1e-6);
        }
    }

    #[test]
    fn serde_round_trip() {
        let a = Preset::Glider.aircraft();
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(serde_json::from_str::<Aircraft>(&json).unwrap(), a);
    }
}
