//! Ordering of candidate solutions.

use std::cmp::Ordering;

/// Values compared by the optimizers: the smaller ranks first.
///
/// The optimizers only compare values, they never combine them, so a value
/// can carry constraint violations next to the objective (see
/// [`crate::Evaluation`]).
pub trait Ranked {
    /// Compares two values; [`Ordering::Less`] means `self` is better.
    fn rank_cmp(&self, other: &Self) -> Ordering;
}

impl Ranked for f64 {
    /// Total order with NaN last.
    fn rank_cmp(&self, other: &Self) -> Ordering {
        match (self.is_nan(), other.is_nan()) {
            (false, false) => self.total_cmp(other),
            (a, b) => a.cmp(&b),
        }
    }
}

/// Feasibility rules (Deb, 2000): a feasible value beats an infeasible one,
/// two feasible values compare by objective, two infeasible ones by
/// violation.
pub(crate) fn feasibility_cmp(
    violation: f64,
    objective: Option<f64>,
    other_violation: f64,
    other_objective: Option<f64>,
) -> Ordering {
    match (violation > 0.0, other_violation > 0.0) {
        (false, false) => objective
            .unwrap_or(f64::NAN)
            .rank_cmp(&other_objective.unwrap_or(f64::NAN)),
        (false, true) => Ordering::Less,
        (true, false) => Ordering::Greater,
        (true, true) => violation.rank_cmp(&other_violation),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nan_ranks_last() {
        let mut v = [3.0, f64::NAN, -1.0, f64::INFINITY];
        v.sort_by(Ranked::rank_cmp);
        assert_eq!(v[..3], [-1.0, 3.0, f64::INFINITY]);
        assert!(v[3].is_nan());
    }

    #[test]
    fn feasibility_rules() {
        use Ordering::*;
        // Feasible: by objective.
        assert_eq!(feasibility_cmp(0.0, Some(1.0), 0.0, Some(2.0)), Less);
        // Feasible beats infeasible, whatever the objectives.
        assert_eq!(feasibility_cmp(0.0, Some(9.0), 0.1, Some(1.0)), Less);
        assert_eq!(feasibility_cmp(0.1, Some(1.0), 0.0, None), Greater);
        // Infeasible: by violation.
        assert_eq!(feasibility_cmp(0.2, Some(1.0), 0.1, Some(9.0)), Greater);
        assert_eq!(feasibility_cmp(0.1, None, 0.1, Some(1.0)), Equal);
    }
}
