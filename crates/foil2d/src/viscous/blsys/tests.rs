//! Finite-difference checks of the interval Newton systems.

use super::*;

/// Primary variables of a station: `a` is N on laminar stations and √Cτ on
/// turbulent ones, `u` the incompressible edge velocity.
#[derive(Debug, Clone, Copy)]
struct Prim {
    x: f64,
    a: f64,
    t: f64,
    d: f64,
    u: f64,
}

impl Prim {
    /// Variable `l` in the column order of `VS1`/`VS2`.
    fn get(&self, l: usize) -> f64 {
        [self.a, self.t, self.d, self.u, self.x][l]
    }

    fn with(mut self, l: usize, v: f64) -> Self {
        match l {
            0 => self.a = v,
            1 => self.t = v,
            2 => self.d = v,
            3 => self.u = v,
            _ => self.x = v,
        }
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Similarity,
    Laminar,
    Transition,
    Turbulent,
    Wake,
}

/// Builds the interval system the way `SETBL` does: station 1 as left by the
/// previous interval, station 2 from its primaries.
fn interval(flow: Flow, par: BlParams, kind: Kind, p1: Prim, p2: Prim, xiforc: f64) -> Bl {
    let mut bl = Bl::new(flow, par);
    bl.amcrit = 9.0;
    bl.xiforc = xiforc;
    bl.bule = 1.0;
    let dw = if kind == Kind::Wake { 0.002 } else { 0.0 };
    let laminar1 = matches!(kind, Kind::Similarity | Kind::Laminar | Kind::Transition);
    let (a1, s1) = if laminar1 { (p1.a, 0.0) } else { (0.0, p1.a) };
    bl.blprv(p1.x, a1, s1, p1.t, p1.d + dw, dw, p1.u);
    bl.blkin();
    bl.blvar(match kind {
        Kind::Turbulent => Regime::Turbulent,
        Kind::Wake => Regime::Wake,
        _ => Regime::Laminar,
    });
    bl.s1 = bl.s2;

    bl.simi = kind == Kind::Similarity;
    bl.turb = matches!(kind, Kind::Turbulent | Kind::Wake);
    bl.wake = kind == Kind::Wake;
    let (a2, s2) = match kind {
        Kind::Similarity | Kind::Laminar => (p2.a, 0.0),
        Kind::Transition => (p1.a, p2.a),
        _ => (0.0, p2.a),
    };
    bl.blprv(p2.x, a2, s2, p2.t, p2.d + dw, dw, p2.u);
    bl.blkin();
    if kind == Kind::Transition {
        bl.trchek();
        assert!(bl.tran && bl.trfree, "the interval must contain transition");
    }
    bl.blsys();
    bl
}

fn residual(bl: &Bl) -> [f64; 3] {
    [-bl.sys.vsrez[0], -bl.sys.vsrez[1], -bl.sys.vsrez[2]]
}

fn central(f: impl Fn(f64) -> [f64; 3], v: f64, h: f64) -> [f64; 3] {
    let (p, m) = (f(v + h), f(v - h));
    [0, 1, 2].map(|k| (p[k] - m[k]) / (2.0 * h))
}

fn flow_msq(msq: f64, re: f64) -> Flow {
    Flow::new(1.0, msq.sqrt(), re)
}

/// How exactly the shear-lag row (row 0 of turbulent, wake and transition
/// intervals) is linearized.
#[derive(Debug, Clone, Copy, PartialEq)]
enum ShearLag {
    /// Exact, except for the Reynolds-number derivative.
    Exact,
    /// XFOIL drops dUQ/dRe_theta (through HKC = Hk - 1 - GCCON/Re_theta): a
    /// relative error of a few 1e-3 on the theta and Ue derivatives.
    Approximate,
}

/// Compares every analytic derivative of the interval residuals with central
/// differences.
fn check_jacobian(
    msq: f64,
    re: f64,
    kind: Kind,
    p1: Prim,
    p2: Prim,
    par: BlParams,
    row0: ShearLag,
) {
    let xiforc = 10.0;
    let flow = flow_msq(msq, re);
    let bl = interval(flow, par, kind, p1, p2, xiforc);
    let sys = bl.sys;
    let scale: [f64; 3] = [0, 1, 2].map(|k| {
        (0..5).fold(0.0_f64, |m, l| {
            m.max(sys.vs1[k][l].abs()).max(sys.vs2[k][l].abs())
        })
    });
    let mut failures = Vec::new();
    let turbulent = matches!(kind, Kind::Turbulent | Kind::Wake | Kind::Transition);
    let mut compare = |what: String, k: usize, analytic: f64, numeric: f64, s: f64| {
        let mut tol = 1e-6 * s + 1e-5 * numeric.abs();
        if k == 0 && turbulent && row0 == ShearLag::Approximate {
            tol = 1e-2 * s;
        }
        if (analytic - numeric).abs() > tol {
            failures.push(format!(
                "{what} row {k}: analytic {analytic:.6e} numeric {numeric:.6e}"
            ));
        }
    };

    // Station 1 does not exist at the similarity station.
    let stations: &[usize] = if kind == Kind::Similarity {
        &[2]
    } else {
        &[1, 2]
    };
    for &st in stations {
        for l in 0..5 {
            if kind == Kind::Similarity && l == 4 {
                continue; // xi only enters through the similarity relations
            }
            let base = if st == 1 { p1 } else { p2 };
            let v = base.get(l);
            let h = 1e-6 * v.abs().max(1e-6);
            let d = central(
                |vv| {
                    let (q1, q2) = if kind == Kind::Similarity {
                        (p1.with(l, vv), p2.with(l, vv))
                    } else if st == 1 {
                        (p1.with(l, vv), p2)
                    } else {
                        (p1, p2.with(l, vv))
                    };
                    residual(&interval(flow, par, kind, q1, q2, xiforc))
                },
                v,
                h,
            );
            for k in 0..3 {
                let a = if st == 1 {
                    sys.vs1[k][l]
                } else {
                    sys.vs2[k][l]
                };
                compare(format!("{kind:?} VS{st}[{l}]"), k, a, d[k], scale[k]);
            }
        }
    }

    // Freestream Mach number squared and Reynolds number.
    if msq > 0.0 {
        let d = central(
            |m| residual(&interval(flow_msq(m, re), par, kind, p1, p2, xiforc)),
            msq,
            1e-6 * msq,
        );
        for k in 0..3 {
            compare(format!("{kind:?} VSM"), k, sys.vsm[k], d[k], scale[k]);
        }
    }
    let d = central(
        |r| residual(&interval(flow_msq(msq, r), par, kind, p1, p2, xiforc)),
        re,
        1e-6 * re,
    );
    for k in 0..3 {
        // XFOIL also drops dUs/dRe in the shear-lag row.
        if !(k == 0 && turbulent) {
            compare(
                format!("{kind:?} VSR"),
                k,
                sys.vsr[k] * re,
                d[k] * re,
                scale[k],
            );
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

const LAMINAR: (Prim, Prim) = (
    Prim {
        x: 0.30,
        a: 3.0,
        t: 3.0e-4,
        d: 8.4e-4,
        u: 1.15,
    },
    Prim {
        x: 0.32,
        a: 3.5,
        t: 3.15e-4,
        d: 8.9e-4,
        u: 1.14,
    },
);
const TRANSITION: (Prim, Prim) = (
    Prim {
        x: 0.30,
        a: 8.8,
        t: 3.0e-4,
        d: 8.4e-4,
        u: 1.15,
    },
    Prim {
        x: 0.32,
        a: 0.07,
        t: 3.3e-4,
        d: 7.0e-4,
        u: 1.14,
    },
);
const TURBULENT: (Prim, Prim) = (
    Prim {
        x: 0.50,
        a: 0.060,
        t: 1.5e-3,
        d: 2.25e-3,
        u: 1.05,
    },
    Prim {
        x: 0.52,
        a: 0.062,
        t: 1.6e-3,
        d: 2.45e-3,
        u: 1.04,
    },
);
const WAKE: (Prim, Prim) = (
    Prim {
        x: 1.05,
        a: 0.050,
        t: 3.0e-3,
        d: 5.6e-3,
        u: 0.95,
    },
    Prim {
        x: 1.10,
        a: 0.048,
        t: 3.05e-3,
        d: 5.3e-3,
        u: 0.96,
    },
);
const SIMILARITY: Prim = Prim {
    x: 0.002,
    a: 0.0,
    t: 1.94e-5,
    d: 4.3e-5,
    u: 0.4,
};

/// XFOIL's constants without the wall term of the G-beta locus, which makes
/// the shear-lag linearization exact.
fn without_wall_term() -> BlParams {
    BlParams {
        gccon: 0.0,
        ..BlParams::default()
    }
}

#[test]
fn laminar_interval_jacobian() {
    let (p1, p2) = LAMINAR;
    for msq in [0.0, 0.09] {
        check_jacobian(
            msq,
            1e6,
            Kind::Laminar,
            p1,
            p2,
            BlParams::default(),
            ShearLag::Exact,
        );
    }
}

#[test]
fn similarity_station_jacobian() {
    let par = BlParams::default();
    check_jacobian(
        0.09,
        1e6,
        Kind::Similarity,
        SIMILARITY,
        SIMILARITY,
        par,
        ShearLag::Exact,
    );
}

#[test]
fn turbulent_interval_jacobian() {
    let (p1, p2) = TURBULENT;
    for msq in [0.0, 0.09] {
        check_jacobian(
            msq,
            1e6,
            Kind::Turbulent,
            p1,
            p2,
            without_wall_term(),
            ShearLag::Exact,
        );
        let par = BlParams::default();
        check_jacobian(
            msq,
            1e6,
            Kind::Turbulent,
            p1,
            p2,
            par,
            ShearLag::Approximate,
        );
    }
}

#[test]
fn wake_interval_jacobian() {
    // GCCON is not used in the wake.
    let (p1, p2) = WAKE;
    check_jacobian(
        0.09,
        1e6,
        Kind::Wake,
        p1,
        p2,
        BlParams::default(),
        ShearLag::Exact,
    );
}

#[test]
fn transition_interval_jacobian() {
    let (p1, p2) = TRANSITION;
    for msq in [0.0, 0.09] {
        check_jacobian(
            msq,
            1e6,
            Kind::Transition,
            p1,
            p2,
            without_wall_term(),
            ShearLag::Exact,
        );
        let par = BlParams::default();
        check_jacobian(
            msq,
            1e6,
            Kind::Transition,
            p1,
            p2,
            par,
            ShearLag::Approximate,
        );
    }
}

#[test]
fn transition_point_is_where_n_reaches_ncrit() {
    let (p1, p2) = TRANSITION;
    let bl = interval(
        flow_msq(0.0, 1e6),
        BlParams::default(),
        Kind::Transition,
        p1,
        p2,
        10.0,
    );
    assert!(bl.tr.xt > p1.x && bl.tr.xt < p2.x, "{:?}", bl.tr);
    assert!(bl.s2.ampl > 9.0);

    // A trip upstream of the free transition point takes over.
    let xf = p1.x + 0.1 * (bl.tr.xt - p1.x);
    let mut forced = Bl::new(flow_msq(0.0, 1e6), BlParams::default());
    forced.amcrit = 9.0;
    forced.xiforc = xf;
    forced.blprv(p1.x, p1.a, 0.0, p1.t, p1.d, 0.0, p1.u);
    forced.blkin();
    forced.blvar(Regime::Laminar);
    forced.s1 = forced.s2;
    forced.blprv(p2.x, p1.a, p2.a, p2.t, p2.d, 0.0, p2.u);
    forced.blkin();
    forced.trchek();
    assert!(forced.tran && forced.trforc && !forced.trfree);
    assert_eq!(forced.tr.xt, xf);
    assert_eq!(forced.tr.xt_xf, 1.0);
}

#[test]
fn gauss_solves_with_pivoting() {
    // A zero leading pivot forces a row exchange.
    let a = [
        [0.0, 2.0, 1.0, 0.0],
        [1.0, 1.0, 0.0, 2.0],
        [3.0, 0.0, 1.0, 1.0],
        [0.0, 1.0, 4.0, 1.0],
    ];
    let x = [1.0, -2.0, 0.5, 3.0];
    let mut z = [[0.0; 5]; 4];
    let mut r = [0.0; 4];
    for k in 0..4 {
        for l in 0..4 {
            z[k][l] = a[k][l];
            r[k] += a[k][l] * x[l];
        }
        z[k][4] = 99.0; // the fifth column is ignored
    }
    gauss4(&mut z, &mut r);
    for k in 0..4 {
        assert!((r[k] - x[k]).abs() < 1e-14, "{r:?}");
    }
}

#[test]
fn freestream_quantities() {
    let f = Flow::new(1.0, 0.0, 2e6);
    assert_eq!(
        (f.tk, f.hstinv, f.rst, f.reybl, f.hvrat),
        (0.0, 0.0, 1.0, 2e6, 0.0)
    );
    assert_eq!(f.edge_msq(1.3), 0.0);
    // The edge Mach number at the freestream speed is the freestream Mach.
    let f = Flow::new(1.0, 0.5, 1e6);
    assert!((f.edge_msq(1.0) - 0.25).abs() < 1e-14);
}
