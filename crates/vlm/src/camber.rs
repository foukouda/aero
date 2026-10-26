//! Camber lines of the sections: NACA 4-digit formulas and airfoil
//! coordinates (AVL `NACA`, `AIRFOIL`/`AFILE` with `GETCAM`), and the Akima
//! interpolation of their slopes (AVL `AKIMA`).

use serde::{Deserialize, Serialize};

use crate::error::{Result, VlmError};

/// Number of camber points AVL keeps per section (`MIN(50, IBX)`).
const CAMBER_POINTS: usize = 50;

/// The camber line of a section as slopes `dz/dx` at chord fractions from 0
/// to 1 (AVL `XASEC`, `SASEC`); only the slopes enter the vortex lattice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Camber {
    x: Vec<f64>,
    slope: Vec<f64>,
}

impl Default for Camber {
    fn default() -> Self {
        Self::flat()
    }
}

impl Camber {
    /// A flat plate (AVL's default section).
    pub fn flat() -> Self {
        Self {
            x: vec![0.0, 1.0],
            slope: vec![0.0, 0.0],
        }
    }

    /// Slopes given directly at increasing chord fractions; the fractions
    /// are rescaled to run from 0 to 1.
    pub fn from_slopes(x: Vec<f64>, slope: Vec<f64>) -> Result<Self> {
        let bad = |reason: &str| VlmError::Airfoil {
            name: "camber slopes".into(),
            reason: reason.into(),
        };
        if x.len() < 2 || x.len() != slope.len() {
            return Err(bad("needs at least two (x, slope) pairs"));
        }
        if x.iter().chain(&slope).any(|v| !v.is_finite()) {
            return Err(bad("values must be finite"));
        }
        if x.windows(2).any(|w| w[1] <= w[0]) {
            return Err(bad("chord fractions must increase"));
        }
        let mut x = x;
        normalize(&mut x);
        Ok(Self { x, slope })
    }

    /// The camber line of the NACA 4-digit airfoil `designation` (thickness
    /// ignored), sampled over the chord fractions `range` (AVL `NACA x1 x2`;
    /// `(0, 1)` is the whole airfoil).
    pub fn naca4(designation: u32, range: (f64, f64)) -> Result<Self> {
        if designation > 9999 {
            return Err(VlmError::Airfoil {
                name: format!("NACA {designation}"),
                reason: "a NACA 4-digit designation has four digits".into(),
            });
        }
        let (xmin, xmax) = clamp_range(range);
        let icam = designation / 1000;
        let ipos = (designation - 1000 * icam) / 100;
        let c = f64::from(icam) / 100.0;
        let p = f64::from(ipos) / 10.0;
        let n = CAMBER_POINTS;
        let mut x = Vec::with_capacity(n);
        let mut slope = Vec::with_capacity(n);
        for i in 0..n {
            let xf = xmin + (xmax - xmin) * i as f64 / (n - 1) as f64;
            let s = if xf < p {
                c * 2.0 * (p - xf) / (p * p)
            } else if xf > p {
                c * 2.0 * (p - xf) / ((1.0 - p) * (1.0 - p))
            } else {
                0.0
            };
            x.push(xf);
            slope.push(s);
        }
        normalize(&mut x);
        Ok(Self { x, slope })
    }

    /// The camber line of an airfoil given by its coordinates, from the
    /// trailing edge over the upper side to the leading edge and back along
    /// the lower side (AVL `AIRFOIL`, `GETCAM`), sampled over the chord
    /// fractions `range`. The coordinates are normalized to a unit chord
    /// from the leftmost point; their order is used as given.
    pub fn from_airfoil(name: &str, x: &[f64], y: &[f64], range: (f64, f64)) -> Result<Self> {
        let bad = |reason: String| VlmError::Airfoil {
            name: name.to_owned(),
            reason,
        };
        if x.len() != y.len() || x.len() < 5 {
            return Err(bad("needs at least five points".into()));
        }
        if x.iter().chain(y).any(|v| !v.is_finite()) {
            return Err(bad("coordinates must be finite".into()));
        }
        let (xc, yc) = camber_line(x, y, CAMBER_POINTS).map_err(bad)?;
        let (xmin, xmax) = clamp_range(range);
        let n = xc.len();
        let mut xs = Vec::with_capacity(n);
        let mut slope = Vec::with_capacity(n);
        for i in 0..n {
            let xf = xmin + (xmax - xmin) * i as f64 / (n - 1) as f64;
            let xa = xc[0] + xf * (xc[n - 1] - xc[0]);
            xs.push(xa);
            slope.push(akima(&xc, &yc, xa).1);
        }
        if slope.iter().any(|s| !s.is_finite()) {
            return Err(bad("the camber line could not be found".into()));
        }
        normalize(&mut xs);
        Ok(Self { x: xs, slope })
    }

    /// Chord fractions of the slope table, from 0 to 1.
    pub fn x(&self) -> &[f64] {
        &self.x
    }

    /// Camber slopes `dz/dx` at [`x`](Self::x).
    pub fn slopes(&self) -> &[f64] {
        &self.slope
    }

    /// Camber slope at the chord fraction `x` (Akima interpolation).
    pub fn slope_at(&self, x: f64) -> f64 {
        akima(&self.x, &self.slope, x).0
    }
}

/// `(x1, x2)` clipped to `[0, 1]` as AVL does.
fn clamp_range((a, b): (f64, f64)) -> (f64, f64) {
    (a.max(0.0), b.min(1.0))
}

/// AVL `NRMLIZ`: rescales increasing values to run from 0 to 1.
fn normalize(x: &mut [f64]) {
    let n = x.len();
    if n <= 1 {
        return;
    }
    let mut dx = x[n - 1] - x[0];
    if dx == 0.0 {
        dx = 1.0;
    }
    let x1 = x[0];
    for v in x.iter_mut() {
        *v = (*v - x1) / dx;
    }
}

/// AVL `AKIMA`: value and slope at `xx` of the Akima spline through
/// `(x, y)` (abscissas increasing or decreasing). Beyond the data it
/// extrapolates with a quadratic through the end points.
pub(crate) fn akima(x: &[f64], y: &[f64], xx: f64) -> (f64, f64) {
    let n = x.len();
    // 1-based accessors, as in the Fortran.
    let xa = |k: usize| x[k - 1];
    let ya = |k: usize| y[k - 1];
    if n == 0 {
        return (0.0, 0.0);
    }
    if xa(1) == xa(n) {
        return (ya(1), 0.0);
    }

    // Binary search: I points to the data point below xx (above for
    // decreasing abscissas).
    let xordr = if xa(1) > xa(n) { -1.0 } else { 1.0 };
    let (mut ibot, mut itop) = (1usize, n);
    let xxo = xx * xordr;
    let mut i;
    loop {
        let nstep = (itop - ibot) / 2;
        i = ibot + nstep;
        let xo = xa(i) * xordr;
        if xxo >= xo {
            ibot = i;
        }
        if xxo < xo {
            itop = i;
        }
        if nstep == 0 {
            break;
        }
    }

    // Slopes of the five segments around the interval [I, I+1].
    let mut d = [0.0_f64; 6]; // d[1..=5]
    for (j, dj) in d.iter_mut().enumerate().skip(1) {
        let k = i as isize + j as isize - 2;
        if k > 1 && k <= n as isize {
            let k = k as usize;
            *dj = (ya(k) - ya(k - 1)) / (xa(k) - xa(k - 1));
        }
    }
    if n == 2 {
        d[2] = d[3];
    }
    if i + 2 > n {
        d[4] = 2.0 * d[3] - d[2];
    }
    if i + 3 > n {
        d[5] = 2.0 * d[4] - d[3];
    }
    if i < 2 {
        d[2] = 2.0 * d[3] - d[4];
    }
    if i < 3 {
        d[1] = 2.0 * d[2] - d[3];
    }

    // Slopes at the ends of the interval.
    let mut t = [0.0_f64; 3]; // t[1..=2]
    for (j, tj) in t.iter_mut().enumerate().skip(1) {
        let mut a = (d[j + 3] - d[j + 2]).abs();
        let mut b = (d[j + 1] - d[j]).abs();
        if a + b == 0.0 {
            a = 1.0;
            b = 1.0;
        }
        *tj = (a * d[j + 1] + b * d[j + 2]) / (a + b);
    }

    if xx == xa(i + 1) {
        return (ya(i + 1), t[2]);
    }
    let xint = xa(i + 1) - xa(i);
    let xdif = xx - xa(i);
    let p0 = ya(i);
    let p1 = t[1];
    let p2 = (3.0 * d[3] - 2.0 * t[1] - t[2]) / xint;
    let p3 = (t[1] + t[2] - 2.0 * d[3]) / (xint * xint);
    let yy = p0 + xdif * (p1 + xdif * (p2 + xdif * p3));
    let slp = p1 + xdif * (2.0 * p2 + xdif * (3.0 * p3));
    (yy, slp)
}

/// AVL `GETCAM` with `LNORM`: the camber line of an airfoil at `nc` points
/// spaced as cosines from the leading edge (leftmost point) to the
/// trailing edge, normalized to a unit chord.
fn camber_line(
    x: &[f64],
    y: &[f64],
    nc: usize,
) -> std::result::Result<(Vec<f64>, Vec<f64>), String> {
    let n = x.len();
    let mut x = x.to_vec();
    let mut y = y.to_vec();
    let mut s = arc_lengths(&x, &y);
    let xp = segment_spline(&x, &s)?;
    let yp = segment_spline(&y, &s)?;

    let mut sle = leftmost(&x, &xp, &s);

    // NORMIT: unit chord from the leading edge (the spline derivatives are
    // unchanged, x and s being scaled alike).
    let xle = seval(sle, &x, &xp, &s);
    let xte = 0.5 * (x[0] + x[n - 1]);
    let dnorm = 1.0 / (xte - xle);
    if !dnorm.is_finite() {
        return Err("zero chord".into());
    }
    for i in 0..n {
        x[i] = (x[i] - xle) * dnorm;
        y[i] *= dnorm;
        s[i] *= dnorm;
    }
    sle *= dnorm;

    let xle = seval(sle, &x, &xp, &s);
    let yle = seval(sle, &y, &yp, &s);
    let xte = 0.5 * (x[0] + x[n - 1]);

    let mut su = sle - 0.01;
    let mut sl = sle + 0.01;
    let fnc1 = (nc - 1) as f64;
    let mut xc = vec![xle];
    let mut yc = vec![yle];
    for i in 2..=nc {
        let xout =
            xle + (xte - xle) * 0.5 * (1.0 - (std::f64::consts::PI * (i - 1) as f64 / fnc1).cos());
        sinvrt(&mut su, xout, &x, &xp, &s);
        let yu = seval(su, &y, &yp, &s);
        sinvrt(&mut sl, xout, &x, &xp, &s);
        let yl = seval(sl, &y, &yp, &s);
        xc.push(xout);
        yc.push(0.5 * (yu + yl));
    }
    Ok((xc, yc))
}

/// AVL `SCALC`: arc lengths along the points.
fn arc_lengths(x: &[f64], y: &[f64]) -> Vec<f64> {
    let mut s = vec![0.0; x.len()];
    for i in 1..x.len() {
        s[i] = s[i - 1] + ((x[i] - x[i - 1]).powi(2) + (y[i] - y[i - 1]).powi(2)).sqrt();
    }
    s
}

/// AVL `SEGSPL`: spline derivatives `dv/ds`, with zero third derivatives at
/// the ends of each segment; segments are separated by repeated points.
fn segment_spline(v: &[f64], s: &[f64]) -> std::result::Result<Vec<f64>, String> {
    let n = v.len();
    if s[0] == s[1] || s[n - 1] == s[n - 2] {
        return Err("first or last point duplicated".into());
    }
    let mut vs = vec![0.0; n];
    let mut iseg0 = 0;
    for iseg in 1..n - 2 {
        if s[iseg] == s[iseg + 1] {
            splind(&v[iseg0..=iseg], &mut vs[iseg0..=iseg], &s[iseg0..=iseg]);
            iseg0 = iseg + 1;
        }
    }
    splind(&v[iseg0..], &mut vs[iseg0..], &s[iseg0..]);
    Ok(vs)
}

/// AVL `SPLIND` with zero third derivatives at both ends (`-999`).
fn splind(x: &[f64], xs: &mut [f64], s: &[f64]) {
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
    a[0] = 1.0;
    c[0] = 1.0;
    xs[0] = 2.0 * (x[1] - x[0]) / (s[1] - s[0]);
    b[n - 1] = 1.0;
    a[n - 1] = 1.0;
    xs[n - 1] = 2.0 * (x[n - 1] - x[n - 2]) / (s[n - 1] - s[n - 2]);
    if n == 2 {
        // Zero second derivative at the right end instead.
        b[n - 1] = 1.0;
        a[n - 1] = 2.0;
        xs[n - 1] = 3.0 * (x[n - 1] - x[n - 2]) / (s[n - 1] - s[n - 2]);
    }
    // TRISOL
    for k in 1..n {
        let km = k - 1;
        c[km] /= a[km];
        xs[km] /= a[km];
        a[k] -= b[k] * c[km];
        xs[k] -= b[k] * xs[km];
    }
    xs[n - 1] /= a[n - 1];
    for k in (0..n - 1).rev() {
        xs[k] -= c[k] * xs[k + 1];
    }
}

/// Interval `[i-1, i]` (0-based) of `s` containing `ss`, as AVL's binary
/// search finds it.
fn interval(ss: f64, s: &[f64]) -> usize {
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

/// AVL `SEVAL`.
fn seval(ss: f64, x: &[f64], xs: &[f64], s: &[f64]) -> f64 {
    let i = interval(ss, s);
    let ds = s[i] - s[i - 1];
    let t = (ss - s[i - 1]) / ds;
    let cx1 = ds * xs[i - 1] - x[i] + x[i - 1];
    let cx2 = ds * xs[i] - x[i] + x[i - 1];
    t * x[i] + (1.0 - t) * x[i - 1] + (t - t * t) * ((1.0 - t) * cx1 - t * cx2)
}

/// AVL `DEVAL`.
fn deval(ss: f64, x: &[f64], xs: &[f64], s: &[f64]) -> f64 {
    let i = interval(ss, s);
    let ds = s[i] - s[i - 1];
    let t = (ss - s[i - 1]) / ds;
    let cx1 = ds * xs[i - 1] - x[i] + x[i - 1];
    let cx2 = ds * xs[i] - x[i] + x[i - 1];
    (x[i] - x[i - 1] + (1.0 - 4.0 * t + 3.0 * t * t) * cx1 + t * (3.0 * t - 2.0) * cx2) / ds
}

/// AVL `D2VAL`.
fn d2val(ss: f64, x: &[f64], xs: &[f64], s: &[f64]) -> f64 {
    let i = interval(ss, s);
    let ds = s[i] - s[i - 1];
    let t = (ss - s[i - 1]) / ds;
    let cx1 = ds * xs[i - 1] - x[i] + x[i - 1];
    let cx2 = ds * xs[i] - x[i] + x[i - 1];
    ((6.0 * t - 4.0) * cx1 + (6.0 * t - 2.0) * cx2) / (ds * ds)
}

/// AVL `SINVRT`: Newton iteration for `s` such that `x(s) = xi`, from the
/// guess in `si`.
fn sinvrt(si: &mut f64, xi: f64, x: &[f64], xs: &[f64], s: &[f64]) {
    let length = s[s.len() - 1] - s[0];
    for _ in 0..10 {
        let res = seval(*si, x, xs, s) - xi;
        let resp = deval(*si, x, xs, s);
        let ds = -res / resp;
        *si += ds;
        if (ds / length).abs() < 1e-5 {
            return;
        }
    }
}

/// AVL `LEFIND`: arc length of the leftmost point of the airfoil.
fn leftmost(x: &[f64], xp: &[f64], s: &[f64]) -> f64 {
    let n = x.len();
    let mut i = 1;
    while i < n && x[i] <= x[i - 1] {
        i += 1;
    }
    let guess = s[(i - 1).min(n - 1)];
    let sref = s[n - 1] - s[0];
    let mut sle = guess;
    for _ in 0..20 {
        let res = deval(sle, x, xp, s);
        let resp = d2val(sle, x, xp, s);
        let dsle = -res / resp;
        sle += dsle;
        if (dsle.abs() / sref) < 1e-5 {
            return sle;
        }
    }
    guess
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn akima_reproduces_lines_and_parabolas() {
        let x = [0.0, 1.0, 2.0, 3.0];
        let line: Vec<f64> = x.iter().map(|v| 2.0 * v + 1.0).collect();
        let (y, s) = akima(&x, &line, 1.7);
        assert!((y - 4.4).abs() < 1e-14 && (s - 2.0).abs() < 1e-14);
        // Decreasing abscissas give the same curve.
        let xr: Vec<f64> = x.iter().rev().copied().collect();
        let lr: Vec<f64> = line.iter().rev().copied().collect();
        let (yr, sr) = akima(&xr, &lr, 1.7);
        assert!((yr - y).abs() < 1e-14 && (sr - s).abs() < 1e-14);
        // At a data point: the data value.
        assert_eq!(akima(&x, &line, 2.0).0, 5.0);
        // Two points: a straight line, extrapolated.
        let (y2, s2) = akima(&[0.0, 1.0], &[1.0, 3.0], 1.5);
        assert!((y2 - 4.0).abs() < 1e-14 && (s2 - 2.0).abs() < 1e-14);
    }

    #[test]
    fn naca_camber_slopes() {
        let c = Camber::naca4(2412, (0.0, 1.0)).unwrap();
        assert_eq!(c.x().len(), 50);
        // dz/dx = 2m(p − x)/p² ahead of the maximum camber: 0.1 at the nose.
        assert!((c.slopes()[0] - 0.1).abs() < 1e-15);
        // Behind it: 2m(p − x)/(1 − p)² = −0.0667 at the trailing edge.
        assert!((c.slopes()[49] + 2.0 * 0.02 * 0.6 / 0.36).abs() < 1e-15);
        assert!(c.slope_at(0.4).abs() < 2e-3);
        let sym = Camber::naca4(12, (0.0, 1.0)).unwrap();
        assert!(sym.slopes().iter().all(|s| *s == 0.0));
        assert!(Camber::naca4(12345, (0.0, 1.0)).is_err());
    }

    #[test]
    fn camber_of_a_naca_airfoil_from_its_coordinates() {
        // A thin NACA 4401 (TE → upper → LE → lower → TE): the camber line
        // found from its coordinates (the mean of both sides at the same x)
        // has the NACA camber slopes; for thick airfoils the two definitions
        // of the camber line differ near the nose.
        let (m, p, t) = (0.04, 0.4, 0.01);
        let yt = |x: f64| {
            5.0 * t
                * (0.2969 * x.sqrt() - 0.1260 * x - 0.3516 * x * x + 0.2843 * x.powi(3)
                    - 0.1036 * x.powi(4))
        };
        let yc = |x: f64| {
            if x < p {
                m / (p * p) * (2.0 * p * x - x * x)
            } else {
                m / ((1.0 - p) * (1.0 - p)) * ((1.0 - 2.0 * p) + 2.0 * p * x - x * x)
            }
        };
        let dyc = |x: f64| {
            if x < p {
                2.0 * m / (p * p) * (p - x)
            } else {
                2.0 * m / ((1.0 - p) * (1.0 - p)) * (p - x)
            }
        };
        let n = 120;
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        for k in (0..=n).rev() {
            let x = 0.5 * (1.0 - (std::f64::consts::PI * k as f64 / n as f64).cos());
            let th = dyc(x).atan();
            xs.push(x - yt(x) * th.sin());
            ys.push(yc(x) + yt(x) * th.cos());
        }
        for k in 1..=n {
            let x = 0.5 * (1.0 - (std::f64::consts::PI * k as f64 / n as f64).cos());
            let th = dyc(x).atan();
            xs.push(x + yt(x) * th.sin());
            ys.push(yc(x) - yt(x) * th.cos());
        }
        let c = Camber::from_airfoil("4401", &xs, &ys, (0.0, 1.0)).unwrap();
        for x in [0.1, 0.25, 0.6, 0.8] {
            assert!(
                (c.slope_at(x) - dyc(x)).abs() < 2e-3,
                "x {x}: {} vs {}",
                c.slope_at(x),
                dyc(x)
            );
        }
    }
}
