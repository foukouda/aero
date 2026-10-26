//! Flap hinge moment from the surface pressures (XFOIL `MHINGE`, `GETXYF`).

use aero_geometry::spline::{deval, seval};
use serde::{Deserialize, Serialize};

use super::panel::PanelGeometry;

/// Moment and force on the part of the airfoil aft of a flap hinge, per
/// unit span, divided by `½ρV²c²` (moment) and `½ρV²c` (forces), as XFOIL's
/// `FMOM` prints them.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HingeMoment {
    /// Hinge moment coefficient (XFOIL `HMOM`), positive when it tends to
    /// raise the flap trailing edge (counterclockwise in airfoil
    /// coordinates).
    pub moment: f64,
    /// x force on the flap (XFOIL `HFX`).
    pub fx: f64,
    /// y force on the flap (XFOIL `HFY`).
    pub fy: f64,
}

/// XFOIL `SINVRT` on the panel spline: `x(s) = xi` from `si`, the input
/// value if the Newton iteration does not converge.
fn sinvrt(si: f64, xi: f64, x: &[f64], xs: &[f64], s: &[f64]) -> f64 {
    let span = s[s.len() - 1] - s[0];
    let mut s_new = si;
    for _ in 0..10 {
        let res = seval(s_new, x, xs, s) - xi;
        let ds = -res / deval(s_new, x, xs, s);
        s_new += ds;
        if (ds / span).abs() < 1.0e-5 {
            return s_new;
        }
    }
    si
}

/// XFOIL `MHINGE`: integrates the pressure coefficients `cp` at the panel
/// nodes over the surfaces aft of the hinge `(xh, yh)`, closing the flap
/// contour through the hinge and across the trailing-edge base.
///
/// As in XFOIL, the panels that straddle the hinge are counted whole and
/// their parts between the node aft of the hinge and the hinge point are
/// added again; the result is XFOIL's to a discretization error.
pub(crate) fn hinge_moment(geo: &PanelGeometry, cp: &[f64], hinge: [f64; 2]) -> HingeMoment {
    let (x, y, s, xp, yp) = (&geo.x, &geo.y, &geo.s, &geo.xp, &geo.yp);
    let n = x.len();
    let [xof, yof] = hinge;

    // Top and bottom arc lengths at the hinge x (GETXYF).
    let tops = sinvrt(s[0] + (x[0] - xof), xof, x, xp, s);
    let bots = sinvrt(s[n - 1] - (x[n - 1] - xof), xof, x, xp, s);
    let topx = seval(tops, x, xp, s);
    let topy = seval(tops, y, yp, s);
    let botx = seval(bots, x, xp, s);
    let boty = seval(bots, y, yp, s);

    let mut h = HingeMoment {
        moment: 0.0,
        fx: 0.0,
        fy: 0.0,
    };
    let mut add = |pmid: f64, (xa, ya): (f64, f64), (xb, yb): (f64, f64)| {
        let (dx, dy) = (xb - xa, yb - ya);
        let xmid = 0.5 * (xa + xb) - xof;
        let ymid = 0.5 * (ya + yb) - yof;
        h.moment += pmid * (xmid * dx + ymid * dy);
        h.fx -= pmid * dy;
        h.fy += pmid * dx;
    };

    // Pressures on the top and bottom surfaces of the flap.
    for i in 1..n {
        if s[i - 1] >= tops && s[i] <= bots {
            continue;
        }
        add(
            0.5 * (cp[i] + cp[i - 1]),
            (x[i - 1], y[i - 1]),
            (x[i], y[i]),
        );
    }

    // Top surface chunk TOPS..S(I-1) missed above.
    let i = (1..n).find(|&i| s[i] > tops).unwrap_or(n - 1);
    let frac = if s[i] != s[i - 1] {
        (tops - s[i - 1]) / (s[i] - s[i - 1])
    } else {
        0.0
    };
    let topp = cp[i] * frac + cp[i - 1] * (1.0 - frac);
    let pmid = 0.5 * (topp + cp[i - 1]);
    add(pmid, (x[i - 1], y[i - 1]), (topx, topy));
    // Inside of the flap, from the top surface to the hinge.
    add(pmid, (topx, topy), (xof, yof));

    // Bottom surface chunk BOTS..S(I) missed above.
    let i = (1..n).rev().find(|&i| s[i - 1] < bots).unwrap_or(1);
    let frac = if s[i] != s[i - 1] {
        (bots - s[i - 1]) / (s[i] - s[i - 1])
    } else {
        0.0
    };
    let botp = cp[i] * frac + cp[i - 1] * (1.0 - frac);
    let pmid = 0.5 * (botp + cp[i]);
    add(pmid, (botx, boty), (x[i], y[i]));
    // Inside of the flap, from the hinge to the bottom surface.
    add(pmid, (xof, yof), (botx, boty));

    // Trailing-edge base.
    add(
        0.5 * (cp[0] + cp[n - 1]),
        (x[n - 1], y[n - 1]),
        (x[0], y[0]),
    );
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use aero_geometry::{NacaOptions, PanelingOptions, naca};

    fn geometry() -> PanelGeometry {
        let af = naca("0012", &NacaOptions::default())
            .unwrap()
            .paneled(&PanelingOptions::default())
            .unwrap();
        PanelGeometry::new(&af).unwrap()
    }

    #[test]
    fn suction_on_top_lifts_the_flap() {
        // Cp = -1 on the top surface only: the flap is pulled up (positive
        // y force about the 0.25 chord aft of the hinge), raising its
        // trailing edge (positive moment, lever about 0.125).
        let geo = geometry();
        let cp: Vec<f64> = geo
            .y
            .iter()
            .map(|&y| if y > 0.0 { -1.0 } else { 0.0 })
            .collect();
        let h = hinge_moment(&geo, &cp, [0.75, 0.0]);
        assert!(h.fy > 0.24 && h.fy < 0.28, "{h:?}");
        assert!(h.fx.abs() < 0.01, "{h:?}");
        assert!((h.moment - h.fy * 0.125).abs() < 0.004, "{h:?}");
    }
}
