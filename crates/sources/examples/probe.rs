use std::path::Path;
fn main() {
    let needle = std::env::args().nth(1).unwrap();
    let dir = Path::new(r"S:\Warframe\Downloaded\Public\Cache.Windows");
    for pkg in ["H.Misc", "B.Misc", "F.Misc"] {
        let a = sources::cache::Archive::open(&dir.join(format!("{pkg}.toc"))).unwrap();
        for e in a.entries() {
            let Ok(raw) = a.read(e) else { continue };
            let text = String::from_utf8_lossy(&raw);
            let hits: Vec<_> = text
                .match_indices(needle.as_str())
                .take(3)
                .map(|(i, _)| i)
                .collect();
            if !hits.is_empty() {
                let i = hits[0];
                let s = text[i.saturating_sub(200)..(i + 400).min(text.len())].replace('\0', " ");
                println!(
                    "== {pkg} {} ({} hits)\n{s}\n",
                    e.path,
                    text.matches(needle.as_str()).count()
                );
            }
        }
    }
}
