//! The beginner's path: from a flight to the conditions of its airfoil and
//! to recommended airfoils (`aero-mission`), for the catalogue and the
//! library.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use aero_aircraft::{Aircraft, Layout, MeshOptions, MeshPart, layout, mesh};
use aero_geometry::Airfoil;
use aero_mission::{
    AirfoilEvaluation, AnalysisSettings, Conditions, Flight, Goal, Wing, WingConstraint, WingModel,
    evaluate, rank,
};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::AppError;
use crate::model::{AirfoilId, MissionSetup};

/// Airfoils to evaluate for a flight.
#[derive(Debug, Clone, PartialEq, Deserialize, Type)]
pub struct RecommendRequest {
    /// Identifier used to cancel the evaluation.
    pub job_id: String,
    /// The flight.
    pub flight: Flight,
    /// The wing.
    pub wing: Wing,
    /// Analysis of every candidate.
    pub settings: AnalysisSettings,
    /// Finite-wing estimate.
    pub model: WingModel,
    /// Catalogue airfoils to evaluate.
    pub catalogue_ids: Vec<String>,
    /// Library airfoils to evaluate.
    pub airfoil_ids: Vec<AirfoilId>,
}

/// Where a candidate comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CandidateSource {
    /// The catalogue.
    Catalogue {
        /// Catalogue identifier.
        id: String,
    },
    /// The library.
    Library {
        /// Library identifier.
        id: AirfoilId,
    },
}

/// A candidate airfoil and its evaluation.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct CandidateResult {
    /// Origin.
    pub source: CandidateSource,
    /// Display name.
    pub name: String,
    /// Outline of the shape (at most [`OUTLINE_POINTS`] points), for a
    /// thumbnail.
    pub outline_x: Vec<f64>,
    /// Ordinates of the outline.
    pub outline_y: Vec<f64>,
    /// Evaluation, when the analysis could run.
    pub evaluation: Option<AirfoilEvaluation>,
    /// Why the analysis could not run.
    pub error: Option<String>,
}

/// The candidates ordered for each goal, best first (indices into the
/// candidates; those that could not be analysed come last).
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct Rankings {
    /// Least drag at the flight's speed.
    pub least_drag: Vec<u32>,
    /// Largest maximum lift.
    pub slow_flight: Vec<u32>,
    /// Least drag among the shapes easy to build.
    pub easy_to_build: Vec<u32>,
}

/// Result of an evaluation.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct RecommendResult {
    /// Conditions of the flight.
    pub conditions: Conditions,
    /// Every candidate, in request order (catalogue, then library).
    pub candidates: Vec<CandidateResult>,
    /// Orders for each goal.
    pub rankings: Rankings,
    /// True if the evaluation was cancelled (the candidates evaluated so
    /// far are returned).
    pub cancelled: bool,
}

/// Progress of an evaluation, streamed over a channel.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RecommendEvent {
    /// The evaluation starts.
    Started {
        /// Number of candidates.
        total: u32,
    },
    /// A candidate is done.
    Progress {
        /// Candidates done so far.
        done: u32,
        /// Number of candidates.
        total: u32,
        /// Name of the candidate just done.
        name: String,
    },
    /// The evaluation ended.
    Finished {
        /// True if it was cancelled.
        cancelled: bool,
    },
}

/// A candidate to evaluate.
pub struct Candidate {
    /// Origin.
    pub source: CandidateSource,
    /// Display name.
    pub name: String,
    /// Geometry.
    pub airfoil: Airfoil,
}

/// Most points of a candidate's outline.
pub const OUTLINE_POINTS: usize = 80;

/// Every k-th point of the airfoil, keeping both trailing-edge points.
fn outline(airfoil: &Airfoil) -> (Vec<f64>, Vec<f64>) {
    let (x, y) = (airfoil.x(), airfoil.y());
    let n = x.len();
    let step = n.div_ceil(OUTLINE_POINTS - 1).max(1);
    let mut keep: Vec<usize> = (0..n).step_by(step).collect();
    if keep.last() != Some(&(n - 1)) {
        keep.push(n - 1);
    }
    (
        keep.iter().map(|&i| x[i]).collect(),
        keep.iter().map(|&i| y[i]).collect(),
    )
}

/// What the My aircraft tab shows for a setup.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct MissionView {
    /// The aircraft, its wing sized when the setup asks for it.
    pub aircraft: Aircraft,
    /// The wing as the flight sees it: its span and mean geometric chord
    /// (area ÷ span).
    pub wing: Wing,
    /// The flight's conditions on that wing.
    pub conditions: Conditions,
    /// Areas, mean aerodynamic chords, tail volumes, centre of gravity.
    pub layout: Layout,
    /// Surfaces of the aircraft for the 3D view.
    pub parts: Vec<MeshPart>,
}

/// The view of `setup`; `airfoil` resolves the airfoil keys of the
/// aircraft.
pub fn mission_view(
    setup: &MissionSetup,
    airfoil: &dyn Fn(&str) -> Option<Airfoil>,
) -> Result<MissionView, AppError> {
    let mut aircraft = setup.aircraft.clone();
    if let Some(cl) = setup.sizing_cl {
        // Same span and taper, chords scaled to the area flying at `cl`.
        let target = setup
            .flight
            .size_wing(cl, &WingConstraint::Span(aircraft.wing.span))?;
        let k = target.span * target.chord / layout(&aircraft)?.wing.area;
        aircraft.wing.root_chord *= k;
        aircraft.wing.tip_chord *= k;
    }
    let layout = layout(&aircraft)?;
    let wing = Wing {
        span: layout.wing.span,
        chord: layout.wing.area / layout.wing.span,
    };
    let conditions = setup.flight.conditions(&wing)?;
    let parts = mesh(&aircraft, airfoil, &MeshOptions::default())?;
    Ok(MissionView {
        aircraft,
        wing,
        conditions,
        layout,
        parts,
    })
}

/// Evaluates `candidates` in parallel (one per core, one core left to the
/// interface), checking `cancel` between candidates.
pub fn recommend(
    request: &RecommendRequest,
    candidates: Vec<Candidate>,
    cancel: &AtomicBool,
    emit: &(impl Fn(RecommendEvent) + Sync),
) -> Result<RecommendResult, AppError> {
    let conditions = request.flight.conditions(&request.wing)?;
    let total = u32::try_from(candidates.len()).unwrap_or(u32::MAX);
    emit(RecommendEvent::Started { total });
    let next = AtomicUsize::new(0);
    let done = AtomicU32::new(0);
    let results: Mutex<Vec<Option<CandidateResult>>> = Mutex::new(vec![None; candidates.len()]);
    let threads = std::thread::available_parallelism()
        .map_or(1, |n| n.get().saturating_sub(1).max(1))
        .min(candidates.len().max(1));
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(c) = candidates.get(i) else { break };
                    let outcome =
                        evaluate(&c.airfoil, &conditions, &request.settings, &request.model);
                    let (outline_x, outline_y) = outline(&c.airfoil);
                    let result = CandidateResult {
                        source: c.source.clone(),
                        name: c.name.clone(),
                        outline_x,
                        outline_y,
                        error: outcome.as_ref().err().map(ToString::to_string),
                        evaluation: outcome.ok(),
                    };
                    if let Some(slot) = results
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .get_mut(i)
                    {
                        *slot = Some(result);
                    }
                    let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                    emit(RecommendEvent::Progress {
                        done: n,
                        total,
                        name: c.name.clone(),
                    });
                }
            });
        }
    });
    let cancelled = cancel.load(Ordering::Relaxed);
    let candidates: Vec<CandidateResult> = results
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .into_iter()
        .flatten()
        .collect();
    let rankings = Rankings {
        least_drag: ranking(&candidates, Goal::LeastDrag),
        slow_flight: ranking(&candidates, Goal::SlowFlight),
        easy_to_build: ranking(&candidates, Goal::EasyToBuild),
    };
    Ok(RecommendResult {
        conditions,
        candidates,
        rankings,
        cancelled,
    })
}

/// Order of `candidates` for `goal`: the evaluated ones ranked, then the
/// others.
fn ranking(candidates: &[CandidateResult], goal: Goal) -> Vec<u32> {
    let evaluated: Vec<(usize, &AirfoilEvaluation)> = candidates
        .iter()
        .enumerate()
        .filter_map(|(i, c)| c.evaluation.as_ref().map(|e| (i, e)))
        .collect();
    let evaluations: Vec<AirfoilEvaluation> = evaluated.iter().map(|(_, e)| (*e).clone()).collect();
    let mut order: Vec<usize> = rank(&evaluations, goal)
        .into_iter()
        .map(|k| evaluated[k].0)
        .collect();
    order.extend(
        candidates
            .iter()
            .enumerate()
            .filter(|(_, c)| c.evaluation.is_none())
            .map(|(i, _)| i),
    );
    order
        .into_iter()
        .map(|i| u32::try_from(i).unwrap_or(u32::MAX))
        .collect()
}

#[cfg(test)]
mod tests {
    use aero_geometry::{NacaOptions, naca};

    use super::*;

    #[test]
    fn outlines_are_small_and_closed() {
        let airfoil = naca("2412", &NacaOptions::default()).unwrap();
        assert!(airfoil.len() > OUTLINE_POINTS);
        let (x, y) = outline(&airfoil);
        assert!(x.len() <= OUTLINE_POINTS && x.len() > OUTLINE_POINTS / 2);
        assert_eq!(x.len(), y.len());
        assert_eq!(x[0], airfoil.x()[0]);
        assert_eq!(*x.last().unwrap(), *airfoil.x().last().unwrap());
    }
}
