//! Tauri commands: thin wrappers that run [`Engine`] operations off the main
//! thread (`spawn_blocking`) so that the interface never freezes.

use std::path::PathBuf;
use std::sync::Arc;

use aero_geometry::{Normalization, PanelingOptions};
use tauri::State;
use tauri::ipc::Channel;

use crate::dto::{
    AirfoilSummary, AnalysisRequest, AnalysisView, AppInfo, BatchImport, BatchResult, GeometryView,
    ImportResult, OptimizationEvent, OptimizeRequest, SessionView, SweepEvent, SweepRequest,
};
use crate::engine::Engine;
use crate::error::{AppError, ErrorKind};
use crate::model::{
    AirfoilId, OptimizationSetup, PROJECT_FORMAT_VERSION, Settings, StoredOptimization, StoredPolar,
};

type Shared<'a> = State<'a, Arc<Engine>>;

/// Runs a closure on the blocking thread pool.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::new(ErrorKind::Internal, format!("background task failed: {e}")))?
}

/// Application information.
#[tauri::command]
#[specta::specta]
pub fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        xfoil_reference: "6.996".to_string(),
        project_format_version: PROJECT_FORMAT_VERSION,
    }
}

/// The current session.
#[tauri::command]
#[specta::specta]
pub fn get_session(engine: Shared<'_>) -> SessionView {
    engine.session_view()
}

/// Starts a new, empty session.
#[tauri::command]
#[specta::specta]
pub fn new_session(engine: Shared<'_>) -> SessionView {
    engine.new_session()
}

/// Generates a NACA 4- or 5-digit airfoil.
#[tauri::command]
#[specta::specta]
pub fn create_naca(
    engine: Shared<'_>,
    designation: String,
    closed_te: bool,
) -> Result<AirfoilSummary, AppError> {
    engine.create_naca(&designation, closed_te)
}

/// Imports an airfoil coordinate file (Selig or Lednicer).
#[tauri::command]
#[specta::specta]
pub async fn import_airfoil(engine: Shared<'_>, path: String) -> Result<ImportResult, AppError> {
    let engine = engine.inner().clone();
    blocking(move || engine.import_file(&PathBuf::from(path))).await
}

/// Generates several NACA airfoils from a list ("0012, 2412 4415").
#[tauri::command]
#[specta::specta]
pub fn create_naca_series(
    engine: Shared<'_>,
    designations: String,
    closed_te: bool,
) -> Result<BatchResult, AppError> {
    engine.create_naca_series(&designations, closed_te)
}

/// Imports coordinate files and whole folders of them.
#[tauri::command]
#[specta::specta]
pub async fn import_paths(engine: Shared<'_>, paths: Vec<String>) -> Result<BatchImport, AppError> {
    let engine = engine.inner().clone();
    blocking(move || {
        let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        Ok(engine.import_paths(&paths))
    })
    .await
}

/// Adds a set of classic airfoils.
#[tauri::command]
#[specta::specta]
pub fn add_examples(engine: Shared<'_>) -> Result<Vec<AirfoilSummary>, AppError> {
    engine.add_examples()
}

/// Shows or hides several airfoils.
#[tauri::command]
#[specta::specta]
pub fn set_airfoils_visible(
    engine: Shared<'_>,
    ids: Vec<AirfoilId>,
    visible: bool,
) -> Result<(), AppError> {
    engine.set_airfoils_visible(&ids, visible)
}

/// Deletes several airfoils.
#[tauri::command]
#[specta::specta]
pub fn delete_airfoils(engine: Shared<'_>, ids: Vec<AirfoilId>) -> Result<(), AppError> {
    engine.delete_airfoils(&ids)
}

/// Duplicates an airfoil.
#[tauri::command]
#[specta::specta]
pub fn duplicate_airfoil(engine: Shared<'_>, id: AirfoilId) -> Result<AirfoilSummary, AppError> {
    engine.duplicate(id)
}

/// Renames an airfoil.
#[tauri::command]
#[specta::specta]
pub fn rename_airfoil(
    engine: Shared<'_>,
    id: AirfoilId,
    name: String,
) -> Result<AirfoilSummary, AppError> {
    engine.rename(id, &name)
}

/// Deletes an airfoil.
#[tauri::command]
#[specta::specta]
pub fn delete_airfoil(engine: Shared<'_>, id: AirfoilId) -> Result<(), AppError> {
    engine.delete(id)
}

/// Shows or hides an airfoil in the plots.
#[tauri::command]
#[specta::specta]
pub fn set_airfoil_visible(
    engine: Shared<'_>,
    id: AirfoilId,
    visible: bool,
) -> Result<(), AppError> {
    engine.set_visible(id, visible)
}

/// Geometry, paneling and properties of an airfoil.
#[tauri::command]
#[specta::specta]
pub async fn get_geometry(engine: Shared<'_>, id: AirfoilId) -> Result<GeometryView, AppError> {
    let engine = engine.inner().clone();
    blocking(move || engine.geometry(id)).await
}

/// Changes the paneling parameters of an airfoil.
#[tauri::command]
#[specta::specta]
pub async fn set_paneling(
    engine: Shared<'_>,
    id: AirfoilId,
    paneling: PanelingOptions,
) -> Result<GeometryView, AppError> {
    let engine = engine.inner().clone();
    blocking(move || engine.set_paneling(id, paneling)).await
}

/// Normalizes an airfoil (unit chord, leading edge at the origin).
#[tauri::command]
#[specta::specta]
pub async fn normalize_airfoil(
    engine: Shared<'_>,
    id: AirfoilId,
    mode: Normalization,
) -> Result<GeometryView, AppError> {
    let engine = engine.inner().clone();
    blocking(move || engine.normalize(id, mode)).await
}

/// Single-point analysis (prescribed α or CL), viscous when enabled.
#[tauri::command]
#[specta::specta]
pub async fn analyze(
    engine: Shared<'_>,
    id: AirfoilId,
    request: AnalysisRequest,
) -> Result<AnalysisView, AppError> {
    let engine = engine.inner().clone();
    blocking(move || engine.analyze(id, &request)).await
}

/// Stores the analysis settings.
#[tauri::command]
#[specta::specta]
pub fn update_settings(engine: Shared<'_>, settings: Settings) {
    engine.update_settings(settings);
}

/// Runs an angle-of-attack sweep, streaming progress on `on_event`.
#[tauri::command]
#[specta::specta]
pub async fn run_sweep(
    engine: Shared<'_>,
    request: SweepRequest,
    on_event: Channel<SweepEvent>,
) -> Result<Vec<StoredPolar>, AppError> {
    let engine = engine.inner().clone();
    blocking(move || engine.sweep(&request, |event| on_event.send(event).is_ok())).await
}

/// Optimizes the shape of an airfoil, streaming progress on `on_event`.
#[tauri::command]
#[specta::specta]
pub async fn run_optimization(
    engine: Shared<'_>,
    request: OptimizeRequest,
    on_event: Channel<OptimizationEvent>,
) -> Result<StoredOptimization, AppError> {
    let engine = engine.inner().clone();
    blocking(move || engine.optimize(&request, |event| on_event.send(event).is_ok())).await
}

/// Cancels a running optimization.
#[tauri::command]
#[specta::specta]
pub fn cancel_optimization(engine: Shared<'_>, job_id: String) -> bool {
    engine.cancel(&job_id)
}

/// Replaces the optimization setup of the session.
#[tauri::command]
#[specta::specta]
pub fn update_optimization_setup(engine: Shared<'_>, setup: OptimizationSetup) {
    engine.update_optimization_setup(setup);
}

/// Adds the best design of the last optimization to the library.
#[tauri::command]
#[specta::specta]
pub fn add_optimized_airfoil(engine: Shared<'_>) -> Result<AirfoilSummary, AppError> {
    engine.add_optimized_airfoil()
}

/// Cancels a running sweep.
#[tauri::command]
#[specta::specta]
pub fn cancel_sweep(engine: Shared<'_>, job_id: String) -> bool {
    engine.cancel(&job_id)
}

/// Writes an airfoil as a Selig `.dat` file.
#[tauri::command]
#[specta::specta]
pub async fn export_dat(engine: Shared<'_>, id: AirfoilId, path: String) -> Result<(), AppError> {
    let engine = engine.inner().clone();
    blocking(move || engine.export_dat(id, &PathBuf::from(path))).await
}

/// Writes the polars of the given airfoils (all if empty) as CSV.
#[tauri::command]
#[specta::specta]
pub async fn export_polars_csv(
    engine: Shared<'_>,
    ids: Vec<AirfoilId>,
    path: String,
) -> Result<(), AppError> {
    let engine = engine.inner().clone();
    blocking(move || engine.export_polars_csv(&ids, &PathBuf::from(path))).await
}

/// Writes the pressure distribution of an analysis as CSV.
#[tauri::command]
#[specta::specta]
pub async fn export_cp_csv(
    engine: Shared<'_>,
    id: AirfoilId,
    request: AnalysisRequest,
    path: String,
) -> Result<(), AppError> {
    let engine = engine.inner().clone();
    blocking(move || engine.export_cp_csv(id, &request, &PathBuf::from(path))).await
}

/// Writes the boundary layer of a viscous analysis as CSV.
#[tauri::command]
#[specta::specta]
pub async fn export_boundary_layer_csv(
    engine: Shared<'_>,
    id: AirfoilId,
    request: AnalysisRequest,
    path: String,
) -> Result<(), AppError> {
    let engine = engine.inner().clone();
    blocking(move || engine.export_boundary_layer_csv(id, &request, &PathBuf::from(path))).await
}

/// Writes a file produced by the interface (PNG or SVG plot).
#[tauri::command]
#[specta::specta]
pub async fn write_file(
    engine: Shared<'_>,
    path: String,
    contents: Vec<u8>,
) -> Result<(), AppError> {
    let engine = engine.inner().clone();
    blocking(move || engine.write_file(&PathBuf::from(path), &contents)).await
}

/// Saves the session as a project file.
#[tauri::command]
#[specta::specta]
pub async fn save_project(engine: Shared<'_>, path: String) -> Result<SessionView, AppError> {
    let engine = engine.inner().clone();
    blocking(move || engine.save_project(&PathBuf::from(path))).await
}

/// Opens a project file.
#[tauri::command]
#[specta::specta]
pub async fn open_project(engine: Shared<'_>, path: String) -> Result<SessionView, AppError> {
    let engine = engine.inner().clone();
    blocking(move || engine.open_project(&PathBuf::from(path))).await
}
