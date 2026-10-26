//! Velocities induced by the horseshoe vortices (AVL `VORVELC`, `VVOR`).

use crate::lattice::Element;
use crate::model::VortexCore;
use crate::vec3::V3;

/// 1/(4π) as written in AVL's `VORVELC` (`DATA PI4INV / 0.079577472 /`).
const PI4INV: f64 = 0.079_577_472;

/// Velocity at `p` induced by a horseshoe vortex of unit circulation whose
/// bound leg runs from `r1` to `r2` and whose trailing legs run to +∞ along
/// x, in Prandtl–Glauert coordinates (`beta = √(1 − M²)`), with Leishman's
/// core model of radius `rcore`; `bound = false` leaves out the bound leg
/// (AVL `VORVELC`).
pub(crate) fn horseshoe(p: V3, bound: bool, r1: V3, r2: V3, beta: f64, rcore: f64) -> V3 {
    let a = [(r1[0] - p[0]) / beta, r1[1] - p[1], r1[2] - p[2]];
    let b = [(r2[0] - p[0]) / beta, r2[1] - p[1], r2[2] - p[2]];
    let asq = a[0] * a[0] + a[1] * a[1] + a[2] * a[2];
    let bsq = b[0] * b[0] + b[1] * b[1] + b[2] * b[2];
    let amag = asq.sqrt();
    let bmag = bsq.sqrt();
    let rcore2 = rcore * rcore;
    let rcore4 = rcore2 * rcore2;

    let (mut u, mut v, mut w) = (0.0, 0.0, 0.0);

    // Bound leg.
    if bound && amag * bmag != 0.0 {
        let axb = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        let axbsq = axb[0] * axb[0] + axb[1] * axb[1] + axb[2] * axb[2];
        if axbsq != 0.0 {
            let adb = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            let alsq = asq + bsq - 2.0 * adb;
            let t = ((bsq - adb) / (bsq * bsq + rcore4).sqrt().sqrt()
                + (asq - adb) / (asq * asq + rcore4).sqrt().sqrt())
                / (axbsq * axbsq + alsq * alsq * rcore4).sqrt();
            u = axb[0] * t;
            v = axb[1] * t;
            w = axb[2] * t;
        }
    }

    // Trailing leg from r1.
    if amag != 0.0 {
        let rsq = a[2] * a[2] + a[1] * a[1];
        let t = -(1.0 - a[0] / amag) / (rsq * rsq + rcore4).sqrt();
        v += a[2] * t;
        w -= a[1] * t;
    }

    // Trailing leg from r2.
    if bmag != 0.0 {
        let rsq = b[2] * b[2] + b[1] * b[1];
        let t = (1.0 - b[0] / bmag) / (rsq * rsq + rcore4).sqrt();
        v += b[2] * t;
        w -= b[1] * t;
    }

    [u * PI4INV / beta, v * PI4INV, w * PI4INV]
}

/// The influence of the elements on field points (AVL `VVOR` without the
/// ground plane): the velocity at a point induced by the unit horseshoe of
/// an element and, with a symmetry plane at `y = 0`, by its mirror image.
pub(crate) struct Influence<'a> {
    pub(crate) elements: &'a [Element],
    pub(crate) beta: f64,
    pub(crate) y_symmetry: bool,
    pub(crate) core: VortexCore,
}

impl Influence<'_> {
    /// Velocity at `p`, a point of component `component`, induced by
    /// element `j`; `self_bound = false` leaves out the bound leg of the
    /// element itself (`LVTEST` with `i = j`).
    pub(crate) fn velocity(&self, p: V3, component: usize, j: usize, self_bound: bool) -> V3 {
        let e = &self.elements[j];
        let dsyz = ((e.rv2[1] - e.rv1[1]).powi(2) + (e.rv2[2] - e.rv1[2]).powi(2)).sqrt();
        let rcore = if component != e.component {
            (self.core.chord_fraction * e.chord).max(self.core.width_fraction * dsyz)
        } else {
            0.0001 * dsyz
        };
        let mut vel = horseshoe(p, self_bound, e.rv1, e.rv2, self.beta, rcore);
        if self.y_symmetry {
            // Image about y = 0, with its ends swapped to keep the sense.
            let xave = 0.5 * (e.rv1[0] + e.rv2[0]);
            let yave = -0.5 * (e.rv1[1] + e.rv2[1]);
            let zave = 0.5 * (e.rv1[2] + e.rv2[2]);
            let bound = !(p[0] == xave && p[1] == yave && p[2] == zave);
            let img = horseshoe(
                p,
                bound,
                [e.rv2[0], -e.rv2[1], e.rv2[2]],
                [e.rv1[0], -e.rv1[1], e.rv1[2]],
                self.beta,
                rcore,
            );
            vel = [vel[0] + img[0], vel[1] + img[1], vel[2] + img[2]];
        }
        vel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Biot–Savart for a straight segment from `r1` to `r2`, unit strength.
    fn segment(p: V3, r1: V3, r2: V3) -> V3 {
        let a = [r1[0] - p[0], r1[1] - p[1], r1[2] - p[2]];
        let b = [r2[0] - p[0], r2[1] - p[1], r2[2] - p[2]];
        let c = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        let csq = c[0] * c[0] + c[1] * c[1] + c[2] * c[2];
        let l = [r2[0] - r1[0], r2[1] - r1[1], r2[2] - r1[2]];
        let na = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
        let nb = (b[0] * b[0] + b[1] * b[1] + b[2] * b[2]).sqrt();
        let k = ((l[0] * a[0] + l[1] * a[1] + l[2] * a[2]) / na
            - (l[0] * b[0] + l[1] * b[1] + l[2] * b[2]) / nb)
            / (4.0 * std::f64::consts::PI * csq);
        // Velocity of a vortex from r1 to r2: (r1 − p) × (r2 − p) direction.
        [-c[0] * k, -c[1] * k, -c[2] * k]
    }

    #[test]
    fn the_mirror_image_keeps_its_bound_leg_unless_the_point_is_on_it() {
        let element = |rv1: V3, rv2: V3| Element {
            rv1,
            rv2,
            rv: [
                0.5 * (rv1[0] + rv2[0]),
                0.5 * (rv1[1] + rv2[1]),
                0.5 * (rv1[2] + rv2[2]),
            ],
            rc: [0.0; 3],
            chord: 1.0,
            slope: 0.0,
            component: 1,
            enc: [0.0, 0.0, 1.0],
        };
        // Both elements are one unit wide: core 1e-4.
        let rcore = 0.0001;
        // Off the plane: at its own middle the element loses its bound leg,
        // its image keeps it.
        let (r1, r2) = ([0.1, 1.0, 0.0], [0.2, 2.0, 0.0]);
        let elements = [element(r1, r2)];
        let infl = Influence {
            elements: &elements,
            beta: 1.0,
            y_symmetry: true,
            core: VortexCore::default(),
        };
        let p = elements[0].rv;
        let got = infl.velocity(p, 1, 0, false);
        let own = horseshoe(p, false, r1, r2, 1.0, rcore);
        let image = horseshoe(
            p,
            true,
            [r2[0], -r2[1], r2[2]],
            [r1[0], -r1[1], r1[2]],
            1.0,
            rcore,
        );
        for k in 0..3 {
            assert!((got[k] - own[k] - image[k]).abs() < 1e-15);
        }
        // Across the plane the image is the element itself: its bound leg
        // is left out at the middle as well.
        let (r1, r2) = ([0.0, -0.5, 0.0], [0.0, 0.5, 0.0]);
        let elements = [element(r1, r2)];
        let infl = Influence {
            elements: &elements,
            beta: 1.0,
            y_symmetry: true,
            core: VortexCore::default(),
        };
        let got = infl.velocity([0.0; 3], 1, 0, false);
        let legs = horseshoe([0.0; 3], false, r1, r2, 1.0, rcore);
        assert!(
            got.iter()
                .zip(legs)
                .all(|(g, l)| (g - 2.0 * l).abs() < 1e-15)
        );
    }

    #[test]
    fn horseshoe_matches_biot_savart_far_from_the_core() {
        let r1 = [0.1, -0.5, 0.02];
        let r2 = [0.2, 0.5, -0.03];
        let p = [0.4, 0.13, 0.3];
        let far = 1.0e7;
        let expected = {
            let s1 = segment(p, [r1[0] + far, r1[1], r1[2]], r1);
            let s2 = segment(p, r1, r2);
            let s3 = segment(p, r2, [r2[0] + far, r2[1], r2[2]]);
            [
                s1[0] + s2[0] + s3[0],
                s1[1] + s2[1] + s3[1],
                s1[2] + s2[2] + s3[2],
            ]
        };
        let got = horseshoe(p, true, r1, r2, 1.0, 1e-6);
        for k in 0..3 {
            // AVL's 1/(4π) has 8 digits.
            assert!(
                (got[k] - expected[k]).abs() < 1e-7 * expected[k].abs().max(1e-3),
                "{k}: {got:?} vs {expected:?}"
            );
        }
        // Prandtl–Glauert: β stretches x.
        let pg = horseshoe(p, true, r1, r2, 0.8, 1e-6);
        assert!((pg[1] - got[1]).abs() > 1e-4);
    }

    #[test]
    fn downwash_behind_a_wide_horseshoe_is_twice_that_at_the_bound_leg() {
        // At the middle of a very wide bound leg the trailing legs induce
        // half the downwash they induce far downstream.
        let r1 = [0.0, -1000.0, 0.0];
        let r2 = [0.0, 1000.0, 0.0];
        let at_leg = horseshoe([0.0, 0.0, 0.0], false, r1, r2, 1.0, 0.0);
        let far = horseshoe([1.0e6, 0.0, 0.0], false, r1, r2, 1.0, 0.0);
        assert!((far[2] / at_leg[2] - 2.0).abs() < 1e-6);
        assert!(at_leg[2] < 0.0, "a positive circulation makes downwash");
    }
}
