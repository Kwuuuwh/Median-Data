use std::fmt::Write;

use anyhow::Result;

use crate::projection::{Context, Projection, Summary};

/// A readable account of what moved since the last build, for review before publishing.
pub struct Changes;

const FILE: &str = "catalog.changes.md";
/// Long lists say nothing a sample does not; the full set lives in the report.
const SHOWN: usize = 40;

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

                if !diff.findings_delta.is_empty() {
                    writeln!(page, "## Findings\n")?;
                    for (rule, delta) in &diff.findings_delta {
                        writeln!(page, "- `{rule}` {delta:+}")?;
                    }
                    writeln!(page)?;
                }
                format!(
                    "items +{} -{}",
                    diff.items_added.len(),
                    diff.items_removed.len()
                )
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
