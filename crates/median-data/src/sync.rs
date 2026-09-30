use std::path::Path;

use anyhow::Result;
use vault::Vault;

use crate::{build, fetch, game, icons};

/// Everything the PC does to the data, in order: distil the game cache, ask every source
/// whether it moved, and only then pin, draw and build.
pub fn run(vault: &Vault, cache: &Path, now_ms: i64) -> Result<()> {
    let game_dir = Path::new(crate::GAME);
    let was = game::stamp(game_dir).ok();
    game::run(cache, game_dir)?;
    let now = game::stamp(game_dir)?;
    match was.as_deref() == Some(now.as_str()) {
        true => eprintln!("game     unchanged"),
        false => eprintln!(
            "game     moved — {} -> {now}",
            was.as_deref().unwrap_or("none")
        ),
    }

    let changes = fetch::changes(vault, None, Path::new(crate::STATE))?;
    let moved: Vec<&fetch::Change> = changes.iter().filter(|c| c.moved()).collect();
    let built = Path::new(crate::OUT).is_file();
    if moved.is_empty() && built {
        eprintln!("nothing changed — {} is up to date", crate::OUT);
        return Ok(());
    }

    for change in &moved {
        if change.source != fetch::RECIPE {
            fetch::one(vault, change.source, now_ms)?;
        }
    }
    icons::run(vault, Path::new(crate::SCOPE), now_ms)?;
    build::run(vault, Path::new(crate::OUT))?;
    eprintln!("built    {}", crate::OUT);
    Ok(())
}
