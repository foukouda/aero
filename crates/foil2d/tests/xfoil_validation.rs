//! Extended validation against XFOIL 6.996 on randomly drawn airfoils
//! (`tests/validation`, built by `tests/validation/build_cases.py`).
//!
//! For every case the full pipeline (generator or `.dat` import, `PANGEN`
//! port, solver) is compared with XFOIL: panel nodes, CL, CM, CDp, Cp min
//! and Cp at every node, with the tolerances of the reference test. The
//! importer is also checked on the original, uncleaned UIUC files.
//!
//! Set `AERO_VALIDATION_REPORT=1` to write `tests/validation/REPORT.md`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use aero_geometry::parse_dat;
use common::{CP_TOL_PIPELINE, cases_in, compare, node_difference, solver, validation_dir};

/// Nodes must agree to the 7-digit printing precision of XFOIL.
const NODE_TOLERANCE: f64 = 2e-7;

/// The validation set must not silently shrink.
const MIN_CASES: usize = 95;

#[test]
fn random_airfoils_match_xfoil() {
    let cases = cases_in(&validation_dir());
    let mut failures = Vec::new();
    let mut rows = Vec::new();
    let mut compared = 0;
    let mut xfoil_failed = Vec::new();
    let mut points = 0;
    let mut breakdowns = 0;
    for case in &cases {
        if let Some(reason) = case.xfoil_failure() {
            xfoil_failed.push(format!("{}: {}", case.name, reason.trim()));
            continue;
        }
        let ours = case.our_panels();
        let nodes = node_difference(&ours, &case.xfoil_panels());
        match nodes {
            Some(d) if d <= NODE_TOLERANCE => {}
            Some(d) => failures.push(format!("{}: nodes differ by {d:.2e}", case.name)),
            None => failures.push(format!("{}: node counts differ", case.name)),
        }
        let (stats, f) = compare(case, "B", &solver(&ours), CP_TOL_PIPELINE);
        let status = if f.is_empty() && nodes.is_some_and(|d| d <= NODE_TOLERANCE) {
            "ok"
        } else {
            "FAIL"
        };
        failures.extend(f);
        compared += 1;
        points += stats.points;
        breakdowns += stats.breakdowns;
        let description = case.manifest["description"].as_str().unwrap_or("");
        rows.push(format!(
            "| {} | {} | {} | {:.1e} | {:.1e} | {:.1e} | {:.1e} | {:.1e} | {} |",
            case.name,
            description,
            ours.len(),
            nodes.unwrap_or(f64::NAN),
            stats.cl.diff,
            stats.cm.diff,
            stats.cdp.diff,
            stats.cp.diff,
            status
        ));
    }

    let header = "| Case | Airfoil | Nodes | max Δnode | max ΔCL | max ΔCM | max ΔCDp | max ΔCp | |\n\
                  |---|---|---|---|---|---|---|---|---|";
    let report = format!(
        "# Extended XFOIL validation\n\n{compared} cases compared with XFOIL 6.996 ({points} polar \
         points plus Cp distributions), {} failure(s); {} case(s) on which XFOIL itself failed; \
         {breakdowns} point(s) where the Kármán–Tsien correction breaks down (refused by aero, \
         flagged as invalid by XFOIL).\n\n{header}\n{}\n",
        failures.len(),
        xfoil_failed.len(),
        rows.join("\n")
    );
    println!("{report}");
    if !xfoil_failed.is_empty() {
        println!("XFOIL failed on:\n{}", xfoil_failed.join("\n"));
    }
    if std::env::var_os("AERO_VALIDATION_REPORT").is_some() {
        std::fs::write(validation_dir().join("REPORT.md"), &report).unwrap();
    }
    assert!(compared >= MIN_CASES, "only {compared} cases compared");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn importer_reads_original_uiuc_files() {
    let mut checked = 0;
    for case in cases_in(&validation_dir()) {
        let Some(original) = case.original_file() else {
            continue;
        };
        let raw = std::fs::read_to_string(&original).unwrap();
        let (ours, report) = parse_dat(&raw, &case.name)
            .unwrap_or_else(|e| panic!("{}: cannot import the original file: {e}", case.name));
        let cleaned = case.buffer_airfoil();
        assert_eq!(
            ours.x(),
            cleaned.x(),
            "{}: x differs from the independent Python conversion ({:?})",
            case.name,
            report.format
        );
        assert_eq!(ours.y(), cleaned.y(), "{}: y differs", case.name);
        checked += 1;
    }
    assert!(checked >= 55, "only {checked} original files checked");
}
