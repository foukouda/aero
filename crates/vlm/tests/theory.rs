//! Checks of the vortex lattice that do not depend on AVL: lifting-line
//! theory, symmetry, and derivatives against finite differences.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::f64::consts::PI;

use aero_vlm::{
    Camber, ControlSurface, FlowCondition, LiftingSurface, Reference, Spacing, SurfaceSection,
    VlmModel, VortexCore, VortexLattice,
};

/// A flat wing of span `b` whose chord at y is `chord(y)`, quarter-chord
/// line straight, `n` sections closer together towards the tip, `per`
/// strips between sections.
fn wing(b: f64, n: usize, per: usize, chord: impl Fn(f64) -> f64) -> LiftingSurface {
    let sections = (0..n)
        .map(|k| {
            let y = 0.5 * b * (0.5 * PI * k as f64 / (n - 1) as f64).sin();
            let c = chord(y).max(1e-3);
            let mut s = SurfaceSection::new([-0.25 * c, y, 0.0], c);
            s.spanwise = Some(Spacing::new(per, 0.0));
            s
        })
        .collect();
    LiftingSurface::new("Wing", Spacing::new(8, 1.0), sections)
}

fn model(surface: LiftingSurface, area: f64, chord: f64, span: f64, y_symmetry: bool) -> VlmModel {
    VlmModel {
        title: "test".into(),
        mach: 0.0,
        y_symmetry,
        reference: Reference {
            area,
            chord,
            span,
            point: [0.0, 0.0, 0.0],
        },
        cd_p: 0.0,
        core: VortexCore::default(),
        surfaces: vec![surface],
    }
}

#[test]
fn elliptic_wing_has_the_lifting_line_slope_and_unit_efficiency() {
    let (b, c0) = (10.0, 1.0);
    let area = PI * b * c0 / 4.0;
    let ar = b * b / area;
    let w = wing(b, 40, 3, |y| {
        c0 * (1.0 - (2.0 * y / b).powi(2)).max(0.0).sqrt()
    });
    let lattice = VortexLattice::new(&model(w, area, area / b, b, true)).unwrap();
    let s = lattice.solve(&FlowCondition::alpha(4.0)).unwrap();
    let t = &s.totals;
    // Lifting surface theory: Helmbold's CLα = 2π A / (2 + √(A² + 4)),
    // below the lifting line's 2π A / (A + 2); and e = 1.
    let helmbold = 2.0 * PI * ar / (2.0 + (ar * ar + 4.0).sqrt());
    let cla = s.derivatives.alpha.cl;
    assert!(
        (cla / helmbold - 1.0).abs() < 0.025,
        "CLα {cla} vs {helmbold}"
    );
    assert!(cla < 2.0 * PI * ar / (ar + 2.0));
    assert!(
        (t.span_efficiency - 1.0).abs() < 0.01,
        "e {}",
        t.span_efficiency
    );
    // Trefftz drag CL²/(π A e) and near-field drag agree.
    assert!((t.cd_trefftz - t.cl_trefftz.powi(2) / (PI * ar * t.span_efficiency)).abs() < 1e-12);
    assert!(
        (t.cd - t.cd_trefftz).abs() < 0.03 * t.cd_trefftz,
        "{} vs {}",
        t.cd,
        t.cd_trefftz
    );
    // The span loading is elliptic.
    let root = s.strips.first().unwrap();
    let shape = |y: f64| (1.0 - (2.0 * y / b).powi(2)).sqrt();
    for st in s.strips.iter().filter(|st| st.leading_edge[1] < 0.3 * b) {
        let ratio = st.chord_cl / root.chord_cl;
        let expected = shape(st.leading_edge[1]) / shape(root.leading_edge[1]);
        assert!(
            (ratio / expected - 1.0).abs() < 0.01,
            "y {}: {ratio} vs {expected}",
            st.leading_edge[1]
        );
    }
}

#[test]
fn lift_slope_tends_to_the_lifting_line_value_as_the_aspect_ratio_grows() {
    // Rectangular wings: below the elliptic lifting line, closer and closer
    // as the aspect ratio grows (14 % at A = 4, under 4 % at A = 32).
    let mut last_error = f64::INFINITY;
    for ar in [4.0, 8.0, 16.0, 32.0] {
        let b = ar;
        let mut root = SurfaceSection::new([0.0; 3], 1.0);
        root.spanwise = Some(Spacing::new(40, -2.0));
        let tip = SurfaceSection::new([0.0, 0.5 * b, 0.0], 1.0);
        let w = LiftingSurface::new("Wing", Spacing::new(8, 1.0), vec![root, tip]);
        let lattice = VortexLattice::new(&model(w, b, 1.0, b, true)).unwrap();
        let s = lattice.solve(&FlowCondition::alpha(2.0)).unwrap();
        let cla = 2.0 * PI * ar / (ar + 2.0);
        let error = 1.0 - s.derivatives.alpha.cl / cla;
        assert!(
            error > 0.0 && error < last_error,
            "A {ar}: {}",
            s.derivatives.alpha.cl
        );
        last_error = error;
    }
    assert!(last_error < 0.04);
}

/// A swept, twisted, cambered half wing with an aileron-like control.
fn swept_wing() -> LiftingSurface {
    let camber = Camber::naca4(4412, (0.0, 1.0)).unwrap();
    let mut root = SurfaceSection::new([0.0, 0.0, 0.0], 1.0);
    root.incidence_deg = 2.0;
    root.spanwise = Some(Spacing::new(12, 1.0));
    root.camber = camber.clone();
    let mut tip = SurfaceSection::new([0.8, 3.0, 0.3], 0.5);
    tip.incidence_deg = -2.0;
    tip.camber = camber;
    let flap = ControlSurface::new("flap", 0.7);
    root.controls.push(flap.clone());
    tip.controls.push(flap);
    LiftingSurface::new("Wing", Spacing::new(8, 1.0), vec![root, tip])
}

#[test]
fn half_model_on_the_symmetry_plane_equals_the_whole_model() {
    let half = VortexLattice::new(&model(swept_wing(), 4.5, 0.75, 6.0, true)).unwrap();
    let mut whole = swept_wing();
    whole.y_duplicate = Some(0.0);
    let whole = VortexLattice::new(&model(whole, 4.5, 0.75, 6.0, false)).unwrap();
    let flow = FlowCondition {
        pitch_rate: 0.03,
        controls: vec![3.0],
        ..FlowCondition::alpha(5.0)
    };
    let a = half.solve(&flow).unwrap();
    let b = whole.solve(&flow).unwrap();
    let close = |x: f64, y: f64| (x - y).abs() < 1e-11 * y.abs().max(1.0);
    for (x, y) in [
        (a.totals.cl, b.totals.cl),
        (a.totals.cd, b.totals.cd),
        (a.totals.pitch, b.totals.pitch),
        (a.totals.cd_trefftz, b.totals.cd_trefftz),
        (a.derivatives.alpha.cl, b.derivatives.alpha.cl),
        (
            a.derivatives.pitch_rate.pitch,
            b.derivatives.pitch_rate.pitch,
        ),
        (
            a.derivatives.controls[0].forces.cl,
            b.derivatives.controls[0].forces.cl,
        ),
    ] {
        assert!(close(x, y), "{x} vs {y}");
    }
    // In symmetric flight the whole model has no side force or roll.
    assert!(b.totals.cy.abs() < 1e-12 && b.totals.roll.abs() < 1e-12);
}

#[test]
fn derivatives_match_finite_differences() {
    let mut w = swept_wing();
    w.y_duplicate = Some(0.0);
    // Antisymmetric flap on the mirror image: an aileron.
    for s in &mut w.sections {
        s.controls[0].duplicate_sign = -1.0;
    }
    let mut m = model(w, 4.5, 0.75, 6.0, false);
    m.reference.point = [0.3, 0.0, 0.0];
    m.mach = 0.3;
    let lattice = VortexLattice::new(&m).unwrap();
    // Roll and yaw rates are zero: with them, AVL's alpha derivatives turn
    // the rates with the angle of attack with the opposite sign (see
    // `StabilityDerivatives::alpha`), which the port keeps.
    let base = FlowCondition {
        alpha_deg: 4.0,
        beta_deg: 3.0,
        roll_rate: 0.0,
        pitch_rate: 0.01,
        yaw_rate: 0.0,
        controls: vec![2.0],
    };
    let s0 = lattice.solve(&base).unwrap();
    let d = &s0.derivatives;
    // Coefficients in stability axes: CL, CY, CD, Cl', Cm, Cn'.
    let coeffs = |s: &aero_vlm::Solution| {
        let t = &s.totals;
        [t.cl, t.cy, t.cd, t.roll_stability, t.pitch, t.yaw_stability]
    };
    let derivs = |c: &aero_vlm::CoefficientDerivatives| [c.cl, c.cy, c.cd, c.roll, c.pitch, c.yaw];
    let fd = |f: &dyn Fn(&mut FlowCondition, f64), h: f64| -> [f64; 6] {
        let mut p = base.clone();
        f(&mut p, h);
        let mut m = base.clone();
        f(&mut m, -h);
        let (a, b) = (
            coeffs(&lattice.solve(&p).unwrap()),
            coeffs(&lattice.solve(&m).unwrap()),
        );
        std::array::from_fn(|k| (a[k] - b[k]) / (2.0 * h))
    };
    let deg = PI / 180.0;
    let checks: [(&str, [f64; 6], [f64; 6]); 6] = [
        (
            "alpha",
            derivs(&d.alpha),
            fd(&|f, h| f.alpha_deg += h / deg, 1e-4),
        ),
        (
            "beta",
            derivs(&d.beta),
            fd(&|f, h| f.beta_deg += h / deg, 1e-4),
        ),
        (
            "p",
            derivs(&d.roll_rate),
            fd(&|f, h| f.roll_rate += h, 1e-4),
        ),
        (
            "q",
            derivs(&d.pitch_rate),
            fd(&|f, h| f.pitch_rate += h, 1e-4),
        ),
        ("r", derivs(&d.yaw_rate), fd(&|f, h| f.yaw_rate += h, 1e-4)),
        (
            "flap",
            derivs(&d.controls[0].forces),
            fd(&|f, h| f.controls[0] += h, 1e-3),
        ),
    ];
    let mut failures = Vec::new();
    for (name, analytic, numeric) in checks {
        for k in 0..6 {
            if (analytic[k] - numeric[k]).abs() > 1e-6 * analytic[k].abs().max(1.0) {
                failures.push(format!("{name}[{k}]: {} vs {}", analytic[k], numeric[k]));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
    // About the neutral point Cm hardly changes with alpha (AVL's
    // Xref − c Cmα/CLα leaves out the drag in the moment transfer).
    let xnp = d.neutral_point.unwrap();
    let mut moved = m.clone();
    moved.reference.point[0] = xnp;
    let level = FlowCondition {
        pitch_rate: 0.0,
        ..base.clone()
    };
    let at_np = VortexLattice::new(&moved).unwrap().solve(&level).unwrap();
    assert!(at_np.derivatives.alpha.pitch.abs() < 0.01 * d.alpha.cl);
    assert!(
        d.alpha.pitch.abs() > 0.1 * d.alpha.cl,
        "the test needs a reference point away from it"
    );
}

#[test]
fn strip_corners_tile_the_surfaces() {
    let lattice = VortexLattice::new(&model(swept_wing(), 4.5, 0.75, 6.0, true)).unwrap();
    let corners = lattice.strip_corners();
    assert_eq!(corners.len(), lattice.strip_count());
    // Each strip's right edge is the next strip's left edge.
    for w in corners.windows(2) {
        let (a, b) = (w[0], w[1]);
        for k in 0..3 {
            assert!((a[3][k] - b[0][k]).abs() < 1e-12 && (a[2][k] - b[1][k]).abs() < 1e-12);
        }
    }
    // Together they cover the planform: trapezoids of the root and tip
    // chords (1 and 0.5) over the 3 m of half span.
    let area: f64 = corners
        .iter()
        .map(|c| (c[3][1] - c[0][1]).abs() * 0.5 * ((c[1][0] - c[0][0]) + (c[2][0] - c[3][0])))
        .sum();
    assert!((area - 3.0 * 0.75).abs() < 1e-12, "{area}");
}
