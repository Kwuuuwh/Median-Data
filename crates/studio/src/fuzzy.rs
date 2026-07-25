use std::collections::{BTreeMap, BTreeSet};

use graph::Graph;
use maud::{Markup, html};

/// Below this a candidate is noise rather than a suggestion. Two unrelated names still read
/// as roughly half the same letter by letter, so the floor sits well above that.
const FLOOR: u8 = 65;

/// One catalog item a printed name might mean.
pub struct Hit {
    pub path: String,
    pub name: String,
    /// How close the names are, 0..=100. Only a hand decision is a fact; this only ranks.
    pub score: u8,
}

/// Every catalog name, indexed by word, so a query only scores the items that share a word
/// with it instead of all eighteen thousand. Owned rather than borrowed: it outlives the
/// request and is rebuilt with the graph.
#[derive(Default)]
pub struct Names {
    items: Vec<(String, String, String)>,
    by_word: BTreeMap<String, Vec<usize>>,
    by_name: BTreeMap<String, usize>,
}

impl Names {
    pub fn build(graph: &Graph) -> Self {
        let mut items = Vec::new();
        let mut by_word: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut by_name = BTreeMap::new();
        for item in graph.items() {
            let name = item.names.en.value.clone();
            let key = fold(&name);
            let at = items.len();
            for word in key.split_whitespace() {
                by_word.entry(word.to_string()).or_default().push(at);
            }
            by_name.entry(key.clone()).or_insert(at);
            items.push((item.unique_name.clone(), name, key));
        }
        Self {
            items,
            by_word,
            by_name,
        }
    }

    /// The items a printed name might mean, best first. An exact name scores full and is
    /// alone; everything else is ranked and left to a person.
    pub fn best(&self, printed: &str, want: usize) -> Vec<Hit> {
        let key = fold(printed);
        if key.is_empty() {
            return Vec::new();
        }
        if let Some(&at) = self.by_name.get(&key) {
            return vec![self.hit(at, 100)];
        }

        let mut seen: BTreeSet<usize> = BTreeSet::new();
        for word in key.split_whitespace() {
            if let Some(list) = self.by_word.get(word) {
                seen.extend(list);
            }
        }
        // Nothing shares a whole word — fall back to everything starting with the same letter,
        // which is what catches a misspelling.
        if seen.is_empty() {
            let first = key.chars().next().unwrap_or(' ');
            seen.extend(
                self.items
                    .iter()
                    .enumerate()
                    .filter(|(_, (_, _, k))| k.starts_with(first))
                    .map(|(at, _)| at),
            );
        }

        let mut hits: Vec<Hit> = seen
            .into_iter()
            .filter_map(|at| {
                let score = score(&key, &self.items[at].2);
                (score >= FLOOR).then(|| self.hit(at, score))
            })
            .collect();
        hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.name.cmp(&b.name)));
        hits.truncate(want);
        hits
    }

    fn hit(&self, at: usize, score: u8) -> Hit {
        let (path, name, _) = &self.items[at];
        Hit {
            path: path.clone(),
            name: name.clone(),
            score,
        }
    }
}

/// How close two folded names are: the better of how they read letter by letter and how many
/// whole words they share.
fn score(a: &str, b: &str) -> u8 {
    let letters = strsim::jaro_winkler(a, b);
    let words = tokens(a);
    let theirs = tokens(b);
    let shared = words.iter().filter(|w| theirs.contains(*w)).count() as f64;
    let overlap = shared / words.len().max(theirs.len()) as f64;
    (letters.max(overlap) * 100.0).round() as u8
}

fn tokens(name: &str) -> BTreeSet<&str> {
    name.split_whitespace().collect()
}

/// Collapse case, spacing and punctuation so two spellings of one name meet.
fn fold(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_alphanumeric() {
            out.extend(ch.to_lowercase());
        } else if !out.ends_with(' ') {
            out.push(' ');
        }
    }
    out.trim().to_string()
}

/// A candidate name with the letters it does not share with the query marked, so where the
/// two differ is visible without reading both.
pub fn diff(name: &str, query: &str) -> Markup {
    let shared = common(&fold(name), &fold(query));
    let mut left = shared.into_iter().peekable();
    let mut out = String::new();
    let mut marked = String::new();

    for ch in name.chars() {
        let folded: String = ch.to_lowercase().collect();
        let plain = ch.is_alphanumeric();
        if plain && left.peek() == Some(&folded) {
            left.next();
            if !marked.is_empty() {
                out.push('\u{0}');
                out.push_str(&marked);
                out.push('\u{0}');
                marked.clear();
            }
            out.push(ch);
        } else if plain {
            marked.push(ch);
        } else {
            // Spacing belongs to whichever run it sits in.
            if marked.is_empty() { out.push(ch) } else { marked.push(ch) }
        }
    }
    if !marked.is_empty() {
        out.push('\u{0}');
        out.push_str(&marked);
        out.push('\u{0}');
    }

    html! {
        @for (i, part) in out.split('\u{0}').enumerate() {
            @if i % 2 == 1 { span.diff { (part) } } @else { (part) }
        }
    }
}

/// The letters two names share in order — the longest common subsequence, over the folded
/// spelling so case and spacing do not count as a difference.
fn common(a: &str, b: &str) -> Vec<String> {
    let a: Vec<char> = a.chars().filter(|c| c.is_alphanumeric()).collect();
    let b: Vec<char> = b.chars().filter(|c| c.is_alphanumeric()).collect();
    let mut table = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            table[i][j] = if a[i] == b[j] {
                table[i + 1][j + 1] + 1
            } else {
                table[i + 1][j].max(table[i][j + 1])
            };
        }
    }
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            out.push(a[i].to_string());
            i += 1;
            j += 1;
        } else if table[i + 1][j] >= table[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_exact_name_scores_full() {
        assert_eq!(score(&fold("Quick Thinking"), &fold("quick  thinking")), 100);
    }

    #[test]
    fn a_shared_word_beats_a_shared_spelling() {
        let s = score(&fold("clashing forest"), &fold("clashing forest stance"));
        assert!(s >= 66, "{s}");
    }

    #[test]
    fn unrelated_names_fall_under_the_floor() {
        assert!(score(&fold("Orokin Cell"), &fold("Quick Thinking")) < FLOOR);
    }

    #[test]
    fn punctuation_is_not_a_difference() {
        assert_eq!(fold("Kahl's Garrison"), "kahl s garrison");
    }

    #[test]
    fn the_marked_part_is_what_the_query_does_not_have() {
        let out = diff("Smeeta Kavat Imprint", "Smeeta Kavat").into_string();
        assert_eq!(out, "Smeeta Kavat <span class=\"diff\">Imprint</span>");
    }

    #[test]
    fn an_identical_name_marks_nothing() {
        assert_eq!(diff("Volt Prime", "volt prime").into_string(), "Volt Prime");
    }
}
