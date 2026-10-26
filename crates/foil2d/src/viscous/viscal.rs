//! Viscous operating points and sequences (XFOIL `xoper.f`: `VISCAL`, the
//! `ALFA` and `ASEQ` commands, `FCPMIN`; `xpol.f`: `PLXADD`).

// Index loops mirror XFOIL's DO loops.
#![allow(clippy::needless_range_loop)]

use super::session::Session;

/// Convergence tolerance on the rms of the normalized Newton changes
/// (XFOIL `EPS1`).
const EPS1: f64 = 1.0e-4;

/// Number of successive unconverged points that stops a sequence (XFOIL
/// `NSEQEX`).
const NSEQEX: usize = 4;

/// Forces and transition points of an operating point (a line of XFOIL's
/// polar file).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PointOutput {
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
pub(crate) struct SideDistributions {
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
    /// XFOIL `VISCAL`: converges the viscous solution at the current angle
    /// of attack with at most `niter` Newton iterations. Returns the number
    /// of iterations and whether the rms change fell below `EPS1`.
    pub(crate) fn viscal(&mut self, niter: usize) -> (usize, bool) {
        // Wake trajectory from the current inviscid solution if necessary.
        if !self.lwake {
            self.xywake();
        }
        // Wake velocities for α = 0°, 90°, then all for the current α.
        self.qwcalc();
        self.qiset();

        if !self.lipan {
            if self.lblini {
                self.gamqv();
            }
            self.stfind();
            self.iblpan();
            self.xicalc();
            self.iblsys();
        }

        // Inviscid edge velocities.
        self.uicalc();
        if !self.lblini {
            // Initial Ue from the inviscid Ue.
            for side in &mut self.sides {
                for ibl in 1..=side.nbl {
                    side.uedg[ibl] = side.uinv[ibl];
                }
            }
        }

        if self.lvconv {
            // Correct CL if a converged point exists.
            self.qvfue();
            self.gamqv();
            self.clcalc();
            self.cdcalc();
        }

        // Source influence matrix if it does not exist.
        if !self.lwdij {
            self.qdcalc();
        }

        // Newton iteration for the entire boundary-layer solution.
        let mut iterations = 0;
        let mut converged = false;
        for iter in 1..=niter {
            iterations = iter;
            self.setbl();
            self.blsolv();
            self.update();
            // (Mach and Reynolds numbers are fixed: MRCL and COMSET do not
            // change them.)
            self.qvfue();
            self.gamqv();
            self.stmove();
            self.clcalc();
            self.cdcalc();
            if self.rmsbl < EPS1 {
                self.lvconv = true;
                self.avisc = self.alfa;
                converged = true;
                break;
            }
        }
        (iterations, converged)
    }

    /// Viscous pressure coefficients at all nodes (airfoil, then wake).
    pub(crate) fn cpv(&self) -> Vec<f64> {
        self.cpcalc(&self.qvis[..self.n + self.nw])
    }

    /// Forces and transition points of the current solution (`FCPMIN`,
    /// `PLRADD`).
    pub(crate) fn output(&self, iterations: usize, converged: bool) -> PointOutput {
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
    pub(crate) fn distributions(&self) -> [SideDistributions; 2] {
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
    pub(crate) fn alfa(&mut self, alpha_deg: f64, itmax: usize) -> PointOutput {
        self.set_alpha(alpha_deg.to_radians());
        let (iterations, converged) = self.viscal(itmax);
        self.output(iterations, converged)
    }

    /// XFOIL `ASEQ`: points from `a1` to `a2` by `da` (degrees), each one
    /// starting from the boundary layer of the previous one, with `itmax + 5`
    /// iterations per point. Stops after `NSEQEX` successive unconverged
    /// points. `visit` receives every computed point.
    pub(crate) fn aseq(
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
