//! Compares the inverse design methods (`aero_foil2d::inverse`) with the
//! airfoils written by XFOIL 6.996 after the same `QDES` and `MDES` sessions
//! (`tests/inverse`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::Path;

use aero_foil2d::inverse::{FullInverse, MixedInverseOptions, SpeedTarget, mixed_inverse};
use aero_geometry::{Airfoil, panel};
use common::{Case, cases_in, inverse_dir};

/// XFOIL writes the coordinates with 7 significant digits (`G15.7`): a
/// rounding error of up to 5e-8 below 1, 5e-7 from 1 to 10. The port must
/// agree to `max(2e-7, 1e-6·|v|)`.
fn tolerance(v: f64) -> f64 {
    (1e-6 * v.abs()).max(2e-7)
}

/// Coordinates of an XFOIL `SAVE` file: a name line, then x y pairs.
fn read_saved(path: &Path) -> Vec<[f64; 2]> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .lines()
        .skip(1)
        .filter_map(|l| {
            let v: Vec<f64> = l
                .split_whitespace()
                .filter_map(|t| t.parse().ok())
                .collect();
            (v.len() == 2).then(|| [v[0], v[1]])
        })
        .collect()
}

/// The `QDES` session of a run: the target taken at the first angle, the
/// commands executed at the last one. Returns the new airfoil and the
/// number of Newton iterations.
fn run_qdes(paneled: &Airfoil, alphas: &[f64], commands: &[&str]) -> (Airfoil, usize) {
    let mut target = SpeedTarget::from_airfoil(paneled, alphas[0]).unwrap();
    let alpha = *alphas.last().unwrap();
    let mut options = MixedInverseOptions {
        first: 0,
        last: paneled.len() - 1,
        curvature_constraints: false,
        max_iterations: 0,
    };
    let mut slope_matching = true;
    let mut result = None;
    let mut lines = commands.iter();
    while let Some(line) = lines.next() {
        let mut words = line.split_whitespace();
        match words.next().unwrap() {
            "MARK" => {
                // XFOIL's node indices are 1-based.
                let mut index = || lines.next().unwrap().trim().parse::<usize>().unwrap() - 1;
                options.first = index();
                options.last = index();
            }
            "SMOO" => target
                .smooth(options.first, options.last, slope_matching)
                .unwrap(),
            "SLOP" => slope_matching = !slope_matching,
            "CPXX" => options.curvature_constraints = !options.curvature_constraints,
            "EXEC" => {
                options.max_iterations = words.next().unwrap().parse().unwrap();
                let r = mixed_inverse(paneled, alpha, &target.speed, &options).unwrap();
                assert!(r.converged, "not converged in {} iterations", r.iterations);
                result = Some((r.airfoil, r.iterations));
            }
            other => panic!("unsupported QDES command {other}"),
        }
    }
    result.expect("an EXEC command")
}

/// Compares with XFOIL's airfoil; true if every coordinate agrees within
/// [`tolerance`].
fn matches(case: &Case, name: &str, ours: &Airfoil) -> bool {
    let xfoil = read_saved(&case.dir.join(format!("inv_{name}.dat")));
    assert_eq!(xfoil.len(), ours.len(), "{}/{name}: node count", case.name);
    let mut worst = 0.0_f64;
    let mut ok = true;
    for (i, (p, q)) in ours.points().zip(&xfoil).enumerate() {
        for k in 0..2 {
            let d = (p[k] - q[k]).abs();
            worst = worst.max(d);
            if d > tolerance(q[k]) {
                ok = false;
                println!("  node {i}: ours {p:?}, XFOIL {q:?}");
            }
        }
    }
    println!("{}/{name}: max |Δ| = {worst:.2e}", case.name);
    ok
}

/// The `MDES` session of a run, at its single angle. Returns the new
/// airfoil and the corrections of `C₁` made by `MAPGEN`.
fn run_mdes(paneled: &Airfoil, alphas: &[f64], commands: &[&str]) -> (Airfoil, Vec<f64>) {
    assert_eq!(alphas.len(), 1, "one angle");
    let mut design = FullInverse::new(paneled, alphas[0]).unwrap();
    let mut result = None;
    for line in commands {
        let mut words = line.split_whitespace();
        let command = words.next().unwrap();
        let numbers: Vec<f64> = words.map(|w| w.parse().unwrap()).collect();
        match command {
            "FILT" => design.filter(),
            "SYMM" => design.set_symmetric(!design.is_symmetric()),
            "TGAP" => design.set_te_gap(numbers[0], numbers[1]).unwrap(),
            "TANG" => design.set_te_angle(numbers[0]).unwrap(),
            "AQ" => design.set_alphas(&numbers).unwrap(),
            "CQ" => design.set_lift_coefficients(&numbers).unwrap(),
            // Without MARK, the target segment is the whole airfoil.
            "SMOO" => design.smooth(0, 256, true).unwrap(),
            "EXEC" => {
                let r = design.execute().unwrap();
                assert!(r.converged, "MAPGEN not converged");
                result = Some((r.airfoil, r.corrections));
            }
            other => panic!("unsupported MDES command {other}"),
        }
    }
    result.expect("an EXEC command")
}

/// The `C₁` corrections XFOIL prints at each `MAPGEN` iteration
/// (`ITERCN, DCNMAX`, list-directed).
fn mapgen_corrections(log: &str) -> Vec<f64> {
    log.lines()
        .filter_map(|l| {
            let words: Vec<&str> = l.rsplit("c>").next().unwrap().split_whitespace().collect();
            (words.len() == 2 && words[0].parse::<usize>().is_ok() && words[1].contains('E'))
                .then(|| words[1].parse().unwrap())
        })
        .collect()
}

/// One inverse design session of a case.
struct Session {
    case: Case,
    paneled: Airfoil,
    name: String,
    alphas: Vec<f64>,
    commands: Vec<String>,
}

impl Session {
    fn commands(&self) -> Vec<&str> {
        self.commands.iter().map(String::as_str).collect()
    }
}

/// Sessions of the cases using the design menu `menu` (`qdes` or `mdes`).
fn sessions(menu: &str) -> Vec<Session> {
    let mut out = Vec::new();
    for case in cases_in(&inverse_dir()) {
        let paneled = panel(&case.buffer_airfoil(), &case.paneling()).unwrap();
        for run in case.manifest["inverse"].as_array().unwrap() {
            let run = run.as_table().unwrap();
            let Some(commands) = run.get(menu).and_then(|v| v.as_array()) else {
                continue;
            };
            out.push(Session {
                case: Case {
                    name: case.name.clone(),
                    dir: case.dir.clone(),
                    manifest: case.manifest.clone(),
                },
                paneled: paneled.clone(),
                name: run["name"].as_str().unwrap().to_string(),
                alphas: run["alphas"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_float().unwrap())
                    .collect(),
                commands: commands
                    .iter()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect(),
            });
        }
    }
    out
}

#[test]
fn full_inverse_matches_xfoil() {
    let mut failures = Vec::new();
    let sessions = sessions("mdes");
    for s in &sessions {
        let (case, name) = (&s.case, s.name.as_str());
        let (ours, corrections) = run_mdes(&s.paneled, &s.alphas, &s.commands());
        let log = std::fs::read_to_string(case.dir.join(format!("inv_{name}.log"))).unwrap();
        let xfoil = mapgen_corrections(&log);
        println!(
            "{}/{name}: C1 corrections {corrections:?}, XFOIL {xfoil:?}",
            case.name
        );
        assert_eq!(
            corrections.len(),
            xfoil.len(),
            "{}/{name}: iterations",
            case.name
        );
        for (a, b) in corrections.iter().zip(&xfoil) {
            assert!(
                (a - b).abs() <= 1e-6 * b.abs(),
                "{}/{name}: C1 correction {a} instead of {b}",
                case.name
            );
        }
        if !matches(case, name, &ours) {
            failures.push(format!("{}/{name}", case.name));
        }
    }
    assert_eq!(sessions.len(), 6, "MDES sessions");
    assert!(
        failures.is_empty(),
        "differences above the output precision: {failures:?}"
    );
}

#[test]
fn mixed_inverse_matches_xfoil() {
    let mut failures = Vec::new();
    let sessions = sessions("qdes");
    for s in &sessions {
        let (case, name) = (&s.case, s.name.as_str());
        let (ours, iterations) = run_qdes(&s.paneled, &s.alphas, &s.commands());
        // XFOIL prints one "dNmax" line per Newton iteration.
        let log = std::fs::read_to_string(case.dir.join(format!("inv_{name}.log"))).unwrap();
        let xfoil_iterations = log
            .lines()
            .filter(|l| l.trim_start().starts_with("dNmax"))
            .count();
        assert_eq!(
            iterations, xfoil_iterations,
            "{}/{name}: iterations",
            case.name
        );
        if !matches(case, name, &ours) {
            failures.push(format!("{}/{name}", case.name));
        }
    }
    assert_eq!(sessions.len(), 7, "QDES sessions");
    assert!(
        failures.is_empty(),
        "differences above the output precision: {failures:?}"
    );
}
