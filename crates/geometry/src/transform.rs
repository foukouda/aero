//! Leading edge, chord and normalization of airfoils.

use serde::{Deserialize, Serialize};

use crate::airfoil::Airfoil;
use crate::error::{GeometryError, Result};
use crate::leading_edge::{LeadingEdge, LeadingEdgeMethod, find_leading_edge};

/// How an airfoil is normalized to unit chord.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Normalization {
    /// XFOIL `NORM`: translate the leading edge to the origin and scale the
    /// leading-edge-to-trailing-edge distance to one, without rotation.
    #[default]
    Xfoil,
    /// Same as [`Normalization::Xfoil`], then rotate so that the trailing
    /// edge lies at `(1, 0)`. Changes the angle-of-attack reference when the
    /// input chord line is not horizontal.
    ChordAligned,
}

/// Similarity transform `p' = scale · R(rotation) · (p − origin)` applied by a
/// normalization.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NormalizationTransform {
    /// Point moved to the origin (the leading edge).
    pub origin: [f64; 2],
    /// Scale factor (inverse of the original chord).
    pub scale: f64,
    /// Rotation angle in radians (counterclockwise positive).
    pub rotation: f64,
}

impl Airfoil {
    /// Leading edge found by XFOIL's `LEFIND` criterion.
    pub fn leading_edge(&self) -> LeadingEdge {
        find_leading_edge(&self.spline(), LeadingEdgeMethod::default())
    }

    /// Leading edge found with the given method.
    pub fn leading_edge_with(&self, method: LeadingEdgeMethod) -> LeadingEdge {
        find_leading_edge(&self.spline(), method)
    }

    /// Chord: distance between the leading edge (`LEFIND`) and the trailing
    /// edge midpoint.
    pub fn chord(&self) -> f64 {
        let le = self.leading_edge();
        let [xte, yte] = self.trailing_edge();
        (xte - le.x).hypot(yte - le.y)
    }

    /// Returns a normalized copy: chord 1 and leading edge at the origin.
    pub fn normalized(&self, mode: Normalization) -> Result<Airfoil> {
        self.normalized_with_transform(mode).map(|(af, _)| af)
    }

    /// Returns a normalized copy together with the applied transform.
    pub fn normalized_with_transform(
        &self,
        mode: Normalization,
    ) -> Result<(Airfoil, NormalizationTransform)> {
        let le = self.leading_edge();
        let [xte, yte] = self.trailing_edge();
        let chord = (xte - le.x).hypot(yte - le.y);
        if chord.is_nan() || chord <= 0.0 {
            return Err(GeometryError::Degenerate(
                "leading and trailing edges coincide".into(),
            ));
        }
        let scale = 1.0 / chord;
        let rotation = match mode {
            Normalization::Xfoil => 0.0,
            Normalization::ChordAligned => -(yte - le.y).atan2(xte - le.x),
        };
        let (sr, cr) = rotation.sin_cos();
        let af = self.mapped(|x, y| {
            let dx = (x - le.x) * scale;
            let dy = (y - le.y) * scale;
            (cr * dx - sr * dy, sr * dx + cr * dy)
        })?;
        Ok((
            af,
            NormalizationTransform {
                origin: [le.x, le.y],
                scale,
                rotation,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naca::{NacaOptions, naca};

    fn transformed(af: &Airfoil, scale: f64, angle: f64, dx: f64, dy: f64) -> Airfoil {
        let (s, c) = angle.sin_cos();
        af.mapped(|x, y| (scale * (c * x - s * y) + dx, scale * (s * x + c * y) + dy))
            .unwrap()
    }

    #[test]
    fn naca_leading_edge_is_at_origin() {
        let af = naca("2412", &NacaOptions::default()).unwrap();
        let le = af.leading_edge();
        assert!(le.converged);
        assert!(le.x.abs() < 1e-6 && le.y.abs() < 1e-6, "{le:?}");
        assert!((af.chord() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn xfoil_normalization_undoes_translation_and_scale() {
        let base = naca("0012", &NacaOptions::default()).unwrap();
        let moved = transformed(&base, 2.5, 0.0, 3.0, -1.0);
        let (norm, tr) = moved
            .normalized_with_transform(Normalization::Xfoil)
            .unwrap();
        assert!((tr.scale - 0.4).abs() < 1e-6);
        assert_eq!(tr.rotation, 0.0);
        for (a, b) in base.points().zip(norm.points()) {
            assert!((a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6);
        }
        assert!((norm.chord() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn chord_aligned_normalization_undoes_rotation() {
        let base = naca("4415", &NacaOptions::default()).unwrap();
        let rotated = transformed(&base, 0.8, 0.3, -0.2, 0.5);
        let xfoil = rotated.normalized(Normalization::Xfoil).unwrap();
        let te = xfoil.trailing_edge();
        assert!(te[1].abs() > 0.1, "XFOIL mode keeps the rotation");
        let aligned = rotated.normalized(Normalization::ChordAligned).unwrap();
        let te = aligned.trailing_edge();
        assert!((te[0] - 1.0).abs() < 1e-9 && te[1].abs() < 1e-9, "{te:?}");
        for (a, b) in base.points().zip(aligned.points()) {
            assert!((a[0] - b[0]).abs() < 1e-5 && (a[1] - b[1]).abs() < 1e-5);
        }
        assert_eq!(aligned.name(), base.name());
    }

    #[test]
    fn leading_edge_methods_agree_on_naca() {
        let af = naca("0012", &NacaOptions::default()).unwrap();
        let a = af.leading_edge_with(LeadingEdgeMethod::FarthestFromTrailingEdge);
        let b = af.leading_edge_with(LeadingEdgeMethod::MaxCurvature);
        assert!(
            (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3,
            "{a:?} {b:?}"
        );
    }
}
