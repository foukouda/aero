//! Mixed inverse design, a port of XFOIL's `QDES`: on a segment of the
//! surface the speed is prescribed and the shape is the unknown, elsewhere
//! the shape is kept and the speed is the unknown. The panel equations
//! `ψ(node) = ψ₀` are solved by Newton iteration for the normal displacements
//! of the segment nodes (XFOIL `MIXED`, with `PSILIN`'s geometric
//! sensitivities `GEOLIN` and `GAMLIN`).

use aero_geometry::{Airfoil, SplineCurve};
use serde::{Deserialize, Serialize};

use super::error::{InverseError, Result};
use crate::inviscid::{InviscidOptions, InviscidSolver};

const QOPI: f64 = 0.25 / std::f64::consts::PI;
const HOPI: f64 = 0.5 / std::f64::consts::PI;

/// Target surface speed of the mixed inverse (XFOIL `QSPEC`), at the nodes
/// of the paneled airfoil, with the normalized arc length of the nodes
/// (XFOIL `SSPEC`). Speeds are signed as XFOIL's `γ`: positive on the upper
/// surface from the trailing edge to the stagnation point, negative after.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpeedTarget {
    /// Angle of attack the target was taken at, degrees.
    pub alpha_deg: f64,
    /// Arc length of the nodes divided by the total arc length.
    pub s: Vec<f64>,
    /// Target speed `q/V∞` at each node.
    pub speed: Vec<f64>,
}

impl SpeedTarget {
    /// The inviscid surface speed of the airfoil at `alpha_deg` (XFOIL
    /// `QDES` start, `GAMQSP`), as the starting target.
    pub fn from_airfoil(paneled: &Airfoil, alpha_deg: f64) -> Result<Self> {
        let state = Initial::new(paneled, alpha_deg)?;
        let total = state.s[state.s.len() - 1];
        Ok(Self {
            alpha_deg,
            s: state.s.iter().map(|v| v / total).collect(),
            speed: state.gam,
        })
    }

    /// Makes the target symmetric about the chord line (XFOIL `SYMM`,
    /// `SYMQSP`): the two surfaces get the mean of their speeds and arc
    /// lengths, for a symmetric airfoil at zero lift.
    pub fn symmetrize(&mut self) {
        let nsp = self.speed.len();
        if nsp == 0 || self.s.len() != nsp {
            return;
        }
        self.alpha_deg = 0.0;
        let mid = 0.5 * (self.s[nsp - 1] - self.s[0]);
        for i in 0..nsp.div_ceil(2) {
            let j = nsp - 1 - i;
            self.s[i] = mid + 0.5 * (self.s[i] - self.s[j]);
            self.speed[i] = 0.5 * (self.speed[i] - self.speed[j]);
        }
        for i in nsp.div_ceil(2)..nsp {
            let j = nsp - 1 - i;
            self.s[i] = -self.s[j] + 2.0 * mid;
            self.speed[i] = -self.speed[j];
        }
    }

    /// Smooths the target between nodes `first` and `last` (XFOIL `SMOO`,
    /// `SMOOQ`), keeping the end values; with `slope_matching` (XFOIL
    /// `SLOP`, on by default) the slopes at the ends are kept too.
    pub fn smooth(&mut self, first: usize, last: usize, slope_matching: bool) -> Result<()> {
        let n = self.speed.len();
        if first >= last || last >= n || self.s.len() != n {
            return Err(InverseError::InvalidSegment {
                first,
                last,
                nodes: n,
            });
        }
        smooq(&self.s, &mut self.speed, first, last, slope_matching);
        Ok(())
    }
}

/// XFOIL `TRISOL`: tridiagonal solve, `a` diagonal, `b` sub, `c` super.
fn trisol(a: &mut [f64], b: &[f64], c: &mut [f64], d: &mut [f64]) {
    let kk = a.len();
    for k in 1..kk {
        let km = k - 1;
        c[km] /= a[km];
        d[km] /= a[km];
        a[k] -= b[k] * c[km];
        d[k] -= b[k] * d[km];
    }
    d[kk - 1] /= a[kk - 1];
    for k in (0..kk - 1).rev() {
        d[k] -= c[k] * d[k + 1];
    }
}

/// XFOIL `SMOOQ` on the arc-length coordinate `w8`, nodes `kq1..=kq2`.
pub(crate) fn smooq(w8: &[f64], qspec: &mut [f64], kq1: usize, kq2: usize, lqslop: bool) {
    if kq2 < kq1 + 2 {
        return; // segment too short
    }
    let nsp = w8.len();
    let smool = 0.002 * (w8[nsp - 1] - w8[0]);
    let smoosq = smool * smool;
    let len = kq2 - kq1 + 1;
    let (mut w1, mut w2, mut w3) = (vec![0.0; len], vec![0.0; len], vec![0.0; len]);
    for i in kq1 + 1..kq2 {
        let dsm = w8[i] - w8[i - 1];
        let dsp = w8[i + 1] - w8[i];
        let dso = 0.5 * (w8[i + 1] - w8[i - 1]);
        let k = i - kq1;
        w1[k] = smoosq * (-1.0 / dsm) / dso;
        w2[k] = smoosq * (1.0 / dsp + 1.0 / dsm) / dso + 1.0;
        w3[k] = smoosq * (-1.0 / dsp) / dso;
    }
    // Fixed end values.
    w2[0] = 1.0;
    w3[0] = 0.0;
    w1[len - 1] = 0.0;
    w2[len - 1] = 1.0;
    if lqslop {
        // Slope matching at both ends.
        let i = kq1 + 1;
        let dsm = w8[i] - w8[i - 1];
        let dsp = w8[i + 1] - w8[i];
        let ds = w8[i + 1] - w8[i - 1];
        let k = i - kq1;
        w1[k] = -1.0 / dsm - (dsm / ds) / dsm;
        w2[k] = 1.0 / dsm + (dsm / ds) / dsm + (dsm / ds) / dsp;
        w3[k] = -(dsm / ds) / dsp;
        let qspp1 = w1[k] * qspec[i - 1] + w2[k] * qspec[i] + w3[k] * qspec[i + 1];

        let i = kq2 - 1;
        let dsm = w8[i] - w8[i - 1];
        let dsp = w8[i + 1] - w8[i];
        let ds = w8[i + 1] - w8[i - 1];
        let k = i - kq1;
        w1[k] = (dsp / ds) / dsm;
        w2[k] = -1.0 / dsp - (dsp / ds) / dsp - (dsp / ds) / dsm;
        w3[k] = 1.0 / dsp + (dsp / ds) / dsp;
        let qspp2 = w1[k] * qspec[i - 1] + w2[k] * qspec[i] + w3[k] * qspec[i + 1];

        qspec[kq1 + 1] = qspp1;
        qspec[kq2 - 1] = qspp2;
    }
    trisol(&mut w2, &w1, &mut w3, &mut qspec[kq1..=kq2]);
}

/// XFOIL `GAUSS`: Gaussian elimination with partial pivoting, one
/// right-hand side, `z` row-major `nn × nn`.
pub(crate) fn gauss(z: &mut [Vec<f64>], r: &mut [f64]) {
    let nn = r.len();
    for np in 0..nn - 1 {
        let np1 = np + 1;
        let mut nx = np;
        for n in np1..nn {
            if z[n][np].abs() > z[nx][np].abs() {
                nx = n;
            }
        }
        let pivot = 1.0 / z[nx][np];
        // Swap the rows (the entries left of the pivot are not used again)
        // and normalize the pivot row.
        z.swap(nx, np);
        z[np][np1..].iter_mut().for_each(|v| *v *= pivot);
        r.swap(nx, np);
        r[np] *= pivot;
        for k in np1..nn {
            let ztmp = z[k][np];
            let (top, bottom) = z.split_at_mut(k);
            let pivot_row = &top[np];
            let row = &mut bottom[0];
            for l in np1..nn {
                row[l] -= ztmp * pivot_row[l];
            }
            r[k] -= ztmp * r[np];
        }
    }
    r[nn - 1] /= z[nn - 1][nn - 1];
    for np in (0..nn - 1).rev() {
        for k in np + 1..nn {
            r[np] -= z[np][k] * r[k];
        }
    }
}

/// XFOIL `ATANC`: `atan2(y, x)` continued from `thold`.
fn atanc(y: f64, x: f64, thold: f64) -> f64 {
    use std::f64::consts::{PI, TAU};
    let thnew = y.atan2(x);
    let dthet = thnew - thold;
    let dtcorr = dthet - TAU * ((dthet + PI.copysign(dthet)) / TAU).trunc();
    thold + dtcorr
}

/// XFOIL's state when `QDES` executes: the paneled airfoil, its splines and
/// panel quantities, and the inviscid solution at the current angle.
struct Initial {
    x: Vec<f64>,
    y: Vec<f64>,
    s: Vec<f64>,
    /// Spline slopes of the initial airfoil (XFOIL `XP`, `YP`), which
    /// `MIXED` does not update.
    xp: Vec<f64>,
    yp: Vec<f64>,
    /// Panel angles of the initial airfoil (`APANEL`), not updated either.
    apanel: Vec<f64>,
    /// Trailing-edge midpoint and chord of the initial airfoil.
    te: [f64; 2],
    chord: f64,
    gam: Vec<f64>,
    psio: f64,
    alfa: f64,
}

impl Initial {
    fn new(paneled: &Airfoil, alpha_deg: f64) -> Result<Self> {
        if !alpha_deg.is_finite() {
            return Err(InverseError::InvalidOption(
                "the angle of attack must be finite".into(),
            ));
        }
        let solver = InviscidSolver::new(paneled, &InviscidOptions::default())?;
        let geo = solver.geometry();
        let n = geo.len();
        let alfa = alpha_deg.to_radians();
        let (sina, cosa) = alfa.sin_cos();
        let base = solver.base_solutions();
        let gam = (0..n)
            .map(|i| cosa * base[(i, 0)] + sina * base[(i, 1)])
            .collect();
        let psio = cosa * base[(n, 0)] + sina * base[(n, 1)];
        Ok(Self {
            x: geo.x.clone(),
            y: geo.y.clone(),
            s: geo.s.clone(),
            xp: geo.xp.clone(),
            yp: geo.yp.clone(),
            apanel: geo.apanel.clone(),
            te: geo.te,
            chord: geo.chord,
            gam,
            psio,
            alfa,
        })
    }
}

/// Trailing-edge panel quantities (XFOIL `TECALC`), from the current nodes
/// and the slopes of the initial airfoil.
#[derive(Clone, Copy)]
struct TrailingEdge {
    scs: f64,
    sds: f64,
    sharp: bool,
}

fn tecalc(x: &[f64], y: &[f64], xp: &[f64], yp: &[f64], chord: f64) -> TrailingEdge {
    let n = x.len();
    let dxte = x[0] - x[n - 1];
    let dyte = y[0] - y[n - 1];
    let dxs = 0.5 * (-xp[0] + xp[n - 1]);
    let dys = 0.5 * (-yp[0] + yp[n - 1]);
    let ante = dxs * dyte - dys * dxte;
    let aste = dxs * dxte + dys * dyte;
    let dste = dxte.hypot(dyte);
    let sharp = dste < 0.0001 * chord;
    if sharp {
        TrailingEdge {
            scs: 1.0,
            sds: 0.0,
            sharp,
        }
    } else {
        TrailingEdge {
            scs: ante / dste,
            sds: aste / dste,
            sharp,
        }
    }
}

/// XFOIL `NCALC`: unit normals from the splined contour.
fn ncalc(x: &[f64], y: &[f64]) -> Result<(Vec<f64>, Vec<f64>)> {
    let curve = SplineCurve::new(x, y)?;
    let (xn, yn) = (curve.dx_nodes(), curve.dy_nodes());
    let s = curve.arc_lengths();
    let n = x.len();
    let mut nx = vec![0.0; n];
    let mut ny = vec![0.0; n];
    for i in 0..n {
        let (sx, sy) = (yn[i], -xn[i]);
        let smod = sx.hypot(sy);
        if smod == 0.0 {
            nx[i] = -1.0;
            ny[i] = 0.0;
        } else {
            nx[i] = sx / smod;
            ny[i] = sy / smod;
        }
    }
    // Average the normals at corners.
    for i in 0..n - 1 {
        if s[i] == s[i + 1] {
            let (sx, sy) = (0.5 * (nx[i] + nx[i + 1]), 0.5 * (ny[i] + ny[i + 1]));
            let smod = sx.hypot(sy);
            let (a, b) = if smod == 0.0 {
                (-1.0, 0.0)
            } else {
                (sx / smod, sy / smod)
            };
            (nx[i], ny[i], nx[i + 1], ny[i + 1]) = (a, b, a, b);
        }
    }
    Ok((nx, ny))
}

fn scalc(x: &[f64], y: &[f64]) -> Vec<f64> {
    let mut s = vec![0.0; x.len()];
    for i in 1..x.len() {
        s[i] = s[i - 1] + (x[i] - x[i - 1]).hypot(y[i] - y[i - 1]);
    }
    s
}

/// Panel geometry seen by `PSILIN` during the iterations.
struct Panels<'a> {
    x: &'a [f64],
    y: &'a [f64],
    s: &'a [f64],
    nx: &'a [f64],
    ny: &'a [f64],
    apanel: &'a [f64],
    te: TrailingEdge,
}

/// Results of XFOIL `PSILIN` with the current `γ` (no source sheet).
struct Psilin {
    psi: f64,
    psi_ni: f64,
    dzdg: Vec<f64>,
    dqdg: Vec<f64>,
    dzdn: Vec<f64>,
    z_qdof: [f64; 4],
}

impl Psilin {
    fn new(n: usize) -> Self {
        Self {
            psi: 0.0,
            psi_ni: 0.0,
            dzdg: vec![0.0; n],
            dqdg: vec![0.0; n],
            dzdn: vec![0.0; n],
            z_qdof: [0.0; 4],
        }
    }
}

/// XFOIL `PSILIN(I, XI, YI, NXI, NYI, PSI, PSI_NI, GEOLIN, .FALSE.)`:
/// streamfunction at a point from the vortex sheet `gam`, the trailing-edge
/// panel and the freestream, with its sensitivities to `γ` and, if
/// `geolin`, to the normal displacements of the nodes and to the target
/// speed degrees of freedom `qf`.
#[allow(clippy::too_many_arguments)]
fn psilin(
    p: &Panels<'_>,
    gam: &[f64],
    qf: &[Vec<f64>; 4],
    io: Option<usize>,
    xi: f64,
    yi: f64,
    nxi: f64,
    nyi: f64,
    alfa: f64,
    geolin: bool,
    out: &mut Psilin,
) {
    use std::f64::consts::PI;
    let (x, y) = (p.x, p.y);
    let n = x.len();
    let seps = (p.s[n - 1] - p.s[0]) * 1.0e-5;
    let (sina, cosa) = alfa.sin_cos();
    let qinf = 1.0;
    *out = Psilin::new(n);
    let (scs, sds) = if p.te.sharp {
        (1.0, 0.0)
    } else {
        (p.te.scs, p.te.sds)
    };
    let on_airfoil = io.is_some();

    'panels: for jo in 0..n {
        let mut jp = jo + 1;
        if jo == n - 1 {
            jp = 0;
            if (x[jo] - x[jp]).powi(2) + (y[jo] - y[jp]).powi(2) < seps * seps {
                break 'panels; // closed trailing edge: no TE panel
            }
        }
        let dso = (x[jo] - x[jp]).hypot(y[jo] - y[jp]);
        if dso == 0.0 {
            continue;
        }
        let dsio = 1.0 / dso;
        let apan = p.apanel[jo];
        let rx1 = xi - x[jo];
        let ry1 = yi - y[jo];
        let rx2 = xi - x[jp];
        let ry2 = yi - y[jp];
        let sx = (x[jp] - x[jo]) * dsio;
        let sy = (y[jp] - y[jo]) * dsio;
        let x1 = sx * rx1 + sy * ry1;
        let x2 = sx * rx2 + sy * ry2;
        let yy = sx * ry1 - sy * rx1;
        let rs1 = rx1 * rx1 + ry1 * ry1;
        let rs2 = rx2 * rx2 + ry2 * ry2;
        let sgn = if on_airfoil || yy >= 0.0 { 1.0 } else { -1.0 };
        let branch = (0.5 - 0.5 * sgn) * PI;
        let (g1, t1) = if io != Some(jo) && rs1 > 0.0 {
            (rs1.ln(), (sgn * x1).atan2(sgn * yy) + branch)
        } else {
            (0.0, 0.0)
        };
        let (g2, t2) = if io != Some(jp) && rs2 > 0.0 {
            (rs2.ln(), (sgn * x2).atan2(sgn * yy) + branch)
        } else {
            (0.0, 0.0)
        };
        let x1i = sx * nxi + sy * nyi;
        let x2i = x1i;
        let yyi = sx * nyi - sy * nxi;

        let (mut x1o, mut x1p, mut x2o, mut x2p, mut yyo, mut yyp) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        if geolin {
            let (nxo, nyo, nxp, nyp) = (p.nx[jo], p.ny[jo], p.nx[jp], p.ny[jp]);
            x1o = -((rx1 - x1 * sx) * nxo + (ry1 - x1 * sy) * nyo) * dsio - (sx * nxo + sy * nyo);
            x1p = ((rx1 - x1 * sx) * nxp + (ry1 - x1 * sy) * nyp) * dsio;
            x2o = -((rx2 - x2 * sx) * nxo + (ry2 - x2 * sy) * nyo) * dsio;
            x2p = ((rx2 - x2 * sx) * nxp + (ry2 - x2 * sy) * nyp) * dsio - (sx * nxp + sy * nyp);
            yyo = ((rx1 + x1 * sy) * nyo - (ry1 - x1 * sx) * nxo) * dsio - (sx * nyo - sy * nxo);
            yyp = -((rx1 - x1 * sy) * nyp - (ry1 + x1 * sx) * nxp) * dsio;
        }

        if jo == n - 1 {
            // Trailing-edge panel: uniform source and vortex.
            let psig = 0.5 * yy * (g1 - g2) + x2 * (t2 - apan) - x1 * (t1 - apan);
            let pgam = 0.5 * x1 * g1 - 0.5 * x2 * g2 + x2 - x1 + yy * (t1 - t2);
            let psigx1 = -(t1 - apan);
            let psigx2 = t2 - apan;
            let psigyy = 0.5 * (g1 - g2);
            let pgamx1 = 0.5 * g1;
            let pgamx2 = -0.5 * g2;
            let pgamyy = t1 - t2;
            let psigni = psigx1 * x1i + psigx2 * x2i + psigyy * yyi;
            let pgamni = pgamx1 * x1i + pgamx2 * x2i + pgamyy * yyi;
            let sigte = 0.5 * scs * (gam[jp] - gam[jo]);
            let gamte = -0.5 * sds * (gam[jp] - gam[jo]);

            out.psi += HOPI * (psig * sigte + pgam * gamte);
            out.dzdg[jo] -= HOPI * psig * scs * 0.5;
            out.dzdg[jp] += HOPI * psig * scs * 0.5;
            out.dzdg[jo] += HOPI * pgam * sds * 0.5;
            out.dzdg[jp] -= HOPI * pgam * sds * 0.5;
            out.psi_ni += HOPI * (psigni * sigte + pgamni * gamte);
            out.dqdg[jo] -= HOPI * (psigni * 0.5 * scs - pgamni * 0.5 * sds);
            out.dqdg[jp] += HOPI * (psigni * 0.5 * scs - pgamni * 0.5 * sds);
            if geolin {
                out.dzdn[jo] += HOPI * (psigx1 * x1o + psigx2 * x2o + psigyy * yyo) * sigte
                    + HOPI * (pgamx1 * x1o + pgamx2 * x2o + pgamyy * yyo) * gamte;
                out.dzdn[jp] += HOPI * (psigx1 * x1p + psigx2 * x2p + psigyy * yyp) * sigte
                    + HOPI * (pgamx1 * x1p + pgamx2 * x2p + pgamyy * yyp) * gamte;
                for (k, f) in qf.iter().enumerate() {
                    out.z_qdof[k] += HOPI * psig * 0.5 * (f[jp] - f[jo]) * scs
                        - HOPI * pgam * 0.5 * (f[jp] - f[jo]) * sds;
                }
            }
            break 'panels;
        }

        // Vortex panel.
        let dxinv = 1.0 / (x1 - x2);
        let psis = 0.5 * x1 * g1 - 0.5 * x2 * g2 + x2 - x1 + yy * (t1 - t2);
        let psid = ((x1 + x2) * psis + 0.5 * (rs2 * g2 - rs1 * g1 + x1 * x1 - x2 * x2)) * dxinv;
        let psx1 = 0.5 * g1;
        let psx2 = -0.5 * g2;
        let psyy = t1 - t2;
        let pdx1 = ((x1 + x2) * psx1 + psis - x1 * g1 - psid) * dxinv;
        let pdx2 = ((x1 + x2) * psx2 + psis + x2 * g2 + psid) * dxinv;
        let pdyy = ((x1 + x2) * psyy - yy * (g1 - g2)) * dxinv;
        let gsum = gam[jp] + gam[jo];
        let gdif = gam[jp] - gam[jo];

        out.psi += QOPI * (psis * gsum + psid * gdif);
        out.dzdg[jo] += QOPI * (psis - psid);
        out.dzdg[jp] += QOPI * (psis + psid);
        let psni = psx1 * x1i + psx2 * x2i + psyy * yyi;
        let pdni = pdx1 * x1i + pdx2 * x2i + pdyy * yyi;
        out.psi_ni += QOPI * (gsum * psni + gdif * pdni);
        out.dqdg[jo] += QOPI * (psni - pdni);
        out.dqdg[jp] += QOPI * (psni + pdni);
        if geolin {
            out.dzdn[jo] += QOPI * gsum * (psx1 * x1o + psx2 * x2o + psyy * yyo)
                + QOPI * gdif * (pdx1 * x1o + pdx2 * x2o + pdyy * yyo);
            out.dzdn[jp] += QOPI * gsum * (psx1 * x1p + psx2 * x2p + psyy * yyp)
                + QOPI * gdif * (pdx1 * x1p + pdx2 * x2p + pdyy * yyp);
            for (k, f) in qf.iter().enumerate() {
                out.z_qdof[k] += QOPI * ((psis - psid) * f[jo] + (psis + psid) * f[jp]);
            }
        }
    }

    // Freestream.
    out.psi += qinf * (cosa * yi - sina * xi);
    out.psi_ni += qinf * (cosa * nyi - sina * nxi);
}

/// Options of [`mixed_inverse`].
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MixedInverseOptions {
    /// First node of the target segment (0-based; XFOIL `IQ1` − 1).
    pub first: usize,
    /// Last node of the target segment (0-based; XFOIL `IQ2` − 1).
    pub last: usize,
    /// Curvature (second difference) of the speed kept at the segment ends
    /// (XFOIL `CPXX`; off by default).
    pub curvature_constraints: bool,
    /// Newton iterations allowed (XFOIL `EXEC n`).
    pub max_iterations: usize,
}

/// Result of [`mixed_inverse`].
#[derive(Debug, Clone, PartialEq)]
pub struct MixedInverseResult {
    /// The new paneled airfoil (same nodes outside the target segment).
    pub airfoil: Airfoil,
    /// True if the node displacements and speed changes fell below
    /// XFOIL's thresholds (`5·10⁻⁵` and `5·10⁻⁴`).
    pub converged: bool,
    /// Newton iterations performed.
    pub iterations: usize,
    /// Surface speed of the new airfoil at the design angle, at each node:
    /// on the segment, the target plus the corrections of the free
    /// parameters.
    pub speed: Vec<f64>,
}

/// Mixed inverse design (XFOIL `QDES` `EXEC`): reshapes the target segment
/// of a paneled airfoil so that, at `alpha_deg`, its surface speed is
/// `target` (one value per node; only the segment's values are used,
/// adjusted by the free parameters of XFOIL's formulation), keeping the
/// rest of the airfoil.
///
/// The segment must not contain the stagnation point at `alpha_deg`.
pub fn mixed_inverse(
    paneled: &Airfoil,
    alpha_deg: f64,
    target: &[f64],
    options: &MixedInverseOptions,
) -> Result<MixedInverseResult> {
    let init = Initial::new(paneled, alpha_deg)?;
    let n = init.x.len();
    let (iq1, iq2) = (options.first, options.last);
    if iq1 >= iq2 || iq2 >= n || target.len() != n {
        return Err(InverseError::InvalidSegment {
            first: iq1,
            last: iq2,
            nodes: n,
        });
    }
    if target.iter().any(|v| !v.is_finite()) {
        return Err(InverseError::InvalidOption(
            "the target speed must be finite".into(),
        ));
    }
    // The target segment must not include the stagnation point.
    if (iq1..iq2).any(|i| init.gam[i] >= 0.0 && init.gam[i + 1] < 0.0) {
        return Err(InverseError::StagnationInSegment);
    }

    let Initial {
        mut x,
        mut y,
        xp,
        yp,
        apanel,
        te,
        chord,
        mut gam,
        mut psio,
        alfa,
        ..
    } = init;
    let mut s = scalc(&x, &y);
    let mut te_panel = tecalc(&x, &y, &xp, &yp, chord);

    // DOF shape functions and the specified speed on the segment.
    const BWT: f64 = 0.1;
    let mut qf: [Vec<f64>; 4] = std::array::from_fn(|_| vec![0.0; n]);
    let mut qdof = [0.0_f64; 4];
    for i in iq1..=iq2 {
        let fs = (s[i] - s[iq1]) / (s[iq2] - s[iq1]);
        qf[0][i] = 1.0 - fs;
        qf[1][i] = fs;
        if options.curvature_constraints {
            qf[2][i] = (-5.0 * fs).exp();
            qf[3][i] = (-5.0 * (1.0 - fs)).exp();
        }
        gam[i] = target[i];
    }

    let size = n + 5;
    let mut converged = false;
    let mut iterations = 0;
    let mut sens = Psilin::new(n);
    for _ in 0..options.max_iterations {
        iterations += 1;
        let mut q = vec![vec![0.0; size]; size];
        let mut dq = vec![0.0; size];
        // Adds a Jacobian entry for row i due to the node speed at j: a
        // local unknown outside the segment, the DOFs inside (GAMLIN).
        let gamlin = |q: &mut Vec<Vec<f64>>, qf: &[Vec<f64>; 4], i: usize, j: usize, coef: f64| {
            if j >= iq1 && j <= iq2 {
                for k in 0..4 {
                    q[i][n + 1 + k] += coef * qf[k][j];
                }
            } else {
                q[i][j] += coef;
            }
        };

        let (nx, ny) = ncalc(&x, &y)?;
        let panels = Panels {
            x: &x,
            y: &y,
            s: &s,
            nx: &nx,
            ny: &ny,
            apanel: &apanel,
            te: te_panel,
        };
        for i in 0..n {
            psilin(
                &panels,
                &gam,
                &qf,
                Some(i),
                x[i],
                y[i],
                nx[i],
                ny[i],
                alfa,
                true,
                &mut sens,
            );
            sens.dzdn[i] += sens.psi_ni;
            let row = &mut q[i];
            for (j, v) in row[..n].iter_mut().enumerate() {
                *v += if (iq1..=iq2).contains(&j) {
                    sens.dzdn[j]
                } else {
                    sens.dzdg[j]
                };
            }
            dq[i] = psio - sens.psi;
            row[n] -= 1.0;
            for (v, z) in row[n + 1..].iter_mut().zip(&sens.z_qdof) {
                *v += z;
            }
        }

        // Kutta condition.
        dq[n] = -(gam[0] + gam[n - 1]);
        gamlin(&mut q, &qf, n, 0, 1.0);
        gamlin(&mut q, &qf, n, n - 1, 1.0);

        if te_panel.sharp {
            // Zero internal velocity in the trailing-edge corner, along the
            // bisector just ahead of the trailing edge.
            let ag1 = (-yp[0]).atan2(-xp[0]);
            let ag2 = atanc(yp[n - 1], xp[n - 1], ag1);
            let abis = 0.5 * (ag1 + ag2);
            let (sbis, cbis) = abis.sin_cos();
            let ds1 = (x[0] - x[1]).hypot(y[0] - y[1]);
            let ds2 = (x[n - 1] - x[n - 2]).hypot(y[n - 1] - y[n - 2]);
            let dsmin = ds1.min(ds2);
            let xbis = te[0] - BWT * dsmin * cbis;
            let ybis = te[1] - BWT * dsmin * sbis;
            let mut bis = Psilin::new(n);
            psilin(
                &panels, &gam, &qf, None, xbis, ybis, -sbis, cbis, alfa, false, &mut bis,
            );
            let res = bis.psi_ni;
            q[n - 1].iter_mut().for_each(|v| *v = 0.0);
            for j in 0..n {
                gamlin(&mut q, &qf, n - 1, j, bis.dqdg[j]);
                q[n - 1][j] = bis.dqdg[j];
            }
            q[n - 1][n] = 0.0;
            dq[n - 1] = -res;
        }

        // Pinned segment end points.
        q[n + 1][iq1] = 1.0;
        dq[n + 1] = 0.0;
        q[n + 2][iq2] = 1.0;
        dq[n + 2] = 0.0;

        if iq1 > 0 && options.curvature_constraints {
            let res = gam[iq1 - 1] - 2.0 * gam[iq1] + gam[iq1 + 1]
                - (target[iq1 - 1] - 2.0 * target[iq1] + target[iq1 + 1]);
            gamlin(&mut q, &qf, n + 3, iq1 - 1, 1.0);
            gamlin(&mut q, &qf, n + 3, iq1, -2.0);
            gamlin(&mut q, &qf, n + 3, iq1 + 1, 1.0);
            dq[n + 3] = -res;
        } else {
            q[n + 3][n + 3] = 1.0;
            dq[n + 3] = -qdof[2];
        }
        if iq2 < n - 1 && options.curvature_constraints {
            let res = gam[iq2 - 1] - 2.0 * gam[iq2] + gam[iq2 + 1]
                - (target[iq2 - 1] - 2.0 * target[iq2] + target[iq2 + 1]);
            gamlin(&mut q, &qf, n + 4, iq2 - 1, 1.0);
            gamlin(&mut q, &qf, n + 4, iq2, -2.0);
            gamlin(&mut q, &qf, n + 4, iq2 + 1, 1.0);
            dq[n + 4] = -res;
        } else {
            q[n + 4][n + 4] = 1.0;
            dq[n + 4] = -qdof[3];
        }

        gauss(&mut q, &mut dq);
        if dq.iter().any(|v| !v.is_finite()) {
            return Err(InverseError::Singular);
        }

        let (mut dnmax, mut dgmax) = (0.0_f64, 0.0_f64);
        for i in 0..n {
            if i >= iq1 && i <= iq2 {
                x[i] += nx[i] * dq[i];
                y[i] += ny[i] * dq[i];
                if dq[i].abs() > dnmax.abs() {
                    dnmax = dq[i];
                }
            } else {
                gam[i] += dq[i];
                if dq[i].abs() > dgmax.abs() {
                    dgmax = dq[i];
                }
            }
        }
        psio += dq[n];
        for k in 0..4 {
            qdof[k] += dq[n + 1 + k];
        }
        s = scalc(&x, &y);
        for i in iq1..=iq2 {
            gam[i] = target[i] + (0..4).map(|k| qdof[k] * qf[k][i]).sum::<f64>();
        }
        te_panel = tecalc(&x, &y, &xp, &yp, chord);
        if dnmax.abs() < 5.0e-5 && dgmax.abs() < 5.0e-4 {
            converged = true;
            break;
        }
    }

    let airfoil = Airfoil::new(paneled.name(), x, y)?;
    Ok(MixedInverseResult {
        airfoil,
        converged,
        iterations,
        speed: gam,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use aero_geometry::{NacaOptions, PanelingOptions, naca, panel};

    fn paneled(code: &str) -> Airfoil {
        panel(
            &naca(code, &NacaOptions::default()).unwrap(),
            &PanelingOptions::default(),
        )
        .unwrap()
    }

    #[test]
    fn unchanged_target_keeps_the_airfoil() {
        let a = paneled("2412");
        let t = SpeedTarget::from_airfoil(&a, 3.0).unwrap();
        let options = MixedInverseOptions {
            first: 20,
            last: 60,
            curvature_constraints: false,
            max_iterations: 5,
        };
        let r = mixed_inverse(&a, 3.0, &t.speed, &options).unwrap();
        assert!(r.converged && r.iterations <= 2, "{r:?}");
        let d = a
            .points()
            .zip(r.airfoil.points())
            .map(|(p, q)| (p[0] - q[0]).hypot(p[1] - q[1]))
            .fold(0.0, f64::max);
        assert!(d < 1e-6, "{d}");
    }

    #[test]
    fn segment_reaches_the_target_speed() {
        // The speed at 2 deg prescribed at 4 deg on the upper surface.
        let a = paneled("2412");
        let target = SpeedTarget::from_airfoil(&a, 2.0).unwrap();
        let options = MixedInverseOptions {
            first: 20,
            last: 60,
            curvature_constraints: false,
            max_iterations: 10,
        };
        let r = mixed_inverse(&a, 4.0, &target.speed, &options).unwrap();
        assert!(r.converged, "{r:?}");
        // On the segment, the speed is the target plus XFOIL's linear
        // correction (the speeds at 2 and 4 deg differ at the ends).
        let s = &target.s;
        let fs = |i: usize| (s[i] - s[20]) / (s[60] - s[20]);
        let d = |i: usize| r.speed[i] - target.speed[i];
        let worst = (20..=60)
            .map(|i| (d(i) - (d(20) * (1.0 - fs(i)) + d(60) * fs(i))).abs())
            .fold(0.0, f64::max);
        assert!(worst < 1e-12, "{worst}");
        assert!(d(60).abs() > 0.05);
        // The new airfoil analyzed at 4 deg has that speed.
        let check = SpeedTarget::from_airfoil(&r.airfoil, 4.0).unwrap();
        let worst = (0..a.len())
            .map(|i| (check.speed[i] - r.speed[i]).abs())
            .fold(0.0, f64::max);
        assert!(worst < 1e-3, "{worst}");
        // Nodes outside the segment did not move.
        for i in (0..20).chain(61..a.len()) {
            assert_eq!(a.x()[i], r.airfoil.x()[i]);
            assert_eq!(a.y()[i], r.airfoil.y()[i]);
        }
    }

    #[test]
    fn invalid_segments() {
        let a = paneled("2412");
        let t = SpeedTarget::from_airfoil(&a, 3.0).unwrap();
        let mut o = MixedInverseOptions {
            first: 60,
            last: 20,
            curvature_constraints: false,
            max_iterations: 5,
        };
        assert!(mixed_inverse(&a, 3.0, &t.speed, &o).is_err());
        // Around the leading edge, the stagnation point is in the segment.
        o.first = 70;
        o.last = 90;
        assert!(matches!(
            mixed_inverse(&a, 3.0, &t.speed, &o),
            Err(InverseError::StagnationInSegment)
        ));
    }

    #[test]
    fn symmetric_target() {
        let a = paneled("0012");
        let mut t = SpeedTarget::from_airfoil(&a, 3.0).unwrap();
        t.symmetrize();
        let n = t.speed.len();
        for i in 0..n {
            assert_eq!(t.speed[i], -t.speed[n - 1 - i]);
            assert!((t.s[i] + t.s[n - 1 - i] - 1.0).abs() < 1e-15);
        }
        // At zero lift the symmetric airfoil is its own symmetric target.
        let zero = SpeedTarget::from_airfoil(&a, 0.0).unwrap();
        let mut sym = zero.clone();
        sym.symmetrize();
        let d = zero
            .speed
            .iter()
            .zip(&sym.speed)
            .map(|(p, q)| (p - q).abs())
            .fold(0.0, f64::max);
        assert!(d < 1e-6, "{d}");
    }

    #[test]
    fn smoothing_keeps_the_ends() {
        let a = paneled("2412");
        let mut t = SpeedTarget::from_airfoil(&a, 3.0).unwrap();
        let before = t.speed.clone();
        t.speed[40] += 0.05; // a bump
        t.smooth(20, 60, true).unwrap();
        assert_eq!(t.speed[20], before[20]);
        assert_eq!(t.speed[60], before[60]);
        assert!((t.speed[40] - before[40]).abs() < 0.05);
        assert!(t.smooth(60, 20, true).is_err());
    }
}
