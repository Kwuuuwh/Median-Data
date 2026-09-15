use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use funnel::{Report, State, Totals};
use vault::{BlobId, Snapshot, Vault};

use crate::assemble::{self, Built, Input};
use crate::curation::Curation;
use crate::{bounties, curation, extract, mastery, regions, spec, taxonomy, version, wiki};

/// Sources whose pinned snapshot the catalog records, so a build can name what it read.
const SOURCES: [&str; 7] = [
    spec::DE,
    spec::WFM,
    spec::DROPS,
    spec::WIKI,
    spec::ICONS,
    spec::WFM_ICONS,
    spec::PORTRAITS,
];

/// Where the build's own artifacts live, beside the catalog.
const REPORT: &str = "catalog.report.json";
const STATE: &str = "catalog.state.json";

/// The Railjack's plexus, which no manifest exports.
const PLEXUS: &str = "/Lotus/Types/Game/CrewShip/RailJack/DefaultHarness";

/// Build the catalog from the latest pinned snapshots into `out`.
pub fn run(vault: &Vault, out: &Path) -> Result<()> {
    let curated = crate::curation::load(Path::new(crate::CURATION))?;
    let built = graph_with(vault, &curated)?;
    let (report, state) = judge(vault, &built, &curated)?;

    print!("{}", report.render());
    taxonomy_tally(&built);
    mastery_tally(&built);
    eprintln!(
        "drops    {} rows are amounts, {} names matched several items, \
         {} rows printed twice kept once",
        built.amounts, built.ambiguous, built.repeated
    );
    let vaulted = &built.vaulted;
    eprintln!(
        "vault    {} relics stated, {} items derived ({} withdrawn — obtainable elsewhere), \
         {} relics nothing states",
        vaulted.relics,
        vaulted.derived,
        vaulted.contradicted,
        crate::vaulting::unstated(&built.graph).len()
    );
    for name in vaulted.unmatched.iter().take(8) {
        eprintln!("vault    the wiki names a relic the catalog does not hold — {name}");
    }
    let rewards = &built.witnessed;
    eprintln!(
        "rewards  {} relics checked against the wiki, {} stated by DE alone",
        rewards.compared, rewards.alone
    );
    for name in rewards.unresolved.iter().take(8) {
        eprintln!("rewards  the wiki awards something the catalog does not hold — {name}");
    }
    let chart = &built.star_chart;
    eprintln!(
        "chart    {} nodes from DE + {} from the wiki, {} places tied, {} not a node, \
         {} nodes no table mentions",
        chart.regions,
        chart.from_wiki,
        chart.bridged,
        chart.not_a_node,
        chart.silent.len()
    );
    eprintln!(
        "chart    {} locations the nodes are filed under",
        chart.locations
    );
    for name in &chart.untyped {
        eprintln!("chart    nothing says what this location is — {name}");
    }
    for place in &chart.unbridged {
        eprintln!("chart    no such node on the star chart — {place}");
    }
    for name in &built.keyless {
        eprintln!("chart    the wiki gives this node no key, left out — {name}");
    }
    let seen = &built.witness;
    eprintln!(
        "wiki     {} labels the wiki agrees with, {} disagreements kept on purpose, \
         {} open, {} it could fill",
        seen.agreed,
        seen.accepted,
        seen.disagree.len(),
        seen.fillable.len()
    );
    for line in seen.disagree.iter().chain(&seen.fillable) {
        eprintln!("wiki     {line}");
    }
    let sold = &built.vendors;
    eprintln!(
        "vendors  {} vendors, {} offerings tied, {} names no item answers to",
        sold.vendors,
        sold.offers,
        sold.unresolved.len()
    );
    let dojo = &built.dojo;
    eprintln!(
        "dojo     {} labs, {} researches tied to a blueprint, {} names no item answers to",
        dojo.labs,
        dojo.research,
        dojo.unresolved.len()
    );
    let drops = &built.drop_witness;
    eprintln!(
        "wiki     {} of its drop rows compared over {} places ({} relic rows left to rotation, \
         {} rows name no catalog item)",
        drops.claims.len(),
        drops.places,
        drops.relics,
        drops.unresolved
    );
    for line in &chart.mismatched {
        eprintln!("chart    mission type disagrees — {line}");
    }
    let matching: Vec<String> = built
        .matching
        .iter()
        .map(|(how, count)| format!("{how} {count}"))
        .collect();
    eprintln!(
        "market   {} listings tied ({}), {} primes linked, {} imprints as own node",
        built.matched.len(),
        matching.join(", "),
        built.primed,
        built.imprinted
    );
    eprintln!(
        "relics   {} refinement steps linked — only the intact relic drops",
        built.refined
    );
    eprintln!(
        "operator {} cosmetics linked to the Drifter's copy of them",
        built.fitted
    );
    std::fs::write(REPORT, serde_json::to_vec_pretty(&report)?)?;

    let blocking = funnel::blocking(&report.findings);
    if !blocking.is_empty() {
        for f in &blocking {
            eprintln!("INVARIANT {} — {}: {}", f.rule, f.entity, f.detail);
        }
        anyhow::bail!(
            "{} invariant violation(s); catalog not written",
            blocking.len()
        );
    }

    project(vault, out, &built, &report, &state)?;
    std::fs::write(STATE, serde_json::to_vec_pretty(&state)?)?;
    Ok(())
}

/// How the taxonomy filled up, largest class first.
fn taxonomy_tally(built: &Built) {
    let mut classes: BTreeMap<&str, usize> = BTreeMap::new();
    for item in built.graph.items() {
        *classes
            .entry(built.taxonomy.class_slug(&item.kind.value))
            .or_default() += 1;
    }
    let mut ranked: Vec<(&str, usize)> = classes.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    eprintln!(
        "taxonomy {} items in {} classes",
        ranked.iter().map(|(_, n)| n).sum::<usize>(),
        ranked.len()
    );
    for (class, count) in ranked {
        eprintln!("           {count:>6}  {class}");
    }
}

/// Items giving mastery per table and class, where their top rank came from, what nodes give.
fn mastery_tally(built: &Built) {
    let mut tables: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    let (mut from_de, mut by_rule) = (0, 0);
    for item in built.graph.items() {
        let Some(mastery) = &item.mastery else {
            continue;
        };
        let class = built.taxonomy.class_slug(&item.kind.value);
        *tables.entry((mastery.value.as_str(), class)).or_default() += 1;
        match item.max_level_cap.as_ref().map(|cap| cap.winner) {
            Some(consensus::Source::De) => from_de += 1,
            Some(_) => by_rule += 1,
            None => {}
        }
    }
    eprintln!(
        "mastery  {} items give mastery, top rank from DE for {from_de} and by rule for {by_rule}",
        tables.values().sum::<usize>()
    );
    for ((table, class), count) in &tables {
        eprintln!("           {count:>6}  {table} {class}");
    }

    let (mut nodes, mut total) = (0, 0);
    for node in built.graph.nodes() {
        if let graph::Node::Region(region) = node {
            if region.mastery_xp > 0 {
                nodes += 1;
                total += region.mastery_xp;
            }
        }
    }
    eprintln!("mastery  {nodes} star-chart nodes give {total} on first completion");
}

/// How much of the catalog the graph can actually explain the source of. Most of what is
/// missing is a source nobody has modelled yet rather than a defect, so this is a metric and
/// not a pile of findings: 8k cosmetics are bought with platinum, blueprints come from the
/// in-game market or clan research, augments from syndicates.
fn acquisition_tally(built: &Built, scope: &projections::Scope) {
    let mut gaps: BTreeMap<&str, usize> = BTreeMap::new();
    let (mut shipped, mut known, mut untracked) = (0, 0, 0);
    for item in built.graph.items() {
        if !scope.allows(&item.unique_name) {
            continue;
        }
        if !built.taxonomy.sourced(&item.kind.value) {
            untracked += 1;
            continue;
        }
        shipped += 1;
        if graph::obtainable(&built.graph, &item.unique_name) {
            known += 1;
        } else {
            *gaps
                .entry(built.taxonomy.class_slug(&item.kind.value))
                .or_default() += 1;
        }
    }
    eprintln!(
        "sources  {known} of {shipped} items have a path we can name \
         ({untracked} more are appearance items, whose sources we do not track)"
    );
    let mut ranked: Vec<(&str, usize)> = gaps.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    for (class, count) in ranked.iter().take(8) {
        eprintln!("           {count:>6}  {class}");
    }
}

/// Rules that decided nothing: a dead rule is either wrong or left over. The answer goes to
/// the funnel rather than to the terminal, so it is read on the same page as everything else.
fn audit(
    policy: &taxonomy::Policy,
    de: &[extract::DeItem],
    recipes: &[extract::DeRecipe],
) -> Vec<String> {
    let facts = de.iter().map(taxonomy::facts).chain(
        recipes
            .iter()
            .map(|r| taxonomy::blueprint_facts(&r.blueprint)),
    );
    policy
        .unused(facts)
        .into_iter()
        .map(|rule| {
            format!(
                "taxonomy rule for '{}' matched nothing — {}",
                rule.kind, rule.reason
            )
        })
        .collect()
}

/// Run every filter of the funnel over an assembled graph.
pub fn judge(vault: &Vault, built: &Built, curated: &Curation) -> Result<(Report, State)> {
    let anchors = funnel::load_anchors(Path::new(crate::ANCHORS))?;
    let previous = std::fs::read(STATE)
        .ok()
        .and_then(|raw| serde_json::from_slice::<State>(&raw).ok());
    let was = previous.as_ref().and_then(|s| s.version.clone());
    let made_by = previous.as_ref().and_then(|s| s.recipe.clone());
    let recipe = crate::recipe::fingerprint(Path::new("."))?;
    let _ = vault;
    let scope = projections::apply(&built.graph, &projections::load(Path::new(crate::SCOPE))?);

    let (report, mut state) = funnel::run(
        &built.graph,
        funnel::Input {
            totals: Totals {
                items: built.graph.items().count(),
                places: built.places,
                enemies: built.enemies,
                edges: built.graph.edge_count(),
                conflicts: built.conflicts.len(),
            },
            gaps: funnel::Gaps {
                unresolved_rewards: built.gaps.unresolved_rewards.clone(),
                dangling_craft: built.gaps.dangling_craft.clone(),
                unknown_drop_items: built.gaps.unknown_drop_items.clone(),
                provisional: built.gaps.provisional.clone(),
                verbatim: curated
                    .verbatim()
                    .into_iter()
                    .filter(|(kind, _)| *kind == "item")
                    .map(|(_, key)| key.to_string())
                    .collect(),
            },
            relic_witness: built
                .relic_witness
                .iter()
                .map(|c| funnel::RelicClaim {
                    relic: c.relic.clone(),
                    reward: c.reward.clone(),
                    chance: c.chance,
                })
                .collect(),
            drop_witness: built
                .drop_witness
                .claims
                .iter()
                .map(|c| funnel::DropClaim {
                    place: c.place.clone(),
                    item: c.item.clone(),
                    chance: c.chance,
                    rotation: c.rotation.clone(),
                })
                .collect(),
            anchors: &anchors,
            taxonomy: &built.taxonomy,
            dead_rules: built.dead_rules.clone(),
            unread_headings: built.unread_headings.clone(),
            shipped: &|entity| scope.allows(entity),
            // A drop-table name declared to be no item at all answers the same question the
            // coverage check asks, so the verdict counts as accepting its finding.
            accepted: curated
                .accepted()
                .into_iter()
                .map(|(rule, entity)| (rule.to_string(), entity.to_string()))
                .chain(
                    curated
                        .dismissed()
                        .into_iter()
                        .filter(|(source, _)| *source == curation::DROPS)
                        .map(|(_, key)| (funnel::DROP_NOT_IN_CATALOG.to_string(), key.to_string())),
                )
                .collect(),
            previous,
        },
    );
    state.version = Some(version::next(
        projections::SCHEMA,
        was.as_deref(),
        report.diff.as_ref(),
        made_by.as_deref() != Some(recipe.as_str()),
    ));
    state.recipe = Some(recipe);
    Ok((report, state))
}

/// What this build is, for the `meta` table: the version it carries and the pinned snapshot
/// every source came from, so a catalog can always say what it was made of.
fn stamps(vault: &Vault, state: &State) -> Vec<(String, String)> {
    let mut out = vec![(
        "version".to_string(),
        state.version.clone().unwrap_or_default(),
    )];
    if let Ok(de) = vault.latest(spec::DE) {
        out.push(("fetched_ms".to_string(), de.created_ms.to_string()));
    }
    for (source, id) in sources(vault) {
        out.push((format!("source.{source}"), id));
    }
    out
}

/// The pinned snapshot every source is at. Published with a release too, so the next run can
/// ask whether anything moved without a vault to compare against.
pub fn sources(vault: &Vault) -> Vec<(String, String)> {
    SOURCES
        .iter()
        .filter_map(|source| {
            let snap = vault.latest(source).ok()?;
            Some(((*source).to_string(), snap.id))
        })
        .collect()
}

/// Render every artifact from the assembled graph.
fn project(vault: &Vault, out: &Path, built: &Built, report: &Report, state: &State) -> Result<()> {
    let textures = vault
        .latest(spec::DE)
        .and_then(|snap| blob(vault, &snap, spec::TEXTURES))
        .and_then(|raw| extract::de_textures(&raw))
        .unwrap_or_default();
    let cards = crate::icons::cards(&built.graph, &built.taxonomy, &built.wfm, &textures);
    let pinned = crate::icons::Pinned::open(
        vault,
        crate::icons::pictures(vault, &built.graph, &cards, &textures),
    );

    let policy = projections::load(Path::new(crate::SCOPE))?;
    let scope = projections::apply(&built.graph, &policy);
    eprintln!(
        "scope    {} shipped, {} held back",
        scope.in_scope(),
        scope.excluded()
    );
    for (reason, count) in scope.tally() {
        eprintln!("           {count:>6}  {reason}");
    }

    acquisition_tally(built, &scope);

    let out_dir = out.parent().filter(|p| !p.as_os_str().is_empty());
    let summaries = projections::run(
        out,
        &projections::Context {
            graph: &built.graph,
            taxonomy: &built.taxonomy,
            scope: &scope,
            report,
            conflicts: &built.conflicts,
            meta: &stamps(vault, state),
            out: out_dir.unwrap_or_else(|| Path::new(".")),
            icons: pinned.as_ref().map(|p| p as &dyn projections::IconSource),
        },
    )?;
    for s in &summaries {
        eprintln!("project  {:<8} {}", s.name, s.detail);
    }
    Ok(())
}

/// Assemble the graph from the vault, writing nothing.
pub fn graph(vault: &Vault) -> Result<Built> {
    graph_with(vault, &Curation::default())
}

/// Assemble the graph, honouring decisions made by hand.
pub fn graph_with(vault: &Vault, curated: &Curation) -> Result<Built> {
    let de_snap = vault.latest(spec::DE)?;
    let wfm_snap = vault.latest(spec::WFM)?;

    let mut de = Vec::new();
    let mut ru = BTreeMap::new();
    for name in spec::ITEM_MANIFESTS {
        de.extend(extract::de_items(name, &blob(vault, &de_snap, name)?)?);
        for item in extract::de_items(name, &blob(vault, &de_snap, &spec::ru(name))?)? {
            ru.insert(item.unique_name, item.name);
        }
    }

    // Inject Regal Aya
    de.push(extract::DeItem {
        unique_name: "/Lotus/Types/Items/MiscItems/PremiumSchismKey".to_string(),
        name: "Regal Aya".to_string(),
        category: "MiscItems".to_string(),
        array: "ExportResources".to_string(),
        parent: None,
        mod_type: None,
        mastery_req: None,
        max_level_cap: None,
    });
    ru.insert(
        "/Lotus/Types/Items/MiscItems/PremiumSchismKey".to_string(),
        "Королевская Айя".to_string(),
    );

    // Inject the Railjack plexus
    de.push(extract::DeItem {
        unique_name: PLEXUS.to_string(),
        name: "Plexus".to_string(),
        category: "CrewShipHarnesses".to_string(),
        array: "CrewShipHarnesses".to_string(),
        parent: None,
        mod_type: None,
        mastery_req: None,
        max_level_cap: None,
    });
    ru.insert(PLEXUS.to_string(), "Плексус".to_string());

    let recipes = extract::de_recipes(&blob(vault, &de_snap, spec::RECIPES)?)?;
    let rewards = extract::de_rewards(&blob(vault, &de_snap, spec::RELICS)?)?;

    let star_chart = extract::de_regions(&blob(vault, &de_snap, spec::REGIONS)?)?;
    let star_chart_ru = extract::de_regions(&blob(vault, &de_snap, &spec::ru(spec::REGIONS))?)?
        .into_iter()
        .map(|r| (r.node.clone(), r))
        .collect();
    let wfm = sources::wfm::parse(&blob(vault, &wfm_snap, spec::WFM_ITEMS)?)?;

    let wiki_snap = vault.latest(spec::WIKI)?;
    let chart = wiki::chart(&blob(vault, &wiki_snap, spec::WIKI_MISSIONS)?)?;
    let wiki_tables = wiki::tables(&blob(vault, &wiki_snap, spec::WIKI_DROPS)?)?;
    let baro = wiki::baro(&blob(vault, &wiki_snap, spec::WIKI_BARO)?)?;
    let dojo = wiki::dojo(&blob(vault, &wiki_snap, spec::WIKI_RESEARCH)?)?;
    let stores = wiki::vendors(&blob(vault, &wiki_snap, spec::WIKI_VENDORS)?)?;
    let market = wiki::market_blueprints(&blob(vault, &wiki_snap, spec::WIKI_BLUEPRINTS)?)?;

    let drop_snap = vault.latest(spec::DROPS)?;
    let tables = sources::drops::parse(&blob(vault, &drop_snap, spec::DROP_TABLES)?)?;
    if !tables.skipped.is_empty() {
        eprintln!("drop sections not read: {}", tables.skipped.join(", "));
    }

    let unread_headings = tables.unknown_headings.clone();

    let policy = taxonomy::load(Path::new(crate::TAXONOMY))?;
    let mastery = mastery::load(Path::new(crate::MASTERY), &policy.tree)?;
    let dead_rules = audit(&policy, &de, &recipes);

    let labels = regions::load(Path::new(crate::REGIONS))?;

    let mut built = assemble::assemble(
        Input {
            de,
            ru,
            recipes,
            rewards,
            regions: star_chart,
            regions_ru: star_chart_ru,
            chart,
            wiki_tables,
            baro,
            dojo,
            stores,
            market,
            wfm,
            drops: tables.drops,
            relic_rows: tables.relics,
            vaulting: wiki::vaulting(&blob(vault, &wiki_snap, spec::WIKI_VOID)?)?,
            composition: wiki::composition(&blob(vault, &wiki_snap, spec::WIKI_VOID)?)?,
        },
        policy,
        &mastery,
        &labels,
        &bounties::load(Path::new(crate::BOUNTIES))?,
        curated,
    );
    built.dead_rules = dead_rules;
    built.unread_headings = unread_headings;
    Ok(built)
}

pub fn blob(vault: &Vault, snap: &Snapshot, logical: &str) -> Result<Vec<u8>> {
    let entry = snap
        .entry(logical)
        .with_context(|| format!("snapshot {} missing {logical}", snap.id))?;
    vault.get(&BlobId::from_hex(entry.blob.clone()))
}
