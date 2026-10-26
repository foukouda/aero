//! Comparisons of the internal viscous session with XFOIL 6.996 outputs
//! (`tests/viscous`).

// Index loops mirror XFOIL's DO loops.
#![allow(clippy::needless_range_loop)]

use std::path::{Path, PathBuf};

use aero_geometry::{NacaOptions, PanelingOptions, naca};

use super::session::{Parameters, Session};
use crate::inviscid::{InviscidOptions, InviscidSolver};

fn viscous_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/viscous")
}

/// A point of an XFOIL `PACC` dump file.
#[derive(Debug, Clone)]
pub(crate) struct DumpPoint {
    pub alpha: f64,
    pub cl: f64,
    pub cd: f64,
    pub cm: f64,
    pub xtr: [f64; 2],
    pub iblte: [usize; 2],
    /// Per side: (x, cp, θ, δ*, cf, ctau) at every station.
    pub sides: [Vec<[f64; 6]>; 2],
}

/// Reads a gfortran sequential unformatted `PACC` dump file.
pub(crate) fn read_dump(path: &Path) -> Vec<DumpPoint> {
    let bytes = std::fs::read(path).unwrap();
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
    // Header: name, settings, types, sizes, nodes.
    let mut points = Vec::new();
    let mut k = 5;
    while k + 3 < records.len() + 1 && k < records.len() {
        let f = f64s(records[k]);
        let idx = i32s(records[k + 1]);
        let side = |r: &[u8]| -> Vec<[f64; 6]> {
            f64s(r)
                .chunks_exact(6)
                .map(|c| [c[0], c[1], c[2], c[3], c[4], c[5]])
                .collect()
        };
        points.push(DumpPoint {
            alpha: f[0],
            cl: f[1],
            cd: f[2],
            cm: f[4],
            xtr: [f[5], f[6]],
            iblte: [idx[2] as usize, idx[3] as usize],
            sides: [side(records[k + 2]), side(records[k + 3])],
        });
        k += 4;
    }
    points
}

fn naca_solver(code: &str) -> InviscidSolver {
    let af = naca(code, &NacaOptions::default())
        .unwrap()
        .paneled(&PanelingOptions::default())
        .unwrap();
    InviscidSolver::new(&af, &InviscidOptions::default()).unwrap()
}

fn params(re: f64, mach: f64) -> Parameters {
    Parameters {
        reynolds: re,
        mach,
        ncrit: [9.0, 9.0],
        xtr: [1.0, 1.0],
        damp2: false,
        vaccel: 0.01,
        wake_length: 1.0,
        moment_reference: [0.25, 0.0],
    }
}

#[test]
fn dump_reader_reads_the_reference_files() {
    let pts = read_dump(&viscous_dir().join("naca0012/vpdump_re1e6.bin"));
    assert_eq!(pts.len(), 4);
    assert_eq!(pts[1].alpha, 4.0);
    assert!((pts[1].cl - 0.4278).abs() < 1e-4);
}

/// Largest relative difference between our forces and boundary layer and
/// those of an XFOIL dump point.
fn compare(s: &Session<'_>, out: &super::viscal::PointOutput, r: &DumpPoint) -> f64 {
    let mut worst: f64 = 0.0;
    let mut check = |ours: f64, theirs: f64, scale: f64| {
        worst = worst.max((ours - theirs).abs() / scale);
    };
    check(out.cl, r.cl, 1.0);
    check(out.cd, r.cd, r.cd);
    check(out.cm, r.cm, 1.0);
    check(out.xtr[0], r.xtr[0], 1.0);
    check(out.xtr[1], r.xtr[1], 1.0);
    let ours = s.distributions();
    for is in 0..2 {
        assert_eq!(ours[is].iblte, r.iblte[is]);
        assert_eq!(ours[is].x.len(), r.sides[is].len());
        let d = &ours[is];
        for (k, row) in r.sides[is].iter().enumerate() {
            let theta_scale = r.sides[is].iter().map(|v| v[2]).fold(0.0, f64::max);
            check(d.x[k], row[0], 1.0);
            check(d.cp[k], row[1], 1.0);
            check(d.theta[k], row[2], theta_scale);
            check(d.dstar[k], row[3], theta_scale);
            check(d.cf[k], row[4], 1.0e-2);
            check(d.ctau[k], row[5], if row[5] > 1.0 { 10.0 } else { 0.1 });
        }
    }
    worst
}

#[test]
fn naca0012_single_points_match_xfoil() {
    let inv = naca_solver("0012");
    let mut s = Session::new(&inv, params(1.0e6, 0.0));
    let refs = read_dump(&viscous_dir().join("naca0012/vpdump_re1e6.bin"));
    // Newton iterations of the cold-started points in XFOIL's log.
    let iterations = [6, 7, 5, 7];
    for (r, it) in refs.iter().zip(iterations) {
        s.reset_boundary_layer();
        let out = s.alfa(r.alpha, 20);
        assert!(out.converged);
        assert_eq!(out.iterations, it, "alpha {}", r.alpha);
        let worst = compare(&s, &out, r);
        assert!(worst < 1e-9, "alpha {}: {worst:e}", r.alpha);
    }
}

#[test]
fn naca0012_sequence_matches_xfoil() {
    // Each point starts from the boundary layer of the previous one.
    let inv = naca_solver("0012");
    let mut s = Session::new(&inv, params(1.0e6, 0.0));
    let refs = read_dump(&viscous_dir().join("naca0012/vdump_re1e6.bin"));
    let mut k = 0;
    let mut worst: f64 = 0.0;
    s.aseq(-2.0, 16.0, 1.0, 20, |s, o| {
        if o.converged {
            worst = worst.max(compare(s, o, &refs[k]));
            k += 1;
        }
    });
    assert_eq!(k, refs.len());
    // The 14° point needs all 25 iterations: round-off differences grow.
    assert!(worst < 1e-6, "{worst:e}");
}
