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

use common::cases_in;
use common::viscous::{
    LogComparison, TOLERANCES, ViscousStats, check_viscous_case, compare_logs, viscous_dir,
};

#[test]
fn viscous_cases_match_xfoil() {
    let cases = cases_in(&viscous_dir());
    assert_eq!(cases.len(), 10);
    let mut total = ViscousStats::default();
    let mut failures = Vec::new();
    for case in &cases {
        let (stats, f) = check_viscous_case(case);
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

#[test]
fn console_logs_match_xfoil() {
    // XFOIL's console output (iteration history, transition points,
    // marching failures) is reproduced line by line; where the iterations go
    // through unstable phases (a point needing all its iterations, an
    // unconverged point) the two paths may separate by round-off, which must
    // be explained by our own sensitivity.
    for case in cases_in(&viscous_dir()) {
        for run in case.viscous_runs() {
            match compare_logs(&case, &run) {
                LogComparison::Identical { lines } => assert!(lines > 50, "{}", case.name),
                LogComparison::RoundOff {
                    line, ours, xfoil, ..
                } => {
                    println!(
                        "{} {}: round-off divergence at line {line}: `{}` vs `{}`",
                        case.name,
                        run.name,
                        ours.trim(),
                        xfoil.trim()
                    );
                }
                other => panic!("{} {}: {other:?}", case.name, run.name),
            }
        }
    }
}
