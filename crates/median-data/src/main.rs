use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use vault::Vault;

mod assemble;
mod bounties;
mod bridge;
mod build;
mod craft;
mod curation;
mod drifters;
mod drops;
mod extract;
mod fetch;
mod icons;
mod imprints;
mod inspect;
mod labs;
mod mastery;
mod merge;
mod names;
mod normalize;
mod orphans;
mod paths;
mod portraits;
mod primes;
mod regions;
mod release;
mod relic;
mod rules;
mod sets;
mod show;
mod spec;
mod taxonomy;
mod unmatched;
mod vaulting;
mod vendors;
mod version;
mod wiki;
mod witness;

const VAULT_DIR: &str = ".data_vault";
const OUT: &str = "catalog.sqlite";
const SCOPE: &str = "config/scope.toml";
const TAXONOMY: &str = "config/taxonomy.toml";
const MASTERY: &str = "config/mastery.toml";
const REGIONS: &str = "config/regions.toml";
const BOUNTIES: &str = "config/bounties.toml";
const ANCHORS: &str = "config/anchors.toml";
const CURATION: &str = "config/curation.toml";
const PACK: &str = "pack";
const STATE: &str = "catalog.state.json";
const STUDIO_ADDR: &str = "127.0.0.1:8787";

/// `check` reports a moved source with this exit code, so a scheduled run can decide whether
/// the expensive steps are worth running without parsing any output.
const CHANGED: u8 = 2;

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("check") => {
            let released = args.next().map(PathBuf::from);
            let moved = fetch::changed(&Vault::open(VAULT_DIR)?, released.as_deref())?;
            Ok(match moved {
                true => ExitCode::from(CHANGED),
                false => ExitCode::SUCCESS,
            })
        }
        Some("fetch") => {
            let vault = Vault::open(VAULT_DIR)?;
            done(match args.next() {
                Some(source) => fetch::one(&vault, &source, now_ms()),
                None => fetch::run(&vault, now_ms()),
            })
        }
        Some("build") => done(build::run(&Vault::open(VAULT_DIR)?, Path::new(OUT))),
        Some("icons") => done(icons::run(
            &Vault::open(VAULT_DIR)?,
            Path::new(SCOPE),
            now_ms(),
        )),
        Some("release") => {
            let previous = args.next().map(PathBuf::from);
            let vault = Vault::open(VAULT_DIR)?;
            done(release::run(
                Path::new(OUT),
                Path::new(PACK),
                Path::new(STATE),
                release::Stamp {
                    schema: projections::SCHEMA,
                    fetched_ms: vault.latest(spec::DE).map(|s| s.created_ms).unwrap_or(0),
                    sources: build::sources(&vault).into_iter().collect(),
                },
                previous.as_deref(),
            ))
        }
        Some("studio") => {
            let addr = args.next().unwrap_or_else(|| STUDIO_ADDR.to_string());
            done(inspect::run(VAULT_DIR, &addr))
        }
        Some("show") => {
            let query = args.next().unwrap_or_default();
            let built = build::graph(&Vault::open(VAULT_DIR)?)?;
            show::run(&built.graph, &query);
            Ok(ExitCode::SUCCESS)
        }
        cmd => {
            eprintln!(
                "usage: median-data <check [MANIFEST]|fetch [SOURCE]|icons|build|release [PREV]|\
                 studio [ADDR]|show QUERY>"
            );
            anyhow::bail!("unknown command: {}", cmd.unwrap_or("(none)"));
        }
    }
}

fn done(result: Result<()>) -> Result<ExitCode> {
    result.map(|()| ExitCode::SUCCESS)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
