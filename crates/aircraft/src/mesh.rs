//! Triangle mesh of an aircraft for a 3D view: each surface lofted from its
//! sections' airfoil coordinates, the fuselage as a body of revolution.

use aero_geometry::{Airfoil, Normalization};
use serde::{Deserialize, Serialize};

use crate::error::{AircraftError, Result};
use crate::params::{Aircraft, Fuselage};
use crate::planform::{Section, Surface, SurfaceKind, surfaces};

/// Which part a mesh is.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartKind {
    /// Main wing.
    Wing,
    /// Canard.
    Canard,
    /// Horizontal tail.
    Tail,
    /// Vertical fin.
    Fin,
    /// Fuselage.
    Fuselage,
}

impl From<SurfaceKind> for PartKind {
    fn from(k: SurfaceKind) -> Self {
        match k {
            SurfaceKind::Wing => Self::Wing,
            SurfaceKind::Canard => Self::Canard,
            SurfaceKind::Tail => Self::Tail,
            SurfaceKind::Fin => Self::Fin,
        }
    }
}

/// A closed triangle mesh.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeshPart {
    /// Which part.
    pub kind: PartKind,
    /// Vertex coordinates `x, y, z, x, y, z, …`, m.
    pub positions: Vec<f32>,
    /// Triangles, three vertex indices each.
    pub indices: Vec<u32>,
}

/// Resolution of the mesh.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MeshOptions {
    /// Points along each side of an airfoil (cosine spacing).
    pub chordwise: u32,
    /// Points around the fuselage.
    pub around: u32,
    /// Stations along the fuselage.
    pub along: u32,
}

impl Default for MeshOptions {
    fn default() -> Self {
        Self {
            chordwise: 24,
            around: 24,
            along: 40,
        }
    }
}

/// An airfoil resampled at common chord stations: `x` from the leading
/// edge (0) to the trailing edge (1), and the upper and lower surfaces.
struct Profile {
    x: Vec<f64>,
    upper: Vec<f64>,
    lower: Vec<f64>,
}

/// Linear interpolation of `y(x)` on a polyline ordered by increasing x.
fn interpolate(points: &[[f64; 2]], x: f64) -> f64 {
    for w in points.windows(2) {
        let ([x1, y1], [x2, y2]) = (w[0], w[1]);
        if (x1..=x2).contains(&x) || (x2..=x1).contains(&x) {
            return if x2 == x1 {
                y1
            } else {
                y1 + (x - x1) / (x2 - x1) * (y2 - y1)
            };
        }
    }
    // Outside (rounding at the ends): the nearest end.
    match (points.first(), points.last()) {
        (Some(a), Some(b)) => {
            if (x - a[0]).abs() < (x - b[0]).abs() {
                a[1]
            } else {
                b[1]
            }
        }
        _ => 0.0,
    }
}

fn profile(airfoil: &Airfoil, m: usize) -> Result<Profile> {
    let a = airfoil.normalized(Normalization::ChordAligned)?;
    let points: Vec<[f64; 2]> = a.points().collect();
    let le = points
        .iter()
        .enumerate()
        .min_by(|p, q| p.1[0].total_cmp(&q.1[0]))
        .map_or(0, |(i, _)| i);
    // Coordinates run from the upper trailing edge round the leading edge.
    let mut upper: Vec<[f64; 2]> = points[..=le].to_vec();
    upper.reverse();
    let lower: Vec<[f64; 2]> = points[le..].to_vec();
    let x: Vec<f64> = (0..=m)
        .map(|k| 0.5 * (1.0 - (std::f64::consts::PI * k as f64 / m as f64).cos()))
        .collect();
    Ok(Profile {
        upper: x.iter().map(|&xi| interpolate(&upper, xi)).collect(),
        lower: x.iter().map(|&xi| interpolate(&lower, xi)).collect(),
        x,
    })
}

/// A ring of points around a section: the upper surface from the trailing
/// edge to the leading edge, then the lower surface back to the trailing
/// edge (`2m + 1` points).
fn ring(kind: SurfaceKind, section: &Section, p: &Profile, mirror: bool) -> Vec<[f64; 3]> {
    let m = p.x.len() - 1;
    let uv = (0..=m)
        .rev()
        .map(|k| (p.x[k], p.upper[k]))
        .chain((1..=m).map(|k| (p.x[k], p.lower[k])));
    let [x0, y0, z0] = section.le;
    let c = section.chord;
    let (sin, cos) = section.twist_deg.to_radians().sin_cos();
    let sign = if mirror { -1.0 } else { 1.0 };
    uv.map(|(u, v)| match kind {
        SurfaceKind::Fin => [x0 + c * u, c * v, z0],
        SurfaceKind::Wing | SurfaceKind::Canard | SurfaceKind::Tail => [
            x0 + c * (u * cos + v * sin),
            sign * y0,
            z0 + c * (v * cos - u * sin),
        ],
    })
    .collect()
}

/// Appends the quad `a b c d` as two triangles.
fn quad(indices: &mut Vec<u32>, a: usize, b: usize, c: usize, d: usize) {
    for i in [a, b, c, a, c, d] {
        indices.push(u32::try_from(i).unwrap_or(u32::MAX));
    }
}

/// Mesh of one surface: rings joined by quads, closed at the tips.
fn surface_mesh(
    surface: &Surface,
    airfoil: &dyn Fn(&str) -> Option<Airfoil>,
    m: usize,
) -> Result<MeshPart> {
    let profiles = surface
        .sections
        .iter()
        .map(|s| {
            let a = airfoil(&s.airfoil)
                .ok_or_else(|| AircraftError::UnknownAirfoil(s.airfoil.clone()))?;
            profile(&a, m)
        })
        .collect::<Result<Vec<_>>>()?;
    // Rings from the left tip to the right tip (or root to tip for a fin).
    let mut rings: Vec<Vec<[f64; 3]>> = Vec::new();
    if surface.kind.mirrored() {
        for (s, p) in surface.sections.iter().zip(&profiles).skip(1).rev() {
            rings.push(ring(surface.kind, s, p, true));
        }
    }
    for (s, p) in surface.sections.iter().zip(&profiles) {
        rings.push(ring(surface.kind, s, p, false));
    }
    let n = 2 * m + 1;
    let mut positions = Vec::with_capacity(rings.len() * n * 3);
    for r in &rings {
        for p in r {
            positions.extend(p.iter().map(|&v| v as f32));
        }
    }
    let mut indices = Vec::new();
    for r in 0..rings.len() - 1 {
        let (a, b) = (r * n, (r + 1) * n);
        for j in 0..n {
            let k = (j + 1) % n;
            quad(&mut indices, a + j, a + k, b + k, b + j);
        }
    }
    // Tip caps: strips between upper and lower points of equal x, ending
    // in a triangle at the leading edge.
    for (base, flip) in [(0, true), ((rings.len() - 1) * n, false)] {
        let le = base + m;
        for k in (1..=m).rev() {
            let (u, u_next) = (base + m - k, base + m - k + 1);
            let (l, l_next) = (base + m + k, base + m + k - 1);
            let corners: Vec<usize> = match (k, flip) {
                (1, true) => vec![u, l, le],
                (1, false) => vec![u, le, l],
                (_, true) => vec![u, l, l_next, l_next, u_next, u],
                (_, false) => vec![u, u_next, l_next, l_next, l, u],
            };
            indices.extend(
                corners
                    .into_iter()
                    .map(|i| u32::try_from(i).unwrap_or(u32::MAX)),
            );
        }
    }
    Ok(MeshPart {
        kind: surface.kind.into(),
        positions,
        indices,
    })
}

/// Mesh of the fuselage: rings of `around` points at `along` stations
/// (denser at the nose), a point at the nose and a cap at the tail.
fn fuselage_mesh(f: &Fuselage, around: usize, along: usize) -> MeshPart {
    let mut positions: Vec<f32> = vec![0.0, 0.0, 0.0];
    let stations: Vec<f64> = (1..=along)
        .map(|i| {
            let t = i as f64 / along as f64;
            // Quadratic spacing: more stations around the nose.
            f.length * t * t.sqrt()
        })
        .collect();
    for &x in &stations {
        let r = f.radius(x.min(f.length));
        for j in 0..around {
            let a = std::f64::consts::TAU * j as f64 / around as f64;
            positions.extend([x as f32, (r * a.cos()) as f32, (r * a.sin()) as f32]);
        }
    }
    let tail_centre = positions.len() / 3;
    positions.extend([f.length as f32, 0.0, 0.0]);
    let mut indices = Vec::new();
    let ring = |i: usize| 1 + i * around;
    for j in 0..around {
        let k = (j + 1) % around;
        for v in [0, ring(0) + k, ring(0) + j] {
            indices.push(u32::try_from(v).unwrap_or(u32::MAX));
        }
    }
    for i in 0..along - 1 {
        for j in 0..around {
            let k = (j + 1) % around;
            quad(
                &mut indices,
                ring(i) + j,
                ring(i) + k,
                ring(i + 1) + k,
                ring(i + 1) + j,
            );
        }
    }
    for j in 0..around {
        let k = (j + 1) % around;
        for v in [tail_centre, ring(along - 1) + j, ring(along - 1) + k] {
            indices.push(u32::try_from(v).unwrap_or(u32::MAX));
        }
    }
    MeshPart {
        kind: PartKind::Fuselage,
        positions,
        indices,
    }
}

/// Mesh of `aircraft`; `airfoil` gives the coordinates of each airfoil key.
pub fn mesh(
    aircraft: &Aircraft,
    airfoil: &dyn Fn(&str) -> Option<Airfoil>,
    options: &MeshOptions,
) -> Result<Vec<MeshPart>> {
    let m = options.chordwise.clamp(4, 200) as usize;
    let mut parts = surfaces(aircraft)?
        .iter()
        .map(|s| surface_mesh(s, airfoil, m))
        .collect::<Result<Vec<_>>>()?;
    if let Some(f) = &aircraft.fuselage {
        parts.push(fuselage_mesh(
            f,
            options.around.clamp(6, 256) as usize,
            options.along.clamp(4, 512) as usize,
        ));
    }
    Ok(parts)
}

#[cfg(test)]
mod tests {
    use aero_geometry::{NacaOptions, naca};

    use super::*;
    use crate::params::{Preset, SurfaceParams};
    use crate::planform::Surface;

    fn airfoils(key: &str) -> Option<Airfoil> {
        let designation = key.strip_prefix("naca:").unwrap_or("2412");
        naca(designation, &NacaOptions::default()).ok()
    }

    fn vertices(part: &MeshPart) -> Vec<[f64; 3]> {
        part.positions
            .chunks(3)
            .map(|p| [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])])
            .collect()
    }

    /// Sum of the triangles' areas projected on the plane normal to `axis`
    /// (absolute values), and the enclosed volume.
    fn projected_area_and_volume(part: &MeshPart, axis: usize) -> (f64, f64) {
        let v = vertices(part);
        let (mut area, mut volume) = (0.0, 0.0);
        for t in part.indices.chunks(3) {
            let [a, b, c] = [v[t[0] as usize], v[t[1] as usize], v[t[2] as usize]];
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let w = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [
                u[1] * w[2] - u[2] * w[1],
                u[2] * w[0] - u[0] * w[2],
                u[0] * w[1] - u[1] * w[0],
            ];
            area += 0.5 * n[axis].abs();
            volume += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.0;
        }
        (area, volume)
    }

    /// The glider's wing without dihedral, incidence or twist, NACA 2412 at
    /// the root and 0012 at the tip.
    fn flat_wing() -> SurfaceParams {
        let mut w = Preset::Glider.aircraft().wing;
        w.incidence_deg = 0.0;
        for (s, airfoil) in w.sections.iter_mut().zip(["naca:2412", "naca:0012"]) {
            s.z = 0.0;
            s.twist_deg = 0.0;
            s.airfoil = airfoil.into();
        }
        w
    }

    #[test]
    fn wing_covers_its_planform_twice() {
        let mut a = Preset::Glider.aircraft();
        a.wing = flat_wing();
        let parts = mesh(&a, &airfoils, &MeshOptions::default()).unwrap();
        let wing = parts.iter().find(|p| p.kind == PartKind::Wing).unwrap();
        let s = Surface::from_params(SurfaceKind::Wing, &a.wing)
            .unwrap()
            .planform();
        let (area, volume) = projected_area_and_volume(wing, 2);
        assert!(
            (area - 2.0 * s.area).abs() < 1e-5 * s.area,
            "{area} vs 2 × {}",
            s.area
        );
        // Closed and consistently oriented: a non-zero enclosed volume of
        // the order of thickness × area.
        assert!(
            volume.abs() > 0.05 * 0.09 * s.area * s.mac && volume.abs() < 0.12 * s.area * s.mac
        );
        // Mirrored: every vertex has its image.
        let v = vertices(wing);
        for p in &v {
            assert!(v.iter().any(|q| (q[0] - p[0]).abs() < 1e-6
                && (q[1] + p[1]).abs() < 1e-6
                && (q[2] - p[2]).abs() < 1e-6));
        }
        let ymax = v.iter().map(|p| p[1]).fold(f64::MIN, f64::max);
        assert!((ymax - 0.75).abs() < 1e-6);
        assert!(wing.indices.iter().all(|&i| (i as usize) < v.len()));
    }

    #[test]
    fn fin_covers_its_side_view_twice() {
        let a = Preset::Trainer.aircraft();
        let parts = mesh(&a, &airfoils, &MeshOptions::default()).unwrap();
        let fin = parts.iter().find(|p| p.kind == PartKind::Fin).unwrap();
        let s = Surface::from_params(SurfaceKind::Fin, a.fin.as_ref().unwrap())
            .unwrap()
            .planform();
        let (area, _) = projected_area_and_volume(fin, 1);
        assert!((area - 2.0 * s.area).abs() < 1e-5 * s.area);
        assert!(vertices(fin).iter().all(|p| p[1].abs() < 0.2 * 0.15));
    }

    #[test]
    fn fuselage_volume() {
        let f = Fuselage {
            length: 1.0,
            diameter: 0.1,
        };
        let part = fuselage_mesh(&f, 64, 400);
        let (_, volume) = projected_area_and_volume(&part, 0);
        // ∫ π r² dx by the midpoint rule.
        let n = 20_000;
        let exact: f64 = (0..n)
            .map(|i| {
                let x = (i as f64 + 0.5) / n as f64;
                std::f64::consts::PI * f.radius(x).powi(2) / n as f64
            })
            .sum();
        assert!(
            (volume.abs() - exact).abs() < 0.01 * exact,
            "{volume} vs {exact}"
        );
    }

    #[test]
    fn every_preset_meshes() {
        for p in Preset::ALL {
            let parts = mesh(&p.aircraft(), &airfoils, &MeshOptions::default()).unwrap();
            assert!(parts.iter().any(|m| m.kind == PartKind::Wing));
            assert_eq!(
                parts.iter().any(|m| m.kind == PartKind::Tail),
                p != Preset::FlyingWing
            );
        }
    }

    #[test]
    fn unknown_airfoils_are_reported() {
        let a = Preset::Trainer.aircraft();
        let err = mesh(&a, &|_| None, &MeshOptions::default()).unwrap_err();
        assert!(matches!(err, AircraftError::UnknownAirfoil(k) if k == "catalogue:clarky"));
    }

    #[test]
    fn meshes_are_closed_and_consistently_oriented() {
        // Every directed edge appears once and its reverse once: the
        // surface is closed and its triangles all face the same way.
        for p in Preset::ALL {
            for part in mesh(&p.aircraft(), &airfoils, &MeshOptions::default()).unwrap() {
                let mut edges = std::collections::HashMap::new();
                for t in part.indices.chunks(3) {
                    for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                        *edges.entry((a, b)).or_insert(0) += 1;
                    }
                }
                for (&(a, b), &count) in &edges {
                    assert_eq!(
                        count, 1,
                        "{p:?} {:?}: edge {a}-{b} used {count} times",
                        part.kind
                    );
                    assert_eq!(
                        edges.get(&(b, a)),
                        Some(&1),
                        "{p:?} {:?}: open edge {a}-{b}",
                        part.kind
                    );
                }
            }
        }
    }

    #[test]
    fn incidence_lowers_the_trailing_edge() {
        let mut a = Preset::Trainer.aircraft();
        a.wing = SurfaceParams {
            incidence_deg: 5.0,
            ..flat_wing()
        };
        let parts = mesh(&a, &airfoils, &MeshOptions::default()).unwrap();
        let v = vertices(parts.iter().find(|p| p.kind == PartKind::Wing).unwrap());
        // The root ring is the second one (left tip, root, right tip); its
        // first point is the upper trailing edge, its middle point the
        // leading edge.
        let m = 24;
        let n = 2 * m + 1;
        let (te, le) = (v[n], v[n + m]);
        assert!((le[2] - a.wing.z).abs() < 1e-6);
        let c = a.wing.sections[0].chord;
        assert!(
            (te[2] - (a.wing.z - c * 5f64.to_radians().sin())).abs() < 2e-3 * c,
            "{te:?}"
        );
        assert!((te[0] - (a.wing.x + c * 5f64.to_radians().cos())).abs() < 2e-3 * c);
    }

    #[test]
    fn each_section_keeps_its_airfoil_and_a_canard_is_meshed() {
        // A thick root, a thin middle section raised by 5 cm, a thick tip.
        let section = |y: f64, z: f64, naca: &str| crate::params::SectionParams {
            y,
            chord: 0.2,
            x: 0.0,
            z,
            twist_deg: 0.0,
            airfoil: format!("naca:{naca}"),
        };
        let mut a = Preset::Trainer.aircraft();
        a.wing = SurfaceParams {
            x: 0.25,
            z: 0.0,
            incidence_deg: 0.0,
            sections: vec![
                section(0.0, 0.0, "0018"),
                section(0.3, 0.05, "0006"),
                section(0.6, 0.05, "0018"),
            ],
        };
        a.canard = Some(a.tail.as_ref().unwrap().scaled(0.8));
        let parts = mesh(&a, &airfoils, &MeshOptions::default()).unwrap();
        assert!(parts.iter().any(|p| p.kind == PartKind::Canard));
        let v = vertices(parts.iter().find(|p| p.kind == PartKind::Wing).unwrap());
        let thickness_at = |y: f64| {
            let z: Vec<f64> = v
                .iter()
                .filter(|p| (p[1] - y).abs() < 1e-6)
                .map(|p| p[2])
                .collect();
            let (lo, hi) = z
                .iter()
                .fold((f64::MAX, f64::MIN), |(lo, hi), &z| (lo.min(z), hi.max(z)));
            (hi - lo, 0.5 * (hi + lo))
        };
        let (root, _) = thickness_at(0.0);
        let (middle, middle_z) = thickness_at(0.3);
        let (tip, _) = thickness_at(0.6);
        assert!((root - 0.18 * 0.2).abs() < 2e-3 * 0.2, "{root}");
        assert!((middle - 0.06 * 0.2).abs() < 2e-3 * 0.2, "{middle}");
        assert!((tip - root).abs() < 1e-6);
        assert!((middle_z - 0.05).abs() < 1e-6);
    }

    #[test]
    fn interpolation_on_polylines() {
        let line = [[0.0, 0.0], [0.5, 1.0], [1.0, 0.0]];
        assert!((interpolate(&line, 0.25) - 0.5).abs() < 1e-15);
        assert!((interpolate(&line, 0.75) - 0.5).abs() < 1e-15);
        assert_eq!(interpolate(&line, 1.1), 0.0);
        assert_eq!(interpolate(&line, -0.1), 0.0);
    }
}
