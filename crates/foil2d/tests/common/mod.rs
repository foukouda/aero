//! Readers for the XFOIL reference data in `tests/reference` (see its
//! README for the file formats and the naming convention).

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use aero_geometry::{Airfoil, NacaOptions, PanelingOptions, TrailingEdge, naca, panel, parse_dat};

/// One reference case.
pub struct Case {
    pub name: String,
    pub dir: PathBuf,
    pub manifest: toml::Table,
}

/// One run (Mach number) of a case.
pub struct Run {
    pub mach: f64,
    pub has_polar: bool,
    pub cp_alphas: Vec<f64>,
}

/// One row of an XFOIL polar file.
#[derive(Debug, Clone, Copy)]
pub struct XfoilPolarRow {
    pub alpha: f64,
    pub cl: f64,
    pub cdp: f64,
    pub cm: f64,
    pub cp_min: Option<f64>,
}

pub fn reference_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/reference")
}

pub fn cases() -> Vec<Case> {
    let mut cases: Vec<Case> = std::fs::read_dir(reference_dir())
        .expect("tests/reference exists")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("case.toml").exists())
        .map(|dir| {
            let manifest = std::fs::read_to_string(dir.join("case.toml"))
                .unwrap()
                .parse()
                .unwrap();
            Case {
                name: dir.file_name().unwrap().to_string_lossy().to_string(),
                dir,
                manifest,
            }
        })
        .collect();
    cases.sort_by(|a, b| a.name.cmp(&b.name));
    cases
}

impl Case {
    pub fn runs(&self) -> Vec<Run> {
        self.manifest["runs"]
            .as_array()
            .expect("[[runs]]")
            .iter()
            .map(|r| {
                let r = r.as_table().unwrap();
                Run {
                    mach: r["mach"].as_float().unwrap(),
                    has_polar: r.contains_key("polar"),
                    cp_alphas: r
                        .get("cp_alphas")
                        .and_then(|v| v.as_array())
                        .map(|a| a.iter().map(|v| v.as_float().unwrap()).collect())
                        .unwrap_or_default(),
                }
            })
            .collect()
    }

    /// XFOIL's paneled airfoil (`PSAV`).
    pub fn xfoil_panels(&self) -> Airfoil {
        let text = std::fs::read_to_string(self.dir.join("panels.dat")).unwrap_or_else(|_| {
            panic!(
                "{}: panels.dat is missing; run tests/reference/generate.py",
                self.name
            )
        });
        parse_dat(&text, &self.name).unwrap().0
    }

    /// Airfoil generated or imported by our code, before paneling.
    pub fn buffer_airfoil(&self) -> Airfoil {
        let airfoil = self.manifest["airfoil"].as_table().unwrap();
        let kind = airfoil["kind"].as_str().unwrap();
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
                naca(airfoil["code"].as_str().unwrap(), &options).unwrap()
            }
            "file" => {
                let file = airfoil["file"].as_str().unwrap();
                let text = std::fs::read_to_string(self.dir.join(file)).unwrap();
                parse_dat(&text, file).unwrap().0
            }
            other => panic!("unknown kind {other}"),
        }
    }

    pub fn paneling(&self) -> PanelingOptions {
        let p = self.manifest["paneling"].as_table().unwrap();
        let f = |k: &str| p[k].as_float().unwrap();
        PanelingOptions {
            n_nodes: usize::try_from(p["n_nodes"].as_integer().unwrap()).unwrap(),
            curvature_bunching: f("cvpar"),
            te_le_density_ratio: f("cterat"),
            refined_density_ratio: f("ctrrat"),
            ..PanelingOptions::default()
        }
    }

    /// Our full pipeline: generator/import followed by our `PANGEN`.
    pub fn our_panels(&self) -> Airfoil {
        panel(&self.buffer_airfoil(), &self.paneling()).unwrap()
    }

    pub fn polar(&self, mach: f64) -> Vec<XfoilPolarRow> {
        let path = self.dir.join(format!("polar_M{mach:.2}.txt"));
        parse_polar(
            &std::fs::read_to_string(&path)
                .unwrap_or_else(|_| panic!("{}: missing {}", self.name, path.display())),
        )
    }

    pub fn cp(&self, mach: f64, alpha: f64) -> Vec<(f64, f64)> {
        let path = self.dir.join(format!("cp_M{mach:.2}_a{alpha:+05.1}.txt"));
        parse_cp(
            &std::fs::read_to_string(&path)
                .unwrap_or_else(|_| panic!("{}: missing {}", self.name, path.display())),
        )
    }
}

/// Parses an XFOIL polar save file, locating columns by their header names.
pub fn parse_polar(text: &str) -> Vec<XfoilPolarRow> {
    let lines: Vec<&str> = text.lines().collect();
    let header = lines
        .iter()
        .position(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            w.first() == Some(&"alpha") && w.contains(&"CL")
        })
        .expect("polar header");
    let names: Vec<&str> = lines[header].split_whitespace().collect();
    let col = |name: &str| names.iter().position(|n| *n == name);
    let (ia, icl, icdp, icm) = (
        col("alpha").unwrap(),
        col("CL").unwrap(),
        col("CDp").unwrap(),
        col("CM").unwrap(),
    );
    let icp = col("Cpmin");
    lines[header + 2..]
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Vec<f64> = l.split_whitespace().map(|t| t.parse().unwrap()).collect();
            XfoilPolarRow {
                alpha: v[ia],
                cl: v[icl],
                cdp: v[icdp],
                cm: v[icm],
                cp_min: icp.map(|i| v[i]),
            }
        })
        .collect()
}

/// Parses an XFOIL `CPWR` file (`x Cp` per line, `#` header).
pub fn parse_cp(text: &str) -> Vec<(f64, f64)> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let v: Vec<f64> = l
                .split_whitespace()
                .take(2)
                .map(|t| t.parse().unwrap())
                .collect();
            (v[0], v[1])
        })
        .collect()
}

/// Mixed absolute/relative tolerance check `|a − b| ≤ atol + rtol·|b|`.
pub fn close(a: f64, b: f64, atol: f64, rtol: f64) -> bool {
    (a - b).abs() <= atol + rtol * b.abs()
}
