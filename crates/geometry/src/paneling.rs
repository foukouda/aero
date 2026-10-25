//! Curvature-based paneling, a line-by-line port of XFOIL's `PANGEN`.
//!
//! Nodes are distributed so that the local panel length `Δs` satisfies
//! `(1 + C·κ)·Δs = const`, where `κ` is a smoothed, normalized curvature
//! of the input contour. An artificial curvature at the trailing edge and in
//! optional refinement zones adds extra bunching there.

use serde::{Deserialize, Serialize};

use crate::airfoil::Airfoil;
use crate::error::{GeometryError, Result};
use crate::leading_edge::lefind;
use crate::spline::{deval, segspl, seval, trisol};

/// Paneling parameters, with the names and meaning of XFOIL's `PPAR` menu.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelingOptions {
    /// Number of panel nodes (`N`). Like XFOIL, this counts nodes: the
    /// contour has `n_nodes - 1` panels plus the trailing-edge panel.
    pub n_nodes: usize,
    /// Panel bunching parameter (`P`, `CVPAR`): strength of the attraction
    /// of nodes to high curvature regions, typically 0 to 1.
    pub curvature_bunching: f64,
    /// TE/LE panel density ratio (`T`, `CTERAT`).
    pub te_le_density_ratio: f64,
    /// Refined-area/LE panel density ratio (`R`, `CTRRAT`).
    pub refined_density_ratio: f64,
    /// Upper-surface refined area x/c limits (`XT`, `XSREF1`/`XSREF2`).
    pub top_refinement: [f64; 2],
    /// Lower-surface refined area x/c limits (`XB`, `XPREF1`/`XPREF2`).
    pub bottom_refinement: [f64; 2],
}

impl Default for PanelingOptions {
    /// 160 nodes with XFOIL 6.99's density ratios.
    fn default() -> Self {
        Self {
            n_nodes: 160,
            curvature_bunching: 1.0,
            te_le_density_ratio: 0.15,
            refined_density_ratio: 0.2,
            top_refinement: [1.0, 1.0],
            bottom_refinement: [1.0, 1.0],
        }
    }
}

impl PanelingOptions {
    /// Minimum number of nodes accepted.
    pub const MIN_NODES: usize = 10;

    /// The defaults of XFOIL 6.996: 240 nodes, TE/LE density ratio 0.3.
    pub fn xfoil_6996() -> Self {
        Self {
            n_nodes: 240,
            te_le_density_ratio: 0.3,
            ..Self::default()
        }
    }

    fn validate(&self) -> Result<()> {
        if self.n_nodes < Self::MIN_NODES {
            return Err(GeometryError::InvalidOption(format!(
                "at least {} panel nodes are required (got {})",
                Self::MIN_NODES,
                self.n_nodes
            )));
        }
        let finite = [
            self.curvature_bunching,
            self.te_le_density_ratio,
            self.refined_density_ratio,
            self.top_refinement[0],
            self.top_refinement[1],
            self.bottom_refinement[0],
            self.bottom_refinement[1],
        ];
        if finite.iter().any(|v| !v.is_finite()) {
            return Err(GeometryError::InvalidOption(
                "paneling parameters must be finite".into(),
            ));
        }
        if self.curvature_bunching < 0.0 || self.te_le_density_ratio <= 0.0 {
            return Err(GeometryError::InvalidOption(
                "bunching parameter must be >= 0 and TE/LE density ratio > 0".into(),
            ));
        }
        Ok(())
    }
}

/// Result of [`panel_with_report`].
#[derive(Debug, Clone, PartialEq)]
pub struct Paneling {
    /// Paneled airfoil (same name as the input).
    pub airfoil: Airfoil,
    /// Whether the node-position Newton iteration converged (XFOIL prints
    /// "Paneling convergence failed. Continuing anyway..." otherwise).
    pub converged: bool,
}

impl Airfoil {
    /// Repanels the airfoil (XFOIL `PANGEN`).
    pub fn paneled(&self, options: &PanelingOptions) -> Result<Airfoil> {
        panel(self, options)
    }
}

/// Repanels an airfoil contour (XFOIL `PANGEN`).
pub fn panel(airfoil: &Airfoil, options: &PanelingOptions) -> Result<Airfoil> {
    panel_with_report(airfoil, options).map(|p| p.airfoil)
}

/// Repanels an airfoil contour and reports convergence (XFOIL `PANGEN`).
pub fn panel_with_report(airfoil: &Airfoil, options: &PanelingOptions) -> Result<Paneling> {
    options.validate()?;

    const IPFAC: usize = 5;
    let n = options.n_nodes;
    let npan = options.n_nodes;

    // Buffer airfoil spline.
    let buffer = airfoil.spline();
    let xb = buffer.x_nodes();
    let yb = buffer.y_nodes();
    let sb = buffer.arc_lengths();
    let nb = sb.len();

    let sbref = 0.5 * (sb[nb - 1] - sb[0]);

    // Curvature array.
    let mut w5: Vec<f64> = sb
        .iter()
        .map(|&s| buffer.curvature(s).abs() * sbref)
        .collect();

    // Leading edge and its normalized curvature.
    let (sble, _) = lefind(&buffer);
    let cvle = buffer.curvature(sble).abs() * sbref;

    // Doubled point (sharp corner) at the leading edge.
    let ible = (0..nb - 1).find(|&i| sble == sb[i] && sble == sb[i + 1]);

    let [xble, yble] = buffer.point(sble);
    let xbte = 0.5 * (xb[0] + xb[nb - 1]);
    let ybte = 0.5 * (yb[0] + yb[nb - 1]);
    let chbsq = (xbte - xble).powi(2) + (ybte - yble).powi(2);

    // Average curvature over 2*NK+1 points within Rcurv of the LE.
    const NK: i32 = 3;
    let mut cvsum = 0.0;
    for k in -NK..=NK {
        let frac = f64::from(k) / f64::from(NK);
        let sbk = sble + frac * sbref / cvle.max(20.0);
        cvsum += buffer.curvature(sbk).abs() * sbref;
    }
    let mut cvavg = cvsum / f64::from(2 * NK + 1);
    if ible.is_some() {
        cvavg = 10.0;
    }

    let cc = 6.0 * options.curvature_bunching;

    // Artificial curvature at the TE to bunch panels there.
    let cvte = cvavg * options.te_le_density_ratio;
    w5[0] = cvte;
    w5[nb - 1] = cvte;

    // Smoothing length: 1 / averaged LE curvature, but no more than 5% of
    // the chord and no less than 1/4 of the average panel spacing.
    let smool = (1.0 / cvavg.max(20.0)).max(0.25 / (npan / 2) as f64);
    let smoosq = (smool * sbref).powi(2);

    let mut w1 = vec![0.0; nb];
    let mut w2 = vec![0.0; nb];
    let mut w3 = vec![0.0; nb];
    w2[0] = 1.0;
    w3[0] = 0.0;
    for i in 1..nb - 1 {
        let dsm = sb[i] - sb[i - 1];
        let dsp = sb[i + 1] - sb[i];
        let dso = 0.5 * (sb[i + 1] - sb[i - 1]);
        if dsm == 0.0 || dsp == 0.0 {
            // Leave the curvature at corner points unchanged.
            w1[i] = 0.0;
            w2[i] = 1.0;
            w3[i] = 0.0;
        } else {
            w1[i] = smoosq * (-1.0 / dsm) / dso;
            w2[i] = smoosq * (1.0 / dsp + 1.0 / dsm) / dso + 1.0;
            w3[i] = smoosq * (-1.0 / dsp) / dso;
        }
    }
    w1[nb - 1] = 0.0;
    w2[nb - 1] = 1.0;

    // Fix the curvature at the LE point by modifying the equations next to it.
    for i in 1..nb - 1 {
        let at_corner = ible.is_some_and(|k| i == k || i == k + 1);
        if sb[i] == sble || at_corner {
            w1[i] = 0.0;
            w2[i] = 1.0;
            w3[i] = 0.0;
            w5[i] = cvle;
        } else if sb[i - 1] < sble && sb[i] > sble && i >= 2 {
            // Equation at the node just before the LE point.
            let dsm = sb[i - 1] - sb[i - 2];
            let dsp = sble - sb[i - 1];
            let dso = 0.5 * (sble - sb[i - 2]);
            w1[i - 1] = smoosq * (-1.0 / dsm) / dso;
            w2[i - 1] = smoosq * (1.0 / dsp + 1.0 / dsm) / dso + 1.0;
            w3[i - 1] = 0.0;
            w5[i - 1] += smoosq * cvle / (dsp * dso);

            // Equation at the node just after the LE point.
            let dsm = sb[i] - sble;
            let dsp = sb[i + 1] - sb[i];
            let dso = 0.5 * (sb[i + 1] - sble);
            w1[i] = 0.0;
            w2[i] = smoosq * (1.0 / dsp + 1.0 / dsm) / dso + 1.0;
            w3[i] = smoosq * (-1.0 / dsp) / dso;
            w5[i] += smoosq * cvle / (dsm * dso);
            break;
        }
    }

    // Artificial curvature at bunching points, fixed there.
    for i in 1..nb - 1 {
        let xoc = ((xb[i] - xble) * (xbte - xble) + (yb[i] - yble) * (ybte - yble)) / chbsq;
        let [lo, hi] = if sb[i] < sble {
            options.top_refinement
        } else {
            options.bottom_refinement
        };
        if xoc > lo && xoc < hi {
            w1[i] = 0.0;
            w2[i] = 1.0;
            w3[i] = 0.0;
            w5[i] = cvle * options.refined_density_ratio;
        }
    }

    // Solve for the smoothed curvature array.
    match ible {
        None => trisol(&mut w2, &w1, &mut w3, &mut w5),
        Some(k) => {
            trisol(&mut w2[..=k], &w1[..=k], &mut w3[..=k], &mut w5[..=k]);
            trisol(
                &mut w2[k + 1..],
                &w1[k + 1..],
                &mut w3[k + 1..],
                &mut w5[k + 1..],
            );
        }
    }

    // Normalize and spline the curvature array.
    let cvmax = w5.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    if !(cvmax > 0.0 && cvmax.is_finite()) {
        return Err(GeometryError::Numerical(
            "paneling: invalid curvature distribution".into(),
        ));
    }
    w5.iter_mut().for_each(|v| *v /= cvmax);
    let mut w6 = vec![0.0; nb];
    segspl(&w5, sb, &mut w6);
    let cv = |s: f64| seval(s, &w5, &w6, sb);
    let cvs = |s: f64| deval(s, &w5, &w6, sb);

    // Initial node positions, uniform in s, with IPFAC times more nodes than
    // requested for more reliable convergence.
    let nn = IPFAC * (n - 1) + 1;
    let rdste = 0.667;
    let rtf = (rdste - 1.0) * 2.0 + 1.0;

    let mut snew = vec![0.0; nn];
    let mut nn1 = 0;
    match ible {
        None => {
            let dsavg = (sb[nb - 1] - sb[0]) / ((nn - 3) as f64 + 2.0 * rtf);
            snew[0] = sb[0];
            for (i, v) in snew.iter_mut().enumerate().take(nn - 1).skip(1) {
                *v = sb[0] + dsavg * ((i - 1) as f64 + rtf);
            }
            snew[nn - 1] = sb[nb - 1];
        }
        Some(k) => {
            let nfrac1 = (n * (k + 1)) / nb;
            nn1 = IPFAC * nfrac1.saturating_sub(1) + 1;
            if nn1 < 3 || nn1 + 2 > nn {
                return Err(GeometryError::Numerical(
                    "paneling: sharp leading edge too close to the trailing edge".into(),
                ));
            }
            let dsavg1 = (sble - sb[0]) / ((nn1 - 2) as f64 + rtf);
            snew[0] = sb[0];
            for (i, v) in snew.iter_mut().enumerate().take(nn1).skip(1) {
                *v = sb[0] + dsavg1 * ((i - 1) as f64 + rtf);
            }
            let nn2 = nn - nn1 + 1;
            let dsavg2 = (sb[nb - 1] - sble) / ((nn2 - 2) as f64 + rtf);
            for i in 2..nn2 {
                snew[i - 2 + nn1] = sble + dsavg2 * ((i - 2) as f64 + rtf);
            }
            snew[nn - 1] = sb[nb - 1];
        }
    }

    // Newton iteration for the node positions.
    let mut w1 = vec![0.0; nn];
    let mut w2 = vec![0.0; nn];
    let mut w3 = vec![0.0; nn];
    let mut w4 = vec![0.0; nn];
    let mut converged = false;
    for _ in 0..20 {
        let cv1 = cv(snew[0]);
        let mut cv2 = cv(snew[1]);
        let cvs1 = cvs(snew[0]);
        let mut cvs2 = cvs(snew[1]);

        let mut cavm = (cv1 * cv1 + cv2 * cv2).sqrt();
        let (mut cavm_s1, mut cavm_s2) = if cavm == 0.0 {
            (0.0, 0.0)
        } else {
            (cvs1 * cv1 / cavm, cvs2 * cv2 / cavm)
        };

        for i in 1..nn - 1 {
            let dsm = snew[i] - snew[i - 1];
            let dsp = snew[i] - snew[i + 1];
            let cv3 = cv(snew[i + 1]);
            let cvs3 = cvs(snew[i + 1]);

            let cavp = (cv3 * cv3 + cv2 * cv2).sqrt();
            let (cavp_s2, cavp_s3) = if cavp == 0.0 {
                (0.0, 0.0)
            } else {
                (cvs2 * cv2 / cavp, cvs3 * cv3 / cavp)
            };

            let fm = cc * cavm + 1.0;
            let fp = cc * cavp + 1.0;
            let rez = dsp * fp + dsm * fm;

            // Lower, main and upper diagonals; the residual requires that
            // (1 + C*curv)*deltaS be equal on both sides of node i.
            w1[i] = -fm + cc * dsm * cavm_s1;
            w2[i] = fp + fm + cc * (dsp * cavp_s2 + dsm * cavm_s2);
            w3[i] = -fp + cc * dsp * cavp_s3;
            w4[i] = -rez;

            cv2 = cv3;
            cvs2 = cvs3;
            cavm = cavp;
            cavm_s1 = cavp_s2;
            cavm_s2 = cavp_s3;
        }

        // Fixed endpoints (at the TE).
        w2[0] = 1.0;
        w3[0] = 0.0;
        w4[0] = 0.0;
        w1[nn - 1] = 0.0;
        w2[nn - 1] = 1.0;
        w4[nn - 1] = 0.0;

        if rtf != 1.0 {
            // Equations next to the TE give the TE panel length ratio RTF.
            let i = 1;
            w4[i] = -((snew[i] - snew[i - 1]) + rtf * (snew[i] - snew[i + 1]));
            w1[i] = -1.0;
            w2[i] = 1.0 + rtf;
            w3[i] = -rtf;

            let i = nn - 2;
            w4[i] = -((snew[i] - snew[i + 1]) + rtf * (snew[i] - snew[i - 1]));
            w3[i] = -1.0;
            w2[i] = 1.0 + rtf;
            w1[i] = -rtf;
        }

        // Fixed sharp LE point.
        if ible.is_some() {
            let i = nn1 - 1;
            w1[i] = 0.0;
            w2[i] = 1.0;
            w3[i] = 0.0;
            w4[i] = sble - snew[i];
        }

        trisol(&mut w2, &w1, &mut w3, &mut w4);

        // Under-relaxation factor that keeps nodes from changing order.
        let mut rlx = 1.0;
        let mut dmax = 0.0_f64;
        for i in 0..nn - 1 {
            let ds = snew[i + 1] - snew[i];
            let dds = w4[i + 1] - w4[i];
            let dsrat = 1.0 + rlx * dds / ds;
            if dsrat > 4.0 {
                rlx = (4.0 - 1.0) * ds / dds;
            }
            if dsrat < 0.2 {
                rlx = (0.2 - 1.0) * ds / dds;
            }
            dmax = dmax.max(w4[i].abs());
        }
        if !(rlx.is_finite() && dmax.is_finite()) {
            return Err(GeometryError::Numerical(
                "paneling: node iteration diverged".into(),
            ));
        }

        for i in 1..nn - 1 {
            snew[i] += rlx * w4[i];
        }

        if dmax < 1.0e-3 {
            converged = true;
            break;
        }
    }

    // New panel node coordinates.
    let mut s: Vec<f64> = (0..n).map(|i| snew[IPFAC * i]).collect();
    let mut x: Vec<f64> = s
        .iter()
        .map(|&v| seval(v, xb, buffer.dx_nodes(), sb))
        .collect();
    let mut y: Vec<f64> = s
        .iter()
        .map(|&v| seval(v, yb, buffer.dy_nodes(), sb))
        .collect();

    // Insert a node at every corner (doubled point) of the buffer airfoil.
    for ib in 0..nb - 1 {
        if sb[ib] != sb[ib + 1] {
            continue;
        }
        let (xc, yc, sc) = (xb[ib], yb[ib], sb[ib]);
        let Some(i) = s.iter().position(|&v| v > sc) else {
            continue;
        };
        s.insert(i, sc);
        x.insert(i, xc);
        y.insert(i, yc);
        // Shift the adjacent nodes to keep panel sizes comparable.
        if i >= 2 {
            s[i - 1] = 0.5 * (s[i] + s[i - 2]);
            x[i - 1] = buffer.x_at(s[i - 1]);
            y[i - 1] = buffer.y_at(s[i - 1]);
        }
        if i + 2 < s.len() {
            s[i + 1] = 0.5 * (s[i] + s[i + 2]);
            x[i + 1] = buffer.x_at(s[i + 1]);
            y[i + 1] = buffer.y_at(s[i + 1]);
        }
    }

    let airfoil = Airfoil::new(airfoil.name(), x, y)?;
    Ok(Paneling { airfoil, converged })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naca::{NacaOptions, naca};

    fn panel_lengths(af: &Airfoil) -> Vec<f64> {
        af.points()
            .collect::<Vec<_>>()
            .windows(2)
            .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
            .collect()
    }

    #[test]
    fn panels_naca0012_with_requested_node_count() {
        let base = naca("0012", &NacaOptions::default()).unwrap();
        let p = panel_with_report(&base, &PanelingOptions::default()).unwrap();
        assert!(p.converged);
        let af = p.airfoil;
        assert_eq!(af.len(), 160);
        assert_eq!(af.name(), "NACA 0012");
        // Endpoints are the buffer trailing-edge points.
        assert_eq!(af.x()[0], base.x()[0]);
        assert_eq!(af.y()[159], base.y()[398]);
        // Paneling of a symmetric section is symmetric.
        for i in 0..80 {
            assert!((af.x()[i] - af.x()[159 - i]).abs() < 1e-10, "x at {i}");
            assert!((af.y()[i] + af.y()[159 - i]).abs() < 1e-10, "y at {i}");
        }
    }

    #[test]
    fn panels_are_bunched_at_leading_and_trailing_edges() {
        let af = panel(
            &naca("0012", &NacaOptions::default()).unwrap(),
            &PanelingOptions::default(),
        )
        .unwrap();
        let ds = panel_lengths(&af);
        let mid = ds[40];
        let le = ds[79].min(ds[80]);
        let te = ds[0];
        assert!(le < 0.3 * mid, "LE {le} mid {mid}");
        assert!(te < mid, "TE {te} mid {mid}");
        assert!(le < te);
        assert!(ds.iter().all(|&d| d > 0.0));
    }

    #[test]
    fn node_count_and_presets() {
        let base = naca("2412", &NacaOptions::default()).unwrap();
        for n in [40, 100, 241] {
            let opts = PanelingOptions {
                n_nodes: n,
                ..PanelingOptions::default()
            };
            assert_eq!(panel(&base, &opts).unwrap().len(), n);
        }
        let opts = PanelingOptions::xfoil_6996();
        assert_eq!(opts.n_nodes, 240);
        assert_eq!(base.paneled(&opts).unwrap().len(), 240);
    }

    #[test]
    fn invalid_options_are_rejected() {
        let base = naca("0012", &NacaOptions::default()).unwrap();
        let bad = [
            PanelingOptions {
                n_nodes: 3,
                ..PanelingOptions::default()
            },
            PanelingOptions {
                curvature_bunching: f64::NAN,
                ..PanelingOptions::default()
            },
            PanelingOptions {
                te_le_density_ratio: 0.0,
                ..PanelingOptions::default()
            },
        ];
        for opts in bad {
            assert!(matches!(
                panel(&base, &opts),
                Err(GeometryError::InvalidOption(_))
            ));
        }
    }

    #[test]
    fn refinement_zone_adds_nodes() {
        let base = naca("0012", &NacaOptions::default()).unwrap();
        let plain = panel(&base, &PanelingOptions::default()).unwrap();
        let refined = panel(
            &base,
            &PanelingOptions {
                top_refinement: [0.4, 0.6],
                ..PanelingOptions::default()
            },
        )
        .unwrap();
        let count = |af: &Airfoil| {
            (0..80)
                .filter(|&i| af.x()[i] > 0.4 && af.x()[i] < 0.6)
                .count()
        };
        assert!(count(&refined) > count(&plain));
    }

    #[test]
    fn corners_are_kept() {
        // Diamond airfoil with a doubled (sharp) leading-edge point.
        let k = 40;
        let mut x = Vec::new();
        let mut y = Vec::new();
        for i in 0..=k {
            let t = 1.0 - f64::from(i) / f64::from(k);
            x.push(t);
            y.push(0.1 * (1.0 - (2.0 * t - 1.0).abs()) + 0.0005 * t);
        }
        for i in 0..=k {
            let t = f64::from(i) / f64::from(k);
            x.push(t);
            y.push(-0.1 * (1.0 - (2.0 * t - 1.0).abs()) - 0.0005 * t);
        }
        let af = Airfoil::new("diamond", x, y).unwrap();
        let p = panel(&af, &PanelingOptions::default()).unwrap();
        // The LE corner is kept as a node.
        assert!(p.points().any(|q| q == [0.0, 0.0]));
        assert!(p.len() >= 160);
    }
}
