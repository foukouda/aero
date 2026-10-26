//! End-to-end optimizations, with the optimized airfoils checked by
//! independent analyses.

#![allow(clippy::unwrap_used)]

use std::ops::ControlFlow;

use aero_foil2d::inviscid::ClSweep;
use aero_foil2d::viscous::{ViscousOptions, ViscousSolver};
use aero_geometry::{
    Airfoil, NacaOptions, PanelingOptions, naca, panel, parse_dat, to_selig_string,
};
use aero_optim::{
    Constraints, DesignPoint, Method, Objective, OptimizationResult, OptimizeOptions, Phase,
    Problem, Stop, Target, optimize,
};

fn problem(cl: f64, constraints: Constraints) -> Problem {
    Problem {
        points: vec![DesignPoint {
            target: Target::Cl(cl),
            weight: 1.0,
        }],
        objective: Objective::MinDrag,
        constraints,
        viscous: ViscousOptions::default(),
        paneling: PanelingOptions::default(),
        design: Default::default(),
    }
}

fn run(airfoil: &Airfoil, problem: &Problem, options: &OptimizeOptions) -> OptimizationResult {
    optimize(airfoil, problem, options, |_| ControlFlow::Continue(())).unwrap()
}

/// Drag at a prescribed lift, reached by a CSEQ sweep from CL = 0: a
/// different solution path from the optimizer's fresh boundary layer.
fn drag_by_sweep(airfoil: &Airfoil, nodes: usize, cl: f64) -> f64 {
    let paneling = PanelingOptions {
        n_nodes: nodes,
        ..PanelingOptions::default()
    };
    let paneled = panel(airfoil, &paneling).unwrap();
    let solver = ViscousSolver::new(&paneled, &ViscousOptions::default()).unwrap();
    let polar = solver
        .sweep_cl(&ClSweep {
            start: 0.0,
            end: cl,
            step: 0.05,
        })
        .unwrap();
    let last = polar.points.last().unwrap();
    assert!((last.cl - cl).abs() < 1e-6, "{nodes} nodes: {polar:?}");
    last.cd
}

#[test]
fn drag_reduction_at_prescribed_lift_survives_independent_analysis() {
    let naca2412 = naca("2412", &NacaOptions::default()).unwrap();
    let p = problem(
        0.5,
        Constraints {
            min_thickness: Some(0.12),
            ..Constraints::default()
        },
    );
    let r = run(
        &naca2412,
        &p,
        &OptimizeOptions {
            max_evaluations: 300,
            ..OptimizeOptions::default()
        },
    );
    // Nelder-Mead does not start a step it could not finish.
    assert!((290..=300).contains(&r.history.len()));
    assert!(r.fit_max_error < 1e-3);
    let cd0 = r.initial.objective.unwrap();
    let cd = r.best.objective.unwrap();
    assert!(r.best.feasible());
    println!(
        "Cd {cd0:.6} -> {cd:.6} ({:.1} %), upper transition {:.3} -> {:.3}",
        100.0 * (1.0 - cd / cd0),
        r.initial.points[0].values.unwrap().transition[0],
        r.best.points[0].values.unwrap().transition[0],
    );
    assert!(cd < 0.9 * cd0, "{cd0} {cd}");

    // The airfoil as a user gets it: written to a .dat file and read back.
    let (exported, _) = parse_dat(&to_selig_string(&r.airfoil), "optimized").unwrap();
    let thickness = exported.properties().max_thickness;
    assert!(thickness >= 0.12 - 1e-6, "{thickness}");
    let cd_check = drag_by_sweep(&exported, 160, 0.5);
    assert!((cd_check / cd - 1.0).abs() < 1e-3, "{cd} {cd_check}");

    // Not an artifact of the discretization: the gain holds with 240
    // panels, which neither airfoil was optimized with.
    let initial = r.initial.x.clone();
    let space = aero_optim::DesignSpace::new(&naca2412, &p.design).unwrap();
    let start = space.airfoil(&initial, "fitted").unwrap();
    let fine0 = drag_by_sweep(&start, 240, 0.5);
    let fine = drag_by_sweep(&exported, 240, 0.5);
    println!("240 panels: Cd {fine0:.6} -> {fine:.6}");
    assert!(fine < 0.9 * fine0, "{fine0} {fine}");
}

#[test]
fn infeasible_start_is_made_feasible() {
    // The NACA 2412 is 12 % thick and has Cm = -0.05 at CL = 0.4: both
    // constraints are violated at the start.
    let p = problem(
        0.4,
        Constraints {
            min_thickness: Some(0.13),
            min_cm: Some(-0.04),
            ..Constraints::default()
        },
    );
    let r = run(
        &naca("2412", &NacaOptions::default()).unwrap(),
        &p,
        &OptimizeOptions {
            max_evaluations: 200,
            ..OptimizeOptions::default()
        },
    );
    assert!(!r.initial.feasible());
    assert!(r.best.feasible(), "{:?}", r.best);
    let g = r.best.geometry.as_ref().unwrap();
    let v = r.best.points[0].values.unwrap();
    assert!(g.max_thickness >= 0.13 && v.cm >= -0.04, "{g:?} {v:?}");
    assert!(r.airfoil.properties().max_thickness >= 0.13);
}

#[test]
fn deterministic_whatever_the_threads() {
    let p = problem(0.5, Constraints::default());
    let af = naca("4412", &NacaOptions::default()).unwrap();
    let options = |threads| OptimizeOptions {
        max_evaluations: 60,
        threads,
        ..OptimizeOptions::default()
    };
    let a = run(&af, &p, &options(1));
    let b = run(&af, &p, &options(3));
    assert_eq!(a, b);
    // Both phases ran: 1 initial, 4 CMA-ES generations of 11, then the
    // Nelder-Mead simplex and steps.
    let phases = |phase| a.history.iter().filter(|h| h.phase == phase).count();
    assert_eq!((phases(Phase::Initial), phases(Phase::Global)), (1, 44));
    assert!(phases(Phase::Local) >= 14);
    assert_eq!(a.stop, Stop::MaxEvaluations);
}

#[test]
fn progress_and_cancellation() {
    let p = problem(0.5, Constraints::default());
    let af = naca("0012", &NacaOptions::default()).unwrap();
    let mut reports = Vec::new();
    let r = optimize(
        &af,
        &p,
        &OptimizeOptions {
            method: Method::CmaEs,
            ..OptimizeOptions::default()
        },
        |progress| {
            reports.push((progress.phase, progress.evaluations, progress.new.len()));
            if progress.phase == Phase::Global {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        },
    )
    .unwrap();
    assert_eq!(r.stop, Stop::Cancelled);
    assert_eq!(reports, [(Phase::Initial, 1, 1), (Phase::Global, 12, 11)]);
    assert_eq!(r.history.len(), 12);
    assert!(r.best.objective <= r.initial.objective);
}
