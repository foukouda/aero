//! Sections of the lifting surfaces and the quantities a designer reads
//! from them: areas, mean aerodynamic chords, aspect ratios, tail volumes,
//! the position of the centre of gravity.

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::params::{Aircraft, SurfaceParams};

/// Which part a surface is.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceKind {
    /// Main wing.
    Wing,
    /// Horizontal tail.
    Tail,
    /// Vertical fin.
    Fin,
}

impl SurfaceKind {
    /// Wings and tails are mirrored about the plane of symmetry; the fin
    /// lies in it.
    pub fn mirrored(self) -> bool {
        self != Self::Fin
    }
}

/// A section of a surface (an airfoil placed in space), in AVL's sense: the
/// leading-edge point, the chord along x, the twist about the y axis.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    /// Leading-edge point `[x, y, z]`, m.
    pub le: [f64; 3],
    /// Chord, m.
    pub chord: f64,
    /// Incidence, degrees, positive nose up.
    pub twist_deg: f64,
    /// Airfoil key.
    pub airfoil: String,
}

/// A lifting surface as sections from root to tip (the right half of a
/// mirrored surface).
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Surface {
    /// Which part.
    pub kind: SurfaceKind,
    /// Sections, root first.
    pub sections: Vec<Section>,
}

impl Surface {
    /// The two sections of a straight-tapered surface. Wing and tail
    /// sections stand in planes parallel to the plane of symmetry; the
    /// fin's sections are horizontal.
    pub fn from_params(kind: SurfaceKind, p: &SurfaceParams) -> Result<Self> {
        p.validate(match kind {
            SurfaceKind::Wing => "wing",
            SurfaceKind::Tail => "tail",
            SurfaceKind::Fin => "fin",
        })?;
        let sweep = p.sweep_deg.to_radians().tan();
        let tip = match kind {
            SurfaceKind::Fin => [p.x + p.span * sweep, 0.0, p.z + p.span],
            SurfaceKind::Wing | SurfaceKind::Tail => {
                let half = 0.5 * p.span;
                [
                    p.x + half * sweep,
                    half,
                    p.z + half * p.dihedral_deg.to_radians().tan(),
                ]
            }
        };
        let (root_twist, tip_twist) = match kind {
            SurfaceKind::Fin => (0.0, 0.0),
            _ => (p.incidence_deg, p.incidence_deg + p.twist_deg),
        };
        Ok(Self {
            kind,
            sections: vec![
                Section {
                    le: [p.x, 0.0, p.z],
                    chord: p.root_chord,
                    twist_deg: root_twist,
                    airfoil: p.root_airfoil.clone(),
                },
                Section {
                    le: tip,
                    chord: p.tip_chord,
                    twist_deg: tip_twist,
                    airfoil: p.tip_airfoil.clone(),
                },
            ],
        })
    }

    /// Spanwise coordinate of a section: y for wings and tails, z for a fin.
    fn spanwise(&self, s: &Section) -> f64 {
        match self.kind {
            SurfaceKind::Fin => s.le[2],
            _ => s.le[1],
        }
    }

    /// Planform properties, from the sections (each panel between two
    /// sections has linearly varying chord and leading edge).
    pub fn planform(&self) -> Planform {
        // Integrals over the half span s of c, c², x_le c and y c.
        let (mut area, mut c2, mut xc, mut yc) = (0.0, 0.0, 0.0, 0.0);
        for w in self.sections.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            let ds = (self.spanwise(b) - self.spanwise(a)).abs();
            let (c1, c2_) = (a.chord, b.chord);
            let (x1, x2) = (a.le[0], b.le[0]);
            let (y1, y2) = (self.spanwise(a), self.spanwise(b));
            area += ds * (c1 + c2_) / 2.0;
            c2 += ds * (c1 * c1 + c1 * c2_ + c2_ * c2_) / 3.0;
            // ∫ u v ds for linear u and v: ds (u1 v1 / 3 + (u1 v2 + u2 v1) / 6 + u2 v2 / 3).
            let lin = |u1: f64, u2: f64, v1: f64, v2: f64| {
                ds * (u1 * v1 / 3.0 + (u1 * v2 + u2 * v1) / 6.0 + u2 * v2 / 3.0)
            };
            xc += lin(x1, x2, c1, c2_);
            yc += lin(y1, y2, c1, c2_);
        }
        let first = self.sections.first().map_or(0.0, |s| self.spanwise(s));
        let last = self.sections.last().map_or(0.0, |s| self.spanwise(s));
        let half_span = (last - first).abs();
        let factor = if self.kind.mirrored() { 2.0 } else { 1.0 };
        let span = factor * half_span;
        let total = factor * area;
        let mac = c2 / area;
        Planform {
            span,
            area: total,
            aspect_ratio: span * span / total,
            mac,
            mac_x: xc / area,
            mac_span: yc / area,
            quarter_mac_x: xc / area + 0.25 * mac,
        }
    }
}

/// Planform properties of a surface.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Planform {
    /// Span (height for a fin), m.
    pub span: f64,
    /// Projected area, both halves for a mirrored surface, m².
    pub area: f64,
    /// Aspect ratio, span² / area.
    pub aspect_ratio: f64,
    /// Mean aerodynamic chord (MAC), m.
    pub mac: f64,
    /// Leading edge of the MAC: distance behind the nose, m.
    pub mac_x: f64,
    /// Spanwise position of the MAC (y, or z for a fin), m.
    pub mac_span: f64,
    /// Quarter point of the MAC, m behind the nose: where the surface's
    /// lift acts, roughly.
    pub quarter_mac_x: f64,
}

/// The surfaces of an aircraft.
pub fn surfaces(aircraft: &Aircraft) -> Result<Vec<Surface>> {
    aircraft.validate()?;
    let mut out = vec![Surface::from_params(SurfaceKind::Wing, &aircraft.wing)?];
    if let Some(t) = &aircraft.tail {
        out.push(Surface::from_params(SurfaceKind::Tail, t)?);
    }
    if let Some(f) = &aircraft.fin {
        out.push(Surface::from_params(SurfaceKind::Fin, f)?);
    }
    Ok(out)
}

/// What a designer reads from the aircraft's layout.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    /// The wing.
    pub wing: Planform,
    /// The horizontal tail.
    pub tail: Option<Planform>,
    /// The fin.
    pub fin: Option<Planform>,
    /// Horizontal tail volume coefficient Sh·lh / (S·c̄), lh between the
    /// quarter points of the MACs.
    pub tail_volume: Option<f64>,
    /// Vertical tail volume coefficient Sv·lv / (S·b).
    pub fin_volume: Option<f64>,
    /// Centre of gravity: distance behind the nose, m.
    pub cg_x: f64,
    /// Overall length (nose to the rearmost point), m.
    pub length: f64,
}

/// The layout of `aircraft`.
pub fn layout(aircraft: &Aircraft) -> Result<Layout> {
    let surfaces = surfaces(aircraft)?;
    let find = |kind: SurfaceKind| {
        surfaces
            .iter()
            .find(|s| s.kind == kind)
            .map(Surface::planform)
    };
    let wing = find(SurfaceKind::Wing).unwrap_or_else(|| Surface::planform(&surfaces[0]));
    let tail = find(SurfaceKind::Tail);
    let fin = find(SurfaceKind::Fin);
    let tail_volume =
        tail.map(|t| t.area * (t.quarter_mac_x - wing.quarter_mac_x) / (wing.area * wing.mac));
    let fin_volume =
        fin.map(|f| f.area * (f.quarter_mac_x - wing.quarter_mac_x) / (wing.area * wing.span));
    let rear = surfaces
        .iter()
        .flat_map(|s| s.sections.iter().map(|c| c.le[0] + c.chord))
        .chain(aircraft.fuselage.map(|f| f.length))
        .fold(0.0_f64, f64::max);
    Ok(Layout {
        wing,
        tail,
        fin,
        tail_volume,
        fin_volume,
        cg_x: wing.mac_x + aircraft.cg_mac_fraction * wing.mac,
        length: rear,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Preset;

    /// Textbook formulas of a straight-tapered wing.
    fn trapezoid(p: &SurfaceParams) -> (f64, f64, f64, f64) {
        let (cr, l, b) = (p.root_chord, p.taper(), p.span);
        let area = b * cr * (1.0 + l) / 2.0;
        let mac = 2.0 / 3.0 * cr * (1.0 + l + l * l) / (1.0 + l);
        let y = b / 6.0 * (1.0 + 2.0 * l) / (1.0 + l);
        let x = p.x + y * p.sweep_deg.to_radians().tan();
        (area, mac, y, x)
    }

    #[test]
    fn matches_the_trapezoid_formulas() {
        for preset in Preset::ALL {
            let a = preset.aircraft();
            let pf = Surface::from_params(SurfaceKind::Wing, &a.wing)
                .unwrap()
                .planform();
            let (area, mac, y, x) = trapezoid(&a.wing);
            assert!((pf.area - area).abs() < 1e-12 * area, "{preset:?}");
            assert!((pf.mac - mac).abs() < 1e-12 * mac, "{preset:?}");
            assert!((pf.mac_span - y).abs() < 1e-12, "{preset:?}");
            assert!((pf.mac_x - x).abs() < 1e-12, "{preset:?}");
            assert!((pf.span - a.wing.span).abs() < 1e-12);
            assert!((pf.aspect_ratio - a.wing.span.powi(2) / area).abs() < 1e-9);
        }
    }

    #[test]
    fn rectangular_wing() {
        let mut a = Preset::Trainer.aircraft();
        a.wing.tip_chord = a.wing.root_chord;
        a.wing.sweep_deg = 10.0;
        let pf = Surface::from_params(SurfaceKind::Wing, &a.wing)
            .unwrap()
            .planform();
        assert!((pf.mac - 0.22).abs() < 1e-15);
        assert!((pf.mac_span - 0.3).abs() < 1e-12);
        assert!((pf.area - 1.2 * 0.22).abs() < 1e-15);
        assert!((pf.mac_x - (0.25 + 0.3 * 10f64.to_radians().tan())).abs() < 1e-12);
    }

    #[test]
    fn sections_follow_sweep_dihedral_and_twist() {
        let a = Preset::Glider.aircraft();
        let s = Surface::from_params(SurfaceKind::Wing, &a.wing).unwrap();
        let tip = &s.sections[1];
        assert!((tip.le[1] - 0.75).abs() < 1e-15);
        assert!((tip.le[0] - (0.22 + 0.75 * 2f64.to_radians().tan())).abs() < 1e-12);
        assert!((tip.le[2] - (0.02 + 0.75 * 5f64.to_radians().tan())).abs() < 1e-12);
        assert_eq!(tip.twist_deg, 1.0);
        let fin = Surface::from_params(SurfaceKind::Fin, a.fin.as_ref().unwrap()).unwrap();
        assert_eq!(fin.sections[1].le[1], 0.0);
        assert!((fin.sections[1].le[2] - 0.17).abs() < 1e-12);
        let pf = fin.planform();
        assert!(
            (pf.area - 0.15 * 0.10).abs() < 1e-12,
            "a fin is not mirrored"
        );
    }

    #[test]
    fn tail_volumes_of_the_presets() {
        // Usual ranges: horizontal 0.35 to 0.8, vertical 0.02 to 0.06.
        for preset in [Preset::Trainer, Preset::Glider, Preset::LightAircraft] {
            let l = layout(&preset.aircraft()).unwrap();
            let vh = l.tail_volume.unwrap();
            let vv = l.fin_volume.unwrap();
            assert!((0.35..=0.8).contains(&vh), "{preset:?} Vh {vh}");
            assert!((0.02..=0.06).contains(&vv), "{preset:?} Vv {vv}");
        }
        let fw = layout(&Preset::FlyingWing.aircraft()).unwrap();
        assert!(fw.tail.is_none() && fw.tail_volume.is_none());
    }

    #[test]
    fn hand_calculation_of_a_layout() {
        // Rectangular wing 1 m × 0.2 m at x = 0.3; rectangular tail
        // 0.4 m × 0.1 m at x = 1.0: lh = (1.0 + 0.025) − (0.3 + 0.05).
        let mut a = Preset::Trainer.aircraft();
        a.wing = SurfaceParams {
            span: 1.0,
            root_chord: 0.2,
            tip_chord: 0.2,
            sweep_deg: 0.0,
            dihedral_deg: 0.0,
            x: 0.3,
            ..a.wing
        };
        let tail = a.tail.as_mut().unwrap();
        *tail = SurfaceParams {
            span: 0.4,
            root_chord: 0.1,
            tip_chord: 0.1,
            x: 1.0,
            ..tail.clone()
        };
        a.cg_mac_fraction = 0.25;
        let l = layout(&a).unwrap();
        let lh = 1.025 - 0.35;
        assert!((l.tail_volume.unwrap() - 0.04 * lh / (0.2 * 0.2)).abs() < 1e-12);
        assert!((l.cg_x - 0.35).abs() < 1e-12);
        assert!(l.length >= 1.1);
    }
}
