//! Forces and moments: strips, surfaces and totals from the forces on the
//! bound vortices, induced drag in the Trefftz plane, and the stability
//! derivatives (AVL `AERO`, `SFFORC`, `TPFORC`, `DERMATS`).
//!
//! Every force is carried with its derivatives with respect to the six
//! unit flow components (u, v, w, p, q, r in body axes) and to the control
//! deflections, as AVL does; the stability derivatives are combinations of
//! them.

use std::f64::consts::PI;

use crate::lattice::Lattice;
use crate::model::VlmModel;
use crate::solution::{
    CoefficientDerivatives, ControlDerivatives, FlowCondition, Solution, StabilityDerivatives,
    StripForces, SurfaceForces, Totals,
};
use crate::solver::DIR;
use crate::vec3::{V3, add, cross, dot, norm, sub};

/// The state of a solution: flow vectors, circulations and induced
/// velocities with their derivatives (`npar` = 6 unit components, then the
/// controls).
pub(crate) struct Flow {
    pub(crate) alfa: f64,
    pub(crate) vinf: V3,
    pub(crate) vinf_a: V3,
    pub(crate) vinf_b: V3,
    pub(crate) wrot: V3,
    pub(crate) gam: Vec<f64>,
    pub(crate) gam_p: Vec<f64>,
    pub(crate) vv: Vec<V3>,
    pub(crate) vv_p: Vec<V3>,
    pub(crate) npar: usize,
}

/// A force and moment with their derivatives.
#[derive(Clone)]
struct Load {
    cf: V3,
    cm: V3,
    cf_p: Vec<V3>,
    cm_p: Vec<V3>,
}

impl Load {
    fn zero(npar: usize) -> Self {
        Self {
            cf: [0.0; 3],
            cm: [0.0; 3],
            cf_p: vec![[0.0; 3]; npar],
            cm_p: vec![[0.0; 3]; npar],
        }
    }
}

/// `m + (r × f) / c`, in AVL's order of operations.
fn moment_add(m: V3, r: V3, f: V3, c: f64) -> V3 {
    [
        m[0] + (f[2] * r[1] - f[1] * r[2]) / c,
        m[1] + (f[0] * r[2] - f[2] * r[0]) / c,
        m[2] + (f[1] * r[0] - f[0] * r[1]) / c,
    ]
}

/// Lift, side force and drag in stability axes and their alpha derivatives.
struct Stability {
    cd: f64,
    cy: f64,
    cl: f64,
    cd_a: f64,
    cl_a: f64,
    cd_p: Vec<f64>,
    cy_p: Vec<f64>,
    cl_p: Vec<f64>,
}

impl Stability {
    fn of(load: &Load, sa: f64, ca: f64) -> Self {
        let f = load.cf;
        Self {
            cd: f[0] * ca + f[2] * sa,
            cy: f[1],
            cl: -f[0] * sa + f[2] * ca,
            cd_a: -f[0] * sa + f[2] * ca,
            cl_a: -f[0] * ca - f[2] * sa,
            cd_p: load.cf_p.iter().map(|d| d[0] * ca + d[2] * sa).collect(),
            cy_p: load.cf_p.iter().map(|d| d[1]).collect(),
            cl_p: load.cf_p.iter().map(|d| -d[0] * sa + d[2] * ca).collect(),
        }
    }
}

/// AVL `AERO`: everything at one flow condition.
pub(crate) fn aero(model: &VlmModel, lattice: &Lattice, s: &Flow, flow: FlowCondition) -> Solution {
    let npar = s.npar;
    let nctl = npar - 6;
    let r = &model.reference;
    let (sref, cref, bref) = (r.area, r.chord, r.span);
    let xyzref = r.point;
    let (sa, ca) = s.alfa.sin_cos();

    // ---- strips (SFFORC)
    let mut strip_out = Vec::with_capacity(lattice.strips.len());
    let mut strip_ref = Vec::with_capacity(lattice.strips.len()); // about XYZREF
    let mut strip_local = Vec::with_capacity(lattice.strips.len()); // CF_LSTRP
    for strip in &lattice.strips {
        let cr = strip.chord;
        let sr = strip.chord * strip.width;
        let spn = [0.0, strip.ensz, -strip.ensy];
        let udrag = s.vinf;
        let ulift = {
            let l = cross(udrag, spn);
            let m = norm(l);
            if m == 0.0 {
                [0.0, 0.0, 1.0]
            } else {
                [l[0] / m, l[1] / m, l[2] / m]
            }
        };
        let rc4 = [strip.rle[0] + 0.25 * cr, strip.rle[1], strip.rle[2]];

        let mut load = Load::zero(npar);
        let mut cnc = 0.0;
        for i in strip.first..strip.first + strip.count {
            let e = &lattice.elements[i];
            let rr = sub(e.rv, rc4);
            let rrot = sub(e.rv, xyzref);
            let vrot = cross(rrot, s.wrot);
            let veff = [
                s.vinf[0] + vrot[0] + s.vv[i][0],
                s.vinf[1] + vrot[1] + s.vv[i][1],
                s.vinf[2] + vrot[2] + s.vv[i][2],
            ];
            let g = sub(e.rv2, e.rv1);
            let f = cross(veff, g);
            let gi = s.gam[i];
            let fgam = [2.0 * gi * f[0], 2.0 * gi * f[1], 2.0 * gi * f[2]];
            let dcf = [fgam[0] / sr, fgam[1] / sr, fgam[2] / sr];
            load.cf = add(load.cf, dcf);
            load.cm = moment_add(load.cm, rr, dcf, cr);
            cnc += cr * (strip.ensy * dcf[1] + strip.ensz * dcf[2]);

            for p in 0..npar {
                let mut dveff = s.vv_p[i * npar + p];
                if p < 3 {
                    dveff[p] += 1.0;
                } else if p < 6 {
                    let mut unit = [0.0; 3];
                    unit[p - 3] = 1.0;
                    dveff = add(cross(rrot, unit), dveff);
                }
                let fp = cross(dveff, g);
                let gp = s.gam_p[i * npar + p];
                let fgam_p = [
                    2.0 * gp * f[0] + 2.0 * gi * fp[0],
                    2.0 * gp * f[1] + 2.0 * gi * fp[1],
                    2.0 * gp * f[2] + 2.0 * gi * fp[2],
                ];
                let dcf_p = [fgam_p[0] / sr, fgam_p[1] / sr, fgam_p[2] / sr];
                load.cf_p[p] = add(load.cf_p[p], dcf_p);
                load.cm_p[p] = moment_add(load.cm_p[p], rr, dcf_p, cr);
            }
        }

        // Moments about the reference point.
        let rref = sub(rc4, xyzref);
        let mut about_ref = load.clone();
        about_ref.cm = moment_add(load.cm, rref, load.cf, cr);
        for p in 0..npar {
            about_ref.cm_p[p] = moment_add(load.cm_p[p], rref, load.cf_p[p], cr);
        }

        let cl_l = dot(ulift, load.cf);
        let cd_l = dot(udrag, load.cf);
        let cmc4 = strip.ensz * load.cm[1] - strip.ensy * load.cm[2];

        // Lift coefficient referred to the velocity perpendicular to the
        // spanwise axis.
        let rrot = sub(strip.sref, xyzref);
        let veff = add(s.vinf, cross(rrot, s.wrot));
        let vspan = dot(veff, strip.ess);
        let vperp = sub(
            veff,
            [
                strip.ess[0] * vspan,
                strip.ess[1] * vspan,
                strip.ess[2] * vspan,
            ],
        );
        let vpsq = dot(vperp, vperp);
        let vpsqi = if vpsq == 0.0 { 1.0 } else { 1.0 / vpsq };

        // Moment about the leading edge, along the leading edge.
        let rle = sub(rc4, strip.rle);
        let mut del = sub(strip.rle2, strip.rle1);
        if lattice.surfaces[strip.surface].image {
            del = [-del[0], -del[1], -del[2]];
        }
        let dmag = norm(del);
        let cmle = if dmag != 0.0 {
            let m = moment_add(load.cm, rle, load.cf, cr);
            del[0] / dmag * m[0] + del[1] / dmag * m[1] + del[2] / dmag * m[2]
        } else {
            0.0
        };

        strip_out.push(StripForces {
            leading_edge: strip.rle,
            chord: cr,
            area: strip.width * cr,
            chord_cl: cnc,
            downwash: 0.0,
            cl_perp: cl_l * vpsqi,
            cl: cl_l,
            cd: cd_l,
            cd_viscous: 0.0,
            cm_quarter: cmc4,
            cm_leading_edge: cmle,
            center_of_pressure: (cl_l != 0.0).then(|| 0.25 - cmc4 / cl_l),
        });
        strip_local.push(load.cf);
        strip_ref.push(about_ref);
    }

    // ---- surfaces and totals
    let mut tot = Load::zero(npar);
    let (mut cdtot, mut cltot, mut cdtot_a, mut cltot_a) = (0.0, 0.0, 0.0, 0.0);
    let (mut cdtot_p, mut cytot_p, mut cltot_p) =
        (vec![0.0; npar], vec![0.0; npar], vec![0.0; npar]);
    let mut surfaces = Vec::with_capacity(lattice.surfaces.len());
    for surf in &lattice.surfaces {
        let mut sload = Load::zero(npar);
        let (mut cd, mut cy, mut cl, mut cd_a, mut cl_a) = (0.0, 0.0, 0.0, 0.0, 0.0);
        let (mut cd_p, mut cy_p, mut cl_p) = (vec![0.0; npar], vec![0.0; npar], vec![0.0; npar]);
        let mut cf_local = [0.0; 3];
        let mut enave = [0.0; 3];
        for j in surf.first_strip..surf.first_strip + surf.n_strips {
            let strip = &lattice.strips[j];
            let sr = strip.chord * strip.width;
            let cr = strip.chord;
            let st = &strip_ref[j];
            let stab = Stability::of(st, sa, ca);
            enave[1] += sr * strip.ensy;
            enave[2] += sr * strip.ensz;
            cd += stab.cd * sr / sref;
            cy += stab.cy * sr / sref;
            cl += stab.cl * sr / sref;
            for k in 0..3 {
                sload.cf[k] += st.cf[k] * sr / sref;
            }
            sload.cm[0] += st.cm[0] * (sr / sref) * (cr / bref);
            sload.cm[1] += st.cm[1] * (sr / sref) * (cr / cref);
            sload.cm[2] += st.cm[2] * (sr / sref) * (cr / bref);
            cd_a += stab.cd_a * sr / sref;
            cl_a += stab.cl_a * sr / sref;
            for p in 0..npar {
                cd_p[p] += stab.cd_p[p] * sr / sref;
                cy_p[p] += stab.cy_p[p] * sr / sref;
                cl_p[p] += stab.cl_p[p] * sr / sref;
                for k in 0..3 {
                    sload.cf_p[p][k] += st.cf_p[p][k] * sr / sref;
                }
                sload.cm_p[p][0] += st.cm_p[p][0] * (sr / sref) * (cr / bref);
                sload.cm_p[p][1] += st.cm_p[p][1] * (sr / sref) * (cr / cref);
                sload.cm_p[p][2] += st.cm_p[p][2] * (sr / sref) * (cr / bref);
            }
            for k in 0..3 {
                cf_local[k] += strip_local[j][k] * sr / surf.area;
            }
        }
        // Lift and drag of the surface in its own axes.
        for v in enave.iter_mut() {
            *v /= surf.area;
        }
        let enmag = norm(enave);
        if enmag == 0.0 {
            enave[2] = 1.0;
        } else {
            enave = [enave[0] / enmag, enave[1] / enmag, enave[2] / enmag];
        }
        let spn = [0.0, enave[2], -enave[1]];
        let udrag = s.vinf;
        let mut ulift = cross(udrag, spn);
        let ulmag = norm(ulift);
        if ulmag == 0.0 {
            ulift[2] = 1.0;
        } else {
            ulift = [ulift[0] / ulmag, ulift[1] / ulmag, ulift[2] / ulmag];
        }

        surfaces.push(SurfaceForces {
            name: surf.name.clone(),
            area: surf.area,
            mean_chord: surf.mean_chord,
            cl,
            roll: DIR * sload.cm[0],
            cy,
            pitch: sload.cm[1],
            cd,
            yaw: DIR * sload.cm[2],
            cd_induced: cd,
            cd_viscous: 0.0,
            cl_surface: dot(ulift, cf_local),
            cd_surface: dot(udrag, cf_local),
            first_strip: surf.first_strip,
            strips: surf.n_strips,
            chordwise: surf.chordwise,
        });

        if surf.load {
            for k in 0..3 {
                tot.cf[k] += sload.cf[k];
                tot.cm[k] += sload.cm[k];
            }
            cdtot += cd;
            cltot += cl;
            cdtot_a += cd_a;
            cltot_a += cl_a;
            for p in 0..npar {
                cdtot_p[p] += cd_p[p];
                cytot_p[p] += cy_p[p];
                cltot_p[p] += cl_p[p];
                for k in 0..3 {
                    tot.cf_p[p][k] += sload.cf_p[p][k];
                    tot.cm_p[p][k] += sload.cm_p[p][k];
                }
            }
        }
    }

    // ---- Trefftz plane (TPFORC)
    let tp = trefftz(model, lattice, s, &mut strip_out);

    // ---- symmetric half model: double the symmetric parts
    if model.y_symmetry {
        cdtot *= 2.0;
        cltot *= 2.0;
        tot.cf = [2.0 * tot.cf[0], 0.0, 2.0 * tot.cf[2]];
        tot.cm = [0.0, 2.0 * tot.cm[1], 0.0];
        cdtot_a *= 2.0;
        cltot_a *= 2.0;
        for p in 0..npar {
            cdtot_p[p] *= 2.0;
            cytot_p[p] = 0.0;
            cltot_p[p] *= 2.0;
            tot.cf_p[p] = [2.0 * tot.cf_p[p][0], 0.0, 2.0 * tot.cf_p[p][2]];
            tot.cm_p[p] = [0.0, 2.0 * tot.cm_p[p][1], 0.0];
        }
    }

    // ---- baseline profile drag along the freestream
    let vsq = dot(s.vinf, s.vinf);
    let vmag = vsq.sqrt();
    let cdref = model.cd_p;
    let cdvtot = cdref * vsq;
    cdtot += cdref * vsq;
    for l in 0..3 {
        tot.cf[l] += cdref * s.vinf[l] * vmag;
        tot.cf_p[l][l] += cdref * vmag;
    }
    for (iu, (cd_u, cf_u)) in cdtot_p
        .iter_mut()
        .zip(tot.cf_p.iter_mut())
        .take(3)
        .enumerate()
    {
        *cd_u += cdref * 2.0 * s.vinf[iu];
        for (cf, v) in cf_u.iter_mut().zip(s.vinf) {
            *cf += cdref * v * s.vinf[iu] / vmag;
        }
    }

    // ---- moments in stability axes
    let crsax = tot.cm[0] * ca + tot.cm[2] * sa;
    let cnsax = tot.cm[2] * ca - tot.cm[0] * sa;
    let totals = Totals {
        cx: DIR * tot.cf[0],
        cy: tot.cf[1],
        cz: DIR * tot.cf[2],
        roll: DIR * tot.cm[0],
        pitch: tot.cm[1],
        yaw: DIR * tot.cm[2],
        roll_stability: DIR * crsax,
        yaw_stability: DIR * cnsax,
        cl: cltot,
        cd: cdtot,
        cd_viscous: cdvtot,
        cd_induced: cdtot - cdvtot,
        cl_trefftz: tp.cl,
        cd_trefftz: tp.cd,
        cy_trefftz: tp.cy,
        span_efficiency: tp.e,
    };

    // ---- stability derivatives (DERMATS)
    let wrot = s.wrot;
    let rx = (wrot[0] * ca + wrot[2] * sa) * DIR;
    let rz = (wrot[2] * ca - wrot[0] * sa) * DIR;
    let wrot_rx = [ca * DIR, 0.0, sa * DIR];
    let wrot_rz = [-sa * DIR, 0.0, ca * DIR];
    let wrot_a = [-rx * sa - rz * ca, 0.0, -rz * sa + rx * ca];
    let crsax_a = -tot.cm[0] * sa + tot.cm[2] * ca;
    let cnsax_a = -tot.cm[2] * sa - tot.cm[0] * ca;
    let crsax_p: Vec<f64> = tot.cm_p.iter().map(|m| m[0] * ca + m[2] * sa).collect();
    let cmsax_p: Vec<f64> = tot.cm_p.iter().map(|m| m[1]).collect();
    let cnsax_p: Vec<f64> = tot.cm_p.iter().map(|m| m[2] * ca - m[0] * sa).collect();

    let d_alpha = |d: &[f64], extra: f64| {
        d[0] * s.vinf_a[0]
            + d[3] * wrot_a[0]
            + d[1] * s.vinf_a[1]
            + d[4] * wrot_a[1]
            + d[2] * s.vinf_a[2]
            + d[5] * wrot_a[2]
            + extra
    };
    let d_beta = |d: &[f64]| d[0] * s.vinf_b[0] + d[1] * s.vinf_b[1] + d[2] * s.vinf_b[2];
    let d_rx = |d: &[f64]| d[3] * wrot_rx[0] + d[5] * wrot_rx[2];
    let d_ry = |d: &[f64]| d[4];
    let d_rz = |d: &[f64]| d[5] * wrot_rz[2] + d[3] * wrot_rz[0];

    let alpha = CoefficientDerivatives {
        cl: d_alpha(&cltot_p, cltot_a),
        cy: d_alpha(&cytot_p, 0.0),
        cd: d_alpha(&cdtot_p, cdtot_a),
        roll: DIR * d_alpha(&crsax_p, crsax_a),
        pitch: d_alpha(&cmsax_p, 0.0),
        yaw: DIR * d_alpha(&cnsax_p, cnsax_a),
    };
    let beta = CoefficientDerivatives {
        cl: d_beta(&cltot_p),
        cy: d_beta(&cytot_p),
        cd: d_beta(&cdtot_p),
        roll: DIR * d_beta(&crsax_p),
        pitch: d_beta(&cmsax_p),
        yaw: DIR * d_beta(&cnsax_p),
    };
    let rate = |f: &dyn Fn(&[f64]) -> f64, norm: f64| CoefficientDerivatives {
        cl: f(&cltot_p) * 2.0 / norm,
        cy: f(&cytot_p) * 2.0 / norm,
        cd: f(&cdtot_p) * 2.0 / norm,
        roll: DIR * f(&crsax_p) * 2.0 / norm,
        pitch: f(&cmsax_p) * 2.0 / norm,
        yaw: DIR * f(&cnsax_p) * 2.0 / norm,
    };
    let roll_rate = rate(&d_rx, bref);
    let pitch_rate = rate(&d_ry, cref);
    let yaw_rate = rate(&d_rz, bref);
    let controls = (0..nctl)
        .map(|c| {
            let p = 6 + c;
            ControlDerivatives {
                name: lattice.controls[c].clone(),
                forces: CoefficientDerivatives {
                    cl: cltot_p[p],
                    cy: cytot_p[p],
                    cd: cdtot_p[p],
                    roll: DIR * crsax_p[p],
                    pitch: cmsax_p[p],
                    yaw: DIR * cnsax_p[p],
                },
                cd_trefftz: tp.cd_d[c],
                span_efficiency: tp.e_d[c],
            }
        })
        .collect();

    let cr_be = d_beta(&crsax_p);
    let cn_be = d_beta(&cnsax_p);
    let cr_rz = d_rz(&crsax_p);
    let cn_rz = d_rz(&cnsax_p);
    let derivatives = StabilityDerivatives {
        alpha,
        beta,
        roll_rate,
        pitch_rate,
        yaw_rate,
        controls,
        neutral_point: (alpha.cl != 0.0).then(|| xyzref[0] - cref * alpha.pitch / alpha.cl),
        spiral_stability: ((cr_rz * cn_be).abs() > 0.0001).then(|| cr_be * cn_rz / (cr_rz * cn_be)),
    };

    Solution {
        flow,
        totals,
        surfaces,
        strips: strip_out,
        derivatives,
    }
}

/// Trefftz-plane results.
struct TrefftzPlane {
    cl: f64,
    cy: f64,
    cd: f64,
    e: f64,
    cd_d: Vec<f64>,
    e_d: Vec<f64>,
}

/// AVL `TPFORC`: lift, side force and induced drag from the circulation of
/// the trailing vortices far downstream; sets the downwash of the strips.
fn trefftz(
    model: &VlmModel,
    lattice: &Lattice,
    s: &Flow,
    strips: &mut [StripForces],
) -> TrefftzPlane {
    let npar = s.npar;
    let nctl = npar - 6;
    let hpi = 1.0 / (2.0 * PI);
    let sref = model.reference.area;
    let core = model.core;
    let ns = lattice.strips.len();

    // Circulation of each strip, and the trailing legs of its last element
    // (only y and z matter in the plane).
    let mut gams = vec![0.0; ns];
    let mut gams_d = vec![0.0; ns * nctl];
    let mut legs = Vec::with_capacity(ns);
    for (j, strip) in lattice.strips.iter().enumerate() {
        for i in strip.first..strip.first + strip.count {
            gams[j] += s.gam[i];
            for c in 0..nctl {
                gams_d[j * nctl + c] += s.gam_p[i * npar + 6 + c];
            }
        }
        let te = &lattice.elements[strip.first + strip.count - 1];
        legs.push((te.rv1, te.rv2, te.rc));
    }
    let component = |j: usize| lattice.surfaces[lattice.strips[j].surface].component;

    let (mut clff, mut cyff, mut cdff) = (0.0, 0.0, 0.0);
    let (mut clff_d, mut cyff_d, mut cdff_d) = (vec![0.0; nctl], vec![0.0; nctl], vec![0.0; nctl]);
    let mut vy_d = vec![0.0; nctl];
    let mut vz_d = vec![0.0; nctl];
    for jc in 0..ns {
        let (rt1, rt2, rtc) = legs[jc];
        let dyt = rt2[1] - rt1[1];
        let dzt = rt2[2] - rt1[2];
        let dst = (dyt * dyt + dzt * dzt).sqrt();
        let ny = -dzt / dst;
        let nz = dyt / dst;
        let (yc, zc) = (rtc[1], rtc[2]);
        let (mut vy, mut vz) = (0.0, 0.0);
        vy_d.iter_mut().for_each(|v| *v = 0.0);
        vz_d.iter_mut().for_each(|v| *v = 0.0);
        for jv in 0..ns {
            let (r1, r2, _) = legs[jv];
            let dsyz = ((r2[1] - r1[1]).powi(2) + (r2[2] - r1[2]).powi(2)).sqrt();
            let rcore = if component(jc) == component(jv) {
                0.0
            } else {
                (core.chord_fraction * lattice.strips[jv].chord).max(core.width_fraction * dsyz)
            };
            let rcore4 = rcore.powi(4);
            let (dy1, dy2) = (yc - r1[1], yc - r2[1]);
            let (dz1, dz2) = (zc - r1[2], zc - r2[2]);
            let rsq1 = ((dy1 * dy1 + dz1 * dz1).powi(2) + rcore4).sqrt();
            let rsq2 = ((dy2 * dy2 + dz2 * dz2).powi(2) + rcore4).sqrt();
            let ky = dz1 / rsq1 - dz2 / rsq2;
            let kz = -dy1 / rsq1 + dy2 / rsq2;
            vy += hpi * gams[jv] * ky;
            vz += hpi * gams[jv] * kz;
            for c in 0..nctl {
                vy_d[c] += hpi * gams_d[jv * nctl + c] * ky;
                vz_d[c] += hpi * gams_d[jv * nctl + c] * kz;
            }
            if model.y_symmetry {
                let (dy1, dy2) = (yc + r1[1], yc + r2[1]);
                let rsq1 = dy1 * dy1 + dz1 * dz1;
                let rsq2 = dy2 * dy2 + dz2 * dz2;
                let ky = dz1 / rsq1 - dz2 / rsq2;
                let kz = -dy1 / rsq1 + dy2 / rsq2;
                vy -= hpi * gams[jv] * ky;
                vz -= hpi * gams[jv] * kz;
                for c in 0..nctl {
                    vy_d[c] -= hpi * gams_d[jv * nctl + c] * ky;
                    vz_d[c] -= hpi * gams_d[jv * nctl + c] * kz;
                }
            }
        }
        strips[jc].downwash = -(ny * vy + nz * vz);

        if lattice.surfaces[lattice.strips[jc].surface].load {
            clff += 2.0 * gams[jc] * dyt / sref;
            cyff -= 2.0 * gams[jc] * dzt / sref;
            cdff += gams[jc] * (dzt * vy - dyt * vz) / sref;
            for c in 0..nctl {
                let gd = gams_d[jc * nctl + c];
                clff_d[c] += 2.0 * gd * dyt / sref;
                cyff_d[c] -= 2.0 * gd * dzt / sref;
                cdff_d[c] += (gd * (dzt * vy - dyt * vz)
                    + gams[jc] * (dzt * vy_d[c] - dyt * vz_d[c]))
                    / sref;
            }
        }
    }
    if model.y_symmetry {
        clff *= 2.0;
        cyff = 0.0;
        cdff *= 2.0;
        for c in 0..nctl {
            clff_d[c] *= 2.0;
            cyff_d[c] = 0.0;
            cdff_d[c] *= 2.0;
        }
    }
    let ar = model.reference.span.powi(2) / sref;
    let (e, e_d) = if cdff == 0.0 {
        (0.0, vec![0.0; nctl])
    } else {
        let e = (clff * clff + cyff * cyff) / (PI * ar * cdff);
        let e_cl = 2.0 * clff / (PI * ar * cdff);
        let e_cy = 2.0 * cyff / (PI * ar * cdff);
        let e_cd = -e / cdff;
        let e_d = (0..nctl)
            .map(|c| e_cl * clff_d[c] + e_cy * cyff_d[c] + e_cd * cdff_d[c])
            .collect();
        (e, e_d)
    };
    TrefftzPlane {
        cl: clff,
        cy: cyff,
        cd: cdff,
        e,
        cd_d: cdff_d,
        e_d,
    }
}
