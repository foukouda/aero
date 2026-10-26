//! Compares the airfoil modifications (`aero_geometry::modify`) with the
//! airfoils written by XFOIL 6.996 after the same `GDES` commands or `INTE`
//! (`tests/geometry`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use aero_geometry::{Airfoil, NacaOptions, apply_gdes, interpolate, naca, parse_dat};

/// XFOIL writes the coordinates with 7 significant digits (`G15.7`), a
/// rounding error of up to 5e-8 for coordinates below 1: the port must agree
/// to that precision, with the same points added or removed at the flap
/// breaks.
const TOLERANCE: f64 = 2e-7;

fn geometry_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/geometry")
}

fn cases() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(geometry_dir())
        .expect("tests/geometry exists")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("case.toml").exists())
        .collect();
    dirs.sort();
    dirs
}

fn read_file(dir: &Path, file: &str) -> Airfoil {
    let text = std::fs::read_to_string(dir.join(file)).expect("airfoil file");
    parse_dat(&text, file).expect("parse").0
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

/// XFOIL's `ABCOPY` (`EXEC`, `PCOP`) removes exactly doubled points.
fn without_doubled_points(a: &Airfoil) -> Vec<[f64; 2]> {
    let mut out: Vec<[f64; 2]> = Vec::with_capacity(a.len());
    for p in a.points() {
        if out.last() != Some(&p) {
            out.push(p);
        }
    }
    out
}

#[test]
fn edits_match_xfoil() {
    let mut count = 0;
    let mut worst: f64 = 0.0;
    let mut failures = Vec::new();
    for dir in cases() {
        let case: toml::Table = std::fs::read_to_string(dir.join("case.toml"))
            .unwrap()
            .parse()
            .unwrap();
        let airfoil = case["airfoil"].as_table().unwrap();
        let base = match airfoil["kind"].as_str().unwrap() {
            "naca" => naca(airfoil["code"].as_str().unwrap(), &NacaOptions::default()).unwrap(),
            "file" => read_file(&dir, airfoil["file"].as_str().unwrap()),
            other => panic!("unknown kind {other}"),
        };
        for edit in case["edits"].as_array().unwrap() {
            let edit = edit.as_table().unwrap();
            let name = edit["name"].as_str().unwrap();
            let tag = format!("{} {name}", dir.file_name().unwrap().to_string_lossy());
            let ours = if let Some(commands) = edit.get("commands") {
                let lines: Vec<&str> = commands
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .collect();
                apply_gdes(&base, &lines)
            } else {
                let inte = edit["interpolate"].as_table().unwrap();
                let other = read_file(&dir, inte["with"].as_str().unwrap());
                let fraction = inte["fraction"].as_float().unwrap();
                interpolate(&base, &other, fraction)
            }
            .unwrap_or_else(|e| panic!("{tag}: {e}"));
            let ours = without_doubled_points(&ours);
            let theirs = read_saved(&dir.join(format!("edit_{name}.dat")));
            count += 1;
            if ours.len() != theirs.len() {
                failures.push(format!(
                    "{tag}: {} points, XFOIL {}",
                    ours.len(),
                    theirs.len()
                ));
                continue;
            }
            let diff = ours
                .iter()
                .zip(&theirs)
                .map(|(a, b)| (a[0] - b[0]).abs().max((a[1] - b[1]).abs()))
                .fold(0.0, f64::max);
            println!("{tag:28} {:4} points  max diff {diff:.2e}", ours.len());
            worst = worst.max(diff);
            if diff > TOLERANCE {
                failures.push(format!("{tag}: max coordinate difference {diff:.2e}"));
            }
        }
    }
    println!("{count} edits, worst {worst:.2e}");
    assert_eq!(count, 17, "all the edits of tests/geometry");
    assert!(failures.is_empty(), "{failures:#?}");
}
