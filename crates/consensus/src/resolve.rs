use crate::source::Source;

/// How a resolved value is backend by its sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Single,
    Confirmed,
    Conflict,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Single => "single",
            Status::Confirmed => "confirmed",
            Status::Conflict => "conflict",
        }
    }
}

/// One source's claim of a property value.
#[derive(Debug, Clone)]
pub struct Claim<T> {
    pub source: Source,
    pub value: T,
}

/// A value with its provenance.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved<T> {
    pub value: T,
    pub status: Status,
    /// The source whose claim was kept.
    pub winner: Source,
    /// Every source that claimed anything, in the order they were asked.
    pub sources: Vec<Source>,
}

/// Merge claims for one property. `priority` orders sources; the earliest present
/// wins a conflict. `None` when there are no claims.
pub fn resolve<T: Clone + PartialEq>(
    claims: &[Claim<T>],
    priority: &[Source],
) -> Option<Resolved<T>> {
    let winner = pick(claims, priority)?;
    let agree = claims.iter().all(|c| c.value == winner.value);

    let mut sources = Vec::new();
    for c in claims {
        if !sources.contains(&c.source) {
            sources.push(c.source);
        }
    }

    let status = if !agree {
        Status::Conflict
    } else if sources.len() >= 2 {
        Status::Confirmed
    } else {
        Status::Single
    };
    Some(Resolved {
        value: winner.value.clone(),
        status,
        winner: winner.source,
        sources,
    })
}

fn pick<'a, T>(claims: &'a [Claim<T>], priority: &[Source]) -> Option<&'a Claim<T>> {
    for &s in priority {
        if let Some(c) = claims.iter().find(|c| c.source == s) {
            return Some(c);
        }
    }
    claims.first()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim(source: Source, value: &str) -> Claim<String> {
        Claim {
            source,
            value: value.into(),
        }
    }

    #[test]
    fn single_source() {
        let r = resolve(&[claim(Source::De, "a")], &[Source::De, Source::Wfm]).unwrap();
        assert_eq!(r.status, Status::Single);
        assert_eq!(r.value, "a");
    }

    #[test]
    fn two_sources_agree_is_confirmed() {
        let r = resolve(
            &[claim(Source::De, "a"), claim(Source::Wfm, "a")],
            &[Source::De, Source::Wfm],
        )
        .unwrap();
        assert_eq!(r.status, Status::Confirmed);
    }

    #[test]
    fn disagreement_is_conflict_priority_wins() {
        let r = resolve(
            &[claim(Source::Wfm, "b"), claim(Source::De, "a")],
            &[Source::De, Source::Wfm],
        )
        .unwrap();
        assert_eq!(r.status, Status::Conflict);
        assert_eq!(r.value, "a");
        assert_eq!(r.winner, Source::De);
        assert_eq!(r.sources, vec![Source::Wfm, Source::De]);
    }

    #[test]
    fn the_winner_is_the_source_kept_not_the_first_asked() {
        let r = resolve(
            &[claim(Source::Wfm, "a"), claim(Source::Curated, "a")],
            &[Source::Curated, Source::De, Source::Wfm],
        )
        .unwrap();
        assert_eq!(r.status, Status::Confirmed);
        assert_eq!(r.winner, Source::Curated);
    }
}
