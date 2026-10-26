//! Wake geometry and source influence matrix (XFOIL `xpanel.f`: `XYWAKE`,
//! `PSWLIN`, `QWCALC`, `QDCALC`; `xutils.f`: `SETEXP`).
//!
//! XFOIL stores the wake nodes after the airfoil nodes in the panel arrays
//! (`X(N+1..N+NW)`); [`Panels`] does the same, with 0-based indices: node
//! `i < n` is on the airfoil, node `n + iw` is wake node `iw`.

// Index loops mirror XFOIL's DO loops.
#![allow(clippy::needless_range_loop)]

use std::f64::consts::PI;

use faer::Mat;
use faer::linalg::solvers::Solve;

use crate::inviscid::{FieldPoint, Influence, InviscidSolver, PanelGeometry, psilin, te_bisector};

const QOPI: f64 = 0.25 / PI;

/// Maximum number of wake nodes (XFOIL `IWX = IQX/8 + 2`).
const IWX: usize = 601 / 8 + 2;

/// Airfoil and wake nodes (XFOIL `X`, `Y`, `S`, `NX`, `NY`, `APANEL` over
/// `1..N+NW`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Panels {
    /// Number of airfoil nodes.
    pub n: usize,
    /// Number of wake nodes.
    pub nw: usize,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub s: Vec<f64>,
    pub nx: Vec<f64>,
    pub ny: Vec<f64>,
    pub apanel: Vec<f64>,
}

/// XFOIL `SETEXP`: geometrically stretched array from 0 to `smax` with
/// first increment `ds1` and `nn` points.
pub(crate) fn setexp(ds1: f64, smax: f64, nn: usize) -> Vec<f64> {
    let sigma = smax / ds1;
    let nex = nn.saturating_sub(1);
    let rnex = nex as f64;
    let rni = 1.0 / rnex;

    // Quadratic for the initial guess of the geometric ratio.
    let aaa = rnex * (rnex - 1.0) * (rnex - 2.0) / 6.0;
    let bbb = rnex * (rnex - 1.0) / 2.0;
    let ccc = rnex - sigma;
    let disc = (bbb * bbb - 4.0 * aaa * ccc).max(0.0);

    let mut ratio = if nex == 2 {
        -ccc / bbb + 1.0
    } else {
        (-bbb + disc.sqrt()) / (2.0 * aaa) + 1.0
    };

    // Newton iteration for the actual geometric ratio (XFOIL continues
    // with the last iterate if it does not converge).
    if ratio != 1.0 {
        let n = nex as i32;
        for _ in 0..100 {
            let sigman = (ratio.powi(n) - 1.0) / (ratio - 1.0);
            let res = sigman.powf(rni) - sigma.powf(rni);
            let dresdr = rni * sigman.powf(rni) * (rnex * ratio.powi(n - 1) - sigman)
                / (ratio.powi(n) - 1.0);
            let dratio = -res / dresdr;
            ratio += dratio;
            if dratio.abs() < 1.0e-5 {
                break;
            }
        }
    }

    let mut s = vec![0.0; nn];
    let mut ds = ds1;
    for i in 1..nn {
        s[i] = s[i - 1] + ds;
        ds *= ratio;
    }
    s
}

#[allow(clippy::too_many_arguments)]
/// `∂ψ/∂n` (with freestream) of the vortex sheet `gam` at a point off the
/// airfoil, as returned by XFOIL's `PSILIN` in `PSI_NI`.
fn dpsi_dn(
    geo: &PanelGeometry,
    gam: &[f64],
    alfa: f64,
    qinf: f64,
    x: f64,
    y: f64,
    nx: f64,
    ny: f64,
    inf: &mut Influence,
) -> f64 {
    let point = FieldPoint {
        node: None,
        x,
        y,
        nx,
        ny,
    };
    psilin(geo, &point, false, inf);
    let (sina, cosa) = alfa.sin_cos();
    let induced: f64 = inf.dq_dgamma.iter().zip(gam).map(|(a, g)| a * g).sum();
    induced + qinf * (cosa * ny - sina * nx)
}

impl Panels {
    /// XFOIL `XYWAKE`: airfoil nodes followed by wake nodes placed along
    /// the streamline leaving the trailing edge, for the vortex
    /// distribution `gam` at angle of attack `alfa` (radians). The wake is
    /// `waklen` chords long.
    pub fn with_wake(geo: &PanelGeometry, gam: &[f64], alfa: f64, qinf: f64, waklen: f64) -> Self {
        let n = geo.len();
        let nw = (n / 12 + 10 * (waklen as usize)).min(IWX);
        let nt = n + nw;

        let mut x = geo.x.clone();
        let mut y = geo.y.clone();
        let mut s = geo.s.clone();
        let mut nx = geo.nx.clone();
        let mut ny = geo.ny.clone();
        let mut apanel = geo.apanel.clone();
        for v in [&mut x, &mut y, &mut s, &mut nx, &mut ny, &mut apanel] {
            v.resize(nt + 1, 0.0); // one spare slot for NX(N+NW+1)
        }

        let ds1 = 0.5 * (geo.s[1] - geo.s[0] + geo.s[n - 1] - geo.s[n - 2]);
        let snew = setexp(ds1, waklen * geo.chord, nw);

        let xte = 0.5 * (geo.x[0] + geo.x[n - 1]);
        let yte = 0.5 * (geo.y[0] + geo.y[n - 1]);

        // First wake point a tiny distance behind the trailing edge.
        let i = n;
        let sx = 0.5 * (geo.yp[n - 1] - geo.yp[0]);
        let sy = 0.5 * (geo.xp[0] - geo.xp[n - 1]);
        let smod = (sx * sx + sy * sy).sqrt();
        nx[i] = sx / smod;
        ny[i] = sy / smod;
        x[i] = xte - 0.0001 * ny[i];
        y[i] = yte + 0.0001 * nx[i];
        s[i] = geo.s[n - 1];

        let mut inf = Influence::new(n);
        let mut normal_after =
            |i: usize, x: &[f64], y: &[f64], nx: &mut [f64], ny: &mut [f64], ap: &mut [f64]| {
                // Streamfunction gradient components at node i.
                let psi_x = dpsi_dn(geo, gam, alfa, qinf, x[i], y[i], 1.0, 0.0, &mut inf);
                let psi_y = dpsi_dn(geo, gam, alfa, qinf, x[i], y[i], 0.0, 1.0, &mut inf);
                let norm = (psi_x * psi_x + psi_y * psi_y).sqrt();
                nx[i + 1] = -psi_x / norm;
                ny[i + 1] = -psi_y / norm;
                ap[i] = psi_y.atan2(psi_x);
            };
        normal_after(i, &x, &y, &mut nx, &mut ny, &mut apanel);

        for i in n + 1..nt {
            let ds = snew[i - n] - snew[i - n - 1];
            x[i] = x[i - 1] - ds * ny[i];
            y[i] = y[i - 1] + ds * nx[i];
            s[i] = s[i - 1] + ds;
            if i + 1 < nt {
                normal_after(i, &x, &y, &mut nx, &mut ny, &mut apanel);
            }
        }
        for v in [&mut x, &mut y, &mut s, &mut nx, &mut ny, &mut apanel] {
            v.truncate(nt);
        }

        Self {
            n,
            nw,
            x,
            y,
            s,
            nx,
            ny,
            apanel,
        }
    }

    /// Total number of nodes.
    pub fn len(&self) -> usize {
        self.n + self.nw
    }
}

/// XFOIL `PSWLIN`: sensitivities of the streamfunction (`dzdm`) and of its
/// normal derivative (`dqdm`) at a field point to the wake mass defects
/// (index `iw` for wake node `n + iw`). `io` is the global node index of the
/// field point, if any.
#[allow(clippy::too_many_arguments)]
pub(crate) fn pswlin(
    pan: &Panels,
    io: Option<usize>,
    xi: f64,
    yi: f64,
    nxi: f64,
    nyi: f64,
    dzdm: &mut [f64],
    dqdm: &mut [f64],
) {
    let (n, nw) = (pan.n, pan.nw);
    let (x, y) = (&pan.x, &pan.y);
    dzdm.iter_mut().for_each(|v| *v = 0.0);
    dqdm.iter_mut().for_each(|v| *v = 0.0);
    let in_wake = io.is_some_and(|i| i >= n && i < n + nw);
    let dist = |a: usize, b: usize| ((x[a] - x[b]).powi(2) + (y[a] - y[b]).powi(2)).sqrt();

    for jo in n..n + nw - 1 {
        let jp = jo + 1;
        let mut jm = jo.wrapping_sub(1);
        let mut jq = jp + 1;
        if jo == n {
            jm = jo;
        } else if jo == n + nw - 2 {
            jq = jp;
        }
        // Local indices into dzdm/dqdm.
        let (lm, lo, lp, lq) = (jm - n, jo - n, jp - n, jq - n);

        let dso = dist(jo, jp);
        let dsio = 1.0 / dso;
        let apan = pan.apanel[jo];

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

        let sgn = if in_wake { 1.0 } else { 1.0_f64.copysign(yy) };
        let branch = (0.5 - 0.5 * sgn) * PI;

        let (g1, t1) = if io != Some(jo) && rs1 > 0.0 {
            (rs1.ln(), (sgn * x1).atan2(sgn * yy) - branch)
        } else {
            (0.0, 0.0)
        };
        let (g2, t2) = if io != Some(jp) && rs2 > 0.0 {
            (rs2.ln(), (sgn * x2).atan2(sgn * yy) - branch)
        } else {
            (0.0, 0.0)
        };

        let x1i = sx * nxi + sy * nyi;
        let x2i = sx * nxi + sy * nyi;
        let yyi = sx * nyi - sy * nxi;

        // Midpoint quantities.
        let x0 = 0.5 * (x1 + x2);
        let rs0 = x0 * x0 + yy * yy;
        let g0 = rs0.ln();
        let t0 = (sgn * x0).atan2(sgn * yy) - branch;

        // Source contribution of the 1-0 half panel.
        let dxinv = 1.0 / (x1 - x0);
        let psum = x0 * (t0 - apan) - x1 * (t1 - apan) + 0.5 * yy * (g1 - g0);
        let pdif =
            ((x1 + x0) * psum + rs1 * (t1 - apan) - rs0 * (t0 - apan) + (x0 - x1) * yy) * dxinv;

        let psx1 = -(t1 - apan);
        let psx0 = t0 - apan;
        let psyy = 0.5 * (g1 - g0);

        let pdx1 = ((x1 + x0) * psx1 + psum + 2.0 * x1 * (t1 - apan) - pdif) * dxinv;
        let pdx0 = ((x1 + x0) * psx0 + psum - 2.0 * x0 * (t0 - apan) + pdif) * dxinv;
        let pdyy = ((x1 + x0) * psyy + 2.0 * (x0 - x1 + yy * (t1 - t0))) * dxinv;

        let dsim = 1.0 / dist(jp, jm);

        dzdm[lm] += QOPI * (-psum * dsim + pdif * dsim);
        dzdm[lo] += QOPI * (-psum * dsio - pdif * dsio);
        dzdm[lp] += QOPI * (psum * (dsio + dsim) + pdif * (dsio - dsim));

        let psni = psx1 * x1i + psx0 * (x1i + x2i) * 0.5 + psyy * yyi;
        let pdni = pdx1 * x1i + pdx0 * (x1i + x2i) * 0.5 + pdyy * yyi;

        dqdm[lm] += QOPI * (-psni * dsim + pdni * dsim);
        dqdm[lo] += QOPI * (-psni * dsio - pdni * dsio);
        dqdm[lp] += QOPI * (psni * (dsio + dsim) + pdni * (dsio - dsim));

        // Source contribution of the 0-2 half panel.
        let dxinv = 1.0 / (x0 - x2);
        let psum = x2 * (t2 - apan) - x0 * (t0 - apan) + 0.5 * yy * (g0 - g2);
        let pdif =
            ((x0 + x2) * psum + rs0 * (t0 - apan) - rs2 * (t2 - apan) + (x2 - x0) * yy) * dxinv;

        let psx0 = -(t0 - apan);
        let psx2 = t2 - apan;
        let psyy = 0.5 * (g0 - g2);

        let pdx0 = ((x0 + x2) * psx0 + psum + 2.0 * x0 * (t0 - apan) - pdif) * dxinv;
        let pdx2 = ((x0 + x2) * psx2 + psum - 2.0 * x2 * (t2 - apan) + pdif) * dxinv;
        let pdyy = ((x0 + x2) * psyy + 2.0 * (x2 - x0 + yy * (t0 - t2))) * dxinv;

        let dsip = 1.0 / dist(jq, jo);

        dzdm[lo] += QOPI * (-psum * (dsip + dsio) - pdif * (dsip - dsio));
        dzdm[lp] += QOPI * (psum * dsio - pdif * dsio);
        dzdm[lq] += QOPI * (psum * dsip + pdif * dsip);

        let psni = psx0 * (x1i + x2i) * 0.5 + psx2 * x2i + psyy * yyi;
        let pdni = pdx0 * (x1i + x2i) * 0.5 + pdx2 * x2i + pdyy * yyi;

        dqdm[lo] += QOPI * (-psni * (dsip + dsio) - pdni * (dsip - dsio));
        dqdm[lp] += QOPI * (psni * dsio - pdni * dsio);
        dqdm[lq] += QOPI * (psni * dsip + pdni * dsip);
    }
}

/// XFOIL `QWCALC` (with the airfoil part of `GGCALC`): inviscid tangential
/// velocities at all nodes for `α = 0°` and `α = 90°` (`QINVU`).
pub(crate) fn base_speeds(inv: &InviscidSolver, pan: &Panels) -> [Vec<f64>; 2] {
    let geo = inv.geometry();
    let gamu = inv.base_solutions();
    let (n, nt) = (pan.n, pan.len());
    let mut q = [vec![0.0; nt], vec![0.0; nt]];
    for i in 0..n {
        q[0][i] = gamu[(i, 0)];
        q[1][i] = gamu[(i, 1)];
    }
    // First wake point: same as the trailing edge.
    q[0][n] = q[0][n - 1];
    q[1][n] = q[1][n - 1];

    let mut inf = Influence::new(n);
    for i in n + 1..nt {
        let point = FieldPoint {
            node: None,
            x: pan.x[i],
            y: pan.y[i],
            nx: pan.nx[i],
            ny: pan.ny[i],
        };
        psilin(geo, &point, false, &mut inf);
        let (mut q1, mut q2) = (0.0, 0.0);
        for j in 0..n {
            q1 += inf.dq_dgamma[j] * gamu[(j, 0)];
            q2 += inf.dq_dgamma[j] * gamu[(j, 1)];
        }
        q[0][i] = q1 + pan.ny[i];
        q[1][i] = q2 - pan.nx[i];
    }
    q
}

/// XFOIL `QDCALC`: the `(N+NW)×(N+NW)` matrix `DIJ` of the tangential
/// velocities at all nodes with respect to all mass defects.
pub(crate) fn source_matrix(inv: &InviscidSolver, pan: &Panels) -> Mat<f64> {
    let geo = inv.geometry();
    let (n, nw, nt) = (pan.n, pan.nw, pan.len());
    let mut dij = Mat::<f64>::zeros(nt, nt);

    // Airfoil sources on the airfoil (geometry only, cached).
    let da = inv.source_velocity_matrix();
    for j in 0..n {
        for i in 0..n {
            dij[(i, j)] = da[(i, j)];
        }
    }

    // dPsi/dm of the wake sources on the airfoil nodes; the Kutta row has no
    // direct source influence.
    let mut bij = Mat::<f64>::zeros(n + 1, nw);
    let mut dzdm = vec![0.0; nw];
    let mut dqdm = vec![0.0; nw];
    for i in 0..n {
        pswlin(
            pan,
            Some(i),
            pan.x[i],
            pan.y[i],
            pan.nx[i],
            pan.ny[i],
            &mut dzdm,
            &mut dqdm,
        );
        for jw in 0..nw {
            bij[(i, jw)] = -dzdm[jw];
        }
    }
    if let Some((point, _, _)) = te_bisector(geo) {
        // Sharp trailing edge: zero velocity along the bisector (the "new"
        // treatment of XFOIL 6.996).
        pswlin(
            pan, None, point.x, point.y, point.nx, point.ny, &mut dzdm, &mut dqdm,
        );
        for jw in 0..nw {
            bij[(n - 1, jw)] = -dqdm[jw];
        }
    }
    inv.lu().solve_in_place(bij.as_mut());
    for jw in 0..nw {
        for i in 0..n {
            dij[(i, n + jw)] = bij[(i, jw)];
        }
    }

    // Velocities at the wake points: direct source influence, plus the
    // effect of all sources on the airfoil vorticity.
    let mut cij = Mat::<f64>::zeros(nw, n);
    let mut inf = Influence::new(n);
    for iw in 0..nw {
        let i = n + iw;
        let point = FieldPoint {
            node: None,
            x: pan.x[i],
            y: pan.y[i],
            nx: pan.nx[i],
            ny: pan.ny[i],
        };
        psilin(geo, &point, true, &mut inf);
        for j in 0..n {
            cij[(iw, j)] = inf.dq_dgamma[j];
            dij[(i, j)] = inf.dq_dsigma[j];
        }
        pswlin(
            pan,
            Some(i),
            pan.x[i],
            pan.y[i],
            pan.nx[i],
            pan.ny[i],
            &mut dzdm,
            &mut dqdm,
        );
        for jw in 0..nw {
            dij[(i, n + jw)] = dqdm[jw];
        }
    }
    for iw in 0..nw {
        let i = n + iw;
        for j in 0..nt {
            let mut sum = 0.0;
            for k in 0..n {
                sum += cij[(iw, k)] * dij[(k, j)];
            }
            dij[(i, j)] += sum;
        }
    }

    // The first wake point has the trailing-edge velocity.
    for j in 0..nt {
        dij[(n, j)] = dij[(n - 1, j)];
    }
    dij
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inviscid::InviscidOptions;
    use aero_geometry::{NacaOptions, PanelingOptions, naca};

    fn solver() -> InviscidSolver {
        let af = naca("2412", &NacaOptions::default())
            .unwrap()
            .paneled(&PanelingOptions::default())
            .unwrap();
        InviscidSolver::new(&af, &InviscidOptions::default()).unwrap()
    }

    fn gam(inv: &InviscidSolver, alfa: f64) -> Vec<f64> {
        let g = inv.base_solutions();
        (0..inv.len())
            .map(|i| alfa.cos() * g[(i, 0)] + alfa.sin() * g[(i, 1)])
            .collect()
    }

    #[test]
    fn setexp_stretches_geometrically() {
        let s = setexp(0.01, 1.0, 23);
        assert_eq!(s.len(), 23);
        assert_eq!(s[0], 0.0);
        assert!((s[1] - 0.01).abs() < 1e-15);
        assert!((s[22] - 1.0).abs() < 1e-6, "{}", s[22]);
        let r = (s[2] - s[1]) / s[1];
        for i in 2..22 {
            let ri = (s[i + 1] - s[i]) / (s[i] - s[i - 1]);
            assert!((ri - r).abs() < 1e-12);
        }
    }

    #[test]
    fn wake_follows_the_streamline() {
        let inv = solver();
        let alfa = 4.0_f64.to_radians();
        let g = gam(&inv, alfa);
        let pan = Panels::with_wake(inv.geometry(), &g, alfa, 1.0, 1.0);
        assert_eq!(pan.nw, 160 / 12 + 10);
        assert_eq!(pan.len(), 160 + 23);
        let (n, nt) = (pan.n, pan.len());
        // Starts just behind the trailing edge, ends one chord downstream.
        assert!((pan.x[n] - 1.0).abs() < 2e-4);
        assert!((pan.s[nt - 1] - pan.s[n] - 1.0).abs() < 1e-5);
        // At positive alpha the wake turns from the trailing-edge direction
        // towards the freestream direction (upwards).
        let slope = |i: usize| (pan.y[i + 1] - pan.y[i]) / (pan.x[i + 1] - pan.x[i]);
        assert!(slope(nt - 2) > slope(n));
        assert!(slope(nt - 2) > 0.0 && slope(nt - 2) < alfa.tan());
        // (XYWAKE traces the streamline with explicit steps; its accuracy is
        // XFOIL's, checked in `wake_matches_xfoil`.)
    }

    /// Wake nodes (s, x, y) of an XFOIL `DUMP` file (rows with 8 columns).
    fn xfoil_wake(file: &str) -> Vec<[f64; 3]> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/viscous")
            .join(file);
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .filter(|l| !l.starts_with('#'))
            .map(|l| {
                l.split_whitespace()
                    .map(|v| v.parse::<f64>().unwrap())
                    .collect::<Vec<_>>()
            })
            .filter(|v| v.len() == 8)
            .map(|v| [v[0], v[1], v[2]])
            .collect()
    }

    #[test]
    fn wake_matches_xfoil() {
        // XFOIL places the wake of a cold-started point with the inviscid
        // vorticity of that angle of attack.
        let af = naca("0012", &NacaOptions::default())
            .unwrap()
            .paneled(&PanelingOptions::default())
            .unwrap();
        let inv = InviscidSolver::new(&af, &InviscidOptions::default()).unwrap();
        for (alpha, file) in [
            (4.0_f64, "naca0012/vbl_re1e6_a+04.0.txt"),
            (12.0, "naca0012/vbl_re1e6_a+12.0.txt"),
        ] {
            let alfa = alpha.to_radians();
            let pan = Panels::with_wake(inv.geometry(), &gam(&inv, alfa), alfa, 1.0, 1.0);
            let reference = xfoil_wake(file);
            assert_eq!(reference.len(), pan.nw);
            for (iw, r) in reference.iter().enumerate() {
                let i = pan.n + iw;
                // DUMP prints 5 decimals.
                for (ours, theirs) in [pan.s[i], pan.x[i], pan.y[i]].iter().zip(r) {
                    assert!(
                        (ours - theirs).abs() < 6e-6,
                        "alpha {alpha} node {iw}: {ours} {theirs}"
                    );
                }
            }
        }
    }

    #[test]
    fn wake_source_derivative_matches_finite_difference() {
        let inv = solver();
        let alfa = 2.0_f64.to_radians();
        let pan = Panels::with_wake(inv.geometry(), &gam(&inv, alfa), alfa, 1.0, 1.0);
        let nw = pan.nw;
        let sigma: Vec<f64> = (0..nw).map(|i| 0.01 * (0.4 * i as f64).sin()).collect();
        let psi = |x: f64, y: f64| {
            let mut dz = vec![0.0; nw];
            let mut dq = vec![0.0; nw];
            pswlin(&pan, None, x, y, 0.0, 1.0, &mut dz, &mut dq);
            dz.iter().zip(&sigma).map(|(a, b)| a * b).sum::<f64>()
        };
        let (px, py, nx, ny) = (1.3, 0.05, 0.6, 0.8);
        let mut dz = vec![0.0; nw];
        let mut dq = vec![0.0; nw];
        pswlin(&pan, None, px, py, nx, ny, &mut dz, &mut dq);
        let q: f64 = dq.iter().zip(&sigma).map(|(a, b)| a * b).sum();
        let h = 1e-6;
        let fd = (psi(px + h * nx, py + h * ny) - psi(px - h * nx, py - h * ny)) / (2.0 * h);
        assert!((q - fd).abs() < 1e-7, "{q} {fd}");
    }

    #[test]
    fn source_matrix_structure() {
        let inv = solver();
        let alfa = 3.0_f64.to_radians();
        let pan = Panels::with_wake(inv.geometry(), &gam(&inv, alfa), alfa, 1.0, 1.0);
        let dij = source_matrix(&inv, &pan);
        let (n, nt) = (pan.n, pan.len());
        assert_eq!((dij.nrows(), dij.ncols()), (nt, nt));
        for j in 0..nt {
            assert_eq!(dij[(n, j)], dij[(n - 1, j)]);
            for i in 0..nt {
                assert!(dij[(i, j)].is_finite());
            }
        }
        // A uniform mass defect on the airfoil only changes the speeds
        // through its two ends... check instead the base speeds: on the
        // airfoil they are the base solutions.
        let q = base_speeds(&inv, &pan);
        assert_eq!(q[0][5], inv.base_solutions()[(5, 0)]);
        assert_eq!(q[1][n], q[1][n - 1]);
    }
}
