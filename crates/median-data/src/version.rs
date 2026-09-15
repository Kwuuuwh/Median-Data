use funnel::Diff;

/// The catalog's version, in three parts and none of them written by hand.
///
/// MAJOR is the only part an application reads: it says whether the file can be opened at all,
/// so it comes from the schema constant and from nothing else. MINOR and PATCH are read off the
/// funnel's own diff, which already knows whether items came and went — so a build that only
/// fixes a translation cannot claim to have added anything.
pub fn next(schema: u32, previous: Option<&str>, diff: Option<&Diff>, reshaped: bool) -> String {
    let Some((major, minor, patch)) = previous.and_then(parse) else {
        return format!("{schema}.0.0");
    };
    if major != schema {
        return format!("{schema}.0.0");
    }
    match diff {
        Some(d) if moved(d) => format!("{major}.{}.0", minor + 1),
        Some(d) if !d.is_empty() => format!("{major}.{minor}.{}", patch + 1),
        _ if reshaped => format!("{major}.{minor}.{}", patch + 1),
        _ => format!("{major}.{minor}.{patch}"),
    }
}

/// Whether the catalog gained or lost things, rather than only describing them differently.
fn moved(diff: &Diff) -> bool {
    !diff.items_added.is_empty()
        || !diff.items_removed.is_empty()
        || !diff.sets_added.is_empty()
        || !diff.sets_removed.is_empty()
}

fn parse(version: &str) -> Option<(u32, u32, u32)> {
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    parts.next().is_none().then_some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_items() -> Diff {
        Diff {
            items_added: vec!["/Lotus/New".into()],
            ..Diff::default()
        }
    }

    fn with_findings() -> Diff {
        Diff {
            findings_delta: [("dead-recipe".to_string(), -1)].into_iter().collect(),
            ..Diff::default()
        }
    }

    #[test]
    fn the_first_build_starts_at_the_schema() {
        assert_eq!(next(3, None, None, false), "3.0.0");
    }

    #[test]
    fn a_new_schema_resets_the_rest() {
        assert_eq!(next(4, Some("3.7.2"), Some(&with_items()), false), "4.0.0");
    }

    #[test]
    fn items_arriving_is_a_minor_release() {
        assert_eq!(next(3, Some("3.7.2"), Some(&with_items()), false), "3.8.0");
        assert_eq!(next(3, Some("3.7.2"), Some(&with_items()), true), "3.8.0");
    }

    #[test]
    fn anything_else_is_a_patch() {
        assert_eq!(
            next(3, Some("3.7.2"), Some(&with_findings()), false),
            "3.7.3"
        );
    }

    #[test]
    fn a_build_made_by_other_rules_is_a_patch_even_when_nothing_moved() {
        assert_eq!(
            next(3, Some("3.7.2"), Some(&Diff::default()), true),
            "3.7.3"
        );
        assert_eq!(next(3, Some("3.7.2"), None, true), "3.7.3");
    }

    #[test]
    fn a_rebuild_that_changed_nothing_keeps_its_version() {
        assert_eq!(
            next(3, Some("3.7.2"), Some(&Diff::default()), false),
            "3.7.2"
        );
        assert_eq!(next(3, Some("3.7.2"), None, false), "3.7.2");
    }

    #[test]
    fn a_version_nobody_can_read_is_treated_as_a_first_build() {
        assert_eq!(next(3, Some("nonsense"), None, false), "3.0.0");
        assert_eq!(next(3, Some("3.7"), None, false), "3.0.0");
    }
}
