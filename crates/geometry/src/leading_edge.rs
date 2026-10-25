//! Leading-edge detection, ported from XFOIL's `LEFIND` and `NSFIND`.

use serde::{Deserialize, Serialize};

use crate::spline::{SplineCurve, trisol};

/// How the leading edge of a contour is defined.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeadingEdgeMethod {
    /// XFOIL `LEFIND` (default): the point whose tangent is perpendicular to
    /// the line joining it to the trailing edge, i.e. the point farthest from
    /// the trailing edge.
    #[default]
    FarthestFromTrailingEdge,
    /// XFOIL `NSFIND`: the point of maximum (smoothed) curvature.
    MaxCurvature,
}

/// Location of the leading edge on a splined contour.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LeadingEdge {
    /// Arc length at the leading edge.
    pub s: f64,
    /// x coordinate.
    pub x: f64,
    /// y coordinate.
    pub y: f64,
    /// True if the leading edge is a corner (doubled point).
    pub sharp: bool,
    /// False if the iteration did not converge; the first-guess node is then
    /// returned, as XFOIL does (this is also the case at a corner).
    pub converged: bool,
}

/// Finds the leading edge of a splined contour.
pub fn find_leading_edge(curve: &SplineCurve, method: LeadingEdgeMethod) -> LeadingEdge {
    let (s, converged) = match method {
        LeadingEdgeMethod::FarthestFromTrailingEdge => lefind(curve),
        LeadingEdgeMethod::MaxCurvature => nsfind(curve),
    };
    let knots = curve.arc_lengths();
    let sharp = knots.windows(2).any(|w| w[0] == s && w[1] == s);
    let [x, y] = curve.point(s);
    LeadingEdge {
        s,
        x,
        y,
        sharp,
        converged,
    }
}

/// XFOIL `LEFIND`. Returns `(s_le, converged)`.
pub(crate) fn lefind(curve: &SplineCurve) -> (f64, bool) {
    let s = curve.arc_lengths();
    let x = curve.x_nodes();
    let y = curve.y_nodes();
    let n = s.len();

    let dseps = (s[n - 1] - s[0]) * 1.0e-5;
    let xte = 0.5 * (x[0] + x[n - 1]);
    let yte = 0.5 * (y[0] + y[n - 1]);

    // First guess: node where the dot product of the chord-to-TE vector and
    // the panel direction changes sign.
    let mut i = if n < 5 {
        (n / 2).saturating_sub(1)
    } else {
        n - 2
    };
    if n >= 5 {
        for k in 2..=n - 3 {
            let dxte = x[k] - xte;
            let dyte = y[k] - yte;
            let dotm = dxte * (x[k] - x[k - 1]) + dyte * (y[k] - y[k - 1]);
            let dotp = dxte * (x[k + 1] - x[k]) + dyte * (y[k + 1] - y[k]);
            if dotm > 0.0 && dotp <= 0.0 {
                i = k;
                break;
            }
        }
    }

    let s_guess = s[i];
    if i > 0 && s[i] == s[i - 1] {
        return (s_guess, true);
    }

    let mut sle = s_guess;
    for _ in 0..50 {
        let [xle, yle] = curve.point(sle);
        let [dxds, dyds] = curve.tangent(sle);
        let [dxdd, dydd] = curve.second_derivative(sle);
        let xchord = xle - xte;
        let ychord = yle - yte;

        let res = xchord * dxds + ychord * dyds;
        let ress = dxds * dxds + dyds * dyds + xchord * dxdd + ychord * dydd;

        let limit = 0.02 * (xchord + ychord).abs();
        let dsle = (-res / ress).clamp(-limit, limit);
        if !dsle.is_finite() {
            break;
        }
        sle += dsle;
        if dsle.abs() < dseps {
            return (sle, true);
        }
    }
    (s_guess, false)
}

/// XFOIL `NSFIND`: maximum of the smoothed curvature. Returns
/// `(s_le, converged)`.
fn nsfind(curve: &SplineCurve) -> (f64, bool) {
    let s = curve.arc_lengths();
    let n = s.len();
    let mut cv: Vec<f64> = s.iter().map(|&si| curve.curvature(si)).collect();

    let smool = 0.006 * (s[n - 1] - s[0]);
    let smoosq = smool * smool;
    let mut a = vec![0.0; n];
    let mut b = vec![0.0; n];
    let mut c = vec![0.0; n];
    a[0] = 1.0;
    c[0] = 0.0;
    for i in 1..n - 1 {
        let dsm = s[i] - s[i - 1];
        let dsp = s[i + 1] - s[i];
        let dso = 0.5 * (s[i + 1] - s[i - 1]);
        if dsm == 0.0 || dsp == 0.0 {
            b[i] = 0.0;
            a[i] = 1.0;
            c[i] = 0.0;
        } else {
            b[i] = smoosq * (-1.0 / dsm) / dso;
            a[i] = smoosq * (1.0 / dsp + 1.0 / dsm) / dso + 1.0;
            c[i] = smoosq * (-1.0 / dsp) / dso;
        }
    }
    b[n - 1] = 0.0;
    a[n - 1] = 1.0;
    trisol(&mut a, &b, &mut c, &mut cv);

    let mut ivmax = 0;
    let mut cvmax = 0.0;
    for (i, v) in cv.iter().enumerate().take(n - 1).skip(1) {
        if v.abs() > cvmax {
            cvmax = v.abs();
            ivmax = i;
        }
    }
    if ivmax == 0 {
        return (s[n / 2], false);
    }

    let i = ivmax;
    let mut ip = i + 1;
    let mut im = i - 1;
    if s[i] == s[ip] {
        ip = (i + 2).min(n - 1);
    }
    if s[i] == s[im] {
        im = i.saturating_sub(2);
    }
    let dsm = s[i] - s[im];
    let dsp = s[ip] - s[i];
    let cvsm = (cv[i] - cv[im]) / dsm;
    let cvsp = (cv[ip] - cv[i]) / dsp;
    let cvs = (cvsm * dsp + cvsp * dsm) / (dsp + dsm);
    let cvss = 2.0 * (cvsp - cvsm) / (dsp + dsm);
    let ds = -cvs / cvss;
    if ds.is_finite() && ds.abs() <= dsm.max(dsp) {
        (s[i] + ds, true)
    } else {
        (s[i], false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ellipse x = 0.5(1 + cos t), y = 0.06 sin t, Selig ordered.
    fn ellipse(n: usize, rotate: f64) -> SplineCurve {
        let (sa, ca) = rotate.sin_cos();
        let mut x = Vec::new();
        let mut y = Vec::new();
        for i in 0..=n {
            let t = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
            let (px, py) = (0.5 * (1.0 + t.cos()), 0.06 * t.sin());
            x.push(ca * px - sa * py);
            y.push(sa * px + ca * py);
        }
        // Open the "trailing edge" slightly so ends differ.
        x[n] += 1e-4;
        SplineCurve::new(&x, &y).unwrap()
    }

    #[test]
    fn lefind_finds_ellipse_nose() {
        let c = ellipse(161, 0.0);
        let le = find_leading_edge(&c, LeadingEdgeMethod::FarthestFromTrailingEdge);
        assert!(le.converged && !le.sharp);
        assert!(le.x.abs() < 1e-5, "x = {}", le.x);
        assert!(le.y.abs() < 1e-5, "y = {}", le.y);
    }

    #[test]
    fn lefind_follows_rotation() {
        let angle: f64 = 0.2;
        let c = ellipse(161, angle);
        let le = find_leading_edge(&c, LeadingEdgeMethod::FarthestFromTrailingEdge);
        // Farthest point from the (rotated) TE is the rotated nose (0, 0).
        assert!(le.x.hypot(le.y) < 1e-4, "{le:?}");
    }

    #[test]
    fn max_curvature_finds_naca_nose() {
        // An ellipse has the same curvature at both ends; use a NACA section,
        // whose curvature peaks at the nose.
        let af = crate::naca::naca("0012", &crate::naca::NacaOptions::default()).unwrap();
        let le = find_leading_edge(&af.spline(), LeadingEdgeMethod::MaxCurvature);
        assert!(le.converged);
        assert!(le.x < 1e-4, "{le:?}");
        assert!(le.y.abs() < 1e-4, "{le:?}");
    }

    #[test]
    fn sharp_leading_edge_is_detected() {
        // Diamond with a doubled leading-edge point.
        let x = [1.0, 0.75, 0.5, 0.25, 0.0, 0.0, 0.25, 0.5, 0.75, 1.0];
        let y = [
            0.0, 0.025, 0.05, 0.025, 0.0, 0.0, -0.025, -0.05, -0.025, -0.0001,
        ];
        let c = SplineCurve::new(&x, &y).unwrap();
        let le = find_leading_edge(&c, LeadingEdgeMethod::FarthestFromTrailingEdge);
        assert!(le.sharp);
        assert_eq!(le.x, 0.0);
    }
}
