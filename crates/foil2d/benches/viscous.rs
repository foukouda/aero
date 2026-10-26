//! Benchmarks of the viscous solver: one point from a fresh boundary layer
//! and a warm-started sweep (NACA 2412, Re = 1e6).

#![allow(missing_docs, clippy::unwrap_used)]

use std::hint::black_box;

use aero_foil2d::inviscid::AlphaSweep;
use aero_foil2d::viscous::{ViscousOptions, ViscousSolver};
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

fn single_point(c: &mut Criterion) {
    let mut group = c.benchmark_group("viscous_point");
    group.sample_size(20);
    for n in [120, 160, 240] {
        let solver = ViscousSolver::new(&paneled(n), &ViscousOptions::default()).unwrap();
        group.bench_with_input(BenchmarkId::from_parameter(n), &solver, |b, s| {
            b.iter(|| s.solve(black_box(4.0)).unwrap());
        });
    }
    group.finish();
}

fn sweep(c: &mut Criterion) {
    let solver = ViscousSolver::new(&paneled(160), &ViscousOptions::default()).unwrap();
    let sweep = AlphaSweep {
        start_deg: -4.0,
        end_deg: 12.0,
        step_deg: 0.5,
    };
    let mut group = c.benchmark_group("viscous_sweep");
    group.sample_size(10);
    group.bench_function("33_points/160", |b| {
        b.iter(|| solver.sweep(black_box(&sweep)).unwrap());
    });
    group.finish();
}

criterion_group!(benches, single_point, sweep);
criterion_main!(benches);
