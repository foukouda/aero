//! Benchmarks of the optimizer: one design evaluation, and a short
//! optimization on one core and on all cores (NACA 2412, CL = 0.5,
//! Re = 1e6, t/c >= 12 %).

#![allow(missing_docs, clippy::unwrap_used)]

use std::hint::black_box;
use std::ops::ControlFlow;

use aero_geometry::{Airfoil, NacaOptions, naca};
use aero_optim::{
    Constraints, DesignPoint, Evaluator, Objective, OptimizeOptions, Problem, Target, optimize,
};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

fn setup() -> (Airfoil, Problem) {
    let airfoil = naca("2412", &NacaOptions::default()).unwrap();
    let problem = Problem {
        points: vec![DesignPoint {
            target: Target::Cl(0.5),
            weight: 1.0,
        }],
        objective: Objective::MinDrag,
        constraints: Constraints {
            min_thickness: Some(0.12),
            ..Constraints::default()
        },
        viscous: Default::default(),
        paneling: Default::default(),
        design: Default::default(),
    };
    (airfoil, problem)
}

fn evaluation(c: &mut Criterion) {
    let (airfoil, problem) = setup();
    let evaluator = Evaluator::new(&airfoil, &problem).unwrap();
    // A design near the initial one, which converges from a fresh
    // boundary layer.
    let mut x = evaluator.space().initial();
    x[2] += 0.005;
    assert!(evaluator.evaluate(&x).feasible());
    let mut group = c.benchmark_group("design_evaluation");
    group.sample_size(20);
    group.bench_function("one_point", |b| {
        b.iter(|| evaluator.evaluate(black_box(&x)));
    });
    group.finish();
}

fn optimization(c: &mut Criterion) {
    let (airfoil, problem) = setup();
    let mut group = c.benchmark_group("optimization_100_evaluations");
    group.sample_size(10);
    for threads in [1, 0] {
        let options = OptimizeOptions {
            max_evaluations: 100,
            threads,
            ..OptimizeOptions::default()
        };
        let label = if threads == 0 {
            "all_cores"
        } else {
            "one_core"
        };
        group.bench_with_input(BenchmarkId::from_parameter(label), &options, |b, o| {
            b.iter(|| optimize(&airfoil, &problem, o, |_| ControlFlow::Continue(())).unwrap());
        });
    }
    group.finish();
}

criterion_group!(benches, evaluation, optimization);
criterion_main!(benches);
