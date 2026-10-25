//! Reading and writing airfoil coordinate files (`.dat`).
//!
//! Supported input layouts, detected automatically:
//! - **Selig**: optional name line, then `x y` pairs from the trailing edge
//!   over the upper surface to the leading edge and back along the lower
//!   surface (also XFOIL's "labeled" and "plain" formats);
//! - **Lednicer**: name line, a line with the number of upper and lower
//!   points, then the upper surface and the lower surface, both from the
//!   leading edge to the trailing edge;
//! - **leading-edge first**: two blocks from the leading edge to the trailing
//!   edge without a count line, as found in some databases;
//! - MSES/ISES files (a domain line with four numbers after the name): the
//!   domain line is skipped and only the first element is read.
//!
//! Parsing is tolerant of blank lines, `#`/`!` comment lines, tabs, commas,
//! semicolons, Windows line endings, a byte-order mark and Fortran `D`
//! exponents.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::airfoil::{Airfoil, dedup_points};
use crate::error::{GeometryError, Result};

/// Layout of an airfoil coordinate file.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatFormat {
    /// Selig (XFOIL) ordering.
    Selig,
    /// Lednicer ordering with a point-count line.
    Lednicer,
    /// Two leading-edge-first blocks without a count line.
    LeadingEdgeFirst,
    /// MSES/ISES file (domain line skipped).
    Mses,
}

/// Non-fatal observation made while importing a file.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImportWarning {
    /// The file had no name line; a fallback name was used.
    MissingName,
    /// Points were ordered clockwise and have been reversed.
    ReversedOrdering,
    /// Repeated successive points were removed.
    DuplicatePointsRemoved {
        /// Number of removed points.
        count: usize,
    },
    /// Lines that are neither comments nor coordinate pairs were skipped.
    IgnoredLines {
        /// 1-based line numbers of the skipped lines.
        lines: Vec<usize>,
    },
    /// The file holds several elements; only the first one was read.
    ExtraElementsIgnored,
}

impl fmt::Display for ImportWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingName => write!(f, "the file has no name line"),
            Self::ReversedOrdering => {
                write!(f, "points were ordered clockwise and have been reversed")
            }
            Self::DuplicatePointsRemoved { count } => {
                write!(f, "{count} duplicated point(s) removed")
            }
            Self::IgnoredLines { lines } => {
                let list: Vec<String> = lines.iter().map(ToString::to_string).collect();
                write!(f, "unreadable line(s) skipped: {}", list.join(", "))
            }
            Self::ExtraElementsIgnored => {
                write!(f, "multi-element file: only the first element was read")
            }
        }
    }
}

/// Outcome of an import: detected format and warnings.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportReport {
    /// Detected file layout.
    pub format: DatFormat,
    /// Non-fatal observations.
    pub warnings: Vec<ImportWarning>,
}

/// One coordinate line of the input.
#[derive(Debug, Clone, Copy)]
struct Row {
    line: usize,
    values: [f64; 2],
    count: usize,
}

/// Parses an airfoil coordinate file. `fallback_name` is used when the file
/// has no name line (for instance the file stem).
pub fn parse_dat(text: &str, fallback_name: &str) -> Result<(Airfoil, ImportReport)> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut warnings = Vec::new();
    let mut name: Option<String> = None;
    let mut rows: Vec<Row> = Vec::new();
    let mut ignored = Vec::new();

    for (index, raw) in text.lines().enumerate() {
        let line_no = index + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
            continue;
        }
        match parse_numbers(line) {
            Some((values, count)) => rows.push(Row {
                line: line_no,
                values,
                count,
            }),
            None if name.is_none() && rows.is_empty() => name = Some(line.to_string()),
            None => ignored.push(line_no),
        }
    }

    if rows.is_empty() {
        return Err(GeometryError::Parse {
            line: text.lines().count().max(1),
            message: "no coordinate lines (pairs of numbers) were found".into(),
        });
    }

    let mut format = DatFormat::Selig;

    // MSES/ISES domain line: four or more numbers right after the name.
    if name.is_some() && rows.first().is_some_and(|r| r.count >= 4) {
        rows.remove(0);
        format = DatFormat::Mses;
    }

    // Multi-element MSES files separate elements by "999 999".
    if let Some(pos) = rows.iter().position(|r| r.values == [999.0, 999.0]) {
        if pos + 1 < rows.len() {
            warnings.push(ImportWarning::ExtraElementsIgnored);
        }
        rows.truncate(pos);
    }

    let (x, y) = if let Some((nu, nl)) = lednicer_counts(&rows) {
        format = DatFormat::Lednicer;
        let points = &rows[1..];
        if points.len() != nu + nl {
            return Err(GeometryError::Parse {
                line: rows[0].line,
                message: format!(
                    "Lednicer header announces {nu} + {nl} points but the file contains {}",
                    points.len()
                ),
            });
        }
        let (upper, lower) = points.split_at(nu);
        merge_le_first(upper, lower)
    } else if let Some(split) = le_first_split(&rows) {
        format = DatFormat::LeadingEdgeFirst;
        let (first, second) = rows.split_at(split);
        // The block with the higher mean ordinate is the upper surface.
        let mean = |b: &[Row]| b.iter().map(|r| r.values[1]).sum::<f64>() / b.len() as f64;
        if mean(first) >= mean(second) {
            merge_le_first(first, second)
        } else {
            merge_le_first(second, first)
        }
    } else {
        rows.iter().map(|r| (r.values[0], r.values[1])).unzip()
    };

    if !ignored.is_empty() {
        warnings.push(ImportWarning::IgnoredLines { lines: ignored });
    }

    let name = match name {
        Some(n) => n,
        None => {
            warnings.push(ImportWarning::MissingName);
            fallback_name.to_string()
        }
    };

    let before = x.len();
    let (x, y) = dedup_points(&x, &y);
    if x.len() != before {
        warnings.push(ImportWarning::DuplicatePointsRemoved {
            count: before - x.len(),
        });
    }

    let (airfoil, reversed) = Airfoil::new_reoriented(name, x, y)?;
    if reversed {
        warnings.push(ImportWarning::ReversedOrdering);
    }
    Ok((airfoil, ImportReport { format, warnings }))
}

/// Formats an airfoil in Selig format: the name, then one `x y` pair per line.
pub fn to_selig_string(airfoil: &Airfoil) -> String {
    let mut out = String::with_capacity(32 * (airfoil.len() + 1));
    out.push_str(airfoil.name());
    out.push('\n');
    for [x, y] in airfoil.points() {
        out.push_str(&format!("{x:>12.8} {y:>12.8}\n"));
    }
    out
}

/// Parses the first two numbers of a line. Returns them with the count of
/// numbers on the line, or `None` if the line does not start with two numbers.
fn parse_numbers(line: &str) -> Option<([f64; 2], usize)> {
    let tokens: Vec<&str> = line
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .filter(|t| !t.is_empty())
        .collect();
    let mut numbers = Vec::with_capacity(tokens.len());
    for token in &tokens {
        match parse_float(token) {
            Some(v) => numbers.push(v),
            None => break,
        }
    }
    if numbers.len() < 2 {
        return None;
    }
    Some(([numbers[0], numbers[1]], numbers.len()))
}

fn parse_float(token: &str) -> Option<f64> {
    let v = token
        .parse::<f64>()
        .ok()
        .or_else(|| token.replace(['D', 'd'], "E").parse::<f64>().ok())?;
    v.is_finite().then_some(v)
}

/// Detects a Lednicer count line: two integral values of at least 2 on the
/// first coordinate row. The caller checks that the counts match the file.
fn lednicer_counts(rows: &[Row]) -> Option<(usize, usize)> {
    let first = rows.first()?;
    let [a, b] = first.values;
    let integral = |v: f64| v >= 2.0 && v.fract() == 0.0 && v < 1.0e6;
    if !(integral(a) && integral(b)) {
        return None;
    }
    // Coordinates are almost never integers >= 2 on both axes, but require
    // the next row to look like a leading-edge point to be safe.
    let next = rows.get(1)?;
    if next.values[0].abs() > 0.5 * a.max(b) {
        return None;
    }
    Some((a as usize, b as usize))
}

/// Detects two leading-edge-first blocks: x increases from a minimum, then
/// jumps back to (nearly) the same minimum and increases again.
fn le_first_split(rows: &[Row]) -> Option<usize> {
    if rows.len() < 6 {
        return None;
    }
    let x: Vec<f64> = rows.iter().map(|r| r.values[0]).collect();
    let xmin = x.iter().copied().fold(f64::INFINITY, f64::min);
    let xmax = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let chord = xmax - xmin;
    if chord <= 0.0 || (x[0] - xmin) > 1e-3 * chord {
        return None;
    }
    let split = (1..x.len()).find(|&i| x[i] < x[i - 1] - 0.5 * chord)?;
    let increasing = |s: &[f64]| s.windows(2).all(|w| w[1] >= w[0]);
    if (x[split] - xmin) <= 1e-3 * chord && increasing(&x[..split]) && increasing(&x[split..]) {
        Some(split)
    } else {
        None
    }
}

/// Builds Selig ordering from upper and lower surfaces given from the
/// leading edge to the trailing edge, merging a shared leading-edge point.
fn merge_le_first(upper: &[Row], lower: &[Row]) -> (Vec<f64>, Vec<f64>) {
    let mut x = Vec::with_capacity(upper.len() + lower.len());
    let mut y = Vec::with_capacity(upper.len() + lower.len());
    for r in upper.iter().rev() {
        x.push(r.values[0]);
        y.push(r.values[1]);
    }
    let skip = usize::from(
        matches!((upper.first(), lower.first()), (Some(u), Some(l)) if u.values == l.values),
    );
    for r in &lower[skip..] {
        x.push(r.values[0]);
        y.push(r.values[1]);
    }
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SELIG: &str = "NACA 0012 test
  1.000000  0.001260
  0.750000  0.040000
  0.300000  0.060000
  0.050000  0.035000
  0.000000  0.000000
  0.050000 -0.035000
  0.300000 -0.060000
  0.750000 -0.040000
  1.000000 -0.001260
";

    const LEDNICER: &str = "NACA 0012 test
       5.       5.

  0.000000  0.000000
  0.050000  0.035000
  0.300000  0.060000
  0.750000  0.040000
  1.000000  0.001260

  0.000000  0.000000
  0.050000 -0.035000
  0.300000 -0.060000
  0.750000 -0.040000
  1.000000 -0.001260
";

    #[test]
    fn reads_selig() {
        let (af, report) = parse_dat(SELIG, "fallback").unwrap();
        assert_eq!(report.format, DatFormat::Selig);
        assert!(report.warnings.is_empty());
        assert_eq!(af.name(), "NACA 0012 test");
        assert_eq!(af.len(), 9);
        assert_eq!(af.x()[0], 1.0);
        assert_eq!(af.y()[8], -0.00126);
    }

    #[test]
    fn reads_lednicer_like_selig() {
        let (selig, _) = parse_dat(SELIG, "s").unwrap();
        let (led, report) = parse_dat(LEDNICER, "l").unwrap();
        assert_eq!(report.format, DatFormat::Lednicer);
        assert!(report.warnings.is_empty());
        assert_eq!(selig.x(), led.x());
        assert_eq!(selig.y(), led.y());
    }

    #[test]
    fn lednicer_count_mismatch_is_an_error() {
        let bad = LEDNICER.replace("       5.       5.", "       6.       5.");
        let err = parse_dat(&bad, "l").unwrap_err();
        assert!(
            matches!(err, GeometryError::Parse { line: 2, .. }),
            "{err:?}"
        );
    }

    #[test]
    fn reads_leading_edge_first_blocks() {
        let text: String = LEDNICER
            .lines()
            .filter(|l| !l.contains("5.       5."))
            .map(|l| format!("{l}\n"))
            .collect();
        let (af, report) = parse_dat(&text, "x").unwrap();
        assert_eq!(report.format, DatFormat::LeadingEdgeFirst);
        let (selig, _) = parse_dat(SELIG, "s").unwrap();
        assert_eq!(af.x(), selig.x());
        assert_eq!(af.y(), selig.y());
    }

    #[test]
    fn tolerates_messy_input() {
        let messy = "\u{feff}# comment\r\n  my foil  \r\n\r\n1.0,0.00126\r\n0.75\t0.04\r\n0.3;0.06\r\n0.05 0.035 extra text\r\n0.0D0 0.0d0\r\n  \r\n0.05 -0.035\r\n0.3 -0.06\r\n! another comment\r\n0.75 -0.04\r\n1.0 -0.00126\r\n";
        let (af, report) = parse_dat(messy, "f").unwrap();
        assert_eq!(af.name(), "my foil");
        assert_eq!(af.len(), 9);
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    }

    #[test]
    fn missing_name_uses_fallback() {
        let plain: String = SELIG.lines().skip(1).map(|l| format!("{l}\n")).collect();
        let (af, report) = parse_dat(&plain, "fallback").unwrap();
        assert_eq!(af.name(), "fallback");
        assert_eq!(report.warnings, vec![ImportWarning::MissingName]);
    }

    #[test]
    fn clockwise_files_are_reversed() {
        let mut lines: Vec<&str> = SELIG.lines().collect();
        lines[1..].reverse();
        let text = lines.join("\n");
        let (af, report) = parse_dat(&text, "cw").unwrap();
        assert!(report.warnings.contains(&ImportWarning::ReversedOrdering));
        assert_eq!(af.y()[0], 0.00126);
    }

    #[test]
    fn duplicates_and_garbage_are_reported() {
        let text = SELIG.replace(
            "  0.000000  0.000000\n",
            "  0.000000  0.000000\n  0.000000  0.000000\nnot a number\n",
        );
        let (af, report) = parse_dat(&text, "d").unwrap();
        assert_eq!(af.len(), 9);
        assert!(
            report
                .warnings
                .contains(&ImportWarning::DuplicatePointsRemoved { count: 1 })
        );
        assert!(
            report
                .warnings
                .contains(&ImportWarning::IgnoredLines { lines: vec![8] })
        );
        for w in &report.warnings {
            assert!(!w.to_string().is_empty());
        }
    }

    #[test]
    fn mses_domain_line_and_elements() {
        let mut text = String::from("foil\n -2.0 3.0 -2.5 3.5\n");
        text.push_str(
            &SELIG
                .lines()
                .skip(1)
                .map(|l| format!("{l}\n"))
                .collect::<String>(),
        );
        text.push_str("999.0 999.0\n0.0 0.0\n1.0 0.0\n");
        let (af, report) = parse_dat(&text, "m").unwrap();
        assert_eq!(report.format, DatFormat::Mses);
        assert_eq!(af.len(), 9);
        assert_eq!(report.warnings, vec![ImportWarning::ExtraElementsIgnored]);
    }

    #[test]
    fn invalid_files_are_errors() {
        assert!(parse_dat("", "e").is_err());
        let err = parse_dat(
            "just text
more text
",
            "e",
        )
        .unwrap_err();
        assert!(err.to_string().contains("no coordinate lines"), "{err}");
        assert!(parse_dat("name only\n", "e").is_err());
        assert!(parse_dat("foil\n0 0\n1 0\n", "e").is_err());
    }

    #[test]
    fn selig_round_trip() {
        let (af, _) = parse_dat(SELIG, "s").unwrap();
        let text = to_selig_string(&af);
        let (back, report) = parse_dat(&text, "b").unwrap();
        assert_eq!(report.format, DatFormat::Selig);
        assert_eq!(back.name(), af.name());
        for (a, b) in af.points().zip(back.points()) {
            assert!((a[0] - b[0]).abs() < 1e-8 && (a[1] - b[1]).abs() < 1e-8);
        }
    }

    #[test]
    fn uiuc_clark_y_lednicer_matches_selig_conversion() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/reference/clarky/");
        let led = std::fs::read_to_string(format!("{root}clarky_uiuc_lednicer.dat")).unwrap();
        let sel = std::fs::read_to_string(format!("{root}clarky.dat")).unwrap();
        let (a, ra) = parse_dat(&led, "a").unwrap();
        let (b, rb) = parse_dat(&sel, "b").unwrap();
        assert_eq!(ra.format, DatFormat::Lednicer);
        assert_eq!(rb.format, DatFormat::Selig);
        assert_eq!(a.len(), 121);
        assert_eq!(a.x(), b.x());
        assert_eq!(a.y(), b.y());
        assert_eq!(a.name(), "CLARK Y AIRFOIL");
    }
}
