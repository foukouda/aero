//! Spanwise and chordwise vortex spacings (AVL `SPACER`, `CSPACER`).
//!
//! A spacing parameter blends equal (0, 3), cosine (1) and sine (2)
//! spacings linearly; a negative parameter reverses the sine spacing
//! (points concentrated at the end instead of the start).

use std::f64::consts::PI;

/// π as written in AVL's `SPACER` (`DATA PI / 3.1415926535 /`), kept so that
/// the spanwise positions are AVL's to the last digit.
#[allow(clippy::approx_constant)]
const PI_SPACER: f64 = 3.141_592_653_5;

/// Normalized positions of `n` points (`n ≥ 2`) from 0 to 1 for the spacing
/// parameter `p` in `[-3, 3]` (AVL `SPACER`).
pub(crate) fn spacer(n: usize, p: f64) -> Vec<f64> {
    let pabs = p.abs();
    // AVL: NABS = IFIX(PABS) + 1, then GO TO (10, 20, 30, 30).
    let (pequ, pcos, psin) = if pabs < 1.0 {
        (1.0 - pabs, pabs, 0.0)
    } else if pabs < 2.0 {
        (0.0, 2.0 - pabs, pabs - 1.0)
    } else {
        (pabs - 2.0, 0.0, 3.0 - pabs)
    };
    let last = (n.max(2) - 1) as f64;
    (0..n)
        .map(|k| {
            let frac = k as f64 / last;
            let theta = frac * PI_SPACER;
            let blend = pequ * frac + pcos * (1.0 - theta.cos()) / 2.0;
            // For p = 0 both of AVL's branches run; the second one wins.
            if p > 0.0 {
                blend + psin * (1.0 - (theta / 2.0).cos())
            } else {
                blend + psin * (theta / 2.0).sin()
            }
        })
        .collect()
}

/// Chordwise fractions of a strip of `nvc` vortices (AVL `CSPACER`).
pub(crate) struct ChordSpacing {
    /// Element edges, `nvc + 1` values from 0 to 1.
    pub(crate) edges: Vec<f64>,
    /// Bound vortex positions (the quarter of each element for equal spacing).
    pub(crate) vortex: Vec<f64>,
    /// Control points (three quarters of each element, moved by `claf`).
    pub(crate) control: Vec<f64>,
}

/// The chordwise spacing of `nvc` elements for the parameter `cspace` and
/// the lift-slope factor `claf`, which moves the control points.
pub(crate) fn cspacer(nvc: usize, cspace: f64, claf: f64) -> ChordSpacing {
    let acsp = cspace.abs();
    let (f0, f1, f2) = if acsp < 1.0 {
        (1.0 - acsp, acsp, 0.0)
    } else if acsp < 2.0 {
        (0.0, 2.0 - acsp, acsp - 1.0)
    } else {
        (acsp - 2.0, 0.0, 3.0 - acsp)
    };

    let n = nvc as f64;
    let dth1 = PI / (4.0 * n + 2.0);
    let dth2 = 0.5 * PI / (4.0 * n + 1.0);
    let dxc0 = 1.0 / (4.0 * n);

    let mut edges = vec![0.0; nvc + 1];
    let mut vortex = vec![0.0; nvc];
    let mut control = vec![0.0; nvc];
    for i in 0..nvc {
        let ivc = (i + 1) as f64;
        // Equal.
        let xc0 = (4.0 * ivc - 4.0) * dxc0;
        let xpt0 = xc0;
        let xvr0 = xc0 + dxc0;
        let xcp0 = xc0 + dxc0 + 2.0 * dxc0 * claf;
        // Cosine.
        let th1 = (4.0 * ivc - 3.0) * dth1;
        let xpt1 = 0.5 * (1.0 - th1.cos());
        let xvr1 = 0.5 * (1.0 - (th1 + dth1).cos());
        let xcp1 = 0.5 * (1.0 - (th1 + dth1 + 2.0 * dth1 * claf).cos());
        // Sine, or reversed sine.
        let (xpt2, xvr2, xcp2) = if cspace > 0.0 {
            let th2 = (4.0 * ivc - 3.0) * dth2;
            (
                1.0 - th2.cos(),
                1.0 - (th2 + dth2).cos(),
                1.0 - (th2 + dth2 + 2.0 * dth2 * claf).cos(),
            )
        } else {
            let th2 = (4.0 * ivc - 4.0) * dth2;
            (
                th2.sin(),
                (th2 + dth2).sin(),
                (th2 + dth2 + 2.0 * dth2 * claf).sin(),
            )
        };
        edges[i] = f0 * xpt0 + f1 * xpt1 + f2 * xpt2;
        vortex[i] = f0 * xvr0 + f1 * xvr1 + f2 * xvr2;
        control[i] = f0 * xcp0 + f1 * xcp1 + f2 * xcp2;
    }
    edges[0] = 0.0;
    edges[nvc] = 1.0;
    ChordSpacing {
        edges,
        vortex,
        control,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spacer_blends_equal_cosine_and_sine() {
        let equal = spacer(5, 0.0);
        for (k, x) in equal.iter().enumerate() {
            assert!((x - k as f64 / 4.0).abs() < 1e-15);
        }
        let cosine = spacer(5, 1.0);
        assert!((cosine[1] - (1.0 - (PI_SPACER / 4.0).cos()) / 2.0).abs() < 1e-15);
        // Sine: fine at the start; reversed sine: fine at the end.
        let sine = spacer(5, 2.0);
        let reversed = spacer(5, -2.0);
        assert!(sine[1] < 0.1 && reversed[1] > 0.3);
        assert!(1.0 - reversed[3] < 0.1);
        // Halfway between equal and cosine.
        let half = spacer(5, 0.5);
        assert!((half[1] - 0.5 * (equal[1] + cosine[1])).abs() < 1e-15);
    }

    #[test]
    fn equal_chordwise_spacing_puts_vortices_at_quarter_chords() {
        let s = cspacer(4, 0.0, 1.0);
        assert_eq!(s.edges, vec![0.0, 0.25, 0.5, 0.75, 1.0]);
        for i in 0..4 {
            assert!((s.vortex[i] - (i as f64 + 0.25) / 4.0).abs() < 1e-15);
            assert!((s.control[i] - (i as f64 + 0.75) / 4.0).abs() < 1e-15);
        }
        // A lift-slope factor moves the control points only.
        let s = cspacer(4, 0.0, 0.5);
        assert!((s.control[0] - 0.125).abs() < 1e-15);
    }

    #[test]
    fn cosine_chordwise_spacing_is_fine_at_both_edges() {
        let s = cspacer(8, 1.0, 1.0);
        let first = s.edges[1] - s.edges[0];
        let middle = s.edges[5] - s.edges[4];
        let last = s.edges[8] - s.edges[7];
        assert!(first < 0.3 * middle && last < 0.3 * middle);
        for i in 0..8 {
            assert!(s.edges[i] < s.vortex[i] && s.vortex[i] < s.control[i]);
            assert!(s.control[i] < s.edges[i + 1]);
        }
    }
}
