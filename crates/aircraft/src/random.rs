//! Random aircraft, to explore shapes: a layout (classic, canard, flying
//! wing, three surfaces), a wing of 2 to 4 sections with random span,
//! aspect ratio, taper, sweep, dihedral (gull and polyhedral wings
//! included) and twist, airfoils drawn from pools by role, and tails sized
//! for usual volume coefficients. Every aircraft is valid; the same seed
//! gives the same aircraft.

use serde::{Deserialize, Serialize};

use crate::error::{AircraftError, Result};
use crate::params::{Aircraft, Fuselage, SectionParams, SurfaceParams};
use crate::planform::layout;

/// SplitMix64: a small, fast, reproducible generator.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    /// A generator from a seed.
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// Next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform in [a, b).
    pub fn uniform(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.unit()
    }

    /// True with probability `p`.
    pub fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }

    /// An integer in [a, b].
    pub fn integer(&mut self, a: usize, b: usize) -> usize {
        a + (self.next_u64() % (b - a + 1) as u64) as usize
    }

    /// An element of `items` (none if empty).
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            None
        } else {
            items.get(self.integer(0, items.len() - 1))
        }
    }
}

/// Layout of a random aircraft.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Configuration {
    /// Wing, horizontal tail and fin behind.
    Classic,
    /// Canard ahead, wing behind with the fin.
    Canard,
    /// No tail: a swept wing with a reflexed airfoil.
    FlyingWing,
    /// Canard, wing and tail.
    ThreeSurfaces,
}

/// Airfoil keys to draw from, by role.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AirfoilPools {
    /// Cambered airfoils for wings and canards.
    pub lifting: Vec<String>,
    /// Reflexed airfoils for flying wings.
    pub reflexed: Vec<String>,
    /// Symmetric airfoils for tails and fins.
    pub symmetric: Vec<String>,
}

impl AirfoilPools {
    fn check(&self) -> Result<()> {
        if self.lifting.is_empty() || self.reflexed.is_empty() || self.symmetric.is_empty() {
            return Err(AircraftError::Invalid(
                "random aircraft need lifting, reflexed and symmetric airfoils".to_string(),
            ));
        }
        Ok(())
    }
}

/// Positions in (0, 1) of the inner sections of a surface, sorted and at
/// least `gap` apart and from the ends.
fn stations(rng: &mut Rng, inner: usize, gap: f64) -> Vec<f64> {
    loop {
        let mut t: Vec<f64> = (0..inner).map(|_| rng.uniform(gap, 1.0 - gap)).collect();
        t.sort_by(f64::total_cmp);
        if t.windows(2).all(|w| w[1] - w[0] >= gap) {
            let mut out = vec![0.0];
            out.extend(t);
            out.push(1.0);
            return out;
        }
    }
}

/// A random wing of `span` and mean chord `mean_chord`, as sections.
#[allow(clippy::too_many_arguments)]
fn random_wing(
    rng: &mut Rng,
    span: f64,
    mean_chord: f64,
    taper: f64,
    sweep_deg: f64,
    dihedral_deg: f64,
    washout_deg: f64,
    airfoils: &[String],
) -> Vec<SectionParams> {
    let half = 0.5 * span;
    let n = rng.integer(2, 4);
    let t = stations(rng, n - 2, 0.15);
    let root_chord = 2.0 * mean_chord / (1.0 + taper);
    let root_airfoil = rng.pick(airfoils).cloned().unwrap_or_default();
    let tip_airfoil = if rng.chance(0.4) {
        rng.pick(airfoils).cloned().unwrap_or_default()
    } else {
        root_airfoil.clone()
    };
    // Gull (inner panel up, outer flat or down) or polyhedral (tips up).
    let shape = rng.unit();
    let mut sections = Vec::with_capacity(n);
    let (mut x, mut z) = (0.0, 0.0);
    for (i, &ti) in t.iter().enumerate() {
        if i > 0 {
            let dy = half * (ti - t[i - 1]);
            let first = i == 1;
            let last = i == n - 1;
            let mut dihedral = dihedral_deg + rng.uniform(-1.5, 1.5);
            if n > 2 && shape < 0.25 {
                dihedral = if first {
                    rng.uniform(8.0, 16.0)
                } else {
                    rng.uniform(-4.0, 2.0)
                };
            } else if n > 2 && shape < 0.5 && last {
                dihedral = rng.uniform(8.0, 18.0);
            }
            let sweep = sweep_deg + rng.uniform(-4.0, 4.0);
            x += dy * sweep.to_radians().tan();
            z += dy * dihedral.to_radians().tan();
        }
        let chord = root_chord
            * (1.0 + (taper - 1.0) * ti)
            * if i == 0 { 1.0 } else { rng.uniform(0.92, 1.08) };
        sections.push(SectionParams {
            y: half * ti,
            chord,
            x,
            z,
            twist_deg: washout_deg * ti,
            airfoil: if i == 0 {
                root_airfoil.clone()
            } else if i == n - 1 {
                tip_airfoil.clone()
            } else if rng.chance(0.3) {
                rng.pick(airfoils).cloned().unwrap_or_default()
            } else {
                root_airfoil.clone()
            },
        });
    }
    sections
}

/// A straight-tapered surface of area `area`, aspect ratio `ar` and taper
/// `taper`, as two sections (`mirrored`: span tip to tip; otherwise its
/// height).
fn tapered(
    area: f64,
    ar: f64,
    taper: f64,
    sweep_deg: f64,
    mirrored: bool,
    airfoil: &str,
) -> Vec<SectionParams> {
    let span = (ar * area).sqrt();
    let mean = area / span;
    let root = 2.0 * mean / (1.0 + taper);
    let outer = if mirrored { 0.5 * span } else { span };
    vec![
        SectionParams {
            y: 0.0,
            chord: root,
            x: 0.0,
            z: 0.0,
            twist_deg: 0.0,
            airfoil: airfoil.to_string(),
        },
        SectionParams {
            y: outer,
            chord: root * taper,
            x: outer * sweep_deg.to_radians().tan(),
            z: 0.0,
            twist_deg: 0.0,
            airfoil: airfoil.to_string(),
        },
    ]
}

/// A surface sized by its volume coefficient.
#[derive(Clone, Copy)]
enum Part {
    Canard,
    Tail,
    Fin,
}

/// Sets the tail or the fin built by `build` for an area, with the area
/// that gives the volume coefficient `target` (a few fixed-point steps,
/// since the arm changes with the chord), at most `cap`.
fn fit_volume(
    aircraft: &mut Aircraft,
    part: Part,
    target: f64,
    cap: f64,
    build: impl Fn(f64) -> SurfaceParams,
) -> Result<()> {
    let wing = layout(aircraft)?.wing;
    let mut area = match part {
        Part::Canard | Part::Tail => target * wing.area * wing.mac / (2.5 * wing.mac),
        Part::Fin => target * wing.area * wing.span / (2.5 * wing.mac),
    }
    .min(cap);
    for _ in 0..8 {
        let surface = Some(build(area));
        match part {
            Part::Canard => aircraft.canard = surface,
            Part::Tail => aircraft.tail = surface,
            Part::Fin => aircraft.fin = surface,
        }
        let l = layout(aircraft)?;
        let volume = match part {
            Part::Canard => l.canard_volume,
            Part::Tail => l.tail_volume,
            Part::Fin => l.fin_volume,
        }
        .unwrap_or(0.0);
        if volume <= 0.0 {
            break;
        }
        area = (area * target / volume).min(cap);
    }
    let surface = Some(build(area));
    match part {
        Part::Canard => aircraft.canard = surface,
        Part::Tail => aircraft.tail = surface,
        Part::Fin => aircraft.fin = surface,
    }
    Ok(())
}

fn draw(rng: &mut Rng, pools: &AirfoilPools) -> Result<(Configuration, Aircraft)> {
    let roll = rng.unit();
    let kind = if roll < 0.5 {
        Configuration::Classic
    } else if roll < 0.68 {
        Configuration::Canard
    } else if roll < 0.9 {
        Configuration::FlyingWing
    } else {
        Configuration::ThreeSurfaces
    };
    let symmetric = rng.pick(&pools.symmetric).cloned().unwrap_or_default();
    let span = rng.uniform(0.8, 2.4);
    let flying_wing = kind == Configuration::FlyingWing;
    let canard_led = matches!(kind, Configuration::Canard);
    let ar = if flying_wing {
        rng.uniform(4.0, 7.0)
    } else {
        rng.uniform(5.5, 12.0)
    };
    let mean_chord = span / ar;
    let wing_airfoils = if flying_wing {
        &pools.reflexed
    } else {
        &pools.lifting
    };
    let taper = if flying_wing {
        rng.uniform(0.3, 0.7)
    } else {
        rng.uniform(0.45, 1.0)
    };
    let sweep = if flying_wing {
        rng.uniform(18.0, 32.0)
    } else if canard_led {
        rng.uniform(8.0, 25.0)
    } else {
        rng.uniform(-2.0, 10.0)
    };
    let dihedral = if flying_wing {
        rng.uniform(-1.0, 3.0)
    } else {
        rng.uniform(-1.0, 6.0)
    };
    let washout = if flying_wing {
        rng.uniform(-5.0, -2.0)
    } else {
        rng.uniform(-3.0, 0.5)
    };
    let sections = random_wing(
        rng,
        span,
        mean_chord,
        taper,
        sweep,
        dihedral,
        washout,
        wing_airfoils,
    );
    let length = span * rng.uniform(0.6, 0.85);
    let wing_x = match kind {
        Configuration::Classic => length * rng.uniform(0.2, 0.3),
        Configuration::Canard => length * rng.uniform(0.5, 0.62),
        Configuration::ThreeSurfaces => length * rng.uniform(0.35, 0.45),
        Configuration::FlyingWing => 0.0,
    };
    let mut aircraft = Aircraft {
        wing: SurfaceParams {
            x: wing_x,
            z: if flying_wing {
                0.0
            } else {
                length * rng.uniform(-0.04, 0.05)
            },
            incidence_deg: if flying_wing {
                rng.uniform(-0.5, 1.0)
            } else {
                rng.uniform(0.0, 3.0)
            },
            sections,
        },
        canard: None,
        tail: None,
        fin: None,
        fuselage: if flying_wing {
            rng.chance(0.3).then(|| Fuselage {
                length: 0.35 * span,
                diameter: 0.07 * span * rng.uniform(0.8, 1.2),
            })
        } else {
            Some(Fuselage {
                length,
                diameter: length * rng.uniform(0.07, 0.11),
            })
        },
        cg_mac_fraction: match kind {
            Configuration::Classic => rng.uniform(0.25, 0.33),
            Configuration::FlyingWing => rng.uniform(0.15, 0.25),
            Configuration::Canard => rng.uniform(-0.15, 0.1),
            Configuration::ThreeSurfaces => rng.uniform(0.1, 0.25),
        },
    };
    let wing = layout(&aircraft)?.wing;
    // The fuselage ends at least half a mean chord behind the wing root's
    // trailing edge and two mean chords behind the wing's quarter-chord
    // point, so that tails and fins have an arm.
    let root_te = aircraft.wing.x + aircraft.wing.sections[0].chord;
    if let Some(f) = aircraft.fuselage.as_mut().filter(|_| !flying_wing) {
        f.length = f
            .length
            .max(root_te + 0.5 * wing.mac)
            .max(wing.quarter_mac_x + 2.0 * wing.mac);
    }
    let tail_end = aircraft
        .fuselage
        .map_or(wing.mac_x + 2.0 * wing.mac, |f| f.length);
    if matches!(kind, Configuration::Classic | Configuration::ThreeSurfaces) {
        // A tail at the end of the fuselage, sized for a volume coefficient
        // of 0.4 to 0.7.
        let target = rng.uniform(0.4, 0.7);
        let (ar_t, taper_t, sweep_t) = (
            rng.uniform(3.5, 5.5),
            rng.uniform(0.55, 1.0),
            rng.uniform(0.0, 15.0),
        );
        let z_t = aircraft
            .fuselage
            .map_or(0.0, |f| 0.15 * f.diameter * rng.uniform(-1.0, 1.0));
        let incidence = rng.uniform(-1.5, 0.5);
        let build = |area: f64| {
            let sections = tapered(area, ar_t, taper_t, sweep_t, true, &symmetric);
            SurfaceParams {
                x: tail_end - 1.05 * sections[0].chord,
                z: z_t,
                incidence_deg: incidence,
                sections,
            }
        };
        fit_volume(&mut aircraft, Part::Tail, target, 0.35 * wing.area, build)?;
    }
    if matches!(kind, Configuration::Canard | Configuration::ThreeSurfaces) {
        // A canard behind the nose, sized for a volume coefficient of 0.3
        // to 0.6 (about 0.5 for the Rutan VariEze).
        let target = rng.uniform(0.3, 0.6);
        let airfoil = rng.pick(&pools.lifting).cloned().unwrap_or_default();
        let (ar_c, taper_c, sweep_c) = (
            rng.uniform(4.0, 7.0),
            rng.uniform(0.6, 1.0),
            rng.uniform(0.0, 10.0),
        );
        let (x_c, z_c) = (0.04 * length, rng.uniform(-0.02, 0.04) * length);
        let incidence = rng.uniform(1.0, 3.0);
        let build = |area: f64| SurfaceParams {
            x: x_c,
            z: z_c,
            incidence_deg: incidence,
            sections: tapered(area, ar_c, taper_c, sweep_c, true, &airfoil),
        };
        fit_volume(&mut aircraft, Part::Canard, target, 0.3 * wing.area, build)?;
    }
    // Fin: at the end of the fuselage, sized for a volume coefficient of
    // 0.025 to 0.045; a small one at the root trailing edge of a flying
    // wing.
    let (ar_f, sweep_f) = (rng.uniform(1.2, 2.0), rng.uniform(20.0, 45.0));
    if flying_wing {
        if rng.chance(0.7) {
            let root = aircraft.wing.sections[0].chord;
            let sections = tapered(
                0.04 * wing.area,
                ar_f,
                rng.uniform(0.4, 0.7),
                sweep_f,
                false,
                &symmetric,
            );
            aircraft.fin = Some(SurfaceParams {
                x: (root - 1.1 * sections[0].chord).max(0.0),
                z: 0.0,
                incidence_deg: 0.0,
                sections,
            });
        }
    } else {
        // Canard aircraft have the wing at the back: smaller fins.
        let target = if canard_led {
            rng.uniform(0.018, 0.03)
        } else {
            rng.uniform(0.025, 0.045)
        };
        let taper_f = rng.uniform(0.4, 0.75);
        let build = |area: f64| {
            let sections = tapered(area, ar_f, taper_f, sweep_f, false, &symmetric);
            SurfaceParams {
                x: tail_end - 1.1 * sections[0].chord,
                z: 0.0,
                incidence_deg: 0.0,
                sections,
            }
        };
        fit_volume(&mut aircraft, Part::Fin, target, 0.2 * wing.area, build)?;
    }
    aircraft.validate()?;
    Ok((kind, aircraft))
}

/// A random aircraft from `rng`; a draw that comes out invalid (it should
/// not) is replaced by the next one.
pub fn random_aircraft(rng: &mut Rng, pools: &AirfoilPools) -> Result<(Configuration, Aircraft)> {
    pools.check()?;
    let mut last = None;
    for _ in 0..20 {
        match draw(rng, pools) {
            Ok(a) => return Ok(a),
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or_else(|| AircraftError::Invalid("no valid random aircraft".to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pools() -> AirfoilPools {
        AirfoilPools {
            lifting: vec!["naca:2412".into(), "naca:4412".into(), "naca:6409".into()],
            reflexed: vec!["catalogue:mh60".into()],
            symmetric: vec!["naca:0009".into(), "naca:0012".into()],
        }
    }

    #[test]
    fn many_random_aircraft_are_valid_and_plausible() {
        let mut rng = Rng::new(7);
        let mut kinds = std::collections::HashSet::new();
        let mut gull_or_polyhedral = 0;
        for _ in 0..2000 {
            let (kind, a) = random_aircraft(&mut rng, &pools()).unwrap();
            a.validate().unwrap();
            kinds.insert(format!("{kind:?}"));
            let l = layout(&a).unwrap();
            assert!((0.8..=2.4).contains(&l.wing.span), "{}", l.wing.span);
            match kind {
                Configuration::Classic | Configuration::ThreeSurfaces => {
                    let vh = l.tail_volume.unwrap();
                    assert!((0.39..=0.71).contains(&vh), "{kind:?} Vh {vh}");
                }
                Configuration::Canard => assert!(a.tail.is_none() && a.canard.is_some()),
                Configuration::FlyingWing => {
                    assert!(a.is_flying_wing());
                    assert!(
                        a.wing
                            .sections
                            .iter()
                            .all(|s| s.airfoil == "catalogue:mh60")
                    );
                }
            }
            if !a.is_flying_wing() {
                let vv = l.fin_volume.unwrap();
                let low = if kind == Configuration::Canard {
                    0.01
                } else {
                    0.02
                };
                assert!((low..=0.05).contains(&vv), "{kind:?} Vv {vv}");
            }
            if let Some(c) = &a.canard {
                assert!(
                    c.x + c.sections[0].chord < a.wing.x,
                    "the canard is ahead of the wing"
                );
                let vc = l.canard_volume.unwrap();
                assert!((0.29..=0.61).contains(&vc), "{kind:?} Vc {vc}");
            }
            let dihedrals: Vec<f64> = a
                .wing
                .sections
                .windows(2)
                .map(|w| ((w[1].z - w[0].z) / (w[1].y - w[0].y)).atan().to_degrees())
                .collect();
            if dihedrals.iter().any(|d| *d > 7.5) {
                gull_or_polyhedral += 1;
            }
        }
        assert_eq!(kinds.len(), 4, "{kinds:?}");
        assert!(gull_or_polyhedral > 200, "{gull_or_polyhedral}");
    }

    #[test]
    fn random_aircraft_mesh() {
        use crate::mesh::{MeshOptions, mesh};
        use aero_geometry::{NacaOptions, naca};
        let resolve = |key: &str| {
            let designation = key.strip_prefix("naca:").unwrap_or("2412");
            naca(designation, &NacaOptions::default()).ok()
        };
        let mut rng = Rng::new(11);
        for _ in 0..100 {
            let (_, a) = random_aircraft(&mut rng, &pools()).unwrap();
            let parts = mesh(&a, &resolve, &MeshOptions::default()).unwrap();
            assert!(parts.iter().all(|p| !p.indices.is_empty()));
        }
    }

    #[test]
    fn the_same_seed_gives_the_same_aircraft() {
        let a = random_aircraft(&mut Rng::new(42), &pools()).unwrap();
        let b = random_aircraft(&mut Rng::new(42), &pools()).unwrap();
        let c = random_aircraft(&mut Rng::new(43), &pools()).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn empty_pools_are_refused() {
        let mut p = pools();
        p.reflexed.clear();
        assert!(random_aircraft(&mut Rng::new(1), &p).is_err());
    }

    #[test]
    fn generator_basics() {
        let mut rng = Rng::new(3);
        for _ in 0..1000 {
            let u = rng.unit();
            assert!((0.0..1.0).contains(&u));
            let k = rng.integer(2, 4);
            assert!((2..=4).contains(&k));
        }
        for inner in 0..=2 {
            for _ in 0..1000 {
                let t = stations(&mut rng, inner, 0.15);
                assert_eq!(t.len(), inner + 2);
                assert_eq!((t[0], t[inner + 1]), (0.0, 1.0));
                assert!(t.windows(2).all(|w| w[1] - w[0] >= 0.15), "{t:?}");
            }
        }
        assert_eq!(rng.pick::<u8>(&[]), None);
    }
}
