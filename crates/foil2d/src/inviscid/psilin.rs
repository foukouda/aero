//! Streamfunction influence coefficients, a port of XFOIL's `PSILIN`.
//!
//! The airfoil carries a vortex sheet whose strength `γ` varies linearly on
//! each panel, an optional source sheet used by the viscous coupling, and a
//! trailing-edge panel with a uniform source and vortex whose strengths are
//! tied to `γ` at the two trailing-edge nodes:
//!
//! ```text
//! σ_TE = ½·SCS·(γ₁ − γ_N),   γ_TE = −½·SDS·(γ₁ − γ_N)
//! ```
//!
//! The source sheet is described, as in XFOIL, by nodal values `σ` of the
//! viscous mass defect `m = Ue·δ*`: the source strength is `dσ/ds`, linear on
//! each half panel.
//!
//! [`psilin`] returns the sensitivities of the streamfunction `ψ` and of its
//! derivative `q = ∂ψ/∂n` along a unit vector `n = (nx, ny)` at a field
//! point, with respect to the nodal `γ` and `σ`. `q` is the velocity
//! component along `(ny, −nx)`. The freestream contribution is not included.
//! Ground-effect images (`LIMAGE`) and the geometric sensitivities of the
//! inverse design modes (`GEOLIN`) are not ported.

use std::f64::consts::PI;

use super::panel::PanelGeometry;

const QOPI: f64 = 0.25 / PI;
const HOPI: f64 = 0.5 / PI;

/// Sensitivities of `ψ` and of `q = ∂ψ/∂n` at a field point with respect to
/// the nodal vortex strengths `γ` and mass defects `σ`.
#[derive(Debug, Clone, PartialEq)]
pub struct Influence {
    /// `∂ψ/∂γ_j` (XFOIL `DZDG`).
    pub dpsi_dgamma: Vec<f64>,
    /// `∂ψ/∂σ_j` (XFOIL `DZDM`); zero unless sources are requested.
    pub dpsi_dsigma: Vec<f64>,
    /// `∂q/∂γ_j` (XFOIL `DQDG`).
    pub dq_dgamma: Vec<f64>,
    /// `∂q/∂σ_j` (XFOIL `DQDM`); zero unless sources are requested.
    pub dq_dsigma: Vec<f64>,
}

impl Influence {
    /// Zeroed coefficients for `n` nodes.
    pub fn new(n: usize) -> Self {
        Self {
            dpsi_dgamma: vec![0.0; n],
            dpsi_dsigma: vec![0.0; n],
            dq_dgamma: vec![0.0; n],
            dq_dsigma: vec![0.0; n],
        }
    }

    fn clear(&mut self) {
        self.dpsi_dgamma.iter_mut().for_each(|v| *v = 0.0);
        self.dpsi_dsigma.iter_mut().for_each(|v| *v = 0.0);
        self.dq_dgamma.iter_mut().for_each(|v| *v = 0.0);
        self.dq_dsigma.iter_mut().for_each(|v| *v = 0.0);
    }
}

/// A field point where influence coefficients are evaluated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldPoint {
    /// Index of the point when it is an airfoil node (this removes the
    /// singular self-influence terms), `None` otherwise.
    pub node: Option<usize>,
    /// x coordinate.
    pub x: f64,
    /// y coordinate.
    pub y: f64,
    /// x component of the unit vector `n` of `q = ∂ψ/∂n`.
    pub nx: f64,
    /// y component of the unit vector `n`.
    pub ny: f64,
}

/// Computes the influence coefficients at a field point (XFOIL `PSILIN`).
/// `sources` enables the source-sheet terms (XFOIL `SIGLIN`).
pub fn psilin(geo: &PanelGeometry, point: &FieldPoint, sources: bool, out: &mut Influence) {
    out.clear();
    let FieldPoint {
        node: io,
        x: xi,
        y: yi,
        nx: nxi,
        ny: nyi,
    } = *point;
    let x = &geo.x;
    let y = &geo.y;
    let n = x.len();
    let seps = (geo.s[n - 1] - geo.s[0]) * 1.0e-5;
    let te = &geo.te_panel;
    let (scs, sds) = (te.scs, te.sds);
    let on_airfoil = io.is_some();

    let dist = |a: usize, b: usize| (x[a] - x[b]).hypot(y[a] - y[b]);

    for jo in 0..n {
        let mut jp = jo + 1;
        let mut jm = jo.wrapping_sub(1);
        let mut jq = jp + 1;
        if jo == 0 {
            jm = jo;
        } else if jo == n - 2 {
            jq = jp;
        } else if jo == n - 1 {
            jp = 0;
            if (x[jo] - x[jp]).powi(2) + (y[jo] - y[jp]).powi(2) < seps * seps {
                // No trailing-edge panel for a closed trailing edge.
                return;
            }
        }

        let dso = dist(jo, jp);
        if dso == 0.0 {
            continue; // null panel
        }
        let dsio = 1.0 / dso;
        let apan = geo.apanel[jo];

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

        // Reflection flag avoiding branch problems with arctan off the body.
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

            out.dpsi_dgamma[jo] -= HOPI * psig * scs * 0.5;
            out.dpsi_dgamma[jp] += HOPI * psig * scs * 0.5;
            out.dpsi_dgamma[jo] += HOPI * pgam * sds * 0.5;
            out.dpsi_dgamma[jp] -= HOPI * pgam * sds * 0.5;

            out.dq_dgamma[jo] -= HOPI * (psigni * 0.5 * scs - pgamni * 0.5 * sds);
            out.dq_dgamma[jp] += HOPI * (psigni * 0.5 * scs - pgamni * 0.5 * sds);
            return;
        }

        if sources {
            // Midpoint quantities.
            let x0 = 0.5 * (x1 + x2);
            let rs0 = x0 * x0 + yy * yy;
            let g0 = rs0.ln();
            let t0 = (sgn * x0).atan2(sgn * yy) + branch;

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

            out.dpsi_dsigma[jm] += QOPI * (-psum * dsim + pdif * dsim);
            out.dpsi_dsigma[jo] += QOPI * (-psum * dsio - pdif * dsio);
            out.dpsi_dsigma[jp] += QOPI * (psum * (dsio + dsim) + pdif * (dsio - dsim));

            let psni = psx1 * x1i + psx0 * (x1i + x2i) * 0.5 + psyy * yyi;
            let pdni = pdx1 * x1i + pdx0 * (x1i + x2i) * 0.5 + pdyy * yyi;

            out.dq_dsigma[jm] += QOPI * (-psni * dsim + pdni * dsim);
            out.dq_dsigma[jo] += QOPI * (-psni * dsio - pdni * dsio);
            out.dq_dsigma[jp] += QOPI * (psni * (dsio + dsim) + pdni * (dsio - dsim));

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

            out.dpsi_dsigma[jo] += QOPI * (-psum * (dsip + dsio) - pdif * (dsip - dsio));
            out.dpsi_dsigma[jp] += QOPI * (psum * dsio - pdif * dsio);
            out.dpsi_dsigma[jq] += QOPI * (psum * dsip + pdif * dsip);

            let psni = psx0 * (x1i + x2i) * 0.5 + psx2 * x2i + psyy * yyi;
            let pdni = pdx0 * (x1i + x2i) * 0.5 + pdx2 * x2i + pdyy * yyi;

            out.dq_dsigma[jo] += QOPI * (-psni * (dsip + dsio) - pdni * (dsip - dsio));
            out.dq_dsigma[jp] += QOPI * (psni * dsio - pdni * dsio);
            out.dq_dsigma[jq] += QOPI * (psni * dsip + pdni * dsip);
        }

        // Vortex panel contribution.
        let dxinv = 1.0 / (x1 - x2);
        let psis = 0.5 * x1 * g1 - 0.5 * x2 * g2 + x2 - x1 + yy * (t1 - t2);
        let psid = ((x1 + x2) * psis + 0.5 * (rs2 * g2 - rs1 * g1 + x1 * x1 - x2 * x2)) * dxinv;

        let psx1 = 0.5 * g1;
        let psx2 = -0.5 * g2;
        let psyy = t1 - t2;

        let pdx1 = ((x1 + x2) * psx1 + psis - x1 * g1 - psid) * dxinv;
        let pdx2 = ((x1 + x2) * psx2 + psis + x2 * g2 + psid) * dxinv;
        let pdyy = ((x1 + x2) * psyy - yy * (g1 - g2)) * dxinv;

        out.dpsi_dgamma[jo] += QOPI * (psis - psid);
        out.dpsi_dgamma[jp] += QOPI * (psis + psid);

        let psni = psx1 * x1i + psx2 * x2i + psyy * yyi;
        let pdni = pdx1 * x1i + pdx2 * x2i + pdyy * yyi;

        out.dq_dgamma[jo] += QOPI * (psni - pdni);
        out.dq_dgamma[jp] += QOPI * (psni + pdni);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aero_geometry::{NacaOptions, PanelingOptions, naca};

    fn geometry() -> PanelGeometry {
        let af = naca("2412", &NacaOptions::default())
            .unwrap()
            .paneled(&PanelingOptions {
                n_nodes: 60,
                ..PanelingOptions::default()
            })
            .unwrap();
        PanelGeometry::new(&af).unwrap()
    }

    /// Streamfunction of the sheets for given nodal strengths.
    fn psi(geo: &PanelGeometry, gamma: &[f64], sigma: &[f64], x: f64, y: f64) -> f64 {
        let mut inf = Influence::new(geo.len());
        let p = FieldPoint {
            node: None,
            x,
            y,
            nx: 0.0,
            ny: 1.0,
        };
        psilin(geo, &p, true, &mut inf);
        inf.dpsi_dgamma
            .iter()
            .zip(gamma)
            .map(|(a, b)| a * b)
            .sum::<f64>()
            + inf
                .dpsi_dsigma
                .iter()
                .zip(sigma)
                .map(|(a, b)| a * b)
                .sum::<f64>()
    }

    #[test]
    fn velocity_derivative_matches_finite_difference() {
        let geo = geometry();
        let n = geo.len();
        let gamma: Vec<f64> = (0..n).map(|i| (0.1 * i as f64).sin()).collect();
        let sigma: Vec<f64> = (0..n).map(|i| 0.01 * (0.3 * i as f64).cos()).collect();
        // Field point off the body; direction normal (nx, ny).
        let (px, py) = (0.4, 0.3);
        let (nx, ny) = (0.6_f64, 0.8_f64);
        let mut inf = Influence::new(n);
        let p = FieldPoint {
            node: None,
            x: px,
            y: py,
            nx,
            ny,
        };
        psilin(&geo, &p, true, &mut inf);
        let q: f64 = inf
            .dq_dgamma
            .iter()
            .zip(&gamma)
            .map(|(a, b)| a * b)
            .sum::<f64>()
            + inf
                .dq_dsigma
                .iter()
                .zip(&sigma)
                .map(|(a, b)| a * b)
                .sum::<f64>();
        let h = 1e-6;
        let dpsi = (psi(&geo, &gamma, &sigma, px + h * nx, py + h * ny)
            - psi(&geo, &gamma, &sigma, px - h * nx, py - h * ny))
            / (2.0 * h);
        assert!((q - dpsi).abs() < 1e-7, "q {q} dpsi/dn {dpsi}");
    }

    #[test]
    fn source_sheet_produces_normal_velocity_jump() {
        // sigma = s gives a unit source strength d(sigma)/ds: the normal
        // velocity jumps by 1 across the sheet.
        let geo = geometry();
        let n = geo.len();
        let sigma = geo.s.clone();
        let gamma = vec![0.0; n];
        let i = 15;
        let (tx, ty) = (geo.x[i + 1] - geo.x[i], geo.y[i + 1] - geo.y[i]);
        let len = tx.hypot(ty);
        let (tx, ty) = (tx / len, ty / len);
        // Outward normal of a counterclockwise contour.
        let (onx, ony) = (ty, -tx);
        let (mx, my) = (
            0.5 * (geo.x[i] + geo.x[i + 1]),
            0.5 * (geo.y[i] + geo.y[i + 1]),
        );
        let eps = 1e-4 * len;
        let normal_velocity = |sign: f64| {
            let mut inf = Influence::new(n);
            let p = FieldPoint {
                node: None,
                x: mx + sign * eps * onx,
                y: my + sign * eps * ony,
                nx: tx,
                ny: ty,
            };
            psilin(&geo, &p, true, &mut inf);
            inf.dq_dgamma
                .iter()
                .zip(&gamma)
                .map(|(a, b)| a * b)
                .sum::<f64>()
                + inf
                    .dq_dsigma
                    .iter()
                    .zip(&sigma)
                    .map(|(a, b)| a * b)
                    .sum::<f64>()
        };
        let jump = normal_velocity(1.0) - normal_velocity(-1.0);
        assert!((jump - 1.0).abs() < 1e-3, "jump {jump}");
    }

    #[test]
    fn influence_buffers_are_reset() {
        let geo = geometry();
        let mut inf = Influence::new(geo.len());
        let p = FieldPoint {
            node: Some(3),
            x: geo.x[3],
            y: geo.y[3],
            nx: geo.nx[3],
            ny: geo.ny[3],
        };
        psilin(&geo, &p, true, &mut inf);
        let first = inf.clone();
        psilin(&geo, &p, true, &mut inf);
        assert_eq!(first, inf);
    }
}
