use crate::source::Source;

/// A disagreement between sources on one property, kept for review with every claim intact
/// so a person can judge it side by side.
#[derive(Debug, Clone)]
pub struct Conflict {
    pub entity: String,
    pub prop: &'static str,
    /// What each source said, printed.
    pub claims: Vec<(Source, String)>,
    /// The value the build kept.
    pub chosen: String,
}

impl Conflict {
    pub fn new(
        entity: impl Into<String>,
        prop: &'static str,
        claims: Vec<(Source, String)>,
        chosen: String,
    ) -> Self {
        Self {
            entity: entity.into(),
            prop,
            claims,
            chosen,
        }
    }

    /// The claims on one line, for artifacts that carry text rather than rows.
    pub fn detail(&self) -> String {
        self.claims
            .iter()
            .map(|(s, v)| format!("{}={v}", s.as_str()))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Every claim behind a resolved value, printed. Sources that said nothing are left out.
pub fn claims<T: std::fmt::Display>(pairs: &[(Source, Option<T>)]) -> Vec<(Source, String)> {
    pairs
        .iter()
        .filter_map(|(s, v)| v.as_ref().map(|v| (*s, v.to_string())))
        .collect()
}
