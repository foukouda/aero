//! Compares the inverse design methods (`aero_foil2d::inverse`) with the
//! airfoils written by XFOIL 6.996 after the same `QDES` sessions
//! (`tests/inverse`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::Path;

use aero_foil2d::inverse::{MixedInverseOptions, SpeedTarget, mixed_inverse};
use aero_geometry::{Airfoil, panel};
use common::{Case, cases_in, inverse_dir};

/// XFOIL writes the coordinates with 7 significant digits (`G15.7`), a
/// rounding error of up to 5e-8 for coordinates below 1.
const TOLERANCE: f64 = 2e-7;

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

fn compare(case: &Case, name: &str, ours: &Airfoil) -> f64 {
    let xfoil = read_saved(&case.dir.join(format!("inv_{name}.dat")));
    assert_eq!(xfoil.len(), ours.len(), "{}/{name}: node count", case.name);
    let worst = ours
        .points()
        .zip(&xfoil)
        .map(|(p, q)| (p[0] - q[0]).abs().max((p[1] - q[1]).abs()))
        .fold(0.0, f64::max);
    println!("{}/{name}: max |Δ| = {worst:.2e}", case.name);
    worst
}

#[test]
fn mixed_inverse_matches_xfoil() {
    let mut runs = 0;
    let mut failures = Vec::new();
    for case in cases_in(&inverse_dir()) {
        let paneled = panel(&case.buffer_airfoil(), &case.paneling()).unwrap();
        for run in case.manifest["inverse"].as_array().unwrap() {
            let run = run.as_table().unwrap();
            let Some(commands) = run.get("qdes").and_then(|v| v.as_array()) else {
                continue;
            };
            let name = run["name"].as_str().unwrap();
            let alphas: Vec<f64> = run["alphas"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_float().unwrap())
                .collect();
            let commands: Vec<&str> = commands.iter().map(|v| v.as_str().unwrap()).collect();
            let (ours, iterations) = run_qdes(&paneled, &alphas, &commands);
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
            if compare(&case, name, &ours) > TOLERANCE {
                failures.push(format!("{}/{name}", case.name));
            }
            runs += 1;
        }
    }
    assert_eq!(runs, 7, "QDES runs");
    assert!(
        failures.is_empty(),
        "differences above {TOLERANCE}: {failures:?}"
    );
}
