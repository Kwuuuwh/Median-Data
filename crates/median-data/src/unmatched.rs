use std::collections::BTreeMap;

use sources::wfm::WfmItem;
use studio::Unresolved;

use crate::curation::MARKET;

/// Market listings the catalog could not tie to an item: the market names no path, or the path
/// it names is gone, and no single catalog item answers to the listing's name. Which item each
/// might mean is ranked where a person can see it, not here.
pub fn find(wfm: &[WfmItem], matched: &BTreeMap<String, String>) -> Vec<Unresolved> {
    let mut out: Vec<Unresolved> = wfm
        .iter()
        .filter(|w| !w.has_tag("set") && !w.has_tag("imprint") && !matched.contains_key(&w.slug))
        .map(|w| Unresolved {
            source: MARKET.to_string(),
            key: w.slug.clone(),
            name: w.en_name.clone().unwrap_or_else(|| w.slug.clone()),
            hint: match &w.game_ref {
                Some(path) => format!("рынок ссылается на {path}"),
                None => "рынок не называет предмет".to_string(),
            },
            count: 1,
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.key.cmp(&b.key)));
    out
}
