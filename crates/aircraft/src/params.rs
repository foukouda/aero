//! The aircraft as a beginner describes it: a few dimensions per part.
//!
//! Axes as in AVL: x from the nose towards the tail, y towards the right
//! wing tip, z up; metres and degrees.

use serde::{Deserialize, Serialize};

use crate::error::{Result, positive, within};

/// A straight-tapered lifting surface: a wing or horizontal tail, mirrored
/// about the plane of symmetry, or a single vertical fin on that plane.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SurfaceParams {
    /// Span, tip to tip, seen from above, m. For a fin: its height.
    pub span: f64,
    /// Chord at the root (plane of symmetry), m.
    pub root_chord: f64,
    /// Chord at the tip, m.
    pub tip_chord: f64,
    /// Sweep of the leading edge, degrees, positive backwards.
    pub sweep_deg: f64,
    /// Dihedral, degrees, positive tips up (not used by a fin).
    pub dihedral_deg: f64,
    /// Incidence of the root section relative to the fuselage axis,
    /// degrees, positive nose up (not used by a fin).
    pub incidence_deg: f64,
    /// Twist of the tip relative to the root, degrees; negative is washout
    /// (tips at a lower angle, which stall later).
    pub twist_deg: f64,
    /// Root leading edge: distance behind the nose, m.
    pub x: f64,
    /// Root leading edge: height above the fuselage axis, m.
    pub z: f64,
    /// Airfoil of the root section (a key the caller resolves, such as
    /// `naca:2412` or `catalogue:sd7037`).
    pub root_airfoil: String,
    /// Airfoil of the tip section.
    pub tip_airfoil: String,
}

impl SurfaceParams {
    /// Checks the dimensions and angles.
    pub fn validate(&self, name: &str) -> Result<()> {
        positive(&format!("{name} span"), self.span)?;
        positive(&format!("{name} root chord"), self.root_chord)?;
        positive(&format!("{name} tip chord"), self.tip_chord)?;
        within(&format!("{name} sweep"), self.sweep_deg, -60.0, 60.0)?;
        within(&format!("{name} dihedral"), self.dihedral_deg, -30.0, 30.0)?;
        within(
            &format!("{name} incidence"),
            self.incidence_deg,
            -15.0,
            15.0,
        )?;
        within(&format!("{name} twist"), self.twist_deg, -15.0, 15.0)?;
        within(&format!("{name} position x"), self.x, -1e3, 1e3)?;
        within(&format!("{name} position z"), self.z, -1e3, 1e3)?;
        Ok(())
    }

    /// Taper ratio, tip chord / root chord.
    pub fn taper(&self) -> f64 {
        self.tip_chord / self.root_chord
    }

    /// The same surface with every length multiplied by `k` (angles and
    /// airfoils kept).
    pub fn scaled(&self, k: f64) -> Self {
        Self {
            span: k * self.span,
            root_chord: k * self.root_chord,
            tip_chord: k * self.tip_chord,
            x: k * self.x,
            z: k * self.z,
            ..self.clone()
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
    /// Horizontal tail; none for a flying wing.
    pub tail: Option<SurfaceParams>,
    /// Vertical fin (its span is its height).
    pub fin: Option<SurfaceParams>,
    /// Fuselage; none for a pure flying wing.
    pub fuselage: Option<Fuselage>,
    /// Centre of gravity, as a fraction of the wing's mean aerodynamic chord
    /// behind its leading edge (0.25 to 0.33 is usual with a tail, 0.15 to
    /// 0.25 for a flying wing).
    pub cg_mac_fraction: f64,
}

impl Aircraft {
    /// Checks every part.
    pub fn validate(&self) -> Result<()> {
        self.wing.validate("wing")?;
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
        within("centre of gravity", self.cg_mac_fraction, -1.0, 2.0)?;
        Ok(())
    }

    /// No horizontal tail.
    pub fn is_flying_wing(&self) -> bool {
        self.tail.is_none()
    }

    /// The same aircraft with every length multiplied by `k`.
    pub fn scaled(&self, k: f64) -> Self {
        Self {
            wing: self.wing.scaled(k),
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

fn surface(
    span: f64,
    chords: (f64, f64),
    sweep_deg: f64,
    dihedral_deg: f64,
    angles: (f64, f64),
    position: (f64, f64),
    airfoil: &str,
) -> SurfaceParams {
    SurfaceParams {
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
        match self {
            Self::Trainer => Aircraft {
                wing: surface(
                    1.2,
                    (0.22, 0.18),
                    0.0,
                    3.0,
                    (2.0, 0.0),
                    (0.25, 0.04),
                    "catalogue:clarky",
                ),
                tail: Some(surface(
                    0.42,
                    (0.13, 0.10),
                    0.0,
                    0.0,
                    (0.0, 0.0),
                    (0.82, 0.0),
                    "naca:0009",
                )),
                fin: Some(surface(
                    0.16,
                    (0.15, 0.08),
                    30.0,
                    0.0,
                    (0.0, 0.0),
                    (0.80, 0.02),
                    "naca:0009",
                )),
                fuselage: Some(Fuselage {
                    length: 0.95,
                    diameter: 0.09,
                }),
                cg_mac_fraction: 0.30,
            },
            Self::Glider => Aircraft {
                wing: surface(
                    1.5,
                    (0.19, 0.12),
                    2.0,
                    5.0,
                    (2.0, -1.0),
                    (0.22, 0.02),
                    "catalogue:sd7037",
                ),
                tail: Some(surface(
                    0.32,
                    (0.10, 0.07),
                    0.0,
                    0.0,
                    (0.0, 0.0),
                    (0.95, 0.0),
                    "naca:0009",
                )),
                fin: Some(surface(
                    0.15,
                    (0.13, 0.07),
                    25.0,
                    0.0,
                    (0.0, 0.0),
                    (0.93, 0.02),
                    "naca:0009",
                )),
                fuselage: Some(Fuselage {
                    length: 1.05,
                    diameter: 0.06,
                }),
                cg_mac_fraction: 0.33,
            },
            Self::FlyingWing => Aircraft {
                wing: surface(
                    1.0,
                    (0.28, 0.14),
                    25.0,
                    0.0,
                    (0.0, -3.0),
                    (0.0, 0.0),
                    "catalogue:mh60",
                ),
                tail: None,
                fin: Some(surface(
                    0.10,
                    (0.12, 0.06),
                    35.0,
                    0.0,
                    (0.0, 0.0),
                    (0.22, 0.0),
                    "naca:0009",
                )),
                fuselage: None,
                cg_mac_fraction: 0.20,
            },
            Self::LightAircraft => Aircraft {
                wing: surface(
                    11.0,
                    (1.63, 1.32),
                    0.0,
                    1.7,
                    (1.5, -3.0),
                    (2.2, 0.7),
                    "naca:2412",
                ),
                tail: Some(surface(
                    3.45,
                    (1.2, 0.8),
                    0.0,
                    0.0,
                    (0.0, 0.0),
                    (6.4, 0.1),
                    "naca:0012",
                )),
                fin: Some(surface(
                    1.5,
                    (1.6, 0.8),
                    35.0,
                    0.0,
                    (0.0, 0.0),
                    (6.0, 0.3),
                    "naca:0012",
                )),
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
    fn invalid_parts_are_rejected() {
        let mut a = Preset::Trainer.aircraft();
        a.wing.span = 0.0;
        assert!(a.validate().is_err());
        let mut a = Preset::Trainer.aircraft();
        a.wing.sweep_deg = 70.0;
        assert!(a.validate().is_err());
        let mut a = Preset::Trainer.aircraft();
        if let Some(t) = a.tail.as_mut() {
            t.tip_chord = f64::NAN;
        }
        assert!(a.validate().is_err());
        let mut a = Preset::Trainer.aircraft();
        a.fuselage = Some(Fuselage {
            length: 1.0,
            diameter: -0.1,
        });
        assert!(a.validate().is_err());
    }

    #[test]
    fn scaling_keeps_the_proportions() {
        let a = Preset::Glider.aircraft();
        let b = a.scaled(2.0);
        assert_eq!(b.wing.span, 2.0 * a.wing.span);
        assert_eq!(b.wing.tip_chord, 2.0 * a.wing.tip_chord);
        assert_eq!(b.wing.sweep_deg, a.wing.sweep_deg);
        assert_eq!(b.tail.as_ref().unwrap().x, 2.0 * a.tail.as_ref().unwrap().x);
        assert_eq!(
            b.fuselage.unwrap().diameter,
            2.0 * a.fuselage.unwrap().diameter
        );
        assert_eq!(b.cg_mac_fraction, a.cg_mac_fraction);
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
        // Continuous at the joints.
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
