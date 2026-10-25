//! Validation of the inviscid solver against XFOIL 6.996 (`tests/reference`).
//!
//! Two levels are checked for every case:
//! - **A**: our solver on XFOIL's own panel nodes (`PSAV`), which isolates the
//!   solver from the paneling;
//! - **B**: our full pipeline (NACA generator or `.dat` import, `PANGEN`
//!   port, solver).
//!
//! Tolerances are mixed `|ours − xfoil| ≤ atol + rtol·|xfoil|`. XFOIL
//! prints CL and CM with 4 decimals, CDp with 5 decimals, Cp with 5
//! decimals and the nodes with 7 significant digits, so the absolute parts
//! slightly exceed the corresponding rounding errors. At level A the
//! rounding of the nodes themselves perturbs Cp by up to ~6e-5 (the local
//! panel geometry amplifies 1e-7 node errors), hence a looser Cp tolerance.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use aero_foil2d::inviscid::{InviscidOptions, InviscidSolver, OperatingPoint};
use common::{Case, cases, close};

/// (atol, rtol) per quantity.
const CL_TOL: (f64, f64) = (1.0e-4, 1.0e-4);
const CM_TOL: (f64, f64) = (1.0e-4, 1.0e-4);
const CDP_TOL: (f64, f64) = (2.0e-5, 1.0e-3);
const CPMIN_TOL: (f64, f64) = (1.0e-4, 1.0e-4);
const CP_TOL_PIPELINE: (f64, f64) = (2.0e-5, 1.0e-5);
const CP_TOL_XFOIL_NODES: (f64, f64) = (1.0e-4, 0.0);

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

struct Worst {
    what: String,
    diff: f64,
}

impl Worst {
    fn new() -> Self {
        Self {
            what: String::new(),
            diff: 0.0,
        }
    }

    fn update(&mut self, diff: f64, what: impl FnOnce() -> String) {
        if diff > self.diff {
            self.diff = diff;
            self.what = what();
        }
    }
}

fn options() -> InviscidOptions {
    // XFOIL never refuses a supercritical point; compare like for like.
    InviscidOptions {
        allow_supercritical: true,
        ..InviscidOptions::default()
    }
}

/// Compares polars and Cp distributions of one case for a given airfoil.
/// Returns the failures and appends a summary line to `report`.
fn compare(
    case: &Case,
    level: &str,
    solver: &InviscidSolver,
    cp_tol: (f64, f64),
    report: &mut Vec<String>,
) -> Vec<String> {
    let mut failures = Vec::new();
    let mut worst_cl = Worst::new();
    let mut worst_cm = Worst::new();
    let mut worst_cp = Worst::new();
    for run in case.runs() {
        let mach = run.mach;
        if run.has_polar {
            for row in case.polar(mach) {
                let ours = solver
                    .solve(OperatingPoint {
                        alpha_deg: row.alpha,
                        mach,
                    })
                    .unwrap();
                let tag = || format!("M={mach} a={}", row.alpha);
                worst_cl.update((ours.cl - row.cl).abs(), tag);
                worst_cm.update((ours.cm - row.cm).abs(), tag);
                let checks = [
                    ("CL", ours.cl, row.cl, CL_TOL),
                    ("CM", ours.cm, row.cm, CM_TOL),
                    ("CDp", ours.cdp, row.cdp, CDP_TOL),
                ];
                for (name, a, b, (atol, rtol)) in checks {
                    if !close(a, b, atol, rtol) {
                        failures.push(format!(
                            "{} [{level}] {}: {name} {a:.6} vs XFOIL {b:.6}",
                            case.name,
                            tag()
                        ));
                    }
                }
                if let Some(cpmin) = row.cp_min {
                    let (atol, rtol) = CPMIN_TOL;
                    if !close(ours.cp_min, cpmin, atol, rtol) {
                        failures.push(format!(
                            "{} [{level}] {}: Cpmin {:.6} vs XFOIL {cpmin:.6}",
                            case.name,
                            tag(),
                            ours.cp_min
                        ));
                    }
                }
            }
        }
        for &alpha in &run.cp_alphas {
            let xfoil = case.cp(mach, alpha);
            let ours = solver
                .solve(OperatingPoint {
                    alpha_deg: alpha,
                    mach,
                })
                .unwrap();
            assert_eq!(xfoil.len(), ours.cp.len(), "{}: Cp node count", case.name);
            for (i, (&cp, &(x, cpx))) in ours.cp.iter().zip(&xfoil).enumerate() {
                let diff = (cp - cpx).abs();
                worst_cp.update(diff, || format!("M={mach} a={alpha} node {i} x={x}"));
                let (atol, rtol) = cp_tol;
                if !close(cp, cpx, atol, rtol) {
                    failures.push(format!(
                        "{} [{level}] M={mach} a={alpha} node {i}: Cp {cp:.6} vs XFOIL {cpx:.6}",
                        case.name
                    ));
                }
            }
        }
    }
    report.push(format!(
        "{:<15} {level}  max|dCL| {:.1e} ({})  max|dCM| {:.1e} ({})  max|dCp| {:.1e} ({})",
        case.name,
        worst_cl.diff,
        worst_cl.what,
        worst_cm.diff,
        worst_cm.what,
        worst_cp.diff,
        worst_cp.what
    ));
    failures
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
    for case in cases() {
        let solver = InviscidSolver::new(&case.xfoil_panels(), &options()).unwrap();
        failures.extend(compare(
            &case,
            "A",
            &solver,
            CP_TOL_XFOIL_NODES,
            &mut report,
        ));
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
    for case in cases() {
        let solver = InviscidSolver::new(&case.our_panels(), &options()).unwrap();
        failures.extend(compare(&case, "B", &solver, CP_TOL_PIPELINE, &mut report));
    }
    println!("{}", report.join("\n"));
    assert!(
        failures.is_empty(),
        "{}\n{}",
        failures.join("\n"),
        report.join("\n")
    );
}
