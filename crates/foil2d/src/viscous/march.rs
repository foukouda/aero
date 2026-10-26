//! Station-by-station marching of the boundary layers and wake (XFOIL
//! `xbl.f`: `MRCHUE`, `MRCHDU`, `DSLIM`).
//!
//! Both marches solve, at each station, the three interval equations plus
//! a fourth one that closes the system: a prescribed edge velocity (direct
//! mode), a prescribed `Hk` (inverse mode, `MRCHUE` past separation) or a
//! prescribed combination of both (`MRCHDU`), which avoids the Goldstein
//! singularity at separation.

// Index loops mirror XFOIL's DO loops.
#![allow(clippy::needless_range_loop)]

use super::blsys::{Regime, gauss4};
use super::closure::hkin;
use super::session::Session;

/// XFOIL `DSLIM`: raises δ* so that `Hk ≥ hklim`.
pub(crate) fn dslim(dstr: f64, thet: f64, msq: f64, hklim: f64) -> f64 {
    let h = dstr / thet;
    let (hk, hk_h, _) = hkin(h, msq);
    let dh = (hklim - hk).max(0.0) / hk_h;
    dstr + dh * thet
}

/// Variables of the trailing-edge "station" that starts the wake: summed
/// momentum and displacement thicknesses (base thickness included) and the
/// θ-weighted shear coefficient.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct WakeStart {
    pub cte: f64,
    pub tte: f64,
    pub dte: f64,
}

impl Session<'_> {
    /// Edge Mach number squared for the incompressible edge speed `uei`.
    pub(crate) fn edge_msq(&self, uei: f64) -> f64 {
        let f = &self.bl.flow;
        uei * uei * f.hstinv / (f.gm1 * (1.0 - 0.5 * uei * uei * f.hstinv))
    }

    pub(crate) fn wake_start(&self) -> WakeStart {
        let (top, bot) = (&self.sides[0], &self.sides[1]);
        let (t1, t2) = (top.thet[top.iblte], bot.thet[bot.iblte]);
        let tte = t1 + t2;
        WakeStart {
            tte,
            dte: top.dstr[top.iblte] + bot.dstr[bot.iblte] + self.geo().te_panel.ante,
            cte: (top.ctau[top.iblte] * t1 + bot.ctau[bot.iblte] * t2) / tte,
        }
    }

    /// Wake gap thickness at station `ibl` of side `is`.
    pub(crate) fn dswak(&self, is: usize, ibl: usize) -> f64 {
        let side = &self.sides[is];
        if ibl > side.iblte {
            self.wgap[ibl - side.iblte]
        } else {
            0.0
        }
    }

    /// Stores the variables of the current station 2 at station `ibl`
    /// (the end of the marching loops).
    #[allow(clippy::too_many_arguments)]
    fn store_station(
        &mut self,
        is: usize,
        ibl: usize,
        ami: f64,
        cti: f64,
        thi: f64,
        dsi: f64,
        uei: f64,
    ) {
        let s2 = self.bl.s2;
        let side = &mut self.sides[is];
        side.ctau[ibl] = if ibl < side.itran { ami } else { cti };
        side.thet[ibl] = thi;
        side.dstr[ibl] = dsi;
        side.uedg[ibl] = uei;
        side.mass[ibl] = dsi * uei;
        side.tau[ibl] = 0.5 * s2.r * s2.u * s2.u * s2.cf;
        side.dis[ibl] = s2.r * s2.u * s2.u * s2.u * s2.di * s2.hs * 0.5;
        side.ctq[ibl] = s2.cq;
        side.delt[ibl] = s2.de;
        side.tstr[ibl] = s2.hs * s2.t;
    }

    /// The transition check of the marching loops: `TRCHEK` and the new
    /// transition station.
    fn march_trchek(&mut self, is: usize, ibl: usize, ami: &mut f64) {
        self.bl.trchek();
        *ami = self.bl.s2.ampl;
        self.sides[is].itran = if self.bl.tran { ibl } else { ibl + 2 };
    }

    /// Secondary variables of a station whose Newton iteration failed
    /// (label 109 of the marching routines).
    fn set_extrapolated(&mut self, is: usize, ibl: usize, wake: bool) {
        let itran = self.sides[is].itran;
        if ibl < itran {
            self.bl.blvar(Regime::Laminar);
        }
        if ibl >= itran {
            self.bl.blvar(Regime::Turbulent);
        }
        if wake {
            self.bl.blvar(Regime::Wake);
        }
        if ibl < itran {
            self.bl.blmid(Regime::Laminar);
        }
        if ibl >= itran {
            self.bl.blmid(Regime::Turbulent);
        }
        if wake {
            self.bl.blmid(Regime::Wake);
        }
    }

    /// XFOIL `MRCHUE`: marches the boundary layers and the wake with the
    /// current edge velocities, switching to inverse mode (prescribed `Hk`)
    /// at separation. Used to initialize the boundary layer.
    pub(crate) fn mrchue(&mut self) {
        // Shape parameters for the separation criteria.
        const HLMAX: f64 = 3.8;
        const HTMAX: f64 = 2.5;

        for is in 0..2 {
            self.bl.amcrit = self.par.ncrit[is];
            self.bl.xiforc = self.xifset(is);

            // Similarity station initialized with Thwaites' formula.
            let (iblte, nbl) = (self.sides[is].iblte, self.sides[is].nbl);
            let xsi = self.sides[is].xssi[2];
            let uei = self.sides[is].uedg[2];
            let bule = 1.0;
            self.bl.bule = bule;
            let ucon = uei / xsi.powf(bule);
            let tsq =
                0.45 / (ucon * (5.0 * bule + 1.0) * self.bl.flow.reybl) * xsi.powf(1.0 - bule);
            let mut thi = tsq.sqrt();
            let mut dsi = 2.2 * thi;
            let mut ami = 0.0;
            // Initial Ctau of the first turbulent station.
            let mut cti = 0.03;

            self.bl.tran = false;
            self.bl.turb = false;
            self.sides[is].itran = iblte;

            let mut ws = WakeStart::default();
            for ibl in 2..=nbl {
                let ibm = ibl - 1;
                self.bl.simi = ibl == 2;
                self.bl.wake = ibl > iblte;
                let simi = self.bl.simi;
                let wake = self.bl.wake;

                let xsi = self.sides[is].xssi[ibl];
                let mut uei = self.sides[is].uedg[ibl];
                let dswaki = self.dswak(is, ibl);

                let mut direct = true;
                let mut converged = false;
                let mut dmax = 0.0;
                let mut hmax = 0.0;
                let mut htarg = 0.0;

                for _itbl in 0..25 {
                    self.bl.blprv(xsi, ami, cti, thi, dsi, dswaki, uei);
                    self.bl.blkin();

                    // Transition check.
                    if !simi && !self.bl.turb {
                        self.bl.trchek();
                        ami = self.bl.s2.ampl;
                        if self.bl.tran {
                            self.sides[is].itran = ibl;
                            if cti <= 0.0 {
                                cti = 0.03;
                                self.bl.s2.s = cti;
                            }
                        } else {
                            self.sides[is].itran = ibl + 2;
                        }
                    }
                    let itran = self.sides[is].itran;

                    if ibl == iblte + 1 {
                        ws = self.wake_start();
                        self.bl.tesys(ws.cte, ws.tte, ws.dte);
                    } else {
                        self.bl.blsys();
                    }

                    if direct {
                        // Direct mode: dUe = 0 in the empty fourth line.
                        let sys = &mut self.bl.sys;
                        sys.vs2[3][0] = 0.0;
                        sys.vs2[3][1] = 0.0;
                        sys.vs2[3][2] = 0.0;
                        sys.vs2[3][3] = 1.0;
                        sys.vsrez[3] = 0.0;
                        gauss4(&mut sys.vs2, &mut sys.vsrez);
                        let rez = sys.vsrez;

                        dmax = (rez[1] / thi).abs().max((rez[2] / dsi).abs());
                        if ibl < itran {
                            dmax = dmax.max((rez[0] / 10.0).abs());
                        }
                        if ibl >= itran {
                            dmax = dmax.max((rez[0] / cti).abs());
                        }
                        let rlx = if dmax > 0.3 { 0.3 / dmax } else { 1.0 };

                        // Is direct mode still applicable?
                        if ibl != iblte + 1 {
                            let msq = self.edge_msq(uei);
                            let htest = (dsi + rlx * rez[2]) / (thi + rlx * rez[1]);
                            let (hktest, _, _) = hkin(htest, msq);
                            if ibl < itran {
                                hmax = HLMAX;
                            }
                            if ibl >= itran {
                                hmax = HTMAX;
                            }
                            direct = hktest < hmax;
                        }

                        if direct {
                            if ibl >= itran {
                                cti += rlx * rez[0];
                            }
                            thi += rlx * rez[1];
                            dsi += rlx * rez[2];
                        } else {
                            // Prescribed Hk for the inverse calculation.
                            let (s1, s2) = (&self.bl.s1, &self.bl.s2);
                            let (hk1, x1, x2, t1) = (s1.hk, s1.x, s2.x, s1.t);
                            if ibl < itran {
                                // Laminar: relatively slow increase of Hk.
                                htarg = hk1 + 0.03 * (x2 - x1) / t1;
                            } else if ibl == itran {
                                // Transition interval: weighted laminar and
                                // turbulent cases.
                                let xt = self.bl.tr.xt;
                                htarg = hk1 + (0.03 * (xt - x1) - 0.15 * (x2 - xt)) / t1;
                            } else if wake {
                                // Turbulent wake: asymptotic behaviour with an
                                // approximate backward Euler step.
                                let cnst = 0.03 * (x2 - x1) / t1;
                                let mut hk2 = hk1;
                                for _ in 0..3 {
                                    hk2 -= (hk2 + cnst * (hk2 - 1.0).powi(3) - hk1)
                                        / (1.0 + 3.0 * cnst * (hk2 - 1.0).powi(2));
                                }
                                self.bl.s2.hk = hk2;
                                htarg = hk2;
                            } else {
                                // Turbulent: relatively fast decrease of Hk.
                                htarg = hk1 - 0.15 * (x2 - x1) / t1;
                            }
                            // Limit the prescribed Hk to something reasonable.
                            htarg = if wake {
                                htarg.max(1.01)
                            } else {
                                htarg.max(hmax)
                            };
                            // Try again with the prescribed Hk.
                            continue;
                        }
                    } else {
                        // Inverse mode: Hk forced to the prescribed value.
                        let s2 = self.bl.s2;
                        let sys = &mut self.bl.sys;
                        sys.vs2[3][0] = 0.0;
                        sys.vs2[3][1] = s2.hk_t;
                        sys.vs2[3][2] = s2.hk_d;
                        sys.vs2[3][3] = s2.hk_u;
                        sys.vsrez[3] = htarg - s2.hk;
                        gauss4(&mut sys.vs2, &mut sys.vsrez);
                        let rez = sys.vsrez;

                        dmax = (rez[1] / thi)
                            .abs()
                            .max((rez[2] / dsi).abs())
                            .max((rez[3] / uei).abs());
                        if ibl >= itran {
                            dmax = dmax.max((rez[0] / cti).abs());
                        }
                        let rlx = if dmax > 0.3 { 0.3 / dmax } else { 1.0 };
                        if ibl >= itran {
                            cti += rlx * rez[0];
                        }
                        thi += rlx * rez[1];
                        dsi += rlx * rez[2];
                        uei += rlx * rez[3];
                    }

                    // Eliminate absurd transients.
                    if ibl >= self.sides[is].itran {
                        cti = cti.clamp(0.0000001, 0.30);
                    }
                    let hklim = if ibl <= iblte { 1.02 } else { 1.00005 };
                    let msq = self.edge_msq(uei);
                    dsi = dslim(dsi - dswaki, thi, msq, hklim) + dswaki;

                    if dmax <= 1.0e-5 {
                        converged = true;
                        break;
                    }
                }

                if !converged {
                    // "MRCHUE: Convergence failed". A large residual means
                    // the solution is garbage: extrapolate instead.
                    if dmax > 0.1 && ibl > 3 {
                        let side = &self.sides[is];
                        if ibl <= iblte {
                            let r = (side.xssi[ibl] / side.xssi[ibm]).powf(0.5);
                            thi = side.thet[ibm] * r;
                            dsi = side.dstr[ibm] * r;
                        } else if ibl == iblte + 1 {
                            cti = ws.cte;
                            thi = ws.tte;
                            dsi = ws.dte;
                        } else {
                            thi = side.thet[ibm];
                            let ratlen =
                                (side.xssi[ibl] - side.xssi[ibm]) / (10.0 * side.dstr[ibm]);
                            dsi = (side.dstr[ibm] + thi * ratlen) / (1.0 + ratlen);
                        }
                        if ibl == side.itran {
                            cti = 0.05;
                        }
                        if ibl > side.itran {
                            cti = side.ctau[ibm];
                        }
                        uei = side.uedg[ibl];
                        if ibl > 2 && ibl < nbl {
                            uei = 0.5 * (side.uedg[ibl - 1] + side.uedg[ibl + 1]);
                        }
                    }
                    self.bl.blprv(xsi, ami, cti, thi, dsi, dswaki, uei);
                    self.bl.blkin();
                    if !simi && !self.bl.turb {
                        self.march_trchek(is, ibl, &mut ami);
                    }
                    self.set_extrapolated(is, ibl, wake);
                }

                self.store_station(is, ibl, ami, cti, thi, dsi, uei);

                // "1" variables become the "2" variables for the next station.
                self.bl.blprv(xsi, ami, cti, thi, dsi, dswaki, uei);
                self.bl.blkin();
                self.bl.s1 = self.bl.s2;

                // Turbulent intervals follow the transition interval or the
                // trailing edge.
                if self.bl.tran || ibl == iblte {
                    self.bl.turb = true;
                    self.sides[is].tforce = self.bl.trforc;
                    self.sides[is].xssitr = self.bl.tr.xt;
                }
                self.bl.tran = false;

                if ibl == iblte {
                    let ws = self.wake_start();
                    thi = ws.tte;
                    dsi = ws.dte;
                }
            }
        }
    }

    /// XFOIL `MRCHDU`: marches the boundary layers and the wake in mixed
    /// mode, the edge velocity and `Hk` lying on a line quasi-normal to the
    /// natural `Ue`–`Hk` characteristic of the boundary layer, so that the
    /// Goldstein or Levy–Lees singularity is never met. Establishes the
    /// transition points before each Newton iteration.
    pub(crate) fn mrchdu(&mut self) {
        const DEPS: f64 = 5.0e-6;
        // How far Hk may deviate from the specified value.
        const SENSWT: f64 = 1000.0;
        // Locals that XFOIL carries from one station (and side) to the next.
        let mut sens = 0.0;
        let mut sennew = 0.0;
        let mut ami = 0.0;

        for is in 0..2 {
            self.bl.amcrit = self.par.ncrit[is];
            self.bl.xiforc = self.xifset(is);
            // Leading-edge pressure gradient parameter x/u du/dx.
            self.bl.bule = 1.0;

            let (iblte, nbl) = (self.sides[is].iblte, self.sides[is].nbl);
            // Old transition station.
            let itrold = self.sides[is].itran;

            self.bl.tran = false;
            self.bl.turb = false;
            self.sides[is].itran = iblte;

            let mut ws = WakeStart::default();
            let mut cti;
            for ibl in 2..=nbl {
                let ibm = ibl - 1;
                self.bl.simi = ibl == 2;
                self.bl.wake = ibl > iblte;
                let simi = self.bl.simi;
                let wake = self.bl.wake;

                // Initialize the station with the existing variables.
                let side = &self.sides[is];
                let xsi = side.xssi[ibl];
                let mut uei = side.uedg[ibl];
                let mut thi = side.thet[ibl];
                let mut dsi = side.dstr[ibl];
                if ibl < itrold {
                    ami = side.ctau[ibl];
                    cti = 0.03;
                } else {
                    cti = side.ctau[ibl];
                    if cti <= 0.0 {
                        cti = 0.03;
                    }
                }
                let dswaki = self.dswak(is, ibl);
                if ibl <= iblte {
                    dsi = (dsi - dswaki).max(1.02000 * thi) + dswaki;
                }
                if ibl > iblte {
                    dsi = (dsi - dswaki).max(1.00005 * thi) + dswaki;
                }

                let mut converged = false;
                let mut dmax = 0.0;
                let mut ueref = 0.0;
                let mut hkref = 0.0;

                for itbl in 1..=25 {
                    self.bl.blprv(xsi, ami, cti, thi, dsi, dswaki, uei);
                    self.bl.blkin();

                    if !simi && !self.bl.turb {
                        self.march_trchek(is, ibl, &mut ami);
                    }

                    if ibl == iblte + 1 {
                        ws = self.wake_start();
                        self.bl.tesys(ws.cte, ws.tte, ws.dte);
                    } else {
                        self.bl.blsys();
                    }

                    let itran = self.sides[is].itran;
                    if itbl == 1 {
                        // Baseline Ue and Hk of the Ue(Hk) relation.
                        ueref = self.bl.s2.u;
                        hkref = self.bl.s2.hk;

                        // Station was turbulent and is now laminar:
                        // extrapolate the baseline Hk.
                        if ibl < itran && ibl >= itrold {
                            let side = &self.sides[is];
                            let uem = side.uedg[ibl - 1];
                            let dsm = side.dstr[ibl - 1];
                            let thm = side.thet[ibl - 1];
                            let msq = self.edge_msq(uem);
                            hkref = hkin(dsm / thm, msq).0;
                        }

                        // Station was laminar: reinitialize or extrapolate
                        // Ctau if it is now turbulent.
                        if ibl < itrold {
                            let side = &mut self.sides[is];
                            if self.bl.tran {
                                side.ctau[ibl] = 0.03;
                            }
                            if self.bl.turb {
                                side.ctau[ibl] = side.ctau[ibl - 1];
                            }
                            if self.bl.tran || self.bl.turb {
                                cti = side.ctau[ibl];
                                self.bl.s2.s = cti;
                            }
                        }
                    }

                    let s2 = self.bl.s2;
                    if simi || ibl == iblte + 1 {
                        // Similarity station or first wake point: prescribed Ue.
                        let sys = &mut self.bl.sys;
                        sys.vs2[3][0] = 0.0;
                        sys.vs2[3][1] = 0.0;
                        sys.vs2[3][2] = 0.0;
                        sys.vs2[3][3] = s2.u_uei;
                        sys.vsrez[3] = ueref - s2.u;
                    } else {
                        // Ue–Hk characteristic slope: dUe response to a unit
                        // dHk.
                        let mut vtmp = self.bl.sys.vs2;
                        let mut vztmp = self.bl.sys.vsrez;
                        vtmp[3][0] = 0.0;
                        vtmp[3][1] = s2.hk_t;
                        vtmp[3][2] = s2.hk_d;
                        vtmp[3][3] = s2.hk_u * s2.u_uei;
                        vztmp[3] = 1.0;
                        gauss4(&mut vtmp, &mut vztmp);

                        // SENSWT × (normalized dUe/dHk).
                        sennew = SENSWT * vztmp[3] * hkref / ueref;
                        if itbl <= 5 {
                            sens = sennew;
                        } else if itbl <= 15 {
                            sens = 0.5 * (sens + sennew);
                        }

                        // Prescribed Ue–Hk combination.
                        let sys = &mut self.bl.sys;
                        sys.vs2[3][0] = 0.0;
                        sys.vs2[3][1] = s2.hk_t * hkref;
                        sys.vs2[3][2] = s2.hk_d * hkref;
                        sys.vs2[3][3] = (s2.hk_u * hkref + sens / ueref) * s2.u_uei;
                        sys.vsrez[3] =
                            -(hkref * hkref) * (s2.hk / hkref - 1.0) - sens * (s2.u / ueref - 1.0);
                    }

                    let sys = &mut self.bl.sys;
                    gauss4(&mut sys.vs2, &mut sys.vsrez);
                    let rez = sys.vsrez;

                    // Maximum changes and underrelaxation (with Ue clamp).
                    dmax = (rez[1] / thi)
                        .abs()
                        .max((rez[2] / dsi).abs())
                        .max((rez[3] / uei).abs());
                    if ibl >= itran {
                        dmax = dmax.max((rez[0] / (10.0 * cti)).abs());
                    }
                    let rlx = if dmax > 0.3 { 0.3 / dmax } else { 1.0 };

                    if ibl < itran {
                        ami += rlx * rez[0];
                    }
                    if ibl >= itran {
                        cti += rlx * rez[0];
                    }
                    thi += rlx * rez[1];
                    dsi += rlx * rez[2];
                    uei += rlx * rez[3];

                    // Eliminate absurd transients.
                    if ibl >= itran {
                        cti = cti.clamp(0.0000001, 0.30);
                    }
                    let hklim = if ibl <= iblte { 1.02 } else { 1.00005 };
                    let msq = self.edge_msq(uei);
                    dsi = dslim(dsi - dswaki, thi, msq, hklim) + dswaki;

                    if dmax <= DEPS {
                        converged = true;
                        break;
                    }
                }

                if !converged {
                    // "MRCHDU: Convergence failed".
                    if dmax > 0.1 && ibl > 3 {
                        let side = &self.sides[is];
                        if ibl <= iblte {
                            let r = (side.xssi[ibl] / side.xssi[ibm]).powf(0.5);
                            thi = side.thet[ibm] * r;
                            dsi = side.dstr[ibm] * r;
                            uei = side.uedg[ibm];
                        } else if ibl == iblte + 1 {
                            cti = ws.cte;
                            thi = ws.tte;
                            dsi = ws.dte;
                            uei = side.uedg[ibm];
                        } else {
                            thi = side.thet[ibm];
                            let ratlen =
                                (side.xssi[ibl] - side.xssi[ibm]) / (10.0 * side.dstr[ibm]);
                            dsi = (side.dstr[ibm] + thi * ratlen) / (1.0 + ratlen);
                            uei = side.uedg[ibm];
                        }
                        if ibl == side.itran {
                            cti = 0.05;
                        }
                        if ibl > side.itran {
                            cti = side.ctau[ibm];
                        }
                    }
                    self.bl.blprv(xsi, ami, cti, thi, dsi, dswaki, uei);
                    self.bl.blkin();
                    if !simi && !self.bl.turb {
                        self.march_trchek(is, ibl, &mut ami);
                    }
                    self.set_extrapolated(is, ibl, wake);
                }

                sens = sennew;

                self.store_station(is, ibl, ami, cti, thi, dsi, uei);

                self.bl.blprv(xsi, ami, cti, thi, dsi, dswaki, uei);
                self.bl.blkin();
                self.bl.s1 = self.bl.s2;

                if self.bl.tran || ibl == iblte {
                    self.bl.turb = true;
                    self.sides[is].tforce = self.bl.trforc;
                    self.sides[is].xssitr = self.bl.tr.xt;
                }
                self.bl.tran = false;
            }
        }
    }
}
