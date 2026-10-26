//! Physics check against wind-tunnel measurements (`tests/wind_tunnel`):
//! the Eppler 387 in the NASA Langley Low-Turbulence Pressure Tunnel at
//! Reynolds numbers of 100 000 and 200 000 (NASA TM-4062).
//!
//! This is not an equivalence test like the XFOIL comparisons: XFOIL's
//! method itself differs from a real flow (transition model, laminar
//! separation bubbles, tunnel corrections). It documents how close the
//! predictions are where a beginner's slow model flies, and guards that
//! agreement.
//!
//! Run with `--nocapture` to see the comparison.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use aero_foil2d::viscous::{ViscousOptions, ViscousSolution, ViscousSolver};
use aero_geometry::{PanelingOptions, parse_dat};

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/wind_tunnel")
}

/// Rows `alpha, cl, cd` of a measurement file (`cd` absent beyond stall).
fn measurements(file: &str) -> Vec<(f64, f64, Option<f64>)> {
    let text = std::fs::read_to_string(dir().join(file)).unwrap();
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("alpha"))
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            let cd = f[2].trim();
            (
                f[0].parse().unwrap(),
                f[1].parse().unwrap(),
                (!cd.is_empty()).then(|| cd.parse().unwrap()),
            )
        })
        .collect()
}

/// Our viscous solutions at the measured angles, swept from 0° outward
/// both ways as the application does (each point from the previous one).
fn solutions(reynolds: f64, mach: f64, alphas: &[f64]) -> BTreeMap<String, ViscousSolution> {
    let text = std::fs::read_to_string(dir().join("e387.dat")).unwrap();
    let airfoil = parse_dat(&text, "e387").unwrap().0;
    let paneled = airfoil.paneled(&PanelingOptions::default()).unwrap();
    let options = ViscousOptions {
        reynolds,
        mach,
        ..ViscousOptions::default()
    };
    let solver = ViscousSolver::new(&paneled, &options).unwrap();
    let mut unique: Vec<f64> = alphas.to_vec();
    unique.sort_by(f64::total_cmp);
    unique.dedup();
    let up = unique.iter().copied().filter(|a| *a >= -0.5);
    let down = unique.iter().rev().copied().filter(|a| *a < -0.5);
    let mut out = BTreeMap::new();
    for branch in [up.collect::<Vec<_>>(), down.collect()] {
        let mut session = solver.session();
        for alpha in branch {
            if let Ok(s) = session.solve(alpha)
                && s.converged
            {
                out.insert(format!("{alpha:.2}"), s);
            }
        }
    }
    out
}

struct Agreement {
    points: usize,
    worst_cl: f64,
    worst_cd_ratio: f64,
}

fn compare(file: &str, reynolds: f64, mach: f64, alpha_max: f64) -> Agreement {
    let rows = measurements(file);
    let alphas: Vec<f64> = rows.iter().map(|r| r.0).collect();
    let ours = solutions(reynolds, mach, &alphas);
    println!("{file}: Re {reynolds:e}, M {mach}");
    println!("  alpha   Cl exp  Cl ours   Cd exp   Cd ours");
    let mut agreement = Agreement {
        points: 0,
        worst_cl: 0.0,
        worst_cd_ratio: 1.0,
    };
    for (alpha, cl, cd) in rows {
        let Some(s) = ours.get(&format!("{alpha:.2}")) else {
            println!("{alpha:7.2}  {cl:7.3}  (not converged)");
            continue;
        };
        let cd_text = cd.map_or("    —   ".to_string(), |c| format!("{c:8.4}"));
        println!(
            "{alpha:7.2}  {cl:7.3}  {:7.3}  {cd_text}  {:8.4}",
            s.cl, s.cd
        );
        if alpha > alpha_max {
            continue;
        }
        let Some(cd) = cd else { continue };
        agreement.points += 1;
        agreement.worst_cl = agreement.worst_cl.max((s.cl - cl).abs());
        let ratio = (s.cd / cd).max(cd / s.cd);
        agreement.worst_cd_ratio = agreement.worst_cd_ratio.max(ratio);
    }
    println!(
        "  up to {alpha_max}°: {} points, worst |ΔCl| {:.3}, worst Cd ratio {:.2}",
        agreement.points, agreement.worst_cl, agreement.worst_cd_ratio
    );
    agreement
}

#[test]
fn eppler_387_in_the_langley_tunnel() {
    // Below 9°, before the stall. Observed on 2026-10-26: |ΔCl| up to 0.067
    // and 0.059 (the lift is over-predicted, as usual for XFOIL), Cd within
    // 22 % and 25 %. The bounds guard that agreement.
    for (file, reynolds, mach, min_points) in [
        ("e387_ltpt_re1e5.csv", 1e5, 0.03, 40),
        ("e387_ltpt_re2e5.csv", 2e5, 0.06, 22),
    ] {
        let a = compare(file, reynolds, mach, 9.1);
        assert!(a.points >= min_points, "{file}: {} points", a.points);
        assert!(a.worst_cl <= 0.08, "{file}: |ΔCl| {}", a.worst_cl);
        assert!(
            a.worst_cd_ratio <= 1.3,
            "{file}: Cd ratio {}",
            a.worst_cd_ratio
        );
    }
}
