//! Reading AVL geometry files (`.avl`), for the features of the vortex
//! lattice: header, `SURFACE` with `YDUPLICATE`, `SCALE`, `TRANSLATE`,
//! `ANGLE`, `COMPONENT`, `NOWAKE`, `NOLOAD`; `SECTION` with `NACA`,
//! `AIRFOIL`, `AFILE`, `CLAF` and `CONTROL`; `CORE`. Bodies, design
//! variables, `CDCL` polars, `NOALBE` and ground effect are refused.
//! Unknown lines are skipped, as AVL does.

use crate::camber::Camber;
use crate::error::{Result, VlmError};
use crate::model::{
    ControlSurface, LiftingSurface, Reference, Spacing, SurfaceSection, VlmModel, VortexCore,
};

/// Reads an AVL geometry file. `load_airfoil` returns the text of an
/// airfoil file named by `AFILE` (relative to the geometry file), or `None`
/// when it does not exist.
pub fn parse_avl(
    text: &str,
    mut load_airfoil: impl FnMut(&str) -> Option<String>,
) -> Result<VlmModel> {
    let mut lines = Lines::new(text);
    let title = lines
        .next_line()
        .map(|(_, l)| l.to_owned())
        .unwrap_or_default();
    let mach = lines.numbers(1, "the Mach number")?[0];
    let sym = lines.numbers(3, "iYsym iZsym Zsym")?;
    if sym[1] != 0.0 {
        return Err(lines.error("ground effect (iZsym) is not supported"));
    }
    if sym[0] < 0.0 {
        return Err(lines.error("a free-surface symmetry plane (iYsym < 0) is not supported"));
    }
    let y_symmetry = sym[0] > 0.0;
    let srefs = lines.numbers(3, "Sref Cref Bref")?;
    let point = lines.numbers(3, "Xref Yref Zref")?;
    let positive = |v: f64| if v <= 0.0 { 1.0 } else { v };
    let mut model = VlmModel {
        title,
        mach,
        y_symmetry,
        reference: Reference {
            area: positive(srefs[0]),
            chord: positive(srefs[1]),
            span: positive(srefs[2]),
            point: [point[0], point[1], point[2]],
        },
        cd_p: 0.0,
        core: VortexCore::default(),
        surfaces: Vec::new(),
    };
    // Optional CDp line.
    if let Some((_, line)) = lines.peek()
        && let Some(v) = numbers(line).first()
    {
        model.cd_p = *v;
        lines.next_line();
    }

    let mut surface: Option<LiftingSurface> = None;
    while let Some((lineno, line)) = lines.next_line() {
        let keyword: String = line.chars().take(4).collect::<String>().to_uppercase();
        let err = |reason: &str| VlmError::AvlFile {
            line: lineno,
            reason: reason.to_owned(),
        };
        match keyword.as_str() {
            "SURF" => {
                if let Some(s) = surface.take() {
                    model.surfaces.push(s);
                }
                let name = lines
                    .next_line()
                    .map(|(_, l)| l.to_owned())
                    .ok_or_else(|| err("SURFACE without a name"))?;
                let v = lines.numbers(2, "Nchord Cspace [Nspan Sspace]")?;
                let count = |x: f64| (x + 0.001) as usize;
                let mut s = LiftingSurface::new(name, Spacing::new(count(v[0]), v[1]), Vec::new());
                if v.len() >= 4 && count(v[2]) > 0 {
                    s.spanwise = Some(Spacing::new(count(v[2]), v[3]));
                }
                surface = Some(s);
            }
            "YDUP" | "INDE" | "COMP" | "SCAL" | "TRAN" | "ANGL" | "AINC" | "NOWA" | "NOLO" => {
                let s = surface
                    .as_mut()
                    .ok_or_else(|| err("keyword outside of a surface"))?;
                match keyword.as_str() {
                    "YDUP" => s.y_duplicate = Some(lines.numbers(1, "YDUPLICATE")?[0]),
                    "INDE" | "COMP" => {
                        s.component = Some(lines.numbers(1, "COMPONENT")?[0] as usize);
                    }
                    "SCAL" => {
                        let v = lines.numbers(3, "SCALE")?;
                        s.scale = [v[0], v[1], v[2]];
                    }
                    "TRAN" => {
                        let v = lines.numbers(3, "TRANSLATE")?;
                        s.translate = [v[0], v[1], v[2]];
                    }
                    "ANGL" | "AINC" => s.angle_deg = lines.numbers(1, "ANGLE")?[0],
                    "NOWA" => s.wake = false,
                    _ => s.load = false,
                }
            }
            "SECT" => {
                let s = surface
                    .as_mut()
                    .ok_or_else(|| err("SECTION outside of a surface"))?;
                let v = lines.numbers(5, "Xle Yle Zle Chord Ainc [Nspan Sspace]")?;
                let mut sec = SurfaceSection::new([v[0], v[1], v[2]], v[3]);
                sec.incidence_deg = v[4];
                if v.len() >= 7 && (v[5] + 0.001) as usize > 0 {
                    sec.spanwise = Some(Spacing::new((v[5] + 0.001) as usize, v[6]));
                }
                s.sections.push(sec);
            }
            "NACA" | "AIRF" | "AFIL" | "CLAF" | "CONT" => {
                let sec = surface
                    .as_mut()
                    .and_then(|s| s.sections.last_mut())
                    .ok_or_else(|| err("keyword outside of a section"))?;
                match keyword.as_str() {
                    "NACA" => {
                        let range = x_range(line);
                        let (_, d) = lines
                            .next_line()
                            .ok_or_else(|| err("NACA without digits"))?;
                        let digits: u32 = d
                            .split_whitespace()
                            .next()
                            .and_then(|t| t.parse().ok())
                            .ok_or_else(|| err("bad NACA designation"))?;
                        sec.camber = Camber::naca4(digits, range)?;
                    }
                    "AIRF" => {
                        let range = x_range(line);
                        let (mut xs, mut ys) = (Vec::new(), Vec::new());
                        while let Some((_, l)) = lines.peek() {
                            let v = numbers(l);
                            if v.len() < 2 {
                                break;
                            }
                            xs.push(v[0]);
                            ys.push(v[1]);
                            lines.next_line();
                        }
                        sec.camber = Camber::from_airfoil("AIRFOIL", &xs, &ys, range)?;
                    }
                    "AFIL" => {
                        let range = x_range(line);
                        let (_, l) = lines
                            .next_line()
                            .ok_or_else(|| err("AFILE without a file"))?;
                        let name = file_name(l);
                        let text = load_airfoil(&name).ok_or_else(|| VlmError::Airfoil {
                            name: name.clone(),
                            reason: "file not found".into(),
                        })?;
                        let (xs, ys) = read_airfoil_file(&name, &text)?;
                        sec.camber = Camber::from_airfoil(&name, &xs, &ys, range)?;
                    }
                    "CLAF" => {
                        let v = lines.numbers(1, "CLAF")?[0];
                        sec.lift_slope_factor = if v <= 0.0 || v >= 2.0 { 1.0 } else { v };
                    }
                    _ => {
                        let (_, l) = lines
                            .next_line()
                            .ok_or_else(|| err("CONTROL without data"))?;
                        let mut tokens = l.split_whitespace();
                        let name = tokens.next().ok_or_else(|| err("CONTROL without a name"))?;
                        let rest: Vec<&str> = tokens.collect();
                        let v = numbers(&rest.join(" "));
                        let mut c = ControlSurface::new(name, 0.0);
                        c.gain = v.first().copied().unwrap_or(1.0);
                        c.hinge = v.get(1).copied().unwrap_or(0.0);
                        if v.len() >= 5 {
                            c.axis = [v[2], v[3], v[4]];
                        }
                        c.duplicate_sign = v.get(5).copied().unwrap_or(1.0);
                        sec.controls.push(c);
                    }
                }
            }
            "CORE" => {
                let v = lines.numbers(3, "CORE")?;
                model.core = VortexCore {
                    chord_fraction: v[0],
                    width_fraction: v[1],
                };
            }
            "BODY" | "BSEC" | "BFIL" => return Err(err("bodies are not supported")),
            "DESI" => return Err(err("design variables are not supported")),
            "CDCL" => return Err(err("CDCL polars are not supported")),
            "NOAL" => return Err(err("NOALBE is not supported")),
            "EOF" => break,
            _ => {} // AVL skips unknown lines
        }
    }
    if let Some(s) = surface.take() {
        model.surfaces.push(s);
    }
    Ok(model)
}

/// The significant lines of an AVL file: comments (`!`, `#` at the start)
/// and blank lines skipped, leading blanks and everything after `!` removed.
struct Lines<'a> {
    lines: Vec<(usize, &'a str)>,
    pos: usize,
}

impl<'a> Lines<'a> {
    fn new(text: &'a str) -> Self {
        let lines = text
            .lines()
            .enumerate()
            .filter_map(|(i, raw)| {
                if raw.starts_with('!') || raw.starts_with('#') {
                    return None;
                }
                let l = raw.trim_start();
                let l = match l.find('!') {
                    Some(k) if k > 0 => &l[..k],
                    _ => l,
                };
                let l = l.trim_end();
                (!l.is_empty() && !l.starts_with('!')).then_some((i + 1, l))
            })
            .collect();
        Self { lines, pos: 0 }
    }

    fn next_line(&mut self) -> Option<(usize, &'a str)> {
        let l = self.lines.get(self.pos).copied();
        self.pos += 1;
        l
    }

    fn peek(&self) -> Option<(usize, &'a str)> {
        self.lines.get(self.pos).copied()
    }

    /// Error at the current line.
    fn error(&self, reason: &str) -> VlmError {
        VlmError::AvlFile {
            line: self
                .lines
                .get(self.pos.saturating_sub(1))
                .map_or(0, |l| l.0),
            reason: reason.to_owned(),
        }
    }

    /// The next line as at least `min` numbers.
    fn numbers(&mut self, min: usize, what: &str) -> Result<Vec<f64>> {
        let (line, text) = self.next_line().ok_or_else(|| VlmError::AvlFile {
            line: 0,
            reason: format!("end of file, expected {what}"),
        })?;
        let v = numbers(text);
        if v.len() < min {
            return Err(VlmError::AvlFile {
                line,
                reason: format!("expected {what}"),
            });
        }
        Ok(v)
    }
}

/// The leading numbers of a line (separated by blanks or commas; Fortran
/// `D` exponents accepted), up to the first token that is not a number.
fn numbers(line: &str) -> Vec<f64> {
    let mut out = Vec::new();
    for token in line
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|t| !t.is_empty())
    {
        match token.replace(['D', 'd'], "e").parse::<f64>() {
            Ok(v) if v.is_finite() => out.push(v),
            _ => break,
        }
    }
    out
}

/// Optional chord-fraction range after `NACA`, `AIRFOIL` or `AFILE`.
fn x_range(line: &str) -> (f64, f64) {
    let rest = line.split_once(char::is_whitespace).map_or("", |(_, r)| r);
    let v = numbers(rest);
    if v.len() >= 2 {
        (v[0].max(0.0), v[1].min(1.0))
    } else {
        (0.0, 1.0)
    }
}

/// File name of an `AFILE` line, possibly between double quotes.
fn file_name(line: &str) -> String {
    let l = line.trim();
    if let Some(rest) = l.strip_prefix('"')
        && let Some(end) = rest.find('"')
    {
        return rest[..end].to_owned();
    }
    l.to_owned()
}

/// Coordinates of an airfoil file as AVL reads them (`READBL`): an
/// optional name line, then x y pairs; clockwise points are reversed.
fn read_airfoil_file(name: &str, text: &str) -> Result<(Vec<f64>, Vec<f64>)> {
    const NUMERIC: &str = "0123456789-+.,EDed \t";
    let mut lines = text.lines().filter(|l| !l.trim().is_empty()).peekable();
    if let Some(first) = lines.peek()
        && first.chars().any(|c| !NUMERIC.contains(c))
    {
        lines.next();
        if let Some(second) = lines.peek()
            && numbers(second).len() >= 4
        {
            return Err(VlmError::Airfoil {
                name: name.to_owned(),
                reason: "multi-element blade files are not supported".into(),
            });
        }
    }
    let (mut xs, mut ys) = (Vec::new(), Vec::new());
    for l in lines {
        let v = numbers(l);
        if v.len() < 2 {
            return Err(VlmError::Airfoil {
                name: name.to_owned(),
                reason: format!("bad coordinate line {l:?}"),
            });
        }
        if v[0] == 999.0 {
            break;
        }
        xs.push(v[0]);
        ys.push(v[1]);
    }
    let n = xs.len();
    let mut area = 0.0;
    for i in 0..n.saturating_sub(1) {
        let rx = xs[i + 1] + xs[i];
        let ry = ys[i + 1] + ys[i];
        let dx = xs[i + 1] - xs[i];
        let dy = ys[i + 1] - ys[i];
        area += 0.25 * (rx * dy - ry * dx);
    }
    if area < 0.0 {
        xs.reverse();
        ys.reverse();
    }
    Ok((xs, ys))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WING: &str = "Test wing
0.2           ! Mach
1 0 0.0
4.0 0.5 8.0
0.1 0.0 0.0
0.012
# a comment
SURFACE
Wing
6 1.0 10 -2.0
YDUPLICATE
0.0
ANGLE
2.0
SECTION
0.0 0.0 0.0 0.5 1.0
NACA 0.0 0.9
2412
CONTROL
flap 0.5 0.75 0 1 0 -1
SECTION
0.1 4.0 0.2 0.3 -1.0 8 1.0
CLAF
1.1
";

    #[test]
    fn reads_header_surface_and_sections() {
        let m = parse_avl(WING, |_| None).unwrap();
        assert_eq!(m.title, "Test wing");
        assert_eq!(m.mach, 0.2);
        assert!(m.y_symmetry);
        assert_eq!(m.reference.area, 4.0);
        assert_eq!(m.reference.point, [0.1, 0.0, 0.0]);
        assert_eq!(m.cd_p, 0.012);
        let s = &m.surfaces[0];
        assert_eq!(s.chordwise, Spacing::new(6, 1.0));
        assert_eq!(s.spanwise, Some(Spacing::new(10, -2.0)));
        assert_eq!(s.y_duplicate, Some(0.0));
        assert_eq!(s.angle_deg, 2.0);
        assert_eq!(s.sections.len(), 2);
        let (a, b) = (&s.sections[0], &s.sections[1]);
        assert_eq!(a.incidence_deg, 1.0);
        assert_eq!(a.controls[0].name, "flap");
        assert_eq!(a.controls[0].gain, 0.5);
        assert_eq!(a.controls[0].hinge, 0.75);
        assert_eq!(a.controls[0].axis, [0.0, 1.0, 0.0]);
        assert_eq!(a.controls[0].duplicate_sign, -1.0);
        assert_eq!(a.camber, Camber::naca4(2412, (0.0, 0.9)).unwrap());
        assert_eq!(b.spanwise, Some(Spacing::new(8, 1.0)));
        assert_eq!(b.lift_slope_factor, 1.1);
        // A mirror image at y = 0 with a symmetry plane is refused.
        assert!(m.validate().is_err());
    }

    #[test]
    fn refuses_what_is_not_supported() {
        let ground = WING.replacen("1 0 0.0", "1 1 -0.5", 1);
        assert!(parse_avl(&ground, |_| None).is_err());
        let body = format!("{WING}BODY\nFuse\n");
        assert!(parse_avl(&body, |_| None).is_err());
        let missing = format!("{WING}AFILE\nnone.dat\n");
        assert!(matches!(
            parse_avl(&missing, |_| None),
            Err(VlmError::Airfoil { .. })
        ));
    }

    #[test]
    fn airfoil_files_with_a_name_and_clockwise_points() {
        let ccw = "My airfoil\n1.0 0.0\n0.5 0.06\n0.0 0.0\n0.5 -0.02\n1.0 0.0\n";
        let (x, y) = read_airfoil_file("a", ccw).unwrap();
        assert_eq!(x, vec![1.0, 0.5, 0.0, 0.5, 1.0]);
        assert_eq!(y[1], 0.06);
        let cw = "1.0 0.0\n0.6 -0.02\n0.0 0.0\n0.4 0.06\n1.0 0.0\n";
        let (x, y) = read_airfoil_file("b", cw).unwrap();
        assert_eq!(
            x,
            vec![1.0, 0.4, 0.0, 0.6, 1.0],
            "clockwise points are reversed"
        );
        assert_eq!(y, vec![0.0, 0.06, 0.0, -0.02, 0.0]);
    }
}
