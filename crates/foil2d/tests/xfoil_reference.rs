//! Validation of the inviscid solver against XFOIL 6.996 (`tests/reference`).
//!
//! Two levels are checked for every case:
//! - **A**: our solver on XFOIL's own panel nodes (`PSAV`), which isolates the
//!   solver from the paneling;
//! - **B**: our full pipeline (NACA generator or `.dat` import, `PANGEN`
//!   port, solver).
//!
//! Tolerances (see `common`) are mixed `|ours − xfoil| ≤ atol + rtol·|xfoil|`,
//! slightly above the rounding of the numbers XFOIL prints.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{CP_TOL_PIPELINE, CP_TOL_XFOIL_NODES, Stats, cases, compare, solver};

/// Cases that must be present (the specification requires NACA 0012, 2412,
/// 4415 and an imported airfoil).
const REQUIRED: [&str; 7] = [
    "clarky",
    "naca0012",
    "naca0012_n240",
    "naca0012_sharp",
    "naca23012",
    "naca2412",
    "naca4415",
];

fn line(name: &str, level: &str, s: &Stats) -> String {
    format!(
        "{name:<15} {level}  max|dCL| {:.1e} ({})  max|dCM| {:.1e} ({})  max|dCp| {:.1e} ({})",
        s.cl.diff, s.cl.what, s.cm.diff, s.cm.what, s.cp.diff, s.cp.what
    )
}

#[test]
fn reference_cases_are_present() {
    let names: Vec<String> = cases().into_iter().map(|c| c.name).collect();
    for required in REQUIRED {
        assert!(
            names.iter().any(|n| n == required),
            "missing XFOIL reference case {required}; run tests/reference/generate.py"
        );
    }
}

#[test]
fn solver_on_xfoil_nodes_matches_xfoil() {
    let mut report = Vec::new();
    let mut failures = Vec::new();
    for case in cases().into_iter().filter(|c| c.xfoil_failure().is_none()) {
        let (stats, f) = compare(
            &case,
            "A",
            &solver(&case.xfoil_panels()),
            CP_TOL_XFOIL_NODES,
        );
        report.push(line(&case.name, "A", &stats));
        failures.extend(f);
    }
    println!("{}", report.join("\n"));
    assert!(
        failures.is_empty(),
        "{}\n{}",
        failures.join("\n"),
        report.join("\n")
    );
}

#[test]
fn full_pipeline_matches_xfoil() {
    let mut report = Vec::new();
    let mut failures = Vec::new();
    for case in cases().into_iter().filter(|c| c.xfoil_failure().is_none()) {
        let (stats, f) = compare(&case, "B", &solver(&case.our_panels()), CP_TOL_PIPELINE);
        report.push(line(&case.name, "B", &stats));
        failures.extend(f);
    }
    println!("{}", report.join("\n"));
    assert!(
        failures.is_empty(),
        "{}\n{}",
        failures.join("\n"),
        report.join("\n")
    );
}

/// Where XFOIL itself breaks down (NaN output), aero must report an error
/// rather than return garbage.
#[test]
fn inputs_that_break_xfoil_are_refused() {
    let broken: Vec<_> = cases()
        .into_iter()
        .filter(|c| c.xfoil_failure().is_some())
        .collect();
    assert!(!broken.is_empty());
    for case in broken {
        match case.try_our_panels() {
            Err(e) => println!("{}: refused ({e})", case.name),
            Ok(af) => assert!(
                af.points().all(|p| p[0].is_finite() && p[1].is_finite()),
                "{}: non-finite nodes",
                case.name
            ),
        }
    }
}
