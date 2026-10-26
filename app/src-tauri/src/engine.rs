//! Application logic behind the Tauri commands.
//!
//! The [`Engine`] owns the session, a cache of factorized solvers (one per
//! airfoil geometry and paneling, so that moving the angle-of-attack slider
//! never refactorizes the matrix) and the cancellation flags of running
//! sweeps. It does not depend on Tauri and is tested directly.

use std::collections::HashMap;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use aero_foil2d::inviscid::{
    AlphaSweep, ClSweep, InviscidOptions, InviscidSolution, InviscidSolver, OperatingPoint, Polar,
    SweepFailure, SweepProgress,
};
use aero_foil2d::viscous::{
    ViscousOptions, ViscousPolar, ViscousSolution, ViscousSolver, ViscousSweepProgress,
};
use aero_geometry::{
    Airfoil, CamberThickness, HingeY, NacaOptions, Normalization, PanelingOptions, TrailingEdge,
    camber_thickness, deflect_flap, interpolate, move_high_points, naca, parse_dat,
    scale_le_radius, scale_thickness_camber, set_te_gap, set_thickness_camber, to_selig_string,
};

use aero_optim::{DesignSpace, Phase, Stop};

use crate::catalogue::{self, CatalogueEntry, CatalogueSource};
use crate::design::DesignSession;
use crate::dto::{
    AirfoilSummary, AnalysisRequest, AnalysisView, BatchFailure, BatchImport, BatchResult,
    BestDesign, DesignEdit, DesignMethod, DesignPreview, DesignView, GeometryView, ImportResult,
    ModificationPreview, OptimizationEvent, OptimizeRequest, SessionView, SweepEvent, SweepRequest,
    ViscousView,
};
use crate::error::{AppError, ErrorKind};
use crate::export;
use crate::mission::{
    self, Candidate, CandidateSource, RecommendEvent, RecommendRequest, RecommendResult,
};
use crate::model::{
    AirfoilEntry, AirfoilId, AirfoilOrigin, Modification, OperatingMode, OptimizationSetup,
    PolarData, Session, Settings, Shape, StoredOptimization, StoredPolar, load_project,
    save_project,
};

/// Largest accepted coordinate file, in bytes.
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Extensions of the files read when a folder is imported.
const COORDINATE_EXTENSIONS: [&str; 6] = ["dat", "txt", "cor", "xy", "csv", "lednicer"];

/// Most files imported in one batch.
const MAX_BATCH_FILES: usize = 5000;

/// NACA sections of [`Engine::add_examples`].
const EXAMPLE_NACA: [&str; 6] = ["0012", "2412", "4412", "4415", "23012", "6409"];

/// The sequences of a viscous sweep over `values` (in sweep order): from
/// the value nearest zero to the end, then from the one before it back to
/// the start. Starting near zero lift or incidence, where the boundary
/// layer is attached, is the usual practice with XFOIL: a sequence that
/// starts in separated flow often stops after a few unconverged points.
fn outward_sequences(values: &[f64]) -> Vec<(f64, f64)> {
    let Some((k, _)) = values
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
    else {
        return Vec::new();
    };
    let mut out = vec![(values[k], values[values.len() - 1])];
    if k > 0 {
        out.push((values[k - 1], values[0]));
    }
    out
}

/// Splits a list of NACA designations such as `"0012, 2412 4415; NACA 23012"`.
pub fn parse_designations(text: &str) -> Vec<String> {
    text.split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .map(|s| {
            let lower = s.to_ascii_lowercase();
            match lower.strip_prefix("naca") {
                Some(rest) => s[s.len() - rest.len()..]
                    .trim_start_matches(['-', ' '])
                    .to_string(),
                None => s.to_string(),
            }
        })
        .filter(|s| !s.is_empty())
        .collect()
}

fn is_coordinate_file(path: &Path) -> bool {
    let hidden = path
        .file_name()
        .is_some_and(|n| n.to_string_lossy().starts_with('.'));
    let known = path.extension().is_some_and(|e| {
        let e = e.to_string_lossy().to_ascii_lowercase();
        COORDINATE_EXTENSIONS.contains(&e.as_str())
    });
    path.is_file() && known && !hidden
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}

/// Sweep progress is sent in batches at most this often, so that very large
/// sweeps do not flood the interface with messages.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(50);

struct CachedSolver {
    revision: u64,
    allow_supercritical: bool,
    flap_hinge: Option<[f64; 2]>,
    airfoil: Arc<Airfoil>,
    solver: Arc<InviscidSolver>,
    /// Viscous solver built on `solver` for the last options used.
    viscous: Option<(ViscousOptions, Arc<ViscousSolver>)>,
}

/// Application state and operations.
#[derive(Default)]
pub struct Engine {
    session: Mutex<Session>,
    project_path: Mutex<Option<PathBuf>>,
    solvers: Mutex<HashMap<AirfoilId, CachedSolver>>,
    jobs: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// Inverse design in progress.
    design: Mutex<Option<DesignSession>>,
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

/// View of a viscous solution.
fn viscous_view(id: AirfoilId, solver: &ViscousSolver, sol: &ViscousSolution) -> AnalysisView {
    let g = solver.inviscid().geometry();
    let x = g.x();
    let le_index = (0..x.len())
        .min_by(|&a, &b| x[a].total_cmp(&x[b]))
        .unwrap_or(0);
    // Kármán–Tsien corrected surface speed, as for the inviscid view.
    let beta = (1.0 - sol.mach * sol.mach).sqrt();
    let tklam = sol.mach * sol.mach / ((1.0 + beta) * (1.0 + beta));
    let surface_speed = sol
        .surface_speed
        .iter()
        .map(|&q| q * (1.0 - tklam) / (1.0 - tklam * q * q))
        .collect();
    AnalysisView {
        id,
        alpha_deg: sol.alpha_deg,
        mach: sol.mach,
        cl_target: sol.cl_target,
        x: x.to_vec(),
        y: g.y().to_vec(),
        cp: sol.cp.clone(),
        surface_speed,
        le_index: u32::try_from(le_index).unwrap_or(0),
        cl: sol.cl,
        cm: sol.cm,
        cdp: sol.cdp,
        cp_min: sol.cp_min,
        cp_min_x: sol.cp_min_x,
        cp_star: sol.cp_star,
        supercritical: sol.supercritical,
        hinge: sol.hinge,
        viscous: Some(ViscousView {
            mach: sol.mach,
            reynolds: sol.reynolds,
            cd: sol.cd,
            cdf: sol.cdf,
            converged: sol.converged,
            iterations: u32::try_from(sol.iterations).unwrap_or(u32::MAX),
            cp_inviscid: sol.cp_inviscid.clone(),
            transition: sol.transition,
            top: sol.top.clone(),
            bottom: sol.bottom.clone(),
            wake: sol.wake.clone(),
        }),
    }
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
            optimization: session.optimization.clone(),
            last_optimization: session.last_optimization.clone(),
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

    /// Generates the NACA sections of a list (see [`parse_designations`]);
    /// invalid designations are reported, the others are added.
    pub fn create_naca_series(&self, text: &str, closed_te: bool) -> Result<BatchResult, AppError> {
        let list = parse_designations(text);
        if list.is_empty() {
            return Err(AppError::new(
                ErrorKind::InvalidInput,
                "type a NACA designation, or several separated by commas",
            ));
        }
        let mut out = BatchResult::default();
        for d in list {
            match self.create_naca(&d, closed_te) {
                Ok(a) => out.added.push(a),
                Err(e) => out.failed.push(BatchFailure {
                    item: d,
                    message: e.message,
                }),
            }
        }
        Ok(out)
    }

    /// Imports coordinate files and folders. A folder contributes its
    /// coordinate files (by extension, not recursively), in name order.
    /// Files that cannot be read are reported, the others are imported.
    pub fn import_paths(&self, paths: &[PathBuf]) -> BatchImport {
        let mut out = BatchImport::default();
        let mut files = Vec::new();
        for path in paths {
            if path.is_dir() {
                match std::fs::read_dir(path) {
                    Ok(entries) => {
                        let mut found: Vec<PathBuf> = entries
                            .filter_map(|e| e.ok().map(|e| e.path()))
                            .filter(|p| is_coordinate_file(p))
                            .collect();
                        found.sort();
                        if found.is_empty() {
                            out.failed.push(BatchFailure {
                                item: display_name(path),
                                message: "the folder contains no coordinate file (.dat, .txt…)"
                                    .into(),
                            });
                        }
                        files.extend(found);
                    }
                    Err(e) => out.failed.push(BatchFailure {
                        item: display_name(path),
                        message: format!("cannot read the folder: {e}"),
                    }),
                }
            } else {
                files.push(path.clone());
            }
        }
        if files.len() > MAX_BATCH_FILES {
            out.failed.push(BatchFailure {
                item: format!("{} files", files.len()),
                message: format!("only the first {MAX_BATCH_FILES} files are imported"),
            });
            files.truncate(MAX_BATCH_FILES);
        }
        for file in files {
            match self.import_file(&file) {
                Ok(r) => out.imported.push(r),
                Err(e) => out.failed.push(BatchFailure {
                    item: display_name(&file),
                    message: e.message,
                }),
            }
        }
        out
    }

    /// Adds classic airfoils to compare: NACA 0012, 2412, 4412, 4415, 23012,
    /// 6409, Clark Y and Eppler 387 (UIUC coordinates).
    pub fn add_examples(&self) -> Result<Vec<AirfoilSummary>, AppError> {
        let mut out = Vec::new();
        for d in EXAMPLE_NACA {
            out.push(self.create_naca(d, false)?);
        }
        for (file, name) in [("clarky.dat", "Clark Y"), ("e387.dat", "Eppler 387")] {
            let text = catalogue::file_text(file).ok_or_else(|| {
                AppError::new(
                    ErrorKind::Internal,
                    format!("catalogue file {file} is missing"),
                )
            })?;
            let id = self.import_text(text, file)?.airfoil.id;
            out.push(self.rename(id, name)?);
        }
        Ok(out)
    }

    /// The airfoil catalogue, sorted by use.
    pub fn catalogue(&self) -> Result<Vec<CatalogueEntry>, AppError> {
        catalogue::entries()
    }

    /// Evaluates catalogue and library airfoils for a flight, calling
    /// `on_event` with progress. `on_event` returning false (receiver gone)
    /// cancels it, as does [`Engine::cancel`].
    pub fn recommend(
        &self,
        request: &RecommendRequest,
        mut on_event: impl FnMut(RecommendEvent) -> bool + Send,
    ) -> Result<RecommendResult, AppError> {
        let entries = catalogue::entries()?;
        let mut candidates = Vec::new();
        for id in &request.catalogue_ids {
            let entry = entries.iter().find(|e| &e.id == id).ok_or_else(|| {
                AppError::new(
                    ErrorKind::NotFound,
                    format!("no airfoil {id} in the catalogue"),
                )
            })?;
            candidates.push(Candidate {
                source: CandidateSource::Catalogue { id: id.clone() },
                name: entry.name.clone(),
                airfoil: entry.airfoil()?,
            });
        }
        {
            let session = lock(&self.session);
            for &id in &request.airfoil_ids {
                let entry = session.get(id)?;
                candidates.push(Candidate {
                    source: CandidateSource::Library { id },
                    name: entry.name().to_string(),
                    airfoil: entry.source.clone(),
                });
            }
        }
        let flag = Arc::new(AtomicBool::new(false));
        lock(&self.jobs).insert(request.job_id.clone(), flag.clone());
        let sink = Mutex::new(&mut on_event);
        let emit = |event: RecommendEvent| {
            let mut send = lock(&sink);
            if !(*send)(event) {
                flag.store(true, Ordering::Relaxed);
            }
        };
        let result = mission::recommend(request, candidates, &flag, &emit);
        lock(&self.jobs).remove(&request.job_id);
        let cancelled = flag.load(Ordering::Relaxed);
        on_event(RecommendEvent::Finished { cancelled });
        result
    }

    /// Adds airfoils of the catalogue to the library.
    pub fn add_from_catalogue(&self, ids: &[String]) -> Result<BatchResult, AppError> {
        let entries = catalogue::entries()?;
        let mut out = BatchResult::default();
        for id in ids {
            let added = entries
                .iter()
                .find(|e| &e.id == id)
                .ok_or_else(|| {
                    AppError::new(
                        ErrorKind::NotFound,
                        format!("no airfoil {id} in the catalogue"),
                    )
                })
                .and_then(|e| {
                    let origin = match &e.source {
                        CatalogueSource::Naca(designation) => AirfoilOrigin::Naca {
                            designation: designation.clone(),
                            closed_te: false,
                        },
                        CatalogueSource::File(file) => AirfoilOrigin::Imported {
                            file_name: file.clone(),
                        },
                    };
                    Ok(self.add(e.airfoil()?, origin))
                });
            match added {
                Ok(a) => out.added.push(a),
                Err(e) => out.failed.push(BatchFailure {
                    item: id.clone(),
                    message: e.message,
                }),
            }
        }
        Ok(out)
    }

    /// The airfoil modified as requested, and a short description.
    fn modified(&self, id: AirfoilId, m: &Modification) -> Result<(Airfoil, String), AppError> {
        let session = lock(&self.session);
        let source = &session.get(id)?.source;
        let pct = |v: f64| format!("{:.2} %", 100.0 * v).replace(".00 %", " %");
        let (airfoil, description) = match *m {
            Modification::Flap {
                hinge_x,
                hinge_y,
                deflection_deg,
            } => (
                deflect_flap(source, hinge_x, hinge_y, deflection_deg)?,
                format!("flap {deflection_deg}° at x {hinge_x}"),
            ),
            Modification::TeGap { gap, blend, split } => (
                set_te_gap(source, gap, blend, split)?,
                format!("TE gap {}", pct(gap)),
            ),
            Modification::LeRadius { factor, blend } => (
                scale_le_radius(source, factor, blend)?,
                format!("LE radius ×{factor}"),
            ),
            Modification::ScaleThicknessCamber { thickness, camber } => (
                scale_thickness_camber(source, thickness, camber)?,
                format!("thickness ×{thickness}, camber ×{camber}"),
            ),
            Modification::SetThicknessCamber { thickness, camber } => {
                let mut parts = Vec::new();
                if let Some(t) = thickness {
                    parts.push(format!("t {}", pct(t)));
                }
                if let Some(c) = camber {
                    parts.push(format!("camber {}", pct(c)));
                }
                (
                    set_thickness_camber(source, thickness, camber)?,
                    parts.join(", "),
                )
            }
            Modification::HighPoints {
                thickness_x,
                camber_x,
            } => {
                let mut parts = Vec::new();
                if let Some(t) = thickness_x {
                    parts.push(format!("max t at {t}"));
                }
                if let Some(c) = camber_x {
                    parts.push(format!("max camber at {c}"));
                }
                (
                    move_high_points(source, thickness_x, camber_x)?,
                    parts.join(", "),
                )
            }
            Modification::Interpolate { other, fraction } => {
                let b = &session.get(other)?.source;
                (
                    interpolate(source, b, fraction)?,
                    format!("{} % toward {}", (100.0 * fraction).round(), b.name()),
                )
            }
        };
        Ok((airfoil, description))
    }

    /// Previews a modification without changing the session.
    pub fn preview_modification(
        &self,
        id: AirfoilId,
        m: &Modification,
    ) -> Result<ModificationPreview, AppError> {
        let (a, description) = self.modified(id, m)?;
        Ok(ModificationPreview {
            x: a.x().to_vec(),
            y: a.y().to_vec(),
            properties: a.properties(),
            description,
        })
    }

    /// Adds the modified airfoil to the library (the original is kept),
    /// named `name` or `"<original> (<change>)"`, with the original's
    /// paneling.
    pub fn apply_modification(
        &self,
        id: AirfoilId,
        m: &Modification,
        name: Option<&str>,
    ) -> Result<AirfoilSummary, AppError> {
        let (a, change) = self.modified(id, m)?;
        let mut session = lock(&self.session);
        let original = session.get(id)?.clone();
        let name = name
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map_or_else(|| format!("{} ({change})", original.name()), str::to_string);
        let new_id = session
            .add(
                a.with_name(name),
                AirfoilOrigin::Modified {
                    of: original.name().to_string(),
                    change,
                },
            )
            .id;
        session.set_paneling(new_id, original.paneling)?;
        session.get(new_id).map(summary)
    }

    /// Adds the airfoil with its flap deflected by each of `deflections`.
    pub fn flap_series(
        &self,
        id: AirfoilId,
        hinge_x: f64,
        hinge_y: HingeY,
        deflections: &[f64],
    ) -> Result<BatchResult, AppError> {
        if deflections.is_empty() {
            return Err(AppError::new(
                ErrorKind::InvalidInput,
                "no deflection given",
            ));
        }
        let base = lock(&self.session).get(id)?.name().to_string();
        let mut out = BatchResult::default();
        for &deflection_deg in deflections {
            let m = Modification::Flap {
                hinge_x,
                hinge_y,
                deflection_deg,
            };
            let name = format!("{base} flap {deflection_deg}°");
            match self.apply_modification(id, &m, Some(&name)) {
                Ok(a) => out.added.push(a),
                Err(e) => out.failed.push(BatchFailure {
                    item: format!("{deflection_deg}°"),
                    message: e.message,
                }),
            }
        }
        Ok(out)
    }

    /// Starts an inverse design of an airfoil at `alpha_deg` (its paneled
    /// geometry, current speed distribution as the target), replacing any
    /// design in progress.
    pub fn design_start(
        &self,
        id: AirfoilId,
        method: DesignMethod,
        alpha_deg: f64,
    ) -> Result<DesignView, AppError> {
        let paneled = lock(&self.session).get(id)?.paneled()?;
        let design = DesignSession::new(id, paneled, method, alpha_deg)?;
        let view = design.view();
        *lock(&self.design) = Some(design);
        Ok(view)
    }

    fn with_design<T>(
        &self,
        f: impl FnOnce(&mut DesignSession) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let mut design = lock(&self.design);
        let design = design.as_mut().ok_or_else(|| {
            AppError::new(ErrorKind::InvalidInput, "no inverse design in progress")
        })?;
        f(design)
    }

    /// Changes the inverse design in progress.
    pub fn design_edit(&self, edit: DesignEdit) -> Result<DesignView, AppError> {
        self.with_design(|d| {
            d.edit(edit)?;
            Ok(d.view())
        })
    }

    /// The airfoil the inverse design would create (`None` while there is
    /// nothing to change).
    pub fn design_preview(&self) -> Result<Option<DesignPreview>, AppError> {
        self.with_design(|d| Ok(d.preview()?.map(|(_, p)| p)))
    }

    /// Adds the airfoil of the inverse design to the library, named `name`
    /// or `"<original> (<method>)"`, with the original's paneling.
    pub fn design_apply(&self, name: Option<&str>) -> Result<AirfoilSummary, AppError> {
        let (id, airfoil, change) = self.with_design(|d| {
            let (a, p) = d.preview()?.ok_or_else(|| {
                AppError::new(
                    ErrorKind::InvalidInput,
                    "draw a new target speed over part of the surface first",
                )
            })?;
            Ok((d.id, a, p.description))
        })?;
        let mut session = lock(&self.session);
        let original = session.get(id)?.clone();
        let name = name
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map_or_else(|| format!("{} ({change})", original.name()), str::to_string);
        let new_id = session
            .add(
                airfoil.with_name(name),
                AirfoilOrigin::Modified {
                    of: original.name().to_string(),
                    change,
                },
            )
            .id;
        session.set_paneling(new_id, original.paneling)?;
        session.get(new_id).map(summary)
    }

    /// Ends the inverse design in progress.
    pub fn design_close(&self) {
        *lock(&self.design) = None;
    }

    /// Camber line, thickness and their maxima, as XFOIL's thickness and
    /// camber commands see them.
    pub fn camber_thickness(&self, id: AirfoilId) -> Result<CamberThickness, AppError> {
        Ok(camber_thickness(&lock(&self.session).get(id)?.source))
    }

    /// Shows or hides several airfoils.
    pub fn set_airfoils_visible(&self, ids: &[AirfoilId], visible: bool) -> Result<(), AppError> {
        let mut session = lock(&self.session);
        for &id in ids {
            session.set_visible(id, visible)?;
        }
        Ok(())
    }

    /// Deletes several airfoils.
    pub fn delete_airfoils(&self, ids: &[AirfoilId]) -> Result<(), AppError> {
        for &id in ids {
            self.delete(id)?;
        }
        Ok(())
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

    /// Paneled airfoil and factorized solver for a flap hinge, from the
    /// cache when possible.
    fn solver(
        &self,
        id: AirfoilId,
        flap_hinge: Option<[f64; 2]>,
    ) -> Result<(Arc<Airfoil>, Arc<InviscidSolver>), AppError> {
        let (entry, allow_supercritical) = self.entry(id)?;
        if let Some(c) = lock(&self.solvers).get(&id)
            && c.revision == entry.revision
            && c.allow_supercritical == allow_supercritical
            && c.flap_hinge == flap_hinge
        {
            return Ok((c.airfoil.clone(), c.solver.clone()));
        }
        let airfoil = Arc::new(entry.paneled()?);
        let options = InviscidOptions {
            allow_supercritical,
            flap_hinge,
            ..InviscidOptions::default()
        };
        let solver = Arc::new(InviscidSolver::new(&airfoil, &options)?);
        lock(&self.solvers).insert(
            id,
            CachedSolver {
                revision: entry.revision,
                allow_supercritical,
                flap_hinge,
                airfoil: airfoil.clone(),
                solver: solver.clone(),
                viscous: None,
            },
        );
        Ok((airfoil, solver))
    }

    /// Viscous solver of an airfoil for the given options, from the cache
    /// when possible. It shares the factorized inviscid solver (copied with
    /// its source influence matrix).
    fn viscous_solver(
        &self,
        id: AirfoilId,
        flap_hinge: Option<[f64; 2]>,
        options: &ViscousOptions,
    ) -> Result<Arc<ViscousSolver>, AppError> {
        let (_, inviscid) = self.solver(id, flap_hinge)?;
        if let Some(c) = lock(&self.solvers).get(&id)
            && Arc::ptr_eq(&c.solver, &inviscid)
            && let Some((cached_options, solver)) = &c.viscous
            && cached_options == options
        {
            return Ok(solver.clone());
        }
        // Computed once in the cached solver, then copied.
        let _ = inviscid.source_velocity_matrix();
        let solver = Arc::new(ViscousSolver::from_inviscid(
            InviscidSolver::clone(&inviscid),
            options,
        )?);
        if let Some(c) = lock(&self.solvers).get_mut(&id)
            && Arc::ptr_eq(&c.solver, &inviscid)
        {
            c.viscous = Some((*options, solver.clone()));
        }
        Ok(solver)
    }

    /// Geometry of an airfoil, with its current paneling.
    pub fn geometry(&self, id: AirfoilId) -> Result<GeometryView, AppError> {
        let (entry, _) = self.entry(id)?;
        let hinge = lock(&self.session).settings.flap_hinge;
        let (paneled, solver) = self.solver(id, hinge)?;
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

    /// Viscous solution of a request.
    fn viscous_solution(
        &self,
        id: AirfoilId,
        request: &AnalysisRequest,
    ) -> Result<(Arc<ViscousSolver>, ViscousSolution), AppError> {
        let options = request.viscous.options(request.mach);
        let solver = self.viscous_solver(id, request.flap_hinge, &options)?;
        let sol = match request.mode {
            OperatingMode::Alpha => solver.solve(request.alpha_deg)?,
            OperatingMode::Cl => solver.solve_cl(request.cl)?,
        };
        Ok((solver, sol))
    }

    /// Inviscid solution of a request.
    fn inviscid_solution(
        &self,
        id: AirfoilId,
        request: &AnalysisRequest,
    ) -> Result<(Arc<InviscidSolver>, InviscidSolution), AppError> {
        let (_, solver) = self.solver(id, request.flap_hinge)?;
        let sol = match request.mode {
            OperatingMode::Alpha => solver.solve(OperatingPoint {
                alpha_deg: request.alpha_deg,
                mach: request.mach,
            })?,
            OperatingMode::Cl => solver.solve_cl(request.cl, request.mach)?,
        };
        Ok((solver, sol))
    }

    /// Single-point analysis (prescribed α or CL), viscous when enabled.
    pub fn analyze(
        &self,
        id: AirfoilId,
        request: &AnalysisRequest,
    ) -> Result<AnalysisView, AppError> {
        if request.viscous.enabled {
            let (solver, sol) = self.viscous_solution(id, request)?;
            return Ok(viscous_view(id, &solver, &sol));
        }
        let (solver, sol) = self.inviscid_solution(id, request)?;
        let x = solver.geometry().x();
        let le_index = (0..x.len())
            .min_by(|&a, &b| x[a].total_cmp(&x[b]))
            .unwrap_or(0);
        Ok(AnalysisView {
            id,
            alpha_deg: sol.alpha_deg,
            mach: sol.mach,
            cl_target: (request.mode == OperatingMode::Cl).then_some(request.cl),
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
            hinge: sol.hinge,
            viscous: None,
        })
    }

    /// Runs a sweep on several airfoils, calling `on_event` with progress.
    /// `on_event` returning false (receiver gone) cancels the sweep, as does
    /// [`Engine::cancel`]. The polars are stored in the session.
    ///
    /// The airfoils are swept in parallel, one per core (one core is left
    /// to the interface); each polar is the same as when swept alone.
    /// Progress events of different airfoils interleave. If an airfoil
    /// fails, the others are still stored and the first error is returned.
    pub fn sweep(
        &self,
        request: &SweepRequest,
        mut on_event: impl FnMut(SweepEvent) -> bool + Send,
    ) -> Result<Vec<StoredPolar>, AppError> {
        let count = match request.mode {
            OperatingMode::Alpha => request.sweep.alphas()?.len(),
            OperatingMode::Cl => request.cl_sweep.values()?.len(),
        };
        let flag = Arc::new(AtomicBool::new(false));
        lock(&self.jobs).insert(request.job_id.clone(), flag.clone());
        let result = self.run_sweep(request, count, &flag, &mut on_event);
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
        on_event: &mut (impl FnMut(SweepEvent) -> bool + Send),
    ) -> Result<Vec<StoredPolar>, AppError> {
        let ids = &request.airfoil_ids;
        let total = u32::try_from(per_airfoil * ids.len()).unwrap_or(u32::MAX);
        if !on_event(SweepEvent::Started { total }) {
            flag.store(true, Ordering::Relaxed);
        }
        let sink = Mutex::new(on_event);
        let emit = |event: SweepEvent| {
            let mut send = lock(&sink);
            if !(*send)(event) {
                flag.store(true, Ordering::Relaxed);
            }
        };
        let done = AtomicU32::new(0);
        let next = AtomicUsize::new(0);
        let threads = std::thread::available_parallelism()
            .map_or(1, |n| n.get().saturating_sub(1).max(1))
            .min(ids.len().max(1));
        let sweep_one = |id: AirfoilId| -> Result<PolarData, AppError> {
            if request.viscous.enabled {
                self.run_viscous_sweep(id, request, per_airfoil, total, &done, flag, &emit)
                    .map(PolarData::Viscous)
            } else {
                self.run_inviscid_sweep(id, request, total, &done, flag, &emit)
                    .map(PolarData::Inviscid)
            }
        };
        let mut polars: Vec<(usize, Result<PolarData, AppError>)> = std::thread::scope(|scope| {
            let workers: Vec<_> = (0..threads)
                .map(|_| {
                    scope.spawn(|| {
                        let mut out = Vec::new();
                        loop {
                            let i = next.fetch_add(1, Ordering::Relaxed);
                            let Some(&id) = ids.get(i) else { break };
                            if flag.load(Ordering::Relaxed) {
                                break;
                            }
                            out.push((i, sweep_one(id)));
                        }
                        out
                    })
                })
                .collect();
            workers
                .into_iter()
                .flat_map(|w| w.join().unwrap_or_else(|e| std::panic::resume_unwind(e)))
                .collect()
        });
        polars.sort_by_key(|(i, _)| *i);

        let mut results = Vec::new();
        let mut first_error = None;
        let mut session = lock(&self.session);
        for (i, polar) in polars {
            let id = ids[i];
            match polar.and_then(|p| session.store_polar(id, p)) {
                Ok(()) => {
                    if let Some(stored) = session.polars.iter().find(|p| p.airfoil_id == id) {
                        results.push(stored.clone());
                    }
                }
                Err(e) => {
                    first_error.get_or_insert(e);
                }
            }
        }
        match first_error {
            Some(e) => Err(e),
            None => Ok(results),
        }
    }

    /// Inviscid sweep of one airfoil.
    fn run_inviscid_sweep(
        &self,
        id: AirfoilId,
        request: &SweepRequest,
        total: u32,
        done: &AtomicU32,
        flag: &AtomicBool,
        emit: &(impl Fn(SweepEvent) + Sync),
    ) -> Result<Polar, AppError> {
        let (_, solver) = self.solver(id, request.flap_hinge)?;
        let mut points = Vec::new();
        let mut failures = Vec::new();
        let mut last_flush = Instant::now();
        let flush = |points: &mut Vec<_>, failures: &mut Vec<_>| {
            emit(SweepEvent::Progress {
                airfoil_id: id,
                points: std::mem::take(points),
                failures: std::mem::take(failures),
                done: done.load(Ordering::Relaxed),
                total,
            });
        };
        let cl_mode = request.mode == OperatingMode::Cl;
        let on_progress = |progress: &SweepProgress| {
            done.fetch_add(1, Ordering::Relaxed);
            match progress.point {
                Some(point) => points.push(point),
                None => failures.push(SweepFailure {
                    alpha_deg: progress.alpha_deg,
                    cl: None,
                    message: progress.error.clone().unwrap_or_default(),
                }),
            }
            let last = progress.index + 1 == progress.total;
            if last || last_flush.elapsed() >= PROGRESS_INTERVAL {
                flush(&mut points, &mut failures);
                last_flush = Instant::now();
            }
            if flag.load(Ordering::Relaxed) {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        };
        let polar = if cl_mode {
            solver.sweep_cl_with(&request.cl_sweep, request.mach, on_progress)?
        } else {
            solver.sweep_with(&request.sweep, request.mach, on_progress)?
        };
        if !points.is_empty() || !failures.is_empty() {
            flush(&mut points, &mut failures);
        }
        Ok(polar)
    }

    /// Viscous sweep of one airfoil: XFOIL ASEQ or CSEQ sequences (each
    /// point starts from the previous one), from the value nearest zero
    /// outward in both directions ([`outward_sequences`]).
    #[allow(clippy::too_many_arguments)]
    fn run_viscous_sweep(
        &self,
        id: AirfoilId,
        request: &SweepRequest,
        per_airfoil: usize,
        total: u32,
        done: &AtomicU32,
        flag: &AtomicBool,
        emit: &(impl Fn(SweepEvent) + Sync),
    ) -> Result<ViscousPolar, AppError> {
        let options = request.viscous.options(request.mach);
        let solver = self.viscous_solver(id, request.flap_hinge, &options)?;
        let mut mine = 0_u32;
        let mut points = Vec::new();
        let mut failures = Vec::new();
        let mut last_flush = Instant::now();
        let flush = |points: &mut Vec<_>, failures: &mut Vec<_>| {
            emit(SweepEvent::ViscousProgress {
                airfoil_id: id,
                points: std::mem::take(points),
                failures: std::mem::take(failures),
                done: done.load(Ordering::Relaxed),
                total,
            });
        };
        let on_progress = |progress: &ViscousSweepProgress, _: Option<&ViscousSolution>| {
            mine += 1;
            done.fetch_add(1, Ordering::Relaxed);
            match progress.point {
                Some(point) => points.push(point),
                None => failures.push(SweepFailure {
                    alpha_deg: progress.alpha_deg,
                    cl: progress.cl_target,
                    message: progress.error.clone().unwrap_or_default(),
                }),
            }
            let last = progress.index + 1 == progress.total;
            if last || last_flush.elapsed() >= PROGRESS_INTERVAL {
                flush(&mut points, &mut failures);
                last_flush = Instant::now();
            }
            if flag.load(Ordering::Relaxed) {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        };
        let mut on_progress = on_progress;
        let values = match request.mode {
            OperatingMode::Alpha => request.sweep.alphas()?,
            OperatingMode::Cl => request.cl_sweep.values()?,
        };
        let mut polar: Option<ViscousPolar> = None;
        for (from, to) in outward_sequences(&values) {
            let part = match request.mode {
                OperatingMode::Alpha => solver.sweep_with(
                    &AlphaSweep {
                        start_deg: from,
                        end_deg: to,
                        step_deg: request.sweep.step_deg,
                    },
                    &mut on_progress,
                )?,
                OperatingMode::Cl => solver.sweep_cl_with(
                    &ClSweep {
                        start: from,
                        end: to,
                        step: request.cl_sweep.step,
                    },
                    &mut on_progress,
                )?,
            };
            match polar.as_mut() {
                None => polar = Some(part),
                Some(p) => {
                    p.points.extend(part.points);
                    p.failures.extend(part.failures);
                    p.stopped |= part.stopped;
                    p.cancelled |= part.cancelled;
                }
            }
            if flag.load(Ordering::Relaxed) {
                break;
            }
        }
        let Some(mut polar) = polar else {
            return Err(AppError::new(ErrorKind::InvalidInput, "empty sweep"));
        };
        // In sweep order (ascending or descending, as requested).
        let ascending = values.first() <= values.last();
        let key = |p: &aero_foil2d::viscous::ViscousPolarPoint| match request.mode {
            OperatingMode::Alpha => p.alpha_deg,
            OperatingMode::Cl => p.cl,
        };
        polar.points.sort_by(|a, b| {
            let order = key(a).total_cmp(&key(b));
            if ascending { order } else { order.reverse() }
        });
        if polar.stopped {
            // The remaining points will not be computed.
            let count = u32::try_from(per_airfoil).unwrap_or(u32::MAX);
            done.fetch_add(count.saturating_sub(mine), Ordering::Relaxed);
        }
        flush(&mut points, &mut failures);
        Ok(polar)
    }

    /// Optimizes the shape of an airfoil, calling `on_event` with progress.
    /// `on_event` returning false (receiver gone) cancels the optimization,
    /// as does [`Engine::cancel`]; the best design so far is then returned.
    /// The result and the setup are stored in the session.
    pub fn optimize(
        &self,
        request: &OptimizeRequest,
        mut on_event: impl FnMut(OptimizationEvent) -> bool,
    ) -> Result<StoredOptimization, AppError> {
        let entry = lock(&self.session).get(request.airfoil_id)?.clone();
        let mut problem = request.setup.problem.clone();
        problem.paneling = entry.paneling;
        let mut options = request.setup.options;
        if options.threads == 0 {
            // Leave a core to the interface; the results do not depend on it.
            options.threads = std::thread::available_parallelism()
                .map_or(1, |n| n.get().saturating_sub(1).max(1));
        }
        problem.validate()?;
        options.validate()?;
        // The same fit as the optimizer's, to draw the designs.
        let space = DesignSpace::new(&entry.source, &problem.design)?;
        let shape = |x: &[f64]| {
            space
                .airfoil(x, entry.name())
                .map(|a| Shape::from(&a))
                .unwrap_or(Shape {
                    x: Vec::new(),
                    y: Vec::new(),
                })
        };
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);

        let flag = Arc::new(AtomicBool::new(false));
        lock(&self.jobs).insert(request.job_id.clone(), flag.clone());
        let started = Instant::now();
        let mut proceed = on_event(OptimizationEvent::Started {
            max_evaluations: count(options.max_evaluations),
            dimension: count(space.dimension()),
            fit_rms_error: space.fit().rms_error,
            initial_shape: shape(&space.initial()),
        });
        let result = aero_optim::optimize(&entry.source, &problem, &options, |p| {
            let best = (p.improved || p.phase == Phase::Initial).then(|| BestDesign {
                evaluation: p.best.clone(),
                shape: shape(&p.best.x),
            });
            proceed = proceed
                && on_event(OptimizationEvent::Progress {
                    phase: p.phase,
                    evaluations: count(p.evaluations),
                    step_size: p.step_size,
                    history: p.new.to_vec(),
                    best,
                });
            if proceed && !flag.load(Ordering::Relaxed) {
                ControlFlow::Continue(())
            } else {
                ControlFlow::Break(())
            }
        });
        lock(&self.jobs).remove(&request.job_id);
        let cancelled = result.as_ref().is_ok_and(|r| r.stop == Stop::Cancelled);
        on_event(OptimizationEvent::Finished { cancelled });
        let result = result?;

        let stored = StoredOptimization {
            airfoil_id: entry.id,
            airfoil_name: entry.name().to_string(),
            problem,
            options: request.setup.options,
            initial: result.initial,
            best: result.best,
            initial_shape: Shape::from(&result.initial_airfoil),
            best_shape: Shape::from(&result.airfoil),
            fit_rms_error: result.fit_rms_error,
            history: result.history,
            stop: result.stop,
            elapsed_s: started.elapsed().as_secs_f64(),
        };
        let mut session = lock(&self.session);
        session.optimization = request.setup.clone();
        session.last_optimization = Some(stored.clone());
        Ok(stored)
    }

    /// Replaces the optimization setup.
    pub fn update_optimization_setup(&self, setup: OptimizationSetup) {
        lock(&self.session).optimization = setup;
    }

    /// Adds the best design of the last optimization to the library.
    pub fn add_optimized_airfoil(&self) -> Result<AirfoilSummary, AppError> {
        let mut session = lock(&self.session);
        let id = session.add_optimized()?;
        session.get(id).map(summary)
    }

    /// Requests the cancellation of a running sweep or optimization.
    /// Returns false if no such job is running.
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
        request: &AnalysisRequest,
        path: &Path,
    ) -> Result<(), AppError> {
        if request.viscous.enabled {
            let (solver, sol) = self.viscous_solution(id, request)?;
            let g = solver.inviscid().geometry();
            return write_file(path, export::viscous_cp_csv(g.x(), g.y(), &sol).as_bytes());
        }
        let (solver, sol) = self.inviscid_solution(id, request)?;
        let csv = export::cp_csv(solver.geometry().x(), solver.geometry().y(), &sol);
        write_file(path, csv.as_bytes())
    }

    /// Writes the boundary layer of a viscous analysis as CSV.
    pub fn export_boundary_layer_csv(
        &self,
        id: AirfoilId,
        request: &AnalysisRequest,
        path: &Path,
    ) -> Result<(), AppError> {
        let (_, sol) = self.viscous_solution(id, request)?;
        write_file(path, export::boundary_layer_csv(&sol).as_bytes())
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
    use crate::model::ViscousSettings;
    use aero_foil2d::inviscid::{AlphaSweep, ClSweep};
    use aero_foil2d::viscous::{AmplificationModel, MachReynoldsVariation};

    const INVISCID: ViscousSettings = ViscousSettings {
        enabled: false,
        reynolds: 1.0e6,
        ncrit: [9.0, 9.0],
        xtr: [1.0, 1.0],
        variation: MachReynoldsVariation::Fixed,
        amplification: AmplificationModel::Original,
    };

    /// A prescribed-α request.
    fn at(alpha_deg: f64, mach: f64, viscous: &ViscousSettings) -> AnalysisRequest {
        AnalysisRequest {
            mode: OperatingMode::Alpha,
            alpha_deg,
            cl: 0.0,
            mach,
            viscous: *viscous,
            flap_hinge: None,
        }
    }

    fn sweep_request(ids: Vec<AirfoilId>, mach: f64) -> SweepRequest {
        SweepRequest {
            job_id: "job".into(),
            airfoil_ids: ids,
            sweep: AlphaSweep {
                start_deg: 0.0,
                end_deg: 10.0,
                step_deg: 1.0,
            },
            mode: OperatingMode::Alpha,
            cl_sweep: ClSweep {
                start: 0.0,
                end: 1.0,
                step: 0.1,
            },
            mach,
            viscous: INVISCID,
            flap_hinge: None,
        }
    }

    fn points(p: &StoredPolar) -> usize {
        match &p.polar {
            PolarData::Inviscid(p) => p.points.len(),
            PolarData::Viscous(p) => p.points.len(),
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
    fn airfoils_from_the_catalogue() {
        let e = Engine::new();
        assert_eq!(e.catalogue().unwrap().len(), 20);
        let ids = ["s1223", "naca0012", "nope"].map(String::from);
        let r = e.add_from_catalogue(&ids).unwrap();
        assert_eq!(r.added.len(), 2);
        assert_eq!(r.added[0].name, "Selig S1223");
        assert!(matches!(
            &r.added[0].origin,
            AirfoilOrigin::Imported { file_name } if file_name == "s1223.dat"
        ));
        assert_eq!(r.added[1].name, "NACA 0012");
        assert_eq!(r.failed.len(), 1);
        assert_eq!(r.failed[0].item, "nope");
        // The examples read the same embedded files.
        let examples = e.add_examples().unwrap();
        assert_eq!(examples.len(), 8);
        assert_eq!(examples[7].name, "Eppler 387");
    }

    fn recommend_request(job_id: &str, airfoil_ids: Vec<AirfoilId>) -> RecommendRequest {
        // 450 g at 26.5 km/h with a 1.2 m × 20 cm wing: Cl ≈ 0.56, Re ≈ 1e5.
        RecommendRequest {
            job_id: job_id.to_string(),
            flight: aero_mission::Flight {
                speed: 7.36,
                altitude: 100.0,
                temperature_offset: 0.0,
                mass: 0.45,
            },
            wing: aero_mission::Wing {
                span: 1.2,
                chord: 0.2,
            },
            settings: aero_mission::AnalysisSettings::default(),
            model: aero_mission::WingModel::default(),
            catalogue_ids: ["s1223", "sd7037", "naca0012"].map(String::from).to_vec(),
            airfoil_ids,
        }
    }

    #[test]
    fn recommendations_for_a_flight() {
        let e = Engine::new();
        let clark_y = e.add_from_catalogue(&["clarky".to_string()]).unwrap().added[0].id;
        let mut events = Vec::new();
        let r = e
            .recommend(&recommend_request("rec", vec![clark_y]), |ev| {
                events.push(ev);
                true
            })
            .unwrap();
        assert!(!r.cancelled);
        assert!((r.conditions.required_cl - 0.56).abs() < 0.01);
        assert_eq!(r.candidates.len(), 4);
        assert_eq!(r.candidates[3].name, "Clark Y");
        assert!(matches!(r.candidates[3].source, CandidateSource::Library { id } if id == clark_y));
        assert!(
            r.candidates
                .iter()
                .all(|c| c.evaluation.is_some() && c.error.is_none())
        );
        // The low-drag glider section first, the high-lift section first
        // for slow flight, the undercambered S1223 never "easy to build".
        let name = |i: u32| r.candidates[i as usize].name.as_str();
        assert_eq!(name(r.rankings.least_drag[0]), "Selig–Donovan SD7037");
        assert_eq!(name(r.rankings.slow_flight[0]), "Selig S1223");
        assert_ne!(name(r.rankings.easy_to_build[0]), "Selig S1223");
        assert_eq!(events.first(), Some(&RecommendEvent::Started { total: 4 }));
        assert_eq!(
            events.last(),
            Some(&RecommendEvent::Finished { cancelled: false })
        );
        assert_eq!(events.len(), 6);

        // Unknown entries are errors before anything runs.
        let mut bad = recommend_request("bad", Vec::new());
        bad.catalogue_ids.push("nope".to_string());
        assert_eq!(
            e.recommend(&bad, |_| true).unwrap_err().kind,
            ErrorKind::NotFound
        );
    }

    #[test]
    fn recommendations_can_be_cancelled() {
        let e = Engine::new();
        // The whole catalogue; the receiver goes away after the first
        // candidate: the candidates already started finish, no new one
        // starts.
        let mut request = recommend_request("cancel", Vec::new());
        request.catalogue_ids = e.catalogue().unwrap().into_iter().map(|c| c.id).collect();
        let r = e
            .recommend(&request, |ev| {
                !matches!(ev, RecommendEvent::Progress { .. })
            })
            .unwrap();
        assert!(r.cancelled);
        assert!(
            !r.candidates.is_empty() && r.candidates.len() < 20,
            "{}",
            r.candidates.len()
        );
        assert_eq!(r.rankings.least_drag.len(), r.candidates.len());
        assert!(!e.cancel("cancel"), "the job is forgotten once finished");
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
        let r1 = e.analyze(a.id, &at(4.0, 0.0, &INVISCID)).unwrap();
        let first = lock(&e.solvers).get(&a.id).map(|c| Arc::as_ptr(&c.solver));
        let r2 = e.analyze(a.id, &at(5.0, 0.0, &INVISCID)).unwrap();
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
        let err = e.analyze(a.id, &at(10.0, 0.6, &INVISCID)).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Mach);
        assert!(err.message.contains("critical Mach"));
        // ... and a flagged result when allowed (the cache is invalidated).
        let mut settings = e.session_view().settings;
        settings.allow_supercritical = true;
        e.update_settings(settings);
        assert!(
            e.analyze(a.id, &at(10.0, 0.6, &INVISCID))
                .unwrap()
                .supercritical
        );
        assert!(r1.viscous.is_none());
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
        assert_eq!(points(&polars[0]), 11);
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
        assert!(
            csv.lines()
                .nth(1)
                .unwrap()
                .starts_with("NACA 0012,inviscid,0,")
        );
    }

    #[test]
    fn sweeps_can_be_cancelled_and_flag_failures() {
        let e = Engine::new();
        let a = e.create_naca("0012", false).unwrap();
        let b = e.create_naca("2412", false).unwrap();
        let mut finished = None;
        // Cancelled through the job flag before any airfoil starts.
        let polars = e
            .sweep(&sweep_request(vec![a.id, b.id], 0.0), |ev| {
                match ev {
                    SweepEvent::Started { .. } => {
                        e.cancel("job");
                    }
                    SweepEvent::Finished { cancelled } => finished = Some(cancelled),
                    _ => {}
                }
                true
            })
            .unwrap();
        assert_eq!(finished, Some(true));
        assert!(polars.is_empty(), "no airfoil may start");
        assert!(!e.cancel("job"), "the job is no longer running");

        // Cancelled while the (parallel) viscous sweeps run: every sweep
        // under way stops, and its polar is marked cancelled.
        let viscous = SweepRequest {
            viscous: crate::model::ViscousSettings::default(),
            ..sweep_request(vec![a.id, b.id], 0.0)
        };
        let polars = e
            .sweep(&viscous, |ev| {
                if matches!(ev, SweepEvent::ViscousProgress { .. }) {
                    e.cancel("job");
                }
                true
            })
            .unwrap();
        assert!(!polars.is_empty());
        for p in &polars {
            match &p.polar {
                PolarData::Viscous(v) => assert!(v.cancelled && v.points.len() < 11),
                PolarData::Inviscid(_) => panic!("viscous sweep expected"),
            }
        }

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
    fn viscous_analysis_and_sweep() {
        let e = Engine::new();
        let a = e.create_naca("0012", false).unwrap();
        let viscous = ViscousSettings::default();
        assert!(viscous.enabled);
        let r = e.analyze(a.id, &at(4.0, 0.0, &viscous)).unwrap();
        let v = r.viscous.as_ref().unwrap();
        // XFOIL 6.996: CL 0.4278, CD 0.00728, transition 0.2537 / 0.9685.
        assert!((r.cl - 0.4278).abs() < 1e-4, "{}", r.cl);
        assert!((v.cd - 0.00728).abs() < 1e-5, "{}", v.cd);
        assert!(v.converged);
        assert!((v.transition[0].x - 0.2537).abs() < 1e-4);
        assert_eq!(v.cp_inviscid.len(), r.cp.len());
        assert!(!v.top.x.is_empty() && !v.wake.x.is_empty());

        // The viscous solver is cached for the same options only.
        let cached = |e: &Engine| {
            lock(&e.solvers)
                .get(&a.id)
                .and_then(|c| c.viscous.as_ref().map(|(_, s)| Arc::as_ptr(s)))
        };
        let first = cached(&e);
        e.analyze(a.id, &at(5.0, 0.0, &viscous)).unwrap();
        assert_eq!(cached(&e), first);
        let low_re = ViscousSettings {
            reynolds: 2.0e5,
            ..viscous
        };
        let r_low = e.analyze(a.id, &at(4.0, 0.0, &low_re)).unwrap();
        assert_ne!(cached(&e), first);
        assert!(r_low.viscous.unwrap().cd > v.cd);

        // Sweep: viscous progress events and a stored viscous polar.
        let request = SweepRequest {
            viscous,
            ..sweep_request(vec![a.id], 0.0)
        };
        let mut streamed = 0;
        let polars = e
            .sweep(&request, |ev| {
                if let SweepEvent::ViscousProgress { points, .. } = ev {
                    streamed += points.len();
                }
                true
            })
            .unwrap();
        assert!(matches!(polars[0].polar, PolarData::Viscous(_)));
        assert_eq!(points(&polars[0]), 11);
        assert_eq!(streamed, 11);
        let csv = e.polars_csv(&[a.id]);
        assert!(
            csv.lines()
                .nth(1)
                .unwrap()
                .starts_with("NACA 0012,viscous,0,1000000,9,9,1,1,0,")
        );

        let dir = std::env::temp_dir().join(format!("aero-app-visc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bl = dir.join("bl.csv");
        e.export_boundary_layer_csv(a.id, &at(4.0, 0.0, &viscous), &bl)
            .unwrap();
        let text = std::fs::read_to_string(&bl).unwrap();
        assert!(text.starts_with("surface,x,y,xi,ue,cp,dstar"));
        assert!(text.contains("\nwake,"));
        let cp = dir.join("cp.csv");
        e.export_cp_csv(a.id, &at(4.0, 0.0, &viscous), &cp).unwrap();
        assert!(
            std::fs::read_to_string(&cp)
                .unwrap()
                .starts_with("x,y,cp,cp_inviscid")
        );
        std::fs::remove_dir_all(&dir).ok();

        // Invalid settings are reported as such.
        let bad = ViscousSettings {
            reynolds: -1.0,
            ..viscous
        };
        let err = e.analyze(a.id, &at(4.0, 0.0, &bad)).unwrap_err();
        assert_eq!(err.kind, ErrorKind::InvalidInput);
    }

    #[test]
    fn prescribed_lift_and_flap_hinge() {
        let e = Engine::new();
        let a = e.create_naca("2412", false).unwrap();
        let hinge = Some([0.75, 0.0]);
        for viscous in [INVISCID, ViscousSettings::default()] {
            let request = AnalysisRequest {
                mode: OperatingMode::Cl,
                alpha_deg: 0.0,
                cl: 0.6,
                mach: 0.0,
                viscous,
                flap_hinge: hinge,
            };
            let r = e.analyze(a.id, &request).unwrap();
            assert!((r.cl - 0.6).abs() < 1e-3, "{}", r.cl);
            assert_eq!(r.cl_target, Some(0.6));
            assert!(r.alpha_deg > 0.0 && r.alpha_deg < 6.0);
            // Same hinge moment as the prescribed-α analysis at that angle.
            let back = e
                .analyze(
                    a.id,
                    &AnalysisRequest {
                        mode: OperatingMode::Alpha,
                        alpha_deg: r.alpha_deg,
                        ..request
                    },
                )
                .unwrap();
            let (h1, h2) = (r.hinge.unwrap().moment, back.hinge.unwrap().moment);
            assert!((h1 - h2).abs() < 1e-3, "{h1} {h2}");
        }
        // Prescribed-CL sweeps, inviscid and viscous.
        for viscous in [INVISCID, ViscousSettings::default()] {
            let request = SweepRequest {
                mode: OperatingMode::Cl,
                viscous,
                flap_hinge: hinge,
                ..sweep_request(vec![a.id], 0.0)
            };
            let polars = e.sweep(&request, |_| true).unwrap();
            let cls: Vec<f64> = match &polars[0].polar {
                PolarData::Inviscid(p) => p.points.iter().map(|q| q.cl).collect(),
                PolarData::Viscous(p) => {
                    assert!(p.prescribed_cl);
                    assert!(p.points.iter().all(|q| q.hinge_moment.is_some()));
                    p.points.iter().map(|q| q.cl).collect()
                }
            };
            assert!(cls.len() >= 10, "{cls:?}");
            assert!(cls.iter().zip(&cls[1..]).all(|(a, b)| b > a));
        }
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
        e.export_cp_csv(a.id, &at(4.0, 0.0, &INVISCID), &cp)
            .unwrap();
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

    fn optimization_request(airfoil_id: AirfoilId, evaluations: usize) -> OptimizeRequest {
        let mut setup = OptimizationSetup::default();
        setup.problem.points[0].target = aero_optim::Target::Cl(0.3);
        setup.options.max_evaluations = evaluations;
        setup.options.threads = 2;
        OptimizeRequest {
            job_id: "opt".into(),
            airfoil_id,
            setup,
        }
    }

    #[test]
    fn optimization_streams_stores_and_adds_its_result() {
        let e = Engine::new();
        let a = e.create_naca("0012", false).unwrap();
        let request = optimization_request(a.id, 30);
        let mut events = Vec::new();
        let r = e
            .optimize(&request, |event| {
                events.push(event);
                true
            })
            .unwrap();
        assert!(matches!(
            events.first(),
            Some(OptimizationEvent::Started { dimension: 14, .. })
        ));
        assert!(matches!(
            events.last(),
            Some(OptimizationEvent::Finished { cancelled: false })
        ));
        let streamed: usize = events
            .iter()
            .map(|event| match event {
                OptimizationEvent::Progress { history, .. } => history.len(),
                _ => 0,
            })
            .sum();
        assert_eq!(streamed, r.history.len());
        assert!(r.best.objective <= r.initial.objective);
        assert_eq!(r.problem.paneling, PanelingOptions::default());
        let view = e.session_view();
        assert_eq!(view.last_optimization.as_ref(), Some(&r));
        assert_eq!(view.optimization, request.setup);

        let added = e.add_optimized_airfoil().unwrap();
        assert_eq!(added.name, format!("{} optimized", a.name));
        assert!(matches!(added.origin, AirfoilOrigin::Optimized { .. }));
        // The optimized airfoil analyzes like the best design.
        let analysis = e
            .analyze(
                added.id,
                &AnalysisRequest {
                    mode: OperatingMode::Cl,
                    cl: 0.3,
                    ..at(0.0, 0.0, &crate::model::ViscousSettings::default())
                },
            )
            .unwrap();
        let cd = analysis.viscous.unwrap().cd;
        let best = r.best.points[0].values.unwrap().cd;
        assert!((cd / best - 1.0).abs() < 1e-3, "{cd} {best}");

        // Saved and read back with the project.
        let text = save_project(&lock(&e.session)).unwrap();
        assert_eq!(load_project(&text).unwrap().last_optimization, Some(r));
    }

    #[test]
    fn optimization_can_be_cancelled() {
        let e = Engine::new();
        let a = e.create_naca("2412", false).unwrap();
        let mut finished = None;
        let r = e
            .optimize(&optimization_request(a.id, 500), |event| match event {
                OptimizationEvent::Progress { .. } => false,
                OptimizationEvent::Finished { cancelled } => {
                    finished = Some(cancelled);
                    true
                }
                _ => true,
            })
            .unwrap();
        assert_eq!(r.stop, aero_optim::Stop::Cancelled);
        assert_eq!(r.history.len(), 1);
        assert_eq!(finished, Some(true));
        assert!(!e.cancel("opt"), "the job is unregistered");
        let mut bad = optimization_request(a.id, 500);
        bad.setup.problem.points.clear();
        assert_eq!(
            e.optimize(&bad, |_| true).unwrap_err().kind,
            ErrorKind::InvalidInput
        );
    }

    #[test]
    fn designation_lists() {
        assert_eq!(
            parse_designations(" 0012, 2412  4415;NACA 23012 naca6409\n"),
            ["0012", "2412", "4415", "23012", "6409"]
        );
        assert!(parse_designations(" , ; ").is_empty());
    }

    #[test]
    fn many_airfoils_at_once() {
        let e = Engine::new();
        let r = e
            .create_naca_series("0012, 2412 9z99, 4415", false)
            .unwrap();
        let names: Vec<&str> = r.added.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["NACA 0012", "NACA 2412", "NACA 4415"]);
        assert_eq!(r.failed.len(), 1);
        assert_eq!(r.failed[0].item, "9z99");
        assert!(e.create_naca_series("  ", false).is_err());

        // A folder: coordinate files in name order, other files ignored,
        // unreadable files reported.
        let dir = std::env::temp_dir().join(format!("aero-batch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("b.dat"), e.dat_text(r.added[1].id).unwrap()).unwrap();
        std::fs::write(dir.join("a.dat"), e.dat_text(r.added[0].id).unwrap()).unwrap();
        std::fs::write(dir.join("broken.dat"), "not an airfoil").unwrap();
        std::fs::write(dir.join("notes.md"), "ignored").unwrap();
        let empty = dir.join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        let imported = e.import_paths(&[dir.clone(), empty, dir.join("missing.dat")]);
        std::fs::remove_dir_all(&dir).ok();
        let files: Vec<&str> = imported
            .imported
            .iter()
            .map(|r| r.airfoil.name.as_str())
            .collect();
        assert_eq!(files, ["NACA 0012", "NACA 2412"]);
        let failed: Vec<&str> = imported.failed.iter().map(|f| f.item.as_str()).collect();
        assert_eq!(failed, ["empty", "broken.dat", "missing.dat"]);

        let examples = e.add_examples().unwrap();
        assert_eq!(examples.len(), 8);
        assert_eq!(examples[6].name, "Clark Y");
        assert_eq!(examples[7].name, "Eppler 387");
        let all: Vec<AirfoilId> = e.session_view().airfoils.iter().map(|a| a.id).collect();
        assert_eq!(all.len(), 13);
        e.set_airfoils_visible(&all[..10], false).unwrap();
        assert_eq!(
            e.session_view()
                .airfoils
                .iter()
                .filter(|a| a.visible)
                .count(),
            3
        );
        e.delete_airfoils(&all[..10]).unwrap();
        assert_eq!(e.session_view().airfoils.len(), 3);
        assert!(e.delete_airfoils(&[999]).is_err());
    }

    #[test]
    fn parallel_sweeps_equal_sweeps_one_by_one() {
        let e = Engine::new();
        let ids: Vec<AirfoilId> = e.add_examples().unwrap().iter().map(|a| a.id).collect();
        for viscous in [INVISCID, crate::model::ViscousSettings::default()] {
            let request = SweepRequest {
                viscous,
                sweep: AlphaSweep {
                    start_deg: -2.0,
                    end_deg: 12.0,
                    step_deg: 1.0,
                },
                ..sweep_request(ids.clone(), 0.0)
            };
            let mut events = Vec::new();
            let together = e
                .sweep(&request, |event| {
                    events.push(event);
                    true
                })
                .unwrap();
            assert_eq!(together.len(), ids.len());
            for (stored, &id) in together.iter().zip(&ids) {
                assert_eq!(stored.airfoil_id, id);
                // The same airfoil swept alone (a single worker).
                let single = e
                    .sweep(
                        &SweepRequest {
                            airfoil_ids: vec![id],
                            ..request.clone()
                        },
                        |_| true,
                    )
                    .unwrap();
                assert_eq!(single[0].polar, stored.polar, "airfoil {id}");
            }
            // Every point is reported once, the last count is the total.
            let mut streamed = 0;
            let mut last_done = 0;
            let mut total = 0;
            for event in &events {
                match event {
                    SweepEvent::Started { total: t } => total = *t,
                    SweepEvent::Progress { points, done, .. } => {
                        streamed += points.len();
                        last_done = last_done.max(*done);
                    }
                    SweepEvent::ViscousProgress { points, done, .. } => {
                        streamed += points.len();
                        last_done = last_done.max(*done);
                    }
                    SweepEvent::Finished { cancelled } => assert!(!cancelled),
                }
            }
            assert_eq!(streamed, together.iter().map(points).sum::<usize>());
            assert_eq!(last_done, total);
        }
    }

    #[test]
    fn viscous_sweeps_march_outward_from_zero() {
        assert_eq!(
            outward_sequences(&[-4.0, -3.0, -2.0, -1.0, 0.0, 1.0, 2.0]),
            [(0.0, 2.0), (-1.0, -4.0)]
        );
        assert_eq!(outward_sequences(&[0.0, 1.0, 2.0]), [(0.0, 2.0)]);
        assert_eq!(
            outward_sequences(&[2.0, 1.0, -1.0]),
            [(1.0, -1.0), (2.0, 2.0)]
        );
        assert!(outward_sequences(&[]).is_empty());

        let e = Engine::new();
        let examples = e.add_examples().unwrap();
        let e387 = examples[7].id;
        let viscous = crate::model::ViscousSettings::default();
        // Starting at -4 deg, in separated flow, XFOIL's sequence on the
        // Eppler 387 stops after 4 unconverged points; marching outward from
        // 0 deg gives the whole polar.
        let request = SweepRequest {
            viscous,
            sweep: AlphaSweep {
                start_deg: -4.0,
                end_deg: 14.0,
                step_deg: 0.5,
            },
            ..sweep_request(vec![e387], 0.0)
        };
        let solver = e.viscous_solver(e387, None, &viscous.options(0.0)).unwrap();
        assert!(solver.sweep(&request.sweep).unwrap().points.len() < 5);
        let stored = e.sweep(&request, |_| true).unwrap();
        let PolarData::Viscous(polar) = &stored[0].polar else {
            panic!("viscous polar expected")
        };
        assert!(polar.points.len() > 30, "{}", polar.points.len());
        let alphas: Vec<f64> = polar.points.iter().map(|p| p.alpha_deg).collect();
        assert!(alphas.windows(2).all(|w| w[0] < w[1]), "{alphas:?}");
        let cl_max = polar.points.iter().map(|p| p.cl).fold(f64::MIN, f64::max);
        assert!(cl_max > 1.0, "{cl_max}");

        // A sweep that starts at 0 is exactly XFOIL's ASEQ.
        let from_zero = SweepRequest {
            sweep: AlphaSweep {
                start_deg: 0.0,
                end_deg: 8.0,
                step_deg: 1.0,
            },
            ..request
        };
        let stored = e.sweep(&from_zero, |_| true).unwrap();
        let PolarData::Viscous(polar) = &stored[0].polar else {
            panic!("viscous polar expected")
        };
        assert_eq!(polar.points, solver.sweep(&from_zero.sweep).unwrap().points);
    }

    #[test]
    fn modifications_preview_and_create_airfoils() {
        let e = Engine::new();
        let a = e.create_naca("2412", false).unwrap();
        let b = e.create_naca("4412", false).unwrap();
        let flap = Modification::Flap {
            hinge_x: 0.75,
            hinge_y: HingeY::Relative(0.5),
            deflection_deg: 10.0,
        };
        let preview = e.preview_modification(a.id, &flap).unwrap();
        assert_eq!(preview.description, "flap 10° at x 0.75");
        assert_eq!(e.session_view().airfoils.len(), 2, "a preview adds nothing");

        let created = e.apply_modification(a.id, &flap, None).unwrap();
        assert_eq!(created.name, "NACA 2412 (flap 10° at x 0.75)");
        assert!(matches!(created.origin, AirfoilOrigin::Modified { .. }));
        let g = e.geometry(created.id).unwrap();
        assert_eq!(g.x, preview.x);

        let half = e
            .apply_modification(
                a.id,
                &Modification::Interpolate {
                    other: b.id,
                    fraction: 0.5,
                },
                Some("between"),
            )
            .unwrap();
        assert_eq!(half.name, "between");
        let camber = e.camber_thickness(half.id).unwrap().max_camber;
        assert!((camber - 0.03).abs() < 1e-3, "{camber}");

        let series = e
            .flap_series(a.id, 0.75, HingeY::Relative(0.5), &[0.0, 5.0, 10.0])
            .unwrap();
        let names: Vec<&str> = series.added.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "NACA 2412 flap 0°",
                "NACA 2412 flap 5°",
                "NACA 2412 flap 10°"
            ]
        );
        let bad = Modification::SetThicknessCamber {
            thickness: Some(50.0),
            camber: None,
        };
        assert_eq!(
            e.preview_modification(a.id, &bad).unwrap_err().kind,
            ErrorKind::InvalidInput
        );
    }
}
