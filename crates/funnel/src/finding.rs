use serde::{Deserialize, Serialize};

/// Which filter of the funnel produced a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    /// A structural rule the data must never break.
    Invariant,
    /// Two sources describing the same fact differently.
    Cross,
    /// A value unlike its siblings in the same category.
    Outlier,
    /// Something a source enumerates that the catalog does not hold.
    Coverage,
    /// A hand-curated fact the build failed to reproduce.
    Anchor,
}

impl Layer {
    pub fn as_str(self) -> &'static str {
        match self {
            Layer::Invariant => "invariant",
            Layer::Cross => "cross",
            Layer::Outlier => "outlier",
            Layer::Coverage => "coverage",
            Layer::Anchor => "anchor",
        }
    }
}

/// One thing worth a human's attention.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub layer: Layer,
    /// The check that fired, e.g. `tradable-needs-slug`.
    pub rule: String,
    /// What the finding is about: a catalog path, a set slug, a place.
    pub entity: String,
    pub detail: String,
    /// The other entities the detail names, so a screen can show them by name instead of
    /// leaving a path in the middle of a sentence.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub about: Vec<String>,
}

impl Finding {
    pub fn new(layer: Layer, rule: &str, entity: impl Into<String>, detail: String) -> Self {
        Self {
            layer,
            rule: rule.to_string(),
            entity: entity.into(),
            detail,
            about: Vec::new(),
        }
    }

    /// Name the entities the detail talks about.
    pub fn about<I, S>(mut self, ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.about = ids.into_iter().map(Into::into).collect();
        self
    }
}

/// Invariants gate the build; the other layers only report.
pub fn blocking(findings: &[Finding]) -> Vec<&Finding> {
    findings
        .iter()
        .filter(|f| f.layer == Layer::Invariant)
        .collect()
}
