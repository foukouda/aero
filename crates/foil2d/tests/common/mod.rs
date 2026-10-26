//! Readers for the XFOIL reference data (`tests/reference`,
//! `tests/validation`; see tests/reference/README.md for the file formats)
//! and the comparison shared by the validation tests.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod viscous;

use std::path::{Path, PathBuf};

use aero_foil2d::inviscid::{
    InviscidError, InviscidOptions, InviscidSolution, InviscidSolver, OperatingPoint,
};
use aero_geometry::{
    Airfoil, ImportOptions, NacaOptions, PanelingOptions, TrailingEdge, naca, panel, parse_dat,
    parse_dat_with,
};

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

fn repo_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests")
        .join(name)
}

pub fn reference_dir() -> PathBuf {
    repo_dir("reference")
}

pub fn validation_dir() -> PathBuf {
    repo_dir("validation")
}

pub fn inverse_dir() -> PathBuf {
    repo_dir("inverse")
}

/// Cases of `tests/reference`.
pub fn cases() -> Vec<Case> {
    cases_in(&reference_dir())
}

/// Every directory of `root` holding a `case.toml`, sorted by name.
pub fn cases_in(root: &Path) -> Vec<Case> {
    let mut cases: Vec<Case> = std::fs::read_dir(root)
        .unwrap_or_else(|_| panic!("{} exists", root.display()))
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

    /// Why XFOIL itself fails on this case: recorded by the generator
    /// (`FAILED.txt`) or declared in the manifest (`xfoil_failure`).
    pub fn xfoil_failure(&self) -> Option<String> {
        std::fs::read_to_string(self.dir.join("FAILED.txt"))
            .ok()
            .or_else(|| {
                self.manifest
                    .get("xfoil_failure")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            })
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

    fn airfoil_table(&self) -> &toml::Table {
        self.manifest["airfoil"].as_table().unwrap()
    }

    /// Path of the original (uncleaned) coordinate file, if any.
    pub fn original_file(&self) -> Option<PathBuf> {
        self.airfoil_table()
            .get("original")
            .and_then(|v| v.as_str())
            .map(|f| self.dir.join(f))
    }

    /// Airfoil generated or imported by our code, with the case's `gdes`
    /// edits applied, before paneling.
    pub fn buffer_airfoil(&self) -> Airfoil {
        let base = self.unmodified_airfoil();
        match self.manifest.get("gdes").and_then(|v| v.as_array()) {
            Some(commands) => {
                let lines: Vec<&str> = commands.iter().map(|v| v.as_str().unwrap()).collect();
                aero_geometry::apply_gdes(&base, &lines).unwrap()
            }
            None => base,
        }
    }

    fn unmodified_airfoil(&self) -> Airfoil {
        let airfoil = self.airfoil_table();
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
                // Doubled points are corners for XFOIL; keep them when asked.
                let options = ImportOptions {
                    merge_duplicate_points: airfoil
                        .get("merge_duplicates")
                        .and_then(toml::Value::as_bool)
                        .unwrap_or(true),
                };
                parse_dat_with(&text, file, &options).unwrap().0
            }
            other => panic!("unknown kind {other}"),
        }
    }

    pub fn paneling(&self) -> PanelingOptions {
        let p = self.manifest["paneling"].as_table().unwrap();
        let f = |k: &str| p[k].as_float().unwrap();
        let zone = |k: &str| {
            p.get(k).and_then(|v| v.as_array()).map_or([1.0, 1.0], |a| {
                [a[0].as_float().unwrap(), a[1].as_float().unwrap()]
            })
        };
        PanelingOptions {
            n_nodes: usize::try_from(p["n_nodes"].as_integer().unwrap()).unwrap(),
            curvature_bunching: f("cvpar"),
            te_le_density_ratio: f("cterat"),
            refined_density_ratio: f("ctrrat"),
            top_refinement: zone("xsref"),
            bottom_refinement: zone("xpref"),
        }
    }

    /// Our full pipeline: generator/import followed by our `PANGEN`.
    pub fn our_panels(&self) -> Airfoil {
        self.try_our_panels().unwrap()
    }

    /// Our full pipeline, returning the paneling error if any.
    pub fn try_our_panels(&self) -> aero_geometry::Result<Airfoil> {
        panel(&self.buffer_airfoil(), &self.paneling())
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

/// Parses an XFOIL polar save file. Rows are read as fixed-width Fortran
/// fields (`1X, F7.3, F9.4, F10.5, F10.5, F9.4, F9.4, ...` in header order,
/// see PINDEX.INC), because an overflowing field is printed as `*****` and
/// runs into its neighbour. An overflowing Cp min is reported as `None`.
pub fn parse_polar(text: &str) -> Vec<XfoilPolarRow> {
    let lines: Vec<&str> = text.lines().collect();
    let header = lines
        .iter()
        .position(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            w.first() == Some(&"alpha") && w.contains(&"CL")
        })
        .expect("polar header");
    let width = |name: &str| match name {
        "alpha" => 7,
        "CL" | "CM" | "Cpmin" => 9,
        "CD" | "CDp" => 10,
        _ => 9,
    };
    let mut fields = Vec::new();
    let mut offset = 1;
    for name in lines[header].split_whitespace() {
        fields.push((name, offset, width(name)));
        offset += width(name);
    }
    let field = |line: &str, name: &str| -> Option<f64> {
        let &(_, start, w) = fields.iter().find(|f| f.0 == name)?;
        line.get(start..start + w)?.trim().parse().ok()
    };
    lines[header + 2..]
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| XfoilPolarRow {
            alpha: field(l, "alpha").expect("alpha"),
            cl: field(l, "CL").expect("CL"),
            cdp: field(l, "CDp").expect("CDp"),
            cm: field(l, "CM").expect("CM"),
            cp_min: field(l, "Cpmin"),
        })
        .collect()
}

/// Parses an XFOIL `CPWR` file (`#` header, then `1X, 2F11.5`: x and Cp in
/// fixed-width fields, which touch when Cp needs all 11 characters).
pub fn parse_cp(text: &str) -> Vec<(f64, f64)> {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .map(|l| {
            let x = l.get(1..12).unwrap().trim().parse().unwrap();
            let cp = l.get(12..23).unwrap().trim().parse().unwrap();
            (x, cp)
        })
        .collect()
}

/// Mixed absolute/relative tolerance check `|a − b| ≤ atol + rtol·|b|`.
pub fn close(a: f64, b: f64, atol: f64, rtol: f64) -> bool {
    (a - b).abs() <= atol + rtol * b.abs()
}

// ------------------------------------------------------------- comparison

/// (atol, rtol) per quantity. XFOIL prints CL and CM with 4 decimals, CDp and
/// Cp with 5 decimals and the nodes with 7 significant digits.
pub const CL_TOL: (f64, f64) = (1.0e-4, 1.0e-4);
pub const CM_TOL: (f64, f64) = (1.0e-4, 1.0e-4);
pub const CDP_TOL: (f64, f64) = (2.0e-5, 1.0e-3);
pub const CPMIN_TOL: (f64, f64) = (1.0e-4, 1.0e-4);
/// Cp with our own paneling (nodes at full precision).
pub const CP_TOL_PIPELINE: (f64, f64) = (2.0e-5, 1.0e-5);
/// Cp on XFOIL's printed nodes: the 7-digit rounding of the nodes perturbs
/// the local panel geometry, hence a looser tolerance.
pub const CP_TOL_XFOIL_NODES: (f64, f64) = (1.0e-4, 0.0);

/// Largest difference seen for one quantity, with where it occurred.
#[derive(Debug, Clone, Default)]
pub struct Worst {
    pub diff: f64,
    pub what: String,
}

impl Worst {
    pub fn update(&mut self, diff: f64, what: impl FnOnce() -> String) {
        if diff > self.diff {
            self.diff = diff;
            self.what = what();
        }
    }
}

/// Maximum differences of one case.
#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub cl: Worst,
    pub cm: Worst,
    pub cdp: Worst,
    pub cp_min: Worst,
    pub cp: Worst,
    pub points: usize,
    /// Points where the Kármán–Tsien correction breaks down, which our
    /// solver refuses and XFOIL only warns about.
    pub breakdowns: usize,
}

/// Solver options matching XFOIL, which never refuses a supercritical point.
pub fn xfoil_like_options() -> InviscidOptions {
    InviscidOptions {
        allow_supercritical: true,
        ..InviscidOptions::default()
    }
}

/// Compares the polars and Cp distributions of a case with a solver.
/// Returns the statistics and the tolerance violations.
pub fn compare(
    case: &Case,
    level: &str,
    solver: &InviscidSolver,
    cp_tol: (f64, f64),
) -> (Stats, Vec<String>) {
    let mut stats = Stats::default();
    let mut failures = Vec::new();
    let xfoil_warned = std::fs::read_to_string(case.dir.join("xfoil.log"))
        .is_ok_and(|log| log.contains("Compressibility corrections invalid"));
    // Our solution, or None for a Karman-Tsien breakdown that XFOIL also
    // reported (XFOIL prints numbers anyway; we refuse them).
    let mut solve =
        |alpha: f64, mach: f64, failures: &mut Vec<String>| -> Option<InviscidSolution> {
            match solver.solve(OperatingPoint {
                alpha_deg: alpha,
                mach,
            }) {
                Ok(s) => Some(s),
                Err(InviscidError::CompressibilityBreakdown { .. }) if xfoil_warned => {
                    stats.breakdowns += 1;
                    None
                }
                Err(e) => {
                    failures.push(format!("{} [{level}] M={mach} a={alpha}: {e}", case.name));
                    None
                }
            }
        };
    let mut results = Vec::new();
    for run in case.runs() {
        let mach = run.mach;
        if run.has_polar {
            for row in case.polar(mach) {
                if let Some(ours) = solve(row.alpha, mach, &mut failures) {
                    results.push((mach, Some(row), None, ours));
                }
            }
        }
        for &alpha in &run.cp_alphas {
            if let Some(ours) = solve(alpha, mach, &mut failures) {
                results.push((mach, None, Some((alpha, case.cp(mach, alpha))), ours));
            }
        }
    }
    for (mach, row, cp, ours) in results {
        if let Some(row) = row {
            {
                stats.points += 1;
                let tag = || format!("M={mach} a={}", row.alpha);
                stats.cl.update((ours.cl - row.cl).abs(), tag);
                stats.cm.update((ours.cm - row.cm).abs(), tag);
                stats.cdp.update((ours.cdp - row.cdp).abs(), tag);
                let mut checks = vec![
                    ("CL", ours.cl, row.cl, CL_TOL),
                    ("CM", ours.cm, row.cm, CM_TOL),
                    ("CDp", ours.cdp, row.cdp, CDP_TOL),
                ];
                if let Some(cpmin) = row.cp_min {
                    stats.cp_min.update((ours.cp_min - cpmin).abs(), tag);
                    checks.push(("Cpmin", ours.cp_min, cpmin, CPMIN_TOL));
                }
                for (name, a, b, (atol, rtol)) in checks {
                    if !close(a, b, atol, rtol) {
                        failures.push(format!(
                            "{} [{level}] {}: {name} {a:.6} vs XFOIL {b:.6}",
                            case.name,
                            tag()
                        ));
                    }
                }
            }
        }
        if let Some((alpha, xfoil)) = cp {
            if xfoil.len() != ours.cp.len() {
                failures.push(format!(
                    "{} [{level}] M={mach} a={alpha}: {} nodes vs XFOIL {}",
                    case.name,
                    ours.cp.len(),
                    xfoil.len()
                ));
                continue;
            }
            for (i, (&cp, &(x, cpx))) in ours.cp.iter().zip(&xfoil).enumerate() {
                stats.cp.update((cp - cpx).abs(), || {
                    format!("M={mach} a={alpha} node {i} x={x}")
                });
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
    (stats, failures)
}

/// Largest distance between corresponding nodes of two airfoils, or `None`
/// if the node counts differ.
pub fn node_difference(a: &Airfoil, b: &Airfoil) -> Option<f64> {
    (a.len() == b.len()).then(|| {
        a.points()
            .zip(b.points())
            .map(|(p, q)| (p[0] - q[0]).hypot(p[1] - q[1]))
            .fold(0.0_f64, f64::max)
    })
}

/// Solver built from airfoil nodes.
pub fn solver(airfoil: &Airfoil) -> InviscidSolver {
    InviscidSolver::new(airfoil, &xfoil_like_options()).unwrap()
}

/// Maps `f` over `items` on all cores, keeping the order.
pub fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism().map_or(4, usize::from);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut results: Vec<Option<R>> = (0..items.len()).map(|_| None).collect();
    let slots = std::sync::Mutex::new(&mut results);
    std::thread::scope(|scope| {
        for _ in 0..threads.min(items.len()) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if i >= items.len() {
                        break;
                    }
                    let r = f(&items[i]);
                    slots.lock().unwrap()[i] = Some(r);
                }
            });
        }
    });
    results.into_iter().map(Option::unwrap).collect()
}
