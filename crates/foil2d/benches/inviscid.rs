//! Benchmarks of the inviscid solver: assembly and factorization per
//! geometry, cost per angle of attack, sweeps, and paneling.

#![allow(missing_docs, clippy::unwrap_used)]

use std::hint::black_box;

use aero_foil2d::inviscid::{AlphaSweep, InviscidOptions, InviscidSolver, OperatingPoint};
use aero_geometry::{Airfoil, NacaOptions, PanelingOptions, naca, panel};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

fn paneled(n_nodes: usize) -> Airfoil {
    let base = naca("2412", &NacaOptions::default()).unwrap();
    panel(
        &base,
        &PanelingOptions {
            n_nodes,
            ..PanelingOptions::default()
        },
    )
    .unwrap()
}

fn factorization(c: &mut Criterion) {
    let mut group = c.benchmark_group("factorize");
    for n in [80, 160, 320] {
        let airfoil = paneled(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &airfoil, |b, af| {
            b.iter(|| InviscidSolver::new(black_box(af), &InviscidOptions::default()).unwrap());
        });
    }
    group.finish();
}

fn per_alpha(c: &mut Criterion) {
    let solver = InviscidSolver::new(&paneled(160), &InviscidOptions::default()).unwrap();
    c.bench_function("solve_alpha/160", |b| {
        b.iter(|| {
            solver
                .solve(black_box(OperatingPoint::incompressible(4.0)))
                .unwrap()
        });
    });
    c.bench_function("solve_alpha_mach/160", |b| {
        b.iter(|| {
            solver
                .solve(black_box(OperatingPoint {
                    alpha_deg: 4.0,
                    mach: 0.3,
                }))
                .unwrap()
        });
    });
    let sweep = AlphaSweep {
        start_deg: -10.0,
        end_deg: 15.0,
        step_deg: 0.25,
    };
    c.bench_function("sweep_101/160", |b| {
        b.iter(|| solver.sweep(black_box(&sweep), 0.0).unwrap());
    });
}

fn paneling(c: &mut Criterion) {
    let base = naca("2412", &NacaOptions::default()).unwrap();
    c.bench_function("pangen/160", |b| {
        b.iter(|| panel(black_box(&base), &PanelingOptions::default()).unwrap());
    });
}

criterion_group!(benches, factorization, per_alpha, paneling);
criterion_main!(benches);
