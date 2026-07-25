/// Where a claim came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// A person decided it. Outranks everything a machine derived.
    Curated,
    De,
    /// The community wiki. Written by people, so it never outranks DE; it witnesses what DE
    /// says and supplies what DE does not export at all.
    Wiki,
    Wfm,
    Rule,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Curated => "curated",
            Source::De => "de",
            Source::Wiki => "wiki",
            Source::Wfm => "wfm",
            Source::Rule => "rule",
        }
    }
}
