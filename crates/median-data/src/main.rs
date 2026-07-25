use std::path::Path;
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
mod drops;
mod extract;
mod fetch;
mod icons;
mod imprints;
mod inspect;
mod labs;
mod merge;
mod names;
mod normalize;
mod orphans;
mod paths;
mod portraits;
mod primes;
mod regions;
mod relic;
mod rules;
mod sets;
mod show;
mod spec;
mod taxonomy;
mod unmatched;
mod vendors;
mod version;
mod wiki;
mod witness;

const VAULT_DIR: &str = ".data_vault";
const OUT: &str = "catalog.sqlite";
const SCOPE: &str = "config/scope.toml";
const TAXONOMY: &str = "config/taxonomy.toml";
const REGIONS: &str = "config/regions.toml";
const BOUNTIES: &str = "config/bounties.toml";
const ANCHORS: &str = "config/anchors.toml";
const CURATION: &str = "config/curation.toml";
const STUDIO_ADDR: &str = "127.0.0.1:8787";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("fetch") => fetch::run(&Vault::open(VAULT_DIR)?, now_ms()),
        Some("build") => build::run(&Vault::open(VAULT_DIR)?, Path::new(OUT)),
        Some("icons") => icons::run(&Vault::open(VAULT_DIR)?, Path::new(SCOPE), now_ms()),
        Some("studio") => inspect::run(VAULT_DIR, STUDIO_ADDR),
        Some("show") => {
            let query = args.next().unwrap_or_default();
            let built = build::graph(&Vault::open(VAULT_DIR)?)?;
            show::run(&built.graph, &query);
            Ok(())
        }
        cmd => {
            eprintln!("usage: median-data <fetch|icons|build|studio|show QUERY>");
            anyhow::bail!("unknown command: {}", cmd.unwrap_or("(none)"));
        }
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
