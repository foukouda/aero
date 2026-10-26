//! The lattice of horseshoe vortices: strips and elements of every surface
//! and their normals (AVL `MAKESURF`, `SDUPL`, `ENCALC`).

use crate::error::{Result, invalid};
use crate::model::{ControlSurface, LiftingSurface, VlmModel};
use crate::spacing::{cspacer, spacer};
use crate::vec3::{V3, add, cross, dot, norm, scale, sub};

/// x/c of the spanwise axis used for the strip normal and the velocity
/// perpendicular to the strip (AVL `SAXFR`).
const SAXFR: f64 = 0.25;

/// Degrees to radians, as AVL's `DTR`.
pub(crate) const DTR: f64 = std::f64::consts::PI / 180.0;

/// A surface of the lattice: a declared surface or its mirror image.
#[derive(Debug, Clone)]
pub(crate) struct SurfaceLattice {
    pub(crate) name: String,
    pub(crate) first_strip: usize,
    pub(crate) n_strips: usize,
    pub(crate) chordwise: usize,
    /// Mirror image made by `YDUPLICATE` (strips run from tip to root).
    pub(crate) image: bool,
    pub(crate) wake: bool,
    pub(crate) load: bool,
    pub(crate) component: usize,
    /// Area of one side (sum of strip chord × width).
    pub(crate) area: f64,
    /// Area divided by the summed strip widths.
    pub(crate) mean_chord: f64,
}

/// A chordwise strip of elements.
#[derive(Debug, Clone)]
pub(crate) struct Strip {
    pub(crate) surface: usize,
    /// Leading edge and chord at the left edge, right edge and middle.
    pub(crate) rle1: V3,
    pub(crate) rle2: V3,
    pub(crate) rle: V3,
    pub(crate) chord: f64,
    /// Width in the y–z plane.
    pub(crate) width: f64,
    /// Incidence in radians.
    pub(crate) ainc: f64,
    pub(crate) first: usize,
    pub(crate) count: usize,
    /// Spanwise unit vector of the strip and its normal in the y–z plane.
    pub(crate) ess: V3,
    pub(crate) ensy: f64,
    pub(crate) ensz: f64,
    /// Reference point on the spanwise axis.
    pub(crate) sref: V3,
}

/// A horseshoe vortex element.
#[derive(Debug, Clone)]
pub(crate) struct Element {
    /// Left and right ends and middle of the bound leg.
    pub(crate) rv1: V3,
    pub(crate) rv2: V3,
    pub(crate) rv: V3,
    /// Control point.
    pub(crate) rc: V3,
    /// Chord of the strip.
    pub(crate) chord: f64,
    /// Camber slope at the control point.
    pub(crate) slope: f64,
    pub(crate) component: usize,
    /// Normal at the control point.
    pub(crate) enc: V3,
}

/// The whole lattice.
#[derive(Debug, Clone)]
pub(crate) struct Lattice {
    pub(crate) surfaces: Vec<SurfaceLattice>,
    pub(crate) strips: Vec<Strip>,
    pub(crate) elements: Vec<Element>,
    /// Names of the control variables.
    pub(crate) controls: Vec<String>,
    /// Change of the control-point normals per degree of each control,
    /// `[element * controls + control]`.
    pub(crate) enc_d: Vec<V3>,
}

/// Control data of a strip for one control variable.
#[derive(Clone, Copy)]
struct StripControl {
    gain: f64,
    xled: f64,
    xted: f64,
    hinge: V3,
    refl: f64,
}

impl Lattice {
    /// Builds the lattice of a validated model.
    pub(crate) fn build(model: &VlmModel) -> Result<Self> {
        let controls = model.control_names();
        let mut lattice = Lattice {
            surfaces: Vec::new(),
            strips: Vec::new(),
            elements: Vec::new(),
            controls,
            enc_d: Vec::new(),
        };
        // Strip control data and element control fractions, kept until the
        // normals are computed.
        let mut strip_controls: Vec<Vec<Option<StripControl>>> = Vec::new();
        let mut dcontrol: Vec<Vec<f64>> = Vec::new();
        for spec in &model.surfaces {
            let index = lattice.surfaces.len();
            let component = spec.component.unwrap_or(index + 1);
            lattice.make_surface(spec, component, &mut strip_controls, &mut dcontrol)?;
            if let Some(y) = spec.y_duplicate {
                lattice.duplicate(index, y, &mut strip_controls, &mut dcontrol);
            }
        }
        lattice.normals(&strip_controls, &dcontrol)?;
        Ok(lattice)
    }

    /// AVL `MAKESURF`.
    fn make_surface(
        &mut self,
        spec: &LiftingSurface,
        component: usize,
        strip_controls: &mut Vec<Vec<Option<StripControl>>>,
        dcontrol: &mut Vec<Vec<f64>>,
    ) -> Result<()> {
        let secs = &spec.sections;
        let nsec = secs.len();
        let nvc = spec.chordwise.count;
        let isurf = self.surfaces.len();
        let first_strip = self.strips.len();

        // Arc length of the sections in the y–z plane (unscaled).
        let mut yzlen = vec![0.0; nsec];
        for i in 1..nsec {
            let dy = secs[i].leading_edge[1] - secs[i - 1].leading_edge[1];
            let dz = secs[i].leading_edge[2] - secs[i - 1].leading_edge[2];
            yzlen[i] = yzlen[i - 1] + (dy * dy + dz * dz).sqrt();
        }

        // Strip edges (ypt) and middles (ycp) along the arc length, and the
        // edge index of every section (iptloc).
        let mut ypt: Vec<f64>;
        let mut ycp: Vec<f64>;
        let mut iptloc = vec![0usize; nsec];
        match spec.spanwise {
            None => {
                ypt = vec![yzlen[0]];
                ycp = Vec::new();
                let mut nvs = 0;
                for isec in 0..nsec - 1 {
                    let dyzlen = yzlen[isec + 1] - yzlen[isec];
                    let sp = secs[isec]
                        .spanwise
                        .ok_or_else(|| invalid(format!("{}: no spanwise spacing", spec.name)))?;
                    let nvint = sp.count;
                    let fspace = spacer(2 * nvint + 1, sp.parameter);
                    let y0 = ypt[nvs];
                    for n in 1..=nvint {
                        ycp.push(y0 + dyzlen * fspace[2 * n - 1]);
                        ypt.push(y0 + dyzlen * fspace[2 * n]);
                    }
                    iptloc[isec + 1] = nvs + nvint;
                    nvs += nvint;
                }
            }
            Some(sp) => {
                let nvs = sp.count;
                let fspace = spacer(2 * nvs + 1, sp.parameter);
                let total = yzlen[nsec - 1] - yzlen[0];
                ypt = vec![yzlen[0]];
                ycp = Vec::with_capacity(nvs);
                for ivs in 1..=nvs {
                    ycp.push(yzlen[0] + total * fspace[2 * ivs - 1]);
                    ypt.push(yzlen[0] + total * fspace[2 * ivs]);
                }
                let npt = nvs + 1;
                // Node nearest each interior section.
                for isec in 1..nsec - 1 {
                    let mut best = 1.0e9;
                    iptloc[isec] = 0;
                    for (ipt, y) in ypt.iter().enumerate() {
                        let d = (yzlen[isec] - y).abs();
                        if d < best {
                            best = d;
                            iptloc[isec] = ipt;
                        }
                    }
                }
                iptloc[0] = 0;
                iptloc[nsec - 1] = npt - 1;
                // Stretch the spacing so that nodes fall on the sections.
                let too_few = || {
                    invalid(format!(
                        "{}: too few spanwise vortices to match the sections",
                        spec.name
                    ))
                };
                for isec in 1..nsec - 1 {
                    let (ipt1, ipt2) = (iptloc[isec - 1], iptloc[isec]);
                    if ipt1 == ipt2 {
                        return Err(too_few());
                    }
                    let ypt1 = ypt[ipt1];
                    let ys = (yzlen[isec] - yzlen[isec - 1]) / (ypt[ipt2] - ypt[ipt1]);
                    for ipt in ipt1..ipt2 {
                        ypt[ipt] = yzlen[isec - 1] + ys * (ypt[ipt] - ypt1);
                        ycp[ipt] = yzlen[isec - 1] + ys * (ycp[ipt] - ypt1);
                    }
                    let (ipt1, ipt2) = (iptloc[isec], iptloc[isec + 1]);
                    if ipt1 == ipt2 {
                        return Err(too_few());
                    }
                    let ypt1 = ypt[ipt1];
                    let ys = (ypt[ipt2] - yzlen[isec]) / (ypt[ipt2] - ypt[ipt1]);
                    for ipt in ipt1..ipt2 {
                        ypt[ipt] = yzlen[isec] + ys * (ypt[ipt] - ypt1);
                        ycp[ipt] = yzlen[isec] + ys * (ycp[ipt] - ypt1);
                    }
                }
            }
        }

        let sc = spec.scale;
        let place = |p: [f64; 3]| -> V3 {
            [
                sc[0] * p[0] + spec.translate[0],
                sc[1] * p[1] + spec.translate[1],
                sc[2] * p[2] + spec.translate[2],
            ]
        };
        let addinc = spec.angle_deg * DTR;
        let ncontrol = self.controls.len();

        for isec in 0..nsec - 1 {
            let (left, right) = (&secs[isec], &secs[isec + 1]);
            let lel = place(left.leading_edge);
            let ler = place(right.leading_edge);
            let width = ((ler[1] - lel[1]).powi(2) + (ler[2] - lel[2]).powi(2)).sqrt();
            let chordl = sc[0] * left.chord;
            let chordr = sc[0] * right.chord;
            let (clafl, clafr) = (left.lift_slope_factor, right.lift_slope_factor);
            let aincl = left.incidence_deg * DTR + addinc;
            let aincr = right.incidence_deg * DTR + addinc;
            let (chsinl, chsinr) = (chordl * aincl.sin(), chordr * aincr.sin());
            let (chcosl, chcosr) = (chordl * aincl.cos(), chordr * aincr.cos());

            // Declarations of each control variable on both sections (the
            // last one of a section wins, as in AVL).
            let find =
                |sec: &'_ crate::model::SurfaceSection, name: &str| -> Option<ControlSurface> {
                    sec.controls.iter().rev().find(|c| c.name == name).cloned()
                };
            let decl: Vec<Option<(ControlSurface, ControlSurface)>> = self
                .controls
                .iter()
                .map(|name| Some((find(left, name)?, find(right, name)?)))
                .collect();

            let (iptl, iptr) = (iptloc[isec], iptloc[isec + 1]);
            for ispan in 1..=(iptr - iptl) {
                let ipt1 = iptl + ispan - 1;
                let ipt2 = iptl + ispan;
                let ivs = iptl + ispan - 1;
                let span = ypt[iptr] - ypt[iptl];
                let f1 = (ypt[ipt1] - ypt[iptl]) / span;
                let f2 = (ypt[ipt2] - ypt[iptl]) / span;
                let fc = (ycp[ivs] - ypt[iptl]) / span;
                let lerp = |f: f64| -> V3 {
                    [
                        (1.0 - f) * lel[0] + f * ler[0],
                        (1.0 - f) * lel[1] + f * ler[1],
                        (1.0 - f) * lel[2] + f * ler[2],
                    ]
                };
                let rle1 = lerp(f1);
                let chord1 = (1.0 - f1) * chordl + f1 * chordr;
                let rle2 = lerp(f2);
                let chord2 = (1.0 - f2) * chordl + f2 * chordr;
                let rle = lerp(fc);
                let chordc = (1.0 - fc) * chordl + fc * chordr;
                if chordc <= 0.0 {
                    return Err(invalid(format!("{}: a strip has no chord", spec.name)));
                }
                let chsin = chsinl + fc * (chsinr - chsinl);
                let chcos = chcosl + fc * (chcosr - chcosl);
                let ainc = chsin.atan2(chcos);

                // Controls of this strip.
                let mut sc_strip = Vec::with_capacity(ncontrol);
                for d in &decl {
                    sc_strip.push(d.as_ref().map(|(cl, cr)| {
                        let gain = cl.gain * (1.0 - fc) + cr.gain * fc;
                        let xhd = chordl * cl.hinge * (1.0 - fc) + chordr * cr.hinge * fc;
                        let (xled, xted) = if xhd >= 0.0 {
                            (xhd, chordc)
                        } else {
                            (0.0, -xhd)
                        };
                        let mut vh = [cl.axis[0] * sc[0], cl.axis[1] * sc[1], cl.axis[2] * sc[2]];
                        if dot(vh, vh) == 0.0 {
                            // Along the hinge line.
                            vh = [
                                (right.leading_edge[0] + (chordr * cr.hinge).abs()
                                    - left.leading_edge[0]
                                    - (chordl * cl.hinge).abs())
                                    * sc[0],
                                (right.leading_edge[1] - left.leading_edge[1]) * sc[1],
                                (right.leading_edge[2] - left.leading_edge[2]) * sc[2],
                            ];
                        }
                        let vmod = norm(vh);
                        StripControl {
                            gain,
                            xled,
                            xted,
                            hinge: scale(vh, 1.0 / vmod),
                            refl: cl.duplicate_sign,
                        }
                    }));
                }

                let clafc = (1.0 - fc) * (chordl / chordc) * clafl + fc * (chordr / chordc) * clafr;
                let cs = cspacer(nvc, spec.chordwise.parameter, clafc);

                let first = self.elements.len();
                for ivc in 0..nvc {
                    let xvr = cs.vortex[ivc];
                    let xcp = cs.control[ivc];
                    let slope = (1.0 - fc) * (chordl / chordc) * left.camber.slope_at(xcp)
                        + fc * (chordr / chordc) * right.camber.slope_at(xcp);
                    let dxoc = cs.edges[ivc + 1] - cs.edges[ivc];
                    self.elements.push(Element {
                        rv1: [rle1[0] + xvr * chord1, rle1[1], rle1[2]],
                        rv2: [rle2[0] + xvr * chord2, rle2[1], rle2[2]],
                        rv: [rle[0] + xvr * chordc, rle[1], rle[2]],
                        rc: [rle[0] + xcp * chordc, rle[1], rle[2]],
                        chord: chordc,
                        slope,
                        component,
                        enc: [0.0; 3],
                    });
                    dcontrol.push(
                        sc_strip
                            .iter()
                            .map(|c| match c {
                                Some(c) => {
                                    let frac = |x: f64| {
                                        ((x / chordc - cs.edges[ivc]) / dxoc).clamp(0.0, 1.0)
                                    };
                                    c.gain * (frac(c.xted) - frac(c.xled))
                                }
                                None => 0.0,
                            })
                            .collect(),
                    );
                }
                self.strips.push(Strip {
                    surface: isurf,
                    rle1,
                    rle2,
                    rle,
                    chord: chordc,
                    width: (f2 - f1).abs() * width,
                    ainc,
                    first,
                    count: nvc,
                    ess: [0.0; 3],
                    ensy: 0.0,
                    ensz: 0.0,
                    sref: [0.0; 3],
                });
                strip_controls.push(sc_strip);
            }
        }

        let n_strips = self.strips.len() - first_strip;
        let (mut area, mut wtot) = (0.0, 0.0);
        for s in &self.strips[first_strip..] {
            area += s.width * s.chord;
            wtot += s.width;
        }
        self.surfaces.push(SurfaceLattice {
            name: spec.name.clone(),
            first_strip,
            n_strips,
            chordwise: nvc,
            image: false,
            wake: spec.wake,
            load: spec.load,
            component,
            area,
            mean_chord: if wtot == 0.0 { 0.0 } else { area / wtot },
        });
        Ok(())
    }

    /// AVL `SDUPL`: mirror image of surface `nn` about `y = ydupl`; the
    /// strip edges are swapped to keep the sense of the circulation.
    fn duplicate(
        &mut self,
        nn: usize,
        ydupl: f64,
        strip_controls: &mut Vec<Vec<Option<StripControl>>>,
        dcontrol: &mut Vec<Vec<f64>>,
    ) {
        let yoff = 2.0 * ydupl;
        let mirror = |p: V3| -> V3 { [p[0], yoff - p[1], p[2]] };
        let src = self.surfaces[nn].clone();
        let isurf = self.surfaces.len();
        let first_strip = self.strips.len();
        for jj in src.first_strip..src.first_strip + src.n_strips {
            let s = self.strips[jj].clone();
            let first = self.elements.len();
            for ii in s.first..s.first + s.count {
                let e = self.elements[ii].clone();
                self.elements.push(Element {
                    rv1: mirror(e.rv2),
                    rv2: mirror(e.rv1),
                    rv: mirror(e.rv),
                    rc: mirror(e.rc),
                    ..e
                });
                let refl: Vec<f64> = strip_controls[jj]
                    .iter()
                    .map(|c| c.map_or(0.0, |c| c.refl))
                    .collect();
                let d: Vec<f64> = dcontrol[ii]
                    .iter()
                    .zip(&refl)
                    .map(|(d, r)| -d * r)
                    .collect();
                dcontrol.push(d);
            }
            strip_controls.push(
                strip_controls[jj]
                    .iter()
                    .map(|c| {
                        c.map(|c| StripControl {
                            hinge: [c.hinge[0], -c.hinge[1], c.hinge[2]],
                            ..c
                        })
                    })
                    .collect(),
            );
            self.strips.push(Strip {
                surface: isurf,
                rle1: mirror(s.rle2),
                rle2: mirror(s.rle1),
                rle: mirror(s.rle),
                first,
                ..s
            });
        }
        self.surfaces.push(SurfaceLattice {
            name: format!("{} (YDUP)", src.name),
            first_strip,
            image: true,
            ..src
        });
    }

    /// AVL `ENCALC`: strip axes and control-point normals, and their change
    /// with the control deflections.
    // The element index addresses three arrays (elements, deflection
    // fractions, normal changes).
    #[allow(clippy::needless_range_loop)]
    fn normals(
        &mut self,
        strip_controls: &[Vec<Option<StripControl>>],
        dcontrol: &[Vec<f64>],
    ) -> Result<()> {
        let ncontrol = self.controls.len();
        self.enc_d = vec![[0.0; 3]; self.elements.len() * ncontrol];
        for (j, strip) in self.strips.iter_mut().enumerate() {
            let le = &self.elements[strip.first];
            let te = &self.elements[strip.first + strip.count - 1];
            let dle = sub(le.rv2, le.rv1);
            let dte = sub(te.rv2, te.rv1);
            let dt = add(scale(dle, 1.0 - SAXFR), scale(dte, SAXFR));
            let dyz = (dt[1] * dt[1] + dt[2] * dt[2]).sqrt();
            if dyz == 0.0 || dyz.is_nan() {
                return Err(invalid("a strip has no width in the y-z plane"));
            }
            strip.ess = scale(dt, 1.0 / norm(dt));
            strip.ensy = -dt[2] / dyz;
            strip.ensz = dt[1] / dyz;
            strip.sref = add(scale(le.rv, 1.0 - SAXFR), scale(te.rv, SAXFR));
            let es = [0.0, strip.ensy, strip.ensz];

            for i in strip.first..strip.first + strip.count {
                let e = &mut self.elements[i];
                let db = sub(e.rv2, e.rv1);
                let eb = scale(db, 1.0 / norm(db));
                let ang = strip.ainc - e.slope.atan();
                let (sinc, cosc) = ang.sin_cos();
                let ec = [cosc, -sinc * es[1], -sinc * es[2]];
                let ecxb = cross(ec, eb);
                let emag = norm(ecxb);
                e.enc = if emag != 0.0 {
                    scale(ecxb, 1.0 / emag)
                } else {
                    es
                };
                for (n, &d) in dcontrol[i].iter().enumerate() {
                    if d == 0.0 {
                        continue;
                    }
                    let Some(c) = strip_controls[j][n] else {
                        continue;
                    };
                    let endot = dot(e.enc, c.hinge);
                    let ep = sub(e.enc, scale(c.hinge, endot));
                    let eq = cross(c.hinge, ep);
                    let k = i * ncontrol + n;
                    self.enc_d[k] = add(self.enc_d[k], scale(eq, DTR * d));
                }
            }
        }
        Ok(())
    }
}
