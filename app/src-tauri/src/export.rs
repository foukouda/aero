//! CSV export of results.

use aero_foil2d::inviscid::InviscidSolution;

use crate::model::StoredPolar;

/// Quotes a CSV field when needed (RFC 4180).
fn field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

/// Polars as CSV, one row per point.
pub fn polars_csv(polars: &[&StoredPolar]) -> String {
    let mut out = String::from("airfoil,mach,alpha_deg,cl,cm,cdp,cp_min,cp_min_x,supercritical\n");
    for p in polars {
        let name = field(&p.airfoil_name);
        for q in &p.polar.points {
            out.push_str(&format!(
                "{name},{},{},{},{},{},{},{},{}\n",
                p.polar.mach, q.alpha_deg, q.cl, q.cm, q.cdp, q.cp_min, q.cp_min_x, q.supercritical
            ));
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

#[cfg(test)]
mod tests {
    use super::*;
    use aero_foil2d::inviscid::{Polar, PolarPoint};

    #[test]
    fn polars_are_written_with_quoted_names() {
        let p = StoredPolar {
            airfoil_id: 0,
            airfoil_name: "foil, \"A\"".into(),
            polar: Polar {
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
            },
        };
        let csv = polars_csv(&[&p]);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(
            lines[1],
            "\"foil, \"\"A\"\"\",0.3,2,0.25,-0.01,0,-0.5,0.1,false"
        );
    }
}
