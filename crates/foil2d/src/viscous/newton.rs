//! The global Newton system of the viscous–inviscid interaction (XFOIL
//! `xbl.f`: `SETBL`, `UPDATE`; `xsolve.f`: `BLSOLV`).
//!
//! Unknowns at each boundary-layer station: `dCτ` (or `dN` in laminar
//! regions), `dθ` and the mass defect `dm = d(Ue·δ*)`. The edge velocities
//! are eliminated through the source influence matrix,
//! `Ue = Ue_inv + Σ DIJ·m`, so every equation depends on all mass defects.

// Index loops mirror XFOIL's DO loops.
#![allow(clippy::needless_range_loop)]

use super::blsys::{Flow, Regime};
use super::march::dslim;
use super::session::Session;

impl Session<'_> {
    /// System line, panel node and sign of the tangential velocity of every
    /// station of the system, in line order.
    fn system_columns(&self) -> Vec<(usize, f64)> {
        let mut cols = vec![(0, 0.0); self.sys.nsys];
        for side in &self.sides {
            for ibl in 2..=side.nbl {
                cols[side.isys[ibl]] = (side.ipan[ibl], side.vti[ibl]);
            }
        }
        cols
    }

    /// Edge velocities `Ue_inv + Σ DIJ·m` (the values `UESET` would set).
    pub(crate) fn ue_from_mass(&self) -> [Vec<f64>; 2] {
        let cols = self.system_columns();
        let mass: Vec<f64> = self.mass_by_line();
        let mut ue = [self.sides[0].uedg.clone(), self.sides[1].uedg.clone()];
        for (is, side) in self.sides.iter().enumerate() {
            for ibl in 2..=side.nbl {
                let i = side.ipan[ibl];
                let mut dui = 0.0;
                for (jv, &(j, vtj)) in cols.iter().enumerate() {
                    dui += -side.vti[ibl] * vtj * self.dij[(i, j)] * mass[jv];
                }
                ue[is][ibl] = side.uinv[ibl] + dui;
            }
        }
        ue
    }

    fn mass_by_line(&self) -> Vec<f64> {
        let mut mass = vec![0.0; self.sys.nsys];
        for side in &self.sides {
            for ibl in 2..=side.nbl {
                mass[side.isys[ibl]] = side.mass[ibl];
            }
        }
        mass
    }

    /// XFOIL `SETBL`: marches the boundary layers with the current edge
    /// velocities and displacement thicknesses, then linearizes the interval
    /// equations of every station into the global Newton system.
    pub(crate) fn setbl(&mut self) {
        // Mach and Reynolds numbers do not depend on CL here (MATYP = RETYP
        // = 1): their sensitivities to the global variable vanish.
        let re_clmr = 0.0;
        let msq_clmr = 0.0;

        self.comset();
        self.bl.flow = Flow::new(self.qinf, self.par.mach, self.par.reynolds);
        self.bl.damp2 = self.par.damp2;

        if !self.lblini {
            // Initialize the boundary layer by marching with Ue.
            self.mrchue();
            self.lblini = true;
        }
        // March with the current Ue and δ* to establish transition.
        self.mrchdu();

        let usav = self.ue_from_mass();
        let nsys = self.sys.nsys;
        let cols = self.system_columns();

        let (top, bot) = (&self.sides[0], &self.sides[1]);
        let ile1 = top.ipan[2];
        let ile2 = bot.ipan[2];
        let ite1 = top.ipan[top.iblte];
        let ite2 = bot.ipan[bot.iblte];
        let jvte1 = top.isys[top.iblte];
        let jvte2 = bot.isys[bot.iblte];
        let dule1 = top.uedg[2] - usav[0][2];
        let dule2 = bot.uedg[2] - usav[1][2];

        // Leading- and trailing-edge Ue sensitivities to all mass defects.
        let mut ule1_m = vec![0.0; nsys];
        let mut ule2_m = vec![0.0; nsys];
        let mut ute1_m = vec![0.0; nsys];
        let mut ute2_m = vec![0.0; nsys];
        for (jv, &(j, vtj)) in cols.iter().enumerate() {
            ule1_m[jv] = -top.vti[2] * vtj * self.dij[(ile1, j)];
            ule2_m[jv] = -bot.vti[2] * vtj * self.dij[(ile2, j)];
            ute1_m[jv] = -top.vti[top.iblte] * vtj * self.dij[(ite1, j)];
            ute2_m[jv] = -bot.vti[bot.iblte] * vtj * self.dij[(ite2, j)];
        }
        self.sides[0].tindex = 0.0;
        self.sides[1].tindex = 0.0;

        let mut u1_m = vec![0.0; nsys];
        let mut u2_m = vec![0.0; nsys];
        let mut d1_m = vec![0.0; nsys];
        let mut d2_m = vec![0.0; nsys];
        // XFOIL locals carried from one station (and side) to the next.
        let mut ami = 0.0;
        let mut cti = 0.0;

        for is in 0..2 {
            // No station "1" at the similarity station.
            u1_m.iter_mut().for_each(|v| *v = 0.0);
            d1_m.iter_mut().for_each(|v| *v = 0.0);
            let mut due1 = 0.0;
            let mut dds1 = 0.0;

            self.bl.bule = 1.0;
            self.bl.amcrit = self.par.ncrit[is];
            self.bl.xiforc = self.xifset(is);
            self.bl.tran = false;
            self.bl.turb = false;

            let (iblte, nbl) = (self.sides[is].iblte, self.sides[is].nbl);
            // Sensitivities of ξ to the leading-edge velocities.
            let (xi_ule1, xi_ule2) = if is == 0 {
                (self.sst_go, -self.sst_gp)
            } else {
                (-self.sst_go, self.sst_gp)
            };

            // Coefficients relating the start of the wake to both
            // trailing-edge stations.
            let mut cte_cte1 = 0.0;
            let mut cte_cte2 = 0.0;
            let mut cte_tte1 = 0.0;
            let mut cte_tte2 = 0.0;
            let tte_tte1 = 1.0;
            let tte_tte2 = 1.0;

            for ibl in 2..=nbl {
                let side = &self.sides[is];
                let iv = side.isys[ibl];
                self.bl.simi = ibl == 2;
                self.bl.wake = ibl > iblte;
                self.bl.tran = ibl == side.itran;
                self.bl.turb = ibl > side.itran;

                let i = side.ipan[ibl];
                let xsi = side.xssi[ibl];
                if ibl < side.itran {
                    ami = side.ctau[ibl];
                }
                if ibl >= side.itran {
                    cti = side.ctau[ibl];
                }
                let uei = side.uedg[ibl];
                let thi = side.thet[ibl];
                let mdi = side.mass[ibl];
                let dsi = mdi / uei;
                let dswaki = self.dswak(is, ibl);

                // Derivatives of DSI (= D2).
                let d2_m2 = 1.0 / uei;
                let d2_u2 = -dsi / uei;
                let vti = side.vti[ibl];
                for (jv, &(j, vtj)) in cols.iter().enumerate() {
                    u2_m[jv] = -vti * vtj * self.dij[(i, j)];
                    d2_m[jv] = d2_u2 * u2_m[jv];
                }
                d2_m[iv] += d2_m2;

                // "Forced" changes due to the mismatch between UEDG and
                // USAV = UINV + DIJ·MASS.
                let due2 = side.uedg[ibl] - usav[is][ibl];
                let dds2 = d2_u2 * due2;

                self.bl.blprv(xsi, ami, cti, thi, dsi, dswaki, uei);
                self.bl.blkin();

                // Transition check.
                if self.bl.tran {
                    self.bl.trchek();
                    ami = self.bl.s2.ampl;
                }
                // (XFOIL prints "SETBL: Xtr???" if the transition interval
                // has no transition any more.)

                if ibl == iblte + 1 {
                    // Start of the wake: trailing-edge base thickness added
                    // to δ*.
                    let ws = self.wake_start();
                    self.bl.tesys(ws.cte, ws.tte, ws.dte);

                    let (top, bot) = (&self.sides[0], &self.sides[1]);
                    let (it1, it2) = (top.iblte, bot.iblte);
                    let dte_mte1 = 1.0 / top.uedg[it1];
                    let dte_ute1 = -top.dstr[it1] / top.uedg[it1];
                    let dte_mte2 = 1.0 / bot.uedg[it2];
                    let dte_ute2 = -bot.dstr[it2] / bot.uedg[it2];
                    cte_cte1 = top.thet[it1] / ws.tte;
                    cte_cte2 = bot.thet[it2] / ws.tte;
                    cte_tte1 = (top.ctau[it1] - ws.cte) / ws.tte;
                    cte_tte2 = (bot.ctau[it2] - ws.cte) / ws.tte;

                    // D1 depends on both trailing-edge δ*.
                    for jv in 0..nsys {
                        d1_m[jv] = dte_ute1 * ute1_m[jv] + dte_ute2 * ute2_m[jv];
                    }
                    d1_m[jvte1] += dte_mte1;
                    d1_m[jvte2] += dte_mte2;

                    due1 = 0.0;
                    dds1 = dte_ute1 * (top.uedg[it1] - usav[0][it1])
                        + dte_ute2 * (bot.uedg[it2] - usav[1][it2]);
                } else {
                    self.bl.blsys();
                }

                // Wall shear etc. for the outputs.
                let s2 = self.bl.s2;
                let side = &mut self.sides[is];
                side.tau[ibl] = 0.5 * s2.r * s2.u * s2.u * s2.cf;
                side.dis[ibl] = s2.r * s2.u * s2.u * s2.u * s2.di * s2.hs * 0.5;
                side.ctq[ibl] = s2.cq;
                side.delt[ibl] = s2.de;

                // Stuff the interval system into the global Jacobian.
                let vs = self.bl.sys;
                let sys = &mut self.sys;
                for k in 0..3 {
                    let xi_coef = vs.vs1[k][4] + vs.vs2[k][4] + vs.vsx[k];
                    let row = sys.m(k, 0, iv);
                    for jv in 0..nsys {
                        sys.vm[row + jv] = vs.vs1[k][2] * d1_m[jv]
                            + vs.vs1[k][3] * u1_m[jv]
                            + vs.vs2[k][2] * d2_m[jv]
                            + vs.vs2[k][3] * u2_m[jv]
                            + xi_coef * (xi_ule1 * ule1_m[jv] + xi_ule2 * ule2_m[jv]);
                    }
                    sys.vb[iv][k] = [vs.vs1[k][0], vs.vs1[k][1]];
                    sys.va[iv][k] = [vs.vs2[k][0], vs.vs2[k][1]];
                    sys.vdel[iv][k][1] = vs.vsr[k] * re_clmr + vs.vsm[k] * msq_clmr;
                    sys.vdel[iv][k][0] = vs.vsrez[k]
                        + (vs.vs1[k][3] * due1 + vs.vs1[k][2] * dds1)
                        + (vs.vs2[k][3] * due2 + vs.vs2[k][2] * dds2)
                        + xi_coef * (xi_ule1 * dule1 + xi_ule2 * dule2);
                }
                if ibl == iblte + 1 {
                    // Coefficients for TTE, DTE, etc.
                    for k in 0..3 {
                        sys.vz[k][0] = vs.vs1[k][0] * cte_cte1;
                        sys.vz[k][1] = vs.vs1[k][0] * cte_tte1 + vs.vs1[k][1] * tte_tte1;
                        sys.vb[iv][k][0] = vs.vs1[k][0] * cte_cte2;
                        sys.vb[iv][k][1] = vs.vs1[k][0] * cte_tte2 + vs.vs1[k][1] * tte_tte2;
                    }
                }

                // Turbulent intervals follow the transition interval.
                if self.bl.tran {
                    self.bl.turb = true;
                    let xt = self.bl.tr.xt;
                    let str = if is == 0 {
                        self.sst - xt
                    } else {
                        self.sst + xt
                    };
                    let (xoc, yoc) = self.chord_coordinates(str);
                    let side = &mut self.sides[is];
                    side.itran = ibl;
                    side.tforce = self.bl.trforc;
                    side.xssitr = xt;
                    side.xoctr = xoc;
                    side.yoctr = yoc;
                }
                if ibl + 1 > nbl {
                    // End of the side: XFOIL prints the transition point.
                    let side = &self.sides[is];
                    let (forced, xoc, itr) = (side.tforce, side.xoctr, side.itran);
                    self.log(|| {
                        format!(
                            " Side{:2}{}transition at x/c = {xoc:7.4}{itr:5}",
                            is + 1,
                            if forced { " forced " } else { "  free  " }
                        )
                    });
                }
                self.bl.tran = false;

                if ibl == iblte {
                    // Station 2 at the trailing edge gets the wake
                    // correlations for the next interval.
                    self.bl.turb = true;
                    self.bl.wake = true;
                    self.bl.blvar(Regime::Wake);
                    self.bl.blmid(Regime::Wake);
                }

                std::mem::swap(&mut u1_m, &mut u2_m);
                std::mem::swap(&mut d1_m, &mut d2_m);
                due1 = due2;
                dds1 = dds2;

                let (x1, x2, xt) = (self.bl.s1.x, self.bl.s2.x, self.bl.tr.xt);
                let side = &mut self.sides[is];
                if ibl == side.itran && x2 > x1 {
                    let ist = self.ist as f64 + 1.0;
                    let itran = side.itran as f64;
                    side.tindex = if is == 0 {
                        ist - itran + 3.0 - (xt - x1) / (x2 - x1)
                    } else {
                        ist + itran - 2.0 + (xt - x1) / (x2 - x1)
                    };
                }

                // Variables of the next interval.
                self.bl.s1 = self.bl.s2;
            }
        }
    }

    /// XFOIL `BLSOLV`: block elimination of the global Newton system. The
    /// mass-influence terms below the diagonal smaller than `VACCEL` are
    /// dropped, as in XFOIL. The solution replaces `vdel`.
    pub(crate) fn blsolv(&mut self) {
        let ivte1 = self.sides[0].isys[self.sides[0].iblte];
        let ivz = self.sides[1].isys[self.sides[1].iblte + 1];
        let s = &self.geo().s;
        let span = s[self.n - 1] - s[0];
        let vacc1 = self.par.vaccel;
        let vacc2 = self.par.vaccel * 2.0 / span;
        let vacc3 = self.par.vaccel * 2.0 / span;

        let sys = &mut self.sys;
        let nsys = sys.nsys;
        let m = |k: usize, l: usize, iv: usize| (iv * 3 + k) * nsys + l;
        let (va, vb, vm, vdel, vz) = (&mut sys.va, &sys.vb, &mut sys.vm, &mut sys.vdel, sys.vz);

        for iv in 0..nsys {
            let ivp = iv + 1;

            // Invert the VA(IV) block. Normalize the first row.
            let pivot = 1.0 / va[iv][0][0];
            va[iv][0][1] *= pivot;
            for l in iv..nsys {
                vm[m(0, l, iv)] *= pivot;
            }
            vdel[iv][0][0] *= pivot;
            vdel[iv][0][1] *= pivot;

            // Eliminate the lower first column of the VA block.
            for k in 1..3 {
                let vtmp = va[iv][k][0];
                va[iv][k][1] -= vtmp * va[iv][0][1];
                for l in iv..nsys {
                    vm[m(k, l, iv)] -= vtmp * vm[m(0, l, iv)];
                }
                vdel[iv][k][0] -= vtmp * vdel[iv][0][0];
                vdel[iv][k][1] -= vtmp * vdel[iv][0][1];
            }

            // Normalize the second row.
            let pivot = 1.0 / va[iv][1][1];
            for l in iv..nsys {
                vm[m(1, l, iv)] *= pivot;
            }
            vdel[iv][1][0] *= pivot;
            vdel[iv][1][1] *= pivot;

            // Eliminate the lower second column of the VA block.
            let vtmp = va[iv][2][1];
            for l in iv..nsys {
                vm[m(2, l, iv)] -= vtmp * vm[m(1, l, iv)];
            }
            vdel[iv][2][0] -= vtmp * vdel[iv][1][0];
            vdel[iv][2][1] -= vtmp * vdel[iv][1][1];

            // Normalize the third row.
            let pivot = 1.0 / vm[m(2, iv, iv)];
            for l in ivp..nsys {
                vm[m(2, l, iv)] *= pivot;
            }
            vdel[iv][2][0] *= pivot;
            vdel[iv][2][1] *= pivot;

            // Eliminate the upper third column of the VA block.
            let vtmp1 = vm[m(0, iv, iv)];
            let vtmp2 = vm[m(1, iv, iv)];
            for l in ivp..nsys {
                vm[m(0, l, iv)] -= vtmp1 * vm[m(2, l, iv)];
                vm[m(1, l, iv)] -= vtmp2 * vm[m(2, l, iv)];
            }
            vdel[iv][0][0] -= vtmp1 * vdel[iv][2][0];
            vdel[iv][1][0] -= vtmp2 * vdel[iv][2][0];
            vdel[iv][0][1] -= vtmp1 * vdel[iv][2][1];
            vdel[iv][1][1] -= vtmp2 * vdel[iv][2][1];

            // Eliminate the upper second column of the VA block.
            let vtmp = va[iv][0][1];
            for l in ivp..nsys {
                vm[m(0, l, iv)] -= vtmp * vm[m(1, l, iv)];
            }
            vdel[iv][0][0] -= vtmp * vdel[iv][1][0];
            vdel[iv][0][1] -= vtmp * vdel[iv][1][1];

            if ivp == nsys {
                break;
            }

            // Eliminate the VB(IV+1) block, rows 1 to 3.
            for k in 0..3 {
                let vtmp1 = vb[ivp][k][0];
                let vtmp2 = vb[ivp][k][1];
                let vtmp3 = vm[m(k, iv, ivp)];
                for l in ivp..nsys {
                    vm[m(k, l, ivp)] -=
                        vtmp1 * vm[m(0, l, iv)] + vtmp2 * vm[m(1, l, iv)] + vtmp3 * vm[m(2, l, iv)];
                }
                for c in 0..2 {
                    vdel[ivp][k][c] -=
                        vtmp1 * vdel[iv][0][c] + vtmp2 * vdel[iv][1][c] + vtmp3 * vdel[iv][2][c];
                }
            }

            if iv == ivte1 {
                // Eliminate the VZ block.
                for k in 0..3 {
                    let vtmp1 = vz[k][0];
                    let vtmp2 = vz[k][1];
                    for l in ivp..nsys {
                        vm[m(k, l, ivz)] -= vtmp1 * vm[m(0, l, iv)] + vtmp2 * vm[m(1, l, iv)];
                    }
                    for c in 0..2 {
                        vdel[ivz][k][c] -= vtmp1 * vdel[iv][0][c] + vtmp2 * vdel[iv][1][c];
                    }
                }
            }

            if ivp + 1 == nsys {
                continue;
            }

            // Eliminate the lower VM column.
            for kv in iv + 2..nsys {
                let vtmp1 = vm[m(0, iv, kv)];
                let vtmp2 = vm[m(1, iv, kv)];
                let vtmp3 = vm[m(2, iv, kv)];
                if vtmp1.abs() > vacc1 {
                    for l in ivp..nsys {
                        vm[m(0, l, kv)] -= vtmp1 * vm[m(2, l, iv)];
                    }
                    vdel[kv][0][0] -= vtmp1 * vdel[iv][2][0];
                    vdel[kv][0][1] -= vtmp1 * vdel[iv][2][1];
                }
                if vtmp2.abs() > vacc2 {
                    for l in ivp..nsys {
                        vm[m(1, l, kv)] -= vtmp2 * vm[m(2, l, iv)];
                    }
                    vdel[kv][1][0] -= vtmp2 * vdel[iv][2][0];
                    vdel[kv][1][1] -= vtmp2 * vdel[iv][2][1];
                }
                if vtmp3.abs() > vacc3 {
                    for l in ivp..nsys {
                        vm[m(2, l, kv)] -= vtmp3 * vm[m(2, l, iv)];
                    }
                    vdel[kv][2][0] -= vtmp3 * vdel[iv][2][0];
                    vdel[kv][2][1] -= vtmp3 * vdel[iv][2][1];
                }
            }
        }

        // Back substitution: eliminate the upper VM columns.
        for iv in (1..nsys).rev() {
            for c in 0..2 {
                let vtmp = vdel[iv][2][c];
                for kv in (0..iv).rev() {
                    vdel[kv][0][c] -= vm[m(0, iv, kv)] * vtmp;
                    vdel[kv][1][c] -= vm[m(1, iv, kv)] * vtmp;
                    vdel[kv][2][c] -= vm[m(2, iv, kv)] * vtmp;
                }
            }
        }
    }

    /// XFOIL `UPDATE`: adds the Newton changes to the boundary-layer
    /// variables, underrelaxed if they are too large, and measures them
    /// (`rmsbl`). With a prescribed α the global variable is CL, whose
    /// change also limits the relaxation.
    pub(crate) fn update(&mut self) {
        // Maximum CL change per iteration (fixed Mach number: MATYP = 1).
        const DCLMAX: f64 = 0.5;
        const DCLMIN: f64 = -0.5;
        const GAMM1: f64 = 0.4;

        let minf = self.par.mach;
        let qinf = self.qinf;
        let hstinv = GAMM1 * (minf / qinf).powi(2) / (1.0 + 0.5 * GAMM1 * minf * minf);

        // New Ue without underrelaxation, and its sensitivity to the global
        // variable.
        let cols = self.system_columns();
        let nsys = self.sys.nsys;
        let mut mplus = vec![0.0; nsys];
        let mut mac = vec![0.0; nsys];
        for side in &self.sides {
            for ibl in 2..=side.nbl {
                let jv = side.isys[ibl];
                mplus[jv] = side.mass[ibl] + self.sys.vdel[jv][2][0];
                mac[jv] = -self.sys.vdel[jv][2][1];
            }
        }
        let mut unew = [
            vec![0.0; self.sides[0].uedg.len()],
            vec![0.0; self.sides[1].uedg.len()],
        ];
        let mut u_ac = unew.clone();
        for (is, side) in self.sides.iter().enumerate() {
            for ibl in 2..=side.nbl {
                let i = side.ipan[ibl];
                let mut dui = 0.0;
                let mut dui_ac = 0.0;
                for (jv, &(j, vtj)) in cols.iter().enumerate() {
                    let ue_m = -side.vti[ibl] * vtj * self.dij[(i, j)];
                    dui += ue_m * mplus[jv];
                    dui_ac += ue_m * mac[jv];
                }
                // UINV does not depend on the global variable CL.
                unew[is][ibl] = side.uinv[ibl] + dui;
                u_ac[is][ibl] = dui_ac;
            }
        }

        // New panel speeds.
        let n = self.n;
        let mut qnew = vec![0.0; n];
        let mut q_ac = vec![0.0; n];
        for (is, side) in self.sides.iter().enumerate() {
            for ibl in 2..=side.iblte {
                let i = side.ipan[ibl];
                qnew[i] = side.vti[ibl] * unew[is][ibl];
                q_ac[i] = side.vti[ibl] * u_ac[is][ibl];
            }
        }

        // New CL from the new speeds.
        let (sa, ca) = self.alfa.sin_cos();
        let beta = (1.0 - minf * minf).sqrt();
        let beta_msq = -0.5 / beta;
        let bfac = 0.5 * minf * minf / (1.0 + beta);
        let bfac_msq = 0.5 / (1.0 + beta) - bfac / (1.0 + beta) * beta_msq;
        let pressure = |q: f64, q_ac: f64| {
            let cginc = 1.0 - (q / qinf).powi(2);
            let cpg = cginc / (beta + bfac * cginc);
            let cpg_ms = -cpg / (beta + bfac * cginc) * (beta_msq + bfac_msq * cginc);
            let cpi_q = -2.0 * q / (qinf * qinf);
            let cpc_cpi = (1.0 - bfac * cpg) / (beta + bfac * cginc);
            (cpg, cpg_ms, cpc_cpi * cpi_q * q_ac)
        };
        let geo = self.geo();
        let (x, y) = (&geo.x, &geo.y);
        let mut clnew = 0.0;
        let mut cl_ms = 0.0;
        let mut cl_ac = 0.0;
        let (mut cpg1, mut cpg1_ms, mut cpg1_ac) = pressure(qnew[0], q_ac[0]);
        for i in 0..n {
            let ip = if i + 1 == n { 0 } else { i + 1 };
            let (cpg2, cpg2_ms, cpg2_ac) = pressure(qnew[ip], q_ac[ip]);
            let dx = (x[ip] - x[i]) * ca + (y[ip] - y[i]) * sa;
            clnew += dx * 0.5 * (cpg2 + cpg1);
            cl_ms += dx * 0.5 * (cpg2_ms + cpg1_ms);
            cl_ac += dx * 0.5 * (cpg2_ac + cpg1_ac);
            cpg1 = cpg2;
            cpg1_ms = cpg2_ms;
            cpg1_ac = cpg2_ac;
        }

        // α prescribed: the global variable is CL, the Mach number does not
        // depend on it (MINF_CL = 0).
        let minf_cl = 0.0;
        let mut rlx = 1.0;
        let dac = (clnew - self.cl) / (1.0 - cl_ac - cl_ms * 2.0 * minf * minf_cl);
        if rlx * dac > DCLMAX {
            rlx = DCLMAX / dac;
        }
        if rlx * dac < DCLMIN {
            rlx = DCLMIN / dac;
        }

        // Changes of the boundary-layer variables and underrelaxation.
        const DHI: f64 = 1.5;
        const DLO: f64 = -0.5;
        let mut rmsbl = 0.0;
        let mut rmxbl: f64 = 0.0;
        let (mut vmxbl, mut imxbl, mut ismxbl) = (' ', 0, 0);
        let vdel = &self.sys.vdel;
        let deltas = |side: &super::session::Side, is: usize, ibl: usize| {
            let iv = side.isys[ibl];
            let dctau = vdel[iv][0][0] - dac * vdel[iv][0][1];
            let dthet = vdel[iv][1][0] - dac * vdel[iv][1][1];
            let dmass = vdel[iv][2][0] - dac * vdel[iv][2][1];
            let duedg = unew[is][ibl] + dac * u_ac[is][ibl] - side.uedg[ibl];
            let ddstr = (dmass - side.dstr[ibl] * duedg) / side.uedg[ibl];
            (dctau, dthet, ddstr, duedg)
        };
        for (is, side) in self.sides.iter().enumerate() {
            for ibl in 2..=side.nbl {
                let (dctau, dthet, ddstr, duedg) = deltas(side, is, ibl);
                let dn1 = if ibl < side.itran {
                    dctau / 10.0
                } else {
                    dctau / side.ctau[ibl]
                };
                let dn2 = dthet / side.thet[ibl];
                let dn3 = ddstr / side.dstr[ibl];
                let dn4 = duedg.abs() / 0.25;
                rmsbl += dn1 * dn1 + dn2 * dn2 + dn3 * dn3 + dn4 * dn4;

                let v1 = if ibl < side.itran { 'n' } else { 'C' };
                for (dn, rmx, v) in [
                    (dn1, dn1, v1),
                    (dn2, dn2, 'T'),
                    (dn3, dn3, 'D'),
                    (dn4, duedg, 'U'),
                ] {
                    let rdn = rlx * dn;
                    if dn.abs() > rmxbl.abs() {
                        rmxbl = rmx;
                        vmxbl = v;
                        imxbl = ibl;
                        ismxbl = is + 1;
                    }
                    if rdn > DHI {
                        rlx = DHI / dn;
                    }
                    if rdn < DLO {
                        rlx = DLO / dn;
                    }
                }
            }
        }
        let nbl_total = (self.sides[0].nbl + self.sides[1].nbl) as f64;
        self.rmsbl = (rmsbl / (4.0 * nbl_total)).sqrt();
        self.rmxbl = rmxbl;
        self.vmxbl = vmxbl;
        self.imxbl = imxbl;
        self.ismxbl = ismxbl;
        self.rlx = rlx;

        // Underrelaxed change of CL.
        self.cl += rlx * dac;

        // Update the boundary-layer variables.
        for is in 0..2 {
            for ibl in 2..=self.sides[is].nbl {
                let (dctau, dthet, ddstr, duedg) = deltas(&self.sides[is], is, ibl);
                let dswaki = self.dswak(is, ibl);
                let side = &mut self.sides[is];
                side.ctau[ibl] += rlx * dctau;
                side.thet[ibl] += rlx * dthet;
                side.dstr[ibl] += rlx * ddstr;
                side.uedg[ibl] += rlx * duedg;

                // Eliminate absurd transients.
                if ibl >= side.itran {
                    side.ctau[ibl] = side.ctau[ibl].min(0.25);
                }
                let hklim = if ibl <= side.iblte { 1.02 } else { 1.00005 };
                let ue = side.uedg[ibl];
                let msq = ue * ue * hstinv / (GAMM1 * (1.0 - 0.5 * ue * ue * hstinv));
                side.dstr[ibl] =
                    dslim(side.dstr[ibl] - dswaki, side.thet[ibl], msq, hklim) + dswaki;

                // New mass defect (nonlinear update).
                side.mass[ibl] = side.dstr[ibl] * side.uedg[ibl];
            }

            // No "islands" of negative Ue.
            let side = &mut self.sides[is];
            for ibl in 3..=side.iblte {
                if side.uedg[ibl - 1] > 0.0 && side.uedg[ibl] <= 0.0 {
                    side.uedg[ibl] = side.uedg[ibl - 1];
                    side.mass[ibl] = side.dstr[ibl] * side.uedg[ibl];
                }
            }
        }

        // The top wake arrays mirror the bottom ones.
        let (top, bot) = self.sides.split_at_mut(1);
        let (top, bot) = (&mut top[0], &bot[0]);
        for kbl in 1..=bot.nbl - bot.iblte {
            let (it, ib) = (top.iblte + kbl, bot.iblte + kbl);
            top.ctau[it] = bot.ctau[ib];
            top.thet[it] = bot.thet[ib];
            top.dstr[it] = bot.dstr[ib];
            top.uedg[it] = bot.uedg[ib];
            top.tau[it] = bot.tau[ib];
            top.dis[it] = bot.dis[ib];
            top.ctq[it] = bot.ctq[ib];
            top.delt[it] = bot.delt[ib];
            top.tstr[it] = bot.tstr[ib];
        }
    }
}
