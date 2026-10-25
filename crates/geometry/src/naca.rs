//! NACA 4- and 5-digit airfoil generators, ported from XFOIL's `naca.f`.
//!
//! The point distribution, thickness polynomial and camber lines are those
//! of XFOIL, so that a generated section followed by [`crate::panel`]
//! reproduces XFOIL's `NACA` command. Two trailing-edge variants exist:
//! - [`TrailingEdge::Open`]: the standard NACA thickness (finite trailing
//!   edge thickness, XFOIL `NACA`);
//! - [`TrailingEdge::Closed`]: XFOIL's `NACS` variant, which evaluates the
//!   thickness at `1.0089304·x` so that it vanishes at `x = 1`.
//!
//! Beyond XFOIL, 5-digit designations accept any design-lift digit (the
//! camber line scales linearly with it) and the reflexed camber lines
//! (third digit 1).

use serde::{Deserialize, Serialize};

use crate::airfoil::Airfoil;
use crate::error::{GeometryError, Result};

/// Trailing-edge closure of a generated NACA section.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrailingEdge {
    /// Standard NACA thickness distribution, open trailing edge.
    #[default]
    Open,
    /// Sharp trailing edge, as XFOIL's `NACS` command.
    Closed,
}

/// Options of the NACA generators.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NacaOptions {
    /// Number of points on each surface, leading edge included (XFOIL uses
    /// `IQX/3 = 200`).
    pub points_per_side: usize,
    /// Trailing-edge closure.
    pub trailing_edge: TrailingEdge,
}

impl Default for NacaOptions {
    fn default() -> Self {
        Self {
            points_per_side: 200,
            trailing_edge: TrailingEdge::Open,
        }
    }
}

/// x location where the standard NACA thickness vanishes (XFOIL `XTES`).
const XTES: f64 = 1.008_930_411_365_142_7;

/// Trailing-edge point bunching exponent (XFOIL `AN`).
const AN: f64 = 1.5;

/// A parsed NACA designation.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Designation {
    Four { m: f64, p: f64, t: f64 },
    Five { camber: FiveDigitCamber, t: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct FiveDigitCamber {
    /// Location parameter `m` of the camber line.
    m: f64,
    /// `k1` constant.
    k1: f64,
    /// `k2/k1` for reflexed camber lines, `None` for standard ones.
    k21: Option<f64>,
    /// Scale factor of the camber line (design lift digit / 2).
    scale: f64,
}

/// Generates a NACA 4- or 5-digit airfoil from its designation, e.g.
/// `"2412"`, `"0012"`, `"23012"` or `"NACA 4415"`.
pub fn naca(designation: &str, options: &NacaOptions) -> Result<Airfoil> {
    let trimmed = designation.trim();
    let digits = match trimmed.get(..4) {
        Some(prefix) if prefix.eq_ignore_ascii_case("naca") => &trimmed[4..],
        _ => trimmed,
    };
    let digits: String = digits.chars().filter(|c| !c.is_whitespace()).collect();
    let des = parse(&digits).ok_or_else(|| GeometryError::InvalidNaca(designation.to_string()))?;
    generate(&des, &digits, options)
}

fn parse(digits: &str) -> Option<Designation> {
    if !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let d: Vec<f64> = digits
        .chars()
        .filter_map(|c| c.to_digit(10).map(f64::from))
        .collect();
    match d.len() {
        4 => Some(Designation::Four {
            m: d[0] / 100.0,
            p: d[1] / 10.0,
            t: (10.0 * d[2] + d[3]) / 100.0,
        }),
        5 => {
            let lift = d[0];
            let position = digits.as_bytes()[1];
            let reflex = digits.as_bytes()[2];
            // Standard (XFOIL) and reflexed camber-line constants, tabulated
            // for a design lift coefficient of 0.3 (first digit 2).
            #[allow(clippy::approx_constant)] // 0.3180 is a tabulated value, not 1/pi
            let (m, k1, k21) = match (position, reflex) {
                (b'1', b'0') => (0.0580, 361.4, None),
                (b'2', b'0') => (0.1260, 51.64, None),
                (b'3', b'0') => (0.2025, 15.957, None),
                (b'4', b'0') => (0.2900, 6.643, None),
                (b'5', b'0') => (0.3910, 3.230, None),
                (b'2', b'1') => (0.1300, 51.99, Some(0.000_764)),
                (b'3', b'1') => (0.2170, 15.793, Some(0.006_77)),
                (b'4', b'1') => (0.3180, 6.520, Some(0.0303)),
                (b'5', b'1') => (0.4410, 3.191, Some(0.1355)),
                _ => return None,
            };
            if lift == 0.0 {
                return None;
            }
            Some(Designation::Five {
                camber: FiveDigitCamber {
                    m,
                    k1,
                    k21,
                    scale: lift / 2.0,
                },
                t: (10.0 * d[3] + d[4]) / 100.0,
            })
        }
        _ => None,
    }
}

/// Standard NACA 4-digit thickness (half-thickness) distribution.
fn thickness(x: f64, t: f64) -> f64 {
    (0.29690 * x.sqrt() - 0.12600 * x - 0.35160 * x.powi(2) + 0.28430 * x.powi(3)
        - 0.10150 * x.powi(4))
        * t
        / 0.20
}

fn camber(des: &Designation, x: f64) -> f64 {
    match *des {
        Designation::Four { m, p, .. } => {
            if x < p {
                m / (p * p) * (2.0 * p * x - x * x)
            } else {
                m / ((1.0 - p) * (1.0 - p)) * ((1.0 - 2.0 * p) + 2.0 * p * x - x * x)
            }
        }
        Designation::Five { camber: c, .. } => {
            let m = c.m;
            let base = match c.k21 {
                None => {
                    if x < m {
                        (c.k1 / 6.0) * (x.powi(3) - 3.0 * m * x * x + m * m * (3.0 - m) * x)
                    } else {
                        (c.k1 / 6.0) * m.powi(3) * (1.0 - x)
                    }
                }
                Some(r) => {
                    let common = -r * (1.0 - m).powi(3) * x - m.powi(3) * x + m.powi(3);
                    if x < m {
                        (c.k1 / 6.0) * ((x - m).powi(3) + common)
                    } else {
                        (c.k1 / 6.0) * (r * (x - m).powi(3) + common)
                    }
                }
            };
            c.scale * base
        }
    }
}

fn generate(des: &Designation, digits: &str, options: &NacaOptions) -> Result<Airfoil> {
    let nside = options.points_per_side;
    if nside < 3 {
        return Err(GeometryError::InvalidOption(format!(
            "points_per_side must be at least 3 (got {nside})"
        )));
    }
    let t = match *des {
        Designation::Four { t, .. } | Designation::Five { t, .. } => t,
    };
    if t <= 0.0 {
        return Err(GeometryError::InvalidNaca(format!(
            "{digits}: zero thickness"
        )));
    }

    let anp = AN + 1.0;
    let mut xx = Vec::with_capacity(nside);
    let mut yt = Vec::with_capacity(nside);
    let mut yc = Vec::with_capacity(nside);
    for i in 0..nside {
        let frac = i as f64 / (nside - 1) as f64;
        let xi = if i == nside - 1 {
            1.0
        } else {
            1.0 - anp * frac * (1.0 - frac).powf(AN) - (1.0 - frac).powf(anp)
        };
        let xt = match options.trailing_edge {
            TrailingEdge::Open => xi,
            TrailingEdge::Closed => xi * XTES,
        };
        xx.push(xi);
        yt.push(thickness(xt, t));
        yc.push(camber(des, xi));
    }

    let mut x = Vec::with_capacity(2 * nside - 1);
    let mut y = Vec::with_capacity(2 * nside - 1);
    for i in (0..nside).rev() {
        x.push(xx[i]);
        y.push(yc[i] + yt[i]);
    }
    for i in 1..nside {
        x.push(xx[i]);
        y.push(yc[i] - yt[i]);
    }
    Airfoil::new(format!("NACA {digits}"), x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(trailing_edge: TrailingEdge) -> NacaOptions {
        NacaOptions {
            points_per_side: 200,
            trailing_edge,
        }
    }

    #[test]
    fn naca0012_open_trailing_edge() {
        let af = naca("0012", &NacaOptions::default()).unwrap();
        assert_eq!(af.name(), "NACA 0012");
        assert_eq!(af.len(), 399);
        // Leading edge at the origin, trailing edge at x = 1.
        assert_eq!(af.x()[199], 0.0);
        assert_eq!(af.y()[199], 0.0);
        assert_eq!(af.x()[0], 1.0);
        // Standard trailing-edge thickness 2 * 0.00126 = 0.00252.
        assert!((af.trailing_edge_gap() - 0.002_52).abs() < 1e-12);
        // Symmetric.
        for i in 0..199 {
            assert_eq!(af.x()[i], af.x()[398 - i]);
            assert_eq!(af.y()[i], -af.y()[398 - i]);
        }
    }

    #[test]
    fn naca0012_closed_trailing_edge() {
        let af = naca("0012", &opts(TrailingEdge::Closed)).unwrap();
        assert!(af.trailing_edge_gap() < 1e-12, "{}", af.trailing_edge_gap());
    }

    #[test]
    fn thickness_polynomial_values() {
        // Maximum half-thickness of a 12% section is 0.0600 near x = 0.3.
        assert!((thickness(0.3, 0.12) - 0.060_012).abs() < 1e-5);
        assert!((thickness(1.0, 0.12) - 0.001_26).abs() < 1e-12);
        assert!(thickness(XTES, 0.12).abs() < 1e-12);
    }

    #[test]
    fn four_digit_camber_line() {
        let des = parse("2412").unwrap();
        assert!((camber(&des, 0.4) - 0.02).abs() < 1e-15);
        assert_eq!(camber(&des, 0.0), 0.0);
        assert!(camber(&des, 1.0).abs() < 1e-15);
        // Continuity at p.
        assert!((camber(&des, 0.4 - 1e-9) - camber(&des, 0.4 + 1e-9)).abs() < 1e-9);
    }

    #[test]
    fn five_digit_camber_lines() {
        // NACA 230xx: maximum camber 1.83% near x = 0.15 (XFOIL reports 0.018385).
        let des = parse("23012").unwrap();
        let (xmax, cmax) = (1..1000)
            .map(|i| f64::from(i) / 1000.0)
            .map(|x| (x, camber(&des, x)))
            .fold((0.0, 0.0), |a, b| if b.1 > a.1 { b } else { a });
        assert!((cmax - 0.018_38).abs() < 5e-5, "{cmax}");
        assert!((xmax - 0.15).abs() < 0.01, "{xmax}");
        assert!(camber(&des, 1.0).abs() < 1e-12);
        // Design lift digit scales the camber line.
        let des4 = parse("43012").unwrap();
        assert!((camber(&des4, 0.1) - 2.0 * camber(&des, 0.1)).abs() < 1e-14);
        // Reflexed camber lines vanish at both ends and are continuous at m.
        for code in ["22112", "23112", "24112", "25112"] {
            let d = parse(code).unwrap();
            assert!(camber(&d, 0.0).abs() < 1e-12, "{code}");
            assert!(camber(&d, 1.0).abs() < 1e-12, "{code}");
            if let Designation::Five { camber: c, .. } = d {
                let jump = camber(&d, c.m - 1e-9) - camber(&d, c.m + 1e-9);
                assert!(jump.abs() < 1e-8, "{code}");
            }
        }
    }

    #[test]
    fn designation_parsing() {
        assert!(naca("NACA 2412", &NacaOptions::default()).is_ok());
        assert!(naca(" naca23012 ", &NacaOptions::default()).is_ok());
        for bad in [
            "", "12", "123456", "24a2", "26012", "03012", "23212", "0000",
        ] {
            assert!(naca(bad, &NacaOptions::default()).is_err(), "{bad}");
        }
        let small = NacaOptions {
            points_per_side: 2,
            ..NacaOptions::default()
        };
        assert!(matches!(
            naca("0012", &small),
            Err(GeometryError::InvalidOption(_))
        ));
    }

    #[test]
    fn point_distribution_matches_xfoil() {
        // XFOIL: x(frac) = 1 - 2.5 frac (1-frac)^1.5 - (1-frac)^2.5
        let af = naca("0012", &NacaOptions::default()).unwrap();
        let frac: f64 = 50.0 / 199.0;
        let expected = 1.0 - 2.5 * frac * (1.0 - frac).powf(1.5) - (1.0 - frac).powf(2.5);
        assert!((af.x()[199 + 50] - expected).abs() < 1e-15);
    }
}
