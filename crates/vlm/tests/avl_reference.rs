//! The port against AVL 3.52 itself: every case of `tests/avl` (geometry
//! file, runs in `case.toml`, AVL's full-precision outputs `ft_*`, `fs_*`,
//! `st_*` made by `tests/avl/generate.py` in the `avl-reference` workflow)
//! is read by [`parse_avl`], solved, and compared quantity by quantity.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use aero_vlm::{FlowCondition, Solution, VortexLattice, parse_avl};

/// Agreement required, relative to the larger of the value and 1 (AVL
/// prints 16 significant digits; the port follows its arithmetic, the LU
/// factorization and the summation order aside).
const TOLERANCE: f64 = 1e-10;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/avl")
}

#[derive(Debug, serde::Deserialize)]
struct Case {
    geometry: String,
    runs: Vec<Run>,
}

#[derive(Debug, serde::Deserialize)]
struct Run {
    name: String,
    #[serde(default)]
    alpha: f64,
    #[serde(default)]
    beta: f64,
    #[serde(default)]
    roll: f64,
    #[serde(default)]
    pitch: f64,
    #[serde(default)]
    yaw: f64,
    #[serde(default)]
    controls: Vec<f64>,
}

/// The labelled lines of an AVL machine-readable output: the numbers before
/// `|` keyed by the text after it.
fn labelled(text: &str) -> HashMap<String, Vec<f64>> {
    let mut out = HashMap::new();
    for line in text.lines() {
        if let Some((values, label)) = line.split_once('|') {
            let v: Vec<f64> = values
                .split_whitespace()
                .filter_map(|t| t.parse().ok())
                .collect();
            out.insert(label.trim().to_owned(), v);
        }
    }
    out
}

fn get<'a>(map: &'a HashMap<String, Vec<f64>>, key: &str) -> &'a [f64] {
    map.iter()
        .find(|(k, _)| k.ends_with(key))
        .map(|(_, v)| v.as_slice())
        .unwrap_or_else(|| panic!("no line ending with {key:?}"))
}

/// Comparison of computed and reference values, keeping the worst.
struct Checker {
    context: String,
    worst: f64,
    worst_what: String,
    failures: Vec<String>,
}

impl Checker {
    fn new(context: String) -> Self {
        Self {
            context,
            worst: 0.0,
            worst_what: String::new(),
            failures: Vec::new(),
        }
    }

    fn check(&mut self, what: &str, got: f64, expected: f64) {
        let err = (got - expected).abs() / expected.abs().max(1.0);
        if err > self.worst {
            self.worst = err;
            self.worst_what = what.to_owned();
        }
        if err > TOLERANCE || err.is_nan() {
            self.failures.push(format!(
                "{what}: got {got:.15e}, AVL {expected:.15e} (error {err:.1e})"
            ));
        }
    }

    fn all(&mut self, what: &str, got: &[f64], expected: &[f64]) {
        assert_eq!(got.len(), expected.len(), "{}: {what}", self.context);
        for (k, (g, e)) in got.iter().zip(expected).enumerate() {
            self.check(&format!("{what}[{k}]"), *g, *e);
        }
    }
}

fn check_totals(c: &mut Checker, s: &Solution, ft: &str) {
    let m = labelled(ft);
    let t = &s.totals;
    c.all(
        "CX Cl Cl'",
        &[t.cx, t.roll, t.roll_stability],
        get(&m, "CXtot, Cltot, Cl'tot"),
    );
    c.all("CY Cm", &[t.cy, t.pitch], get(&m, "CYtot, Cmtot"));
    c.all(
        "CZ Cn Cn'",
        &[t.cz, t.yaw, t.yaw_stability],
        get(&m, "CZtot, Cntot, Cn'tot"),
    );
    c.all("CL", &[t.cl], get(&m, "CLtot"));
    c.all("CD", &[t.cd], get(&m, "CDtot"));
    c.all(
        "CDvis CDind",
        &[t.cd_viscous, t.cd_induced],
        get(&m, "CDvis, CDind"),
    );
    c.all(
        "Trefftz",
        &[t.cl_trefftz, t.cd_trefftz, t.cy_trefftz, t.span_efficiency],
        get(&m, "CLff, CDff, CYff, e"),
    );
}

fn check_strips(c: &mut Checker, s: &Solution, fs: &str) {
    let lines: Vec<&str> = fs.lines().collect();
    let mut k = 0;
    let mut surface = 0;
    while k < lines.len() {
        if lines[k].trim() != "SURFACE" {
            k += 1;
            continue;
        }
        let name = lines[k + 1].trim();
        let sf = &s.surfaces[surface];
        assert_eq!(sf.name, name, "{}: surface {surface}", c.context);
        let ints: Vec<usize> = lines[k + 2]
            .split('|')
            .next()
            .unwrap()
            .split_whitespace()
            .map(|t| t.parse().unwrap())
            .collect();
        let (nv, ns, j1) = (ints[1], ints[2], ints[3]);
        assert_eq!(
            (sf.chordwise, sf.strips, sf.first_strip + 1),
            (nv, ns, j1),
            "{}: {name}",
            c.context
        );
        let num = |l: &str| -> Vec<f64> {
            l.split('|')
                .next()
                .unwrap()
                .split_whitespace()
                .map(|t| t.parse().unwrap())
                .collect()
        };
        c.all(
            &format!("{name} area"),
            &[sf.area, sf.mean_chord],
            &num(lines[k + 3]),
        );
        c.all(
            &format!("{name} forces"),
            &[
                sf.cl,
                sf.roll,
                sf.cy,
                sf.pitch,
                sf.cd,
                sf.yaw,
                sf.cd_induced,
                sf.cd_viscous,
            ],
            &num(lines[k + 4]),
        );
        c.all(
            &format!("{name} own"),
            &[sf.cl_surface, sf.cd_surface],
            &num(lines[k + 5]),
        );
        for j in 0..ns {
            let mut v = num(lines[k + 8 + j]);
            let st = &s.strips[sf.first_strip + j];
            let mut xcp = st.center_of_pressure.unwrap_or(999.0);
            // Without lift (a fin in symmetric flight, cl ~ 1e-18) the
            // centre of pressure is rounding noise divided by rounding noise.
            if v[9].abs() < 1e-12 {
                v[14] = 999.0;
                xcp = 999.0;
            }
            c.all(
                &format!("{name} strip {}", j + 1),
                &[
                    (sf.first_strip + j + 1) as f64,
                    st.leading_edge[0],
                    st.leading_edge[1],
                    st.leading_edge[2],
                    st.chord,
                    st.area,
                    st.chord_cl,
                    st.downwash,
                    st.cl_perp,
                    st.cl,
                    st.cd,
                    st.cd_viscous,
                    st.cm_quarter,
                    st.cm_leading_edge,
                    xcp,
                ],
                &v,
            );
        }
        surface += 1;
        k += 8 + ns;
    }
    assert_eq!(
        surface,
        s.surfaces.len(),
        "{}: number of surfaces",
        c.context
    );
}

fn check_derivatives(c: &mut Checker, s: &Solution, st: &str) {
    let m = labelled(st);
    let d = &s.derivatives;
    let pairs = [
        ("CLa, CLb", d.alpha.cl, d.beta.cl),
        ("CYa, CYb", d.alpha.cy, d.beta.cy),
        ("CDa, CDb", d.alpha.cd, d.beta.cd),
        ("Cla, Clb", d.alpha.roll, d.beta.roll),
        ("Cma, Cmb", d.alpha.pitch, d.beta.pitch),
        ("Cna, Cnb", d.alpha.yaw, d.beta.yaw),
    ];
    for (key, a, b) in pairs {
        c.all(key, &[a, b], get(&m, key));
    }
    let rates = [
        (
            "CLp, CLq, CLr",
            [d.roll_rate.cl, d.pitch_rate.cl, d.yaw_rate.cl],
        ),
        (
            "CYp, CYq, CYr",
            [d.roll_rate.cy, d.pitch_rate.cy, d.yaw_rate.cy],
        ),
        (
            "CDp, CDq, CDr",
            [d.roll_rate.cd, d.pitch_rate.cd, d.yaw_rate.cd],
        ),
        (
            "Clp, Clq, Clr",
            [d.roll_rate.roll, d.pitch_rate.roll, d.yaw_rate.roll],
        ),
        (
            "Cmp, Cmq, Cmr",
            [d.roll_rate.pitch, d.pitch_rate.pitch, d.yaw_rate.pitch],
        ),
        (
            "Cnp, Cnq, Cnr",
            [d.roll_rate.yaw, d.pitch_rate.yaw, d.yaw_rate.yaw],
        ),
    ];
    for (key, v) in rates {
        c.all(key, &v, get(&m, key));
    }
    if !d.controls.is_empty() {
        let col = |f: &dyn Fn(&aero_vlm::ControlDerivatives) -> f64| -> Vec<f64> {
            d.controls.iter().map(f).collect()
        };
        c.all("CLd", &col(&|x| x.forces.cl), get(&m, "CLd*"));
        c.all("CYd", &col(&|x| x.forces.cy), get(&m, "CYd*"));
        c.all("CDd", &col(&|x| x.forces.cd), get(&m, "CDd*"));
        c.all("Cld", &col(&|x| x.forces.roll), get(&m, "Cld*"));
        c.all("Cmd", &col(&|x| x.forces.pitch), get(&m, "Cmd*"));
        c.all("Cnd", &col(&|x| x.forces.yaw), get(&m, "Cnd*"));
        c.all("CDffd", &col(&|x| x.cd_trefftz), get(&m, "CDffd*"));
        c.all("ed", &col(&|x| x.span_efficiency), get(&m, "ed*"));
    }
    c.all(
        "Xnp",
        &[d.neutral_point.unwrap_or(-1.0e30)],
        get(&m, "Neutral point  Xnp"),
    );
    let spiral = get(&m, "( > 1 if spirally stable )")[0];
    c.all(
        "spiral",
        &[d.spiral_stability.unwrap_or(-1.0e30)],
        &[spiral],
    );
}

#[test]
fn matches_avl() {
    let mut cases: Vec<PathBuf> = fs::read_dir(root())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("case.toml").exists())
        .collect();
    cases.sort();
    assert!(cases.len() >= 5, "AVL cases missing in {:?}", root());

    let mut failures = Vec::new();
    for dir in cases {
        let case: Case =
            toml::from_str(&fs::read_to_string(dir.join("case.toml")).unwrap()).unwrap();
        let text = fs::read_to_string(dir.join(&case.geometry)).unwrap();
        let model = parse_avl(&text, |name| fs::read_to_string(dir.join(name)).ok()).unwrap();
        let lattice = VortexLattice::new(&model).unwrap();
        for run in &case.runs {
            let flow = FlowCondition {
                alpha_deg: run.alpha,
                beta_deg: run.beta,
                roll_rate: run.roll,
                pitch_rate: run.pitch,
                yaw_rate: run.yaw,
                controls: run.controls.clone(),
            };
            let s = lattice.solve(&flow).unwrap();
            let name = format!(
                "{}/{}",
                dir.file_name().unwrap().to_string_lossy(),
                run.name
            );
            let read = |prefix: &str| {
                fs::read_to_string(dir.join(format!("{prefix}_{}.txt", run.name))).unwrap()
            };
            let ft = read("ft");
            let counts = labelled(&ft);
            assert_eq!(
                get(&counts, "# vortices"),
                &[lattice.vortex_count() as f64],
                "{name}"
            );
            assert_eq!(
                get(&counts, "# strips"),
                &[lattice.strip_count() as f64],
                "{name}"
            );

            let mut c = Checker::new(name.clone());
            check_totals(&mut c, &s, &ft);
            check_strips(&mut c, &s, &read("fs"));
            check_derivatives(&mut c, &s, &read("st"));
            println!(
                "{name}: {} vortices, worst relative error {:.1e} ({})",
                lattice.vortex_count(),
                c.worst,
                c.worst_what
            );
            for f in c.failures.iter().take(12) {
                failures.push(format!("{name}: {f}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "differences with AVL:\n{}",
        failures.join("\n")
    );
}
