use std::fmt::Write;

use anyhow::Result;

use crate::projection::{Context, Projection, Summary};

/// A readable account of what moved since the last build, for review before publishing.
pub struct Changes;

const FILE: &str = "catalog.changes.md";
/// Long lists say nothing a sample does not; the full set lives in the report.
const SHOWN: usize = 40;
/// How many sources of one move are worth printing.
const SOURCES: usize = 6;

/// Items whose sources moved: what dropped them before, and what does now. A thing that used
/// to come off one boss and now falls off ordinary enemies is worth less, and nothing else
/// in the build would say so.
fn moved(page: &mut String, ctx: &Context<'_>, items: &[funnel::Moved]) -> Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    writeln!(page, "## Dropped differently\n")?;
    for item in items.iter().take(SHOWN) {
        writeln!(page, "- `{}`{}", item.item, named(ctx, &item.item))?;
        part(page, "now drops from", &item.added)?;
        part(page, "no longer drops from", &item.gone)?;
        part(page, "chance moved", &item.rechanced)?;
    }
    if items.len() > SHOWN {
        writeln!(page, "- … and {} more", items.len() - SHOWN)?;
    }
    writeln!(page)?;
    Ok(())
}

/// The item's printed name, where the graph holds it.
fn named(ctx: &Context<'_>, path: &str) -> String {
    match ctx.graph.get(path) {
        Some(graph::Node::Item(item)) => format!(" — {}", item.names.en.value),
        _ => String::new(),
    }
}

/// One side of a move, listed short: a long list says nothing a sample does not.
fn part(page: &mut String, what: &str, sources: &[String]) -> Result<()> {
    if sources.is_empty() {
        return Ok(());
    }
    let shown: Vec<&str> = sources.iter().take(SOURCES).map(String::as_str).collect();
    let rest = match sources.len() > SOURCES {
        true => format!(" … and {} more", sources.len() - SOURCES),
        false => String::new(),
    };
    writeln!(page, "  - {what}: {}{rest}", shown.join("; "))?;
    Ok(())
}

impl Projection for Changes {
    fn name(&self) -> &'static str {
        "changes"
    }

    fn file(&self, ctx: &Context<'_>) -> Result<Option<Summary>> {
        let path = ctx.out.join(FILE);
        let mut page = String::new();
        let totals = &ctx.report.totals;

        writeln!(page, "# Catalog changes\n")?;
        writeln!(
            page,
            "{} items, {} places, {} edges, {} conflicts.\n",
            totals.items, totals.places, totals.edges, totals.conflicts
        )?;

        let detail = match &ctx.report.diff {
            None => {
                writeln!(page, "First build — nothing to compare against.\n")?;
                "first build".to_string()
            }
            Some(diff) if diff.is_empty() => {
                writeln!(page, "Nothing changed since the last build.\n")?;
                "no changes".to_string()
            }
            Some(diff) => {
                list(&mut page, "Items added", &diff.items_added)?;
                list(&mut page, "Items removed", &diff.items_removed)?;
                list(&mut page, "Sets added", &diff.sets_added)?;
                list(&mut page, "Sets removed", &diff.sets_removed)?;

                moved(&mut page, ctx, &diff.drops_changed)?;

                if !diff.findings_delta.is_empty() {
                    writeln!(page, "## Findings\n")?;
                    for (rule, delta) in &diff.findings_delta {
                        writeln!(page, "- `{rule}` {delta:+}")?;
                    }
                    writeln!(page)?;
                }
                match diff.drops_changed.is_empty() {
                    true => format!(
                        "items +{} -{}",
                        diff.items_added.len(),
                        diff.items_removed.len()
                    ),
                    false => format!(
                        "items +{} -{}, {} dropped differently",
                        diff.items_added.len(),
                        diff.items_removed.len(),
                        diff.drops_changed.len()
                    ),
                }
            }
        };

        writeln!(page, "## Review queue\n")?;
        for (rule, count) in ctx.report.by_rule() {
            writeln!(page, "- `{rule}` — {count}")?;
        }

        let tally = ctx.scope.tally();
        if !tally.is_empty() {
            writeln!(page, "\n## Out of scope\n")?;
            for (reason, count) in tally {
                writeln!(page, "- {reason} — {count}")?;
            }
        }

        std::fs::write(&path, page)?;
        Ok(Some(Summary {
            name: self.name(),
            detail,
        }))
    }
}

fn list(page: &mut String, title: &str, entries: &[String]) -> Result<()> {
    if entries.is_empty() {
        return Ok(());
    }
    writeln!(page, "## {title} ({})\n", entries.len())?;
    for entry in entries.iter().take(SHOWN) {
        writeln!(page, "- `{entry}`")?;
    }
    if entries.len() > SHOWN {
        writeln!(page, "- … {} more", entries.len() - SHOWN)?;
    }
    writeln!(page)?;
    Ok(())
}
