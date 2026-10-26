//! Geometric properties of an airfoil (XFOIL `GEOPAR`, `TCCALC`, `SOPPS`).
//!
//! Thickness and camber follow XFOIL's definition: in chord-line axes (from
//! the leading edge to the trailing edge midpoint), the thickness at a
//! chordwise station is the distance between the two surface points having
//! that same chordwise abscissa, and the camber is their mean ordinate.
//! XFOIL only evaluates them at the contour nodes; here the maxima are
//! refined on the splined contour.

use serde::{Deserialize, Serialize};

use crate::airfoil::Airfoil;
use crate::leading_edge::lefind;
use crate::spline::SplineCurve;

/// Geometric properties of an airfoil. Lengths are divided by the chord.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeometricProperties {
    /// Chord (leading edge to trailing edge midpoint), in input units.
    pub chord: f64,
    /// Leading-edge point, in input coordinates.
    pub leading_edge: [f64; 2],
    /// Trailing-edge midpoint, in input coordinates.
    pub trailing_edge: [f64; 2],
    /// Maximum thickness `t/c`.
    pub max_thickness: f64,
    /// Chordwise position `x/c` of the maximum thickness.
    pub max_thickness_x: f64,
    /// Maximum camber `f/c` (signed: negative for a negatively cambered section).
    pub max_camber: f64,
    /// Chordwise position `x/c` of the maximum camber.
    pub max_camber_x: f64,
    /// Leading-edge radius `r/c` (inverse curvature at the leading edge;
    /// zero for a sharp leading edge).
    pub le_radius: f64,
    /// Trailing-edge included angle, in degrees.
    pub te_angle_deg: f64,
    /// Trailing-edge thickness (gap) divided by the chord.
    pub te_thickness: f64,
    /// Cross-section area divided by the chord squared.
    pub area: f64,
}

impl Airfoil {
    /// Computes the geometric properties of the airfoil.
    pub fn properties(&self) -> GeometricProperties {
        properties(self)
    }
}

/// XFOIL `ATANC`: `atan2(y, x)` continued from `thold` across the branch cut.
pub(crate) fn atanc(y: f64, x: f64, thold: f64) -> f64 {
    use std::f64::consts::{PI, TAU};
    let thnew = y.atan2(x);
    let dthet = thnew - thold;
    let dtcorr = dthet - TAU * ((dthet + PI.copysign(dthet)) / TAU).trunc();
    thold + dtcorr
}

/// Chord-line frame of a contour: from the point at arc length `sle` to the
/// trailing-edge midpoint.
pub(crate) struct ChordFrame {
    pub(crate) sle: f64,
    pub(crate) le: [f64; 2],
    pub(crate) dir: [f64; 2],
    pub(crate) chord: f64,
}

impl ChordFrame {
    pub(crate) fn new(curve: &SplineCurve, sle: f64) -> Self {
        let x = curve.x_nodes();
        let y = curve.y_nodes();
        let n = x.len() - 1;
        let le = curve.point(sle);
        let te = [0.5 * (x[0] + x[n]), 0.5 * (y[0] + y[n])];
        let chord = (te[0] - le[0]).hypot(te[1] - le[1]);
        let dir = [(te[0] - le[0]) / chord, (te[1] - le[1]) / chord];
        Self {
            sle,
            le,
            dir,
            chord,
        }
    }

    /// `(xbar, ybar)` of a point in chord-line axes (not divided by the chord).
    pub(crate) fn local(&self, p: [f64; 2]) -> (f64, f64) {
        let dx = p[0] - self.le[0];
        let dy = p[1] - self.le[1];
        (
            dx * self.dir[0] + dy * self.dir[1],
            dy * self.dir[0] - dx * self.dir[1],
        )
    }
}

/// XFOIL `SOPPS`: arc length of the point on the other surface with the same
/// chordwise abscissa as the point at `si`.
pub(crate) fn sopps(curve: &SplineCurve, frame: &ChordFrame, si: f64) -> f64 {
    let s = curve.arc_lengths();
    let n = s.len() - 1;
    let slen = s[n] - s[0];
    let sle = frame.sle;
    let (s_in, s_opp) = if si < sle { (s[0], s[n]) } else { (s[n], s[0]) };
    let sfrac = (si - sle) / (s_in - sle);
    let first = sle + sfrac * (s_opp - sle);
    if sfrac.abs() <= 1.0e-5 {
        return sle;
    }
    let (xbar, _) = frame.local(curve.point(si));
    let mut sopp = first;
    for _ in 0..12 {
        let (xo, _) = frame.local(curve.point(sopp));
        let [dx, dy] = curve.tangent(sopp);
        let res = xo - xbar;
        let resd = dx * frame.dir[0] + dy * frame.dir[1];
        if res.abs() / slen < 1.0e-5 {
            return sopp;
        }
        if resd == 0.0 {
            return first;
        }
        let ds = -res / resd;
        sopp += ds;
        if !sopp.is_finite() {
            return first;
        }
        if ds.abs() / slen < 1.0e-5 {
            return sopp;
        }
    }
    first
}

/// Thickness, camber and chordwise position at upper-surface arc length `s`.
fn station(curve: &SplineCurve, frame: &ChordFrame, s: f64) -> (f64, f64, f64) {
    let (xbar, ybar) = frame.local(curve.point(s));
    let (_, yopp) = frame.local(curve.point(sopps(curve, frame, s)));
    (xbar, (ybar - yopp).abs(), 0.5 * (ybar + yopp))
}

/// Golden-section maximization of `f` on `[a, b]`.
fn maximize(mut a: f64, mut b: f64, f: impl Fn(f64) -> f64) -> f64 {
    let r = 0.5 * (5.0_f64.sqrt() - 1.0);
    let mut c = b - r * (b - a);
    let mut d = a + r * (b - a);
    let (mut fc, mut fd) = (f(c), f(d));
    for _ in 0..80 {
        if fc > fd {
            b = d;
            d = c;
            fd = fc;
            c = b - r * (b - a);
            fc = f(c);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + r * (b - a);
            fd = f(d);
        }
    }
    0.5 * (a + b)
}

fn properties(airfoil: &Airfoil) -> GeometricProperties {
    let curve = airfoil.spline();
    let s = curve.arc_lengths().to_vec();
    let n = s.len() - 1;
    let (sle, _) = lefind(&curve);
    let frame = ChordFrame::new(&curve, sle);
    let chord = frame.chord;

    // Candidate stations: upper-surface nodes plus a uniform sampling.
    let mut samples: Vec<f64> = s.iter().copied().filter(|&v| v < sle).collect();
    samples.extend((0..400).map(|k| s[0] + (sle - s[0]) * f64::from(k) / 400.0));
    samples.sort_by(f64::total_cmp);
    samples.dedup();

    let stations: Vec<(f64, (f64, f64, f64))> = samples
        .iter()
        .map(|&si| (si, station(&curve, &frame, si)))
        .collect();

    let refine = |key: &dyn Fn(&(f64, f64, f64)) -> f64| -> (f64, f64, f64) {
        let Some(best) = (0..stations.len())
            .max_by(|&i, &j| key(&stations[i].1).total_cmp(&key(&stations[j].1)))
        else {
            return (0.0, 0.0, 0.0);
        };
        let lo = stations[best.saturating_sub(1)].0;
        let hi = stations[(best + 1).min(stations.len() - 1)].0;
        let s_best = maximize(lo, hi, |v| key(&station(&curve, &frame, v)));
        let refined = station(&curve, &frame, s_best);
        if key(&refined) >= key(&stations[best].1) {
            refined
        } else {
            stations[best].1
        }
    };

    let (xt, thick, _) = refine(&|st| st.1);
    // Signed camber: maximize |camber| and keep its sign.
    let (xc, _, camb) = refine(&|st| st.2.abs());

    let curv_le = curve.curvature(sle);
    let le_radius = if curv_le.abs() > 0.001 * (s[n] - s[0]) {
        1.0 / curv_le
    } else {
        0.0
    };

    let xp = curve.dx_nodes();
    let yp = curve.dy_nodes();
    let ang1 = (-yp[0]).atan2(-xp[0]);
    let ang2 = atanc(yp[n], xp[n], ang1);

    GeometricProperties {
        chord,
        leading_edge: frame.le,
        trailing_edge: airfoil.trailing_edge(),
        max_thickness: thick / chord,
        max_thickness_x: xt / chord,
        max_camber: camb / chord,
        max_camber_x: xc / chord,
        le_radius: le_radius / chord,
        te_angle_deg: (ang2 - ang1).to_degrees(),
        te_thickness: airfoil.trailing_edge_gap() / chord,
        area: airfoil.area() / (chord * chord),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naca::{NacaOptions, TrailingEdge, naca};

    #[test]
    fn atanc_continues_across_branch_cut() {
        use std::f64::consts::PI;
        let a = atanc(-1.0, -1.0, 0.75 * PI);
        assert!((a - 1.25 * PI).abs() < 1e-14);
        assert!((atanc(1.0, 0.0, 0.0) - 0.5 * PI).abs() < 1e-14);
    }

    #[test]
    fn naca0012_properties() {
        let p = naca("0012", &NacaOptions::default()).unwrap().properties();
        assert!((p.chord - 1.0).abs() < 1e-6);
        // Analytic maximum of the thickness polynomial: 0.120034 at x = 0.2998.
        assert!((p.max_thickness - 0.120_034).abs() < 2e-5, "{p:?}");
        assert!((p.max_thickness_x - 0.2998).abs() < 2e-3, "{p:?}");
        assert!(p.max_camber.abs() < 1e-9);
        // Leading-edge radius 1.1019 t^2 = 0.015867.
        assert!((p.le_radius - 0.015_867).abs() < 3e-4, "{}", p.le_radius);
        // Included TE angle 2 atan(0.14031) = 15.97 deg.
        assert!((p.te_angle_deg - 15.97).abs() < 0.1, "{}", p.te_angle_deg);
        assert!((p.te_thickness - 0.002_52).abs() < 1e-8);
        assert!(p.area > 0.08 && p.area < 0.083, "{}", p.area);
    }

    #[test]
    fn cambered_sections() {
        let p = naca("2412", &NacaOptions::default()).unwrap().properties();
        assert!((p.max_camber - 0.02).abs() < 1e-4, "{p:?}");
        assert!((p.max_camber_x - 0.4).abs() < 5e-3, "{p:?}");
        assert!((p.max_thickness - 0.12).abs() < 5e-4, "{p:?}");
        let p = naca("4415", &NacaOptions::default()).unwrap().properties();
        assert!((p.max_camber - 0.04).abs() < 2e-4, "{p:?}");
        assert!((p.max_thickness - 0.15).abs() < 1e-3, "{p:?}");
    }

    #[test]
    fn negative_camber_keeps_its_sign() {
        let af = naca("2412", &NacaOptions::default()).unwrap();
        // Mirroring reverses the orientation: rebuild in Selig order.
        let x: Vec<f64> = af.x().iter().rev().copied().collect();
        let y: Vec<f64> = af.y().iter().rev().map(|v| -v).collect();
        let neg = Airfoil::new("neg", x, y).unwrap();
        assert!((neg.properties().max_camber + 0.02).abs() < 1e-4);
    }

    #[test]
    fn sharp_trailing_edge_has_zero_thickness() {
        let p = naca(
            "0012",
            &NacaOptions {
                trailing_edge: TrailingEdge::Closed,
                ..NacaOptions::default()
            },
        )
        .unwrap()
        .properties();
        assert!(p.te_thickness < 1e-12);
        assert!(p.te_angle_deg > 10.0);
    }
}
