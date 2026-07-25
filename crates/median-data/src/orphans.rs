use std::collections::BTreeMap;

use studio::Unresolved;

/// How often one source printed a name nothing answers to, and where it was printed.
#[derive(Debug, Default, Clone)]
pub struct Seen {
    pub count: usize,
    pub hint: String,
}

/// Names one source prints that no catalog item answers to.
pub type Missing = BTreeMap<String, Seen>;

/// Record a printed name nothing answers to. The first place it was seen is kept as the hint,
/// which is enough to judge it: what matters afterwards is how many rows hang on it.
pub fn note(missing: &mut Missing, printed: &str, hint: impl FnOnce() -> String) {
    let seen = missing.entry(printed.to_string()).or_default();
    seen.count += 1;
    if seen.hint.is_empty() {
        seen.hint = hint();
    }
}

/// One source's orphans as rows for the mapping screen, the ones holding the most rows first.
pub fn rows(source: &str, missing: &Missing) -> Vec<Unresolved> {
    let mut out: Vec<Unresolved> = missing
        .iter()
        .map(|(name, seen)| Unresolved {
            source: source.to_string(),
            key: name.clone(),
            name: name.clone(),
            hint: seen.hint.clone(),
            count: seen.count,
        })
        .collect();
    out.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_sighting_is_the_hint_and_the_rest_only_count() {
        let mut missing = Missing::new();
        note(&mut missing, "Endo", || "Earth/Mariana".to_string());
        note(&mut missing, "Endo", || "Venus/Kiliken".to_string());
        assert_eq!(missing["Endo"].count, 2);
        assert_eq!(missing["Endo"].hint, "Earth/Mariana");
    }

    #[test]
    fn the_heaviest_orphan_comes_first() {
        let mut missing = Missing::new();
        note(&mut missing, "Once", String::new);
        note(&mut missing, "Twice", String::new);
        note(&mut missing, "Twice", String::new);
        let rows = rows("drops", &missing);
        assert_eq!(rows[0].name, "Twice");
        assert_eq!(rows[0].source, "drops");
    }
}
