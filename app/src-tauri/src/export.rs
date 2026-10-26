//! CSV export of results.

use aero_foil2d::inviscid::InviscidSolution;
use aero_foil2d::viscous::{BoundaryLayer, ViscousSolution};

use crate::model::{PolarData, StoredPolar};

/// Quotes a CSV field when needed (RFC 4180).
fn field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

/// Polars as CSV, one row per point. Inviscid and viscous polars share the
/// columns; those that do not apply are left empty.
pub fn polars_csv(polars: &[&StoredPolar]) -> String {
    let mut out = String::from(
        "airfoil,analysis,mach,reynolds,ncrit_top,ncrit_bottom,trip_top,trip_bottom,alpha_deg,\
         cl,cd,cdp,cdf,cm,cp_min,cp_min_x,xtr_top,xtr_bottom,supercritical\n",
    );
    for p in polars {
        let name = field(&p.airfoil_name);
        match &p.polar {
            PolarData::Inviscid(polar) => {
                for q in &polar.points {
                    out.push_str(&format!(
                        "{name},inviscid,{},,,,,,{},{},,{},,{},{},{},,,{}\n",
                        polar.mach,
                        q.alpha_deg,
                        q.cl,
                        q.cdp,
                        q.cm,
                        q.cp_min,
                        q.cp_min_x,
                        q.supercritical
                    ));
                }
            }
            PolarData::Viscous(polar) => {
                let o = &polar.options;
                for q in &polar.points {
                    out.push_str(&format!(
                        "{name},viscous,{},{},{},{},{},{},{},{},{},{},{},{},{},,{},{},\n",
                        o.mach,
                        o.reynolds,
                        o.ncrit[0],
                        o.ncrit[1],
                        o.xtr[0],
                        o.xtr[1],
                        q.alpha_deg,
                        q.cl,
                        q.cd,
                        q.cdp,
                        q.cdf,
                        q.cm,
                        q.cp_min,
                        q.xtr_top,
                        q.xtr_bottom
                    ));
                }
            }
        }
    }
    out
}

/// Pressure distribution as CSV, one row per panel node.
pub fn cp_csv(x: &[f64], y: &[f64], solution: &InviscidSolution) -> String {
    let mut out = String::from("x,y,cp,surface_speed\n");
    for i in 0..x.len().min(solution.cp.len()) {
        out.push_str(&format!(
            "{},{},{},{}\n",
            x[i], y[i], solution.cp[i], solution.surface_speed_compressible[i]
        ));
    }
    out
}

/// Viscous and inviscid pressure distributions as CSV, one row per node.
pub fn viscous_cp_csv(x: &[f64], y: &[f64], solution: &ViscousSolution) -> String {
    let mut out = String::from("x,y,cp,cp_inviscid,surface_speed\n");
    for i in 0..x.len().min(solution.cp.len()) {
        out.push_str(&format!(
            "{},{},{},{},{}\n",
            x[i], y[i], solution.cp[i], solution.cp_inviscid[i], solution.surface_speed[i]
        ));
    }
    out
}

/// Boundary layer as CSV: top surface, bottom surface and wake, from the
/// stagnation point (or the trailing edge) downstream.
pub fn boundary_layer_csv(solution: &ViscousSolution) -> String {
    let mut out = String::from("surface,x,y,xi,ue,cp,dstar,theta,cf,h,amplification,sqrt_ctau\n");
    let opt = |v: Option<f64>| v.map(|v| v.to_string()).unwrap_or_default();
    let mut rows = |name: &str, b: &BoundaryLayer| {
        for i in 0..b.x.len() {
            out.push_str(&format!(
                "{name},{},{},{},{},{},{},{},{},{},{},{}\n",
                b.x[i],
                b.y[i],
                b.xi[i],
                b.ue[i],
                b.cp[i],
                b.dstar[i],
                b.theta[i],
                b.cf[i],
                b.h[i],
                opt(b.amplification[i]),
                opt(b.sqrt_ctau[i])
            ));
        }
    };
    rows("top", &solution.top);
    rows("bottom", &solution.bottom);
    rows("wake", &solution.wake);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use aero_foil2d::inviscid::{Polar, PolarPoint};
    use aero_foil2d::viscous::{ViscousOptions, ViscousPolar, ViscousPolarPoint};

    #[test]
    fn polars_are_written_with_quoted_names() {
        let p = StoredPolar {
            airfoil_id: 0,
            airfoil_name: "foil, \"A\"".into(),
            polar: PolarData::Inviscid(Polar {
                mach: 0.3,
                points: vec![PolarPoint {
                    alpha_deg: 2.0,
                    cl: 0.25,
                    cm: -0.01,
                    cdp: 0.0,
                    cp_min: -0.5,
                    cp_min_x: 0.1,
                    supercritical: false,
                }],
                failures: vec![],
                cancelled: false,
            }),
        };
        let v = StoredPolar {
            airfoil_id: 1,
            airfoil_name: "B".into(),
            polar: PolarData::Viscous(ViscousPolar {
                options: ViscousOptions::default(),
                points: vec![ViscousPolarPoint {
                    alpha_deg: 4.0,
                    cl: 0.43,
                    cd: 0.0073,
                    cdp: 0.0012,
                    cdf: 0.0058,
                    cm: 0.006,
                    cp_min: -1.4,
                    xtr_top: 0.25,
                    xtr_bottom: 0.97,
                    iterations: 7,
                }],
                failures: vec![],
                stopped: false,
                cancelled: false,
            }),
        };
        let csv = polars_csv(&[&p, &v]);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 3);
        let columns = lines[0].split(',').count();
        assert!(
            lines
                .iter()
                .all(|l| l.replace("\"\"", "").split(',').count() >= columns)
        );
        assert_eq!(
            lines[1],
            "\"foil, \"\"A\"\"\",inviscid,0.3,,,,,,2,0.25,,0,,-0.01,-0.5,0.1,,,false"
        );
        assert_eq!(
            lines[2],
            "B,viscous,0,1000000,9,9,1,1,4,0.43,0.0073,0.0012,0.0058,0.006,-1.4,,0.25,0.97,"
        );
    }
}
