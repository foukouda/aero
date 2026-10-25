//! Compares the NACA generator and `PANGEN` port against the panel nodes
//! written by XFOIL 6.996 (`PSAV`) for every case in `tests/reference`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use aero_geometry::{
    Airfoil, ImportOptions, NacaOptions, PanelingOptions, TrailingEdge, naca, panel, parse_dat,
    parse_dat_with,
};

/// Maximum node-position difference, in chord units. XFOIL writes nodes with
/// 7 significant digits (`G15.7`), i.e. a rounding error of up to 5e-8: the
/// port must agree to that precision.
const NODE_TOLERANCE: f64 = 2e-7;

fn reference_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/reference")
}

fn cases() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(reference_dir())
        .expect("tests/reference exists")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("case.toml").exists())
        .collect();
    dirs.sort();
    dirs
}

fn buffer_airfoil(dir: &Path, case: &toml::Table) -> Airfoil {
    let airfoil = case["airfoil"].as_table().expect("[airfoil]");
    let kind = airfoil["kind"].as_str().expect("kind");
    match kind {
        "naca" | "naca_sharp" => {
            let options = NacaOptions {
                trailing_edge: if kind == "naca" {
                    TrailingEdge::Open
                } else {
                    TrailingEdge::Closed
                },
                ..NacaOptions::default()
            };
            naca(airfoil["code"].as_str().expect("code"), &options).expect("NACA")
        }
        "file" => {
            let file = airfoil["file"].as_str().expect("file");
            let text = std::fs::read_to_string(dir.join(file)).expect("airfoil file");
            // Doubled points are corners for XFOIL; keep them when asked.
            let options = ImportOptions {
                merge_duplicate_points: airfoil
                    .get("merge_duplicates")
                    .and_then(toml::Value::as_bool)
                    .unwrap_or(true),
            };
            parse_dat_with(&text, file, &options).expect("parse").0
        }
        other => panic!("unknown kind {other}"),
    }
}

fn paneling_options(case: &toml::Table) -> PanelingOptions {
    let p = case["paneling"].as_table().expect("[paneling]");
    let f = |k: &str| p[k].as_float().expect(k);
    PanelingOptions {
        n_nodes: usize::try_from(p["n_nodes"].as_integer().expect("n_nodes")).expect("n"),
        curvature_bunching: f("cvpar"),
        te_le_density_ratio: f("cterat"),
        refined_density_ratio: f("ctrrat"),
        ..PanelingOptions::default()
    }
}

#[test]
fn paneling_matches_xfoil_nodes() {
    let cases = cases();
    assert!(cases.len() >= 7, "reference cases are missing");
    let mut report = Vec::new();
    for dir in &cases {
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let case: toml::Table = std::fs::read_to_string(dir.join("case.toml"))
            .unwrap()
            .parse()
            .unwrap();
        let xfoil_text = std::fs::read_to_string(dir.join("panels.dat")).unwrap_or_else(|_| {
            panic!("{name}: panels.dat is missing; run tests/reference/generate.py")
        });
        let (xfoil, _) = parse_dat(&xfoil_text, &name).unwrap();

        let ours = panel(&buffer_airfoil(dir, &case), &paneling_options(&case)).unwrap();
        assert_eq!(ours.len(), xfoil.len(), "{name}: node count");
        let max_diff = ours
            .points()
            .zip(xfoil.points())
            .map(|(a, b)| (a[0] - b[0]).hypot(a[1] - b[1]))
            .fold(0.0_f64, f64::max);
        report.push(format!("{name}: max node difference {max_diff:.2e}"));
        assert!(
            max_diff < NODE_TOLERANCE,
            "{name}: nodes differ from XFOIL by {max_diff:.3e}\n{}",
            report.join("\n")
        );
    }
    println!("{}", report.join("\n"));
}
