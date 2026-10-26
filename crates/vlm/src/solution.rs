//! The flow condition and the results of a solution, with AVL's meanings
//! and signs.
//!
//! Forces and moments are coefficients referred to the reference area,
//! chord (pitch) and span (roll, yaw), moments about the reference point.
//! As AVL prints them, the body-axis and stability-axis quantities use the
//! standard orientation (x forward, z down): a positive rolling moment
//! lowers the right wing, a positive yawing moment turns the nose right.

use serde::{Deserialize, Serialize};

/// A flow condition (AVL `OPER` variables).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowCondition {
    /// Angle of attack, degrees.
    pub alpha_deg: f64,
    /// Sideslip angle, degrees (wind from the right is positive).
    pub beta_deg: f64,
    /// Roll rate about the stability x axis, `p·b/2V`.
    pub roll_rate: f64,
    /// Pitch rate, `q·c/2V`.
    pub pitch_rate: f64,
    /// Yaw rate about the stability z axis, `r·b/2V`.
    pub yaw_rate: f64,
    /// Deflections of the control variables in degrees, in the order of
    /// [`VortexLattice::control_names`](crate::VortexLattice::control_names);
    /// missing ones are zero.
    pub controls: Vec<f64>,
}

impl FlowCondition {
    /// A flow at the angle of attack `alpha_deg`, nothing else.
    pub fn alpha(alpha_deg: f64) -> Self {
        Self {
            alpha_deg,
            beta_deg: 0.0,
            roll_rate: 0.0,
            pitch_rate: 0.0,
            yaw_rate: 0.0,
            controls: Vec::new(),
        }
    }
}

/// Total forces and moments (AVL `FT`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Totals {
    /// Body-axis force coefficients (x forward, z down).
    pub cx: f64,
    /// Side force coefficient (y right).
    pub cy: f64,
    /// Body-axis normal force coefficient (z down).
    pub cz: f64,
    /// Rolling moment about the body x axis.
    pub roll: f64,
    /// Pitching moment.
    pub pitch: f64,
    /// Yawing moment about the body z axis.
    pub yaw: f64,
    /// Rolling moment about the stability x axis.
    pub roll_stability: f64,
    /// Yawing moment about the stability z axis.
    pub yaw_stability: f64,
    /// Lift coefficient.
    pub cl: f64,
    /// Drag coefficient from the forces on the vortices (near field), with
    /// the profile drag `CDp`.
    pub cd: f64,
    /// Viscous part of [`cd`](Self::cd) (here `CDp`).
    pub cd_viscous: f64,
    /// Induced part of [`cd`](Self::cd).
    pub cd_induced: f64,
    /// Lift coefficient in the Trefftz plane (far field).
    pub cl_trefftz: f64,
    /// Induced drag coefficient in the Trefftz plane, the most accurate.
    pub cd_trefftz: f64,
    /// Side force coefficient in the Trefftz plane.
    pub cy_trefftz: f64,
    /// Span efficiency `(CL² + CY²) / (π A CDi)`.
    pub span_efficiency: f64,
}

/// Forces of one surface (AVL `FN`, `FS`), referred to the reference
/// quantities, moments in body axes about the reference point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SurfaceForces {
    /// Name; mirror images end with `(YDUP)`.
    pub name: String,
    /// Area of the surface (one side).
    pub area: f64,
    /// Area divided by the width.
    pub mean_chord: f64,
    /// Lift coefficient.
    pub cl: f64,
    /// Rolling moment.
    pub roll: f64,
    /// Side force coefficient.
    pub cy: f64,
    /// Pitching moment.
    pub pitch: f64,
    /// Drag coefficient.
    pub cd: f64,
    /// Yawing moment.
    pub yaw: f64,
    /// Induced drag coefficient.
    pub cd_induced: f64,
    /// Viscous drag coefficient.
    pub cd_viscous: f64,
    /// Lift coefficient referred to the surface's own area.
    pub cl_surface: f64,
    /// Drag coefficient referred to the surface's own area.
    pub cd_surface: f64,
    /// Index of the first strip of the surface in [`Solution::strips`].
    pub first_strip: usize,
    /// Number of strips.
    pub strips: usize,
    /// Number of chordwise vortices per strip.
    pub chordwise: usize,
}

/// Forces of one strip (AVL `FS`), referred to the strip area and chord.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StripForces {
    /// Leading edge at the middle of the strip.
    pub leading_edge: [f64; 3],
    /// Chord.
    pub chord: f64,
    /// Area (chord × width).
    pub area: f64,
    /// Span loading: chord × normal force coefficient (`c·cl` of AVL).
    pub chord_cl: f64,
    /// Induced angle from the downwash in the Trefftz plane (radians).
    pub downwash: f64,
    /// Lift coefficient referred to the velocity perpendicular to the
    /// sweep (the one to compare with the airfoil's maximum).
    pub cl_perp: f64,
    /// Lift coefficient.
    pub cl: f64,
    /// Drag coefficient.
    pub cd: f64,
    /// Viscous drag coefficient.
    pub cd_viscous: f64,
    /// Pitching moment about the quarter chord.
    pub cm_quarter: f64,
    /// Moment about the leading edge.
    pub cm_leading_edge: f64,
    /// Centre of pressure as a fraction of the chord, when there is lift.
    pub center_of_pressure: Option<f64>,
}

/// Derivatives of the six coefficients with respect to one variable, in
/// stability axes (AVL `ST`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct CoefficientDerivatives {
    /// Of the lift coefficient.
    pub cl: f64,
    /// Of the side force coefficient.
    pub cy: f64,
    /// Of the drag coefficient.
    pub cd: f64,
    /// Of the rolling moment.
    pub roll: f64,
    /// Of the pitching moment.
    pub pitch: f64,
    /// Of the yawing moment.
    pub yaw: f64,
}

/// Derivatives with respect to one control variable, per degree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlDerivatives {
    /// Name of the control.
    pub name: String,
    /// Derivatives of the forces and moments.
    pub forces: CoefficientDerivatives,
    /// Of the Trefftz-plane induced drag.
    pub cd_trefftz: f64,
    /// Of the span efficiency.
    pub span_efficiency: f64,
}

/// Stability and control derivatives in stability axes (AVL `ST`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StabilityDerivatives {
    /// Per radian of angle of attack, the stability-axis rates held.
    ///
    /// As in AVL 3.52, which this port matches: with a roll or yaw rate,
    /// the rotation of those rates with the angle of attack enters with
    /// the opposite sign (AVL's `WROT_A` takes the rates already turned to
    /// the standard axes). At zero roll and yaw rates — steady flight — the
    /// derivatives are exact.
    pub alpha: CoefficientDerivatives,
    /// Per radian of sideslip.
    pub beta: CoefficientDerivatives,
    /// Per unit `p·b/2V`.
    pub roll_rate: CoefficientDerivatives,
    /// Per unit `q·c/2V`.
    pub pitch_rate: CoefficientDerivatives,
    /// Per unit `r·b/2V`.
    pub yaw_rate: CoefficientDerivatives,
    /// Per degree of each control.
    pub controls: Vec<ControlDerivatives>,
    /// x of the neutral point, where the pitching moment does not change
    /// with the angle of attack; `None` without lift slope.
    pub neutral_point: Option<f64>,
    /// `Clβ Cnr / (Clr Cnβ)`, above 1 when spirally stable; `None` when
    /// undefined.
    pub spiral_stability: Option<f64>,
}

/// The solution at one flow condition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Solution {
    /// The flow condition.
    pub flow: FlowCondition,
    /// Total forces and moments.
    pub totals: Totals,
    /// Forces of each surface, mirror images included.
    pub surfaces: Vec<SurfaceForces>,
    /// Forces of each strip.
    pub strips: Vec<StripForces>,
    /// Stability and control derivatives.
    pub derivatives: StabilityDerivatives,
}
