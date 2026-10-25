//! Data exchanged with the user interface. Every type here is exported to
//! TypeScript by `tauri-specta`; none is written by hand on the frontend.

use aero_foil2d::inviscid::{AlphaSweep, PolarPoint, SweepFailure};
use aero_geometry::{DatFormat, GeometricProperties, PanelingOptions};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::model::{AirfoilId, AirfoilOrigin, Settings, StoredPolar};

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

/// Result of a single angle-of-attack analysis.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct AnalysisView {
    /// Airfoil.
    pub id: AirfoilId,
    /// Angle of attack, degrees.
    pub alpha_deg: f64,
    /// Mach number.
    pub mach: f64,
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
}

/// A sweep request.
#[derive(Debug, Clone, PartialEq, Deserialize, Type)]
pub struct SweepRequest {
    /// Identifier chosen by the caller, used to cancel the sweep.
    pub job_id: String,
    /// Airfoils to analyze.
    pub airfoil_ids: Vec<AirfoilId>,
    /// Angles of attack.
    pub sweep: AlphaSweep,
    /// Mach number.
    pub mach: f64,
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
    /// The sweep ended.
    Finished {
        /// True if it was cancelled.
        cancelled: bool,
    },
}
