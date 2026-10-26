//! The public viscous solver.

use std::ops::ControlFlow;

use aero_geometry::Airfoil;
use serde::{Deserialize, Serialize};

use super::error::{Result, ViscousError};
use super::session::{Parameters, Session, Variation};
use crate::inviscid::{
    AlphaSweep, ClSweep, HingeMoment, InviscidError, InviscidOptions, InviscidSolver, SweepFailure,
    hinge_moment, sonic_cp,
};

/// How the Mach and Reynolds numbers vary with the lift coefficient (XFOIL
/// `TYPE`). With a variation, [`ViscousOptions::mach`] and
/// [`ViscousOptions::reynolds`] are the values at CL = 1.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MachReynoldsVariation {
    /// Fixed Mach and Reynolds numbers (type 1: fixed chord and speed).
    #[default]
    Fixed,
    /// `M·√CL` and `Re·√CL` fixed (type 2: fixed chord and wing loading,
    /// the speed varies).
    InverseSqrtCl,
    /// `M` and `Re·CL` fixed (type 3: fixed speed and lift, the chord
    /// varies).
    InverseCl,
}

/// Amplification rate of the eᴺ transition criterion.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmplificationModel {
    /// Drela's envelope correlation (XFOIL `DAMPL`, the default).
    #[default]
    Original,
    /// The modified correlation of XFOIL's `DAMP` command (`DAMPL2`).
    Modified,
}

/// Options of a viscous analysis (XFOIL `OPER`: `VISC`, `MACH`, `TYPE`,
/// `VPAR`, `DAMP`, `ITER`).
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViscousOptions {
    /// Reynolds number based on the chord and the freestream speed (at
    /// CL = 1 with a [`MachReynoldsVariation`]).
    pub reynolds: f64,
    /// Freestream Mach number, `0 ≤ M < 1` (at CL = 1 with a
    /// [`MachReynoldsVariation`]).
    pub mach: f64,
    /// Dependence of the Mach and Reynolds numbers on the lift.
    pub variation: MachReynoldsVariation,
    /// Critical amplification ratio `Ncrit` of the eᴺ transition criterion
    /// on the top and bottom surfaces (XFOIL default 9: average wind
    /// tunnel).
    pub ncrit: [f64; 2],
    /// Trip (forced transition) locations x/c on the top and bottom
    /// surfaces; 1 or more means free transition only.
    pub xtr: [f64; 2],
    /// Amplification rate correlation.
    pub amplification: AmplificationModel,
    /// Newton iterations allowed for one operating point (XFOIL `ITER`,
    /// default 20). Points of a sweep get 5 more, as with XFOIL's `ASEQ`.
    pub max_iterations: usize,
    /// Threshold below which `BLSOLV` drops off-diagonal mass-influence
    /// terms (XFOIL `VACC`, default 0.01).
    pub vaccel: f64,
}

impl Default for ViscousOptions {
    fn default() -> Self {
        Self {
            reynolds: 1.0e6,
            mach: 0.0,
            variation: MachReynoldsVariation::Fixed,
            ncrit: [9.0, 9.0],
            xtr: [1.0, 1.0],
            amplification: AmplificationModel::Original,
            max_iterations: 20,
            vaccel: 0.01,
        }
    }
}

impl ViscousOptions {
    /// Checks that every option is in range.
    pub fn validate(&self) -> Result<()> {
        let bad = |msg: String| Err(ViscousError::InvalidOption(msg));
        if !(self.reynolds.is_finite() && self.reynolds > 0.0) {
            return bad(format!(
                "Reynolds number {} (must be positive)",
                self.reynolds
            ));
        }
        if !(0.0..1.0).contains(&self.mach) {
            return Err(InviscidError::InvalidMach(self.mach).into());
        }
        for n in self.ncrit {
            if !(n.is_finite() && n > 0.0) {
                return bad(format!("Ncrit {n} (must be positive)"));
            }
        }
        for x in self.xtr {
            if !x.is_finite() {
                return bad(format!("trip location {x}"));
            }
        }
        if self.max_iterations == 0 {
            return bad("at least one Newton iteration is needed".into());
        }
        if !(self.vaccel.is_finite() && self.vaccel >= 0.0) {
            return bad(format!("VACCEL {} (must be ≥ 0)", self.vaccel));
        }
        Ok(())
    }
}

/// Transition point of one surface.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transition {
    /// Chord-based x/c (XFOIL `XOCTR`).
    pub x: f64,
    /// Chord-based y/c.
    pub y: f64,
    /// Arc length from the stagnation point.
    pub xi: f64,
    /// True if forced by the trip, false for free transition (N = Ncrit).
    pub forced: bool,
}

/// Boundary-layer quantities at the stations of one surface (from the
/// stagnation point to the trailing edge) or of the wake.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BoundaryLayer {
    /// Coordinates of the stations (panel nodes).
    pub x: Vec<f64>,
    /// See [`Self::x`].
    pub y: Vec<f64>,
    /// Arc length from the stagnation point ξ.
    pub xi: Vec<f64>,
    /// Edge velocity `Ue/V∞` (incompressible).
    pub ue: Vec<f64>,
    /// Pressure coefficient.
    pub cp: Vec<f64>,
    /// Displacement thickness δ* (in the wake, of both layers plus the
    /// trailing-edge base thickness).
    pub dstar: Vec<f64>,
    /// Momentum thickness θ.
    pub theta: Vec<f64>,
    /// Skin friction coefficient `τw / (½ρ∞V∞²)` (zero in the wake).
    pub cf: Vec<f64>,
    /// Shape parameter `H = δ*/θ`.
    pub h: Vec<f64>,
    /// Amplification ratio N of the laminar stations (`None` once
    /// turbulent).
    pub amplification: Vec<Option<f64>>,
    /// Square root of the shear stress coefficient √Cτ of the turbulent
    /// stations (`None` while laminar).
    pub sqrt_ctau: Vec<Option<f64>>,
}

/// Solution of a viscous analysis at one operating point.
///
/// Coefficients are referred to a unit chord in airfoil coordinates, as
/// for [`crate::inviscid::InviscidSolution`].
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViscousSolution {
    /// Angle of attack, degrees.
    pub alpha_deg: f64,
    /// Reynolds number of the solution.
    pub reynolds: f64,
    /// Freestream Mach number of the solution.
    pub mach: f64,
    /// Prescribed lift coefficient, for a prescribed-CL point.
    pub cl_target: Option<f64>,
    /// Lift coefficient (pressure integration of the viscous `Cp`).
    pub cl: f64,
    /// Drag coefficient (Squire–Young formula at the end of the wake).
    pub cd: f64,
    /// Pressure drag coefficient (pressure integration, XFOIL polar `CDp`).
    pub cdp: f64,
    /// Skin-friction drag coefficient.
    pub cdf: f64,
    /// Moment coefficient about the moment reference point.
    pub cm: f64,
    /// True if the Newton iteration converged (rms of the normalized changes
    /// below 10⁻⁴).
    pub converged: bool,
    /// Newton iterations performed.
    pub iterations: usize,
    /// Rms of the normalized changes of the last iteration.
    pub rms_change: f64,
    /// Viscous pressure coefficient at each airfoil node.
    pub cp: Vec<f64>,
    /// Inviscid pressure coefficient at each airfoil node.
    pub cp_inviscid: Vec<f64>,
    /// Viscous surface speed at each node, signed like
    /// [`crate::inviscid::InviscidSolution::surface_speed`].
    pub surface_speed: Vec<f64>,
    /// Minimum viscous pressure coefficient (airfoil and wake, as XFOIL).
    pub cp_min: f64,
    /// Its x location.
    pub cp_min_x: f64,
    /// Sonic pressure coefficient `Cp*`, if `M > 0`.
    pub cp_star: Option<f64>,
    /// True if `Cp min < Cp*`.
    pub supercritical: bool,
    /// True if the local speed is too large somewhere for the Kármán–Tsien
    /// correction (XFOIL's "compressibility corrections invalid").
    pub compressibility_breakdown: bool,
    /// Flap hinge moment from the viscous pressures, if a hinge is set
    /// ([`InviscidOptions::flap_hinge`]).
    pub hinge: Option<HingeMoment>,
    /// Transition on the top and bottom surfaces.
    pub transition: [Transition; 2],
    /// Top surface, from the stagnation point to the trailing edge.
    pub top: BoundaryLayer,
    /// Bottom surface, from the stagnation point to the trailing edge.
    pub bottom: BoundaryLayer,
    /// Wake, from the trailing edge downstream.
    pub wake: BoundaryLayer,
}

/// One converged point of a viscous polar.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ViscousPolarPoint {
    /// Angle of attack, degrees.
    pub alpha_deg: f64,
    /// Lift coefficient.
    pub cl: f64,
    /// Drag coefficient.
    pub cd: f64,
    /// Pressure drag coefficient.
    pub cdp: f64,
    /// Skin-friction drag coefficient.
    pub cdf: f64,
    /// Moment coefficient.
    pub cm: f64,
    /// Minimum pressure coefficient.
    pub cp_min: f64,
    /// Transition x/c on the top surface.
    pub xtr_top: f64,
    /// Transition x/c on the bottom surface.
    pub xtr_bottom: f64,
    /// Reynolds number of the point (varies with CL with a
    /// [`MachReynoldsVariation`]).
    #[serde(default)]
    pub reynolds: f64,
    /// Mach number of the point.
    #[serde(default)]
    pub mach: f64,
    /// Flap hinge moment coefficient, if a hinge is set.
    #[serde(default)]
    pub hinge_moment: Option<f64>,
    /// Newton iterations.
    pub iterations: usize,
}

impl From<&ViscousSolution> for ViscousPolarPoint {
    fn from(s: &ViscousSolution) -> Self {
        Self {
            alpha_deg: s.alpha_deg,
            cl: s.cl,
            cd: s.cd,
            cdp: s.cdp,
            cdf: s.cdf,
            cm: s.cm,
            cp_min: s.cp_min,
            xtr_top: s.transition[0].x,
            xtr_bottom: s.transition[1].x,
            reynolds: s.reynolds,
            mach: s.mach,
            hinge_moment: s.hinge.map(|h| h.moment),
            iterations: s.iterations,
        }
    }
}

/// Result of a viscous sweep.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViscousPolar {
    /// Options of the analysis.
    pub options: ViscousOptions,
    /// True for a prescribed-CL sweep (XFOIL `CSEQ`), false for an
    /// angle-of-attack sweep (`ASEQ`).
    #[serde(default)]
    pub prescribed_cl: bool,
    /// Converged points, in sweep order.
    pub points: Vec<ViscousPolarPoint>,
    /// Points that did not converge or were refused (critical Mach number
    /// exceeded).
    pub failures: Vec<SweepFailure>,
    /// True if the sweep stopped after 4 successive unconverged points, as
    /// XFOIL's sequences do.
    pub stopped: bool,
    /// True if the sweep was cancelled by the caller.
    pub cancelled: bool,
}

/// Progress report passed to the sweep callback after each point.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViscousSweepProgress {
    /// 0-based index of the point just computed.
    pub index: usize,
    /// Number of points of the sweep.
    pub total: usize,
    /// Angle of attack of the point, degrees.
    pub alpha_deg: f64,
    /// Prescribed lift coefficient, for a prescribed-CL sweep.
    pub cl_target: Option<f64>,
    /// The point, if it converged.
    pub point: Option<ViscousPolarPoint>,
    /// Why the point is not part of the polar.
    pub error: Option<String>,
}

/// One operating-point command of an XFOIL session.
#[derive(Debug, Clone, Copy)]
enum Command {
    /// `ALFA` (radians).
    Alfa(f64),
    /// A point of `ASEQ` (radians).
    Aseq(f64),
    /// `CL`.
    Cl(f64),
    /// A point of `CSEQ`.
    Cseq(f64),
}

/// XFOIL's viscous–inviscid analysis of one geometry.
///
/// The solver owns the inviscid solver (influence matrices and their
/// factorization), is immutable and `Send + Sync`. Each call of
/// [`Self::solve`] or [`Self::solve_cl`] starts from a fresh boundary layer
/// (XFOIL `INIT` then `ALFA` or `CL`); [`Self::sweep`] and
/// [`Self::sweep_cl`] reuse the boundary layer of each point as the initial
/// guess of the next one, like XFOIL's `ASEQ` and `CSEQ`, and reproduce
/// XFOIL's sequences. [`Self::session`] gives full control over the warm
/// starts.
///
/// ```
/// use aero_foil2d::viscous::{ViscousOptions, ViscousSolver};
/// use aero_geometry::{NacaOptions, PanelingOptions, naca};
///
/// let airfoil = naca("0012", &NacaOptions::default())?.paneled(&PanelingOptions::default())?;
/// let options = ViscousOptions { reynolds: 1e6, ..ViscousOptions::default() };
/// let solver = ViscousSolver::new(&airfoil, &options)?;
/// let s = solver.solve(4.0)?;
/// assert!(s.converged);
/// assert!((s.cl - 0.4278).abs() < 1e-4 && (s.cd - 0.00728).abs() < 1e-5);
/// let t = solver.solve_cl(0.5)?; // angle of attack found by the solver
/// assert!((t.cl - 0.5).abs() < 1e-3 && t.alpha_deg > 4.0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone)]
pub struct ViscousSolver {
    inviscid: InviscidSolver,
    options: ViscousOptions,
}

impl ViscousSolver {
    /// Builds the solver for a paneled airfoil, with the default inviscid
    /// options (critical Mach number exceeded is an error, moment about the
    /// quarter chord).
    pub fn new(airfoil: &Airfoil, options: &ViscousOptions) -> Result<Self> {
        let inviscid = InviscidSolver::new(airfoil, &InviscidOptions::default())?;
        Self::from_inviscid(inviscid, options)
    }

    /// Builds the solver around an existing inviscid solver, whose options
    /// (moment reference, `allow_supercritical`, flap hinge) apply.
    pub fn from_inviscid(inviscid: InviscidSolver, options: &ViscousOptions) -> Result<Self> {
        options.validate()?;
        Ok(Self {
            inviscid,
            options: *options,
        })
    }

    /// The underlying inviscid solver.
    pub fn inviscid(&self) -> &InviscidSolver {
        &self.inviscid
    }

    /// The options.
    pub fn options(&self) -> &ViscousOptions {
        &self.options
    }

    /// A session keeping the boundary layer from one point to the next.
    pub fn session(&self) -> ViscousSession<'_> {
        ViscousSession {
            solver: self,
            state: Session::new(&self.inviscid, self.parameters()),
        }
    }

    fn parameters(&self) -> Parameters {
        let o = &self.options;
        Parameters {
            reynolds: o.reynolds,
            mach: o.mach,
            variation: match o.variation {
                MachReynoldsVariation::Fixed => Variation::Fixed,
                MachReynoldsVariation::InverseSqrtCl => Variation::InverseSqrtCl,
                MachReynoldsVariation::InverseCl => Variation::InverseCl,
            },
            ncrit: o.ncrit,
            xtr: o.xtr,
            damp2: o.amplification == AmplificationModel::Modified,
            vaccel: o.vaccel,
            wake_length: 1.0,
            moment_reference: self.inviscid.options().moment_reference,
        }
    }

    /// Solves one angle of attack from a fresh boundary layer.
    ///
    /// An unconverged solution is returned with
    /// [`ViscousSolution::converged`] unset. Errors: invalid angle; critical
    /// Mach number exceeded or Kármán–Tsien breakdown, unless the inviscid
    /// options allow supercritical solutions (XFOIL behaviour: the solution
    /// is then returned with [`ViscousSolution::supercritical`] or
    /// [`ViscousSolution::compressibility_breakdown`] set).
    pub fn solve(&self, alpha_deg: f64) -> Result<ViscousSolution> {
        self.session().solve(alpha_deg)
    }

    /// Solves for a prescribed lift coefficient from a fresh boundary layer
    /// (XFOIL `CL`): the angle of attack is a Newton variable of the
    /// viscous solution. Same outcomes as [`Self::solve`].
    pub fn solve_cl(&self, cl: f64) -> Result<ViscousSolution> {
        self.session().solve_cl(cl)
    }

    /// Runs an angle-of-attack sweep with XFOIL's `ASEQ` semantics.
    pub fn sweep(&self, sweep: &AlphaSweep) -> Result<ViscousPolar> {
        self.sweep_with(sweep, |_, _| ControlFlow::Continue(()))
    }

    /// Runs a sweep, calling `on_progress` after each point with the progress
    /// report and the solution (`None` if the point was refused). Returning
    /// [`ControlFlow::Break`] cancels the sweep; the points computed so far
    /// are returned with [`ViscousPolar::cancelled`] set.
    ///
    /// Each point starts from the boundary layer of the previous one, with
    /// [`ViscousOptions::max_iterations`] + 5 Newton iterations; the sweep
    /// stops after 4 successive unconverged points ([`ViscousPolar::stopped`]).
    pub fn sweep_with(
        &self,
        sweep: &AlphaSweep,
        on_progress: impl FnMut(&ViscousSweepProgress, Option<&ViscousSolution>) -> ControlFlow<()>,
    ) -> Result<ViscousPolar> {
        self.session().sweep_with(sweep, on_progress)
    }

    /// Runs a prescribed-CL sweep with XFOIL's `CSEQ` semantics.
    pub fn sweep_cl(&self, sweep: &ClSweep) -> Result<ViscousPolar> {
        self.sweep_cl_with(sweep, |_, _| ControlFlow::Continue(()))
    }

    /// Runs a prescribed-CL sweep, as [`Self::sweep_with`] does for angles.
    pub fn sweep_cl_with(
        &self,
        sweep: &ClSweep,
        on_progress: impl FnMut(&ViscousSweepProgress, Option<&ViscousSolution>) -> ControlFlow<()>,
    ) -> Result<ViscousPolar> {
        self.session().sweep_cl_with(sweep, on_progress)
    }
}

/// A viscous analysis keeping its boundary layer between points, as an
/// XFOIL session does: each [`Self::solve`] or [`Self::solve_cl`] starts
/// from the previous solution (XFOIL `ALFA`, `CL`), [`Self::reset`]
/// discards it (XFOIL `INIT`).
#[derive(Debug, Clone)]
pub struct ViscousSession<'a> {
    solver: &'a ViscousSolver,
    state: Session<'a>,
}

impl ViscousSession<'_> {
    /// Solves one angle of attack starting from the current boundary layer,
    /// with [`ViscousOptions::max_iterations`] Newton iterations.
    pub fn solve(&mut self, alpha_deg: f64) -> Result<ViscousSolution> {
        if !alpha_deg.is_finite() {
            return Err(InviscidError::InvalidAlpha(alpha_deg).into());
        }
        let mut s = self.run(
            Command::Alfa(alpha_deg.to_radians()),
            self.solver.options.max_iterations,
        )?;
        s.alpha_deg = alpha_deg;
        Ok(s)
    }

    /// Solves for a prescribed lift coefficient starting from the current
    /// boundary layer (XFOIL `CL`).
    pub fn solve_cl(&mut self, cl: f64) -> Result<ViscousSolution> {
        if !cl.is_finite() {
            return Err(InviscidError::LiftNotReached(cl).into());
        }
        self.run(Command::Cl(cl), self.solver.options.max_iterations)
    }

    /// Discards the boundary layer: the next point starts from scratch.
    pub fn reset(&mut self) {
        self.state.reset_boundary_layer();
    }

    /// Runs an angle-of-attack sweep from the current boundary layer, like
    /// [`ViscousSolver::sweep_with`] (XFOIL `ASEQ`).
    pub fn sweep_with(
        &mut self,
        sweep: &AlphaSweep,
        on_progress: impl FnMut(&ViscousSweepProgress, Option<&ViscousSolution>) -> ControlFlow<()>,
    ) -> Result<ViscousPolar> {
        let alphas = sweep.alphas()?;
        // XFOIL steps the angle in radians from the first one.
        let dtor = std::f64::consts::PI / 180.0;
        let daa = if sweep.end_deg < sweep.start_deg {
            -sweep.step_deg.abs()
        } else {
            sweep.step_deg.abs()
        };
        let (aa1, daa) = (sweep.start_deg * dtor, daa * dtor);
        self.sequence(
            &alphas,
            false,
            |k| Command::Aseq(aa1 + daa * k as f64),
            on_progress,
        )
    }

    /// Runs a prescribed-CL sweep from the current boundary layer, like
    /// [`ViscousSolver::sweep_cl_with`] (XFOIL `CSEQ`).
    pub fn sweep_cl_with(
        &mut self,
        sweep: &ClSweep,
        on_progress: impl FnMut(&ViscousSweepProgress, Option<&ViscousSolution>) -> ControlFlow<()>,
    ) -> Result<ViscousPolar> {
        let values = sweep.values()?;
        let dcl = if sweep.end < sweep.start {
            -sweep.step.abs()
        } else {
            sweep.step.abs()
        };
        let cl1 = sweep.start;
        self.sequence(
            &values,
            true,
            |k| Command::Cseq(cl1 + dcl * k as f64),
            on_progress,
        )
    }

    /// The sequence loop of XFOIL's `ASEQ` and `CSEQ`.
    fn sequence(
        &mut self,
        values: &[f64],
        prescribed_cl: bool,
        command: impl Fn(usize) -> Command,
        mut on_progress: impl FnMut(&ViscousSweepProgress, Option<&ViscousSolution>) -> ControlFlow<()>,
    ) -> Result<ViscousPolar> {
        const NSEQEX: usize = 4;
        let total = values.len();
        let mut polar = ViscousPolar {
            options: self.solver.options,
            prescribed_cl,
            points: Vec::with_capacity(total),
            failures: Vec::new(),
            stopped: false,
            cancelled: false,
        };
        let itmax = self.solver.options.max_iterations + 5;
        let mut unconverged = 0;
        for (index, &value) in values.iter().enumerate() {
            let result = self.run(command(index), itmax).map(|mut s| {
                if !prescribed_cl {
                    // The nominal angle rather than its radian round trip.
                    s.alpha_deg = value;
                }
                s
            });
            let alpha_deg = match &result {
                Ok(s) => s.alpha_deg,
                Err(_) if prescribed_cl => self.state.alfa.to_degrees(),
                Err(_) => value,
            };
            let cl_target = prescribed_cl.then_some(value);
            let (point, error) = match &result {
                Ok(s) if s.converged => {
                    let p = ViscousPolarPoint::from(s);
                    polar.points.push(p);
                    (Some(p), None)
                }
                Ok(s) => (
                    None,
                    Some(format!(
                        "not converged after {} iterations (rms change {:.2e})",
                        s.iterations, s.rms_change
                    )),
                ),
                Err(e) => (None, Some(e.to_string())),
            };
            if let Some(message) = &error {
                polar.failures.push(SweepFailure {
                    alpha_deg,
                    cl: cl_target,
                    message: message.clone(),
                });
            }
            let progress = ViscousSweepProgress {
                index,
                total,
                alpha_deg,
                cl_target,
                point,
                error,
            };
            if on_progress(&progress, result.as_ref().ok()).is_break() {
                polar.cancelled = index + 1 < total;
                break;
            }
            // XFOIL counts the points whose Newton iteration failed.
            if self.state.lvconv {
                unconverged = 0;
            } else {
                unconverged += 1;
                if unconverged >= NSEQEX {
                    polar.stopped = index + 1 < total;
                    break;
                }
            }
        }
        Ok(polar)
    }

    /// Starts or stops recording XFOIL's console messages: marching
    /// failures, transition points and the convergence history of each
    /// Newton iteration, formatted as XFOIL prints them. Useful to diagnose
    /// a difficult point or to compare with an XFOIL session.
    pub fn record_log(&mut self, enabled: bool) {
        match (enabled, &self.state.trace) {
            (true, None) => self.state.trace = Some(Vec::new()),
            (false, _) => self.state.trace = None,
            _ => {}
        }
    }

    /// The messages recorded since the last call, one line each.
    pub fn take_log(&mut self) -> Vec<String> {
        match &mut self.state.trace {
            Some(lines) => std::mem::take(lines),
            None => Vec::new(),
        }
    }

    fn run(&mut self, command: Command, itmax: usize) -> Result<ViscousSolution> {
        let st = &mut self.state;
        match command {
            Command::Alfa(alfa) => st.alfa_command(alfa),
            Command::Aseq(alfa) => st.aseq_point(alfa),
            Command::Cl(cl) => st.cl_command(cl),
            Command::Cseq(cl) => st.cseq_point(cl),
        }
        let (iterations, converged) = st.viscal(itmax);
        let hinge = self.solver.inviscid.options().flap_hinge;
        let solution = solution(st, iterations, converged, hinge);

        let minf = st.minf;
        let allowed = self.solver.inviscid.options().allow_supercritical;
        if solution.compressibility_breakdown && !allowed {
            return Err(InviscidError::CompressibilityBreakdown { mach: minf }.into());
        }
        if solution.supercritical && !allowed {
            return Err(InviscidError::CriticalMachExceeded {
                mach: minf,
                cp_min: solution.cp_min,
                x: solution.cp_min_x,
                cp_star: solution.cp_star.unwrap_or(f64::NEG_INFINITY),
            }
            .into());
        }
        Ok(solution)
    }
}

/// Builds the public solution from the session state.
fn solution(
    st: &Session<'_>,
    iterations: usize,
    converged: bool,
    hinge: Option<[f64; 2]>,
) -> ViscousSolution {
    let n = st.n;
    let cpv = st.cpv();
    let (imin, cp_min) =
        cpv.iter().enumerate().fold(
            (0, f64::INFINITY),
            |best, (i, &v)| if v < best.1 { (i, v) } else { best },
        );
    let cp_min_x = st.pan.x[imin];
    let mach = st.minf;
    let cp_star = (mach > 0.0).then(|| sonic_cp(mach));
    let supercritical = cp_star.is_some_and(|c| cp_min < c);
    let beta = (1.0 - mach * mach).sqrt();
    let bfac = 0.5 * mach * mach / (1.0 + beta);
    let compressibility_breakdown = mach > 0.0
        && st.qvis[..st.n + st.nw].iter().any(|&q| {
            let cpinc = 1.0 - (q / st.qinf).powi(2);
            beta + bfac * cpinc <= 0.0
        });

    let que = 0.5 * st.qinf * st.qinf;
    let layer = |is: usize, range: std::ops::RangeInclusive<usize>| {
        let side = &st.sides[is];
        let mut b = BoundaryLayer::default();
        for ibl in range {
            let i = side.ipan[ibl];
            b.x.push(st.pan.x[i]);
            b.y.push(st.pan.y[i]);
            b.xi.push(side.xssi[ibl]);
            b.ue.push(side.uedg[ibl]);
            b.cp.push(cpv[i]);
            b.dstar.push(side.dstr[ibl]);
            b.theta.push(side.thet[ibl]);
            b.cf.push(side.tau[ibl] / que);
            b.h.push(side.dstr[ibl] / side.thet[ibl]);
            let laminar = ibl < side.itran;
            b.amplification.push(laminar.then_some(side.ctau[ibl]));
            b.sqrt_ctau.push((!laminar).then_some(side.ctau[ibl]));
        }
        b
    };
    let (top, bot) = (&st.sides[0], &st.sides[1]);
    let transition = |is: usize| {
        let side = &st.sides[is];
        Transition {
            x: side.xoctr,
            y: side.yoctr,
            xi: side.xssitr,
            forced: side.tforce,
        }
    };

    ViscousSolution {
        alpha_deg: st.alfa.to_degrees(),
        reynolds: st.reinf,
        mach,
        cl_target: (!st.lalfa).then_some(st.clspec),
        cl: st.cl,
        cd: st.cd,
        cdp: st.cdp,
        cdf: st.cdf,
        cm: st.cm,
        converged,
        iterations,
        rms_change: st.rmsbl,
        cp: cpv[..n].to_vec(),
        cp_inviscid: st.cpcalc(&st.qinv[..n]),
        surface_speed: st.qvis[..n].to_vec(),
        cp_min,
        cp_min_x,
        cp_star,
        supercritical,
        compressibility_breakdown,
        hinge: hinge.map(|h| hinge_moment(st.geo(), &cpv[..n], h)),
        transition: [transition(0), transition(1)],
        top: layer(0, 2..=top.iblte),
        bottom: layer(1, 2..=bot.iblte),
        wake: layer(1, bot.iblte + 1..=bot.nbl),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aero_geometry::{NacaOptions, PanelingOptions, naca};

    fn solver(options: &ViscousOptions) -> ViscousSolver {
        let af = naca("0012", &NacaOptions::default())
            .unwrap()
            .paneled(&PanelingOptions::default())
            .unwrap();
        ViscousSolver::new(&af, options).unwrap()
    }

    #[test]
    fn solver_is_send_and_sync() {
        fn check<T: Send + Sync>() {}
        check::<ViscousSolver>();
    }

    #[test]
    fn invalid_options_are_refused() {
        let af = naca("0012", &NacaOptions::default())
            .unwrap()
            .paneled(&PanelingOptions::default())
            .unwrap();
        for o in [
            ViscousOptions {
                reynolds: 0.0,
                ..ViscousOptions::default()
            },
            ViscousOptions {
                mach: 1.0,
                ..ViscousOptions::default()
            },
            ViscousOptions {
                ncrit: [9.0, f64::NAN],
                ..ViscousOptions::default()
            },
            ViscousOptions {
                max_iterations: 0,
                ..ViscousOptions::default()
            },
        ] {
            assert!(ViscousSolver::new(&af, &o).is_err(), "{o:?}");
        }
        let s = solver(&ViscousOptions::default());
        assert!(s.solve(f64::NAN).is_err());
    }

    #[test]
    fn symmetric_airfoil_at_zero_incidence() {
        let s = solver(&ViscousOptions::default()).solve(0.0).unwrap();
        assert!(s.converged);
        assert!(s.cl.abs() < 1e-8 && s.cm.abs() < 1e-8);
        // (The paneling is symmetric only to about 1e-7.)
        assert!((s.transition[0].x - s.transition[1].x).abs() < 1e-4);
        assert_eq!(s.top.x.len(), s.bottom.x.len());
        // XFOIL's polar: CD 0.00540, CDp 0.00046.
        assert!((s.cd - 0.00540).abs() < 5e-6 && (s.cdp - 0.00046).abs() < 5e-6);
        assert!(s.cdf > 0.0 && s.cdf < s.cd);
        // Laminar then turbulent along each surface.
        let lam = s
            .top
            .amplification
            .iter()
            .take_while(|v| v.is_some())
            .count();
        assert!(lam > 10 && lam < s.top.x.len());
        assert!(s.top.sqrt_ctau[lam..].iter().all(Option::is_some));
        assert!(s.wake.sqrt_ctau.iter().all(Option::is_some));
        assert!(s.wake.cf.iter().all(|&c| c == 0.0));
    }

    #[test]
    fn sweep_reuses_the_boundary_layer() {
        let solver = solver(&ViscousOptions::default());
        let sweep = AlphaSweep {
            start_deg: 0.0,
            end_deg: 4.0,
            step_deg: 2.0,
        };
        let mut seen = Vec::new();
        let polar = solver
            .sweep_with(&sweep, |p, _| {
                seen.push(p.index);
                ControlFlow::Continue(())
            })
            .unwrap();
        assert_eq!(seen, [0, 1, 2]);
        assert_eq!(polar.points.len(), 3);
        assert!(!polar.stopped && !polar.cancelled);
        // Warm and cold starts converge to the same solution.
        let cold = solver.solve(4.0).unwrap();
        assert!((polar.points[2].cl - cold.cl).abs() < 1e-4);
        assert!((polar.points[2].cd - cold.cd).abs() < 1e-6);

        let cancelled = solver
            .sweep_with(&sweep, |_, _| ControlFlow::Break(()))
            .unwrap();
        assert!(cancelled.cancelled);
        assert_eq!(cancelled.points.len(), 1);
    }

    #[test]
    fn session_warm_start_and_reset() {
        let solver = solver(&ViscousOptions::default());
        let mut session = solver.session();
        let a = session.solve(2.0).unwrap();
        let b = session.solve(3.0).unwrap();
        session.reset();
        let c = session.solve(3.0).unwrap();
        assert!(a.converged && b.converged && c.converged);
        assert!(b.iterations < c.iterations);
        assert_eq!(c, solver.solve(3.0).unwrap());
    }
}
