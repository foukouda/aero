//! Cubic splines parameterized by arc length, ported from XFOIL's `spline.f`.
//!
//! Splines are stored in Hermite form, exactly as in XFOIL: the knot
//! abscissas `s`, the values `v` and the first derivatives `dv/ds` at the
//! knots. Evaluation (`SEVAL`, `DEVAL`, `D2VAL`) reconstructs the cubic on the
//! bracketing interval from these three arrays.
//!
//! [`Spline::segmented`] reproduces `SEGSPL`: two successive knots with the
//! same abscissa mark a joint where the first derivative may jump (a corner of
//! an airfoil contour). Each segment between joints uses zero third
//! derivative end conditions.

use serde::{Deserialize, Serialize};

use crate::error::{GeometryError, Result};

/// End condition of a cubic spline, matching the codes of XFOIL's `SPLIND`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum EndCondition {
    /// Zero second derivative ("natural" spline end, code `999`).
    ZeroSecondDerivative,
    /// Zero third derivative (code `-999`), used by `SEGSPL`.
    ZeroThirdDerivative,
    /// Prescribed first derivative `dv/ds` at the end.
    FirstDerivative(f64),
}

/// A cubic spline `v(s)` in Hermite form.
#[derive(Debug, Clone, PartialEq)]
pub struct Spline {
    s: Vec<f64>,
    v: Vec<f64>,
    d: Vec<f64>,
}

impl Spline {
    /// Builds a spline through `(s[i], v[i])` with the given end conditions
    /// (XFOIL `SPLIND`). The abscissas must be strictly increasing.
    pub fn new(s: &[f64], v: &[f64], start: EndCondition, end: EndCondition) -> Result<Self> {
        check_inputs(s, v)?;
        if s.windows(2).any(|w| w[1] <= w[0]) {
            return Err(GeometryError::InvalidSpline(
                "abscissas must be strictly increasing".into(),
            ));
        }
        let mut d = vec![0.0; s.len()];
        splind(v, s, start, end, &mut d);
        Ok(Self {
            s: s.to_vec(),
            v: v.to_vec(),
            d,
        })
    }

    /// Builds a spline that may have derivative discontinuities at joints
    /// (XFOIL `SEGSPL`). A joint is a pair of successive equal abscissas.
    ///
    /// The abscissas must be non-decreasing; the first and last two knots
    /// must differ, and no three successive knots may be equal.
    pub fn segmented(s: &[f64], v: &[f64]) -> Result<Self> {
        check_segmented(s, v)?;
        let mut d = vec![0.0; s.len()];
        segspl(v, s, &mut d);
        Ok(Self {
            s: s.to_vec(),
            v: v.to_vec(),
            d,
        })
    }

    /// Value of the spline at `t` (XFOIL `SEVAL`). Outside the knot range the
    /// end cubic is extrapolated.
    pub fn eval(&self, t: f64) -> f64 {
        seval(t, &self.v, &self.d, &self.s)
    }

    /// First derivative `dv/ds` at `t` (XFOIL `DEVAL`).
    pub fn deriv(&self, t: f64) -> f64 {
        deval(t, &self.v, &self.d, &self.s)
    }

    /// Second derivative `d²v/ds²` at `t` (XFOIL `D2VAL`).
    pub fn deriv2(&self, t: f64) -> f64 {
        d2val(t, &self.v, &self.d, &self.s)
    }

    /// Knot abscissas.
    pub fn knots(&self) -> &[f64] {
        &self.s
    }

    /// Values at the knots.
    pub fn values(&self) -> &[f64] {
        &self.v
    }

    /// First derivatives at the knots.
    pub fn derivatives(&self) -> &[f64] {
        &self.d
    }
}

/// A planar curve `(x(s), y(s))` splined against its polygonal arc length,
/// as XFOIL does for airfoil contours (`SCALC` followed by two `SEGSPL`).
#[derive(Debug, Clone, PartialEq)]
pub struct SplineCurve {
    s: Vec<f64>,
    x: Vec<f64>,
    xs: Vec<f64>,
    y: Vec<f64>,
    ys: Vec<f64>,
}

impl SplineCurve {
    /// Splines the polyline `(x[i], y[i])`. Two identical successive points
    /// mark a corner.
    pub fn new(x: &[f64], y: &[f64]) -> Result<Self> {
        if x.len() != y.len() {
            return Err(GeometryError::LengthMismatch {
                x: x.len(),
                y: y.len(),
            });
        }
        let s = arc_lengths(x, y);
        check_segmented(&s, x)?;
        check_inputs(&s, y)?;
        let mut xs = vec![0.0; x.len()];
        let mut ys = vec![0.0; y.len()];
        segspl(x, &s, &mut xs);
        segspl(y, &s, &mut ys);
        Ok(Self {
            s,
            x: x.to_vec(),
            xs,
            y: y.to_vec(),
            ys,
        })
    }

    /// Splines coordinates already known to satisfy the `SEGSPL` invariants
    /// (validated [`crate::Airfoil`] contours).
    pub(crate) fn from_validated(x: &[f64], y: &[f64]) -> Self {
        let s = arc_lengths(x, y);
        let mut xs = vec![0.0; x.len()];
        let mut ys = vec![0.0; y.len()];
        segspl(x, &s, &mut xs);
        segspl(y, &s, &mut ys);
        Self {
            s,
            x: x.to_vec(),
            xs,
            y: y.to_vec(),
            ys,
        }
    }

    /// Number of nodes.
    pub fn len(&self) -> usize {
        self.s.len()
    }

    /// Always false: a spline curve has at least two nodes.
    pub fn is_empty(&self) -> bool {
        self.s.is_empty()
    }

    /// Arc length at each node.
    pub fn arc_lengths(&self) -> &[f64] {
        &self.s
    }

    /// Total arc length.
    pub fn length(&self) -> f64 {
        self.s.last().copied().unwrap_or(0.0) - self.s.first().copied().unwrap_or(0.0)
    }

    /// Node x coordinates.
    pub fn x_nodes(&self) -> &[f64] {
        &self.x
    }

    /// Node y coordinates.
    pub fn y_nodes(&self) -> &[f64] {
        &self.y
    }

    /// `dx/ds` at the nodes.
    pub fn dx_nodes(&self) -> &[f64] {
        &self.xs
    }

    /// `dy/ds` at the nodes.
    pub fn dy_nodes(&self) -> &[f64] {
        &self.ys
    }

    /// Point `(x, y)` at arc length `s`.
    pub fn point(&self, s: f64) -> [f64; 2] {
        [self.x_at(s), self.y_at(s)]
    }

    /// `x(s)`.
    pub fn x_at(&self, s: f64) -> f64 {
        seval(s, &self.x, &self.xs, &self.s)
    }

    /// `y(s)`.
    pub fn y_at(&self, s: f64) -> f64 {
        seval(s, &self.y, &self.ys, &self.s)
    }

    /// Tangent `(dx/ds, dy/ds)` at `s`.
    pub fn tangent(&self, s: f64) -> [f64; 2] {
        [
            deval(s, &self.x, &self.xs, &self.s),
            deval(s, &self.y, &self.ys, &self.s),
        ]
    }

    /// Second derivatives `(d²x/ds², d²y/ds²)` at `s`.
    pub fn second_derivative(&self, s: f64) -> [f64; 2] {
        [
            d2val(s, &self.x, &self.xs, &self.s),
            d2val(s, &self.y, &self.ys, &self.s),
        ]
    }

    /// Signed curvature at `s` (XFOIL `CURV`); positive for a counterclockwise
    /// turning contour.
    pub fn curvature(&self, s: f64) -> f64 {
        curv(s, &self.x, &self.xs, &self.y, &self.ys, &self.s)
    }

    /// Finds `s` such that `x(s) = x_target` by Newton iteration from
    /// `s_guess` (XFOIL `SINVRT`). Returns `None` if it does not converge.
    pub fn invert_x(&self, x_target: f64, s_guess: f64) -> Option<f64> {
        sinvrt(s_guess, x_target, &self.x, &self.xs, &self.s)
    }
}

fn check_inputs(s: &[f64], v: &[f64]) -> Result<()> {
    if s.len() != v.len() {
        return Err(GeometryError::LengthMismatch {
            x: s.len(),
            y: v.len(),
        });
    }
    if s.len() < 2 {
        return Err(GeometryError::TooFewPoints {
            found: s.len(),
            min: 2,
        });
    }
    if let Some(index) = s
        .iter()
        .zip(v)
        .position(|(a, b)| !a.is_finite() || !b.is_finite())
    {
        return Err(GeometryError::NonFinite { index });
    }
    Ok(())
}

fn check_segmented(s: &[f64], v: &[f64]) -> Result<()> {
    check_inputs(s, v)?;
    let n = s.len();
    if s.windows(2).any(|w| w[1] < w[0]) {
        return Err(GeometryError::InvalidSpline(
            "abscissas must be non-decreasing".into(),
        ));
    }
    if s[0] == s[1] {
        return Err(GeometryError::DuplicatePoint { index: 1 });
    }
    if s[n - 1] == s[n - 2] {
        return Err(GeometryError::DuplicatePoint { index: n - 1 });
    }
    if let Some(i) = s.windows(3).position(|w| w[0] == w[1] && w[1] == w[2]) {
        return Err(GeometryError::DuplicatePoint { index: i + 2 });
    }
    Ok(())
}

/// Polygonal arc length of a polyline (XFOIL `SCALC`).
pub fn arc_lengths(x: &[f64], y: &[f64]) -> Vec<f64> {
    let mut s = Vec::with_capacity(x.len());
    let mut acc = 0.0;
    for i in 0..x.len().min(y.len()) {
        if i > 0 {
            acc += ((x[i] - x[i - 1]).powi(2) + (y[i] - y[i - 1]).powi(2)).sqrt();
        }
        s.push(acc);
    }
    s
}

/// Solves a tridiagonal system in place (XFOIL `TRISOL`).
///
/// Row `k` reads `b[k]·d[k-1] + a[k]·d[k] + c[k]·d[k+1] = rhs[k]`, with the
/// right-hand side passed in `d` and replaced by the solution. `a` and `c`
/// are destroyed.
pub(crate) fn trisol(a: &mut [f64], b: &[f64], c: &mut [f64], d: &mut [f64]) {
    let n = d.len();
    if n == 0 {
        return;
    }
    for k in 1..n {
        let km = k - 1;
        c[km] /= a[km];
        d[km] /= a[km];
        a[k] -= b[k] * c[km];
        d[k] -= b[k] * d[km];
    }
    d[n - 1] /= a[n - 1];
    for k in (0..n - 1).rev() {
        d[k] -= c[k] * d[k + 1];
    }
}

/// Spline derivatives with end conditions (XFOIL `SPLIND`). Requires
/// `x.len() == s.len() == xs.len() >= 2` and strictly increasing `s`.
pub(crate) fn splind(x: &[f64], s: &[f64], start: EndCondition, end: EndCondition, xs: &mut [f64]) {
    let n = x.len();
    let mut a = vec![0.0; n];
    let mut b = vec![0.0; n];
    let mut c = vec![0.0; n];

    for i in 1..n - 1 {
        let dsm = s[i] - s[i - 1];
        let dsp = s[i + 1] - s[i];
        b[i] = dsp;
        a[i] = 2.0 * (dsm + dsp);
        c[i] = dsm;
        xs[i] = 3.0 * ((x[i + 1] - x[i]) * dsm / dsp + (x[i] - x[i - 1]) * dsp / dsm);
    }

    match start {
        EndCondition::ZeroSecondDerivative => {
            a[0] = 2.0;
            c[0] = 1.0;
            xs[0] = 3.0 * (x[1] - x[0]) / (s[1] - s[0]);
        }
        EndCondition::ZeroThirdDerivative => {
            a[0] = 1.0;
            c[0] = 1.0;
            xs[0] = 2.0 * (x[1] - x[0]) / (s[1] - s[0]);
        }
        EndCondition::FirstDerivative(value) => {
            a[0] = 1.0;
            c[0] = 0.0;
            xs[0] = value;
        }
    }

    let last = n - 1;
    match end {
        EndCondition::ZeroSecondDerivative => {
            b[last] = 1.0;
            a[last] = 2.0;
            xs[last] = 3.0 * (x[last] - x[last - 1]) / (s[last] - s[last - 1]);
        }
        EndCondition::ZeroThirdDerivative => {
            b[last] = 1.0;
            a[last] = 1.0;
            xs[last] = 2.0 * (x[last] - x[last - 1]) / (s[last] - s[last - 1]);
        }
        EndCondition::FirstDerivative(value) => {
            a[last] = 1.0;
            b[last] = 0.0;
            xs[last] = value;
        }
    }

    // Two points with zero third derivative at both ends is singular:
    // fall back to the zero second derivative condition at the end.
    if n == 2
        && start == EndCondition::ZeroThirdDerivative
        && end == EndCondition::ZeroThirdDerivative
    {
        b[last] = 1.0;
        a[last] = 2.0;
        xs[last] = 3.0 * (x[last] - x[last - 1]) / (s[last] - s[last - 1]);
    }

    trisol(&mut a, &b, &mut c, xs);
}

/// Segmented spline derivatives (XFOIL `SEGSPL`). The caller guarantees the
/// invariants checked by `check_segmented`.
pub(crate) fn segspl(x: &[f64], s: &[f64], xs: &mut [f64]) {
    let n = x.len();
    let mut iseg0 = 0;
    for iseg in 1..n.saturating_sub(2) {
        if s[iseg] == s[iseg + 1] {
            splind(
                &x[iseg0..=iseg],
                &s[iseg0..=iseg],
                EndCondition::ZeroThirdDerivative,
                EndCondition::ZeroThirdDerivative,
                &mut xs[iseg0..=iseg],
            );
            iseg0 = iseg + 1;
        }
    }
    splind(
        &x[iseg0..],
        &s[iseg0..],
        EndCondition::ZeroThirdDerivative,
        EndCondition::ZeroThirdDerivative,
        &mut xs[iseg0..],
    );
}

/// Index `i` of the right end of the interval `[s[i-1], s[i]]` used to
/// evaluate the spline at `ss` (binary search of `SEVAL`).
fn interval(s: &[f64], ss: f64) -> usize {
    let mut ilow = 0;
    let mut i = s.len() - 1;
    while i - ilow > 1 {
        let imid = (i + ilow) / 2;
        if ss < s[imid] {
            i = imid;
        } else {
            ilow = imid;
        }
    }
    i
}

/// Hermite coefficients on the interval ending at `i`.
#[inline]
fn coefficients(ss: f64, x: &[f64], xs: &[f64], s: &[f64]) -> (usize, f64, f64, f64, f64) {
    let i = interval(s, ss);
    let ds = s[i] - s[i - 1];
    let t = (ss - s[i - 1]) / ds;
    let cx1 = ds * xs[i - 1] - x[i] + x[i - 1];
    let cx2 = ds * xs[i] - x[i] + x[i - 1];
    (i, ds, t, cx1, cx2)
}

/// XFOIL `SEVAL`.
pub(crate) fn seval(ss: f64, x: &[f64], xs: &[f64], s: &[f64]) -> f64 {
    let (i, _, t, cx1, cx2) = coefficients(ss, x, xs, s);
    t * x[i] + (1.0 - t) * x[i - 1] + (t - t * t) * ((1.0 - t) * cx1 - t * cx2)
}

/// XFOIL `DEVAL`.
pub(crate) fn deval(ss: f64, x: &[f64], xs: &[f64], s: &[f64]) -> f64 {
    let (i, ds, t, cx1, cx2) = coefficients(ss, x, xs, s);
    let d = x[i] - x[i - 1] + (1.0 - 4.0 * t + 3.0 * t * t) * cx1 + t * (3.0 * t - 2.0) * cx2;
    d / ds
}

/// XFOIL `D2VAL`.
pub(crate) fn d2val(ss: f64, x: &[f64], xs: &[f64], s: &[f64]) -> f64 {
    let (_, ds, t, cx1, cx2) = coefficients(ss, x, xs, s);
    ((6.0 * t - 4.0) * cx1 + (6.0 * t - 2.0) * cx2) / (ds * ds)
}

/// XFOIL `CURV`: signed curvature of the splined curve `(x(s), y(s))`.
pub(crate) fn curv(ss: f64, x: &[f64], xs: &[f64], y: &[f64], ys: &[f64], s: &[f64]) -> f64 {
    let (i, ds, t, cx1, cx2) = coefficients(ss, x, xs, s);
    let xd = x[i] - x[i - 1] + (1.0 - 4.0 * t + 3.0 * t * t) * cx1 + t * (3.0 * t - 2.0) * cx2;
    let xdd = (6.0 * t - 4.0) * cx1 + (6.0 * t - 2.0) * cx2;

    let cy1 = ds * ys[i - 1] - y[i] + y[i - 1];
    let cy2 = ds * ys[i] - y[i] + y[i - 1];
    let yd = y[i] - y[i - 1] + (1.0 - 4.0 * t + 3.0 * t * t) * cy1 + t * (3.0 * t - 2.0) * cy2;
    let ydd = (6.0 * t - 4.0) * cy1 + (6.0 * t - 2.0) * cy2;

    let sd = (xd * xd + yd * yd).sqrt().max(0.001 * ds);
    (xd * ydd - yd * xdd) / (sd * sd * sd)
}

/// XFOIL `SINVRT`: Newton inversion of `x(s) = xi` starting from `si`.
pub(crate) fn sinvrt(si: f64, xi: f64, x: &[f64], xs: &[f64], s: &[f64]) -> Option<f64> {
    let span = s[s.len() - 1] - s[0];
    let mut si = si;
    for _ in 0..10 {
        let res = seval(si, x, xs, s) - xi;
        let resp = deval(si, x, xs, s);
        let ds = -res / resp;
        si += ds;
        if !si.is_finite() {
            return None;
        }
        if (ds / span).abs() < 1.0e-5 {
            return Some(si);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn trisol_solves_tridiagonal_system() {
        // [2 1 0; 1 3 1; 0 1 2] x = [3 5 3]  ->  x = [1 1 1]
        let mut a = vec![2.0, 3.0, 2.0];
        let b = vec![0.0, 1.0, 1.0];
        let mut c = vec![1.0, 1.0, 0.0];
        let mut d = vec![3.0, 5.0, 3.0];
        trisol(&mut a, &b, &mut c, &mut d);
        for v in d {
            assert!(close(v, 1.0, 1e-14));
        }
    }

    #[test]
    fn natural_spline_reproduces_linear_function() {
        let s = [0.0, 0.3, 1.1, 2.0, 3.7];
        let v: Vec<f64> = s.iter().map(|t| 2.0 * t - 1.0).collect();
        let sp = Spline::new(
            &s,
            &v,
            EndCondition::ZeroSecondDerivative,
            EndCondition::ZeroSecondDerivative,
        )
        .unwrap();
        for t in [0.0, 0.17, 1.5, 2.9, 3.7] {
            assert!(close(sp.eval(t), 2.0 * t - 1.0, 1e-13));
            assert!(close(sp.deriv(t), 2.0, 1e-13));
            assert!(close(sp.deriv2(t), 0.0, 1e-12));
        }
    }

    #[test]
    fn clamped_spline_reproduces_cubic_exactly() {
        let f = |t: f64| t * t * t - 2.0 * t * t + 0.5 * t + 1.0;
        let df = |t: f64| 3.0 * t * t - 4.0 * t + 0.5;
        let d2f = |t: f64| 6.0 * t - 4.0;
        let s = [0.0, 0.4, 0.9, 1.6, 2.0, 2.5];
        let v: Vec<f64> = s.iter().map(|&t| f(t)).collect();
        let sp = Spline::new(
            &s,
            &v,
            EndCondition::FirstDerivative(df(0.0)),
            EndCondition::FirstDerivative(df(2.5)),
        )
        .unwrap();
        for k in 0..=50 {
            let t = 2.5 * f64::from(k) / 50.0;
            assert!(close(sp.eval(t), f(t), 1e-12), "value at {t}");
            assert!(close(sp.deriv(t), df(t), 1e-11), "slope at {t}");
            assert!(close(sp.deriv2(t), d2f(t), 1e-10), "curvature at {t}");
        }
    }

    #[test]
    fn zero_third_derivative_spline_reproduces_quadratic() {
        // A quadratic has a zero third derivative everywhere.
        let f = |t: f64| 0.5 * t * t - t + 3.0;
        let s = [0.0, 1.0, 1.5, 3.0, 4.2];
        let v: Vec<f64> = s.iter().map(|&t| f(t)).collect();
        let sp = Spline::new(
            &s,
            &v,
            EndCondition::ZeroThirdDerivative,
            EndCondition::ZeroThirdDerivative,
        )
        .unwrap();
        for t in [0.0, 0.5, 2.2, 4.2] {
            assert!(close(sp.eval(t), f(t), 1e-12));
            assert!(close(sp.deriv(t), t - 1.0, 1e-12));
        }
    }

    #[test]
    fn two_point_spline_is_linear() {
        let sp = Spline::new(
            &[0.0, 2.0],
            &[1.0, 5.0],
            EndCondition::ZeroThirdDerivative,
            EndCondition::ZeroThirdDerivative,
        )
        .unwrap();
        assert!(close(sp.eval(1.0), 3.0, 1e-14));
        assert!(close(sp.deriv(0.3), 2.0, 1e-14));
    }

    #[test]
    fn spline_is_c2_continuous_at_knots() {
        let s = [0.0, 0.5, 1.2, 1.9, 3.0];
        let v = [0.0, 0.8, -0.3, 0.4, 1.0];
        let sp = Spline::new(
            &s,
            &v,
            EndCondition::ZeroSecondDerivative,
            EndCondition::ZeroSecondDerivative,
        )
        .unwrap();
        for &k in &s[1..4] {
            let e = 1e-7;
            assert!(close(sp.eval(k - e), sp.eval(k + e), 1e-6));
            assert!(close(sp.deriv(k - e), sp.deriv(k + e), 1e-5));
            assert!(close(sp.deriv2(k - e), sp.deriv2(k + e), 1e-4));
        }
        assert!(close(sp.deriv2(0.0), 0.0, 1e-12));
        assert!(close(sp.deriv2(3.0), 0.0, 1e-12));
    }

    #[test]
    fn spline_interpolates_knots() {
        let s = [0.0, 0.5, 1.2, 1.9, 3.0];
        let v = [0.0, 0.8, -0.3, 0.4, 1.0];
        let sp = Spline::segmented(&s, &v).unwrap();
        for (t, val) in s.iter().zip(v) {
            assert!(close(sp.eval(*t), val, 1e-14));
        }
    }

    #[test]
    fn segmented_spline_allows_slope_jump_at_joint() {
        // |t - 1| sampled with a joint at t = 1.
        let s = [0.0, 0.5, 1.0, 1.0, 1.5, 2.0];
        let v = [1.0, 0.5, 0.0, 0.0, 0.5, 1.0];
        let sp = Spline::segmented(&s, &v).unwrap();
        assert!(close(sp.deriv(0.99), -1.0, 1e-12));
        assert!(close(sp.deriv(1.01), 1.0, 1e-12));
        assert!(close(sp.eval(1.0), 0.0, 1e-14));
    }

    #[test]
    fn segmented_spline_rejects_bad_knots() {
        assert!(Spline::segmented(&[0.0, 0.0, 1.0], &[0.0, 1.0, 2.0]).is_err());
        assert!(Spline::segmented(&[0.0, 1.0, 1.0], &[0.0, 1.0, 2.0]).is_err());
        assert!(Spline::segmented(&[0.0, 1.0, 1.0, 1.0, 2.0], &[0.0; 5]).is_err());
        assert!(Spline::segmented(&[0.0, 2.0, 1.0], &[0.0; 3]).is_err());
        assert!(Spline::segmented(&[0.0, f64::NAN, 1.0], &[0.0; 3]).is_err());
        assert!(Spline::segmented(&[0.0], &[0.0]).is_err());
        assert!(
            Spline::new(
                &[0.0, 1.0, 1.0],
                &[0.0; 3],
                EndCondition::ZeroSecondDerivative,
                EndCondition::ZeroSecondDerivative
            )
            .is_err()
        );
    }

    #[test]
    fn arc_lengths_of_polyline() {
        let s = arc_lengths(&[0.0, 3.0, 3.0], &[0.0, 4.0, 5.0]);
        assert_eq!(s, vec![0.0, 5.0, 6.0]);
    }

    fn circle(n: usize, r: f64) -> (Vec<f64>, Vec<f64>) {
        let mut x = Vec::new();
        let mut y = Vec::new();
        for i in 0..=n {
            let th = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
            x.push(r * th.cos());
            y.push(r * th.sin());
        }
        (x, y)
    }

    #[test]
    fn curve_curvature_of_circle() {
        let (x, y) = circle(400, 2.0);
        let c = SplineCurve::new(&x, &y).unwrap();
        let total = c.length();
        for k in 1..10 {
            let s = total * f64::from(k) / 10.0;
            assert!(
                close(c.curvature(s), 0.5, 1e-4),
                "curvature {}",
                c.curvature(s)
            );
            let [px, py] = c.point(s);
            assert!(close(px.hypot(py), 2.0, 1e-6));
            let [tx, ty] = c.tangent(s);
            assert!(close(tx.hypot(ty), 1.0, 1e-4));
        }
    }

    #[test]
    fn clockwise_circle_has_negative_curvature() {
        let (x, mut y) = circle(200, 1.0);
        y.iter_mut().for_each(|v| *v = -*v);
        let c = SplineCurve::new(&x, &y).unwrap();
        assert!(c.curvature(c.length() / 3.0) < 0.0);
    }

    #[test]
    fn curve_inversion_finds_abscissa() {
        let x: Vec<f64> = (0..20).map(|i| f64::from(i) * 0.1).collect();
        let y: Vec<f64> = x.iter().map(|v| v * v).collect();
        let c = SplineCurve::new(&x, &y).unwrap();
        let s = c.invert_x(0.55, 0.5).unwrap();
        assert!(close(c.x_at(s), 0.55, 1e-7));
        assert!(close(c.y_at(s), 0.3025, 1e-4));
    }

    #[test]
    fn curve_accessors_are_consistent() {
        let (x, y) = circle(16, 1.0);
        let c = SplineCurve::new(&x, &y).unwrap();
        assert_eq!(c.len(), 17);
        assert!(!c.is_empty());
        assert_eq!(c.x_nodes(), &x[..]);
        assert_eq!(c.y_nodes(), &y[..]);
        assert_eq!(c.dx_nodes().len(), 17);
        assert_eq!(c.dy_nodes().len(), 17);
        assert_eq!(c.arc_lengths()[0], 0.0);
        let [ax, ay] = c.second_derivative(0.3);
        assert!(ax.is_finite() && ay.is_finite());
        assert!(SplineCurve::new(&[0.0, 1.0], &[0.0]).is_err());
    }

    #[test]
    fn spline_accessors() {
        let sp = Spline::segmented(&[0.0, 1.0, 2.0], &[1.0, 2.0, 0.0]).unwrap();
        assert_eq!(sp.knots(), &[0.0, 1.0, 2.0]);
        assert_eq!(sp.values(), &[1.0, 2.0, 0.0]);
        assert_eq!(sp.derivatives().len(), 3);
    }
}
