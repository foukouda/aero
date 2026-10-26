//! Readers for the XFOIL viscous reference data (`tests/viscous`, see
//! tests/viscous/README.md) and the comparison of a viscous solution with
//! XFOIL's full-precision dump files.

use std::path::{Path, PathBuf};

use aero_foil2d::inviscid::AlphaSweep;
use aero_foil2d::viscous::{BoundaryLayer, ViscousOptions, ViscousSolution};

use super::Case;

pub fn viscous_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/viscous")
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
