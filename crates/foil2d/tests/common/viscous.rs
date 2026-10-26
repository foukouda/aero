//! Readers for the XFOIL viscous reference data (`tests/viscous`, see
//! tests/viscous/README.md) and the comparison of a viscous solution with
//! XFOIL's full-precision dump files.

use std::path::{Path, PathBuf};

use std::ops::ControlFlow;

use aero_foil2d::inviscid::{AlphaSweep, InviscidOptions, InviscidSolver};
use aero_foil2d::viscous::{BoundaryLayer, ViscousOptions, ViscousSolution, ViscousSolver};

use super::{Case, close};

pub fn viscous_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/viscous")
}

pub fn viscous_validation_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/viscous_validation")
}

/// One `[[viscous]]` run of a case: one XFOIL session.
pub struct ViscousRun {
    pub name: String,
    pub options: ViscousOptions,
    pub polar: AlphaSweep,
    pub points: Vec<f64>,
}

impl Case {
    pub fn viscous_runs(&self) -> Vec<ViscousRun> {
        self.manifest["viscous"]
            .as_array()
            .expect("[[viscous]]")
            .iter()
            .map(|r| {
                let r = r.as_table().unwrap();
                let f = |k: &str| r[k].as_float().unwrap();
                let pair = |k: &str| match &r[k] {
                    toml::Value::Array(a) => [a[0].as_float().unwrap(), a[1].as_float().unwrap()],
                    v => [v.as_float().unwrap(); 2],
                };
                let polar = r["polar"].as_table().unwrap();
                let p = |k: &str| polar[k].as_float().unwrap();
                ViscousRun {
                    name: r["name"].as_str().unwrap().to_string(),
                    options: ViscousOptions {
                        reynolds: f("reynolds"),
                        mach: f("mach"),
                        ncrit: pair("ncrit"),
                        xtr: pair("xtr"),
                        ..ViscousOptions::default()
                    },
                    polar: AlphaSweep {
                        start_deg: p("start"),
                        end_deg: p("end"),
                        step_deg: p("step"),
                    },
                    points: r["points"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_float().unwrap())
                        .collect(),
                }
            })
            .collect()
    }
}

/// A point of an XFOIL `PACC` dump file.
#[derive(Debug, Clone)]
pub struct DumpPoint {
    pub alpha: f64,
    pub cl: f64,
    pub cd: f64,
    pub cm: f64,
    pub xtr: [f64; 2],
    pub iblte: [usize; 2],
    /// Per side, (x, Cp, θ, δ*, Cf, N or √Cτ) from the stagnation point to
    /// the end of the wake.
    pub sides: [Vec<[f64; 6]>; 2],
}

/// Reads a gfortran sequential unformatted `PACC` dump file (4-byte record
/// markers, 8-byte reals, 4-byte integers).
pub fn read_dump(path: &Path) -> Vec<DumpPoint> {
    let bytes = std::fs::read(path).unwrap_or_else(|_| panic!("missing {}", path.display()));
    let mut records = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        let len = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
        records.push(&bytes[pos + 4..pos + 4 + len]);
        pos += len + 8;
    }
    let f64s = |r: &[u8]| -> Vec<f64> {
        r.chunks_exact(8)
            .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
            .collect()
    };
    let i32s = |r: &[u8]| -> Vec<i32> {
        r.chunks_exact(4)
            .map(|c| i32::from_le_bytes(c.try_into().unwrap()))
            .collect()
    };
    let side = |r: &[u8]| -> Vec<[f64; 6]> {
        f64s(r)
            .chunks_exact(6)
            .map(|c| [c[0], c[1], c[2], c[3], c[4], c[5]])
            .collect()
    };
    // Five header records, then four records per point.
    assert_eq!((records.len() - 5) % 4, 0, "{}", path.display());
    records[5..]
        .chunks_exact(4)
        .map(|r| {
            let f = f64s(r[0]);
            let idx = i32s(r[1]);
            DumpPoint {
                alpha: f[0],
                cl: f[1],
                cd: f[2],
                cm: f[4],
                xtr: [f[5], f[6]],
                iblte: [idx[2] as usize, idx[3] as usize],
                sides: [side(r[2]), side(r[3])],
            }
        })
        .collect()
}

/// A row of an XFOIL viscous polar file.
#[derive(Debug, Clone, Copy)]
pub struct ViscousPolarRow {
    pub alpha: f64,
    pub cl: f64,
    pub cd: f64,
    pub cdp: f64,
    pub cm: f64,
    pub cp_min: f64,
    pub xtr: [f64; 2],
}

/// Parses an XFOIL viscous polar file. The columns after Cpmin are, per
/// side, the transition x/c and its fractional panel index (the header
/// labels them in a different order).
pub fn parse_viscous_polar(text: &str) -> Vec<ViscousPolarRow> {
    let lines: Vec<&str> = text.lines().collect();
    let header = lines
        .iter()
        .position(|l| l.trim_start().starts_with("------"))
        .expect("polar header");
    lines[header + 1..]
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Vec<f64> = l.split_whitespace().map(|w| w.parse().unwrap()).collect();
            ViscousPolarRow {
                alpha: v[0],
                cl: v[1],
                cd: v[2],
                cdp: v[3],
                cm: v[4],
                cp_min: v[5],
                xtr: [v[6], v[8]],
            }
        })
        .collect()
}

/// Largest differences between solutions and XFOIL dump points.
#[derive(Debug, Clone, Default)]
pub struct ViscousStats {
    pub points: usize,
    /// Forces: CL, CM absolute; CD relative.
    pub forces: super::Worst,
    /// Transition x/c.
    pub transition: super::Worst,
    /// Boundary layer: x, Cp absolute; θ and δ* relative to the largest θ
    /// of the side; Cf relative to 0.01; N relative to 10 and √Cτ to 0.1.
    pub boundary_layer: super::Worst,
}

impl ViscousStats {
    pub fn merge(&mut self, other: &Self) {
        self.points += other.points;
        for (a, b) in [
            (&mut self.forces, &other.forces),
            (&mut self.transition, &other.transition),
            (&mut self.boundary_layer, &other.boundary_layer),
        ] {
            a.update(b.diff, || b.what.clone());
        }
    }

    pub fn worst(&self) -> f64 {
        self.forces
            .diff
            .max(self.transition.diff)
            .max(self.boundary_layer.diff)
    }
}

/// Compares a solution with an XFOIL dump point.
pub fn compare_viscous(s: &ViscousSolution, d: &DumpPoint, tag: &str, stats: &mut ViscousStats) {
    stats.points += 1;
    let f = &mut stats.forces;
    f.update((s.cl - d.cl).abs(), || {
        format!("{tag}: CL {} vs {}", s.cl, d.cl)
    });
    f.update((s.cm - d.cm).abs(), || {
        format!("{tag}: CM {} vs {}", s.cm, d.cm)
    });
    f.update((s.cd - d.cd).abs() / d.cd, || {
        format!("{tag}: CD {} vs {}", s.cd, d.cd)
    });
    for k in 0..2 {
        stats
            .transition
            .update((s.transition[k].x - d.xtr[k]).abs(), || {
                format!("{tag}: xtr[{k}] {} vs {}", s.transition[k].x, d.xtr[k])
            });
    }

    let check = |stats: &mut ViscousStats,
                 layer: &BoundaryLayer,
                 j: usize,
                 row: &[f64; 6],
                 scale: f64,
                 what: &str| {
        let n_or_ctau = layer.amplification[j]
            .or(layer.sqrt_ctau[j])
            .unwrap_or(f64::NAN);
        let ctau_scale = if layer.amplification[j].is_some() {
            10.0
        } else {
            0.1
        };
        let diffs = [
            ("x", (layer.x[j] - row[0]).abs()),
            ("Cp", (layer.cp[j] - row[1]).abs()),
            ("theta", (layer.theta[j] - row[2]).abs() / scale),
            ("dstar", (layer.dstar[j] - row[3]).abs() / scale),
            ("Cf", (layer.cf[j] - row[4]).abs() / 0.01),
            ("N/ctau", (n_or_ctau - row[5]).abs() / ctau_scale),
        ];
        for (name, diff) in diffs {
            let diff = if diff.is_nan() { f64::INFINITY } else { diff };
            stats
                .boundary_layer
                .update(diff, || format!("{tag}: {what} station {j}: {name}"));
        }
    };

    for (k, surface) in [&s.top, &s.bottom].into_iter().enumerate() {
        let rows = &d.sides[k];
        let iblte = d.iblte[k];
        let scale = rows.iter().map(|r| r[2]).fold(0.0, f64::max);
        let n_wake = rows.len() - iblte;
        if surface.x.len() != iblte - 1 || s.wake.x.len() != n_wake {
            stats.boundary_layer.update(f64::INFINITY, || {
                format!(
                    "{tag}: side {k} has {} + {} stations, XFOIL {} + {}",
                    surface.x.len(),
                    s.wake.x.len(),
                    iblte - 1,
                    n_wake
                )
            });
            continue;
        }
        for (j, row) in rows[1..iblte].iter().enumerate() {
            check(
                stats,
                surface,
                j,
                row,
                scale,
                if k == 0 { "top" } else { "bottom" },
            );
        }
        for (j, row) in rows[iblte..].iter().enumerate() {
            check(stats, &s.wake, j, row, scale, "wake");
        }
    }
}

/// Largest accepted differences with the dump files (scaled as described
/// in `ViscousStats`): forces, transition x/c, boundary layer. Most points
/// agree with XFOIL to 1e-10 or better. Round-off differences (which also
/// differ between debug and release builds) grow on points that need all
/// their Newton iterations, and Cf right at the transition point follows the
/// tiny shifts of the transition location.
pub const TOLERANCES: [f64; 3] = [1.0e-6, 1.0e-5, 2.0e-4];

/// Half a unit of the last printed digit, plus round-off.
const PRINTED_4: f64 = 5.1e-5;
const PRINTED_5: f64 = 5.1e-6;

pub fn viscous_solver(case: &Case, run: &ViscousRun) -> ViscousSolver {
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
pub fn check_viscous_case(case: &Case) -> (ViscousStats, Vec<String>) {
    let mut stats = ViscousStats::default();
    let mut failures = Vec::new();
    for run in case.viscous_runs() {
        let solver = viscous_solver(case, &run);
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

/// XFOIL's console lines that `ViscousSession::take_log` reproduces.
pub fn xfoil_log_lines(case: &Case, run: &ViscousRun) -> Vec<String> {
    let log = std::fs::read_to_string(case.dir.join(format!("xfoil_{}.log", run.name))).unwrap();
    log.lines()
        .filter(|l| {
            l.starts_with(" MRCHUE: Convergence")
                || l.starts_with(" MRCHUE: Inverse")
                || l.starts_with(" MRCHDU")
                || l.starts_with(" Side")
                || l.contains("   rms: ")
                || l.starts_with("       a =")
        })
        .map(|l| l.trim_end().to_string())
        .collect()
}

/// Our console log of the XFOIL session of a run: the polar sweep, then
/// the single points from a fresh boundary layer. The Reynolds number is
/// multiplied by `1 + eps` (sensitivity checks).
pub fn our_log_lines(case: &Case, run: &ViscousRun, eps: f64) -> Vec<String> {
    let mut perturbed = ViscousRun {
        name: run.name.clone(),
        options: run.options,
        polar: run.polar,
        points: run.points.clone(),
    };
    perturbed.options.reynolds *= 1.0 + eps;
    let solver = viscous_solver(case, &perturbed);
    let mut session = solver.session();
    session.record_log(true);
    session
        .sweep_with(&run.polar, |_, _| ControlFlow::Continue(()))
        .unwrap();
    for &alpha in &run.points {
        session.reset();
        // The log is kept whatever the outcome.
        let _ = session.solve(alpha);
    }
    session
        .take_log()
        .iter()
        .map(|l| l.trim_end().to_string())
        .collect()
}

/// How our console log compares with XFOIL's.
#[derive(Debug, Clone, PartialEq)]
pub enum LogComparison {
    /// Identical lines (as many as the shorter log).
    Identical { lines: usize },
    /// Identical up to `line`, where the two lines differ only in the last
    /// printed digit (3 units at most, or a tie on the variable with the
    /// largest change), or by no more than our own computation does when the
    /// Reynolds number is perturbed by 1e-15 to 1e-11 (relative): the iterations are there so sensitive that
    /// last-bit differences between two implementations (math libraries,
    /// operation order) change the path. `difference` is the relative
    /// difference with XFOIL, `sensitivity` the largest one between our
    /// perturbed and unperturbed runs (infinite when a perturbation changes
    /// an earlier line).
    RoundOff {
        line: usize,
        ours: String,
        xfoil: String,
        difference: f64,
        sensitivity: f64,
    },
    /// A difference our own sensitivity does not explain.
    Different {
        line: usize,
        ours: String,
        xfoil: String,
        difference: f64,
        sensitivity: f64,
    },
}

/// Is the token a Fortran real (`0.1183E+00`, `-1.1799`, `NaN`)?
fn real(token: &str) -> Option<f64> {
    if token.contains('.') || token == "NaN" || token.ends_with("Infinity") {
        token.parse::<f64>().ok().or(match token {
            "NaN" => Some(f64::NAN),
            "Infinity" => Some(f64::INFINITY),
            "-Infinity" => Some(f64::NEG_INFINITY),
            _ => None,
        })
    } else {
        None
    }
}

/// Relative difference between two console lines: the largest relative
/// difference of their reals when the words and integers are the same
/// (the variable of the largest change may differ when two are tied),
/// infinite otherwise.
pub fn line_difference(a: &str, b: &str) -> f64 {
    let ta: Vec<&str> = a.split_whitespace().collect();
    let tb: Vec<&str> = b.split_whitespace().collect();
    if ta.len() != tb.len() {
        return f64::INFINITY;
    }
    let variables = ["n", "C", "T", "D", "U"];
    let mut worst: f64 = 0.0;
    for (x, y) in ta.iter().zip(&tb) {
        if x == y {
            continue;
        }
        match (real(x), real(y)) {
            (Some(u), Some(v)) => {
                let d = (u - v).abs() / u.abs().max(v.abs()).max(1e-30);
                worst = worst.max(if d.is_nan() { f64::INFINITY } else { d });
            }
            _ if variables.contains(x) && variables.contains(y) => {}
            _ => return f64::INFINITY,
        }
    }
    worst
}

/// Lines that are equivalent for the computation:
/// - the same values with another variable named as the largest change: an
///   exact tie (at the first station of the similarity solution,
///   dθ/θ = dδ*/δ*) decided by the last bit;
/// - the same marching failure (routine, station, side) with residuals on
///   the same side of 0.1. The residual of a local Newton iteration that
///   failed after 25 steps is erratic; it only decides whether the station
///   is extrapolated from upstream (above 0.1, then its value does not
///   matter) or keeps its last iterate (below, then within 1 %).
fn same_or_tie(a: &str, b: &str) -> bool {
    if a == b || line_difference(a, b) == 0.0 {
        return true;
    }
    let failure = |l: &str| {
        let l = l.trim_start();
        (l.starts_with("MRCHUE: Convergence failed") || l.starts_with("MRCHDU: Convergence failed"))
            .then(|| {
                let (head, res) = l.rsplit_once('=')?;
                Some((head.to_string(), real(res.trim())?))
            })
            .flatten()
    };
    match (failure(a), failure(b)) {
        (Some((ha, ra)), Some((hb, rb))) if ha == hb => {
            let garbage = |r: f64| r > 0.1 || r.is_nan();
            match (garbage(ra), garbage(rb)) {
                (true, true) => true,
                (false, false) => (ra - rb).abs() <= 0.01 * ra.abs().max(rb.abs()),
                _ => false,
            }
        }
        _ => false,
    }
}

/// Compares our console log with XFOIL's (exact ties aside); at the first
/// difference, measures
/// the sensitivity of our own computation to tiny perturbations.
pub fn compare_logs(case: &Case, run: &ViscousRun) -> LogComparison {
    let ours = our_log_lines(case, run, 0.0);
    let xfoil = xfoil_log_lines(case, run);
    let Some(line) = ours
        .iter()
        .zip(&xfoil)
        .position(|(a, b)| !same_or_tie(a, b))
    else {
        return LogComparison::Identical {
            lines: ours.len().min(xfoil.len()),
        };
    };
    let difference = line_difference(&ours[line], &xfoil[line]);
    let mut sensitivity: f64 = 0.0;
    for eps in [
        1e-15, -1e-15, 1e-14, -1e-14, 1e-13, -1e-13, 1e-12, -1e-12, 1e-11, -1e-11,
    ] {
        let perturbed = our_log_lines(case, run, eps);
        match ours
            .iter()
            .zip(&perturbed)
            .position(|(a, b)| !same_or_tie(a, b))
        {
            Some(j) if j < line => sensitivity = f64::INFINITY,
            Some(j) if j == line => {
                sensitivity = sensitivity.max(line_difference(&ours[line], &perturbed[line]));
            }
            _ => {}
        }
        // Smallest perturbations first; stop once the difference is
        // explained.
        if sensitivity == f64::INFINITY || difference <= 10.0 * sensitivity {
            break;
        }
    }
    let (ours, xfoil) = (ours[line].clone(), xfoil[line].clone());
    // A difference in the last printed digit, or within an order of
    // magnitude of our own sensitivity.
    let explained =
        sensitivity > 0.0 && (sensitivity == f64::INFINITY || difference <= 10.0 * sensitivity);
    if round_off_only(&ours, &xfoil) || explained {
        LogComparison::RoundOff {
            line,
            ours,
            xfoil,
            difference,
            sensitivity,
        }
    } else {
        LogComparison::Different {
            line,
            ours,
            xfoil,
            difference,
            sensitivity,
        }
    }
}
/// Unit of the last printed digit of a Fortran real (`0.1183E+00`,
/// `-1.1799`), or `None` for a word or an integer.
fn last_digit_unit(token: &str) -> Option<f64> {
    let (mantissa, exponent) = match token.find(['E', 'e']) {
        Some(k) => (&token[..k], token[k + 1..].parse::<i32>().ok()?),
        None => (token, 0),
    };
    let dot = mantissa.find('.')?;
    let decimals = mantissa.len() - dot - 1;
    token.parse::<f64>().ok()?;
    Some(10f64.powi(exponent - decimals as i32))
}

fn round_off_only(ours: &str, xfoil: &str) -> bool {
    let a: Vec<&str> = ours.split_whitespace().collect();
    let b: Vec<&str> = xfoil.split_whitespace().collect();
    if a.len() != b.len() {
        return false;
    }
    let variables = ["n", "C", "T", "D", "U"];
    a.iter().zip(&b).all(|(x, y)| {
        if x == y {
            return true;
        }
        match (last_digit_unit(x), last_digit_unit(y)) {
            (Some(unit), Some(_)) => {
                let (vx, vy) = (x.parse::<f64>().unwrap(), y.parse::<f64>().unwrap());
                (vx - vy).abs() <= 3.0 * unit * (1.0 + 1e-9)
            }
            // The variable with the largest change, when two are tied.
            _ => variables.contains(x) && variables.contains(y),
        }
    })
}
