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
    /// Canard, ahead of the wing.
    Canard,
    /// Horizontal tail.
    Tail,
    /// Vertical fin.
    Fin,
}

impl SurfaceKind {
    /// Wings, canards and tails are mirrored about the plane of symmetry;
    /// the fin lies in it.
    pub fn mirrored(self) -> bool {
        self != Self::Fin
    }

    /// Name in messages.
    pub fn name(self) -> &'static str {
        match self {
            Self::Wing => "wing",
            Self::Canard => "canard",
            Self::Tail => "tail",
            Self::Fin => "fin",
        }
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
    /// The sections of a surface in space: wing, canard and tail sections
    /// stand in planes parallel to the plane of symmetry, the fin's are
    /// horizontal (its sections' `y` is their height).
    pub fn from_params(kind: SurfaceKind, p: &SurfaceParams) -> Result<Self> {
        p.validate(kind.name())?;
        let sections = p
            .sections
            .iter()
            .map(|s| match kind {
                SurfaceKind::Fin => Section {
                    le: [p.x + s.x, 0.0, p.z + s.y],
                    chord: s.chord,
                    twist_deg: 0.0,
                    airfoil: s.airfoil.clone(),
                },
                _ => Section {
                    le: [p.x + s.x, s.y, p.z + s.z],
                    chord: s.chord,
                    twist_deg: p.incidence_deg + s.twist_deg,
                    airfoil: s.airfoil.clone(),
                },
            })
            .collect();
        Ok(Self { kind, sections })
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
    if let Some(c) = &aircraft.canard {
        out.push(Surface::from_params(SurfaceKind::Canard, c)?);
    }
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
    /// The canard.
    pub canard: Option<Planform>,
    /// The horizontal tail.
    pub tail: Option<Planform>,
    /// The fin.
    pub fin: Option<Planform>,
    /// Canard volume coefficient Sc·lc / (S·c̄), lc from the canard's
    /// quarter-MAC point forward to the wing's.
    pub canard_volume: Option<f64>,
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
    let canard = find(SurfaceKind::Canard);
    let tail = find(SurfaceKind::Tail);
    let fin = find(SurfaceKind::Fin);
    let canard_volume =
        canard.map(|c| c.area * (wing.quarter_mac_x - c.quarter_mac_x) / (wing.area * wing.mac));
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
        canard,
        tail,
        fin,
        canard_volume,
        tail_volume,
        fin_volume,
        cg_x: wing.mac_x + aircraft.cg_mac_fraction * wing.mac,
        length: rear,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{Preset, SectionParams, TaperedParams};

    fn tapered(span: f64, root: f64, tip: f64, sweep: f64, dihedral: f64, x: f64) -> TaperedParams {
        TaperedParams {
            span,
            root_chord: root,
            tip_chord: tip,
            sweep_deg: sweep,
            dihedral_deg: dihedral,
            incidence_deg: 2.0,
            twist_deg: -1.0,
            x,
            z: 0.02,
            root_airfoil: "naca:2412".into(),
            tip_airfoil: "naca:0012".into(),
        }
    }

    /// Textbook formulas of a straight-tapered wing.
    fn trapezoid(p: &TaperedParams) -> (f64, f64, f64, f64) {
        let (cr, l, b) = (p.root_chord, p.tip_chord / p.root_chord, p.span);
        let area = b * cr * (1.0 + l) / 2.0;
        let mac = 2.0 / 3.0 * cr * (1.0 + l + l * l) / (1.0 + l);
        let y = b / 6.0 * (1.0 + 2.0 * l) / (1.0 + l);
        let x = p.x + y * p.sweep_deg.to_radians().tan();
        (area, mac, y, x)
    }

    #[test]
    fn matches_the_trapezoid_formulas() {
        for t in [
            tapered(1.2, 0.22, 0.18, 0.0, 3.0, 0.25),
            tapered(1.5, 0.19, 0.12, 2.0, 5.0, 0.22),
            tapered(1.0, 0.28, 0.14, 25.0, 0.0, 0.0),
            tapered(11.0, 1.63, 1.32, 0.0, 1.7, 2.2),
        ] {
            let pf = Surface::from_params(SurfaceKind::Wing, &t.surface(true))
                .unwrap()
                .planform();
            let (area, mac, y, x) = trapezoid(&t);
            assert!((pf.area - area).abs() < 1e-12 * area);
            assert!((pf.mac - mac).abs() < 1e-12 * mac);
            assert!((pf.mac_span - y).abs() < 1e-12);
            assert!((pf.mac_x - x).abs() < 1e-12);
            assert!((pf.span - t.span).abs() < 1e-12);
            assert!((pf.aspect_ratio - t.span.powi(2) / area).abs() < 1e-9);
        }
    }

    #[test]
    fn a_kinked_wing_adds_its_panels() {
        // A rectangular centre panel and a tapered outer panel: the area and
        // the MAC integrals add up panel by panel.
        let section = |y: f64, chord: f64, x: f64| SectionParams {
            y,
            chord,
            x,
            z: 0.0,
            twist_deg: 0.0,
            airfoil: "naca:2412".into(),
        };
        let wing = SurfaceParams {
            x: 0.3,
            z: 0.0,
            incidence_deg: 0.0,
            sections: vec![
                section(0.0, 0.2, 0.0),
                section(0.3, 0.2, 0.0),
                section(0.6, 0.1, 0.05),
            ],
        };
        let pf = Surface::from_params(SurfaceKind::Wing, &wing)
            .unwrap()
            .planform();
        let half = 0.3 * 0.2 + 0.3 * 0.15;
        assert!((pf.area - 2.0 * half).abs() < 1e-15);
        let c2 = 0.3 * 0.04 + 0.3 * (0.04 + 0.02 + 0.01) / 3.0;
        assert!((pf.mac - c2 / half).abs() < 1e-12);
        assert!((pf.span - 1.2).abs() < 1e-15);
    }

    #[test]
    fn sections_follow_the_surface() {
        let a = Preset::Glider.aircraft();
        let s = Surface::from_params(SurfaceKind::Wing, &a.wing).unwrap();
        let tip = &s.sections[1];
        assert!((tip.le[1] - 0.75).abs() < 1e-15);
        assert!((tip.le[0] - (0.22 + 0.75 * 2f64.to_radians().tan())).abs() < 1e-12);
        assert!((tip.le[2] - (0.02 + 0.75 * 5f64.to_radians().tan())).abs() < 1e-12);
        // Incidence 2°, tip twist −1°.
        assert_eq!((s.sections[0].twist_deg, tip.twist_deg), (2.0, 1.0));
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
            assert!(l.canard.is_none() && l.canard_volume.is_none());
        }
        let fw = layout(&Preset::FlyingWing.aircraft()).unwrap();
        assert!(fw.tail.is_none() && fw.tail_volume.is_none());
    }

    #[test]
    fn hand_calculation_of_a_layout() {
        // Rectangular wing 1 m × 0.2 m at x = 0.3; rectangular tail
        // 0.4 m × 0.1 m at x = 1.0: lh = (1.0 + 0.025) − (0.3 + 0.05);
        // rectangular canard 0.4 m × 0.05 m at x = 0.0: lc = 0.35 − 0.0125.
        let rect = |span: f64, chord: f64, x: f64| {
            TaperedParams {
                span,
                root_chord: chord,
                tip_chord: chord,
                sweep_deg: 0.0,
                dihedral_deg: 0.0,
                x,
                ..tapered(1.0, 0.2, 0.2, 0.0, 0.0, 0.0)
            }
            .surface(true)
        };
        let mut a = Preset::Trainer.aircraft();
        a.wing = rect(1.0, 0.2, 0.3);
        a.tail = Some(rect(0.4, 0.1, 1.0));
        a.canard = Some(rect(0.4, 0.05, 0.0));
        a.cg_mac_fraction = 0.25;
        let l = layout(&a).unwrap();
        let lh = 1.025 - 0.35;
        assert!((l.tail_volume.unwrap() - 0.04 * lh / (0.2 * 0.2)).abs() < 1e-12);
        let lc = 0.35 - 0.0125;
        assert!((l.canard_volume.unwrap() - 0.02 * lc / (0.2 * 0.2)).abs() < 1e-12);
        assert!((l.cg_x - 0.35).abs() < 1e-12);
        assert!(l.length >= 1.1);
    }
}
