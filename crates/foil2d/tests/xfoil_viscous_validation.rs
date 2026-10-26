//! Extended validation of the viscous solver against XFOIL 6.996 on random
//! analyses (`tests/viscous_validation`, built by
//! `tests/viscous_validation/build_cases.py`): random airfoils, paneling,
//! Reynolds and Mach numbers, Ncrit, trips and angles of attack, compared
//! as in `xfoil_viscous.rs` (same converged points, forces, transition,
//! boundary layer at every station).
//!
//! Some random analyses are hard for XFOIL itself (separated flows, points
//! that do not converge): the Newton iterations then go through unstable
//! phases where the last-bit differences between two implementations (math
//! libraries, operation order) grow until the two codes follow different
//! paths. Such a case is accepted only if our console log is identical to
//! XFOIL's, line by line, up to a first difference no larger than the
//! change of that line when our own computation is perturbed by 1e-15 to
//! 1e-13 (see `LogComparison::RoundOff`); any other difference fails.
//!
//! Set `AERO_VALIDATION_REPORT=1` to write
//! `tests/viscous_validation/REPORT.md`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::viscous::{
    LogComparison, TOLERANCES, check_viscous_case, compare_logs, viscous_validation_dir,
};
use std::ops::ControlFlow;

use common::{cases_in, par_map};

/// The validation set must not silently shrink.
const MIN_CASES: usize = 45;

/// Most cases must agree on every point.
const MIN_EXACT: usize = 30;

#[test]
fn random_viscous_analyses_match_xfoil() {
    let root = viscous_validation_dir();
    let cases = cases_in(&root);
    let mut failures = Vec::new();
    let mut rows = Vec::new();
    let mut xfoil_failed = Vec::new();
    let mut points = 0;
    let mut exact = 0;
    let mut diverging = Vec::new();
    // The comparisons, in parallel (cases on which XFOIL failed are skipped).
    let results = par_map(&cases, |case| {
        case.xfoil_failure().is_none().then(|| {
            let run = &case.viscous_runs()[0];
            (check_viscous_case(case), compare_logs(case, run))
        })
    });
    for (case, result) in cases.iter().zip(results) {
        let Some(((stats, mut f), log)) = result else {
            let reason = case.xfoil_failure().unwrap_or_default();
            xfoil_failed.push(format!("{}: {}", case.name, reason.trim()));
            continue;
        };
        let worst = [
            stats.forces.diff,
            stats.transition.diff,
            stats.boundary_layer.diff,
        ];
        for ((w, t), name) in worst
            .iter()
            .zip(TOLERANCES)
            .zip(["forces", "transition", "BL"])
        {
            if *w > t {
                let what = match name {
                    "forces" => &stats.forces.what,
                    "transition" => &stats.transition.what,
                    _ => &stats.boundary_layer.what,
                };
                f.push(format!("{}: {name} differ by {w:.2e} ({what})", case.name));
            }
        }
        let run = &case.viscous_runs()[0];
        let o = &run.options;
        let status = match (&log, f.is_empty()) {
            (
                LogComparison::Different {
                    line,
                    ours,
                    xfoil,
                    difference,
                    sensitivity,
                },
                _,
            ) => {
                f.push(format!(
                    "{}: console logs differ at line {line} by {difference:.1e} (our own \
                     sensitivity {sensitivity:.1e}):\n  ours : {ours}\n  XFOIL: {xfoil}",
                    case.name
                ));
                "FAIL".to_string()
            }
            (_, true) => {
                exact += 1;
                "ok".to_string()
            }
            (
                LogComparison::RoundOff {
                    line,
                    ours,
                    xfoil,
                    difference,
                    sensitivity,
                },
                false,
            ) => {
                diverging.push(format!(
                    "`{}`: identical logs up to line {line}, then `{}` vs XFOIL `{}` (relative \
                     difference {difference:.1e}; perturbing our Re by 1e-15..1e-11 changes this \
                     line by up to {})",
                    case.name,
                    ours.trim(),
                    xfoil.trim(),
                    if sensitivity.is_finite() {
                        format!("{sensitivity:.1e}")
                    } else {
                        "more (an earlier line already changes)".to_string()
                    }
                ));
                f.clear();
                format!("round-off divergence (log line {line})")
            }
            (LogComparison::Identical { .. }, false) => "FAIL".to_string(),
        };
        rows.push(format!(
            "| {} | {} | {:.0e} | {} | {} | {} | {} | {:.1e} | {:.1e} | {:.1e} | {} |",
            case.name,
            case.manifest["description"].as_str().unwrap_or(""),
            o.reynolds,
            o.mach,
            if o.ncrit[0] == o.ncrit[1] {
                format!("{}", o.ncrit[0])
            } else {
                format!("{} / {}", o.ncrit[0], o.ncrit[1])
            },
            if o.xtr == [1.0, 1.0] {
                "free".to_string()
            } else {
                format!("{} / {}", o.xtr[0], o.xtr[1])
            },
            stats.points,
            stats.forces.diff,
            stats.transition.diff,
            stats.boundary_layer.diff,
            status
        ));
        points += stats.points;
        failures.extend(f);
    }
    let compared = rows.len();
    let header = "| Case | Airfoil | Re | M | Ncrit | Trips | Points | forces | x_tr | BL | |\n\
                  |---|---|---|---|---|---|---|---|---|---|---|";
    let report = format!(
        "# Extended viscous XFOIL validation\n\n{compared} random viscous analyses compared with \
         XFOIL 6.996 ({points} converged points compared, polar sequences and single points), \
         {} failure(s); {} case(s) on which XFOIL itself failed.\n\n{exact} cases agree with \
         XFOIL on every point. In the other {} cases the Newton iterations go through unstable \
         phases (separated flows, unconverged points) where last-bit differences grow until the \
         two codes follow different paths; for each of them our console output is identical to \
         XFOIL's up to a first difference no larger than the change our own computation shows when \
         its Reynolds number is perturbed by 1e-15 to 1e-11 (relative):\n\n{}\n\nMaximum \
         differences per case: forces (CL, CM absolute, CD relative), transition x/c, boundary \
         layer at every station (x, Cp absolute; θ, δ* relative to the largest θ; Cf relative to \
         0.01; N relative to 10; √Cτ relative to 0.1).\n\n{header}\n{}\n",
        failures.len(),
        xfoil_failed.len(),
        diverging.len(),
        diverging
            .iter()
            .map(|d| format!("- {d}"))
            .collect::<Vec<_>>()
            .join("\n"),
        rows.join("\n")
    );
    println!("{report}");
    if !xfoil_failed.is_empty() {
        println!("XFOIL failed on:\n{}", xfoil_failed.join("\n"));
    }
    if std::env::var_os("AERO_VALIDATION_REPORT").is_some() {
        std::fs::write(root.join("REPORT.md"), &report).unwrap();
    }
    assert!(compared >= MIN_CASES, "only {compared} cases compared");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(
        exact >= MIN_EXACT,
        "only {exact} cases agree on every point"
    );
}

#[test]
fn cases_that_break_xfoil_terminate() {
    // XFOIL hangs on some analyses (it was stopped after 15 minutes); ours
    // must finish, converged or not.
    let mut checked = 0;
    for case in cases_in(&viscous_validation_dir()) {
        if case.xfoil_failure().is_none() {
            continue;
        }
        for run in case.viscous_runs() {
            let solver = common::viscous::viscous_solver(&case, &run);
            let polar = run.run_polar(&mut solver.session(), |_, _| ControlFlow::Continue(()));
            assert!(polar.points.len() + polar.failures.len() <= run.polar_len());
            for &alpha in &run.points {
                let s = run.run_point(&mut solver.session(), alpha).unwrap();
                assert!(s.iterations <= run.options.max_iterations);
            }
        }
        checked += 1;
    }
    assert!(checked >= 1);
}

/// Diagnostic: `AERO_TRACE_CASE=<case directory> cargo test --release
/// --test xfoil_viscous_validation trace_case -- --ignored --nocapture`
/// prints our console log next to XFOIL's around their first difference;
/// `AERO_TRACE_PERTURB=<eps>` multiplies the Reynolds number by `1 + eps`.
#[test]
#[ignore = "diagnostic, needs AERO_TRACE_CASE"]
fn trace_case() {
    use common::viscous::{our_log_lines, xfoil_log_lines};
    let dir = std::path::PathBuf::from(std::env::var("AERO_TRACE_CASE").unwrap());
    let parent = dir.parent().unwrap();
    let case = cases_in(parent)
        .into_iter()
        .find(|c| c.dir.ends_with(dir.file_name().unwrap()))
        .unwrap();
    let eps: f64 = std::env::var("AERO_TRACE_PERTURB")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0);
    for run in case.viscous_runs() {
        let ours = our_log_lines(&case, &run, eps);
        let theirs = xfoil_log_lines(&case, &run);
        match ours.iter().zip(&theirs).position(|(a, b)| a != b) {
            None => println!(
                "{}: identical ({} lines)",
                run.name,
                ours.len().min(theirs.len())
            ),
            Some(k) => {
                println!("{}: first difference at line {k}", run.name);
                for j in k.saturating_sub(6)..(k + 4).min(ours.len()).min(theirs.len()) {
                    println!("ours : {}\nxfoil: {}", ours[j], theirs[j]);
                }
            }
        }
    }
}
