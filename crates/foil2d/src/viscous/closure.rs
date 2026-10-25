//! Closure relations of the integral boundary layer (XFOIL `xblsys.f`:
//! `HKIN`, `HSL`, `HST`, `CFL`, `CFT`, `HCT`, `DIL`, `DILW`, `DAMPL`,
//! `DAMPL2`; `xbl.f`: `BLPINI`).
//!
//! Every function returns the correlated quantity followed by its partial
//! derivatives, in the order of the arguments.
//!
//! Notation: `hk` kinematic shape parameter, `rt` momentum-thickness
//! Reynolds number Re_θ, `msq` edge Mach number squared, `th` momentum
//! thickness θ.

// XFOIL writes ln 10 as 2.3026 and 2.3025851; the truncated values are kept.
#![allow(clippy::approx_constant)]

/// Constants of the turbulent closure and of the shear-lag equation (XFOIL
/// common block `BLPAR`, set by `BLPINI`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BlParams {
    /// Shear-coefficient lag constant `SCCON`.
    pub sccon: f64,
    /// G–β locus constant `GACON`: `G = GACON·√(1 + GBCON·β)`.
    pub gacon: f64,
    /// G–β locus constant `GBCON`.
    pub gbcon: f64,
    /// Wall term of the G–β locus `GCCON`.
    pub gccon: f64,
    /// Wall/wake dissipation length ratio `DLCON`.
    pub dlcon: f64,
    /// Initial shear coefficient at transition, `CTRCON·exp(−CTRCEX/(Hk−1))`.
    pub ctrcon: f64,
    /// See [`Self::ctrcon`].
    pub ctrcex: f64,
    /// Weight of the equilibrium `Ue'` term in the shear-lag equation.
    pub duxcon: f64,
    /// Ctau weighting coefficient implied by the G–β constants.
    pub ctcon: f64,
    /// Turbulent skin-friction factor.
    pub cffac: f64,
}

impl Default for BlParams {
    /// XFOIL's `BLPINI` values.
    fn default() -> Self {
        let gacon = 6.70;
        let gbcon = 0.75;
        Self {
            sccon: 5.6,
            gacon,
            gbcon,
            gccon: 18.0,
            dlcon: 0.9,
            ctrcon: 1.8,
            ctrcex: 3.3,
            duxcon: 1.0,
            ctcon: 0.5 / (gacon * gacon * gbcon),
            cffac: 1.0,
        }
    }
}

/// Kinematic shape parameter from `H` and `M²` (Whitfield, air): `HKIN`.
pub(crate) fn hkin(h: f64, msq: f64) -> (f64, f64, f64) {
    let den = 1.0 + 0.113 * msq;
    let hk = (h - 0.29 * msq) / den;
    let hk_h = 1.0 / den;
    let hk_msq = (-0.29 - 0.113 * hk) / den;
    (hk, hk_h, hk_msq)
}

/// Laminar dissipation function `2·CD/H*` (Falkner–Skan): `DIL`.
pub(crate) fn dil(hk: f64, rt: f64) -> (f64, f64, f64) {
    let (di, di_hk) = if hk < 4.0 {
        (
            (0.00205 * (4.0 - hk).powf(5.5) + 0.207) / rt,
            (-0.00205 * 5.5 * (4.0 - hk).powf(4.5)) / rt,
        )
    } else {
        let hkb = hk - 4.0;
        let den = 1.0 + 0.02 * hkb * hkb;
        (
            (-0.0016 * hkb * hkb / den + 0.207) / rt,
            (-0.0016 * 2.0 * hkb * (1.0 / den - 0.02 * hkb * hkb / (den * den))) / rt,
        )
    };
    (di, di_hk, -di / rt)
}

/// Laminar wake dissipation function `2·CD/H*`: `DILW`.
pub(crate) fn dilw(hk: f64, rt: f64) -> (f64, f64, f64) {
    let (hs, hs_hk, hs_rt, _) = hsl(hk, rt, 0.0);
    let rcd = 1.10 * (1.0 - 1.0 / hk).powi(2) / hk;
    let rcd_hk = -1.10 * (1.0 - 1.0 / hk) * 2.0 / hk.powi(3) - rcd / hk;
    let di = 2.0 * rcd / (hs * rt);
    let di_hk = 2.0 * rcd_hk / (hs * rt) - (di / hs) * hs_hk;
    let di_rt = -di / rt - (di / hs) * hs_rt;
    (di, di_hk, di_rt)
}

/// Laminar kinetic-energy shape parameter `H*`: `HSL`.
pub(crate) fn hsl(hk: f64, _rt: f64, _msq: f64) -> (f64, f64, f64, f64) {
    let (hs, hs_hk) = if hk < 4.35 {
        let tmp = hk - 4.35;
        let hk1 = hk + 1.0;
        (
            0.0111 * tmp * tmp / hk1 - 0.0278 * tmp.powi(3) / hk1 + 1.528
                - 0.0002 * (tmp * hk).powi(2),
            0.0111 * (2.0 * tmp - tmp * tmp / hk1) / hk1
                - 0.0278 * (3.0 * tmp * tmp - tmp.powi(3) / hk1) / hk1
                - 0.0002 * 2.0 * tmp * hk * (tmp + hk),
        )
    } else {
        let hs2 = 0.015;
        (
            hs2 * (hk - 4.35).powi(2) / hk + 1.528,
            hs2 * 2.0 * (hk - 4.35) / hk - hs2 * (hk - 4.35).powi(2) / (hk * hk),
        )
    };
    (hs, hs_hk, 0.0, 0.0)
}

/// Laminar skin friction coefficient (Falkner–Skan): `CFL`.
pub(crate) fn cfl(hk: f64, rt: f64, _msq: f64) -> (f64, f64, f64, f64) {
    let (cf, cf_hk) = if hk < 5.5 {
        let tmp = (5.5 - hk).powi(3) / (hk + 1.0);
        (
            (0.0727 * tmp - 0.07) / rt,
            (-0.0727 * tmp * 3.0 / (5.5 - hk) - 0.0727 * tmp / (hk + 1.0)) / rt,
        )
    } else {
        let tmp = 1.0 - 1.0 / (hk - 4.5);
        (
            (0.015 * tmp * tmp - 0.07) / rt,
            (0.015 * tmp * 2.0 / (hk - 4.5).powi(2)) / rt,
        )
    };
    (cf, cf_hk, -cf / rt, 0.0)
}

/// Turbulent kinetic-energy shape parameter `H*`: `HST`.
pub(crate) fn hst(hk: f64, rt: f64, msq: f64) -> (f64, f64, f64, f64) {
    const HSMIN: f64 = 1.500;
    const DHSINF: f64 = 0.015;

    // Limited Re_theta dependence for Re_theta < 200.
    let (ho, ho_rt) = if rt > 400.0 {
        (3.0 + 400.0 / rt, -400.0 / (rt * rt))
    } else {
        (4.0, 0.0)
    };
    let (rtz, rtz_rt) = if rt > 200.0 { (rt, 1.0) } else { (200.0, 0.0) };

    let (hs, hs_hk, hs_rt) = if hk < ho {
        // Attached branch (arctan(y+) + Schlichting profiles).
        let hr = (ho - hk) / (ho - 1.0);
        let hr_hk = -1.0 / (ho - 1.0);
        let hr_rt = (1.0 - hr) / (ho - 1.0) * ho_rt;
        let a = 2.0 - HSMIN - 4.0 / rtz;
        (
            a * hr * hr * 1.5 / (hk + 0.5) + HSMIN + 4.0 / rtz,
            -a * hr * hr * 1.5 / (hk + 0.5).powi(2) + a * hr * 2.0 * 1.5 / (hk + 0.5) * hr_hk,
            a * hr * 2.0 * 1.5 / (hk + 0.5) * hr_rt
                + (hr * hr * 1.5 / (hk + 0.5) - 1.0) * 4.0 / (rtz * rtz) * rtz_rt,
        )
    } else {
        // Separated branch.
        let grt = rtz.ln();
        let hdif = hk - ho;
        let rtmp = hk - ho + 4.0 / grt;
        let htmp = 0.007 * grt / (rtmp * rtmp) + DHSINF / hk;
        let htmp_hk = -0.014 * grt / rtmp.powi(3) - DHSINF / (hk * hk);
        let htmp_rt = -0.014 * grt / rtmp.powi(3) * (-ho_rt - 4.0 / (grt * grt) / rtz * rtz_rt)
            + 0.007 / (rtmp * rtmp) / rtz * rtz_rt;
        (
            hdif * hdif * htmp + HSMIN + 4.0 / rtz,
            hdif * 2.0 * htmp + hdif * hdif * htmp_hk,
            hdif * hdif * htmp_rt - 4.0 / (rtz * rtz) * rtz_rt + hdif * 2.0 * htmp * (-ho_rt),
        )
    };

    // Whitfield's minor additional compressibility correction.
    let fm = 1.0 + 0.014 * msq;
    let hs = (hs + 0.028 * msq) / fm;
    (hs, hs_hk / fm, hs_rt / fm, 0.028 / fm - 0.014 * hs / fm)
}

/// Turbulent skin friction coefficient (Coles): `CFT`.
pub(crate) fn cft(hk: f64, rt: f64, msq: f64, cffac: f64) -> (f64, f64, f64, f64) {
    const GM1: f64 = 1.4 - 1.0;
    let fc = (1.0 + 0.5 * GM1 * msq).sqrt();
    let mut grt = (rt / fc).ln();
    let mut grt_rt = 1.0 / rt;
    let mut grt_msq = -0.25 * GM1 / (fc * fc);
    if grt < 3.0 {
        grt = 3.0;
        grt_rt = 0.0;
        grt_msq = 0.0;
    }

    let gex = -1.74 - 0.31 * hk;
    let arg = (-1.33 * hk).max(-20.0);
    let thk = (4.0 - hk / 0.875).tanh();

    let cfo = cffac * 0.3 * arg.exp() * (grt / 2.3026).powf(gex);
    let cf = (cfo + 1.1e-4 * (thk - 1.0)) / fc;
    let cf_hk =
        (-1.33 * cfo - 0.31 * (grt / 2.3026).ln() * cfo - 1.1e-4 * (1.0 - thk * thk) / 0.875) / fc;
    let cf_rt = gex * cfo / (fc * grt) * grt_rt;
    let cf_msq = gex * cfo / (fc * grt) * grt_msq - 0.25 * GM1 * cf / (fc * fc);
    (cf, cf_hk, cf_rt, cf_msq)
}

/// Density-thickness shape parameter `H**` (Whitfield): `HCT`.
pub(crate) fn hct(hk: f64, msq: f64) -> (f64, f64, f64) {
    (
        msq * (0.064 / (hk - 0.8) + 0.251),
        msq * (-0.064 / (hk - 0.8).powi(2)),
        0.064 / (hk - 0.8) + 0.251,
    )
}

/// Envelope eᴺ amplification rate `dN/dx` and its derivatives with respect
/// to `hk`, `th`, `rt` (Drela & Giles 1987): `DAMPL`.
pub(crate) fn dampl(hk: f64, th: f64, rt: f64) -> (f64, f64, f64, f64) {
    amplification(hk, th, rt, false)
}

/// Modified envelope method blending in the Orr–Sommerfeld maximum
/// amplification rate of separated profiles (`H > 3.5`): `DAMPL2`.
pub(crate) fn dampl2(hk: f64, th: f64, rt: f64) -> (f64, f64, f64, f64) {
    let (ax1, ax1_hk, ax1_th, ax1_rt) = amplification(hk, th, rt, true);
    const HK1: f64 = 3.5;
    const HK2: f64 = 4.0;
    if hk < HK1 {
        return (ax1, ax1_hk, ax1_th, ax1_rt);
    }

    // Blending fraction over HK1 < Hk < HK2.
    let hnorm = (hk - HK1) / (HK2 - HK1);
    let hn_hk = 1.0 / (HK2 - HK1);
    let (hfac, hf_hk) = if hnorm >= 1.0 {
        (1.0, 0.0)
    } else {
        (
            3.0 * hnorm * hnorm - 2.0 * hnorm.powi(3),
            (6.0 * hnorm - 6.0 * hnorm * hnorm) * hn_hk,
        )
    };

    // Modified amplification rate.
    let gr = rt.log10();
    let gr_rt = 1.0 / (2.3025851 * rt);
    let gr0 = 0.30 + 0.35 * (-0.15 * (hk - 5.0)).exp();
    let gr0_hk = -0.35 * (-0.15 * (hk - 5.0)).exp() * 0.15;
    let tnr = (1.2 * (gr - gr0)).tanh();
    let tnr_rt = (1.0 - tnr * tnr) * 1.2 * gr_rt;
    let tnr_hk = -(1.0 - tnr * tnr) * 1.2 * gr0_hk;

    let mut ax2 = (0.086 * tnr - 0.25 / (hk - 1.0).powf(1.5)) / th;
    let mut ax2_hk = (0.086 * tnr_hk + 1.5 * 0.25 / (hk - 1.0).powf(2.5)) / th;
    let mut ax2_rt = (0.086 * tnr_rt) / th;
    let mut ax2_th = -ax2 / th;
    if ax2 < 0.0 {
        ax2 = 0.0;
        ax2_hk = 0.0;
        ax2_rt = 0.0;
        ax2_th = 0.0;
    }

    (
        hfac * ax2 + (1.0 - hfac) * ax1,
        hfac * ax2_hk + (1.0 - hfac) * ax1_hk + hf_hk * (ax2 - ax1),
        hfac * ax2_th + (1.0 - hfac) * ax1_th,
        hfac * ax2_rt + (1.0 - hfac) * ax1_rt,
    )
}

/// Common part of `DAMPL` and `DAMPL2`; `modified` adds the `0.1·exp(−20/(H−1))`
/// term of `DAMPL2` to the `m(H)` correlation. Returns `(ax, ax_hk, ax_th, ax_rt)`.
fn amplification(hk: f64, th: f64, rt: f64, modified: bool) -> (f64, f64, f64, f64) {
    const DGR: f64 = 0.08;

    let hmi = 1.0 / (hk - 1.0);
    let hmi_hk = -hmi * hmi;

    // log10(critical Re_theta) - H correlation for Falkner-Skan profiles.
    let aa = 2.492 * hmi.powf(0.43);
    let aa_hk = (aa / hmi) * 0.43 * hmi_hk;
    let bb = (14.0 * hmi - 9.24).tanh();
    let bb_hk = (1.0 - bb * bb) * 14.0 * hmi_hk;
    let grcrit = aa + 0.7 * (bb + 1.0);
    let grc_hk = aa_hk + 0.7 * bb_hk;

    let gr = rt.log10();
    let gr_rt = 1.0 / (2.3025851 * rt);

    if gr < grcrit - DGR {
        // No amplification below the critical Re_theta.
        return (0.0, 0.0, 0.0, 0.0);
    }

    // Steep cubic ramp turning the amplification on smoothly.
    let rnorm = (gr - (grcrit - DGR)) / (2.0 * DGR);
    let rn_hk = -grc_hk / (2.0 * DGR);
    let rn_rt = gr_rt / (2.0 * DGR);
    let (rfac, rfac_hk, rfac_rt) = if rnorm >= 1.0 {
        (1.0, 0.0, 0.0)
    } else {
        let rfac = 3.0 * rnorm * rnorm - 2.0 * rnorm.powi(3);
        let rfac_rn = 6.0 * rnorm - 6.0 * rnorm * rnorm;
        (rfac, rfac_rn * rn_hk, rfac_rn * rn_rt)
    };

    // Amplification envelope slope dN/dRe_theta.
    let arg = 3.87 * hmi - 2.52;
    let arg_hk = 3.87 * hmi_hk;
    let ex = (-arg * arg).exp();
    let ex_hk = ex * (-2.0 * arg * arg_hk);
    let dadr = 0.028 * (hk - 1.0) - 0.0345 * ex;
    let dadr_hk = 0.028 - 0.0345 * ex_hk;

    // m(H) correlation: theta dRe_theta/dx.
    let (af, af_hmi) = if modified {
        let brg = -20.0 * hmi;
        (
            -0.05 + 2.7 * hmi - 5.5 * hmi * hmi + 3.0 * hmi.powi(3) + 0.1 * brg.exp(),
            2.7 - 11.0 * hmi + 9.0 * hmi * hmi - 2.0 * brg.exp(),
        )
    } else {
        (
            -0.05 + 2.7 * hmi - 5.5 * hmi * hmi + 3.0 * hmi.powi(3),
            2.7 - 11.0 * hmi + 9.0 * hmi * hmi,
        )
    };
    let af_hk = af_hmi * hmi_hk;

    let ax = (af * dadr / th) * rfac;
    let ax_hk = (af_hk * dadr / th + af * dadr_hk / th) * rfac + (af * dadr / th) * rfac_hk;
    let ax_th = -ax / th;
    let ax_rt = (af * dadr / th) * rfac_rt;
    (ax, ax_hk, ax_th, ax_rt)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Central difference of `f` at `x`.
    fn fd(f: impl Fn(f64) -> f64, x: f64) -> f64 {
        fd_step(f, x, 1e-6 * x.abs().max(1e-3))
    }

    fn fd_step(f: impl Fn(f64) -> f64, x: f64, h: f64) -> f64 {
        (f(x + h) - f(x - h)) / (2.0 * h)
    }

    /// Central difference with respect to M^2 (which may be 0).
    fn fd_msq(f: impl Fn(f64) -> f64, m: f64) -> f64 {
        fd_step(f, m, 1e-5)
    }

    fn check(name: &str, analytic: f64, numeric: f64) {
        let tol = 1e-6 * numeric.abs().max(analytic.abs()) + 1e-9;
        assert!(
            (analytic - numeric).abs() <= tol,
            "{name}: analytic {analytic:e} numeric {numeric:e}"
        );
    }

    // Shape parameters sampled on both sides of every branch switch of the
    // correlations (4, 4.35, 5.5, the H* attached/separated limit, ...).
    const HKS: [f64; 9] = [1.3, 1.8, 2.5, 3.2, 3.9, 4.2, 5.0, 6.5, 9.0];
    const RTS: [f64; 6] = [80.0, 150.0, 300.0, 1000.0, 5000.0, 2.0e4];
    const MSQS: [f64; 3] = [0.0, 0.1, 0.4];

    #[test]
    fn hkin_derivatives() {
        for &h in &[1.4, 2.6, 4.0] {
            for &m in &MSQS {
                let (_, hk_h, hk_m) = hkin(h, m);
                check("hk_h", hk_h, fd(|v| hkin(v, m).0, h));
                check("hk_msq", hk_m, fd_msq(|v| hkin(h, v).0, m));
            }
        }
        // Incompressible: Hk = H.
        assert_eq!(hkin(2.59, 0.0).0, 2.59);
    }

    #[test]
    fn laminar_correlation_derivatives() {
        for &hk in &HKS {
            for &rt in &RTS {
                let (_, a, b) = dil(hk, rt);
                check("dil_hk", a, fd(|v| dil(v, rt).0, hk));
                check("dil_rt", b, fd(|v| dil(hk, v).0, rt));
                // XFOIL's DILW has a sign error in dRCD/dHk: the first term
                // is -2.2(1-1/Hk)/Hk^3 instead of +2.2(1-1/Hk)/Hk^3. The
                // port keeps it (it only affects the Newton convergence in
                // laminar wakes); add the missing 2x term back to compare.
                let (_, a, b) = dilw(hk, rt);
                let (hs, ..) = hsl(hk, rt, 0.0);
                let missing = 2.0 * 2.0 * (1.10 * (1.0 - 1.0 / hk) * 2.0 / hk.powi(3)) / (hs * rt);
                check("dilw_hk", a + missing, fd(|v| dilw(v, rt).0, hk));
                check("dilw_rt", b, fd(|v| dilw(hk, v).0, rt));
                let (_, a, _, _) = hsl(hk, rt, 0.0);
                check("hsl_hk", a, fd(|v| hsl(v, rt, 0.0).0, hk));
                let (_, a, b, _) = cfl(hk, rt, 0.0);
                check("cfl_hk", a, fd(|v| cfl(v, rt, 0.0).0, hk));
                check("cfl_rt", b, fd(|v| cfl(hk, v, 0.0).0, rt));
            }
        }
    }

    #[test]
    fn turbulent_correlation_derivatives() {
        for &hk in &HKS {
            for &rt in &RTS {
                // Avoid the kinks of the Re_theta limiters at 200 and 400.
                for &m in &MSQS {
                    let (_, a, b, c) = hst(hk, rt, m);
                    check("hst_hk", a, fd(|v| hst(v, rt, m).0, hk));
                    check("hst_rt", b, fd(|v| hst(hk, v, m).0, rt));
                    check("hst_msq", c, fd_msq(|v| hst(hk, rt, v).0, m));
                    let (_, a, b, c) = cft(hk, rt, m, 1.0);
                    check("cft_hk", a, fd(|v| cft(v, rt, m, 1.0).0, hk));
                    if (rt / (1.0 + 0.2 * m).sqrt()).ln() > 3.0 {
                        check("cft_rt", b, fd(|v| cft(hk, v, m, 1.0).0, rt));
                        check("cft_msq", c, fd_msq(|v| cft(hk, rt, v, 1.0).0, m));
                    }
                    let (_, a, b) = hct(hk, m);
                    check("hct_hk", a, fd(|v| hct(v, m).0, hk));
                    check("hct_msq", b, fd_msq(|v| hct(hk, v).0, m));
                }
            }
        }
    }

    #[test]
    fn amplification_derivatives() {
        let th = 2.0e-4;
        for &hk in &[2.2, 2.6, 3.0, 3.6, 3.8, 4.5, 6.0] {
            for &rt in &[100.0, 250.0, 400.0, 900.0, 3000.0] {
                for f in [dampl, dampl2] {
                    let (ax, a, b, c) = f(hk, th, rt);
                    if ax == 0.0 {
                        continue;
                    }
                    check("ax_hk", a, fd(|v| f(v, th, rt).0, hk));
                    check("ax_th", b, fd(|v| f(hk, v, rt).0, th));
                    check("ax_rt", c, fd(|v| f(hk, th, v).0, rt));
                }
            }
        }
    }

    #[test]
    fn known_values() {
        // Blasius flat plate (Hk = 2.59): Cf Re_theta = 0.441, H* = 1.573.
        let (cf, ..) = cfl(2.59, 1000.0, 0.0);
        assert!((cf * 1000.0 - 0.441).abs() < 0.015, "{cf}");
        let (hs, ..) = hsl(2.59, 1000.0, 0.0);
        assert!((hs - 1.573).abs() < 5e-3, "{hs}");
        // Laminar profiles are stable at low Re_theta (Blasius:
        // Re_theta,crit ~ 200).
        assert_eq!(dampl(2.59, 1e-3, 150.0).0, 0.0);
        assert!(dampl(2.59, 1e-3, 400.0).0 > 0.0);
        // Turbulent flat plate: Cf ~ 0.0036 at Re_theta = 5000, H = 1.4.
        let (cf, ..) = cft(1.4, 5000.0, 0.0, 1.0);
        assert!((cf - 0.0030).abs() < 6e-4, "{cf}");
        assert_eq!(hct(2.0, 0.0).0, 0.0);
        let p = BlParams::default();
        assert!((p.ctcon - 0.5 / (6.7 * 6.7 * 0.75)).abs() < 1e-15);
    }
}
