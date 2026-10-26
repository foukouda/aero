//! Comparisons of the internal viscous session with XFOIL 6.996 outputs
//! (`tests/viscous`).

// Index loops mirror XFOIL's DO loops.
#![allow(clippy::needless_range_loop)]

use std::path::{Path, PathBuf};

use aero_geometry::{NacaOptions, PanelingOptions, naca};

use super::session::{Parameters, Session};
use crate::inviscid::{InviscidOptions, InviscidSolver};

// XFOIL's `ALFA`, `ASEQ`, `FCPMIN` and `PLXADD` (dump file) on the internal
// session, to compare it with XFOIL's files at full precision.

/// Number of successive unconverged points that stops a sequence (XFOIL
/// `NSEQEX`).
const NSEQEX: usize = 4;

/// Forces and transition points of an operating point (a line of XFOIL's
/// polar file).
#[derive(Debug, Clone, Copy, PartialEq)]
struct PointOutput {
    pub alpha_deg: f64,
    pub cl: f64,
    pub cd: f64,
    pub cdp: f64,
    pub cdf: f64,
    pub cm: f64,
    pub cpmin: f64,
    /// Transition x/c (top, bottom).
    pub xtr: [f64; 2],
    /// Fractional panel index of the transition points.
    pub tindex: [f64; 2],
    pub converged: bool,
    pub iterations: usize,
    pub rms: f64,
}

/// Boundary-layer distributions of one side as written by XFOIL's `PACC`
/// dump file (`PLXADD`): stagnation point, airfoil stations and, on both
/// sides, the wake.
#[derive(Debug, Clone, Default, PartialEq)]
struct SideDistributions {
    pub x: Vec<f64>,
    pub cp: Vec<f64>,
    pub theta: Vec<f64>,
    pub dstar: Vec<f64>,
    pub cf: Vec<f64>,
    /// Amplification N before transition, √Cτ after.
    pub ctau: Vec<f64>,
    /// Trailing-edge station (1-based, as in XFOIL).
    pub iblte: usize,
}

impl Session<'_> {
    /// Forces and transition points of the current solution (`FCPMIN`,
    /// `PLRADD`).
    fn output(&self, iterations: usize, converged: bool) -> PointOutput {
        let cpmin = self.cpv().into_iter().fold(f64::INFINITY, f64::min);
        PointOutput {
            alpha_deg: self.alfa.to_degrees(),
            cl: self.cl,
            cd: self.cd,
            cdp: self.cdp,
            cdf: self.cdf,
            cm: self.cm,
            cpmin,
            xtr: [self.sides[0].xoctr, self.sides[1].xoctr],
            tindex: [self.sides[0].tindex, self.sides[1].tindex],
            converged,
            iterations,
            rms: self.rmsbl,
        }
    }

    /// Boundary-layer distributions as XFOIL's `PLXADD` writes them.
    fn distributions(&self) -> [SideDistributions; 2] {
        let minf = self.par.mach;
        let beta = (1.0 - minf * minf).sqrt();
        let bfac = 0.5 * minf * minf / (1.0 + beta);
        let que = 0.5 * self.qinf * self.qinf;
        let cpv = self.cpv();
        let geo = self.geo();
        let (top, bot) = (&self.sides[0], &self.sides[1]);
        let nside = [(top.iblte + (bot.nbl - bot.iblte)).max(2), bot.nbl.max(2)];

        // Stagnation point.
        let x0 = aero_geometry::spline::seval(self.sst, &geo.x, &geo.xp, &geo.s);
        let cp0 = 1.0 / (beta + bfac);
        let th0 = 0.5 * (top.thet[2] + bot.thet[2]);
        let ds0 = 0.5 * (top.dstr[2] + bot.dstr[2]);

        let mut out = [SideDistributions::default(), SideDistributions::default()];
        for (is, side) in self.sides.iter().enumerate() {
            let d = &mut out[is];
            d.iblte = side.iblte;
            d.x.push(x0);
            d.cp.push(cp0);
            d.theta.push(th0);
            d.dstar.push(ds0);
            d.cf.push(0.0);
            d.ctau.push(0.0);
            for ibl in 2..=nside[is] {
                let i = side.ipan[ibl];
                d.x.push(self.pan.x[i]);
                d.cp.push(cpv[i]);
                d.theta.push(side.thet[ibl]);
                d.dstar.push(side.dstr[ibl]);
                d.cf.push(side.tau[ibl] / que);
                d.ctau.push(side.ctau[ibl]);
            }
        }
        out
    }

    /// XFOIL `ALFA` in viscous mode: one point at `alpha_deg`, starting from
    /// the current boundary layer (call [`Self::reset_boundary_layer`] first
    /// for XFOIL's `INIT`).
    fn alfa(&mut self, alpha_deg: f64, itmax: usize) -> PointOutput {
        self.set_alpha(alpha_deg.to_radians());
        let (iterations, converged) = self.viscal(itmax);
        self.output(iterations, converged)
    }

    /// XFOIL `ASEQ`: points from `a1` to `a2` by `da` (degrees), each one
    /// starting from the boundary layer of the previous one, with `itmax + 5`
    /// iterations per point. Stops after `NSEQEX` successive unconverged
    /// points. `visit` receives every computed point.
    fn aseq(
        &mut self,
        a1: f64,
        a2: f64,
        da: f64,
        itmax: usize,
        mut visit: impl FnMut(&Session<'_>, &PointOutput),
    ) {
        let dtor = std::f64::consts::PI / 180.0;
        let daa = if a2 < a1 { -da.abs() } else { da.abs() };
        let (aa1, aa2, daa) = (a1 * dtor, a2 * dtor, daa * dtor);
        let npoint = if daa != 0.0 {
            ((aa2 - aa1) / daa + 0.5) as usize + 1
        } else {
            1
        };
        let mut iseqex = 0;
        for ipoint in 1..=npoint {
            let alfa = aa1 + daa * (ipoint - 1) as f64;
            self.set_alpha(alfa);
            let (iterations, converged) = self.viscal(itmax + 5);
            let out = self.output(iterations, converged);
            visit(self, &out);
            if converged {
                iseqex = 0;
            } else {
                iseqex += 1;
                if iseqex >= NSEQEX {
                    break;
                }
            }
        }
    }
}

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
fn compare(s: &Session<'_>, out: &PointOutput, r: &DumpPoint) -> f64 {
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
    // The 14° point needs all 25 iterations: round-off differences (which
    // differ between debug and release builds, powi) grow to about 1e-5.
    assert!(worst < 2e-5, "{worst:e}");
}
