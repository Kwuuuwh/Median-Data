use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Directories the build is made of besides its sources.
const INPUTS: [&str; 2] = ["config", "crates"];

/// Lockfile of the build's dependencies.
const LOCK: &str = "Cargo.lock";

/// Crate that takes no part in the build.
const IDLE: &str = "crates/studio/";

/// Fingerprint of the rules and code the catalog is built by.
pub fn fingerprint(root: &Path) -> Result<String> {
    let mut files = Vec::new();
    for input in INPUTS {
        collect(&root.join(input), &mut files)?;
    }
    files.push(root.join(LOCK));

    let mut named: Vec<(String, PathBuf)> = files
        .into_iter()
        .filter(|file| file.is_file())
        .filter_map(|file| {
            let name = file
                .strip_prefix(root)
                .ok()?
                .to_string_lossy()
                .replace('\\', "/");
            (!name.starts_with(IDLE)).then_some((name, file))
        })
        .collect();
    named.sort();

    let mut hasher = blake3::Hasher::new();
    for (name, file) in named {
        let bytes = fs::read(&file).with_context(|| format!("read {}", file.display()))?;
        hasher.update(name.as_bytes());
        hasher.update(&[0]);
        hasher.update(&unix(&bytes));
        hasher.update(&[0]);
    }
    Ok(format!("recipe-{}", &hasher.finalize().to_hex()[..16]))
}

/// Every file under `dir`, however deep.
fn collect(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).with_context(|| format!("list {}", dir.display()))? {
        let path = entry?.path();
        match path.is_dir() {
            true => collect(&path, files)?,
            false => files.push(path),
        }
    }
    Ok(())
}

/// The bytes with every `\r\n` read as `\n`.
fn unix(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for (at, byte) in bytes.iter().enumerate() {
        if *byte == b'\r' && bytes.get(at + 1) == Some(&b'\n') {
            continue;
        }
        out.push(*byte);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Throwaway checkout in the temp directory.
    struct Checkout(PathBuf);

    impl Checkout {
        fn new(name: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("median-recipe-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            let checkout = Checkout(root);
            checkout.write("config/taxonomy.toml", "kind = \"resource\"\n");
            checkout.write("crates/graph/src/lib.rs", "pub fn graph() {}\n");
            checkout.write("crates/studio/src/lib.rs", "pub fn page() {}\n");
            checkout.write("Cargo.lock", "version = 4\n");
            checkout
        }

        fn write(&self, name: &str, text: &str) {
            let path = self.0.join(name);
            fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
            fs::write(path, text).expect("written");
        }

        fn fingerprint(&self) -> String {
            fingerprint(&self.0).expect("a fingerprint")
        }
    }

    impl Drop for Checkout {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_changed_rule_changes_the_fingerprint() {
        let checkout = Checkout::new("rule");
        let before = checkout.fingerprint();

        checkout.write("config/taxonomy.toml", "kind = \"railjack-resource\"\n");

        assert_ne!(checkout.fingerprint(), before);
    }

    #[test]
    fn a_changed_build_crate_changes_the_fingerprint() {
        let checkout = Checkout::new("code");
        let before = checkout.fingerprint();

        checkout.write("crates/graph/src/lib.rs", "pub fn graph() { let _ = 1; }\n");

        assert_ne!(checkout.fingerprint(), before);
    }

    #[test]
    fn the_studio_and_line_endings_leave_it_alone() {
        let checkout = Checkout::new("idle");
        let before = checkout.fingerprint();

        checkout.write("crates/studio/src/lib.rs", "pub fn page() { let _ = 2; }\n");
        checkout.write("config/taxonomy.toml", "kind = \"resource\"\r\n");

        assert_eq!(checkout.fingerprint(), before);
    }
}
