//! Times the operations of the speed comparison with XFOIL
//! (`tests/speed/compare.py`): the same airfoils, paneling and commands as
//! the XFOIL sessions, from the paneled airfoil on (the matrix
//! factorization and the wake set-up are included, as in XFOIL's first
//! ALFA).
//!
//! ```sh
//! cargo run --release -p aero-foil2d --example xfoil_speed -- --repeat 5
//! ```
//!
//! Prints one JSON object per task: median time in seconds over the
//! repetitions, points computed and points that did not converge.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use aero_foil2d::inviscid::{AlphaSweep, ClSweep, InviscidOptions, InviscidSolver};
use aero_foil2d::viscous::{ViscousOptions, ViscousSolver};
use aero_geometry::{Airfoil, NacaOptions, PanelingOptions, naca, panel};

/// Airfoils of the batch task (the XFOIL sessions use XFOIL's NACA command).
const BATCH: [&str; 8] = [
    "0012", "2412", "4412", "4415", "23012", "6409", "0009", "2415",
];

fn paneled(code: &str) -> Airfoil {
    // PanelingOptions::default() = PPAR N 160, P 1, T 0.15, R 0.2.
    panel(
        &naca(code, &NacaOptions::default()).unwrap(),
        &PanelingOptions::default(),
    )
    .unwrap()
}

fn viscous() -> ViscousOptions {
    // VISC 1e6, Ncrit 9, free transition, ITER 20 (XFOIL's defaults).
    ViscousOptions {
        reynolds: 1e6,
        ..ViscousOptions::default()
    }
}

fn alpha_sweep() -> AlphaSweep {
    AlphaSweep {
        start_deg: -4.0,
        end_deg: 14.0,
        step_deg: 0.5,
    }
}

/// Median wall time of `repeat` runs of `task`, with the counts of the last
/// run.
fn time(repeat: usize, mut task: impl FnMut() -> (usize, usize)) -> (f64, usize, usize) {
    let mut times = Vec::with_capacity(repeat);
    let mut counts = (0, 0);
    for _ in 0..repeat {
        let start = Instant::now();
        counts = task();
        times.push(start.elapsed().as_secs_f64());
    }
    times.sort_by(f64::total_cmp);
    (times[times.len() / 2], counts.0, counts.1)
}

/// Viscous polar of one airfoil, from its NACA designation (geometry and
/// paneling included, as in an XFOIL session).
fn batch_polar(code: &str) -> (usize, usize) {
    let solver = ViscousSolver::new(&paneled(code), &viscous()).unwrap();
    let polar = solver.sweep(&alpha_sweep()).unwrap();
    (polar.points.len(), polar.failures.len())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str, default: usize| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .map_or(default, |v| v.parse().expect("a number"))
    };
    let repeat = arg("--repeat", 5).max(1);
    let threads = arg(
        "--threads",
        std::thread::available_parallelism().map_or(1, |n| n.get()),
    );

    let airfoil = paneled("2412");
    let report = |task: &str, (seconds, points, failed): (f64, usize, usize)| {
        println!(
            "{{\"task\": \"{task}\", \"seconds\": {seconds:.6}, \"points\": {points}, \"failed\": {failed}}}"
        );
    };

    report(
        "inviscid_polar",
        time(repeat, || {
            let solver = InviscidSolver::new(&airfoil, &InviscidOptions::default()).unwrap();
            let polar = solver.sweep(&alpha_sweep(), 0.0).unwrap();
            (polar.points.len(), polar.failures.len())
        }),
    );
    report(
        "viscous_point",
        time(repeat, || {
            let solver = ViscousSolver::new(&airfoil, &viscous()).unwrap();
            let s = solver.solve(4.0).unwrap();
            (1, usize::from(!s.converged))
        }),
    );
    report(
        "viscous_polar",
        time(repeat, || {
            let solver = ViscousSolver::new(&airfoil, &viscous()).unwrap();
            let polar = solver.sweep(&alpha_sweep()).unwrap();
            (polar.points.len(), polar.failures.len())
        }),
    );
    report(
        "viscous_cl_polar",
        time(repeat, || {
            let solver = ViscousSolver::new(&airfoil, &viscous()).unwrap();
            let polar = solver
                .sweep_cl(&ClSweep {
                    start: 0.0,
                    end: 1.2,
                    step: 0.1,
                })
                .unwrap();
            (polar.points.len(), polar.failures.len())
        }),
    );
    report(
        "batch_sequential",
        time(repeat, || {
            BATCH.iter().fold((0, 0), |(p, f), code| {
                let (dp, df) = batch_polar(code);
                (p + dp, f + df)
            })
        }),
    );
    report(
        "batch_parallel",
        time(repeat, || {
            let next = AtomicUsize::new(0);
            let results: Vec<(usize, usize)> = std::thread::scope(|scope| {
                let workers: Vec<_> = (0..threads.min(BATCH.len()))
                    .map(|_| {
                        scope.spawn(|| {
                            let mut out = Vec::new();
                            while let Some(code) = BATCH.get(next.fetch_add(1, Ordering::Relaxed)) {
                                out.push(batch_polar(code));
                            }
                            out
                        })
                    })
                    .collect();
                workers
                    .into_iter()
                    .flat_map(|w| w.join().unwrap())
                    .collect()
            });
            results
                .iter()
                .fold((0, 0), |(p, f), (dp, df)| (p + dp, f + df))
        }),
    );
}
