//! Session model and project file format.
//!
//! This module does not depend on Tauri: it is plain data plus pure
//! operations, so that it can be reused by a CLI later.

use aero_foil2d::inviscid::{AlphaSweep, ClSweep, Polar};
use aero_foil2d::viscous::{
    AmplificationModel, MachReynoldsVariation, ViscousOptions, ViscousPolar,
};
use aero_geometry::{Airfoil, Normalization, PanelingOptions, panel};
use aero_optim::{
    Constraints, DesignPoint, Evaluation, HistoryEntry, Objective, OptimizeOptions, Problem, Stop,
    Target,
};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::{AppError, ErrorKind};

/// Identifier of an airfoil in the session.
pub type AirfoilId = u32;

/// Version of the project file format written by this application.
///
/// - 1: inviscid analyses only.
/// - 2: viscous settings; stored polars are tagged `inviscid` or `viscous`.
/// - 3: shape optimization setup and last result.
pub const PROJECT_FORMAT_VERSION: u32 = 3;

/// Magic string identifying project files.
pub const PROJECT_FORMAT: &str = "aero-project";

/// Plot colors assigned to new airfoils, in order.
const PALETTE: [&str; 16] = [
    "#2563eb", "#dc2626", "#16a34a", "#d97706", "#7c3aed", "#0891b2", "#db2777", "#65a30d",
    "#4f46e5", "#ea580c", "#0d9488", "#9333ea", "#be123c", "#15803d", "#a16207", "#0369a1",
];

/// Where an airfoil comes from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AirfoilOrigin {
    /// Generated NACA section.
    Naca {
        /// Designation, e.g. "2412".
        designation: String,
        /// Sharp (closed) trailing edge.
        closed_te: bool,
    },
    /// Imported coordinate file.
    Imported {
        /// File name (without directory).
        file_name: String,
    },
    /// Copy of another airfoil.
    Duplicate {
        /// Name of the original airfoil.
        of: String,
    },
    /// Result of a shape optimization.
    Optimized {
        /// Name of the initial airfoil.
        of: String,
    },
}

/// An airfoil of the session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AirfoilEntry {
    /// Identifier.
    pub id: AirfoilId,
    /// Geometry as generated or imported (before paneling).
    pub source: Airfoil,
    /// Origin of the geometry.
    pub origin: AirfoilOrigin,
    /// Paneling parameters used for the analyses.
    pub paneling: PanelingOptions,
    /// Shown in the plots.
    pub visible: bool,
    /// Plot color (CSS).
    pub color: String,
    /// Incremented whenever the geometry or the paneling changes; used to
    /// invalidate cached solvers. Not saved.
    #[serde(skip)]
    pub revision: u64,
}

impl AirfoilEntry {
    /// Display name.
    pub fn name(&self) -> &str {
        self.source.name()
    }

    /// Paneled airfoil (XFOIL `PANGEN`).
    pub fn paneled(&self) -> Result<Airfoil, AppError> {
        Ok(panel(&self.source, &self.paneling)?)
    }
}

/// Viscous analysis settings (XFOIL `VISC`, `VPAR`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type)]
pub struct ViscousSettings {
    /// Viscous analysis (boundary layer, drag, transition) instead of the
    /// inviscid panel method alone.
    pub enabled: bool,
    /// Chord Reynolds number.
    pub reynolds: f64,
    /// Critical amplification ratio Ncrit on the top and bottom surfaces.
    pub ncrit: [f64; 2],
    /// Trip locations x/c on the top and bottom surfaces (1 = free
    /// transition).
    pub xtr: [f64; 2],
    /// Dependence of the Mach and Reynolds numbers on the lift (XFOIL
    /// `TYPE`).
    #[serde(default)]
    pub variation: MachReynoldsVariation,
    /// Amplification rate correlation (XFOIL `DAMP`).
    #[serde(default)]
    pub amplification: AmplificationModel,
}

impl Default for ViscousSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            reynolds: 1.0e6,
            ncrit: [9.0, 9.0],
            xtr: [1.0, 1.0],
            variation: MachReynoldsVariation::Fixed,
            amplification: AmplificationModel::Original,
        }
    }
}

impl ViscousSettings {
    /// Solver options for a Mach number.
    pub fn options(&self, mach: f64) -> ViscousOptions {
        ViscousOptions {
            reynolds: self.reynolds,
            mach,
            ncrit: self.ncrit,
            xtr: self.xtr,
            variation: self.variation,
            amplification: self.amplification,
            ..ViscousOptions::default()
        }
    }
}

/// What single-point analyses and sweeps prescribe.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum OperatingMode {
    /// The angle of attack (XFOIL `ALFA`, `ASEQ`).
    #[default]
    Alpha,
    /// The lift coefficient (XFOIL `CL`, `CSEQ`).
    Cl,
}

fn default_cl() -> f64 {
    0.5
}

fn default_cl_sweep() -> ClSweep {
    ClSweep {
        start: 0.0,
        end: 1.2,
        step: 0.1,
    }
}

/// Analysis settings of the session.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type)]
pub struct Settings {
    /// Angle of attack of the single-point analysis, degrees.
    pub alpha_deg: f64,
    /// Freestream Mach number (0 for incompressible flow).
    pub mach: f64,
    /// Angle-of-attack sweep.
    pub sweep: AlphaSweep,
    /// Allow solutions beyond the critical Mach number (XFOIL behaviour).
    pub allow_supercritical: bool,
    /// Show the panel nodes in the airfoil view.
    pub show_nodes: bool,
    /// Viscous analysis settings (absent from format 1 projects).
    #[serde(default)]
    pub viscous: ViscousSettings,
    /// Prescribed angle of attack or lift coefficient.
    #[serde(default)]
    pub mode: OperatingMode,
    /// Lift coefficient of the single-point analysis in prescribed-CL mode.
    #[serde(default = "default_cl")]
    pub cl: f64,
    /// Lift-coefficient sweep in prescribed-CL mode.
    #[serde(default = "default_cl_sweep")]
    pub cl_sweep: ClSweep,
    /// Flap hinge for the hinge moments (XFOIL `FNEW`).
    #[serde(default)]
    pub flap_hinge: Option<[f64; 2]>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            alpha_deg: 4.0,
            mach: 0.0,
            sweep: AlphaSweep {
                start_deg: -6.0,
                end_deg: 12.0,
                step_deg: 0.5,
            },
            allow_supercritical: false,
            show_nodes: false,
            viscous: ViscousSettings::default(),
            mode: OperatingMode::Alpha,
            cl: default_cl(),
            cl_sweep: default_cl_sweep(),
            flap_hinge: None,
        }
    }
}

/// A computed polar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PolarData {
    /// Inviscid sweep.
    Inviscid(Polar),
    /// Viscous sweep.
    Viscous(ViscousPolar),
}

/// Sweep result stored in the session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct StoredPolar {
    /// Airfoil the polar belongs to.
    pub airfoil_id: AirfoilId,
    /// Name of the airfoil when the polar was computed.
    pub airfoil_name: String,
    /// The polar.
    pub polar: PolarData,
}

/// Definition of the shape optimization of the Optimize tab.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct OptimizationSetup {
    /// The problem; its paneling is replaced by the airfoil's when run.
    pub problem: Problem,
    /// Search options.
    pub options: OptimizeOptions,
}

impl Default for OptimizationSetup {
    /// Minimum drag at CL = 0.5, Re = 1e6, thickness at least 12 %.
    fn default() -> Self {
        Self {
            problem: Problem {
                points: vec![DesignPoint {
                    target: Target::Cl(0.5),
                    weight: 1.0,
                }],
                objective: Objective::MinDrag,
                constraints: Constraints {
                    min_thickness: Some(0.12),
                    ..Constraints::default()
                },
                viscous: ViscousOptions::default(),
                paneling: PanelingOptions::default(),
                design: Default::default(),
            },
            options: OptimizeOptions {
                max_evaluations: 600,
                ..OptimizeOptions::default()
            },
        }
    }
}

/// Airfoil coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct Shape {
    /// Abscissas.
    pub x: Vec<f64>,
    /// Ordinates.
    pub y: Vec<f64>,
}

impl From<&Airfoil> for Shape {
    fn from(a: &Airfoil) -> Self {
        Self {
            x: a.x().to_vec(),
            y: a.y().to_vec(),
        }
    }
}

/// Result of the last shape optimization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct StoredOptimization {
    /// Airfoil that was optimized.
    pub airfoil_id: AirfoilId,
    /// Its name at the time.
    pub airfoil_name: String,
    /// The problem, with the paneling used.
    pub problem: Problem,
    /// The search options.
    pub options: OptimizeOptions,
    /// The fitted initial design.
    pub initial: Evaluation,
    /// The best design.
    pub best: Evaluation,
    /// Coordinates of the fitted initial design.
    pub initial_shape: Shape,
    /// Coordinates of the best design.
    pub best_shape: Shape,
    /// Rms ordinate error of the CST fit of the initial airfoil.
    pub fit_rms_error: f64,
    /// Every evaluation, in order.
    pub history: Vec<HistoryEntry>,
    /// Why the search stopped.
    pub stop: Stop,
    /// Duration, seconds.
    pub elapsed_s: f64,
}

/// The whole state of a working session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Session {
    /// Airfoil library.
    pub airfoils: Vec<AirfoilEntry>,
    /// Next identifier to assign.
    pub next_id: AirfoilId,
    /// Analysis settings.
    pub settings: Settings,
    /// Computed polars (one per airfoil, the latest sweep).
    pub polars: Vec<StoredPolar>,
    /// Shape optimization setup (absent before format 3).
    #[serde(default)]
    pub optimization: OptimizationSetup,
    /// Result of the last shape optimization.
    #[serde(default)]
    pub last_optimization: Option<StoredOptimization>,
}

impl Session {
    /// Adds an airfoil and returns the new entry.
    pub fn add(&mut self, airfoil: Airfoil, origin: AirfoilOrigin) -> &AirfoilEntry {
        let id = self.next_id;
        self.next_id += 1;
        let color = PALETTE[id as usize % PALETTE.len()].to_string();
        let index = self.airfoils.len();
        self.airfoils.push(AirfoilEntry {
            id,
            source: airfoil,
            origin,
            paneling: PanelingOptions::default(),
            visible: true,
            color,
            revision: 0,
        });
        &self.airfoils[index]
    }

    /// Looks up an airfoil.
    pub fn get(&self, id: AirfoilId) -> Result<&AirfoilEntry, AppError> {
        self.airfoils
            .iter()
            .find(|a| a.id == id)
            .ok_or_else(|| not_found(id))
    }

    fn get_mut(&mut self, id: AirfoilId) -> Result<&mut AirfoilEntry, AppError> {
        self.airfoils
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or_else(|| not_found(id))
    }

    /// Duplicates an airfoil (named "`<name> (copy)`") and returns the new id.
    pub fn duplicate(&mut self, id: AirfoilId) -> Result<AirfoilId, AppError> {
        let original = self.get(id)?.clone();
        let name = format!("{} (copy)", original.name());
        let new_id = self
            .add(
                original.source.clone().with_name(name),
                AirfoilOrigin::Duplicate {
                    of: original.name().to_string(),
                },
            )
            .id;
        self.get_mut(new_id)?.paneling = original.paneling;
        Ok(new_id)
    }

    /// Adds the best design of the last optimization to the library, with
    /// the paneling it was optimized with, and returns its id.
    pub fn add_optimized(&mut self) -> Result<AirfoilId, AppError> {
        let opt = self
            .last_optimization
            .clone()
            .ok_or_else(|| AppError::new(ErrorKind::NotFound, "no optimization result to add"))?;
        let airfoil = Airfoil::new(
            format!("{} optimized", opt.airfoil_name),
            opt.best_shape.x,
            opt.best_shape.y,
        )?;
        let id = self
            .add(
                airfoil,
                AirfoilOrigin::Optimized {
                    of: opt.airfoil_name,
                },
            )
            .id;
        self.get_mut(id)?.paneling = opt.problem.paneling;
        Ok(id)
    }

    /// Renames an airfoil. The name must not be blank.
    pub fn rename(&mut self, id: AirfoilId, name: &str) -> Result<(), AppError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::new(
                ErrorKind::InvalidInput,
                "the airfoil name cannot be empty",
            ));
        }
        let entry = self.get_mut(id)?;
        entry.source.set_name(name);
        for p in self.polars.iter_mut().filter(|p| p.airfoil_id == id) {
            p.airfoil_name = name.to_string();
        }
        Ok(())
    }

    /// Removes an airfoil and its results.
    pub fn remove(&mut self, id: AirfoilId) -> Result<(), AppError> {
        let before = self.airfoils.len();
        self.airfoils.retain(|a| a.id != id);
        if self.airfoils.len() == before {
            return Err(not_found(id));
        }
        self.polars.retain(|p| p.airfoil_id != id);
        Ok(())
    }

    /// Shows or hides an airfoil in the plots.
    pub fn set_visible(&mut self, id: AirfoilId, visible: bool) -> Result<(), AppError> {
        self.get_mut(id)?.visible = visible;
        Ok(())
    }

    /// Changes the paneling parameters (validated by repaneling first).
    pub fn set_paneling(
        &mut self,
        id: AirfoilId,
        paneling: PanelingOptions,
    ) -> Result<(), AppError> {
        let entry = self.get_mut(id)?;
        panel(&entry.source, &paneling)?;
        entry.paneling = paneling;
        entry.revision += 1;
        self.polars.retain(|p| p.airfoil_id != id);
        Ok(())
    }

    /// Normalizes the source geometry of an airfoil.
    pub fn normalize(&mut self, id: AirfoilId, mode: Normalization) -> Result<(), AppError> {
        let entry = self.get_mut(id)?;
        entry.source = entry.source.normalized(mode)?;
        entry.revision += 1;
        self.polars.retain(|p| p.airfoil_id != id);
        Ok(())
    }

    /// Stores (or replaces) the polar of an airfoil.
    pub fn store_polar(&mut self, airfoil_id: AirfoilId, polar: PolarData) -> Result<(), AppError> {
        let airfoil_name = self.get(airfoil_id)?.name().to_string();
        self.polars.retain(|p| p.airfoil_id != airfoil_id);
        self.polars.push(StoredPolar {
            airfoil_id,
            airfoil_name,
            polar,
        });
        Ok(())
    }
}

fn not_found(id: AirfoilId) -> AppError {
    AppError::new(ErrorKind::NotFound, format!("airfoil {id} does not exist"))
}

/// Project file: a session with format metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectFile {
    /// Always [`PROJECT_FORMAT`].
    pub format: String,
    /// Format version, [`PROJECT_FORMAT_VERSION`] when written.
    pub format_version: u32,
    /// Version of the application that wrote the file.
    pub app_version: String,
    /// The session.
    pub session: Session,
}

/// Serializes a session as a project file (pretty JSON).
pub fn save_project(session: &Session) -> Result<String, AppError> {
    let file = ProjectFile {
        format: PROJECT_FORMAT.to_string(),
        format_version: PROJECT_FORMAT_VERSION,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        session: session.clone(),
    };
    serde_json::to_string_pretty(&file).map_err(|e| {
        AppError::new(
            ErrorKind::Project,
            format!("cannot serialize the project: {e}"),
        )
    })
}

/// Reads a project file, checking its format and version.
pub fn load_project(text: &str) -> Result<Session, AppError> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| {
        AppError::new(
            ErrorKind::Project,
            format!("the file is not valid JSON: {e}"),
        )
    })?;
    if value.get("format").and_then(|v| v.as_str()) != Some(PROJECT_FORMAT) {
        return Err(AppError::new(
            ErrorKind::Project,
            "this file is not an aero project",
        ));
    }
    let version = value
        .get("format_version")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| AppError::new(ErrorKind::Project, "the project has no format version"))?;
    if version > u64::from(PROJECT_FORMAT_VERSION) {
        return Err(AppError::new(
            ErrorKind::Project,
            format!(
                "this project was written by a newer version of the application (format {version}, \
                 this version reads up to {PROJECT_FORMAT_VERSION}); please update the application"
            ),
        ));
    }
    // Format migrations, keyed on `version`.
    let mut value = value;
    if version < 2 {
        // Format 1 stored inviscid polars only, untagged.
        if let Some(polars) = value
            .pointer_mut("/session/polars")
            .and_then(serde_json::Value::as_array_mut)
        {
            for p in polars {
                if let Some(polar) = p
                    .get_mut("polar")
                    .and_then(serde_json::Value::as_object_mut)
                {
                    polar.insert("kind".into(), "inviscid".into());
                }
            }
        }
    }
    let file: ProjectFile = serde_json::from_value(value)
        .map_err(|e| AppError::new(ErrorKind::Project, format!("invalid project content: {e}")))?;
    let mut session = file.session;
    // Identifiers must stay unique after loading.
    let max_id = session.airfoils.iter().map(|a| a.id).max();
    if let Some(max_id) = max_id {
        session.next_id = session.next_id.max(max_id + 1);
    }
    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aero_geometry::{NacaOptions, naca};

    fn session_with_two() -> (Session, AirfoilId, AirfoilId) {
        let mut s = Session::default();
        let a = s
            .add(
                naca("0012", &NacaOptions::default()).unwrap(),
                AirfoilOrigin::Naca {
                    designation: "0012".into(),
                    closed_te: false,
                },
            )
            .id;
        let b = s
            .add(
                naca("2412", &NacaOptions::default()).unwrap(),
                AirfoilOrigin::Imported {
                    file_name: "n2412.dat".into(),
                },
            )
            .id;
        (s, a, b)
    }

    #[test]
    fn library_operations() {
        let (mut s, a, b) = session_with_two();
        assert_ne!(a, b);
        assert_ne!(s.get(a).unwrap().color, s.get(b).unwrap().color);

        let c = s.duplicate(a).unwrap();
        assert_eq!(s.get(c).unwrap().name(), "NACA 0012 (copy)");
        assert_eq!(s.get(c).unwrap().source.x(), s.get(a).unwrap().source.x());

        s.rename(c, "  my foil ").unwrap();
        assert_eq!(s.get(c).unwrap().name(), "my foil");
        assert!(s.rename(c, "   ").is_err());

        s.set_visible(b, false).unwrap();
        assert!(!s.get(b).unwrap().visible);

        s.remove(b).unwrap();
        assert!(s.get(b).is_err());
        assert!(s.remove(b).is_err());
        assert_eq!(s.airfoils.len(), 2);
    }

    #[test]
    fn paneling_changes_bump_revision_and_drop_results() {
        let (mut s, a, _) = session_with_two();
        let polar = Polar {
            mach: 0.0,
            points: vec![],
            failures: vec![],
            cancelled: false,
        };
        s.store_polar(a, PolarData::Inviscid(polar.clone()))
            .unwrap();
        assert_eq!(s.polars.len(), 1);
        let opts = PanelingOptions {
            n_nodes: 100,
            ..PanelingOptions::default()
        };
        s.set_paneling(a, opts).unwrap();
        assert_eq!(s.get(a).unwrap().revision, 1);
        assert!(s.polars.is_empty());
        assert_eq!(s.get(a).unwrap().paneled().unwrap().len(), 100);
        let bad = PanelingOptions {
            n_nodes: 2,
            ..PanelingOptions::default()
        };
        assert!(s.set_paneling(a, bad).is_err());
        assert_eq!(s.get(a).unwrap().paneling.n_nodes, 100);
        s.store_polar(a, PolarData::Inviscid(polar)).unwrap();
        s.normalize(a, Normalization::ChordAligned).unwrap();
        assert_eq!(s.get(a).unwrap().revision, 2);
        assert!(s.polars.is_empty());
    }

    #[test]
    fn project_round_trip() {
        let (mut s, a, _) = session_with_two();
        s.settings.mach = 0.3;
        s.store_polar(
            a,
            PolarData::Inviscid(Polar {
                mach: 0.3,
                points: vec![],
                failures: vec![],
                cancelled: false,
            }),
        )
        .unwrap();
        s.settings.viscous.reynolds = 3.0e5;
        let text = save_project(&s).unwrap();
        let back = load_project(&text).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn project_versions_are_checked() {
        let (s, _, _) = session_with_two();
        let text = save_project(&s).unwrap();
        let newer = text.replace(
            &format!("\"format_version\": {PROJECT_FORMAT_VERSION}"),
            "\"format_version\": 99",
        );
        let err = load_project(&newer).unwrap_err();
        assert!(err.message.contains("newer version"), "{}", err.message);
        assert!(load_project("{}").is_err());
        assert!(load_project("not json").is_err());
        let wrong = text.replace("aero-project", "something-else");
        assert!(load_project(&wrong).is_err());
    }

    #[test]
    fn project_with_invalid_geometry_is_rejected() {
        let (s, _, _) = session_with_two();
        let text = save_project(&s).unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
        value["session"]["airfoils"][0]["source"]["x"] = serde_json::json!([0.0, 1.0]);
        let err = load_project(&value.to_string()).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Project);
    }

    #[test]
    fn version_1_fixture_still_loads() {
        // Frozen file written by format version 1: guarantees backward
        // compatibility of future versions.
        let text = include_str!("../tests/fixtures/project_v1.json");
        let s = load_project(text).unwrap();
        assert_eq!(s.airfoils.len(), 1);
        assert_eq!(s.airfoils[0].name(), "NACA 2412");
        assert_eq!(s.next_id, 1);
        assert_eq!(s.polars.len(), 1);
        assert!(matches!(s.polars[0].polar, PolarData::Inviscid(_)));
        // Settings that did not exist in format 1 get their defaults.
        assert_eq!(s.settings.viscous, ViscousSettings::default());
    }

    #[test]
    fn version_2_fixture_still_loads() {
        // Frozen file of format version 2 (viscous settings, prescribed CL,
        // flap hinge; no optimization).
        let text = include_str!("../tests/fixtures/project_v2.json");
        let s = load_project(text).unwrap();
        assert_eq!(s.airfoils.len(), 1);
        assert_eq!(s.settings.mode, OperatingMode::Cl);
        assert_eq!(s.settings.cl, 0.6);
        assert_eq!(s.settings.flap_hinge, Some([0.7, 0.0]));
        assert_eq!(
            s.settings.viscous.variation,
            MachReynoldsVariation::InverseSqrtCl
        );
        assert_eq!(s.optimization, OptimizationSetup::default());
        assert_eq!(s.last_optimization, None);
    }
}
