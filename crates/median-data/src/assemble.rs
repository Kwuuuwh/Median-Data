use std::collections::BTreeMap;

use consensus::Conflict;
use funnel::{Gaps, RelicClaim};
use graph::{Graph, Node, Taxonomy};
use sources::drops::{Drop, RelicRow};
use sources::wfm::WfmItem;

use crate::bridge::Bridge;
use crate::curation::Curation;
use crate::extract::{DeItem, DeRecipe, DeRegion, DeReward};
use crate::names::{self, Index};
use crate::paths::Paths;
use crate::regions::{self, Labels, Linked};
use crate::taxonomy::Policy;
use crate::{craft, drifters, drops, imprints, merge, primes, relic, rules, sets};

/// What one build produced besides the graph itself.
pub struct Built {
    pub graph: Graph,
    /// The tree that classified this graph's items.
    pub taxonomy: Taxonomy,
    pub conflicts: Vec<Conflict>,
    pub gaps: Gaps,
    /// Drop-table relic rewards resolved to catalog paths, to check DE against.
    pub relic_witness: Vec<RelicClaim>,
    /// What checking relic contents against the wiki came to.
    pub witnessed: crate::relic::Witnessed,
    pub places: usize,
    pub enemies: usize,
    /// What the vault pass settled.
    pub vaulted: crate::vaulting::Vaulted,
    /// Drop rows the tables print twice, kept once.
    pub repeated: usize,
    /// What tying the star chart to the drop tables produced.
    pub star_chart: Linked,
    /// What the wiki says about our reference labels.
    pub witness: regions::Witness,
    /// Nodes the wiki names but gives no key, so nothing can be joined to them.
    pub keyless: Vec<String>,
    /// What the wiki says about the mission drop tables.
    pub drop_witness: crate::witness::Drops,
    /// What tying vendor offerings to the catalog produced.
    pub vendors: crate::vendors::Linked,
    /// What tying dojo research to the catalog produced.
    pub dojo: crate::labs::Linked,
    /// Every printed name no catalog item answers to, from each source that prints names.
    pub orphans: Vec<studio::Unresolved>,
    /// Drop rows naming an amount rather than an item.
    pub amounts: usize,
    /// Drop rows whose printed name matches several items.
    pub ambiguous: usize,
    /// The market listings this build read, kept for the curation queue.
    pub wfm: Vec<WfmItem>,
    /// Listing slug -> the catalog path it was tied to.
    pub matched: BTreeMap<String, String>,
    /// How many listings each kind of match accounts for.
    pub matching: BTreeMap<&'static str, usize>,
    /// Ordinary items linked to their prime counterpart.
    pub primed: usize,
    /// Operator cosmetics linked to the Drifter's copy of them.
    pub fitted: usize,
    /// Market imprints modelled as their own node.
    pub imprinted: usize,
    /// Relic refinement steps linked.
    pub refined: usize,
    /// Classification rules that decided nothing, as the audit describes them.
    pub dead_rules: Vec<String>,
    /// Headings the drop-table parser knew the layout of and still could not place.
    pub unread_headings: Vec<String>,
}

/// Raw inputs of one build.
pub struct Input {
    pub de: Vec<DeItem>,
    pub ru: BTreeMap<String, String>,
    pub recipes: Vec<DeRecipe>,
    pub rewards: Vec<DeReward>,
    pub regions: Vec<DeRegion>,
    /// The same nodes from the Russian manifest, keyed by DE's node key.
    pub regions_ru: BTreeMap<String, DeRegion>,
    /// The star chart as the wiki describes it, for what DE leaves out and to check the rest.
    pub chart: crate::wiki::Chart,
    /// The wiki's own mission reward tables, keyed by the alias the star chart names them by.
    pub wiki_tables: BTreeMap<String, Vec<crate::wiki::Row>>,
    /// Everything Baro has ever brought.
    pub baro: Vec<crate::wiki::Offered>,
    /// The dojo labs and their research.
    pub dojo: crate::wiki::Dojo,
    /// Every vendor the wiki lists by stock.
    pub stores: Vec<crate::wiki::Store>,
    /// Blueprints the market sells for credits.
    pub market: Vec<crate::wiki::Priced>,
    pub wfm: Vec<WfmItem>,
    pub drops: Vec<Drop>,
    pub relic_rows: Vec<RelicRow>,
    /// What the wiki says about which relics are in the vault.
    pub vaulting: Vec<crate::wiki::Vaulting>,
    /// What the wiki says every relic awards, to check DE's own account against.
    pub composition: Vec<crate::wiki::Slot>,
}

/// Merge every source into the knowledge graph, honouring curated decisions.
pub fn assemble(
    input: Input,
    mut taxonomy: Policy,
    mastery: &crate::mastery::Policy,
    labels: &Labels,
    settlements: &crate::bounties::Settlements,
    curated: &Curation,
) -> Built {
    let links = curated.market_links();
    let terms = curated.terms();
    for (kind, key, ru) in curated.term.iter().map(|t| (&t.kind, &t.key, &t.ru)) {
        if kind == "class" || kind == "kind" {
            taxonomy.tree.relabel(key, ru);
        }
    }
    let paths = Paths::build(&input.de, &input.recipes);
    let bridge = Bridge::new(&input.wfm, &paths, &links);
    let matched = bridge
        .matched()
        .iter()
        .map(|(slug, (path, _))| (slug.to_string(), path.clone()))
        .collect();
    let matching = bridge.tally();

    let built = rules::built(&input.recipes);
    let economy = rules::void_economy(&input.rewards, &input.recipes);

    let mut conflicts = Vec::new();
    let facts = merge::Facts {
        built: &built,
        economy: &economy,
        taxonomy: &taxonomy,
        mastery,
    };
    let items = merge::items(
        input.de,
        &input.ru,
        &bridge,
        curated,
        &facts,
        &mut conflicts,
    );

    let mut graph = Graph::new();
    let mut names = BTreeMap::new();
    let mut ru_names = BTreeMap::new();
    for item in items {
        names.insert(item.unique_name.clone(), item.names.en.value.clone());
        if let Some(ru) = &item.names.ru {
            ru_names.insert(item.unique_name.clone(), ru.value.clone());
        }
        graph.insert(Node::Item(item));
    }
    let named = craft::Named {
        en: &names,
        ru: &ru_names,
    };
    for item in craft::blueprints(
        &input.recipes,
        &bridge,
        &named,
        &facts,
        curated,
        &mut conflicts,
    ) {
        graph.insert(Node::Item(item));
    }

    // Every item is in the graph by now and nothing after this adds one, so the reverse
    // name index is built once and read by everything that resolves a printed name.
    let index = Index::build(&graph, &curated.named());

    let dangling_craft = craft::link(&mut graph, &input.recipes);
    let unresolved_rewards = relic::link(&mut graph, &input.rewards);
    let refined = relic::refine(&mut graph);
    sets::link(&mut graph, &bridge);
    let imprinted = imprints::link(&mut graph, &input.wfm);
    let primed = primes::link(&mut graph);
    let fitted = drifters::link(&mut graph);

    let dropped = drops::link(&mut graph, &input.drops, settlements, &index, &terms);
    let looted = drops::curated(&mut graph, &curated.drop, &index, &terms);
    let missed = &dropped.missed;
    let vaulted = crate::vaulting::mark(
        &mut graph,
        &input.vaulting,
        &input.wfm,
        &matched,
        &curated.picks(),
        &mut conflicts,
    );
    let wiki_witness = regions::witness(&input.regions, &input.chart, labels);
    let star_chart = regions::link(
        &mut graph,
        &input.regions,
        &input.regions_ru,
        &input.chart.nodes,
        labels,
        &terms,
    );

    let stock = crate::vendors::Stock {
        stores: &input.stores,
        baro: &input.baro,
        market: &input.market,
    };
    let vendors = crate::vendors::link(&mut graph, &stock, &index, curated, &terms);
    let dojo = crate::labs::link(&mut graph, &input.dojo, &index, &terms);
    let relic_witness = witness(&index, &input.relic_rows);
    let witnessed = relic::witnessed(&graph, &index, &input.composition, &mut conflicts);
    let drop_witness =
        crate::witness::drops(&graph, &input.chart.nodes, &input.wiki_tables, &index);

    let mut orphans = crate::orphans::rows(crate::curation::DROPS, &missed.unknown);
    orphans.extend(crate::orphans::rows(crate::curation::DROPS, &looted));
    orphans.extend(crate::orphans::rows(
        crate::curation::VENDOR,
        &vendors.unresolved,
    ));
    orphans.extend(crate::orphans::rows(
        crate::curation::DOJO,
        &dojo.unresolved,
    ));

    Built {
        conflicts,
        gaps: Gaps {
            unresolved_rewards: unresolved_rewards.into_iter().collect(),
            dangling_craft: dangling_craft.into_iter().collect(),
            unknown_drop_items: missed.unknown.keys().cloned().collect(),
            provisional: taxonomy.provisional(),
            verbatim: curated
                .verbatim()
                .into_iter()
                .filter(|(kind, _)| *kind == "item")
                .map(|(_, key)| key.to_string())
                .collect(),
        },
        orphans,
        relic_witness,
        witnessed,
        places: dropped.places,
        enemies: dropped.enemies,
        vaulted,
        repeated: missed.repeated,
        star_chart,
        witness: wiki_witness,
        keyless: input.chart.keyless,
        drop_witness,
        vendors,
        dojo,
        amounts: missed.not_an_item,
        ambiguous: missed.ambiguous,
        wfm: input.wfm,
        matched,
        matching,
        primed,
        fitted,
        imprinted,
        refined,
        dead_rules: Vec::new(),
        unread_headings: Vec::new(),
        graph,
        taxonomy: taxonomy.tree,
    }
}

/// Resolve printed drop-table relic rows to catalog paths. Rows naming something the
/// catalog lacks are dropped: coverage already reports those.
fn witness(index: &Index, rows: &[RelicRow]) -> Vec<RelicClaim> {
    let mut out = Vec::new();
    for row in rows {
        let (_, printed) = names::quantity(&row.reward);
        let Some(reward) = index.get(printed) else {
            continue;
        };
        for relic in index.relics(&row.relic, &row.refinement) {
            out.push(RelicClaim {
                relic: relic.clone(),
                reward: reward.to_string(),
                chance: row.chance,
            });
        }
    }
    out
}
