//! The [`Airfoil`] contour type.

use serde::{Deserialize, Serialize};

use crate::error::{GeometryError, Result};
use crate::spline::{SplineCurve, arc_lengths};

/// Minimum number of points of an airfoil contour.
pub const MIN_POINTS: usize = 5;

/// A closed (or open trailing edge) airfoil contour.
///
/// Points follow the Selig convention, which is also XFOIL's internal
/// ordering: from the trailing edge along the upper surface to the leading
/// edge, then along the lower surface back to the trailing edge. This is a
/// counterclockwise traversal.
///
/// Invariants, checked on construction and deserialization:
/// - at least [`MIN_POINTS`] points, all finite;
/// - the first two and the last two points differ;
/// - two identical successive points mark a corner; three are rejected;
/// - non-zero enclosed area with counterclockwise orientation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RawAirfoil")]
pub struct Airfoil {
    name: String,
    x: Vec<f64>,
    y: Vec<f64>,
}

#[derive(Deserialize)]
struct RawAirfoil {
    name: String,
    x: Vec<f64>,
    y: Vec<f64>,
}

impl TryFrom<RawAirfoil> for Airfoil {
    type Error = GeometryError;

    fn try_from(raw: RawAirfoil) -> Result<Self> {
        Airfoil::new(raw.name, raw.x, raw.y)
    }
}

impl Airfoil {
    /// Creates an airfoil from coordinates in Selig (counterclockwise) order.
    pub fn new(name: impl Into<String>, x: Vec<f64>, y: Vec<f64>) -> Result<Self> {
        validate_points(&x, &y)?;
        let area = signed_area(&x, &y);
        if area == 0.0 {
            return Err(GeometryError::Degenerate(
                "the contour encloses no area".into(),
            ));
        }
        if area < 0.0 {
            return Err(GeometryError::ClockwiseOrdering);
        }
        Ok(Self {
            name: name.into(),
            x,
            y,
        })
    }

    /// Creates an airfoil, reversing the point order if it is clockwise.
    /// Returns the airfoil and whether it was reversed.
    pub fn new_reoriented(
        name: impl Into<String>,
        mut x: Vec<f64>,
        mut y: Vec<f64>,
    ) -> Result<(Self, bool)> {
        validate_points(&x, &y)?;
        let reversed = signed_area(&x, &y) < 0.0;
        if reversed {
            x.reverse();
            y.reverse();
        }
        Ok((Self::new(name, x, y)?, reversed))
    }

    /// Creates an airfoil from `(x, y)` pairs in Selig order.
    pub fn from_points(name: impl Into<String>, points: &[[f64; 2]]) -> Result<Self> {
        let (x, y) = points.iter().map(|p| (p[0], p[1])).unzip();
        Self::new(name, x, y)
    }

    /// Airfoil name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Renames the airfoil.
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    /// Returns a copy with another name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// x coordinates.
    pub fn x(&self) -> &[f64] {
        &self.x
    }

    /// y coordinates.
    pub fn y(&self) -> &[f64] {
        &self.y
    }

    /// Number of points.
    pub fn len(&self) -> usize {
        self.x.len()
    }

    /// Always false: an airfoil has at least [`MIN_POINTS`] points.
    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }

    /// Iterates over the points as `[x, y]`.
    pub fn points(&self) -> impl Iterator<Item = [f64; 2]> + '_ {
        self.x.iter().zip(&self.y).map(|(&x, &y)| [x, y])
    }

    /// Polygonal arc length at each point.
    pub fn arc_lengths(&self) -> Vec<f64> {
        arc_lengths(&self.x, &self.y)
    }

    /// Spline of the contour against arc length (XFOIL `SCALC` + `SEGSPL`).
    pub fn spline(&self) -> SplineCurve {
        SplineCurve::from_validated(&self.x, &self.y)
    }

    /// Trailing edge point: midpoint of the first and last points.
    pub fn trailing_edge(&self) -> [f64; 2] {
        let n = self.len() - 1;
        [0.5 * (self.x[0] + self.x[n]), 0.5 * (self.y[0] + self.y[n])]
    }

    /// Distance between the first and last points (trailing edge gap).
    pub fn trailing_edge_gap(&self) -> f64 {
        let n = self.len() - 1;
        (self.x[0] - self.x[n]).hypot(self.y[0] - self.y[n])
    }

    /// Enclosed area (positive for counterclockwise contours).
    pub fn area(&self) -> f64 {
        signed_area(&self.x, &self.y)
    }

    /// Returns a copy with consecutive duplicated points (corners) merged,
    /// as XFOIL's `ABCOPY` does before running the panel solver.
    pub fn without_duplicate_points(&self) -> Result<Self> {
        let (x, y) = dedup_points(&self.x, &self.y);
        Self::new(self.name.clone(), x, y)
    }

    /// Applies an affine map `p -> a·p + b` to every point.
    pub(crate) fn mapped(&self, f: impl Fn(f64, f64) -> (f64, f64)) -> Result<Self> {
        let (x, y) = self.points().map(|[x, y]| f(x, y)).unzip();
        Self::new(self.name.clone(), x, y)
    }
}

/// Removes exactly repeated successive points. Returns the new coordinates.
pub(crate) fn dedup_points(x: &[f64], y: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let mut xo = Vec::with_capacity(x.len());
    let mut yo = Vec::with_capacity(y.len());
    for (&xi, &yi) in x.iter().zip(y) {
        if xo.last() == Some(&xi) && yo.last() == Some(&yi) {
            continue;
        }
        xo.push(xi);
        yo.push(yi);
    }
    (xo, yo)
}

/// Signed area of the closed polygon, XFOIL's `LOAD` formula: positive for
/// counterclockwise ordering.
pub(crate) fn signed_area(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len();
    let mut area = 0.0;
    for i in 0..n {
        let ip = if i + 1 == n { 0 } else { i + 1 };
        area += 0.5 * (y[i] + y[ip]) * (x[i] - x[ip]);
    }
    area
}

fn validate_points(x: &[f64], y: &[f64]) -> Result<()> {
    if x.len() != y.len() {
        return Err(GeometryError::LengthMismatch {
            x: x.len(),
            y: y.len(),
        });
    }
    let n = x.len();
    if n < MIN_POINTS {
        return Err(GeometryError::TooFewPoints {
            found: n,
            min: MIN_POINTS,
        });
    }
    if let Some(index) = x
        .iter()
        .zip(y)
        .position(|(a, b)| !a.is_finite() || !b.is_finite())
    {
        return Err(GeometryError::NonFinite { index });
    }
    let same = |i: usize, j: usize| x[i] == x[j] && y[i] == y[j];
    if same(0, 1) {
        return Err(GeometryError::DuplicatePoint { index: 1 });
    }
    if same(n - 2, n - 1) {
        return Err(GeometryError::DuplicatePoint { index: n - 1 });
    }
    if let Some(i) = (0..n - 2).find(|&i| same(i, i + 1) && same(i + 1, i + 2)) {
        return Err(GeometryError::DuplicatePoint { index: i + 2 });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small diamond-shaped "airfoil" in Selig order.
    fn diamond() -> (Vec<f64>, Vec<f64>) {
        (
            vec![1.0, 0.5, 0.0, 0.5, 1.0],
            vec![0.0, 0.1, 0.0, -0.1, -0.001],
        )
    }

    #[test]
    fn creates_valid_airfoil() {
        let (x, y) = diamond();
        let af = Airfoil::new("diamond", x.clone(), y.clone()).unwrap();
        assert_eq!(af.name(), "diamond");
        assert_eq!(af.len(), 5);
        assert!(!af.is_empty());
        assert_eq!(af.x(), &x[..]);
        assert_eq!(af.y(), &y[..]);
        assert!(af.area() > 0.0);
        assert_eq!(af.points().count(), 5);
        let te = af.trailing_edge();
        assert!((te[0] - 1.0).abs() < 1e-15 && (te[1] + 0.0005).abs() < 1e-15);
        assert!((af.trailing_edge_gap() - 0.001).abs() < 1e-15);
        assert_eq!(af.arc_lengths().len(), 5);
        assert_eq!(af.spline().len(), 5);
    }

    #[test]
    fn rejects_clockwise_but_can_reorient() {
        let (mut x, mut y) = diamond();
        x.reverse();
        y.reverse();
        assert_eq!(
            Airfoil::new("cw", x.clone(), y.clone()),
            Err(GeometryError::ClockwiseOrdering)
        );
        let (af, reversed) = Airfoil::new_reoriented("cw", x, y).unwrap();
        assert!(reversed);
        assert!(af.area() > 0.0);
        let (x, y) = diamond();
        let (_, reversed) = Airfoil::new_reoriented("ccw", x, y).unwrap();
        assert!(!reversed);
    }

    #[test]
    fn rejects_invalid_inputs() {
        assert!(matches!(
            Airfoil::new("a", vec![0.0; 5], vec![0.0; 4]),
            Err(GeometryError::LengthMismatch { .. })
        ));
        assert!(matches!(
            Airfoil::new("a", vec![1.0, 0.0, 1.0], vec![0.1, 0.0, -0.1]),
            Err(GeometryError::TooFewPoints { .. })
        ));
        let (mut x, y) = diamond();
        x[2] = f64::NAN;
        assert!(matches!(
            Airfoil::new("a", x, y),
            Err(GeometryError::NonFinite { index: 2 })
        ));
        let flat = Airfoil::new("flat", vec![1.0, 0.5, 0.0, 0.5, 1.0], vec![0.0; 5]);
        assert!(matches!(flat, Err(GeometryError::Degenerate(_))));
    }

    #[test]
    fn corner_points_are_allowed_but_not_triples() {
        let x = vec![1.0, 0.5, 0.0, 0.0, 0.5, 1.0];
        let y = vec![0.0, 0.1, 0.0, 0.0, -0.1, -0.001];
        let af = Airfoil::new("corner", x.clone(), y.clone()).unwrap();
        assert_eq!(af.without_duplicate_points().unwrap().len(), 5);
        let x3 = vec![1.0, 0.5, 0.0, 0.0, 0.0, 0.5, 1.0];
        let y3 = vec![0.0, 0.1, 0.0, 0.0, 0.0, -0.1, -0.001];
        assert!(matches!(
            Airfoil::new("triple", x3, y3),
            Err(GeometryError::DuplicatePoint { index: 4 })
        ));
        let mut xs = x.clone();
        xs.insert(0, 1.0);
        let mut ys = y.clone();
        ys.insert(0, 0.0);
        assert!(matches!(
            Airfoil::new("first", xs, ys),
            Err(GeometryError::DuplicatePoint { index: 1 })
        ));
    }

    #[test]
    fn serde_round_trip_and_validation() {
        let (x, y) = diamond();
        let af = Airfoil::new("diamond", x, y).unwrap();
        let json = serde_json::to_string(&af).unwrap();
        let back: Airfoil = serde_json::from_str(&json).unwrap();
        assert_eq!(af, back);
        let bad = r#"{"name":"bad","x":[1,0,1],"y":[0,0,0]}"#;
        assert!(serde_json::from_str::<Airfoil>(bad).is_err());
    }

    #[test]
    fn renaming() {
        let (x, y) = diamond();
        let mut af = Airfoil::new("a", x, y).unwrap();
        af.set_name("b");
        assert_eq!(af.name(), "b");
        assert_eq!(af.with_name("c").name(), "c");
    }

    #[test]
    fn from_points_and_mapping() {
        let af = Airfoil::from_points(
            "p",
            &[
                [1.0, 0.0],
                [0.5, 0.1],
                [0.0, 0.0],
                [0.5, -0.1],
                [1.0, -0.001],
            ],
        )
        .unwrap();
        let moved = af.mapped(|x, y| (2.0 * x + 1.0, 2.0 * y)).unwrap();
        assert_eq!(moved.x()[0], 3.0);
        assert_eq!(moved.y()[1], 0.2);
    }
}
