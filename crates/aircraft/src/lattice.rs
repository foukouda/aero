//! The aircraft as a vortex lattice for AVL's method (`aero-vlm`): wing,
//! canard and tail mirrored about the plane of symmetry, the fin on it,
//! the camber line of every section from its airfoil, and a pitch control
//! for trimming — the whole tail (or canard) turning on its leading edge,
//! or elevons over the rear quarter of a flying wing. The fuselage is not
//! part of the lattice.

use std::collections::HashMap;

use aero_geometry::Airfoil;
use aero_vlm::{
    Camber, ControlSurface, LiftingSurface, Reference, Spacing, SurfaceSection, VlmModel,
    VortexCore,
};
use serde::{Deserialize, Serialize};

use crate::error::{AircraftError, Result};
use crate::params::Aircraft;
use crate::planform::{Surface, SurfaceKind, layout, surfaces};

/// Name of the pitch control of every lattice.
pub const ELEVATOR: &str = "elevator";

/// Hinge of a flying wing's elevons, as a fraction of the chord.
pub const ELEVON_HINGE: f64 = 0.75;

/// How finely the surfaces are divided.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LatticeOptions {
    /// Chordwise vortices on the wing and canard.
    pub wing_chordwise: u32,
    /// Spanwise vortices on each half of the wing.
    pub wing_spanwise: u32,
    /// Chordwise vortices on the tail and fin.
    pub tail_chordwise: u32,
    /// Spanwise vortices on each half of the canard and tail, and on the fin.
    pub tail_spanwise: u32,
    /// Mach number of the flight.
    pub mach: f64,
}

impl Default for LatticeOptions {
    /// 8 × 20 vortices on each half wing, 6 × 8 on the other surfaces:
    /// within about 1 % of finer lattices for lift and moments.
    fn default() -> Self {
        Self {
            wing_chordwise: 8,
            wing_spanwise: 20,
            tail_chordwise: 6,
            tail_spanwise: 8,
            mach: 0.0,
        }
    }
}

/// Where a strip of the lattice comes from.
#[derive(Debug, Clone, PartialEq)]
pub struct StripOrigin {
    /// The part.
    pub kind: SurfaceKind,
    /// Panel between sections `panel` and `panel + 1` (root first).
    pub panel: usize,
    /// The left half, mirror image of the right one.
    pub left: bool,
    /// Leading edges of the panel's two sections (mirrored for the left
    /// half), to place the strip along the panel.
    ends: [[f64; 3]; 2],
}

impl StripOrigin {
    /// Fraction of the way from section `panel` to `panel + 1` of a strip
    /// whose leading edge is `le` (as the vortex lattice reports it).
    pub fn fraction(&self, le: [f64; 3]) -> f64 {
        let [a, b] = self.ends;
        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let r = [le[0] - a[0], le[1] - a[1], le[2] - a[2]];
        let dd = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
        if dd == 0.0 {
            0.0
        } else {
            ((r[0] * d[0] + r[1] * d[1] + r[2] * d[2]) / dd).clamp(0.0, 1.0)
        }
    }
}

/// The lattice of an aircraft and where each of its strips comes from.
#[derive(Debug, Clone)]
pub struct AircraftLattice {
    /// The vortex-lattice model.
    pub model: VlmModel,
    /// Origin of every strip, in the lattice's order.
    pub strips: Vec<StripOrigin>,
    /// The surfaces, in the lattice's order (each mirrored surface is
    /// followed by its left half).
    pub surfaces: Vec<Surface>,
}

/// The vortex lattice of `aircraft`; `airfoil` resolves the section keys.
/// The reference quantities are the wing's area, mean aerodynamic chord and
/// span; moments are about the centre of gravity, on the fuselage axis.
pub fn lattice(
    aircraft: &Aircraft,
    airfoil: &dyn Fn(&str) -> Option<Airfoil>,
    options: &LatticeOptions,
) -> Result<AircraftLattice> {
    let parts = surfaces(aircraft)?;
    let l = layout(aircraft)?;
    let counts = [
        options.wing_chordwise,
        options.wing_spanwise,
        options.tail_chordwise,
        options.tail_spanwise,
    ];
    if counts.iter().any(|&n| !(1..=200).contains(&n)) || !(0.0..0.9).contains(&options.mach) {
        return Err(AircraftError::Invalid(
            "the lattice needs 1 to 200 vortices each way and a Mach number below 0.9".into(),
        ));
    }

    // Camber lines, one per airfoil.
    let mut cambers: HashMap<String, Camber> = HashMap::new();
    for key in parts
        .iter()
        .flat_map(|s| s.sections.iter().map(|c| &c.airfoil))
    {
        if cambers.contains_key(key) {
            continue;
        }
        let a = airfoil(key).ok_or_else(|| AircraftError::UnknownAirfoil(key.clone()))?;
        let camber = Camber::from_airfoil(key, a.x(), a.y(), (0.0, 1.0))
            .map_err(|e| AircraftError::Invalid(format!("airfoil {key}: {e}")))?;
        cambers.insert(key.clone(), camber);
    }

    // The surface that trims the aircraft in pitch.
    let pitch_control = if aircraft.tail.is_some() {
        SurfaceKind::Tail
    } else if aircraft.canard.is_some() {
        SurfaceKind::Canard
    } else {
        SurfaceKind::Wing
    };

    let mut model_surfaces = Vec::new();
    let mut strips = Vec::new();
    let mut ordered = Vec::new();
    for part in &parts {
        let (chordwise, spanwise) = match part.kind {
            SurfaceKind::Wing => (options.wing_chordwise, options.wing_spanwise),
            SurfaceKind::Canard => (options.wing_chordwise, options.tail_spanwise),
            SurfaceKind::Tail | SurfaceKind::Fin => (options.tail_chordwise, options.tail_spanwise),
        };
        let counts = panel_counts(part, spanwise as usize);
        let sections = part
            .sections
            .iter()
            .enumerate()
            .map(|(k, s)| {
                let mut sec = SurfaceSection::new(s.le, s.chord);
                sec.incidence_deg = s.twist_deg;
                sec.camber = cambers.get(&s.airfoil).cloned().unwrap_or_default();
                sec.spanwise = counts.get(k).map(|&n| Spacing::new(n, 1.0));
                if part.kind == pitch_control {
                    let hinge = if part.kind == SurfaceKind::Wing {
                        ELEVON_HINGE
                    } else {
                        0.0
                    };
                    let mut c = ControlSurface::new(ELEVATOR, hinge);
                    if hinge == 0.0 {
                        // An all-moving surface turns about the y axis.
                        c.axis = [0.0, 1.0, 0.0];
                    }
                    sec.controls.push(c);
                }
                sec
            })
            .collect();
        let mut surface = LiftingSurface::new(
            part.kind.name(),
            Spacing::new(chordwise as usize, 1.0),
            sections,
        );
        let halves: &[bool] = if part.kind.mirrored() {
            surface.y_duplicate = Some(0.0);
            &[false, true]
        } else {
            &[false]
        };
        for &left in halves {
            let mirror = |p: [f64; 3]| if left { [p[0], -p[1], p[2]] } else { p };
            for (panel, &n) in counts.iter().enumerate() {
                let ends = [
                    mirror(part.sections[panel].le),
                    mirror(part.sections[panel + 1].le),
                ];
                for _ in 0..n {
                    strips.push(StripOrigin {
                        kind: part.kind,
                        panel,
                        left,
                        ends,
                    });
                }
            }
            ordered.push(part.clone());
        }
        model_surfaces.push(surface);
    }

    let model = VlmModel {
        title: "aircraft".into(),
        mach: options.mach,
        y_symmetry: false,
        reference: Reference {
            area: l.wing.area,
            chord: l.wing.mac,
            span: l.wing.span,
            point: [l.cg_x, 0.0, 0.0],
        },
        cd_p: 0.0,
        core: VortexCore::default(),
        surfaces: model_surfaces,
    };
    Ok(AircraftLattice {
        model,
        strips,
        surfaces: ordered,
    })
}

/// Strips of each panel of a surface: `total` shared out by the panels'
/// lengths in the y–z plane, at least one each.
fn panel_counts(surface: &Surface, total: usize) -> Vec<usize> {
    let lengths: Vec<f64> = surface
        .sections
        .windows(2)
        .map(|w| ((w[1].le[1] - w[0].le[1]).powi(2) + (w[1].le[2] - w[0].le[2]).powi(2)).sqrt())
        .collect();
    let sum: f64 = lengths.iter().sum();
    lengths
        .iter()
        .map(|len| ((total as f64 * len / sum).round() as usize).max(1))
        .collect()
}

#[cfg(test)]
mod tests {
    use aero_geometry::{NacaOptions, naca};
    use aero_vlm::{FlowCondition, VortexLattice};

    use super::*;
    use crate::params::Preset;

    fn airfoil(key: &str) -> Option<Airfoil> {
        let code = key.strip_prefix("naca:")?;
        naca(code, &NacaOptions::default()).ok()
    }

    /// The presets with NACA airfoils everywhere.
    fn presets() -> Vec<(Preset, Aircraft)> {
        [
            Preset::Trainer,
            Preset::Glider,
            Preset::FlyingWing,
            Preset::LightAircraft,
        ]
        .into_iter()
        .map(|p| {
            let mut a = p.aircraft();
            for s in [
                Some(&mut a.wing),
                a.canard.as_mut(),
                a.tail.as_mut(),
                a.fin.as_mut(),
            ]
            .into_iter()
            .flatten()
            {
                for (k, sec) in s.sections.iter_mut().enumerate() {
                    sec.airfoil = if k == 0 {
                        "naca:2412".into()
                    } else {
                        "naca:0010".into()
                    };
                }
            }
            (p, a)
        })
        .collect()
    }

    #[test]
    fn presets_become_lattices_that_lift_and_are_stable() {
        for (preset, a) in presets() {
            let lat = lattice(&a, &airfoil, &LatticeOptions::default()).unwrap();
            let l = layout(&a).unwrap();
            assert_eq!(lat.model.reference.area, l.wing.area, "{preset:?}");
            assert_eq!(lat.model.reference.point[0], l.cg_x);
            let vl = VortexLattice::new(&lat.model).unwrap();
            assert_eq!(vl.strip_count(), lat.strips.len(), "{preset:?}");
            assert_eq!(vl.control_names(), [ELEVATOR.to_string()]);
            let s = vl.solve(&FlowCondition::alpha(4.0)).unwrap();
            let d = &s.derivatives;
            assert!(
                s.totals.cl > 0.2 && s.totals.cl < 1.0,
                "{preset:?}: CL {}",
                s.totals.cl
            );
            // Every preset is stable in pitch with its centre of gravity:
            // the neutral point is behind it.
            let np = d.neutral_point.unwrap();
            assert!(
                np > l.cg_x,
                "{preset:?}: neutral point {np} ahead of CG {}",
                l.cg_x
            );
            // A positive elevator (trailing edge down) pitches the nose down.
            assert!(d.controls[0].forces.pitch < 0.0, "{preset:?}");
            // Weathercock stability where there is a fin.
            if a.fin.is_some() {
                assert!(d.beta.yaw > 0.0, "{preset:?}: Cnβ {}", d.beta.yaw);
            }
        }
    }

    #[test]
    fn strips_know_their_part_and_place() {
        let (_, a) = presets().remove(0);
        let lat = lattice(&a, &airfoil, &LatticeOptions::default()).unwrap();
        let vl = VortexLattice::new(&lat.model).unwrap();
        let s = vl.solve(&FlowCondition::alpha(2.0)).unwrap();
        for (origin, strip) in lat.strips.iter().zip(&s.strips) {
            let f = origin.fraction(strip.leading_edge);
            assert!((0.0..=1.0).contains(&f));
            let y = strip.leading_edge[1];
            match origin.kind {
                SurfaceKind::Fin => assert_eq!(y, 0.0),
                _ => assert_eq!(y < 0.0, origin.left, "{origin:?}"),
            }
        }
        let wing = lat
            .strips
            .iter()
            .filter(|o| o.kind == SurfaceKind::Wing)
            .count();
        assert_eq!(wing, 2 * 20);
        // The surfaces in the lattice's order.
        let kinds: Vec<SurfaceKind> = lat.surfaces.iter().map(|s| s.kind).collect();
        assert_eq!(kinds[..2], [SurfaceKind::Wing, SurfaceKind::Wing]);
    }

    #[test]
    fn wing_incidence_shifts_the_lift() {
        let (_, mut a) = presets().remove(0);
        let cl = |a: &Aircraft| {
            let lat = lattice(a, &airfoil, &LatticeOptions::default()).unwrap();
            let s = VortexLattice::new(&lat.model)
                .unwrap()
                .solve(&FlowCondition::alpha(0.0))
                .unwrap();
            (s.totals.cl, s.derivatives.alpha.cl)
        };
        let (cl0, cla) = cl(&a);
        a.wing.incidence_deg += 1.0;
        let (cl1, _) = cl(&a);
        // One more degree on the whole wing: about the wing's share of the
        // aircraft's lift slope.
        let per_degree = (cl1 - cl0) / cla.to_radians();
        assert!(per_degree > 0.7 && per_degree < 1.1, "{per_degree}");
    }

    #[test]
    fn fraction_along_a_panel() {
        let o = StripOrigin {
            kind: SurfaceKind::Wing,
            panel: 0,
            left: true,
            ends: [[0.1, -0.2, 0.0], [0.3, -1.2, 0.1]],
        };
        let at = |f: f64| [0.1 + 0.2 * f, -0.2 - 1.0 * f, 0.1 * f];
        for f in [0.0, 0.3, 0.75, 1.0] {
            assert!((o.fraction(at(f)) - f).abs() < 1e-14);
        }
        assert_eq!(o.fraction(at(1.5)), 1.0);
    }

    #[test]
    fn panel_counts_follow_the_panel_lengths() {
        let (_, a) = presets().remove(0);
        let mut wing = Surface::from_params(SurfaceKind::Wing, &a.wing).unwrap();
        let tip = wing.sections[1].clone();
        let mut mid = tip.clone();
        mid.le = [tip.le[0] * 0.25, tip.le[1] * 0.25, tip.le[2] * 0.25];
        wing.sections.insert(1, mid);
        assert_eq!(panel_counts(&wing, 20), vec![5, 15]);
        assert_eq!(panel_counts(&wing, 1), vec![1, 1]);
    }
}
