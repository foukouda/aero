//! The configuration of a vortex lattice: lifting surfaces described by
//! sections, as in an AVL geometry file.
//!
//! Axes as in AVL: x downstream (from the nose to the tail), y towards the
//! right wing tip, z up. Lengths in any consistent unit; angles in degrees.

use serde::{Deserialize, Serialize};

use crate::camber::Camber;
use crate::error::{Result, invalid};

/// Largest number of chordwise vortices per strip (AVL `KCMAX`).
pub const MAX_CHORDWISE: usize = 50;

/// A number of vortices and how they are spaced: the parameter blends
/// equal (0), cosine (1) and sine (2) spacings, from −3 to 3; a negative
/// sine spacing is fine at the end instead of the start (AVL `Nchord
/// Cspace`, `Nspan Sspace`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Spacing {
    /// Number of vortices.
    pub count: usize,
    /// Spacing parameter, from −3 to 3.
    pub parameter: f64,
}

impl Spacing {
    /// `count` vortices spaced by `parameter`.
    pub fn new(count: usize, parameter: f64) -> Self {
        Self { count, parameter }
    }

    fn validate(&self, what: &str, max: usize) -> Result<()> {
        if self.count == 0 || self.count > max {
            return Err(invalid(format!(
                "{what}: the number of vortices must be from 1 to {max}, got {}",
                self.count
            )));
        }
        if !(-3.0..=3.0).contains(&self.parameter) {
            return Err(invalid(format!(
                "{what}: the spacing parameter must be from -3 to 3, got {}",
                self.parameter
            )));
        }
        Ok(())
    }
}

/// A control surface declared on a section (AVL `CONTROL`). A control acts
/// on the panel between two consecutive sections that both declare it; its
/// deflection is one variable of the flow, in degrees.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlSurface {
    /// Name of the control variable; sections sharing a name move together.
    pub name: String,
    /// Deflection of this surface per degree of the control variable.
    pub gain: f64,
    /// Hinge position as a fraction of the chord; the surface lies behind
    /// it (a trailing-edge flap). A negative value `−h` makes a
    /// leading-edge surface ahead of `h`.
    pub hinge: f64,
    /// Hinge axis; zero for the hinge line itself.
    pub axis: [f64; 3],
    /// Sign of the deflection on the mirrored half (`YDUPLICATE`): +1 for
    /// an elevator, −1 for an aileron.
    pub duplicate_sign: f64,
}

impl ControlSurface {
    /// A trailing-edge control hinged at `hinge` (fraction of the chord),
    /// with unit gain, about its hinge line, deflected alike on both halves.
    pub fn new(name: impl Into<String>, hinge: f64) -> Self {
        Self {
            name: name.into(),
            gain: 1.0,
            hinge,
            axis: [0.0; 3],
            duplicate_sign: 1.0,
        }
    }
}

/// A section of a lifting surface (AVL `SECTION`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SurfaceSection {
    /// Leading edge `(x, y, z)`.
    pub leading_edge: [f64; 3],
    /// Chord.
    pub chord: f64,
    /// Incidence (twist) in degrees, positive nose up; it rotates the
    /// camber line in the boundary conditions only, not the geometry.
    pub incidence_deg: f64,
    /// Spanwise vortices from this section to the next, when the surface
    /// does not give one spacing for its whole span.
    pub spanwise: Option<Spacing>,
    /// Camber line of the airfoil.
    pub camber: Camber,
    /// Lift slope of the airfoil divided by 2π (AVL `CLAF`, from 0 to 2;
    /// 1 for a thin airfoil, about 1 + 0.77 t/c for a thickness t/c).
    pub lift_slope_factor: f64,
    /// Control surfaces declared on this section.
    pub controls: Vec<ControlSurface>,
}

impl SurfaceSection {
    /// A flat section with no incidence and no control.
    pub fn new(leading_edge: [f64; 3], chord: f64) -> Self {
        Self {
            leading_edge,
            chord,
            incidence_deg: 0.0,
            spanwise: None,
            camber: Camber::flat(),
            lift_slope_factor: 1.0,
            controls: Vec::new(),
        }
    }
}

/// A lifting surface (AVL `SURFACE`): sections from root to tip (or left
/// to right), with its lattice and placement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiftingSurface {
    /// Name.
    pub name: String,
    /// Chordwise vortices of every strip.
    pub chordwise: Spacing,
    /// Spanwise vortices over the whole surface; `None` uses the spacing of
    /// each section.
    pub spanwise: Option<Spacing>,
    /// Component index: surfaces of one component (a wing and its strake)
    /// interact with thin vortex cores, other components with thicker ones.
    /// `None` gives the surface its own component.
    pub component: Option<usize>,
    /// Mirror the surface about the plane `y = value` (AVL `YDUPLICATE`).
    pub y_duplicate: Option<f64>,
    /// Scale factors of the section coordinates (the chord scales as x).
    pub scale: [f64; 3],
    /// Translation added to the scaled section coordinates.
    pub translate: [f64; 3],
    /// Incidence added to every section, in degrees.
    pub angle_deg: f64,
    /// The surface sheds a wake (false for AVL `NOWAKE`: the circulation of
    /// each strip then sums to zero).
    pub wake: bool,
    /// The surface counts in the total forces (false for AVL `NOLOAD`).
    pub load: bool,
    /// Sections, at least two.
    pub sections: Vec<SurfaceSection>,
}

impl LiftingSurface {
    /// A surface with defaults: no mirror image, unit scale, no offset,
    /// its own component, a wake and counted in the loads.
    pub fn new(name: impl Into<String>, chordwise: Spacing, sections: Vec<SurfaceSection>) -> Self {
        Self {
            name: name.into(),
            chordwise,
            spanwise: None,
            component: None,
            y_duplicate: None,
            scale: [1.0; 3],
            translate: [0.0; 3],
            angle_deg: 0.0,
            wake: true,
            load: true,
            sections,
        }
    }
}

/// Reference quantities of the coefficients and the moment reference point
/// (AVL `Sref Cref Bref`, `Xref Yref Zref`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Reference {
    /// Reference area (the whole wing, both halves).
    pub area: f64,
    /// Reference chord, for the pitching moment.
    pub chord: f64,
    /// Reference span, for the rolling and yawing moments.
    pub span: f64,
    /// Moment reference point, usually the centre of gravity.
    pub point: [f64; 3],
}

/// Sizes of the vortex cores between surfaces of different components, as
/// fractions of the strip chord and width (AVL `CORE`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VortexCore {
    /// Fraction of the chord of the influencing strip.
    pub chord_fraction: f64,
    /// Fraction of the width of the influencing strip.
    pub width_fraction: f64,
}

impl Default for VortexCore {
    /// AVL's defaults: 0 and 2.
    fn default() -> Self {
        Self {
            chord_fraction: 0.0,
            width_fraction: 2.0,
        }
    }
}

/// A vortex-lattice configuration, as an AVL geometry file describes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VlmModel {
    /// Title.
    pub title: String,
    /// Mach number (Prandtl–Glauert correction), below 1.
    pub mach: f64,
    /// Only the half `y ≥ 0` is described, mirrored by a symmetry plane at
    /// `y = 0` (AVL `iYsym = 1`); the flow is then symmetric (no sideslip,
    /// roll or yaw).
    pub y_symmetry: bool,
    /// Reference quantities and moment point.
    pub reference: Reference,
    /// Profile drag coefficient added to the total drag (AVL `CDp`).
    pub cd_p: f64,
    /// Vortex cores between components.
    pub core: VortexCore,
    /// Lifting surfaces.
    pub surfaces: Vec<LiftingSurface>,
}

impl VlmModel {
    /// Names of the control variables, in the order they are first
    /// declared (AVL's numbering `D1`, `D2`…).
    pub fn control_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for c in self
            .surfaces
            .iter()
            .flat_map(|s| &s.sections)
            .flat_map(|s| &s.controls)
        {
            if !names.contains(&c.name) {
                names.push(c.name.clone());
            }
        }
        names
    }

    /// Checks the configuration.
    pub fn validate(&self) -> Result<()> {
        if !(0.0..1.0).contains(&self.mach) {
            return Err(invalid(format!(
                "the Mach number must be from 0 to below 1, got {}",
                self.mach
            )));
        }
        let r = &self.reference;
        for (what, v) in [("area", r.area), ("chord", r.chord), ("span", r.span)] {
            if !(v.is_finite() && v > 0.0) {
                return Err(invalid(format!("the reference {what} must be positive")));
            }
        }
        if r.point.iter().any(|v| !v.is_finite()) || !self.cd_p.is_finite() {
            return Err(invalid("the reference point and CDp must be finite"));
        }
        let c = &self.core;
        let fractions = [c.chord_fraction, c.width_fraction];
        if fractions.iter().any(|v| !v.is_finite() || *v < 0.0) {
            return Err(invalid("the vortex core fractions must be positive"));
        }
        if self.surfaces.is_empty() {
            return Err(invalid("there is no lifting surface"));
        }
        for s in &self.surfaces {
            validate_surface(s, self.y_symmetry)?;
        }
        Ok(())
    }
}

fn validate_surface(s: &LiftingSurface, y_symmetry: bool) -> Result<()> {
    let name = &s.name;
    if s.sections.len() < 2 {
        return Err(invalid(format!(
            "{name}: a surface needs at least two sections"
        )));
    }
    s.chordwise
        .validate(&format!("{name}, chordwise"), MAX_CHORDWISE)?;
    if let Some(sp) = &s.spanwise {
        sp.validate(&format!("{name}, spanwise"), usize::MAX)?;
    }
    let finite = |v: &[f64]| v.iter().all(|x| x.is_finite());
    if !finite(&s.scale) || !finite(&s.translate) || !s.angle_deg.is_finite() {
        return Err(invalid(format!(
            "{name}: scale, translation and angle must be finite"
        )));
    }
    if s.scale.contains(&0.0) {
        return Err(invalid(format!("{name}: a scale factor is zero")));
    }
    if let Some(y) = s.y_duplicate {
        if !y.is_finite() {
            return Err(invalid(format!(
                "{name}: the duplicate plane must be finite"
            )));
        }
        if y_symmetry && y == 0.0 {
            return Err(invalid(format!(
                "{name}: a mirror image at y = 0 duplicates the symmetry plane"
            )));
        }
    }
    let last = s.sections.len() - 1;
    for (k, sec) in s.sections.iter().enumerate() {
        if !finite(&sec.leading_edge) || !sec.incidence_deg.is_finite() {
            return Err(invalid(format!(
                "{name}, section {}: coordinates must be finite",
                k + 1
            )));
        }
        if !(sec.chord.is_finite() && sec.chord >= 0.0) {
            return Err(invalid(format!(
                "{name}, section {}: negative chord",
                k + 1
            )));
        }
        if !(sec.lift_slope_factor > 0.0 && sec.lift_slope_factor < 2.0) {
            return Err(invalid(format!(
                "{name}, section {}: the lift-slope factor must be from 0 to 2",
                k + 1
            )));
        }
        if s.spanwise.is_none() && k < last {
            match &sec.spanwise {
                Some(sp) => sp.validate(&format!("{name}, section {}", k + 1), usize::MAX)?,
                None => {
                    return Err(invalid(format!(
                        "{name}, section {}: no spanwise spacing (give one for the surface or for each section)",
                        k + 1
                    )));
                }
            }
        }
        for c in &sec.controls {
            if c.name.trim().is_empty() || c.name.contains(char::is_whitespace) {
                return Err(invalid(format!(
                    "{name}, section {}: a control name must be one word",
                    k + 1
                )));
            }
            if !finite(&[c.gain, c.hinge, c.duplicate_sign]) || !finite(&c.axis) {
                return Err(invalid(format!(
                    "{name}, section {}, control {}: values must be finite",
                    k + 1,
                    c.name
                )));
            }
        }
    }
    for (k, w) in s.sections.windows(2).enumerate() {
        let dy = w[1].leading_edge[1] - w[0].leading_edge[1];
        let dz = w[1].leading_edge[2] - w[0].leading_edge[2];
        if dy.hypot(dz) == 0.0 {
            return Err(invalid(format!(
                "{name}: sections {} and {} are at the same place in y and z",
                k + 1,
                k + 2
            )));
        }
    }
    Ok(())
}
