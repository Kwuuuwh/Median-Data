use std::collections::{BTreeMap, BTreeSet};

use consensus::{Claim, Conflict, Resolved, Source, resolve};
use graph::{Edge, Extra, Graph, Item, Names, Node, Recipe, Rel, recipe_id};

use crate::bridge::Bridge;
use crate::curation::Curation;
use crate::extract::DeRecipe;
use crate::taxonomy;
use crate::{merge, rules};

/// The display names the build already knows, by item path — what a blueprint is named after.
pub struct Named<'a> {
    pub en: &'a BTreeMap<String, String>,
    pub ru: &'a BTreeMap<String, String>,
}

/// Blueprints the manifests never declare — DE knows them only as recipe keys. Named from
/// a hand decision first, then the market when it trades them, otherwise after the item they
/// build: `{result} Blueprint` in English, `{результат} (Чертеж)` in Russian, matching the
/// spelling DE gives its own localized blueprints. Curated names still win.
pub fn blueprints(
    recipes: &[DeRecipe],
    bridge: &Bridge<'_>,
    names: &Named<'_>,
    facts: &merge::Facts<'_>,
    curated: &Curation,
    conflicts: &mut Vec<Conflict>,
) -> Vec<Item> {
    let hand = curated.names();
    let picks = curated.picks();
    let mut made = BTreeSet::new();
    let mut out = Vec::new();
    for r in recipes {
        if names.en.contains_key(&r.blueprint) || !made.insert(r.blueprint.clone()) {
            continue;
        }
        let bp = r.blueprint.as_str();
        let wfm = bridge.get(&r.blueprint);

        // The market lists a blueprint under the name of what it builds ("Amesha Wings"), so
        // its spelling is only taken where the result has no name to build one from.
        let en = picks
            .get(&(bp, "name_en"))
            .map(|v| (Source::Curated, v.to_string()))
            .or_else(|| {
                names
                    .en
                    .get(&r.result)
                    .map(|result| (Source::Rule, format!("{result} Blueprint")))
            })
            .or_else(|| {
                wfm.and_then(|w| w.en_name.clone())
                    .map(|n| (Source::Wfm, n))
            });
        let Some((en_source, en)) = en else {
            continue;
        };

        let ru = picks
            .get(&(bp, "name_ru"))
            .or_else(|| hand.get(bp))
            .map(|v| claim(Source::Curated, v.to_string()))
            .or_else(|| {
                names
                    .ru
                    .get(&r.result)
                    .map(|result| claim(Source::Rule, format!("{result} (Чертеж)")))
            })
            .or_else(|| {
                wfm.and_then(|w| w.ru_name.clone())
                    .map(|n| claim(Source::Wfm, n))
            });

        let prime = match picks.get(&(bp, "prime")).copied() {
            Some("true") => claim(Source::Curated, true),
            Some("false") => claim(Source::Curated, false),
            _ => claim(Source::Rule, rules::prime(&en, &r.blueprint, facts.economy)),
        };
        let tradable = match picks.get(&(bp, "tradable")).copied() {
            Some("true") => Some(claim(Source::Curated, true)),
            Some("false") => Some(claim(Source::Curated, false)),
            _ => wfm.map(|_| claim(Source::Wfm, true)),
        };
        out.push(Item {
            unique_name: r.blueprint.clone(),
            names: Names {
                en: claim(en_source, en),
                ru,
            },
            category: claim(Source::Rule, taxonomy::BLUEPRINT.to_string()),
            kind: merge::settle(
                bp,
                picks.get(&(bp, "kind")).copied(),
                facts.taxonomy.classify_blueprint(bp),
                &facts.taxonomy.tree,
                conflicts,
            ),
            slug: wfm.map(|w| claim(Source::Wfm, w.slug.clone())),
            tradable,
            vaulted: None,
            prime,
            ducats: wfm.and_then(|w| w.ducats),
            mastery: None,
            mastery_req: None,
            max_level_cap: None,
            extra: Extra::None,
        });
    }
    out
}

/// Recipe nodes with their produces/requires edges. Endpoints the graph does not hold are
/// skipped and reported.
pub fn link(graph: &mut Graph, recipes: &[DeRecipe]) -> BTreeSet<String> {
    let mut dangling = BTreeSet::new();
    for r in recipes {
        if !graph.has(&r.result) {
            dangling.insert(r.result.clone());
            continue;
        }
        let id = recipe_id(&r.blueprint);
        graph.insert(Node::Recipe(Recipe {
            blueprint: r.blueprint.clone(),
            build_price: r.build_price,
            build_time: r.build_time,
            consumed: r.consumed,
            rush_price: r.rush_price,
            output: r.output,
        }));
        graph.link(Edge {
            from: id.clone(),
            to: r.result.clone(),
            rel: Rel::Produces,
        });
        for (item, count) in &r.ingredients {
            if !graph.has(item) {
                dangling.insert(item.clone());
                continue;
            }
            graph.link(Edge {
                from: id.clone(),
                to: item.clone(),
                rel: Rel::Requires { count: *count },
            });
        }
    }
    dangling
}

fn claim<T: Clone + PartialEq>(source: Source, value: T) -> Resolved<T> {
    resolve(&[Claim { source, value }], &[source]).expect("one claim resolves")
}
