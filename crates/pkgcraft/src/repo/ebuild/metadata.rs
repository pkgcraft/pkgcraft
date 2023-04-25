use std::{fs, io};

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexSet;
use once_cell::sync::OnceCell;
use tracing::{error, warn};

#[derive(Debug, Default)]
pub struct Metadata {
    repo: String,
    profiles_base: Utf8PathBuf,
    arches: OnceCell<IndexSet<String>>,
    categories: OnceCell<IndexSet<String>>,
}

impl Metadata {
    pub(super) fn new<P: Into<Utf8PathBuf>>(repo: &str, path: P) -> Self {
        Self {
            repo: repo.to_string(),
            profiles_base: path.into(),
            ..Default::default()
        }
    }

    /// Return the full path to a repo's `profiles` directory.
    pub fn profiles_base(&self) -> &Utf8Path {
        &self.profiles_base
    }

    /// Return a repo's known architectures from `profiles/arch.list`.
    pub fn arches(&self) -> &IndexSet<String> {
        self.arches.get_or_init(|| {
            let path = self.profiles_base.join("arch.list");
            match fs::read_to_string(&path) {
                Ok(s) => s
                    .lines()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty() && !s.starts_with('#'))
                    .map(String::from)
                    .collect(),
                Err(e) => {
                    if e.kind() != io::ErrorKind::NotFound {
                        warn!("{}: failed reading {path:?}: {e}", self.repo);
                    }
                    IndexSet::new()
                }
            }
        })
    }

    /// Return a repo's configured categories from `profiles/categories`.
    pub fn categories(&self) -> &IndexSet<String> {
        self.categories.get_or_init(|| {
            let path = self.profiles_base.join("categories");
            match fs::read_to_string(&path) {
                Ok(s) => s
                    .lines()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty() && !s.starts_with('#'))
                    .map(String::from)
                    .collect(),
                Err(e) => {
                    if e.kind() != io::ErrorKind::NotFound {
                        warn!("{}: failed reading {path:?}: {e}", self.repo);
                    }
                    IndexSet::new()
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::repo::ebuild_temp::Repo as TempRepo;
    use crate::test::assert_unordered_eq;

    use super::*;

    #[test]
    fn test_arches() {
        let repo = TempRepo::new("test", None, None).unwrap();
        let mut metadata: Metadata;

        // nonexistent file
        metadata = Metadata::new("test", repo.path());
        assert!(metadata.arches().is_empty());

        // empty file
        metadata = Metadata::new("test", repo.path());
        fs::write(metadata.profiles_base().join("arch.list"), "").unwrap();
        assert!(metadata.arches().is_empty());

        // multiple
        let data = indoc::indoc! {r#"
            amd64
            arm64
            amd64-linux
        "#};
        metadata = Metadata::new("test", repo.path());
        fs::write(metadata.profiles_base().join("arch.list"), data).unwrap();
        assert_unordered_eq(metadata.arches(), ["amd64", "arm64", "amd64-linux"]);
    }
}
