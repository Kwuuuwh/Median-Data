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
    /// Relics whose contents the export does not carry yet, filled from the drop tables.
    pub relics_from_tables: usize,
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
    /// Items linked to what comes along with them.
    pub granted: usize,
    /// Breeds an incubation can turn out.
    pub hatched: usize,
    /// Yields of the open worlds' veins.
    pub mined: usize,
    /// Star-chart nodes linked to the enemies they send.
    pub haunted: crate::haunts::Haunted,
    /// Relic refinement steps linked.
    pub refined: usize,
    /// Items that took DE weapon facts.
    pub armed: usize,
    /// Wiki weapon entries hung on a DE weapon.
    pub mounts: usize,
    /// Weapons a riven fits.
    pub rivened: usize,
    /// Riven classes with their stat pool in the graph.
    pub pools: usize,
    /// Gun mods and arcanes that took their effects.
    pub upgrades: (usize, usize),
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
    /// DE weapon facts, keyed by item.
    pub weapons: BTreeMap<String, graph::Weapon>,
    /// The wiki's weapon entries, every slot page together.
    pub wiki_weapons: Vec<crate::wiki::WeaponEntry>,
    /// The weapon lines the market trades rivens for.
    pub riven_weapons: Vec<sources::wfm::RivenWeapon>,
    /// The lines of the client the catalog reads, distilled out of its cache.
    pub phrases: crate::phrases::Phrases,
    /// The stats each class of riven rolls from.
    pub riven_pools: Vec<crate::extract::DeRivenPool>,
    /// The riven stats the market lets an auction name.
    pub riven_attributes: Vec<sources::wfm::RivenAttribute>,
    /// Which client line a riven stat is shown with, where that was chosen by hand.
    pub riven_lines: crate::pools::Lines,
    /// The splicing table, written by hand.
    pub splicing: crate::splicing::Splicing,
    /// Rows the wiki's splicing table has.
    pub wiki_splice_rows: usize,
    /// Mods for guns, with their top rank.
    pub mods: Vec<crate::extract::DeMod>,
    /// Arcanes, with their top rank.
    pub arcanes: Vec<crate::extract::DeArcane>,
    /// The star chart as the wiki describes it, for what DE leaves out and to check the rest.
    pub chart: crate::wiki::Chart,
    /// The wiki's own mission reward tables, keyed by the alias the star chart names them by.
    pub wiki_tables: BTreeMap<String, Vec<crate::wiki::Row>>,
    /// Everything Baro has ever brought.
    pub baro: Vec<crate::wiki::Offered>,
    /// The dojo labs and their research.
    pub dojo: crate::wiki::Dojo,
    /// What each of the game's own vendor manifests sells.
    pub stalls: Vec<crate::offers::Stall>,
    /// Who the wiki says keeps which counter, for putting a person behind a manifest.
    pub keepers: Vec<crate::wiki::Store>,
    /// Blueprints the market sells for credits.
    pub market: Vec<crate::wiki::Priced>,
    pub wfm: Vec<WfmItem>,
    pub drops: Vec<Drop>,
    pub relic_rows: Vec<RelicRow>,
    /// What the wiki says about which relics are in the vault.
    pub vaulting: Vec<crate::wiki::Vaulting>,
    /// What the wiki says every relic awards, to check DE's own account against.
    pub composition: Vec<crate::wiki::Slot>,
    /// What the game client calls things, distilled out of its cache.
    pub spoken: crate::game::Names,
    /// What the game hands over along with an item, distilled out of its cache.
    pub grants: crate::grants::Grants,
    /// What the incubator hatches, distilled out of its cache.
    pub incubator: crate::incubator::Incubator,
    /// What the open worlds' veins yield, distilled out of its cache.
    pub mining: crate::mining::Mining,
    /// Which enemies each tileset sends, distilled out of its cache.
    pub spawns: crate::spawns::Spawns,
}

/// Where things stand, as written by hand.
pub struct Ground<'a> {
    pub areas: &'a crate::areas::Areas,
    pub tilesets: &'a crate::haunts::Tilesets,
}

/// Merge every source into the knowledge graph, honouring curated decisions.
pub fn assemble(
    input: Input,
    mut taxonomy: Policy,
    mastery: &crate::mastery::Policy,
    labels: &Labels,
    settlements: &crate::bounties::Settlements,
    ground: Ground<'_>,
    curated: &Curation,
) -> Built {
    let links = curated.market_links();
    let terms = curated.terms(&input.spoken.name);
    for (kind, key, ru) in curated.term.iter().map(|t| (&t.kind, &t.key, &t.ru)) {
        if kind == "class" || kind == "kind" {
            taxonomy.tree.relabel(key, ru);
        }
    }
    let paths = Paths::build(&input.de, &input.recipes);
    let bridge = Bridge::new(&input.wfm, &paths, &links, &input.recipes);
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
    let armed = crate::weapons::attach(&mut graph, &input.weapons);
    let mounted = crate::weapons::mount(&mut graph, input.wiki_weapons);
    let rivened = crate::rivens::judge(&mut graph, &input.riven_weapons, &mut conflicts);
    let unspoken = crate::phrases::link(&mut graph, &input.phrases);
    let pooled = crate::pools::link(
        &mut graph,
        input.riven_pools,
        &crate::pools::Witnesses {
            phrases: &input.phrases,
            lines: &input.riven_lines,
            market: &input.riven_attributes,
        },
    );
    let unspliced = crate::splicing::link(&mut graph, &input.splicing, input.wiki_splice_rows);
    let upgraded = crate::upgrades::link(&mut graph, input.mods, input.arcanes);
    sets::link(&mut graph, &bridge);
    let imprinted = imprints::link(&mut graph, &input.wfm);
    let primed = primes::link(&mut graph);
    let fitted = drifters::link(&mut graph);
    let granted = crate::grants::link(&mut graph, &input.grants);
    let hatched = crate::incubator::link(&mut graph, &input.incubator);
    let mined = crate::mining::link(&mut graph, &input.mining, ground.areas);

    let dropped = drops::link(&mut graph, &input.drops, settlements, &index, &terms);
    let looted = drops::curated(&mut graph, &curated.drop, &index, &terms);
    let unpaid = drops::paid(&mut graph, &curated.payout, &index, &terms);
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
        &input.chart,
        labels,
        &terms,
    );

    let stock = crate::vendors::Stock {
        stalls: &input.stalls,
        keepers: &input.keepers,
        baro: &input.baro,
        market: &input.market,
    };
    let vendors = crate::vendors::link(&mut graph, &stock, &index, curated, &terms);
    let haunted = crate::haunts::link(&mut graph, &input.spawns, ground.tilesets);
    crate::areas::link(&mut graph, ground.areas);
    let dojo = crate::labs::link(&mut graph, &input.dojo, &index, &terms);
    let relic_witness = witness(&index, &input.relic_rows);
    let relics_from_tables = relic::from_tables(&mut graph, &input.relic_rows, &index);
    let witnessed = relic::witnessed(&graph, &index, &input.composition, &mut conflicts);
    let drop_witness =
        crate::witness::drops(&graph, &input.chart.nodes, &input.wiki_tables, &index);

    let mut orphans = crate::orphans::rows(crate::curation::DROPS, &missed.unknown);
    orphans.extend(crate::orphans::rows(crate::curation::DROPS, &looted));
    orphans.extend(crate::orphans::rows(crate::curation::DROPS, &unpaid));
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
            ambiguous: input.spoken.ambiguous.keys().cloned().collect(),
            in_the_game: known_sets(&graph, &input.spoken),
            unmatched_weapons: mounted.unmatched,
            unread_weapons: mounted.unread,
            unknown_rivens: rivened.unknown,
            unread_rivens: rivened.unread,
            unspoken,
            unread_pools: pooled.unread,
            unspliced,
            unread_effects: upgraded.unread,
            verbatim: curated
                .verbatim()
                .into_iter()
                .filter(|(kind, _)| *kind == "item")
                .map(|(_, key)| key.to_string())
                .collect(),
        },
        orphans,
        relic_witness,
        relics_from_tables,
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
        granted,
        hatched,
        mined,
        haunted,
        refined,
        armed,
        mounts: mounted.mounts,
        rivened: rivened.weapons,
        pools: pooled.pools,
        upgrades: (upgraded.mods, upgraded.arcanes),
        dead_rules: Vec::new(),
        unread_headings: Vec::new(),
        graph,
        taxonomy: taxonomy.tree,
    }
}

/// Resolve printed drop-table relic rows to catalog paths. Rows naming something the
/// catalog lacks are dropped: coverage already reports those.
/// Sets the game client knows by name. A set is the market's word for a bundle of parts, so
/// the client never prints `Citrine Prime Set` — but it does print `Citrine Prime`, and that
/// is enough to say the thing exists in the game and the export is merely behind.
fn known_sets(
    graph: &graph::Graph,
    spoken: &crate::game::Names,
) -> std::collections::BTreeSet<String> {
    let printed: std::collections::BTreeSet<String> =
        spoken.name.keys().map(|name| name.to_lowercase()).collect();
    graph
        .nodes()
        .filter_map(|node| match node {
            graph::Node::Set(set) => Some(set),
            _ => None,
        })
        .filter(|set| {
            let name = set.names.en.value.to_lowercase();
            let base = name.strip_suffix(" set").unwrap_or(&name);
            printed.contains(base)
        })
        .map(|set| set.slug.clone())
        .collect()
}

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
