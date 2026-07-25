use std::collections::BTreeMap;

use anyhow::Result;
use sources::wiki::{self, File};

/// Endings the wiki gives a subject's own picture. Anything else on the page belongs to
/// something else — a floof, a token, a mod card — so a name that does not end this way is
/// left alone rather than guessed at.
const ENDINGS: [&str; 8] = [
    "", "1sh", "portrait", "icon", "sigil", "logo", "flag", "promo",
];

/// The picture chosen for one vendor.
pub struct Portrait {
    pub vendor: String,
    pub file: File,
}

/// Find a picture for each vendor, by the name of the wiki page they are described on. Asked
/// for by title first, which is one batched call for every vendor at once; only the ones that
/// answer nothing cost a page listing of their own.
pub fn choose(agent: &ureq::Agent, wanted: &[(String, String)]) -> Result<Vec<Portrait>> {
    let mut titles = Vec::new();
    for (_, page) in wanted {
        titles.extend(named(page));
    }
    let held: BTreeMap<String, File> = wiki::files(agent, &titles)?
        .into_iter()
        .map(|f| (f.name.clone(), f))
        .collect();

    let mut out = Vec::new();
    for (vendor, page) in wanted {
        let direct = named(page)
            .into_iter()
            .find_map(|title| held.get(title.trim_start_matches("File:")).cloned());
        let file = match direct {
            Some(file) => Some(file),
            None => pick(page, &wiki::page_files(agent, page)?),
        };
        if let Some(file) = file {
            out.push(Portrait {
                vendor: vendor.clone(),
                file,
            });
        }
    }
    Ok(out)
}

/// The titles a subject's picture is usually filed under.
fn named(page: &str) -> Vec<String> {
    let plain = page.replace(' ', "");
    [
        format!("File:{page}.png"),
        format!("File:{page}.jpg"),
        format!("File:{plain}.png"),
        format!("File:{plain}.jpg"),
        format!("File:{page} 1SH.png"),
    ]
    .into_iter()
    .collect()
}

/// The one file on a page that is the page's own subject: its name is the subject's, give or
/// take an ending the wiki uses for portraits and emblems.
fn pick(page: &str, files: &[File]) -> Option<File> {
    let subject = fold(page);
    let mut best: Option<(usize, &File)> = None;
    for file in files {
        let stem = fold(
            file.name
                .rsplit_once('.')
                .map_or(file.name.as_str(), |(s, _)| s),
        );
        let Some(tail) = stem.strip_prefix(&subject) else {
            continue;
        };
        let Some(rank) = ENDINGS.iter().position(|e| *e == tail) else {
            continue;
        };
        if best.is_none_or(|(seen, _)| rank < seen) {
            best = Some((rank, file));
        }
    }
    best.map(|(_, file)| file.clone())
}

/// Collapse a name to letters and digits, so `Baro Ki'Teer` and `BaroKi'Teer` meet.
fn fold(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str) -> File {
        File {
            name: name.to_string(),
            url: format!("https://wiki/{name}"),
            width: 100,
            height: 200,
        }
    }

    #[test]
    fn the_subjects_own_picture_wins() {
        let files = [file("OtakTokenOrnament.png"), file("Otak.jpg")];
        assert_eq!(pick("Otak", &files).unwrap().name, "Otak.jpg");
    }

    #[test]
    fn an_emblem_answers_when_there_is_no_portrait() {
        let files = [file("CephalonSudaSigil.png"), file("CephalonSudaFlag.png")];
        assert_eq!(
            pick("Cephalon Suda", &files).unwrap().name,
            "CephalonSudaSigil.png"
        );
    }

    #[test]
    fn a_toy_named_after_the_subject_is_not_their_picture() {
        let files = [file("BaroKi'TeerFloof.png"), file("BaroKi'TeerGlyph.png")];
        assert!(pick("Baro Ki'Teer", &files).is_none());
    }

    #[test]
    fn spacing_and_punctuation_do_not_hide_a_match() {
        let files = [file("Fisher Hai-Luk 1SH.png")];
        assert_eq!(
            pick("Fisher Hai-Luk", &files).unwrap().name,
            "Fisher Hai-Luk 1SH.png"
        );
    }
}
