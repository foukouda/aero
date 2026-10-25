//! Boundary-layer station variables and the Newton system of one interval
//! (XFOIL `xblsys.f`: `BLPRV`, `BLKIN`, `BLVAR`, `BLMID`, `AXSET`,
//! `TRCHEK2`, `BLDIF`, `TRDIF`, `BLSYS`, `TESYS`; `xsolve.f`: `GAUSS`).
//!
//! XFOIL keeps the variables of the two stations of the current interval in
//! the common blocks `V_VAR1` and `V_VAR2` (`XBL.INC`) and copies them as
//! whole blocks (`COM1 = COM2`). [`Station`] is that block and [`Bl`] holds
//! everything `XBL.INC` declares, so the routines below read and write the
//! same quantities in the same order as the Fortran.
//!
//! Primary variables of a station: arc length `x` (ξ), amplification
//! `ampl` (N, laminar) or shear stress coefficient `s` (√Cτ, turbulent),
//! momentum thickness `t` (θ), displacement thickness `d` (δ*, without the
//! wake gap `dw`) and compressible edge velocity `u`. Derivatives are named
//! `<quantity>_<variable>`: `_u`, `_t`, `_d`, `_s` refer to the station's
//! own primaries, `_ms` to the freestream Mach number squared and `_re` to
//! the Reynolds number.

use super::closure::{BlParams, cfl, cft, dampl, dampl2, dil, dilw, hct, hkin, hsl, hst};

/// Variables of one station and their sensitivities (XFOIL `COM1`/`COM2`,
/// 73 values).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Station {
    pub x: f64,
    pub u: f64,
    pub t: f64,
    pub d: f64,
    pub s: f64,
    pub ampl: f64,
    pub u_uei: f64,
    pub u_ms: f64,
    pub dw: f64,
    pub h: f64,
    pub h_t: f64,
    pub h_d: f64,
    pub m: f64,
    pub m_u: f64,
    pub m_ms: f64,
    pub r: f64,
    pub r_u: f64,
    pub r_ms: f64,
    pub v: f64,
    pub v_u: f64,
    pub v_ms: f64,
    pub v_re: f64,
    pub hk: f64,
    pub hk_u: f64,
    pub hk_t: f64,
    pub hk_d: f64,
    pub hk_ms: f64,
    pub hs: f64,
    pub hs_u: f64,
    pub hs_t: f64,
    pub hs_d: f64,
    pub hs_ms: f64,
    pub hs_re: f64,
    pub hc: f64,
    pub hc_u: f64,
    pub hc_t: f64,
    pub hc_d: f64,
    pub hc_ms: f64,
    pub rt: f64,
    pub rt_u: f64,
    pub rt_t: f64,
    pub rt_ms: f64,
    pub rt_re: f64,
    pub cf: f64,
    pub cf_u: f64,
    pub cf_t: f64,
    pub cf_d: f64,
    pub cf_ms: f64,
    pub cf_re: f64,
    pub di: f64,
    pub di_u: f64,
    pub di_t: f64,
    pub di_d: f64,
    pub di_s: f64,
    pub di_ms: f64,
    pub di_re: f64,
    pub us: f64,
    pub us_u: f64,
    pub us_t: f64,
    pub us_d: f64,
    pub us_ms: f64,
    pub us_re: f64,
    pub cq: f64,
    pub cq_u: f64,
    pub cq_t: f64,
    pub cq_d: f64,
    pub cq_ms: f64,
    pub cq_re: f64,
    pub de: f64,
    pub de_u: f64,
    pub de_t: f64,
    pub de_d: f64,
    pub de_ms: f64,
}

/// Freestream quantities of the boundary-layer equations (XFOIL `V_VAR`,
/// set by `SETBL`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Flow {
    pub qinf: f64,
    /// Kármán–Tsien parameter `TKBL` and its derivative wrt `M∞²`.
    pub tk: f64,
    pub tk_ms: f64,
    /// Stagnation density.
    pub rst: f64,
    pub rst_ms: f64,
    /// Inverse stagnation enthalpy.
    pub hstinv: f64,
    pub hstinv_ms: f64,
    /// Reynolds number based on freestream density, velocity, viscosity.
    pub reybl: f64,
    pub reybl_ms: f64,
    pub reybl_re: f64,
    pub gm1: f64,
    /// Sutherland viscosity-law constant. XFOIL's solver never assigns it
    /// (only plotting code uses 0.35), so it is 0 in the actual analyses.
    pub hvrat: f64,
}

impl Flow {
    /// `SETBL` with `COMSET`, for a freestream speed `qinf`, Mach number
    /// `minf` and Reynolds number `reinf`.
    pub fn new(qinf: f64, minf: f64, reinf: f64) -> Self {
        let gamma = 1.4;
        let gm1 = gamma - 1.0;
        let msq = minf * minf;
        let beta = (1.0 - msq).sqrt();
        let beta_msq = -0.5 / beta;
        let tk = msq / ((1.0 + beta) * (1.0 + beta));
        let tk_ms = 1.0 / ((1.0 + beta) * (1.0 + beta)) - 2.0 * tk / (1.0 + beta) * beta_msq;

        let den = 1.0 + 0.5 * gm1 * msq;
        let rst = den.powf(1.0 / gm1);
        let rst_ms = 0.5 * rst / den;
        let hstinv = gm1 * (minf / qinf).powi(2) / den;
        let hstinv_ms = gm1 * (1.0 / qinf).powi(2) / den - 0.5 * gm1 * hstinv / den;

        let hvrat = 0.0;
        let herat = 1.0 - 0.5 * qinf * qinf * hstinv;
        let herat_ms = -0.5 * qinf * qinf * hstinv_ms;
        let reybl_re = herat.powi(3).sqrt() * (1.0 + hvrat) / (herat + hvrat);
        let reybl = reinf * reybl_re;
        let reybl_ms = reybl * (1.5 / herat - 1.0 / (herat + hvrat)) * herat_ms;
        Self {
            qinf,
            tk,
            tk_ms,
            rst,
            rst_ms,
            hstinv,
            hstinv_ms,
            reybl,
            reybl_ms,
            reybl_re,
            gm1,
            hvrat,
        }
    }

    /// Edge Mach number squared for an incompressible edge speed `uei`.
    pub fn edge_msq(&self, uei: f64) -> f64 {
        uei * uei * self.hstinv / (self.gm1 * (1.0 - 0.5 * uei * uei * self.hstinv))
    }
}

/// Midpoint skin friction of the interval (XFOIL `V_VARA`, set by `BLMID`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Midpoint {
    pub cfm: f64,
    pub cfm_ms: f64,
    pub cfm_re: f64,
    pub cfm_u1: f64,
    pub cfm_t1: f64,
    pub cfm_d1: f64,
    pub cfm_u2: f64,
    pub cfm_t2: f64,
    pub cfm_d2: f64,
}

/// Transition location in the interval and its sensitivities (XFOIL
/// `V_VARA`, set by `TRCHEK2`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct TransitionPoint {
    pub xt: f64,
    pub xt_a1: f64,
    pub xt_ms: f64,
    pub xt_re: f64,
    pub xt_xf: f64,
    pub xt_x1: f64,
    pub xt_t1: f64,
    pub xt_d1: f64,
    pub xt_u1: f64,
    pub xt_x2: f64,
    pub xt_t2: f64,
    pub xt_d2: f64,
    pub xt_u2: f64,
}

/// Newton system of one interval (XFOIL `V_SYS`):
///
/// ```text
/// VS1·[dA1 dT1 dD1 dU1 dX1]ᵀ + VS2·[dA2 dT2 dD2 dU2 dX2]ᵀ = VSREZ
/// ```
///
/// Rows: amplification or shear-lag, momentum, shape parameter equations
/// (the fourth row is filled by the marching routines). `dA` is replaced by
/// `dS` at turbulent stations. `vsm`, `vsr`, `vsx` are the derivatives with
/// respect to `M∞²`, the Reynolds number and the trip location.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct System {
    pub vs1: [[f64; 5]; 4],
    pub vs2: [[f64; 5]; 4],
    pub vsrez: [f64; 4],
    pub vsr: [f64; 4],
    pub vsm: [f64; 4],
    pub vsx: [f64; 4],
}

/// Closure set of a station or interval (XFOIL's `ITYP` argument).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Regime {
    /// `ITYP = 0`: similarity station at the stagnation point (`BLDIF`
    /// only).
    Similarity,
    /// `ITYP = 1`.
    Laminar,
    /// `ITYP = 2`.
    Turbulent,
    /// `ITYP = 3`.
    Wake,
}

/// Amplification rate averaged over an interval (`AXSET`) and its
/// sensitivities.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Ax {
    pub ax: f64,
    pub hk1: f64,
    pub t1: f64,
    pub rt1: f64,
    pub a1: f64,
    pub hk2: f64,
    pub t2: f64,
    pub rt2: f64,
    pub a2: f64,
}

/// XFOIL `AXSET`: rms average of the amplification rates at both ends of
/// the interval, plus a small term ensuring `dN/dx > 0` near `N = Ncrit`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn axset(
    hk1: f64,
    t1: f64,
    rt1: f64,
    a1: f64,
    hk2: f64,
    t2: f64,
    rt2: f64,
    a2: f64,
    acrit: f64,
    damp2: bool,
) -> Ax {
    let rate = if damp2 { dampl2 } else { dampl };
    let (ax1, ax1_hk1, ax1_t1, ax1_rt1) = rate(hk1, t1, rt1);
    let (ax2, ax2_hk2, ax2_t2, ax2_rt2) = rate(hk2, t2, rt2);

    let axsq = 0.5 * (ax1 * ax1 + ax2 * ax2);
    let (axa, axa_ax1, axa_ax2) = if axsq <= 0.0 {
        (0.0, 0.0, 0.0)
    } else {
        let axa = axsq.sqrt();
        (axa, 0.5 * ax1 / axa, 0.5 * ax2 / axa)
    };

    let arg = (20.0 * (acrit - 0.5 * (a1 + a2))).min(20.0);
    let (exn, exn_a1, exn_a2) = if arg <= 0.0 {
        (1.0, 0.0, 0.0)
    } else {
        let exn = (-arg).exp();
        (exn, 20.0 * 0.5 * exn, 20.0 * 0.5 * exn)
    };
    let dax = exn * 0.002 / (t1 + t2);
    let dax_a1 = exn_a1 * 0.002 / (t1 + t2);
    let dax_a2 = exn_a2 * 0.002 / (t1 + t2);
    let dax_t1 = -dax / (t1 + t2);
    let dax_t2 = -dax / (t1 + t2);

    Ax {
        ax: axa + dax,
        hk1: axa_ax1 * ax1_hk1,
        t1: axa_ax1 * ax1_t1 + dax_t1,
        rt1: axa_ax1 * ax1_rt1,
        a1: dax_a1,
        hk2: axa_ax2 * ax2_hk2,
        t2: axa_ax2 * ax2_t2 + dax_t2,
        rt2: axa_ax2 * ax2_rt2,
        a2: dax_a2,
    }
}

/// XFOIL `GAUSS` for the 4×4 system formed by the first four columns of
/// `z`; `z` is destroyed and `r` replaced by the solution.
#[allow(clippy::needless_range_loop)]
pub(crate) fn gauss4(z: &mut [[f64; 5]; 4], r: &mut [f64; 4]) {
    const NN: usize = 4;
    for np in 0..NN - 1 {
        let np1 = np + 1;
        // Max pivot index.
        let mut nx = np;
        for n in np1..NN {
            if z[n][np].abs() > z[nx][np].abs() {
                nx = n;
            }
        }
        let pivot = 1.0 / z[nx][np];
        // Switch pivots.
        z[nx][np] = z[np][np];
        // Switch rows and normalize the pivot row.
        for l in np1..NN {
            let temp = z[nx][l] * pivot;
            z[nx][l] = z[np][l];
            z[np][l] = temp;
        }
        let temp = r[nx] * pivot;
        r[nx] = r[np];
        r[np] = temp;
        // Forward elimination.
        for k in np1..NN {
            let ztmp = z[k][np];
            for l in np1..NN {
                z[k][l] -= ztmp * z[np][l];
            }
            r[k] -= ztmp * r[np];
        }
    }
    r[NN - 1] /= z[NN - 1][NN - 1];
    for np in (0..NN - 1).rev() {
        for k in np + 1..NN {
            r[np] -= z[np][k] * r[k];
        }
    }
}

/// Everything XFOIL's `XBL.INC` declares: the two stations of the current
/// interval, the interval flags, the freestream quantities, the transition
/// point and the interval Newton system.
#[derive(Debug, Clone, Default)]
pub(crate) struct Bl {
    pub s1: Station,
    pub s2: Station,
    /// Similarity (stagnation point) station.
    pub simi: bool,
    /// Transition occurs in the interval.
    pub tran: bool,
    /// Turbulent interval.
    pub turb: bool,
    /// Wake interval.
    pub wake: bool,
    /// Transition is forced by a trip.
    pub trforc: bool,
    /// Free transition (N reaches Ncrit).
    pub trfree: bool,
    pub flow: Flow,
    /// Similarity pressure-gradient parameter `x/u du/dx` (`BULE`).
    pub bule: f64,
    /// Arc length of the trip on the current side (`XIFORC`).
    pub xiforc: f64,
    /// Critical amplification ratio of the current side (`AMCRIT`).
    pub amcrit: f64,
    /// Use `DAMPL2` instead of `DAMPL` (`IDAMPV`).
    pub damp2: bool,
    pub par: BlParams,
    pub mid: Midpoint,
    pub tr: TransitionPoint,
    pub sys: System,
}

impl Bl {
    /// New state for the given freestream quantities and closure constants.
    pub fn new(flow: Flow, par: BlParams) -> Self {
        Self {
            flow,
            par,
            ..Self::default()
        }
    }

    /// XFOIL `BLPRV`: sets the primary variables of station 2 from the
    /// incompressible edge velocity `uei`.
    #[allow(clippy::too_many_arguments)]
    pub fn blprv(
        &mut self,
        xsi: f64,
        ami: f64,
        cti: f64,
        thi: f64,
        dsi: f64,
        dswaki: f64,
        uei: f64,
    ) {
        let f = self.flow;
        let s2 = &mut self.s2;
        s2.x = xsi;
        s2.ampl = ami;
        s2.s = cti;
        s2.t = thi;
        s2.d = dsi - dswaki;
        s2.dw = dswaki;

        let den = 1.0 - f.tk * (uei / f.qinf).powi(2);
        s2.u = uei * (1.0 - f.tk) / den;
        s2.u_uei = (1.0 + f.tk * (2.0 * s2.u * uei / (f.qinf * f.qinf) - 1.0)) / den;
        s2.u_ms = (s2.u * (uei / f.qinf).powi(2) - uei) * f.tk_ms / den;
    }

    /// XFOIL `BLKIN`: turbulence-independent secondary variables of
    /// station 2.
    pub fn blkin(&mut self) {
        let f = self.flow;
        let s2 = &mut self.s2;
        let u2sq = s2.u * s2.u;

        // Edge Mach number squared.
        let den = f.gm1 * (1.0 - 0.5 * u2sq * f.hstinv);
        s2.m = u2sq * f.hstinv / den;
        let tr2 = 1.0 + 0.5 * f.gm1 * s2.m;
        s2.m_u = 2.0 * s2.m * tr2 / s2.u;
        s2.m_ms = u2sq * tr2 / den * f.hstinv_ms;

        // Edge static density (isentropic).
        s2.r = f.rst * tr2.powf(-1.0 / f.gm1);
        s2.r_u = -s2.r / tr2 * 0.5 * s2.m_u;
        s2.r_ms = -s2.r / tr2 * 0.5 * s2.m_ms + f.rst_ms * tr2.powf(-1.0 / f.gm1);

        // Shape parameter.
        s2.h = s2.d / s2.t;
        s2.h_d = 1.0 / s2.t;
        s2.h_t = -s2.h / s2.t;

        // Edge static/stagnation enthalpy.
        let herat = 1.0 - 0.5 * u2sq * f.hstinv;
        let he_u2 = -s2.u * f.hstinv;
        let he_ms = -0.5 * u2sq * f.hstinv_ms;

        // Molecular viscosity.
        s2.v = herat.powi(3).sqrt() * (1.0 + f.hvrat) / (herat + f.hvrat) / f.reybl;
        let v2_he = s2.v * (1.5 / herat - 1.0 / (herat + f.hvrat));
        s2.v_u = v2_he * he_u2;
        s2.v_ms = -s2.v / f.reybl * f.reybl_ms + v2_he * he_ms;
        s2.v_re = -s2.v / f.reybl * f.reybl_re;

        // Kinematic shape parameter.
        let (hk, hk_h, hk_m) = hkin(s2.h, s2.m);
        s2.hk = hk;
        s2.hk_u = hk_m * s2.m_u;
        s2.hk_t = hk_h * s2.h_t;
        s2.hk_d = hk_h * s2.h_d;
        s2.hk_ms = hk_m * s2.m_ms;

        // Momentum-thickness Reynolds number.
        s2.rt = s2.r * s2.u * s2.t / s2.v;
        s2.rt_u = s2.rt * (1.0 / s2.u + s2.r_u / s2.r - s2.v_u / s2.v);
        s2.rt_t = s2.rt / s2.t;
        s2.rt_ms = s2.rt * (s2.r_ms / s2.r - s2.v_ms / s2.v);
        s2.rt_re = s2.rt * (-s2.v_re / s2.v);
    }

    /// XFOIL `BLVAR`: secondary variables of station 2 for the given
    /// closure set (laminar, turbulent or wake).
    pub fn blvar(&mut self, ityp: Regime) {
        // XFOIL never calls BLVAR for the similarity station: laminar.
        let ityp = if ityp == Regime::Similarity {
            Regime::Laminar
        } else {
            ityp
        };
        let par = self.par;
        let s2 = &mut self.s2;

        if ityp == Regime::Wake {
            s2.hk = s2.hk.max(1.00005);
        } else {
            s2.hk = s2.hk.max(1.05000);
        }

        // Density-thickness shape parameter H**.
        let (hc, hc_hk, hc_m) = hct(s2.hk, s2.m);
        s2.hc = hc;
        s2.hc_u = hc_hk * s2.hk_u + hc_m * s2.m_u;
        s2.hc_t = hc_hk * s2.hk_t;
        s2.hc_d = hc_hk * s2.hk_d;
        s2.hc_ms = hc_hk * s2.hk_ms + hc_m * s2.m_ms;

        // Kinetic-energy shape parameter H*.
        let (hs, hs_hk, hs_rt, hs_m) = if ityp == Regime::Laminar {
            hsl(s2.hk, s2.rt, s2.m)
        } else {
            hst(s2.hk, s2.rt, s2.m)
        };
        s2.hs = hs;
        s2.hs_u = hs_hk * s2.hk_u + hs_rt * s2.rt_u + hs_m * s2.m_u;
        s2.hs_t = hs_hk * s2.hk_t + hs_rt * s2.rt_t;
        s2.hs_d = hs_hk * s2.hk_d;
        s2.hs_ms = hs_hk * s2.hk_ms + hs_rt * s2.rt_ms + hs_m * s2.m_ms;
        s2.hs_re = hs_rt * s2.rt_re;

        // Normalized slip velocity Us.
        s2.us = 0.5 * s2.hs * (1.0 - (s2.hk - 1.0) / (par.gbcon * s2.h));
        let us_hs = 0.5 * (1.0 - (s2.hk - 1.0) / (par.gbcon * s2.h));
        let us_hk = 0.5 * s2.hs * (-1.0 / (par.gbcon * s2.h));
        let us_h = 0.5 * s2.hs * (s2.hk - 1.0) / (par.gbcon * s2.h * s2.h);
        s2.us_u = us_hs * s2.hs_u + us_hk * s2.hk_u;
        s2.us_t = us_hs * s2.hs_t + us_hk * s2.hk_t + us_h * s2.h_t;
        s2.us_d = us_hs * s2.hs_d + us_hk * s2.hk_d + us_h * s2.h_d;
        s2.us_ms = us_hs * s2.hs_ms + us_hk * s2.hk_ms;
        s2.us_re = us_hs * s2.hs_re;

        if ityp != Regime::Wake && s2.us > 0.95 {
            s2.us = 0.98;
            s2.us_u = 0.0;
            s2.us_t = 0.0;
            s2.us_d = 0.0;
            s2.us_ms = 0.0;
            s2.us_re = 0.0;
        }
        if ityp == Regime::Wake && s2.us > 0.99995 {
            s2.us = 0.99995;
            s2.us_u = 0.0;
            s2.us_t = 0.0;
            s2.us_d = 0.0;
            s2.us_ms = 0.0;
            s2.us_re = 0.0;
        }

        // Equilibrium wake-layer shear coefficient (Ctau)eq^1/2.
        let mut hkc = s2.hk - 1.0;
        let mut hkc_hk2 = 1.0;
        let mut hkc_rt2 = 0.0;
        if ityp == Regime::Turbulent {
            let gcc = par.gccon;
            hkc = s2.hk - 1.0 - gcc / s2.rt;
            hkc_hk2 = 1.0;
            hkc_rt2 = gcc / (s2.rt * s2.rt);
            if hkc < 0.01 {
                hkc = 0.01;
                hkc_hk2 = 0.0;
                hkc_rt2 = 0.0;
            }
        }

        let hkb = s2.hk - 1.0;
        let usb = 1.0 - s2.us;
        let ctc = par.ctcon;
        let den = usb * s2.h * s2.hk * s2.hk;
        s2.cq = (ctc * s2.hs * hkb * hkc * hkc / den).sqrt();
        let cq_hs = ctc * hkb * hkc * hkc / den * 0.5 / s2.cq;
        let cq_us = ctc * s2.hs * hkb * hkc * hkc / den / usb * 0.5 / s2.cq;
        let cq_hk = ctc * s2.hs * hkc * hkc / den * 0.5 / s2.cq
            - ctc * s2.hs * hkb * hkc * hkc / (usb * s2.h * s2.hk.powi(3)) * 2.0 * 0.5 / s2.cq
            + ctc * s2.hs * hkb * hkc / den * 2.0 * 0.5 / s2.cq * hkc_hk2;
        let cq_rt = ctc * s2.hs * hkb * hkc / den * 2.0 * 0.5 / s2.cq * hkc_rt2;
        let cq_h = -ctc * s2.hs * hkb * hkc * hkc / den / s2.h * 0.5 / s2.cq;

        s2.cq_u = cq_hs * s2.hs_u + cq_us * s2.us_u + cq_hk * s2.hk_u;
        s2.cq_t = cq_hs * s2.hs_t + cq_us * s2.us_t + cq_hk * s2.hk_t;
        s2.cq_d = cq_hs * s2.hs_d + cq_us * s2.us_d + cq_hk * s2.hk_d;
        s2.cq_ms = cq_hs * s2.hs_ms + cq_us * s2.us_ms + cq_hk * s2.hk_ms;
        s2.cq_re = cq_hs * s2.hs_re + cq_us * s2.us_re;

        s2.cq_u += cq_rt * s2.rt_u;
        s2.cq_t += cq_h * s2.h_t + cq_rt * s2.rt_t;
        s2.cq_d += cq_h * s2.h_d;
        s2.cq_ms += cq_rt * s2.rt_ms;
        s2.cq_re += cq_rt * s2.rt_re;

        // Skin friction coefficient.
        let (cf, cf_hk, cf_rt, cf_m) = match ityp {
            Regime::Wake => (0.0, 0.0, 0.0, 0.0),
            Regime::Laminar | Regime::Similarity => cfl(s2.hk, s2.rt, s2.m),
            Regime::Turbulent => {
                let turb = cft(s2.hk, s2.rt, s2.m, par.cffac);
                let lam = cfl(s2.hk, s2.rt, s2.m);
                // Laminar Cf greater than turbulent Cf (unreasonably small
                // Re_theta only): use the laminar value.
                if lam.0 > turb.0 { lam } else { turb }
            }
        };
        s2.cf = cf;
        s2.cf_u = cf_hk * s2.hk_u + cf_rt * s2.rt_u + cf_m * s2.m_u;
        s2.cf_t = cf_hk * s2.hk_t + cf_rt * s2.rt_t;
        s2.cf_d = cf_hk * s2.hk_d;
        s2.cf_ms = cf_hk * s2.hk_ms + cf_rt * s2.rt_ms + cf_m * s2.m_ms;
        s2.cf_re = cf_rt * s2.rt_re;

        // Dissipation function 2 CD / H*.
        if ityp == Regime::Laminar || ityp == Regime::Similarity {
            let (di, di_hk, di_rt) = dil(s2.hk, s2.rt);
            s2.di = di;
            s2.di_u = di_hk * s2.hk_u + di_rt * s2.rt_u;
            s2.di_t = di_hk * s2.hk_t + di_rt * s2.rt_t;
            s2.di_d = di_hk * s2.hk_d;
            s2.di_s = 0.0;
            s2.di_ms = di_hk * s2.hk_ms + di_rt * s2.rt_ms;
            s2.di_re = di_rt * s2.rt_re;
        } else if ityp == Regime::Turbulent {
            // Turbulent wall contribution.
            let (cf2t, cf2t_hk, cf2t_rt, cf2t_m) = cft(s2.hk, s2.rt, s2.m, par.cffac);
            let cf2t_u = cf2t_hk * s2.hk_u + cf2t_rt * s2.rt_u + cf2t_m * s2.m_u;
            let cf2t_t = cf2t_hk * s2.hk_t + cf2t_rt * s2.rt_t;
            let cf2t_d = cf2t_hk * s2.hk_d;
            let cf2t_ms = cf2t_hk * s2.hk_ms + cf2t_rt * s2.rt_ms + cf2t_m * s2.m_ms;
            let cf2t_re = cf2t_rt * s2.rt_re;

            s2.di = (0.5 * cf2t * s2.us) * 2.0 / s2.hs;
            let di_hs = -(0.5 * cf2t * s2.us) * 2.0 / (s2.hs * s2.hs);
            let di_us = (0.5 * cf2t) * 2.0 / s2.hs;
            let di_cf2t = (0.5 * s2.us) * 2.0 / s2.hs;

            s2.di_s = 0.0;
            s2.di_u = di_hs * s2.hs_u + di_us * s2.us_u + di_cf2t * cf2t_u;
            s2.di_t = di_hs * s2.hs_t + di_us * s2.us_t + di_cf2t * cf2t_t;
            s2.di_d = di_hs * s2.hs_d + di_us * s2.us_d + di_cf2t * cf2t_d;
            s2.di_ms = di_hs * s2.hs_ms + di_us * s2.us_ms + di_cf2t * cf2t_ms;
            s2.di_re = di_hs * s2.hs_re + di_us * s2.us_re + di_cf2t * cf2t_re;

            // Minimum Hk for the wake layer to still exist.
            let grt = s2.rt.ln();
            let hmin = 1.0 + 2.1 / grt;
            let hm_rt2 = -(2.1 / (grt * grt)) / s2.rt;

            // Correction of the wall dissipation for very low Hk.
            let fl = (s2.hk - 1.0) / (hmin - 1.0);
            let fl_hk2 = 1.0 / (hmin - 1.0);
            let fl_rt2 = (-fl / (hmin - 1.0)) * hm_rt2;
            let tfl = fl.tanh();
            let dfac = 0.5 + 0.5 * tfl;
            let df_fl = 0.5 * (1.0 - tfl * tfl);
            let df_hk2 = df_fl * fl_hk2;
            let df_rt2 = df_fl * fl_rt2;

            s2.di_s *= dfac;
            s2.di_u = s2.di_u * dfac + s2.di * (df_hk2 * s2.hk_u + df_rt2 * s2.rt_u);
            s2.di_t = s2.di_t * dfac + s2.di * (df_hk2 * s2.hk_t + df_rt2 * s2.rt_t);
            s2.di_d = s2.di_d * dfac + s2.di * (df_hk2 * s2.hk_d);
            s2.di_ms = s2.di_ms * dfac + s2.di * (df_hk2 * s2.hk_ms + df_rt2 * s2.rt_ms);
            s2.di_re = s2.di_re * dfac + s2.di * (df_rt2 * s2.rt_re);
            s2.di *= dfac;
        } else {
            // No wall contribution in the wake.
            s2.di = 0.0;
            s2.di_s = 0.0;
            s2.di_u = 0.0;
            s2.di_t = 0.0;
            s2.di_d = 0.0;
            s2.di_ms = 0.0;
            s2.di_re = 0.0;
        }

        // Turbulent outer-layer contribution.
        if ityp == Regime::Turbulent || ityp == Regime::Wake {
            let dd = s2.s * s2.s * (0.995 - s2.us) * 2.0 / s2.hs;
            let dd_hs = -s2.s * s2.s * (0.995 - s2.us) * 2.0 / (s2.hs * s2.hs);
            let dd_us = -s2.s * s2.s * 2.0 / s2.hs;
            let dd_s2 = s2.s * 2.0 * (0.995 - s2.us) * 2.0 / s2.hs;

            s2.di += dd;
            s2.di_s = dd_s2;
            s2.di_u += dd_hs * s2.hs_u + dd_us * s2.us_u;
            s2.di_t += dd_hs * s2.hs_t + dd_us * s2.us_t;
            s2.di_d += dd_hs * s2.hs_d + dd_us * s2.us_d;
            s2.di_ms += dd_hs * s2.hs_ms + dd_us * s2.us_ms;
            s2.di_re += dd_hs * s2.hs_re + dd_us * s2.us_re;

            // Laminar stress contribution to the outer-layer CD.
            let dd = 0.15 * (0.995 - s2.us).powi(2) / s2.rt * 2.0 / s2.hs;
            let dd_us = -0.15 * (0.995 - s2.us) * 2.0 / s2.rt * 2.0 / s2.hs;
            let dd_hs = -dd / s2.hs;
            let dd_rt = -dd / s2.rt;

            s2.di += dd;
            s2.di_u += dd_hs * s2.hs_u + dd_us * s2.us_u + dd_rt * s2.rt_u;
            s2.di_t += dd_hs * s2.hs_t + dd_us * s2.us_t + dd_rt * s2.rt_t;
            s2.di_d += dd_hs * s2.hs_d + dd_us * s2.us_d;
            s2.di_ms += dd_hs * s2.hs_ms + dd_us * s2.us_ms + dd_rt * s2.rt_ms;
            s2.di_re += dd_hs * s2.hs_re + dd_us * s2.us_re + dd_rt * s2.rt_re;
        }

        if ityp == Regime::Turbulent {
            let (di2l, di2l_hk, di2l_rt) = dil(s2.hk, s2.rt);
            if di2l > s2.di {
                // Laminar CD greater than turbulent CD: use the laminar one.
                s2.di = di2l;
                s2.di_s = 0.0;
                s2.di_u = di2l_hk * s2.hk_u + di2l_rt * s2.rt_u;
                s2.di_t = di2l_hk * s2.hk_t + di2l_rt * s2.rt_t;
                s2.di_d = di2l_hk * s2.hk_d;
                s2.di_ms = di2l_hk * s2.hk_ms + di2l_rt * s2.rt_ms;
                s2.di_re = di2l_rt * s2.rt_re;
            }
        }

        if ityp == Regime::Wake {
            // Laminar wake CD.
            let (di2l, di2l_hk, di2l_rt) = dilw(s2.hk, s2.rt);
            if di2l > s2.di {
                s2.di = di2l;
                s2.di_s = 0.0;
                s2.di_u = di2l_hk * s2.hk_u + di2l_rt * s2.rt_u;
                s2.di_t = di2l_hk * s2.hk_t + di2l_rt * s2.rt_t;
                s2.di_d = di2l_hk * s2.hk_d;
                s2.di_ms = di2l_hk * s2.hk_ms + di2l_rt * s2.rt_ms;
                s2.di_re = di2l_rt * s2.rt_re;
            }
            // Double dissipation for the two wake halves.
            s2.di *= 2.0;
            s2.di_s *= 2.0;
            s2.di_u *= 2.0;
            s2.di_t *= 2.0;
            s2.di_d *= 2.0;
            s2.di_ms *= 2.0;
            s2.di_re *= 2.0;
        }

        // BL thickness Delta from the simplified Green's correlation.
        s2.de = (3.15 + 1.72 / (s2.hk - 1.0)) * s2.t + s2.d;
        let de_hk = (-1.72 / (s2.hk - 1.0).powi(2)) * s2.t;
        s2.de_u = de_hk * s2.hk_u;
        s2.de_t = de_hk * s2.hk_t + (3.15 + 1.72 / (s2.hk - 1.0));
        s2.de_d = de_hk * s2.hk_d + 1.0;
        s2.de_ms = de_hk * s2.hk_ms;

        const HDMAX: f64 = 12.0;
        if s2.de > HDMAX * s2.t {
            s2.de = HDMAX * s2.t;
            s2.de_u = 0.0;
            s2.de_t = HDMAX;
            s2.de_d = 0.0;
            s2.de_ms = 0.0;
        }
    }

    /// XFOIL `BLMID`: midpoint skin friction of the interval.
    pub fn blmid(&mut self, ityp: Regime) {
        if self.simi {
            let s2 = self.s2;
            let s1 = &mut self.s1;
            s1.hk = s2.hk;
            s1.hk_t = s2.hk_t;
            s1.hk_d = s2.hk_d;
            s1.hk_u = s2.hk_u;
            s1.hk_ms = s2.hk_ms;
            s1.rt = s2.rt;
            s1.rt_t = s2.rt_t;
            s1.rt_u = s2.rt_u;
            s1.rt_ms = s2.rt_ms;
            s1.rt_re = s2.rt_re;
            s1.m = s2.m;
            s1.m_u = s2.m_u;
            s1.m_ms = s2.m_ms;
        }
        let (s1, s2) = (&self.s1, &self.s2);

        let hka = 0.5 * (s1.hk + s2.hk);
        let rta = 0.5 * (s1.rt + s2.rt);
        let ma = 0.5 * (s1.m + s2.m);

        let (cfm, cfm_hka, cfm_rta, cfm_ma) = match ityp {
            Regime::Wake => (0.0, 0.0, 0.0, 0.0),
            Regime::Laminar | Regime::Similarity => cfl(hka, rta, ma),
            Regime::Turbulent => {
                let turb = cft(hka, rta, ma, self.par.cffac);
                let lam = cfl(hka, rta, ma);
                if lam.0 > turb.0 { lam } else { turb }
            }
        };

        self.mid = Midpoint {
            cfm,
            cfm_u1: 0.5 * (cfm_hka * s1.hk_u + cfm_ma * s1.m_u + cfm_rta * s1.rt_u),
            cfm_t1: 0.5 * (cfm_hka * s1.hk_t + cfm_rta * s1.rt_t),
            cfm_d1: 0.5 * (cfm_hka * s1.hk_d),
            cfm_u2: 0.5 * (cfm_hka * s2.hk_u + cfm_ma * s2.m_u + cfm_rta * s2.rt_u),
            cfm_t2: 0.5 * (cfm_hka * s2.hk_t + cfm_rta * s2.rt_t),
            cfm_d2: 0.5 * (cfm_hka * s2.hk_d),
            cfm_ms: 0.5
                * (cfm_hka * s1.hk_ms
                    + cfm_ma * s1.m_ms
                    + cfm_rta * s1.rt_ms
                    + cfm_hka * s2.hk_ms
                    + cfm_ma * s2.m_ms
                    + cfm_rta * s2.rt_ms),
            cfm_re: 0.5 * (cfm_rta * s1.rt_re + cfm_rta * s2.rt_re),
        };
    }

    /// XFOIL `TRCHEK` (second-order version `TRCHEK2`): solves the
    /// amplification equation over the interval for `N2`, then checks for
    /// free or forced transition and sets the transition point `XT` and its
    /// sensitivities.
    pub fn trchek(&mut self) {
        const DAEPS: f64 = 5.0e-5;
        let amcrit = self.amcrit;
        let xiforc = self.xiforc;

        // Station 2 is clobbered by BLKIN at the transition point.
        let c2sav = self.s2;

        let (x1, t1, d1, u1, ampl1) = (self.s1.x, self.s1.t, self.s1.d, self.s1.u, self.s1.ampl);
        let (hk1, rt1) = (self.s1.hk, self.s1.rt);
        let (x2, t2, d2, u2) = (c2sav.x, c2sav.t, c2sav.d, c2sav.u);

        // Average amplification rate over X1..X2 and initial guess for N2.
        let ax = axset(
            hk1, t1, rt1, ampl1, c2sav.hk, t2, c2sav.rt, c2sav.ampl, amcrit, self.damp2,
        );
        self.s2.ampl = ampl1 + ax.ax * (x2 - x1);

        // Quantities of the last iteration, used by the sensitivities.
        let mut wf1 = 0.0;
        let mut wf2 = 0.0;
        let (mut wf1_a1, mut wf1_x1, mut wf1_x2) = (0.0, 0.0, 0.0);
        let (mut wf2_a1, mut wf2_x1, mut wf2_x2) = (0.0, 0.0, 0.0);
        let (mut xt_a2, mut tt_a2, mut dt_a2, mut ut_a2) = (0.0, 0.0, 0.0, 0.0);
        let mut amplt_a2 = 0.0;
        let mut xt = x2;
        let (mut hkt_tt, mut hkt_dt, mut hkt_ut, mut hkt_ms) = (0.0, 0.0, 0.0, 0.0);
        let (mut rtt_tt, mut rtt_ut, mut rtt_ms, mut rtt_re) = (0.0, 0.0, 0.0, 0.0);
        let mut axt = Ax::default();

        for _ in 0..30 {
            let ampl2 = self.s2.ampl;
            // Weighting factors defining the "T" quantities from 1 and 2.
            let (amplt, sfa, sfa_a1, sfa_a2);
            if ampl2 <= amcrit {
                // No transition yet: "T" is the same as "2".
                amplt = ampl2;
                amplt_a2 = 1.0;
                sfa = 1.0;
                sfa_a1 = 0.0;
                sfa_a2 = 0.0;
            } else {
                // Transition in X1..X2: "T" is set from N1, N2.
                amplt = amcrit;
                amplt_a2 = 0.0;
                sfa = (amplt - ampl1) / (ampl2 - ampl1);
                sfa_a1 = (sfa - 1.0) / (ampl2 - ampl1);
                sfa_a2 = (-sfa) / (ampl2 - ampl1);
            }

            // (XFOIL also carries the derivatives wrt the trip location,
            // which do not matter for a free transition.)
            let (sfx, sfx_x1, sfx_x2) = if xiforc < x2 {
                let sfx = (xiforc - x1) / (x2 - x1);
                (sfx, (sfx - 1.0) / (x2 - x1), (-sfx) / (x2 - x1))
            } else {
                (1.0, 0.0, 0.0)
            };

            // Weighting factor from free or forced transition.
            let wf2_a2;
            if sfa < sfx {
                wf2 = sfa;
                wf2_a1 = sfa_a1;
                wf2_a2 = sfa_a2;
                wf2_x1 = 0.0;
                wf2_x2 = 0.0;
            } else {
                wf2 = sfx;
                wf2_a1 = 0.0;
                wf2_a2 = 0.0;
                wf2_x1 = sfx_x1;
                wf2_x2 = sfx_x2;
            }
            wf1 = 1.0 - wf2;
            wf1_a1 = -wf2_a1;
            let wf1_a2 = -wf2_a2;
            wf1_x1 = -wf2_x1;
            wf1_x2 = -wf2_x2;

            // Interpolate the BL variables to XT.
            xt = x1 * wf1 + x2 * wf2;
            let tt = t1 * wf1 + t2 * wf2;
            let dt = d1 * wf1 + d2 * wf2;
            let ut = u1 * wf1 + u2 * wf2;
            xt_a2 = x1 * wf1_a2 + x2 * wf2_a2;
            tt_a2 = t1 * wf1_a2 + t2 * wf2_a2;
            dt_a2 = d1 * wf1_a2 + d2 * wf2_a2;
            ut_a2 = u1 * wf1_a2 + u2 * wf2_a2;

            // Laminar secondary "T" variables HKT, RTT (computed in "2").
            self.s2.x = xt;
            self.s2.t = tt;
            self.s2.d = dt;
            self.s2.u = ut;
            self.blkin();
            let hkt = self.s2.hk;
            hkt_tt = self.s2.hk_t;
            hkt_dt = self.s2.hk_d;
            hkt_ut = self.s2.hk_u;
            hkt_ms = self.s2.hk_ms;
            let rtt = self.s2.rt;
            rtt_tt = self.s2.rt_t;
            rtt_ut = self.s2.rt_u;
            rtt_ms = self.s2.rt_ms;
            rtt_re = self.s2.rt_re;

            // Restore station 2, except AMPL2.
            let amsave = self.s2.ampl;
            self.s2 = c2sav;
            self.s2.ampl = amsave;

            // Amplification rate over the current X1..XT interval.
            axt = axset(hk1, t1, rt1, ampl1, hkt, tt, rtt, amplt, amcrit, self.damp2);
            if axt.ax <= 0.0 {
                // No amplification here.
                break;
            }

            let ax_a2 = (axt.hk2 * hkt_tt + axt.t2 + axt.rt2 * rtt_tt) * tt_a2
                + (axt.hk2 * hkt_dt) * dt_a2
                + (axt.hk2 * hkt_ut + axt.rt2 * rtt_ut) * ut_a2
                + axt.a2 * amplt_a2;

            // Residual of the implicit amplification equation.
            let ampl2 = self.s2.ampl;
            let res = ampl2 - ampl1 - axt.ax * (x2 - x1);
            let res_a2 = 1.0 - ax_a2 * (x2 - x1);
            let da2 = -res / res_a2;

            let mut rlx = 1.0;
            let dxt = xt_a2 * da2;
            if rlx * (dxt / (x2 - x1)).abs() > 0.05 {
                rlx = 0.05 * ((x2 - x1) / dxt).abs();
            }
            if rlx * da2.abs() > 1.0 {
                rlx = 1.0 * (1.0 / da2).abs();
            }

            if da2.abs() < DAEPS {
                break;
            }

            if (ampl2 > amcrit && ampl2 + rlx * da2 < amcrit)
                || (ampl2 < amcrit && ampl2 + rlx * da2 > amcrit)
            {
                // Limited Newton step: N2 does not step across Ncrit.
                self.s2.ampl = amcrit;
            } else {
                self.s2.ampl = ampl2 + rlx * da2;
            }
        }
        // After 30 iterations XFOIL reports "TRCHEK2: N2 convergence
        // failed" and goes on with the last iterate.

        // Free or forced transition.
        let ampl2 = self.s2.ampl;
        self.trfree = ampl2 >= amcrit;
        self.trforc = xiforc > x1 && xiforc <= x2;
        self.tran = self.trforc || self.trfree;
        if !self.tran {
            return;
        }
        if self.trfree && self.trforc {
            self.trforc = xiforc < xt;
            self.trfree = xiforc >= xt;
        }

        if self.trforc {
            // Forced transition: XT is prescribed.
            self.tr = TransitionPoint {
                xt: xiforc,
                xt_xf: 1.0,
                ..TransitionPoint::default()
            };
            return;
        }

        // Free transition: sensitivities of XT.
        let x_t_x1 = wf1;
        let tt_t1 = wf1;
        let dt_d1 = wf1;
        let ut_u1 = wf1;
        let x_t_x2 = wf2;
        let tt_t2 = wf2;
        let dt_d2 = wf2;
        let ut_u2 = wf2;

        let mut xt_a1 = x1 * wf1_a1 + x2 * wf2_a1;
        let tt_a1 = t1 * wf1_a1 + t2 * wf2_a1;
        let dt_a1 = d1 * wf1_a1 + d2 * wf2_a1;
        let ut_a1 = u1 * wf1_a1 + u2 * wf2_a1;

        let mut xt_x1 = x1 * wf1_x1 + x2 * wf2_x1 + x_t_x1;
        let tt_x1 = t1 * wf1_x1 + t2 * wf2_x1;
        let dt_x1 = d1 * wf1_x1 + d2 * wf2_x1;
        let ut_x1 = u1 * wf1_x1 + u2 * wf2_x1;

        let mut xt_x2 = x1 * wf1_x2 + x2 * wf2_x2 + x_t_x2;
        let tt_x2 = t1 * wf1_x2 + t2 * wf2_x2;
        let dt_x2 = d1 * wf1_x2 + d2 * wf2_x2;
        let ut_x2 = u1 * wf1_x2 + u2 * wf2_x2;

        // AX = AX(HK1, T1, RT1, A1, HKT, TT, RTT, AT).
        let s1 = &self.s1;
        let g_tt = axt.hk2 * hkt_tt + axt.t2 + axt.rt2 * rtt_tt;
        let g_dt = axt.hk2 * hkt_dt;
        let g_ut = axt.hk2 * hkt_ut + axt.rt2 * rtt_ut;

        let ax_t1 = axt.hk1 * s1.hk_t + axt.t1 + axt.rt1 * s1.rt_t + g_tt * tt_t1;
        let ax_d1 = axt.hk1 * s1.hk_d + g_dt * dt_d1;
        let ax_u1 = axt.hk1 * s1.hk_u + axt.rt1 * s1.rt_u + g_ut * ut_u1;
        let ax_a1 = axt.a1 + g_tt * tt_a1 + g_dt * dt_a1 + g_ut * ut_a1;
        let ax_x1 = g_tt * tt_x1 + g_dt * dt_x1 + g_ut * ut_x1;

        let ax_t2 = g_tt * tt_t2;
        let ax_d2 = g_dt * dt_d2;
        let ax_u2 = g_ut * ut_u2;
        let ax_a2 = axt.a2 * amplt_a2 + g_tt * tt_a2 + g_dt * dt_a2 + g_ut * ut_a2;
        let ax_x2 = g_tt * tt_x2 + g_dt * dt_x2 + g_ut * ut_x2;

        let ax_ms = axt.hk2 * hkt_ms + axt.rt2 * rtt_ms + axt.hk1 * s1.hk_ms + axt.rt1 * s1.rt_ms;
        let ax_re = axt.rt2 * rtt_re + axt.rt1 * s1.rt_re;

        // Sensitivities of the residual RES = AMPL2 - AMPL1 - AX*(X2-X1).
        let z_ax = -(x2 - x1);
        let z_a1 = z_ax * ax_a1 - 1.0;
        let z_t1 = z_ax * ax_t1;
        let z_d1 = z_ax * ax_d1;
        let z_u1 = z_ax * ax_u1;
        let z_x1 = z_ax * ax_x1 + axt.ax;
        let z_a2 = z_ax * ax_a2 + 1.0;
        let z_t2 = z_ax * ax_t2;
        let z_d2 = z_ax * ax_d2;
        let z_u2 = z_ax * ax_u2;
        let z_x2 = z_ax * ax_x2 - axt.ax;
        let z_ms = z_ax * ax_ms;
        let z_re = z_ax * ax_re;

        // Sensitivities of XT with RES stationary for the A2 constraint.
        let q = xt_a2 / z_a2;
        xt_a1 -= q * z_a1;
        xt_x1 -= q * z_x1;
        xt_x2 -= q * z_x2;
        self.tr = TransitionPoint {
            xt,
            xt_a1,
            xt_t1: -q * z_t1,
            xt_d1: -q * z_d1,
            xt_u1: -q * z_u1,
            xt_x1,
            xt_t2: -q * z_t2,
            xt_d2: -q * z_d2,
            xt_u2: -q * z_u2,
            xt_x2,
            xt_ms: -q * z_ms,
            xt_re: -q * z_re,
            xt_xf: 0.0,
        };
    }

    /// XFOIL `BLSYS`: sets up the Newton system of the current interval in
    /// terms of the incompressible edge velocity.
    pub fn blsys(&mut self) {
        if self.wake {
            self.blvar(Regime::Wake);
            self.blmid(Regime::Wake);
        } else if self.turb || self.tran {
            self.blvar(Regime::Turbulent);
            self.blmid(Regime::Turbulent);
        } else {
            self.blvar(Regime::Laminar);
            self.blmid(Regime::Laminar);
        }

        // At the similarity station, "1" and "2" variables are the same.
        if self.simi {
            self.s1 = self.s2;
        }

        if self.tran {
            self.trdif();
        } else if self.simi {
            self.bldif(Regime::Similarity);
        } else if !self.turb {
            self.bldif(Regime::Laminar);
        } else if self.wake {
            self.bldif(Regime::Wake);
        } else {
            self.bldif(Regime::Turbulent);
        }

        let sys = &mut self.sys;
        if self.simi {
            // "1" variables are really "2" variables.
            for k in 0..4 {
                for l in 0..5 {
                    sys.vs2[k][l] += sys.vs1[k][l];
                    sys.vs1[k][l] = 0.0;
                }
            }
        }

        // Change the system over to incompressible Uei and Mach.
        let (s1, s2) = (&self.s1, &self.s2);
        for k in 0..4 {
            let res_u1 = sys.vs1[k][3];
            let res_u2 = sys.vs2[k][3];
            let res_ms = sys.vsm[k];
            sys.vs1[k][3] = res_u1 * s1.u_uei;
            sys.vs2[k][3] = res_u2 * s2.u_uei;
            sys.vsm[k] = res_u1 * s1.u_ms + res_u2 * s2.u_ms + res_ms;
        }
    }

    /// XFOIL `TESYS`: "dummy" system between the trailing edge and the first
    /// wake point infinitesimally behind it.
    pub fn tesys(&mut self, cte: f64, tte: f64, dte: f64) {
        self.sys = System::default();
        self.blvar(Regime::Wake);
        let s2 = &self.s2;
        let sys = &mut self.sys;
        sys.vs1[0][0] = -1.0;
        sys.vs2[0][0] = 1.0;
        sys.vsrez[0] = cte - s2.s;
        sys.vs1[1][1] = -1.0;
        sys.vs2[1][1] = 1.0;
        sys.vsrez[1] = tte - s2.t;
        sys.vs1[2][2] = -1.0;
        sys.vs2[2][2] = 1.0;
        sys.vsrez[2] = dte - s2.d - s2.dw;
    }

    /// XFOIL `BLDIF`: Newton system coefficients and residuals of the
    /// amplification or shear-lag, momentum and shape-parameter equations
    /// for a similarity, laminar, turbulent or wake interval.
    pub fn bldif(&mut self, ityp: Regime) {
        let par = self.par;
        let (s1, s2) = (self.s1, self.s2);
        let mid = self.mid;

        let (xlog, ulog, tlog, hlog, ddlog);
        if ityp == Regime::Similarity {
            // Similarity logarithmic differences (prescribed).
            xlog = 1.0;
            ulog = self.bule;
            tlog = 0.5 * (1.0 - self.bule);
            hlog = 0.0;
            ddlog = 0.0;
        } else {
            xlog = (s2.x / s1.x).ln();
            ulog = (s2.u / s1.u).ln();
            tlog = (s2.t / s1.t).ln();
            hlog = (s2.hs / s1.hs).ln();
            ddlog = 1.0;
        }

        let mut sys = System::default();

        // Local upwinding, based on the local change in log(Hk-1)
        // (mainly kicks in at transition).
        let hupwt = 1.0;
        let (hdcon, hd_hk1, hd_hk2) = if ityp == Regime::Wake {
            // Less upwinding in the wake.
            let hdcon = hupwt / (s2.hk * s2.hk);
            (hdcon, 0.0, -hdcon * 2.0 / s2.hk)
        } else {
            let hdcon = 5.0 * hupwt / (s2.hk * s2.hk);
            (hdcon, 0.0, -hdcon * 2.0 / s2.hk)
        };

        let arg = ((s2.hk - 1.0) / (s1.hk - 1.0)).abs();
        let hl = arg.ln();
        let hl_hk1 = -1.0 / (s1.hk - 1.0);
        let hl_hk2 = 1.0 / (s2.hk - 1.0);

        // Upwinding parameter: 0.5 trapezoidal, 1 backward Euler.
        let hlsq = (hl * hl).min(15.0);
        let ehh = (-hlsq * hdcon).exp();
        let upw = 1.0 - 0.5 * ehh;
        let upw_hl = ehh * hl * hdcon;
        let upw_hd = 0.5 * ehh * hlsq;

        let upw_hk1 = upw_hl * hl_hk1 + upw_hd * hd_hk1;
        let upw_hk2 = upw_hl * hl_hk2 + upw_hd * hd_hk2;

        let upw_u1 = upw_hk1 * s1.hk_u;
        let upw_t1 = upw_hk1 * s1.hk_t;
        let upw_d1 = upw_hk1 * s1.hk_d;
        let upw_u2 = upw_hk2 * s2.hk_u;
        let upw_t2 = upw_hk2 * s2.hk_t;
        let upw_d2 = upw_hk2 * s2.hk_d;
        let upw_ms = upw_hk1 * s1.hk_ms + upw_hk2 * s2.hk_ms;

        match ityp {
            Regime::Similarity => {
                // Stagnation point: zero amplification factor.
                sys.vs2[0][0] = 1.0;
                sys.vsr[0] = 0.0;
                sys.vsrez[0] = -s2.ampl;
            }
            Regime::Laminar => {
                // Amplification equation.
                let ax = axset(
                    s1.hk,
                    s1.t,
                    s1.rt,
                    s1.ampl,
                    s2.hk,
                    s2.t,
                    s2.rt,
                    s2.ampl,
                    self.amcrit,
                    self.damp2,
                );
                let rezc = s2.ampl - s1.ampl - ax.ax * (s2.x - s1.x);
                let z_ax = -(s2.x - s1.x);

                sys.vs1[0][0] = z_ax * ax.a1 - 1.0;
                sys.vs1[0][1] = z_ax * (ax.hk1 * s1.hk_t + ax.t1 + ax.rt1 * s1.rt_t);
                sys.vs1[0][2] = z_ax * (ax.hk1 * s1.hk_d);
                sys.vs1[0][3] = z_ax * (ax.hk1 * s1.hk_u + ax.rt1 * s1.rt_u);
                sys.vs1[0][4] = ax.ax;
                sys.vs2[0][0] = z_ax * ax.a2 + 1.0;
                sys.vs2[0][1] = z_ax * (ax.hk2 * s2.hk_t + ax.t2 + ax.rt2 * s2.rt_t);
                sys.vs2[0][2] = z_ax * (ax.hk2 * s2.hk_d);
                sys.vs2[0][3] = z_ax * (ax.hk2 * s2.hk_u + ax.rt2 * s2.rt_u);
                sys.vs2[0][4] = -ax.ax;
                sys.vsm[0] = z_ax
                    * (ax.hk1 * s1.hk_ms
                        + ax.rt1 * s1.rt_ms
                        + ax.hk2 * s2.hk_ms
                        + ax.rt2 * s2.rt_ms);
                sys.vsr[0] = z_ax * (ax.rt1 * s1.rt_re + ax.rt2 * s2.rt_re);
                sys.vsx[0] = 0.0;
                sys.vsrez[0] = -rezc;
            }
            Regime::Turbulent | Regime::Wake => {
                // Shear-lag equation.
                let sa = (1.0 - upw) * s1.s + upw * s2.s;
                let cqa = (1.0 - upw) * s1.cq + upw * s2.cq;
                let cfa = (1.0 - upw) * s1.cf + upw * s2.cf;
                let hka = (1.0 - upw) * s1.hk + upw * s2.hk;

                let usa = 0.5 * (s1.us + s2.us);
                let rta = 0.5 * (s1.rt + s2.rt);
                let dea = 0.5 * (s1.de + s2.de);
                let da = 0.5 * (s1.d + s2.d);

                // Increased dissipation length in the wake.
                let ald = if ityp == Regime::Wake { par.dlcon } else { 1.0 };

                // Equilibrium 1/Ue dUe/dx.
                let (hkc, hkc_hka);
                if ityp == Regime::Turbulent {
                    let v = hka - 1.0 - par.gccon / rta;
                    if v < 0.01 {
                        hkc = 0.01;
                        hkc_hka = 0.0;
                    } else {
                        hkc = v;
                        hkc_hka = 1.0;
                    }
                } else {
                    hkc = hka - 1.0;
                    hkc_hka = 1.0;
                }

                let hr = hkc / (par.gacon * ald * hka);
                let hr_hka = hkc_hka / (par.gacon * ald * hka) - hr / hka;

                // XFOIL also assembles UQ_T1 ... UQ_RE (including the RTA
                // dependence of HKC) but never uses them: UQ enters the
                // system through CFA, HKA, DA and UPW only.
                let uq = (0.5 * cfa - hr * hr) / (par.gbcon * da);
                let uq_hka = -2.0 * hr * hr_hka / (par.gbcon * da);
                let uq_cfa = 0.5 / (par.gbcon * da);
                let uq_da = -uq / da;

                let scc = par.sccon * 1.333 / (1.0 + usa);
                let scc_usa = -scc / (1.0 + usa);

                let slog = (s2.s / s1.s).ln();
                let dxi = s2.x - s1.x;

                let rezc = scc * (cqa - sa * ald) * dxi - dea * 2.0 * slog
                    + dea * 2.0 * (uq * dxi - ulog) * par.duxcon;

                let z_cfa = dea * 2.0 * uq_cfa * dxi * par.duxcon;
                let z_hka = dea * 2.0 * uq_hka * dxi * par.duxcon;
                let z_da = dea * 2.0 * uq_da * dxi * par.duxcon;
                let z_sl = -dea * 2.0;
                let z_ul = -dea * 2.0 * par.duxcon;
                let z_dxi = scc * (cqa - sa * ald) + dea * 2.0 * uq * par.duxcon;
                let z_usa = scc_usa * (cqa - sa * ald) * dxi;
                let z_cqa = scc * dxi;
                let z_sa = -scc * dxi * ald;
                let z_dea = 2.0 * ((uq * dxi - ulog) * par.duxcon - slog);

                let z_upw = z_cqa * (s2.cq - s1.cq)
                    + z_sa * (s2.s - s1.s)
                    + z_cfa * (s2.cf - s1.cf)
                    + z_hka * (s2.hk - s1.hk);
                let z_de1 = 0.5 * z_dea;
                let z_de2 = 0.5 * z_dea;
                let z_us1 = 0.5 * z_usa;
                let z_us2 = 0.5 * z_usa;
                let z_d1 = 0.5 * z_da;
                let z_d2 = 0.5 * z_da;
                let z_u1 = -z_ul / s1.u;
                let z_u2 = z_ul / s2.u;
                let z_x1 = -z_dxi;
                let z_x2 = z_dxi;
                let z_s1 = (1.0 - upw) * z_sa - z_sl / s1.s;
                let z_s2 = upw * z_sa + z_sl / s2.s;
                let z_cq1 = (1.0 - upw) * z_cqa;
                let z_cq2 = upw * z_cqa;
                let z_cf1 = (1.0 - upw) * z_cfa;
                let z_cf2 = upw * z_cfa;
                let z_hk1 = (1.0 - upw) * z_hka;
                let z_hk2 = upw * z_hka;

                sys.vs1[0][0] = z_s1;
                sys.vs1[0][1] = z_upw * upw_t1 + z_de1 * s1.de_t + z_us1 * s1.us_t;
                sys.vs1[0][2] = z_d1 + z_upw * upw_d1 + z_de1 * s1.de_d + z_us1 * s1.us_d;
                sys.vs1[0][3] = z_u1 + z_upw * upw_u1 + z_de1 * s1.de_u + z_us1 * s1.us_u;
                sys.vs1[0][4] = z_x1;
                sys.vs2[0][0] = z_s2;
                sys.vs2[0][1] = z_upw * upw_t2 + z_de2 * s2.de_t + z_us2 * s2.us_t;
                sys.vs2[0][2] = z_d2 + z_upw * upw_d2 + z_de2 * s2.de_d + z_us2 * s2.us_d;
                sys.vs2[0][3] = z_u2 + z_upw * upw_u2 + z_de2 * s2.de_u + z_us2 * s2.us_u;
                sys.vs2[0][4] = z_x2;
                sys.vsm[0] = z_upw * upw_ms
                    + z_de1 * s1.de_ms
                    + z_us1 * s1.us_ms
                    + z_de2 * s2.de_ms
                    + z_us2 * s2.us_ms;

                sys.vs1[0][1] += z_cq1 * s1.cq_t + z_cf1 * s1.cf_t + z_hk1 * s1.hk_t;
                sys.vs1[0][2] += z_cq1 * s1.cq_d + z_cf1 * s1.cf_d + z_hk1 * s1.hk_d;
                sys.vs1[0][3] += z_cq1 * s1.cq_u + z_cf1 * s1.cf_u + z_hk1 * s1.hk_u;

                sys.vs2[0][1] += z_cq2 * s2.cq_t + z_cf2 * s2.cf_t + z_hk2 * s2.hk_t;
                sys.vs2[0][2] += z_cq2 * s2.cq_d + z_cf2 * s2.cf_d + z_hk2 * s2.hk_d;
                sys.vs2[0][3] += z_cq2 * s2.cq_u + z_cf2 * s2.cf_u + z_hk2 * s2.hk_u;

                sys.vsm[0] += z_cq1 * s1.cq_ms
                    + z_cf1 * s1.cf_ms
                    + z_hk1 * s1.hk_ms
                    + z_cq2 * s2.cq_ms
                    + z_cf2 * s2.cf_ms
                    + z_hk2 * s2.hk_ms;
                sys.vsr[0] =
                    z_cq1 * s1.cq_re + z_cf1 * s1.cf_re + z_cq2 * s2.cq_re + z_cf2 * s2.cf_re;
                sys.vsx[0] = 0.0;
                sys.vsrez[0] = -rezc;
            }
        }

        // Momentum equation.
        let ha = 0.5 * (s1.h + s2.h);
        let ma = 0.5 * (s1.m + s2.m);
        let xa = 0.5 * (s1.x + s2.x);
        let ta = 0.5 * (s1.t + s2.t);
        let hwa = 0.5 * (s1.dw / s1.t + s2.dw / s2.t);

        // Cf term, using the central value CFM for better accuracy in drag.
        let cfx = 0.50 * mid.cfm * xa / ta + 0.25 * (s1.cf * s1.x / s1.t + s2.cf * s2.x / s2.t);
        let cfx_xa = 0.50 * mid.cfm / ta;
        let cfx_ta = -0.50 * mid.cfm * xa / (ta * ta);

        let cfx_x1 = 0.25 * s1.cf / s1.t + cfx_xa * 0.5;
        let cfx_x2 = 0.25 * s2.cf / s2.t + cfx_xa * 0.5;
        let cfx_t1 = -0.25 * s1.cf * s1.x / (s1.t * s1.t) + cfx_ta * 0.5;
        let cfx_t2 = -0.25 * s2.cf * s2.x / (s2.t * s2.t) + cfx_ta * 0.5;
        let cfx_cf1 = 0.25 * s1.x / s1.t;
        let cfx_cf2 = 0.25 * s2.x / s2.t;
        let cfx_cfm = 0.50 * xa / ta;

        let btmp = ha + 2.0 - ma + hwa;

        let rezt = tlog + btmp * ulog - xlog * 0.5 * cfx;
        let z_cfx = -xlog * 0.5;
        let z_ha = ulog;
        let z_hwa = ulog;
        let z_ma = -ulog;
        let z_xl = -ddlog * 0.5 * cfx;
        let z_ul = ddlog * btmp;
        let z_tl = ddlog;

        let z_cfm = z_cfx * cfx_cfm;
        let z_cf1 = z_cfx * cfx_cf1;
        let z_cf2 = z_cfx * cfx_cf2;

        let z_t1 = -z_tl / s1.t + z_cfx * cfx_t1 + z_hwa * 0.5 * (-s1.dw / (s1.t * s1.t));
        let z_t2 = z_tl / s2.t + z_cfx * cfx_t2 + z_hwa * 0.5 * (-s2.dw / (s2.t * s2.t));
        let z_x1 = -z_xl / s1.x + z_cfx * cfx_x1;
        let z_x2 = z_xl / s2.x + z_cfx * cfx_x2;
        let z_u1 = -z_ul / s1.u;
        let z_u2 = z_ul / s2.u;

        sys.vs1[1][1] = 0.5 * z_ha * s1.h_t + z_cfm * mid.cfm_t1 + z_cf1 * s1.cf_t + z_t1;
        sys.vs1[1][2] = 0.5 * z_ha * s1.h_d + z_cfm * mid.cfm_d1 + z_cf1 * s1.cf_d;
        sys.vs1[1][3] = 0.5 * z_ma * s1.m_u + z_cfm * mid.cfm_u1 + z_cf1 * s1.cf_u + z_u1;
        sys.vs1[1][4] = z_x1;
        sys.vs2[1][1] = 0.5 * z_ha * s2.h_t + z_cfm * mid.cfm_t2 + z_cf2 * s2.cf_t + z_t2;
        sys.vs2[1][2] = 0.5 * z_ha * s2.h_d + z_cfm * mid.cfm_d2 + z_cf2 * s2.cf_d;
        sys.vs2[1][3] = 0.5 * z_ma * s2.m_u + z_cfm * mid.cfm_u2 + z_cf2 * s2.cf_u + z_u2;
        sys.vs2[1][4] = z_x2;

        sys.vsm[1] = 0.5 * z_ma * s1.m_ms
            + z_cfm * mid.cfm_ms
            + z_cf1 * s1.cf_ms
            + 0.5 * z_ma * s2.m_ms
            + z_cf2 * s2.cf_ms;
        sys.vsr[1] = z_cfm * mid.cfm_re + z_cf1 * s1.cf_re + z_cf2 * s2.cf_re;
        sys.vsx[1] = 0.0;
        sys.vsrez[1] = -rezt;

        // Shape parameter equation.
        let xot1 = s1.x / s1.t;
        let xot2 = s2.x / s2.t;

        let ha = 0.5 * (s1.h + s2.h);
        let hsa = 0.5 * (s1.hs + s2.hs);
        let hca = 0.5 * (s1.hc + s2.hc);
        let hwa = 0.5 * (s1.dw / s1.t + s2.dw / s2.t);

        let dix = (1.0 - upw) * s1.di * xot1 + upw * s2.di * xot2;
        let cfx = (1.0 - upw) * s1.cf * xot1 + upw * s2.cf * xot2;
        let dix_upw = s2.di * xot2 - s1.di * xot1;
        let cfx_upw = s2.cf * xot2 - s1.cf * xot1;

        let btmp = 2.0 * hca / hsa + 1.0 - ha - hwa;

        let rezh = hlog + btmp * ulog + xlog * (0.5 * cfx - dix);
        let z_cfx = xlog * 0.5;
        let z_dix = -xlog;
        let z_hca = 2.0 * ulog / hsa;
        let z_ha = -ulog;
        let z_hwa = -ulog;
        let z_xl = ddlog * (0.5 * cfx - dix);
        let z_ul = ddlog * btmp;
        let z_hl = ddlog;

        let z_upw = z_cfx * cfx_upw + z_dix * dix_upw;

        let z_hs1 = -hca * ulog / (hsa * hsa) - z_hl / s1.hs;
        let z_hs2 = -hca * ulog / (hsa * hsa) + z_hl / s2.hs;

        let z_cf1 = (1.0 - upw) * z_cfx * xot1;
        let z_cf2 = upw * z_cfx * xot2;
        let z_di1 = (1.0 - upw) * z_dix * xot1;
        let z_di2 = upw * z_dix * xot2;

        let mut z_t1 = (1.0 - upw) * (z_cfx * s1.cf + z_dix * s1.di) * (-xot1 / s1.t);
        let mut z_t2 = upw * (z_cfx * s2.cf + z_dix * s2.di) * (-xot2 / s2.t);
        let z_x1 = (1.0 - upw) * (z_cfx * s1.cf + z_dix * s1.di) / s1.t - z_xl / s1.x;
        let z_x2 = upw * (z_cfx * s2.cf + z_dix * s2.di) / s2.t + z_xl / s2.x;
        let z_u1 = -z_ul / s1.u;
        let z_u2 = z_ul / s2.u;

        z_t1 += z_hwa * 0.5 * (-s1.dw / (s1.t * s1.t));
        z_t2 += z_hwa * 0.5 * (-s2.dw / (s2.t * s2.t));

        sys.vs1[2][0] = z_di1 * s1.di_s;
        sys.vs1[2][1] = z_hs1 * s1.hs_t + z_cf1 * s1.cf_t + z_di1 * s1.di_t + z_t1;
        sys.vs1[2][2] = z_hs1 * s1.hs_d + z_cf1 * s1.cf_d + z_di1 * s1.di_d;
        sys.vs1[2][3] = z_hs1 * s1.hs_u + z_cf1 * s1.cf_u + z_di1 * s1.di_u + z_u1;
        sys.vs1[2][4] = z_x1;
        sys.vs2[2][0] = z_di2 * s2.di_s;
        sys.vs2[2][1] = z_hs2 * s2.hs_t + z_cf2 * s2.cf_t + z_di2 * s2.di_t + z_t2;
        sys.vs2[2][2] = z_hs2 * s2.hs_d + z_cf2 * s2.cf_d + z_di2 * s2.di_d;
        sys.vs2[2][3] = z_hs2 * s2.hs_u + z_cf2 * s2.cf_u + z_di2 * s2.di_u + z_u2;
        sys.vs2[2][4] = z_x2;
        sys.vsm[2] = z_hs1 * s1.hs_ms
            + z_cf1 * s1.cf_ms
            + z_di1 * s1.di_ms
            + z_hs2 * s2.hs_ms
            + z_cf2 * s2.cf_ms
            + z_di2 * s2.di_ms;
        sys.vsr[2] = z_hs1 * s1.hs_re
            + z_cf1 * s1.cf_re
            + z_di1 * s1.di_re
            + z_hs2 * s2.hs_re
            + z_cf2 * s2.cf_re
            + z_di2 * s2.di_re;

        sys.vs1[2][1] += 0.5 * (z_hca * s1.hc_t + z_ha * s1.h_t) + z_upw * upw_t1;
        sys.vs1[2][2] += 0.5 * (z_hca * s1.hc_d + z_ha * s1.h_d) + z_upw * upw_d1;
        sys.vs1[2][3] += 0.5 * (z_hca * s1.hc_u) + z_upw * upw_u1;
        sys.vs2[2][1] += 0.5 * (z_hca * s2.hc_t + z_ha * s2.h_t) + z_upw * upw_t2;
        sys.vs2[2][2] += 0.5 * (z_hca * s2.hc_d + z_ha * s2.h_d) + z_upw * upw_d2;
        sys.vs2[2][3] += 0.5 * (z_hca * s2.hc_u) + z_upw * upw_u2;

        sys.vsm[2] += 0.5 * (z_hca * s1.hc_ms) + z_upw * upw_ms + 0.5 * (z_hca * s2.hc_ms);

        sys.vsx[2] = 0.0;
        sys.vsrez[2] = -rezh;

        self.sys = sys;
    }

    /// XFOIL `TRDIF`: Newton system of the transition interval, the sum of
    /// the laminar equations over `X1 < ξ < XT` and of the turbulent ones
    /// over `XT < ξ < X2`.
    pub fn trdif(&mut self) {
        let c1sav = self.s1;
        let c2sav = self.s2;
        let tr = self.tr;
        let (x1, t1, d1, u1) = (c1sav.x, c1sav.t, c1sav.d, c1sav.u);
        let (x2, t2, d2, u2) = (c2sav.x, c2sav.t, c2sav.d, c2sav.u);

        // Weighting factors for linear interpolation to the transition point.
        let wf2 = (tr.xt - x1) / (x2 - x1);
        let wf2_xt = 1.0 / (x2 - x1);

        let wf2_a1 = wf2_xt * tr.xt_a1;
        let wf2_x1 = wf2_xt * tr.xt_x1 + (wf2 - 1.0) / (x2 - x1);
        let wf2_x2 = wf2_xt * tr.xt_x2 - wf2 / (x2 - x1);
        let wf2_t1 = wf2_xt * tr.xt_t1;
        let wf2_t2 = wf2_xt * tr.xt_t2;
        let wf2_d1 = wf2_xt * tr.xt_d1;
        let wf2_d2 = wf2_xt * tr.xt_d2;
        let wf2_u1 = wf2_xt * tr.xt_u1;
        let wf2_u2 = wf2_xt * tr.xt_u2;
        let wf2_ms = wf2_xt * tr.xt_ms;
        let wf2_re = wf2_xt * tr.xt_re;
        let wf2_xf = wf2_xt * tr.xt_xf;

        let wf1 = 1.0 - wf2;
        let (wf1_a1, wf1_x1, wf1_x2) = (-wf2_a1, -wf2_x1, -wf2_x2);
        let (wf1_t1, wf1_t2, wf1_d1, wf1_d2) = (-wf2_t1, -wf2_t2, -wf2_d1, -wf2_d2);
        let (wf1_u1, wf1_u2, wf1_ms, wf1_re, wf1_xf) =
            (-wf2_u1, -wf2_u2, -wf2_ms, -wf2_re, -wf2_xf);

        // Interpolated primary variable q at XT and its sensitivities to
        // (A1, X1, X2, T1, T2, D1, D2, U1, U2, MS, RE, XF).
        let interp = |q1: f64, q2: f64, own: usize| {
            let mut s = [
                q1 * wf1_a1 + q2 * wf2_a1,
                q1 * wf1_x1 + q2 * wf2_x1,
                q1 * wf1_x2 + q2 * wf2_x2,
                q1 * wf1_t1 + q2 * wf2_t1,
                q1 * wf1_t2 + q2 * wf2_t2,
                q1 * wf1_d1 + q2 * wf2_d1,
                q1 * wf1_d2 + q2 * wf2_d2,
                q1 * wf1_u1 + q2 * wf2_u1,
                q1 * wf1_u2 + q2 * wf2_u2,
                q1 * wf1_ms + q2 * wf2_ms,
                q1 * wf1_re + q2 * wf2_re,
                q1 * wf1_xf + q2 * wf2_xf,
            ];
            if own > 0 {
                s[own] += wf1;
                s[own + 1] += wf2;
            }
            (q1 * wf1 + q2 * wf2, s)
        };
        const A1: usize = 0;
        const X1: usize = 1;
        const X2: usize = 2;
        const T1: usize = 3;
        const T2: usize = 4;
        const D1: usize = 5;
        const D2: usize = 6;
        const U1: usize = 7;
        const U2: usize = 8;
        const MS: usize = 9;
        const RE: usize = 10;
        const XF: usize = 11;

        // FIRST, the laminar part between X1 and XT.
        let (tt, tt_) = interp(t1, t2, T1);
        let (dt, dt_) = interp(d1, d2, D1);
        let (ut, ut_) = interp(u1, u2, U1);
        let xt_ = [
            tr.xt_a1, tr.xt_x1, tr.xt_x2, tr.xt_t1, tr.xt_t2, tr.xt_d1, tr.xt_d2, tr.xt_u1,
            tr.xt_u2, tr.xt_ms, tr.xt_re, tr.xt_xf,
        ];

        // Primary "T" variables at XT, placed into "2".
        self.s2.x = tr.xt;
        self.s2.t = tt;
        self.s2.d = dt;
        self.s2.u = ut;
        self.s2.ampl = self.amcrit;
        self.s2.s = 0.0;

        // Laminar secondary "T" variables, X1-XT midpoint CFM and system.
        self.blkin();
        self.blvar(Regime::Laminar);
        self.blmid(Regime::Laminar);
        self.bldif(Regime::Laminar);

        // Convert the sensitivities wrt the "T" variables into
        // sensitivities wrt the "1" and "2" variables. The amplification
        // equation is not needed: row 0 is left empty.
        let vs = self.sys;
        let chain = |k: usize, var: usize| {
            vs.vs2[k][1] * tt_[var]
                + vs.vs2[k][2] * dt_[var]
                + vs.vs2[k][3] * ut_[var]
                + vs.vs2[k][4] * xt_[var]
        };
        let mut blrez = [0.0; 4];
        let mut blm = [0.0; 4];
        let mut blr = [0.0; 4];
        let mut blx = [0.0; 4];
        let mut bl1 = [[0.0; 5]; 4];
        let mut bl2 = [[0.0; 5]; 4];
        for k in 1..3 {
            blrez[k] = vs.vsrez[k];
            blm[k] = vs.vsm[k] + chain(k, MS);
            blr[k] = vs.vsr[k] + chain(k, RE);
            blx[k] = vs.vsx[k] + chain(k, XF);

            bl1[k][0] = vs.vs1[k][0] + chain(k, A1);
            bl1[k][1] = vs.vs1[k][1] + chain(k, T1);
            bl1[k][2] = vs.vs1[k][2] + chain(k, D1);
            bl1[k][3] = vs.vs1[k][3] + chain(k, U1);
            bl1[k][4] = vs.vs1[k][4] + chain(k, X1);

            bl2[k][0] = 0.0;
            bl2[k][1] = chain(k, T2);
            bl2[k][2] = chain(k, D2);
            bl2[k][3] = chain(k, U2);
            bl2[k][4] = chain(k, X2);
        }

        // SECOND, the turbulent part between XT and X2.

        // Equilibrium shear coefficient CQT at the transition point.
        self.blvar(Regime::Turbulent);

        // Initial shear coefficient ST at the transition point (CQ2 etc. are
        // really CQT etc.).
        let s2 = &self.s2;
        let ctr = self.par.ctrcon * (-self.par.ctrcex / (s2.hk - 1.0)).exp();
        let ctr_hk2 = ctr * self.par.ctrcex / (s2.hk - 1.0).powi(2);

        let st = ctr * s2.cq;
        let st_tt = ctr * s2.cq_t + s2.cq * ctr_hk2 * s2.hk_t;
        let st_dt = ctr * s2.cq_d + s2.cq * ctr_hk2 * s2.hk_d;
        let st_ut = ctr * s2.cq_u + s2.cq * ctr_hk2 * s2.hk_u;
        let st_ms0 = ctr * s2.cq_ms + s2.cq * ctr_hk2 * s2.hk_ms;
        let st_re0 = ctr * s2.cq_re;

        // ST sensitivities wrt the actual "1" and "2" variables.
        let mut st_ = [0.0; 12];
        for (v, s) in st_.iter_mut().enumerate() {
            *s = st_tt * tt_[v] + st_dt * dt_[v] + st_ut * ut_[v];
        }
        st_[MS] += st_ms0;
        st_[RE] += st_re0;

        self.s2.ampl = 0.0;
        self.s2.s = st;

        // Turbulent secondary "T" variables using the proper CTI.
        self.blvar(Regime::Turbulent);

        // "1" variables become the "T" variables, "2" variables are reset to
        // their saved turbulent values.
        self.s1 = self.s2;
        self.s2 = c2sav;

        // XT-X2 midpoint CFM and system for dCt, dTh, dDs, dUe, dXi.
        self.blmid(Regime::Turbulent);
        self.bldif(Regime::Turbulent);

        let vs = self.sys;
        let chain = |k: usize, var: usize| {
            vs.vs1[k][0] * st_[var]
                + vs.vs1[k][1] * tt_[var]
                + vs.vs1[k][2] * dt_[var]
                + vs.vs1[k][3] * ut_[var]
                + vs.vs1[k][4] * xt_[var]
        };
        let mut btrez = [0.0; 4];
        let mut btm = [0.0; 4];
        let mut btr = [0.0; 4];
        let mut btx = [0.0; 4];
        let mut bt1 = [[0.0; 5]; 4];
        let mut bt2 = [[0.0; 5]; 4];
        for k in 0..3 {
            btrez[k] = vs.vsrez[k];
            btm[k] = vs.vsm[k] + chain(k, MS);
            btr[k] = vs.vsr[k] + chain(k, RE);
            btx[k] = vs.vsx[k] + chain(k, XF);

            bt1[k][0] = chain(k, A1);
            bt1[k][1] = chain(k, T1);
            bt1[k][2] = chain(k, D1);
            bt1[k][3] = chain(k, U1);
            bt1[k][4] = chain(k, X1);

            bt2[k][0] = vs.vs2[k][0];
            bt2[k][1] = vs.vs2[k][1] + chain(k, T2);
            bt2[k][2] = vs.vs2[k][2] + chain(k, D2);
            bt2[k][3] = vs.vs2[k][3] + chain(k, U2);
            bt2[k][4] = vs.vs2[k][4] + chain(k, X2);
        }

        // Laminar and turbulent parts added up in terms of the honest "1"
        // and "2" variables.
        let sys = &mut self.sys;
        sys.vsrez[0] = btrez[0];
        sys.vsrez[1] = blrez[1] + btrez[1];
        sys.vsrez[2] = blrez[2] + btrez[2];
        sys.vsm[0] = btm[0];
        sys.vsm[1] = blm[1] + btm[1];
        sys.vsm[2] = blm[2] + btm[2];
        sys.vsr[0] = btr[0];
        sys.vsr[1] = blr[1] + btr[1];
        sys.vsr[2] = blr[2] + btr[2];
        sys.vsx[0] = btx[0];
        sys.vsx[1] = blx[1] + btx[1];
        sys.vsx[2] = blx[2] + btx[2];
        for l in 0..5 {
            sys.vs1[0][l] = bt1[0][l];
            sys.vs2[0][l] = bt2[0][l];
            sys.vs1[1][l] = bl1[1][l] + bt1[1][l];
            sys.vs2[1][l] = bl2[1][l] + bt2[1][l];
            sys.vs1[2][l] = bl1[2][l] + bt1[2][l];
            sys.vs2[2][l] = bl2[2][l] + bt2[2][l];
        }

        // Restore the clobbered "1" quantities.
        self.s1 = c1sav;
    }
}

#[cfg(test)]
mod tests;
