//! Application logic behind the Tauri commands.
//!
//! The [`Engine`] owns the session, a cache of factorized solvers (one per
//! airfoil geometry and paneling, so that moving the angle-of-attack slider
//! never refactorizes the matrix) and the cancellation flags of running
//! sweeps. It does not depend on Tauri and is tested directly.

use std::collections::HashMap;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use aero_foil2d::inviscid::{InviscidOptions, InviscidSolver, OperatingPoint, Polar, SweepFailure};
use aero_geometry::{
    Airfoil, NacaOptions, Normalization, PanelingOptions, TrailingEdge, naca, parse_dat,
    to_selig_string,
};

use crate::dto::{
    AirfoilSummary, AnalysisView, GeometryView, ImportResult, SessionView, SweepEvent, SweepRequest,
};
use crate::error::{AppError, ErrorKind};
use crate::export;
use crate::model::{
    AirfoilEntry, AirfoilId, AirfoilOrigin, Session, Settings, StoredPolar, load_project,
    save_project,
};

/// Largest accepted coordinate file, in bytes.
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Sweep progress is sent in batches at most this often, so that very large
/// sweeps do not flood the interface with messages.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(50);

struct CachedSolver {
    revision: u64,
    allow_supercritical: bool,
    airfoil: Arc<Airfoil>,
    solver: Arc<InviscidSolver>,
}

/// Application state and operations.
#[derive(Default)]
pub struct Engine {
    session: Mutex<Session>,
    project_path: Mutex<Option<PathBuf>>,
    solvers: Mutex<HashMap<AirfoilId, CachedSolver>>,
    jobs: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

/// Locks a mutex, recovering the data if another thread panicked.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn summary(entry: &AirfoilEntry) -> AirfoilSummary {
    AirfoilSummary {
        id: entry.id,
        name: entry.name().to_string(),
        n_points: u32::try_from(entry.source.len()).unwrap_or(u32::MAX),
        visible: entry.visible,
        color: entry.color.clone(),
        origin: entry.origin.clone(),
    }
}

fn read_text_file(path: &Path) -> Result<String, AppError> {
    let size = std::fs::metadata(path)
        .map_err(|e| {
            AppError::new(
                ErrorKind::Io,
                format!("cannot open {}: {e}", path.display()),
            )
        })?
        .len();
    if size > MAX_FILE_SIZE {
        return Err(AppError::new(
            ErrorKind::InvalidFile,
            format!(
                "{} is too large to be an airfoil or project file",
                path.display()
            ),
        ));
    }
    let bytes = std::fs::read(path).map_err(|e| {
        AppError::new(
            ErrorKind::Io,
            format!("cannot read {}: {e}", path.display()),
        )
    })?;
    String::from_utf8(bytes).or_else(|e| {
        // Old coordinate files are often Latin-1 encoded.
        Ok(e.into_bytes().iter().map(|&b| char::from(b)).collect())
    })
}

fn write_file(path: &Path, contents: &[u8]) -> Result<(), AppError> {
    std::fs::write(path, contents).map_err(|e| {
        AppError::new(
            ErrorKind::Io,
            format!("cannot write {}: {e}", path.display()),
        )
    })
}

impl Engine {
    /// Creates an engine with an empty session.
    pub fn new() -> Self {
        Self::default()
    }

    /// The session as shown by the interface.
    pub fn session_view(&self) -> SessionView {
        let session = lock(&self.session);
        SessionView {
            airfoils: session.airfoils.iter().map(summary).collect(),
            settings: session.settings,
            polars: session.polars.clone(),
            project_path: lock(&self.project_path)
                .as_ref()
                .map(|p| p.display().to_string()),
        }
    }

    /// Starts a new, empty session.
    pub fn new_session(&self) -> SessionView {
        *lock(&self.session) = Session::default();
        *lock(&self.project_path) = None;
        lock(&self.solvers).clear();
        self.session_view()
    }

    fn add(&self, airfoil: Airfoil, origin: AirfoilOrigin) -> AirfoilSummary {
        summary(lock(&self.session).add(airfoil, origin))
    }

    /// Generates a NACA 4- or 5-digit airfoil.
    pub fn create_naca(
        &self,
        designation: &str,
        closed_te: bool,
    ) -> Result<AirfoilSummary, AppError> {
        let options = NacaOptions {
            trailing_edge: if closed_te {
                TrailingEdge::Closed
            } else {
                TrailingEdge::Open
            },
            ..NacaOptions::default()
        };
        let airfoil = naca(designation, &options)?;
        Ok(self.add(
            airfoil,
            AirfoilOrigin::Naca {
                designation: designation.trim().to_string(),
                closed_te,
            },
        ))
    }

    /// Imports an airfoil from the text of a coordinate file.
    pub fn import_text(&self, text: &str, file_name: &str) -> Result<ImportResult, AppError> {
        let stem = Path::new(file_name)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| file_name.to_string());
        let (airfoil, report) = parse_dat(text, &stem).map_err(|e| {
            let base: AppError = e.into();
            AppError::new(
                ErrorKind::InvalidFile,
                format!("{file_name} is not a valid airfoil file: {}", base.message),
            )
        })?;
        let summary = self.add(
            airfoil,
            AirfoilOrigin::Imported {
                file_name: file_name.to_string(),
            },
        );
        Ok(ImportResult {
            airfoil: summary,
            format: report.format,
            warnings: report.warnings.iter().map(ToString::to_string).collect(),
        })
    }

    /// Imports an airfoil coordinate file.
    pub fn import_file(&self, path: &Path) -> Result<ImportResult, AppError> {
        let text = read_text_file(path)?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        self.import_text(&text, &name)
    }

    /// Duplicates an airfoil.
    pub fn duplicate(&self, id: AirfoilId) -> Result<AirfoilSummary, AppError> {
        let mut session = lock(&self.session);
        let new_id = session.duplicate(id)?;
        session.get(new_id).map(summary)
    }

    /// Renames an airfoil.
    pub fn rename(&self, id: AirfoilId, name: &str) -> Result<AirfoilSummary, AppError> {
        let mut session = lock(&self.session);
        session.rename(id, name)?;
        session.get(id).map(summary)
    }

    /// Deletes an airfoil.
    pub fn delete(&self, id: AirfoilId) -> Result<(), AppError> {
        lock(&self.session).remove(id)?;
        lock(&self.solvers).remove(&id);
        Ok(())
    }

    /// Shows or hides an airfoil.
    pub fn set_visible(&self, id: AirfoilId, visible: bool) -> Result<(), AppError> {
        lock(&self.session).set_visible(id, visible)
    }

    /// Replaces the analysis settings.
    pub fn update_settings(&self, settings: Settings) {
        lock(&self.session).settings = settings;
    }

    fn entry(&self, id: AirfoilId) -> Result<(AirfoilEntry, bool), AppError> {
        let session = lock(&self.session);
        Ok((
            session.get(id)?.clone(),
            session.settings.allow_supercritical,
        ))
    }

    /// Paneled airfoil and factorized solver, from the cache when possible.
    fn solver(&self, id: AirfoilId) -> Result<(Arc<Airfoil>, Arc<InviscidSolver>), AppError> {
        let (entry, allow_supercritical) = self.entry(id)?;
        if let Some(c) = lock(&self.solvers).get(&id)
            && c.revision == entry.revision
            && c.allow_supercritical == allow_supercritical
        {
            return Ok((c.airfoil.clone(), c.solver.clone()));
        }
        let airfoil = Arc::new(entry.paneled()?);
        let options = InviscidOptions {
            allow_supercritical,
            ..InviscidOptions::default()
        };
        let solver = Arc::new(InviscidSolver::new(&airfoil, &options)?);
        lock(&self.solvers).insert(
            id,
            CachedSolver {
                revision: entry.revision,
                allow_supercritical,
                airfoil: airfoil.clone(),
                solver: solver.clone(),
            },
        );
        Ok((airfoil, solver))
    }

    /// Geometry of an airfoil, with its current paneling.
    pub fn geometry(&self, id: AirfoilId) -> Result<GeometryView, AppError> {
        let (entry, _) = self.entry(id)?;
        let (paneled, solver) = self.solver(id)?;
        let properties = entry.source.properties();
        let normalized = (properties.chord - 1.0).abs() < 1e-3
            && properties.leading_edge[0].abs() < 1e-3
            && properties.leading_edge[1].abs() < 1e-3;
        Ok(GeometryView {
            id,
            name: entry.name().to_string(),
            color: entry.color.clone(),
            x: entry.source.x().to_vec(),
            y: entry.source.y().to_vec(),
            panel_x: paneled.x().to_vec(),
            panel_y: paneled.y().to_vec(),
            paneling: entry.paneling,
            properties,
            sharp_trailing_edge: solver.geometry().trailing_edge_panel().sharp,
            normalized,
        })
    }

    /// Changes the paneling of an airfoil.
    pub fn set_paneling(
        &self,
        id: AirfoilId,
        paneling: PanelingOptions,
    ) -> Result<GeometryView, AppError> {
        lock(&self.session).set_paneling(id, paneling)?;
        self.geometry(id)
    }

    /// Normalizes an airfoil (unit chord, leading edge at the origin).
    pub fn normalize(&self, id: AirfoilId, mode: Normalization) -> Result<GeometryView, AppError> {
        lock(&self.session).normalize(id, mode)?;
        self.geometry(id)
    }

    /// Single angle-of-attack analysis.
    pub fn analyze(
        &self,
        id: AirfoilId,
        alpha_deg: f64,
        mach: f64,
    ) -> Result<AnalysisView, AppError> {
        let (_, solver) = self.solver(id)?;
        let sol = solver.solve(OperatingPoint { alpha_deg, mach })?;
        let x = solver.geometry().x();
        let le_index = (0..x.len())
            .min_by(|&a, &b| x[a].total_cmp(&x[b]))
            .unwrap_or(0);
        Ok(AnalysisView {
            id,
            alpha_deg,
            mach,
            x: x.to_vec(),
            y: solver.geometry().y().to_vec(),
            cp: sol.cp.clone(),
            surface_speed: sol.surface_speed_compressible.clone(),
            le_index: u32::try_from(le_index).unwrap_or(0),
            cl: sol.cl,
            cm: sol.cm,
            cdp: sol.cdp,
            cp_min: sol.cp_min,
            cp_min_x: sol.cp_min_x,
            cp_star: sol.cp_star,
            supercritical: sol.supercritical,
        })
    }

    /// Runs a sweep on several airfoils, calling `on_event` with progress.
    /// `on_event` returning false (receiver gone) cancels the sweep, as does
    /// [`Engine::cancel`]. The polars are stored in the session.
    pub fn sweep(
        &self,
        request: &SweepRequest,
        mut on_event: impl FnMut(SweepEvent) -> bool,
    ) -> Result<Vec<StoredPolar>, AppError> {
        let alphas = request.sweep.alphas()?;
        let flag = Arc::new(AtomicBool::new(false));
        lock(&self.jobs).insert(request.job_id.clone(), flag.clone());
        let result = self.run_sweep(request, alphas.len(), &flag, &mut on_event);
        lock(&self.jobs).remove(&request.job_id);
        let cancelled = flag.load(Ordering::Relaxed);
        on_event(SweepEvent::Finished { cancelled });
        result
    }

    fn run_sweep(
        &self,
        request: &SweepRequest,
        per_airfoil: usize,
        flag: &AtomicBool,
        on_event: &mut impl FnMut(SweepEvent) -> bool,
    ) -> Result<Vec<StoredPolar>, AppError> {
        let total = u32::try_from(per_airfoil * request.airfoil_ids.len()).unwrap_or(u32::MAX);
        if !on_event(SweepEvent::Started { total }) {
            flag.store(true, Ordering::Relaxed);
        }
        let mut done = 0_u32;
        let mut results = Vec::new();
        for &id in &request.airfoil_ids {
            if flag.load(Ordering::Relaxed) {
                break;
            }
            let (_, solver) = self.solver(id)?;
            let mut points = Vec::new();
            let mut failures = Vec::new();
            let mut last_flush = Instant::now();
            let mut flush = |points: &mut Vec<_>, failures: &mut Vec<_>, done: u32| {
                let event = SweepEvent::Progress {
                    airfoil_id: id,
                    points: std::mem::take(points),
                    failures: std::mem::take(failures),
                    done,
                    total,
                };
                if !on_event(event) {
                    flag.store(true, Ordering::Relaxed);
                }
            };
            let polar: Polar = solver.sweep_with(&request.sweep, request.mach, |progress| {
                done += 1;
                match progress.point {
                    Some(point) => points.push(point),
                    None => failures.push(SweepFailure {
                        alpha_deg: progress.alpha_deg,
                        message: progress.error.clone().unwrap_or_default(),
                    }),
                }
                let last = progress.index + 1 == progress.total;
                if last || last_flush.elapsed() >= PROGRESS_INTERVAL {
                    flush(&mut points, &mut failures, done);
                    last_flush = Instant::now();
                }
                if flag.load(Ordering::Relaxed) {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            })?;
            if !points.is_empty() || !failures.is_empty() {
                flush(&mut points, &mut failures, done);
            }
            let mut session = lock(&self.session);
            session.store_polar(id, polar)?;
            if let Some(stored) = session.polars.iter().find(|p| p.airfoil_id == id) {
                results.push(stored.clone());
            }
        }
        Ok(results)
    }

    /// Requests the cancellation of a running sweep. Returns false if no
    /// such job is running.
    pub fn cancel(&self, job_id: &str) -> bool {
        match lock(&self.jobs).get(job_id) {
            Some(flag) => {
                flag.store(true, Ordering::Relaxed);
                true
            }
            None => false,
        }
    }

    /// Selig `.dat` text of an airfoil's source geometry.
    pub fn dat_text(&self, id: AirfoilId) -> Result<String, AppError> {
        Ok(to_selig_string(&lock(&self.session).get(id)?.source))
    }

    /// Writes an airfoil's source geometry as a Selig `.dat` file.
    pub fn export_dat(&self, id: AirfoilId, path: &Path) -> Result<(), AppError> {
        write_file(path, self.dat_text(id)?.as_bytes())
    }

    /// CSV text of the polars of the given airfoils (all if empty).
    pub fn polars_csv(&self, ids: &[AirfoilId]) -> String {
        let session = lock(&self.session);
        let selected: Vec<&StoredPolar> = session
            .polars
            .iter()
            .filter(|p| ids.is_empty() || ids.contains(&p.airfoil_id))
            .collect();
        export::polars_csv(&selected)
    }

    /// Writes the polars as CSV.
    pub fn export_polars_csv(&self, ids: &[AirfoilId], path: &Path) -> Result<(), AppError> {
        write_file(path, self.polars_csv(ids).as_bytes())
    }

    /// Writes the pressure distribution of an analysis as CSV.
    pub fn export_cp_csv(
        &self,
        id: AirfoilId,
        alpha_deg: f64,
        mach: f64,
        path: &Path,
    ) -> Result<(), AppError> {
        let (_, solver) = self.solver(id)?;
        let sol = solver.solve(OperatingPoint { alpha_deg, mach })?;
        let csv = export::cp_csv(solver.geometry().x(), solver.geometry().y(), &sol);
        write_file(path, csv.as_bytes())
    }

    /// Writes arbitrary bytes produced by the interface (PNG, SVG).
    pub fn write_file(&self, path: &Path, contents: &[u8]) -> Result<(), AppError> {
        write_file(path, contents)
    }

    /// Saves the session as a project file.
    pub fn save_project(&self, path: &Path) -> Result<SessionView, AppError> {
        let text = save_project(&lock(&self.session))?;
        write_file(path, text.as_bytes())?;
        *lock(&self.project_path) = Some(path.to_path_buf());
        Ok(self.session_view())
    }

    /// Opens a project file, replacing the session.
    pub fn open_project(&self, path: &Path) -> Result<SessionView, AppError> {
        let session = load_project(&read_text_file(path)?)?;
        *lock(&self.session) = session;
        *lock(&self.project_path) = Some(path.to_path_buf());
        lock(&self.solvers).clear();
        Ok(self.session_view())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aero_foil2d::inviscid::AlphaSweep;

    fn sweep_request(ids: Vec<AirfoilId>, mach: f64) -> SweepRequest {
        SweepRequest {
            job_id: "job".into(),
            airfoil_ids: ids,
            sweep: AlphaSweep {
                start_deg: 0.0,
                end_deg: 10.0,
                step_deg: 1.0,
            },
            mach,
        }
    }

    #[test]
    fn library_and_geometry() {
        let e = Engine::new();
        let a = e.create_naca("2412", false).unwrap();
        assert_eq!(a.name, "NACA 2412");
        assert!(e.create_naca("9999x", false).is_err());
        let g = e.geometry(a.id).unwrap();
        assert_eq!(g.panel_x.len(), 160);
        assert!(g.normalized);
        assert!(!g.sharp_trailing_edge);
        assert!((g.properties.max_camber - 0.02).abs() < 1e-3);

        let g = e
            .set_paneling(
                a.id,
                PanelingOptions {
                    n_nodes: 120,
                    ..PanelingOptions::default()
                },
            )
            .unwrap();
        assert_eq!(g.panel_x.len(), 120);

        let b = e.duplicate(a.id).unwrap();
        e.rename(b.id, "copy").unwrap();
        e.set_visible(b.id, false).unwrap();
        let view = e.session_view();
        assert_eq!(view.airfoils.len(), 2);
        assert!(!view.airfoils[1].visible);
        e.delete(b.id).unwrap();
        assert_eq!(e.session_view().airfoils.len(), 1);
        assert!(e.geometry(b.id).is_err());
    }

    #[test]
    fn import_reports_format_and_errors() {
        let e = Engine::new();
        let text = "my foil\n1 0.001\n0.5 0.05\n0 0\n0.5 -0.05\n1 -0.001\n";
        let r = e.import_text(text, "foil.dat").unwrap();
        assert_eq!(r.airfoil.name, "my foil");
        assert!(r.warnings.is_empty());
        let err = e.import_text("garbage\n", "bad.dat").unwrap_err();
        assert_eq!(err.kind, ErrorKind::InvalidFile);
        assert!(err.message.contains("bad.dat"));
        let missing = e.import_file(Path::new("does/not/exist.dat")).unwrap_err();
        assert_eq!(missing.kind, ErrorKind::Io);
    }

    #[test]
    fn analysis_reuses_the_factorization() {
        let e = Engine::new();
        let a = e.create_naca("0012", false).unwrap();
        let r1 = e.analyze(a.id, 4.0, 0.0).unwrap();
        let first = lock(&e.solvers).get(&a.id).map(|c| Arc::as_ptr(&c.solver));
        let r2 = e.analyze(a.id, 5.0, 0.0).unwrap();
        let second = lock(&e.solvers).get(&a.id).map(|c| Arc::as_ptr(&c.solver));
        assert_eq!(first, second, "the solver must be cached");
        assert!(
            (r1.cl - 0.4829).abs() < 1e-4,
            "XFOIL: 0.4829, got {}",
            r1.cl
        );
        assert!(r2.cl > r1.cl);
        assert_eq!(r1.x.len(), 160);
        assert!(r1.x[r1.le_index as usize] < 1e-3);

        // Beyond the critical Mach number: a clear error by default.
        let err = e.analyze(a.id, 10.0, 0.6).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Mach);
        assert!(err.message.contains("critical Mach"));
        // ... and a flagged result when allowed (the cache is invalidated).
        let mut settings = e.session_view().settings;
        settings.allow_supercritical = true;
        e.update_settings(settings);
        assert!(e.analyze(a.id, 10.0, 0.6).unwrap().supercritical);
    }

    #[test]
    fn sweeps_stream_progress_and_store_polars() {
        let e = Engine::new();
        let a = e.create_naca("0012", false).unwrap();
        let b = e.create_naca("4415", false).unwrap();
        let mut events = Vec::new();
        let polars = e
            .sweep(&sweep_request(vec![a.id, b.id], 0.0), |ev| {
                events.push(ev);
                true
            })
            .unwrap();
        assert_eq!(polars.len(), 2);
        assert_eq!(polars[0].polar.points.len(), 11);
        assert_eq!(events.first(), Some(&SweepEvent::Started { total: 22 }));
        assert_eq!(
            events.last(),
            Some(&SweepEvent::Finished { cancelled: false })
        );
        // Points arrive in batches; all of them are reported once.
        let streamed: usize = events
            .iter()
            .map(|e| match e {
                SweepEvent::Progress { points, .. } => points.len(),
                _ => 0,
            })
            .sum();
        assert_eq!(streamed, 22);
        assert!(events.len() < 22, "progress must be batched");
        assert!(
            events
                .iter()
                .any(|e| matches!(e, SweepEvent::Progress { done: 22, .. }))
        );
        assert_eq!(e.session_view().polars.len(), 2);

        let csv = e.polars_csv(&[a.id]);
        assert_eq!(csv.lines().count(), 12);
        assert!(csv.lines().nth(1).unwrap().starts_with("NACA 0012,0,0,"));
    }

    #[test]
    fn sweeps_can_be_cancelled_and_flag_failures() {
        let e = Engine::new();
        let a = e.create_naca("0012", false).unwrap();
        let b = e.create_naca("2412", false).unwrap();
        let mut finished = None;
        let polars = e
            .sweep(&sweep_request(vec![a.id, b.id], 0.0), |ev| {
                match ev {
                    // Cancel through the job flag after the first airfoil.
                    SweepEvent::Progress { airfoil_id, .. } if airfoil_id == a.id => {
                        e.cancel("job");
                    }
                    SweepEvent::Finished { cancelled } => finished = Some(cancelled),
                    _ => {}
                }
                true
            })
            .unwrap();
        assert_eq!(finished, Some(true));
        assert_eq!(polars.len(), 1, "the second airfoil must not be analyzed");
        assert!(!e.cancel("job"), "the job is no longer running");

        // A receiver that disappears also stops the sweep.
        let polars = e
            .sweep(&sweep_request(vec![a.id, b.id], 0.0), |_| false)
            .unwrap();
        assert!(polars.is_empty());

        let mut failed = 0;
        e.sweep(&sweep_request(vec![a.id], 0.5), |ev| {
            if let SweepEvent::Progress { failures, .. } = ev {
                for f in &failures {
                    assert!(f.message.contains("critical Mach"));
                }
                failed += failures.len();
            }
            true
        })
        .unwrap();
        assert!(failed > 0);
    }

    #[test]
    fn exports_and_projects() {
        let dir = std::env::temp_dir().join(format!("aero-app-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let e = Engine::new();
        let a = e.create_naca("2412", false).unwrap();
        e.sweep(&sweep_request(vec![a.id], 0.0), |_| true).unwrap();

        let dat = dir.join("foil.dat");
        e.export_dat(a.id, &dat).unwrap();
        let back = e.import_file(&dat).unwrap();
        assert_eq!(back.airfoil.n_points, a.n_points);

        let cp = dir.join("cp.csv");
        e.export_cp_csv(a.id, 4.0, 0.0, &cp).unwrap();
        assert_eq!(std::fs::read_to_string(&cp).unwrap().lines().count(), 161);
        e.export_polars_csv(&[], &dir.join("polars.csv")).unwrap();
        e.write_file(&dir.join("plot.svg"), b"<svg/>").unwrap();

        let project = dir.join("session.aero.json");
        let saved = e.save_project(&project).unwrap();
        assert!(saved.project_path.is_some());
        let other = Engine::new();
        let opened = other.open_project(&project).unwrap();
        assert_eq!(opened.airfoils.len(), 2);
        assert_eq!(opened.polars.len(), 1);
        assert_eq!(opened.polars, saved.polars);
        // Identifiers keep increasing after opening.
        let c = other.create_naca("0012", false).unwrap();
        assert!(c.id > back.airfoil.id);

        assert!(e.new_session().airfoils.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }
}
