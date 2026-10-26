//! The viscous Newton iteration of one operating point (XFOIL `xoper.f`:
//! `VISCAL`).

// Index loops mirror XFOIL's DO loops.
#![allow(clippy::needless_range_loop)]

use super::session::Session;

/// Convergence tolerance on the rms of the normalized Newton changes
/// (XFOIL `EPS1`).
const EPS1: f64 = 1.0e-4;

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
}
