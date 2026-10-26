//! Data exchanged with the user interface. Every type here is exported to
//! TypeScript by `tauri-specta`; none is written by hand on the frontend.

use aero_foil2d::inviscid::{AlphaSweep, ClSweep, HingeMoment, PolarPoint, SweepFailure};
use aero_foil2d::viscous::{BoundaryLayer, Transition, ViscousPolarPoint};
use aero_geometry::{DatFormat, GeometricProperties, PanelingOptions};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::model::{
    AirfoilId, AirfoilOrigin, OperatingMode, OptimizationSetup, Settings, Shape,
    StoredOptimization, StoredPolar, ViscousSettings,
};
use aero_optim::{Evaluation, HistoryEntry, Phase};

/// Static information about the application.
#[derive(Debug, Clone, Serialize, Type)]
pub struct AppInfo {
    /// Application version.
    pub version: String,
    /// XFOIL version the solver is validated against.
    pub xfoil_reference: String,
    /// Project file format version written by this build.
    pub project_format_version: u32,
}

/// Library entry as listed in the user interface.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct AirfoilSummary {
    /// Identifier.
    pub id: AirfoilId,
    /// Name.
    pub name: String,
    /// Number of points of the source geometry.
    pub n_points: u32,
    /// Shown in the plots.
    pub visible: bool,
    /// Plot color.
    pub color: String,
    /// Origin of the geometry.
    pub origin: AirfoilOrigin,
}

/// Whole session as shown by the user interface.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct SessionView {
    /// Airfoil library.
    pub airfoils: Vec<AirfoilSummary>,
    /// Analysis settings.
    pub settings: Settings,
    /// Computed polars.
    pub polars: Vec<StoredPolar>,
    /// Path of the project file, if the session was saved or opened.
    pub project_path: Option<String>,
    /// Shape optimization setup.
    pub optimization: OptimizationSetup,
    /// Result of the last shape optimization.
    pub last_optimization: Option<StoredOptimization>,
}

/// Result of an import.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct ImportResult {
    /// The new library entry.
    pub airfoil: AirfoilSummary,
    /// Detected file layout.
    pub format: DatFormat,
    /// Non-fatal observations, as sentences.
    pub warnings: Vec<String>,
}

/// Geometry of an airfoil for display.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct GeometryView {
    /// Identifier.
    pub id: AirfoilId,
    /// Name.
    pub name: String,
    /// Plot color.
    pub color: String,
    /// Source x coordinates.
    pub x: Vec<f64>,
    /// Source y coordinates.
    pub y: Vec<f64>,
    /// Panel node x coordinates.
    pub panel_x: Vec<f64>,
    /// Panel node y coordinates.
    pub panel_y: Vec<f64>,
    /// Paneling parameters.
    pub paneling: PanelingOptions,
    /// Geometric properties of the source geometry.
    pub properties: GeometricProperties,
    /// True if the paneled trailing edge is sharp (XFOIL criterion).
    pub sharp_trailing_edge: bool,
    /// True if the leading edge is at the origin and the chord is one
    /// (coefficients are referred to a unit chord).
    pub normalized: bool,
}

/// A single-point analysis request.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Type)]
pub struct AnalysisRequest {
    /// Prescribed angle of attack or lift coefficient.
    pub mode: OperatingMode,
    /// Angle of attack, degrees (prescribed α).
    pub alpha_deg: f64,
    /// Lift coefficient (prescribed CL).
    pub cl: f64,
    /// Mach number.
    pub mach: f64,
    /// Viscous settings (an inviscid analysis when disabled).
    pub viscous: ViscousSettings,
    /// Flap hinge for the hinge moment.
    pub flap_hinge: Option<[f64; 2]>,
}

/// Result of a single-point analysis.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct AnalysisView {
    /// Airfoil.
    pub id: AirfoilId,
    /// Angle of attack, degrees.
    pub alpha_deg: f64,
    /// Mach number of the solution.
    pub mach: f64,
    /// Prescribed lift coefficient, for a prescribed-CL analysis.
    pub cl_target: Option<f64>,
    /// Panel node x coordinates.
    pub x: Vec<f64>,
    /// Panel node y coordinates.
    pub y: Vec<f64>,
    /// Pressure coefficient at each node.
    pub cp: Vec<f64>,
    /// Surface speed Q/Q∞ at each node (Kármán–Tsien corrected).
    pub surface_speed: Vec<f64>,
    /// Index of the leading-edge node (smallest x): nodes before it are on
    /// the upper surface.
    pub le_index: u32,
    /// Lift coefficient.
    pub cl: f64,
    /// Quarter-chord moment coefficient.
    pub cm: f64,
    /// Pressure drag coefficient.
    pub cdp: f64,
    /// Minimum pressure coefficient.
    pub cp_min: f64,
    /// Its x location.
    pub cp_min_x: f64,
    /// Sonic pressure coefficient when M > 0.
    pub cp_star: Option<f64>,
    /// True if the critical Mach number is exceeded.
    pub supercritical: bool,
    /// Flap hinge moment, if a hinge is set.
    pub hinge: Option<HingeMoment>,
    /// Viscous results, when the analysis is viscous (`cp`, `cl`, `cm`,
    /// `cdp` are then the viscous values).
    pub viscous: Option<ViscousView>,
}

/// Results specific to a viscous analysis.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct ViscousView {
    /// Reynolds number of the solution.
    pub reynolds: f64,
    /// Mach number of the solution.
    pub mach: f64,
    /// Drag coefficient.
    pub cd: f64,
    /// Skin-friction drag coefficient.
    pub cdf: f64,
    /// True if the Newton iteration converged.
    pub converged: bool,
    /// Newton iterations performed.
    pub iterations: u32,
    /// Inviscid pressure coefficient at each node.
    pub cp_inviscid: Vec<f64>,
    /// Transition on the top and bottom surfaces.
    pub transition: [Transition; 2],
    /// Boundary layer of the top surface (stagnation point to trailing edge).
    pub top: BoundaryLayer,
    /// Boundary layer of the bottom surface.
    pub bottom: BoundaryLayer,
    /// Wake.
    pub wake: BoundaryLayer,
}

/// A sweep request.
#[derive(Debug, Clone, PartialEq, Deserialize, Type)]
pub struct SweepRequest {
    /// Identifier chosen by the caller, used to cancel the sweep.
    pub job_id: String,
    /// Airfoils to analyze.
    pub airfoil_ids: Vec<AirfoilId>,
    /// Prescribed angles of attack or lift coefficients.
    pub mode: OperatingMode,
    /// Angles of attack (prescribed α).
    pub sweep: AlphaSweep,
    /// Lift coefficients (prescribed CL).
    pub cl_sweep: ClSweep,
    /// Mach number.
    pub mach: f64,
    /// Viscous settings (an inviscid sweep when disabled).
    pub viscous: ViscousSettings,
    /// Flap hinge for the hinge moments.
    pub flap_hinge: Option<[f64; 2]>,
}

/// Progress of a sweep, streamed over a channel.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SweepEvent {
    /// The sweep starts.
    Started {
        /// Total number of points (all airfoils).
        total: u32,
    },
    /// Batch of points computed since the previous progress event.
    Progress {
        /// Airfoil.
        airfoil_id: AirfoilId,
        /// New points.
        points: Vec<PolarPoint>,
        /// New failed points (for instance beyond the critical Mach number).
        failures: Vec<SweepFailure>,
        /// Points done so far (all airfoils).
        done: u32,
        /// Total number of points.
        total: u32,
    },
    /// Batch of viscous points computed since the previous progress event.
    ViscousProgress {
        /// Airfoil.
        airfoil_id: AirfoilId,
        /// New converged points.
        points: Vec<ViscousPolarPoint>,
        /// New failed points (not converged, or refused).
        failures: Vec<SweepFailure>,
        /// Points done so far (all airfoils).
        done: u32,
        /// Total number of points.
        total: u32,
    },
    /// The sweep ended.
    Finished {
        /// True if it was cancelled.
        cancelled: bool,
    },
}

/// A shape optimization to run.
#[derive(Debug, Clone, PartialEq, Deserialize, Type)]
pub struct OptimizeRequest {
    /// Identifier used to cancel the job.
    pub job_id: String,
    /// Airfoil to optimize.
    pub airfoil_id: AirfoilId,
    /// Problem and options.
    pub setup: OptimizationSetup,
}

/// New best design of an optimization.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct BestDesign {
    /// Its evaluation.
    pub evaluation: Evaluation,
    /// Its coordinates.
    pub shape: Shape,
}

/// Progress of an optimization, streamed over a channel.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OptimizationEvent {
    /// The initial airfoil is fitted.
    Started {
        /// Evaluation budget.
        max_evaluations: u32,
        /// Number of design variables.
        dimension: u32,
        /// Rms ordinate error of the CST fit.
        fit_rms_error: f64,
        /// Coordinates of the fitted initial design.
        initial_shape: Shape,
    },
    /// Evaluations since the previous event.
    Progress {
        /// Current phase.
        phase: Phase,
        /// Evaluations so far.
        evaluations: u32,
        /// Step size of the optimizer.
        step_size: f64,
        /// The new evaluations.
        history: Vec<HistoryEntry>,
        /// The best design, when it changed.
        best: Option<BestDesign>,
    },
    /// The optimization ended.
    Finished {
        /// True if it was cancelled.
        cancelled: bool,
    },
}

/// An item of a batch operation that failed.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct BatchFailure {
    /// The item: a NACA designation or a file name.
    pub item: String,
    /// Why it failed.
    pub message: String,
}

/// Result of a batch creation of airfoils.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Type)]
pub struct BatchResult {
    /// New library entries, in order.
    pub added: Vec<AirfoilSummary>,
    /// Items that could not be added.
    pub failed: Vec<BatchFailure>,
}

/// Result of a batch import of files and folders.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Type)]
pub struct BatchImport {
    /// Imported files, in order.
    pub imported: Vec<ImportResult>,
    /// Files or folders that could not be read.
    pub failed: Vec<BatchFailure>,
}
