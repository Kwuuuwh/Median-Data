use std::collections::BTreeMap;
use std::fmt::Write;

use serde::{Deserialize, Serialize};

use crate::diff::Diff;
use crate::finding::{Finding, Layer};

/// Sizes of the build the funnel judged.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Totals {
    pub items: usize,
    pub places: usize,
    pub edges: usize,
    /// Property-level disagreements recorded while merging sources.
    pub conflicts: usize,
}

/// The verdict on one build: what it holds, what needs looking at, what moved.
#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    pub totals: Totals,
    pub findings: Vec<Finding>,
    /// What a person looked at and let through, so it is counted without being asked again.
    #[serde(default)]
    pub accepted: Vec<Finding>,
    pub diff: Option<Diff>,
}

impl Report {
    /// How many findings each layer produced.
    pub fn by_layer(&self) -> BTreeMap<&'static str, usize> {
        let mut counts = BTreeMap::new();
        for f in &self.findings {
            *counts.entry(f.layer.as_str()).or_default() += 1;
        }
        counts
    }

    /// How many findings each rule produced.
    pub fn by_rule(&self) -> BTreeMap<&str, usize> {
        let mut counts = BTreeMap::new();
        for f in &self.findings {
            *counts.entry(f.rule.as_str()).or_default() += 1;
        }
        counts
    }

    /// Findings a human still has to judge — everything that is not a hard failure.
    pub fn queue(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.layer != Layer::Invariant)
            .count()
    }

    /// A summary for the console.
    pub fn render(&self) -> String {
        let mut out = String::new();
        let t = &self.totals;
        let _ = writeln!(
            out,
            "catalog  {} items, {} places, {} edges, {} conflicts",
            t.items, t.places, t.edges, t.conflicts
        );

        if !self.accepted.is_empty() {
            let mut by_rule: BTreeMap<&str, usize> = BTreeMap::new();
            for f in &self.accepted {
                *by_rule.entry(f.rule.as_str()).or_default() += 1;
            }
            let summary: Vec<String> = by_rule.iter().map(|(r, c)| format!("{r} {c}")).collect();
            let _ = writeln!(
                out,
                "accepted {} findings let through on purpose ({})",
                self.accepted.len(),
                summary.join(", ")
            );
        }

        let layers = self.by_layer();
        if layers.is_empty() {
            let _ = writeln!(out, "funnel   nothing flagged");
        } else {
            let summary: Vec<String> = layers.iter().map(|(l, c)| format!("{l} {c}")).collect();
            let _ = writeln!(out, "funnel   {}", summary.join(", "));
            for (rule, count) in self.by_rule() {
                let _ = writeln!(out, "           {count:>6}  {rule}");
            }
            let _ = writeln!(out, "queue    {} to review", self.queue());
        }

        match &self.diff {
            None => {
                let _ = writeln!(out, "diff     no previous build to compare with");
            }
            Some(d) if d.is_empty() => {
                let _ = writeln!(out, "diff     unchanged since the last build");
            }
            Some(d) => {
                let _ = writeln!(
                    out,
                    "diff     items +{} -{}, sets +{} -{}",
                    d.items_added.len(),
                    d.items_removed.len(),
                    d.sets_added.len(),
                    d.sets_removed.len()
                );
                for (rule, delta) in &d.findings_delta {
                    let _ = writeln!(out, "           {delta:>+6}  {rule}");
                }
            }
        }
        out
    }
}
