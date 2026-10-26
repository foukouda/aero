//! The analysis of a whole aircraft: trim, stability, drag and stall of the
//! example trainer flying with NACA airfoils.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use aero_aircraft::{Aircraft, LatticeOptions, Preset, SurfaceKind, layout};
use aero_geometry::{Airfoil, NacaOptions, PanelingOptions, naca};
use aero_mission::{AnalysisSettings, AnalysisWarning, Flight, PolarCache, analyze, section_polar};
use aero_vlm::{FlowCondition, VortexLattice};

fn airfoil(key: &str) -> Option<Airfoil> {
    naca(key.strip_prefix("naca:")?, &NacaOptions::default()).ok()
}

fn trainer() -> Aircraft {
    let mut a = Preset::Trainer.aircraft();
    for s in [Some(&mut a.wing), a.tail.as_mut(), a.fin.as_mut()]
        .into_iter()
        .flatten()
    {
        for sec in &mut s.sections {
            sec.airfoil = "naca:0009".into();
        }
    }
    for sec in &mut a.wing.sections {
        sec.airfoil = "naca:2412".into();
    }
    a
}

/// Quick polars: 1° steps, 120 panels.
fn cache() -> PolarCache<fn(&str) -> Option<Airfoil>> {
    let settings = AnalysisSettings {
        alpha_step: 1.0,
        alpha_max: 18.0,
        alpha_min: -10.0,
        paneling: PanelingOptions {
            n_nodes: 120,
            ..PanelingOptions::default()
        },
        ..AnalysisSettings::default()
    };
    PolarCache::new(airfoil as fn(&str) -> Option<Airfoil>, settings)
}

fn flight(speed: f64) -> Flight {
    Flight {
        speed,
        altitude: 100.0,
        temperature_offset: 0.0,
        mass: 0.8,
    }
}

#[test]
fn trainer_flies_trimmed_stable_and_with_a_consistent_drag() {
    let a = trainer();
    let mut polars = cache();
    let options = LatticeOptions::default();
    let r = analyze(&a, &flight(12.0), &airfoil, &mut polars, &options).unwrap();
    let l = layout(&a).unwrap();

    // Trimmed: the required lift with no pitching moment, a small elevator.
    let t = &r.trim;
    assert!(t.trimmed, "{t:?}");
    assert!((t.cl - r.required_cl).abs() < 1e-6 && t.cm.abs() < 1e-6);
    assert!(
        t.elevator_deg.abs() < 8.0 && t.alpha_deg.abs() < 8.0,
        "{t:?}"
    );
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);

    // Stable in pitch and yaw, with a sensible margin.
    let st = &r.stability;
    assert!((st.static_margin - (st.neutral_point - l.cg_x) / l.wing.mac).abs() < 1e-12);
    assert!(st.static_margin > 0.05 && st.static_margin < 0.4, "{st:?}");
    assert!(st.cm_alpha < 0.0 && st.cn_beta > 0.0 && st.cl_beta < 0.0);

    // Drag: the parts add up, each in its usual range.
    let d = &r.drag;
    let sum = d.induced + d.wing_profile + d.tail_profile + d.fuselage;
    assert!((d.total - sum).abs() < 1e-15);
    assert!(d.wing_profile > 0.005 && d.wing_profile < 0.03, "{d:?}");
    assert!(d.tail_profile > 0.0 && d.tail_profile < d.wing_profile);
    assert!(d.fuselage > 0.0 && d.fuselage < 0.02);
    assert!(d.lift_to_drag > 5.0 && d.lift_to_drag < 20.0, "{d:?}");
    // Drag and power from the coefficient.
    let w = 0.8 * 9.80665;
    assert!((d.drag - w / d.lift_to_drag).abs() < 1e-9 * w);
    assert!((d.power - d.drag * 12.0).abs() < 1e-9);

    // Every strip has its polar data; the wing's lift is below its maximum.
    for s in &r.strips {
        assert!(s.cd.is_some() && s.cl_max.is_some(), "{s:?}");
        if s.part == SurfaceKind::Wing {
            assert!(s.cl < s.cl_max.unwrap());
            assert!(s.reynolds > 1e5 && s.reynolds < 2.5e5);
        }
    }

    // The stall: slower than the flight, where the wing reaches its
    // maximum lift.
    let stall = r.stall.unwrap();
    assert!(
        stall.speed > 0.4 * 12.0 && stall.speed < 0.9 * 12.0,
        "{stall:?}"
    );
    assert!(stall.cl_max > r.required_cl && stall.cl_max < 1.6);
    assert_eq!(stall.part, SurfaceKind::Wing);
    let w_check = 0.5 * 1.2133 * stall.speed.powi(2) * l.wing.area * stall.cl_max;
    assert!((w_check / w - 1.0).abs() < 1e-3);

    // The polars were kept: a second analysis computes none.
    let kept = polars.len();
    let again = analyze(&a, &flight(12.5), &airfoil, &mut polars, &options).unwrap();
    assert!(again.trim.trimmed);
    assert!(polars.len() <= kept + 2, "{} then {}", kept, polars.len());
}

#[test]
fn static_margin_follows_the_centre_of_gravity() {
    let mut polars = cache();
    let options = LatticeOptions::default();
    let mut a = trainer();
    let mac = layout(&a).unwrap().wing.mac;
    let r1 = analyze(&a, &flight(12.0), &airfoil, &mut polars, &options).unwrap();
    a.cg_mac_fraction += 0.1;
    let r2 = analyze(&a, &flight(12.0), &airfoil, &mut polars, &options).unwrap();
    let change = r1.stability.static_margin - r2.stability.static_margin;
    // Moving the CG back by 10 % of the chord removes about 10 % of margin
    // (the neutral point moves a little with the trim).
    assert!((change - 0.1).abs() < 0.01, "{change}");
    assert!(
        (r2.stability.center_of_gravity - r1.stability.center_of_gravity - 0.1 * mac).abs() < 1e-12
    );
    // A CG further back needs less down force on the tail: the elevator
    // moves trailing edge down.
    assert!(r2.trim.elevator_deg > r1.trim.elevator_deg);
}

#[test]
fn flying_too_slowly_stalls_the_wing() {
    let mut polars = cache();
    let r = analyze(
        &trainer(),
        &flight(5.0),
        &airfoil,
        &mut polars,
        &LatticeOptions::default(),
    )
    .unwrap();
    assert!(
        r.warnings.contains(&AnalysisWarning::WingStalled)
            || r.warnings.contains(&AnalysisWarning::CannotTrim),
        "{:?}",
        r.warnings
    );
}

#[test]
fn at_the_stall_speed_found_the_first_strip_is_at_its_maximum_lift() {
    let mut polars = cache();
    let options = LatticeOptions::default();
    let a = trainer();
    let r = analyze(&a, &flight(12.0), &airfoil, &mut polars, &options).unwrap();
    let stall = r.stall.unwrap();
    let at = analyze(&a, &flight(stall.speed), &airfoil, &mut polars, &options).unwrap();
    assert!((at.required_cl - stall.cl_max).abs() < 1e-9);
    // The most loaded wing strip, relative to its maximum, is right at it,
    // with the maximum at the Reynolds number of the stall speed.
    let worst = at
        .strips
        .iter()
        .filter(|s| s.part == SurfaceKind::Wing)
        .map(|s| s.cl / s.cl_max.unwrap())
        .fold(f64::NEG_INFINITY, f64::max);
    assert!((worst - 1.0).abs() < 0.01, "{worst}");
}

#[test]
fn airfoils_blend_between_the_sections_of_a_panel() {
    let mut polars = cache();
    let options = LatticeOptions::default();
    let mut a = trainer();
    a.wing.sections[0].airfoil = "naca:4412".into();
    a.wing.sections[1].airfoil = "naca:0009".into();
    let r = analyze(&a, &flight(12.0), &airfoil, &mut polars, &options).unwrap();
    let wing: Vec<_> = r
        .strips
        .iter()
        .filter(|s| s.part == SurfaceKind::Wing && !s.left)
        .collect();
    let (root, tip) = (wing.first().unwrap(), wing.last().unwrap());
    // The maximum lift of each strip is the blend of both airfoils' at its
    // Reynolds number, by its place along the panel.
    let max = |key: &str, re: f64| {
        let lo = 10f64.powf((re.log10() * 5.0).floor() / 5.0);
        let hi = lo * 10f64.powf(0.2);
        let mut p = cache();
        let (a, b) = (
            aero_mission::PolarSource::polar(&mut p, key, lo, 0.0).unwrap(),
            aero_mission::PolarSource::polar(&mut p, key, hi, 0.0).unwrap(),
        );
        let t = (re / lo).ln() / (hi / lo).ln();
        a.cl_max().unwrap() + t * (b.cl_max().unwrap() - a.cl_max().unwrap())
    };
    for s in [root, tip] {
        let f = s.span_fraction;
        let expected = (1.0 - f) * max("naca:4412", s.reynolds) + f * max("naca:0009", s.reynolds);
        assert!(
            (s.cl_max.unwrap() - expected).abs() < 1e-9,
            "{f}: {} vs {expected}",
            s.cl_max.unwrap()
        );
    }
    assert!(root.cl_max.unwrap() > tip.cl_max.unwrap() + 0.2);
}

#[test]
fn the_analysis_reads_the_trimmed_lattice_and_the_polars() {
    // A wing swept back 25°, where the lift normal to the sweep differs
    // from the lift referred to the flight.
    let mut a = trainer();
    let tip = &mut a.wing.sections[1];
    tip.x = tip.y * 25f64.to_radians().tan();
    let mut polars = cache();
    let options = LatticeOptions::default();
    let f = flight(12.0);
    let r = analyze(&a, &f, &airfoil, &mut polars, &options).unwrap();
    assert!(r.trim.trimmed);

    // The same lattice solved at the trimmed angle and elevator.
    let mach = 12.0 / f.atmosphere().unwrap().speed_of_sound;
    let lat = aero_aircraft::lattice(&a, &airfoil, &LatticeOptions { mach, ..options }).unwrap();
    let vl = VortexLattice::new(&lat.model).unwrap();
    let s = vl
        .solve(&FlowCondition {
            controls: vec![r.trim.elevator_deg],
            ..FlowCondition::alpha(r.trim.alpha_deg)
        })
        .unwrap();
    let d = &s.derivatives;
    assert!((r.drag.induced - s.totals.cd_trefftz).abs() < 1e-12);
    assert!((r.drag.span_efficiency - s.totals.span_efficiency).abs() < 1e-12);
    assert!((r.stability.cn_beta - d.beta.yaw).abs() < 1e-12);
    assert!((r.stability.cl_beta - d.beta.roll).abs() < 1e-12);
    assert!((r.stability.neutral_point - d.neutral_point.unwrap()).abs() < 1e-12);
    assert_eq!(r.strips.len(), s.strips.len());

    let l = layout(&a).unwrap();
    let nu = f.atmosphere().unwrap().kinematic_viscosity;
    let (mut wing, mut tail) = (0.0, 0.0);
    for (k, st) in r.strips.iter().enumerate() {
        let vlm = &s.strips[k];
        // The lift compared with the airfoil's maximum is the one normal to
        // the sweep.
        assert_eq!(st.cl, vlm.cl_perp);
        if st.part == SurfaceKind::Wing {
            assert!(vlm.cl_perp > 1.1 * vlm.cl, "{} {}", vlm.cl_perp, vlm.cl);
            // The profile drag: the NACA 2412 polars at the strip's
            // Reynolds number (interpolated in ln Re) and its lift.
            let re = 12.0 * vlm.chord / nu;
            let lo = 10f64.powf((re.log10() * 5.0 + 1e-9).floor() / 5.0);
            let hi = 10f64.powf(((lo.log10() * 5.0).round() + 1.0) / 5.0);
            let cd_lo = aero_mission::PolarSource::polar(&mut polars, "naca:2412", lo, 0.0)
                .unwrap()
                .cd_at(vlm.cl)
                .unwrap();
            let cd_hi = aero_mission::PolarSource::polar(&mut polars, "naca:2412", hi, 0.0)
                .unwrap()
                .cd_at(vlm.cl)
                .unwrap();
            let t = (re / lo).ln() / (hi / lo).ln();
            let expected = cd_lo + t * (cd_hi - cd_lo);
            assert!(
                (st.cd.unwrap() - expected).abs() < 1e-12,
                "{} vs {expected}",
                st.cd.unwrap()
            );
            wing += st.cd.unwrap() * st.area;
        } else {
            tail += st.cd.unwrap() * st.area;
        }
    }
    assert!((r.drag.wing_profile - wing / l.wing.area).abs() < 1e-12);
    assert!((r.drag.tail_profile - tail / l.wing.area).abs() < 1e-12);
}

#[test]
fn polars_stop_soon_after_the_stall() {
    let a = airfoil("naca:0012").unwrap();
    let settings = AnalysisSettings {
        alpha_step: 1.0,
        alpha_max: 25.0,
        alpha_min: -25.0,
        paneling: PanelingOptions {
            n_nodes: 120,
            ..PanelingOptions::default()
        },
        ..AnalysisSettings::default()
    };
    let p = section_polar(&a, 1e6, 0.0, &settings).unwrap();
    let top = p
        .points
        .iter()
        .max_by(|a, b| a.cl.total_cmp(&b.cl))
        .unwrap();
    let bottom = p
        .points
        .iter()
        .min_by(|a, b| a.cl.total_cmp(&b.cl))
        .unwrap();
    let (first, last) = (p.points.first().unwrap(), p.points.last().unwrap());
    // A few points past each stall, not up to the limits of the sweep.
    assert!(
        last.alpha_deg <= top.alpha_deg + 5.0 && last.alpha_deg < 25.0,
        "{:?}",
        p.points
    );
    assert!(first.alpha_deg >= bottom.alpha_deg - 5.0 && first.alpha_deg > -25.0);
    // Symmetric airfoil: symmetric polar.
    assert!((top.cl + bottom.cl).abs() < 0.05);
}
