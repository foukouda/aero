//! Recommendations on real airfoils at a Reynolds number of about 1e5,
//! where the viscous solver is checked against XFOIL (`tests/viscous`) and
//! the E387 against wind-tunnel measurements (`tests/wind_tunnel`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use aero_foil2d::viscous::{ViscousOptions, ViscousSolver};
use aero_geometry::{Airfoil, PanelingOptions, parse_dat};
use aero_mission::{
    AnalysisSettings, Conditions, Fit, Flight, Goal, Wing, WingModel, evaluate, rank,
};

/// Coordinate files of the XFOIL reference cases.
const AIRFOILS: [(&str, &str); 5] = [
    ("e387", "e387/e387.dat"),
    ("sd7037", "sd7037_cl/sd7037.dat"),
    ("mh32", "mh32_re5e4/mh32.dat"),
    ("s1223", "s1223/s1223.dat"),
    ("clarky", "clarky/clarky.dat"),
];

fn airfoil(file: &str) -> Airfoil {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/viscous")
        .join(file);
    parse_dat(&std::fs::read_to_string(path).unwrap(), file)
        .unwrap()
        .0
}

/// 26.5 km/h at 100 m with a 1.2 m × 20 cm wing: Re ≈ 1e5.
fn conditions(mass: f64) -> Conditions {
    let flight = Flight {
        speed: 7.36,
        altitude: 100.0,
        temperature_offset: 0.0,
        mass,
    };
    flight
        .conditions(&Wing {
            span: 1.2,
            chord: 0.2,
        })
        .unwrap()
}

fn evaluate_all(conditions: &Conditions) -> Vec<(&'static str, aero_mission::AirfoilEvaluation)> {
    let settings = AnalysisSettings::default();
    let model = WingModel::default();
    std::thread::scope(|scope| {
        let jobs: Vec<_> = AIRFOILS
            .iter()
            .map(|&(id, file)| {
                scope.spawn(move || {
                    (
                        id,
                        evaluate(&airfoil(file), conditions, &settings, &model).unwrap(),
                    )
                })
            })
            .collect();
        jobs.into_iter().map(|j| j.join().unwrap()).collect()
    })
}

fn report(conditions: &Conditions, evaluations: &[(&str, aero_mission::AirfoilEvaluation)]) {
    println!(
        "Re {:.0}, required Cl {:.2}",
        conditions.reynolds, conditions.required_cl
    );
    for (id, e) in evaluations {
        println!(
            "  {id:7} Cl max {:5.2} at {:4.1}°  Cd {:7.4}  {:?}  easy to build: {} (t/c {:.3}, undercamber {:+.3})",
            e.cl_max.map_or(f64::NAN, |p| p.cl),
            e.cl_max.map_or(f64::NAN, |p| p.alpha_deg),
            e.operating_point.map_or(f64::NAN, |p| p.cd),
            e.fit,
            e.buildability.easy,
            e.buildability.thickness,
            e.buildability.undercamber,
        );
    }
}

#[test]
fn a_light_model_gets_a_low_drag_section() {
    // 450 g: Cl ≈ 0.57.
    let c = conditions(0.45);
    assert!((c.reynolds - 1e5).abs() < 1e3);
    let evaluations = evaluate_all(&c);
    report(&c, &evaluations);
    let all: Vec<_> = evaluations.iter().map(|(_, e)| e.clone()).collect();
    let best = evaluations[rank(&all, Goal::LeastDrag)[0]].0;
    assert!(["e387", "sd7037", "mh32"].contains(&best), "{best}");
    // The high-lift section is not the answer for a light model.
    let s1223 = &evaluations.iter().find(|(id, _)| *id == "s1223").unwrap().1;
    let lowest = all
        .iter()
        .filter_map(|e| e.operating_point)
        .map(|p| p.cd)
        .fold(f64::INFINITY, f64::min);
    assert!(s1223.operating_point.is_none_or(|p| p.cd > 1.2 * lowest));
    // The undercambered S1223 is hard to build, the flat-bottomed Clark Y easy.
    let easy = |id: &str| {
        evaluations
            .iter()
            .find(|(i, _)| *i == id)
            .unwrap()
            .1
            .buildability
            .easy
    };
    assert!(!easy("s1223") && easy("clarky"));
}

#[test]
fn a_heavy_model_needs_the_high_lift_section() {
    // 1.1 kg: Cl ≈ 1.39, beyond what the low-drag sections can carry.
    let c = conditions(1.1);
    let evaluations = evaluate_all(&c);
    report(&c, &evaluations);
    let all: Vec<_> = evaluations.iter().map(|(_, e)| e.clone()).collect();
    for goal in [Goal::LeastDrag, Goal::SlowFlight] {
        assert_eq!(evaluations[rank(&all, goal)[0]].0, "s1223", "{goal:?}");
    }
    for (id, e) in &evaluations {
        if *id == "s1223" {
            assert!(e.fit.suited(), "{:?}", e.fit);
        } else {
            assert!(
                matches!(e.fit, Fit::NearStall | Fit::CannotFly),
                "{id}: {:?}",
                e.fit
            );
        }
    }
}

#[test]
fn operating_point_matches_a_direct_solution() {
    // The interpolated drag at the required lift coefficient agrees with a
    // viscous solution at that lift coefficient.
    let c = conditions(0.45);
    let e387 = airfoil("e387/e387.dat");
    let e = evaluate(
        &e387,
        &c,
        &AnalysisSettings::default(),
        &WingModel::default(),
    )
    .unwrap();
    let op = e.operating_point.unwrap();
    let paneled = e387.paneled(&PanelingOptions::default()).unwrap();
    let options = ViscousOptions {
        reynolds: c.reynolds,
        mach: c.mach,
        ..ViscousOptions::default()
    };
    let direct = ViscousSolver::new(&paneled, &options)
        .unwrap()
        .solve_cl(c.required_cl)
        .unwrap();
    assert!(direct.converged);
    println!(
        "interpolated Cd {:.5} at {:.2}°, direct {:.5} at {:.2}°",
        op.cd, op.alpha_deg, direct.cd, direct.alpha_deg
    );
    assert!((op.cd - direct.cd).abs() < 0.03 * direct.cd);
    assert!((op.alpha_deg - direct.alpha_deg).abs() < 0.1);
}

#[test]
fn a_fast_flight_needs_negative_angles() {
    // 120 g on the same wing and speed: Cl ≈ 0.15, below the E387's lift
    // at 0° (about 0.41), so the polar is swept downward to reach it.
    let c = conditions(0.12);
    assert!(c.required_cl < 0.2);
    let e = evaluate(
        &airfoil("e387/e387.dat"),
        &c,
        &AnalysisSettings::default(),
        &WingModel::default(),
    )
    .unwrap();
    let op = e.operating_point.unwrap();
    println!("Cl {:.3} at {:.2}°, Cd {:.4}", op.cl, op.alpha_deg, op.cd);
    assert!(op.alpha_deg < -2.0);
    assert_eq!(e.fit, Fit::Comfortable);
    // The downward branch stops once below the required lift.
    let negative = e.polar.iter().filter(|p| p.alpha_deg < 0.0).count();
    assert!(negative >= 4 && e.polar[0].cl < c.required_cl);
    assert!(e.polar[1].cl > c.required_cl);
}
