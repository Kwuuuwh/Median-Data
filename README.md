# median-data

Builds the Warframe catalog that the [median](https://github.com/) desktop app reads: one
SQLite database and a pack of pictures, assembled from the game's own export and three public
sources, checked, and published as a release.

Nothing here is scraped by guesswork and nothing is hand-typed twice. Every fact carries the
source it came from, every disagreement between sources is recorded rather than averaged
away, and a rebuild from the same inputs produces a **byte-identical** file.

## What it produces

| artifact | size | what it is |
| --- | --- | --- |
| `catalog.sqlite` | ~50 MB | items, recipes, relics, drops, the star chart, vendors, dojo research, costs, a full-text index |
| `pack/` | ~270 MB | 12 600 lossless WebP pictures at 128 px, named by content hash |
| `catalog.report.json` | small | everything the checks found, by layer and rule |
| `catalog.changes.md` | small | what moved since the previous build |

The database says what it is: `PRAGMA user_version` is the schema number, and a `meta` table
carries the version, when the export was fetched, and the pinned snapshot id of every source
the build read.

## The pipeline

```
fetch      →  Data Vault  →  extractors  →  consensus  →  knowledge graph  →  funnel  →  projections
(network)     (pinned,        (pure)         (source        (typed nodes       (checks)   (sqlite, pack,
              content-                       priority,      and edges)                     search, costs)
              addressed)                     provenance)
```

* **Data Vault** — every byte a source ever returned, stored under its BLAKE3 hash and pinned
  into a snapshot. A build reads the vault, never the network, so it is reproducible and
  offline.
* **Consensus** — one property, several sources, one resolved value with its provenance and,
  where they disagree, a recorded conflict. No weights, no averaging.
* **Funnel** — six layers of checks: invariants (these stop the build), cross-source
  agreement, outliers, coverage, curated anchor facts, and the diff against the last build.
* **Projections** — each artifact is one implementation of a trait over the finished graph.

## Sources

| source | what it gives |
| --- | --- |
| Digital Extremes public export | items, recipes, relic rewards, the star chart, textures — in English and Russian |
| warframe.com drop tables | what drops where, with chances |
| warframe.market | tradability, market slugs, ducats, Russian names, mod card art |
| wiki.warframe.com | Lua data modules: Railjack nodes, vendors, Baro's history, clan research, vendor portraits |

The wiki is read as data modules through `action=raw`, never scraped from rendered HTML, and
it never outranks DE — it is a second witness and it fills what DE does not export at all.

## Running it

```sh
cargo run --release -p median-data -- fetch    # pin every source into the vault
cargo run --release -p median-data -- icons    # pin the pictures (needs a build first)
cargo run --release -p median-data -- build    # assemble, check, and write the artifacts
cargo run --release -p median-data -- studio   # curation UI on 127.0.0.1:8787
cargo run --release -p median-data -- show Q   # what the graph holds about one thing
```

A fresh clone has no vault, so `fetch` gets today's data — which is not the data any given
release was built from. Exact reproduction of a release needs that release's pinned sources.

## Curation

What a rule cannot decide, a person decides in Studio, and every decision lands in
`config/curation.toml` as reviewable text: a name tied to an item, a Russian word written by
hand, a value kept where sources disagreed, a finding accepted with the reason it is not a
defect. Curated values outrank every derived one, and the build reads them on every run.

The other `config/*.toml` files are declarative too — the taxonomy and its derivation rules,
what ships, the star chart's reference labels, bounty settlements, and the anchor facts the
build must keep reproducing.

## Versioning

`MAJOR.MINOR.PATCH`, and none of it is written by hand:

* **MAJOR** — the schema. An application reads it to decide whether it can open the file at
  all. It moves only when a table or a column does.
* **MINOR** — items arrived or left (a Prime Access, a game update).
* **PATCH** — everything else: translations, curation, metadata, chances.

MINOR and PATCH are read off the funnel's own diff against the previous build, so a build that
only fixes a translation cannot claim to have added anything, and a rebuild that changed
nothing keeps its version.

## Licence and the data

The code is MIT — see [LICENSE](LICENSE).

The data is not ours. Item names, statistics and artwork belong to Digital Extremes; vendor
portraits come from wiki.warframe.com. This is a non-commercial fan project under DE's
[Fan Content Policy](https://www.warframe.com/fan-kit), and the catalog it publishes is meant
for use with the game, not apart from it.
