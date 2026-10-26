//! Modifications of an airfoil contour, ported from XFOIL's geometry design
//! menu (`GDES`) and its `INTE` command:
//!
//! | Function | XFOIL | Routines |
//! |---|---|---|
//! | [`deflect_flap`], [`hinge_point`] | `FLAP` | `FLAP`, `GETXYF`, `SSS`, `INSIDE`, `SCHECK` |
//! | [`set_te_gap`] | `TGAP` | `TGAP` |
//! | [`scale_le_radius`] | `LERA` | `LERAD`, `LERSCL` |
//! | [`scale_thickness_camber`] | `TFAC` | `TCSCAL`, `THKCAM` |
//! | [`set_thickness_camber`] | `TSET` | `TCSET`, `THKCAM` |
//! | [`move_high_points`] | `HIGH` | `HIPNT` |
//! | [`camber_thickness`] | — | `GETCAM`, `GETMAX`, `XLFIND`, `SORTOL` |
//! | [`interpolate`] | `INTE` | `INTE`, `INTER` |
//!
//! Like XFOIL's commands, they act on the contour as given (XFOIL's buffer
//! airfoil, before paneling) and keep its point distribution, apart from
//! the points that the flap deflection adds or removes near its breaks.

use serde::{Deserialize, Serialize};

use crate::airfoil::Airfoil;
use crate::error::{GeometryError, Result};
use crate::leading_edge::lefind;
use crate::properties::{ChordFrame, sopps};
use crate::spline::{SplineCurve, d2val, deval, segspl, seval};

fn invalid(message: impl Into<String>) -> GeometryError {
    GeometryError::InvalidOption(message.into())
}

fn finite(value: f64, what: &str) -> Result<()> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(invalid(format!("{what} must be finite")))
    }
}

/// Builds the modified airfoil, keeping the original name.
fn rebuild(original: &Airfoil, x: Vec<f64>, y: Vec<f64>) -> Result<Airfoil> {
    Airfoil::new(original.name(), x, y)
}

// ------------------------------------------------------------------ flap

/// Vertical position of a flap hinge.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum HingeY {
    /// Ordinate `y`.
    Absolute(f64),
    /// Fraction of the local thickness: 0 on the lower surface, 1 on the
    /// upper one (XFOIL's `y/t`, entered as 999 then the fraction).
    Relative(f64),
}

/// Arc lengths and ordinates of the upper and lower surface points at
/// abscissa `xf` (XFOIL `GETXYF`). `SINVRT` failures keep the initial
/// guess, as XFOIL does.
fn surface_points_at(curve: &SplineCurve, xf: f64) -> (f64, f64, f64, f64) {
    let s = curve.arc_lengths();
    let x = curve.x_nodes();
    let n = s.len() - 1;
    let tops0 = s[0] + (x[0] - xf);
    let bots0 = s[n] - (x[n] - xf);
    let tops = curve.invert_x(xf, tops0).unwrap_or(tops0);
    let bots = curve.invert_x(xf, bots0).unwrap_or(bots0);
    (tops, bots, curve.y_at(tops), curve.y_at(bots))
}

/// The hinge point `(x, y)` of a flap hinged at abscissa `x` (XFOIL
/// `GETXYF`).
pub fn hinge_point(airfoil: &Airfoil, x: f64, y: HingeY) -> Result<[f64; 2]> {
    finite(x, "the hinge abscissa")?;
    let curve = airfoil.spline();
    let (_, _, top_y, bot_y) = surface_points_at(&curve, x);
    let yf = match y {
        HingeY::Absolute(v) => v,
        HingeY::Relative(r) => top_y * r + bot_y * (1.0 - r),
    };
    finite(yf, "the hinge ordinate")?;
    Ok([x, yf])
}

/// XFOIL `INSIDE`: true if `(xf, yf)` is inside the contour (sum of the
/// sines of the subtended angles, as XFOIL computes it).
fn inside(x: &[f64], y: &[f64], xf: f64, yf: f64) -> bool {
    let n = x.len();
    let mut angle = 0.0;
    for i in 0..n {
        let ip = if i == n - 1 { 0 } else { i + 1 };
        let (xb1, yb1) = (x[i] - xf, y[i] - yf);
        let (xb2, yb2) = (x[ip] - xf, y[ip] - yf);
        angle +=
            (xb1 * yb2 - yb1 * xb2) / ((xb1 * xb1 + yb1 * yb1) * (xb2 * xb2 + yb2 * yb2)).sqrt();
    }
    angle.abs() > 1.0
}

/// XFOIL `SSS`: arc lengths `(s1, s2)` of the surface break points of a
/// flap deflected by `del` about `(xbf, ybf)`, near `ss`; `s1` on the fixed
/// part, `s2` on the flap. `side` is 1 for the upper surface, 2 for the
/// lower one.
#[allow(clippy::too_many_arguments)]
fn break_points(
    curve: &SplineCurve,
    ss: f64,
    del: f64,
    xbf: f64,
    ybf: f64,
    side: u8,
) -> (f64, f64) {
    const EPS: f64 = 1.0e-5;
    let s = curve.arc_lengths();
    let (x, xp, y, yp) = (
        curve.x_nodes(),
        curve.dx_nodes(),
        curve.y_nodes(),
        curve.dy_nodes(),
    );
    let stot = (s[s.len() - 1] - s[0]).abs();
    let sind = (0.5 * del.abs()).sin();
    let ssgn = if side == 1 { -1.0 } else { 1.0 };

    let rsq = (seval(ss, x, xp, s) - xbf).powi(2) + (seval(ss, y, yp, s) - ybf).powi(2);
    let mut s1 = ss - (sind * rsq.sqrt() + EPS * stot) * ssgn;
    let mut s2 = ss + (sind * rsq.sqrt() + EPS * stot) * ssgn;

    let mut converged = false;
    for _ in 0..10 {
        let x1 = seval(s1, x, xp, s);
        let x1p = deval(s1, x, xp, s);
        let y1 = seval(s1, y, yp, s);
        let y1p = deval(s1, y, yp, s);
        let x2 = seval(s2, x, xp, s);
        let x2p = deval(s2, x, xp, s);
        let y2 = seval(s2, y, yp, s);
        let y2p = deval(s2, y, yp, s);

        let r1 = ((x1 - xbf).powi(2) + (y1 - ybf).powi(2)).sqrt();
        let r2 = ((x2 - xbf).powi(2) + (y2 - ybf).powi(2)).sqrt();
        let rrsq = (x1 - x2).powi(2) + (y1 - y2).powi(2);
        let rr = rrsq.sqrt();

        if r1 <= EPS * stot || r2 <= EPS * stot {
            return (ss, ss);
        }
        let r1_s1 = (x1p * (x1 - xbf) + y1p * (y1 - ybf)) / r1;
        let r2_s2 = (x2p * (x2 - xbf) + y2p * (y2 - ybf)) / r2;

        let (rs1, a11, a12, rs2, a21, a22);
        if sind > 0.01 {
            if rr == 0.0 {
                return (s1, s2);
            }
            let rr_s1 = (x1p * (x1 - x2) + y1p * (y1 - y2)) / rr;
            let rr_s2 = -(x2p * (x1 - x2) + y2p * (y1 - y2)) / rr;
            // Included angle via the dot product.
            let dot = (xbf - x1) * (x2 - x1) + (ybf - y1) * (y2 - y1);
            rs1 = dot / rr - sind * r1;
            a11 = ((xbf - x1) * (-x1p) + (ybf - y1) * (-y1p)) / rr
                + ((-x1p) * (x2 - x1) + (-y1p) * (y2 - y1)) / rr
                - dot * rr_s1 / rrsq
                - sind * r1_s1;
            a12 = ((xbf - x1) * x2p + (ybf - y1) * y2p) / rr - dot * rr_s2 / rrsq;
            // Equal length segments.
            rs2 = r1 - r2;
            a21 = r1_s1;
            a22 = -r2_s2;
        } else {
            // Small angle approximation.
            rs1 = (r1 + r2) * sind + (s1 - s2) * ssgn;
            a11 = r1_s1 * sind + ssgn;
            a12 = r2_s2 * sind - ssgn;
            // Sum of the segments to the hinge normal to the surface.
            let x1pp = d2val(s1, x, xp, s);
            let y1pp = d2val(s1, y, yp, s);
            let x2pp = d2val(s2, x, xp, s);
            let y2pp = d2val(s2, y, yp, s);
            let xtot = x1 + x2 - 2.0 * xbf;
            let ytot = y1 + y2 - 2.0 * ybf;
            rs2 = xtot * (x1p + x2p) + ytot * (y1p + y2p);
            a21 = x1p * (x1p + x2p) + y1p * (y1p + y2p) + xtot * x1pp + ytot * y1pp;
            a22 = x2p * (x1p + x2p) + y2p * (y1p + y2p) + xtot * x2pp + ytot * y2pp;
        }
        let det = a11 * a22 - a12 * a21;
        let ds1 = (-(rs1 * a22 - a12 * rs2) / det).clamp(-0.01 * stot, 0.01 * stot);
        let ds2 = (-(a11 * rs2 - rs1 * a21) / det).clamp(-0.01 * stot, 0.01 * stot);
        s1 += ds1;
        s2 += ds2;
        if ds1.abs() + ds2.abs() < EPS * stot {
            converged = true;
            break;
        }
    }
    if !converged {
        s1 = ss;
        s2 = ss;
    }
    if del == 0.0 {
        s1 = 0.5 * (s1 + s2);
        s2 = s1;
    }
    (s1, s2)
}

/// XFOIL `SCHECK`: replaces each segment shorter than `stol` times an
/// adjacent one by its midpoint (removes the splinters left by the flap
/// breaks). Doubled points (corners) are kept.
fn remove_splinters(x: &mut Vec<f64>, y: &mut Vec<f64>, stol: f64) {
    'restart: loop {
        let n = x.len();
        // 1-based I = 2 .. N-2 of XFOIL is 0-based i = 1 .. n-3.
        for i in 1..n.saturating_sub(2) {
            let dsm1 = (x[i] - x[i - 1]).hypot(y[i] - y[i - 1]);
            let dsp1 = (x[i + 1] - x[i]).hypot(y[i + 1] - y[i]);
            let dsp2 = (x[i + 2] - x[i + 1]).hypot(y[i + 2] - y[i + 1]);
            if dsp1 == 0.0 {
                continue;
            }
            if dsp1 < stol * dsm1 || dsp1 < stol * dsp2 {
                x[i] = 0.5 * (x[i] + x[i + 1]);
                y[i] = 0.5 * (y[i] + y[i + 1]);
                x.remove(i + 1);
                y.remove(i + 1);
                continue 'restart;
            }
        }
        return;
    }
}

/// Deflects the part of the airfoil behind a hinge by `deflection_deg`
/// degrees, positive downward (XFOIL `GDES` `FLAP`). The hinge is usually
/// inside the contour; the surface where the flap moves in is cut, a
/// circular arc of points closes the gap that opens on the other surface,
/// and short splinter segments are removed (`SCHECK`).
pub fn deflect_flap(
    airfoil: &Airfoil,
    hinge_x: f64,
    hinge_y: HingeY,
    deflection_deg: f64,
) -> Result<Airfoil> {
    finite(deflection_deg, "the flap deflection")?;
    let [xbf, ybf] = hinge_point(airfoil, hinge_x, hinge_y)?;
    let rdef = deflection_deg.to_radians();
    if rdef == 0.0 {
        return Ok(airfoil.clone());
    }
    let curve = airfoil.spline();
    let (tops, bots, _, _) = surface_points_at(&curve, xbf);
    let (atop, abot) = if inside(airfoil.x(), airfoil.y(), xbf, ybf) {
        ((-rdef).max(0.0), rdef.max(0.0))
    } else {
        let [dxb, dyb] = curve.tangent(bots);
        let [dxt, dyt] = curve.tangent(tops);
        let (chx, chy) = (dxb - dxt, dyb - dyt);
        let fvx = curve.x_at(bots) + curve.x_at(tops);
        let fvy = curve.y_at(bots) + curve.y_at(tops);
        let crsp = chx * (ybf - 0.5 * fvy) - chy * (xbf - 0.5 * fvx);
        if crsp > 0.0 {
            // Hinge above the airfoil.
            (rdef.max(0.0), rdef.max(0.0))
        } else {
            ((-rdef).max(0.0), (-rdef).max(0.0))
        }
    };

    let (st1, st2) = break_points(&curve, tops, atop, xbf, ybf, 1);
    let (sb1, sb2) = break_points(&curve, bots, abot, xbf, ybf, 2);
    let [xt1, yt1] = curve.point(st1);
    let [xb1, yb1] = curve.point(sb1);

    // 1-based copies of XFOIL's buffer arrays (index 0 unused).
    let mut nb = airfoil.len();
    let one_based =
        |v: &[f64]| -> Vec<f64> { std::iter::once(0.0).chain(v.iter().copied()).collect() };
    let mut sb = one_based(curve.arc_lengths());
    let mut xb = one_based(airfoil.x());
    let mut yb = one_based(airfoil.y());

    // Points adjacent to the breaks.
    let (mut it1, mut it2, mut ib1, mut ib2) = (0, 0, 0, 0);
    for i in 1..nb {
        if sb[i] <= st1 && sb[i + 1] > st1 {
            it1 = i + 1;
        }
        if sb[i] < st2 && sb[i + 1] >= st2 {
            it2 = i;
        }
        if sb[i] <= sb1 && sb[i + 1] > sb1 {
            ib1 = i;
        }
        if sb[i] < sb2 && sb[i + 1] >= sb2 {
            ib2 = i + 1;
        }
    }
    if it1 == 0 || it2 == 0 || ib1 == 0 || ib2 == 0 || it1 + 1 > nb || ib1 < 2 {
        return Err(invalid(
            "the flap breaks could not be located: is the hinge inside the airfoil?",
        ));
    }
    let dsavg = (sb[nb] - sb[1]) / (nb - 1) as f64;

    // Smallest fraction of the s increments i+1 and i+2 away from a break.
    const SFRAC: f64 = 0.33333;
    let at = |s: f64| curve.point(s);
    let (mut t1new, mut t2new, mut b1new, mut b2new) = ([0.0; 2], [0.0; 2], [0.0; 2], [0.0; 2]);
    let (mut lt1new, mut lt2new, mut lb1new, mut lb2new) = (false, false, false, false);
    if atop != 0.0 {
        let st1p = st1 + SFRAC * (sb[it1] - st1);
        let st1q = st1 + SFRAC * (sb[it1 + 1] - st1);
        if sb[it1] < st1q {
            t1new = at(st1q);
        } else {
            t1new = at(st1p);
            lt1new = true;
        }
        let st2p = st2 + SFRAC * (sb[it2] - st2);
        let it2q = it2.saturating_sub(1).max(1);
        let st2q = st2 + SFRAC * (sb[it2q] - st2);
        if sb[it2] > st2q {
            t2new = at(st2q);
        } else {
            t2new = at(st2p);
            lt2new = true;
        }
    }
    if abot != 0.0 {
        let sb1p = sb1 + SFRAC * (sb[ib1] - sb1);
        let sb1q = sb1 + SFRAC * (sb[ib1 - 1] - sb1);
        if sb[ib1] > sb1q {
            b1new = at(sb1q);
        } else {
            b1new = at(sb1p);
            lb1new = true;
        }
        let sb2p = sb2 + SFRAC * (sb[ib2] - sb2);
        let ib2q = (ib2 + 1).min(nb);
        let sb2q = sb2 + SFRAC * (sb[ib2q] - sb2);
        if sb[ib2] < sb2q {
            b2new = at(sb2q);
        } else {
            b2new = at(sb2p);
            lb2new = true;
        }
    }

    let (sind, cosd) = rdef.sin_cos();
    let rotate = |p: [f64; 2], c: f64, s: f64| -> [f64; 2] {
        let (xbar, ybar) = (p[0] - xbf, p[1] - ybf);
        [xbf + xbar * c + ybar * s, ybf - xbar * s + ybar * c]
    };

    // Rotate the flap points about the hinge.
    for i in 1..=nb {
        if i >= it1 && i <= ib1 {
            continue;
        }
        let [xr, yr] = rotate([xb[i], yb[i]], cosd, sind);
        xb[i] = xr;
        yb[i] = yr;
    }

    // Delete the upper surface points which "disappeared".
    let idif = it1 as isize - it2 as isize - 1;
    if idif > 0 {
        let idif = idif as usize;
        nb -= idif;
        it1 -= idif;
        ib1 -= idif;
        ib2 -= idif;
        for i in it2 + 1..=nb {
            sb[i] = sb[i + idif];
            xb[i] = xb[i + idif];
            yb[i] = yb[i + idif];
        }
    }
    // Delete the lower surface points which "disappeared".
    let idif = ib2 as isize - ib1 as isize - 1;
    if idif > 0 {
        let idif = idif as usize;
        nb -= idif;
        ib2 -= idif;
        for i in ib1 + 1..=nb {
            sb[i] = sb[i + idif];
            xb[i] = xb[i + idif];
            yb[i] = yb[i + idif];
        }
    }
    xb.truncate(nb + 1);
    yb.truncate(nb + 1);

    // Inserts `npadd` slots before 1-based index `at` (the points from
    // `at` on move up by `npadd`).
    let make_room = |v: &mut Vec<f64>, at: usize, npadd: usize| {
        let tail: Vec<f64> = v.split_off(at);
        v.extend(std::iter::repeat_n(0.0, npadd));
        v.extend(tail);
    };

    if atop == 0.0 {
        // New surface on top: points along a circular arc.
        let dsnew = rdef.abs() * (xt1 - xbf).hypot(yt1 - ybf);
        let npadd = (1.5 * dsnew / dsavg + 1.0) as usize;
        if npadd > 0 {
            make_room(&mut xb, it1, npadd);
            make_room(&mut yb, it1, npadd);
            nb += npadd;
            it1 += npadd;
            ib1 += npadd;
            ib2 += npadd;
            let dang = rdef / npadd as f64;
            for ip in 1..=npadd {
                let ang = dang * (ip as f64 - 0.5);
                let [xr, yr] = rotate([xt1, yt1], ang.cos(), ang.sin());
                xb[it1 - ip] = xr;
                yb[it1 - ip] = yr;
            }
        }
    } else {
        // Point in the corner and possibly two adjacent points.
        let npadd = 1 + usize::from(lt2new) + usize::from(lt1new);
        make_room(&mut xb, it1, npadd);
        make_room(&mut yb, it1, npadd);
        nb += npadd;
        it1 += npadd;
        ib1 += npadd;
        ib2 += npadd;
        if lt1new {
            (xb[it1 - 1], yb[it1 - 1]) = (t1new[0], t1new[1]);
            (xb[it1 - 2], yb[it1 - 2]) = (xt1, yt1);
        } else {
            (xb[it1], yb[it1]) = (t1new[0], t1new[1]);
            (xb[it1 - 1], yb[it1 - 1]) = (xt1, yt1);
        }
        let [xr, yr] = rotate(t2new, cosd, sind);
        let k = if lt2new { it2 + 1 } else { it2 };
        (xb[k], yb[k]) = (xr, yr);
    }

    if abot == 0.0 {
        // New surface on the bottom: points along a circular arc.
        let dsnew = rdef.abs() * (xb1 - xbf).hypot(yb1 - ybf);
        let npadd = (1.5 * dsnew / dsavg + 1.0) as usize;
        if npadd > 0 {
            make_room(&mut xb, ib1 + 1, npadd);
            make_room(&mut yb, ib1 + 1, npadd);
            nb += npadd;
            let dang = rdef / npadd as f64;
            for ip in 1..=npadd {
                let ang = dang * (ip as f64 - 0.5);
                let [xr, yr] = rotate([xb1, yb1], ang.cos(), ang.sin());
                xb[ib1 + ip] = xr;
                yb[ib1 + ip] = yr;
            }
        }
    } else {
        let npadd = 1 + usize::from(lb2new) + usize::from(lb1new);
        make_room(&mut xb, ib1 + 1, npadd);
        make_room(&mut yb, ib1 + 1, npadd);
        nb += npadd;
        ib2 += npadd;
        if lb1new {
            (xb[ib1 + 1], yb[ib1 + 1]) = (b1new[0], b1new[1]);
            (xb[ib1 + 2], yb[ib1 + 2]) = (xb1, yb1);
        } else {
            (xb[ib1], yb[ib1]) = (b1new[0], b1new[1]);
            (xb[ib1 + 1], yb[ib1 + 1]) = (xb1, yb1);
        }
        let [xr, yr] = rotate(b2new, cosd, sind);
        let k = if lb2new { ib2 - 1 } else { ib2 };
        (xb[k], yb[k]) = (xr, yr);
    }
    debug_assert_eq!(xb.len(), nb + 1);

    let mut x: Vec<f64> = xb.split_off(1);
    let mut y: Vec<f64> = yb.split_off(1);
    remove_splinters(&mut x, &mut y, 0.2);
    rebuild(airfoil, x, y)
}

// ------------------------------------------------------- TE gap, LE radius

/// Changes the trailing-edge gap to `gap` (XFOIL `GDES` `TGAP`). The
/// thickness change decays from the trailing edge over the blending
/// distance `blend` (0 to 1, fraction of the chord; 1 changes the whole
/// airfoil linearly); `split` (-1 to 1) shares it between the lower and the
/// upper surface (0: equally).
pub fn set_te_gap(airfoil: &Airfoil, gap: f64, blend: f64, split: f64) -> Result<Airfoil> {
    finite(gap, "the gap")?;
    finite(blend, "the blending distance")?;
    finite(split, "the split")?;
    let curve = airfoil.spline();
    let (sble, _) = lefind(&curve);
    let [xble, yble] = curve.point(sble);
    let (xb, yb) = (airfoil.x(), airfoil.y());
    let nb = xb.len() - 1;
    let (xbte, ybte) = (0.5 * (xb[0] + xb[nb]), 0.5 * (yb[0] + yb[nb]));
    let chbsq = (xbte - xble).powi(2) + (ybte - yble).powi(2);

    let (dxn, dyn_) = (xb[0] - xb[nb], yb[0] - yb[nb]);
    let old_gap = dxn.hypot(dyn_);
    let (dxu, dyu) = if old_gap > 0.0 {
        (dxn / old_gap, dyn_ / old_gap)
    } else {
        let (xp, yp) = (curve.dx_nodes(), curve.dy_nodes());
        (-0.5 * (yp[nb] - yp[0]), 0.5 * (xp[nb] - xp[0]))
    };
    let doc = blend.clamp(0.0, 1.0);
    let dgap = gap - old_gap;
    let s = curve.arc_lengths();

    let mut x = xb.to_vec();
    let mut y = yb.to_vec();
    for i in 0..=nb {
        let xoc = ((xb[i] - xble) * (xbte - xble) + (yb[i] - yble) * (ybte - yble)) / chbsq;
        let tfac = if doc == 0.0 {
            if i == 0 || i == nb { 1.0 } else { 0.0 }
        } else {
            let arg = ((1.0 - xoc) * (1.0 / doc - 1.0)).min(15.0);
            (-arg).exp()
        };
        if s[i] <= sble {
            x[i] += 0.5 * (1.0 + split) * dgap * xoc * tfac * dxu;
            y[i] += 0.5 * (1.0 + split) * dgap * xoc * tfac * dyu;
        } else {
            x[i] -= 0.5 * (1.0 - split) * dgap * xoc * tfac * dxu;
            y[i] -= 0.5 * (1.0 - split) * dgap * xoc * tfac * dyu;
        }
    }
    rebuild(airfoil, x, y)
}

/// Scales the leading-edge radius by about `factor` (XFOIL `GDES` `LERA`,
/// `LERSCL`): the thickness is scaled by `√factor` at the leading edge, the
/// change decaying over the blending distance `blend` (fraction of the
/// chord, at least 0.001).
pub fn scale_le_radius(airfoil: &Airfoil, factor: f64, blend: f64) -> Result<Airfoil> {
    finite(factor, "the radius factor")?;
    finite(blend, "the blending distance")?;
    let doc = blend.max(0.001);
    let curve = airfoil.spline();
    let (sle, _) = lefind(&curve);
    let frame = ChordFrame::new(&curve, sle);
    let [dxc, dyc] = frame.dir;
    let [xle, yle] = frame.le;
    let srfac = factor.abs().sqrt();
    let s = curve.arc_lengths();

    let mut x = Vec::with_capacity(s.len());
    let mut y = Vec::with_capacity(s.len());
    for (i, &si) in s.iter().enumerate() {
        let (xi, yi) = (airfoil.x()[i], airfoil.y()[i]);
        let xbar = (xi - xle) * dxc + (yi - yle) * dyc;
        let ybar = (yi - yle) * dxc - (xi - xle) * dyc;
        let [xopp, yopp] = curve.point(sopps(&curve, &frame, si));
        let ybarop = (yopp - yle) * dxc - (xopp - xle) * dyc;
        let xoc = xbar / frame.chord;
        let arg = (xoc / doc).min(15.0);
        let tfac = 1.0 - (1.0 - srfac) * (-arg).exp();
        let ybarct = 0.5 * (ybar + ybarop) + tfac * 0.5 * (ybar - ybarop);
        x.push(xle + xbar * dxc - ybarct * dyc);
        y.push(yle + ybarct * dxc + xbar * dyc);
    }
    rebuild(airfoil, x, y)
}

// --------------------------------------------------- thickness and camber

/// XFOIL `THKCAM`: scales the thickness by `tfac` and the camber by `cfac`
/// about the chord line.
fn thickness_camber_factors(airfoil: &Airfoil, tfac: f64, cfac: f64) -> Result<Airfoil> {
    let curve = airfoil.spline();
    let (sble, _) = lefind(&curve);
    let frame = ChordFrame::new(&curve, sble);
    let [dxc, dyc] = frame.dir;
    let s = curve.arc_lengths();
    let mut x = Vec::with_capacity(s.len());
    let mut y = Vec::with_capacity(s.len());
    for (i, &si) in s.iter().enumerate() {
        let (xb, yb) = (airfoil.x()[i], airfoil.y()[i]);
        let [xbopp, ybopp] = curve.point(sopps(&curve, &frame, si));
        let xcavg = 0.5 * (xb + xbopp) * dxc + 0.5 * (yb + ybopp) * dyc;
        let ycavg = cfac * (0.5 * (yb + ybopp) * dxc - 0.5 * (xb + xbopp) * dyc);
        let xcdel = 0.5 * (xb - xbopp) * dxc + 0.5 * (yb - ybopp) * dyc;
        let ycdel = tfac * (0.5 * (yb - ybopp) * dxc - 0.5 * (xb - xbopp) * dyc);
        x.push((xcavg + xcdel) * dxc - (ycavg + ycdel) * dyc);
        y.push((ycavg + ycdel) * dxc + (xcavg + xcdel) * dyc);
    }
    rebuild(airfoil, x, y)
}

/// Scales the thickness by `thickness_factor` and the camber by
/// `camber_factor` (XFOIL `GDES` `TFAC`).
pub fn scale_thickness_camber(
    airfoil: &Airfoil,
    thickness_factor: f64,
    camber_factor: f64,
) -> Result<Airfoil> {
    finite(thickness_factor, "the thickness factor")?;
    finite(camber_factor, "the camber factor")?;
    thickness_camber_factors(airfoil, thickness_factor, camber_factor)
}

/// Camber line and half-thickness distribution of an airfoil, as XFOIL's
/// `GETCAM` builds them (vertical distances, sorted by x), with their
/// maxima (`GETMAX`).
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CamberThickness {
    /// Camber line `(x, y)`, from the leftmost point, which is at y = 0.
    pub camber: Vec<[f64; 2]>,
    /// Half thickness `(x, t/2)`.
    pub half_thickness: Vec<[f64; 2]>,
    /// Maximum thickness (twice the largest half thickness).
    pub max_thickness: f64,
    /// Its abscissa.
    pub max_thickness_x: f64,
    /// Maximum camber.
    pub max_camber: f64,
    /// Its abscissa.
    pub max_camber_x: f64,
}

/// XFOIL `XLFIND`: arc length of the leftmost point (`dx/ds = 0`).
fn xlfind(curve: &SplineCurve) -> f64 {
    let s = curve.arc_lengths();
    let x = curve.x_nodes();
    let n = s.len();
    let dslen = s[n - 1] - s[0];
    let dseps = dslen * 1.0e-5;
    // First guess: first node (1-based I = 3 .. N-2) where x increases;
    // after an unsuccessful search, Fortran leaves I = N-1.
    let mut i = n - 2; // 0-based of the 1-based N-1
    for k in 2..n.saturating_sub(2) {
        if x[k + 1] - x[k] > 0.0 {
            i = k;
            break;
        }
    }
    let first = s[i];
    if s[i] == s[i - 1] {
        return first; // sharp leading edge
    }
    let (xs, sv) = (curve.dx_nodes(), s);
    let mut sle = first;
    for _ in 0..50 {
        let dxds = deval(sle, x, xs, sv);
        let dxdd = d2val(sle, x, xs, sv);
        if dxdd == 0.0 {
            return first;
        }
        let dsle = (-dxds / dxdd).clamp(-0.01 * dslen.abs(), 0.01 * dslen.abs());
        sle += dsle;
        if dsle.abs() < dseps {
            return sle;
        }
    }
    first
}

/// XFOIL `SORTOL`: sorts `(s, w)` pairs by `s` (stable) and removes the
/// pairs closer than `tol` to their predecessor.
fn sort_tolerance(tol: f64, s: &mut Vec<f64>, w: &mut Vec<f64>) {
    let mut pairs: Vec<(f64, f64)> = s.iter().copied().zip(w.iter().copied()).collect();
    // Bubble sort in XFOIL: stable, as sort_by.
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut k = 0;
    while k + 1 < pairs.len() {
        let (a, b) = (pairs[k], pairs[k + 1]);
        if (a.0 - b.0).powi(2) + (a.1 - b.1).powi(2) < tol * tol {
            pairs.remove(k + 1);
            // XFOIL restarts the scan after a removal; removing the
            // successor and re-checking the same point is equivalent.
        } else {
            k += 1;
        }
    }
    *s = pairs.iter().map(|p| p.0).collect();
    *w = pairs.iter().map(|p| p.1).collect();
}

/// XFOIL `GETMAX`: abscissa and value of the largest |y| of `y(x)`,
/// refined by Newton iteration on the spline.
fn getmax(x: &[f64], y: &[f64]) -> (f64, f64) {
    let n = x.len();
    let xlen = x[n - 1] - x[0];
    let xtol = xlen * 1.0e-5;
    let mut yp = vec![0.0; n];
    segspl(y, x, &mut yp);
    let mut ymax0 = y[0];
    let mut xmax0 = x[0];
    let mut ddx = 0.0;
    for i in 1..n {
        if y[i].abs() > ymax0.abs() {
            ymax0 = y[i];
            xmax0 = 0.5 * (x[i - 1] + x[i]);
            // XFOIL reads X(I+1) beyond the last point; use the last one.
            ddx = 0.5 * (x[(i + 1).min(n - 1)] - x[i - 1]).abs();
        }
    }
    let mut xmax = xmax0;
    for _ in 0..10 {
        let ymax = seval(xmax, y, &yp, x);
        let res = deval(xmax, y, &yp, x);
        let resp = d2val(xmax, y, &yp, x);
        if (xlen * resp).abs() < 1.0e-6 {
            return (xmax, ymax);
        }
        let dx = -res / resp;
        let dx = (0.5 * ddx).min(dx.abs()).copysign(dx);
        xmax += dx;
        if dx.abs() < xtol {
            return (xmax, ymax);
        }
    }
    (xmax0, ymax0)
}

/// Camber and thickness arrays of XFOIL's `GETCAM`, before the maxima.
struct CamberArrays {
    xcm: Vec<f64>,
    ycm: Vec<f64>,
    xtk: Vec<f64>,
    ytk: Vec<f64>,
    /// Leftmost point arc length and ordinate.
    sl: f64,
    yl: f64,
}

fn getcam(curve: &SplineCurve) -> CamberArrays {
    let sl = xlfind(curve);
    let [xl, yl] = curve.point(sl);
    let frame = ChordFrame::new(curve, sl);
    let s = curve.arc_lengths();
    let (x, y) = (curve.x_nodes(), curve.y_nodes());
    let n = s.len();
    let mut xcm = Vec::with_capacity(n + 1);
    let mut ycm = Vec::with_capacity(n + 1);
    let mut xtk = Vec::with_capacity(n + 1);
    let mut ytk = Vec::with_capacity(n + 1);
    for i in 0..n {
        let [mut xopp, mut yopp] = curve.point(sopps(curve, &frame, s[i]));
        if i == 0 {
            (xopp, yopp) = (x[n - 1], y[n - 1]);
        } else if i == n - 1 {
            (xopp, yopp) = (x[0], y[0]);
        }
        xcm.push(0.5 * (x[i] + xopp));
        ycm.push(0.5 * (y[i] + yopp));
        xtk.push(0.5 * (x[i] + xopp));
        ytk.push((0.5 * (y[i] - yopp)).abs());
    }
    let tol = 1.0e-5 * (s[n - 1] - s[0]);
    xcm.push(xl);
    ycm.push(yl);
    sort_tolerance(tol, &mut xcm, &mut ycm);
    let yof = ycm[0];
    ycm.iter_mut().for_each(|v| *v -= yof);
    xtk.push(xl);
    ytk.push(0.0);
    sort_tolerance(tol, &mut xtk, &mut ytk);
    CamberArrays {
        xcm,
        ycm,
        xtk,
        ytk,
        sl,
        yl,
    }
}

/// Camber line, thickness and their maxima, as XFOIL's thickness and
/// camber commands see them.
pub fn camber_thickness(airfoil: &Airfoil) -> CamberThickness {
    let c = getcam(&airfoil.spline());
    let (cx, cy) = getmax(&c.xcm, &c.ycm);
    let (tx, ty) = getmax(&c.xtk, &c.ytk);
    CamberThickness {
        camber: c.xcm.iter().zip(&c.ycm).map(|(&x, &y)| [x, y]).collect(),
        half_thickness: c.xtk.iter().zip(&c.ytk).map(|(&x, &y)| [x, y]).collect(),
        max_thickness: 2.0 * ty,
        max_thickness_x: tx,
        max_camber: cy,
        max_camber_x: cx,
    }
}

/// Sets the maximum thickness and/or camber (XFOIL `GDES` `TSET`): they are
/// scaled from their current maxima ([`camber_thickness`]); `None` leaves
/// one unchanged. Errors if a scaling factor exceeds 100 (XFOIL asks for
/// confirmation).
pub fn set_thickness_camber(
    airfoil: &Airfoil,
    thickness: Option<f64>,
    camber: Option<f64>,
) -> Result<Airfoil> {
    let ct = camber_thickness(airfoil);
    let (tymax, cymax) = (0.5 * ct.max_thickness, ct.max_camber);
    let mut tfac = 1.0;
    let mut cfac = 1.0;
    if let Some(c) = camber {
        finite(c, "the camber")?;
        if cymax != 0.0 {
            cfac = c / cymax;
        }
    }
    if let Some(t) = thickness {
        finite(t, "the thickness")?;
        if tymax != 0.0 {
            tfac = t / (2.0 * tymax);
        }
    }
    if tfac.abs() > 100.0 || cfac.abs() > 100.0 {
        return Err(invalid(format!(
            "questionable input: it implies {tfac:.2} x thickness and {cfac:.2} x camber"
        )));
    }
    thickness_camber_factors(airfoil, tfac, cfac)
}

/// XFOIL `SPLINA`: slopes by averaging the adjacent segment slopes
/// (non-oscillating); end slopes from the end segments.
fn splina(x: &[f64], s: &[f64]) -> Vec<f64> {
    let n = x.len();
    let mut xs = vec![0.0; n];
    let mut lend = true;
    let mut xs1 = 0.0;
    let mut xs2 = 0.0;
    for i in 0..n - 1 {
        let ds = s[i + 1] - s[i];
        if ds == 0.0 {
            xs[i] = xs1;
            lend = true;
        } else {
            xs2 = (x[i + 1] - x[i]) / ds;
            if lend {
                xs[i] = xs2;
                lend = false;
            } else {
                xs[i] = 0.5 * (xs1 + xs2);
            }
        }
        xs1 = xs2;
    }
    xs[n - 1] = xs1;
    xs
}

/// Moves the thickness and/or camber high points to the abscissas given
/// (XFOIL `GDES` `HIGH`); `None` (or a value ≤ 0) leaves one where it is.
/// The thickness and camber distributions slide along x through a smooth
/// mapping, and the contour is rebuilt at the original abscissas.
pub fn move_high_points(
    airfoil: &Airfoil,
    thickness_x: Option<f64>,
    camber_x: Option<f64>,
) -> Result<Airfoil> {
    let curve = airfoil.spline();
    let mut c = getcam(&curve);
    let (cxmax, _) = getmax(&c.xcm, &c.ycm);
    let (txmax, _) = getmax(&c.xtk, &c.ytk);
    let thpnt = thickness_x.filter(|&v| v > 0.0).unwrap_or(txmax);
    let chpnt = camber_x.filter(|&v| v > 0.0).unwrap_or(cxmax);
    finite(thpnt, "the thickness high point")?;
    finite(chpnt, "the camber high point")?;

    // Smooth mapping of x through (x0, old) -> (x0, new) -> (x1, x1).
    let remap = |xs: &mut Vec<f64>, old: f64, new: f64| {
        let first = xs[0];
        let last = xs[xs.len() - 1];
        let xfn = [first, old, last];
        let yfn = [first, new, last];
        let yfnp = splina(&yfn, &xfn);
        for v in xs.iter_mut() {
            *v = seval(*v, &yfn, &yfnp, &xfn);
        }
    };
    remap(&mut c.xtk, txmax, thpnt);
    remap(&mut c.xcm, cxmax, chpnt);

    let mut ytkp = vec![0.0; c.xtk.len()];
    segspl(&c.ytk, &c.xtk, &mut ytkp);
    let mut ycmp = vec![0.0; c.xcm.len()];
    segspl(&c.ycm, &c.xcm, &mut ycmp);
    let s = curve.arc_lengths();
    let x = airfoil.x().to_vec();
    let mut y = Vec::with_capacity(x.len());
    for (i, &xi) in x.iter().enumerate() {
        let ycc = seval(xi, &c.ycm, &ycmp, &c.xcm);
        let ytt = seval(xi, &c.ytk, &ytkp, &c.xtk);
        let yi = if s[i] <= c.sl { ycc + ytt } else { ycc - ytt };
        y.push(yi + c.yl);
    }
    rebuild(airfoil, x, y)
}

// --------------------------------------------------------- interpolation

/// Interpolates two airfoils: `fraction` 0 gives `a`, 1 gives `b`, values
/// outside extrapolate (XFOIL `INTE`, `INTER`). Points with the same
/// normalized arc length from the leading edge on each surface are
/// averaged; the result has the point count of `a`.
pub fn interpolate(a: &Airfoil, b: &Airfoil, fraction: f64) -> Result<Airfoil> {
    finite(fraction, "the fraction")?;
    let c0 = a.spline();
    let c1 = b.spline();
    let (sle0, _) = lefind(&c0);
    let (sle1, _) = lefind(&c1);
    let s0 = c0.arc_lengths();
    let s1 = c1.arc_lengths();
    let (f0, f1) = (1.0 - fraction, fraction);
    let tops0 = s0[0] - sle0;
    let tops1 = s1[0] - sle1;
    let bots0 = s0[s0.len() - 1] - sle0;
    let bots1 = s1[s1.len() - 1] - sle1;
    let mut x = Vec::with_capacity(s0.len());
    let mut y = Vec::with_capacity(s0.len());
    for &st0 in s0 {
        let st1 = if st0 < sle0 {
            sle1 + tops1 * (st0 - sle0) / tops0
        } else {
            sle1 + bots1 * (st0 - sle0) / bots0
        };
        let [xt0, yt0] = c0.point(st0);
        let [xt1, yt1] = c1.point(st1);
        x.push(f0 * xt0 + f1 * xt1);
        y.push(f0 * yt0 + f1 * yt1);
    }
    rebuild(a, x, y)
}

// ------------------------------------------------------- XFOIL commands

/// Numbers of a command or answer line, as XFOIL's `GETFLT` reads them
/// (blank or comma separated; reading stops at the first non-number).
fn numbers(line: &str) -> Vec<f64> {
    line.split(|c: char| c.is_whitespace() || c == ',')
        .filter(|t| !t.is_empty())
        .map_while(|t| t.parse::<f64>().ok())
        .collect()
}

/// Applies XFOIL `GDES` commands given as text, as typed in XFOIL: a
/// command with its arguments, then the answers to the prompts for the
/// arguments left out, one per line (a blank answer keeps XFOIL's default
/// where it has one). Supported: `FLAP` (`F`), `TGAP`, `LERA`, `TFAC`,
/// `TSET`, `HIGH`. For example `["FLAP 0.75 999 10", "0.5"]` deflects by
/// 10° down a flap hinged at x = 0.75, half way between the surfaces
/// (y = 999 makes XFOIL ask for y/t).
pub fn apply_gdes(airfoil: &Airfoil, lines: &[&str]) -> Result<Airfoil> {
    let mut current = airfoil.clone();
    let mut k = 0;
    // Answer to a prompt: the first number of the next line; `None` for a
    // blank line.
    let answer = |k: &mut usize, what: &str| -> Result<Option<f64>> {
        let line = lines
            .get(*k)
            .ok_or_else(|| invalid(format!("missing answer for {what}")))?;
        *k += 1;
        if line.trim().is_empty() {
            return Ok(None);
        }
        numbers(line)
            .first()
            .copied()
            .map(Some)
            .ok_or_else(|| invalid(format!("not a number for {what}: {line:?}")))
    };
    let required = |k: &mut usize, what: &str| -> Result<f64> {
        answer(k, what)?.ok_or_else(|| invalid(format!("missing value for {what}")))
    };
    while k < lines.len() {
        let line = lines[k].trim();
        k += 1;
        if line.is_empty() {
            continue;
        }
        let (command, args) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let r = numbers(args);
        let n = r.len();
        current = match command.to_ascii_uppercase().as_str() {
            "FLAP" | "F" => {
                let (x, y) = if n >= 2 {
                    (r[0], r[1])
                } else {
                    (
                        required(&mut k, "the hinge x")?,
                        required(&mut k, "the hinge y")?,
                    )
                };
                let hinge_y = if y == 999.0 {
                    HingeY::Relative(required(&mut k, "the hinge y/t")?)
                } else {
                    HingeY::Absolute(y)
                };
                let deflection = if n >= 3 {
                    r[2]
                } else {
                    required(&mut k, "the deflection")?
                };
                deflect_flap(&current, x, hinge_y, deflection)?
            }
            "TGAP" => {
                let gap = if n >= 1 {
                    r[0]
                } else {
                    required(&mut k, "the gap")?
                };
                let blend = if n >= 2 {
                    r[1]
                } else {
                    required(&mut k, "the blending distance")?
                };
                let split = if n >= 3 {
                    r[2]
                } else {
                    required(&mut k, "the top/bottom split")?
                };
                set_te_gap(&current, gap, blend, split)?
            }
            "LERA" => {
                let factor = if n >= 1 {
                    r[0]
                } else {
                    required(&mut k, "the radius ratio")?
                };
                let blend = if n >= 2 {
                    r[1]
                } else {
                    required(&mut k, "the blending distance")?
                };
                scale_le_radius(&current, factor, blend)?
            }
            "TFAC" => {
                let tf = if n >= 1 {
                    r[0]
                } else {
                    required(&mut k, "the thickness factor")?
                };
                let cf = if n >= 2 {
                    r[1]
                } else {
                    required(&mut k, "the camber factor")?
                };
                scale_thickness_camber(&current, tf, cf)?
            }
            "TSET" => {
                // 999 or a blank answer: unchanged.
                let keep = |v: Option<f64>| v.filter(|&v| v != 999.0);
                let t = if n >= 1 {
                    Some(r[0])
                } else {
                    answer(&mut k, "the thickness")?
                };
                let c = if n >= 2 {
                    Some(r[1])
                } else {
                    answer(&mut k, "the camber")?
                };
                set_thickness_camber(&current, keep(t), keep(c))?
            }
            "HIGH" => {
                let t = if n >= 1 {
                    Some(r[0])
                } else {
                    answer(&mut k, "the thickness high point")?
                };
                let c = if n >= 2 {
                    Some(r[1])
                } else {
                    answer(&mut k, "the camber high point")?
                };
                move_high_points(&current, t, c)?
            }
            other => return Err(invalid(format!("unsupported GDES command {other:?}"))),
        };
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NacaOptions, naca};

    fn naca_airfoil(code: &str) -> Airfoil {
        naca(code, &NacaOptions::default()).unwrap()
    }

    #[test]
    fn hinge_points() {
        let a = naca_airfoil("0012");
        let mid = hinge_point(&a, 0.7, HingeY::Relative(0.5)).unwrap();
        assert!(mid[1].abs() < 1e-9, "{mid:?}");
        let top = hinge_point(&a, 0.7, HingeY::Relative(1.0)).unwrap();
        let t = top[1];
        assert!(t > 0.03 && t < 0.05, "{t}");
        assert_eq!(
            hinge_point(&a, 0.7, HingeY::Absolute(0.01)).unwrap(),
            [0.7, 0.01]
        );
    }

    #[test]
    fn flap_rotates_the_trailing_edge_about_the_hinge() {
        let a = naca_airfoil("2412");
        assert_eq!(
            deflect_flap(&a, 0.7, HingeY::Relative(0.5), 0.0).unwrap(),
            a
        );
        let hinge = hinge_point(&a, 0.7, HingeY::Relative(0.5)).unwrap();
        for defl in [10.0_f64, -10.0] {
            let f = deflect_flap(&a, 0.7, HingeY::Relative(0.5), defl).unwrap();
            // The trailing edge rotated about the hinge, down for positive
            // deflections.
            let (te0, te1) = ([a.x()[0], a.y()[0]], [f.x()[0], f.y()[0]]);
            let r0 = (te0[0] - hinge[0]).hypot(te0[1] - hinge[1]);
            let r1 = (te1[0] - hinge[0]).hypot(te1[1] - hinge[1]);
            assert!((r0 - r1).abs() < 1e-12);
            let angle = (te0[1] - hinge[1]).atan2(te0[0] - hinge[0])
                - (te1[1] - hinge[1]).atan2(te1[0] - hinge[0]);
            assert!(
                (angle.to_degrees() - defl).abs() < 1e-9,
                "{}",
                angle.to_degrees()
            );
            // The points ahead of the hinge are unchanged.
            for (x, y) in a.x().iter().zip(a.y()).filter(|(x, _)| **x < 0.6) {
                assert!(
                    f.points().any(|[fx, fy]| fx == *x && fy == *y),
                    "({x}, {y})"
                );
            }
        }
    }

    #[test]
    fn te_gap_and_le_radius() {
        let a = naca_airfoil("0012");
        let g = set_te_gap(&a, 0.01, 0.8, 0.0).unwrap();
        let gap = (g.x()[0] - g.x()[g.len() - 1]).hypot(g.y()[0] - g.y()[g.len() - 1]);
        assert!((gap - 0.01).abs() < 2e-4, "{gap}");
        let r0 = a.properties().le_radius;
        let r = scale_le_radius(&a, 2.0, 0.2)
            .unwrap()
            .properties()
            .le_radius;
        assert!(r > 1.5 * r0 && r < 2.5 * r0, "{r0} {r}");
    }

    #[test]
    fn thickness_and_camber() {
        let a = naca_airfoil("2412");
        let ct = camber_thickness(&a);
        assert!((ct.max_thickness - 0.12).abs() < 2e-3, "{ct:?}");
        assert!((ct.max_camber - 0.02).abs() < 1e-3 && (ct.max_camber_x - 0.4).abs() < 0.02);

        let thick = scale_thickness_camber(&a, 1.25, 1.0).unwrap();
        assert!((camber_thickness(&thick).max_thickness / ct.max_thickness - 1.25).abs() < 1e-3);
        let set = set_thickness_camber(&a, Some(0.15), Some(0.03)).unwrap();
        let cs = camber_thickness(&set);
        assert!(
            (cs.max_thickness - 0.15).abs() < 1e-4 && (cs.max_camber - 0.03).abs() < 1e-4,
            "{cs:?}"
        );
        assert!(set_thickness_camber(&a, Some(50.0), None).is_err());

        let moved = move_high_points(&a, Some(0.4), Some(0.3)).unwrap();
        let cm = camber_thickness(&moved);
        assert!(
            (cm.max_thickness_x - 0.4).abs() < 0.01 && (cm.max_camber_x - 0.3).abs() < 0.01,
            "{cm:?}"
        );
        assert!((cm.max_thickness - ct.max_thickness).abs() < 2e-3);
    }

    #[test]
    fn interpolation() {
        let a = naca_airfoil("0012");
        let b = naca_airfoil("0018");
        assert_eq!(interpolate(&a, &b, 0.0).unwrap(), a);
        let mid = interpolate(&a, &b, 0.5).unwrap();
        assert!((mid.properties().max_thickness - 0.15).abs() < 1e-3);
        let extra = interpolate(&a, &b, -0.5).unwrap();
        assert!((extra.properties().max_thickness - 0.09).abs() < 1e-3);
    }

    #[test]
    fn xfoil_command_scripts() {
        let a = naca_airfoil("2412");
        let direct = deflect_flap(&a, 0.75, HingeY::Relative(0.5), 10.0).unwrap();
        assert_eq!(
            apply_gdes(&a, &["FLAP 0.75 999 10", "0.5"]).unwrap(),
            direct
        );
        assert_eq!(
            apply_gdes(&a, &["flap", "0.75", "999", "0.5", "10"]).unwrap(),
            direct
        );
        let chained = apply_gdes(&a, &["TFAC 1.1 1", "TSET 0.13", ""]).unwrap();
        assert!((camber_thickness(&chained).max_thickness - 0.13).abs() < 1e-4);
        assert!(apply_gdes(&a, &["MODI"]).is_err());
        assert!(apply_gdes(&a, &["FLAP 0.75 999 10"]).is_err());
    }

    #[test]
    fn splinters_and_sorting() {
        let mut x = vec![0.0, 1.0, 1.01, 2.0, 3.0];
        let mut y = vec![0.0; 5];
        remove_splinters(&mut x, &mut y, 0.2);
        assert_eq!(x, [0.0, 1.005, 2.0, 3.0]);
        let mut s = vec![3.0, 1.0, 2.0, 1.0 + 1e-9];
        let mut w = vec![0.0, 1.0, 2.0, 1.0];
        sort_tolerance(1e-6, &mut s, &mut w);
        assert_eq!(s, [1.0, 2.0, 3.0]);
        assert_eq!(splina(&[0.0, 1.0, 4.0], &[0.0, 1.0, 2.0]), [1.0, 2.0, 3.0]);
    }
}
