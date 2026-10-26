//! State of a viscous analysis, kept from one operating point to the next as
//! in an XFOIL session, and its bookkeeping routines (XFOIL `xpanel.f`:
//! `STFIND`, `IBLPAN`, `XICALC`, `UICALC`, `QVFUE`, `QISET`, `GAMQV`,
//! `STMOVE`, `UESET`; `xbl.f`: `IBLSYS`, `XIFSET`; `xoper.f`: `SPECAL`;
//! `xfoil.f`: `COMSET`, `CPCALC`, `CDCALC`).
//!
//! Boundary-layer stations are numbered per side as in XFOIL: station 1 is
//! the stagnation point (no unknowns), 2 the first station after it,
//! `iblte` the trailing edge and `iblte + 1 ..= nbl` the wake, which belongs
//! to the bottom side. Side 0 is XFOIL's side 1 (top), side 1 its side 2
//! (bottom). Panel nodes are 0-based: node `n + iw − 1` is wake node `iw`.

// Index loops mirror XFOIL's DO loops.
#![allow(clippy::needless_range_loop)]

use aero_geometry::spline::{EndCondition, Spline, deval, seval};
use faer::Mat;

use super::blsys::Bl;
use super::closure::BlParams;
use super::panels::{Panels, base_speeds, source_matrix};
use crate::inviscid::{InviscidSolver, PanelGeometry, clcalc};

/// Minimum ξ of a station near the stagnation point, relative to the
/// airfoil perimeter (XFOIL `XFEPS`).
const XFEPS: f64 = 1.0e-7;

/// Boundary-layer arrays of one side (XFOIL's `IPAN`, `VTI`, `XSSI`,
/// `UEDG`, `UINV`, `CTAU`, `THET`, `DSTR`, `MASS`, `TAU`, … columns).
#[derive(Debug, Clone, Default)]
pub(crate) struct Side {
    /// Panel node of each station.
    pub ipan: Vec<usize>,
    /// Sign of the panel tangential velocity seen by the boundary layer.
    pub vti: Vec<f64>,
    /// Line of each station in the global Newton system (0-based).
    pub isys: Vec<usize>,
    /// Arc length from the stagnation point ξ.
    pub xssi: Vec<f64>,
    /// Edge velocity.
    pub uedg: Vec<f64>,
    /// Inviscid edge velocity and its derivative wrt α.
    pub uinv: Vec<f64>,
    pub uinv_a: Vec<f64>,
    /// Amplification N (laminar) or √Cτ (turbulent).
    pub ctau: Vec<f64>,
    /// Momentum thickness θ.
    pub thet: Vec<f64>,
    /// Displacement thickness δ*.
    pub dstr: Vec<f64>,
    /// Mass defect `Ue·δ*`.
    pub mass: Vec<f64>,
    /// Wall shear stress.
    pub tau: Vec<f64>,
    /// Dissipation.
    pub dis: Vec<f64>,
    /// Equilibrium √Cτ.
    pub ctq: Vec<f64>,
    /// Boundary-layer thickness δ.
    pub delt: Vec<f64>,
    /// Kinetic-energy thickness θ*.
    pub tstr: Vec<f64>,
    /// Last station.
    pub nbl: usize,
    /// Trailing-edge station.
    pub iblte: usize,
    /// Transition station (first turbulent interval).
    pub itran: usize,
    /// Transition forced by the trip.
    pub tforce: bool,
    /// ξ of the transition point.
    pub xssitr: f64,
    /// Chord-based x/c and y/c of the transition point.
    pub xoctr: f64,
    pub yoctr: f64,
    /// Fractional panel index of the transition point.
    pub tindex: f64,
}

impl Side {
    fn new(capacity: usize) -> Self {
        let v = || vec![0.0; capacity];
        Self {
            ipan: vec![0; capacity],
            vti: v(),
            isys: vec![0; capacity],
            xssi: v(),
            uedg: v(),
            uinv: v(),
            uinv_a: v(),
            ctau: v(),
            thet: v(),
            dstr: v(),
            mass: v(),
            tau: v(),
            dis: v(),
            ctq: v(),
            delt: v(),
            tstr: v(),
            ..Self::default()
        }
    }
}

/// The global Newton system of `BLSOLV` (XFOIL `VA`, `VB`, `VM`, `VZ`,
/// `VDEL`), one block line per boundary-layer station.
#[derive(Debug, Clone, Default)]
pub(crate) struct GlobalSystem {
    /// Number of block lines.
    pub nsys: usize,
    /// Diagonal blocks: columns are dCτ (or dN) and dθ of the station.
    pub va: Vec<[[f64; 2]; 3]>,
    /// Sub-diagonal blocks (previous station).
    pub vb: Vec<[[f64; 2]; 3]>,
    /// Mass-defect columns, `vm[(iv·3 + k)·nsys + jv]` is XFOIL's
    /// `VM(k, jv, iv)`.
    pub vm: Vec<f64>,
    /// Right-hand sides: residual and sensitivity to the global variable.
    pub vdel: Vec<[[f64; 2]; 3]>,
    /// Extra block coupling the first wake station to the top trailing edge.
    pub vz: [[f64; 2]; 3],
}

impl GlobalSystem {
    pub fn resize(&mut self, nsys: usize) {
        self.nsys = nsys;
        self.va.clear();
        self.va.resize(nsys, [[0.0; 2]; 3]);
        self.vb.clear();
        self.vb.resize(nsys, [[0.0; 2]; 3]);
        self.vdel.clear();
        self.vdel.resize(nsys, [[0.0; 2]; 3]);
        self.vm.clear();
        self.vm.resize(3 * nsys * nsys, 0.0);
        self.vz = [[0.0; 2]; 3];
    }

    /// Index of XFOIL's `VM(k, jv, iv)` (0-based).
    #[inline]
    pub fn m(&self, k: usize, jv: usize, iv: usize) -> usize {
        (iv * 3 + k) * self.nsys + jv
    }
}

/// Parameters of a viscous analysis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Parameters {
    /// Reynolds number based on chord and freestream speed.
    pub reynolds: f64,
    /// Freestream Mach number.
    pub mach: f64,
    /// Critical amplification ratio per side (top, bottom).
    pub ncrit: [f64; 2],
    /// Trip location x/c per side; ≥ 1 for free transition only.
    pub xtr: [f64; 2],
    /// Use XFOIL's alternative amplification rate `DAMPL2` (`VPAR` `I`).
    pub damp2: bool,
    /// Threshold of the mass-influence terms dropped by `BLSOLV`.
    pub vaccel: f64,
    /// Wake length in chords.
    pub wake_length: f64,
    /// Moment reference point.
    pub moment_reference: [f64; 2],
}

/// The state of an XFOIL session in `OPER` with viscous mode on: the
/// current operating point, the wake, the boundary layer and the flags that
/// decide what is recomputed (XFOIL `LWAKE`, `LIPAN`, `LBLINI`, `LVCONV`,
/// `LWDIJ`).
#[derive(Debug, Clone)]
pub(crate) struct Session<'a> {
    pub inv: &'a InviscidSolver,
    pub par: Parameters,
    /// Number of airfoil nodes.
    pub n: usize,
    /// Number of wake nodes.
    pub nw: usize,

    /// Angle of attack (radians).
    pub alfa: f64,
    pub qinf: f64,
    /// Kármán–Tsien parameter of the freestream Mach number.
    pub tklam: f64,
    pub cl: f64,
    pub cm: f64,
    pub cdp: f64,
    pub cd: f64,
    pub cdf: f64,
    pub clf: f64,

    /// Airfoil and wake nodes.
    pub pan: Panels,
    pub lwake: bool,
    /// Angle of attack of the current wake.
    pub awake: f64,
    /// Inviscid speeds at all nodes for α = 0° and 90°.
    pub qinvu: [Vec<f64>; 2],
    /// Inviscid and viscous tangential speeds at all nodes.
    pub qinv: Vec<f64>,
    pub qinv_a: Vec<f64>,
    pub qvis: Vec<f64>,
    /// Surface vorticity (airfoil nodes).
    pub gam: Vec<f64>,
    pub gam_a: Vec<f64>,
    /// Source influence matrix of all nodes.
    pub dij: Mat<f64>,
    pub lwdij: bool,

    pub lipan: bool,
    pub lblini: bool,
    pub lvconv: bool,
    /// Angle of attack of the converged viscous solution.
    pub avisc: f64,

    /// Stagnation point: node before it, arc length and sensitivities to
    /// the vorticity of the two nodes around it.
    pub ist: usize,
    pub sst: f64,
    pub sst_go: f64,
    pub sst_gp: f64,
    pub sides: [Side; 2],
    /// Wake gap (thickness of the dead-air region behind a blunt trailing
    /// edge), indexed by wake node 1..=nw.
    pub wgap: Vec<f64>,

    /// Interval equations (XFOIL `XBL.INC`), kept between calls as XFOIL's
    /// common blocks are.
    pub bl: Bl,
    pub sys: GlobalSystem,
    /// Convergence measures of the last Newton update.
    pub rmsbl: f64,
    pub rmxbl: f64,
    /// Variable (`n`, `C`, `T`, `D`, `U`) and station (side, station) of
    /// the largest change.
    pub vmxbl: char,
    pub imxbl: usize,
    pub ismxbl: usize,
    pub rlx: f64,
    /// When set, receives XFOIL's console messages (marching failures,
    /// transition points, Newton iterations) with XFOIL's formatting, to
    /// compare the iteration histories.
    pub trace: Option<Vec<String>>,
}

impl<'a> Session<'a> {
    /// A session for the given inviscid solver, with no boundary layer yet.
    pub fn new(inv: &'a InviscidSolver, par: Parameters) -> Self {
        let geo = inv.geometry();
        let n = geo.len();
        let qinf = 1.0;
        let gam = gamma(inv, 0.0);
        let pan = Panels::with_wake(geo, &gam, 0.0, qinf, par.wake_length);
        let nw = pan.nw;
        let nt = n + nw;
        let capacity = nt + 2;
        let mut s = Self {
            inv,
            par,
            n,
            nw,
            alfa: 0.0,
            qinf,
            tklam: 0.0,
            cl: 0.0,
            cm: 0.0,
            cdp: 0.0,
            cd: 0.0,
            cdf: 0.0,
            clf: 0.0,
            pan,
            lwake: false,
            awake: 0.0,
            qinvu: [vec![0.0; nt], vec![0.0; nt]],
            qinv: vec![0.0; nt],
            qinv_a: vec![0.0; nt],
            qvis: vec![0.0; nt],
            gam,
            gam_a: vec![0.0; n],
            dij: Mat::zeros(nt, nt),
            lwdij: false,
            lipan: false,
            lblini: false,
            lvconv: false,
            avisc: 0.0,
            ist: 0,
            sst: 0.0,
            sst_go: 0.0,
            sst_gp: 0.0,
            sides: [Side::new(capacity), Side::new(capacity)],
            wgap: vec![0.0; nw + 1],
            bl: Bl::new(Default::default(), BlParams::default()),
            sys: GlobalSystem::default(),
            rmsbl: 0.0,
            rmxbl: 0.0,
            vmxbl: ' ',
            imxbl: 0,
            ismxbl: 0,
            rlx: 1.0,
            trace: None,
        };
        // Base speeds on the airfoil (GGCALC); the wake part is set by
        // QWCALC once the wake exists.
        let g = inv.base_solutions();
        for i in 0..n {
            s.qinvu[0][i] = g[(i, 0)];
            s.qinvu[1][i] = g[(i, 1)];
        }
        s.comset();
        s
    }

    /// Records a console message when tracing.
    pub fn log(&mut self, message: impl FnOnce() -> String) {
        if let Some(trace) = &mut self.trace {
            trace.push(message());
        }
    }

    pub fn geo(&self) -> &'a PanelGeometry {
        self.inv.geometry()
    }

    /// XFOIL `COMSET`: Kármán–Tsien parameter.
    pub fn comset(&mut self) {
        let minf = self.par.mach;
        let beta = (1.0 - minf * minf).sqrt();
        self.tklam = minf * minf / ((1.0 + beta) * (1.0 + beta));
    }

    /// XFOIL `INIT`: the next point starts from a fresh boundary layer.
    pub fn reset_boundary_layer(&mut self) {
        self.lblini = false;
        self.lipan = false;
    }

    /// The operating-point part of XFOIL's `ALFA`/`ASEQ` commands: inviscid
    /// solution at `alfa` (`SPECAL`) and invalidation of the wake and of the
    /// converged flag when the angle changes.
    pub fn set_alpha(&mut self, alfa: f64) {
        self.alfa = alfa;
        self.qinf = 1.0;
        self.specal();
        if (self.alfa - self.awake).abs() > 1.0e-5 {
            self.lwake = false;
        }
        if (self.alfa - self.avisc).abs() > 1.0e-5 {
            self.lvconv = false;
        }
    }

    /// XFOIL `SPECAL` at fixed Mach number: inviscid vorticity, speeds and
    /// forces at the current angle of attack.
    fn specal(&mut self) {
        let g = self.inv.base_solutions();
        let (sina, cosa) = self.alfa.sin_cos();
        for i in 0..self.n {
            self.gam[i] = cosa * g[(i, 0)] + sina * g[(i, 1)];
            self.gam_a[i] = -sina * g[(i, 0)] + cosa * g[(i, 1)];
        }
        self.qiset();
        self.comset();
        self.clcalc();
    }

    /// XFOIL `CLCALC` with the current vorticity.
    pub fn clcalc(&mut self) {
        let geo = self.geo();
        let c = clcalc(
            &geo.x,
            &geo.y,
            &self.gam,
            &self.gam_a,
            self.alfa,
            self.par.mach,
            self.par.moment_reference,
        );
        self.cl = c.cl;
        self.cm = c.cm;
        self.cdp = c.cdp;
    }

    /// XFOIL `XYWAKE` for the current vorticity.
    pub fn xywake(&mut self) {
        self.pan = Panels::with_wake(
            self.geo(),
            &self.gam,
            self.alfa,
            self.qinf,
            self.par.wake_length,
        );
        self.lwake = true;
        self.awake = self.alfa;
        self.lwdij = false;
    }

    /// XFOIL `QWCALC`: inviscid base speeds at the wake nodes.
    pub fn qwcalc(&mut self) {
        let q = base_speeds(self.inv, &self.pan);
        for k in 0..2 {
            self.qinvu[k][self.n..].copy_from_slice(&q[k][self.n..]);
        }
    }

    /// XFOIL `QDCALC`.
    pub fn qdcalc(&mut self) {
        self.dij = source_matrix(self.inv, &self.pan);
        self.lwdij = true;
    }

    /// XFOIL `QISET`: inviscid speeds at all nodes for the current α.
    pub fn qiset(&mut self) {
        let (sina, cosa) = self.alfa.sin_cos();
        for i in 0..self.n + self.nw {
            self.qinv[i] = cosa * self.qinvu[0][i] + sina * self.qinvu[1][i];
            self.qinv_a[i] = -sina * self.qinvu[0][i] + cosa * self.qinvu[1][i];
        }
    }

    /// XFOIL `GAMQV`.
    pub fn gamqv(&mut self) {
        for i in 0..self.n {
            self.gam[i] = self.qvis[i];
            self.gam_a[i] = self.qinv_a[i];
        }
    }

    /// XFOIL `STFIND`: stagnation point from the sign change of γ.
    pub fn stfind(&mut self) {
        let n = self.n;
        let s = &self.geo().s;
        let gam = &self.gam;
        // XFOIL: "Stagnation point not found. Continuing ..." with I = N/2.
        let i = (0..n - 1)
            .find(|&i| gam[i] >= 0.0 && gam[i + 1] < 0.0)
            .unwrap_or(n / 2 - 1);
        self.ist = i;
        let dgam = gam[i + 1] - gam[i];
        let ds = s[i + 1] - s[i];

        // Evaluated so as to minimize roundoff for very small γ.
        let mut sst = if gam[i] < -gam[i + 1] {
            s[i] - ds * (gam[i] / dgam)
        } else {
            s[i + 1] - ds * (gam[i + 1] / dgam)
        };
        // Tweak if it falls right on a node (very unlikely).
        if sst <= s[i] {
            sst = s[i] + 1.0e-7;
        }
        if sst >= s[i + 1] {
            sst = s[i + 1] - 1.0e-7;
        }
        self.sst = sst;
        self.sst_go = (sst - s[i + 1]) / dgam;
        self.sst_gp = (s[i] - sst) / dgam;
    }

    /// XFOIL `IBLPAN`: station → panel node pointers.
    pub fn iblpan(&mut self) {
        let (n, nw, ist) = (self.n, self.nw, self.ist);

        let top = &mut self.sides[0];
        let mut ibl = 1;
        for i in (0..=ist).rev() {
            ibl += 1;
            top.ipan[ibl] = i;
            top.vti[ibl] = 1.0;
        }
        top.iblte = ibl;
        top.nbl = ibl;

        let bot = &mut self.sides[1];
        let mut ibl = 1;
        for i in ist + 1..n {
            ibl += 1;
            bot.ipan[ibl] = i;
            bot.vti[ibl] = -1.0;
        }
        bot.iblte = ibl;
        for iw in 1..=nw {
            let ibl = bot.iblte + iw;
            bot.ipan[ibl] = n + iw - 1;
            bot.vti[ibl] = -1.0;
        }
        bot.nbl = bot.iblte + nw;

        // Upper wake pointers (for plotting only in XFOIL).
        let (top, bot) = self.sides.split_at_mut(1);
        let (top, bot) = (&mut top[0], &bot[0]);
        for iw in 1..=nw {
            top.ipan[top.iblte + iw] = bot.ipan[bot.iblte + iw];
            top.vti[top.iblte + iw] = 1.0;
        }
        self.lipan = true;
    }

    /// XFOIL `XICALC`: ξ of every station and the wake gap.
    pub fn xicalc(&mut self) {
        let geo = self.geo();
        let n = self.n;
        let (s, x, y) = (&geo.s, &self.pan.x, &self.pan.y);
        let xeps = XFEPS * (s[n - 1] - s[0]);
        let sst = self.sst;

        let top = &mut self.sides[0];
        top.xssi[1] = 0.0;
        for ibl in 2..=top.iblte {
            top.xssi[ibl] = (sst - s[top.ipan[ibl]]).max(xeps);
        }
        let bot = &mut self.sides[1];
        bot.xssi[1] = 0.0;
        for ibl in 2..=bot.iblte {
            bot.xssi[ibl] = (s[bot.ipan[ibl]] - sst).max(xeps);
        }

        let (top, bot) = self.sides.split_at_mut(1);
        let (top, bot) = (&mut top[0], &mut bot[0]);
        top.xssi[top.iblte + 1] = top.xssi[top.iblte];
        bot.xssi[bot.iblte + 1] = bot.xssi[bot.iblte];
        for ibl in bot.iblte + 2..=bot.nbl {
            let i = bot.ipan[ibl];
            let dxssi = ((x[i] - x[i - 1]).powi(2) + (y[i] - y[i - 1]).powi(2)).sqrt();
            let ibl1 = top.iblte + ibl - bot.iblte;
            top.xssi[ibl1] = top.xssi[ibl1 - 1] + dxssi;
            bot.xssi[ibl] = bot.xssi[ibl - 1] + dxssi;
        }

        // Trailing-edge "flap" (wake gap) cubic.
        const TELRAT: f64 = 2.50;
        let (xp, yp) = (&geo.xp, &geo.yp);
        let crosp = (xp[0] * yp[n - 1] - yp[0] * xp[n - 1])
            / ((xp[0] * xp[0] + yp[0] * yp[0]) * (xp[n - 1] * xp[n - 1] + yp[n - 1] * yp[n - 1]))
                .sqrt();
        let dwdxte = (crosp / (1.0 - crosp * crosp).sqrt()).clamp(-3.0 / TELRAT, 3.0 / TELRAT);
        let aa = 3.0 + TELRAT * dwdxte;
        let bb = -2.0 - TELRAT * dwdxte;

        let te = &geo.te_panel;
        for iw in 1..=self.nw {
            self.wgap[iw] = if te.sharp {
                0.0
            } else {
                let ibl = bot.iblte + iw;
                let zn = 1.0 - (bot.xssi[ibl] - bot.xssi[bot.iblte]) / (TELRAT * te.ante);
                if zn >= 0.0 {
                    te.ante * (aa + bb * zn) * zn * zn
                } else {
                    0.0
                }
            };
        }
    }

    /// XFOIL `IBLSYS`: station → Newton system line pointers.
    pub fn iblsys(&mut self) {
        let mut iv = 0;
        for side in &mut self.sides {
            for ibl in 2..=side.nbl {
                side.isys[ibl] = iv;
                iv += 1;
            }
        }
        self.sys.resize(iv);
    }

    /// XFOIL `UICALC`: inviscid edge velocities.
    pub fn uicalc(&mut self) {
        for side in &mut self.sides {
            side.uinv[1] = 0.0;
            side.uinv_a[1] = 0.0;
            for ibl in 2..=side.nbl {
                let i = side.ipan[ibl];
                side.uinv[ibl] = side.vti[ibl] * self.qinv[i];
                side.uinv_a[ibl] = side.vti[ibl] * self.qinv_a[i];
            }
        }
    }

    /// XFOIL `QVFUE`: panel speeds from the edge velocities.
    pub fn qvfue(&mut self) {
        for side in &self.sides {
            for ibl in 2..=side.nbl {
                self.qvis[side.ipan[ibl]] = side.vti[ibl] * side.uedg[ibl];
            }
        }
    }

    /// XFOIL `STMOVE`: moves the stagnation point to its new panel and
    /// shifts the boundary-layer variables accordingly.
    pub fn stmove(&mut self) {
        let istold = self.ist;
        self.stfind();
        if istold == self.ist {
            self.xicalc();
        } else {
            self.iblpan();
            self.uicalc();
            self.xicalc();
            self.iblsys();

            let (grow, shrink, idif) = if self.ist > istold {
                // More stations on the top side.
                (0, 1, self.ist - istold)
            } else {
                (1, 0, istold - self.ist)
            };
            self.sides[grow].itran += idif;
            self.sides[shrink].itran = self.sides[shrink].itran.saturating_sub(idif);

            // Move the growing side's variables downstream.
            let g = &mut self.sides[grow];
            for ibl in (idif + 2..=g.nbl).rev() {
                g.ctau[ibl] = g.ctau[ibl - idif];
                g.thet[ibl] = g.thet[ibl - idif];
                g.dstr[ibl] = g.dstr[ibl - idif];
                g.uedg[ibl] = g.uedg[ibl - idif];
            }
            // Variables between the old and new stagnation points.
            let dudx = g.uedg[idif + 2] / g.xssi[idif + 2];
            for ibl in (2..=idif + 1).rev() {
                g.ctau[ibl] = g.ctau[idif + 2];
                g.thet[ibl] = g.thet[idif + 2];
                g.dstr[ibl] = g.dstr[idif + 2];
                g.uedg[ibl] = dudx * g.xssi[ibl];
            }
            // Move the shrinking side's variables upstream.
            let sh = &mut self.sides[shrink];
            for ibl in 2..=sh.nbl {
                sh.ctau[ibl] = sh.ctau[ibl + idif];
                sh.thet[ibl] = sh.thet[ibl + idif];
                sh.dstr[ibl] = sh.dstr[ibl + idif];
                sh.uedg[ibl] = sh.uedg[ibl + idif];
            }

            // Tweak Ue so it is not zero, in case the stagnation point is
            // right on a node.
            const UEPS: f64 = 1.0e-7;
            for side in &mut self.sides {
                for ibl in 2..=side.nbl {
                    let i = side.ipan[ibl];
                    if side.uedg[ibl] <= UEPS {
                        side.uedg[ibl] = UEPS;
                        self.qvis[i] = side.vti[ibl] * UEPS;
                        if i < self.n {
                            self.gam[i] = side.vti[ibl] * UEPS;
                        }
                    }
                }
            }
        }

        // New mass defects, since Ue may have been tweaked.
        for side in &mut self.sides {
            for ibl in 2..=side.nbl {
                side.mass[ibl] = side.dstr[ibl] * side.uedg[ibl];
            }
        }
    }

    /// XFOIL `XIFSET`: ξ of the trip on side `is`.
    pub fn xifset(&self, is: usize) -> f64 {
        let side = &self.sides[is];
        let xi_te = side.xssi[side.iblte];
        let xstrip = self.par.xtr[is];
        if xstrip >= 1.0 {
            return xi_te;
        }
        let geo = self.geo();
        let n = self.n;
        let [xle, yle] = geo.le;
        let [xte, yte] = geo.te;
        let (chx, chy) = (xte - xle, yte - yle);
        let chsq = chx * chx + chy * chy;

        // Chord-based x/c, splined against the arc length.
        let w1: Vec<f64> = (0..n)
            .map(|i| ((geo.x[i] - xle) * chx + (geo.y[i] - yle) * chy) / chsq)
            .collect();
        let s = &geo.s;
        let sle = geo.sle;
        let str0 = if is == 0 {
            sle + (s[0] - sle) * xstrip
        } else {
            sle + (s[n - 1] - sle) * xstrip
        };
        let str = match Spline::new(
            s,
            &w1,
            EndCondition::ZeroThirdDerivative,
            EndCondition::ZeroThirdDerivative,
        ) {
            Ok(w) => sinvrt(str0, xstrip, &w, s[n - 1] - s[0]),
            Err(_) => str0,
        };
        let xiforc = if is == 0 {
            (self.sst - str).min(xi_te)
        } else {
            (str - self.sst).min(xi_te)
        };
        if xiforc < 0.0 {
            // "Stagnation point is past trip on side IS".
            xi_te
        } else {
            xiforc
        }
    }

    /// Chord-based coordinates of the point at arc length `s` on the
    /// airfoil contour (the transition location output of `SETBL`).
    pub fn chord_coordinates(&self, s: f64) -> (f64, f64) {
        let geo = self.geo();
        let [xle, yle] = geo.le;
        let [xte, yte] = geo.te;
        let (chx, chy) = (xte - xle, yte - yle);
        let chsq = chx * chx + chy * chy;
        let xtr = seval(s, &geo.x, &geo.xp, &geo.s);
        let ytr = seval(s, &geo.y, &geo.yp, &geo.s);
        (
            ((xtr - xle) * chx + (ytr - yle) * chy) / chsq,
            ((ytr - yle) * chx - (xtr - xle) * chy) / chsq,
        )
    }

    /// XFOIL `CDCALC`: drag from the Squire–Young formula at the end of
    /// the wake, and skin-friction drag and lift.
    pub fn cdcalc(&mut self) {
        let (sa, ca) = self.alfa.sin_cos();
        let bot = &self.sides[1];
        self.cd = if self.lblini {
            let nbl = bot.nbl;
            let thwake = bot.thet[nbl];
            let urat = bot.uedg[nbl] / self.qinf;
            let uewake = bot.uedg[nbl] * (1.0 - self.tklam) / (1.0 - self.tklam * urat * urat);
            let shwake = bot.dstr[nbl] / bot.thet[nbl];
            2.0 * thwake * (uewake / self.qinf).powf(0.5 * (5.0 + shwake))
        } else {
            0.0
        };

        let (x, y) = (&self.pan.x, &self.pan.y);
        let mut cdf = 0.0;
        let mut clf = 0.0;
        for side in &self.sides {
            for ibl in 3..=side.iblte {
                let i = side.ipan[ibl];
                let im = side.ipan[ibl - 1];
                let dx = (x[i] - x[im]) * ca + (y[i] - y[im]) * sa;
                let dy = -(x[i] - x[im]) * sa + (y[i] - y[im]) * ca;
                let tau = 0.5 * (side.tau[ibl] + side.tau[ibl - 1]);
                cdf += tau * dx * 2.0 / (self.qinf * self.qinf);
                clf += tau * dy * 2.0 / (self.qinf * self.qinf);
            }
        }
        self.cdf = cdf;
        self.clf = clf;
    }

    /// XFOIL `CPCALC`: compressible pressure coefficients of the speeds `q`.
    pub fn cpcalc(&self, q: &[f64]) -> Vec<f64> {
        let minf = self.par.mach;
        let beta = (1.0 - minf * minf).sqrt();
        let bfac = 0.5 * minf * minf / (1.0 + beta);
        q.iter()
            .map(|&qi| {
                let cpinc = 1.0 - (qi / self.qinf).powi(2);
                cpinc / (beta + bfac * cpinc)
            })
            .collect()
    }
}

/// Inviscid vorticity at angle of attack `alfa`.
fn gamma(inv: &InviscidSolver, alfa: f64) -> Vec<f64> {
    let g = inv.base_solutions();
    let (sina, cosa) = alfa.sin_cos();
    (0..inv.len())
        .map(|i| cosa * g[(i, 0)] + sina * g[(i, 1)])
        .collect()
}

/// XFOIL `SINVRT`: Newton inversion of `w(s) = xi` from `si`; the input
/// value is returned if it does not converge.
fn sinvrt(si: f64, xi: f64, w: &Spline, span: f64) -> f64 {
    let (s, v, d) = (w.knots(), w.values(), w.derivatives());
    let mut s_new = si;
    for _ in 0..10 {
        let res = seval(s_new, v, d, s) - xi;
        let resp = deval(s_new, v, d, s);
        let ds = -res / resp;
        s_new += ds;
        if (ds / span).abs() < 1.0e-5 {
            return s_new;
        }
    }
    si
}

/// Fortran `Ew.d` formatting as gfortran prints it (`0.1183E+00`,
/// `-.1714E+01` when the width leaves no room for the leading zero).
pub(crate) fn fortran_e(value: f64, w: usize, d: usize) -> String {
    if !value.is_finite() {
        return format!(
            "{:>w$}",
            if value.is_nan() {
                "NaN"
            } else if value > 0.0 {
                "Infinity"
            } else {
                "-Infinity"
            }
        );
    }
    let (mut mantissa, mut exponent) = (value.abs(), 0_i32);
    if mantissa != 0.0 {
        exponent = mantissa.log10().floor() as i32 + 1;
        mantissa /= 10f64.powi(exponent);
        // Rounding may carry into a new digit.
        if (mantissa * 10f64.powi(d as i32)).round() >= 10f64.powi(d as i32) {
            mantissa /= 10.0;
            exponent += 1;
        }
    }
    let digits = format!("{:.d$}", mantissa);
    let digits = digits.trim_start_matches('0');
    let sign = if value < 0.0 { "-" } else { "" };
    let body = format!(
        "{sign}{digits}E{}{:02}",
        if exponent < 0 { '-' } else { '+' },
        exponent.abs()
    );
    let padded = if body.len() < w {
        format!("0{}", body.trim_start_matches('-'))
    } else {
        body.clone()
    };
    let padded = if value < 0.0 && body.len() < w {
        format!("-{padded}")
    } else {
        padded
    };
    format!("{padded:>w$}")
}
