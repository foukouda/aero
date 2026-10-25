//! Panel geometry of the analyzed airfoil (XFOIL `NCALC`, `APCALC`,
//! `TECALC`).

use aero_geometry::Airfoil;
use serde::{Deserialize, Serialize};

use super::error::Result;

/// Trailing-edge panel data (XFOIL `TECALC`).
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrailingEdgePanel {
    /// Normal-projected trailing-edge gap area `ANTE`.
    pub ante: f64,
    /// Streamwise-projected trailing-edge gap area `ASTE`.
    pub aste: f64,
    /// Trailing-edge gap length `DSTE`.
    pub dste: f64,
    /// Sharp trailing edge (`DSTE < 1e-4 · chord`).
    pub sharp: bool,
    /// Source strength factor `SCS = ANTE / DSTE` (1 for a sharp edge).
    pub scs: f64,
    /// Vortex strength factor `SDS = ASTE / DSTE` (0 for a sharp edge).
    pub sds: f64,
}

/// Geometry of the paneled airfoil as seen by the panel method: XFOIL's
/// arrays `X`, `Y`, `S`, `XP`, `YP`, `NX`, `NY`, `APANEL` and the
/// trailing-edge quantities.
///
/// Node `i` and node `i + 1` bound panel `i`; the last panel joins node
/// `n − 1` to node `0` across the trailing edge.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelGeometry {
    pub(crate) x: Vec<f64>,
    pub(crate) y: Vec<f64>,
    pub(crate) s: Vec<f64>,
    pub(crate) xp: Vec<f64>,
    pub(crate) yp: Vec<f64>,
    pub(crate) nx: Vec<f64>,
    pub(crate) ny: Vec<f64>,
    pub(crate) apanel: Vec<f64>,
    pub(crate) le: [f64; 2],
    pub(crate) te: [f64; 2],
    pub(crate) chord: f64,
    pub(crate) te_panel: TrailingEdgePanel,
}

impl PanelGeometry {
    /// Builds the panel geometry from the airfoil nodes, which are used as
    /// panel nodes without repaneling. Repeated successive points are merged
    /// first, as XFOIL's `ABCOPY` does.
    pub fn new(airfoil: &Airfoil) -> Result<Self> {
        let airfoil = airfoil.without_duplicate_points()?;
        let spline = airfoil.spline();
        let x = airfoil.x().to_vec();
        let y = airfoil.y().to_vec();
        let s = spline.arc_lengths().to_vec();
        let xp = spline.dx_nodes().to_vec();
        let yp = spline.dy_nodes().to_vec();
        let n = x.len();

        let le = airfoil.leading_edge();
        let te = airfoil.trailing_edge();
        let chord = (te[0] - le.x).hypot(te[1] - le.y);

        // NCALC: outward unit normals (no corners after merging duplicates).
        let mut nx = vec![0.0; n];
        let mut ny = vec![0.0; n];
        for i in 0..n {
            let sx = yp[i];
            let sy = -xp[i];
            let smod = sx.hypot(sy);
            if smod == 0.0 {
                nx[i] = -1.0;
                ny[i] = 0.0;
            } else {
                nx[i] = sx / smod;
                ny[i] = sy / smod;
            }
        }

        // TECALC.
        let dxte = x[0] - x[n - 1];
        let dyte = y[0] - y[n - 1];
        let dxs = 0.5 * (-xp[0] + xp[n - 1]);
        let dys = 0.5 * (-yp[0] + yp[n - 1]);
        let ante = dxs * dyte - dys * dxte;
        let aste = dxs * dxte + dys * dyte;
        let dste = dxte.hypot(dyte);
        let sharp = dste < 0.0001 * chord;
        let (scs, sds) = if sharp {
            (1.0, 0.0)
        } else {
            (ante / dste, aste / dste)
        };

        // APCALC: panel angles.
        let mut apanel = vec![0.0; n];
        for i in 0..n - 1 {
            let sx = x[i + 1] - x[i];
            let sy = y[i + 1] - y[i];
            apanel[i] = if sx == 0.0 && sy == 0.0 {
                (-ny[i]).atan2(-nx[i])
            } else {
                sx.atan2(-sy)
            };
        }
        apanel[n - 1] = if sharp {
            std::f64::consts::PI
        } else {
            let sx = x[0] - x[n - 1];
            let sy = y[0] - y[n - 1];
            (-sx).atan2(sy) + std::f64::consts::PI
        };

        Ok(Self {
            x,
            y,
            s,
            xp,
            yp,
            nx,
            ny,
            apanel,
            le: [le.x, le.y],
            te,
            chord,
            te_panel: TrailingEdgePanel {
                ante,
                aste,
                dste,
                sharp,
                scs,
                sds,
            },
        })
    }

    /// Number of panel nodes.
    pub fn len(&self) -> usize {
        self.x.len()
    }

    /// Always false: an airfoil has at least five nodes.
    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }

    /// Node x coordinates.
    pub fn x(&self) -> &[f64] {
        &self.x
    }

    /// Node y coordinates.
    pub fn y(&self) -> &[f64] {
        &self.y
    }

    /// Arc length at each node.
    pub fn arc_lengths(&self) -> &[f64] {
        &self.s
    }

    /// Outward unit normal x components at the nodes.
    pub fn normal_x(&self) -> &[f64] {
        &self.nx
    }

    /// Outward unit normal y components at the nodes.
    pub fn normal_y(&self) -> &[f64] {
        &self.ny
    }

    /// Panel angles (XFOIL `APANEL`); the last one is the trailing-edge panel.
    pub fn panel_angles(&self) -> &[f64] {
        &self.apanel
    }

    /// Leading edge (`LEFIND`).
    pub fn leading_edge(&self) -> [f64; 2] {
        self.le
    }

    /// Trailing-edge midpoint.
    pub fn trailing_edge(&self) -> [f64; 2] {
        self.te
    }

    /// Chord length.
    pub fn chord(&self) -> f64 {
        self.chord
    }

    /// Trailing-edge panel data.
    pub fn trailing_edge_panel(&self) -> &TrailingEdgePanel {
        &self.te_panel
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aero_geometry::{NacaOptions, PanelingOptions, TrailingEdge, naca};

    fn paneled(code: &str, te: TrailingEdge) -> Airfoil {
        let opts = NacaOptions {
            trailing_edge: te,
            ..NacaOptions::default()
        };
        naca(code, &opts)
            .unwrap()
            .paneled(&PanelingOptions::default())
            .unwrap()
    }

    #[test]
    fn blunt_trailing_edge_panel() {
        let g = PanelGeometry::new(&paneled("0012", TrailingEdge::Open)).unwrap();
        assert_eq!(g.len(), 160);
        assert!(!g.is_empty());
        let te = g.trailing_edge_panel();
        assert!(!te.sharp);
        assert!((te.dste - 0.002_52).abs() < 1e-9);
        // Nearly vertical gap, bisector nearly along +x: the normal
        // projection is almost the whole gap.
        assert!((te.scs - 1.0).abs() < 0.02, "{te:?}");
        assert!(te.sds.abs() < 1e-3);
        assert!((g.chord() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn sharp_trailing_edge_panel() {
        let g = PanelGeometry::new(&paneled("0012", TrailingEdge::Closed)).unwrap();
        let te = g.trailing_edge_panel();
        assert!(te.sharp);
        assert_eq!((te.scs, te.sds), (1.0, 0.0));
        assert_eq!(g.panel_angles()[159], std::f64::consts::PI);
    }

    #[test]
    fn normals_point_outward() {
        let g = PanelGeometry::new(&paneled("2412", TrailingEdge::Open)).unwrap();
        // Upper surface mid-chord: normal points up; lower: down.
        assert!(g.normal_y()[40] > 0.9);
        assert!(g.normal_y()[120] < -0.9);
        // Leading edge: normal points forward.
        let ile = (0..g.len())
            .min_by(|&a, &b| g.x()[a].total_cmp(&g.x()[b]))
            .unwrap();
        assert!(g.normal_x()[ile] < -0.9);
        for i in 0..g.len() {
            assert!((g.normal_x()[i].hypot(g.normal_y()[i]) - 1.0).abs() < 1e-12);
        }
        assert_eq!(g.x().len(), g.y().len());
        assert_eq!(g.arc_lengths()[0], 0.0);
        assert!(g.leading_edge()[0].abs() < 1e-4);
        assert!((g.trailing_edge()[0] - 1.0).abs() < 1e-12);
    }
}
