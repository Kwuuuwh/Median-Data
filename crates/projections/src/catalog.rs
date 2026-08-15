use anyhow::Result;
use consensus::Resolved;
use graph::{Extra, Graph, Node, Rel};
use rusqlite::{Statement, Transaction, params};

use crate::projection::{Context, Projection, Summary};
use crate::search::FOLD;

/// The relational view the desktop app reads. Everything the graph holds lands here,
/// including what the scope policy keeps out of the other artifacts — nothing is lost
/// silently, it is only marked.
pub struct Catalog;

/// The shape of this database. It moves only when a table or a column does, and an
/// application reads it to decide whether it can open the file at all — so it lives here,
/// beside the schema it describes, and is written both as `PRAGMA user_version` and as a row
/// of `meta`.
pub const SCHEMA: u32 = 9;

pub const SETUP: &str = "\
CREATE TABLE meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
) WITHOUT ROWID;
CREATE TABLE items (
  unique_name TEXT PRIMARY KEY,
  name_en     TEXT NOT NULL,
  name_ru     TEXT,
  category    TEXT NOT NULL,
  class       TEXT NOT NULL,
  kind        TEXT NOT NULL,
  slug        TEXT,
  tradable    INTEGER,
  vaulted     INTEGER,
  prime       INTEGER NOT NULL,
  ducats      INTEGER,
  in_scope    INTEGER NOT NULL
) WITHOUT ROWID;
CREATE TABLE classes (
  slug    TEXT PRIMARY KEY,
  name_en TEXT NOT NULL,
  name_ru TEXT NOT NULL,
  sourced INTEGER NOT NULL,
  ord     INTEGER NOT NULL
) WITHOUT ROWID;
CREATE TABLE kinds (
  slug    TEXT PRIMARY KEY,
  class   TEXT NOT NULL,
  name_en TEXT NOT NULL,
  name_ru TEXT NOT NULL,
  ord     INTEGER NOT NULL
) WITHOUT ROWID;
CREATE TABLE out_of_scope (
  unique_name TEXT PRIMARY KEY,
  reason      TEXT NOT NULL
) WITHOUT ROWID;
CREATE TABLE recipes (
  blueprint   TEXT PRIMARY KEY,
  result      TEXT NOT NULL,
  build_price INTEGER,
  build_time  INTEGER,
  consumed    INTEGER NOT NULL,
  rush_price  INTEGER
) WITHOUT ROWID;
CREATE TABLE recipe_requires (
  blueprint TEXT NOT NULL,
  item      TEXT NOT NULL,
  count     INTEGER NOT NULL,
  PRIMARY KEY (blueprint, item)
) WITHOUT ROWID;
CREATE TABLE relic_rewards (
  relic  TEXT NOT NULL,
  reward TEXT NOT NULL,
  rarity TEXT NOT NULL,
  PRIMARY KEY (relic, reward)
) WITHOUT ROWID;
CREATE TABLE relics (
  unique_name TEXT PRIMARY KEY,
  base        TEXT NOT NULL,
  refinement  TEXT NOT NULL,
  vaulted_in  TEXT
) WITHOUT ROWID;
CREATE TABLE relic_chances (
  rarity     TEXT NOT NULL,
  refinement TEXT NOT NULL,
  chance     REAL NOT NULL,
  PRIMARY KEY (rarity, refinement)
) WITHOUT ROWID;
CREATE TABLE sets (
  slug    TEXT PRIMARY KEY,
  name_en TEXT NOT NULL,
  name_ru TEXT,
  ducats  INTEGER,
  vaulted INTEGER,
  built   TEXT
) WITHOUT ROWID;
CREATE TABLE item_drifters (
  operator TEXT NOT NULL,
  drifter  TEXT NOT NULL,
  PRIMARY KEY (operator, drifter)
) WITHOUT ROWID;
CREATE TABLE item_primes (
  plain TEXT NOT NULL,
  prime TEXT NOT NULL,
  PRIMARY KEY (plain, prime)
) WITHOUT ROWID;
CREATE TABLE set_members (
  slug TEXT NOT NULL,
  item TEXT NOT NULL,
  PRIMARY KEY (slug, item)
) WITHOUT ROWID;
CREATE TABLE imprints (
  slug    TEXT PRIMARY KEY,
  name_en TEXT NOT NULL,
  name_ru TEXT,
  animal  TEXT NOT NULL
) WITHOUT ROWID;
CREATE TABLE places (
  name     TEXT PRIMARY KEY,
  name_ru  TEXT,
  kind     TEXT NOT NULL,
  region   TEXT,
  location TEXT,
  node     TEXT,
  label    TEXT,
  extra    INTEGER,
  event    INTEGER
) WITHOUT ROWID;
CREATE TABLE enemies (
  name    TEXT PRIMARY KEY,
  name_ru TEXT
) WITHOUT ROWID;
CREATE TABLE locations (
  name    TEXT PRIMARY KEY,
  name_ru TEXT,
  kind    TEXT
) WITHOUT ROWID;
CREATE TABLE bounty_places (
  place          TEXT PRIMARY KEY,
  settlement     TEXT NOT NULL,
  settlement_ru  TEXT,
  giver          TEXT,
  giver_ru       TEXT,
  min_level      INTEGER NOT NULL,
  max_level      INTEGER NOT NULL,
  activity       TEXT NOT NULL,
  activity_ru    TEXT
) WITHOUT ROWID;
CREATE TABLE labs (
  key     TEXT PRIMARY KEY,
  name    TEXT NOT NULL,
  name_ru TEXT,
  faction TEXT NOT NULL
) WITHOUT ROWID;
CREATE TABLE research (
  lab       TEXT NOT NULL,
  blueprint TEXT NOT NULL,
  credits   INTEGER NOT NULL,
  time      INTEGER NOT NULL,
  affinity  INTEGER NOT NULL,
  prereq    TEXT,
  PRIMARY KEY (lab, blueprint)
) WITHOUT ROWID;
CREATE TABLE research_costs (
  blueprint TEXT NOT NULL,
  resource  TEXT NOT NULL,
  count     INTEGER NOT NULL,
  PRIMARY KEY (blueprint, resource)
) WITHOUT ROWID;
CREATE TABLE vendors (
  key      TEXT PRIMARY KEY,
  name     TEXT NOT NULL,
  name_ru  TEXT,
  currency TEXT,
  kind     TEXT,
  rotates  INTEGER NOT NULL
) WITHOUT ROWID;
CREATE TABLE vendor_offers (
  vendor   TEXT NOT NULL,
  item     TEXT NOT NULL,
  cost     INTEGER,
  currency TEXT,
  store    TEXT,
  credits  INTEGER,
  count   INTEGER NOT NULL,
  rank    INTEGER,
  timer   INTEGER,
  times   INTEGER NOT NULL,
  always  INTEGER NOT NULL,
  gone    INTEGER NOT NULL,
  PRIMARY KEY (vendor, item)
) WITHOUT ROWID;
CREATE TABLE regions (
  node         TEXT PRIMARY KEY,
  name         TEXT NOT NULL,
  name_ru      TEXT,
  location     TEXT NOT NULL,
  mission      INTEGER NOT NULL,
  mission_en   TEXT,
  mission_ru   TEXT,
  faction      INTEGER NOT NULL,
  faction_en   TEXT,
  faction_ru   TEXT,
  faction_icon TEXT,
  node_type    INTEGER NOT NULL,
  node_type_en TEXT,
  node_type_ru TEXT,
  mastery      INTEGER NOT NULL,
  min_level    INTEGER NOT NULL,
  max_level    INTEGER NOT NULL,
  tileset      TEXT,
  tileset_ru   TEXT,
  origin       TEXT NOT NULL,
  railjack     INTEGER NOT NULL,
  hidden       INTEGER NOT NULL
) WITHOUT ROWID;
CREATE TABLE item_drops (
  place    TEXT NOT NULL,
  item     TEXT NOT NULL,
  rotation TEXT,
  stage    TEXT,
  rarity   TEXT NOT NULL,
  chance   REAL NOT NULL,
  count    INTEGER
);
CREATE TABLE enemy_drops (
  enemy        TEXT NOT NULL,
  item         TEXT NOT NULL,
  rarity       TEXT NOT NULL,
  chance       REAL NOT NULL,
  table_chance REAL,
  min_level    INTEGER,
  max_level    INTEGER,
  count        INTEGER
);
CREATE TABLE provenance (
  unique_name TEXT NOT NULL,
  prop        TEXT NOT NULL,
  status      TEXT NOT NULL,
  winner      TEXT NOT NULL,
  sources     TEXT NOT NULL,
  PRIMARY KEY (unique_name, prop)
) WITHOUT ROWID;
CREATE TABLE conflicts (
  unique_name TEXT NOT NULL,
  prop        TEXT NOT NULL,
  chosen      TEXT NOT NULL,
  detail      TEXT NOT NULL,
  PRIMARY KEY (unique_name, prop)
) WITHOUT ROWID;
CREATE TABLE conflict_claims (
  unique_name TEXT NOT NULL,
  prop        TEXT NOT NULL,
  source      TEXT NOT NULL,
  value       TEXT NOT NULL,
  PRIMARY KEY (unique_name, prop, source)
) WITHOUT ROWID;
CREATE TABLE findings (
  layer  TEXT NOT NULL,
  rule   TEXT NOT NULL,
  entity TEXT NOT NULL,
  detail TEXT NOT NULL
);
CREATE INDEX idx_items_category ON items(category);
CREATE INDEX idx_items_class ON items(class);
CREATE INDEX idx_items_kind ON items(kind);
CREATE INDEX idx_items_scope ON items(in_scope);
CREATE INDEX idx_kinds_class ON kinds(class);
CREATE INDEX idx_relics_base ON relics(base);
CREATE INDEX idx_item_drops_item ON item_drops(item);
CREATE INDEX idx_item_drops_place ON item_drops(place);
CREATE INDEX idx_enemy_drops_item ON enemy_drops(item);
CREATE INDEX idx_enemy_drops_enemy ON enemy_drops(enemy);
CREATE INDEX idx_recipe_requires_item ON recipe_requires(item);
CREATE INDEX idx_relic_rewards_reward ON relic_rewards(reward);
CREATE INDEX idx_places_region ON places(region);
CREATE INDEX idx_vendor_offers_item ON vendor_offers(item);
CREATE INDEX idx_research_blueprint ON research(blueprint);
CREATE INDEX idx_regions_location ON regions(location);
CREATE INDEX idx_set_members_item ON set_members(item);
CREATE INDEX idx_provenance_status ON provenance(status);
CREATE INDEX idx_findings_layer ON findings(layer);
CREATE INDEX idx_findings_entity ON findings(entity);";

impl Projection for Catalog {
    fn name(&self) -> &'static str {
        "catalog"
    }

    fn db(&self, tx: &Transaction<'_>, ctx: &Context<'_>) -> Result<Option<Summary>> {
        // What this file is, written first so a reader can decide whether to read the rest.
        tx.execute_batch(SETUP)?;
        stamp(tx, ctx)?;
        reference(tx)?;
        tree(tx, ctx)?;
        let items = nodes(tx, ctx)?;
        edges(tx, ctx.graph)?;
        review(tx, ctx)?;

        Ok(Some(Summary {
            name: self.name(),
            detail: format!("{items} items, {} edges", ctx.graph.edge_count()),
        }))
    }
}

/// Say what this build is: its schema, the rule its names are folded by, its version, and
/// the pinned snapshots behind it.
fn stamp(tx: &Transaction<'_>, ctx: &Context<'_>) -> Result<()> {
    let mut insert = tx.prepare("INSERT INTO meta (key, value) VALUES (?1, ?2)")?;
    insert.execute(("schema", SCHEMA.to_string()))?;
    insert.execute(("fold", FOLD))?;
    for (key, value) in ctx.meta {
        insert.execute((key, value))?;
    }
    Ok(())
}

fn reference(tx: &Transaction<'_>) -> Result<()> {
    let mut chances =
        tx.prepare("INSERT INTO relic_chances (rarity, refinement, chance) VALUES (?1, ?2, ?3)")?;
    for (rarity, refinement, chance) in graph::CHANCES {
        chances.execute((rarity, refinement, chance))?;
    }
    Ok(())
}

/// The taxonomy itself, so the app groups items without hardcoding the tree.
fn tree(tx: &Transaction<'_>, ctx: &Context<'_>) -> Result<()> {
    let mut classes = tx.prepare(
        "INSERT INTO classes (slug, name_en, name_ru, sourced, ord) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    let mut kinds = tx.prepare(
        "INSERT INTO kinds (slug, class, name_en, name_ru, ord) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for (ci, class) in ctx.taxonomy.classes().iter().enumerate() {
        classes.execute((
            &class.slug,
            &class.en,
            &class.ru,
            class.sourced as i64,
            ci as i64,
        ))?;
        for (li, leaf) in class.kind.iter().enumerate() {
            kinds.execute((&leaf.slug, &class.slug, &leaf.en, &leaf.ru, li as i64))?;
        }
    }
    Ok(())
}

fn nodes(tx: &Transaction<'_>, ctx: &Context<'_>) -> Result<usize> {
    let graph = ctx.graph;
    let mut items = tx.prepare(
        "INSERT INTO items \
         (unique_name, name_en, name_ru, category, class, kind, slug, tradable, vaulted, \
          prime, ducats, in_scope) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    )?;
    let mut dropped =
        tx.prepare("INSERT INTO out_of_scope (unique_name, reason) VALUES (?1, ?2)")?;
    let mut prov = tx.prepare(
        "INSERT INTO provenance (unique_name, prop, status, winner, sources) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    let mut sets = tx.prepare(
        "INSERT INTO sets (slug, name_en, name_ru, ducats, vaulted, built) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    let mut recipes = tx.prepare(
        "INSERT INTO recipes (blueprint, result, build_price, build_time, consumed, rush_price) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    let mut relics = tx.prepare(
        "INSERT INTO relics (unique_name, base, refinement, vaulted_in) \
                    VALUES (?1, ?2, ?3, ?4)",
    )?;
    let mut imprints = tx
        .prepare("INSERT INTO imprints (slug, name_en, name_ru, animal) VALUES (?1, ?2, ?3, ?4)")?;
    let mut places = tx.prepare(
        "INSERT INTO places (name, name_ru, kind, region, location, node, label, \
                    extra, event) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;
    let mut enemies = tx.prepare("INSERT INTO enemies (name, name_ru) VALUES (?1, ?2)")?;
    let mut locations =
        tx.prepare("INSERT INTO locations (name, name_ru, kind) VALUES (?1, ?2, ?3)")?;
    let mut vendors = tx.prepare(
        "INSERT INTO vendors (key, name, name_ru, currency, kind, rotates) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    let mut labs =
        tx.prepare("INSERT INTO labs (key, name, name_ru, faction) VALUES (?1, ?2, ?3, ?4)")?;
    let mut bounties = tx.prepare(
        "INSERT INTO bounty_places (place, settlement, settlement_ru, giver, giver_ru, \
         min_level, max_level, activity, activity_ru) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;
    let mut regions = tx.prepare(
        "INSERT INTO regions (node, name, name_ru, location, mission, mission_en, \
         mission_ru, faction, faction_en, faction_ru, faction_icon, node_type, node_type_en, \
         node_type_ru, mastery, min_level, max_level, tileset, tileset_ru, origin, railjack, \
         hidden) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, \
         ?18, ?19, ?20, ?21, ?22)",
    )?;

    let mut count = 0;
    for node in graph.nodes() {
        match node {
            Node::Item(it) => {
                let kept = ctx.scope.allows(&it.unique_name);
                items.execute((
                    &it.unique_name,
                    &it.names.en.value,
                    it.names.ru.as_ref().map(|r| r.value.clone()),
                    &it.category.value,
                    ctx.taxonomy.class_slug(&it.kind.value),
                    it.kind.value.as_str(),
                    it.slug.as_ref().map(|r| r.value.clone()),
                    it.tradable.as_ref().map(|r| r.value as i64),
                    it.vaulted.as_ref().map(|r| r.value as i64),
                    it.prime.value as i64,
                    it.ducats,
                    kept as i64,
                ))?;
                if let Some(reason) = ctx.scope.reason(&it.unique_name) {
                    dropped.execute((&it.unique_name, reason))?;
                }
                put(&mut prov, &it.unique_name, "name_en", &it.names.en)?;
                if let Some(ru) = &it.names.ru {
                    put(&mut prov, &it.unique_name, "name_ru", ru)?;
                }
                put(&mut prov, &it.unique_name, "category", &it.category)?;
                put(&mut prov, &it.unique_name, "kind", &it.kind)?;
                put(&mut prov, &it.unique_name, "prime", &it.prime)?;
                if let Some(slug) = &it.slug {
                    put(&mut prov, &it.unique_name, "slug", slug)?;
                }
                if let Some(tradable) = &it.tradable {
                    put(&mut prov, &it.unique_name, "tradable", tradable)?;
                }
                if let Some(vaulted) = &it.vaulted {
                    put(&mut prov, &it.unique_name, "vaulted", vaulted)?;
                }
                if let Extra::Relic(r) = &it.extra {
                    relics.execute((
                        &it.unique_name,
                        &r.base,
                        &r.refinement,
                        r.vaulted_in.as_deref(),
                    ))?;
                }
                count += 1;
            }
            Node::Set(s) => {
                sets.execute((
                    &s.slug,
                    &s.names.en.value,
                    s.names.ru.as_ref().map(|r| r.value.clone()),
                    s.ducats,
                    s.vaulted.as_ref().map(|r| r.value as i64),
                    target(graph, &node.id(), &Rel::Represents),
                ))?;
            }
            Node::Imprint(i) => {
                imprints.execute((
                    &i.slug,
                    &i.names.en.value,
                    i.names.ru.as_ref().map(|r| r.value.clone()),
                    &i.animal,
                ))?;
            }
            Node::Place(p) => {
                let region = target(graph, &node.id(), &Rel::At)
                    .map(|id| id.trim_start_matches("region:").to_string());
                places.execute((
                    &p.name,
                    p.name_ru.as_deref(),
                    p.kind.as_str(),
                    region,
                    p.table.as_ref().map(|t| t.location.clone()),
                    p.table.as_ref().map(|t| t.node.clone()),
                    p.table.as_ref().map(|t| t.label.clone()),
                    p.table.as_ref().map(|t| t.extra as i64),
                    p.table.as_ref().map(|t| t.event as i64),
                ))?;
                if let Some(b) = &p.bounty {
                    bounties.execute((
                        &p.name,
                        &b.settlement,
                        b.settlement_ru.as_deref(),
                        b.giver.as_deref(),
                        b.giver_ru.as_deref(),
                        b.min_level,
                        b.max_level,
                        &b.activity,
                        b.activity_ru.as_deref(),
                    ))?;
                }
            }
            Node::Enemy(e) => {
                enemies.execute((&e.name, e.name_ru.as_deref()))?;
            }
            Node::Location(l) => {
                locations.execute((&l.name, l.name_ru.as_deref(), l.kind.as_deref()))?;
            }
            Node::Vendor(v) => {
                vendors.execute((
                    &v.key,
                    &v.name,
                    v.name_ru.as_deref(),
                    v.currency.as_deref(),
                    v.kind.as_deref(),
                    v.rotates as i64,
                ))?;
            }
            Node::Lab(l) => {
                labs.execute((&l.key, &l.name, l.name_ru.as_deref(), &l.faction))?;
            }
            Node::Region(r) => {
                regions.execute(params![
                    &r.node,
                    &r.name,
                    r.name_ru.as_deref(),
                    &r.location,
                    r.mission,
                    r.mission_label.en.as_deref(),
                    r.mission_label.ru.as_deref(),
                    r.faction,
                    r.faction_label.en.as_deref(),
                    r.faction_label.ru.as_deref(),
                    r.faction_label.icon.as_deref(),
                    r.node_type,
                    r.type_label.en.as_deref(),
                    r.type_label.ru.as_deref(),
                    r.mastery,
                    r.min_level,
                    r.max_level,
                    r.tileset.en.as_deref(),
                    r.tileset.ru.as_deref(),
                    r.origin.as_str(),
                    r.railjack as i64,
                    r.hidden as i64,
                ])?;
            }
            Node::Recipe(r) => {
                if let Some(result) = target(graph, &node.id(), &Rel::Produces) {
                    recipes.execute((
                        &r.blueprint,
                        &result,
                        r.build_price,
                        r.build_time,
                        r.consumed as i64,
                        r.rush_price,
                    ))?;
                }
            }
        }
    }
    Ok(count)
}

fn edges(tx: &Transaction<'_>, graph: &Graph) -> Result<()> {
    let mut requires = tx.prepare(
        "INSERT OR IGNORE INTO recipe_requires (blueprint, item, count) VALUES (?1, ?2, ?3)",
    )?;
    let mut rewards = tx.prepare(
        "INSERT OR IGNORE INTO relic_rewards (relic, reward, rarity) VALUES (?1, ?2, ?3)",
    )?;
    let mut members =
        tx.prepare("INSERT OR IGNORE INTO set_members (slug, item) VALUES (?1, ?2)")?;
    let mut primes =
        tx.prepare("INSERT OR IGNORE INTO item_primes (plain, prime) VALUES (?1, ?2)")?;
    let mut drifters =
        tx.prepare("INSERT OR IGNORE INTO item_drifters (operator, drifter) VALUES (?1, ?2)")?;
    let mut drops = tx.prepare(
        "INSERT INTO item_drops (place, item, rotation, stage, rarity, chance, count) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    let mut kills = tx.prepare(
        "INSERT INTO enemy_drops \
         (enemy, item, rarity, chance, table_chance, min_level, max_level, count) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?;
    let mut research = tx.prepare(
        "INSERT OR IGNORE INTO research (lab, blueprint, credits, time, affinity, prereq) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    let mut research_cost = tx.prepare(
        "INSERT OR IGNORE INTO research_costs (blueprint, resource, count) VALUES (?1, ?2, ?3)",
    )?;
    let mut offers = tx.prepare(
        "INSERT OR IGNORE INTO vendor_offers \
         (vendor, item, cost, currency, store, credits, count, rank, timer, times, always, gone) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    )?;

    for edge in sorted(graph) {
        match &edge.rel {
            Rel::Requires { count } => {
                requires.execute((strip(&edge.from, "recipe:"), &edge.to, count))?;
            }
            Rel::Rewards { rarity } => {
                rewards.execute((&edge.from, &edge.to, rarity))?;
            }
            Rel::Member => {
                members.execute((strip(&edge.from, "set:"), &edge.to))?;
            }
            Rel::Primed => {
                primes.execute((&edge.from, &edge.to))?;
            }
            Rel::Fits => {
                drifters.execute((&edge.from, &edge.to))?;
            }
            Rel::Drops(d) => match edge.from.strip_prefix("enemy:") {
                Some(enemy) => {
                    kills.execute((
                        enemy,
                        &edge.to,
                        &d.rarity,
                        d.chance,
                        d.table_chance,
                        d.levels.map(|l| l.min),
                        d.levels.map(|l| l.max),
                        d.count,
                    ))?;
                }
                None => {
                    drops.execute((
                        strip(&edge.from, "place:"),
                        &edge.to,
                        d.rotation.as_deref(),
                        d.stage.as_deref(),
                        &d.rarity,
                        d.chance,
                        d.count,
                    ))?;
                }
            },
            Rel::Researched(r) => {
                research.execute((
                    strip(&edge.from, "lab:"),
                    &edge.to,
                    r.credits,
                    r.time,
                    r.affinity,
                    r.prereq.as_deref(),
                ))?;
                for (resource, count) in &r.resources {
                    research_cost.execute((&edge.to, resource, count))?;
                }
            }
            Rel::Sells(offer) => {
                offers.execute(params![
                    strip(&edge.from, "vendor:"),
                    &edge.to,
                    offer.cost,
                    offer.currency.as_deref(),
                    offer.store.as_deref(),
                    offer.credits,
                    offer.count,
                    offer.rank,
                    offer.timer,
                    offer.times as i64,
                    offer.always as i64,
                    offer.gone as i64,
                ])?;
            }
            // Produces, Represents, Yields and At are read back from a node column, and the
            // refinement chain is already in `relics` as base plus refinement.
            Rel::Produces | Rel::Represents | Rel::Yields | Rel::At | Rel::Refines => {}
        }
    }
    Ok(())
}

fn review(tx: &Transaction<'_>, ctx: &Context<'_>) -> Result<()> {
    let mut conf = tx.prepare(
        "INSERT OR IGNORE INTO conflicts (unique_name, prop, chosen, detail) \
         VALUES (?1, ?2, ?3, ?4)",
    )?;
    let mut said = tx.prepare(
        "INSERT OR IGNORE INTO conflict_claims (unique_name, prop, source, value) \
         VALUES (?1, ?2, ?3, ?4)",
    )?;
    for c in ctx.conflicts {
        conf.execute((&c.entity, c.prop, &c.chosen, c.detail()))?;
        for (source, value) in &c.claims {
            said.execute((&c.entity, c.prop, source.as_str(), value))?;
        }
    }

    let mut found =
        tx.prepare("INSERT INTO findings (layer, rule, entity, detail) VALUES (?1, ?2, ?3, ?4)")?;
    for f in &ctx.report.findings {
        found.execute((f.layer.as_str(), &f.rule, &f.entity, &f.detail))?;
    }
    Ok(())
}

/// Edges in a stable order, so the artifact is byte-identical across builds.
fn sorted(graph: &Graph) -> Vec<&graph::Edge> {
    let mut edges: Vec<&graph::Edge> = graph.edges().collect();
    edges.sort_by(|a, b| (&a.from, a.rel.as_str(), &a.to).cmp(&(&b.from, b.rel.as_str(), &b.to)));
    edges
}

/// The single node a relation points at, if any.
fn target(graph: &Graph, id: &str, rel: &Rel) -> Option<String> {
    graph
        .from(id)
        .into_iter()
        .find(|e| e.rel.as_str() == rel.as_str())
        .map(|e| e.to.clone())
}

fn put<T>(stmt: &mut Statement<'_>, uid: &str, prop: &str, r: &Resolved<T>) -> Result<()> {
    let sources: Vec<&str> = r.sources.iter().map(|s| s.as_str()).collect();
    stmt.execute((
        uid,
        prop,
        r.status.as_str(),
        r.winner.as_str(),
        sources.join(","),
    ))?;
    Ok(())
}

fn strip<'a>(id: &'a str, prefix: &str) -> &'a str {
    id.strip_prefix(prefix).unwrap_or(id)
}
