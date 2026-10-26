//! How the aircraft flies: its vortex lattice (AVL's method, `aero-vlm`)
//! joined to the viscous polars of its airfoils, as XFLR5's viscous VLM.
//!
//! For a level flight ([`analyze`]):
//!
//! - trim: the angle of attack and the elevator that give the required
//!   lift with the pitching moment balanced about the centre of gravity;
//! - stability: neutral point and static margin, weathercock and dihedral
//!   effects, spiral stability;
//! - each strip of the lattice at its own Reynolds number: its lift
//!   coefficient against the maximum of its airfoil, its profile drag from
//!   the polar;
//! - drag: induced (Trefftz plane), profile (the strips), fuselage skin
//!   friction; glide ratio and power;
//! - stall: the aircraft's maximum trimmed lift coefficient, when the first
//!   strip reaches the maximum of its airfoil (critical-section method), the
//!   stall speed, and where the stall starts.
//!
//! The 2D polars are the costly part: they come from a [`PolarSource`]
//! (usually a [`PolarCache`]) at Reynolds numbers on a fixed grid (steps of
//! 10^0.2), so that small changes of the aircraft reuse them.

use std::collections::{BTreeMap, HashMap};
use std::f64::consts::PI;

use aero_aircraft::{
    Aircraft, AircraftLattice, ELEVATOR, Fuselage, LatticeOptions, SurfaceKind, lattice, layout,
};
use aero_foil2d::viscous::{ViscousOptions, ViscousSolver};
use aero_geometry::Airfoil;
use aero_vlm::{FlowCondition, Solution, VortexLattice};
use serde::{Deserialize, Serialize};

use crate::error::{MissionError, Result};
use crate::flight::Flight;
use crate::recommend::{AnalysisSettings, SectionPoint};

/// Reynolds numbers of the polars: `10^(k/5)`.
pub const REYNOLDS_STEPS_PER_DECADE: f64 = 5.0;

/// Largest elevator deflection the linearized controls are trusted with,
/// degrees.
pub const MAX_ELEVATOR: f64 = 15.0;

/// Step of the Mach numbers of the polars (rounded down to it): the
/// compressibility of the boundary layer hardly changes below it, and the
/// polars are reused across speeds.
pub const POLAR_MACH_STEP: f64 = 0.05;

/// A viscous polar of an airfoil at one Reynolds number.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionPolar {
    /// Reynolds number.
    pub reynolds: f64,
    /// Converged points by increasing angle.
    pub points: Vec<SectionPoint>,
}

impl SectionPolar {
    /// The attached-flow branch: from the negative stall to the positive
    /// one (the points between the lowest and the highest lift).
    fn attached(&self) -> &[SectionPoint] {
        let arg = |better: fn(f64, f64) -> bool| {
            let mut best = 0;
            for (i, p) in self.points.iter().enumerate() {
                if better(p.cl, self.points[best].cl) {
                    best = i;
                }
            }
            best
        };
        if self.points.is_empty() {
            return &[];
        }
        let lo = arg(|a, b| a < b);
        let hi = arg(|a, b| a > b);
        if lo <= hi { &self.points[lo..=hi] } else { &[] }
    }

    /// Highest lift coefficient.
    pub fn cl_max(&self) -> Option<f64> {
        self.attached().last().map(|p| p.cl)
    }

    /// Lowest lift coefficient (the negative stall, or the lowest angle
    /// swept).
    pub fn cl_min(&self) -> Option<f64> {
        self.attached().first().map(|p| p.cl)
    }

    /// Drag coefficient at the lift coefficient `cl` on the attached
    /// branch, clamped to its ends.
    pub fn cd_at(&self, cl: f64) -> Option<f64> {
        let a = self.attached();
        let (first, last) = (a.first()?, a.last()?);
        if cl <= first.cl {
            return Some(first.cd);
        }
        if cl >= last.cl {
            return Some(last.cd);
        }
        a.windows(2).find_map(|w| {
            let (p, q) = (w[0], w[1]);
            (cl >= p.cl && cl <= q.cl && q.cl > p.cl)
                .then(|| p.cd + (cl - p.cl) / (q.cl - p.cl) * (q.cd - p.cd))
        })
    }
}

/// Where the 2D polars come from.
pub trait PolarSource {
    /// The polar of the airfoil `key` at the Reynolds number `reynolds` and
    /// the Mach number `mach`.
    fn polar(&mut self, key: &str, reynolds: f64, mach: f64) -> Result<SectionPolar>;
}

/// The viscous polar of `airfoil` at `reynolds` and `mach`: swept from 0°
/// up to the stall and down to the negative stall (or the settings' angle
/// limits), each point starting from the previous boundary layer.
pub fn section_polar(
    airfoil: &Airfoil,
    reynolds: f64,
    mach: f64,
    settings: &AnalysisSettings,
) -> Result<SectionPolar> {
    if !(settings.alpha_step > 0.0 && settings.alpha_min < 0.0 && settings.alpha_max > 0.0) {
        return Err(MissionError::InvalidInput(
            "the polar must sweep from a negative to a positive angle".into(),
        ));
    }
    let paneled = airfoil.paneled(&settings.paneling)?;
    let options = ViscousOptions {
        reynolds,
        mach,
        ncrit: [settings.ncrit; 2],
        ..ViscousOptions::default()
    };
    let solver = ViscousSolver::new(&paneled, &options)?;
    let sweep = |step: f64, limit: f64| -> Vec<SectionPoint> {
        let mut session = solver.session();
        let mut out: Vec<SectionPoint> = Vec::new();
        let mut stop = SweepStop::new(step);
        let n = (limit.abs() / step.abs() + 1e-9).floor() as usize;
        for i in 0..=n {
            let alpha = step * i as f64;
            let done = match session.solve(alpha) {
                Ok(s) if s.converged => {
                    out.push(SectionPoint {
                        alpha_deg: alpha,
                        cl: s.cl,
                        cd: s.cd,
                        cm: s.cm,
                    });
                    stop.converged(s.cl)
                }
                _ => stop.failed(),
            };
            if done {
                break;
            }
        }
        out
    };
    let up = sweep(settings.alpha_step, settings.alpha_max);
    let mut down = sweep(-settings.alpha_step, -settings.alpha_min);
    down.retain(|p| p.alpha_deg < 0.0);
    down.reverse();
    Ok(SectionPolar {
        reynolds,
        points: down.into_iter().chain(up).collect(),
    })
}

/// When a polar sweep stops: after four unconverged points in a row, or
/// past the stall — once the lift has fallen 0.1 below its extreme on two
/// converged points (in the direction of the sweep).
struct SweepStop {
    sign: f64,
    failures: u32,
    past: u32,
    extreme: f64,
}

impl SweepStop {
    fn new(step: f64) -> Self {
        Self {
            sign: step.signum(),
            failures: 0,
            past: 0,
            extreme: f64::NEG_INFINITY,
        }
    }

    /// A converged point of lift `cl`; true to stop.
    fn converged(&mut self, cl: f64) -> bool {
        self.failures = 0;
        let lift = cl * self.sign;
        if lift < self.extreme - 0.1 {
            self.past += 1;
        }
        self.extreme = self.extreme.max(lift);
        self.past == 2
    }

    /// An unconverged point; true to stop.
    fn failed(&mut self) -> bool {
        self.failures += 1;
        self.failures == 4
    }
}

/// A [`PolarSource`] keeping every polar it computes.
pub struct PolarCache<F: Fn(&str) -> Option<Airfoil>> {
    airfoil: F,
    settings: AnalysisSettings,
    polars: HashMap<(String, u64, u64), SectionPolar>,
}

impl<F: Fn(&str) -> Option<Airfoil>> PolarCache<F> {
    /// A cache resolving airfoil keys with `airfoil` and analysing them
    /// with `settings`.
    pub fn new(airfoil: F, settings: AnalysisSettings) -> Self {
        Self {
            airfoil,
            settings,
            polars: HashMap::new(),
        }
    }

    /// Number of polars kept.
    pub fn len(&self) -> usize {
        self.polars.len()
    }

    /// No polar kept yet.
    pub fn is_empty(&self) -> bool {
        self.polars.is_empty()
    }
}

impl<F: Fn(&str) -> Option<Airfoil>> PolarSource for PolarCache<F> {
    fn polar(&mut self, key: &str, reynolds: f64, mach: f64) -> Result<SectionPolar> {
        let id = (key.to_owned(), reynolds.to_bits(), mach.to_bits());
        if let Some(p) = self.polars.get(&id) {
            return Ok(p.clone());
        }
        let airfoil = (self.airfoil)(key)
            .ok_or_else(|| MissionError::InvalidInput(format!("unknown airfoil {key}")))?;
        let p = section_polar(&airfoil, reynolds, mach, &self.settings)?;
        self.polars.insert(id, p.clone());
        Ok(p)
    }
}

/// The trimmed flight.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrimResult {
    /// Angle of attack of the fuselage axis, degrees.
    pub alpha_deg: f64,
    /// Elevator (or all-moving tail, canard, elevons) deflection, degrees,
    /// positive trailing edge down.
    pub elevator_deg: f64,
    /// Lift coefficient (the required one when trimmed).
    pub cl: f64,
    /// Pitching moment left (zero when trimmed).
    pub cm: f64,
    /// The aircraft could be trimmed within ±20° of angle of attack and
    /// ±[`MAX_ELEVATOR`] of elevator.
    pub trimmed: bool,
}

/// Static stability at the trimmed flight.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StabilityResult {
    /// Neutral point: distance behind the nose, m.
    pub neutral_point: f64,
    /// Centre of gravity: distance behind the nose, m.
    pub center_of_gravity: f64,
    /// Static margin, (neutral point − centre of gravity) / mean
    /// aerodynamic chord: positive is stable (5 to 15 % is usual).
    pub static_margin: f64,
    /// Lift slope of the aircraft, per radian.
    pub cl_alpha: f64,
    /// Pitching moment slope, per radian (negative is stable).
    pub cm_alpha: f64,
    /// Weathercock stability Cnβ, per radian (positive is stable).
    pub cn_beta: f64,
    /// Dihedral effect Clβ, per radian (negative is stable).
    pub cl_beta: f64,
    /// Spiral stability `Clβ Cnr / (Clr Cnβ)`, above 1 when the aircraft
    /// comes back to level flight by itself.
    pub spiral: Option<f64>,
}

/// Drag of the aircraft in the trimmed flight.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DragBreakdown {
    /// Induced drag coefficient (Trefftz plane).
    pub induced: f64,
    /// Profile drag of the wing (and canard).
    pub wing_profile: f64,
    /// Profile drag of the tail and fin.
    pub tail_profile: f64,
    /// Skin friction of the fuselage.
    pub fuselage: f64,
    /// Total drag coefficient.
    pub total: f64,
    /// Span efficiency of the lift distribution.
    pub span_efficiency: f64,
    /// Glide ratio, lift / drag.
    pub lift_to_drag: f64,
    /// Drag, N.
    pub drag: f64,
    /// Power needed to fly, drag × speed, W (before propeller and motor
    /// losses).
    pub power: f64,
}

/// The stall of the trimmed aircraft.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StallResult {
    /// Maximum trimmed lift coefficient of the aircraft.
    pub cl_max: f64,
    /// Stall speed in level flight, m/s.
    pub speed: f64,
    /// Where the stall starts.
    pub part: SurfaceKind,
    /// How far out along the half span it starts, 0 at the root, 1 at the
    /// tip.
    pub span_fraction: f64,
}

/// One strip of the lattice in the trimmed flight.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StripResult {
    /// Part it belongs to.
    pub part: SurfaceKind,
    /// The left half.
    pub left: bool,
    /// Distance from the root along the span (y, or the height of a fin),
    /// m.
    pub span_position: f64,
    /// Fraction of the half span, 0 at the root, 1 at the tip.
    pub span_fraction: f64,
    /// Leading edge at the middle of the strip, m.
    pub leading_edge: [f64; 3],
    /// Chord, m.
    pub chord: f64,
    /// Area, m².
    pub area: f64,
    /// Reynolds number.
    pub reynolds: f64,
    /// Lift coefficient referred to the velocity normal to the sweep.
    pub cl: f64,
    /// Maximum lift coefficient of the airfoil at this Reynolds number.
    pub cl_max: Option<f64>,
    /// Lowest lift coefficient of the airfoil (the negative stall).
    pub cl_min: Option<f64>,
    /// Profile drag coefficient from the polar.
    pub cd: Option<f64>,
    /// Corners of the strip, m: leading and trailing edge at one end, then
    /// trailing and leading edge at the other (for drawing).
    pub corners: [[f64; 3]; 4],
}

/// Something the user should know about the analysis.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisWarning {
    /// The aircraft cannot be trimmed with a reasonable elevator: the
    /// centre of gravity is too far forward or back, or the tail too small.
    CannotTrim,
    /// Part of the wing flies beyond the maximum lift of its airfoil: it is
    /// stalled at this speed.
    WingStalled,
    /// Part of the tail (or canard) flies beyond its airfoil's range.
    TailStalled,
    /// Some strips are outside the polars computed (no data).
    MissingPolars,
}

/// How the aircraft flies in a level flight.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AircraftAnalysis {
    /// Lift coefficient needed for level flight.
    pub required_cl: f64,
    /// Dynamic pressure, Pa.
    pub dynamic_pressure: f64,
    /// Reynolds number on the wing's mean aerodynamic chord.
    pub reynolds: f64,
    /// Trim.
    pub trim: TrimResult,
    /// Static stability.
    pub stability: StabilityResult,
    /// Drag.
    pub drag: DragBreakdown,
    /// Stall.
    pub stall: Option<StallResult>,
    /// Every strip of the lattice, in its order.
    pub strips: Vec<StripResult>,
    /// What to look at.
    pub warnings: Vec<AnalysisWarning>,
    /// Number of horseshoe vortices.
    pub vortices: u32,
}

/// The Reynolds number of the grid just at or below `re`.
fn grid_below(re: f64) -> f64 {
    10f64.powf((re.log10() * REYNOLDS_STEPS_PER_DECADE + 1e-9).floor() / REYNOLDS_STEPS_PER_DECADE)
}

/// The next grid Reynolds number above `re` on the grid.
fn grid_next(re: f64) -> f64 {
    10f64.powf(((re.log10() * REYNOLDS_STEPS_PER_DECADE).round() + 1.0) / REYNOLDS_STEPS_PER_DECADE)
}

/// The polars of one airfoil, by Reynolds number on the grid.
struct AirfoilPolars {
    by_re: BTreeMap<u64, SectionPolar>,
}

impl AirfoilPolars {
    /// `f` of the polars on both sides of `re`, interpolated in ln Re
    /// (clamped beyond the ends).
    fn interpolate(&self, re: f64, f: impl Fn(&SectionPolar) -> Option<f64>) -> Option<f64> {
        let polars: Vec<&SectionPolar> = self.by_re.values().collect();
        let first = polars.first()?;
        if re <= first.reynolds || polars.len() == 1 {
            return f(first);
        }
        for w in polars.windows(2) {
            let (a, b) = (w[0], w[1]);
            if re <= b.reynolds {
                let t = (re / a.reynolds).ln() / (b.reynolds / a.reynolds).ln();
                return match (f(a), f(b)) {
                    (Some(x), Some(y)) => Some(x + t * (y - x)),
                    (x, y) => x.or(y),
                };
            }
        }
        polars.last().and_then(|p| f(p))
    }
}

/// Solves the lattice at `alpha_deg` and `elevator_deg`.
fn solve_at(vl: &VortexLattice, alpha_deg: f64, elevator_deg: f64) -> Result<Solution> {
    Ok(vl.solve(&FlowCondition {
        controls: vec![elevator_deg],
        ..FlowCondition::alpha(alpha_deg)
    })?)
}

/// Trims the lattice for the lift coefficient `cl` with the moment
/// balanced (Newton on the angle of attack and the elevator).
fn trim(vl: &VortexLattice, cl: f64) -> Result<(TrimResult, Solution)> {
    let deg = PI / 180.0;
    let (mut alpha, mut elevator) = (0.0, 0.0);
    let mut s = solve_at(vl, alpha, elevator)?;
    for _ in 0..20 {
        let d = &s.derivatives;
        let e = &d.controls[0].forces;
        let (r1, r2) = (s.totals.cl - cl, s.totals.pitch);
        let (a11, a12, a21, a22) = (d.alpha.cl * deg, e.cl, d.alpha.pitch * deg, e.pitch);
        let det = a11 * a22 - a12 * a21;
        if det == 0.0 || !det.is_finite() {
            break;
        }
        let da = -(r1 * a22 - a12 * r2) / det;
        let de = -(a11 * r2 - a21 * r1) / det;
        alpha += da;
        elevator += de;
        if alpha.abs() > 30.0 || elevator.abs() > 45.0 {
            break;
        }
        s = solve_at(vl, alpha, elevator)?;
        if da.abs() < 1e-7 && de.abs() < 1e-7 {
            break;
        }
    }
    let converged = (s.totals.cl - cl).abs() < 1e-6 && s.totals.pitch.abs() < 1e-6;
    let trimmed = converged && alpha.abs() <= 20.0 && elevator.abs() <= MAX_ELEVATOR;
    Ok((
        TrimResult {
            alpha_deg: alpha,
            elevator_deg: elevator,
            cl: s.totals.cl,
            cm: s.totals.pitch,
            trimmed,
        },
        s,
    ))
}

/// Wetted area of the fuselage, m² (∫ 2π r √(1 + r'²) dx).
fn wetted_area(f: &Fuselage) -> f64 {
    let n = 400;
    let dx = f.length / n as f64;
    (0..n)
        .map(|i| {
            let (x0, x1) = (i as f64 * dx, (i + 1) as f64 * dx);
            let (r0, r1) = (f.radius(x0), f.radius(x1));
            PI * (r0 + r1) * (dx * dx + (r1 - r0) * (r1 - r0)).sqrt()
        })
        .sum()
}

/// Fuselage skin friction drag coefficient referred to `area`: turbulent
/// flat plate (Prandtl–Schlichting, 0.455 / (log Re)^2.58) at the
/// fuselage's Reynolds number, form factor 1 + 60/(l/d)³ + (l/d)/400
/// (Raymer), on the wetted area.
fn fuselage_drag(f: &Fuselage, speed: f64, nu: f64, area: f64) -> f64 {
    let re = speed * f.length / nu;
    let cf = 0.455 / re.log10().powf(2.58);
    let fineness = f.length / f.diameter;
    let form = 1.0 + 60.0 / fineness.powi(3) + fineness / 400.0;
    cf * form * wetted_area(f) / area
}

/// How `aircraft` flies in `flight`. `airfoil` resolves the section keys
/// (for the camber lines); `polars` gives the 2D polars.
pub fn analyze(
    aircraft: &Aircraft,
    flight: &Flight,
    airfoil: &dyn Fn(&str) -> Option<Airfoil>,
    polars: &mut dyn PolarSource,
    options: &LatticeOptions,
) -> Result<AircraftAnalysis> {
    PreparedAnalysis::new(aircraft, flight, airfoil, options)?.run(polars)
}

/// A polar an analysis needs.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolarRequest {
    /// Airfoil key.
    pub key: String,
    /// Reynolds number, on the grid.
    pub reynolds: f64,
    /// Mach number, rounded down to [`POLAR_MACH_STEP`].
    pub mach: f64,
}

/// A strip placed between the sections of its panel.
struct Place {
    /// Airfoils of the panel's two sections.
    keys: [String; 2],
    /// Fraction of the way from the first to the second.
    fraction: f64,
    /// Reynolds number of the flight.
    reynolds: f64,
}

/// An analysis ready to run: the lattice built and factored, the strips
/// placed, and the polars it will need known — so that a caller can
/// compute them first, in parallel.
pub struct PreparedAnalysis {
    flight: Flight,
    density: f64,
    nu: f64,
    mach: f64,
    layout: aero_aircraft::Layout,
    fuselage: Option<Fuselage>,
    lat: AircraftLattice,
    vl: VortexLattice,
    places: Vec<Place>,
    corners: Vec<[[f64; 3]; 4]>,
    /// Lowest and highest Reynolds number of each airfoil's strips.
    ranges: BTreeMap<String, (f64, f64)>,
}

impl PreparedAnalysis {
    /// Builds and factors the lattice of `aircraft` for `flight`.
    pub fn new(
        aircraft: &Aircraft,
        flight: &Flight,
        airfoil: &dyn Fn(&str) -> Option<Airfoil>,
        options: &LatticeOptions,
    ) -> Result<Self> {
        let atmosphere = flight.atmosphere()?;
        let speed = flight.speed;
        let nu = atmosphere.kinematic_viscosity;
        let mach = speed / atmosphere.speed_of_sound;
        let layout = layout(aircraft)?;
        let lat: AircraftLattice = lattice(
            aircraft,
            airfoil,
            &LatticeOptions {
                mach: mach.min(0.85),
                ..*options
            },
        )?;
        let vl = VortexLattice::new(&lat.model)?;
        debug_assert_eq!(vl.control_names(), [ELEVATOR.to_string()]);

        // The strips' geometry (any flow gives it).
        let s0 = vl.solve(&FlowCondition::alpha(0.0))?;
        let mut places = Vec::with_capacity(lat.strips.len());
        for (origin, strip) in lat.strips.iter().zip(&s0.strips) {
            let surface = lat
                .surfaces
                .iter()
                .find(|sf| sf.kind == origin.kind)
                .ok_or_else(|| MissionError::InvalidInput("strip without a surface".into()))?;
            places.push(Place {
                keys: [
                    surface.sections[origin.panel].airfoil.clone(),
                    surface.sections[origin.panel + 1].airfoil.clone(),
                ],
                fraction: origin.fraction(strip.leading_edge),
                reynolds: speed * strip.chord / nu,
            });
        }
        let mut ranges: BTreeMap<String, (f64, f64)> = BTreeMap::new();
        for p in &places {
            for key in &p.keys {
                let r = ranges.entry(key.clone()).or_insert((f64::INFINITY, 0.0));
                r.0 = r.0.min(p.reynolds);
                r.1 = r.1.max(p.reynolds);
            }
        }
        let corners = vl.strip_corners();
        Ok(Self {
            flight: *flight,
            density: atmosphere.density,
            nu,
            mach,
            layout,
            fuselage: aircraft.fuselage,
            lat,
            vl,
            places,
            corners,
            ranges,
        })
    }

    /// Number of horseshoe vortices.
    pub fn vortex_count(&self) -> usize {
        self.vl.vortex_count()
    }

    /// The polars [`run`](Self::run) asks for: for each airfoil, Reynolds
    /// numbers on the grid from half its strips' lowest (for the stall
    /// speed) to just above their highest.
    pub fn polar_requests(&self) -> Vec<PolarRequest> {
        let mach = (self.mach / POLAR_MACH_STEP + 1e-9).floor() * POLAR_MACH_STEP;
        let mut out = Vec::new();
        for (key, (lo, hi)) in &self.ranges {
            let mut re = grid_below(0.5 * lo);
            loop {
                out.push(PolarRequest {
                    key: key.clone(),
                    reynolds: re,
                    mach,
                });
                if re >= *hi {
                    break;
                }
                re = grid_next(re);
            }
        }
        out
    }

    /// Trims the aircraft and reads its strips against the polars.
    pub fn run(&self, polars: &mut dyn PolarSource) -> Result<AircraftAnalysis> {
        let flight = &self.flight;
        let speed = flight.speed;
        let nu = self.nu;
        let q = 0.5 * self.density * speed * speed;
        let l = &self.layout;
        let lat = &self.lat;
        let vl = &self.vl;
        let places = &self.places;
        let sref = lat.model.reference.area;
        let required_cl = flight.weight() / (q * sref);

        let (trim_result, s) = trim(vl, required_cl)?;
        let mut warnings = Vec::new();
        if !trim_result.trimmed {
            warnings.push(AnalysisWarning::CannotTrim);
        }

        let d = &s.derivatives;
        let mac = l.wing.mac;
        let neutral_point = d.neutral_point.unwrap_or(f64::NAN);
        let stability = StabilityResult {
            neutral_point,
            center_of_gravity: l.cg_x,
            static_margin: (neutral_point - l.cg_x) / mac,
            cl_alpha: d.alpha.cl,
            cm_alpha: d.alpha.pitch,
            cn_beta: d.beta.yaw,
            cl_beta: d.beta.roll,
            spiral: d.spiral_stability,
        };

        // ---- the polars of every airfoil
        let mut tables: HashMap<String, AirfoilPolars> = HashMap::new();
        for r in self.polar_requests() {
            let polar = polars.polar(&r.key, r.reynolds, r.mach)?;
            tables
                .entry(r.key)
                .or_insert_with(|| AirfoilPolars {
                    by_re: BTreeMap::new(),
                })
                .by_re
                .insert(r.reynolds.to_bits(), polar);
        }
        let blend = |p: &Place, re: f64, f: &dyn Fn(&SectionPolar) -> Option<f64>| -> Option<f64> {
            let a = tables.get(&p.keys[0])?.interpolate(re, f);
            let b = tables.get(&p.keys[1])?.interpolate(re, f);
            Some((1.0 - p.fraction) * a? + p.fraction * b?)
        };

        let mut strips = Vec::with_capacity(places.len());
        let (mut wing_profile, mut tail_profile) = (0.0, 0.0);
        for (k, ((origin, strip), place)) in
            lat.strips.iter().zip(&s.strips).zip(places).enumerate()
        {
            let cl = strip.cl_perp;
            let cl_max = blend(place, place.reynolds, &|p: &SectionPolar| p.cl_max());
            let cl_min = blend(place, place.reynolds, &|p: &SectionPolar| p.cl_min());
            let cd = blend(place, place.reynolds, &|p: &SectionPolar| p.cd_at(strip.cl));
            let lifting = matches!(origin.kind, SurfaceKind::Wing | SurfaceKind::Canard);
            match cd {
                Some(cd) => {
                    let c = cd * strip.area / sref;
                    if lifting {
                        wing_profile += c
                    } else {
                        tail_profile += c
                    }
                }
                None => {
                    if !warnings.contains(&AnalysisWarning::MissingPolars) {
                        warnings.push(AnalysisWarning::MissingPolars);
                    }
                }
            }
            let beyond = cl_max.is_some_and(|m| cl > m) || cl_min.is_some_and(|m| cl < m);
            let flag = if origin.kind == SurfaceKind::Wing {
                AnalysisWarning::WingStalled
            } else {
                AnalysisWarning::TailStalled
            };
            if beyond && !warnings.contains(&flag) {
                warnings.push(flag);
            }
            let surface = lat.surfaces.iter().find(|sf| sf.kind == origin.kind);
            let span_of = |le: [f64; 3]| match origin.kind {
                SurfaceKind::Fin => le[2],
                _ => le[1].abs(),
            };
            let (root, tip) = surface.map_or((0.0, 1.0), |sf| {
                let first = sf.sections.first().map_or(0.0, |c| span_of(c.le));
                let last = sf.sections.last().map_or(1.0, |c| span_of(c.le));
                (first, last)
            });
            let position = span_of(strip.leading_edge);
            strips.push(StripResult {
                part: origin.kind,
                left: origin.left,
                span_position: position - root,
                span_fraction: if tip > root {
                    (position - root) / (tip - root)
                } else {
                    0.0
                },
                leading_edge: strip.leading_edge,
                chord: strip.chord,
                area: strip.area,
                reynolds: place.reynolds,
                cl,
                cl_max,
                cl_min,
                cd,
                corners: self.corners[k],
            });
        }

        let fuselage = self
            .fuselage
            .map_or(0.0, |f| fuselage_drag(&f, speed, nu, sref));
        let induced = s.totals.cd_trefftz;
        let total = induced + wing_profile + tail_profile + fuselage;
        let drag = DragBreakdown {
            induced,
            wing_profile,
            tail_profile,
            fuselage,
            total,
            span_efficiency: s.totals.span_efficiency,
            lift_to_drag: s.totals.cl / total,
            drag: q * sref * total,
            power: q * sref * total * speed,
        };

        // ---- stall (critical-section method): the trimmed lift coefficient at
        // which the most loaded wing or canard strip reaches the maximum lift
        // of its airfoil, at the Reynolds numbers of the speed of that flight.
        let weight = flight.weight();
        let rho = self.density;
        let loading = |cl: f64| -> Result<Option<(f64, usize)>> {
            let (t, sol) = trim(vl, cl)?;
            if (t.cl - cl).abs() > 1e-6 || t.cm.abs() > 1e-6 {
                return Ok(None);
            }
            let v = (2.0 * weight / (rho * sref * cl)).sqrt();
            let mut worst: Option<(f64, usize)> = None;
            for (k, (origin, place)) in lat.strips.iter().zip(places).enumerate() {
                if !matches!(origin.kind, SurfaceKind::Wing | SurfaceKind::Canard) {
                    continue;
                }
                let c = sol.strips[k].cl_perp;
                let re = place.reynolds * v / speed;
                let Some(m) = blend(place, re, &|p: &SectionPolar| p.cl_max()) else {
                    continue;
                };
                if c > 0.0 && m > 0.0 && worst.is_none_or(|(r, _)| c / m > r) {
                    worst = Some((c / m, k));
                }
            }
            Ok(worst)
        };
        let stall = if trim_result.trimmed {
            stall_search(required_cl, &loading)?.map(|(cl_max, k)| StallResult {
                cl_max,
                speed: (2.0 * weight / (rho * sref * cl_max)).sqrt(),
                part: lat.strips[k].kind,
                span_fraction: strips[k].span_fraction,
            })
        } else {
            None
        };

        Ok(AircraftAnalysis {
            required_cl,
            dynamic_pressure: q,
            reynolds: speed * mac / nu,
            trim: trim_result,
            stability,
            drag,
            stall,
            strips,
            warnings,
            vortices: vl.vortex_count() as u32,
        })
    }
}

/// The lift coefficient where `loading(cl)` — the largest ratio of a
/// strip's lift to its maximum, and that strip — reaches 1: bracketed from
/// `cl0`, then regula falsi (Illinois). `None` when no bracket is found
/// (the aircraft cannot be trimmed there, or never stalls below CL = 4).
fn stall_search(
    cl0: f64,
    loading: &dyn Fn(f64) -> Result<Option<(f64, usize)>>,
) -> Result<Option<(f64, usize)>> {
    let Some((r0, k0)) = loading(cl0)? else {
        return Ok(None);
    };
    // Bracket: a lift coefficient below the stall and one above, stepping
    // by the loading ratio (lift scales roughly with it).
    let f0 = r0 - 1.0;
    let (mut lo, mut hi) = ((cl0, f0, k0), (cl0, f0, k0));
    let below = f0 < 0.0;
    let mut current = (cl0, f0, k0);
    let mut found = false;
    for _ in 0..40 {
        let factor = 1.0 / (current.1 + 1.0).max(0.05);
        let cl = if below {
            current.0 * factor.clamp(1.1, 1.5)
        } else {
            current.0 * factor.clamp(0.6, 0.9)
        };
        if !(0.02..=4.0).contains(&cl) {
            return Ok(None);
        }
        let Some((r, k)) = loading(cl)? else {
            return Ok(None);
        };
        current = (cl, r - 1.0, k);
        if below && current.1 >= 0.0 {
            hi = current;
            found = true;
            break;
        }
        if !below && current.1 < 0.0 {
            lo = current;
            found = true;
            break;
        }
        if below {
            lo = current;
        } else {
            hi = current;
        }
    }
    if !found {
        return Ok(None);
    }
    // Illinois.
    let mut side = 0;
    let mut best = hi;
    for _ in 0..60 {
        let c = (lo.0 * hi.1 - hi.0 * lo.1) / (hi.1 - lo.1);
        let Some((r, k)) = loading(c)? else {
            return Ok(None);
        };
        let f = r - 1.0;
        best = (c, f, k);
        if f.abs() < 1e-6 || (hi.0 - lo.0).abs() < 1e-9 {
            break;
        }
        if f < 0.0 {
            lo = (c, f, k);
            if side == -1 {
                hi.1 /= 2.0;
            }
            side = -1;
        } else {
            hi = (c, f, k);
            if side == 1 {
                lo.1 /= 2.0;
            }
            side = 1;
        }
    }
    if best.1.abs() > 1e-6 {
        // Not converged (a jump of the maximum lift with the Reynolds
        // number): keep the highest lift known to be below the stall.
        best = lo;
    }
    Ok(Some((best.0, best.2)))
}

#[cfg(test)]
mod tests {
    use aero_aircraft::{MeshOptions, PartKind, mesh};

    use super::*;

    fn point(alpha_deg: f64, cl: f64, cd: f64) -> SectionPoint {
        SectionPoint {
            alpha_deg,
            cl,
            cd,
            cm: 0.0,
        }
    }

    /// A polar with a negative stall at −8°, a positive one at 12° and
    /// post-stall points on both sides.
    fn polar(reynolds: f64, scale: f64) -> SectionPolar {
        SectionPolar {
            reynolds,
            points: vec![
                point(-10.0, -0.5, 0.10),
                point(-8.0, -0.6, 0.05),
                point(0.0, 0.2, 0.010 * scale),
                point(4.0, 0.6, 0.012 * scale),
                point(12.0, 1.2, 0.040),
                point(14.0, 1.0, 0.12),
            ],
        }
    }

    #[test]
    fn sweeps_stop_past_the_stall_or_after_four_failures() {
        // Upward: the lift peaks at 1.2, then falls by more than 0.1 twice.
        let mut up = SweepStop::new(0.5);
        let stops: Vec<bool> = [0.2, 0.8, 1.2, 1.15, 1.05, 0.9]
            .iter()
            .map(|&cl| up.converged(cl))
            .collect();
        assert_eq!(stops, [false, false, false, false, false, true]);
        // Downward: the same with negative lift.
        let mut down = SweepStop::new(-0.5);
        let stops: Vec<bool> = [-0.2, -0.9, -0.75, -0.7]
            .iter()
            .map(|&cl| down.converged(cl))
            .collect();
        assert_eq!(stops, [false, false, false, true]);
        // Four failures in a row, a converged point resetting the count.
        let mut f = SweepStop::new(1.0);
        assert!(!f.failed() && !f.failed() && !f.failed());
        assert!(!f.converged(0.5));
        assert!(!f.failed() && !f.failed() && !f.failed());
        assert!(f.failed());
    }

    #[test]
    fn polars_are_read_on_their_attached_branch() {
        let p = polar(1e5, 1.0);
        assert_eq!(p.cl_max(), Some(1.2));
        assert_eq!(p.cl_min(), Some(-0.6));
        assert!((p.cd_at(0.4).unwrap() - 0.011).abs() < 1e-15);
        assert!((p.cd_at(-0.2).unwrap() - 0.030).abs() < 1e-15);
        // Beyond the stalls: the end values, not the post-stall points.
        assert_eq!(p.cd_at(1.5), Some(0.040));
        assert_eq!(p.cd_at(-0.9), Some(0.05));
        assert_eq!(
            SectionPolar {
                reynolds: 1e5,
                points: Vec::new()
            }
            .cl_max(),
            None
        );
    }

    #[test]
    fn polars_are_interpolated_in_the_logarithm_of_the_reynolds_number() {
        let mut by_re = BTreeMap::new();
        for (re, scale) in [(1e5, 1.0), (2e5, 0.5)] {
            by_re.insert(f64::to_bits(re), polar(re, scale));
        }
        let t = AirfoilPolars { by_re };
        let cd = |re: f64| t.interpolate(re, |p| p.cd_at(0.2)).unwrap();
        assert!((cd(2f64.sqrt() * 1e5) - 0.0075).abs() < 1e-15);
        assert!((cd(1e5) - 0.010).abs() < 1e-15);
        // Clamped beyond the polars.
        assert!((cd(5e4) - 0.010).abs() < 1e-15);
        assert!((cd(4e5) - 0.005).abs() < 1e-15);
    }

    #[test]
    fn reynolds_grid() {
        assert!((grid_below(1.0e5) - 1.0e5).abs() < 1e-6);
        assert!((grid_below(1.5e5) - 1.0e5).abs() < 1e-6);
        assert!((grid_below(1.6e5) - 10f64.powf(5.2)).abs() < 1e-6);
        assert!((grid_next(1.0e5) - 10f64.powf(5.2)).abs() < 1e-6);
        assert_eq!(grid_below(1.4e5).to_bits(), grid_below(1.5e5).to_bits());
    }

    #[test]
    fn fuselage_wetted_area_and_drag() {
        let f = Fuselage {
            length: 1.0,
            diameter: 0.1,
        };
        // Against the area of a fine mesh of the same fuselage.
        let a = aero_aircraft::Preset::Trainer.aircraft();
        let a = Aircraft {
            fuselage: Some(f),
            ..a
        };
        let any =
            |_: &str| aero_geometry::naca("0012", &aero_geometry::NacaOptions::default()).ok();
        let parts = mesh(
            &a,
            &any,
            &MeshOptions {
                chordwise: 8,
                around: 256,
                along: 800,
            },
        )
        .unwrap();
        let part = parts.iter().find(|p| p.kind == PartKind::Fuselage).unwrap();
        let v = |i: u32| {
            let k = 3 * i as usize;
            [
                part.positions[k] as f64,
                part.positions[k + 1] as f64,
                part.positions[k + 2] as f64,
            ]
        };
        let area: f64 = part
            .indices
            .chunks(3)
            .map(|t| {
                let (a, b, c) = (v(t[0]), v(t[1]), v(t[2]));
                let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                let w = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                let n = [
                    u[1] * w[2] - u[2] * w[1],
                    u[2] * w[0] - u[0] * w[2],
                    u[0] * w[1] - u[1] * w[0],
                ];
                0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt()
            })
            .sum();
        assert!(
            (wetted_area(&f) / area - 1.0).abs() < 2e-3,
            "{} vs {area}",
            wetted_area(&f)
        );
    }

    #[test]
    fn fuselage_drag_formula() {
        let f = Fuselage {
            length: 1.0,
            diameter: 0.1,
        };
        // Re = 1e6: Cf = 0.455 / 6^2.58; fineness 10: form factor 1.085.
        let cd = fuselage_drag(&f, 15.0, 1.5e-5, 0.3);
        let expected = 0.455 / 6f64.powf(2.58) * 1.085 * wetted_area(&f) / 0.3;
        assert!((cd - expected).abs() < 1e-15, "{cd} vs {expected}");
        assert!((0.455 / 6f64.powf(2.58) - 0.00447).abs() < 1e-5);
    }
}
