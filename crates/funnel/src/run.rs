use std::collections::BTreeSet;

use graph::Graph;

use crate::anchor::Anchors;
use crate::coverage::Gaps;
use crate::cross::{DropClaim, RelicClaim};
use crate::diff::{self, State};
use crate::report::{Report, Totals};
use crate::{anchor, coverage, cross, invariant, outlier};

/// Everything the funnel needs beyond the graph itself.
pub struct Input<'a> {
    pub totals: Totals,
    /// What the build could not attach.
    pub gaps: Gaps,
    /// An independent account of relic rewards, to check DE against.
    pub relic_witness: Vec<RelicClaim>,
    /// An independent account of the mission drop tables, to check the official ones against.
    pub drop_witness: Vec<DropClaim>,
    pub anchors: &'a Anchors,
    /// Findings a person looked at and let through, by check and entity.
    pub accepted: BTreeSet<(String, String)>,
    /// The previous build, when there is one.
    pub previous: Option<State>,
}

/// Run every filter over the built graph. A finding somebody accepted is moved aside rather
/// than dropped: it stays counted and readable, and stops crowding the list.
pub fn run(graph: &Graph, input: Input<'_>) -> (Report, State) {
    let mut all = invariant::check(graph);
    all.extend(cross::relic_rewards(graph, &input.relic_witness));
    all.extend(cross::drops(graph, &input.drop_witness));
    all.extend(cross::set_composition(graph));
    all.extend(outlier::check(graph));
    all.extend(coverage::check(graph, &input.gaps));
    all.extend(anchor::check(graph, input.anchors));

    let (accepted, findings): (Vec<_>, Vec<_>) = all.into_iter().partition(|f| {
        input
            .accepted
            .contains(&(f.rule.clone(), f.entity.clone()))
    });

    let state = diff::snapshot(graph, &findings);
    let changed = input.previous.map(|before| diff::compare(&before, &state));

    let report = Report {
        totals: input.totals,
        findings,
        accepted,
        diff: changed,
    };
    (report, state)
}
