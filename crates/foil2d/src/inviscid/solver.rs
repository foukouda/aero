//! The inviscid panel solver (XFOIL `GGCALC`, `SPECAL`, `CLCALC`, `CPCALC`).

use std::ops::ControlFlow;
use std::sync::OnceLock;

use aero_geometry::Airfoil;
use faer::linalg::solvers::{PartialPivLu, Solve};
use faer::{Mat, MatRef};
use serde::{Deserialize, Serialize};

use super::error::{InviscidError, Result};
use super::hinge::{HingeMoment, hinge_moment};
use super::panel::PanelGeometry;
use super::psilin::{FieldPoint, Influence, psilin};

/// Ratio of specific heats of air (XFOIL `GAMMA`).
const GAMMA: f64 = 1.4;

/// Distance of the internal control point ahead of a sharp trailing edge,
/// as a fraction of the smaller adjacent panel length (XFOIL `BWT`).
const BWT: f64 = 0.1;

/// Solver options.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InviscidOptions {
    /// When false (default), a solution whose minimum pressure coefficient is
    /// below the sonic value `Cp*` (critical Mach number exceeded) is an
    /// error. When true, such solutions are returned with
    /// [`InviscidSolution::supercritical`] set, as XFOIL does; only a
    /// breakdown of the Kármán–Tsien correction is then an error.
    pub allow_supercritical: bool,
    /// Moment reference point (XFOIL `XCMREF`, `YCMREF`), in airfoil
    /// coordinates. Defaults to the quarter chord `(0.25, 0)`.
    pub moment_reference: [f64; 2],
    /// Flap hinge point (XFOIL `FNEW`): when set, the solutions include the
    /// hinge moment of the part of the airfoil aft of it.
    pub flap_hinge: Option<[f64; 2]>,
}

impl Default for InviscidOptions {
    fn default() -> Self {
        Self {
            allow_supercritical: false,
            moment_reference: [0.25, 0.0],
            flap_hinge: None,
        }
    }
}

/// Freestream conditions of one analysis.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OperatingPoint {
    /// Angle of attack in degrees.
    pub alpha_deg: f64,
    /// Freestream Mach number, `0 ≤ M < 1` (0: incompressible).
    pub mach: f64,
}

impl OperatingPoint {
    /// Incompressible operating point.
    pub fn incompressible(alpha_deg: f64) -> Self {
        Self {
            alpha_deg,
            mach: 0.0,
        }
    }
}

/// Inviscid solution at one operating point.
///
/// Force coefficients are integrated like XFOIL's `CLCALC`: they are
/// referred to a unit chord in airfoil coordinates (normalize the airfoil to
/// get conventional coefficients).
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InviscidSolution {
    /// Angle of attack in degrees.
    pub alpha_deg: f64,
    /// Freestream Mach number.
    pub mach: f64,
    /// Lift coefficient from pressure integration.
    pub cl: f64,
    /// Pitching moment coefficient about the moment reference point.
    pub cm: f64,
    /// Pressure drag coefficient (zero in exact inviscid flow; its value
    /// measures the discretization error).
    pub cdp: f64,
    /// Lift-curve slope `dCl/dα` per radian.
    pub cl_alpha: f64,
    /// Pressure coefficient at each node (Kármán–Tsien corrected).
    pub cp: Vec<f64>,
    /// Incompressible surface speed `Q/Q∞` at each node (XFOIL `QINV`),
    /// signed: positive in the direction of increasing node index.
    pub surface_speed: Vec<f64>,
    /// Surface speed with the Kármán–Tsien correction (XFOIL `QCOMP`).
    pub surface_speed_compressible: Vec<f64>,
    /// Minimum pressure coefficient.
    pub cp_min: f64,
    /// x coordinate of the minimum pressure coefficient.
    pub cp_min_x: f64,
    /// Node index of the minimum pressure coefficient.
    pub cp_min_index: usize,
    /// Sonic pressure coefficient `Cp*`, if `M > 0`.
    pub cp_star: Option<f64>,
    /// True if `Cp min < Cp*` (only possible with
    /// [`InviscidOptions::allow_supercritical`]).
    pub supercritical: bool,
    /// Flap hinge moment, if a hinge is set ([`InviscidOptions::flap_hinge`]).
    pub hinge: Option<HingeMoment>,
}

/// An angle-of-attack sweep, with the semantics of XFOIL's `ASEQ`.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AlphaSweep {
    /// First angle of attack, degrees.
    pub start_deg: f64,
    /// Last angle of attack, degrees (included when it falls on the grid).
    pub end_deg: f64,
    /// Increment, degrees; its sign is adjusted to go from start to end.
    pub step_deg: f64,
}

impl AlphaSweep {
    /// Largest number of points of a sweep.
    pub const MAX_POINTS: usize = 100_000;

    /// The angles of attack of the sweep (XFOIL: `N = INT((A2 − A1)/ΔA + 0.5) + 1`).
    pub fn alphas(&self) -> Result<Vec<f64>> {
        sweep_values(self.start_deg, self.end_deg, self.step_deg)
    }
}

/// A sweep of prescribed lift coefficients, with the semantics of XFOIL's
/// `CSEQ`.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ClSweep {
    /// First lift coefficient.
    pub start: f64,
    /// Last lift coefficient (included when it falls on the grid).
    pub end: f64,
    /// Increment; its sign is adjusted to go from start to end.
    pub step: f64,
}

impl ClSweep {
    /// The lift coefficients of the sweep (XFOIL: `N = INT((CL2 − CL1)/ΔCL + 0.5) + 1`).
    pub fn values(&self) -> Result<Vec<f64>> {
        sweep_values(self.start, self.end, self.step)
    }
}

/// Values `a1 + k·step` from `a1` to `a2`, as XFOIL's sequences.
fn sweep_values(a1: f64, a2: f64, step_deg: f64) -> Result<Vec<f64>> {
    if !(a1.is_finite() && a2.is_finite() && step_deg.is_finite()) {
        return Err(InviscidError::InvalidSweep("values must be finite".into()));
    }
    if step_deg == 0.0 {
        if a1 == a2 {
            return Ok(vec![a1]);
        }
        return Err(InviscidError::InvalidSweep(
            "the step must not be zero".into(),
        ));
    }
    let step = if a2 < a1 {
        -step_deg.abs()
    } else {
        step_deg.abs()
    };
    let count = ((a2 - a1) / step + 0.5).floor() + 1.0;
    if count > AlphaSweep::MAX_POINTS as f64 {
        return Err(InviscidError::InvalidSweep(format!(
            "too many points ({count}), at most {}",
            AlphaSweep::MAX_POINTS
        )));
    }
    Ok((0..count as usize).map(|k| a1 + step * k as f64).collect())
}

/// Coefficients of one converged point of a polar.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PolarPoint {
    /// Angle of attack, degrees.
    pub alpha_deg: f64,
    /// Lift coefficient.
    pub cl: f64,
    /// Moment coefficient.
    pub cm: f64,
    /// Pressure drag coefficient.
    pub cdp: f64,
    /// Minimum pressure coefficient.
    pub cp_min: f64,
    /// Its x location.
    pub cp_min_x: f64,
    /// True if the critical Mach number is exceeded.
    pub supercritical: bool,
    /// Flap hinge moment coefficient, if a hinge is set.
    #[serde(default)]
    pub hinge_moment: Option<f64>,
}

impl From<&InviscidSolution> for PolarPoint {
    fn from(s: &InviscidSolution) -> Self {
        Self {
            alpha_deg: s.alpha_deg,
            cl: s.cl,
            cm: s.cm,
            cdp: s.cdp,
            cp_min: s.cp_min,
            cp_min_x: s.cp_min_x,
            supercritical: s.supercritical,
            hinge_moment: s.hinge.map(|h| h.moment),
        }
    }
}

/// A point of a sweep that could not be computed.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SweepFailure {
    /// Angle of attack, degrees (the last one reached for a prescribed-CL
    /// point).
    pub alpha_deg: f64,
    /// Prescribed lift coefficient, for a prescribed-CL sweep.
    #[serde(default)]
    pub cl: Option<f64>,
    /// Human readable reason.
    pub message: String,
}

/// Result of an angle-of-attack sweep.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Polar {
    /// Freestream Mach number.
    pub mach: f64,
    /// Computed points, in sweep order.
    pub points: Vec<PolarPoint>,
    /// Points that failed (for instance beyond the critical Mach number).
    pub failures: Vec<SweepFailure>,
    /// True if the sweep was cancelled before completion.
    pub cancelled: bool,
}

/// Progress report passed to the sweep callback after each point.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SweepProgress {
    /// 0-based index of the point just computed.
    pub index: usize,
    /// Total number of points of the sweep.
    pub total: usize,
    /// Angle of attack of the point, degrees.
    pub alpha_deg: f64,
    /// The computed point, or `None` if it failed.
    pub point: Option<PolarPoint>,
    /// Reason of the failure when `point` is `None`.
    pub error: Option<String>,
}

/// XFOIL's linear-vorticity streamfunction panel method for one geometry.
///
/// Construction assembles the influence matrix (`GGCALC`), factorizes it
/// once with a dense LU decomposition, and solves for the two base
/// solutions `α = 0°` and `α = 90°`. Each operating point is then a
/// superposition costing `O(N)`. The solver is immutable and `Send + Sync`:
/// it can be shared between threads to run analyses in parallel.
///
/// The source influence matrix, only needed by a viscous coupling, is
/// computed on first access.
#[derive(Debug, Clone)]
pub struct InviscidSolver {
    geometry: PanelGeometry,
    options: InviscidOptions,
    aij: Mat<f64>,
    bij: OnceLock<Mat<f64>>,
    dij: OnceLock<Mat<f64>>,
    lu: PartialPivLu<f64>,
    gamu: Mat<f64>,
}

/// Control point and bisector direction `(cos, sin)` of the sharp
/// trailing-edge condition (XFOIL `GGCALC`), `None` for a blunt edge.
pub(crate) fn te_bisector(geometry: &PanelGeometry) -> Option<(FieldPoint, f64, f64)> {
    if !geometry.te_panel.sharp {
        return None;
    }
    let n = geometry.len();
    let (x, y) = (&geometry.x, &geometry.y);
    let ag1 = (-geometry.yp[0]).atan2(-geometry.xp[0]);
    let ag2 = atanc(geometry.yp[n - 1], geometry.xp[n - 1], ag1);
    let abis = 0.5 * (ag1 + ag2);
    let (sbis, cbis) = abis.sin_cos();

    // Minimum panel length adjacent to the trailing edge.
    let ds1 = (x[0] - x[1]).hypot(y[0] - y[1]);
    let ds2 = (x[n - 1] - x[n - 2]).hypot(y[n - 1] - y[n - 2]);
    let dsmin = ds1.min(ds2);
    let [xte, yte] = geometry.te;
    let point = FieldPoint {
        node: None,
        x: xte - BWT * dsmin * cbis,
        y: yte - BWT * dsmin * sbis,
        nx: -sbis,
        ny: cbis,
    };
    Some((point, cbis, sbis))
}

/// Source influence matrix `BIJ` (XFOIL `GGCALC` with `SIGLIN`).
fn assemble_source_influence(geometry: &PanelGeometry) -> Mat<f64> {
    let n = geometry.len();
    let mut bij = Mat::<f64>::zeros(n + 1, n);
    let mut inf = Influence::new(n);
    for i in 0..n {
        let point = FieldPoint {
            node: Some(i),
            x: geometry.x[i],
            y: geometry.y[i],
            nx: geometry.nx[i],
            ny: geometry.ny[i],
        };
        psilin(geometry, &point, true, &mut inf);
        for j in 0..n {
            bij[(i, j)] = -inf.dpsi_dsigma[j];
        }
    }
    if let Some((point, _, _)) = te_bisector(geometry) {
        psilin(geometry, &point, true, &mut inf);
        for j in 0..n {
            bij[(n - 1, j)] = -inf.dq_dsigma[j];
        }
    }
    bij
}

impl InviscidSolver {
    /// Builds the solver for an airfoil whose points are the panel nodes
    /// (panel it first with [`aero_geometry::panel`] to reproduce XFOIL).
    pub fn new(airfoil: &Airfoil, options: &InviscidOptions) -> Result<Self> {
        if !(options.moment_reference[0].is_finite() && options.moment_reference[1].is_finite()) {
            return Err(InviscidError::Geometry(
                aero_geometry::GeometryError::InvalidOption(
                    "moment reference must be finite".into(),
                ),
            ));
        }
        let geometry = PanelGeometry::new(airfoil)?;
        let n = geometry.len();
        let x = &geometry.x;
        let y = &geometry.y;

        let mut aij = Mat::<f64>::zeros(n + 1, n + 1);
        let mut rhs = Mat::<f64>::zeros(n + 1, 2);
        let mut inf = Influence::new(n);

        // psi(x_i) - psi_0 = 0 at every node, for alpha = 0 and 90 degrees.
        for i in 0..n {
            let point = FieldPoint {
                node: Some(i),
                x: x[i],
                y: y[i],
                nx: geometry.nx[i],
                ny: geometry.ny[i],
            };
            psilin(&geometry, &point, false, &mut inf);
            for j in 0..n {
                aij[(i, j)] = inf.dpsi_dgamma[j];
            }
            aij[(i, n)] = -1.0;
            rhs[(i, 0)] = -y[i];
            rhs[(i, 1)] = x[i];
        }

        // Kutta condition: gamma_1 + gamma_N = 0.
        aij[(n, 0)] = 1.0;
        aij[(n, n - 1)] = 1.0;

        if let Some((point, cbis, sbis)) = te_bisector(&geometry) {
            // Zero internal velocity in the trailing-edge corner: the last
            // node equation is replaced by a zero velocity component along
            // the bisector at a point just inside the trailing edge.
            psilin(&geometry, &point, false, &mut inf);
            for j in 0..n {
                aij[(n - 1, j)] = inf.dq_dgamma[j];
            }
            aij[(n - 1, n)] = 0.0;
            rhs[(n - 1, 0)] = -cbis;
            rhs[(n - 1, 1)] = -sbis;
        }

        let lu = aij.partial_piv_lu();
        let u = lu.U();
        let pivot_max = (0..=n).fold(0.0_f64, |m, i| m.max(u[(i, i)].abs()));
        let pivot_min = (0..=n).fold(f64::INFINITY, |m, i| m.min(u[(i, i)].abs()));
        if !(pivot_max.is_finite() && pivot_min > 1e-13 * pivot_max) {
            return Err(InviscidError::SingularMatrix);
        }
        lu.solve_in_place(rhs.as_mut());
        if (0..=n).any(|i| !(rhs[(i, 0)].is_finite() && rhs[(i, 1)].is_finite())) {
            return Err(InviscidError::SingularMatrix);
        }

        Ok(Self {
            geometry,
            options: *options,
            aij,
            bij: OnceLock::new(),
            dij: OnceLock::new(),
            lu,
            gamu: rhs,
        })
    }

    /// Panel geometry (nodes, normals, trailing-edge panel).
    pub fn geometry(&self) -> &PanelGeometry {
        &self.geometry
    }

    /// Options the solver was built with.
    pub fn options(&self) -> &InviscidOptions {
        &self.options
    }

    /// Number of panel nodes.
    pub fn len(&self) -> usize {
        self.geometry.len()
    }

    /// Always false.
    pub fn is_empty(&self) -> bool {
        self.geometry.is_empty()
    }

    /// The `(N+1)×(N+1)` streamfunction influence matrix of the nodal vortex
    /// strengths and of `ψ₀` (XFOIL `AIJ` before factorization), including
    /// the Kutta condition row.
    pub fn influence_matrix(&self) -> MatRef<'_, f64> {
        self.aij.as_ref()
    }

    /// The `(N+1)×N` source influence matrix (XFOIL `BIJ`):
    /// `−∂(residual)/∂σ_j` for the airfoil mass defects, needed by the
    /// viscous coupling. Computed on first access.
    pub fn source_influence_matrix(&self) -> MatRef<'_, f64> {
        self.bij
            .get_or_init(|| assemble_source_influence(&self.geometry))
            .as_ref()
    }

    /// The `N×N` matrix `∂γ_i/∂σ_j` of the surface speeds with respect to the
    /// airfoil mass defects, `AIJ⁻¹·BIJ` without its `ψ₀` row (XFOIL `DIJ`
    /// for airfoil nodes, `QDCALC`). Computed on first access.
    pub fn source_velocity_matrix(&self) -> MatRef<'_, f64> {
        self.dij
            .get_or_init(|| {
                let n = self.len();
                let mut x = self.source_influence_matrix().to_owned();
                self.lu.solve_in_place(x.as_mut());
                x.subrows(0, n).to_owned()
            })
            .as_ref()
    }

    /// LU factorization of [`Self::influence_matrix`].
    pub fn lu(&self) -> &PartialPivLu<f64> {
        &self.lu
    }

    /// The `(N+1)×2` base solutions: columns are `γ` (and `ψ₀` in the last
    /// row) for `α = 0°` and `α = 90°` (XFOIL `GAMU`).
    pub fn base_solutions(&self) -> MatRef<'_, f64> {
        self.gamu.as_ref()
    }

    /// Influence coefficients at an arbitrary field point (XFOIL `PSILIN`).
    pub fn influence_at(&self, point: &FieldPoint, sources: bool) -> Influence {
        let mut inf = Influence::new(self.len());
        psilin(&self.geometry, point, sources, &mut inf);
        inf
    }

    /// Solves one operating point (XFOIL `SPECAL` with fixed Mach number).
    pub fn solve(&self, op: OperatingPoint) -> Result<InviscidSolution> {
        if !op.alpha_deg.is_finite() {
            return Err(InviscidError::InvalidAlpha(op.alpha_deg));
        }
        validate_mach(op.mach)?;
        let n = self.len();
        let alfa = op.alpha_deg.to_radians();
        let (sina, cosa) = alfa.sin_cos();

        let mut gam = vec![0.0; n];
        let mut gam_a = vec![0.0; n];
        for i in 0..n {
            let (g0, g90) = (self.gamu[(i, 0)], self.gamu[(i, 1)]);
            gam[i] = cosa * g0 + sina * g90;
            gam_a[i] = -sina * g0 + cosa * g90;
        }

        let minf = op.mach;
        let beta = (1.0 - minf * minf).sqrt();
        let bfac = 0.5 * minf * minf / (1.0 + beta);

        // CPCALC.
        let mut cp = Vec::with_capacity(n);
        let mut denneg = false;
        for &g in &gam {
            let cpinc = 1.0 - g * g;
            let den = beta + bfac * cpinc;
            if den <= 0.0 {
                denneg = true;
            }
            cp.push(cpinc / den);
        }
        if denneg {
            return Err(InviscidError::CompressibilityBreakdown { mach: minf });
        }

        let coefficients = clcalc(
            &self.geometry.x,
            &self.geometry.y,
            &gam,
            &gam_a,
            alfa,
            minf,
            self.options.moment_reference,
        );

        // FCPMIN.
        let (cp_min_index, cp_min) =
            cp.iter().enumerate().fold(
                (0, cp[0]),
                |best, (i, &v)| if v < best.1 { (i, v) } else { best },
            );
        let cp_min_x = self.geometry.x[cp_min_index];

        let cp_star = (minf > 0.0).then(|| sonic_cp(minf));
        let supercritical = cp_star.is_some_and(|c| cp_min < c);
        if supercritical && !self.options.allow_supercritical {
            return Err(InviscidError::CriticalMachExceeded {
                mach: minf,
                cp_min,
                x: cp_min_x,
                cp_star: cp_star.unwrap_or(f64::NEG_INFINITY),
            });
        }

        let cp_for_hinge = cp.clone();
        let tklam = minf * minf / ((1.0 + beta) * (1.0 + beta));
        let surface_speed_compressible = gam
            .iter()
            .map(|&g| g * (1.0 - tklam) / (1.0 - tklam * g * g))
            .collect();

        Ok(InviscidSolution {
            alpha_deg: op.alpha_deg,
            mach: minf,
            cl: coefficients.cl,
            cm: coefficients.cm,
            cdp: coefficients.cdp,
            cl_alpha: coefficients.cl_alpha,
            cp,
            surface_speed: gam,
            surface_speed_compressible,
            cp_min,
            cp_min_x,
            cp_min_index,
            cp_star,
            supercritical,
            hinge: self
                .options
                .flap_hinge
                .map(|h| hinge_moment(&self.geometry, &cp_for_hinge, h)),
        })
    }

    /// Solves for a prescribed lift coefficient (XFOIL `CLI`, `SPECCL`):
    /// Newton iteration on α from α = 0. Fails if the lift cannot be
    /// reached.
    pub fn solve_cl(&self, cl: f64, mach: f64) -> Result<InviscidSolution> {
        if !cl.is_finite() {
            return Err(InviscidError::LiftNotReached(cl));
        }
        validate_mach(mach)?;
        let n = self.len();
        let mut alfa: f64 = 0.0;
        let mut gam = vec![0.0; n];
        let mut gam_a = vec![0.0; n];
        let mut coefficients = |alfa: f64| {
            let (sina, cosa) = alfa.sin_cos();
            for i in 0..n {
                let (g0, g90) = (self.gamu[(i, 0)], self.gamu[(i, 1)]);
                gam[i] = cosa * g0 + sina * g90;
                gam_a[i] = -sina * g0 + cosa * g90;
            }
            clcalc(
                &self.geometry.x,
                &self.geometry.y,
                &gam,
                &gam_a,
                alfa,
                mach,
                self.options.moment_reference,
            )
        };
        let mut c = coefficients(alfa);
        for _ in 0..20 {
            let dalfa = (cl - c.cl) / c.cl_alpha;
            alfa += dalfa;
            c = coefficients(alfa);
            if dalfa.abs() <= 1.0e-6 {
                break;
            }
        }
        let error = (c.cl - cl).abs();
        if error.is_nan() || error > 1.0e-6 * cl.abs().max(1.0) {
            return Err(InviscidError::LiftNotReached(cl));
        }
        self.solve(OperatingPoint {
            alpha_deg: alfa.to_degrees(),
            mach,
        })
    }

    /// Runs a prescribed-CL sweep (XFOIL `CSEQ` without the boundary layer).
    pub fn sweep_cl(&self, sweep: &ClSweep, mach: f64) -> Result<Polar> {
        self.sweep_cl_with(sweep, mach, |_| ControlFlow::Continue(()))
    }

    /// Runs a prescribed-CL sweep, calling `on_progress` after each point
    /// as [`Self::sweep_with`] does.
    pub fn sweep_cl_with(
        &self,
        sweep: &ClSweep,
        mach: f64,
        on_progress: impl FnMut(&SweepProgress) -> ControlFlow<()>,
    ) -> Result<Polar> {
        validate_mach(mach)?;
        let values = sweep.values()?;
        self.run_sweep(&values, mach, true, on_progress)
    }

    /// Runs an angle-of-attack sweep reusing the factorization.
    pub fn sweep(&self, sweep: &AlphaSweep, mach: f64) -> Result<Polar> {
        self.sweep_with(sweep, mach, |_| ControlFlow::Continue(()))
    }

    /// Runs a sweep, calling `on_progress` after each point. Returning
    /// [`ControlFlow::Break`] cancels the sweep; the points computed so far
    /// are returned with [`Polar::cancelled`] set.
    pub fn sweep_with(
        &self,
        sweep: &AlphaSweep,
        mach: f64,
        on_progress: impl FnMut(&SweepProgress) -> ControlFlow<()>,
    ) -> Result<Polar> {
        validate_mach(mach)?;
        let alphas = sweep.alphas()?;
        self.run_sweep(&alphas, mach, false, on_progress)
    }

    fn run_sweep(
        &self,
        values: &[f64],
        mach: f64,
        prescribed_cl: bool,
        mut on_progress: impl FnMut(&SweepProgress) -> ControlFlow<()>,
    ) -> Result<Polar> {
        let total = values.len();
        let mut polar = Polar {
            mach,
            points: Vec::with_capacity(total),
            failures: Vec::new(),
            cancelled: false,
        };
        for (index, &value) in values.iter().enumerate() {
            let result = if prescribed_cl {
                self.solve_cl(value, mach)
            } else {
                self.solve(OperatingPoint {
                    alpha_deg: value,
                    mach,
                })
            };
            let alpha_deg = match &result {
                Ok(s) => s.alpha_deg,
                Err(_) if prescribed_cl => f64::NAN,
                Err(_) => value,
            };
            let cl = prescribed_cl.then_some(value);
            let (point, error) = match result {
                Ok(solution) => {
                    let p = PolarPoint::from(&solution);
                    polar.points.push(p);
                    (Some(p), None)
                }
                Err(e) => {
                    let message = e.to_string();
                    polar.failures.push(SweepFailure {
                        alpha_deg,
                        cl,
                        message: message.clone(),
                    });
                    (None, Some(message))
                }
            };
            let progress = SweepProgress {
                index,
                total,
                alpha_deg,
                point,
                error,
            };
            if on_progress(&progress).is_break() {
                polar.cancelled = index + 1 < total;
                break;
            }
        }
        Ok(polar)
    }
}

fn validate_mach(mach: f64) -> Result<()> {
    if (0.0..1.0).contains(&mach) {
        Ok(())
    } else {
        Err(InviscidError::InvalidMach(mach))
    }
}

/// Sonic pressure coefficient (XFOIL `COMSET`).
pub fn sonic_cp(mach: f64) -> f64 {
    let gm1 = GAMMA - 1.0;
    2.0 / (GAMMA * mach * mach)
        * (((1.0 + 0.5 * gm1 * mach * mach) / (1.0 + 0.5 * gm1)).powf(GAMMA / gm1) - 1.0)
}

/// XFOIL `ATANC`.
fn atanc(y: f64, x: f64, thold: f64) -> f64 {
    use std::f64::consts::{PI, TAU};
    let dthet = y.atan2(x) - thold;
    thold + dthet - TAU * ((dthet + PI.copysign(dthet)) / TAU).trunc()
}

pub(crate) struct Coefficients {
    pub cl: f64,
    pub cm: f64,
    pub cdp: f64,
    pub cl_alpha: f64,
    /// dCL/d(M∞²).
    pub cl_msq: f64,
}

/// XFOIL `CLCALC`: trapezoidal integration of the (compressible) surface
/// pressures around the contour, trailing-edge gap segment included.
pub(crate) fn clcalc(
    x: &[f64],
    y: &[f64],
    gam: &[f64],
    gam_a: &[f64],
    alfa: f64,
    minf: f64,
    reference: [f64; 2],
) -> Coefficients {
    let n = x.len();
    let (sa, ca) = alfa.sin_cos();
    let [xref, yref] = reference;
    let beta = (1.0 - minf * minf).sqrt();
    let beta_msq = -0.5 / beta;
    let bfac = 0.5 * minf * minf / (1.0 + beta);
    let bfac_msq = 0.5 / (1.0 + beta) - bfac / (1.0 + beta) * beta_msq;

    let pressure = |g: f64, g_a: f64| {
        let cginc = 1.0 - g * g;
        let cpg = cginc / (beta + bfac * cginc);
        let cpg_msq = -cpg / (beta + bfac * cginc) * (beta_msq + bfac_msq * cginc);
        let cpi_gam = -2.0 * g;
        let cpc_cpi = (1.0 - bfac * cpg) / (beta + bfac * cginc);
        (cpg, cpc_cpi * cpi_gam * g_a, cpg_msq)
    };

    let mut cl = 0.0;
    let mut cm = 0.0;
    let mut cdp = 0.0;
    let mut cl_alpha = 0.0;
    let mut cl_msq = 0.0;
    let (mut cpg1, mut cpg1_alf, mut cpg1_msq) = pressure(gam[0], gam_a[0]);
    for i in 0..n {
        let ip = if i + 1 == n { 0 } else { i + 1 };
        let (cpg2, cpg2_alf, cpg2_msq) = pressure(gam[ip], gam_a[ip]);

        let dx = (x[ip] - x[i]) * ca + (y[ip] - y[i]) * sa;
        let dy = (y[ip] - y[i]) * ca - (x[ip] - x[i]) * sa;
        let dg = cpg2 - cpg1;

        let ax = (0.5 * (x[ip] + x[i]) - xref) * ca + (0.5 * (y[ip] + y[i]) - yref) * sa;
        let ay = (0.5 * (y[ip] + y[i]) - yref) * ca - (0.5 * (x[ip] + x[i]) - xref) * sa;
        let ag = 0.5 * (cpg2 + cpg1);

        let dx_alf = -(x[ip] - x[i]) * sa + (y[ip] - y[i]) * ca;
        let ag_alf = 0.5 * (cpg2_alf + cpg1_alf);

        cl += dx * ag;
        cdp -= dy * ag;
        cm -= dx * (ag * ax + dg * dx / 12.0) + dy * (ag * ay + dg * dy / 12.0);
        cl_alpha += dx * ag_alf + ag * dx_alf;
        cl_msq += dx * 0.5 * (cpg2_msq + cpg1_msq);

        cpg1 = cpg2;
        cpg1_alf = cpg2_alf;
        cpg1_msq = cpg2_msq;
    }
    Coefficients {
        cl,
        cm,
        cdp,
        cl_alpha,
        cl_msq,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aero_geometry::{NacaOptions, PanelingOptions, TrailingEdge, naca};

    fn solver(code: &str, te: TrailingEdge) -> InviscidSolver {
        let af = naca(
            code,
            &NacaOptions {
                trailing_edge: te,
                ..NacaOptions::default()
            },
        )
        .unwrap()
        .paneled(&PanelingOptions::default())
        .unwrap();
        InviscidSolver::new(&af, &InviscidOptions::default()).unwrap()
    }

    #[test]
    fn solver_is_send_and_sync() {
        fn check<T: Send + Sync>() {}
        check::<InviscidSolver>();
    }

    #[test]
    fn symmetric_airfoil_at_zero_alpha_has_no_lift() {
        for te in [TrailingEdge::Open, TrailingEdge::Closed] {
            let s = solver("0012", te);
            let sol = s.solve(OperatingPoint::incompressible(0.0)).unwrap();
            assert!(sol.cl.abs() < 1e-10, "{te:?}: cl {}", sol.cl);
            assert!(sol.cm.abs() < 1e-10, "{te:?}: cm {}", sol.cm);
            let a = s.solve(OperatingPoint::incompressible(5.0)).unwrap();
            let b = s.solve(OperatingPoint::incompressible(-5.0)).unwrap();
            assert!((a.cl + b.cl).abs() < 1e-10);
            assert!((a.cm + b.cm).abs() < 1e-10);
        }
    }

    #[test]
    fn superposition_matches_direct_solution() {
        // gamma(alpha) must satisfy A gamma = rhs(alpha) directly.
        let s = solver("2412", TrailingEdge::Open);
        let alpha: f64 = 7.0;
        let sol = s.solve(OperatingPoint::incompressible(alpha)).unwrap();
        let n = s.len();
        let (sa, ca) = alpha.to_radians().sin_cos();
        let a = s.influence_matrix();
        let psi0 = ca * s.base_solutions()[(n, 0)] + sa * s.base_solutions()[(n, 1)];
        for i in 0..n {
            let mut lhs = -psi0;
            for j in 0..n {
                lhs += a[(i, j)] * sol.surface_speed[j];
            }
            let rhs = -(ca * s.geometry().y()[i] - sa * s.geometry().x()[i]);
            assert!((lhs - rhs).abs() < 1e-12, "row {i}: {lhs} vs {rhs}");
        }
        // Kutta condition.
        assert!((sol.surface_speed[0] + sol.surface_speed[n - 1]).abs() < 1e-12);
    }

    #[test]
    fn interior_streamfunction_equals_body_value() {
        // psi = psi_0 on the nodes implies psi ~ psi_0 inside the body.
        for te in [TrailingEdge::Open, TrailingEdge::Closed] {
            let s = solver("2412", te);
            let alpha: f64 = 5.0;
            let sol = s.solve(OperatingPoint::incompressible(alpha)).unwrap();
            let n = s.len();
            let (sa, ca) = alpha.to_radians().sin_cos();
            let psi0 = ca * s.base_solutions()[(n, 0)] + sa * s.base_solutions()[(n, 1)];
            for (px, py) in [(0.3, 0.03), (0.6, 0.02), (0.05, 0.0)] {
                let inf = s.influence_at(
                    &FieldPoint {
                        node: None,
                        x: px,
                        y: py,
                        nx: 0.0,
                        ny: 1.0,
                    },
                    false,
                );
                let psi: f64 = inf
                    .dpsi_dgamma
                    .iter()
                    .zip(&sol.surface_speed)
                    .map(|(a, g)| a * g)
                    .sum::<f64>()
                    + ca * py
                    - sa * px;
                assert!(
                    (psi - psi0).abs() < 1e-4,
                    "{te:?} ({px}, {py}): {psi} vs {psi0}"
                );
            }
        }
    }

    #[test]
    fn matrices_have_expected_shapes() {
        let s = solver("0012", TrailingEdge::Open);
        let n = s.len();
        assert_eq!(s.influence_matrix().nrows(), n + 1);
        assert_eq!(s.influence_matrix().ncols(), n + 1);
        assert_eq!(s.source_influence_matrix().nrows(), n + 1);
        assert_eq!(s.source_influence_matrix().ncols(), n);
        // Computed lazily, once, with non-trivial coefficients; the Kutta
        // row has no source influence.
        assert!(s.source_influence_matrix()[(10, 10)].abs() > 0.0);
        assert!((0..n).all(|j| s.source_influence_matrix()[(n, j)] == 0.0));
        let sharp = solver("0012", TrailingEdge::Closed);
        assert!(sharp.source_influence_matrix()[(n - 1, 5)].abs() > 0.0);
        assert_eq!(s.base_solutions().ncols(), 2);
        assert_eq!(s.lu().U().nrows(), n + 1);
        assert!(!s.is_empty());
        assert_eq!(s.options().moment_reference, [0.25, 0.0]);
    }

    #[test]
    fn compressibility_and_critical_mach() {
        let s = solver("0012", TrailingEdge::Open);
        let inc = s.solve(OperatingPoint::incompressible(2.0)).unwrap();
        let com = s
            .solve(OperatingPoint {
                alpha_deg: 2.0,
                mach: 0.3,
            })
            .unwrap();
        // Karman-Tsien increases |Cl| roughly like the Prandtl-Glauert factor.
        let ratio = com.cl / inc.cl;
        assert!(ratio > 1.03 && ratio < 1.07, "{ratio}");
        assert!(com.cp_star.is_some() && !com.supercritical);

        let err = s
            .solve(OperatingPoint {
                alpha_deg: 8.0,
                mach: 0.6,
            })
            .unwrap_err();
        assert!(
            matches!(err, InviscidError::CriticalMachExceeded { .. }),
            "{err}"
        );

        let lax = InviscidSolver::new(
            &naca("0012", &NacaOptions::default())
                .unwrap()
                .paneled(&PanelingOptions::default())
                .unwrap(),
            &InviscidOptions {
                allow_supercritical: true,
                ..InviscidOptions::default()
            },
        )
        .unwrap();
        let sup = lax
            .solve(OperatingPoint {
                alpha_deg: 8.0,
                mach: 0.6,
            })
            .unwrap();
        assert!(sup.supercritical);

        for bad in [-0.1, 1.0, f64::NAN] {
            assert!(matches!(
                s.solve(OperatingPoint {
                    alpha_deg: 0.0,
                    mach: bad
                }),
                Err(InviscidError::InvalidMach(_))
            ));
        }
        assert!(matches!(
            s.solve(OperatingPoint::incompressible(f64::INFINITY)),
            Err(InviscidError::InvalidAlpha(_))
        ));
    }

    #[test]
    fn sonic_cp_values() {
        // Cp* at M = 0.5 is -2.1332; at M = 0.8 it is -0.4346.
        assert!((sonic_cp(0.5) + 2.1332).abs() < 1e-3, "{}", sonic_cp(0.5));
        assert!((sonic_cp(0.8) + 0.4346).abs() < 1e-3);
    }

    #[test]
    fn sweep_semantics() {
        let a = AlphaSweep {
            start_deg: -2.0,
            end_deg: 4.0,
            step_deg: 2.0,
        };
        assert_eq!(a.alphas().unwrap(), vec![-2.0, 0.0, 2.0, 4.0]);
        let down = AlphaSweep {
            start_deg: 4.0,
            end_deg: -2.0,
            step_deg: 2.0,
        };
        assert_eq!(down.alphas().unwrap(), vec![4.0, 2.0, 0.0, -2.0]);
        let one = AlphaSweep {
            start_deg: 1.0,
            end_deg: 1.0,
            step_deg: 0.0,
        };
        assert_eq!(one.alphas().unwrap(), vec![1.0]);
        let bad = AlphaSweep {
            start_deg: 0.0,
            end_deg: 1.0,
            step_deg: 0.0,
        };
        assert!(bad.alphas().is_err());
        let huge = AlphaSweep {
            start_deg: 0.0,
            end_deg: 1.0e9,
            step_deg: 1.0e-3,
        };
        assert!(huge.alphas().is_err());
    }

    #[test]
    fn sweep_reuses_solution_and_flags_failures() {
        let s = solver("0012", TrailingEdge::Open);
        let sweep = AlphaSweep {
            start_deg: 0.0,
            end_deg: 10.0,
            step_deg: 1.0,
        };
        let polar = s.sweep(&sweep, 0.0).unwrap();
        assert_eq!(polar.points.len(), 11);
        assert!(polar.failures.is_empty() && !polar.cancelled);
        let direct = s.solve(OperatingPoint::incompressible(6.0)).unwrap();
        assert_eq!(polar.points[6].cl, direct.cl);

        // At M = 0.5 the high angles exceed the critical Mach number.
        let polar = s.sweep(&sweep, 0.5).unwrap();
        assert!(!polar.failures.is_empty());
        assert_eq!(polar.points.len() + polar.failures.len(), 11);

        // Cancellation after three points.
        let mut seen = Vec::new();
        let polar = s
            .sweep_with(&sweep, 0.0, |p| {
                seen.push(p.index);
                if p.index == 2 {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            })
            .unwrap();
        assert!(polar.cancelled);
        assert_eq!(polar.points.len(), 3);
        assert_eq!(seen, vec![0, 1, 2]);
        assert!(matches!(
            s.sweep(&sweep, 2.0),
            Err(InviscidError::InvalidMach(_))
        ));
    }

    #[test]
    fn lift_slope_output_matches_finite_difference() {
        let s = solver("4415", TrailingEdge::Open);
        let a = s.solve(OperatingPoint::incompressible(3.0)).unwrap();
        let h = 1e-4;
        let p = s.solve(OperatingPoint::incompressible(3.0 + h)).unwrap();
        let m = s.solve(OperatingPoint::incompressible(3.0 - h)).unwrap();
        let fd = (p.cl - m.cl) / (2.0 * h.to_radians());
        assert!(
            (a.cl_alpha - fd).abs() < 1e-5 * fd.abs(),
            "{} {fd}",
            a.cl_alpha
        );
    }

    #[test]
    fn influence_at_point() {
        let s = solver("0012", TrailingEdge::Open);
        let inf = s.influence_at(
            &FieldPoint {
                node: None,
                x: 0.5,
                y: 0.5,
                nx: 0.0,
                ny: 1.0,
            },
            false,
        );
        assert_eq!(inf.dpsi_dgamma.len(), s.len());
        assert!(inf.dpsi_dsigma.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn serde_round_trip_of_outputs() {
        let s = solver("0012", TrailingEdge::Open);
        let sol = s.solve(OperatingPoint::incompressible(3.0)).unwrap();
        let json = serde_json::to_string(&sol).unwrap();
        assert_eq!(
            serde_json::from_str::<InviscidSolution>(&json).unwrap(),
            sol
        );
        let o = InviscidOptions::default();
        let back: InviscidOptions =
            serde_json::from_str(&serde_json::to_string(&o).unwrap()).unwrap();
        assert_eq!(back, o);
    }

    #[test]
    fn prescribed_lift_matches_angle_of_attack() {
        let s = solver("2412", TrailingEdge::Open);
        for mach in [0.0, 0.3] {
            let a = s
                .solve(OperatingPoint {
                    alpha_deg: 3.0,
                    mach,
                })
                .unwrap();
            let c = s.solve_cl(a.cl, mach).unwrap();
            assert!((c.alpha_deg - 3.0).abs() < 1e-7, "{}", c.alpha_deg);
            assert!((c.cl - a.cl).abs() < 1e-9);
        }
        assert!(matches!(
            s.solve_cl(50.0, 0.0),
            Err(InviscidError::LiftNotReached(_))
        ));
        let polar = s
            .sweep_cl(
                &ClSweep {
                    start: 0.0,
                    end: 1.0,
                    step: 0.25,
                },
                0.0,
            )
            .unwrap();
        let cls: Vec<f64> = polar.points.iter().map(|p| p.cl).collect();
        for (cl, want) in cls.iter().zip([0.0, 0.25, 0.5, 0.75, 1.0]) {
            assert!((cl - want).abs() < 1e-9);
        }
    }
}
