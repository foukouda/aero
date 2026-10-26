//! Validation of the viscous solver against XFOIL 6.996 (`tests/viscous`).
//!
//! Every case is run as XFOIL ran it: the polar as one warm-started sweep
//! (`ASEQ`), the single points from a fresh boundary layer (`INIT`, `ALFA`).
//! The same points must converge; forces, transition points and the
//! boundary layer at every station are compared with XFOIL's
//! full-precision dump files, and the other polar columns (CDp, Cpmin) with
//! the printed polar files.
//!
//! Run with `--nocapture` to see the differences per case.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::ops::ControlFlow;

use aero_foil2d::inviscid::{InviscidOptions, InviscidSolver};
use aero_foil2d::viscous::{ViscousSolution, ViscousSolver};
use common::viscous::{
    ViscousRun, ViscousStats, compare_viscous, parse_viscous_polar, read_dump, viscous_dir,
};
use common::{Case, cases_in, close};

/// Largest accepted differences with the dump files (scaled as described
/// in `ViscousStats`): forces, transition x/c, boundary layer. Most points
/// agree with XFOIL to 1e-10 or better. Round-off differences (which also
/// differ between debug and release builds) grow on points that need all
/// their Newton iterations, and Cf right at the transition point follows the
/// tiny shifts of the transition location.
const TOLERANCES: [f64; 3] = [1.0e-6, 1.0e-5, 2.0e-4];

/// Half a unit of the last printed digit, plus round-off.
const PRINTED_4: f64 = 5.1e-5;
const PRINTED_5: f64 = 5.1e-6;

fn solver(case: &Case, run: &ViscousRun) -> ViscousSolver {
    // XFOIL never refuses a supercritical point.
    let inviscid = InviscidSolver::new(
        &case.our_panels(),
        &InviscidOptions {
            allow_supercritical: true,
            ..InviscidOptions::default()
        },
    )
    .unwrap();
    ViscousSolver::from_inviscid(inviscid, &run.options).unwrap()
}

/// Runs one case; returns its statistics and the list of mismatches.
fn check_case(case: &Case) -> (ViscousStats, Vec<String>) {
    let mut stats = ViscousStats::default();
    let mut failures = Vec::new();
    for run in case.viscous_runs() {
        let solver = solver(case, &run);
        let file = |prefix: &str, ext: &str| case.dir.join(format!("{prefix}_{}.{ext}", run.name));

        // The polar: one warm-started sweep.
        let mut converged: Vec<ViscousSolution> = Vec::new();
        let polar = solver
            .sweep_with(&run.polar, |_, s| {
                if let Some(s) = s.filter(|s| s.converged) {
                    converged.push(s.clone());
                }
                ControlFlow::Continue(())
            })
            .unwrap();
        assert!(
            polar
                .failures
                .iter()
                .all(|f| f.message.starts_with("not converged"))
        );
        let dump = read_dump(&file("vdump", "bin"));
        let rows = parse_viscous_polar(&std::fs::read_to_string(file("vpolar", "txt")).unwrap());
        let ours: Vec<f64> = converged.iter().map(|s| s.alpha_deg).collect();
        let theirs: Vec<f64> = dump.iter().map(|d| d.alpha).collect();
        if ours.len() != theirs.len() || ours.iter().zip(&theirs).any(|(a, b)| (a - b).abs() > 1e-9)
        {
            failures.push(format!(
                "{} {}: converged points {ours:?}, XFOIL {theirs:?}",
                case.name, run.name
            ));
            continue;
        }
        for ((s, d), row) in converged.iter().zip(&dump).zip(&rows) {
            let tag = format!("{} {} polar a={}", case.name, run.name, d.alpha);
            compare_viscous(s, d, &tag, &mut stats);
            for (name, a, b, tol) in [
                ("CDp", s.cdp, row.cdp, PRINTED_5),
                ("Cpmin", s.cp_min, row.cp_min, PRINTED_4),
                ("CD", s.cd, row.cd, PRINTED_5),
            ] {
                if !close(a, b, tol, 0.0) {
                    failures.push(format!("{tag}: {name} {a:.6} vs XFOIL {b:.6}"));
                }
            }
        }

        // The single points, each from a fresh boundary layer.
        let mut dump = read_dump(&file("vpdump", "bin")).into_iter();
        let mut next = dump.next();
        for &alpha in &run.points {
            let s = solver.solve(alpha).unwrap();
            let tag = format!("{} {} point a={alpha}", case.name, run.name);
            match &next {
                Some(d) if s.converged && (d.alpha - alpha).abs() < 1e-9 => {
                    compare_viscous(&s, d, &tag, &mut stats);
                    next = dump.next();
                }
                Some(d) if !s.converged && (d.alpha - alpha).abs() > 1e-9 => {}
                None if !s.converged => {}
                _ => failures.push(format!(
                    "{tag}: converged {}, XFOIL's next converged point {:?}",
                    s.converged,
                    next.as_ref().map(|d| d.alpha)
                )),
            }
        }
        if next.is_some() {
            failures.push(format!(
                "{} {}: XFOIL has more single points",
                case.name, run.name
            ));
        }
    }
    (stats, failures)
}

#[test]
fn viscous_cases_match_xfoil() {
    let cases = cases_in(&viscous_dir());
    assert_eq!(cases.len(), 10);
    let mut total = ViscousStats::default();
    let mut failures = Vec::new();
    for case in &cases {
        let (stats, f) = check_case(case);
        println!(
            "{:16} {:3} points  forces {:9.2e}  xtr {:9.2e}  BL {:9.2e}  [{}]",
            case.name,
            stats.points,
            stats.forces.diff,
            stats.transition.diff,
            stats.boundary_layer.diff,
            stats.boundary_layer.what
        );
        total.merge(&stats);
        failures.extend(f);
    }
    println!(
        "total: {} points, worst {:.2e}",
        total.points,
        total.worst()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    let worst = [
        total.forces.diff,
        total.transition.diff,
        total.boundary_layer.diff,
    ];
    assert!(
        worst.iter().zip(TOLERANCES).all(|(w, t)| *w <= t),
        "{total:#?}"
    );
}
