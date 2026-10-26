//! Full inverse design, a port of XFOIL's `MDES`: the airfoil is the
//! conformal image of a circle, defined by the Fourier coefficients `Cn` of
//! the complex function `P(w) + iQ(w)` on the circle; the surface speed
//! prescribed over the whole airfoil gives `P`, hence the `Cn`, hence the
//! shape (Lighthill's method, with XFOIL's trailing-edge gap and angle
//! constraints).

use std::f64::consts::PI;

use aero_geometry::{Airfoil, EndCondition, Spline};
use serde::{Deserialize, Serialize};

use super::complex::C64;
use super::error::{InverseError, Result};
use super::mixed::smooq;
use crate::inviscid::PanelGeometry;

/// Number of points on the circle (XFOIL `NC1` for `IQX = 601`).
const NC: usize = 257;
/// Number of Fourier coefficients (`MC = NC/4`).
const MC: usize = NC / 4;

/// How the design points of a full inverse are specified (XFOIL `IACQSP`).
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesignBy {
    /// Angle of attack (`AQ`).
    Alpha,
    /// Lift coefficient (`CQ`); the angle is solved for.
    Lift,
}

/// One design point of a full inverse: the target surface speed at an
/// angle of attack or lift coefficient (XFOIL `QSPEC(·, k)`).
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesignPoint {
    /// Angle of attack, degrees.
    pub alpha_deg: f64,
    /// Lift coefficient of the target distribution.
    pub cl: f64,
    /// Pitching moment coefficient about the quarter chord.
    pub cm: f64,
    /// Target speed `q/V∞` at the circle points, signed as XFOIL's `γ`.
    pub speed: Vec<f64>,
}

/// Result of [`FullInverse::execute`].
#[derive(Debug, Clone, PartialEq)]
pub struct FullInverseResult {
    /// The new airfoil, with one point per circle point (257).
    pub airfoil: Airfoil,
    /// True if the trailing-edge gap constraint converged.
    pub converged: bool,
    /// Newton iterations on the trailing-edge gap.
    pub iterations: usize,
    /// Size of the correction of `C₁` at each iteration.
    pub corrections: Vec<f64>,
}

/// State of the circle-plane mapping (XFOIL `CIRCLE.INC`).
#[derive(Debug, Clone)]
struct Mapping {
    wc: Vec<f64>,
    dwc: f64,
    /// `exp(i m w)`, indexed `[m][ic]`.
    eiw: Vec<Vec<C64>>,
    /// Trailing-edge angle / π.
    agte: f64,
    qimold: f64,
    sc: Vec<f64>,
    xcold: Vec<f64>,
    ycold: Vec<f64>,
    dzte: C64,
    chordz: C64,
    zleold: C64,
    zc: Vec<C64>,
    /// `dz/dC₁` (XFOIL `ZC_CN(·, 1)`; the other columns are not used).
    zc_cn: Vec<C64>,
    piq: Vec<C64>,
    cn: Vec<C64>,
}

impl Mapping {
    /// XFOIL `EIWSET`.
    fn new() -> Self {
        let dwc = 2.0 * PI / (NC - 1) as f64;
        let wc: Vec<f64> = (0..NC).map(|ic| dwc * ic as f64).collect();
        let mut eiw = vec![vec![C64::new(1.0, 0.0); NC]; MC + 1];
        eiw[1] = wc.iter().map(|&w| C64::new(0.0, w).exp()).collect();
        for m in 2..=MC {
            eiw[m] = (0..NC).map(|ic| eiw[1][(m * ic) % (NC - 1)]).collect();
        }
        Self {
            wc,
            dwc,
            eiw,
            agte: 0.0,
            qimold: 0.0,
            sc: vec![0.0; NC],
            xcold: vec![0.0; NC],
            ycold: vec![0.0; NC],
            dzte: C64::ZERO,
            chordz: C64::ZERO,
            zleold: C64::ZERO,
            zc: vec![C64::ZERO; NC],
            zc_cn: vec![C64::ZERO; NC],
            piq: vec![C64::ZERO; NC],
            cn: vec![C64::ZERO; MC + 1],
        }
    }

    /// XFOIL `PIQSUM`: `P + iQ` from the coefficients.
    fn piqsum(&mut self) {
        for ic in 0..NC {
            let mut z = C64::ZERO;
            for m in 0..=MC {
                z = z + self.cn[m] * self.eiw[m][ic].conj();
            }
            self.piq[ic] = z;
        }
    }

    /// XFOIL `FTP`: Fourier coefficients of `P + iQ` (trapezoidal rule).
    fn ftp(&mut self) {
        for m in 0..=MC {
            let e = &self.eiw[m];
            let mut z = C64::ZERO;
            for (p, e) in self.piq[1..NC - 1].iter().zip(&e[1..NC - 1]) {
                z = z + *p * *e;
            }
            let ends = (self.piq[0] * e[0] + self.piq[NC - 1] * e[NC - 1]).scale(0.5);
            self.cn[m] = (ends + z).scale(self.dwc).div_real(PI);
        }
        self.cn[0] = self.cn[0].scale(0.5);
    }

    /// XFOIL `CNFILT`: modified Hanning filter of the coefficients.
    fn cnfilt(&mut self, ffilt: f64) {
        if ffilt == 0.0 {
            return;
        }
        for (m, cn) in self.cn.iter_mut().enumerate() {
            let freq = m as f64 / MC as f64;
            let cwt = 0.5 * (1.0 + (PI * freq).cos());
            let cwtx = if ffilt > 0.0 { cwt.powf(ffilt) } else { cwt };
            *cn = cn.scale(cwtx);
        }
    }

    /// XFOIL `CNSYMM`: drops the imaginary (camber) parts.
    fn cnsymm(&mut self) {
        for cn in &mut self.cn[1..] {
            *cn = C64::new(cn.re, 0.0);
        }
        self.piqsum();
    }

    /// `dz/dw` at circle point `ic`.
    fn dzdw(&self, ic: usize) -> C64 {
        let sinw = 2.0 * (0.5 * self.wc[ic]).sin();
        let sinwe = if sinw > 0.0 {
            sinw.powf(1.0 - self.agte)
        } else {
            0.0
        };
        let hwc = 0.5 * (self.wc[ic] - PI) * (1.0 + self.agte) - 0.5 * PI;
        let p = self.piq[ic];
        C64::new(p.re, p.im + hwc).exp().scale(sinwe)
    }

    /// XFOIL `ZCCALC(1)`: the geometry `z(w)` by integration of `dz/dw`,
    /// its sensitivity to `C₁` and its normalized arc length.
    fn zccalc(&mut self) {
        self.zc[0] = C64::new(4.0, 0.0);
        self.zc_cn[0] = C64::ZERO;
        let mut dzdw1 = self.dzdw(0);
        for ic in 1..NC {
            let dzdw2 = self.dzdw(ic);
            self.zc[ic] = (dzdw1 + dzdw2).scale(0.5).scale(self.dwc) + self.zc[ic - 1];
            let dz_piq1 = dzdw1.scale(0.5).scale(self.dwc);
            let dz_piq2 = dzdw2.scale(0.5).scale(self.dwc);
            self.zc_cn[ic] = dz_piq1 * self.eiw[1][ic - 1].conj()
                + dz_piq2 * self.eiw[1][ic].conj()
                + self.zc_cn[ic - 1];
            dzdw1 = dzdw2;
        }
        self.sc[0] = 0.0;
        for ic in 1..NC {
            self.sc[ic] = self.sc[ic - 1] + (self.zc[ic] - self.zc[ic - 1]).abs();
        }
        let total = self.sc[NC - 1];
        self.sc.iter_mut().for_each(|v| *v /= total);
    }

    /// XFOIL `ZLEFIND`: the leading edge of `z(w)`, where the chord line is
    /// normal to the contour, splined near the farthest point from the
    /// trailing edge.
    fn zlefind(&self) -> Result<C64> {
        const HALF: usize = 16; // (NTX − 1)/2
        let zc = &self.zc;
        let zte = (zc[0] + zc[NC - 1]).scale(0.5);
        let mut dmax = 0.0;
        let mut icle = 0;
        for (ic, z) in zc.iter().enumerate() {
            let dist = (*z - zte).abs();
            if dist > dmax {
                dmax = dist;
                icle = ic;
            }
        }
        let ic1 = icle.saturating_sub(HALF);
        let ic2 = (icle + HALF).min(NC - 1);
        let (d1, d2) = (self.dzdw(ic1), self.dzdw(ic2));
        let w = &self.wc[ic1..=ic2];
        let xc: Vec<f64> = zc[ic1..=ic2].iter().map(|z| z.re).collect();
        let yc: Vec<f64> = zc[ic1..=ic2].iter().map(|z| z.im).collect();
        use EndCondition::FirstDerivative as D;
        let sx = Spline::new(w, &xc, D(d1.re), D(d2.re))?;
        let sy = Spline::new(w, &yc, D(d1.im), D(d2.im))?;
        let xcte = 0.5 * (zc[0] + zc[NC - 1]).re;
        let ycte = 0.5 * (zc[0] + zc[NC - 1]).im;
        let mut wcle = self.wc[icle];
        let mut converged = false;
        for _ in 0..10 {
            let xchord = sx.eval(wcle) - xcte;
            let ychord = sy.eval(wcle) - ycte;
            let (dxdw, dydw) = (sx.deriv(wcle), sy.deriv(wcle));
            let (dxdd, dydd) = (sx.deriv2(wcle), sy.deriv2(wcle));
            let res = xchord * dxdw + ychord * dydw;
            let resw = dxdw * dxdw + dydw * dydw + xchord * dxdd + ychord * dydd;
            let dwcle = -res / resw;
            wcle += dwcle;
            if dwcle.abs() < 1.0e-5 {
                converged = true;
                break;
            }
        }
        if !converged {
            wcle = self.wc[icle];
        }
        Ok(C64::new(sx.eval(wcle), sy.eval(wcle)))
    }

    /// XFOIL `ZCNORM(1)`: scales and rotates `z(w)` to the old chord, puts
    /// the leading edge at the old position.
    fn zcnorm(&mut self) -> Result<()> {
        let zle = self.zlefind()?;
        self.zc.iter_mut().for_each(|z| *z = *z - zle);
        let zte = (self.zc[0] + self.zc[NC - 1]).scale(0.5);
        let zte_cn = (self.zc_cn[0] + self.zc_cn[NC - 1]).scale(0.5);
        for ic in 0..NC {
            let zcnew = (self.chordz * self.zc[ic]) / zte;
            let zc_zte = -(zcnew / zte);
            self.zc[ic] = zcnew;
            self.zc_cn[ic] = (self.chordz * self.zc_cn[ic]) / zte + zc_zte * zte_cn;
        }
        // Rotation added to the mapping so that QCCALC gets the right angle.
        let qimoff = -(self.chordz / zte).arg();
        self.cn[0] = C64::new(self.cn[0].re, self.cn[0].im - qimoff);
        let zleold = self.zleold;
        self.zc.iter_mut().for_each(|z| *z = *z + zleold);
        Ok(())
    }

    /// XFOIL `SCINIT`: the circle-plane coordinate of the current airfoil
    /// and its mapping coefficients.
    fn scinit(&mut self, geo: &PanelGeometry) -> Result<()> {
        const CEPS: f64 = 1.0e-7;
        const SEPS: f64 = 5.0e-7;
        let (x, y, xp, yp, s) = (&geo.x, &geo.y, &geo.xp, &geo.yp, &geo.s);
        let n = x.len();
        let sle = geo.sle;
        let seval = aero_geometry::spline::seval;
        let deval = aero_geometry::spline::deval;

        self.agte = ((xp[n - 1]).atan2(-yp[n - 1]) - (xp[0]).atan2(-yp[0])) / PI - 1.0;
        let ag0 = xp[0].atan2(-yp[0]);
        // Offset making Q(w) − Q₀ = 0 at w = 0 and 2π (no Gibbs problem).
        let qim0 = ag0 + 0.5 * PI * (1.0 + self.agte);
        let xle = seval(sle, x, xp, s);
        let yle = seval(sle, y, yp, s);
        self.dzte = C64::new(x[0] - x[n - 1], y[0] - y[n - 1]);
        self.chordz = C64::new(0.5 * (x[0] + x[n - 1]) - xle, 0.5 * (y[0] + y[n - 1]) - yle);
        self.zleold = C64::new(xle, yle);

        // Initial s(w), with an approximate ds/dw at the nose.
        let curve = aero_geometry::SplineCurve::new(x, y)?;
        let cvle = curve.curvature(sle) * s[n - 1];
        let dsdwle = (0.5 / cvle.abs()).max(1.0e-3);
        let tops = sle / s[n - 1];
        let bots = (s[n - 1] - sle) / s[n - 1];
        let wwt = 1.0 - 2.0 * dsdwle / tops;
        for ic in 0..=(NC - 1) / 2 {
            self.sc[ic] = tops * (1.0 - (wwt * self.wc[ic]).cos()) / (1.0 - (wwt * PI).cos());
        }
        let wwt = 1.0 - 2.0 * dsdwle / bots;
        for ic in (NC - 1) / 2 + 1..NC {
            self.sc[ic] = 1.0
                - bots * (1.0 - (wwt * (self.wc[NC - 1] - self.wc[ic])).cos())
                    / (1.0 - (wwt * PI).cos());
        }

        let mut scold = vec![0.0; NC];
        for _ in 0..30 {
            // Q(w) − Q₀ from the surface angle at s(w).
            for ic in 0..NC {
                let sic = s[0] + (s[n - 1] - s[0]) * self.sc[ic];
                let dxds = deval(sic, x, xp, s);
                let dyds = deval(sic, y, yp, s);
                let qim = dxds.atan2(-dyds) - 0.5 * (self.wc[ic] - PI) * (1.0 + self.agte) - qim0;
                self.piq[ic] = C64::new(0.0, qim);
            }
            self.ftp();
            self.cn[0] = C64::new(0.0, self.cn[0].im + qim0);
            self.piqsum();
            scold.copy_from_slice(&self.sc);

            // Correct C₁ for the trailing-edge gap.
            for _ in 0..5 {
                self.zccalc();
                let zle = self.zlefind()?;
                let zte = (self.zc[0] + self.zc[NC - 1]).scale(0.5);
                let dzwt = (zte - zle).abs() / self.chordz.abs();
                let dcn = -((self.zc[0] - self.zc[NC - 1] - self.dzte.scale(dzwt))
                    / (self.zc_cn[0] - self.zc_cn[NC - 1]));
                self.cn[1] = self.cn[1] + dcn;
                self.piqsum();
                if dcn.abs() < CEPS {
                    break;
                }
            }
            let dscmax = self
                .sc
                .iter()
                .zip(&scold)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f64::max);
            if dscmax < SEPS {
                break;
            }
        }
        self.zcnorm()?;
        for ic in 0..NC {
            self.xcold[ic] = self.zc[ic].re;
            self.ycold[ic] = self.zc[ic].im;
        }
        self.qimold = self.cn[0].im;
        Ok(())
    }

    /// XFOIL `QCCALC` (incompressible): the surface speed on the circle for
    /// a prescribed angle (`by = Alpha`), or for a prescribed lift, the
    /// angle being updated (`by = Lift`). Returns `(cl, cm, speed)`.
    fn qccalc(&self, by: DesignBy, alfa: &mut f64, cl_spec: f64) -> (f64, f64, Vec<f64>) {
        const AEPS: f64 = 5.0e-7;
        let (beta, bfac, qinf) = (1.0, 0.0, 1.0);
        let mut qcir = vec![0.0; NC];
        let mut qc_a = vec![0.0; NC];
        let (mut cl, mut cm) = (cl_spec, 0.0);
        for _ in 0..10 {
            let alfcir = *alfa - self.cn[0].im;
            let mut cmt = C64::ZERO;
            let mut cft = C64::ZERO;
            let mut cft_a = C64::ZERO;
            for ic in 0..NC {
                let eppp = (-self.piq[ic].re).exp();
                let sinw = 2.0 * (0.5 * self.wc[ic]).sin();
                let sinwe = if self.agte == 0.0 {
                    1.0
                } else if sinw > 0.0 {
                    sinw.powf(self.agte)
                } else {
                    0.0
                };
                qcir[ic] = 2.0 * (0.5 * self.wc[ic] - alfcir).cos() * sinwe * eppp;
                qc_a[ic] = 2.0 * (0.5 * self.wc[ic] - alfcir).sin() * sinwe * eppp;
            }
            // Integrate Cp dz for the complex force CL + iCD.
            let cp = |q: f64| {
                let cpinc = 1.0 - (q / qinf) * (q / qinf);
                let cpi_q = -(2.0 * q / (qinf * qinf));
                let cpcom = cpinc / (beta + bfac * cpinc);
                let cpc_q = (1.0 - bfac * cpcom) / (beta + bfac * cpinc) * cpi_q;
                (cpcom, cpc_q)
            };
            let (mut cpcom1, cpc_q1) = cp(qcir[0]);
            let mut cpc_a1 = cpc_q1 * qc_a[0];
            for ic in 0..NC {
                let icp = if ic == NC - 1 { 0 } else { ic + 1 };
                let (cpcom2, cpc_q2) = cp(qcir[icp]);
                let cpc_a2 = cpc_q2 * qc_a[icp];
                let za = (self.zc[icp] + self.zc[ic]).scale(0.5) - C64::new(0.25, 0.0);
                let dz = self.zc[icp] - self.zc[ic];
                cmt = cmt - dz.scale(0.5 * (cpcom1 + cpcom2)) * za.conj()
                    + (dz.scale(cpcom1 - cpcom2) * dz.conj()).div_real(12.0);
                cft = cft + dz.scale(0.5 * (cpcom1 + cpcom2));
                cft_a = cft_a + dz.scale(0.5 * (cpc_a1 + cpc_a2));
                cpcom1 = cpcom2;
                cpc_a1 = cpc_a2;
            }
            // Rotate the force into the freestream axes.
            let eia = C64::new(0.0, -*alfa).exp();
            cft = cft * eia;
            cft_a = cft_a * eia + cft * C64::new(0.0, -1.0);
            let clt = cft.re;
            let clt_a = cft_a.re;
            cm = cmt.re;
            match by {
                DesignBy::Alpha => {
                    cl = clt;
                    break;
                }
                DesignBy::Lift => {
                    let dalfa = (cl_spec - clt) / clt_a;
                    *alfa += dalfa;
                    if dalfa.abs() < AEPS {
                        break;
                    }
                }
            }
        }
        (cl, cm, qcir)
    }

    /// XFOIL `CNCALC`: the coefficients from a surface speed on the circle.
    /// With `symmetric`, the change of the real parts is doubled and the
    /// imaginary parts are dropped.
    fn cncalc(&mut self, qc: &[f64], symmetric: bool) -> Result<()> {
        let spline = Spline::new(
            &self.wc,
            qc,
            EndCondition::ZeroThirdDerivative,
            EndCondition::ZeroThirdDerivative,
        )?;
        let qcw = spline.derivatives();
        // The stagnation point, where q changes sign.
        let ic = (1..NC)
            .find(|&ic| qc[ic] < 0.0)
            .ok_or(InverseError::NoStagnationPoint)?;
        let guess = self.wc[ic];
        let wcle = spline.invert(0.0, guess).unwrap_or(guess);
        let alfcir = 0.5 * (wcle - PI);
        for ic in 1..NC - 1 {
            let cosw = 2.0 * (0.5 * self.wc[ic] - alfcir).cos();
            let sinw = 2.0 * (0.5 * self.wc[ic]).sin();
            let sinwe = sinw.powf(self.agte);
            let pfun = if cosw.abs() < 1.0e-4 {
                // Asymptotic form near the stagnation point.
                (sinwe / qcw[ic]).abs()
            } else {
                (cosw * sinwe / qc[ic]).abs()
            };
            self.piq[ic] = C64::new(pfun.ln(), 0.0);
        }
        // Extrapolate P(w) to the trailing edge.
        let p: Vec<f64> = self.piq.iter().map(|z| z.re).collect();
        self.piq[0] = C64::new(3.0 * p[1] - 3.0 * p[2] + p[3], 0.0);
        self.piq[NC - 1] = C64::new(3.0 * p[NC - 2] - 3.0 * p[NC - 3] + p[NC - 4], 0.0);
        let saved = self.cn.clone();
        self.ftp();
        self.cn[0] = C64::new(0.0, self.qimold);
        if symmetric {
            for (cn, old) in self.cn[1..].iter_mut().zip(&saved[1..]) {
                *cn = C64::new(2.0 * cn.re - old.re, 0.0);
            }
        }
        self.piqsum();
        Ok(())
    }

    /// XFOIL `MAPGEN`: the geometry from the coefficients, `C₁` being
    /// adjusted by Newton iteration for the trailing-edge gap (the new
    /// airfoil is left in `zc`). Returns the corrections of `C₁` and whether
    /// they converged.
    fn mapgen(&mut self) -> Result<(Vec<f64>, bool)> {
        // Rotation offset so that the first angle is near the old one.
        let dx = self.xcold[1] - self.xcold[0];
        let dy = self.ycold[1] - self.ycold[0];
        let qim0 = dx.atan2(-dy) + 0.5 * PI * (1.0 + self.agte);
        let qimoff = qim0 - self.cn[0].im;
        self.cn[0] = C64::new(self.cn[0].re, self.cn[0].im + qimoff);
        self.piqsum();
        self.zccalc();
        self.zcnorm()?;
        // Lighthill's first constraint.
        self.cn[0] = C64::new(0.0, self.cn[0].im);
        let mut corrections = Vec::new();
        let mut converged = false;
        for _ in 0..10 {
            let res = self.zc[0] - self.zc[NC - 1] - self.dzte;
            let jac = self.zc_cn[0] - self.zc_cn[NC - 1];
            let dcn = res / jac;
            self.cn[1] = self.cn[1] - dcn;
            let dcnmax = dcn.abs();
            corrections.push(dcnmax);
            self.piqsum();
            self.zccalc();
            self.zcnorm()?;
            if !dcnmax.is_finite() {
                return Err(InverseError::Singular);
            }
            if dcnmax <= 5.0e-5 {
                converged = true;
                break;
            }
        }
        Ok((corrections, converged))
    }
}

/// A full inverse design session (XFOIL `MDES`): the mapping of the
/// airfoil, its design points and their target speed distributions.
///
/// Create it from the paneled airfoil and an angle of attack, change the
/// targets ([`set_alphas`](Self::set_alphas), [`filter`](Self::filter),
/// [`smooth`](Self::smooth), [`set_target`](Self::set_target), ...), then
/// [`execute`](Self::execute) to get the new airfoil.
#[derive(Debug, Clone)]
pub struct FullInverse {
    map: Mapping,
    name: String,
    /// Speed of the current airfoil at its design angle (XFOIL `QGAMM`).
    current: DesignPoint,
    /// Normalized arc length of the circle points (XFOIL `SSPEC`).
    arc_length: Vec<f64>,
    by: DesignBy,
    points: Vec<DesignPoint>,
    /// Angles of the design points, radians (XFOIL `ALQSP`).
    alfas: Vec<f64>,
    symmetric: bool,
}

impl FullInverse {
    /// Starts a full inverse design of a paneled airfoil at `alpha_deg`
    /// (XFOIL `MDES` entry: `EIWSET`, `SCINIT`, `MAPGAM`, `CNCALC`,
    /// `QSPCIR`). The target is the current speed distribution.
    pub fn new(paneled: &Airfoil, alpha_deg: f64) -> Result<Self> {
        if !alpha_deg.is_finite() {
            return Err(InverseError::InvalidOption(
                "the angle of attack must be finite".into(),
            ));
        }
        let geo = PanelGeometry::new(paneled)?;
        let mut map = Mapping::new();
        map.scinit(&geo)?;
        let alfa = alpha_deg.to_radians();
        let (cl, cm, speed) = map.qccalc(DesignBy::Alpha, &mut alfa.clone(), 0.0);
        let arc_length = map.sc.clone();
        let current = DesignPoint {
            alpha_deg,
            cl,
            cm,
            speed,
        };
        map.cncalc(&current.speed, false)?;
        let mut session = Self {
            map,
            name: paneled.name().to_string(),
            points: vec![DesignPoint {
                speed: Vec::new(),
                ..current.clone()
            }],
            current,
            arc_length,
            by: DesignBy::Alpha,
            alfas: vec![alfa],
            symmetric: false,
        };
        session.qspcir();
        Ok(session)
    }

    /// XFOIL `QSPCIR`: the target speed of every design point.
    fn qspcir(&mut self) {
        for (p, alfa) in self.points.iter_mut().zip(&mut self.alfas) {
            let (cl, cm, speed) = self.map.qccalc(self.by, alfa, p.cl);
            p.alpha_deg = alfa.to_degrees();
            p.cl = cl;
            p.cm = cm;
            p.speed = speed;
        }
    }

    /// The design points and their target speeds.
    pub fn points(&self) -> &[DesignPoint] {
        &self.points
    }

    /// The speed of the original airfoil at the starting angle.
    pub fn current(&self) -> &DesignPoint {
        &self.current
    }

    /// Normalized arc length of the circle points on the original airfoil
    /// (0 at the upper trailing edge, 1 at the lower one).
    pub fn arc_length(&self) -> &[f64] {
        &self.arc_length
    }

    /// Coordinates of the circle points on the original airfoil.
    pub fn circle_points(&self) -> (&[f64], &[f64]) {
        (&self.map.xcold, &self.map.ycold)
    }

    /// Trailing-edge gap `(dx, dy)` of the new airfoil.
    pub fn te_gap(&self) -> [f64; 2] {
        [self.map.dzte.re, self.map.dzte.im]
    }

    /// Trailing-edge angle of the new airfoil, degrees.
    pub fn te_angle_deg(&self) -> f64 {
        self.map.agte * 180.0
    }

    /// True if symmetry is enforced.
    pub fn is_symmetric(&self) -> bool {
        self.symmetric
    }

    /// How the design points are specified.
    pub fn design_by(&self) -> DesignBy {
        self.by
    }

    /// Design points at these angles of attack, degrees (XFOIL `AQ`).
    pub fn set_alphas(&mut self, alphas_deg: &[f64]) -> Result<()> {
        if alphas_deg.is_empty() || alphas_deg.iter().any(|a| !a.is_finite()) {
            return Err(InverseError::InvalidOption(
                "give at least one finite angle".into(),
            ));
        }
        self.by = DesignBy::Alpha;
        self.alfas = alphas_deg.iter().map(|a| a.to_radians()).collect();
        self.points = alphas_deg
            .iter()
            .map(|&alpha_deg| DesignPoint {
                alpha_deg,
                cl: 0.0,
                cm: 0.0,
                speed: Vec::new(),
            })
            .collect();
        self.qspcir();
        Ok(())
    }

    /// Design points at these lift coefficients (XFOIL `CQ`); the angles
    /// are solved for, starting from the current ones.
    pub fn set_lift_coefficients(&mut self, cls: &[f64]) -> Result<()> {
        if cls.is_empty() || cls.iter().any(|c| !c.is_finite()) {
            return Err(InverseError::InvalidOption(
                "give at least one finite lift coefficient".into(),
            ));
        }
        self.by = DesignBy::Lift;
        // The angles are solved for from the previous ones.
        let start = self.alfas[0];
        self.alfas.resize(cls.len(), start);
        self.points = cls
            .iter()
            .zip(&self.alfas)
            .map(|(&cl, alfa)| DesignPoint {
                alpha_deg: alfa.to_degrees(),
                cl,
                cm: 0.0,
                speed: Vec::new(),
            })
            .collect();
        self.qspcir();
        Ok(())
    }

    /// Enforces (or stops enforcing) a symmetric airfoil (XFOIL `SYMM`):
    /// when switched on, the camber part of the mapping is removed.
    pub fn set_symmetric(&mut self, symmetric: bool) {
        if symmetric && !self.symmetric {
            self.map.cnsymm();
            self.qspcir();
        }
        self.symmetric = symmetric;
    }

    /// Trailing-edge gap `(dx, dy)` of the new airfoil (XFOIL `TGAP`).
    pub fn set_te_gap(&mut self, dx: f64, dy: f64) -> Result<()> {
        if !dx.is_finite() || !dy.is_finite() {
            return Err(InverseError::InvalidOption(
                "the trailing-edge gap must be finite".into(),
            ));
        }
        self.map.dzte = C64::new(dx, dy);
        Ok(())
    }

    /// Trailing-edge angle of the new airfoil, degrees (XFOIL `TANG`).
    pub fn set_te_angle(&mut self, angle_deg: f64) -> Result<()> {
        if !angle_deg.is_finite() {
            return Err(InverseError::InvalidOption(
                "the trailing-edge angle must be finite".into(),
            ));
        }
        self.map.agte = angle_deg / 180.0;
        Ok(())
    }

    /// Hanning filter of the mapping coefficients (XFOIL `FILT`, strength
    /// 0.2), which smooths all the target distributions.
    pub fn filter(&mut self) {
        self.map.cnfilt(0.2);
        self.map.piqsum();
        self.qspcir();
    }

    /// Smooths the target of the first design point between circle points
    /// `first` and `last` (XFOIL `SMOO`; the whole airfoil is `0` to
    /// `256`), keeping the end values and, with `slope_matching`, the end
    /// slopes.
    pub fn smooth(&mut self, first: usize, last: usize, slope_matching: bool) -> Result<()> {
        if first >= last || last >= NC {
            return Err(InverseError::InvalidSegment {
                first,
                last,
                nodes: NC,
            });
        }
        let mut speed = self.points[0].speed.clone();
        smooq(&self.arc_length, &mut speed, first, last, slope_matching);
        self.set_target(&speed)
    }

    /// Replaces the target of the first design point (XFOIL `MODI`): the
    /// mapping coefficients are recomputed from it, and the targets of all
    /// the design points follow.
    pub fn set_target(&mut self, speed: &[f64]) -> Result<()> {
        if speed.len() != NC || speed.iter().any(|v| !v.is_finite()) {
            return Err(InverseError::InvalidOption(format!(
                "the target needs {NC} finite values"
            )));
        }
        self.map.cncalc(speed, self.symmetric)?;
        self.qspcir();
        Ok(())
    }

    /// Resets the target to the speed of the original airfoil (XFOIL
    /// `QSET`).
    pub fn reset(&mut self) -> Result<()> {
        let speed = self.current.speed.clone();
        self.map.cncalc(&speed, false)?;
        if self.symmetric {
            self.map.cnsymm();
        }
        self.qspcir();
        Ok(())
    }

    /// Generates the airfoil of the current mapping (XFOIL `EXEC`,
    /// `MAPGEN`), with the trailing-edge gap constraint.
    pub fn execute(&mut self) -> Result<FullInverseResult> {
        let (corrections, converged) = self.map.mapgen()?;
        let x = self.map.zc.iter().map(|z| z.re).collect();
        let y = self.map.zc.iter().map(|z| z.im).collect();
        let airfoil = Airfoil::new(&self.name, x, y)?;
        Ok(FullInverseResult {
            airfoil,
            converged,
            iterations: corrections.len(),
            corrections,
        })
    }
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
    fn unchanged_target_gives_the_airfoil_back() {
        let a = paneled("2412");
        let mut design = FullInverse::new(&a, 3.0).unwrap();
        assert_eq!(design.points().len(), 1);
        assert!((design.current().cl - 0.616).abs() < 0.01);
        let r = design.execute().unwrap();
        assert!(r.converged);
        assert_eq!(r.airfoil.len(), NC);
        // Same shape: compare thickness and camber.
        let (p, q) = (a.properties(), r.airfoil.properties());
        assert!((p.max_thickness - q.max_thickness).abs() < 1e-3);
        assert!((p.max_camber - q.max_camber).abs() < 1e-3);
    }

    #[test]
    fn design_by_lift_finds_the_angle() {
        let a = paneled("2412");
        let mut design = FullInverse::new(&a, 3.0).unwrap();
        design.set_lift_coefficients(&[0.8]).unwrap();
        let p = &design.points()[0];
        assert!((p.cl - 0.8).abs() < 1e-5, "{p:?}");
        assert!(p.alpha_deg > 3.0);
        design.set_alphas(&[2.0, 4.0]).unwrap();
        assert_eq!(design.points().len(), 2);
        assert!(design.points()[1].cl > design.points()[0].cl);
    }

    #[test]
    fn symmetric_design() {
        let a = paneled("2412");
        let mut design = FullInverse::new(&a, 0.0).unwrap();
        design.set_symmetric(true);
        let r = design.execute().unwrap();
        assert!(r.airfoil.properties().max_camber.abs() < 1e-3);
        assert!(design.smooth(10, 5, true).is_err());
        assert!(design.set_target(&[1.0; 3]).is_err());
    }
}
