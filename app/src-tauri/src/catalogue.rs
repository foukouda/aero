//! Catalogue of well-known airfoils sorted by use, for the beginner's path
//! ("which airfoil for my aircraft?"). The list and its descriptions are
//! `data/catalogue/catalogue.json`; coordinates come from the UIUC Airfoil
//! Coordinates Database (see NOTICE) or from the NACA generator.

use aero_geometry::{Airfoil, NacaOptions, naca, parse_dat};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::{AppError, ErrorKind};

/// What an airfoil of the catalogue is good for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum CatalogueCategory {
    /// High lift for slow flight and heavy loads.
    SlowFlight,
    /// Low drag for sailplanes and gliders.
    Glider,
    /// Forgiving, easy-to-build sections for trainers and sport models.
    Trainer,
    /// Symmetric sections: aerobatics, tails.
    Aerobatic,
    /// Reflexed sections, stable without a tail.
    FlyingWing,
}

/// Where the coordinates of a catalogue airfoil come from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum CatalogueSource {
    /// A coordinate file of `data/catalogue`.
    File(String),
    /// A NACA designation, generated.
    Naca(String),
}

/// A text in the languages of the interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct LocalizedText {
    /// English.
    pub en: String,
    /// French.
    pub fr: String,
}

/// An airfoil of the catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct CatalogueEntry {
    /// Stable identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Main use.
    pub category: CatalogueCategory,
    /// Designer and year, when known.
    pub designer: String,
    /// Origin of the coordinates.
    pub source: CatalogueSource,
    /// What it is good for, in a sentence or two.
    pub description: LocalizedText,
}

/// The coordinate files, embedded in the executable.
const FILES: [(&str, &str); 14] = [
    ("ag35.dat", include_str!("../data/catalogue/ag35.dat")),
    ("clarky.dat", include_str!("../data/catalogue/clarky.dat")),
    ("e387.dat", include_str!("../data/catalogue/e387.dat")),
    ("e423.dat", include_str!("../data/catalogue/e423.dat")),
    ("fx63137.dat", include_str!("../data/catalogue/fx63137.dat")),
    ("mh32.dat", include_str!("../data/catalogue/mh32.dat")),
    ("mh45.dat", include_str!("../data/catalogue/mh45.dat")),
    ("mh60.dat", include_str!("../data/catalogue/mh60.dat")),
    ("rg15.dat", include_str!("../data/catalogue/rg15.dat")),
    ("s1223.dat", include_str!("../data/catalogue/s1223.dat")),
    ("s5010.dat", include_str!("../data/catalogue/s5010.dat")),
    ("sd7003.dat", include_str!("../data/catalogue/sd7003.dat")),
    ("sd7037.dat", include_str!("../data/catalogue/sd7037.dat")),
    ("sd7062.dat", include_str!("../data/catalogue/sd7062.dat")),
];

/// Text of an embedded coordinate file.
pub fn file_text(file: &str) -> Option<&'static str> {
    FILES
        .iter()
        .find(|(name, _)| *name == file)
        .map(|(_, text)| *text)
}

/// The catalogue, in display order.
pub fn entries() -> Result<Vec<CatalogueEntry>, AppError> {
    serde_json::from_str(include_str!("../data/catalogue/catalogue.json")).map_err(|e| {
        AppError::new(
            ErrorKind::Internal,
            format!("invalid airfoil catalogue: {e}"),
        )
    })
}

/// The catalogue entry `id`.
pub fn entry(id: &str) -> Result<CatalogueEntry, AppError> {
    entries()?.into_iter().find(|e| e.id == id).ok_or_else(|| {
        AppError::new(
            ErrorKind::NotFound,
            format!("no airfoil {id} in the catalogue"),
        )
    })
}

impl CatalogueEntry {
    /// The airfoil's geometry, named after the entry.
    pub fn airfoil(&self) -> Result<Airfoil, AppError> {
        let airfoil = match &self.source {
            CatalogueSource::Naca(designation) => naca(designation, &NacaOptions::default())?,
            CatalogueSource::File(file) => {
                let text = file_text(file).ok_or_else(|| {
                    AppError::new(
                        ErrorKind::Internal,
                        format!("catalogue file {file} is missing"),
                    )
                })?;
                parse_dat(text, &self.id)?.0
            }
        };
        Ok(airfoil.with_name(&self.name))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use aero_foil2d::inviscid::{InviscidOptions, InviscidSolver, OperatingPoint};
    use aero_geometry::PanelingOptions;

    use super::*;

    #[test]
    fn every_entry_loads() {
        let entries = entries().unwrap();
        assert_eq!(entries.len(), 20);
        let ids: HashSet<_> = entries.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids.len(), entries.len(), "duplicate ids");
        let mut used_files = HashSet::new();
        for e in &entries {
            let airfoil = e.airfoil().unwrap();
            assert_eq!(airfoil.name(), e.name);
            assert!(
                airfoil.paneled(&PanelingOptions::default()).is_ok(),
                "{}",
                e.id
            );
            assert!(
                !e.description.en.is_empty() && !e.description.fr.is_empty(),
                "{}",
                e.id
            );
            assert!(!e.designer.is_empty(), "{}", e.id);
            if let CatalogueSource::File(f) = &e.source {
                used_files.insert(f.as_str());
            }
        }
        // Every embedded file belongs to an entry.
        assert_eq!(used_files.len(), FILES.len());
        for category in [
            CatalogueCategory::SlowFlight,
            CatalogueCategory::Glider,
            CatalogueCategory::Trainer,
            CatalogueCategory::Aerobatic,
            CatalogueCategory::FlyingWing,
        ] {
            assert!(
                entries.iter().filter(|e| e.category == category).count() >= 3,
                "{category:?}"
            );
        }
        assert_eq!(entry("s1223").unwrap().name, "Selig S1223");
        assert_eq!(entry("nope").unwrap_err().kind, ErrorKind::NotFound);
    }

    /// Inviscid pitching moment about the quarter chord at α = 2°.
    fn cm(entry: &CatalogueEntry) -> f64 {
        let paneled = entry
            .airfoil()
            .unwrap()
            .paneled(&PanelingOptions::default())
            .unwrap();
        let solver = InviscidSolver::new(&paneled, &InviscidOptions::default()).unwrap();
        solver
            .solve(OperatingPoint::incompressible(2.0))
            .unwrap()
            .cm
    }

    #[test]
    fn descriptions_match_the_shapes() {
        let entries = entries().unwrap();
        let get = |id: &str| entries.iter().find(|e| e.id == id).unwrap();
        let props = |id: &str| get(id).airfoil().unwrap().properties();

        // Symmetric sections have no camber.
        for e in entries
            .iter()
            .filter(|e| e.category == CatalogueCategory::Aerobatic)
        {
            assert!(props(&e.id).max_camber.abs() < 1e-3, "{}", e.id);
        }
        // Gliders are thin, the trainer SD7062 thick, the high-lift
        // sections strongly cambered.
        for e in entries
            .iter()
            .filter(|e| e.category == CatalogueCategory::Glider)
        {
            assert!(props(&e.id).max_thickness < 0.10, "{}", e.id);
        }
        assert!(props("sd7062").max_thickness > 0.13);
        for e in entries
            .iter()
            .filter(|e| e.category == CatalogueCategory::SlowFlight)
        {
            assert!(props(&e.id).max_camber > 0.04, "{}", e.id);
        }
        // Reflexed flying-wing sections have a pitching moment close to
        // zero, unlike the cambered sections of the other categories.
        for e in &entries {
            let cm = cm(e);
            match e.category {
                CatalogueCategory::FlyingWing => assert!(cm > -0.02, "{} Cm {cm}", e.id),
                CatalogueCategory::SlowFlight | CatalogueCategory::Trainer => {
                    assert!(cm < -0.05, "{} Cm {cm}", e.id)
                }
                _ => {}
            }
        }
    }
}
