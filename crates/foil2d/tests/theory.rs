//! Validation of the inviscid solver against theoretical results: symmetry,
//! thin-airfoil theory, exact potential-flow solutions (circle, ellipse,
//! Kármán–Trefftz airfoil) and convergence with the number of panels.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::f64::consts::PI;

use aero_foil2d::inviscid::{InviscidOptions, InviscidSolver, OperatingPoint};
use aero_geometry::{Airfoil, NacaOptions, Normalization, PanelingOptions, TrailingEdge, naca};

fn solver_for(airfoil: &Airfoil) -> InviscidSolver {
    InviscidSolver::new(airfoil, &InviscidOptions::default()).unwrap()
}

fn naca_solver(code: &str, n_nodes: usize) -> InviscidSolver {
    let af = naca(code, &NacaOptions::default())
        .unwrap()
        .paneled(&PanelingOptions {
            n_nodes,
            ..PanelingOptions::default()
        })
        .unwrap();
    solver_for(&af)
}

fn cl(solver: &InviscidSolver, alpha_deg: f64) -> f64 {
    solver
        .solve(OperatingPoint::incompressible(alpha_deg))
        .unwrap()
        .cl
}

#[test]
fn symmetric_airfoils_have_no_lift_at_zero_incidence() {
    for code in ["0006", "0012", "0018"] {
        for te in [TrailingEdge::Open, TrailingEdge::Closed] {
            let af = naca(
                code,
                &NacaOptions {
                    trailing_edge: te,
                    ..NacaOptions::default()
                },
            )
            .unwrap()
            .paneled(&PanelingOptions::default())
            .unwrap();
            let s = solver_for(&af);
            let sol = s.solve(OperatingPoint::incompressible(0.0)).unwrap();
            assert!(sol.cl.abs() < 1e-10, "NACA {code} {te:?}: Cl = {}", sol.cl);
            assert!(sol.cm.abs() < 1e-10, "NACA {code} {te:?}: Cm = {}", sol.cm);
            for alpha in [2.0, 7.5] {
                assert!((cl(&s, alpha) + cl(&s, -alpha)).abs() < 1e-10);
            }
        }
    }
}

fn slope_ratio(code: &str, n_nodes: usize) -> f64 {
    naca_solver(code, n_nodes)
        .solve(OperatingPoint::incompressible(0.0))
        .unwrap()
        .cl_alpha
        / (2.0 * PI)
}

#[test]
fn thin_airfoil_lift_slope_is_two_pi() {
    // A 1% thick section (its tiny nose radius needs fine paneling).
    let ratio = slope_ratio("0001", 640);
    assert!(
        (ratio - 1.0).abs() < 0.01,
        "NACA 0001: dCl/da = {ratio} x 2pi"
    );

    // The zero-thickness limit of the slope is 2 pi.
    let (r2, r4) = (slope_ratio("0002", 320), slope_ratio("0004", 320));
    let intercept = r2 - (r4 - r2);
    assert!(
        (intercept - 1.0).abs() < 3e-3,
        "extrapolated slope {intercept} x 2pi"
    );
}

#[test]
fn lift_slope_grows_with_thickness() {
    // Thickness increases the slope roughly like 2 pi (1 + k t/c), with
    // k = 0.77 for Joukowski sections and slightly more for NACA sections.
    let mut previous = 2.0 * PI;
    for (code, t) in [
        ("0006", 0.06),
        ("0009", 0.09),
        ("0012", 0.12),
        ("0015", 0.15),
        ("0018", 0.18),
    ] {
        let slope = naca_solver(code, 160)
            .solve(OperatingPoint::incompressible(0.0))
            .unwrap()
            .cl_alpha;
        let factor = (slope / (2.0 * PI) - 1.0) / t;
        assert!(slope > previous, "slope must grow with thickness");
        assert!(
            factor > 0.65 && factor < 0.9,
            "NACA {code}: (slope/2pi - 1)/t = {factor}"
        );
        previous = slope;
    }
}

#[test]
fn thin_cambered_airfoil_zero_lift_angle() {
    // Thin-airfoil theory: alpha_L0 = -(1/pi) int_0^pi dyc/dx (cos th - 1) dth
    // for the NACA 24xx mean line (m = 0.02, p = 0.4).
    let (m, p) = (0.02_f64, 0.4_f64);
    let slope = |x: f64| {
        if x < p {
            2.0 * m / (p * p) * (p - x)
        } else {
            2.0 * m / ((1.0 - p) * (1.0 - p)) * (p - x)
        }
    };
    let k = 200_000;
    let alpha_l0: f64 = -(0..k)
        .map(|i| {
            let th = PI * (f64::from(i) + 0.5) / f64::from(k);
            slope(0.5 * (1.0 - th.cos())) * (th.cos() - 1.0)
        })
        .sum::<f64>()
        / f64::from(k);
    let theory_deg = alpha_l0.to_degrees();

    let s = naca_solver("2401", 160);
    let (c0, c1) = (cl(&s, 0.0), cl(&s, 1.0));
    let panel_deg = -c0 / (c1 - c0);
    assert!(
        (panel_deg - theory_deg).abs() < 0.05,
        "panel {panel_deg} vs theory {theory_deg}"
    );
}

/// Ellipse of chord 1 and thickness ratio `t`, x = 0.5 (1 + cos th),
/// y = 0.5 t sin th, from th = d to 2 pi - d (a tiny blunt "trailing edge").
fn ellipse(t: f64, n: usize, d: f64) -> (Airfoil, Vec<f64>) {
    let thetas: Vec<f64> = (0..n)
        .map(|i| d + (2.0 * PI - 2.0 * d) * i as f64 / (n - 1) as f64)
        .collect();
    let x = thetas.iter().map(|th| 0.5 * (1.0 + th.cos())).collect();
    let y = thetas.iter().map(|th| 0.5 * t * th.sin()).collect();
    (Airfoil::new("ellipse", x, y).unwrap(), thetas)
}

/// Exact surface speed on an ellipse (semi-axes a, b) with the Kutta
/// condition at the rear point, by the Joukowski map of a circle.
fn ellipse_speed(a: f64, b: f64, theta: f64, alpha: f64) -> f64 {
    let k = (a - b) / (a + b);
    let jac = (1.0 - 2.0 * k * (2.0 * theta).cos() + k * k).sqrt();
    2.0 * ((theta - alpha).sin() + alpha.sin()).abs() / jac
}

#[test]
fn circle_pressure_distribution() {
    let (af, thetas) = ellipse(1.0, 161, 0.01);
    let s = solver_for(&af);
    let sol = s.solve(OperatingPoint::incompressible(0.0)).unwrap();
    let max_err = thetas
        .iter()
        .zip(&sol.cp)
        .map(|(th, cp)| (cp - (1.0 - 4.0 * th.sin().powi(2))).abs())
        .fold(0.0_f64, f64::max);
    assert!(max_err < 2e-3, "max |Cp - Cp_exact| = {max_err}");
}

#[test]
fn ellipse_pressure_and_lift() {
    let t = 0.12;
    let (af, thetas) = ellipse(t, 321, 0.01);
    let s = solver_for(&af);
    for alpha_deg in [0.0, 4.0] {
        let alpha = f64::to_radians(alpha_deg);
        let sol = s.solve(OperatingPoint::incompressible(alpha_deg)).unwrap();
        let max_err = thetas
            .iter()
            .zip(&sol.cp)
            .filter(|(th, _)| th.sin().abs() > 0.1) // away from the opened rear point
            .map(|(th, cp)| (cp - (1.0 - ellipse_speed(0.5, 0.5 * t, *th, alpha).powi(2))).abs())
            .fold(0.0_f64, f64::max);
        assert!(
            max_err < 5e-3,
            "alpha {alpha_deg}: max |Cp - Cp_exact| = {max_err}"
        );
    }
    for alpha_deg in [2.0_f64, 4.0, 6.0] {
        let exact = 2.0 * PI * (1.0 + t) * alpha_deg.to_radians().sin();
        let got = cl(&s, alpha_deg);
        assert!(
            (got / exact - 1.0).abs() < 1e-3,
            "alpha {alpha_deg}: Cl {got} vs {exact}"
        );
    }
}

/// Minimal complex arithmetic for the conformal mapping.
#[derive(Clone, Copy, Debug)]
struct C(f64, f64);

impl C {
    fn polar(r: f64, th: f64) -> Self {
        C(r * th.cos(), r * th.sin())
    }
    fn abs(self) -> f64 {
        self.0.hypot(self.1)
    }
    fn arg(self) -> f64 {
        self.1.atan2(self.0)
    }
    fn add(self, o: C) -> C {
        C(self.0 + o.0, self.1 + o.1)
    }
    fn sub(self, o: C) -> C {
        C(self.0 - o.0, self.1 - o.1)
    }
    fn mul(self, o: C) -> C {
        C(self.0 * o.0 - self.1 * o.1, self.0 * o.1 + self.1 * o.0)
    }
    fn div(self, o: C) -> C {
        let d = o.0 * o.0 + o.1 * o.1;
        C(
            (self.0 * o.0 + self.1 * o.1) / d,
            (self.1 * o.0 - self.0 * o.1) / d,
        )
    }
    fn scale(self, s: f64) -> C {
        C(self.0 * s, self.1 * s)
    }
}

/// Kármán–Trefftz airfoil: returns the contour (Selig order, sharp TE), the
/// exact Cp at each point for `alpha`, and the exact lift coefficient.
fn karman_trefftz(alpha: f64, n_points: usize) -> (Airfoil, Vec<f64>, f64) {
    let b = 1.0;
    let mu = C(-0.08, 0.08);
    let tau = 10.0_f64.to_radians();
    let nexp = 2.0 - tau / PI;
    let radius = C(b, 0.0).sub(mu).abs();
    let beta = -C(b, 0.0).sub(mu).arg();
    let theta_te = -beta;
    let gamma = 4.0 * PI * radius * (alpha + beta).sin();

    let mut x = Vec::with_capacity(n_points);
    let mut y = Vec::with_capacity(n_points);
    let mut speed = Vec::with_capacity(n_points);
    let mut warg = 0.0;
    for k in 0..n_points {
        let th = theta_te + 2.0 * PI * k as f64 / (n_points - 1) as f64;
        if k == 0 || k == n_points - 1 {
            x.push(nexp * b);
            y.push(0.0);
            speed.push(0.0);
            continue;
        }
        let zeta = mu.add(C::polar(radius, th));
        let w = zeta.sub(C(b, 0.0)).div(zeta.add(C(b, 0.0)));
        // Continuous argument of w along the contour.
        let a = w.arg();
        warg = if k == 1 {
            a
        } else {
            let mut d = a - warg.rem_euclid(2.0 * PI);
            d -= 2.0 * PI * (d / (2.0 * PI)).round();
            warg + d
        };
        let wn = C::polar(w.abs().powf(nexp), nexp * warg);
        let one = C(1.0, 0.0);
        let z = one.add(wn).div(one.sub(wn)).scale(nexp * b);
        x.push(z.0);
        y.push(z.1);

        // dz/dzeta = 4 n^2 b^2 w^(n-1) / ((zeta + b)^2 (1 - w^n)^2)
        let wn1 = wn.div(w);
        let zb = zeta.add(C(b, 0.0));
        let den = zb.mul(zb).mul(one.sub(wn)).mul(one.sub(wn));
        let dzdzeta = wn1.div(den).scale(4.0 * nexp * nexp * b * b);
        // dF/dzeta = U (e^{-i a} - R^2 e^{i a} / (zeta - mu)^2) + i Gamma / (2 pi (zeta - mu))
        let zm = zeta.sub(mu);
        let dfdzeta = C::polar(1.0, -alpha)
            .sub(C::polar(radius * radius, alpha).div(zm.mul(zm)))
            .add(C(0.0, gamma / (2.0 * PI)).div(zm));
        speed.push(dfdzeta.abs() / dzdzeta.abs());
    }
    let airfoil = Airfoil::new("Karman-Trefftz", x, y).unwrap();
    let chord = airfoil.chord();
    let cp = speed.iter().map(|q| 1.0 - q * q).collect();
    (airfoil, cp, 2.0 * gamma / chord)
}

/// Maximum Cp error (away from the trailing-edge stagnation point) and lift
/// ratio for a Kármán–Trefftz airfoil discretized with `n` points.
fn karman_trefftz_errors(alpha_deg: f64, n: usize) -> (f64, f64, f64) {
    let (af, cp_exact, cl_exact) = karman_trefftz(alpha_deg.to_radians(), n);
    // Normalizing (translation and scaling, no rotation) keeps alpha and the
    // coefficients unchanged.
    let normalized = af.normalized(Normalization::Xfoil).unwrap();
    let s = solver_for(&normalized);
    assert!(s.geometry().trailing_edge_panel().sharp);
    let sol = s.solve(OperatingPoint::incompressible(alpha_deg)).unwrap();
    let (max_err, peak) = cp_exact
        .iter()
        .zip(&sol.cp)
        .zip(normalized.x())
        .filter(|(_, x)| **x < 0.97)
        .fold((0.0_f64, 0.0_f64), |(e, p), ((ce, c), _)| {
            (e.max((ce - c).abs()), p.max(ce.abs()))
        });
    (max_err, peak, sol.cl / cl_exact)
}

#[test]
fn karman_trefftz_airfoil_matches_exact_solution() {
    for alpha_deg in [0.0, 4.0, 8.0] {
        let (max_err, peak, cl_ratio) = karman_trefftz_errors(alpha_deg, 241);
        assert!(
            (cl_ratio - 1.0).abs() < 5e-4,
            "alpha {alpha_deg}: Cl ratio {cl_ratio}"
        );
        assert!(
            max_err < 0.01 * (1.0 + peak),
            "alpha {alpha_deg}: max |Cp - Cp_exact| = {max_err}"
        );
    }
    // The largest error, at the suction peak, converges at second order.
    let (coarse, _, _) = karman_trefftz_errors(8.0, 241);
    let (fine, _, _) = karman_trefftz_errors(8.0, 481);
    assert!(fine < 0.3 * coarse, "Cp error {coarse} -> {fine}");
}

#[test]
fn lift_converges_with_panel_count() {
    let counts = [40, 80, 160, 320, 640];
    let lifts: Vec<f64> = counts
        .iter()
        .map(|&n| cl(&naca_solver("2412", n), 4.0))
        .collect();
    let diffs: Vec<f64> = lifts.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
    for w in diffs.windows(2) {
        assert!(w[1] < w[0], "Cl differences must decrease: {lifts:?}");
    }
    // Observed order of convergence and Richardson extrapolation.
    let order = (diffs[2] / diffs[3]).log2();
    // The trailing-edge treatment makes the lift converge at about first
    // order (the Cp distribution itself converges at second order, see the
    // Karman-Trefftz test).
    assert!(order > 1.0, "observed order {order}, lifts {lifts:?}");
    let extrapolated = lifts[4] + (lifts[4] - lifts[3]) / (2f64.powf(order) - 1.0);
    let error_160 = (lifts[2] / extrapolated - 1.0).abs();
    assert!(
        error_160 < 2e-3,
        "Cl(160) error {error_160}, lifts {lifts:?}"
    );
    println!("lifts {lifts:?}, order {order:.2}, extrapolated {extrapolated:.6}");
}
