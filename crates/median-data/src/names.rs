use std::collections::{BTreeMap, BTreeSet};

use graph::{Extra, Graph};

/// Reverse index from a printed English name to a catalog path. Sources that name items
/// by their display name (drop tables) resolve through it.
pub struct Index {
    by_name: BTreeMap<String, String>,
    by_relic: BTreeMap<(String, String), Vec<String>>,
    ambiguous: BTreeSet<String>,
    /// Names tied to an item by hand. They answer before any derived match, so a decision
    /// made once resolves the name for every source that prints it.
    curated: BTreeMap<String, String>,
}

/// One item competing for a printed name.
struct Candidate {
    path: String,
    refinement: Option<String>,
    base: Option<String>,
}

impl Index {
    /// Build the index over the graph's items. Where a name is shared, the lexicographically
    /// smallest path wins, so a build never depends on insertion order. Names tied by hand are
    /// kept apart and answer first.
    pub fn build(graph: &Graph, curated: &BTreeMap<&str, &str>) -> Self {
        let mut grouped: BTreeMap<String, Vec<Candidate>> = BTreeMap::new();
        let mut by_relic = BTreeMap::new();

        for item in graph.items() {
            let key = normalize(&item.names.en.value);
            if key.is_empty() {
                continue;
            }
            let relic = match &item.extra {
                Extra::Relic(r) => Some(r),
                Extra::None => None,
            };
            if let Some(r) = relic {
                // DE ships a relic twice where it was re-released, same name and same
                // rewards, so a printed name answers to every record that carries it.
                by_relic
                    .entry((key.clone(), r.refinement.clone()))
                    .or_insert_with(Vec::new)
                    .push(item.unique_name.clone());
            }
            grouped.entry(key).or_default().push(Candidate {
                path: item.unique_name.clone(),
                refinement: relic.map(|r| r.refinement.clone()),
                base: relic.map(|r| r.base.clone()),
            });
        }

        let mut by_name = BTreeMap::new();
        let mut ambiguous = BTreeSet::new();
        for (key, mut candidates) in grouped {
            // A relic's four refinements are one thing to a source that just prints its name,
            // and what drops is the intact one.
            if candidates.iter().all(|c| c.refinement.is_some()) {
                let bases = candidates
                    .iter()
                    .filter_map(|c| c.base.as_deref())
                    .collect::<BTreeSet<_>>()
                    .len();
                candidates.retain(|c| c.refinement.as_deref() == Some("intact"));
                if bases > 1 {
                    ambiguous.insert(key.clone());
                }
            } else {
                // A tier folder holds both training copies and real items (the Primed mods),
                // so only drop a tiered candidate when an untiered one answers to the name.
                if candidates.iter().any(|c| !graph::tiered(&c.path)) {
                    candidates.retain(|c| !graph::tiered(&c.path));
                }
                if candidates.len() > 1 {
                    ambiguous.insert(key.clone());
                }
            }

            if let Some(best) = candidates.iter().map(|c| &c.path).min() {
                by_name.insert(key, best.clone());
            }
        }
        Self {
            by_name,
            by_relic,
            ambiguous,
            curated: curated
                .iter()
                .filter(|(_, item)| graph.has(item))
                .map(|(name, item)| (normalize(name), item.to_string()))
                .collect(),
        }
    }

    /// Resolve a printed name, trying it as-is, then without a trailing `Blueprint`, then
    /// without a stack size printed inside it. Both fallbacks run only once the name itself
    /// has answered to nothing, so a name that legitimately reads that way — the `Lith X1`
    /// relic series — is never taken apart.
    pub fn get(&self, printed: &str) -> Option<&str> {
        if let Some(path) = self.curated.get(&normalize(printed)) {
            return Some(path);
        }
        if let Some((name, refinement)) = relic_grade(printed) {
            let key = (normalize(name), refinement.to_string());
            // A printed relic name can answer to more than one record; one path is all a
            // caller asking for "the item" can use, so the first is kept and `relics` is
            // there for the callers that need every one.
            if let Some(path) = self.by_relic.get(&key).and_then(|paths| paths.first()) {
                return Some(path);
            }
        }
        let key = normalize(printed);
        if let Some(path) = self.by_name.get(&key) {
            return Some(path);
        }
        if let Some(path) = key
            .strip_suffix(" blueprint")
            .and_then(|trimmed| self.by_name.get(trimmed))
        {
            return Some(path);
        }
        let stripped = without_stack(&key)?;
        self.by_name.get(&stripped).map(String::as_str)
    }

    /// Resolve a relic named separately from its refinement.
    pub fn relics(&self, printed: &str, refinement: &str) -> &[String] {
        let key = (normalize(printed), refinement.to_string());
        self.by_relic.get(&key).map_or(&[], Vec::as_slice)
    }

    /// Whether a printed name matches more than one item. A name tied by hand is settled.
    pub fn is_ambiguous(&self, printed: &str) -> bool {
        let key = normalize(printed);
        !self.curated.contains_key(&key) && self.ambiguous.contains(&key)
    }
}

/// Collapse whitespace and case so printed names match catalog ones.
fn normalize(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Split a stack size off a printed reward. The drop tables write `100X Oxium`, the wiki
/// writes both `10 x Ki'Teer Fireworks` and `Cipher x 100`.
pub fn quantity(printed: &str) -> (Option<i64>, &str) {
    if let Some((head, count)) = printed.rsplit_once(" x ")
        && let Ok(count) = count.trim().replace(',', "").parse::<i64>()
    {
        return (Some(count), head.trim_end());
    }
    let Some((head, rest)) = printed.split_once(char::is_whitespace) else {
        return (None, printed);
    };
    let (digits, rest) = match head.strip_suffix(['X', 'x']) {
        Some(digits) => (digits, rest),
        None => match rest.trim_start().split_once(char::is_whitespace) {
            Some(("x" | "X", tail)) => (head, tail),
            _ => return (None, printed),
        },
    };
    match digits.replace(',', "").parse::<i64>() {
        Ok(count) => (Some(count), rest.trim()),
        Err(_) => (None, printed),
    }
}

/// A normalized name with a stack size taken out of the middle of it. The gem vendors write
/// what a blueprint yields inside its name — `Adramal Alloy X20 Blueprint`, `Heart Nyth x3
/// Blueprint`, `Vapor Specter X 10 Blueprint` — where the catalog names the blueprint alone.
/// A size at either end is `quantity`'s job; only the middle is read here.
fn without_stack(key: &str) -> Option<String> {
    let words: Vec<&str> = key.split(' ').collect();
    for at in 1..words.len().saturating_sub(1) {
        let taken = match words[at].strip_prefix('x') {
            Some(digits) if is_count(digits) => 1,
            _ if words[at] == "x" && is_count(words[at + 1]) && at + 2 < words.len() => 2,
            _ => continue,
        };
        let kept: Vec<&str> = words[..at]
            .iter()
            .chain(&words[at + taken..])
            .copied()
            .collect();
        return Some(kept.join(" "));
    }
    None
}

fn is_count(word: &str) -> bool {
    !word.is_empty() && word.replace(',', "").parse::<i64>().is_ok()
}

/// Split a relic printed with its refinement, as in `Lith Q3 Relic (Radiant)`.
pub fn relic_grade(printed: &str) -> Option<(&str, &'static str)> {
    let (name, rest) = printed.rsplit_once(" (")?;
    let refinement = match rest.strip_suffix(')')? {
        "Intact" => "intact",
        "Exceptional" => "exceptional",
        "Flawless" => "flawless",
        "Radiant" => "radiant",
        _ => return None,
    };
    Some((name, refinement))
}

/// Rewards printed as an amount of something rather than as a catalog item.
pub fn is_amount(printed: &str) -> bool {
    let text = printed.trim();
    if matches!(text, "Region Resource" | "Ducats") || text.starts_with("Return:") {
        return true;
    }
    let Some((head, rest)) = text.split_once(' ') else {
        return false;
    };
    if head.replace(',', "").parse::<i64>().is_err() {
        return false;
    }
    matches!(rest, "Endo" | "Credits") || rest.ends_with("Cache")
}

#[cfg(test)]
mod tests {
    use super::*;

    use consensus::{Resolved, Source, Status};
    use graph::{Item, Kind, Names as ItemNames, Node};

    fn catalog(names: &[(&str, &str)]) -> Graph {
        let mut graph = Graph::new();
        for (path, name) in names {
            graph.insert(Node::Item(Item {
                unique_name: (*path).to_string(),
                names: ItemNames {
                    en: single(name.to_string()),
                    ru: None,
                },
                category: single("Test".to_string()),
                kind: single(Kind::unknown()),
                slug: None,
                tradable: None,
                vaulted: None,
                prime: single(false),
                ducats: None,
                extra: Extra::None,
            }));
        }
        graph
    }

    fn single<T>(value: T) -> Resolved<T> {
        Resolved {
            value,
            status: Status::Single,
            winner: Source::De,
            sources: vec![Source::De],
        }
    }

    #[test]
    fn a_name_tied_by_hand_answers_before_any_derived_match() {
        let graph = catalog(&[("/A/Sunder", "Sunder"), ("/B/Sunder", "Sunder")]);
        let plain = Index::build(&graph, &BTreeMap::new());
        assert_eq!(plain.get("Sunder"), Some("/A/Sunder"));
        assert!(plain.is_ambiguous("Sunder"));

        let curated = BTreeMap::from([("Sunder", "/B/Sunder")]);
        let decided = Index::build(&graph, &curated);
        assert_eq!(decided.get("sunder  "), Some("/B/Sunder"));
        assert!(!decided.is_ambiguous("Sunder"));
    }

    #[test]
    fn a_name_tied_to_something_the_catalog_lost_is_ignored() {
        let graph = catalog(&[("/A/Sunder", "Sunder")]);
        let curated = BTreeMap::from([("Legendary Core", "/Gone")]);
        assert_eq!(Index::build(&graph, &curated).get("Legendary Core"), None);
    }

    #[test]
    fn a_stack_size_inside_the_name_resolves_to_the_blueprint() {
        let graph = catalog(&[
            ("/Prospecting/AlloyBlueprint", "Adramal Alloy Blueprint"),
            ("/Prospecting/SpecterBlueprint", "Vapor Specter Blueprint"),
        ]);
        let index = Index::build(&graph, &BTreeMap::new());
        assert_eq!(
            index.get("Adramal Alloy X20 Blueprint"),
            Some("/Prospecting/AlloyBlueprint")
        );
        assert_eq!(
            index.get("Vapor Specter X 10 Blueprint"),
            Some("/Prospecting/SpecterBlueprint")
        );
    }

    #[test]
    fn a_name_that_reads_like_a_stack_size_keeps_it() {
        let graph = catalog(&[
            ("/Projections/LithX1", "Lith X1 Relic"),
            ("/Projections/LithRelic", "Lith Relic"),
        ]);
        let index = Index::build(&graph, &BTreeMap::new());
        assert_eq!(index.get("Lith X1 Relic"), Some("/Projections/LithX1"));
    }

    #[test]
    fn a_stack_size_is_only_read_out_of_the_middle() {
        assert_eq!(
            without_stack("adramal alloy x20 blueprint"),
            Some("adramal alloy blueprint".to_string())
        );
        assert_eq!(
            without_stack("vapor specter x 10 blueprint"),
            Some("vapor specter blueprint".to_string())
        );
        assert_eq!(without_stack("cipher x 100"), None);
        assert_eq!(without_stack("100x oxium"), None);
        assert_eq!(without_stack("orokin cell"), None);
    }

    #[test]
    fn splits_quantity_prefix() {
        assert_eq!(quantity("100X Oxium"), (Some(100), "Oxium"));
        assert_eq!(quantity("1,500X Ferrite"), (Some(1500), "Ferrite"));
        assert_eq!(quantity("Redirection"), (None, "Redirection"));
        assert_eq!(
            quantity("2,000 Credits Cache"),
            (None, "2,000 Credits Cache")
        );
    }

    #[test]
    fn splits_a_quantity_written_after_the_name() {
        assert_eq!(quantity("Cipher x 100"), (Some(100), "Cipher"));
        assert_eq!(
            quantity("Squad Health Restore (Large) x 100"),
            (Some(100), "Squad Health Restore (Large)")
        );
        assert_eq!(
            quantity("Dark Split-Sword (Dual Swords)"),
            (None, "Dark Split-Sword (Dual Swords)")
        );
    }

    #[test]
    fn splits_a_quantity_written_with_a_loose_x() {
        assert_eq!(
            quantity("10 x Ki'Teer Fireworks"),
            (Some(10), "Ki'Teer Fireworks")
        );
        assert_eq!(
            quantity("3 Day Affinity Booster"),
            (None, "3 Day Affinity Booster")
        );
    }

    #[test]
    fn reads_printed_refinement() {
        assert_eq!(
            relic_grade("Lith Q3 Relic (Radiant)"),
            Some(("Lith Q3 Relic", "radiant"))
        );
        assert_eq!(relic_grade("Lith Q3 Relic"), None);
        assert_eq!(relic_grade("Ivara Prime Blueprint (Nope)"), None);
    }

    #[test]
    fn amounts_are_not_items() {
        assert!(is_amount("80 Endo"));
        assert!(is_amount("3,000 Credits Cache"));
        assert!(is_amount("15,000 Credits"));
        assert!(is_amount("10,000 Höllars Cache"));
        assert!(is_amount("Region Resource"));
        assert!(is_amount("Return: 105,000"));
        assert!(!is_amount("Orokin Cell"));
        assert!(!is_amount("2 Day Affinity Booster"));
    }

    #[test]
    fn normalizes_case_and_spacing() {
        assert_eq!(normalize("Volt  Prime   Chassis"), "volt prime chassis");
    }
}
