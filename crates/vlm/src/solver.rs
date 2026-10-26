//! The vortex-lattice solver: influence matrix, circulations for unit
//! flows and the solution at a flow condition (AVL `SETUP`, `GUCALC`,
//! `GAMSUM`, `VELSUM`).

use faer::Mat;
use faer::linalg::solvers::{PartialPivLu, Solve};

use crate::error::{Result, VlmError, invalid};
use crate::forces::{Flow, aero};
use crate::influence::Influence;
use crate::lattice::{DTR, Lattice};
use crate::model::VlmModel;
use crate::solution::{FlowCondition, Solution};
use crate::vec3::{V3, cross, dot, sub};

/// AVL's standard axes for the stability rates and printed moments (x
/// forward, z down): the sign `DIR` applied to its geometric axes.
pub(crate) const DIR: f64 = -1.0;

/// A vortex lattice ready to be solved at any flow condition.
///
/// Building it computes and factors the influence matrix and the
/// circulations and induced velocities for unit freestream components,
/// unit rotations and unit control deflections; [`solve`](Self::solve)
/// then only combines them, in a time proportional to the number of
/// vortices.
#[derive(Debug)]
pub struct VortexLattice {
    model: VlmModel,
    pub(crate) lattice: Lattice,
    /// Number of unit solutions: 6 (u, v, w, p, q, r), then 6 per control.
    pub(crate) ncol: usize,
    /// Circulations of the unit solutions, `[element * ncol + column]`.
    pub(crate) gam: Vec<f64>,
    /// Velocities induced at the bound-vortex middles by the unit
    /// solutions, `[element * ncol + column]`.
    pub(crate) vv: Vec<V3>,
}

impl VortexLattice {
    /// Builds and factors the lattice of `model`.
    pub fn new(model: &VlmModel) -> Result<Self> {
        model.validate()?;
        let lattice = Lattice::build(model)?;
        let n = lattice.elements.len();
        let nctl = lattice.controls.len();
        let ncol = 6 + 6 * nctl;
        let beta = (1.0 - model.mach * model.mach).sqrt();
        let influence = Influence {
            elements: &lattice.elements,
            beta,
            y_symmetry: model.y_symmetry,
            core: model.core,
        };

        // Normal velocity at the control points per unit circulation.
        let mut aic = Mat::<f64>::zeros(n, n);
        for (i, e) in lattice.elements.iter().enumerate() {
            for j in 0..n {
                aic[(i, j)] = dot(influence.velocity(e.rc, e.component, j, true), e.enc);
            }
        }
        // Surfaces without a wake: the circulation of each strip sums to
        // zero instead of the flow tangency at its last control point.
        let mut tangency = vec![true; n];
        for s in lattice.surfaces.iter().filter(|s| !s.wake) {
            for strip in &lattice.strips[s.first_strip..s.first_strip + s.n_strips] {
                let iv = strip.first + strip.count - 1;
                for j in 0..n {
                    aic[(iv, j)] = 0.0;
                }
                for j in strip.first..=iv {
                    aic[(iv, j)] = 1.0;
                }
                tangency[iv] = false;
            }
        }
        let lu: PartialPivLu<f64> = aic.partial_piv_lu();
        let u = lu.U();
        let pivot_max = (0..n).fold(0.0_f64, |m, i| m.max(u[(i, i)].abs()));
        let pivot_min = (0..n).fold(f64::INFINITY, |m, i| m.min(u[(i, i)].abs()));
        if !(pivot_max.is_finite() && pivot_min > 1e-13 * pivot_max) {
            return Err(VlmError::Singular);
        }

        // Right-hand sides: −n·V for unit freestream components, unit
        // rotations about the reference point, and their change with each
        // control (the normals rotate with the deflection, linearized).
        let xyzref = model.reference.point;
        let mut rhs = Mat::<f64>::zeros(n, ncol);
        for (i, e) in lattice.elements.iter().enumerate() {
            if !tangency[i] {
                continue;
            }
            for iu in 0..6 {
                let mut unit = [0.0; 3];
                let v = if iu < 3 {
                    unit[iu] = 1.0;
                    unit
                } else {
                    unit[iu - 3] = 1.0;
                    cross(sub(e.rc, xyzref), unit)
                };
                rhs[(i, iu)] = -dot(e.enc, v);
                for c in 0..nctl {
                    rhs[(i, 6 + 6 * c + iu)] = -dot(lattice.enc_d[i * nctl + c], v);
                }
            }
        }
        lu.solve_in_place(rhs.as_mut());
        let mut gam = vec![0.0; n * ncol];
        for i in 0..n {
            for c in 0..ncol {
                let g = rhs[(i, c)];
                if !g.is_finite() {
                    return Err(VlmError::Singular);
                }
                gam[i * ncol + c] = g;
            }
        }

        // Velocities at the bound-vortex middles (without the element's own
        // bound leg).
        let mut vv = vec![[0.0; 3]; n * ncol];
        for (i, e) in lattice.elements.iter().enumerate() {
            let row = &mut vv[i * ncol..(i + 1) * ncol];
            for j in 0..n {
                let w = influence.velocity(e.rv, e.component, j, i != j);
                let g = &gam[j * ncol..(j + 1) * ncol];
                for (acc, gj) in row.iter_mut().zip(g) {
                    acc[0] += w[0] * gj;
                    acc[1] += w[1] * gj;
                    acc[2] += w[2] * gj;
                }
            }
        }

        Ok(Self {
            model: model.clone(),
            lattice,
            ncol,
            gam,
            vv,
        })
    }

    /// The model the lattice was built from.
    pub fn model(&self) -> &VlmModel {
        &self.model
    }

    /// Names of the control variables, in the order of
    /// [`FlowCondition::controls`].
    pub fn control_names(&self) -> &[String] {
        &self.lattice.controls
    }

    /// Number of horseshoe vortices.
    pub fn vortex_count(&self) -> usize {
        self.lattice.elements.len()
    }

    /// Number of chordwise strips.
    pub fn strip_count(&self) -> usize {
        self.lattice.strips.len()
    }

    /// Corners of every strip, in the order of [`Solution::strips`]:
    /// leading and trailing edge at its left end, then trailing and
    /// leading edge at its right end (a quadrilateral, for drawing).
    pub fn strip_corners(&self) -> Vec<[[f64; 3]; 4]> {
        self.lattice
            .strips
            .iter()
            .map(|s| {
                let te = |le: V3, c: f64| [le[0] + c, le[1], le[2]];
                [s.rle1, te(s.rle1, s.chord1), te(s.rle2, s.chord2), s.rle2]
            })
            .collect()
    }

    /// Forces, moments and derivatives at `flow`.
    pub fn solve(&self, flow: &FlowCondition) -> Result<Solution> {
        let values = [
            flow.alpha_deg,
            flow.beta_deg,
            flow.roll_rate,
            flow.pitch_rate,
            flow.yaw_rate,
        ];
        if values.iter().chain(&flow.controls).any(|v| !v.is_finite()) {
            return Err(invalid("the flow condition must be finite"));
        }
        let nctl = self.lattice.controls.len();
        if flow.controls.len() > nctl {
            return Err(invalid(format!(
                "{} control deflections given for {nctl} controls",
                flow.controls.len()
            )));
        }
        let mut delta = flow.controls.clone();
        delta.resize(nctl, 0.0);

        let r = &self.model.reference;
        let alfa = flow.alpha_deg * DTR;
        let beta = flow.beta_deg * DTR;
        let (sa, ca) = alfa.sin_cos();
        let (sb, cb) = beta.sin_cos();
        let vinf = [ca * cb, -sb, sa * cb];
        // Rates given about the stability axes (standard orientation),
        // turned into body rates in AVL's geometric axes.
        let rx = DIR * flow.roll_rate * 2.0 / r.span;
        let rz = DIR * flow.yaw_rate * 2.0 / r.span;
        let wrot = [
            rx * ca - rz * sa,
            flow.pitch_rate * 2.0 / r.chord,
            rz * ca + rx * sa,
        ];
        let u = [vinf[0], vinf[1], vinf[2], wrot[0], wrot[1], wrot[2]];

        // GAMSUM / VELSUM: circulations and velocities at this condition,
        // and their derivatives with respect to the six unit components
        // (columns 0..6) and to the controls (columns 6..).
        let n = self.lattice.elements.len();
        let npar = 6 + nctl;
        let mut gam = vec![0.0; n];
        let mut gam_p = vec![0.0; n * npar];
        let mut vv = vec![[0.0; 3]; n];
        let mut vv_p = vec![[0.0; 3]; n * npar];
        for i in 0..n {
            let g = &self.gam[i * self.ncol..(i + 1) * self.ncol];
            let w = &self.vv[i * self.ncol..(i + 1) * self.ncol];
            for k in 0..6 {
                let mut gu = g[k];
                let mut wu = w[k];
                for c in 0..nctl {
                    gu += g[6 + 6 * c + k] * delta[c];
                    for m in 0..3 {
                        wu[m] += w[6 + 6 * c + k][m] * delta[c];
                    }
                }
                gam_p[i * npar + k] = gu;
                vv_p[i * npar + k] = wu;
            }
            for c in 0..nctl {
                let (mut gd, mut wd) = (0.0, [0.0; 3]);
                for k in 0..6 {
                    gd += g[6 + 6 * c + k] * u[k];
                    for m in 0..3 {
                        wd[m] += w[6 + 6 * c + k][m] * u[k];
                    }
                }
                gam_p[i * npar + 6 + c] = gd;
                vv_p[i * npar + 6 + c] = wd;
            }
            let (mut gi, mut wi) = (0.0, [0.0; 3]);
            for k in 0..6 {
                gi += gam_p[i * npar + k] * u[k];
                for m in 0..3 {
                    wi[m] += vv_p[i * npar + k][m] * u[k];
                }
            }
            gam[i] = gi;
            vv[i] = wi;
        }

        let state = Flow {
            alfa,
            vinf,
            vinf_a: [-sa * cb, 0.0, ca * cb],
            vinf_b: [-ca * sb, -cb, -sa * sb],
            wrot,
            gam,
            gam_p,
            vv,
            vv_p,
            npar,
        };
        Ok(aero(&self.model, &self.lattice, &state, flow.clone()))
    }
}
